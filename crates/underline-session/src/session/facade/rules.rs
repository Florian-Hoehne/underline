//! UC-005, M-09.10 to M-09.12: the editing rules of the facade that need
//! neither the session nor the text's history: escaping typed text,
//! whether typed AsciiDoc text keeps its block, the marker of the next list
//! item, the empty lines before the cursor, and taking typed characters
//! into the blocks without parsing again.
//!
//! Ported from `crates/underline-editor/src/facade_view.rs`
//! (`escape_markdown`, `asciidoc_plain`, `next_marker`,
//! `line_breaks_before`) and from `insert_typed` of
//! `crates/underline-facade/src/blocks.rs`, with defect D-18 fixed: the
//! source offsets of the cells of a table move with the text too (A-02).

use ropey::Rope;
use underline_facade::blocks::{Block, BlockKind, Run};

/// M-09.10: escapes the characters Markdown would read as formatting:
/// `\ * _ ` [ ] < > ~`, and at the start of a block also `# - + |`.
pub(super) fn escape_markdown(text: &str, at_block_start: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.chars().enumerate() {
        let start = at_block_start && i == 0;
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '~')
            || (start && matches!(c, '#' | '-' | '+' | '|'))
        {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Whether typing `text` between the characters `before` and `after` of an
/// AsciiDoc note keeps the blocks as they are: no character that could
/// start markup, and plain text or spaces on both sides, so no formatting
/// begins or ends there.
pub(super) fn asciidoc_plain(before: Option<char>, after: Option<char>, text: &str) -> bool {
    let plain = |c: char| c.is_alphanumeric() || c == ' ';
    text.chars().all(|c| plain(c) || ",;!?".contains(c))
        && before.is_some_and(plain)
        && after.is_none_or(|c| plain(c) || c == '\n')
}

/// M-09.11: the marker of the next list item after the item line `line`:
/// same indent, same bullet (`**` in AsciiDoc for the second level), the
/// next number, and an open task box for tasks.
pub(super) fn next_marker(line: &str) -> String {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &line[indent.len()..];
    let task = |after: &str| {
        let boxes = ["[ ] ", "[x] ", "[X] ", "[*] "];
        if boxes.iter().any(|b| after.starts_with(b)) {
            "[ ] "
        } else {
            ""
        }
    };
    if let Some(bullet) = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '-' | '*' | '+' | '.'))
    {
        let count = rest.chars().take_while(|&c| c == bullet).count();
        let marker = &rest[..count];
        let after = rest.get(count + 1..).unwrap_or("");
        return format!("{indent}{marker} {}", task(after));
    }
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if let Ok(number) = digits.parse::<usize>() {
        let delimiter = rest[digits.len()..].chars().next().unwrap_or('.');
        let after = rest.get(digits.len() + 2..).unwrap_or("");
        return format!(
            "{indent}{}{delimiter} {}",
            number.saturating_add(1),
            task(after)
        );
    }
    format!("{indent}- ")
}

/// The line breaks right before char `at` of `rope`, with only spaces
/// between them; two or more mean a blank line, where a paragraph starts.
pub(super) fn line_breaks_before(rope: &Rope, at: usize) -> usize {
    let mut chars = rope.chars_at(at.min(rope.len_chars()));
    let mut breaks = 0;
    while let Some(c) = chars.prev() {
        match c {
            '\n' => breaks += 1,
            ' ' | '\t' | '\r' => {}
            _ => break,
        }
    }
    breaks
}

/// The source offset right after the last thing block `block` covers: its
/// span or its last run, whichever ends later.
pub(super) fn ext_end(block: &Block) -> usize {
    block.span.end.max(block.runs.last().map_or(0, |r| r.end))
}

/// The number of characters the facade shows of `block`.
pub(super) fn shown_chars(block: &Block) -> usize {
    block.runs.iter().map(|r| r.offsets.len()).sum()
}

