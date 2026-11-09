# Spike F0: results

**Question (ADR-021, plan section 6):** who owns the note text in the Tauri editor?
**Decision rule (agreed):** variant A (backend-authoritative) if its p99 typing latency is at most 16 ms on
WebKitGTK (Linux) in both views at 10,000 lines; otherwise variant B (WASM core in the webview).

**Result:** variant A meets the rule. Typing p99 is 7 ms in the source view and 9 to 10 ms in the facade.
The facade figure assumes the facade keeps only a window of blocks in the DOM, which is the plan's own
mitigation (section 11). Recommendation: **A** (details and conditions in section 7).

## 1. Setup

| | |
|---|---|
| Machine | Intel Core i5-8500 (6 cores, 3.0 GHz, max 4.1 GHz), 31 GB RAM, monitor 3440x1440 at 59.9 Hz |
| OS | Ubuntu 26.04.1 LTS, kernel 7.0.0-42, GNOME Shell 50.1 on Wayland (the app ran as a native Wayland client) |
| Webview | WebKitGTK `libwebkit2gtk-4.1-0` 2.52.6 (JavaScriptCore 2.52.6); UA `AppleWebKit/605.1.15 … Version/60.5 Safari/605.1.15` |
| Tauri | `tauri` 2.12 (lock from the root workspace), `tauri-cli` 2.12.4, `@tauri-apps/api` 2.12.4 |
| Frontend | Svelte 5.57.3, Vite 8.3.7, TypeScript 6.0, `@codemirror/view` 6.43.15, `@codemirror/state` 6.7.6, `@codemirror/lang-markdown` 6.5.2 (only in the highlighting runs) |
| Rust | rustc 1.99.1, release profile (`opt-level = 3`); `wasm-bindgen` 0.2.130; the WASM binary is 700 KB without `wasm-opt` |
| Build | `cargo build --release --features custom-protocol` (the frontend embedded, as `cargo tauri build` does it) |
| Results | `results/run-20261109-191404.json` (main run), `results/diag-all.json`, `results/diag-200.json` (DOM cost taken apart) |

Only Linux was measured, as the user decided. Variant A uses the real `underline_facade::blocks`. Variant B uses
`model/src/blocks_copy.rs`, a verbatim copy of it. A test (`the_copy_of_the_blocks_parses_like_the_facade`)
checks that the copy and the original give identical blocks for the test note.

## 2. Method

- **Test note:** `model/src/note.rs` generates it deterministically: exactly 10,000 lines and 449,746 UTF-16
  units, which gives 3,697 facade blocks. It mixes headings, soft-wrapped paragraphs with inline formatting and
  links, nested and ordered lists, task lists, quotes, fenced code (Rust, Python, TypeScript, Bash, JSON),
  tables, images, Mermaid and PlantUML blocks, and thematic breaks. There was no time for an AsciiDoc note.
- **Owner** (`model/src/owner.rs`, the same code in A and B):
  - The rope is `underline_editor_core::Document`, with its undo merging.
  - It ports the minimum of `facade_view.rs`: `type_text` (Markdown escaping and the `insert_typed` fast
    path), `enter` (M-09.11 markers), `delete_back` (the escape goes with its character, joining blocks),
    `escape_markdown` and `next_marker`.
  - Enter and Backspace in the facade parse the whole note again, as the egui facade does.
  - The patch sends only the blocks that differ (common prefix and suffix by kind and runs).
  - All offsets at the boundary are UTF-16 code units. The owner converts them in one place: ropey's
    `utf16_cu_to_char` for the source, and per block text for the facade.
- **Source view:** CodeMirror 6 as a view only, with no keymap, no history and no highlighting (except the
  marked runs).
  - A `beforeinput` handler cancels `insertText`, `insertParagraph` and `deleteContentBackward` and sends
    them to the owner.
  - A transaction filter catches anything else CodeMirror would change itself (paste, drop, composition),
    takes it back and sends it to the owner as a `replace`.
