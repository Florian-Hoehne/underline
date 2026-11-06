# ADR-005: Save every file atomically through a temporary file

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** NFR-005, NFR-006, NFR-009

## Context

A change must be on disk at most 2 seconds after the last keystroke (NFR-005), and a crash during a save must leave
either the old or the new content (NFR-006). Settings must be written within 1 second (NFR-009), and a workspace that
cannot be created must leave nothing half set up (UC-001).

## Decision

- Every file Underline writes goes through one function in `underline-core`: notes (autosave and MCP writes), both
  settings files, the Home note and the workspace settings file of a new workspace (UC-001), and exports.
- That function writes to a hidden temporary file in the same directory, flushes it with `fsync`, replaces the target
  with an atomic rename, and then flushes the directory, so the rename survives a power loss.
- Autosave is debounced: a note is written 1 second after the last change, and at the latest 2 seconds after it.

## Consequences

- A half-written file cannot occur; a stray temporary file can remain after a crash and is removed the next time the
  workspace is opened.
- File watchers (including the one of ADR-002) see a rename instead of a modify event and must handle both.
- On Windows the rename replaces the target with `MoveFileEx` semantics; flushing a directory is a no-op there.
