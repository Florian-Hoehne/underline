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
use std::sync::Arc;

use tauri::State;
use underline_core::NoteId;
use underline_session::{
    CreateAnswer, NewWorkspaceRequest, OpenAnswer, Session, SettingName, StartInfo, StartOpened,
    UiError, UiErrorKind,
};

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
    on_session(&session, &waker, Session::start_workspace).await
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
    on_session(&session, &waker, move |session| {
        session.open_chosen(&PathBuf::from(path), anyway)
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
    on_session(&session, &waker, move |session| {
        session.create_workspace(request)
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

/// M-06.3, UC-004 A5: closes the tab of `note`.
#[tauri::command]
#[specta::specta]
pub async fn tab_close(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.close_tab(&NoteId::from_relative(&note));
    })
    .await
}

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

    use super::SharedSession;
    use crate::bindings;
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
        mock.invoke("tab_close", json!({ "note": "Ideas" }))
            .unwrap();

        let snapshot = mock.app.state::<SharedSession>().lock().snapshot();
        let tabs: Vec<_> = snapshot.tabs.tabs.iter().map(|t| t.note.as_str()).collect();
        assert_eq!(tabs, ["Home"]);
        assert_eq!(snapshot.tabs.active.as_deref(), Some("Home"));
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
}