- **Facade:** one `contenteditable` root over the blocks' HTML (`model/src/render.rs`), with `white-space:
  pre-wrap`.
  - The text of each element is exactly the block's facade text; markers come from CSS `::before`, so a DOM
    offset is the facade index.
  - `beforeinput` is cancelled and sent as (block, UTF-16 offset in the block, input). The patch replaces only
    the changed elements and sets the cursor.
  - Three DOM modes:
    - `window`: about 160 blocks around the cursor, with spacer `div`s of estimated height above and below.
      This is the main mode.
    - `all`: all 3,697 blocks.
    - `cv`: all blocks with `content-visibility: auto`.
- **Mirror (B):** every edit is kept as `{base revision, char index, removed chars, inserted}`. After the next
  frame (`requestAnimationFrame` + `setTimeout 0`) the edits go to the backend in one `b_mirror` call. The
  backend applies them to its own `Document` and refuses a wrong base revision. Afterwards the backend hash is
  compared with the owner hash.
- **A-async:** variant A with `async` Tauri commands, which run on the thread pool instead of the main thread.
  It was measured at the middle position only.
- **Driver:** in-page.
  - It dispatches synthetic `beforeinput` events to the view, one after the other. Each waits for the previous
    one to be painted, plus a deterministic jitter of 0 to 17 ms, so that keys arrive at any phase of the
    frame.
  - Per scenario: 30 warm-up keys (not counted), **500 typed keys**, then 100 Backspaces, then 30 times Enter
    followed by Backspace.
  - The typed text is `the quick brown fox *jumps* over the lazy_dog, `. The `*` and `_` are escaped and
    stay on the fast path.
  - Positions: paragraphs at 2 %, 50 % and 98 % of the note (lines about 200, 5,000 and 9,800), typed from
    the end to the start, so earlier positions do not move.
- **Measures per input:**
  - **proc:** from the handler's start to the DOM being patched (`performance.now()`). This is the processing
    time (a).
  - **next paint:** from `event.timeStamp` to a task queued in the first `requestAnimationFrame` after the
    patch, so just after that frame is painted.
  - **double rAF:** from `event.timeStamp` to the second `requestAnimationFrame` (b, as the task asked).
    Because of vsync this is always 1 to 2 frames, so it is not below 16.7 ms even for zero work.
  - **in first frame:** whether the patch was in the DOM before the first animation frame after the event, and
    that frame came within 17.5 ms.
  - **owner:** the owner's own time. In A it is measured in Rust (`Instant`, exact). In B it is the WASM call
    plus `JSON.parse`, measured in JS.
- **Timer resolution:** WebKitGTK's `performance.now()` has a resolution of **1 ms** here.
  `crossOriginIsolated` stays `false` even with COOP/COEP headers on `tauri://`. So all webview figures are
  whole milliseconds (±1 ms); the Rust-side owner times are exact.
- **Stalls:** in 3 inputs of the whole run no frame came within 1 s, probably while the window was being
  covered. They are left out of the paint figures.

## 3. Results: typing (processing time, ms)

Main table: 500 keys per row, facade in `window` mode, no highlighting.

| Owner | View | Position | median | p95 | p99 | max | next paint p99 | in first frame |
|---|---|---|---|---|---|---|---|---|
| **A** | source | start | 5 | 6 | **7** | 10 | 32 | 89 % |
| **A** | source | middle | 5 | 6 | **7** | 9 | 25 | 89 % |
| **A** | source | end | 6 | 6 | **7** | 19 | 25 | 87 % |
| **A** | facade | start | 8 | 9 | **10** | 11 | 28 | 75 % |
| **A** | facade | middle | 8 | 9 | **9** | 11 | 27 | 70 % |
| **A** | facade | end | 7 | 8 | **9** | 10 | 28 | 72 % |
| **B** | source | start | 3 | 4 | **4** | 5 | 23 | 99 % |
| **B** | source | middle | 3 | 4 | **5** | 6 | 24 | 98 % |
| **B** | source | end | 3 | 4 | **5** | 6 | 25 | 98 % |
| **B** | facade | start | 5 | 6 | **7** | 7 | 24 | 97 % |
| **B** | facade | middle | 4 | 5 | **6** | 12 | 26 | 98 % |
| **B** | facade | end | 5 | 5 | **5** | 5 | 23 | 98 % |
| A-async | source | middle | 6 | 7 | 9 | 10 | 26 | 85 % |
| A-async | facade | middle | 9 | 10 | 10 | 16 | 28 | 69 % |

Extra rows at the middle position:

| Owner | View | Note | median | p95 | p99 | max |
|---|---|---|---|---|---|---|
| A | source | CM6 `lang-markdown` highlighting | 9 | 13 | 14 | 16 |
| B | source | CM6 `lang-markdown` highlighting | 7 | 9 | 10 | 11 |
| A | facade | all 3,697 blocks in the DOM | 23 | 34 | **46** | 57 |
| B | facade | all 3,697 blocks in the DOM | 22 | 34 | **46** | 52 |

