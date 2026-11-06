# Underline

A modern note-taking desktop app for Linux, Windows and macOS, written in Rust.
Notes are Markdown or AsciiDoc files in a workspace directory, kept in the same
layout as a GitHub or GitLab wiki, and an MCP server lets you work on them
together with an AI agent.

## Layout

| Path | What it holds |
| --- | --- |
| `apps/underline-desktop` | The `underline` binary: the desktop window (egui/eframe) and the `mcp` subcommand |
| `crates/underline-core` | Workspaces (directories of `<Page>.md` and `<Page>.adoc` files), settings in `underline.yaml`, the app state between starts, atomic writes, git through the installed `git` |
| `crates/underline-facade` | Facade engine: one format module per note format (Markdown, AsciiDoc) |
| `crates/underline-wiki` | GitHub and GitLab wiki remotes (project URL to `.wiki.git` repository) |
| `crates/underline-mcp` | Model Context Protocol server with `list_notes`, `read_note` and `write_note` tools |

## Build and run

Install Rust with [rustup](https://rustup.rs). On Linux the window needs the
usual X11 or Wayland development libraries.

```sh
cargo run --bin underline                # open the last opened workspace (or ~/Underline); "New workspace…" creates one (UC-001)
cargo run --bin underline -- ~/notes     # open another workspace
cargo run --bin underline -- mcp ~/notes # serve the workspace to an AI agent over MCP stdio
```

To use it from an MCP client, register the command `underline mcp <workspace dir>`
as a stdio server.

## Development process

Underline follows the [AI Unified Process](https://unifiedprocess.ai): the product vision, requirements, entity
model, and use cases live as Markdown in `docs/` and are the input for implementation and tests.
The `aiup-core` Claude Code plugin is declared in `.claude/settings.json`; Claude Code offers to install it when you
open the repository. To install it by hand:

```text
/plugin marketplace add ai-unified-process/marketplace
/plugin install aiup-core@ai-unified-process-marketplace
```

Start with `docs/vision.md`, then run `/requirements`, `/entity-model`, `/use-case-diagram`, and
`/use-case-spec UC-001`. `/next` tells you which step comes next.

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs the same checks on Linux, Windows and macOS.
