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
use underline_core::NoteFormat;
use underline_facade::blocks::{parse_blocks, Block, BlockKind};

use super::rules::{ext_end, has_reference_definitions, shift_block};

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

/// The blocks of `blocks`, which were those of the text before `change`,
/// made those of `rope`, the text after it, parsing as little as needed.
/// `refs` says whether the note may hold link reference definitions; it is
/// set when the parsed region holds one.
pub(super) fn reparse(
    blocks: &mut Vec<Block>,
    rope: &Rope,
    format: NoteFormat,
    change: Change,
    refs: &mut bool,
) -> Spliced {
    let markdown = format == NoteFormat::Markdown;
    if markdown && *refs {
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
        let text = rope.byte_slice(r0..r1).to_string();
        if markdown && has_reference_definitions(&text) {
            *refs = true;
            return parse_whole(blocks, rope, format, refs);
        }
        let parsed: Vec<Block> = parse_blocks(&text, format)
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

/// The blocks of `text` and whether a Markdown note may hold link
/// reference definitions.
pub(super) fn parse_all(text: &str, format: NoteFormat) -> (Vec<Block>, bool) {
    let refs = format == NoteFormat::Markdown && has_reference_definitions(text);
    (parse_blocks(text, format), refs)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let spliced = reparse(&mut blocks, &rope, format, change, &mut refs);
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
}
