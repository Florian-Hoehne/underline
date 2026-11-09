//! What the session tells the window by itself (ADR-022). Every event
//! carries the whole changed part of the state, never a delta.

use serde::Serialize;

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
}