How the time splits (p99, ms):

| | A source | A facade (window) | B source | B facade (window) |
|---|---|---|---|---|
| Owner (Rust in A, WASM + JSON in B) | 0.05 | 2.4 to 3.1 | ≤ 1 | ≤ 3 |
| DOM (CM dispatch or block swap + selection) | 4 to 5 | 4 to 5 | 4 to 5 | 3 to 4 |
| IPC round trip (the rest of proc in A) | about 1 to 2 | about 1 to 3 | – | – |

Owner alone, outside the webview (`model/examples/owner_bench.rs` natively;
`tools/wasm-owner-bench.mjs` runs the same WASM in Node/V8, not JavaScriptCore). Medians and p99 in ms:

| Input | native median / p99 | WASM (V8) median / p99 |
|---|---|---|
| Facade: type one char (`insert_typed` fast path) | 0.43 / 0.55 | 0.39–0.68 / 0.46–2.4 |
| Source: type one char | 0.001 / 0.003 | 0.008 / 0.03 |
| Facade: Backspace (parses the whole note again) | 23 / 42–49 | 22–25 / 23–29 |
| Parse the whole note (comrak, 3,697 blocks) | 23 | – |

For comparison, the egui baseline from ADR-012 (processing time per frame, 10,000 lines) is source view median
0.58 ms and p99 1.2 ms, facade median 1.3 ms and p99 1.4 ms.

## 4. Results: Backspace, Enter, loading, view switch

**Backspace and Enter** (100 Backspaces, and 30 times Enter + Backspace, processing time in ms):

| Owner | View | Backspace median / p99 | Enter+Backspace median / p99 |
|---|---|---|---|
| A | source | 3–4 / 6–7 | 4–5 / 8–9 |
| B | source | 1–2 / 4–5 | 2–3 / 6–7 |
| A | facade (window) | 28–29 / **62–65** | 30–31 / **56–65** |
| B | facade (window) | 23–24 / **54–60** | 23–24 / **58–62** |

Every Backspace and Enter in the facade parses the whole note again, in both variants, so the owner's time
dominates; the DOM part stays at 1 to 2 ms. This misses 16 ms whoever owns the text. The egui app runs the same
code path; the ADR-012 baseline measured only typing.

**Loading and view switch** (ms, until the second frame after the DOM is built; switch figures are medians of
5):

| | A | B | A-async |
|---|---|---|---|
| Open the source view (note to owner, CM6 built) | 49 | 29 | 47 |
| Open the facade, window of 160 blocks (28 KB HTML, 309 elements) | 120 | 101 | 52 |
| Open the facade, all blocks (734 K UTF-16 units of HTML, 7,609 elements) | 306 | 281 | 400 |
| Open the facade, all blocks, `content-visibility: auto` | 1,037 | 1,279 | 1,258 |
| Switch facade → source (CM6 `setState` with the whole text) | 34 | 39 | 34 |
| Switch source → facade, window (whole note parsed again) | 64 | 54 | 70 |
| Switch source → facade, all blocks | 266 | 416 | – |

The first opening of each variant is a cold measurement (first IPC, first layout), so its scatter is high. For
example A's window opening took 586 ms in a validation run and 120 ms in the main run.

**DOM cost taken apart** (`src/lib/diag.ts`; one block swapped, cursor set, layout forced, ms median / p99):

| DOM | swap | set selection | forced layout | until the second frame |
|---|---|---|---|---|
| 3,697 blocks, editable | 1 / 5 | 5 / 12 | 0 / 0 | 33 / 50 |
| 3,697 blocks, editable, `content-visibility: auto` | 1 / 2 | **271 / 282** | **422 / 438** | 717 / 750 |
| 3,697 blocks, not editable, `content-visibility: auto` | 0 / 1 | 0 | 271 / 278 | 567 / 584 |
| 200 blocks, editable | 0 / 1 | 2 / 3 | 0 / 0 | 33 / 34 |

## 5. Correctness

All checks are automated (`runChecks` in `src/lib/bench.ts`) and pass in A and B, except one:

