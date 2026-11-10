//! The commands the window calls: a thin layer over `underline_session`.
//!
//! Every command returns `Result<_, UiError>`, so the window gets the same
//! texts the egui app shows. A command that changes the session wakes the
//! tick, which sends the changes to the window as events (ADR-022).
//!
//! A new command is registered in three places: `bindings::builder` (handler
//! and TypeScript bindings), `build.rs` (its `allow-…` permission) and
//! `capabilities/main.json` (granted to the window); the tests below check
//! that they agree. Commands that do file, git or search I/O must be `async`
//! (or use `tauri::async_runtime::spawn_blocking`) and hold the session lock
//! only briefly.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_specta::Event as _;
use underline_core::NoteId;
use underline_session::{
    CloseQuestion, CreateAnswer, EditingViewName, MirrorView, NewWorkspaceRequest, OpenAnswer,
    Session, SettingName, StartInfo, StartOpened, SyncAnswer, SyncBatch, Synced, UiError,
    UiErrorKind,
};

use crate::clipboard::SystemClipboard;
use crate::events::CloseQuestionEvent;
use crate::ticker::Waker;

/// The session as the app manages it: shared, so a command can hand it to
/// the blocking pool.
pub type SharedSession = Arc<Mutex<Session>>;

/// M-01: what the window needs to know when it starts: the workspace folder
/// from the command line, the known workspaces, the operating system, the
/// settings with the theme's palette and the status message.
#[tauri::command]
#[specta::specta]
pub fn app_start(session: State<'_, SharedSession>) -> Result<StartInfo, UiError> {
    Ok(session.lock().start_info())
}

/// UC-010 A3, A4, M-20.5: changes a setting from outside the settings
/// window, such as the theme from the Theme menu (M-03.6) or the menu bar by
/// the burger button (M-03.1). It applies at once and is stored at once; a
/// failure is said in the status bar.
///
/// Storing writes a file, so it runs on the blocking pool, not on an async
/// worker (ADR-022). The session stays locked while the one settings file
/// is written, which keeps the setting and its file in step.
#[tauri::command]
#[specta::specta]
pub async fn settings_quick(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    key: SettingName,
    value: String,
) -> Result<(), UiError> {
    let session = Arc::clone(&session);
    let waker = waker.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        session.lock().quick_setting(key, value);
        waker.wake();
    })
    .await
    .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// Runs `work` on the session on the blocking pool, as file and git I/O
/// must not run on an async worker (ADR-022), and wakes the tick after it,
/// which sends the changes to the window as events.
async fn on_session<T: Send + 'static>(
    session: &State<'_, SharedSession>,
    waker: &State<'_, Waker>,
    work: impl FnOnce(&mut Session) -> T + Send + 'static,
) -> Result<T, UiError> {
    let session = Arc::clone(session);
    let waker = waker.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = work(&mut session.lock());
        waker.wake();
        result
    })
    .await
    .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// Runs `work`, which only reads the session, on the blocking pool; the
/// tick is not woken, as nothing changed.
async fn read_session<T: Send + 'static>(
    session: &State<'_, SharedSession>,
    work: impl FnOnce(&Session) -> T + Send + 'static,
) -> Result<T, UiError> {
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || work(&session.lock()))
        .await
        .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// M-01.1, UC-002 A1/A2: opens the workspace given on the command line,
/// else the one opened last. The window shows its loading state until the
/// answer comes; `Problem` shows the Open dialog with what stands in the
/// way. Only the first call opens anything.
#[tauri::command]
#[specta::specta]
pub async fn ws_start(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
) -> Result<StartOpened, UiError> {
    let shared = Arc::clone(&session);
    // The directory is checked and read without holding the session (A-22).
    let prepared = blocking(move || {
        let plan = shared.lock().plan_start();
        plan.map(|(plan, dir)| plan.prepare(&dir, false))
    })
    .await?;
    on_session(&session, &waker, move |session| {
        session.finish_start(prepared)
    })
    .await
}

