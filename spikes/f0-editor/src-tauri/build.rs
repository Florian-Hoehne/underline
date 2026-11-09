//! The spike's own commands get `allow-<command>` permissions, which
//! `capabilities/main.json` grants to the window.

const COMMANDS: &[&str] = &[
    "config",
    "note",
    "a_open",
    "a_source_input",
    "a_facade_input",
    "a_source_input_async",
    "a_facade_input_async",
    "a_render_all",
    "a_render_range",
    "a_block_count",
    "a_text",
    "a_block_text",
    "a_bench_positions",
    "b_open",
    "b_mirror",
    "b_text",
    "b_hash",
    "hash_of",
    "save_results",
    "log",
    "quit",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("the Tauri build script failed");
}
