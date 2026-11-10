//! ADR-025: a change of the backend applied to the window's editor as the
//! egui app applies it (`docs/migration/variante-b.md`, sections 3.7 and
//! 3.8): an agent's change is an undo step of its own
//! (`crates/underline-editor/src/source_view.rs:288-292`, UC-019 BR-007);
//! the links a rename rewrote and a conflict resolved are edits like typing,
//! without a seal (`source_view.rs:422-444`). The cursor and the selection
//! stay at the same place of the content, and the view does not scroll
//! (`source_view.rs:428-437`).

use super::changeset::{ChangeSet, OutOfRange};
use crate::{Outcome, SourceEditor};

/// Where `at` stands after `start..end` was replaced by `inserted` chars:
/// after the change it moves with the text, inside it to the same place of
/// the new text as far as it reaches, before it it stays (egui's
/// `apply_edits`).
#[must_use]
pub fn shifted(at: usize, (start, end): (usize, usize), inserted: usize) -> usize {
    if at >= end {
        at + inserted - (end - start)
    } else if at > start {
        start + inserted.min(at - start)
    } else {
        at
    }
}

/// Applies `set` of the backend to `editor`; `sealed` makes it an undo
/// step of its own. The edits the window must show are in the outcome; the
/// cursor did not move for the view (`moved` is false).
///
/// # Errors
///
/// The set is made on a text of another length; nothing is applied, and the
/// copies are to be synced whole.
pub fn apply_remote(
    editor: &mut SourceEditor,
    set: &ChangeSet,
    sealed: bool,
) -> Result<Outcome, OutOfRange> {
    let len = editor.document().len_chars();
    if set.base_len() != len {
        return Err(OutOfRange {
            end: set.base_len(),
            len,
        });
    }
    let (mut cursor, mut anchor) = (editor.cursor(), editor.anchor());
    if sealed {
        editor.seal();
    }
    let mut outcome = Outcome::default();
    for (start, end, text) in set.replacements().into_iter().rev() {
        outcome
            .changes
            .extend(editor.replace(start..end, &text).changes);
        let inserted = text.chars().count();
        cursor = shifted(cursor, (start, end), inserted);
        anchor = shifted(anchor, (start, end), inserted);
    }
    if sealed {
        editor.seal();
    }
    editor.set_selection(cursor, anchor);
    Ok(outcome)
}