/// Runs `work` on the blocking pool.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, UiError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// M-21.1, UC-002 step 2: the recently opened workspaces, the most recent
/// first, for the Open dialog.
#[tauri::command]
#[specta::specta]
pub async fn ws_recent(session: State<'_, SharedSession>) -> Result<Vec<String>, UiError> {
    read_session(&session, Session::recent_workspaces).await
}

/// M-21.1, UC-002 steps 4–9: opens the directory the user chose in the Open
/// dialog, or says what stands in the way. With `anyway`, a directory inside
/// a workspace is opened all the same (A5 step 2, A6).
#[tauri::command]
#[specta::specta]
pub async fn ws_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    path: String,
    anyway: bool,
) -> Result<OpenAnswer, UiError> {
    let plan = read_session(&session, Session::plan_open).await?;
    // Checking and reading the directory runs without the session (A-22).
    let prepared = blocking(move || plan.prepare(&PathBuf::from(path), anyway)).await?;
    on_session(&session, &waker, move |session| {
        session.finish_open(prepared)
    })
    .await
}

/// M-21.2, UC-001 step 2: checks the directory of the Create dialog's first
/// stage; the answer is the next stage or the first one with the problem.
#[tauri::command]
#[specta::specta]
pub async fn ws_check_new(
    session: State<'_, SharedSession>,
    path: String,
) -> Result<CreateAnswer, UiError> {
    read_session(&session, move |session| session.check_new_directory(&path)).await
}

/// M-21.2, UC-001 step 5, A3: creates the workspace and opens it with its
/// Home note; a failure is said in the status bar (A5, A6).
#[tauri::command]
#[specta::specta]
pub async fn ws_create(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    request: NewWorkspaceRequest,
) -> Result<CreateAnswer, UiError> {
    let plan = on_session(&session, &waker, move |session| {
        session.plan_create(request)
    })
    .await?;
    let plan = match plan {
        Ok(plan) => plan,
        Err(answer) => return Ok(answer),
    };
    // Creating the directory and its git repository runs without the
    // session (A-22).
    let prepared = blocking(move || plan.prepare()).await?;
    on_session(&session, &waker, move |session| {
        session.finish_create(prepared)
    })
    .await
}

/// M-05.2, UC-004 step 2: opens the note `note` of the overview in a tab, or
/// activates its tab, and moves it to the top of the overview.
#[tauri::command]
#[specta::specta]
pub async fn tab_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.open_note(&NoteId::from_relative(&note));
    })
    .await
}

/// M-06.2, UC-004 A5: activates the tab of `note`.
#[tauri::command]
#[specta::specta]
pub async fn tab_activate(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.activate_tab(&NoteId::from_relative(&note));
    })
    .await
}

/// M-06.3, UC-004 A5: closes the tab of `note`, storing its changes first.
/// When they cannot be stored, the tab stays and the answer is the question
/// "Changes not stored" (UC-005 A14 step 3); with `force` ("Close anyway")
/// the tab closes all the same.
#[tauri::command]
#[specta::specta]
pub async fn tab_close(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    force: bool,
) -> Result<Option<CloseQuestion>, UiError> {
    on_session(&session, &waker, move |session| {
        session.close_tab(&NoteId::from_relative(&note), force)
    })
    .await
}

/// M-10.9, UC-005 BR-009: stores the note of the tab at once; the status
/// bar says "Stored {t}" or why it could not be stored.
#[tauri::command]
#[specta::specta]
pub async fn tab_store(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.store_tab(&NoteId::from_relative(&note));
    })
    .await
}

/// M-07.2, UC-004 BR-003: the tab of `note` shows `view` from now on; the
/// tabs are remembered with it (M-22.2).
#[tauri::command]
#[specta::specta]
pub async fn tab_set_view(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    view: EditingViewName,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.set_editing_view(&NoteId::from_relative(&note), view)
    })
    .await?
}

/// UC-004 A9: writes the copy a sync asked for, after the session was let
/// go, and puts the outcome in the status line.
fn keep_copy(session: &SharedSession, synced: Synced) -> SyncAnswer {
    if let Some(copy) = &synced.copy {
        let written = copy.write();
        session.lock().copy_kept(copy, written);
    }
    synced.answer
}