| Check | A | B |
|---|---|---|
| After the whole run, the backend text equals the owner text (FNV-1a hash and string equality; 0 mirror errors, revisions match) | yes | yes |
| Source view: type `🙂`, `日本`, `x`; the CM doc, owner text and backend text are byte-exact as expected | yes | yes |
| Source view: the cursor lands at +5 UTF-16 units | yes | yes |
| Source view: 4 Backspaces remove `x`, `本`, `日`, then `🙂` as a whole (no half surrogate) | yes | yes |
| Facade: `🙂`, `日本`, `x`, `*` land byte-exact in the source as `🙂日本x\*` | yes | yes |
| Facade: block DOM text equals the owner's block text; cursor at +6 | yes | yes |
| Facade: 5 Backspaces remove `\*` (with its backslash), `x`, `本`, `日`, `🙂` | yes | yes |
| Facade: synthetic composition (`compositionstart` / `insertCompositionText` / `compositionend` with `日本`) commits `日本` | yes | yes |
| Facade: Enter then Backspace gives the original text back | **no** | **no** |

**The failing check** is not about the spike's plumbing. It comes from the facade logic ported from
`facade_view.rs`:

- The cursor stands before a space (`…image| block…`), and Enter inserts `\n\n` there.
- The new paragraph's leading space is no facade character (comrak strips it).
- Backspace at the start of that block deletes from the end of the previous block up to its first facade
  character, so the space goes too: `imageblock`.

The egui facade runs the same code, so it very likely loses the space too. This was not checked in the egui
app.

**Further finding in `underline-facade`:** `insert_typed` moves the source offsets of every block's `runs`,
but not those of the cells inside `BlockKind::Table`. After the first keystroke before a table, its cell offsets
are stale until the next full parse. The spike noticed it because its block diff compared tables with their
offsets. Whether the egui app reads those cell offsets between two parses was not checked.

German dead keys and compose (`ä` through `xdotool`) and a real IME were **not tested** (section 8).

## 6. Observations

- **The DOM size, not the owner, decides the facade.**
  - With all 3,697 blocks in one `contenteditable`, setting the selection alone costs about 5 ms (p99 12) in
    WebKitGTK. Typing p99 is then 46 ms in both A and B.
  - With a window of about 160 blocks, typing p99 is 5 to 10 ms.
  - The facade must keep only the blocks near the viewport in the DOM, as the plan's risk table already says.
    The spike's window is crude: it re-renders on scroll and has estimated spacer heights. A real one needs a
    height cache per block, keeping the cursor's block, and the minimap/sieve on top of it.
- **`content-visibility: auto` is unusable in WebKitGTK 2.52 for this.**
  - Any DOM change in a document of 3,697 `content-visibility` blocks costs 270 to 430 ms of layout.
  - Loading also gets 3 to 4 times slower.
- **Backspace and Enter in the facade miss 16 ms in both variants.** The cause is the full comrak parse:
  23 ms native, about the same in WASM, plus the clone and diff of the blocks. This needs incremental parsing,
  that is, re-parsing only the blocks around the edit, whoever owns the text. It is independent of the A/B
  decision.
- **IPC cost of A:** about 1 to 3 ms per keystroke on top of B; B's processing time is roughly half of A's.
  - In time to paint, A puts the patch into the first frame after the key in 70 to 89 % of the inputs, B in
    97 to 99 %.
  - At a p99 of 7 to 10 ms, A stays well within the 16 ms rule. In practice, A shows about every fourth facade
    keystroke one frame (16.7 ms) later than B would.
  - `async` commands (A-async) are not faster; they are about 1 ms slower than sync commands on the main
    thread.
- **CM6 integration notes:**
  - Routing input through `EditorView.domEventHandlers({ beforeinput })` works: returning `true` cancels it,
    and CM's own handlers are skipped.
  - CM defers handlers while it is updating (`updateState != Idle`), so a synthetic event during a dispatch is
    handled a microtask later.
  - Paste, drop and IME have to be caught in a transaction filter and taken back. During a composition, CM's
    own changes must be let through and inverted at `compositionend` (implemented, but tested only
    synthetically).
  - `lang-markdown` highlighting adds 3 to 7 ms p99 per keystroke in the webview. The plan's syntect spans
    from the backend would move that cost off the keystroke path, but were not measured.
  - Soft wrap and a line number gutter were on.
- **Facade DOM rules that worked:**
  - The text content of each block equals the facade text, and markers come from CSS `::before`. A `Range`
    from the block start to the selection then gives the UTF-16 offset directly.
  - The elements must **not** carry absolute source offsets: those of every later block move on each keystroke
    (O(n) DOM writes). The offsets stay per run in the owner's block model; the view addresses blocks by
    index.
