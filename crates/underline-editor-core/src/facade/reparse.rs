//! ADR-024: after an edit the facade parses only the blocks around it, not
//! the whole note (the egui facade parses the whole note after every Enter
//! and Backspace).
//!
//! The region parsed reaches from a block before the edit to a block after
//! it; list items, quotes and admonitions next to the edit come along whole,
//! as their numbers and depths depend on their neighbours. The region is
//! taken when the blocks before the edit come out of it exactly as they
//! were, and the block that ends it comes out as it was, moved by the edit;
//! otherwise it grows, up to the whole note. What the region gives in
//! between replaces the old blocks there. A Markdown note with link
//! reference definitions is parsed whole, as only that sees them all.

use ropey::Rope;
use underline_markup::blocks::{parse_blocks, parse_region, Block, BlockKind, Region};
use underline_markup::NoteFormat;

use super::rules::{byte_text, ext_end, has_reference_definitions, shift_block};

/// The most blocks a region may span before the whole note is parsed.
const MOST_BLOCKS: usize = 512;

/// The bytes one edit changed: `start..old_end` of the text before became
/// `start..new_end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) start: usize,
    pub(super) old_end: usize,
    pub(super) new_end: usize,
}

impl Change {
    /// How far the bytes after the change moved.
    fn delta(self) -> isize {
        isize::try_from(self.new_end).unwrap_or(isize::MAX)
            - isize::try_from(self.old_end).unwrap_or(isize::MAX)
    }
}

/// One change of the block list: `removed` blocks from `from` on became
/// `added` blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Spliced {
    pub(super) from: usize,
    pub(super) removed: usize,
    pub(super) added: usize,
}

/// The bytes that differ between `old` and `new`: the common start and end
/// are left out, at char boundaries. `None` when the texts are the same.
pub(super) fn changed_bytes(old: &Rope, new: &Rope) -> Option<Change> {
    let (old_len, new_len) = (old.len_bytes(), new.len_bytes());
    let mut prefix = common_prefix(old, new);
    if prefix == old_len && prefix == new_len {
        return None;
    }
    // At a char boundary of both texts (they agree up to `prefix`).
    prefix = old.char_to_byte(old.byte_to_char(prefix));
    let most = old_len.min(new_len) - prefix;
    let mut suffix = common_suffix(old, new, most);
    while suffix > 0 && !is_boundary(old, old_len - suffix) {
        suffix -= 1;
    }
    Some(Change {
        start: prefix,
        old_end: old_len - suffix,
        new_end: new_len - suffix,
    })
}

fn is_boundary(rope: &Rope, byte: usize) -> bool {
    rope.char_to_byte(rope.byte_to_char(byte)) == byte
}

/// How many bytes `a` and `b` share from their start, chunk by chunk.
fn common_prefix(a: &Rope, b: &Rope) -> usize {
    let mut left = a.chunks();
    let mut right = b.chunks();
    let (mut x, mut y): (&[u8], &[u8]) = (&[], &[]);
    let mut same = 0;
    loop {
        if x.is_empty() {
            match left.next() {
                Some(chunk) => x = chunk.as_bytes(),
                None => return same,
            }
        }
        if y.is_empty() {
            match right.next() {
                Some(chunk) => y = chunk.as_bytes(),
                None => return same,
            }
        }
        let n = x.len().min(y.len());
        if x[..n] == y[..n] {
            same += n;
            x = &x[n..];
            y = &y[n..];
        } else {
            return same + x.iter().zip(y).take_while(|(p, q)| p == q).count();
        }
    }
}

/// How many bytes `a` and `b` share at their end, at most `most`.
fn common_suffix(a: &Rope, b: &Rope, most: usize) -> usize {
    let (mut left, ..) = a.chunks_at_byte(a.len_bytes());
    let (mut right, ..) = b.chunks_at_byte(b.len_bytes());
    let (mut x, mut y): (&[u8], &[u8]) = (&[], &[]);
    let mut same = 0;
    while same < most {
        if x.is_empty() {
            match left.prev() {
                Some(chunk) => x = chunk.as_bytes(),
                None => break,
            }
        }
        if y.is_empty() {
            match right.prev() {
                Some(chunk) => y = chunk.as_bytes(),
                None => break,
            }
        }
        let n = x.len().min(y.len()).min(most - same);
        let (tx, ty) = (&x[x.len() - n..], &y[y.len() - n..]);
        if tx == ty {
            same += n;
            x = &x[..x.len() - n];
            y = &y[..y.len() - n];
        } else {
            let tail = tx
                .iter()
                .rev()
                .zip(ty.iter().rev())
                .take_while(|(p, q)| p == q)
                .count();
            return same + tail;
        }
    }
    same.min(most)
}

/// Blocks whose neighbours decide their numbers or depths.
fn is_container(kind: &BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::ListItem { .. } | BlockKind::Quote(_) | BlockKind::Admonition(_)
    )
}

/// `block` parsed from a region starting at byte `origin`, as it stands in
/// the note.
fn placed(mut block: Block, origin: usize) -> Block {
    shift_block(&mut block, 0, isize::try_from(origin).unwrap_or(isize::MAX));
    block
}