/// ADR-025: the window's edits made on revision `batch.base` (none to
/// fetch the backend's changes); the answer carries the backend's changes
/// since, the new revision and its hash.
///
/// Synchronous: it runs on the main thread in the order the
/// window sent it. The window has at most one batch of a note on its way
/// (`docs/migration/variante-b.md` section 3.3), so order would hold as an
/// async command too; synchronous keeps it simple, and the work under the
/// session is small (the edits, the transform over the backend's changes
/// since, the hash). A copy to keep (UC-004 A9, rare) is written after the
/// session is let go.
#[tauri::command]
#[specta::specta]
pub fn ed_sync(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    batch: SyncBatch,
) -> Result<SyncAnswer, UiError> {
    let synced = session
        .lock()
        .editor_sync(&NoteId::from_relative(&note), &batch);
    waker.wake();
    Ok(keep_copy(&session, synced?))
}

/// ADR-025: the window's copy of the text went apart from the backend's;
/// the window's `text` is taken.
#[tauri::command]
#[specta::specta]
pub fn ed_resync(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    text: String,
) -> Result<SyncAnswer, UiError> {
    let synced = session
        .lock()
        .editor_resync(&NoteId::from_relative(&note), &text);
    waker.wake();
    Ok(keep_copy(&session, synced?))
}

/// M-11.2, A-01: the registers `"+` and `"*` of the window's Vim keys read
/// the system clipboard (ADR-014) as plain text; `null` without text.
#[tauri::command]
#[specta::specta]
pub async fn clip_read_text(
    clipboard: State<'_, SharedClipboard>,
) -> Result<Option<String>, UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || clipboard.lock().read_text()).await
}

/// M-11.2, A-01: a yank into `"+` or `"*` puts the text on the system
/// clipboard.
#[tauri::command]
#[specta::specta]
pub async fn clip_write_text(
    clipboard: State<'_, SharedClipboard>,
    text: String,
) -> Result<(), UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || clipboard.lock().write_text(&text)).await
}

/// M-08.1, M-08.2: the source view shows the lines `from..to` (from 0, the
/// end not included); their colours come as the event `ed-highlight`.
#[tauri::command]
#[specta::specta]
pub fn ed_viewport(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    from: u32,
    to: u32,
) -> Result<(), UiError> {
    let set = session
        .lock()
        .editor_viewport(&NoteId::from_relative(&note), from, to);
    waker.wake();
    set
}

/// M-21.4, UC-005 A14 step 3: "Close anyway": Underline ends without the
/// changes that could not be stored. What can be stored is stored once
/// more, and the app state is written, when the app exits.
#[tauri::command]
#[specta::specta]
pub fn app_close_anyway(closer: State<'_, Closer>) -> Result<(), UiError> {
    closer.close_anyway();
    Ok(())
}

/// Ends the app (M-21.4); `main.rs` makes it from the app handle, so the
/// command stays free of the runtime type and testable.
pub struct Closer {
    exit: Box<dyn Fn() + Send + Sync>,
    anyway: AtomicBool,
    /// Everything was stored; the end the app asks for next goes through.
    confirmed: AtomicBool,
    /// ADR-025 section 3.9: the close handshake waiting for the window, by
    /// its number (0: none).
    handshake: AtomicU64,
    next: AtomicU64,
    /// What closing does after the window sent its edits
    /// ([`close_after_flush`] with the app's handle).
    after_flush: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Closer {
    /// A closer that ends the app with `exit`.
    pub fn new(exit: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            exit: Box::new(exit),
            anyway: AtomicBool::new(false),
            confirmed: AtomicBool::new(false),
            handshake: AtomicU64::new(0),
            next: AtomicU64::new(0),
            after_flush: OnceLock::new(),
        }
    }

    /// Sets what closing does after the window sent its edits.
    pub fn set_after_flush(&self, after: impl Fn() + Send + Sync + 'static) {
        let _ = self.after_flush.set(Box::new(after));
    }

