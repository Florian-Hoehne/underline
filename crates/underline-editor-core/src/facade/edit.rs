//! UC-005, M-09.10 to M-09.14, M-11.2, M-11.3: editing in the facade. Every
//! edit is an edit of the markup at the source position the facade
//! character stands for, on the tab's one text and history (M-10.1), so
//! blocks that are not edited keep every byte (NFR-007).
//!
//! After each edit the blocks follow at once: typed characters join their
//! block without parsing ([`insert_typed`]), everything else parses only
//! the blocks around the edit ([`reparse`], ADR-024). Which blocks changed
//! is gathered in [`Dirty`], so the window gets only those.
//!
//! Ported from `crates/underline-editor/src/facade_view.rs`: `type_text`,
//! `key`, `enter`, `delete_back`, `paste`, `cursor_gap`, `position_at`,
//! `char_of`, `next_visible`, `vertical` and the `FacadeTarget` of the Vim
//! and Nano keys (a line is a block, OQ-077). What egui reads from its
//! layout (rows of a wrapped block) the window measures and sends along
//! ([`Rows`]). Fixed here (A-02): D-17, joining blocks keeps the spaces
//! that indent the joined block's first line, so Enter and Backspace give
//! the text back; a `\r\n` pair is one step for Left, Right, Backspace and
//! Delete, as in the source view (A-26).
//!
//! Moved here from `underline-session` (ADR-025, plan section 17, B2), so
//! the same rules run in the webview; what one input gathers on its way is
//! one [`Gathered`], which a [`FacadeEdit`] borrows.

use std::ops::Range;

use ropey::Rope;
use underline_markup::blocks::{link_at, Block, BlockKind};
use underline_markup::formatting::apply_format;
use underline_markup::NoteFormat;

use super::reparse::{changed_bytes, reparse, Change, Spliced};
use super::rules::{
    asciidoc_plain, byte_text, ceil_boundary, escape_markdown, ext_end, floor_boundary,
    insert_typed, line_breaks_before, next_marker, shown_chars,
};
use crate::{EditorCommand, Key, Modifiers, Outcome, SourceEditor, Target, Tool};

/// Which blocks changed in one input: the range of them in the blocks as
/// they are now, through every splice of the input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dirty(Option<(usize, usize)>);

impl Dirty {
    /// Takes in one more splice of the blocks.
    pub(super) fn splice(&mut self, spliced: Spliced) {
        let Spliced {
            from,
            removed,
            added,
        } = spliced;
        let end = from + added;
        let map = |at: usize, low: bool| {
            if at < from || (at == from && low) {
                at
            } else if at >= from + removed {
                at + added - removed
            } else if low {
                from
            } else {
                end
            }
        };
        self.0 = Some(match self.0 {
            None => (from, end),
            Some((lo, hi)) => (map(lo, true).min(from), map(hi, false).max(end)),
        });
    }

    /// The changed blocks, by their indices now.
    pub fn range(self) -> Option<Range<usize>> {
        self.0.map(|(lo, hi)| lo..hi)
    }
}

/// M-09.13: the rows the window measured for the cursor, in chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rows {
    /// The cursor they were measured at.
    pub at: usize,
    /// The place one row up, if there is a row above in the block.
    pub up: Option<usize>,
    /// The place one row down, if there is a row below in the block.
    pub down: Option<usize>,
    /// The start of the cursor's row.
    pub start: usize,
    /// The end of the cursor's row.
    pub end: usize,
}

/// M-09.11: the empty lines the cursor stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    /// The block they come before (the number of blocks: at the end).
    pub before: usize,
    /// How many empty lines are shown, at least one.
    pub lines: usize,
}

