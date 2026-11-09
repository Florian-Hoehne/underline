//! Spike F0: the Tauri shell. Variant A keeps the owner of the text here and
//! answers every input with a patch; variant B only mirrors the edits the
//! WASM owner in the webview made.
//!
//! `F0_AUTORUN=1 f0-editor` runs the whole measurement, writes the results to
//! `F0_OUT` (or `results/run-<time>.json`) and quits.

use std::time::Instant;

use f0_model::{
    BenchPosition, FacadeInput, FacadePatch, Mirror, MirrorEdit, MirrorError, Owner, SourceInput,
    SourcePatch,
};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, State};

/// The two copies of the text.
struct AppState {
    /// Variant A: the owner.
    a: Mutex<Owner>,
    /// Variant B: the backend's mirror of the WASM owner.
    b: Mutex<Mirror>,
    note: String,
}

/// A patch with the time the owner took, in microseconds.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Timed<T> {
    patch: T,
    owner_us: f64,
}

fn timed<T>(f: impl FnOnce() -> T) -> Timed<T> {
    let start = Instant::now();
    let patch = f();
    Timed {
        patch,
        owner_us: start.elapsed().as_secs_f64() * 1e6,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    autorun: bool,
    keys: Option<usize>,
    /// Comma-separated owners to measure (`A,B,A-async`).
    variants: Option<String>,
    /// Whether to run the extra scenarios (highlighting, no content-visibility).
    extras: bool,
    /// Whether to run the correctness checks.
    checks: bool,
    /// Only take the facade's DOM work apart (`F0_DIAG=<blocks or 0>`).
    diag: Option<usize>,
}

#[tauri::command]
fn config() -> Config {
    Config {
        autorun: std::env::var_os("F0_AUTORUN").is_some(),
        keys: std::env::var("F0_KEYS").ok().and_then(|k| k.parse().ok()),
        variants: std::env::var("F0_VARIANTS").ok(),
        extras: std::env::var_os("F0_NO_EXTRAS").is_none(),
        checks: std::env::var_os("F0_NO_CHECKS").is_none(),
        diag: std::env::var("F0_DIAG").ok().and_then(|d| d.parse().ok()),
    }
}

#[tauri::command]
fn note(state: State<'_, AppState>) -> String {
    state.note.clone()
}

#[tauri::command]
fn a_open(state: State<'_, AppState>) {
    *state.a.lock() = Owner::new(&state.note, false);
}

#[tauri::command]
fn a_source_input(state: State<'_, AppState>, input: SourceInput) -> Timed<SourcePatch> {
    let mut owner = state.a.lock();
    timed(|| owner.source_input(&input))
}

#[tauri::command]
fn a_facade_input(state: State<'_, AppState>, input: FacadeInput) -> Timed<FacadePatch> {
    let mut owner = state.a.lock();
    timed(|| owner.facade_input(&input))
}

/// The same as `a_facade_input`, but off the main thread (Tauri runs async
/// commands on its thread pool).
#[tauri::command]
async fn a_facade_input_async(
    state: State<'_, AppState>,
    input: FacadeInput,
) -> Result<Timed<FacadePatch>, String> {
    let mut owner = state.a.lock();
    Ok(timed(|| owner.facade_input(&input)))
}

#[tauri::command]
async fn a_source_input_async(
    state: State<'_, AppState>,
    input: SourceInput,
) -> Result<Timed<SourcePatch>, String> {
    let mut owner = state.a.lock();
    Ok(timed(|| owner.source_input(&input)))
}

#[tauri::command]
fn a_render_all(state: State<'_, AppState>) -> String {
    state.a.lock().render_all()
}

#[tauri::command]
fn a_render_range(state: State<'_, AppState>, from: usize, to: usize) -> Vec<String> {
    state.a.lock().render_range(from, to)
}

#[tauri::command]
fn a_block_count(state: State<'_, AppState>) -> usize {
    state.a.lock().block_count()
}

#[tauri::command]
fn a_text(state: State<'_, AppState>) -> String {
    state.a.lock().text()
}

#[tauri::command]
fn a_block_text(state: State<'_, AppState>, index: usize) -> String {
    state.a.lock().block_text(index)
}

#[tauri::command]
fn a_bench_positions(state: State<'_, AppState>) -> Vec<BenchPosition> {
    state.a.lock().bench_positions()
}

#[tauri::command]
fn b_open(state: State<'_, AppState>) {
    *state.b.lock() = Mirror::new(&state.note);
}

/// Variant B: takes the edits of the WASM owner.
#[tauri::command]
async fn b_mirror(state: State<'_, AppState>, edits: Vec<MirrorEdit>) -> Result<u64, MirrorError> {
    state.b.lock().apply(&edits)
}

#[tauri::command]
fn b_text(state: State<'_, AppState>) -> String {
    state.b.lock().text()
}

#[tauri::command]
fn b_hash(state: State<'_, AppState>) -> String {
    f0_model::hash(&state.b.lock().text())
}

#[tauri::command]
fn hash_of(text: String) -> String {
    f0_model::hash(&text)
}

/// Writes the results of a run; returns the file's path.
#[tauri::command]
fn save_results(json: String) -> Result<String, String> {
    let path = std::env::var("F0_OUT").unwrap_or_else(|_| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("results/run-{secs}.json")
    });
    std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
    Ok(path)
}

#[tauri::command]
fn log(message: String) {
    println!("[f0] {message}");
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn main() {
    let text = f0_model::note::generate();
    tauri::Builder::default()
        .manage(AppState {
            a: Mutex::new(Owner::new(&text, false)),
            b: Mutex::new(Mirror::new(&text)),
            note: text,
        })
        .invoke_handler(tauri::generate_handler![
            config,
            note,
            a_open,
            a_source_input,
            a_facade_input,
            a_source_input_async,
            a_facade_input_async,
            a_render_all,
            a_render_range,
            a_block_count,
            a_text,
            a_block_text,
            a_bench_positions,
            b_open,
            b_mirror,
            b_text,
            b_hash,
            hash_of,
            save_results,
            log,
            quit,
        ])
        .run(tauri::generate_context!())
        .expect("the spike window failed");
}
