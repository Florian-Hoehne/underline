//! Generates the Tauri context and the permissions of the app's own commands
//! (`allow-<command>`), which `capabilities/main.json` grants to the window.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_start",
        "settings_quick",
        "wiki_check",
        "ws_start",
        "ws_recent",
        "ws_open",
        "ws_check_new",
        "ws_create",
        "tab_open",
        "tab_activate",
        "tab_close",
        "tab_store",
        "ed_open",
        "ed_input",
        "ed_select",
        "ed_viewport",
        "ed_clipboard",
        "ed_apply",
        "ed_fold",
        "ed_keys_status",
        "clip_read_text",
        "clip_write_text",
        "app_close_anyway",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("the Tauri build script failed");
}
