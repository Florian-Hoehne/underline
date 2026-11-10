//! The editing rules of the source view in the Normal input mode (UC-005,
//! M-08.4, M-10.1), without a UI.
//!
//! Ported from `underline-editor`'s `source_view.rs` (`key`, `insert`,
//! `replace`, `undo`, `redo`, `reload`, `at_line`, `visible_line`), so the
//! Tauri window edits exactly as the egui widget does:
//!
//! - Backspace and Delete take one char, or the selection, whatever modifier
//!   is held: there is no deleting by words.
//! - Enter inserts a line break, Tab four spaces; Ctrl/Cmd+A selects all.
//! - The arrows move one char or one line of the markup (not one wrapped
//!   row), whatever modifier is held: there is no moving by words. Up and
//!   Down keep the cursor's column as far as the line allows; folded lines
//!   are stepped over. Page Up and Page Down move a page of lines, Home and
//!   End to the ends of the line, Ctrl/Cmd+Home and Ctrl/Cmd+End to the ends
//!   of the text. Shift extends the selection.
//! - Ctrl/Cmd+Z undoes, Ctrl/Cmd+Shift+Z and Ctrl/Cmd+Y redo, with the
//!   [`Document`]'s merging rules; afterwards the cursor stands at the end of
//!   the text put back (BR-004).
//!
//! The folding keys and the toolbar shortcuts come with feature F4.
//!
//! Every change is reported as an [`Applied`] change in web units
//! (`crate::utf16`): UTF-16 units with each line break counted once, as
//! CodeMirror has it. The text itself keeps its line breaks byte for byte
//! (NFR-007). Unlike egui, a `\r\n` pair is one step for the arrows,
//! Backspace and Delete, as the window has no place between its two chars.

use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::Range;

use crate::document::{Document, Edit};
use crate::input::{Key, Modifiers};
use crate::utf16::{len_web, normalize_breaks, WebOffsets};

/// One change of the text, as a view patches it, in web units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// Where the replaced text started, in web units.
    pub from: usize,
    /// Where the replaced text ended, in web units of the text before.
    pub to: usize,
    /// The text put in its place, its line breaks normalised.
    pub insert: String,
    /// The first line the change touched.
    pub line: usize,
    /// The lines the change added; negative when it removed lines.
    pub added_lines: isize,
    /// The change came from undo or redo, which may touch any number of
    /// lines.
    pub history: bool,
}

/// What one input did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The changes of the text, in the order they were made.
    pub changes: Vec<Applied>,
    /// The cursor or the selection moved.
    pub moved: bool,
}

impl Outcome {
    /// Whether the text changed.
    #[must_use]
    pub fn edited(&self) -> bool {
        !self.changes.is_empty()
    }
}

/// The text of a note with its cursor and selection, as the source view
/// edits it.
#[derive(Debug, Clone, Default)]
pub struct SourceEditor {
    doc: Document,
    /// The cursor and the other end of the selection, as char indices.
    cursor: usize,
    anchor: usize,
    /// The lines hidden by folded sections (feature F4), in order.
    hidden: Vec<Range<usize>>,
    /// The text may hold a `\r`; without one, web units are UTF-16 units.
    may_have_cr: bool,
    /// The `\r\n` pairs of the revision they were found in.
    web: RefCell<Option<(u64, WebOffsets)>>,
}