/// The blocks of `blocks`, which were those of the text `old` before
/// `change`, made those of `rope`, the text after it, parsing as little as
/// needed. `refs` says whether the note may hold link reference
/// definitions; it is set when the parsed region holds one.
///
/// Correctness before speed (FR-097, package 7a review): the blocks around
/// the edit are parsed alone only when the lines the edit touched, before
/// and after it, and the line on each side of them hold no line that can
/// reach beyond its block (`reaching_line`: fences, delimiters, front
/// matter, block attributes, reference definitions, headings, table rows,
/// HTML, an empty list marker, a `\r`), and not in the first block of the
/// note, where a front matter or a document header stands; else the whole
/// note is parsed.
pub(super) fn reparse(
    blocks: &mut Vec<Block>,
    old: &Rope,
    rope: &Rope,
    format: NoteFormat,
    change: Change,
    refs: &mut bool,
) -> Spliced {
    let markdown = format == NoteFormat::Markdown;
    if markdown && *refs {
        return parse_whole(blocks, rope, format, refs);
    }
    if !local_is_safe(blocks, old, rope, format, change) {
        return parse_whole(blocks, rope, format, refs);
    }
    let n = blocks.len();
    let delta = change.delta();
    let new_len = rope.len_bytes();
    let old_len = new_len.saturating_add_signed(-delta);
    // The blocks the edit touches.
    let first = blocks.partition_point(|b| ext_end(b) < change.start);
    let last = blocks
        .partition_point(|b| b.span.start <= change.old_end)
        .max(first);
    let mut lead = first;
    while lead > 0 && is_container(&blocks[lead - 1].kind) {
        lead -= 1;
    }
    let mut trail = last;
    while trail < n && is_container(&blocks[trail].kind) {
        trail += 1;
    }
    // FR-098, OQ-222: the notation the AsciiDoc header sets with `:stem:`,
    // which a part of the note parsed alone does not hold.
    let stem = if format == NoteFormat::AsciiDoc {
        underline_markup::stem_in_lines(lines_of(rope))
    } else {
        underline_markup::blocks::Stem::Off
    };
    let (mut before, mut after) = (1, 1);
    loop {
        let mut lo = lead.saturating_sub(before);
        if format == NoteFormat::AsciiDoc {
            // A document title is one only at the start of the note.
            while lo > 0 && matches!(blocks[lo].kind, BlockKind::Heading(1)) {
                lo -= 1;
            }
        }
        let hi = (trail + after).min(n);
        if hi - lo > MOST_BLOCKS {
            return parse_whole(blocks, rope, format, refs);
        }
        let r0 = if lo == 0 { 0 } else { blocks[lo].span.start };
        let r1_old = if hi == n {
            old_len
        } else {
            blocks[hi].span.start
        };
        let r1 = r1_old.saturating_add_signed(delta).min(new_len);
        // Block starts are char boundaries; `byte_text` keeps it so even
        // if a parser's span were not.
        let text = byte_text(rope, r0..r1);
        if markdown && has_reference_definitions(&text) {
            *refs = true;
            return parse_whole(blocks, rope, format, refs);
        }
        // A part of the note is no front matter of its own (UC-024 BR-001).
        let region = Region {
            at_start: r0 == 0,
            stem,
        };
        let parsed: Vec<Block> = parse_region(&text, format, &region)
            .into_iter()
            .map(|block| placed(block, r0))
            .collect();
        let whole = lo == 0 && hi == n;
        let kept_before = first - lo;
        let kept_after = hi - trail;
        let fits = parsed.len() >= kept_before + kept_after;
        let before_ok = fits && parsed[..kept_before] == blocks[lo..first];
        let after_ok = fits
            && blocks[trail..hi]
                .iter()
                .zip(&parsed[parsed.len() - kept_after..])
                .all(|(old, new)| {
                    let mut moved = old.clone();
                    shift_block(&mut moved, 0, delta);
                    moved == *new
                });
        if before_ok && after_ok {
            let middle: Vec<Block> = parsed[kept_before..parsed.len() - kept_after].to_vec();
            let added = middle.len();
            for block in &mut blocks[trail..] {
                shift_block(block, 0, delta);
            }
            blocks.splice(first..trail, middle);
            return Spliced {
                from: first,
                removed: trail - first,
                added,
            };
        }
        if whole || (!before_ok && lo == 0) || (!after_ok && hi == n) {
            if !whole {
                return parse_whole(blocks, rope, format, refs);
            }
            // The edit changed blocks before it too (a setext heading):
            // the region is the whole note.
            let added = parsed.len();
            *blocks = parsed;
            return Spliced {
                from: 0,
                removed: n,
                added,
            };
        }
        if !before_ok {
            before *= 4;
        }
        if !after_ok {
            after *= 4;
        }
    }
}

/// Parses the whole note again.
fn parse_whole(
    blocks: &mut Vec<Block>,
    rope: &Rope,
    format: NoteFormat,
    refs: &mut bool,
) -> Spliced {
    let text = rope.to_string();
    let removed = blocks.len();
    *blocks = parse_blocks(&text, format);
    if format == NoteFormat::Markdown {
        *refs = has_reference_definitions(&text);
    }
    Spliced {
        from: 0,
        removed,
        added: blocks.len(),
    }
}

/// ADR-025, FR-097: the blocks of `blocks`, which were those of the text
/// `old`, made those of `new`, parsing only the blocks around what changed
/// between them (one view of a note follows the edits of another, UC-005
/// BR-016). Returns the changed blocks by their indices now, `None` when the
/// texts are the same. Where parsing around the change could differ from a
/// full parse (see `reaching_line`), the whole note is parsed, and every
/// block is changed.
#[must_use = "the changed blocks are to be made anew"]
pub fn reparse_between(
    blocks: &mut Vec<Block>,
    old: &Rope,
    new: &Rope,
    format: NoteFormat,
    refs: &mut bool,
) -> Option<std::ops::Range<usize>> {
    let change = changed_bytes(old, new)?;
    let spliced = reparse(blocks, old, new, format, change, refs);
    Some(spliced.from..spliced.from + spliced.added)
}

