//! The owner of the text: takes one input of either view and answers with a
//! patch (M-08.4, M-09.10 to M-09.12, UC-005).
//!
//! The facade rules are a minimal port of `underline-editor`'s
//! `facade_view.rs` (`type_text`, `enter`, `delete_back`, `escape_markdown`,
//! `next_marker`): typing escapes Markdown and joins the block without
//! parsing again (`insert_typed`); Enter and Backspace parse the whole note
//! again, as the egui facade does. Not ported: selections, the empty-lines
//! gap after Enter, Delete, arrows, AsciiDoc.

use serde::{Deserialize, Serialize};
use underline_editor_core::Document;

use crate::blocks::{insert_typed, link_at, Block, BlockKind};
use crate::mirror::MirrorEdit;
use crate::render::{all_html, block_html};

/// What the user did, as the view's `beforeinput` names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Input {
    /// `insertText` (also the commit of an IME composition).
    InsertText { text: String },
    /// `insertParagraph` / `insertLineBreak`.
    InsertParagraph,
    /// `deleteContentBackward`.
    DeleteBackward,
    /// Source view only: the text up to UTF-16 offset `to` replaced by
    /// `text` (what CodeMirror itself changed, e.g. a composition or a paste).
    Replace { to: usize, text: String },
}

/// An input of the source view at the cursor `at`, a UTF-16 offset into the
/// whole text.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInput {
    pub at: usize,
    pub input: Input,
}

/// What the source view changes: UTF-16 offsets of the text before the
/// change, and the cursor after it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePatch {
    pub revision: u64,
    pub from: usize,
    pub to: usize,
    pub insert: String,
    pub cursor: usize,
}

/// An input of the facade in block `block` (its position among the blocks)
/// before UTF-16 offset `at` of the block's text.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacadeInput {
    pub block: usize,
    pub at: usize,
    pub input: Input,
}

/// What the facade changes: the blocks `from..to` (before the change) are
/// replaced by the elements in `html`; then the cursor goes to UTF-16 offset
/// `cursor_at` of block `cursor_block`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacadePatch {
    pub revision: u64,
    pub from: usize,
    pub to: usize,
    pub html: Vec<String>,
    pub cursor_block: usize,
    pub cursor_at: usize,
    /// Whether the whole note was parsed again.
    pub reparsed: bool,
    pub blocks: usize,
}

/// Where the benchmark types: a paragraph near the start, middle and end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchPosition {
    pub name: String,
    pub block: usize,
    /// UTF-16 offset in the block's text.
    pub at: usize,
    /// UTF-16 offset of the same place in the whole text.
    pub source: usize,
    /// The 0-based line of that place.
    pub line: usize,
}

/// The note's text with its facade blocks.
#[derive(Debug, Default)]
pub struct Owner {
    doc: Document,
    blocks: Vec<Block>,
    parsed: Option<u64>,
    /// Variant B: the edits still to send to the backend.
    mirror: Option<Vec<MirrorEdit>>,
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The char index of UTF-16 offset `at` in `text`.
fn utf16_to_char(text: &str, at: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.chars().enumerate() {
        if units >= at {
            return i;
        }
        units += c.len_utf16();
    }
    text.chars().count()
}

fn char_to_utf16(text: &str, at: usize) -> usize {
    text.chars().take(at).map(char::len_utf16).sum()
}

/// Escapes characters that Markdown would read as formatting (port of
/// `facade_view.rs` `escape_markdown`, M-09.10).
pub fn escape_markdown(text: &str, at_block_start: bool) -> String {
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

/// The marker of the next list item (port of `facade_view.rs` `next_marker`).
fn next_marker(line: &str) -> String {
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
        return format!("{indent}{}{delimiter} {}", number + 1, task(after));
    }
    format!("{indent}- ")
}

/// Whether two blocks show the same, ignoring where they are.
fn looks_same(a: &Block, b: &Block) -> bool {
    // A table's cells carry source offsets, which move with every edit
    // before them; compare only what they show.
    let kinds = match (&a.kind, &b.kind) {
        (BlockKind::Table(x), BlockKind::Table(y)) => {
            x.header == y.header
                && x.align == y.align
                && x.caption == y.caption
                && x.rows.len() == y.rows.len()
                && x.rows.iter().zip(&y.rows).all(|(r, s)| {
                    r.len() == s.len()
                        && r.iter().zip(s).all(|(c, d)| {
                            c.len() == d.len()
                                && c.iter()
                                    .zip(d)
                                    .all(|(u, v)| u.text == v.text && u.style == v.style)
                        })
                })
        }
        (x, y) => x == y,
    };
    kinds
        && a.runs.len() == b.runs.len()
        && a.runs
            .iter()
            .zip(&b.runs)
            .all(|(x, y)| x.text == y.text && x.style == y.style)
}

impl Owner {
    /// An owner of `text`. With `mirrored`, every edit is also kept for the
    /// backend (variant B).
    pub fn new(text: &str, mirrored: bool) -> Self {
        Self {
            doc: Document::new(text),
            blocks: Vec::new(),
            parsed: None,
            mirror: mirrored.then(Vec::new),
        }
    }

