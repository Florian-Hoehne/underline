//! The commands the window calls: a thin layer over `underline_session`.
//!
//! Every command returns `Result<_, UiError>`, so the window gets the same
//! texts the egui app shows. A new command is registered in three places:
//! `bindings::builder` (handler and TypeScript bindings), `build.rs` (its
//! `allow-…` permission) and `capabilities/main.json` (granted to the window).
//! Commands that do file, git or search I/O must be `async` (or use
//! `tauri::async_runtime::spawn_blocking`) and hold the session lock only
//! briefly (ADR-022).

use parking_lot::Mutex;
use tauri::State;
use underline_session::{Session, StartInfo, UiError};

/// M-01: what the window needs to know when it starts: the workspace folder
/// from the command line and the operating system.
#[tauri::command]
#[specta::specta]
pub fn app_start(session: State<'_, Mutex<Session>>) -> Result<StartInfo, UiError> {
    Ok(session.lock().start_info())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use parking_lot::Mutex;
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::WebviewWindowBuilder;
    use underline_session::Session;

    use crate::bindings;

    /// Every command, by its Rust name; keep in step with `bindings::builder`
    /// and `build.rs`.
    const COMMANDS: &[&str] = &["app_start"];

    /// Invokes `cmd` in a mock app whose session was started with `given`.
    fn invoke(given: Option<PathBuf>, cmd: &str) -> Result<Value, Value> {
        let app = mock_builder()
            .manage(Mutex::new(Session::new(given)))
            .invoke_handler(bindings::builder().invoke_handler())
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("the mock window opens");
        let request = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().expect("a valid URL"),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&webview, request)
            .map(|body| body.deserialize::<Value>().expect("a JSON response"))
    }

    #[test]
    fn m01_1_app_start_passes_the_given_dir_along() {
        let response = invoke(Some(PathBuf::from("notes")), "app_start").unwrap();

        assert_eq!(response["givenDir"], json!("notes"));
        assert!(["macos", "windows", "linux"].contains(&response["platform"].as_str().unwrap()));
    }

    #[test]
    fn m01_1_app_start_without_a_dir() {
        let response = invoke(None, "app_start").unwrap();

        assert_eq!(response["givenDir"], Value::Null);
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
}