/// The lines of `rope` as the parsers split a note: at `\n` only. The rope
/// also breaks a line at U+000B, U+000C, U+0085, U+2028, U+2029 and a lone
/// `\r`, which comrak and the AsciiDoc parser do not.
fn lines_of(rope: &Rope) -> impl Iterator<Item = String> + '_ {
    let mut chars = rope.chars().peekable();
    std::iter::from_fn(move || {
        chars.peek()?;
        let mut line = String::new();
        for c in chars.by_ref() {
            if c == '\n' {
                break;
            }
            line.push(c);
        }
        Some(line)
    })
}

/// Whether `line` (a line of the rope, its break included) ends or holds a
/// break the rope knows but the parsers do not (see [`lines_of`]).
fn holds_rope_only_break(line: &str) -> bool {
    line.contains(['\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{2029}'])
        || line.trim_end_matches(['\r', '\n']).contains('\r')
        || (line.ends_with('\r') && !line.ends_with("\r\n"))
}

/// The lines of `rope` from the one holding byte `from` to the one holding
/// byte `to`, with the line before and the line after them.
fn lines_around(rope: &Rope, from: usize, to: usize) -> std::ops::Range<usize> {
    let bytes = rope.len_bytes();
    let first = rope.byte_to_line(from.min(bytes));
    let last = rope.byte_to_line(to.min(bytes));
    first.saturating_sub(1)..(last + 2).min(rope.len_lines())
}

/// Whether parsing the blocks around `change` alone gives what a full
/// parse gives: no line touched (in `old` or in `rope`) nor next to it can
/// reach beyond its block, and the edit is not in the first block of the
/// note.
fn local_is_safe(
    blocks: &[Block],
    old: &Rope,
    rope: &Rope,
    format: NoteFormat,
    change: Change,
) -> bool {
    // A front matter or an AsciiDoc document header is the start of the note.
    if blocks.first().is_none_or(|b| change.start <= ext_end(b)) {
        return false;
    }
    let lines_ok = |text: &Rope, end: usize| {
        let edited = {
            let bytes = text.len_bytes();
            text.byte_to_line(change.start.min(bytes))..=text.byte_to_line(end.min(bytes))
        };
        lines_around(text, change.start, end).all(|line| {
            let line_text = text.line(line).to_string();
            // The rope's lines are the parsers' lines only without the
            // breaks the rope alone knows; with one, the note is parsed whole.
            if holds_rope_only_break(&line_text) {
                return false;
            }
            // An ATX heading next to the edit stays one whatever the line
            // beside it says.
            let heading_beside = format == NoteFormat::Markdown
                && !edited.contains(&line)
                && line_text.trim_start_matches(' ').starts_with('#');
            heading_beside || !reaching_line(&line_text, format)
        })
    };
    lines_ok(old, change.old_end) && lines_ok(rope, change.new_end)
}

/// FR-097, ADR-024: whether `line` may change blocks beyond its own: a
/// fence, a delimiter or thematic break, a front matter or setext line, a
/// table row, a heading or document title, a block attribute or anchor, an
/// attribute entry or directive, a reference definition, HTML, an empty
/// list marker, or a `\r`.
fn reaching_line(line: &str, format: NoteFormat) -> bool {
    if line.contains('\r') {
        return true;
    }
    let text = line.trim_end_matches(['\n', ' ', '\t']);
    let s = text.trim_start_matches([' ', '\t']);
    let Some(first) = s.chars().next() else {
        return false;
    };
    // FR-098: `$…$` and `$$…$$` are inline in comrak, so they never reach
    // beyond their paragraph; a paragraph of one display formula is a block
    // formula by its own text.
    if s.starts_with("```") || s.starts_with("~~~") || s.contains("]:") {
        return true;
    }
    if matches!(first, '#' | '=' | '[' | '<' | '|' | ':' | '.' | '/' | '+') {
        return true;
    }
    if format == NoteFormat::Markdown && s.contains('|') {
        return true;
    }
    if format == NoteFormat::AsciiDoc && (s.contains("::") || s.contains("===")) {
        return true;
    }
    // A line of one sign repeated (`---`, `***`, `___`, `....`, `--`).
    let signs: String = s.chars().filter(|c| *c != ' ').collect();
    if signs.len() >= 2
        && signs.chars().all(|c| c == first)
        && matches!(first, '-' | '*' | '_' | '~' | '`' | '=')
    {
        return true;
    }
    // A list marker with nothing after it.
    let marker = s.trim_start_matches(|c: char| c.is_ascii_digit());
    matches!(s, "-" | "*" | "+") || (marker.len() < s.len() && matches!(marker, "." | ")"))
}