    /// The whole text.
    pub fn text(&self) -> String {
        self.doc.text()
    }

    /// The revision of the text.
    pub fn revision(&self) -> u64 {
        self.doc.revision()
    }

    /// The edits not yet sent to the backend (variant B).
    pub fn take_mirror(&mut self) -> Vec<MirrorEdit> {
        self.mirror.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// The number of blocks (after parsing, if the text changed).
    pub fn block_count(&mut self) -> usize {
        self.reparse();
        self.blocks.len()
    }

    /// The facade text of block `index`.
    pub fn block_text(&mut self, index: usize) -> String {
        self.reparse();
        self.blocks.get(index).map(Block::text).unwrap_or_default()
    }

    fn reparse(&mut self) -> bool {
        if self.parsed == Some(self.doc.revision()) {
            return false;
        }
        self.blocks = crate::parse(&self.doc.text());
        self.parsed = Some(self.doc.revision());
        true
    }

    /// Replaces chars `range` by `text` and notes it for the mirror.
    fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
        if let Some(log) = self.mirror.as_mut() {
            log.push(MirrorEdit {
                base: self.doc.revision(),
                at: range.start,
                remove: range.end - range.start,
                insert: text.to_owned(),
            });
        }
        self.doc.replace(range, text);
    }

    /// The HTML of the whole facade (view switch and first load).
    pub fn render_all(&mut self) -> String {
        self.reparse();
        all_html(&self.blocks)
    }

    /// The HTML of blocks `from..to` (clamped), for a facade that keeps only
    /// the blocks near the viewport in the DOM.
    pub fn render_range(&mut self, from: usize, to: usize) -> Vec<String> {
        self.reparse();
        let to = to.min(self.blocks.len());
        let from = from.min(to);
        (from..to).map(|i| block_html(i, &self.blocks[i])).collect()
    }

    /// Handles an input of the source view (M-08.4).
    pub fn source_input(&mut self, input: &SourceInput) -> SourcePatch {
        let rope = self.doc.rope();
        let at = rope.utf16_cu_to_char(input.at.min(rope.len_utf16_cu()));
        let at16 = rope.char_to_utf16_cu(at);
        let (range, text) = match &input.input {
            Input::InsertText { text } => (at..at, text.clone()),
            Input::InsertParagraph => (at..at, "\n".to_owned()),
            Input::DeleteBackward if at > 0 => (at - 1..at, String::new()),
            Input::Replace { to, text } => {
                let to = rope
                    .utf16_cu_to_char((*to).min(rope.len_utf16_cu()))
                    .max(at);
                (at..to, text.clone())
            }
            Input::DeleteBackward => {
                return SourcePatch {
                    revision: self.doc.revision(),
                    from: at16,
                    to: at16,
                    insert: String::new(),
                    cursor: at16,
                }
            }
        };
        let from = rope.char_to_utf16_cu(range.start);
        let to = rope.char_to_utf16_cu(range.end);
        self.replace(range, &text);
        SourcePatch {
            revision: self.doc.revision(),
            from,
            to,
            cursor: from + utf16_len(&text),
            insert: text,
        }
    }

