//! What the window and the session exchange about creating, renaming,
//! deleting and restoring notes (UC-003, UC-006, UC-007, inventory M-05.4
//! to M-05.6 and M-14): the dialogs' starting values, the requests and the
//! answers, and the deleted notes at the end of the note overview.

use serde::{Deserialize, Serialize};

use crate::workspace_view::FormatName;

/// UC-003 step 2: what the dialog "New note" starts with: the folder of the
/// active note and the default format of the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewNoteStart {
    /// The folder of the active note, `""` for the top folder.
    pub folder: String,
    /// The default format of new notes (UC-001 BR-001).
    pub format: FormatName,
    /// UC-003 A7, BR-008: the templates of the workspace the dialog offers
    /// besides "No template", by their place; none without templates.
    pub templates: Vec<TemplateView>,
    /// UC-003 BR-008: the template folder, as the settings give it
    /// (`templates`), for saying where templates are kept.
    pub template_folder: String,
}

/// UC-003 BR-008: one template the dialog "New note" offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TemplateView {
    /// The template's note, sent back in [`NewNoteRequest::template`].
    pub note: String,
    /// What the dialog shows: its title, with the folder below the template
    /// folder after it when it lies deeper ("Retro (work)").
    pub label: String,
    /// Its note format, which the new note gets (A7 step 1).
    pub format: FormatName,
}

/// UC-003 steps 2–3: what the user entered in "New note".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewNoteRequest {
    /// "Title:".
    pub title: String,
    /// "Folder (empty for the top folder, new folders are created):".
    pub folder: String,
    /// "Note format:".
    pub format: FormatName,
    /// UC-003 A7: "Template:", the template's note; `None` for "No
    /// template".
    #[serde(default)]
    pub template: Option<String>,
    /// UC-004 A18 (FR-096): the note opens to the side, as the link that
    /// offered it was followed (Ctrl+Shift+click, "Follow link to the
    /// side").
    #[serde(default)]
    pub side: bool,
}

/// The answer to "Create" in "New note" (UC-003 steps 4–6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CreateNoteAnswer {
    /// The dialog closes; why nothing was created is in the status (A5).
    Done,
    /// Step 6: the note is created and open in the active tab; its cursor
    /// goes to the end of its text, which the view shows with the keyboard.
    Created {
        /// The new note.
        note: String,
        /// The revision of the backend's text `offset` is in.
        rev: u32,
        /// The end of the text, in web units.
        offset: u32,
    },
    /// The dialog stays with what the user entered and says why (A1, A2,
    /// A4).
    Problem {
        /// Why, as the egui app says it.
        problem: String,
        /// A2, A3: the note that exists, for "Open the existing note".
        existing: Option<String>,
    },
}

/// UC-006 step 2: what the dialog "Rename note" starts with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RenameStart {
    /// The note to rename.
    pub note: String,
    /// Its title, selected when the dialog shows.
    pub title: String,
    /// Its folder, `""` for the top folder.
    pub folder: String,
    /// BR-009: it is the Home note, the start page of a GitHub wiki.
    pub home: bool,
}

/// The answer to "Rename" (UC-006 steps 4–7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RenameAnswer {
    /// The dialog closes; nothing changed (A4), or the status says why the
    /// note could not be renamed (A5).
    Done,
    /// Step 7: the note `from` is `to` now; its tab shows it under the new
    /// name, so the window's editor core of `from` goes on as `to`.
    Renamed {
        /// The note before.
        from: String,
        /// The note now.
        to: String,
    },
    /// The dialog stays with what the user entered and says why (A1–A3).
    Problem {
        /// Why, as the egui app says it.
        problem: String,
    },
}

/// UC-007 step 2: the question "Delete note".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeleteQuestion {
    /// The note to delete.
    pub note: String,
    /// Its title: "Delete the note {title}?".
    pub title: String,
    /// BR-001: "No links in other notes lead to it." or "{n} link(s) in
    /// {m} note(s) lead to it and will lead nowhere.".
    pub links: String,
    /// BR-010: it is the Home note, the start page of a GitHub wiki.
    pub home: bool,
}

/// The answer to "Delete" (UC-007 steps 4–5, A3, A8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DeleteAnswer {
    /// The question closes: the note is deleted, or the status says why
    /// not (A2, A3).
    Done,
    /// A8: the note cannot be kept among the deleted notes; the question
    /// asks to delete it for good.
    CannotKeep {
        /// "The note cannot be kept among the deleted notes ({reason}), so
        /// it cannot be restored. Delete it for good?"
        question: String,
    },
}

/// UC-007 A5 step 1, M-05.6: a note among the deleted notes, most recently
/// deleted first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeletedNoteView {
    /// Names the deleted note for `deleted_restore` and `deleted_purge`.
    pub key: String,
    /// The note's title.
    pub title: String,
    /// The note's folder, "top folder" for the top one.
    pub folder: String,
    /// What resting the pointer on the title shows: "{folder} · deleted
    /// {local time}", the folder "top folder" for the top one.
    pub hover: String,
    /// When it was deleted, local time to the minute.
    pub deleted: String,
}

/// The answer to restoring a deleted note (UC-007 A4–A6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RestoreAnswer {
    /// Nothing was restored; the status says why, if anything happened.
    Done,
    /// "Restored {t}": the note is back, as `note`.
    Restored {
        /// The restored note.
        note: String,
    },
    /// A6: its place is taken; "Restore note" asks for another title or
    /// folder, or again after a problem.
    Ask {
        /// "Title:".
        title: String,
        /// "Folder (empty for the top folder, new folders are created):".
        folder: String,
        /// Why, as the egui app says it.
        problem: String,
    },
}

/// UC-025 steps 1 and 4, A1, A2: the answer to asking for a daily note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DailyNoteAnswer {
    /// No note opened; the status bar says why (A2, A4, A5).
    Done,
    /// A2, OQ-271: the active note is no daily note, so there is none
    /// before or after it; the window says so with the key of today's.
    NotDaily,
    /// The daily note `note` is in the active tab, its cursor where it was:
    /// its open tab became the active tab (A1), or the daily note of
    /// another day opened (A2). The view takes the keyboard.
    Shown {
        /// The daily note.
        note: String,
    },
    /// The daily note is open in the active tab; its cursor goes to
    /// `offset`, which the view shows with the keyboard (step 4, A1).
    Opened {
        /// The daily note.
        note: String,
        /// The revision of the backend's text `offset` is in.
        rev: u32,
        /// Where the template put the cursor, else the end of the text, in
        /// web units.
        offset: u32,
    },
}

/// What the status bar offers beside the status message, while it shows
/// the report the offer belongs to (M-04.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StatusOffer {
    /// UC-007 A4, M-14.3: "Undo" right after deleting a note
    /// (`note_undo_delete`).
    UndoDelete,
    /// UC-020 A5, M-04.6: "Undo" before the report "Deleted todo {name}"
    /// (`todo_undo`).
    UndoTodoDelete,
    /// UC-021 step 8, M-04.6: "Undo" before the report "Marked todo {name}
    /// done/open" (`todo_undo`).
    UndoTodoMark,
    /// UC-022 A6: "Undo" after the report of a replacement in the workspace
    /// (`replace_undo`).
    UndoReplace,
    /// UC-026 A3: "Undo" after the report of a restored snapshot
    /// (`snapshot_undo_restore`).
    UndoRestore,
    /// UC-025 A4: "Open settings" after the report that the
    /// daily note folder is not allowed (`settings_open`, "This
    /// workspace").
    WorkspaceSettings,
}
