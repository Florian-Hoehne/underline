# ADR-028: Edit note attributes as lines of the note, and check the front matter with `serde_norway` in the WASM editor core

**Status:** Accepted
**Date:** 2026-11-16
**Requirements:** NFR-007, NFR-002, NFR-013, C-008, FR-093

## Context

The note properties show the front matter of a Markdown note and the document attributes of an AsciiDoc note as
fields, where the user adds, changes, and removes them (FR-093, UC-024). Three requirements constrain this:

- **Format fidelity (NFR-007):** an attribute change may change only the lines of that attribute. Comments, blank
  lines, the order, and how the other values are written stay as they are (UC-024 BR-003).
- **Typing latency (NFR-002):** the note text lives in the webview (ADR-025). The fields are read from it and written
  back as edits of the open tab, as one undo step, without waiting for the backend.
- **Validity:** a front matter the YAML rules do not accept is shown read-only with its line and reason and never
  rewritten (UC-024 A5).

The YAML library of the workspace is `serde_norway` (ADR-004, settings). It deserialises into values. Writing values
back through it would drop comments and reformat the whole front matter. The library must fit the MIT license
(C-008) and build for `wasm32-unknown-unknown`.

This ADR overlaps the following:

- **ADR-025:** the WASM core grows by the YAML parser.
- **ADR-004:** the same library, now also in `underline-markup`.

## Decision

- **Editing:**
  - `underline-markup::attributes` finds the front matter (Markdown) or the attribute entries of the header
    (AsciiDoc) and reads each attribute with its line range and kind.
  - A change replaces only the lines of that attribute. A new attribute goes after the last one, and removing the
    last one removes the front matter or header entry. The front matter is never re-serialised from parsed values.
  - A value that would read back as another kind (`true`, `12`, `a: b`) is written in double quotes. A newline or
    other character that would break the line is escaped, so a value never creates another key.
- **Checking:**
  - `serde_norway` (MIT or Apache-2.0) parses the front matter only to decide whether it is valid YAML and what kind
    each value has.
  - A front matter it refuses is reported with the library's line and reason and is not offered for editing.
  - AsciiDoc document attributes are read by the crate's own line rules; YAML is not involved.
- **Placement:**
  - `underline-markup` takes `serde_norway` and `thiserror` as workspace dependencies.
  - The editor view exposes the reading and the edits to the webview (`noteHead`, `attribute`, `facadeAttribute`).
    The export and the search use the same module natively.

## Consequences

- **Fidelity:** comments, order, and the way untouched values are written survive every change. Tests check
  byte-identical round trips.
- **One implementation:** one module decides what an attribute is, for the panel, the facade table, the exports, and
  the tag search.
- **Line-based editing is harder:** the line rules must cover quoting, block lists, comments after values, CRLF, and
  a byte order mark themselves. Test-monkey's property tests guard them.
- **WASM size:** the file grows by the YAML parser. It is 2.67 MB after package 6a, packages 2 to 5 included.
- **Error wording:** YAML error reasons are the library's English texts.
- **Alternatives considered:**
  - **Re-serialise the parsed YAML with `serde_norway`:** this loses comments and reformats untouched values, against
    NFR-007.
  - **A comment-preserving YAML editing library:** none is maintained in Rust that builds for wasm32 and keeps
    byte-exact formatting.
  - **Read and write the attributes in the backend:** every field change would need an IPC round trip and a rebase
    against typing, against ADR-025.
