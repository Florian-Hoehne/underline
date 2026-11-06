# Open Questions

Questions about the specification that the specification does not answer yet. A question is `Resolved` only when
the answer is in the specification itself; **Resolved in** names where. Resolved and withdrawn questions stay as
history.

## OQ-001: What does the "date" of a todo mean next to its due date?

**Element:** FR-065
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** The scheduled date: the day the user plans to work on the todo.

The brief names "Frist, Dringlichkeit und Datum". Readings: (a) scheduled date (planned work day); (b) creation date,
set automatically; (c) a reminder date on which Underline notifies the user.

## OQ-002: Which urgency levels does a todo have?

**Element:** FR-064
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Four levels: low, normal, high, critical.

## OQ-003: Which notes does the note overview list?

**Element:** FR-017
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** All notes of the workspace, with the recently opened notes listed first.

The brief says "Übersicht der Notizen (aktuelle Notizen)". Readings: (a) all notes, recent first; (b) only the
recently opened notes; (c) the folder tree of the workspace with a separate list of recent notes.

## OQ-004: Which settings file does a change in the interface go to?

**Element:** FR-071
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** To the workspace settings file when that file already sets the changed setting, otherwise to the user
settings file.

## OQ-005: Where does a dropped image or file go?

**Element:** FR-033
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** It is copied into the workspace, next to the note, and the note links to the copy.

Readings: (a) copy into the workspace; (b) link to the original location outside the workspace; (c) ask on every drop.

## OQ-006: Default interval of the git sync?

**Element:** FR-045
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Every 5 minutes, adjustable by the user.

## OQ-007: When is the MCP server "offline"?

**Element:** FR-058
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** When the user has switched the MCP server off, or when no agent is connected to it.

## OQ-008: Which themes besides the named ones?

**Element:** FR-022
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Light, Nord, Dracula, and Gruvbox in addition to Dark, GitHub (light and dark), Solarized (light and
dark), Night Owl, and Ubuntu.

## OQ-009: Must diagrams render without a network connection?

**Element:** NFR-014
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Yes: diagram rendering and export work offline, like every function except git push, pull, and sync.

PlantUML diagrams are often rendered by an online service; working offline rules that out.

## OQ-010: Thresholds for startup and typing speed?

**Element:** NFR-001
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** The last opened note is shown within 1 second of start for a workspace of 1,000 notes (NFR-001); a typed
character appears within 16 milliseconds (NFR-002).

## OQ-011: How long may an edit wait before it is saved?

**Element:** NFR-005
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** At most 2 seconds after the last keystroke.

## OQ-012: Which operating system versions are supported?

**Element:** C-001
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Current Linux desktops, Windows 10 or later, macOS 13 or later.

## OQ-013: Do rename, delete, and search belong to the first release?

**Element:** FR-007
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Rename (FR-005) and delete (FR-006) are High; search (FR-007) is Medium. The brief does not name them.
