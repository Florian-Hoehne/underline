# Underline

A modern note-taking desktop app for Linux, Windows and macOS, written in Rust.
Notes are Markdown or AsciiDoc files in a workspace directory, kept in the same
layout as a GitHub or GitLab wiki, and an MCP server lets you work on them
together with an AI agent.

## Layout

| Path | What it holds |
| --- | --- |
| `apps/underline-tauri/src-tauri` | The desktop app on Tauri 2 (binary `underline`): a thin layer of commands and events over `underline-session`, and the `mcp` subcommand |
| `apps/underline-tauri/ui` | The window's frontend: Svelte 5, TypeScript and Vite; the source view on CodeMirror 6 |
| `crates/underline-session` | The workspace session without a user interface: tabs, saving, background services, and the tick that sends the window its events |
| `crates/underline-editor-core` | Editor logic without a user interface: the text as a rope with undo, Vim and Nano keys, toolbar functions, facade editing; builds for WebAssembly too |
| `crates/underline-editor-view` | The editor's tab as the window holds it: what the window and the editor core exchange in the source view and the facade |
| `crates/underline-editor-wasm` | The editor's tab compiled to WebAssembly for the webview |
| `crates/underline-core` | Workspaces (directories of `.md` and `.adoc` notes in any folder, minus hidden and git-ignored ones), settings in `underline.yaml`, the state between starts (per workspace in a hidden `.underline/` folder that git ignores by itself), atomic writes, git through the installed `git` |
| `crates/underline-markup` | Note formats and the facade parser: blocks, sections, and formatting of Markdown and AsciiDoc; builds for WebAssembly too |
| `crates/underline-facade` | Facade engine: one format module per note format (Markdown, AsciiDoc) |
| `crates/underline-wiki` | GitHub and GitLab wiki remotes (project URL to `.wiki.git` repository) |
| `crates/underline-mcp` | Model Context Protocol server: look up notes (`list_notes`, `search_notes`, `read_note`), write them (`create_note`, `replace_in_note`, `insert_into_note`, `rewrite_note`), and manage todos (`list_todos`, `create_todo`, `update_todo`, `delete_todo`) |

## Build and run

Install Rust with [rustup](https://rustup.rs), the Tauri CLI (`cargo install tauri-cli --version "^2"`), and
Node.js with npm. The editor's core runs in the webview as WebAssembly, so add the target with
`rustup target add wasm32-unknown-unknown` and the `wasm-bindgen` CLI in the version `Cargo.lock` pins
(`cargo install wasm-bindgen-cli --version <pinned> --locked --root .tools`; `npm run wasm` in
`apps/underline-tauri/ui` names the version). On Linux, Tauri needs `pkg-config`, `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`,
`libgtk-3-dev`, `librsvg2-dev`, `libxdo-dev` and `libssl-dev`.

```sh
cd apps/underline-tauri/ui && npm ci && cd ..
cargo tauri dev                 # open the last opened workspace, or choose one; "Open workspace…" (UC-002) and "New workspace…" (UC-001)
cargo tauri dev -- -- ~/notes   # open another workspace
cargo tauri build               # bundles: AppImage and deb on Linux, NSIS on Windows, dmg on macOS
underline mcp ~/notes           # the built binary serves the workspace to an AI agent over MCP stdio
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
cargo check --target wasm32-unknown-unknown -p underline-markup -p underline-editor-core -p underline-editor-view -p underline-editor-wasm
# frontend, in apps/underline-tauri/ui:
npm run lint && npm run format:check && npm run check && npm test && npm run build
```

CI runs the same checks on Linux, Windows and macOS.