impl SourceEditor {
    /// The text `text` with the cursor at its start and an empty history.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            doc: Document::new(text),
            may_have_cr: text.contains('\r'),
            ..Self::default()
        }
    }

    /// Runs `f` with the `\r\n` pairs of the text as it is now.
    fn with_web<R>(&self, f: impl FnOnce(&WebOffsets) -> R) -> R {
        if !self.may_have_cr {
            return f(&WebOffsets::default());
        }
        let revision = self.doc.revision();
        let mut cached = self.web.borrow_mut();
        if cached.as_ref().is_none_or(|(made, _)| *made != revision) {
            *cached = Some((revision, WebOffsets::of(self.doc.rope())));
        }
        let (_, offsets) = cached.as_ref().expect("made just now");
        f(offsets)
    }

    /// The web offset of char index `at`.
    fn to_web(&self, at: usize) -> usize {
        self.with_web(|web| web.to_web(self.doc.rope(), at))
    }

    /// The text as the window shows it: the line breaks normalised.
    #[must_use]
    pub fn web_text(&self) -> String {
        normalize_breaks(&self.doc.text()).into_owned()
    }

    /// Whether a `\r\n` pair starts at char index `at`.
    fn pair_at(&self, at: usize) -> bool {
        let rope = self.doc.rope();
        at + 1 < rope.len_chars() && rope.char(at) == '\r' && rope.char(at + 1) == '\n'
    }

    /// One step left of `at`: a `\r\n` pair is one step.
    fn step_left(&self, at: usize) -> usize {
        if at >= 2 && self.pair_at(at - 2) {
            at - 2
        } else {
            at.saturating_sub(1)
        }
    }

    /// One step right of `at`: a `\r\n` pair is one step.
    fn step_right(&self, at: usize) -> usize {
        let len = self.doc.len_chars();
        if self.pair_at(at) {
            at + 2
        } else {
            (at + 1).min(len)
        }
    }

    /// The document with its history.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The whole text.
    #[must_use]
    pub fn text(&self) -> String {
        self.doc.text()
    }

    /// Counts every change of the text.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.doc.revision()
    }

    /// The cursor, as a char index.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The other end of the selection, as a char index; the cursor when
    /// nothing is selected.
    #[must_use]
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The selection as char indices, start before end.
    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    /// The selected text.
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.doc.rope().slice(self.selection()).to_string()
    }

    /// Puts the cursor and the anchor at these char indices, within the
    /// text, as a click or a drag of the mouse does (M-08.3). Returns whether
    /// they moved.
    pub fn set_selection(&mut self, cursor: usize, anchor: usize) -> bool {
        let len = self.doc.len_chars();
        let before = (self.cursor, self.anchor);
        self.cursor = cursor.min(len);
        self.anchor = anchor.min(len);
        before != (self.cursor, self.anchor)
    }

    /// Like [`Self::set_selection`], with web offsets of the window.
    pub fn set_selection_web(&mut self, cursor: usize, anchor: usize) -> bool {
        let (cursor, anchor) = self.with_web(|web| {
            let rope = self.doc.rope();
            (web.from_web(rope, cursor), web.from_web(rope, anchor))
        });
        self.set_selection(cursor, anchor)
    }

    /// The cursor and the anchor as web offsets.
    #[must_use]
    pub fn selection_web(&self) -> (usize, usize) {
        self.with_web(|web| {
            let rope = self.doc.rope();
            (web.to_web(rope, self.cursor), web.to_web(rope, self.anchor))
        })
    }

    /// The lines hidden by folded sections, in order and not overlapping
    /// (feature F4); Up and Down step over them.
    pub fn set_hidden_lines(&mut self, hidden: Vec<Range<usize>>) {
        self.hidden = hidden;
    }

    /// Shows `text` instead of the current text, after a change from outside
    /// (UC-004 A7). The cursor stays at the same line and column as far as
    /// the new text allows; the undo history starts anew (UC-005 BR-004).
    pub fn reload(&mut self, text: &str) {
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        self.doc = self.doc.replaced_by(text);
        self.may_have_cr = text.contains('\r');
        self.cursor = self.at_line(line, column);
        self.anchor = self.cursor;
    }

    /// UC-019 BR-007: the next edit is not undone together with the last.
    pub fn seal(&mut self) {
        self.doc.seal();
    }

    /// Typed, pasted or composed text replaces the selection (UC-005 step 1).
    pub fn type_text(&mut self, text: &str) -> Outcome {
        self.insert(text)
    }

    /// M-08.6: cutting takes the selection away and returns it for the
    /// clipboard; with nothing selected nothing happens.
    pub fn cut(&mut self) -> (String, Outcome) {
        let text = self.selected_text();
        if text.is_empty() {
            return (text, Outcome::default());
        }
        let outcome = self.insert("");
        (text, outcome)
    }

    /// M-08.4, M-10.1: a key pressed with `modifiers`; `page` is how many
    /// lines Page Up and Page Down move.
    pub fn key(&mut self, key: Key, modifiers: Modifiers, page: usize) -> Outcome {
        let rope_len = self.doc.len_chars();
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        let target = match key {
            Key::Backspace => {
                if self.cursor == self.anchor && self.cursor > 0 {
                    self.anchor = self.step_left(self.cursor);
                }
                return self.insert("");
            }
            Key::Delete => {
                if self.cursor == self.anchor && self.cursor < rope_len {
                    self.anchor = self.step_right(self.cursor);
                }
                return self.insert("");
            }
            Key::Enter => return self.insert("\n"),
            Key::Tab => return self.insert("    "),
            Key::Z if modifiers.command && modifiers.shift => return self.redo(),
            Key::Z if modifiers.command => return self.undo(),
            Key::Y if modifiers.command => return self.redo(),
            Key::A if modifiers.command => {
                let moved = self.set_selection(rope_len, 0);
                return Outcome {
                    changes: Vec::new(),
                    moved,
                };
            }
            Key::ArrowLeft => self.step_left(self.cursor),
            Key::ArrowRight => self.step_right(self.cursor),
            Key::ArrowUp => {
                let up = self.visible_line(line.saturating_sub(1), -1);
                self.at_line(up, column)
            }
            Key::ArrowDown => {
                let down = self.visible_line(line + 1, 1);
                self.at_line(down, column)
            }
            Key::PageUp => self.at_line(line.saturating_sub(page), column),
            Key::PageDown => self.at_line(line + page, column),
            Key::Home if modifiers.command => 0,
            Key::End if modifiers.command => rope_len,
            Key::Home => self.doc.rope().line_to_char(line),
            Key::End => self.at_line(line, usize::MAX),
            _ => return Outcome::default(),
        };
        let anchor = if modifiers.shift { self.anchor } else { target };
        let moved = self.set_selection(target, anchor);
        Outcome {
            changes: Vec::new(),
            moved,
        }
    }

    /// Replaces the chars in `range` by `text` and puts the cursor after it
    /// (ADR-012).
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Outcome {
        let len = self.doc.len_chars();
        let (start, end) = (range.start.min(len), range.end.min(len));
        if start == end && text.is_empty() {
            return Outcome::default();
        }
        let removed = self.doc.rope().slice(start..end).to_string();
        let lines_before = self.doc.len_lines();
        let line = self.doc.replace(start..end, text);
        self.may_have_cr |= text.contains('\r');
        let applied = self.applied(start, &removed, text, line, lines_before, false);
        self.cursor = start + text.chars().count();
        self.anchor = self.cursor;
        Outcome {
            changes: vec![applied],
            moved: true,
        }
    }

    /// The change of `removed` to `inserted` at char index `at`, made just
    /// now, in web units. A `\r` just before or a `\n` just after the change
    /// is taken into it when the change may have joined it into a pair or
    /// parted it from one.
    fn applied(
        &self,
        at: usize,
        removed: &str,
        inserted: &str,
        line: usize,
        lines_before: usize,
        history: bool,
    ) -> Applied {
        let rope = self.doc.rope();
        let end = at + inserted.chars().count();
        let before = (at > 0).then(|| rope.char(at - 1));
        let after = (end < rope.len_chars()).then(|| rope.char(end));
        // The chars next to the neighbours, before and after the change.
        let first = |text: &str| text.chars().next().or(after);
        let last = |text: &str| text.chars().last().or(before);
        let joins_left = first(removed) == Some('\n') || first(inserted) == Some('\n');
        let joins_right = last(removed) == Some('\r') || last(inserted) == Some('\r');
        let left = if before == Some('\r') && joins_left {
            "\r"
        } else {
            ""
        };
        let right = if after == Some('\n') && joins_right {
            "\n"
        } else {
            ""
        };
        let start = at - left.len();
        let from = self.to_web(start);
        let (old, new): (Cow<'_, str>, Cow<'_, str>) = if left.is_empty() && right.is_empty() {
            (Cow::Borrowed(removed), Cow::Borrowed(inserted))
        } else {
            (
                Cow::Owned(format!("{left}{removed}{right}")),
                Cow::Owned(format!("{left}{inserted}{right}")),
            )
        };
        Applied {
            from,
            to: from + len_web(&old),
            insert: normalize_breaks(&new).into_owned(),
            line,
            added_lines: lines(self.doc.len_lines()) - lines(lines_before),
            history,
        }
    }

    /// Takes back the last undo step (M-10.1).
    pub fn undo(&mut self) -> Outcome {
        let lines_before = self.doc.len_lines();
        let edit = self.doc.undo_edit();
        self.after_history(edit, lines_before)
    }

    /// Applies the last undone step again (M-10.1).
    pub fn redo(&mut self) -> Outcome {
        let lines_before = self.doc.len_lines();
        let edit = self.doc.redo_edit();
        self.after_history(edit, lines_before)
    }

    fn after_history(&mut self, edit: Option<Edit>, lines_before: usize) -> Outcome {
        let Some(edit) = edit else {
            return Outcome::default();
        };
        self.may_have_cr |= edit.inserted.contains('\r');
        let rope = self.doc.rope();
        let line = rope.char_to_line(edit.at.min(rope.len_chars()));
        let applied = self.applied(
            edit.at,
            &edit.removed,
            &edit.inserted,
            line,
            lines_before,
            true,
        );
        // BR-004: the cursor stands at the end of the text put back.
        self.cursor = edit.at + edit.inserted.chars().count();
        self.anchor = self.cursor;
        Outcome {
            changes: vec![applied],
            moved: true,
        }
    }

    /// Replaces the selection by `text`.
    fn insert(&mut self, text: &str) -> Outcome {
        self.replace(self.selection(), text)
    }

    /// The char index at `column` of `line`, clamped to the line.
    fn at_line(&self, line: usize, column: usize) -> usize {
        let line = line.min(self.doc.len_lines().saturating_sub(1));
        let length = self.doc.line_text(line).chars().count();
        self.doc.rope().line_to_char(line) + column.min(length)
    }

    fn is_hidden_line(&self, line: usize) -> bool {
        self.hidden.iter().any(|range| range.contains(&line))
    }

    /// The nearest line from `line` in the direction of `step` (1 or -1)
    /// that is shown, or `line` when there is none.
    fn visible_line(&self, line: usize, step: isize) -> usize {
        let lines = self.doc.len_lines();
        let mut at = Some(line);
        while let Some(current) = at.filter(|l| *l < lines && self.is_hidden_line(*l)) {
            at = current.checked_add_signed(step);
        }
        match at {
            Some(found) if found < lines => found,
            _ => line,
        }
    }
}