/// The blocks of `text` and whether a Markdown note may hold link
/// reference definitions.
pub fn parse_all(text: &str, format: NoteFormat) -> (Vec<Block>, bool) {
    let refs = format == NoteFormat::Markdown && has_reference_definitions(text);
    (parse_blocks(text, format), refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-097, UC-005 BR-016: the blocks of one view follow the text
    /// another view changed, as a full parse gives them.
    #[test]
    fn fr097_reparse_between_two_texts_as_a_full_parse() {
        let old = "# Title\n\nFirst para.\n\n- one\n- two\n\nLast para.\n".repeat(50);
        let mut new = old.clone();
        new.insert_str(400, "more words ");
        let mut blocks = parse_blocks(&old, NoteFormat::Markdown);
        let mut refs = false;
        let dirty = reparse_between(
            &mut blocks,
            &Rope::from_str(&old),
            &Rope::from_str(&new),
            NoteFormat::Markdown,
            &mut refs,
        );
        assert!(dirty.is_some());
        assert_eq!(blocks, parse_blocks(&new, NoteFormat::Markdown));
        let same = Rope::from_str(&new);
        assert_eq!(
            reparse_between(&mut blocks, &same, &same, NoteFormat::Markdown, &mut refs),
            None
        );
    }

    /// Applies the replacement of `old[range]` by `insert` to the blocks of
    /// `old`, incrementally, and checks them against a full parse.
    fn check(old: &str, range: std::ops::Range<usize>, insert: &str, format: NoteFormat) {
        let mut blocks = parse_blocks(old, format);
        let mut new = old.to_owned();
        new.replace_range(range.clone(), insert);
        let rope = Rope::from_str(&new);
        let change = changed_bytes(&Rope::from_str(old), &rope).unwrap_or(Change {
            start: range.start,
            old_end: range.start,
            new_end: range.start,
        });
        let mut refs = has_reference_definitions(old);
        let spliced = reparse(
            &mut blocks,
            &Rope::from_str(old),
            &rope,
            format,
            change,
            &mut refs,
        );
        assert_eq!(
            blocks,
            parse_blocks(&new, format),
            "{old:?} [{range:?} → {insert:?}] spliced {spliced:?}"
        );
    }

    const NOTE: &str = "# Title\n\nFirst para with *em*.\n\n- one\n- two\n  - inner\n- three\n\n1. a\n2. b\n3. c\n\n> quote\n> more\n\n```rust\nfn x() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\nLast para.\n";

    /// ADR-024: an edit anywhere, parsed incrementally, gives the blocks of a
    /// full parse.
    #[test]
    fn adr024_incremental_reparse_equals_the_full_parse() {
        let at = |s: &str| NOTE.find(s).unwrap();
        // Enter in a paragraph, a list, an ordered list, a quote, code.
        check(
            NOTE,
            at("with") - 1..at("with") - 1,
            "\n\n",
            NoteFormat::Markdown,
        );
        check(
            NOTE,
            at("two") + 3..at("two") + 3,
            "\n- ",
            NoteFormat::Markdown,
        );
        check(
            NOTE,
            at("2. b") + 4..at("2. b") + 4,
            "\n3. ",
            NoteFormat::Markdown,
        );
        check(
            NOTE,
            at("quote") + 5..at("quote") + 5,
            "\n\n> ",
            NoteFormat::Markdown,
        );
        check(
            NOTE,
            at("fn x") + 6..at("fn x") + 6,
            "\n",
            NoteFormat::Markdown,
        );
        // Joining blocks, removing a break, removing a whole list item.
        check(NOTE, at("\n\nFirst")..at("First"), "", NoteFormat::Markdown);
        check(NOTE, at("---")..at("---") + 5, "", NoteFormat::Markdown);
        check(NOTE, at("- two")..at("- three"), "", NoteFormat::Markdown);
        // A fence opened: everything after it is code.
        check(NOTE, at("Last")..at("Last"), "```\n", NoteFormat::Markdown);
        check(
            NOTE,
            at("First")..at("First"),
            "```\n",
            NoteFormat::Markdown,
        );
        // Typing in a table, at the start and the end of the note.
        check(NOTE, at("| 1")..at("| 1") + 3, "| 10", NoteFormat::Markdown);
        check(NOTE, 0..0, "Lead\n\n", NoteFormat::Markdown);
        check(
            NOTE,
            NOTE.len()..NOTE.len(),
            "\n\nMore",
            NoteFormat::Markdown,
        );
        // A setext underline turns the paragraph before into a heading.
        check("A\n\nB\n\nC\n", 4..4, "\n===", NoteFormat::Markdown);
    }

    #[test]
    fn adr024_incremental_reparse_of_asciidoc_equals_the_full_parse() {
        let note = "= Doc\nAda\n\n== One\n\nText *b*.\n\n* a\n* b\n** c\n\n. x\n. y\n\n[source,rust]\n----\nfn a() {}\n----\n\nNOTE: Mind.\n\n|===\n| a | b\n|===\n\nEnd.\n";
        let at = |s: &str| note.find(s).unwrap();
        check(
            note,
            at("Text") + 4..at("Text") + 4,
            "\n\n",
            NoteFormat::AsciiDoc,
        );
        check(
            note,
            at("* b") + 3..at("* b") + 3,
            "\n* ",
            NoteFormat::AsciiDoc,
        );
        check(
            note,
            at(". x") + 3..at(". x") + 3,
            "\n. ",
            NoteFormat::AsciiDoc,
        );
        check(
            note,
            at("fn a") + 3..at("fn a") + 3,
            "\n",
            NoteFormat::AsciiDoc,
        );
        check(note, at("\n\nText")..at("Text"), "", NoteFormat::AsciiDoc);
        check(note, at("End")..at("End"), "----\n", NoteFormat::AsciiDoc);
        check(
            note,
            at("Ada") + 3..at("Ada") + 3,
            "\n:toc:",
            NoteFormat::AsciiDoc,
        );
    }

    #[test]
    fn adr024_reference_definitions_parse_the_whole_note() {
        check(
            "[a] b\n\nc\n\n[a]: http://x\n",
            6..6,
            "x",
            NoteFormat::Markdown,
        );
        check(
            "[a] b\n\nc\n",
            9..9,
            "\n\n[a]: http://x",
            NoteFormat::Markdown,
        );
    }

    #[test]
    fn adr024_crlf_and_emoji_edits_parse_like_the_whole_note() {
        let note = "A 🙂\r\n\r\nB 日本\r\n\r\n- x\r\n- y\r\n";
        let at = |s: &str| note.find(s).unwrap();
        check(
            note,
            at("B") + 1..at("B") + 1,
            "\r\n\r\n",
            NoteFormat::Markdown,
        );
        check(note, at("\r\n\r\nB")..at("B"), "", NoteFormat::Markdown);
        check(
            note,
            at("y") + 1..at("y") + 1,
            "\r\n- ",
            NoteFormat::Markdown,
        );
    }

    #[test]
    fn changed_bytes_finds_the_edit_at_char_boundaries() {
        let old = Rope::from_str("ab🙂cd");
        let new = Rope::from_str("ab🙃cd");
        assert_eq!(
            changed_bytes(&old, &new),
            Some(Change {
                start: 2,
                old_end: 6,
                new_end: 6
            })
        );
        assert_eq!(changed_bytes(&old, &old.clone()), None);
        let big: String = "line of text\n".repeat(5000);
        let mut edited = big.clone();
        edited.insert_str(30_000, "XY");
        assert_eq!(
            changed_bytes(&Rope::from_str(&big), &Rope::from_str(&edited)),
            Some(Change {
                start: 30_000,
                old_end: 30_000,
                new_end: 30_002
            })
        );
        // Repeated text: the change is somewhere in the repetition.
        let change = changed_bytes(&Rope::from_str("aaaa"), &Rope::from_str("aaa")).unwrap();
        assert_eq!(
            (change.old_end - change.start, change.new_end - change.start),
            (1, 0)
        );
    }

    /// UC-024 BR-001, OQ-216: edits in and after a front matter, and after
    /// a rule that looks like the start of one, parse as the whole note.
    #[test]
    fn uc024_front_matter_reparsed() {
        let note = "---\nstatus: draft\n---\n# Plan\n\nText\n\n---\na: b\n---\n\nEnd\n";
        for at in [5, 13, 22, 30, 36, 40, 45] {
            check(note, at..at, "x", NoteFormat::Markdown);
            check(note, at..at + 1, "", NoteFormat::Markdown);
        }
        // Breaking and mending the front matter.
        check(note, 4..5, ":", NoteFormat::Markdown);
        check(note, 0..1, "", NoteFormat::Markdown);
    }

    /// A tiny deterministic generator, so a failing seed can be replayed.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as usize
        }

        fn below(&mut self, n: usize) -> usize {
            self.next() % n
        }
    }

    const PIECES: &[&str] = &[
        "# H\n",
        "## Sub\n",
        "text *em* here",
        "\n",
        "\n\n",
        "\r\n",
        "- item\n",
        "  - inner\n",
        "1. one\n",
        "> quote\n",
        "```\n",
        "```rust\n",
        "~~~\n",
        "---\n",
        "| a | b |\n",
        "|---|---|\n",
        "| 1 | 2 |\n",
        "[x]: http://e.org\n",
        "[x]",
        "\u{1F600}",
        "\u{e9}",
        "= Doc\n",
        "== Two\n",
        "----\n",
        "|===\n",
        "NOTE: n\n",
        "* a\n",
        ". x\n",
        "[source]\n",
        "    code\n",
        "<div>\n",
        "$$\n",
        "$x$ ",
        "$`y`$",
        "$$z$$",
        "```math\n",
        ":stem: latexmath\n",
        "stem:[a]",
        "latexmath:[b]",
        "[latexmath]\n",
        "[stem]\n",
        // OQ-222: AsciiMath.
        ":stem:\n",
        "asciimath:[x/y]",
        "[asciimath]\n",
        "++++\n",
        "word ",
        // Breaks the rope knows but the parsers do not (FR-098 review).
        "\r",
        "\u{85}",
        "\u{b}",
        "\u{2028}",
        "  ",
        "\t",
    ];

    fn text_of(rng: &mut Lcg, pieces: usize) -> String {
        (0..pieces)
            .map(|_| PIECES[rng.below(PIECES.len())])
            .collect()
    }

    /// FR-097, UC-005 BR-016 (property): whatever changed between two
    /// texts (one to three edits anywhere, at char boundaries), the blocks of
    /// the old text brought up to the new one equal a full parse. Seeds are
    /// printed on failure; 2500 per format (about 5000 in all) keep the run
    /// short, and the seeds that failed are cases of their own above.
    #[test]
    fn fr097_reparse_between_random_edits_equals_the_full_parse() {
        for format in [NoteFormat::Markdown, NoteFormat::AsciiDoc] {
            for seed in 0..2500u64 {
                let mut rng = Lcg(seed * 7919 + 13);
                let n = 1 + rng.below(120);
                let old = text_of(&mut rng, n);
                let mut new = old.clone();
                for _ in 0..=rng.below(3) {
                    let chars: Vec<usize> = new
                        .char_indices()
                        .map(|(i, _)| i)
                        .chain([new.len()])
                        .collect();
                    let a = chars[rng.below(chars.len())];
                    let b = chars[rng.below(chars.len())];
                    let (a, b) = (a.min(b), a.max(b));
                    let n = rng.below(4);
                    let insert = text_of(&mut rng, n);
                    new.replace_range(a..b, &insert);
                }
                let mut blocks = parse_blocks(&old, format);
                let mut refs = has_reference_definitions(&old);
                let _ = reparse_between(
                    &mut blocks,
                    &Rope::from_str(&old),
                    &Rope::from_str(&new),
                    format,
                    &mut refs,
                );
                assert_eq!(
                    blocks,
                    parse_blocks(&new, format),
                    "{format:?} seed {seed}: {old:?} -> {new:?}"
                );
            }
        }
    }

    /// UC-024 BR-001, FR-097: a front matter is the start of the note, so
    /// an edit that closes (or opens) one further down changes the first
    /// block too; the blocks stay those of a full parse. Found by the
    /// property test with seed 4263 (before: a thematic break and a
    /// paragraph, after: the front matter).
    #[test]
    fn uc024_br001_closing_a_front_matter_below_a_blank_line_parses_as_a_whole() {
        let old = "---\nword\n\n---x\n";
        let at = old.find('x').unwrap();
        check(old, at..at + 1, "", NoteFormat::Markdown);
        let mut blocks = parse_blocks(old, NoteFormat::Markdown);
        let new = "---\nword\n\n---\n";
        let mut refs = false;
        let _ = reparse_between(
            &mut blocks,
            &Rope::from_str(old),
            &Rope::from_str(new),
            NoteFormat::Markdown,
            &mut refs,
        );
        assert_eq!(blocks, parse_blocks(new, NoteFormat::Markdown));
    }

    /// The blocks of `old` brought to `new` by `reparse_between` against a
    /// full parse of `new`.
    fn between(old: &str, new: &str, format: NoteFormat) {
        let mut blocks = parse_blocks(old, format);
        let mut refs = has_reference_definitions(old);
        let _ = reparse_between(
            &mut blocks,
            &Rope::from_str(old),
            &Rope::from_str(new),
            format,
            &mut refs,
        );
        assert_eq!(blocks, parse_blocks(new, format), "{old:?} -> {new:?}");
    }

    /// FR-097, ADR-024: a fence that stays unclosed runs to the end of the
    /// note; opening one at the top level (it was inside a list item that
    /// the next item ended) takes in the blocks after it.
    #[test]
    fn fr097_an_unclosed_fence_taking_in_the_blocks_after_it_parses_as_a_whole() {
        between(
            "- a\n  ~~~\n- item\n",
            "a\n  ~~~\n- item\n",
            NoteFormat::Markdown,
        );
    }

    /// FR-097, ADR-024: a link reference definition inside a list item
    /// turns the `[x]` of an earlier block into a link; the note parses as
    /// a whole.
    #[test]
    fn fr097_a_reference_definition_in_a_list_item_parses_as_a_whole() {
        between("[x]\n\n|\n#", "[x]\n\n|\n#\n* [x]:h", NoteFormat::Markdown);
    }

    /// FR-097, ADR-024: taking the marker of an empty list item away.
    #[test]
    fn fr097_taking_the_marker_of_an_empty_list_item_away_equals_the_full_parse() {
        between("-\n  <v>\n1", "\n  <v>\n1", NoteFormat::Markdown);
    }

    /// FR-098, OQ-222, ADR-029: typing in a formula, and in a paragraph of
    /// a note whose header sets `:stem: latexmath`, parses only around the
    /// key and gives what a full parse gives; changing the header changes
    /// every `stem:[…]`.
    #[test]
    fn fr098_formulas_reparse_as_a_full_parse() {
        let md = "# T\n\nSome $x^2$ text.\n\n$$\ny\n$$\n\nEnd $a$.\n";
        let at = md.find("x^2").unwrap() + 1;
        check(md, at..at, "+1", NoteFormat::Markdown);
        check(md, at..at, "$", NoteFormat::Markdown);
        let y = md.find("\ny\n").unwrap() + 1;
        check(md, y..y, "z", NoteFormat::Markdown);
        let adoc =
            "= T\n:stem: latexmath\n\nA para.\n\nSee stem:[a^2] here.\n\n[stem]\n++++\nb\n++++\n";
        let at = adoc.find("a^2").unwrap() + 1;
        check(adoc, at..at, "+1", NoteFormat::AsciiDoc);
        let b = adoc.find("\nb\n").unwrap() + 1;
        check(adoc, b..b, "c", NoteFormat::AsciiDoc);
        let value = adoc.find("latexmath\n").unwrap();
        check(adoc, value..value + 5, "ascii", NoteFormat::AsciiDoc);
        let mut blocks = parse_blocks(adoc, NoteFormat::AsciiDoc);
        // OQ-222: `:stem:` alone is AsciiMath now; without it, no formula.
        let new = adoc.replace(":stem: latexmath\n", "");
        let mut refs = false;
        let _ = reparse_between(
            &mut blocks,
            &Rope::from_str(adoc),
            &Rope::from_str(&new),
            NoteFormat::AsciiDoc,
            &mut refs,
        );
        assert_eq!(blocks, parse_blocks(&new, NoteFormat::AsciiDoc));
        assert!(blocks
            .iter()
            .all(|b| !matches!(b.kind, BlockKind::Formula { .. })));
        // OQ-222: AsciiMath, typed in and switched to LaTeX in the header.
        let ascii = "= T\n:stem:\n\nA para.\n\nSee stem:[a/b] and asciimath:[x^2] here.\n\n[asciimath]\n++++\nsqrt 2\n++++\n";
        let at = ascii.find("a/b").unwrap() + 1;
        check(ascii, at..at, "/c", NoteFormat::AsciiDoc);
        let at = ascii.find("x^2").unwrap() + 3;
        check(ascii, at..at, "+y", NoteFormat::AsciiDoc);
        let at = ascii.find("sqrt 2").unwrap() + 6;
        check(ascii, at..at, "/3", NoteFormat::AsciiDoc);
        let header = ascii.find(":stem:").unwrap() + 6;
        check(ascii, header..header, " latexmath", NoteFormat::AsciiDoc);
    }

    /// FR-097, ADR-024: AsciiDoc, a title after block attributes.
    #[test]
    fn fr097_asciidoc_a_title_after_block_attributes_equals_the_full_parse() {
        between("t\n\n[]\n= ", "t\n\n[]\n= m\n<", NoteFormat::AsciiDoc);
        between("$\n\n= c\nn", "\n= c\nn", NoteFormat::AsciiDoc);
    }

    /// NFR-002, FR-097 (review): how often typing parses the whole note.
    /// Letters, spaces and Enter typed at places all over a note with
    /// headings, paragraphs, lists, a quote, a code block and a table: most
    /// keys parse only around them (the share is printed).
    #[test]
    fn nfr002_typing_mostly_parses_only_around_the_key() {
        let section = "## Section\nRight below the heading.\n\nSome text of a paragraph, with *emphasis* and a [link](x.md).\nIts second line goes on.\n\n- one item\n- two items\n  - inner item\n\n> a quote\n> more\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n";
        let original = format!("# Title\n\nIntro paragraph.\n\n{}", section.repeat(20));
        let mut text = original.clone();
        let mut rng = Lcg(42);
        let (mut keys, mut whole) = (0, 0);
        for _ in 0..2000 {
            let at = {
                let chars: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
                chars[rng.below(chars.len())]
            };
            let typed = match rng.below(20) {
                0 => "\n",
                1..=4 => " ",
                _ => "e",
            };
            let old = Rope::from_str(&text);
            let mut new = text.clone();
            new.insert_str(at, typed);
            let new_rope = Rope::from_str(&new);
            let mut blocks = parse_blocks(&text, NoteFormat::Markdown);
            let change = Change {
                start: at,
                old_end: at,
                new_end: at + typed.len(),
            };
            keys += 1;
            if !local_is_safe(&blocks, &old, &new_rope, NoteFormat::Markdown, change) {
                whole += 1;
            }
            // And what is parsed around the key is what a full parse gives.
            let mut refs = has_reference_definitions(&text);
            reparse(
                &mut blocks,
                &old,
                &new_rope,
                NoteFormat::Markdown,
                change,
                &mut refs,
            );
            assert_eq!(
                blocks,
                parse_blocks(&new, NoteFormat::Markdown),
                "{typed:?} at {at}"
            );
            text = new;
        }
        // Typing on in one paragraph, as one writes.
        text = original;
        let first = text.find("Its second line").unwrap() + 3;
        let mut in_paragraph = 0;
        for (k, at) in (first..first + 300).enumerate() {
            let typed = if k % 6 == 5 { " " } else { "w" };
            let old = Rope::from_str(&text);
            let mut new = text.clone();
            new.insert_str(at, typed);
            let blocks = parse_blocks(&text, NoteFormat::Markdown);
            let change = Change {
                start: at,
                old_end: at,
                new_end: at + 1,
            };
            if !local_is_safe(
                &blocks,
                &old,
                &Rope::from_str(&new),
                NoteFormat::Markdown,
                change,
            ) {
                in_paragraph += 1;
            }
            text = new;
        }
        println!(
            "NFR-002: {in_paragraph} of 300 keys typed on in a paragraph parse the whole note"
        );
        assert_eq!(in_paragraph, 0);
        let share = f64::from(whole) / f64::from(keys);
        println!(
            "NFR-002: {whole} of {keys} keys parse the whole note ({:.1} %)",
            share * 100.0
        );
        assert!(share < 0.5, "{share}");
    }
    /// FR-098, OQ-222, UC-005 BR-016 (property, test monkey): random edits
    /// that add or remove `$`, `$$`, backticks, `\$`, `latexmath:[`,
    /// `asciimath:[`, `stem:[`, `]`, `\]`, `++++`, `[stem]`, `[asciimath]`,
    /// AsciiMath, header lines of `:stem:` (LaTeX and AsciiMath),
    /// CRLF and emoji leave the blocks of the old text, brought up to the
    /// new one, equal to a full parse. The seed is in the failure message.
    #[test]
    fn fr098_random_formula_edits_equal_the_full_parse() {
        const TOKENS: &[&str] = &[
            "$",
            "$$",
            "`",
            "\\$",
            "\\",
            "latexmath:[",
            "stem:[",
            "]",
            "\\]",
            "++++\n",
            "[stem]\n",
            "[latexmath]\n",
            ":stem: latexmath\n",
            ":stem:\n",
            // OQ-222: AsciiMath forms.
            ":stem: asciimath\n",
            "asciimath:[",
            "[asciimath]\n",
            "asciimath:[a/b]",
            "sqrt(x)",
            "(",
            ")",
            "\r\n",
            "\n",
            "\n\n",
            "\u{1F600}",
            "x",
            "a^2",
            " ",
            "```math\n",
            "```\n",
            "$`",
            "`$",
            "*",
            "_",
            "....\n",
            "// c\n",
            "[",
            "stem:[a]",
            "$x$",
            "$$\nx\n$$",
            "$$x$$",
        ];
        let pick = |rng: &mut Lcg, n: usize| -> String {
            (0..n).map(|_| TOKENS[rng.below(TOKENS.len())]).collect()
        };
        for format in [NoteFormat::Markdown, NoteFormat::AsciiDoc] {
            for seed in 0..4000u64 {
                let mut rng = Lcg(seed * 15_485_863 + 99);
                let n = 5 + rng.below(70);
                let old = pick(&mut rng, n);
                let mut new = old.clone();
                for _ in 0..=rng.below(2) {
                    let chars: Vec<usize> = new
                        .char_indices()
                        .map(|(i, _)| i)
                        .chain([new.len()])
                        .collect();
                    let a = chars[rng.below(chars.len())];
                    let b = chars[rng.below(chars.len())];
                    let k = rng.below(3);
                    let insert = pick(&mut rng, k);
                    new.replace_range(a.min(b)..a.max(b), &insert);
                }
                let mut blocks = parse_blocks(&old, format);
                let mut refs = has_reference_definitions(&old);
                let _ = reparse_between(
                    &mut blocks,
                    &Rope::from_str(&old),
                    &Rope::from_str(&new),
                    format,
                    &mut refs,
                );
                assert_eq!(
                    blocks,
                    parse_blocks(&new, format),
                    "{format:?} seed {seed}: {old:?} -> {new:?}"
                );
            }
        }
    }

    /// FR-098, OQ-222 (found by the test monkey): the header is read the
    /// same for the rope of a window's reparse and for the string of a full
    /// parse. The rope also breaks a line at U+2028 or U+0085, which `str`
    /// does not, so `= T` and `:stem: latexmath` split by U+2028 were a header with `:stem:` for
    /// the reparse only, and `stem:[a]` was a formula in one and not in the
    /// other.
    #[test]
    fn fr098_the_stem_header_is_read_alike_after_a_unicode_line_break() {
        for separator in ["\u{2028}", "\u{85}", "\u{b}", "\u{c}"] {
            let adoc = format!("= T{separator}:stem: latexmath\n\nSee stem:[a] here.\n");
            let at = adoc.find("[a").unwrap() + 2;
            check(&adoc, at..at, "b", NoteFormat::AsciiDoc);
        }
    }

    /// FR-098, OQ-222, UC-005 BR-016 (property, test monkey): a note that
    /// does start with a header (`:stem:`, `:stem: asciimath`, `:stem!:`,
    /// `= T` and a stem entry in either notation) and a body of AsciiMath
    /// and LaTeX formulas in every form: random edits of the body and of the
    /// header lines leave the blocks of the old text, brought up to the new
    /// one, equal to a full parse. The seed is in the failure message.
    #[test]
    fn oq222_random_edits_of_a_note_with_a_stem_header_equal_the_full_parse() {
        const HEADERS: &[&str] = &[
            "",
            ":stem:\n\n",
            ":stem: asciimath\n\n",
            ":stem: latexmath\n\n",
            ":stem:\r\n\r\n",
            "= T\n:stem:\n\n",
            "= T\n:stem: latexmath\n\n",
            "= T\n:stem!:\n\n",
            "= T\n:stem: tex\n:stem!:\n\n",
            "= T\n:stem:\n:author: A\n\n",
            "// c\n= T\n:stem:\n\n",
        ];
        const BODY: &[&str] = &[
            "asciimath:[a/b]",
            "asciimath:[",
            "stem:[x^2]",
            "stem:[",
            "latexmath:[\\frac{a}{b}]",
            "[asciimath]\n++++\nsum_(i=1)^n i\n++++\n",
            "[stem]\n++++\n(a+b)/2\n++++\n",
            "[latexmath]\n++++\n\\sqrt{x}\n++++\n",
            "[asciimath]\n",
            "[stem]\n",
            "++++\n",
            "]",
            "\\]",
            "\\",
            "text(\"a]b\")",
            "(",
            ")",
            "{:",
            ":}",
            "\n",
            "\n\n",
            "\r\n",
            " ",
            "$",
            "`",
            "*bold*",
            "\u{2028}",
            "\u{1F600}",
            ":stem:\n",
            ":stem: latexmath\n",
            "= T\n",
            "// c\n",
            "----\n",
            "|===\n",
            "* item\n",
            ". one\n",
            "word",
        ];
        let pick = |rng: &mut Lcg, n: usize| -> String {
            (0..n).map(|_| BODY[rng.below(BODY.len())]).collect()
        };
        for seed in 0..8000u64 {
            let mut rng = Lcg(seed * 32_452_843 + 7);
            let header = HEADERS[rng.below(HEADERS.len())];
            let n = 3 + rng.below(30);
            let old = format!("{header}{}", pick(&mut rng, n));
            let mut new = old.clone();
            for _ in 0..=rng.below(3) {
                let chars: Vec<usize> = new
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([new.len()])
                    .collect();
                let a = chars[rng.below(chars.len())];
                let b = chars[rng.below(chars.len())];
                let k = rng.below(3);
                let insert = pick(&mut rng, k);
                new.replace_range(a.min(b)..a.max(b), &insert);
            }
            let format = NoteFormat::AsciiDoc;
            let mut blocks = parse_blocks(&old, format);
            let mut refs = has_reference_definitions(&old);
            let _ = reparse_between(
                &mut blocks,
                &Rope::from_str(&old),
                &Rope::from_str(&new),
                format,
                &mut refs,
            );
            assert_eq!(
                blocks,
                parse_blocks(&new, format),
                "seed {seed}: {old:?} -> {new:?}"
            );
        }
    }
}
