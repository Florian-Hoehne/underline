# ADR-003: Facade engine with one format module per note format

**Status:** Proposed
**Date:** 2026-11-06
**Requirements:** NFR-012, NFR-007, C-002, C-003, C-004

## Context

Further note formats must become available without changing the editing views, the tools, the export, or the MCP
server (NFR-012). Opening a note without an edit must leave it unchanged, and a facade edit may change only the
edited block (NFR-007). Markdown and AsciiDoc notes must look as they do on GitHub and GitLab (C-003, C-004) and stay
plain files (C-002).

## Decision

- The facade engine defines one Rust trait for a note format: parse a file into a document of blocks that keeps the
  source span of each block, render a document for the facade, write an edited block back as markup, report the
  format-specific options (for example image captions), and export.
- Markdown follows CommonMark with the GitHub Flavored Markdown extensions (tables, checklists, strikethrough,
  autolinks). AsciiDoc follows the syntax as implemented by Asciidoctor.
- A facade edit replaces only the source span of the edited block; unchanged blocks are written back byte for byte.
- The note format of a file comes from its extension (`.md`, `.adoc`), looked up in a registry of format modules.

## Consequences

- A new format is one new module in the registry.
- Keeping source spans makes the parser more work than a plain render-only parser.
- No mature AsciiDoc parser exists in Rust; one has to be chosen or written (open).