- **Tauri pitfalls met:**
  - A plain `cargo build --release` without the `custom-protocol` feature loads the dev server URL: the window
    stays blank, with no error.
  - Without app-command permissions (`AppManifest::commands` plus a capability), every `invoke` is rejected,
    including the one that would report the error.
  - B needs `script-src 'wasm-unsafe-eval'` and `connect-src 'self'` (for the `.wasm` fetch) in the CSP.
    ADR-023's CSP would have to allow that.
- **Variant B and wasm32:**
  - `underline-editor-core` builds for wasm32 as it is.
  - `underline-facade` does not: it depends on `underline-core`, which pulls in `notify`, `directories` and
    git, and on typst, rquickjs, resvg, merman, ureq and image.
  - For real, B would need a new wasm-buildable crate (for example `underline-markup`) with these parts:
    - `blocks.rs`: comrak only, plus `NoteFormat`.
    - `asciidoc.rs`: pure, but calls `diagram::DiagramLanguage::from_code_language`. That enum
      (`diagram/mod.rs`) would move along without the renderers.
    - `sections.rs` and `formatting.rs` (they need `NoteFormat`).
    - Probably `highlight.rs` and `themes.rs`: syntect with `regex-fancy` is pure Rust but large, and they need
      `underline_core::Theme`.
    - `NoteFormat` and `Theme` would move out of `underline-core` into that crate or a tiny shared one.
    - `clip.rs` (`scraper`) and `rename.rs` (comrak) could follow later.
  - In both variants, the facade's editing rules (`type_text`, `enter`, `delete_back`, `escape_markdown`,
    `next_marker`, `asciidoc_plain`, the gap logic) still live in the egui crate (`facade_view.rs`). They have to
    move into `underline-editor-core` anyway, as ADR-021 says.
  - The WASM binary is 700 KB, and opening the source view in B also copies the 450 K text into WASM memory
    (17 ms).

## 7. Recommendation

The agreed rule says: **variant A**. Its typing p99 at 10,000 lines on WebKitGTK is 7 ms in the source view
and 9 to 10 ms in the facade, below 16 ms in both views at all three positions.

Conditions, which hold for either variant:

1. The facade keeps only a window of blocks in the DOM (virtualization). Without it, neither variant meets
   16 ms (p99 46 ms). Do not use `content-visibility: auto` on WebKitGTK.
2. Backspace and Enter in the facade need incremental parsing (p99 54 to 65 ms today, in both variants).
3. Keep A's keystroke path lean. Backend work per keystroke (syntect spans, search hits, minimap) must not be
   computed synchronously in the input command, or the margin shrinks. The headroom to 16 ms is
   about 6 ms in the facade.

The margin of A is real but not large. Real OS key events add WebKitGTK's input path, which the in-page driver
does not include. The measured machine is a mid-range desktop from 2018. If a later measurement with
`xdotool`, on slower hardware or on WebView2 lands near 16 ms, B remains the fallback. The owner code already
runs unchanged in WASM, at about native speed in V8.

These are product and architecture decisions for the user: accepting A on these numbers, the conditions above,
and whether the Enter/Backspace space loss and the stale table offsets are fixed in the egui app as well. The
result belongs in a new ADR (`/architecture-decision`), as ADR-021 asks.

## 8. Not tested

- **Real OS key events:** `xdotool` is not installed (`sudo apt install xdotool`). Also, the GNOME session
  starts Xwayland with `-enable-ei-portal`, so XTEST input to an X11 client may need approval through the remote
  desktop portal. All figures come from the in-page driver; WebKitGTK's own key path (UI process to web
  process) is not included.
- **German dead keys / compose** (`xdotool type 'ä'`): not tested, for the same reason.
- **IME:** iBus is installed, but only with xkb engines (no mozc/anthy), so Japanese composition was not
  tested. Only a synthetic composition in the facade was checked; CM6's composition path in the source view was
  not exercised.
- **Windows (WebView2) and macOS (WKWebView):** not measured (user decision: Linux only).
- **AsciiDoc note:** not generated or measured (the copy for B is Markdown only).
- **Selections** (typing over a selection, Delete, arrows, the empty-lines gap after Enter, undo/redo through
  the owner): not part of the spike.
- **Scrolling** in the windowed facade: not measured; the window is re-rendered on scroll without a height
  cache.
