# ADR-024: The backend owns the text of an open note; every edit is one IPC command

**Status:** Superseded
**Date:** 2026-11-10
**Requirements:** NFR-002, NFR-007, NFR-008, NFR-013
**Superseded by:** ADR-025

## Context

ADR-021 left open who owns the text of an open note: the Tauri backend, with one IPC command per edit (variant A),
or the editor core compiled to WASM in the webview, with edits mirrored to the backend (variant B). The spike F0
decided it with an agreed rule: choose A if its typing latency reaches p99 ≤ 16 ms on WebKitGTK in both views for a
note of 10,000 lines (NFR-002). Agent changes go through the window (ADR-018) and must appear within 1 second
(NFR-008). Facade edits may change only the edited block (NFR-007).

The spike measured on Linux only (user decision) with an in-page input driver, in a release build. The results are
in `spikes/f0-editor/RESULTS.md`:

| Variant | View | p99 per key | Key in the next frame |
|---|---|---|---|
| A | source view | 7 ms | 87–89 % |
| A | facade (160 blocks in the DOM) | 9–10 ms | 70–75 % |
| B | source view | 4–5 ms | 97–99 % |
| B | facade (160 blocks in the DOM) | 5–7 ms | 97–99 % |

With all 3,697 blocks of the facade in the DOM, both variants reach p99 46 ms. Enter and Backspace in the facade
parse the whole note again in both variants, as in the egui code, and reach p99 54–65 ms.

## Decision

- **Variant A.** The rope (`underline_editor_core::Document`) of every open tab lives in the backend
  (`underline-session`). Each input of either view is one IPC command. The command returns a patch: changed text
  range or changed blocks, the new cursor and the revision. The frontend applies the patch and keeps no text of its
  own.
- **The facade is virtualized.** Only a window of blocks around the visible area is in the DOM, with placeholder
  heights for the rest. `content-visibility: auto` is not used, because on WebKitGTK one DOM change then costs
  270–430 ms of layout.
- **Enter and Backspace in the facade parse incrementally.** They no longer parse the whole note again.
- **The keystroke command stays small.** Syntax highlighting, search hits, the minimap and sections are computed
  after the command returns and sent as events. They are never computed inside it.
- **Measuring point.** In F3 (source view) and F5 (facade) the typing latency is measured again with the spike's
  method. If the facade puts fewer than 90 % of the keys into the next frame, the project switches to variant B by a
  new ADR. `underline-editor-core` keeps building for `wasm32-unknown-unknown` so that this stays possible.

## Consequences

- There is one copy of the text. Agent edits (ADR-018), renames, outside changes and autosave work on the backend's
  rope as in the egui app, with no synchronisation protocol.
- Every key costs one IPC round trip, about 4–6 ms on WebKitGTK. There is little headroom for work inside the
  command.
- Real OS key events, dead keys, a real IME, WebView2 and WKWebView were not measured. They are tested in F3 and F5.
- **Alternatives considered:**
  - Variant B: better frame timing, but it needs the facade's parser (`blocks`, `asciidoc`, `sections`,
    `formatting`, `NoteFormat`, `Theme`) extracted into a WASM-buildable crate and a rebase protocol for agent
    edits. It stays the fallback.
