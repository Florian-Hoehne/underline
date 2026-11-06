# ADR-013: Web images are the only network access of the editor, through ureq

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** NFR-015, NFR-014, NFR-013, UC-004 BR-004

## Context

A note may show an image from a web address, as GitHub shows it; without a network connection a placeholder names
its address (UC-004 BR-004). No note content may leave the machine except through git and the MCP server (NFR-015),
and everything else works offline (NFR-014). Diagrams render without a network (ADR-009), git goes through the
installed git program (ADR-006), and the editor itself has no other reason to reach the network.

## Decision

- **Only web images.** The facade engine (`crates/underline-facade`, module `images`) is the only place that makes
  network requests from the editor. Nothing else in the editor opens network connections; links to web addresses are
  opened by the system's browser after the user agrees (UC-004 A4).
- **A plain request.** Each web image is fetched with one `GET` of its address, with the user agent `Underline`, no
  cookies, no referrer, and no other headers or data of the note. The server learns the address and that Underline
  asks for it, as it would from a browser showing the same note.
- **Library.** `ureq` 3 (MIT or Apache-2.0): a small blocking HTTP client in pure Rust with TLS through `rustls`,
  on all three platforms (NFR-013). It honors proxy settings from the environment (`HTTPS_PROXY` and the like).
- **Limits.** A request is given up after 15 seconds; images larger than 50 MB are not loaded. Requests run on the
  image worker thread, never on the UI thread, and their results are cached for the session like files.
- **Failure.** Any failure (no network, refused, timeout, not an image) shows the placeholder naming the address;
  the rest of the note is unaffected.

## Consequences

- Reading a note with web images tells their servers that the note was opened, as a browser would; notes without
  web images cause no network traffic at all.
- A web image is loaded again in each session; there is no cache on disk.
- Settings to turn web images off, or to allow only some hosts, can be added later without changing this design.
- **Alternatives considered:** `reqwest` (async, pulls in an async runtime the app does not otherwise need);
  loading web images through the system browser or a webview (adds a browser engine, which ADR-001 avoided); not
  showing web images at all (contradicts UC-004 BR-004 and C-003).
