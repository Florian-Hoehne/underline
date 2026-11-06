# ADR-008: Keep the state of a workspace in its own `.underline/` folder

**Status:** Accepted
**Date:** 2026-11-06
**Supersedes:** ADR-007
**Requirements:** FR-071, NFR-009, NFR-001, C-002, C-006

## Context

After a restart or a crash Underline must come back with the last opened workspace and its open tabs (FR-071,
UC-001, UC-002), and the note overview lists the recently opened notes first (FR-017). A change must be stored within
1 second (NFR-009). ADR-007 kept all of this in one file in the user's data directory; the user wants the state of a
workspace to stay with the workspace, and kept out of git when the workspace is a git repository.

## Decision

- **Per workspace:** the open tabs (note, position, editing view, whether active) and when each note was last
  opened are kept in `<workspace>/.underline/state.yaml`, in every workspace, git repository or not.
- **Git ignores the folder by itself:** `.underline/` always holds a `.gitignore` with the single line `*`. The
  repository's own `.gitignore` is never touched, no change shows in `git status`, and the rule holds in every clone
  and after a later `git init`.
- **Per user:** the known workspaces and when each was last opened stay in `state.yaml` in the user's data
  directory (`~/.local/share/underline` on Linux, `%APPDATA%\underline` on Windows,
  `~/Library/Application Support/underline` on macOS, found with the `directories` crate), because Underline reads
  it before it knows which workspace to open.
- Both files are YAML, read and written in full with `serde_norway`, not meant to be edited by hand.
- Every change is written atomically (ADR-005) at most 1 second after it happens; changes within that time are
  written together. `.underline/` and its `.gitignore` are created with the first write.
- When a file is read, entries whose workspace directory or note file is gone are dropped. A file that cannot be read
  is renamed to `state.yaml.damaged`, Underline continues with an empty state, and the status bar says so.
- The state is read and written in `crates/underline-core`; the desktop app only reports the changes.

## Consequences

- A workspace's tabs and recent notes move with its directory, also to another machine when the directory is copied
  without git; a git clone starts without them.
- Opening a workspace adds the hidden `.underline/` folder to its directory (UC-002 BR-001); it never shows as a
  change in git and holds no notes, so it is not listed in the note overview (hidden folders are skipped).
- A directory that holds only `.underline/` is not empty and cannot take a new workspace (UC-001 BR-002).
- **Alternatives considered:** appending to the repository's `.gitignore` changes a committed file;
  `.git/info/exclude` holds only for one clone; one file in the user's data directory (ADR-007) separates the state
  from its workspace.
