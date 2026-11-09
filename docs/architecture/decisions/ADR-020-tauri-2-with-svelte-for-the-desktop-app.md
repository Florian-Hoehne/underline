# ADR-020: Build the desktop window with Tauri 2 and a Svelte/TypeScript frontend

**Status:** Accepted
**Date:** 2026-11-09
**Requirements:** C-001, NFR-013, NFR-001, NFR-002, NFR-011
**Supersedes:** ADR-001

## Context

Underline runs as one desktop app on Linux, Windows 10 or later, and macOS 13 or later (C-001), with the same
features everywhere (NFR-013), starts within 1 second (NFR-001), shows a typed character within 16 milliseconds
(NFR-002), and every function is reachable by keyboard (NFR-011). ADR-001 chose egui/eframe. The user decided to move
the window to Tauri 2 (plan in `docs/migration/plan.md`). The domain logic already lives in UI-free library crates
(`underline-core`, `underline-facade`, `underline-mcp`, `underline-wiki`), and about 2,600 to 3,000 lines of
orchestration still live in `apps/underline-desktop`. Overlaps: ADR-003 (facade engine stays), ADR-012 (editor
widget, replaced by ADR-021), ADR-013 (network access of the editor, see ADR-023), ADR-016 (file dialogs).

## Decision

- Build the window with Tauri 2 in `apps/underline-tauri/src-tauri` and a Svelte 5 + TypeScript frontend (Vite, no
  SvelteKit) in `apps/underline-tauri/ui`. The targets are Linux (WebKitGTK), Windows (WebView2) and macOS (WKWebView).
  There are no mobile or web targets.
- Domain logic stays in Rust library crates. The orchestration of `app.rs` is ported, with tests, into
  `crates/underline-session`. The Tauri commands are a thin layer over it.
- Commands, events and their types are generated as TypeScript with `specta`/`tauri-specta`
  (`ui/src/lib/bindings.ts`). A test fails when the generated file is outdated.
- The binary keeps the `mcp [DIR]` subcommand (ADR-002). It is named `underline` once it replaces the egui app,
  because installed agent configurations point at it (ADR-017).
- Tauri plugins and capabilities are kept to what a feature needs. Each plugin is named in the ADR of its feature.
- CI adds lint, type check, tests and the build of the frontend to the checks of ADR-001. On Linux it installs the
  WebKitGTK development packages.
- The egui app `apps/underline-desktop` stays unchanged until feature parity is confirmed
  (`docs/migration/paritaet.md`). Only then is it removed.

## Consequences

- HTML rendering, CSS theming, accessibility of web content and a large frontend ecosystem become available. The
  editor is rebuilt in the webview (ADR-021).
- A browser engine starts with the app. Startup time (NFR-001) and typing latency (NFR-002) have to be measured
  again on all three webviews.
- Two languages and two toolchains (cargo, npm) have to be maintained.
- **Alternatives considered:**
  - Staying on egui (ADR-001): rejected by the user's decision to migrate.
  - Leptos (all Rust via WASM): the editor needs direct DOM work and CodeMirror, which is reached through
    `web-sys` with more friction, and it has fewer test tools.
  - React + TypeScript: a heavier runtime and bundle for no gain in this UI.
