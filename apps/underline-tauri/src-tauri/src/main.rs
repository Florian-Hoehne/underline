//! Underline on Tauri 2 (migration from the egui app, M-01).
//!
//! `underline-tauri [WORKSPACE_DIR]` opens the window and hands the directory
//! to it (M-01.1, UC-002 A1/A2).
//! `underline-tauri mcp [WORKSPACE_DIR]` serves the workspace to an AI agent
//! over MCP on stdin and stdout, without a window (M-01.2). MCP install writes
//! the path of the running binary into the agent tools' configuration, so this
//! binary must understand `mcp` just like the egui app.

mod bindings;
mod clipboard;
mod commands;
mod events;
mod ticker;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_specta::Event as _;
use underline_core::Workspace;
use underline_session::Session;

use crate::clipboard::SystemClipboard;
use crate::commands::{Closer, SharedClipboard, SharedSession};
use crate::events::CloseQuestionEvent;
use crate::ticker::Waker;

/// The window starts hidden and the frontend shows it after its first
/// render, in the colours of the theme (plan section 14). Should the
/// frontend fail to load, the window is shown after this time anyway.
const SHOW_WINDOW_AT_THE_LATEST: Duration = Duration::from_secs(3);

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
enum Cli {
    /// `underline-tauri [DIR]`: open the window.
    Window { given: Option<PathBuf> },
    /// `underline-tauri mcp [DIR]`: serve the workspace over MCP.
    Mcp { given: Option<PathBuf> },
}

impl Cli {
    /// Reads the arguments after the program name. Only the first positional
    /// argument counts, there is no `--help` (M-01.1).
    fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut args = args.into_iter().peekable();
        let serve_mcp = args.peek().map(String::as_str) == Some("mcp");
        if serve_mcp {
            args.next();
        }
        let given = args.next().map(PathBuf::from);
        if serve_mcp {
            Self::Mcp { given }
        } else {
            Self::Window { given }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse(std::env::args().skip(1)) {
        Cli::Mcp { given } => serve_mcp(given),
        Cli::Window { given } => Ok(run_window(given)?),
    }
}

/// M-01.2: serves the workspace over MCP on stdin and stdout.
fn serve_mcp(given: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Workspace::open(given.unwrap_or_else(default_dir))?;
    // UC-017 BR-007: the server follows the stored agent access, whether the
    // window runs or not.
    let server = underline_mcp::Server::new(workspace);
    server.serve(io::stdin().lock(), io::stdout().lock())?;
    Ok(())
}

/// M-01.1: opens the window; the window learns the given directory through
/// `app_start`. M-01.3: the app state is read before the window opens, as in
/// the egui app.
fn run_window(given: Option<PathBuf>) -> tauri::Result<()> {
    let bindings = bindings::builder();
    #[cfg(debug_assertions)]
    {
        if let Err(error) = bindings::export(&bindings, bindings::BINDINGS_PATH) {
            eprintln!("The TypeScript bindings could not be written: {error}");
        }
        if let Err(error) = bindings::export_palettes(bindings::PALETTES_PATH) {
            eprintln!("The palettes could not be written: {error}");
        }
    }

    // The waker stays in the managed state, so the tick thread runs until the
    // process ends; the exit handler below writes the app state last.
    let (waker, wakes) = Waker::new();
    let mut session = Session::load(given, Instant::now());
    // ADR-011, ADR-022: the workspace watcher wakes the tick when it has
    // changes.
    let wake = waker.clone();
    session.set_wake(move || wake.wake());
    // UC-010 BR-004: the colours are made on a thread of their own, so
    // they are ready when the first note shows.
    session.warm_colors();
    let app = tauri::Builder::default()
        .manage(SharedSession::new(Mutex::new(session)))
        // ADR-014: the source view copies and pastes through the system's
        // clipboard, kept open while the app runs.
        .manage(SharedClipboard::new(Mutex::new(SystemClipboard::new())))
        .manage(waker)
        .invoke_handler(bindings.invoke_handler())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if ask_before_closing(window.app_handle()) {
                    api.prevent_close();
                }
            }
        })
        .setup(move |app| {
            bindings.mount_events(app);
            let handle = app.handle().clone();
            app.manage(Closer::new(move || handle.exit(0)));
            ticker::spawn(app.handle().clone(), wakes)?;
            show_window_at_the_latest(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())?;
    app.run(|app, event| match event {
        // M-21.4: an end the window did not ask for, such as the last window
        // going, stores everything first like closing the window does.
        RunEvent::ExitRequested { api, .. } if !app.state::<Closer>().closing_anyway() => {
            if ask_before_closing(app) {
                api.prevent_exit();
            }
        }
        // M-21.4, M-22.1: what can be stored is stored, and the app state is
        // written, when the app ends.
        RunEvent::Exit => {
            if let Err(problems) = app.state::<SharedSession>().lock().end() {
                for problem in problems {
                    eprintln!("At the end: {problem}");
                }
            }
        }
        _ => {}
    });
    Ok(())
}

/// M-21.4, UC-005 BR-001: closing the window or ending the app stores every
/// change first; when that fails, it asks "Changes not stored" (A14 step 3)
/// and returns `true`: the window stays open. "Close anyway" ends the app
/// (`app_close_anyway`).
fn ask_before_closing(app: &tauri::AppHandle) -> bool {
    let question = app.state::<SharedSession>().lock().request_close();
    app.state::<Waker>().wake();
    let Some(question) = question else {
        return false;
    };
    if let Err(error) = CloseQuestionEvent(question).emit(app) {
        eprintln!("The question before closing could not be sent: {error}");
    }
    true
}

/// Shows the main window after [`SHOW_WINDOW_AT_THE_LATEST`] if the
/// frontend has not shown it.
fn show_window_at_the_latest(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(SHOW_WINDOW_AT_THE_LATEST);
        let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(window) = app.get_webview_window("main") {
                if !window.is_visible().unwrap_or(true) {
                    eprintln!("The window did not show itself; showing it now");
                    if let Err(error) = window.show() {
                        eprintln!("The window could not be shown: {error}");
                    }
                }
            }
        }));
        if shown.is_err() {
            eprintln!("Showing the window at the latest failed");
        }
    });
}

/// `~/Underline` (or `%USERPROFILE%\Underline` on Windows), as in the egui app.
fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Underline")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn m01_1_without_arguments_opens_the_window() {
        assert_eq!(parse(&[]), Cli::Window { given: None });
    }

    #[test]
    fn m01_1_the_first_argument_is_the_workspace() {
        assert_eq!(
            parse(&["notes", "ignored"]),
            Cli::Window {
                given: Some(PathBuf::from("notes"))
            }
        );
    }

    #[test]
    fn m01_2_mcp_serves_the_given_workspace() {
        assert_eq!(
            parse(&["mcp", "notes"]),
            Cli::Mcp {
                given: Some(PathBuf::from("notes"))
            }
        );
    }

    #[test]
    fn m01_2_mcp_without_a_directory() {
        assert_eq!(parse(&["mcp"]), Cli::Mcp { given: None });
    }

    #[test]
    fn m01_2_default_dir_is_underline_in_home() {
        assert!(default_dir().ends_with("Underline"));
    }
}