/// M-09.11, UC-005: the empty lines char `cursor` of `rope` stands on, when
/// it starts a new, still empty paragraph: outside every block of `blocks`,
/// after a blank line. An empty paragraph is no block, so without this the
/// cursor would stay at the end of the block before until something is
/// typed. Each more Enter there adds a line (`cursor_gap` of the egui
/// facade).
pub fn gap_at(rope: &Rope, cursor: usize, blocks: &[Block]) -> Option<Gap> {
    let cursor = cursor.min(rope.len_chars());
    let breaks = line_breaks_before(rope, cursor);
    if blocks.is_empty() {
        // A note of only, say, a comment has no blocks but text.
        if !rope.slice(..cursor).chars().all(char::is_whitespace) {
            return None;
        }
        return Some(Gap {
            before: 0,
            lines: breaks + 1,
        });
    }
    let byte = rope.char_to_byte(cursor);
    let next = blocks.partition_point(|b| ext_end(b) < byte);
    let inside = blocks.get(next).is_some_and(|b| byte >= b.span.start);
    (!inside && breaks >= 2).then(|| Gap {
        before: next,
        lines: breaks - 1,
    })
}

/// ADR-024, ADR-025: what one input of the facade gathers on its way,
/// through every edit of it (the keys of the input mode first, then the
/// facade's rules).
#[derive(Debug, Default)]
pub struct Gathered {
    /// Which blocks changed.
    pub dirty: Dirty,
    /// Whether the cursor moved, and the edits of the text.
    pub outcome: Outcome,
    /// What the keys asked of the window.
    pub commands: Vec<EditorCommand>,
    /// UC-005 A2: a toolbar function that asks first.
    pub request: Option<Tool>,
    /// What Vim's `"+` yanked for the system clipboard.
    pub copied: Option<String>,
}

impl Gathered {
    /// Takes in that the cursor moved (M-08.8: a fold moves it, too).
    pub fn moved(&mut self, moved: bool) {
        self.outcome.moved |= moved;
    }
}

/// UC-005: the facade's text and blocks while one input is applied; what
/// the input changes is gathered in the [`Gathered`] it borrows.
#[derive(Debug)]
pub struct FacadeEdit<'a> {
    editor: &'a mut SourceEditor,
    blocks: &'a mut Vec<Block>,
    format: NoteFormat,
    /// Markdown: the note may hold link reference definitions.
    refs: &'a mut bool,
    rows: Option<Rows>,
    /// The system clipboard's text, read by the window for Vim's `"+`.
    clipboard: Option<&'a str>,
    gathered: &'a mut Gathered,
}

impl<'a> FacadeEdit<'a> {
    /// An edit of the text of `editor` in `format`, whose blocks `blocks`
    /// follow it; `refs` tells whether a Markdown note may hold link
    /// reference definitions (then an edit parses it whole), and what the
    /// edit changes goes into `gathered`.
    pub fn new(
        editor: &'a mut SourceEditor,
        blocks: &'a mut Vec<Block>,
        format: NoteFormat,
        refs: &'a mut bool,
        gathered: &'a mut Gathered,
    ) -> Self {
        Self {
            editor,
            blocks,
            format,
            refs,
            rows: None,
            clipboard: None,
            gathered,
        }
    }

    /// M-09.13: with the rows the window measured for the cursor.
    #[must_use]
    pub fn with_rows(mut self, rows: Option<Rows>) -> Self {
        self.rows = rows;
        self
    }

    /// With the system clipboard's text the window read for Vim's `"+`.
    #[must_use]
    pub fn with_clipboard(mut self, clipboard: Option<&'a str>) -> Self {
        self.clipboard = clipboard;
        self
    }
}

