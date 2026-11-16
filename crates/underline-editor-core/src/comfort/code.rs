//! UC-005 BR-019, BR-021: where the text is code, so close pairs writes no
//! inline markup there and Tab finds no list item there: the lines of fenced
//! blocks (Markdown ```` ``` ```` and `~~~`; AsciiDoc `----`, `....`, `++++`
//! and `////`), and a code span (`` `…` ``) of the line before the cursor.
//!
//! A light scan of the lines, without parsing the note, so it costs little
//! per key; indented code is not told apart from the lines of a list item.

use ropey::Rope;
use underline_markup::NoteFormat;

use super::{line_of, line_start, line_text};

/// The fence a line opens or closes: its char and how many.
fn fence(line: &str, format: NoteFormat) -> Option<(char, usize)> {
    match format {
        NoteFormat::Markdown => {
            let indent = line.chars().take_while(|c| *c == ' ').count();
            if indent > 3 {
                return None;
            }
            let rest = &line[indent..];
            let c = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
            let count = rest.chars().take_while(|x| *x == c).count();
            (count >= 3).then_some((c, count))
        }
        NoteFormat::AsciiDoc => {
            let trimmed = line.trim_end();
            let c = trimmed
                .chars()
                .next()
                .filter(|c| matches!(c, '-' | '.' | '+' | '/'))?;
            let count = trimmed.chars().count();
            (count >= 4 && trimmed.chars().all(|x| x == c)).then_some((c, count))
        }
    }
}

/// For each line from the first to `last`, whether it belongs to a fenced
/// block (its fences included).
#[must_use]
pub fn code_lines(rope: &Rope, format: NoteFormat, last: usize) -> Vec<bool> {
    let last = last.min(rope.len_lines().saturating_sub(1));
    let mut mask = Vec::with_capacity(last + 1);
    let mut open: Option<(char, usize)> = None;
    for line in 0..=last {
        let text = line_text(rope, line);
        match (open, fence(&text, format)) {
            (None, Some(found)) => {
                open = Some(found);
                mask.push(true);
            }
            (Some((c, count)), Some((d, n))) if c == d && closes(format, count, n) => {
                // A Markdown closing fence holds no info string.
                let bare = format == NoteFormat::AsciiDoc || text.trim().chars().all(|x| x == c);
                if bare {
                    open = None;
                }
                mask.push(true);
            }
            (Some(_), _) => mask.push(true),
            (None, None) => mask.push(false),
        }
    }
    mask
}

/// Whether a fence of `n` chars closes one of `count`.
fn closes(format: NoteFormat, count: usize, n: usize) -> bool {
    match format {
        NoteFormat::Markdown => n >= count,
        NoteFormat::AsciiDoc => n == count,
    }
}

/// BR-019: whether char `at` of `rope` is in code: in a fenced block, or
/// after an odd number of backticks on its line (a code span still open).
#[must_use]
pub fn in_code(rope: &Rope, at: usize, format: NoteFormat) -> bool {
    let at = at.min(rope.len_chars());
    let line = line_of(rope, at);
    if code_lines(rope, format, line)
        .last()
        .copied()
        .unwrap_or(false)
    {
        return true;
    }
    let before = rope.slice(line_start(rope, line)..at).to_string();
    let mut ticks = 0;
    let mut escaped = false;
    for c in before.chars() {
        match c {
            '\\' if !escaped => {
                escaped = true;
                continue;
            }
            '`' if !escaped => ticks += 1,
            _ => {}
        }
        escaped = false;
    }
    ticks % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: NoteFormat = NoteFormat::Markdown;
    const ADOC: NoteFormat = NoteFormat::AsciiDoc;

    fn at(text: &str, marker: &str) -> (Rope, usize) {
        let byte = text.find(marker).expect("the marker");
        let rope = Rope::from_str(text);
        let at = rope.byte_to_char(byte);
        (rope, at)
    }

    /// BR-019: in Markdown, fenced blocks and open code spans are code.
    #[test]
    fn br019_markdown_fences_and_code_spans_are_code() {
        let text = "a *b*\n```rust\nlet x = 1;\n```\nafter `co|de` and |end\n";
        let (rope, inside) = at(text, "let");
        assert!(in_code(&rope, inside, MD));
        let (rope, after) = at(text, "after");
        assert!(!in_code(&rope, after, MD));
        let (rope, span) = at(text, "|de");
        assert!(in_code(&rope, span, MD));
        let (rope, end) = at(text, "|end");
        assert!(!in_code(&rope, end, MD), "the span is closed");
        assert_eq!(
            code_lines(&rope, MD, 9),
            [false, true, true, true, false, false]
        );
    }

    /// BR-019: a longer fence is closed only by one as long; an escaped
    /// backtick opens no span.
    #[test]
    fn br019_fence_lengths_and_escaped_backticks() {
        let text = "````\n```\nstill\n````\nout \\` here\n";
        let (rope, still) = at(text, "still");
        assert!(in_code(&rope, still, MD));
        let (rope, here) = at(text, "here");
        assert!(!in_code(&rope, here, MD));
        // An unclosed fence runs to the end.
        let (rope, end) = at("~~~\nlast", "last");
        assert!(in_code(&rope, end, MD));
    }

    /// BR-019: AsciiDoc's listing, literal and passthrough blocks are code.
    #[test]
    fn br019_asciidoc_delimited_blocks_are_code() {
        let text = "= T\n\n----\nlisting\n----\n\n....\nliteral\n....\nplain é\n";
        for marker in ["listing", "literal"] {
            let (rope, inside) = at(text, marker);
            assert!(in_code(&rope, inside, ADOC), "{marker}");
        }
        let (rope, plain) = at(text, "plain");
        assert!(!in_code(&rope, plain, ADOC));
        let (rope, list) = at("* item\n", "item");
        assert!(!in_code(&rope, list, ADOC));
    }
}
