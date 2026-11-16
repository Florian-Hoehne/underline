//! FR-081 to FR-086 (package 4) in the tab: the keys and the commands of
//! the editing comfort in both views, with the rules of
//! `underline_editor_core::comfort`.
//!
//! - UC-005 A25, BR-019: close pairs while it is on (UC-010 BR-001).
//! - A26, BR-020, OQ-203: Alt+Up, Alt+Down and Ctrl+Shift+D (Cmd on macOS)
//!   move and duplicate lines in the source view, blocks in the facade.
//! - A27, BR-021, OQ-204: Tab and Shift+Tab indent and outdent list items;
//!   outside a list item Tab keeps its meaning.
//! - UC-004 A16, BR-017: going to a line, in either view.
//!
//! The keys reach this only where the input mode left them: in the Vim
//! keys' normal mode they are Vim's (BR-009).

use underline_editor_core::comfort::goto::LineTarget;
use underline_editor_core::facade::{Before, FacadeEdit, Gathered, MakeViews};
use underline_editor_core::{EditAction, Key, Modifiers, Outcome};

use super::{Done, EditorTab, FacadeCall, Handled};
use crate::editor_view::{units, EditorPatch, LineTargetView};
use crate::facade_view::{BlockView, FacadePatch};

/// A key held without Ctrl, Cmd or Alt; Shift may be held.
fn plain(modifiers: Modifiers) -> bool {
    !modifiers.command && !modifiers.ctrl && !modifiers.alt && !modifiers.mac_cmd
}

/// UC-005 A26, A27: `action` in the facade.
pub(super) fn facade_edit(edit: &mut FacadeEdit<'_>, action: EditAction) {
    match action {
        EditAction::MoveLineUp => edit.move_blocks(true),
        EditAction::MoveLineDown => edit.move_blocks(false),
        EditAction::DuplicateLine => edit.duplicate_blocks(),
        EditAction::Indent => {
            edit.indent_items(false);
        }
        EditAction::Outdent => {
            edit.indent_items(true);
        }
    }
}

/// UC-005 A25 to A27: a key of the editing comfort in the facade; whether
/// it took the key (else the facade's own rules do). Tab outside a list
/// item is the facade's: it does nothing (M-09.13).
pub(super) fn facade_key(
    edit: &mut FacadeEdit<'_>,
    key: Key,
    modifiers: Modifiers,
    pairs: bool,
) -> bool {
    if let Some(action) = EditAction::for_key(key, modifiers) {
        facade_edit(edit, action);
        return true;
    }
    match key {
        Key::Tab if plain(modifiers) => edit.indent_items(modifiers.shift),
        Key::Backspace if pairs && plain(modifiers) && !modifiers.shift => edit.backspace_paired(),
        _ => false,
    }
}

impl EditorTab {
    /// UC-010 BR-001, UC-005 BR-019: close pairs is on or off.
    pub fn set_close_pairs(&mut self, on: bool) {
        self.close_pairs = on;
    }

    /// UC-004 A16 step 1: how many lines the note has, as the source view
    /// numbers them.
    #[must_use]
    pub fn line_count(&self) -> u32 {
        units(self.editor.document().len_lines())
    }

    /// UC-005 A26, A27: `action` in the source view.
    pub(super) fn source_edit(&mut self, action: EditAction) -> Outcome {
        match action {
            EditAction::MoveLineUp => self.editor.move_lines(true),
            EditAction::MoveLineDown => self.editor.move_lines(false),
            EditAction::DuplicateLine => self.editor.duplicate_lines(),
            EditAction::Indent | EditAction::Outdent => self
                .editor
                .indent_list(self.format, action == EditAction::Outdent)
                .unwrap_or_default(),
        }
    }

    /// UC-005 A25 to A27: a key of the editing comfort in the source view;
    /// `None` when the key keeps its usual meaning (Tab outside a list item
    /// writes four spaces, M-08.4).
    pub(super) fn source_comfort_key(&mut self, key: Key, modifiers: Modifiers) -> Option<Outcome> {
        if let Some(action) = EditAction::for_key(key, modifiers) {
            return Some(self.source_edit(action));
        }
        match key {
            Key::Tab if plain(modifiers) => {
                let listed = self.editor.indent_list(self.format, modifiers.shift);
                // OQ-256: Shift+Tab outside a list takes space away, never
                // writes it; Tab keeps writing four spaces (M-08.4).
                if listed.is_none() && modifiers.shift {
                    Some(self.editor.outdent_lines())
                } else {
                    listed
                }
            }
            Key::Backspace if self.close_pairs && plain(modifiers) && !modifiers.shift => {
                self.editor.backspace_paired()
            }
            _ => None,
        }
    }

    /// UC-004 BR-017: what the status bar says when `target` lies beyond
    /// the note: "Went to the last line." or "Went to the end of the line.".
    #[must_use]
    pub fn line_note(&self, target: LineTargetView) -> Option<&'static str> {
        LineTarget::from(target)
            .clamped(self.editor.document().rope())
            .map(|clamped| clamped.message())
    }

    /// UC-004 A16 step 3, BR-017: the source view's cursor goes to the line
    /// and column of `target` and is shown; a folded section holding it is
    /// expanded.
    pub fn source_go_to_line(&mut self, target: LineTargetView) -> Handled<EditorPatch> {
        let base = self.editor.revision();
        let done = Done {
            outcome: self.editor.go_to(LineTarget::from(target)),
            ..Done::default()
        };
        self.handled(base, done)
    }

    /// UC-004 A16 step 3, BR-017: the facade's cursor goes to the block
    /// that holds the line of `target`, at the place of that line within
    /// it, and is shown; a folded section holding it is expanded.
    pub fn facade_go_to_line<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        target: LineTargetView,
    ) -> Handled<FacadePatch> {
        self.take_selection(call.at);
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let mut gathered = Gathered::default();
        self.with_edit(&mut gathered, (None, None), |edit, _| {
            edit.go_to_line(LineTarget::from(target));
        });
        self.facade_finish(call, before, gathered)
    }
}