/// Every source offset of `runs` at or after `at` moves by `by`.
fn shift_runs(runs: &mut [Run], at: usize, by: isize) {
    for run in runs {
        for offset in &mut run.offsets {
            if *offset >= at {
                *offset = offset.saturating_add_signed(by);
            }
        }
        if run.end >= at {
            run.end = run.end.saturating_add_signed(by);
        }
    }
}

/// Every source offset of `block` at or after `at` moves by `by`: its span,
/// its runs, and the runs of its table cells (D-18).
pub(super) fn shift_block(block: &mut Block, at: usize, by: isize) {
    if block.span.start >= at {
        block.span.start = block.span.start.saturating_add_signed(by);
    }
    if block.span.end >= at {
        block.span.end = block.span.end.saturating_add_signed(by);
    }
    shift_runs(&mut block.runs, at, by);
    if let BlockKind::Table(table) = &mut block.kind {
        for row in &mut table.rows {
            for cell in row {
                shift_runs(cell, at, by);
            }
        }
    }
}

/// Takes note of `raw` inserted at source offset `at` for facade text `text`
/// in block `index`, before facade character `facade_at`, without parsing
/// again (ADR-012, ADR-024): the characters join the run they were typed
/// into, and every source position after `at` moves by the length of `raw`,
/// those of table cells included (D-18).
///
/// Only valid for an insertion that cannot change the structure of the
/// blocks: no line break, and formatting characters escaped. Returns `false`
/// when the insertion does not fit the block; then the caller parses again.
pub(super) fn insert_typed(
    blocks: &mut [Block],
    index: usize,
    facade_at: usize,
    at: usize,
    text: &str,
    raw: &str,
) -> bool {
    let Some(block) = blocks.get(index) else {
        return false;
    };
    if text.contains('\n') || block.source_offset(facade_at) != at || block.runs.is_empty() {
        return false;
    }
    // Source offsets of the new characters: escapes stand before them.
    let mut new_offsets = Vec::new();
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            if let Some(&(j, next)) = chars.peek() {
                if next.is_ascii_punctuation() {
                    new_offsets.push(at + j);
                    chars.next();
                    continue;
                }
            }
        }
        new_offsets.push(at + i);
    }
    if new_offsets.len() != text.chars().count() {
        return false;
    }
    let shift = raw.len();
    let by = isize::try_from(shift).unwrap_or(isize::MAX);
    for block in blocks.iter_mut() {
        // A block that ends before `at` keeps its offsets.
        if block.span.end < at && block.runs.last().is_none_or(|r| r.end < at) {
            continue;
        }
        let empty_at = block.span.start == at && block.span.start == block.span.end;
        let start = block.span.start;
        shift_block(block, at, by);
        if empty_at {
            block.span.start = start;
        }
    }
    // The run that holds facade character `facade_at`, or the last one.
    let block = &mut blocks[index];
    if block.span.start > at {
        block.span.start = at;
    }
    let mut rest = facade_at;
    let count = block.runs.len();
    for (i, run) in block.runs.iter_mut().enumerate() {
        let len = run.offsets.len();
        if rest < len || (rest == len && i + 1 == count) {
            let byte = run
                .text
                .char_indices()
                .nth(rest)
                .map_or(run.text.len(), |(b, _)| b);
            run.text.insert_str(byte, text);
            run.offsets.splice(rest..rest, new_offsets);
            if rest == len {
                run.end = at + shift;
            }
            return true;
        }
        rest -= len;
    }
    false
}

