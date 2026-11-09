# ADR-021: Edit notes in the webview with CodeMirror 6, an own facade surface, and the Rust editor core

**Status:** Proposed
**Date:** 2026-11-09
**Requirements:** NFR-002, NFR-007, NFR-011, NFR-012, NFR-013, C-002
**Supersedes:** ADR-012 (on acceptance)

## Context

With the window in a webview (ADR-020), the egui editor widget of ADR-012 cannot be reused as it is. The editor must
type within 16 milliseconds in notes of 10,000 lines (NFR-002). An edit in the facade may change only the edited
block (NFR-007). The code goes further than ADR-012's text: it patches the source bytes of the edited characters in
place and never serializes a block again. Every function must work by keyboard (NFR-011), a new note format must
reach the facade without changing the editor (NFR-012), and notes stay plain Markdown and AsciiDoc files (C-002).
About 3,200 lines of the editor (rope and undo, the Vim/Nano key maps, the editing rules of both views) do not
depend on egui.

## Decision

- **Editor core in Rust:** `crates/underline-editor-core` holds the rope document with its undo rules, the Nano key
  map, the Vim key map of the facade, and the editing rules of the source view and the facade, without egui. It
  builds natively and for `wasm32-unknown-unknown`. The egui widget uses it until it is removed.
- **Source view:** CodeMirror 6 as the view. Markup highlighting comes from the syntect spans of
  `underline-facade` (ADR-010) as decorations. Folding is fed from the facade's sections. Line numbers go left or
  right in a gutter of its own. Search hits and the agent cursor are decorations. The Vim input mode of the source
  view is `@replit/codemirror-vim`, wired to the store and close commands, the system clipboard registers and the
  status line (user decision). Nano and Normal go through the editor core.
- **Facade:** one `contenteditable` surface over HTML blocks rendered from the facade's block model, carrying the
  source offsets. Every `beforeinput` is cancelled and handed to the editor core's facade rules. Only the changed
  blocks are replaced. IME composition is let through and committed at `compositionend`. Diagrams are shown as SVG.
- **Owner of the text:** the spike F0 decides whether the rope lives in the backend (one IPC call per edit) or in
  the webview as WASM (edits are mirrored to the backend with revisions). It measures typing latency (median and
  p99, 10,000 lines) on WebKitGTK and WebView2, and IME and emoji input. The result is recorded as a new ADR. If the
  backend variant reaches p99 at or below 16 ms on WebKitGTK, it is chosen; otherwise the WASM variant.
- Offsets crossing the boundary are converted between Rust byte/char offsets and JavaScript UTF-16 code units in one
  place.

## Consequences

- Format fidelity and AsciiDoc support stay with the existing Rust code. CodeMirror brings virtual scrolling, IME and
  accessibility to the source view.
- **Vim deviates by user decision:** in the source view `codemirror-vim` offers more commands than the facade's Vim
  key map, so the two views no longer behave the same. This is recorded in `docs/migration/paritaet.md`.
- An own `contenteditable` surface is the largest risk: IME, autocorrect and selection across three webviews need
  early tests on every operating system.
- The minimap with its search sieve, the enlarged view and the status line are own frontend components.
- **Alternatives considered:**
  - ProseMirror, TipTap or Milkdown for the facade: they serialize the document again, which breaks NFR-007, and
    have no AsciiDoc.
  - Showing the markup of the block under the cursor: against FR-009, already rejected in ADR-012.
  - The editor core's own Vim map in the source view: rejected by the user in favour of `codemirror-vim`.