    /// The window answered the close handshake: with `sent`, closing goes
    /// on; without, the window asks the user ("Changes not stored") and the
    /// handshake ends. An answer is not tied to the handshake's number: one
    /// that comes after the fallback went on (or without a handshake)
    /// changes nothing, so a close never runs twice.
    pub fn flushed(&self, sent: bool) {
        if self.end_handshake(None) && sent {
            if let Some(after) = self.after_flush.get() {
                after();
            }
        }
    }

    /// "Close anyway": ends the app without asking again.
    pub fn close_anyway(&self) {
        self.anyway.store(true, Ordering::SeqCst);
        (self.exit)();
    }

    /// Everything was stored: ends the app without asking again.
    pub fn close_now(&self) {
        self.confirmed.store(true, Ordering::SeqCst);
        (self.exit)();
    }

    /// Whether the user chose "Close anyway".
    pub fn closing_anyway(&self) -> bool {
        self.anyway.load(Ordering::SeqCst)
    }

    /// Whether the app ends without asking (again).
    pub fn closing(&self) -> bool {
        self.closing_anyway() || self.confirmed.load(Ordering::SeqCst)
    }

    /// ADR-025: a close waits for the window to send its edits; the number
    /// of this wait.
    pub fn begin_handshake(&self) -> u64 {
        let number = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.handshake.store(number, Ordering::SeqCst);
        number
    }

    /// Ends the wait `number` (or any with `None`); whether it was still
    /// waiting, so only one of the window and the fallback goes on.
    pub fn end_handshake(&self, number: Option<u64>) -> bool {
        match number {
            Some(number) => self
                .handshake
                .compare_exchange(number, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            None => self.handshake.swap(0, Ordering::SeqCst) != 0,
        }
    }
}

/// M-21.4, UC-005 BR-001, ADR-025 section 3.9: after the window sent its
/// edits (or did not answer in time), every change is stored; when that
/// fails, the window asks "Changes not stored" (A14 step 3), else the app
/// ends.
pub fn close_after_flush<R: Runtime>(app: &AppHandle<R>) {
    let question = app.state::<SharedSession>().lock().request_close();
    app.state::<Waker>().wake();
    match question {
        None => app.state::<Closer>().close_now(),
        Some(question) => {
            if let Err(error) = CloseQuestionEvent(question).emit(app) {
                eprintln!("The question before closing could not be sent: {error}");
            }
        }
    }
}

/// ADR-025 section 3.9: the window's answer to `ed-flush` with `closing`:
/// with `sent`, it sent the edits of every note, and Underline closes now
/// after storing everything (or asks why it cannot); without, it could not
/// send them and asks the user itself ("Changes not stored"). Synchronous:
/// it runs on the main thread, which also ends the app.
#[tauri::command]
#[specta::specta]
pub fn app_request_close(closer: State<'_, Closer>, sent: bool) -> Result<(), UiError> {
    closer.flushed(sent);
    Ok(())
}

/// ADR-025: the backend's text of the tab of `note` as the window's editor
/// core starts from it, with its revision and the tab's cursor.
#[tauri::command]
#[specta::specta]
pub fn ed_mirror(session: State<'_, SharedSession>, note: String) -> Result<MirrorView, UiError> {
    session.lock().editor_mirror(&NoteId::from_relative(&note))
}

/// The system clipboard, shared by the commands (ADR-014).
pub type SharedClipboard = Arc<Mutex<SystemClipboard>>;

/// M-03.5: checks the wiki project URL of the Workspace menu; the status
/// bar says the repository or why the URL does not fit. Nothing is stored.
#[tauri::command]
#[specta::specta]
pub fn wiki_check(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    url: String,
) -> Result<(), UiError> {
    session.lock().check_wiki(&url);
    waker.wake();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::Receiver;
    use std::sync::Arc;
    use std::time::Instant;

    use parking_lot::Mutex;
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};
    use underline_core::{StateStore, SETTINGS_FILE_NAME};
    use underline_session::{Session, SettingsState};

    use super::{Closer, SharedClipboard, SharedSession};
    use crate::bindings;
    use crate::clipboard::SystemClipboard;
    use crate::ticker::Waker;

    /// Every command, by its Rust name; keep in step with `bindings::builder`
    /// and `build.rs`.
    const COMMANDS: &[&str] = &[
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
        "clip_read_text",
        "clip_write_text",
        "app_close_anyway",
        "ed_sync",
        "ed_resync",
        "ed_mirror",
        "app_request_close",
    ];

