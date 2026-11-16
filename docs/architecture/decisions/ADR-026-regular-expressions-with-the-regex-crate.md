# ADR-026: Match the regular expressions of every search with the `regex` crate, in the backend and in the WASM editor core

**Status:** Accepted
**Date:** 2026-11-16
**Requirements:** NFR-013, NFR-002, NFR-017, C-008, FR-075

## Context

The search switch regular expression (FR-075, UC-008 BR-011) applies to three searches that run in two places:

- the search of the workspace (UC-008) and the replace of the workspace (UC-022) run in the backend, on the search
  thread of `underline-core`, over up to 10,000 notes (NFR-017: places within 2 seconds for 95% of search texts,
  replacing in 1,000 notes within 10 seconds);
- the find field of a note marks and replaces in the text the webview owns, in the WASM editor core (ADR-025,
  UC-005 A20 to A22).

The same search text with the same switches must find the same places in all three, on every platform (NFR-013), and
typing must never wait for a search (NFR-002), whatever expression the user types. The notation is the usual one of
programming editors, without look-behind (OQ-193), with `$0` to `$9` in the replacement (UC-022 BR-003). The library
must fit the MIT license (C-008).

This ADR overlaps ADR-025 (the WASM core grows by the engine) and ADR-010 (syntect brings `fancy-regex` for its
grammars; that engine stays syntect's own).

## Decision

- **Limits:** an expression longer than 4,096 bytes, or one that compiles to more than 1 MiB, is not valid ("it is
  too large").
- **Engine:** the `regex` crate (MIT or Apache-2.0), one version for the workspace, used only in
  `underline-markup::search` (`Query`, `Replacement`). The backend and the WASM core both call that module, so there
  is one implementation of what matches and what a replacement writes.
- **Options:** multi-line mode (`^` and `$` at line ends, also before `\r\n`), case-insensitive unless match case is
  on. A place never spans a line end unless the expression names one (`\n`); a match of nothing is no place. Whole
  word is checked on the place found (letters and digits belong to a word), not by wrapping the expression in `\b`.
- **Replacement:** `Query::replacement` reads `$0` to `$9` and `$$`; a group the expression does not have is an
  error that the field shows (UC-022 A2).
- **Features:** the workspace dependency takes `std`, `unicode-case` and `unicode-perl` only (Unicode case folding and
  Unicode `\w`, `\d`, `\s`), so the WASM core stays small; `underline-core` adds `perf` for the backend's search
  thread. `\p{…}` classes are not available.
- **Measured** (release, Linux, 10,000 notes of 1–50 KB, texts kept by the search thread): 20 of 20 search texts,
  regular expressions included, find their places within 2 seconds (slowest 0.81 s, `(echo|delta) golf` with 736,452
  places); replacing `Acme` in 1,000 notes takes 0.19 s. The WASM file grows from 1.44 MB to 2.02 MB (2.71 MB with the
  crate's default features).

## Consequences

- The find field, the search of the workspace and the replace of the workspace agree on every platform; the rules are
  tested once, natively, in `underline-markup`.
- The engine runs in linear time: no expression a user types can hold up typing or the search thread by
  backtracking.
- Look-around and backreferences in the pattern are not available, as OQ-193 assumes; such an expression is reported
  as not valid with the engine's reason.
- The WASM file is about 0.6 MB larger, which ADR-025's bundle estimate did not include.
- The first search after a workspace opens reads every note; in 10,000 notes this took 2.3 s on the measuring
  machine, before the texts are kept.
- **Alternatives considered:**
  - `fancy-regex` (already in the build through syntect): offers look-around, but backtracks, so a typed expression
    can take exponential time (NFR-002).
  - The webview's JavaScript `RegExp` for the find field and `regex` in the backend: two engines with different
    notations and case folding would find different places in the find field and in the workspace (NFR-013).
  - PCRE2: a C library in the build of every platform and of the WASM core.