impl FacadeEdit<'_> {
    fn rope(&self) -> &Rope {
        self.editor.document().rope()
    }

    fn markdown(&self) -> bool {
        self.format == NoteFormat::Markdown
    }

    fn take(&mut self, outcome: Outcome) {
        self.gathered.outcome.moved |= outcome.moved;
        self.gathered.outcome.changes.extend(outcome.changes);
    }

    fn len(&self) -> usize {
        self.editor.document().len_chars()
    }

    fn set_selection(&mut self, cursor: usize, anchor: usize) {
        let moved = self.editor.set_selection(cursor, anchor);
        self.gathered.outcome.moved |= moved;
    }

    /// The first block that ends at source byte `byte` or after it.
    fn block_ending_at_or_after(&self, byte: usize) -> Option<usize> {
        let index = self.blocks.partition_point(|b| ext_end(b) < byte);
        (index < self.blocks.len()).then_some(index)
    }

    /// The block holding char `char` of the text, and the facade index in
    /// it.
    fn position_at(&self, char: usize) -> Option<(usize, usize)> {
        let rope = self.rope();
        let byte = rope.char_to_byte(char.min(rope.len_chars()));
        let index = self
            .block_ending_at_or_after(byte)
            .unwrap_or(self.blocks.len().checked_sub(1)?);
        let block = &self.blocks[index];
        let at = block
            .facade_index(byte)
            .unwrap_or(if byte < block.span.start {
                0
            } else {
                shown_chars(block)
            });
        Some((index, at))
    }

    fn cursor_position(&self) -> Option<(usize, usize)> {
        self.position_at(self.editor.cursor())
    }

    /// The char index in the text of facade index `at` of block `index`.
    fn char_of(&self, index: usize, at: usize) -> usize {
        let rope = self.rope();
        let offset = self.blocks[index].source_offset(at).min(rope.len_bytes());
        rope.byte_to_char(offset)
    }

    fn char_of_byte(&self, byte: usize) -> usize {
        let rope = self.rope();
        rope.byte_to_char(byte.min(rope.len_bytes()))
    }

    /// Whether block `index` is hidden by a folded section (M-08.8).
    fn hidden(&self, index: usize) -> bool {
        let hidden = self.editor.hidden_lines();
        if hidden.is_empty() {
            return false;
        }
        let rope = self.rope();
        let line = rope.byte_to_line(self.blocks[index].span.start.min(rope.len_bytes()));
        hidden.iter().any(|range| range.contains(&line))
    }

    /// The next block from `index` in the direction of `step` that is
    /// shown, if any.
    fn next_visible(&self, index: usize, step: isize) -> Option<usize> {
        let mut at = index.checked_add_signed(step)?;
        while at < self.blocks.len() {
            if !self.hidden(at) {
                return Some(at);
            }
            at = at.checked_add_signed(step)?;
        }
        None
    }

    /// M-09.11: the empty lines the cursor stands on, if it does.
    fn cursor_gap(&self) -> Option<Gap> {
        gap_at(self.rope(), self.editor.cursor(), self.blocks)
    }

    /// The blocks follow the edit `change` of the text.
    fn follow(&mut self, change: Change) {
        let spliced = reparse(
            self.blocks,
            self.editor.document().rope(),
            self.format,
            change,
            self.refs,
        );
        self.gathered.dirty.splice(spliced);
    }

    /// Replaces the chars `range` by `text`, the cursor after it.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let len = self.len();
        let (from, to) = (range.start.min(len), range.end.min(len));
        let rope = self.rope();
        let start = rope.char_to_byte(from);
        let old_end = rope.char_to_byte(to);
        let outcome = self.editor.replace(from..to, text);
        let edited = outcome.edited();
        self.take(outcome);
        if edited {
            self.follow(Change {
                start,
                old_end,
                new_end: start + text.len(),
            });
        }
    }

    /// An edit whose bytes are not known beforehand (undo, redo): the
    /// blocks follow what changed.
    fn tracked(&mut self, edit: impl FnOnce(&mut SourceEditor) -> Outcome) {
        let before = self.rope().clone();
        let outcome = edit(self.editor);
        let edited = outcome.edited();
        self.take(outcome);
        if edited {
            if let Some(change) = changed_bytes(&before, self.rope()) {
                self.follow(change);
            }
        }
    }

    /// M-10.1: undoes the last edit of the note's one history.
    pub fn undo(&mut self) {
        self.tracked(SourceEditor::undo);
    }

    /// M-10.1: redoes the last undone edit.
    pub fn redo(&mut self) {
        self.tracked(SourceEditor::redo);
    }

    fn delete_selection(&mut self) -> bool {
        let selection = self.editor.selection();
        if selection.is_empty() {
            return false;
        }
        self.replace(selection, "");
        true
    }

    /// M-09.10, UC-005 step 3: types `text` at the cursor, escaped where
    /// Markdown would read it as formatting; raw in code, markup, images
    /// and tables. Typing that cannot change the structure joins the
    /// block without parsing.
    pub fn type_text(&mut self, text: &str) {
        self.delete_selection();
        let cursor = self.editor.cursor();
        let byte = self.rope().char_to_byte(cursor);
        let markdown = self.markdown();
        let position = self.cursor_position();
        let (raw, fast) = match position {
            Some((index, at)) => match &self.blocks[index].kind {
                BlockKind::Code { .. } if markdown => (text.to_owned(), !text.contains(['`', '~'])),
                BlockKind::Markup | BlockKind::Image { .. } | BlockKind::Table(_) => {
                    (text.to_owned(), false)
                }
                _ if markdown => (escape_markdown(text, at == 0), at > 0),
                // AsciiDoc has no escape that always works; plain text away
                // from markup joins its block, anything else parses again.
                _ => {
                    let rope = self.rope();
                    let near = |i: usize| (i < rope.len_chars()).then(|| rope.char(i));
                    let before = cursor.checked_sub(1).and_then(near);
                    let plain = asciidoc_plain(before, near(cursor), text);
                    (text.to_owned(), at > 0 && plain)
                }
            },
            None => (text.to_owned(), false),
        };
        // A typed character can change where a bare link ends.
        let fast = fast && link_at(self.blocks, byte).is_none();
        let outcome = self.editor.replace(cursor..cursor, &raw);
        if !outcome.edited() {
            self.take(outcome);
            return;
        }
        self.take(outcome);
        if let (true, Some((index, at))) = (fast, position) {
            if insert_typed(self.blocks, index, at, byte, text, &raw) {
                self.gathered.dirty.splice(Spliced {
                    from: index,
                    removed: 1,
                    added: 1,
                });
                return;
            }
        }
        self.follow(Change {
            start: byte,
            old_end: byte,
            new_end: byte + raw.len(),
        });
    }

    /// M-09.11: Enter: a new line in code and markup, a new list item with
    /// the next marker, a new quote paragraph in Markdown, else a new
    /// paragraph.
    fn enter(&mut self) {
        self.delete_selection();
        let Some((index, _)) = self.cursor_position() else {
            self.type_text("\n");
            return;
        };
        let markdown = self.markdown();
        let block = &self.blocks[index];
        let insert = match &block.kind {
            BlockKind::Code { .. } | BlockKind::Markup => "\n".to_owned(),
            BlockKind::ListItem { .. } => {
                let rope = self.rope();
                let line = byte_text(rope, block.span.clone());
                format!("\n{}", next_marker(line.lines().next().unwrap_or("")))
            }
            BlockKind::Quote(depth) if markdown => format!("\n\n{} ", ">".repeat(*depth)),
            _ => "\n\n".to_owned(),
        };
        let cursor = self.editor.cursor();
        self.replace(cursor..cursor, &insert);
    }

    /// M-09.12: deletes the facade character before index `at` of block
    /// `index`, with its escape; at the start of a block, joins it with the
    /// block before, a thematic break going whole.
    fn delete_back(&mut self, index: usize, at: usize) {
        let rope = self.rope();
        let len = rope.len_bytes();
        let block = &self.blocks[index];
        let range = if at > 0 {
            let start = block.source_offset(at - 1).min(len);
            let c = (start < len).then(|| rope.char(rope.byte_to_char(start)));
            let mut end = start + c.map_or(0, char::len_utf8);
            let byte_at = |i: usize| (i < len).then(|| rope.byte(i));
            let mut start = if start > 0
                && byte_at(start - 1) == Some(b'\\')
                && c.is_some_and(|c| c.is_ascii_punctuation())
            {
                start - 1
            } else {
                start
            };
            // A `\r\n` pair goes as one (A-26).
            if c == Some('\n') && start > 0 && byte_at(start - 1) == Some(b'\r') {
                start -= 1;
            }
            if c == Some('\r') && byte_at(end) == Some(b'\n') {
                end += 1;
            }
            start..end
        } else if index > 0 {
            let previous = &self.blocks[index - 1];
            let start = match previous.kind {
                BlockKind::ThematicBreak => previous.span.start,
                _ => previous.source_offset(shown_chars(previous)),
            };
            let start = floor_boundary(rope, start);
            let end = ceil_boundary(rope, block.source_offset(0).max(start).min(len));
            // D-17: the spaces indenting the block's first line stay.
            let between = rope.byte_slice(start..end).to_string();
            let end = match between.rfind('\n') {
                Some(last) if between[last + 1..].chars().all(|c| c == ' ' || c == '\t') => {
                    start + last + 1
                }
                _ => end,
            };
            start..end
        } else {
            return;
        };
        if range.is_empty() {
            return;
        }
        let (start, end) = (self.char_of_byte(range.start), self.char_of_byte(range.end));
        self.replace(start..end, "");
    }

    /// M-09.14: pastes `markup` as it is; markup of several lines stands on
    /// lines of its own, with blank lines around it.
    pub fn paste_markup(&mut self, markup: &str) {
        self.delete_selection();
        let cursor = self.editor.cursor();
        let (lead, trail) = if markup.contains('\n') {
            let rope = self.rope();
            let before: String = {
                let from = cursor.saturating_sub(2);
                rope.slice(from..cursor).to_string()
            };
            let after: String = {
                let to = (cursor + 2).min(rope.len_chars());
                rope.slice(cursor..to).to_string()
            };
            let lead = if cursor == 0 || before.ends_with("\n\n") {
                ""
            } else if before.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            let trail = if cursor >= rope.len_chars() || after.starts_with("\n\n") {
                ""
            } else if after.starts_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            (lead, trail)
        } else {
            ("", "")
        };
        self.replace(cursor..cursor, &format!("{lead}{markup}{trail}"));
    }

    /// M-10.2, UC-005 A2: applies the toolbar function `tool` at the
    /// selection; Link and Image ask first.
    pub fn apply_tool(&mut self, tool: Tool) {
        let Some(format) = tool.format() else {
            self.gathered.request = Some(tool);
            return;
        };
        let text = self.editor.text();
        let edit = apply_format(&text, self.format, self.editor.selection_bytes(), &format);
        let change = Change {
            start: edit.range.start,
            old_end: edit.range.end,
            new_end: edit.range.start + edit.text.len(),
        };
        let outcome = self
            .editor
            .replace_bytes(edit.range, &edit.text, edit.selection);
        let edited = outcome.edited();
        self.take(outcome);
        if edited {
            self.follow(change);
        }
    }

    /// Steps over the `\n` of a `\r\n` pair, which the window has no place
    /// in (A-26), in the direction of `step`.
    fn off_pair(&self, at: usize, step: isize) -> usize {
        let rope = self.rope();
        let inside =
            at > 0 && at < rope.len_chars() && rope.char(at - 1) == '\r' && rope.char(at) == '\n';
        if !inside {
            return at;
        }
        if step < 0 {
            at - 1
        } else {
            at + 1
        }
    }

    /// One shown character left of facade index `at` of block `index`,
    /// into the block before from its start.
    fn left_of(&self, index: usize, at: usize) -> usize {
        let target = if at > 0 {
            self.char_of(index, at - 1)
        } else if let Some(previous) = self.next_visible(index, -1) {
            self.char_of(previous, shown_chars(&self.blocks[previous]))
        } else {
            self.char_of(index, 0)
        };
        self.off_pair(target, -1)
    }

    /// One shown character right of facade index `at` of block `index`,
    /// into the block after from its end.
    fn right_of(&self, index: usize, at: usize) -> usize {
        let count = shown_chars(&self.blocks[index]);
        let target = if at < count {
            self.char_of(index, at + 1)
        } else if let Some(next) = self.next_visible(index, 1) {
            self.char_of(next, 0)
        } else {
            self.char_of(index, count)
        };
        self.off_pair(target, 1)
    }

    /// The facade index in block `index` of the char `target` the window
    /// measured, if it lies in that block.
    fn measured_in(&self, index: usize, target: usize) -> Option<usize> {
        let (block, at) = self.position_at(target)?;
        (block == index).then(|| at.min(shown_chars(&self.blocks[index])))
    }

    /// The rows the window measured, when they are of facade index `at` of
    /// block `index`.
    fn rows_at(&self, index: usize, at: usize) -> Option<Rows> {
        self.rows
            .filter(|rows| self.position_at(rows.at) == Some((index, at)))
    }

    /// M-09.13: one shown row up or down from facade index `at` of block
    /// `index`; from the first or last row into the block before or after,
    /// at the same index as far as that block goes. Without rows measured
    /// at this place, the block counts as one row.
    fn vertical(&self, index: usize, at: usize, up: bool) -> usize {
        let within = self
            .rows_at(index, at)
            .and_then(|rows| if up { rows.up } else { rows.down })
            .and_then(|target| self.measured_in(index, target));
        if let Some(moved) = within {
            return self.char_of(index, moved);
        }
        match self.next_visible(index, if up { -1 } else { 1 }) {
            Some(other) => {
                let count = shown_chars(&self.blocks[other]);
                self.char_of(other, at.min(count))
            }
            None => self.char_of(index, at),
        }
    }

    /// M-09.13: the start or the end of the cursor's row; without rows
    /// measured, of its block.
    fn row_end(&self, index: usize, at: usize, end: bool) -> usize {
        let count = shown_chars(&self.blocks[index]);
        let measured = self
            .rows_at(index, at)
            .map(|rows| if end { rows.end } else { rows.start })
            .and_then(|target| self.measured_in(index, target));
        let to = measured.unwrap_or(if end { count } else { 0 });
        self.char_of(index, to)
    }

    /// M-09.11 to M-09.13, M-10.1: a key pressed with `modifiers` (`key`
    /// of the egui facade).
    pub fn key(&mut self, key: Key, modifiers: Modifiers) {
        // UC-005: on the empty lines of a new paragraph, Enter adds one more
        // and Backspace takes the last one away again.
        let gap = (!modifiers.command && !modifiers.alt)
            .then(|| self.cursor_gap())
            .flatten();
        if let (Some(gap), Key::Enter | Key::Backspace) = (gap, key) {
            if self.delete_selection() && key == Key::Backspace {
                return;
            }
            let cursor = self.editor.cursor();
            if key == Key::Enter {
                self.replace(cursor..cursor, "\n");
            } else {
                // Back over the spaces to the line break before them; from
                // the first empty line, back over the whole paragraph break.
                let rope = self.rope();
                let mut start = cursor;
                let mut breaks = 0;
                while start > 0 && (breaks == 0 || gap.lines == 1) {
                    match rope.char(start - 1) {
                        '\n' => breaks += 1,
                        '\r' => {}
                        // Spaces ending the paragraph (a hard break) stay.
                        ' ' | '\t' if breaks < 2 => {}
                        _ => break,
                    }
                    start -= 1;
                }
                self.replace(start..cursor, "");
            }
            return;
        }
        let Some((index, at)) = self.cursor_position() else {
            if key == Key::Enter {
                self.type_text("\n");
            }
            return;
        };
        if let Some(tool) = Tool::for_key(key, modifiers) {
            self.apply_tool(tool);
            return;
        }
        let count = shown_chars(&self.blocks[index]);
        let target = match key {
            Key::Z if modifiers.command && modifiers.shift => return self.redo(),
            Key::Z if modifiers.command => return self.undo(),
            Key::Y if modifiers.command => return self.redo(),
            Key::A if modifiers.command => {
                let len = self.len();
                self.set_selection(len, 0);
                return;
            }
            Key::Enter => return self.enter(),
            Key::Backspace => {
                if !self.delete_selection() {
                    self.delete_back(index, at);
                }
                return;
            }
            Key::Delete => {
                if self.delete_selection() {
                    return;
                }
                if at < count {
                    self.delete_back(index, at + 1);
                } else if index + 1 < self.blocks.len() {
                    self.delete_back(index + 1, 0);
                }
                return;
            }
            Key::ArrowLeft => self.left_of(index, at),
            Key::ArrowRight => self.right_of(index, at),
            Key::ArrowUp | Key::ArrowDown => self.vertical(index, at, key == Key::ArrowUp),
            Key::Home | Key::End => self.row_end(index, at, key == Key::End),
            // M-09.13: Tab, Page Up and Page Down do nothing.
            _ => return,
        };
        let anchor = if modifiers.shift {
            self.editor.anchor()
        } else {
            target
        };
        self.set_selection(target, anchor);
    }
}

