//! What the window learns about the open workspace, its note overview and
//! its tabs, and the answers of the Open and Create dialogs (UC-001,
//! UC-002). Paths cross as display strings.

use serde::{Deserialize, Serialize};
use underline_core::{EditingView, NoteFormat, PaneSide};

/// The open workspace as the window names it (M-03.3, M-04.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    /// The workspace's directory, as the status bar shows it.
    pub root: String,
    /// The folder name, which the top bar shows without the menu bar.
    pub name: String,
}

/// One line of the note overview (M-05.2, UC-002 BR-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OverviewItem {
    /// The note, by its id (`guide/Getting-Started`), which the window sends
    /// back to open it.
    pub note: String,
    /// The title, with the folder when the title occurs more than once.
    pub label: String,
    /// The title alone, which go to note matches (UC-004 A12, BR-014).
    pub title: String,
    /// The folder, `None` for the top folder, which go to note matches and
    /// shows (UC-004 A12 step 2).
    pub folder: Option<String>,
    /// The note's format, which names the kinds of export of its menu
    /// (M-05.4, `StartInfo::export_kinds`).
    pub format: FormatName,
    /// M-05.3, UC-011 step 2: the letter of its git change after the title
    /// (`N`, `M`, `R`, `C`), and `M` for an open note with changes not
    /// stored yet; only in a git workspace.
    pub mark: Option<String>,
}

/// The editing view a tab shows its note in (UC-004 BR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum EditingViewName {
    /// The markup as it is stored.
    Source,
    /// The formatted note.
    Facade,
}

impl From<EditingView> for EditingViewName {
    fn from(view: EditingView) -> Self {
        match view {
            EditingView::Source => Self::Source,
            EditingView::Facade => Self::Facade,
        }
    }
}

impl From<EditingViewName> for EditingView {
    fn from(view: EditingViewName) -> Self {
        match view {
            EditingViewName::Source => Self::Source,
            EditingViewName::Facade => Self::Facade,
        }
    }
}

/// UC-004 BR-018: a pane of the split view; without it, every tab is in
/// the left one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum PaneName {
    /// The left pane, the only one without the split view.
    #[default]
    Left,
    /// The right pane of the split view.
    Right,
}

impl From<PaneSide> for PaneName {
    fn from(side: PaneSide) -> Self {
        match side {
            PaneSide::Left => Self::Left,
            PaneSide::Right => Self::Right,
        }
    }
}

impl From<PaneName> for PaneSide {
    fn from(name: PaneName) -> Self {
        match name {
            PaneName::Left => Self::Left,
            PaneName::Right => Self::Right,
        }
    }
}

/// UC-004 BR-018, FR-096: the split view while it is shown: the right
/// pane's tabs, the active tab of each pane and the active pane. The left
/// pane's tabs are [`TabsView::tabs`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SplitView {
    /// The tabs of the right pane, left to right.
    pub right: Vec<TabView>,
    /// The note of the left pane's active tab.
    pub left_active: Option<String>,
    /// The note of the right pane's active tab.
    pub right_active: Option<String>,
    /// The active pane: the one the user last worked in.
    pub pane: PaneName,
    /// OQ-220: the share of the width the left pane takes, in tenths of a
    /// percent (500 is half), as remembered for the workspace; the window
    /// keeps each pane at its least width besides.
    pub left_share: u16,
}

/// One tab of the tab bar (M-06.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabView {
    /// The note the tab shows, by its id.
    pub note: String,
    /// The note's title, without marks.
    pub title: String,
    /// The title, marked " ⚠" when the last save failed, else " •" with
    /// changes not stored, and " (deleted)" when the note was deleted
    /// outside.
    pub label: String,
    /// The note's file, shown when the pointer rests on the tab.
    pub path: String,
    /// The editing view of the tab (M-06.7).
    pub view: EditingViewName,
    /// M-10.8: why the last save failed, for the line above the editor
    /// "Not stored: {err}. Underline keeps trying."; `None` when it did not.
    pub save_error: Option<String>,
}

