//! What the window and the session exchange about exporting a note (UC-009,
//! inventory M-15): the kinds of export with their labels, where the
//! system's file dialog starts, its answer, and the answer of the export.

use serde::{Deserialize, Serialize};
use underline_core::NoteFormat;
use underline_facade::export::ExportKind;

/// UC-009 step 1: what a note is exported to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ExportKindName {
    /// The other note format (FR-014).
    OtherFormat,
    /// One HTML file (FR-015).
    Html,
    /// A PDF file (FR-016).
    Pdf,
}

impl From<ExportKindName> for ExportKind {
    fn from(kind: ExportKindName) -> Self {
        match kind {
            ExportKindName::OtherFormat => Self::OtherFormat,
            ExportKindName::Html => Self::Html,
            ExportKindName::Pdf => Self::Pdf,
        }
    }
}

impl From<ExportKind> for ExportKindName {
    fn from(kind: ExportKind) -> Self {
        match kind {
            ExportKind::OtherFormat => Self::OtherFormat,
            ExportKind::Html => Self::Html,
            ExportKind::Pdf => Self::Pdf,
        }
    }
}

/// One kind of export with the name the user chooses it by ("AsciiDoc",
/// "HTML", "PDF"; egui's `ExportKind::label`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportChoice {
    /// The kind.
    pub kind: ExportKindName,
    /// Its name.
    pub label: String,
}

impl ExportChoice {
    /// The kinds of export of a note in `format`, in egui's order.
    #[must_use]
    pub fn all(format: NoteFormat) -> Vec<Self> {
        ExportKind::ALL
            .iter()
            .map(|kind| Self {
                kind: (*kind).into(),
                label: kind.label(format).to_owned(),
            })
            .collect()
    }
}

/// M-05.4: the kinds of export by the note's format, for the submenu
/// "Export" of the note overview; the window knows each note's format from
/// the overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportKindsView {
    /// Of a Markdown note: AsciiDoc, HTML, PDF.
    pub markdown: Vec<ExportChoice>,
    /// Of an AsciiDoc note: Markdown, HTML, PDF.
    pub ascii_doc: Vec<ExportChoice>,
}

impl Default for ExportKindsView {
    fn default() -> Self {
        Self {
            markdown: ExportChoice::all(NoteFormat::Markdown),
            ascii_doc: ExportChoice::all(NoteFormat::AsciiDoc),
        }
    }
}

/// UC-009 step 1, BR-008: the window "Export note" asks "Export {title}
/// as:" with the kinds of export, the first one chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportStart {
    /// The note to export.
    pub note: String,
    /// Its title.
    pub title: String,
    /// The kinds, in egui's order.
    pub choices: Vec<ExportChoice>,
}

/// UC-009 step 2, ADR-016: what the system's file dialog answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogOutcome {
    /// The user chose this file.
    Chosen(std::path::PathBuf),
    /// A1: the user cancelled.
    Cancelled,
    /// No dialog could be shown, as on a Linux system without a desktop
    /// portal and without zenity.
    Unavailable,
}

/// The answer to asking where the export goes (UC-009 steps 2–3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportPlaceAnswer {
    /// Nothing more: the user cancelled (A1), or the status says why the
    /// note cannot be exported.
    Done,
    /// The user chose a file in the system's dialog; the backend keeps it,
    /// and the window runs the export once its edits are sent
    /// (`export_run` without a typed path).
    Chosen {
        /// The chosen file, for the window to show nothing of: only for
        /// tests and logs.
        path: String,
    },
    /// ADR-016: no dialog could be shown; the window "Export note" asks
    /// for the path in a field.
    Typed {
        /// "Export {title} as {label} to:".
        heading: String,
        /// The proposal: the folder of the last export, else the home
        /// folder, and the note's name with the extension (BR-007).
        path: String,
    },
}

/// The answer to running an export (UC-009 steps 4–5, A4, A5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportAnswer {
    /// The export ran or could not be made; the status says which.
    Done,
    /// A5: the place is a note's file; the status says "A note cannot be
    /// replaced by an export; choose another place", and the window asks
    /// for the place again.
    AskAgain,
    /// ADR-016, review F11: the path field stays with what the user typed
    /// and says why: an empty path or a folder (new texts, A-117), a note
    /// or a file of the workspace's own (A5, A-116), or why the export
    /// failed (UC-009 A4).
    Problem {
        /// The text under the field.
        problem: String,
    },
    /// The typed path holds a file: "The file exists. Export again to
    /// replace it." The window's button then says "Replace".
    Exists {
        /// The text under the field.
        problem: String,
        /// The path the user may now agree to replace; agreeing holds for
        /// this path only (inventory D-03).
        path: String,
    },
}

/// ADR-016: the path typed in the window "Export note" when no file dialog
/// could be shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TypedExportPath {
    /// The path in the field.
    pub path: String,
    /// The path the user agreed to replace with "Replace" (the `path` of
    /// [`ExportAnswer::Exists`]); `None` before. It holds only while the
    /// field holds the same path (inventory D-03).
    pub replace: Option<String>,
}

/// M-04.8: what the status bar offers for one export it reports: "Open"
/// and "Show", named by the note when several exports are reported
/// (A-120).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportOfferView {
    /// The title of the note exported.
    pub title: String,
    /// "Open": the file can be opened with the system's program
    /// (`export_open`); not after a failure, nor for a file without the
    /// kind's extension (A-119).
    pub open: bool,
    /// "Show": blocks were only approximated (`export_approximated`).
    pub show: bool,
}

/// M-15.6: what "Approximated blocks" lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportShown {
    /// The title of the note exported, for the window's heading.
    pub title: String,
    /// The lines of the approximated blocks, from 1.
    pub lines: Vec<u32>,
}
