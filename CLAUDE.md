# Project context

Underline is a note-taking desktop app written in Rust (see `README.md` for the crate layout).

This project follows the [AI Unified Process](https://unifiedprocess.ai). The `aiup-core` plugin is enabled in
`.claude/settings.json`. Before making product, domain, or architecture decisions, read:

- `docs/vision.md`
- `docs/requirements.md`
- `docs/entity_model.md`
- the relevant documents under `docs/use_cases/` and `docs/test_cases/`
- the accepted architecture decision records under `docs/architecture/decisions/`

Files that do not exist yet have not been produced; create them with the matching skill rather than by hand
(`/requirements`, `/entity-model`, `/use-case-diagram`, `/use-case-spec`, `/test-case`, `/architecture-decision`).
`/next` names the next step.

The AIUP documents in `docs/` (vision, requirements, glossary, entity model, use cases, test cases) describe the
product from the user's point of view only. Keep technical detail out of them: no programming language, libraries,
code style, protocols, file formats of internals, or API responses. Technical decisions go into architecture decision
records (`docs/architecture/decisions/`, via `/architecture-decision`) and into this file.

## Workflow rules

1. Derive requirements from the product vision.
2. Reconcile the entity model and use case diagram when requirements change.
3. Do not implement a use case before its `UC-*.md` specification exists and has been reviewed.
4. Keep requirement, use case, and test case identifiers stable and traceable in code and tests: name the use case
   (`UC-001`) in the doc comment of the code that implements it and in the name or doc comment of its tests.
5. There is no AIUP stack plugin for Rust: implement use cases by hand, following the conventions below and the
   conventions already established in the codebase.
6. Follow the accepted architecture decision records; record a new architecture decision with
   `/architecture-decision`, linked to the non-functional requirement or constraint it serves.
7. A gap in a specification goes into `docs/open_questions.md` (`/open-questions`), not into an assumption in the code.

## Technical decisions

The technical counterpart of the requirements is in `docs/architecture/decisions/`:

| ADR     | Decision                                              | Status   |
|---------|-------------------------------------------------------|----------|
| ADR-001 | Rust with egui/eframe, crate layout, CI checks        | Superseded by ADR-020 |
| ADR-002 | MCP server over stdio (`underline mcp <dir>`)         | Accepted |
| ADR-003 | Facade engine crate, comrak and own AsciiDoc parser   | Accepted |
| ADR-004 | Layered YAML settings in `underline.yaml`             | Accepted |
| ADR-005 | Atomic writes of every file, debounced autosave       | Accepted |
| ADR-006 | Git through the installed git program                 | Accepted |
| ADR-007 | App state (workspaces, tabs, recent notes) in a state file | Superseded |
| ADR-008 | Workspace state in `.underline/`, ignored by git by itself | Accepted |
| ADR-009 | Offline diagrams: Mermaid with `merman`, PlantUML's JavaScript build in QuickJS, rasterized with resvg | Accepted |
| ADR-010 | Code and markup highlighting with syntect, palettes per theme | Accepted |
| ADR-011 | Watch the workspace with notify, poll as fallback | Accepted |
| ADR-012 | Own editor widget on egui (rope, block facade, key maps), prototype first | Superseded by ADR-021 |
| ADR-013 | Web images are the editor's only network access, a plain request through ureq | Accepted |
| ADR-014 | Clipboard through arboard; HTML converted to and from markup | Accepted |
| ADR-015 | Exports from the facade's blocks; PDF set by Typst as a library | Accepted |
| ADR-016 | The system's file dialogs through rfd | Accepted |
| ADR-017 | MCP install edits each agent tool's user configuration file | Accepted |
| ADR-018 | Agent changes go through the running window by request files | Accepted |
| ADR-019 | Todos as one YAML file each in `.todos/`, never merged line by line | Accepted |
| ADR-020 | Tauri 2 with a Svelte/TypeScript frontend (replaces ADR-001) | Accepted |
| ADR-021 | Editor in the webview: CodeMirror 6, own facade surface, Rust editor core (replaces ADR-012) | Accepted |
| ADR-022 | UI-free session crate, backend tick, events | Accepted |
| ADR-023 | Webview loads nothing from the network; own `ul-asset://` protocol | Accepted |
| ADR-024 | The backend owns the note text; one IPC command per edit; facade virtualized | Superseded by ADR-025 |
| ADR-025 | The webview owns the note text: editor core as WASM, backend mirror with rebase | Accepted |

Follow the accepted ones; a proposed one becomes binding once the user accepts it (`/architecture-decision ADR-003 accept`).

## Code conventions

- Domain logic lives in the library crates (`crates/underline-core`, `crates/underline-markup`,
  `crates/underline-facade`, `crates/underline-editor-core`, `crates/underline-editor-view`,
  `crates/underline-editor-wasm`, `crates/underline-session`, `crates/underline-wiki`, `crates/underline-mcp`);
  `apps/underline-tauri` only wires them to the window and the command line.
- Unit tests sit next to the code in `#[cfg(test)] mod tests`.
- Errors are typed with `thiserror` in the library crates.

## Migration to Tauri 2 (branch `migration/tauri`)

The window moves from egui to Tauri 2 (ADR-020 to ADR-023). The plan is in `docs/migration/plan.md`. The parity
checklist is `docs/migration/feature-inventar.md` (IDs `M-xx.y`), and deviations go into
`docs/migration/paritaet.md`.

- **Stack:**
  - Tauri 2 backend in `apps/underline-tauri/src-tauri`, binary `underline-tauri` (renamed to `underline` at
    cutover; it keeps the `mcp` subcommand).
  - Svelte 5 + TypeScript + Vite frontend in `apps/underline-tauri/ui`.
  - Orchestration without UI in `crates/underline-session`.
  - Editor logic without egui in `crates/underline-editor-core` (also builds for `wasm32-unknown-unknown`).
  - Source view on CodeMirror 6 with `@replit/codemirror-vim`. The facade is an own `contenteditable` driven by
    the Rust editor core.
- **Legacy:** the user confirmed parity; the egui app (`apps/underline-desktop`) and the egui editor widget
  (`crates/underline-editor`) are removed. Their code stays in the git history before the cutover.
- **Tools:** Node comes from nvm, so run `source ~/.nvm/nvm.sh` first. On Linux, Tauri needs `pkg-config`,
  `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `libgtk-3-dev`, `librsvg2-dev`, `libxdo-dev` and `libssl-dev`.
- **Start:** `cd apps/underline-tauri && cargo tauri dev`, or `npm run tauri dev` in `apps/underline-tauri/ui`.
  Pass a workspace with `cargo tauri dev -- -- ~/notes`.
- **Build:** `cd apps/underline-tauri && cargo tauri build`. Bundles are AppImage and deb on Linux, NSIS on Windows,
  dmg on macOS.
- **Frontend checks** in `apps/underline-tauri/ui`: `npm run lint`, `npm run format:check`, `npm run check`
  (svelte-check), `npm test` (Vitest).
- **Frontend dev server for app-tester:** `npm run dev` in `apps/underline-tauri/ui` serves
  `http://localhost:1420`.
  - Outside Tauri, the frontend installs a typed IPC mock (`mockIPC`), so Playwright can test flows without the
    backend.
  - Test the real app with `cargo tauri dev` and read the Rust log in that terminal.
- **IPC types:** commands, events and DTOs are generated into `apps/underline-tauri/ui/src/lib/bindings.ts` by
  tauri-specta on every debug start. Never edit the file by hand. A Rust test fails when it is outdated.
- **Conventions:**
  - Tauri commands are a thin layer: parse, lock the session, call `underline-session`, map errors to `UiError`.
  - No domain logic in commands or in Svelte components.
  - Doc comments and test names cite the use case and the inventory ID (`UC-005`, `M-10.8`).
  - UI texts are taken over verbatim from the egui app.
  - Plugins and capabilities stay minimal (plan section 8). A new plugin needs a reason in an ADR.
- **Workflow per feature (plan section 9):**
  1. Commands and events.
  2. UI.
  3. Tests.
  4. rust-reviewer: fix the Muss-Findings.
  5. ux-reviewer: fix the Muss-Findings.
  6. app-tester against the inventory.
  7. Tick the inventory item.
  8. Commit, one commit per feature.

## Verification

Run these after every change and report any that could not be completed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Tauri frontend, in apps/underline-tauri/ui:
npm run lint && npm run format:check && npm run check && npm test
```
