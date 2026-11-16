# ADR-027: Count the words and characters of a note with the `unicode-segmentation` crate, in the WASM editor core

**Status:** Proposed
**Date:** 2026-11-12
**Requirements:** NFR-002, NFR-013, C-008, FR-090

## Context

The note properties show the number of words and characters and the reading time of the active note and of the
selection (FR-090, UC-004 BR-020). The counts follow the text as the user types. Typing must not wait for them
(NFR-002), and they must be the same on every platform (NFR-013).

- A character is what a reader sees as one character (OQ-261). An accented letter written with a combining mark, a
  flag or an emoji joined with ZWJ is therefore one character, not several code points.
- Words are runs of letters and digits; a run of CJK characters counts as one word (OQ-261).

The text of an open note lives in the webview, in the WASM editor core (ADR-025). The counts are computed there, from
the blocks the facade parser already holds, so a key needs no IPC call. Neither the Rust standard library nor the
crates already in the WASM core split a text into grapheme clusters. The library must fit the MIT license (C-008).

This ADR overlaps ADR-025 (the WASM core grows by the crate) and ADR-026 (the same kind of decision for `regex`).

## Decision

- **Crate:** `unicode-segmentation` (MIT or Apache-2.0), one version for the workspace (1.13, already in `Cargo.lock`
  through other crates).
  - It is used only in `underline-markup::statistics`.
  - Grapheme clusters are taken in their extended form (`graphemes(true)`) for the character count.
- **Words:** the word rule of OQ-261 is applied in `underline-markup::statistics` on the shown text. The crate's own
  word boundaries are not used for it.
- **One implementation:**
  - The source view, the facade and the selection all call `underline-markup::statistics` through the editor core.
  - The tab caches the result per text revision and selection.

## Consequences

- **Agreement:** The counts agree on every platform and in both views. They are tested once, natively, in
  `underline-markup`.
- **No IPC and no event:** A key costs no IPC round trip, and no backend event is needed.
- **Size:** The WASM file grows by the crate's grapheme tables. It is 2.32 MB after package 5, including packages 2
  to 4.
- **Unicode versions:** A new Unicode version needs a crate update to count new emoji sequences as one character.
- **Alternatives considered:**
  - **`Intl.Segmenter` in the webview:** the counting would leave the Rust core and differ with the engine's ICU data
    (WebKitGTK, WebView2, WKWebView), against NFR-013. The selection mapping lives in the core.
  - **Counting code points:** an accented letter written with a combining mark, or an emoji sequence, would count as
    several characters, against OQ-261.
  - **Computing in the backend on the mirror:** every change would need an event round trip to show the count, and a
    selection is only known in the webview.
