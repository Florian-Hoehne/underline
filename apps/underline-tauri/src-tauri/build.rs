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
        "tab_set_view",
        "ed_viewport",
        "ed_sync",
        "ed_resync",
        "ed_mirror",
        "app_request_close",
        "clip_read_text",
        "clip_write_text",
        "clip_read",
        "clip_write",
        "clip_store_image",
        "clip_forget",
        "ed_embed",
        "ed_drop",
        "ed_drag",
        "ed_note_link",
        "app_close_anyway",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("the Tauri build script failed");
}
