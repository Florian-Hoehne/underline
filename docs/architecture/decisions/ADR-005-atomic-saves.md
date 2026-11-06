# ADR-005: Save notes atomically through a temporary file

**Status:** Proposed
**Date:** 2026-11-06
**Requirements:** NFR-005, NFR-006

## Context

A change must be on disk at most 2 seconds after the last keystroke (NFR-005), and a crash during a save must leave
either the old or the new content (NFR-006).

## Decision

- Autosave is debounced: a note is written 1 second after the last change, and at the latest 2 seconds after it.
- Every write goes to a temporary file in the same directory, is flushed with `fsync`, and replaces the note with an
  atomic rename.

## Consequences

- A half-written note cannot occur; a stray temporary file can remain after a crash and is removed at the next start.
- File watchers (including the one of ADR-002) see a rename instead of a modify event and must handle both.