/// M-11.2, M-11.3: the facade for the Vim and Nano keys: a line is a block
/// (OQ-077), and moving goes by the characters and rows the facade shows.
#[derive(Debug)]
pub struct FacadeTarget<'e, 'a> {
    /// The edit the keys work on.
    pub edit: &'e mut FacadeEdit<'a>,
}

impl FacadeTarget<'_, '_> {
    fn position(&self, at: usize) -> Option<(usize, usize)> {
        self.edit.position_at(at)
    }
}

impl Target for FacadeTarget<'_, '_> {
    fn len(&self) -> usize {
        self.edit.len()
    }

    fn char_at(&self, at: usize) -> Option<char> {
        let rope = self.edit.rope();
        (at < rope.len_chars()).then(|| rope.char(at))
    }

    fn cursor(&self) -> (usize, usize) {
        (self.edit.editor.cursor(), self.edit.editor.anchor())
    }

    fn set_cursor(&mut self, cursor: usize, anchor: usize) {
        let len = self.len();
        self.edit.set_selection(cursor.min(len), anchor.min(len));
        self.edit.gathered.outcome.moved = true;
    }

    fn left(&mut self, at: usize) -> usize {
        match self.position(at) {
            Some((index, i)) if i > 0 || self.edit.next_visible(index, -1).is_some() => {
                self.edit.left_of(index, i)
            }
            _ => at,
        }
    }