/// A count of lines as a signed number, for the difference of two counts.
fn lines(count: usize) -> isize {
    isize::try_from(count).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers {
        alt: false,
        ctrl: false,
        shift: false,
        mac_cmd: false,
        command: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ..NONE
    };
    const COMMAND: Modifiers = Modifiers {
        ctrl: true,
        command: true,
        ..NONE
    };
    const COMMAND_SHIFT: Modifiers = Modifiers {
        shift: true,
        ..COMMAND
    };
    const ALT: Modifiers = Modifiers { alt: true, ..NONE };

    fn editor(text: &str, cursor: usize) -> SourceEditor {
        let mut editor = SourceEditor::new(text);
        editor.set_selection(cursor, cursor);
        editor
    }

    fn press(editor: &mut SourceEditor, key: Key, modifiers: Modifiers) -> Outcome {
        editor.key(key, modifiers, 3)
    }

    fn typed(editor: &mut SourceEditor, text: &str) {
        for c in text.chars() {
            editor.type_text(&c.to_string());
        }
    }

    #[test]
    fn uc005_m08_4_typing_replaces_the_selection() {
        let mut editor = editor("hello world", 0);
        editor.set_selection(11, 6);

        let outcome = editor.type_text("there");

        assert_eq!(editor.text(), "hello there");
        assert_eq!((editor.cursor(), editor.anchor()), (11, 11));
        assert_eq!(
            outcome.changes,
            [Applied {
                from: 6,
                to: 11,
                insert: "there".into(),
                line: 0,
                added_lines: 0,
                history: false,
            }]
        );
    }

    #[test]
    fn uc005_m08_4_backspace_and_delete_take_one_char_whatever_the_modifiers() {
        let mut editor = editor("one two", 4);

        press(&mut editor, Key::Backspace, COMMAND);
        assert_eq!(editor.text(), "onetwo", "no deleting by words");
        press(&mut editor, Key::Delete, ALT);
        assert_eq!(editor.text(), "onewo");
        assert_eq!(editor.cursor(), 3);
    }

    #[test]
    fn uc005_m08_4_backspace_and_delete_take_the_selection() {
        let mut editor = editor("abcdef", 0);
        editor.set_selection(1, 4);

        press(&mut editor, Key::Delete, NONE);

        assert_eq!(editor.text(), "aef");
        assert_eq!(editor.cursor(), 1);
    }

    #[test]
    fn uc005_m08_4_nothing_to_delete_at_the_ends_changes_nothing() {
        let mut editor = editor("ab", 0);

        assert!(!press(&mut editor, Key::Backspace, NONE).edited());
        editor.set_selection(2, 2);
        assert!(!press(&mut editor, Key::Delete, NONE).edited());
        assert_eq!(editor.text(), "ab");
        assert_eq!(editor.revision(), 0);
    }

    #[test]
    fn uc005_m08_4_enter_breaks_the_line_and_tab_inserts_four_spaces() {
        let mut editor = editor("ab", 1);

        press(&mut editor, Key::Enter, NONE);
        press(&mut editor, Key::Tab, SHIFT);

        assert_eq!(editor.text(), "a\n    b");
        assert_eq!(editor.cursor(), 6);
    }

    #[test]
    fn uc005_m08_4_ctrl_a_selects_everything_without_a_change() {
        let mut editor = editor("abc\ndef", 2);

        let outcome = press(&mut editor, Key::A, COMMAND);

        assert_eq!(editor.selection(), 0..7);
        assert_eq!(editor.cursor(), 7);
        assert!(outcome.moved && !outcome.edited());
        // Without the command key, A is nothing for the keys: it is typed.
        assert_eq!(press(&mut editor, Key::A, NONE), Outcome::default());
    }

    #[test]
    fn uc005_m08_4_left_and_right_move_one_char_whatever_the_modifiers() {
        let mut editor = editor("one two", 4);

        press(&mut editor, Key::ArrowLeft, COMMAND);
        assert_eq!(editor.cursor(), 3, "no moving by words");
        press(&mut editor, Key::ArrowRight, ALT);
        press(&mut editor, Key::ArrowRight, NONE);
        assert_eq!(editor.cursor(), 5);
        editor.set_selection(0, 0);
        press(&mut editor, Key::ArrowLeft, NONE);
        assert_eq!(editor.cursor(), 0);
        editor.set_selection(7, 7);
        press(&mut editor, Key::ArrowRight, NONE);
        assert_eq!(editor.cursor(), 7);
    }

    #[test]
    fn uc005_m08_4_up_and_down_keep_the_column_as_far_as_the_line_allows() {
        // Lines: "abcdef" (0..6), "ab" (7..9), "abcdef" (10..16).
        let mut editor = editor("abcdef\nab\nabcdef", 5);

        press(&mut editor, Key::ArrowDown, NONE);
        assert_eq!(editor.cursor(), 9, "end of the short line");
        press(&mut editor, Key::ArrowDown, NONE);
        assert_eq!(
            editor.cursor(),
            12,
            "like egui, the column is the one the cursor has now"
        );
        press(&mut editor, Key::ArrowUp, NONE);
        press(&mut editor, Key::ArrowUp, NONE);
        assert_eq!(editor.cursor(), 2);
        press(&mut editor, Key::ArrowUp, NONE);
        assert_eq!(editor.cursor(), 2, "the first line stays");
        editor.set_selection(16, 16);
        press(&mut editor, Key::ArrowDown, NONE);
        assert_eq!(editor.cursor(), 16, "the last line stays");
    }

    #[test]
    fn uc004_br012_m08_4_up_and_down_step_over_folded_lines() {
        let mut editor = editor("a\nb\nc\nd\ne", 0);
        editor.set_hidden_lines(std::iter::once(1..3).collect());

        press(&mut editor, Key::ArrowDown, NONE);
        assert_eq!(editor.cursor(), 6, "from line 0 to line 3");
        press(&mut editor, Key::ArrowUp, NONE);
        assert_eq!(editor.cursor(), 0);
        editor.set_hidden_lines(std::iter::once(3..5).collect());
        editor.set_selection(4, 4);
        press(&mut editor, Key::ArrowDown, NONE);
        assert_eq!(
            editor.cursor(),
            6,
            "like egui, with nothing shown below the next line is taken"
        );
    }

    #[test]
    fn uc005_m08_4_page_up_and_down_move_a_page_of_lines() {
        let text = (0..10).map(|i| format!("{i}{i}")).collect::<Vec<_>>();
        let mut editor = editor(&text.join("\n"), 1);

        editor.key(Key::PageDown, NONE, 4);
        assert_eq!(editor.cursor(), 4 * 3 + 1);
        editor.key(Key::PageDown, NONE, 40);
        assert_eq!(editor.cursor(), 9 * 3 + 1, "the last line");
        editor.key(Key::PageUp, NONE, 2);
        assert_eq!(editor.cursor(), 7 * 3 + 1);
    }

    #[test]
    fn uc005_m08_4_home_and_end_go_to_the_line_or_the_text() {
        let mut editor = editor("abc\ndef\nghi", 5);

        press(&mut editor, Key::End, NONE);
        assert_eq!(editor.cursor(), 7);
        press(&mut editor, Key::Home, NONE);
        assert_eq!(editor.cursor(), 4);
        press(&mut editor, Key::End, COMMAND);
        assert_eq!(editor.cursor(), 11);
        press(&mut editor, Key::Home, COMMAND);
        assert_eq!(editor.cursor(), 0);
    }

    #[test]
    fn uc005_m08_4_shift_extends_the_selection() {
        let mut editor = editor("abc\ndef", 1);

        press(&mut editor, Key::ArrowRight, SHIFT);
        press(&mut editor, Key::ArrowDown, SHIFT);
        assert_eq!((editor.cursor(), editor.anchor()), (6, 1));
        press(&mut editor, Key::Home, COMMAND_SHIFT);
        assert_eq!((editor.cursor(), editor.anchor()), (0, 1));
        press(&mut editor, Key::ArrowRight, NONE);
        assert_eq!(editor.selection(), 1..1, "without Shift the selection goes");
    }

    #[test]
    fn uc005_m10_1_undo_and_redo_keys_with_the_merging_rules() {
        let mut editor = editor("", 0);
        typed(&mut editor, "hello world");

        press(&mut editor, Key::Z, COMMAND);
        assert_eq!(editor.text(), "hello");
        assert_eq!(editor.cursor(), 5, "at the end of the text put back");
        press(&mut editor, Key::Z, COMMAND);
        assert_eq!(editor.text(), "");
        press(&mut editor, Key::Z, COMMAND_SHIFT);
        assert_eq!(editor.text(), "hello");
        press(&mut editor, Key::Y, COMMAND);
        assert_eq!(editor.text(), "hello world");
        assert_eq!(editor.cursor(), 11);
        assert!(
            !press(&mut editor, Key::Y, COMMAND).edited(),
            "nothing to redo"
        );
    }

    #[test]
    fn uc005_m10_1_backspaces_in_a_row_are_one_step_and_a_line_break_its_own() {
        let mut editor = editor("", 0);
        typed(&mut editor, "abc");
        press(&mut editor, Key::Enter, NONE);
        typed(&mut editor, "def");
        for _ in 0..2 {
            press(&mut editor, Key::Backspace, NONE);
        }

        press(&mut editor, Key::Z, COMMAND);
        assert_eq!(editor.text(), "abc\ndef");
        press(&mut editor, Key::Z, COMMAND);
        assert_eq!(editor.text(), "abc\n");
        press(&mut editor, Key::Z, COMMAND);
        assert_eq!(editor.text(), "abc");
    }

    #[test]
    fn uc005_m10_1_undo_reports_the_change_for_the_view() {
        let mut editor = editor("a\nb", 3);
        press(&mut editor, Key::Enter, NONE);

        let outcome = press(&mut editor, Key::Z, COMMAND);

        assert_eq!(
            outcome.changes,
            [Applied {
                from: 3,
                to: 4,
                insert: String::new(),
                line: 1,
                added_lines: -1,
                history: true,
            }]
        );
    }

    #[test]
    fn uc004_a7_reload_keeps_line_and_column_and_forgets_the_history() {
        let mut editor = editor("abc\ndefgh\n", 0);
        typed(&mut editor, "x");
        editor.set_selection(8, 8);

        editor.reload("one\ntwo\nthree\n");

        assert_eq!(editor.text(), "one\ntwo\nthree\n");
        assert_eq!(editor.cursor(), 7, "line 1, column 3 at most");
        assert!(!editor.undo().edited(), "the history starts anew");
    }

    #[test]
    fn m08_6_cut_takes_the_selection() {
        let mut editor = editor("hello world", 0);
        assert_eq!(editor.cut(), (String::new(), Outcome::default()));
        editor.set_selection(5, 11);

        let (cut, outcome) = editor.cut();

        assert_eq!(cut, " world");
        assert_eq!(editor.text(), "hello");
        assert!(outcome.edited());
    }

    #[test]
    fn adr021_changes_and_selection_in_utf16_units_with_emoji_and_cjk() {
        let mut editor = editor("", 0);

        let outcome = editor.type_text("🙂");
        assert_eq!((outcome.changes[0].from, outcome.changes[0].to), (0, 0));
        typed(&mut editor, "日本x");
        assert_eq!(editor.selection_web(), (5, 5));

        let outcome = press(&mut editor, Key::Backspace, NONE);
        assert_eq!((outcome.changes[0].from, outcome.changes[0].to), (4, 5));
        press(&mut editor, Key::Backspace, NONE);
        press(&mut editor, Key::Backspace, NONE);
        let outcome = press(&mut editor, Key::Backspace, NONE);
        assert_eq!(
            (outcome.changes[0].from, outcome.changes[0].to),
            (0, 2),
            "the emoji goes as a whole"
        );
        assert_eq!(editor.text(), "");

        let mut editor = editor_with_selection("a🙂b", 3, 2);
        assert_eq!(editor.selected_text(), "🙂", "inside the pair is the pair");
        assert_eq!(editor.selection_web(), (3, 1));
        editor.set_selection_web(1, 1);
        assert_eq!(editor.cursor(), 1);
    }

    fn editor_with_selection(text: &str, cursor16: usize, anchor16: usize) -> SourceEditor {
        let mut editor = SourceEditor::new(text);
        editor.set_selection_web(cursor16, anchor16);
        editor
    }

    /// Applies `applied` to `view`, the window's copy, as CodeMirror does.
    fn patch(view: &mut Vec<u16>, applied: &[Applied]) {
        for change in applied {
            let insert: Vec<u16> = change.insert.encode_utf16().collect();
            view.splice(change.from..change.to, insert);
        }
    }

    /// The window's copy of `editor`'s text, kept by the patches.
    struct Window(Vec<u16>);

    impl Window {
        fn of(editor: &SourceEditor) -> Self {
            Self(editor.web_text().encode_utf16().collect())
        }

        fn follow(&mut self, editor: &SourceEditor, outcome: &Outcome) {
            patch(&mut self.0, &outcome.changes);
            assert_eq!(
                String::from_utf16(&self.0).unwrap(),
                editor.web_text(),
                "the window's text drifted"
            );
        }
    }

    #[test]
    fn nfr007_a_crlf_note_keeps_its_bytes_and_the_window_counts_breaks_once() {
        let text = "# Head\r\n🙂 line\r\nend\r\n";
        let editor = editor(text, 0);

        assert_eq!(editor.text(), text, "opening changes no byte");
        assert_eq!(editor.web_text(), "# Head\n🙂 line\nend\n");
    }

    #[test]
    fn nfr007_typing_backspace_and_enter_across_a_crlf_break() {
        let text = "ab\r\n🙂c\r\n";
        let mut editor = editor(text, 0);
        let mut window = Window::of(&editor);
        // After the first pair: web offset 3.
        editor.set_selection_web(3, 3);
        assert_eq!(editor.cursor(), 4);

        let outcome = editor.type_text("x");
        window.follow(&editor, &outcome);
        assert_eq!(outcome.changes[0].from, 3);
        assert_eq!(editor.selection_web(), (4, 4));

        // Back over the x, then over the whole pair.
        for _ in 0..2 {
            let outcome = press(&mut editor, Key::Backspace, NONE);
            window.follow(&editor, &outcome);
        }
        assert_eq!(editor.text(), "ab🙂c\r\n", "the pair went as a whole");

        // Enter inserts \n, as in egui; the other breaks stay \r\n.
        let outcome = press(&mut editor, Key::Enter, NONE);
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\n🙂c\r\n");

        // Arrows step over a pair at once; Delete takes it whole.
        editor.set_selection(5, 5);
        press(&mut editor, Key::ArrowRight, NONE);
        assert_eq!(editor.cursor(), 7);
        press(&mut editor, Key::ArrowLeft, NONE);
        assert_eq!(editor.cursor(), 5);
        let outcome = press(&mut editor, Key::Delete, NONE);
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\n🙂c");

        // Undo puts the pair back, byte for byte.
        let outcome = press(&mut editor, Key::Z, COMMAND);
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\n🙂c\r\n");
        assert_eq!(editor.selection_web(), (7, 7), "after the pair put back");
    }

    #[test]
    fn nfr007_mixed_endings_and_lone_cr_stay_in_step_with_the_window() {
        let mut editor = editor("a\rb\r\nc\nd", 0);
        let mut window = Window::of(&editor);
        assert_eq!(editor.web_text(), "a\nb\nc\nd");

        // The window's place after the lone \r is char 2.
        editor.set_selection_web(2, 2);
        assert_eq!(editor.cursor(), 2);
        // Taking the lone \r out joins the lines.
        editor.set_selection(1, 2);
        let outcome = editor.type_text("");
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\r\nc\nd");

        editor.set_selection(2, 2);
        let outcome = editor.type_text("\r");
        window.follow(&editor, &outcome);
        // b, \r (new, lone), \r\n
        let outcome = editor.type_text("x");
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\rx\r\nc\nd");

        // Taking the x out joins nothing: \r\r\n is a lone \r and a pair.
        let outcome = press(&mut editor, Key::Backspace, NONE);
        window.follow(&editor, &outcome);
        // Taking the lone \r out leaves the pair.
        let outcome = press(&mut editor, Key::Backspace, NONE);
        window.follow(&editor, &outcome);
        assert_eq!(editor.text(), "ab\r\nc\nd");

        // A \n inserted right after a lone \r joins them into one break.
        let mut lone = SourceEditor::new("a\rb");
        let mut window = Window::of(&lone);
        lone.set_selection(2, 2);
        let outcome = lone.type_text("\n");
        window.follow(&lone, &outcome);
        assert_eq!(lone.web_text(), "a\nb");
        // And undo parts them again.
        let outcome = lone.undo();
        window.follow(&lone, &outcome);
        assert_eq!(lone.web_text(), "a\nb");
        assert_eq!(lone.text(), "a\rb");
    }
}