- **Backend syntect spans as CM6 decorations** (the plan's highlighting): not built; only CM6's own
  `lang-markdown` was measured as a stand-in for "some highlighting".
- **Agent edits / rebase for B's mirror:** the mirror only refuses a wrong base revision; no rebase was
  implemented.
- **Sub-millisecond resolution in the webview:** `performance.now()` is coarsened to 1 ms. COOP/COEP headers
  on `tauri://` did not make the page cross-origin isolated.

## Appendix: full tables of the main run

Generated with `python3 -I summarize.py results/run-20261109-191404.json`. "proc" is the processing time.
"next paint" and "double rAF" run from the event's time stamp. "owner p99" is Rust-timed in A and JS-timed
(1 ms resolution) in B.

### type

| owner | view | where | note | n | proc median | proc p95 | proc p99 | proc max | owner p99 | DOM p99 | next paint median | next paint p99 | double rAF median | double rAF p99 | in first frame | reparsed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A | source | end |  | 500 | 6.00 | 6.00 | 7.00 | 19.00 | 0.05 | 5.00 | 15.00 | 25.00 | 27.00 | 39.00 | 87 % | 0 |
| A | source | middle |  | 500 | 5.00 | 6.00 | 7.00 | 9.00 | 0.05 | 4.00 | 15.00 | 25.00 | 27.00 | 35.00 | 89 % | 0 |
| A | source | start |  | 500 | 5.00 | 6.00 | 7.00 | 10.00 | 0.05 | 4.00 | 14.00 | 32.00 | 27.00 | 48.00 | 89 % | 0 |
| A | source | middle | CM6 lang-markdown highlighting | 500 | 9.00 | 13.00 | 14.00 | 16.00 | 0.03 | 11.00 | 18.00 | 26.00 | 27.00 | 36.00 | 88 % | 0 |
| A | facade | end | window of 160 blocks | 500 | 7.00 | 8.00 | 9.00 | 10.00 | 2.44 | 4.00 | 18.00 | 28.00 | 30.00 | 38.00 | 72 % | 0 |
| A | facade | middle | window of 160 blocks | 500 | 8.00 | 9.00 | 9.00 | 11.00 | 2.57 | 4.00 | 18.00 | 27.00 | 30.00 | 38.00 | 70 % | 0 |
| A | facade | start | window of 160 blocks | 500 | 8.00 | 9.00 | 10.00 | 11.00 | 3.05 | 5.00 | 17.00 | 28.00 | 29.00 | 39.00 | 75 % | 0 |
| A | facade | middle | all blocks in the DOM | 500 | 23.00 | 34.00 | 46.00 | 57.00 | 2.52 | 41.00 | 40.00 | 64.00 | 41.00 | 66.00 | 9 % | 0 |
| B | source | end |  | 500 | 3.00 | 4.00 | 5.00 | 6.00 | 1.00 | 5.00 | 13.00 | 25.00 | 25.00 | 43.00 | 98 % | 0 |
| B | source | middle |  | 500 | 3.00 | 4.00 | 5.00 | 6.00 | 1.00 | 5.00 | 14.00 | 24.00 | 25.00 | 34.00 | 98 % | 0 |
| B | source | start |  | 500 | 3.00 | 4.00 | 4.00 | 5.00 | 1.00 | 4.00 | 13.00 | 23.00 | 25.00 | 37.00 | 99 % | 0 |
| B | source | middle | CM6 lang-markdown highlighting | 500 | 7.00 | 9.00 | 10.00 | 11.00 | 1.00 | 10.00 | 16.00 | 31.00 | 25.00 | 42.00 | 97 % | 0 |
| B | facade | end | window of 160 blocks | 500 | 5.00 | 5.00 | 5.00 | 5.00 | 3.00 | 3.00 | 14.00 | 23.00 | 26.00 | 34.00 | 98 % | 0 |
| B | facade | middle | window of 160 blocks | 500 | 4.00 | 5.00 | 6.00 | 12.00 | 3.00 | 4.00 | 13.00 | 26.00 | 25.00 | 38.00 | 98 % | 0 |
| B | facade | start | window of 160 blocks | 500 | 5.00 | 6.00 | 7.00 | 7.00 | 3.00 | 4.00 | 15.00 | 24.00 | 25.00 | 34.00 | 97 % | 0 |
| B | facade | middle | all blocks in the DOM | 500 | 22.00 | 34.00 | 46.00 | 52.00 | 3.00 | 43.00 | 39.00 | 66.00 | 39.00 | 66.00 | 13 % | 0 |
| A-async | source | middle |  | 500 | 6.00 | 7.00 | 9.00 | 10.00 | 0.04 | 5.00 | 16.00 | 26.00 | 28.00 | 36.00 | 85 % | 0 |
| A-async | facade | middle | window of 160 blocks | 500 | 9.00 | 10.00 | 10.00 | 16.00 | 2.73 | 5.00 | 19.00 | 28.00 | 30.00 | 38.00 | 69 % | 0 |

### backspace

| owner | view | where | note | n | proc median | proc p95 | proc p99 | proc max | owner p99 | DOM p99 | next paint median | next paint p99 | double rAF median | double rAF p99 | in first frame | reparsed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A | source | end |  | 100 | 4.00 | 6.00 | 6.00 | 6.00 | 0.05 | 4.00 | 12.00 | 23.00 | 26.00 | 35.00 | 90 % | 0 |
| A | source | middle |  | 100 | 3.00 | 6.00 | 6.00 | 6.00 | 0.03 | 4.00 | 14.00 | 24.00 | 27.00 | 35.00 | 84 % | 0 |
| A | source | start |  | 100 | 3.00 | 6.00 | 7.00 | 12.00 | 0.03 | 4.00 | 13.00 | 23.00 | 27.00 | 35.00 | 91 % | 0 |
| A | source | middle | CM6 lang-markdown highlighting | 100 | 8.00 | 13.00 | 14.00 | 20.00 | 0.05 | 12.00 | 17.00 | 27.00 | 27.00 | 35.00 | 87 % | 0 |
| A | facade | end | window of 160 blocks | 100 | 29.00 | 41.00 | 62.00 | 62.00 | 59.45 | 2.00 | 39.00 | 74.00 | 55.00 | 89.00 | 0 % | 100 |
| A | facade | middle | window of 160 blocks | 100 | 28.00 | 60.00 | 64.00 | 66.00 | 61.08 | 2.00 | 40.00 | 73.00 | 55.00 | 88.00 | 0 % | 100 |
| A | facade | start | window of 160 blocks | 100 | 29.00 | 60.00 | 65.00 | 66.00 | 61.75 | 2.00 | 41.00 | 78.00 | 57.00 | 93.00 | 0 % | 100 |
| A | facade | middle | all blocks in the DOM | 100 | 61.00 | 64.00 | 64.00 | 65.00 | 26.40 | 38.00 | 77.00 | 80.00 | 77.00 | 81.00 | 0 % | 100 |
| B | source | end |  | 100 | 2.00 | 4.00 | 5.00 | 6.00 | 1.00 | 5.00 | 13.00 | 23.00 | 24.00 | 34.00 | 99 % | 0 |
| B | source | middle |  | 100 | 2.00 | 4.00 | 5.00 | 5.00 | 1.00 | 4.00 | 11.00 | 21.00 | 24.00 | 33.00 | 99 % | 0 |
| B | source | start |  | 100 | 1.00 | 4.00 | 4.00 | 4.00 | 1.00 | 4.00 | 13.00 | 23.00 | 26.00 | 33.00 | 100 % | 0 |
| B | source | middle | CM6 lang-markdown highlighting | 100 | 7.00 | 11.00 | 11.00 | 12.00 | 1.00 | 11.00 | 17.00 | 24.00 | 26.00 | 34.00 | 96 % | 0 |
| B | facade | end | window of 160 blocks | 100 | 23.00 | 49.00 | 60.00 | 63.00 | 60.00 | 1.00 | 25.00 | 62.00 | 30.00 | 63.00 | 0 % | 100 |
| B | facade | middle | window of 160 blocks | 100 | 24.00 | 48.00 | 58.00 | 61.00 | 57.00 | 1.00 | 26.00 | 60.00 | 30.00 | 61.00 | 0 % | 100 |
| B | facade | start | window of 160 blocks | 100 | 24.00 | 45.00 | 54.00 | 61.00 | 53.00 | 2.00 | 26.00 | 56.00 | 27.00 | 57.00 | 0 % | 100 |
| B | facade | middle | all blocks in the DOM | 100 | 58.00 | 61.00 | 61.00 | 64.00 | 24.00 | 38.00 | 75.00 | 79.00 | 75.00 | 80.00 | 0 % | 100 |
| A-async | source | middle |  | 100 | 4.00 | 7.00 | 7.00 | 7.00 | 0.05 | 5.00 | 13.00 | 23.00 | 27.00 | 36.00 | 80 % | 0 |
| A-async | facade | middle | window of 160 blocks | 100 | 32.00 | 61.00 | 64.00 | 64.00 | 60.44 | 2.00 | 41.00 | 79.00 | 57.00 | 95.00 | 0 % | 100 |

### enter+backspace

| owner | view | where | note | n | proc median | proc p95 | proc p99 | proc max | owner p99 | DOM p99 | next paint median | next paint p99 | double rAF median | double rAF p99 | in first frame | reparsed |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A | source | end |  | 60 | 5.00 | 8.00 | 9.00 | 9.00 | 0.04 | 6.00 | 18.00 | 30.00 | 34.00 | 54.00 | 92 % | 0 |
| A | source | middle |  | 60 | 4.00 | 7.00 | 9.00 | 9.00 | 0.04 | 5.00 | 15.00 | 30.00 | 32.00 | 61.00 | 95 % | 0 |
| A | source | start |  | 60 | 5.00 | 7.00 | 8.00 | 8.00 | 0.04 | 5.00 | 20.00 | 31.00 | 32.00 | 53.00 | 87 % | 0 |
| A | source | middle | CM6 lang-markdown highlighting | 60 | 7.00 | 13.00 | 14.00 | 14.00 | 0.05 | 12.00 | 21.00 | 38.00 | 38.00 | 54.00 | 85 % | 0 |
| A | facade | end | window of 160 blocks | 60 | 30.00 | 36.00 | 56.00 | 56.00 | 53.57 | 2.00 | 38.00 | 67.00 | 51.00 | 82.00 | 0 % | 60 |
| A | facade | middle | window of 160 blocks | 60 | 30.00 | 60.00 | 64.00 | 64.00 | 60.34 | 2.00 | 42.00 | 76.00 | 56.00 | 90.00 | 0 % | 60 |
| A | facade | start | window of 160 blocks | 60 | 31.00 | 63.00 | 65.00 | 65.00 | 61.11 | 2.00 | 44.00 | 88.00 | 58.00 | 102.00 | 0 % | 60 |
| A | facade | middle | all blocks in the DOM | 60 | 37.00 | 43.00 | 87.00 | 87.00 | 30.55 | 61.00 | 56.00 | 106.00 | 58.00 | 108.00 | 0 % | 60 |
| B | source | end |  | 60 | 3.00 | 6.00 | 7.00 | 7.00 | 1.00 | 6.00 | 16.00 | 29.00 | 29.00 | 49.00 | 100 % | 0 |
| B | source | middle |  | 60 | 2.00 | 6.00 | 6.00 | 6.00 | 1.00 | 5.00 | 16.00 | 29.00 | 31.00 | 62.00 | 98 % | 0 |
| B | source | start |  | 60 | 3.00 | 5.00 | 6.00 | 6.00 | 1.00 | 5.00 | 18.00 | 29.00 | 30.00 | 51.00 | 100 % | 0 |
| B | source | middle | CM6 lang-markdown highlighting | 60 | 5.00 | 8.00 | 10.00 | 10.00 | 1.00 | 10.00 | 18.00 | 31.00 | 34.00 | 51.00 | 98 % | 0 |
| B | facade | end | window of 160 blocks | 60 | 23.00 | 40.00 | 58.00 | 58.00 | 57.00 | 1.00 | 26.00 | 61.00 | 32.00 | 63.00 | 0 % | 60 |
| B | facade | middle | window of 160 blocks | 60 | 24.00 | 49.00 | 58.00 | 58.00 | 57.00 | 2.00 | 26.00 | 61.00 | 29.00 | 63.00 | 0 % | 60 |
| B | facade | start | window of 160 blocks | 60 | 24.00 | 45.00 | 62.00 | 62.00 | 60.00 | 2.00 | 27.00 | 65.00 | 29.00 | 67.00 | 0 % | 60 |
| B | facade | middle | all blocks in the DOM | 60 | 82.00 | 84.00 | 90.00 | 90.00 | 24.00 | 67.00 | 100.00 | 109.00 | 101.00 | 111.00 | 0 % | 60 |
| A-async | source | middle |  | 60 | 6.00 | 9.00 | 12.00 | 12.00 | 0.03 | 8.00 | 18.00 | 33.00 | 36.00 | 64.00 | 90 % | 0 |
| A-async | facade | middle | window of 160 blocks | 60 | 30.00 | 59.00 | 66.00 | 66.00 | 61.42 | 3.00 | 39.00 | 77.00 | 53.00 | 91.00 | 0 % | 60 |

