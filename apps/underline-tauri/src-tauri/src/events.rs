//! The events the backend sends the window (ADR-022), typed for the
//! TypeScript bindings. Each carries the whole changed part of the state.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event as _;
use underline_session::{
    ChangedView, ChangesView, CloseQuestion, DeletedNoteView, DiffView, EnrichView, Event,
    FlushView, HighlightView, IndexesView, OverviewItem, SearchView, SettingsView,
    SettingsWindowView, StatusView, TabPropertiesView, TabsView, WorkspaceView,
};

/// M-04.7: the status message changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "status")]
pub struct StatusEvent(pub StatusView);

/// UC-010, M-01.6: the settings in effect changed, with the palette of the
/// theme.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "settings")]
pub struct SettingsEvent(pub SettingsView);

/// UC-010 A5, M-20.2 to M-20.4: what the settings window shows changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "settings-window")]
pub struct SettingsWindowEvent(pub SettingsWindowView);

/// M-21.3, M-03.3, M-04.7: a workspace was opened; `null` while none is.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "workspace")]
pub struct WorkspaceEvent(pub Option<WorkspaceView>);

/// M-05.2: the note overview changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "overview")]
pub struct OverviewEvent(pub Vec<OverviewItem>);

/// M-06: the tab bar changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "tabs")]
pub struct TabsEvent(pub TabsView);

/// M-08.2, ADR-024: the colours of the lines the source view shows.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-highlight")]
pub struct HighlightEvent(pub HighlightView);

/// M-16.2: the properties of the active tab's note; `null` without one.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "properties")]
pub struct PropertiesEvent(pub Option<TabPropertiesView>);

/// M-21.4, UC-005 A14 step 3: closing Underline failed to store changes;
/// the window asks "Changes not stored".
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "close-question")]
pub struct CloseQuestionEvent(pub CloseQuestion);

/// ADR-025: the backend changed the text of a note; the window fetches the
/// change with `ed_sync`.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-changed")]
pub struct ChangedEvent(pub ChangedView);

/// ADR-025: the window sends its edits now, even none.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-flush")]
pub struct FlushEvent(pub FlushView);

/// ADR-025: what the backend made of the facade's blocks, by content.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-enrich")]
pub struct EnrichEvent(pub EnrichView);

/// M-16.5: the outline, code and figures of the active tab's note, for one
/// revision of its text.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "indexes")]
pub struct IndexesEvent(pub IndexesView);

/// M-05.6, UC-007 A5: the deleted notes of the workspace, most recently
/// deleted first.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "deleted")]
pub struct DeletedEvent(pub Vec<DeletedNoteView>);

/// M-13.1, UC-008 step 2: the text or the result of the workspace search
/// changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "search")]
pub struct SearchEvent(pub SearchView);

/// M-19.1, M-19.2, UC-011, UC-012: the git changes of the workspace;
/// `null` without git or before git answered.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "changes")]
pub struct ChangesEvent(pub Option<ChangesView>);

/// M-19.3, UC-011 step 6: the diff of the chosen file or the active note;
/// `null` without either.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "diff")]
pub struct DiffEvent(pub Option<DiffView>);

/// Sends `events` to the window. A failure is logged: the next change sends
/// the whole state again.
pub fn emit_all<R: Runtime>(app: &AppHandle<R>, events: Vec<Event>) {
    for event in events {
        let sent = match event {
            Event::Status(status) => StatusEvent(status).emit(app),
            Event::Settings(settings) => SettingsEvent(*settings).emit(app),
            Event::SettingsWindow(window) => SettingsWindowEvent(*window).emit(app),
            Event::Workspace(workspace) => WorkspaceEvent(workspace).emit(app),
            Event::Overview(overview) => OverviewEvent(overview).emit(app),
            Event::Tabs(tabs) => TabsEvent(tabs).emit(app),
            Event::Highlight(highlight) => HighlightEvent(highlight).emit(app),
            Event::Properties(properties) => PropertiesEvent(properties).emit(app),
            Event::Changed(changed) => ChangedEvent(changed).emit(app),
            Event::Flush(flush) => FlushEvent(flush).emit(app),
            Event::Enrich(enrich) => EnrichEvent(*enrich).emit(app),
            Event::Indexes(indexes) => IndexesEvent(indexes).emit(app),
            Event::Deleted(deleted) => DeletedEvent(deleted).emit(app),
            Event::Search(search) => SearchEvent(search).emit(app),
            Event::Changes(changes) => ChangesEvent(changes).emit(app),
            Event::Diff(diff) => DiffEvent(diff).emit(app),
        };
        if let Err(error) = sent {
            eprintln!("An event could not be sent to the window: {error}");
        }
    }
}
