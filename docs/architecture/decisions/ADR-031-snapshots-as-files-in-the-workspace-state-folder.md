# ADR-031: Keep snapshots as plain files in the workspace's `.underline/` folder, one file per snapshot

**Status:** Proposed
**Date:** 2026-11-13
**Requirements:** NFR-005, NFR-014, C-006, FR-101, FR-102

## Context

Underline keeps earlier versions of a note as snapshots on this computer. It does so in a workspace that is a git
repository and in one that is not (FR-101; UC-026 BR-001 to BR-005). A snapshot is taken in these cases:

- while the user edits;
- before a change from another program, an agent, a replace in the workspace (UC-022 BR-005), a git pull or switch of
  branch, a conflict choice, or a restore.

The user lists the snapshots of a note, compares one with the note and restores it (FR-102).

The rules a snapshot must follow:

- A snapshot must never be committed or shared through git.
- It must keep the note's exact bytes.
- It must follow the note when it is renamed or deleted.
- It must not slow typing or the autosave (NFR-005).
- The MCP server can change a note that is not open from a separate process (ADR-002, ADR-018), and the snapshot must be
  taken there too.

ADR-008 already keeps the workspace's state in `.underline/`, a folder that ignores itself through its own
`.gitignore`. ADR-005 makes every write atomic.

## Decision

- **Place:** `<workspace>/.underline/snapshots/<the note's file as in the workspace>/`, for example
  `.underline/snapshots/Projects/Plan.md/`. The snapshots of a deleted note move with it into its folder under
  `.underline/deleted/<at>/.snapshots/` and come back when it is restored.
- **Format:**
  - One file per snapshot, named `<taken at, in milliseconds>-<cause>[-<n>].snap`. The cause is one of `editing`,
    `outside`, `agent`, `replace`, `git` or `restore`. `<n>` counts the snapshots of one note within the same
    millisecond, across causes, so their order is always defined.
  - The file holds the note's bytes exactly, byte order mark and line breaks included. There is no index file: the
    names are the index.
  - Files with other names are ignored and never pruned. A file that is not text is listed as damaged and cannot be
    restored.
- **Writing:**
  - Snapshots are written atomically and never overwrite (ADR-005). Keys and names are checked against leaving the
    folder, and no symbolic link below the workspace is followed for writing, moving, deleting or reading.
  - A snapshot whose text equals the newest one kept is not taken again. The check and the write hold the lock of the
    note's folder (one of 64); pruning takes the same lock before it removes a folder.
  - Moving snapshots (rename, deletion, restore) links each to the next free number, never over an existing file.
  - Snapshots taken while editing are written on the saver's thread before the note's next save, so they never block
    typing or the autosave. The editing snapshot is taken at most every 10 minutes and holds what the file had before.
  - Replace and restore take theirs at once, because they write the note at once.
- **Keeping:**
  - Snapshots are kept 30 days. Older ones are not listed and are pruned on a thread of their own when the workspace
    opens.
  - Pruning also removes stale temporary files and the leftover snapshots of deleted notes older than 30 days.
  - There is no size or count limit yet (OQ-294).
  - Snapshots that cannot follow a rename, a deletion or a restore stay where they are, and the user is told.
- **Comparing:** a line comparison stops after 200 ms and is then coarser, never smaller. The list compares at most 30
  snapshots within one second; the others show no line count.
- **Failure:**
  - Restore, replace, and an agent's change of a note that is not open are refused when their snapshot cannot be
    taken.
  - The other causes go on and say so in the status bar (OQ-291).

## Consequences

- Snapshots work without git, are never committed, and are invisible to other tools that read the workspace's notes.
- A snapshot is an ordinary file the user can open, copy or delete by hand; nothing is lost when an index would be out
  of date.
- **Disk use:** a workspace's `.underline/` folder grows by up to 30 days of a note's versions. There is no cap and no
  display of the space used (OQ-294).
- **Cross-process races:** the check against the newest snapshot runs under a lock in the process, held across the
  write and its fsync. Two processes, such as
  the window and the MCP server, may each take an equal snapshot. They never overwrite each other's file.
- **Rename and delete by another program:** a note renamed or deleted outside Underline leaves its snapshots under the
  old name (OQ-296).
- **Alternatives considered:**
  - **Git objects or a hidden branch:** this needs git, against BR-005, and a hidden branch could be pushed by mistake.
  - **One database file** (for example SQLite): this adds a dependency and a single point of corruption. The notes
    themselves are plain files (C-006).
  - **Storing deltas instead of whole texts:** this saves space, but a damaged delta breaks every later version, and
    restoring needs the chain. Notes are small.
  - **The user's data folder** (`~/.local/share/underline`): snapshots would not move with the workspace. The machine's
    folder would grow for every workspace ever opened.
