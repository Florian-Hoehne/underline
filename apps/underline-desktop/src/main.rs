//! Underline desktop app.
//!
//! `underline [NOTEBOOK_DIR]` opens the notebook in a window.
//! `underline mcp [NOTEBOOK_DIR]` serves the notebook to an AI agent over
//! MCP on stdin and stdout.

mod app;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use underline_core::Notebook;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable();
    let serve_mcp = args.peek().map(String::as_str) == Some("mcp");
    if serve_mcp {
        args.next();
    }
    let notebook = Notebook::open(args.next().map(PathBuf::from).unwrap_or_else(default_dir))?;

    if serve_mcp {
        return serve_stdio(underline_mcp::Server::new(notebook));
    }

    eframe::run_native(
        "Underline",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(app::UnderlineApp::new(notebook)))),
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
