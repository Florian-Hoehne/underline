//! Typed bindings for the window (plan section 4): `tauri-specta` registers
//! the commands and events and writes `ui/src/lib/bindings.ts` from their Rust
//! types. Next to them, the palettes of all themes are written to
//! `ui/src/lib/palettes.generated.json` for the mock backend and the
//! frontend's contrast test (plan section 14).
//!
//! Debug builds write both files on every start; the tests below fail when a
//! checked-in file is outdated.

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Runtime;
use tauri_specta::{collect_commands, collect_events, Builder};
use underline_session::PaletteView;

use crate::commands;
use crate::events::{
    CloseQuestionEvent, FoundEvent, HighlightEvent, OverviewEvent, PropertiesEvent, SectionsEvent,
    SettingsEvent, StatusEvent, TabsEvent, TextReplacedEvent, WorkspaceEvent,
};

/// Where the generated bindings live.
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/lib/bindings.ts");

/// Where the generated palettes live.
pub const PALETTES_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ui/src/lib/palettes.generated.json"
);

/// All commands the window can call and all events it gets, with their
/// types.
///
/// `disable_serde_phases()` exports one TypeScript type per DTO instead of a
/// `_Serialize`/`_Deserialize` pair. That is fine because the DTOs have one
/// shape in both directions. A DTO that ever serializes differently from how
/// it deserializes must not rely on it and needs its own phases.
pub fn builder<R: Runtime>() -> Builder<R> {
    Builder::new()
        .commands(collect_commands![
            commands::app_start,
            commands::settings_quick,
            commands::wiki_check,
            commands::ws_start,
            commands::ws_recent,
            commands::ws_open,
            commands::ws_check_new,
            commands::ws_create,
            commands::tab_open,
            commands::tab_activate,
            commands::tab_close,
            commands::tab_store,
            commands::ed_open,
            commands::ed_input,
            commands::ed_select,
            commands::ed_viewport,
            commands::ed_clipboard,
            commands::ed_apply,
            commands::ed_fold,
            commands::ed_keys_status,
            commands::clip_read_text,
            commands::clip_write_text,
            commands::app_close_anyway
        ])
        .events(collect_events![
            StatusEvent,
            SettingsEvent,
            WorkspaceEvent,
            OverviewEvent,
            TabsEvent,
            TextReplacedEvent,
            HighlightEvent,
            SectionsEvent,
            FoundEvent,
            PropertiesEvent,
            CloseQuestionEvent
        ])
        .disable_serde_phases()
}

/// Writes the TypeScript bindings of `builder` to `path`.
pub fn export<R: Runtime>(
    builder: &Builder<R>,
    path: impl AsRef<Path>,
) -> Result<(), specta_typescript::Error> {
    builder.export(Typescript::default(), path)
}

/// The palettes of all themes as JSON, in the order of the Theme menu.
pub fn palettes_json() -> String {
    let mut json = serde_json::to_string_pretty(&PaletteView::all())
        .expect("palettes are plain strings and flags");
    json.push('\n');
    json
}

/// Writes [`palettes_json`] to `path`.
pub fn export_palettes(path: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::write(path, palettes_json())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads a checked-in file; Git may check it out with CRLF line endings
    /// on Windows.
    fn checked_in(path: &str) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n")
    }

    /// Run with `UNDERLINE_UPDATE_BINDINGS=1` (or start a debug build of the
    /// app) to rewrite the checked-in file.
    #[test]
    fn checked_in_bindings_are_current() {
        let fresh =
            std::env::temp_dir().join(format!("underline-bindings-{}.ts", std::process::id()));
        export(&builder::<tauri::test::MockRuntime>(), &fresh).expect("the bindings export");
        let generated = std::fs::read_to_string(&fresh).expect("the export is readable");
        let _ = std::fs::remove_file(&fresh);

        if std::env::var_os("UNDERLINE_UPDATE_BINDINGS").is_some() {
            std::fs::write(BINDINGS_PATH, &generated).expect("the bindings are written");
            return;
        }
        assert!(
            checked_in(BINDINGS_PATH) == generated.replace("\r\n", "\n"),
            "{BINDINGS_PATH} is outdated: run `UNDERLINE_UPDATE_BINDINGS=1 cargo test -p \
             underline-tauri bindings` or start a debug build of the app, then commit the file"
        );
    }

    /// M-01.6: the palettes the frontend tests use are those of the themes.
    #[test]
    fn checked_in_palettes_are_current() {
        if std::env::var_os("UNDERLINE_UPDATE_BINDINGS").is_some() {
            export_palettes(PALETTES_PATH).expect("the palettes are written");
            return;
        }
        assert!(
            checked_in(PALETTES_PATH) == palettes_json(),
            "{PALETTES_PATH} is outdated: run `UNDERLINE_UPDATE_BINDINGS=1 cargo test -p \
             underline-tauri bindings` or start a debug build of the app, then commit the file"
        );
    }
}
