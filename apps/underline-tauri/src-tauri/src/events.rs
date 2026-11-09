//! The events the backend sends the window (ADR-022), typed for the
//! TypeScript bindings. Each carries the whole changed part of the state.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event as _;
use underline_session::{Event, SettingsView, StatusView};

/// M-04.7: the status message changed.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "status")]
pub struct StatusEvent(pub StatusView);

/// UC-010, M-01.6: the settings in effect changed, with the palette of the
/// theme.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "settings")]
pub struct SettingsEvent(pub SettingsView);

/// Sends `events` to the window. A failure is logged: the next change sends
/// the whole state again.
pub fn emit_all<R: Runtime>(app: &AppHandle<R>, events: Vec<Event>) {
    for event in events {
        let sent = match event {
            Event::Status(status) => StatusEvent(status).emit(app),
            Event::Settings(settings) => SettingsEvent(*settings).emit(app),
        };
        if let Err(error) = sent {
            eprintln!("An event could not be sent to the window: {error}");
        }
    }
}