    /// The block and facade char index at source byte `byte` (port of
    /// `position_at`).
    fn locate(&self, byte: usize) -> (usize, usize) {
        let Some(last) = self.blocks.len().checked_sub(1) else {
            return (0, 0);
        };
        let index = self
            .blocks
            .iter()
            .position(|b| byte <= b.span.end.max(b.runs.last().map_or(0, |r| r.end)))
            .unwrap_or(last);
        let block = &self.blocks[index];
        let at = block
            .facade_index(byte)
            .unwrap_or(if byte < block.span.start {
                0
            } else {
                block.text().chars().count()
            });
        (index, at)
    }

    /// Handles an input of the facade (M-09.10 to M-09.12).
    pub fn facade_input(&mut self, input: &FacadeInput) -> FacadePatch {
        self.reparse();
        // Typing does not need the old blocks: clone them only for the
        // inputs that parse again.
        let mut old_blocks = match input.input {
            Input::InsertText { .. } => None,
            _ => Some(self.blocks.clone()),
        };
        let Some(block) = self.blocks.get(input.block) else {
            return self.no_change(input);
        };
        let block_text = block.text();
        let at = utf16_to_char(&block_text, input.at);
        let byte = block.source_offset(at);
        let kind = block.kind.clone();
        let span = block.span.clone();
        let char_at = self.doc.rope().byte_to_char(byte);

        let cursor_byte = match &input.input {
            Input::InsertText { text } => {
                let (raw, fast) = match &kind {
                    BlockKind::Code { .. } => (text.clone(), !text.contains(['`', '~'])),
                    BlockKind::Markup | BlockKind::Image { .. } | BlockKind::Table(_) => {
                        (text.clone(), false)
                    }
                    _ => (escape_markdown(text, at == 0), at > 0),
                };
                let fast = fast && link_at(&self.blocks, byte).is_none();
                if !fast {
                    old_blocks = Some(self.blocks.clone());
                }
                self.replace(char_at..char_at, &raw);
                if fast && insert_typed(&mut self.blocks, input.block, at, byte, text, &raw) {
                    // Typing keeps the structure: only this block changes.
                    self.parsed = Some(self.doc.revision());
                    let block = &self.blocks[input.block];
                    let cursor = at + text.chars().count();
                    return FacadePatch {
                        revision: self.doc.revision(),
                        from: input.block,
                        to: input.block + 1,
                        html: vec![block_html(input.block, block)],
                        cursor_block: input.block,
                        cursor_at: char_to_utf16(&block.text(), cursor),
                        reparsed: false,
                        blocks: self.blocks.len(),
                    };
                }
                byte + raw.len()
            }
            Input::InsertParagraph => {
                let insert = match &kind {
                    BlockKind::Code { .. } | BlockKind::Markup => "\n".to_owned(),
                    BlockKind::ListItem { .. } => {
                        let first = self.doc.rope().byte_to_line(span.start);
                        format!("\n{}", next_marker(&self.doc.line_text(first)))
                    }
                    BlockKind::Quote(depth) => format!("\n\n{} ", ">".repeat(*depth)),
                    _ => "\n\n".to_owned(),
                };
                self.replace(char_at..char_at, &insert);
                byte + insert.len()
            }
            Input::Replace { .. } => return self.no_change(input),
            Input::DeleteBackward => {
                let rope = self.doc.rope();
                let range = if at > 0 {
                    let start = self.blocks[input.block].source_offset(at - 1);
                    let c = rope.char(rope.byte_to_char(start));
                    let end = start + c.len_utf8();
                    // An escaped character goes with its backslash.
                    let escaped =
                        start > 0 && rope.byte(start - 1) == b'\\' && c.is_ascii_punctuation();
                    (if escaped { start - 1 } else { start })..end
                } else if input.block > 0 {
                    let previous = &self.blocks[input.block - 1];
                    let start = match previous.kind {
                        BlockKind::ThematicBreak => previous.span.start,
                        _ => previous.source_offset(previous.text().chars().count()),
                    };
                    start..byte
                } else {
                    return self.no_change(input);
                };
                if range.is_empty() {
                    return self.no_change(input);
                }
                let chars = rope.byte_to_char(range.start)..rope.byte_to_char(range.end);
                self.replace(chars, "");
                range.start
            }
        };
        // Parse again and send only the blocks that differ.
        let old_blocks = old_blocks.unwrap_or_else(|| self.blocks.clone());
        self.reparse();
        let (old, new) = (&old_blocks, &self.blocks);
        let mut prefix = 0;
        while prefix < old.len() && prefix < new.len() && looks_same(&old[prefix], &new[prefix]) {
            prefix += 1;
        }
        let mut suffix = 0;
        while suffix < old.len() - prefix && suffix < new.len() - prefix {
            // What the DOM shows depends only on kind and runs, so blocks
            // that look the same need no new element, wherever they are.
            let (a, b) = (&old[old.len() - 1 - suffix], &new[new.len() - 1 - suffix]);
            if !looks_same(a, b) {
                break;
            }
            suffix += 1;
        }
        let (cursor_block, cursor_char) = self.locate(cursor_byte);
        let cursor_at = char_to_utf16(&self.blocks[cursor_block].text(), cursor_char);
        FacadePatch {
            revision: self.doc.revision(),
            from: prefix,
            to: old.len() - suffix,
            html: (prefix..new.len() - suffix)
                .map(|i| block_html(i, &new[i]))
                .collect(),
            cursor_block,
            cursor_at,
            reparsed: true,
            blocks: new.len(),
        }
    }

