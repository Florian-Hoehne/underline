# ADR-018: Agent changes go through the running window by request files in `.underline/agents/`

**Status:** Accepted
**Date:** 2026-11-09
**Requirements:** FR-056, FR-057, NFR-006, NFR-008, NFR-016, UC-019 A5, A6, A11, BR-004, BR-007, BR-010, UC-017 BR-007

## Context

The MCP server is its own process, started by the agent tool (ADR-002); the window is another, or none. UC-019 asks
that an agent's change never loses what the user typed and has not saved (A5, A6), that it is one undo step of the
open tab (A11), and that the status bar reports changes to notes not open (BR-010). Only the window knows the unsaved
text and the undo history of a tab. While the window is closed, the server must write on its own (UC-017 BR-007).
Presence files of running servers are already kept in `.underline/agents/` (UC-017, ADR-008).

## Decision

- **The window says it runs.** A running window keeps `.underline/agents/window`, renewed every second like the
  servers' presence files; a file not renewed for 3 seconds does not count.
- **While the window runs, every agent change goes through it.** The server writes the change as a request file
  `.underline/agents/requests/<server pid>-<n>.yaml` (note, kind of change, text, agent name), written atomically
  (ADR-005). The window looks for requests about every 100 ms while an agent is connected, applies each to the open
  tab (refusing it by UC-019 A5, or merging it, A6) or, for a note not open, to the file, stores the note atomically,
  and answers in `.underline/agents/answers/<same name>.yaml`. The server waits up to 1 second for the answer, then
  removes both files.
- **While no window runs**, the server applies the change to the file itself and stores it atomically.
- **The window does not answer in time** (hung, or closed in between): when its file is stale the server applies the
  change itself; otherwise it refuses with "Underline is busy; try again", so nothing is written twice.
- **One place for the rules.** Finding the passage, inserting, the "touches a line" rule, conflicts, and creating
  notes are one core function used by both the server and the window, so both decide the same way.
- Requests and answers use YAML through `serde_norway`, already a dependency (ADR-004); no new crate.

## Consequences

- No socket or port is opened, so only processes of this user on this computer take part (NFR-016), on every
  operating system alike (NFR-013); the folder is ignored by git (ADR-008).
- An agent's change waits for the window's next look, about 100 ms, well within NFR-008.
- A request left by a killed server is removed by the window after a minute.
- **Alternatives considered:** a local socket or named pipe between server and window (faster, but platform-specific
  code on Windows and Unix, and a listener to secure); letting the server write the file and the window merge it as an
  outside change (UC-004 A9; cannot refuse A5 before writing, and gives no undo step of its own); the window as the
  MCP server (agents could not work while it is closed, against UC-017 BR-007).