/// The tab "Todos" (M-06.5, UC-021 BR-005), as it is remembered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoTabView {
    /// The tab is open.
    pub open: bool,
    /// The tab is the active one of its pane; no note tab of that pane is
    /// then.
    pub active: bool,
    /// OQ-279: the pane the tab is in: the one active when it was opened;
    /// the left one without the split view.
    pub pane: PaneName,
}

/// The tab bar (M-06): the note tabs left to right and the active one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabsView {
    /// The note tabs, left to right; with the split view, those of the left
    /// pane.
    pub tabs: Vec<TabView>,
    /// The note of the active note tab, of the active pane with the split
    /// view (UC-004 BR-018); `None` without tabs. The note overview marks it
    /// even while the tab "Todos" is the active one; the tab bar then marks
    /// the tab "Todos" instead.
    pub active: Option<String>,
    /// The tab "Todos"; its list comes with feature F15.
    pub todo_list: TodoTabView,
    /// FR-096: the split view, while it is shown.
    pub split: Option<SplitView>,
}

/// The workspace part of the window at once: what `ws_start` returns, so
/// the window leaves its loading state with everything in place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    /// The open workspace, if any.
    pub workspace: Option<WorkspaceView>,
    /// The note overview.
    pub overview: Vec<OverviewItem>,
    /// The tab bar.
    pub tabs: TabsView,
}

/// What the Open dialog offers besides choosing another directory
/// (M-21.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenOffer {
    /// UC-002 A3, A4: "Create a new workspace there".
    Create {
        /// The directory that does not exist.
        directory: String,
    },
    /// UC-002 A5, A6: "Open {outer}" and "Open {inner} anyway".
    Outer {
        /// The workspace the directory lies in.
        outer: String,
        /// The chosen directory.
        inner: String,
    },
}

/// The answer to choosing a directory to open (UC-002 step 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenAnswer {
    /// The dialog closes: the workspace is open now, was open already (A9),
    /// or there was nothing to open; a failure is in the status.
    Done,
    /// The dialog shows what stands in the way (A3, A5, A7, A11).
    Problem {
        /// The text of the path field.
        path: String,
        /// Why the directory cannot be opened, as the egui app says it.
        problem: String,
        /// What the dialog offers instead.
        offer: Option<OpenOffer>,
        /// The recently opened workspaces, without one that is gone (A3).
        recent: Vec<String>,
    },
}

/// What `ws_start` did (M-01.1): the answer, and the workspace part of the
/// window afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartOpened {
    /// `Problem` shows the Open dialog, as the egui app does at start.
    pub answer: OpenAnswer,
    /// The workspace, overview and tabs after opening.
    pub snapshot: WorkspaceSnapshot,
}

/// The stage the Create dialog goes to (M-21.2, UC-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum CreateAnswer {
    /// The dialog closes: the workspace was created, or creating it failed
    /// and the status says why (A5, A6).
    Done,
    /// Step 3: the directory may take the workspace; the format and the git
    /// option are asked next.
    Options {
        /// The directory, as typed.
        directory: String,
    },
    /// Step 1 again, saying why the directory was refused (A1, A7).
    Directory {
        /// The text of the path field.
        path: String,
        /// Why, as the egui app says it.
        problem: String,
        /// A1, A2: the directory that holds files, to open it instead.
        offer: Option<String>,
    },
}

/// A note format as the Create dialog names it (UC-001 BR-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum FormatName {
    /// Markdown, preselected.
    Markdown,
    /// AsciiDoc.
    AsciiDoc,
}

impl From<FormatName> for NoteFormat {
    fn from(format: FormatName) -> Self {
        match format {
            FormatName::Markdown => Self::Markdown,
            FormatName::AsciiDoc => Self::AsciiDoc,
        }
    }
}

impl From<NoteFormat> for FormatName {
    fn from(format: NoteFormat) -> Self {
        match format {
            NoteFormat::Markdown => Self::Markdown,
            NoteFormat::AsciiDoc => Self::AsciiDoc,
        }
    }
}

/// What the user chose in the Create dialog (UC-001 steps 1–4, A3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewWorkspaceRequest {
    /// The directory, as the Options stage named it.
    pub directory: String,
    /// The default format of new notes.
    pub format: FormatName,
    /// "Create as git repository".
    pub git: bool,
    /// The address of the git remote; empty for none. Only used with `git`.
    pub remote: String,
}
