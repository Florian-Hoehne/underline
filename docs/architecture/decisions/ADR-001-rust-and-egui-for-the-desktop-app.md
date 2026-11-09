# ADR-001: Build the desktop app in Rust with egui/eframe

**Status:** Superseded
**Superseded by:** ADR-020
**Date:** 2026-11-09
**Requirements:** C-001, NFR-001, NFR-002, NFR-013

## Context

Underline runs as one desktop app on Linux, Windows, and macOS (C-001) with the same features everywhere (NFR-013).
It must start within 1 second (NFR-001) and show a typed character within 16 milliseconds (NFR-002). The existing
code base is a Rust workspace with an egui/eframe window.

## Decision

- Write Underline in Rust (stable toolchain, edition 2021, minimum version as in `Cargo.toml`).
- Build the window with egui/eframe, native on all three platforms (X11 and Wayland on Linux).
- Keep the domain logic in library crates (`crates/underline-core`, `crates/underline-wiki`, `crates/underline-mcp`);
  `apps/underline-desktop` only wires them to the window and the command line.
- CI runs `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` on Linux, Windows, and macOS.

## Consequences

- One native binary per platform without a browser engine, which keeps startup and typing latency low.
- egui is an immediate-mode toolkit: rich-text editing for the facade, the minimap, and folding have to be built on
  top of it rather than taken from a ready widget.
- **Alternatives considered:** a web-view shell (Tauri, Electron) would give HTML rendering of notes for free but
  costs startup time and memory.
