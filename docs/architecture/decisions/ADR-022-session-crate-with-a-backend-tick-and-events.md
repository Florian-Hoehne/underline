# ADR-022: A UI-free session crate drives the window by a backend tick and pushes changes as events

**Status:** Proposed
**Date:** 2026-11-09
**Requirements:** NFR-005, NFR-008, NFR-009, NFR-002, NFR-013

## Context

The egui app polls its background services in the frame loop and keeps several fixed clocks:

| Every | Work |
|---|---|
| 100 ms | agent requests (ADR-018) |
| 250 ms | autosave |
| 500 ms | user settings and the state file |
| 1 s | presence and todos |
| 3 s | git changes |
| the sync interval | git sync |

These clocks are what keeps the quality requirements:

- autosave within 2 seconds (NFR-005);
- agent changes visible within 1 second (NFR-008);
- settings stored within 1 second (NFR-009).

A webview has no frame loop on the Rust side. Much git and file I/O runs on the UI thread today and takes time away
from typing (NFR-002). All of this has to behave the same on every system (NFR-013).

## Decision

- `crates/underline-session` holds the workspace session without UI:
  - open tabs with their text on disk, autosave clock and save errors;
  - the services of `underline-core`;
  - the settings state, the status message with its offers, and the rules ported from `apps/underline-desktop`.

  It depends on no UI or Tauri crate. Its DTOs derive `specta::Type` behind a `specta` feature.
- A backend thread calls `Session::tick(now)` every 100 ms. The tick keeps the clocks of the egui app unchanged and
  returns the events to emit.
- The services' wake callbacks trigger a tick at once, where they used to ask egui for a repaint.
- The tick and the commands emit events such as `status`, `overview`, `tab-changed`, `agent-edit`, `agent-marks`,
  `search-result`, `changes`, `sync`, `conflicts`, `todos`, `settings`, `diagram-ready` and `export-done`. Each event
  carries the whole changed part of the state, never a delta. Editor edits are the exception.
- Commands that do file, git or search I/O are `async` or use `spawn_blocking`, so they run on Tauri's thread pool
  and never on the main thread. They lock the session only briefly; the lock is `parking_lot::Mutex`, which does not
  poison.
- A command returns `Result<T, UiError>`. `UiError.message` is the status text the egui app shows today.

## Consequences

- The timing rules stay where the tests can reach them: `tick` is tested with a given `now`. The Tauri layer stays
  thin.
- The frontend does not poll. It mirrors the events in stores.
- Until the egui app is removed, the orchestration exists twice, in `app.rs` and in the session crate (user
  decision: the egui app stays unchanged as the reference).
- **Alternatives considered:**
  - Polling commands from the frontend on timers: the timing would move into the UI, and it would add IPC traffic.
  - Moving the egui app onto the new crate first: one copy of the logic, but the reference changes without tests.
    Rejected by the user.
