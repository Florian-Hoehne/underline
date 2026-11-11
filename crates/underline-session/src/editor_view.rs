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
    /// Review F15: "Close anyway" opens the other workspace without the
    /// todos that cannot be stored (`ws_switch_anyway`); `what` is then
    /// "the workspace {name}".
    pub switching: bool,
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

/// UC-004 A2 to A4, M-07.3: what following a link asks of the window. The
/// status bar says what else happened (a heading not found, a file that
/// does not exist, a link leading outside the workspace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LinkAnswer {
    /// Nothing more for the window.
    Done,
    /// A2: the tab of `note`, active now, puts its cursor at the heading
    /// the link names and shows it there.
    Jump {
        /// The note whose tab jumps.
        note: String,
        /// The revision of the backend's text `offset` is in.
        rev: u32,
        /// Where the heading's text starts, in web units.
        offset: u32,
    },
    /// A4: the first web address of the session: "Open web address" asks
    /// whether to open it (`link_open_web`).
    AskWeb {
        /// The address, as the link has it.
        address: String,
    },
    /// A3: the linked note does not exist; "New note" offers to create
    /// it with its folder and title filled in.
    NewNote {
        /// The folder of the link, `""` for the top folder.
        folder: String,
        /// The title of the link.
        title: String,
        /// The default format of new notes, preselected (UC-003 step 2).
        format: crate::workspace_view::FormatName,
        /// "{title} does not exist yet. Create it?"
        problem: String,
    },
}

/// M-16.5, FR-041 to FR-043: one entry of the outline, the code index or
/// the figure index of the tool panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IndexEntryView {
    /// What the entry shows, at most 60 characters.
    pub label: String,
    /// The nesting of a heading, 0 for the highest heading level of the
    /// note; 0 for code and figures. Each level indents by 14 px.
    pub depth: u32,
    /// Where the block starts in the text, in web units of `rev`.
    pub offset: u32,
}

/// M-16.5, UC-004 step 4: "Outline", "Code" and "Figures" of an open
/// note, made from its blocks for one revision of the backend's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IndexesView {
    /// The note.
    pub note: String,
    /// The revision of the backend's text the lists were made from.
    pub rev: u32,
    /// The headings.
    pub outline: Vec<IndexEntryView>,
    /// The code blocks that are no diagrams.
    pub code: Vec<IndexEntryView>,
    /// The images and the diagrams.
    pub figures: Vec<IndexEntryView>,
}
