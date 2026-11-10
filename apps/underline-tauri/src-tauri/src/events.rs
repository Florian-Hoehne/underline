//! The events the backend sends the window (ADR-022), typed for the
//! TypeScript bindings. Each carries the whole changed part of the state.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event as _;
use underline_session::{
    BlocksView, ChangedView, CloseQuestion, EnrichView, Event, FlushView, FoundView, HighlightView,
    OverviewItem, SectionsView, SettingsView, StatusView, TabPropertiesView, TabsView,
    TextReplaced, WorkspaceView,
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

/// UC-004 A7: the text of an open tab was replaced, such as after a change
/// from outside; the window asks for it again.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-replaced")]
pub struct TextReplacedEvent(pub TextReplaced);

/// M-08.2, ADR-024: the colours of the lines the source view shows.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-highlight")]
pub struct HighlightEvent(pub HighlightView);

/// M-08.8, M-12.2: the sections of the active tab's note, with what is
/// folded, after the input that changed them.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-sections")]
pub struct SectionsEvent(pub SectionsView);

/// M-11.3, UC-008 BR-008: the marked hits of the Nano search in the active
/// tab's note.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-found")]
pub struct FoundEvent(pub FoundView);

/// M-09, ADR-024: blocks of the active tab's facade changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "ed-blocks")]
pub struct BlocksEvent(pub BlocksView);

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

/// Sends `events` to the window. A failure is logged: the next change sends
/// the whole state again.
pub fn emit_all<R: Runtime>(app: &AppHandle<R>, events: Vec<Event>) {
    for event in events {
        let sent = match event {
            Event::Status(status) => StatusEvent(status).emit(app),
            Event::Settings(settings) => SettingsEvent(*settings).emit(app),
            Event::Workspace(workspace) => WorkspaceEvent(workspace).emit(app),
            Event::Overview(overview) => OverviewEvent(overview).emit(app),
            Event::Tabs(tabs) => TabsEvent(tabs).emit(app),
            Event::TextReplaced(replaced) => TextReplacedEvent(replaced).emit(app),
            Event::Highlight(highlight) => HighlightEvent(highlight).emit(app),
            Event::Sections(sections) => SectionsEvent(sections).emit(app),
            Event::Found(found) => FoundEvent(found).emit(app),
            Event::Properties(properties) => PropertiesEvent(properties).emit(app),
            Event::Blocks(blocks) => BlocksEvent(blocks).emit(app),
            Event::Changed(changed) => ChangedEvent(changed).emit(app),
            Event::Flush(flush) => FlushEvent(flush).emit(app),
            Event::Enrich(enrich) => EnrichEvent(*enrich).emit(app),
        };
        if let Err(error) = sent {
            eprintln!("An event could not be sent to the window: {error}");
        }
    }
}
