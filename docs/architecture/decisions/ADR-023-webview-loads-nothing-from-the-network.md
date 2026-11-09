# ADR-023: The webview loads nothing from the network; workspace files and web images come through an own protocol

**Status:** Proposed
**Date:** 2026-11-09
**Requirements:** NFR-015, NFR-014, NFR-016

## Context

No note content may leave the machine except by git push and to a connected agent (NFR-015). Everything except git
works offline (NFR-014). Only processes on this machine may reach Underline (NFR-016). ADR-013 makes web images the
editor's only network access: a plain request through `ureq`, after the user agreed. A webview, unlike egui, loads
`<img src="https://…">`, fonts and scripts by itself. Overlaps: ADR-013 (compatible; the request stays in the
backend), ADR-018 (no socket or port).

## Decision

- The content security policy of the window allows only the app's own files, the IPC channel, the own protocol
  `ul-asset:` and `data:` images. It names no other host.
- `ul-asset://` is answered by the backend. It serves:
  - files below the open workspace root, after resolving links and `..`;
  - web images that the backend fetched under ADR-013 and holds in its cache.

  Every other path is refused.
- The frontend bundles all its fonts and scripts. No CDN is used.
- No local HTTP server and no port is opened. IPC uses Tauri's in-process channel.

## Consequences

- A link or image in a note can never make the webview contact a host by itself. ADR-013's consent stays the single
  gate.
- Every image, including local ones, goes through the protocol handler. The handler is the one place to test the
  workspace boundary.
- **Alternatives considered:**
  - Tauri's asset protocol with a scope: its scope is static configuration, while the workspace changes at run time.
  - Letting the webview fetch web images: against NFR-015 and ADR-013.