    fn right(&mut self, at: usize) -> usize {
        let Some((index, i)) = self.position(at) else {
            return at;
        };
        let count = shown_chars(&self.edit.blocks[index]);
        if i < count || self.edit.next_visible(index, 1).is_some() {
            self.edit.right_of(index, i)
        } else {
            at
        }
    }

    fn up(&mut self, at: usize) -> usize {
        match self.position(at) {
            Some((index, i)) => self.edit.vertical(index, i, true),
            None => at,
        }
    }

    fn down(&mut self, at: usize) -> usize {
        match self.position(at) {
            Some((index, i)) => self.edit.vertical(index, i, false),
            None => at,
        }
    }

    fn line(&mut self, at: usize) -> Range<usize> {
        let Some((index, _)) = self.position(at) else {
            return at..at;
        };
        let count = shown_chars(&self.edit.blocks[index]);
        self.edit.char_of(index, 0)..self.edit.char_of(index, count)
    }

    fn whole_line(&mut self, at: usize) -> Range<usize> {
        let Some((index, _)) = self.position(at) else {
            return at..at;
        };
        let start = self.edit.char_of_byte(self.edit.blocks[index].span.start);
        let end = match self.edit.next_visible(index, 1) {
            Some(next) => self.edit.char_of_byte(self.edit.blocks[next].span.start),
            None => self.len(),
        };
        start..end
    }