    /// A mock app whose session was started with `given` and keeps its user
    /// settings in `dir`, with its window and the receiving end of the wakes.
    struct Mock {
        app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
        wakes: Receiver<()>,
    }

    impl Mock {
        fn new(given: Option<PathBuf>, dir: &Path) -> Self {
            let session = Session::new(
                given,
                StateStore::in_memory(),
                None,
                SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
            );
            let (waker, wakes) = Waker::new();
            let app = mock_builder()
                .manage(Arc::new(Mutex::new(session)))
                .manage(waker)
                .manage(Closer::new(|| {}))
                .manage(SharedClipboard::new(Mutex::new(
                    SystemClipboard::in_memory(),
                )))
                .invoke_handler(bindings::builder().invoke_handler())
                .build(mock_context(noop_assets()))
                .expect("the mock app builds");
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .expect("the mock window opens");
            Self {
                app,
                webview,
                wakes,
            }
        }

        fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().expect("a valid URL"),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            };
            get_ipc_response(&self.webview, request)
                .map(|body| body.deserialize::<Value>().expect("a JSON response"))
        }

        fn status(&self) -> String {
            self.app.state::<SharedSession>().lock().status().to_owned()
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-tauri-commands-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn m01_1_app_start_passes_the_given_dir_along() {
        let dir = temp_dir("start-given");
        let mock = Mock::new(Some(PathBuf::from("notes")), &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["givenDir"], json!("notes"));
        assert!(["macos", "windows", "linux"].contains(&response["platform"].as_str().unwrap()));
    }

    #[test]
    fn m01_1_app_start_without_a_dir() {
        let dir = temp_dir("start-none");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["givenDir"], Value::Null);
        assert_eq!(response["status"], json!(""));
    }

    #[test]
    fn m01_6_app_start_gives_the_settings_with_the_palette() {
        let dir = temp_dir("start-palette");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["settings"]["theme"], json!("Light"));
        assert_eq!(response["settings"]["showMenuBar"], json!(false));
        assert_eq!(
            response["settings"]["palette"]["background"],
            json!("#ffffff")
        );
        assert_eq!(response["themes"].as_array().unwrap().len(), 11);
    }

    #[test]
    fn uc010_m03_6_settings_quick_stores_the_theme_and_wakes_the_tick() {
        let dir = temp_dir("quick");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("settings_quick", json!({ "key": "theme", "value": "Nord" }));

        assert_eq!(response, Ok(Value::Null));
        assert_eq!(
            std::fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "theme: Nord\n"
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
        let start = mock.invoke("app_start", json!({})).unwrap();
        assert_eq!(start["settings"]["palette"]["theme"], json!("Nord"));
    }

    #[test]
    fn uc010_settings_quick_refuses_an_unknown_key() {
        let dir = temp_dir("quick-unknown");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("settings_quick", json!({ "key": "colour", "value": "red" }));

        assert!(response.is_err());
        assert!(!dir.join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn m03_5_wiki_check_sets_the_status() {
        let dir = temp_dir("wiki");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("wiki_check", json!({ "url": "https://gitlab.com/a/b" }));

        assert_eq!(response, Ok(Value::Null));
        assert_eq!(
            mock.status(),
            "Wiki repository: https://gitlab.com/a/b.wiki.git"
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
    }

    /// A workspace folder `dir/name` with the Markdown notes `notes`.
    fn workspace_with(dir: &Path, name: &str, notes: &[&str]) -> PathBuf {
        let root = dir.join(name);
        std::fs::create_dir_all(&root).unwrap();
        for note in notes {
            std::fs::write(root.join(format!("{note}.md")), format!("# {note}\n")).unwrap();
        }
        root
    }

    #[test]
    fn uc002_a1_m01_1_ws_start_opens_the_given_workspace() {
        let dir = temp_dir("ws-start");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        let mock = Mock::new(Some(root), &dir);

        let response = mock.invoke("ws_start", json!({})).unwrap();

        assert_eq!(response["answer"], json!({ "kind": "done" }));
        assert_eq!(response["snapshot"]["workspace"]["name"], json!("notes"));
        assert_eq!(
            response["snapshot"]["overview"],
            json!([
                { "note": "Home", "label": "Home" },
                { "note": "Ideas", "label": "Ideas" }
            ])
        );
        assert_eq!(response["snapshot"]["tabs"]["active"], json!("Home"));
        assert_eq!(
            response["snapshot"]["tabs"]["todoList"],
            json!({ "open": false, "active": false })
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
    }

    #[test]
    fn uc002_a3_m21_1_ws_open_offers_to_create_a_workspace_there() {
        let dir = temp_dir("ws-open-missing");
        let mock = Mock::new(None, &dir);
        let missing = dir.join("gone").display().to_string();

        let response = mock
            .invoke("ws_open", json!({ "path": missing, "anyway": false }))
            .unwrap();

        assert_eq!(response["kind"], json!("problem"));
        assert_eq!(
            response["offer"],
            json!({ "kind": "create", "directory": missing })
        );
        assert_eq!(response["recent"], json!([]));
    }

    #[test]
    fn uc001_m21_2_ws_check_new_and_ws_create_make_a_workspace() {
        let dir = temp_dir("ws-create");
        let mock = Mock::new(None, &dir);
        let path = dir.join("fresh").display().to_string();

        let checked = mock
            .invoke("ws_check_new", json!({ "path": path }))
            .unwrap();
        assert_eq!(checked, json!({ "stage": "options", "directory": path }));

        let created = mock
            .invoke(
                "ws_create",
                json!({ "request": {
                    "directory": path, "format": "Markdown", "git": false, "remote": ""
                } }),
            )
            .unwrap();

        assert_eq!(created, json!({ "stage": "done" }));
        assert!(dir.join("fresh").join("Home.md").is_file());
        assert!(mock.status().starts_with("Workspace created in "));
    }

    #[test]
    fn m05_2_m06_tab_commands_open_activate_and_close() {
        let dir = temp_dir("tabs");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("tab_open", json!({ "note": "Ideas" })).unwrap();
        mock.invoke("tab_activate", json!({ "note": "Home" }))
            .unwrap();
        let closed = mock
            .invoke("tab_close", json!({ "note": "Ideas", "force": false }))
            .unwrap();
        assert_eq!(closed, Value::Null);

        let snapshot = mock.app.state::<SharedSession>().lock().snapshot();
        let tabs: Vec<_> = snapshot.tabs.tabs.iter().map(|t| t.note.as_str()).collect();
        assert_eq!(tabs, ["Home"]);
        assert_eq!(snapshot.tabs.active.as_deref(), Some("Home"));
    }

    /// ADR-025, UC-005, M-10.9: the window's core starts from the mirror
    /// (`ed_mirror`), its edits arrive with `ed_sync`, and the note is stored
    /// and closed; a closed tab has no mirror.
    #[test]
    fn adr025_uc005_m10_9_editor_commands_sync_store_and_close() {
        let dir = temp_dir("editor");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        let mirror = mock.invoke("ed_mirror", json!({ "note": "Home" })).unwrap();
        assert_eq!(mirror["text"], json!("# Home\n"));
        let answer = mock
            .invoke(
                "ed_sync",
                json!({ "note": "Home", "batch": {
                    "base": mirror["rev"], "seq": 1,
                    "edits": [{ "at": 0, "remove": 0, "insert": "🙂" }]
                } }),
            )
            .unwrap();
        assert_eq!(answer["resync"], json!(false));
        assert_eq!(answer["remote"], json!([]));
        mock.invoke("ed_viewport", json!({ "note": "Home", "from": 0, "to": 5 }))
            .unwrap();

        mock.invoke("tab_store", json!({ "note": "Home" })).unwrap();

        assert_eq!(mock.status(), "Stored Home");
        assert_eq!(
            std::fs::read_to_string(root.join("Home.md")).unwrap(),
            "🙂# Home\n"
        );
        let closed = mock
            .invoke("tab_close", json!({ "note": "Home", "force": false }))
            .unwrap();
        assert_eq!(closed, Value::Null);
        assert!(mock.invoke("ed_mirror", json!({ "note": "Home" })).is_err());
    }

    /// M-11.2, M-08.6, ADR-014: the window's Vim registers and its clipboard
    /// keys read and write the system clipboard through the backend.
    #[test]
    fn m11_2_m08_6_the_clipboard_commands_read_and_write_it() {
        let dir = temp_dir("clipboard");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("clip_write_text", json!({ "text": "yanked" }))
            .unwrap();

        assert_eq!(
            mock.invoke("clip_read_text", json!({})).unwrap(),
            json!("yanked")
        );
        assert_eq!(
            mock.app
                .state::<SharedClipboard>()
                .lock()
                .read_text()
                .as_deref(),
            Some("yanked")
        );
    }

    /// M-07.2: `tab_set_view` switches the view of a tab and remembers it
    /// (the snapshot); a note without an open tab is an error.
    #[test]
    fn m07_2_tab_set_view_switches_the_view() {
        let dir = temp_dir("facade");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("tab_set_view", json!({ "note": "Home", "view": "source" }))
            .unwrap();

        let snapshot = mock.app.state::<SharedSession>().lock().snapshot();
        assert_eq!(
            snapshot.tabs.tabs[0].view,
            underline_session::EditingViewName::Source
        );
        let unknown = mock
            .invoke("tab_set_view", json!({ "note": "Gone", "view": "facade" }))
            .unwrap_err();
        assert!(unknown.to_string().contains("not open"), "{unknown}");
    }

    #[test]
    fn the_window_may_call_every_command() {
        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();

        for command in COMMANDS {
            let permission = format!("allow-{}", command.replace('_', "-"));
            assert!(
                permissions.contains(&json!(permission)),
                "capabilities/main.json lacks {permission}"
            );
        }
    }

    #[test]
    fn every_command_has_a_permission_from_the_build_script() {
        let build = include_str!("../build.rs");

        for command in COMMANDS {
            assert!(
                build.contains(&format!("\"{command}\"")),
                "build.rs lacks {command}"
            );
        }
    }

    #[test]
    fn every_command_is_in_the_bindings() {
        let bindings = std::fs::read_to_string(bindings::BINDINGS_PATH).unwrap();

        for command in COMMANDS {
            assert!(
                bindings.contains(&format!("__TAURI_INVOKE(\"{command}\"")),
                "the bindings lack {command}"
            );
        }
    }

    /// ADR-025 section 3.9: of the window's answer and the fallback, only
    /// one goes on with a close; a later close waits anew.
    #[test]
    fn adr025_one_end_of_the_close_handshake_goes_on() {
        let closer = Closer::new(|| {});
        let first = closer.begin_handshake();

        assert!(closer.end_handshake(None), "the window answered");
        assert!(
            !closer.end_handshake(Some(first)),
            "the fallback stays still"
        );

        let second = closer.begin_handshake();
        assert!(
            !closer.end_handshake(Some(first)),
            "an old wait ends nothing"
        );
        assert!(closer.end_handshake(Some(second)), "the fallback goes on");
        assert!(!closer.end_handshake(None));
        assert!(!closer.closing());
        closer.close_now();
        assert!(closer.closing());
    }

    /// ADR-025 section 3.9: a window's answer after the fallback went on
    /// does not close a second time; without its edits sent, it closes not.
    #[test]
    fn adr025_a_late_or_failed_answer_does_not_close() {
        use std::sync::atomic::Ordering;

        let closes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let closer = Closer::new(|| {});
        let counted = std::sync::Arc::clone(&closes);
        closer.set_after_flush(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });

        let number = closer.begin_handshake();
        assert!(closer.end_handshake(Some(number)), "the fallback goes on");
        closer.flushed(true);
        assert_eq!(
            closes.load(Ordering::SeqCst),
            0,
            "the late answer changes nothing"
        );

        closer.begin_handshake();
        closer.flushed(false);
        assert_eq!(
            closes.load(Ordering::SeqCst),
            0,
            "edits not sent: the window asks"
        );

        closer.begin_handshake();
        closer.flushed(true);
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }
}
