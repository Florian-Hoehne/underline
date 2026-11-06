# Underline

A modern note-taking desktop app for Linux, Windows and macOS, written in Rust.
Notes are Markdown pages kept in the same layout as a GitHub or GitLab wiki,
and an MCP server lets you work on them together with an AI agent.

## Layout

| Path | What it holds |
| --- | --- |
| `apps/underline-desktop` | The `underline` binary: the desktop window (egui/eframe) and the `mcp` subcommand |
| `crates/underline-core` | Notes and the notebook, a directory of `<Page>.md` files |
| `crates/underline-wiki` | GitHub and GitLab wiki remotes (project URL to `.wiki.git` repository) |
| `crates/underline-mcp` | Model Context Protocol server with `list_notes`, `read_note` and `write_note` tools |

## Build and run

Install Rust with [rustup](https://rustup.rs). On Linux the window needs the
usual X11 or Wayland development libraries.

```sh
cargo run --bin underline                # open ~/Underline in a window
cargo run --bin underline -- ~/notes     # open another notebook
cargo run --bin underline -- mcp ~/notes # serve the notebook to an AI agent over MCP stdio
```

To use it from an MCP client, register the command `underline mcp <notebook dir>`
as a stdio server.

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same checks on Linux, Windows and macOS.