    fn no_change(&self, input: &FacadeInput) -> FacadePatch {
        FacadePatch {
            revision: self.doc.revision(),
            from: input.block,
            to: input.block,
            html: Vec::new(),
            cursor_block: input.block,
            cursor_at: input.at,
            reparsed: false,
            blocks: self.blocks.len(),
        }
    }

    /// Paragraphs near 2 %, 50 % and 98 % of the note, with a place in the
    /// middle of their first line.
    pub fn bench_positions(&mut self) -> Vec<BenchPosition> {
        self.reparse();
        let n = self.blocks.len();
        let mut out = Vec::new();
        for (name, share) in [("start", 2), ("middle", 50), ("end", 98)] {
            let target = n * share / 100;
            let index = (target..n)
                .chain((0..target).rev())
                .find(|&i| {
                    let b = &self.blocks[i];
                    matches!(b.kind, BlockKind::Paragraph)
                        && b.runs.len() == 1
                        && b.runs[0].style == Default::default()
                        && b.text().chars().count() > 40
                })
                .unwrap_or(0);
            let block = &self.blocks[index];
            let text = block.text();
            // After the 20th char, at the end of a word.
            let at_char = text
                .char_indices()
                .skip(20)
                .find(|(_, c)| *c == ' ')
                .map_or(20, |(i, _)| text[..i].chars().count());
            let byte = block.source_offset(at_char);
            let rope = self.doc.rope();
            let char_at = rope.byte_to_char(byte);
            out.push(BenchPosition {
                name: name.to_owned(),
                block: index,
                at: char_to_utf16(&text, at_char),
                source: rope.char_to_utf16_cu(char_at),
                line: rope.char_to_line(char_at),
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::Mirror;

    fn typed(owner: &mut Owner, block: usize, at: usize, text: &str) -> FacadePatch {
        owner.facade_input(&FacadeInput {
            block,
            at,
            input: Input::InsertText {
                text: text.to_owned(),
            },
        })
    }

    /// The copy of the facade's blocks (variant B) parses the test note the
    /// same as `underline-facade` (variant A).
    #[test]
    fn the_copy_of_the_blocks_parses_like_the_facade() {
        let note = crate::note::generate();
        let ours = crate::parse(&note);
        let theirs =
            underline_facade::blocks::parse_blocks(&note, underline_core::NoteFormat::Markdown);
        assert_eq!(format!("{ours:?}"), format!("{theirs:?}"));
    }

    /// M-09.10: typing escapes Markdown and changes one block only.
    #[test]
    fn m09_10_typing_in_the_facade_escapes_and_patches_one_block() {
        let mut owner = Owner::new("# Title\n\nSome text here.\n\nMore.\n", false);
        let patch = typed(&mut owner, 1, 4, "*");
        assert!(!patch.reparsed);
        assert_eq!((patch.from, patch.to, patch.cursor_at), (1, 2, 5));
        assert_eq!(owner.text(), "# Title\n\nSome\\* text here.\n\nMore.\n");
        assert_eq!(owner.block_text(1), "Some* text here.");
    }

    /// Offsets cross the boundary as UTF-16: emoji and CJK land byte-exact.
    #[test]
    fn emoji_and_cjk_land_byte_exact_in_both_views() {
        let mut owner = Owner::new("Some text here.\n", false);
        let patch = typed(&mut owner, 0, 4, "🙂");
        assert_eq!(patch.cursor_at, 6);
        typed(&mut owner, 0, 6, "日本");
        assert_eq!(owner.text(), "Some🙂日本 text here.\n");
        // Source view: after the emoji (UTF-16 offset 6) and back over it.
        let patch = owner.source_input(&SourceInput {
            at: 6,
            input: Input::DeleteBackward,
        });
        assert_eq!((patch.from, patch.to, patch.cursor), (4, 6, 4));
        assert_eq!(owner.text(), "Some日本 text here.\n");
        // Facade backspace over a CJK char.
        let patch = owner.facade_input(&FacadeInput {
            block: 0,
            at: 6,
            input: Input::DeleteBackward,
        });
        assert_eq!(owner.text(), "Some日 text here.\n");
        assert_eq!(patch.cursor_at, 5);
    }

    /// M-09.11 / M-09.12: Enter splits the paragraph, Backspace joins it.
    #[test]
    fn m09_11_enter_splits_and_backspace_joins() {
        let mut owner = Owner::new("# T\n\nab cd\n\n- one\n", false);
        let patch = owner.facade_input(&FacadeInput {
            block: 1,
            at: 2,
            input: Input::InsertParagraph,
        });
        assert!(patch.reparsed);
        assert_eq!(owner.text(), "# T\n\nab\n\n cd\n\n- one\n");
        assert_eq!((patch.from, patch.to, patch.html.len()), (1, 2, 2));
        let patch = owner.facade_input(&FacadeInput {
            block: 3,
            at: 3,
            input: Input::InsertParagraph,
        });
        assert_eq!(owner.text(), "# T\n\nab\n\n cd\n\n- one\n- \n");
        assert_eq!(patch.blocks, 5);
        let patch = owner.facade_input(&FacadeInput {
            block: 2,
            at: 0,
            input: Input::DeleteBackward,
        });
        // The leading space of " cd" is no facade char, so it goes too (as in
        // the egui facade).
        assert_eq!(owner.text(), "# T\n\nabcd\n\n- one\n- \n");
        assert_eq!((patch.cursor_block, patch.cursor_at), (1, 2));
    }

    /// Variant B: the backend's mirror ends with the owner's text.
    #[test]
    fn the_mirror_follows_the_owner() {
        let note = crate::note::generate();
        let mut owner = Owner::new(&note, true);
        let mut mirror = Mirror::new(&note);
        let positions = owner.bench_positions();
        for p in &positions {
            for (i, c) in "a*b 🙂".chars().enumerate() {
                typed(&mut owner, p.block, p.at + i, &c.to_string());
            }
        }
        owner.source_input(&SourceInput {
            at: 3,
            input: Input::InsertParagraph,
        });
        mirror.apply(&owner.take_mirror()).unwrap();
        assert_eq!(crate::hash(&mirror.text()), crate::hash(&owner.text()));
        assert_eq!(mirror.revision(), owner.revision());
    }
}
