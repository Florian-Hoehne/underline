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
| ADR-001 | Rust with egui/eframe, crate layout, CI checks        | Accepted |
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
| ADR-012 | Own editor widget on egui (rope, block facade, key maps), prototype first | Accepted |
| ADR-013 | Web images are the editor's only network access, a plain request through ureq | Accepted |
| ADR-014 | Clipboard through arboard; HTML converted to and from markup | Accepted |
| ADR-015 | Exports from the facade's blocks; PDF set by Typst as a library | Accepted |
| ADR-016 | The system's file dialogs through rfd | Accepted |

Follow the accepted ones; a proposed one becomes binding once the user accepts it (`/architecture-decision ADR-003 accept`).

## Code conventions

- Domain logic lives in the library crates (`crates/underline-core`, `crates/underline-facade`,
  `crates/underline-editor`, `crates/underline-wiki`, `crates/underline-mcp`); `apps/underline-desktop` only wires them to the window and the command line.
- Unit tests sit next to the code in `#[cfg(test)] mod tests`.
- Errors are typed with `thiserror` in the library crates.

## Verification

Run these after every change and report any that could not be completed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
