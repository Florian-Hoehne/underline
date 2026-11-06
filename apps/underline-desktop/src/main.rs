//! Underline desktop app.
//!
//! `underline [WORKSPACE_DIR]` opens the workspace in a window; without a
//! directory it opens the last opened workspace (ADR-008).
//! `underline mcp [WORKSPACE_DIR]` serves the workspace to an AI agent over
//! MCP on stdin and stdout.

mod app;

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use underline_core::{StateStore, Workspace};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable();
    let serve_mcp = args.peek().map(String::as_str) == Some("mcp");
    if serve_mcp {
        args.next();
    }
    let given = args.next().map(PathBuf::from);

    if serve_mcp {
        let workspace = Workspace::open(given.unwrap_or_else(default_dir))?;
        return serve_stdio(underline_mcp::Server::new(workspace));
    }

    // ADR-008: without a directory on the command line, the last opened
    // workspace opens again (UC-001 postcondition).
    let (state, notice) = match StateStore::default_path() {
        Some(path) => StateStore::load(path),
        None => (StateStore::in_memory(), None),
    };
    let dir = given
        .or_else(|| state.last_workspace().map(Path::to_owned))
        .unwrap_or_else(default_dir);
    let workspace = Workspace::open(dir)?;

    eframe::run_native(
        "Underline",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::UnderlineApp::new(workspace, state, notice)))),
    )?;
    Ok(())
}

fn serve_stdio(server: underline_mcp::Server) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        if let Some(reply) = server.handle_line(&line?) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// `~/Underline` (or `%USERPROFILE%\Underline` on Windows).
fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Underline")
}