/// Markdown: whether `text` holds a line that may be a link reference
/// definition (`[label]: target`), which links anywhere in the note use;
/// such a note is parsed whole after an edit, as only that sees them all.
pub(super) fn has_reference_definitions(text: &str) -> bool {
    text.match_indices("]:").any(|(at, _)| {
        let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
        let line = &text[line_start..at];
        let trimmed = line.trim_start_matches(' ');
        line.len() - trimmed.len() <= 3 && trimmed.starts_with('[')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use underline_core::NoteFormat;
    use underline_facade::blocks::parse_blocks;

    #[test]
    fn uc005_m09_10_markdown_escapes_formatting_and_block_start_markers() {
        assert_eq!(escape_markdown("a*b_c`d", false), "a\\*b\\_c\\`d");
        assert_eq!(escape_markdown("[x]<y>~\\", false), "\\[x\\]\\<y\\>\\~\\\\");
        assert_eq!(escape_markdown("# not", true), "\\# not");
        assert_eq!(escape_markdown("-", true), "\\-");
        assert_eq!(escape_markdown("+|", true), "\\+|");
        assert_eq!(escape_markdown("#-+|", false), "#-+|");
        assert_eq!(escape_markdown("🙂日本", true), "🙂日本");
    }

    #[test]
    fn uc005_m09_10_asciidoc_plain_text_between_plain_characters() {
        assert!(asciidoc_plain(Some('a'), Some('b'), "xy z,;!?"));
        assert!(asciidoc_plain(Some(' '), None, "x"));
        assert!(asciidoc_plain(Some('a'), Some('\n'), "x"));
        assert!(!asciidoc_plain(None, Some('b'), "x"));
        assert!(!asciidoc_plain(Some('*'), Some('b'), "x"));
        assert!(!asciidoc_plain(Some('a'), Some('_'), "x"));
        assert!(!asciidoc_plain(Some('a'), Some('b'), "*"));
    }

    #[test]
    fn uc005_m09_11_next_marker_keeps_indent_bullet_number_and_open_task() {
        assert_eq!(next_marker("- item"), "- ");
        assert_eq!(next_marker("  * item"), "  * ");
        assert_eq!(next_marker("** inner"), "** ");
        assert_eq!(next_marker(". first"), ". ");
        assert_eq!(next_marker("3. three"), "4. ");
        assert_eq!(next_marker("9) nine"), "10) ");
        assert_eq!(next_marker("- [x] done"), "- [ ] ");
        assert_eq!(next_marker("1. [ ] open"), "2. [ ] ");
        assert_eq!(next_marker("text"), "- ");
    }

    #[test]
    fn m09_11_line_breaks_before_count_blank_lines() {
        assert_eq!(line_breaks_before(&Rope::from_str("A\r\n\r\nB"), 5), 2);
        let rope = Rope::from_str("A\n\n  B\n\n\n\nC");
        assert_eq!(line_breaks_before(&rope, 3), 2);
        assert_eq!(line_breaks_before(&rope, 0), 0);
        assert_eq!(line_breaks_before(&rope, 2), 1);
        assert_eq!(line_breaks_before(&rope, 10), 4);
    }

    /// D-18: typing before a table moves the source offsets of its cells.
    #[test]
    fn d18_typing_before_a_table_moves_its_cell_offsets() {
        let text = "Para\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let mut blocks = parse_blocks(text, NoteFormat::Markdown);
        // "Para" → "Paxra": typed at facade index 2, source byte 2.
        assert!(insert_typed(&mut blocks, 0, 2, 2, "x", "x"));
        let typed = "Paxra\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(blocks, parse_blocks(typed, NoteFormat::Markdown));
    }

    #[test]
    fn uc005_m09_10_typed_text_joins_its_run_like_a_full_parse() {
        let text = "# Head\n\nSome *bold* text.\n\n- one\n";
        let mut blocks = parse_blocks(text, NoteFormat::Markdown);
        // After "bold" inside the emphasis: facade index 9 of the paragraph.
        let at = blocks[1].source_offset(9);
        assert!(insert_typed(&mut blocks, 1, 9, at, "*x", "\\*x"));
        let mut typed = text.to_owned();
        typed.insert_str(at, "\\*x");
        assert_eq!(blocks, parse_blocks(&typed, NoteFormat::Markdown));
    }

    #[test]
    fn markdown_reference_definitions_are_found() {
        assert!(has_reference_definitions("x\n[a]: http://x\n"));
        assert!(has_reference_definitions("   [a]: b"));
        assert!(!has_reference_definitions("a [b]: c"));
        assert!(!has_reference_definitions("    [a]: indented code"));
        assert!(!has_reference_definitions("plain"));
    }
}
