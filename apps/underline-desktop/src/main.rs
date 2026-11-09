//! Underline desktop app.
//!
//! `underline [WORKSPACE_DIR]` opens the workspace in a window; without a
//! directory it opens the last opened workspace, or offers to open or create
//! one (UC-002 A1, A2).
//! `underline mcp [WORKSPACE_DIR]` serves the workspace to an AI agent over
//! MCP on stdin and stdout.

mod app;
mod settings;

use std::io;
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
        // UC-017 BR-007: the server follows the stored agent access, whether
        // the window runs or not.
        let server = underline_mcp::Server::new(workspace);
        server.serve(io::stdin().lock(), io::stdout().lock())?;
        return Ok(());
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

/// `~/Underline` (or `%USERPROFILE%\Underline` on Windows).
fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Underline")
}
