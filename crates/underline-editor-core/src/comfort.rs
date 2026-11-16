//! FR-081 to FR-086 (package 4, editing comfort): the rules behind close
//! pairs (UC-005 A25, BR-019), moving and duplicating lines (A26, BR-020),
//! indenting list items (A27, BR-021), and going to a line (UC-004 A16,
//! A17, BR-017), on the text alone, so both editing views follow the same
//! rules (ADR-025).
//!
//! Every rule gives a [`TextEdit`]: one replacement of the text with the
//! selection after it, which the editor applies as one change, so it is one
//! step for undo. The source view applies them on lines of the markup
//! ([`crate::SourceEditor`]); the facade on its blocks (`facade::FacadeEdit`).

pub mod code;
pub mod goto;
pub mod lines;
pub mod lists;
pub mod pairs;

use std::ops::Range;

use ropey::Rope;

use crate::input::{Key, Modifiers};

/// One edit of the text: the chars `range` are replaced by `text`, and the
/// selection is then at `cursor` and `anchor`, chars of the text after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// The chars replaced.
    pub range: Range<usize>,
    /// What replaces them.
    pub text: String,
    /// The cursor afterwards.
    pub cursor: usize,
    /// The other end of the selection afterwards.
    pub anchor: usize,
}

/// FR-082, FR-083: an action of the editing comfort that a key or a
/// command of the palette asks for (UC-005 A26, A27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditAction {
    /// A26: the line or block at the cursor goes up past the one above.
    MoveLineUp,
    /// A26: the line or block at the cursor goes down past the one below.
    MoveLineDown,
    /// A26: a copy of the line or block goes below it, the cursor in it.
    DuplicateLine,
    /// A27: the list items at the cursor go one level deeper.
    Indent,
    /// A27: the list items at the cursor go one level up.
    Outdent,
}

impl EditAction {
    /// UC-005 A26, OQ-203: the action of `key` pressed with `modifiers`:
    /// Alt+Up and Alt+Down move (Option on macOS), Ctrl+Shift+D (Cmd+Shift+D)
    /// duplicates. Tab and Shift+Tab are not here: outside a list item they
    /// keep the meaning they had (BR-021), which the view decides.
    #[must_use]
    pub fn for_key(key: Key, modifiers: Modifiers) -> Option<Self> {
        let alt_only = modifiers.alt && !modifiers.ctrl && !modifiers.command && !modifiers.shift;
        match key {
            Key::ArrowUp if alt_only => Some(Self::MoveLineUp),
            Key::ArrowDown if alt_only => Some(Self::MoveLineDown),
            Key::D if modifiers.command && modifiers.shift && !modifiers.alt => {
                Some(Self::DuplicateLine)
            }
            _ => None,
        }
    }
}

/// Whether `c` ends a line (the breaks ropey counts).
fn is_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// The char where `line` of `rope` starts.
pub(crate) fn line_start(rope: &Rope, line: usize) -> usize {
    rope.line_to_char(line.min(rope.len_lines().saturating_sub(1)))
}

/// The char where the text of `line` ends, before its line break.
pub(crate) fn content_end(rope: &Rope, line: usize) -> usize {
    let line = line.min(rope.len_lines().saturating_sub(1));
    let start = rope.line_to_char(line);
    let mut end = if line + 1 < rope.len_lines() {
        rope.line_to_char(line + 1)
    } else {
        rope.len_chars()
    };
    while end > start && is_break(rope.char(end - 1)) {
        end -= 1;
    }
    end
}

/// The text of `line` without its line break.
pub(crate) fn line_text(rope: &Rope, line: usize) -> String {
    let start = line_start(rope, line);
    rope.slice(start..content_end(rope, line)).to_string()
}

/// The line of char `at`, within the text.
pub(crate) fn line_of(rope: &Rope, at: usize) -> usize {
    rope.char_to_line(at.min(rope.len_chars()))
}

/// The lines a selection from `cursor` to `anchor` touches: a selection
/// that ends at the start of a line does not touch that line.
pub(crate) fn touched_lines(rope: &Rope, cursor: usize, anchor: usize) -> (usize, usize) {
    let (start, end) = (cursor.min(anchor), cursor.max(anchor));
    let first = line_of(rope, start);
    let mut last = line_of(rope, end);
    if end > start && last > first && end == line_start(rope, last) {
        last -= 1;
    }
    (first, last)
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

    /// TC-005-36, OQ-203: Alt+Up, Alt+Down and Ctrl+Shift+D (Cmd+Shift+D on
    /// macOS) are the keys; with other modifiers they are no action.
    #[test]
    fn tc_005_36_the_keys_of_moving_and_duplicating() {
        let alt = Modifiers { alt: true, ..NONE };
        let command_shift = Modifiers {
            command: true,
            ctrl: true,
            shift: true,
            ..NONE
        };
        let mac = Modifiers::from_web(false, false, true, true, true);
        assert_eq!(
            EditAction::for_key(Key::ArrowUp, alt),
            Some(EditAction::MoveLineUp)
        );
        assert_eq!(
            EditAction::for_key(Key::ArrowDown, alt),
            Some(EditAction::MoveLineDown)
        );
        assert_eq!(
            EditAction::for_key(Key::D, command_shift),
            Some(EditAction::DuplicateLine)
        );
        assert_eq!(
            EditAction::for_key(Key::D, mac),
            Some(EditAction::DuplicateLine)
        );
        assert_eq!(EditAction::for_key(Key::ArrowUp, NONE), None);
        assert_eq!(
            EditAction::for_key(Key::ArrowUp, Modifiers { shift: true, ..alt }),
            None
        );
        assert_eq!(
            EditAction::for_key(
                Key::D,
                Modifiers {
                    shift: false,
                    ..command_shift
                }
            ),
            None
        );
    }

    #[test]
    fn lines_of_the_text_without_their_breaks() {
        let rope = Rope::from_str("a\r\nbé\n\nc");
        assert_eq!(line_text(&rope, 0), "a");
        assert_eq!(line_text(&rope, 1), "bé");
        assert_eq!(line_text(&rope, 2), "");
        assert_eq!(content_end(&rope, 3), rope.len_chars());
        assert_eq!(
            touched_lines(&rope, 3, 0),
            (0, 0),
            "up to the start of line 1"
        );
        assert_eq!(touched_lines(&rope, 4, 0), (0, 1));
    }
}
