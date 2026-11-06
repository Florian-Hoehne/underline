# ADR-007: Keep the app state in a state file in the user's data directory

**Status:** Superseded by [ADR-008](ADR-008-workspace-state-in-underline-folder.md)
**Date:** 2026-11-06
**Requirements:** FR-071, NFR-009, NFR-001, C-002, C-006

## Context

After a restart or a crash Underline must come back with the last opened workspace and its open tabs (FR-071,
UC-001 postconditions), and the note overview lists the recently opened notes first (FR-017, OQ-003). The entity
model keeps this as WORKSPACE.last_opened, NOTE.last_opened, and OPEN_TAB. A change must be stored within 1 second
(NFR-009). This state is not a setting: the settings files `underline.yaml` stay for what the user sets and edits
by hand (C-006), and a workspace stays a directory of plain files that other editors and git see unchanged (C-002).

## Decision

- Underline keeps one state file per user, `state.yaml`, in the platform's data directory, found with the
  `directories` crate: `~/.local/share/underline` on Linux (`$XDG_DATA_HOME` when set),
  `%APPDATA%\underline` on Windows, and `~/Library/Application Support/underline` on macOS.
- It holds the known workspaces, each with its directory and when it was last opened; and per workspace its open
  tabs (note, position, editing view, whether active) and when each of its notes was last opened.
- The file is YAML, read and written with `serde_norway` into one state struct, and always written in full; it is
  not meant to be edited by hand.
- Every change is written atomically (ADR-005) at most 1 second after it happens; changes within that second are
  written together.
- On start, workspaces whose directory is gone and notes whose file is gone are dropped from the state. A file that
  cannot be read is renamed to `state.yaml.damaged` and Underline starts with an empty state, saying so in the status
  bar.
- The state is read and written in `crates/underline-core`; the desktop app only reports the changes.

## Consequences

- Workspaces carry no Underline files besides their workspace settings file; nothing has to be git-ignored.
- Tabs and recent notes do not travel with a workspace to another machine.
- Moving a workspace directory outside Underline loses its tabs and recent notes; it opens as a new workspace.
- **Alternatives considered:** the user settings file would mix frequently changing state with hand-edited settings;
  a hidden folder per workspace would show up in git and in other editors.
