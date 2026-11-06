# ADR-010: Highlight code and markup with syntect

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** FR-035, FR-038, NFR-002, NFR-010, NFR-014, C-003, C-008

## Context

Code blocks are highlighted for the language they name (FR-035) in the colors of the active theme (FR-038), and a
note should look as it does on GitHub (C-003). Typing must stay within 16 milliseconds in notes of up to 10,000 lines
(NFR-002), highlighting must work offline (NFR-014), and every theme must keep its contrast (NFR-010). The library
must be compatible with the MIT license (C-008); this rules out `giallo` (EUPL-1.2).

## Decision

- **Library:** `syntect` (MIT) with its pure-Rust regex backend (`fancy-regex` feature), so the build needs no C
  library on any platform. It reads TextMate/Sublime Text grammars, the grammar family GitHub highlights with.
- **Grammars:** the syntect defaults, extended by the grammar collection of `bat` through the `two-face` crate (MIT or
  Apache-2.0), loaded from a precompiled, embedded dump so startup does not parse grammars. An AsciiDoc grammar is
  vendored with its license after a license check; Markdown comes with the defaults.
- **Language names:** a code block's language name is matched against the grammars' names and file extensions,
  case-insensitively, with the aliases GitHub accepts (for example `js`, `py`, `sh`, `yml`). An unknown or missing
  name shows the block as plain monospace text.
- **Where:** in the facade every code block is highlighted. In the source view the markup of Markdown and AsciiDoc is
  highlighted too (headings, emphasis, links, lists, code fences), and code inside a fence in its own language.
- **Theme colors:** each Underline theme defines one palette of highlight colors for the usual scopes (comment,
  keyword, string, number, type, function, variable, punctuation, markup heading, emphasis, link, code). The palette
  is turned into a syntect theme at start and when the theme changes. Themes that have an official palette (GitHub,
  Solarized, Nord, Dracula, Gruvbox, Night Owl) take its colors; every palette is checked against NFR-010 in a test.
- **Speed:** highlighting state is kept per line, so an edit recolors only from the changed line until the state is
  the same again; lines outside the visible area are colored on the worker thread and cached by content and theme.
  A long code block is shown uncolored for the moment it takes and never blocks typing.
- **Location:** highlighting lives in `crates/underline-facade` behind a `Highlighter` type; the desktop app only turns
  its colored spans into egui text.

## Consequences

- Code looks close to GitHub, because both use TextMate grammars; differences in a few grammars are accepted.
- The embedded grammar dump makes the binary larger by a few megabytes.
- syntect is not incremental like tree-sitter; the per-line state and the cache keep typing fast.
- **Alternatives considered:** `arborium` (tree-sitter, more precise and incremental, but C grammars in the build and
  colors further from GitHub); `syntastica` (MPL-2.0, tree-sitter); `giallo` (EUPL-1.2, not compatible with MIT).
