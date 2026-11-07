# ADR-015: PDF and HTML export from the facade's blocks, PDF set by Typst as a library

**Status:** Accepted
**Date:** 2026-11-07
**Requirements:** FR-014, FR-015, FR-016, NFR-013, NFR-014, NFR-015, UC-009 BR-002 to BR-005, BR-009

## Context

UC-009 exports a note to the other note format, to a standalone HTML file, and to a PDF file, with its diagrams and
images, offline and without anything leaving the machine. The PDF needs pages of the system's paper size, page
numbers, text that can be selected and searched, every script the note holds, no heading alone at a page's foot, and
images and diagrams not split across pages. Underline has no layout engine for pages: egui lays out text for the
screen only.

## Decision

- **One model, three writers.** Every export starts from the facade's blocks of the note (ADR-003): Markdown and
  AsciiDoc already parse into them, with diagrams and images as blocks. The facade crate gains three writers: markup
  of the other note format (with a list of approximated blocks for UC-009 A2), HTML, and Typst markup.
- **HTML.** One file: diagrams as inline SVG (ADR-009), images as `data:` URLs, code highlighted with syntect into
  inline styles (ADR-010), and a small style sheet in the file with a `prefers-color-scheme: dark` variant.
- **PDF.** The note's Typst markup is set by Typst 0.15 used as a library (`typst`, `typst-layout`, `typst-pdf`,
  `typst-kit`; Apache-2.0), through an own `World` that serves the one document, its images, and its diagrams as SVG.
  Fonts: the fonts Typst ships (`typst-kit` embedded fonts) plus the fonts installed on the system, so other scripts
  find a font. Page size A4 or US Letter by the system's region; headings kept with what follows, figures in
  unbreakable blocks scaled to fit a page. No package downloads: the `World` serves no packages.
- **Writing the file.** Through the atomic write of ADR-005.

## Consequences

- A spike set a note with umlauts, Japanese, an SVG, a table, and highlighted code into a one-page A4 PDF in about
  0.25 s, offline, with selectable text and the document title.
- Typst is a large dependency: a clean release build of it takes about four minutes, and the application grows by
  several MB. It is used only for the PDF export.
- The three writers share one model, so a block kind added to the facade is exported everywhere once its writers know
  it.
- **Alternatives considered:** an own page layout with `printpdf` (page breaks, tables, and font embedding all by hand,
  plainer result); printing the HTML export with an installed browser (depends on a browser being installed and on its
  print settings); `genpdf` (unmaintained, no SVG).
