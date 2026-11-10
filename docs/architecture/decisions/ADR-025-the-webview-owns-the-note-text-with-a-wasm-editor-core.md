# ADR-025: The webview owns the text of an open note; the editor core runs there as WASM, the backend keeps a mirror

**Status:** Accepted
**Date:** 2026-11-10
**Requirements:** NFR-002, NFR-005, NFR-007, NFR-008, NFR-013
**Supersedes:** ADR-024

## Context

ADR-024 chose variant A: the backend owns the rope, and every input is one IPC command that returns a patch. It set
a measuring point for F5. If the facade puts fewer than 90 % of the keys into the next frame, the project switches to
variant B by a new ADR.

Feature F5b (facade editing) measured with the spike's method, on WebKitGTK, in a release build, for a note of
10,000 lines:

| Input | p99 | Keys in the next frame |
|---|---|---|
| Typing in the facade | 8–15 ms | 64–89 % |
| Enter and Backspace in the facade | 8–17 ms | – |

The time is spent as follows:

- The IPC round trip takes 2–5 ms.
- The backend takes less than 1.5 ms.
- Applying the patch in the window takes 4–5 ms.

The round trip alone pushes a key past the frame. The source view reached 85–91 % in F3 and F4, which is also at the
limit. In the spike, variant B reached 97–99 % in both views (`spikes/f0-editor/RESULTS.md`). The user decided on
variant B (2026-11-10).

The following still holds:

- Agent changes must appear within 1 second (NFR-008) and go through the window (ADR-018).
- Autosave writes within 2 seconds (NFR-005, ADR-005).
- A facade edit changes only its block (NFR-007).
- The editor behaves the same on every platform (NFR-013).

This ADR overlaps the following accepted ADRs:

- **ADR-021:** the editor core stays Rust and becomes the WASM core named there.
- **ADR-022:** the session crate keeps the tick and the events.
- **ADR-023:** the WASM file ships with the frontend and loads nothing from the network.
- **ADR-003:** the facade parser moves into its own crate. The facade crate re-exports it, so its interface stays.

## Decision

**The owner of the text.**

- The rope of the open tab lives in the webview: `underline_editor_core::Document` and the editor logic, compiled to
  `wasm32-unknown-unknown` with `wasm-bindgen`.
- A key in either view is handled in WASM without an IPC call:
  - Source view: the Vim and Nano keys and the toolbar.
  - Facade: the typing rules, Enter, Backspace, the formatting and the incremental reparse.
- The view is patched in the same task as the key.

**The mirror in the backend.**

- `underline-session` keeps a copy of the text of every open tab, as before. Autosave, search, sections, links,
  highlighting, figures, the minimap, renames and the MCP side (ADR-018) work on that copy.
- After the frame, the webview sends its edits in one call. Each edit is a replacement of a character range, tagged
  with the revision it was made on.
- The backend applies the edits and confirms the new revision.

**One revision line per tab with rebase (single-server operational transformation).**

- The backend counts the revisions.
- A change of the backend arrives in the webview as an event with its edits and its revision. Such changes are agent
  edits, outside changes, link rewrites after a rename, and a reload.
  - The webview moves its unconfirmed edits over it, applies it to its rope and confirms it.
- The backend moves late edits of the webview over its own changes made since their base revision.
- Undo, the cursor and the selection behave on such a change as in the egui app.

**What stays in the backend.**

- Syntax highlighting (syntect), diagrams, images and assets, link resolution and the file system stay in the
  backend.
- They arrive as events tagged with the revision they were computed for. The webview drops results of an older
  revision or maps them through the edits since.

**The new crates.**

- **`underline-markup`** builds for wasm32. It holds the facade parser and its types: `NoteFormat`, `blocks` (comrak
  without default features), `asciidoc`, `sections`, `formatting` and `link_at`.
  - `underline-facade` and `underline-core` re-export these.
  - The egui app and every other caller stay unchanged.
  - The tests move with the code.
- **The facade editing rules move into `underline-editor-core`.** These are the rules, the reparse, the edit and the
  input of F5b. The crate depends on `underline-markup` and keeps building for wasm32.
- **`underline-editor-wasm`** is the `wasm-bindgen` wrapper (`cdylib`).
  - Its version of `wasm-bindgen` is pinned exactly and matches the CLI.
  - The WASM file is built before the frontend and embedded with it.

**The measuring point.**

- The method of F0 and F5b is repeated for both views.
- The target is p99 ≤ 16 ms and at least 90 % of the keys in the next frame.
- The edits mirrored to the backend must match the webview's text: the test compares hashes.

## Consequences

**Easier.**

- A key costs no IPC round trip. Both views have headroom in the frame.
- The editor logic still lives in one Rust code base. It is tested natively with `cargo test` and runs in the
  webview unchanged.

**Harder.**

- There are two copies of the text and a synchronisation protocol. It needs tests:
  - edits running at the same time;
  - an agent edit while the user types;
  - closing and autosave with edits that are not confirmed yet. The webview sends them before the backend saves or
    closes.
- The build gains a step: the `wasm32-unknown-unknown` target and `wasm-bindgen-cli`.
  - The CI and `cargo tauri build` run it before the frontend.
  - The bundle grows by the WASM file, about 1 MB.
- The facade parser leaves `underline-facade`. This touches the crate graph of the egui app.
  - Its behaviour must not change. The moved tests and the egui app's own tests guard that.
- Results from the backend can be stale and must carry a revision.

**Unchanged.**

- Real OS key events, dead keys, IME, WebView2 and WKWebView are still unmeasured. They are tested in F16 and
  Phase 4.

**Alternatives considered.**

- **Keep variant A and make the patch cheaper.** The IPC round trip of 2–5 ms remains. It alone misses the frame,
  so the target cannot be reached.
- **Variant B for the facade only.** This would mean two owner models side by side. The source view was also at
  the limit (85–91 %).
