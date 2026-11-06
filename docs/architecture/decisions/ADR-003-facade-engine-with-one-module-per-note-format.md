# ADR-003: Facade engine with one format module per note format

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** NFR-012, NFR-007, C-002, C-003, C-004

## Context

Further note formats must become available without changing the editing views, the tools, the export, or the MCP
server (NFR-012). Opening a note without an edit must leave it unchanged, and a facade edit may change only the
edited block (NFR-007). Markdown and AsciiDoc notes must look as they do on GitHub and GitLab (C-003, C-004) and stay
plain files (C-002).

## Decision

- The facade engine lives in its own library crate, `crates/underline-facade`. `underline-core` keeps workspaces and
  notes; the desktop app and the MCP server both reach note content through the engine.
- The engine defines one Rust trait for a note format: parse a file into a document of blocks that keeps the source
  span of each block, render a document for the facade, write an edited block back as markup, report the
  format-specific options (for example image captions), and export.
- The note format of a file comes from its extension (`.md`, `.adoc`), looked up in a registry of format modules.
- A facade edit replaces only the source span of the edited block; unchanged blocks are written back byte for byte.
- **Markdown** is parsed with `comrak`: CommonMark with the GitHub Flavored Markdown extensions (tables, task lists,
  strikethrough, autolinks), with the source position of every node.
- **AsciiDoc** is parsed by an own parser in the crate for the blocks Underline shows: section titles, paragraphs,
  lists, tables, source and literal blocks, images, admonitions, and the document header. Any other block is kept as
  a raw block that the facade shows as markup and writes back unchanged. The rendering follows Asciidoctor.

## Consequences

- A new format is one new module in the registry.
- Keeping source spans makes the parsers more work than render-only parsers.
- The AsciiDoc parser is ours to maintain; its subset grows block by block, and raw blocks keep unknown syntax safe
  in the meantime (NFR-007).
- **Alternatives considered:** `pulldown-cmark` lacks GFM autolinks of bare URLs; the young `asciidoc-parser` crate
  covers too little yet; bundling Asciidoctor needs a Ruby runtime and gives no source spans.
