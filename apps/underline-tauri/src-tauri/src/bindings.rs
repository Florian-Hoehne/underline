//! Typed bindings for the window (plan section 4): `tauri-specta` registers
//! the commands and writes `ui/src/lib/bindings.ts` from their Rust types.
//!
//! Debug builds write the file on every start; the test below fails when the
//! checked-in file is outdated.

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Runtime;
use tauri_specta::{collect_commands, Builder};

use crate::commands;

/// Where the generated bindings live.
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/src/lib/bindings.ts");

/// All commands the window can call, with their types.
///
/// `disable_serde_phases()` exports one TypeScript type per DTO instead of a
/// `_Serialize`/`_Deserialize` pair. That is fine because the DTOs have one
/// shape in both directions. A DTO that ever serializes differently from how
/// it deserializes must not rely on it and needs its own phases.
pub fn builder<R: Runtime>() -> Builder<R> {
    Builder::new()
        .commands(collect_commands![commands::app_start])
        .disable_serde_phases()
}

/// Writes the TypeScript bindings of `builder` to `path`.
pub fn export<R: Runtime>(
    builder: &Builder<R>,
    path: impl AsRef<Path>,
) -> Result<(), specta_typescript::Error> {
    builder.export(Typescript::default(), path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Git may check the file out with CRLF line endings on Windows.
        let checked_in = std::fs::read_to_string(BINDINGS_PATH)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            checked_in == generated.replace("\r\n", "\n"),
            "{BINDINGS_PATH} is outdated: run `UNDERLINE_UPDATE_BINDINGS=1 cargo test -p \
             underline-tauri bindings` or start a debug build of the app, then commit the file"
        );
    }
}
