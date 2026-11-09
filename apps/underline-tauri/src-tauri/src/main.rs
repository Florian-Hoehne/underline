//! Underline on Tauri 2 (migration from the egui app, M-01).
//!
//! `underline-tauri [WORKSPACE_DIR]` opens the window and hands the directory
//! to it (M-01.1, UC-002 A1/A2).
//! `underline-tauri mcp [WORKSPACE_DIR]` serves the workspace to an AI agent
//! over MCP on stdin and stdout, without a window (M-01.2). MCP install writes
//! the path of the running binary into the agent tools' configuration, so this
//! binary must understand `mcp` just like the egui app.

mod bindings;
mod commands;

use std::io;
use std::path::PathBuf;

use parking_lot::Mutex;
use underline_core::Workspace;
use underline_session::Session;

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
/// `app_start`.
fn run_window(given: Option<PathBuf>) -> tauri::Result<()> {
    let bindings = bindings::builder();
    #[cfg(debug_assertions)]
    if let Err(error) = bindings::export(&bindings, bindings::BINDINGS_PATH) {
        eprintln!("The TypeScript bindings could not be written: {error}");
    }

    tauri::Builder::default()
        .manage(Mutex::new(Session::new(given)))
        .invoke_handler(bindings.invoke_handler())
        .setup(move |app| {
            bindings.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
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
