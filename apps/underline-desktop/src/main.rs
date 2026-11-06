//! Underline desktop app.
//!
//! `underline [WORKSPACE_DIR]` opens the workspace in a window; without a
//! directory it opens the last opened workspace, or offers to open or create
//! one (UC-002 A1, A2).
//! `underline mcp [WORKSPACE_DIR]` serves the workspace to an AI agent over
//! MCP on stdin and stdout.

mod app;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

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

    // UC-002 A1/A2: the app opens the given directory, else the last opened
    // workspace, else offers to open or create one.
    let (state, notice) = match StateStore::default_path() {
        Some(path) => StateStore::load(path),
        None => (StateStore::in_memory(), None),
    };

    eframe::run_native(
        "Underline",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            underline_editor::fonts::install(&cc.egui_ctx);
            Ok(Box::new(app::UnderlineApp::new(
                cc.egui_ctx.clone(),
                given,
                state,
                notice,
            )))
        }),
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
