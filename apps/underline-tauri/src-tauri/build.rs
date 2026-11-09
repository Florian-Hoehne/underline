//! Generates the Tauri context and the permissions of the app's own commands
//! (`allow-<command>`), which `capabilities/main.json` grants to the window.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&["app_start"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("the Tauri build script failed");
}
