//! What the window and the session exchange about the text of an open note:
//! the editor's data (`underline_editor_view::editor_view`, ADR-025), under
//! its former path, and what only the session knows of a tab (its
//! properties, the question before closing).

use serde::{Deserialize, Serialize};

pub use underline_editor_view::editor_view::*;

/// M-16.2, UC-004 BR-010: the properties of a stored note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NotePropertiesView {
    /// "Place": the place in the workspace, such as `guide/Install.adoc`.
    pub place: String,
    /// "Format": "Markdown" or "AsciiDoc".
    pub format: String,
    /// "Size", such as `2 KB`.
    pub size: String,
    /// "Last change", local time to the minute.
    pub last_change: String,
}

/// M-16.2: the properties section of the tool panel for the active tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabPropertiesView {
    /// The note of the active tab.
    pub note: String,
    /// `None` when the file cannot be read: "Not saved yet".
    pub properties: Option<NotePropertiesView>,
}

/// M-06.3, M-21.4, UC-005 A14 step 3: "Changes not stored": closing failed
/// to store, and the user decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CloseQuestion {
    /// The note of the tab to close; `None` when Underline closes.
    pub tab: Option<String>,
    /// "the tab {title}" or "Underline", for "Close {what} anyway and lose
    /// these changes?".
    pub what: String,
    /// Why, for "Changes could not be stored: {error}".
    pub error: String,
}

/// M-10.5, UC-005 A9: files dragged over the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DragName {
    /// They came into the window.
    Over,
    /// They left it without a drop.
    Left,
    /// They were dropped while no note is shown.
    NoNote,
}

/// UC-005 A9 to A13, A19, M-10.4, M-10.5: what putting a file into a note
/// gave the window: something to insert at the cursor, the question about
/// a large file, or nothing (the status bar says why).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EmbedAnswer {
    /// Inserts `insertion` at the cursor.
    Insert {
        /// What goes into the text.
        insertion: Insertion,
    },
    /// A19: the file is larger than 10 MB; the image dialog asks whether to
    /// copy it or only to link to where it is.
    Large {
        /// The file as the user gave it; "Link to its place" links to it.
        path: String,
        /// "{file} has N MB. Copy it into the workspace, or only link to
        /// where it is (it is then not part of the workspace and not pushed
        /// with git)?"
        question: String,
    },
    /// Nothing is inserted; the status bar says why.
    Refused,
}