    fn separator(&mut self, at: usize) -> &'static str {
        match self.position(at) {
            Some((index, _))
                if matches!(self.edit.blocks[index].kind, BlockKind::ListItem { .. }) =>
            {
                "\n"
            }
            _ => "\n\n",
        }
    }

    fn text(&self, range: Range<usize>) -> String {
        let len = self.len();
        self.edit
            .rope()
            .slice(range.start.min(len)..range.end.min(len))
            .to_string()
    }

    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.edit.replace(range, text);
    }

    fn undo(&mut self) {
        self.edit.undo();
    }

    fn redo(&mut self) {
        self.edit.redo();
    }

    fn open_line(&mut self, at: usize, below: bool) {
        let Some((index, _)) = self.position(at) else {
            return;
        };
        if below {
            // Enter at the end of the block: a list goes on with its marker.
            let count = shown_chars(&self.edit.blocks[index]);
            let end = self.edit.char_of(index, count);
            self.edit.set_selection(end, end);
            self.edit.enter();
        } else {
            let block = &self.edit.blocks[index];
            let start = block.span.start;
            let content = block.source_offset(0).max(start);
            let list = matches!(block.kind, BlockKind::ListItem { .. });
            let (insert, cursor) = if list {
                let marker = byte_text(self.edit.rope(), start..content);
                let cursor = marker.chars().count();
                (format!("{marker}\n"), cursor)
            } else {
                ("\n\n".to_owned(), 0)
            };
            let at = self.edit.char_of_byte(start);
            self.edit.replace(at..at, &insert);
            self.edit.set_selection(at + cursor, at + cursor);
        }
    }

    fn find(&self, pattern: &str, from: usize, forward: bool) -> Option<usize> {
        self.edit.editor.find(pattern, from, forward)
    }

    fn clipboard_read(&mut self) -> Option<String> {
        self.edit.clipboard.map(str::to_owned)
    }

    fn clipboard_write(&mut self, text: &str) {
        self.edit.gathered.copied = Some(text.to_owned());
    }

    fn command(&mut self, command: EditorCommand) {
        self.edit.gathered.commands.push(command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adr024_dirty_follows_every_splice_of_an_input() {
        let mut dirty = Dirty::default();
        dirty.splice(Spliced {
            from: 5,
            removed: 1,
            added: 1,
        });
        assert_eq!(dirty.range(), Some(5..6));
        // A block inserted before: the changed one moves.
        dirty.splice(Spliced {
            from: 2,
            removed: 0,
            added: 1,
        });
        assert_eq!(dirty.range(), Some(2..7));
        // Blocks removed over the end of it.
        dirty.splice(Spliced {
            from: 6,
            removed: 3,
            added: 1,
        });
        assert_eq!(dirty.range(), Some(2..7));
        // A join of the last two: the range shrinks with them.
        let mut joined = Dirty::default();
        joined.splice(Spliced {
            from: 3,
            removed: 2,
            added: 1,
        });
        assert_eq!(joined.range(), Some(3..4));
    }

    #[test]
    fn m09_11_the_gap_needs_a_blank_line_outside_every_block() {
        use underline_markup::blocks::parse_blocks;
        let text = "First.\n\n\n\nSecond.\n";
        let blocks = parse_blocks(text, NoteFormat::Markdown);
        let rope = Rope::from_str(text);
        assert_eq!(
            gap_at(&rope, 8, &blocks),
            Some(Gap {
                before: 1,
                lines: 1
            })
        );
        assert_eq!(
            gap_at(&rope, 9, &blocks),
            Some(Gap {
                before: 1,
                lines: 2
            })
        );
        assert_eq!(gap_at(&rope, 6, &blocks), None);
        assert_eq!(gap_at(&rope, 10, &blocks), None);
        // An empty note: the gap is the whole note.
        let empty = Rope::from_str("\n\n");
        assert_eq!(
            gap_at(&empty, 2, &[]),
            Some(Gap {
                before: 0,
                lines: 3
            })
        );
    }
}
