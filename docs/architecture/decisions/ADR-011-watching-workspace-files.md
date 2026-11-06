# ADR-011: Watch the workspace with notify, poll when events are missing

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** NFR-008, NFR-003, NFR-013, FR-017, FR-046, FR-070

## Context

A change to an open note by another program, by git, or by an agent through the MCP server (a separate process,
ADR-002) must appear in its tab within 1 second (NFR-008, UC-004 A7 and BR-009). A deleted note stays open, marked
(UC-004 A8), and a change from outside while a change of the user is not stored yet keeps both versions (UC-004 A9,
UC-005 A16). The note overview follows new, renamed, and deleted notes (FR-017), and a changed workspace settings
file applies (FR-070). Workspaces can hold 10,000 notes (NFR-003). Underline writes every file atomically through a
temporary file and a rename (ADR-005), so its own writes look like renames to a watcher.

## Decision

- **Library:** `notify` (CC0-1.0) with `notify-debouncer-full` (MIT or Apache-2.0), which uses inotify on Linux,
  FSEvents on macOS, and ReadDirectoryChangesW on Windows, joins the events of an atomic rename into one change, and
  follows renames by file id. Events are collected for 200 milliseconds, which keeps changes within the 1 second of
  NFR-008.
- **Scope:** the whole workspace directory, recursively. Events in hidden folders (`.git`, `.underline`), in folders
  the git repository ignores (the set from `git ls-files --ignored`, ADR-006, refreshed when a `.gitignore` changes),
  and in folders that are workspaces of their own (UC-002 BR-007) are dropped, so they cause no work.
- **Own writes:** every write through the atomic write of ADR-005 records the path and a hash of the written content.
  An event for that path whose file now has that content is Underline's own write and is dropped. Comparing content,
  not times, also drops a change that another program made identical to the note.
- **What a change means:** `underline-core` turns the events into typed workspace events, which the desktop app
  handles:
  - a note's content changed: reload its tab (UC-004 A7), or keep both versions when the tab holds an unstored
    change (UC-004 A9);
  - a note was deleted or moved away: mark its tab (UC-004 A8) and update the note overview;
  - a note was created or moved in: update the note overview;
  - the workspace settings file changed: read it again (ADR-004);
  - more than 100 changes within one collection window, as after `git checkout` or `git pull`: rescan the workspace
    once (UC-002 step 7) and reload the open tabs whose files changed, instead of handling each event.
- **Fallback:** when the watcher cannot be started or stops delivering (for example a network drive, or the Linux
  limit on inotify watches reached), Underline switches to polling: the files of the open tabs every second (by size,
  modification time, and, when those changed, content hash), and the whole workspace every 10 seconds. The status bar
  says once that changes from outside may show up later in the note overview. NFR-008 still holds for open notes.
- **Location:** `crates/underline-core` provides a `WorkspaceWatcher` that sends the typed events over a channel;
  the desktop app asks egui for a repaint when an event arrives. The MCP server (ADR-002) needs no watcher of its
  own: it reads the files when an agent asks.

## Consequences

- Changes from the MCP server, git, and other editors all reach the window by the same path.
- A workspace of 10,000 notes in a few hundred folders stays well below the usual inotify limit; very large trees fall
  back to polling instead of failing.
- Remembering the hash of each own write costs a few bytes per recently written file; entries older than a minute
  are dropped.
- **Alternatives considered:** polling only (simple, but costs CPU and battery in large workspaces and delays the
  overview); watching only the open notes (fewer watches, but the overview would lag behind git and agents).
