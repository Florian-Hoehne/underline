# ADR-012: An own editor widget on egui for the source view and the facade

**Status:** Superseded
**Superseded by:** ADR-021
**Date:** 2026-11-09
**Requirements:** FR-008, FR-009, FR-010, FR-018, FR-023, FR-024, FR-025, FR-027, FR-030, FR-031, FR-033, FR-034,
FR-044, NFR-002, NFR-005, NFR-007, NFR-011, NFR-013

## Context

Underline is built with egui (ADR-001), which has no rich-text editor and whose `TextEdit` lays out the whole text
every frame. UC-004 and UC-005 need tabs, a source view and a facade with the same functions, typing within
16 milliseconds in notes of 10,000 lines (NFR-002), folding, a minimap, the normal, Vim, and Nano input modes, undo
per tab, copy on select with markup and formatted text, dropped files and links, and autosave within 2 seconds. In the
facade the user edits formatted content without seeing markup (FR-009, UC-005), and only the edited block may change
in the file (NFR-007, ADR-003).

## Decision

- **A crate of its own:** `crates/underline-editor` holds the editor widget and its model; the desktop app places it in
  a tab. It depends on egui and on `underline-facade` (blocks, highlighting, diagrams), not the other way round.
- **Text model:** the markup of a note is held in a rope (`ropey` 1.x, MIT or Apache-2.0). Every change, from either
  view, is a command on that rope (insert, delete, replace a block's span); the facade never keeps a second copy of
  the text. Undo and redo record these commands per tab (UC-005 BR-004).
- **Source view:** only the visible lines and a small margin are laid out, each line into an egui galley that is cached
  by its text and its highlighting (ADR-010); scrolling and the minimap work on line heights, not on laid-out text.
  Folding hides line ranges of sections.
- **Facade:** the document is the list of blocks of ADR-003. Each visible block is laid out as formatted content
  (headings, emphasis, links, lists, tables, code, images, diagrams). The cursor and selection live in the formatted
  content; an edit is applied to that block's model and the format module writes the block back as markup, which
  replaces exactly the block's source span in the rope (NFR-007). Blocks the facade cannot format are edited as their
  markup in a frame (UC-005 BR-008). Only blocks in view are laid out.
- **One cursor for both views:** a position in the facade maps to a position in the markup and back through the
  source spans of the blocks and their inline elements, so switching the view keeps the cursor (UC-005 A1).
- **Input modes:** the editor exposes commands (move, select, delete, yank, put, search, undo, format, close tab,
  store). Three key maps translate keys into commands: the platform's usual keys, a Vim state machine (normal, insert,
  visual), and the Nano keys (UC-005 BR-009, BR-010, BR-015). The key maps are the same in both views; the active
  mode and its commands are reported to the status bar.
- **Platform services:** IME composition and accessibility go through egui (AccessKit); the clipboard uses `arboard`
  (MIT or Apache-2.0), which can put HTML and plain text on the clipboard at once (UC-005 BR-011) and read HTML when
  pasting (BR-014). Dropped files come from eframe's drop events.
- **Tabs and autosave:** the desktop app keeps one editor model per tab in a simple tab bar of its own. A change marks
  the tab as changed; a timer stores it 1 second after the last change and at most 2 seconds after the first unstored
  one (ADR-005), on a worker thread, and at once when the tab, the workspace, or the app is closed.
- **First step: a prototype.** Before the facade is built, a prototype of the source view must show, measured in a
  benchmark on a typical laptop, that a typed character appears within 16 milliseconds in a note of 10,000 lines with
  highlighting, and that scrolling through it keeps 60 frames per second. If it does not, this ADR is revisited before
  more is built on it.

## Prototype result (2026-11-06)

The source view prototype (`crates/underline-editor`, benchmark `adr012_*`, run with
`cargo test --release -p underline-editor -- --ignored --nocapture`) measured the CPU time of a frame (input,
highlighting, layout, painting, tessellation) on the development container:

| Measurement                                         | Median   | p99      | Max      | Budget   |
|-----------------------------------------------------|----------|----------|----------|----------|
| Typing at line 5,000 of a 10,000-line note          | 0.09 ms  | 0.28 ms  | 14.6 ms  | 16 ms    |
| Scrolling 50 lines per frame through the note       | 0.11 ms  | 0.32 ms  | 0.65 ms  | 16.7 ms  |
| Typing in the facade of the same note (2026-11-06) | 1.18 ms  | 1.85 ms  | 1.95 ms  | 16 ms    |

Both criteria are met, so the decision stands. In the facade, typed text joins its block without parsing the note
again, because escaped text cannot change the structure of the blocks; other edits parse the note again with comrak
(about 9 ms for 10,000 lines). Highlighting a large note for the first time took 0.5 to 1.4 seconds
in one frame; since coloring is spread over frames of at most 4 milliseconds (ADR-010), the first frame at line 5,000
takes 4 ms, its colors follow after about 60 frames, and typing stays below 0.3 ms per frame. GPU drawing comes on
top of the measured CPU time.

## Consequences

- Underline owns a substantial piece of code: layout, cursor movement, selection, IME, and accessibility of the
  editor. The prototype limits the risk before the facade depends on it.
- One rope as the single source of truth keeps both views, autosave, outside changes (ADR-011), and the MCP server
  consistent, and makes NFR-007 a property of block replacement.
- Rich-text editing of lists and tables in the facade is the hardest part; it is built block type by block type, and
  a block type that is not ready yet is edited as markup in a frame (UC-005 BR-008) in the meantime.
- **Risk:** eframe's drop events deliver files, not text, so dropping a link from a browser (UC-005 A12) may need
  platform code for text drops on Linux, Windows, and macOS; until it exists, links can be pasted.
- **Alternatives considered:** egui's `TextEdit` (lays out the whole text every frame and cannot edit formatted
  content); a live preview that shows the markup of the block under the cursor (simpler, but the user sees markup in
  the facade, against FR-009); another UI toolkit or a web view with a ready editor (reopens ADR-001).

**Measured again on 2026-11-07, with the minimap and folding** (release build, the same 10,000-line note): typing in
the source view median 0.58 ms, p99 1.2 ms; typing in the facade median 1.3 ms, p99 1.4 ms; scrolling 50 lines per
frame median 0.56 ms, p99 0.69 ms. The minimap's bars are built at most every 150 ms while the text changes, and the
sections at most every 300 ms while nothing is folded, so typing stays far below one frame.
