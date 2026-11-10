//! What the session tells the window by itself (ADR-022). Every event
//! carries the whole changed part of the state, never a delta.

use serde::Serialize;

use crate::editor_view::{FoundView, HighlightView, SectionsView, TabPropertiesView};
use crate::facade_view::BlocksView;
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
}

/// The session replaced the text of an open tab by itself, such as after a
/// change from outside (UC-004 A7); the window asks for it again
/// (`ed_open`) unless it shows this revision already.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextReplaced {
    /// The note of the tab.
    pub note: String,
    /// The revision of the new text.
    pub rev: u32,
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
    /// The text of an open tab was replaced (UC-004 A7).
    TextReplaced(TextReplaced),
    /// The colours of the lines the source view shows (M-08.2).
    Highlight(HighlightView),
    /// The sections of the active tab's note with what is folded (M-08.8,
    /// M-12.2).
    Sections(SectionsView),
    /// The marked hits of the Nano search in the active tab (M-11.3).
    Found(FoundView),
    /// The properties section of the tool panel changed (M-16.2); `None`
    /// without an active note tab.
    Properties(Option<TabPropertiesView>),
    /// Blocks of the active tab's facade changed (M-09, ADR-024).
    Blocks(BlocksView),
    /// ADR-025: the backend changed the mirror of a note (`ed-changed`).
    Changed(ChangedView),
    /// ADR-025: the window is to send its edits now (`ed-flush`).
    Flush(FlushView),
    /// ADR-025: what the backend made of the facade's blocks (`ed-enrich`).
    Enrich(Box<EnrichView>),
}
