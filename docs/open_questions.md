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

## OQ-014: Can a workspace sync with more than one git remote?

**Element:** Entity: GIT_REMOTE
**Raised by:** /entity-model, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** No: a workspace syncs with at most one git remote, the one its current branch tracks.

## OQ-015: May two notes of a workspace have the same title?

**Element:** Entity: NOTE.title
**Raised by:** /entity-model, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** No: titles are unique within a workspace, so a local link by title always finds one note.

Notes in different folders of a workspace could share a file name (`a/Home.md`, `b/Home.md`). Readings: (a) titles
are unique in the workspace; (b) titles are unique per folder, and local links name the folder.

## OQ-016: Are a due date and a scheduled date required for a todo?

**Element:** Entity: TODO.due_date
**Raised by:** /entity-model, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Both are optional; only the title is required when a todo is created.

## OQ-017: Which built-in defaults do the settings have?

**Element:** Entity: SETTINGS
**Raised by:** /entity-model, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** Theme Light, input mode Normal, line numbers shown on the left, copy on select on, git sync every
5 minutes, MCP server off.

## OQ-018: Does creating a workspace as a git repository make a first commit, on which branch?

**Element:** UC-001 A3
**Raised by:** spec review, 2026-11-06
**Blocking:** Yes
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), A3 and BR-007

Answer: yes. The Home note and the workspace settings file are committed as "Create workspace" on the branch the
user's git setup names as default (main when it names none).

## OQ-019: What happens when the git remote of a new workspace already holds a history?

**Element:** UC-001 BR-004
**Raised by:** spec review, 2026-11-06
**Blocking:** Yes
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), BR-004

Answer: creation stays offline. The first git sync overwrites neither side and tells the user; an existing
repository is opened as a workspace (UC-002) instead.

## OQ-020: In which editing view does the Home note of a new workspace open?

**Element:** UC-001 step 7, Entity: OPEN_TAB.editing_view
**Raised by:** spec review, 2026-11-06
**Blocking:** Yes
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), step 7

Answer: in the facade.

## OQ-021: What happens to the open workspace when a new one is created?

**Element:** UC-001 step 6
**Raised by:** spec review, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), step 6 and postconditions

Answer: it is closed in the same window, and its open tabs are remembered. It stays open when creating fails or is
cancelled.

## OQ-022: Do hidden files and folders make a directory non-empty?

**Element:** UC-001 BR-002
**Raised by:** spec review, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), BR-002

Answer: yes, any content counts, including a lone git repository.

## OQ-023: May a workspace be created inside another workspace?

**Element:** UC-001 step 2
**Raised by:** spec review, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), A7 and BR-006

Answer: no; the user is asked for another directory.

## OQ-024: Is the Home note the start page of a GitLab wiki too?

**Element:** UC-001 BR-003
**Raised by:** spec review, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), BR-003

Answer: the note stays titled Home with its title as heading; BR-003 claims the start page for a GitHub wiki only.

## OQ-025: What if the leftovers of a failed workspace creation cannot be removed?

**Element:** UC-001 A5
**Raised by:** spec review, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), A6

Answer: the status bar names what is left, so the user can delete it by hand.
