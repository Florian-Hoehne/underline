//! What the session tells the window by itself (ADR-022). Every event
//! carries the whole changed part of the state, never a delta.

use serde::Serialize;

use crate::editor_view::{HighlightView, IndexesView, TabPropertiesView};
use crate::notes_view::{DeletedNoteView, StatusOffer};
use crate::sync_view::{ChangedView, EnrichView, FlushView};
use crate::workspace_view::{OverviewItem, TabsView, WorkspaceView};
use crate::SettingsView;

/// The status message of the status bar (M-04.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    /// The text, as the egui app shows it; empty when there is none.
    pub text: String,
    /// What the status bar offers beside the text, such as "Undo" after
    /// deleting a note (UC-007 A4); it belongs to this text.
    pub offer: Option<StatusOffer>,
}

/// A change the window has to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The status message changed.
    Status(StatusView),
    /// The settings in effect changed (UC-010); boxed, as it is much larger
    /// than a status.
    Settings(Box<SettingsView>),
    /// A workspace was opened (M-21.3); `None` while none is open.
    Workspace(Option<WorkspaceView>),
    /// The note overview changed (M-05.2).
    Overview(Vec<OverviewItem>),
    /// The tab bar changed (M-06).
    Tabs(TabsView),
    /// The colours of the lines the source view shows (M-08.2).
    Highlight(HighlightView),
    /// The properties section of the tool panel changed (M-16.2); `None`
    /// without an active note tab.
    Properties(Option<TabPropertiesView>),
    /// ADR-025: the backend changed the mirror of a note (`ed-changed`).
    Changed(ChangedView),
    /// ADR-025: the window is to send its edits now (`ed-flush`).
    Flush(FlushView),
    /// ADR-025: what the backend made of the facade's blocks (`ed-enrich`).
    Enrich(Box<EnrichView>),
    /// M-16.5: the outline, code and figures of the active tab's note.
    Indexes(IndexesView),
    /// M-05.6, UC-007 A5: the deleted notes of the workspace changed.
    Deleted(Vec<DeletedNoteView>),
}
