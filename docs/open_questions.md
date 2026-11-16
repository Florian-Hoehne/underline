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
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), step 7 and BR-008

Answer: reading (a): all notes of the workspace, the recently opened ones first (most recent on top), then the others
by title from A to Z.

The brief says "Übersicht der Notizen (aktuelle Notizen)". Readings: (a) all notes, recent first; (b) only the
recently opened notes; (c) the folder tree of the workspace with a separate list of recent notes.

## OQ-004: Which settings file does a change in the interface go to?

**Element:** FR-071
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [Entity model](entity_model.md), SETTINGS constraints

Answer: to the workspace settings file when that file already sets the changed setting, otherwise to the user
settings file.

## OQ-005: Where does a dropped image or file go?

**Element:** FR-033
**Raised by:** /requirements, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A9 to A11 and BR-012

Answer: reading (a): it is copied into the folder of the note and linked by its place; a file of the workspace is linked where it is.

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
**Status:** Resolved
**Resolved in:** [Requirements](requirements.md), FR-058 (agent cursor moved to UC-019 in the review of UC-004)

Answer: as FR-058 states: while the MCP server is switched off or no agent is connected.
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
**Status:** Resolved
**Resolved in:** [Requirements](requirements.md), NFR-014; [UC-004](use_cases/UC-004-read-note.md), BR-004

Answer: yes; NFR-014 names diagram rendering among the functions that work offline.

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
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), BR-004; [Entity model](entity_model.md), NOTE

Answer: reading (b). Titles are unique per folder; the note overview shows the folder next to a title that occurs
more than once. How a local link names the folder is left to the use case that follows links.

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

## OQ-026: What happens when git cannot make the first commit of a new workspace?

**Element:** UC-001 A5
**Raised by:** architecture review (ADR-006), 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-001](use_cases/UC-001-create-workspace.md), A5

Answer: A5 applies. The status bar says why, for example that git is not installed or has no user name and email, and
how to set them in git. Underline keeps no identity of its own.

## OQ-027: What happens when the directory to open does not exist?

**Element:** UC-002 step 4
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), A3, A4

Answer: The system says so and offers to create a new workspace there (UC-001); nothing is created silently.

## OQ-028: Which default format has a workspace without a workspace settings file?

**Element:** UC-002 step 6, Entity: SETTINGS.default_format
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), BR-001, BR-002

Answer: The format most of its notes have, Markdown on a tie or without notes; opening writes no settings file.

## OQ-029: Which files of an opened directory are notes?

**Element:** UC-002 step 7
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), BR-003

Answer: Every `.md` and `.adoc` file in the directory and its folders, except in hidden folders and except files the git repository ignores.

## OQ-030: Which note opens when a workspace is opened for the first time?

**Element:** UC-002 step 8
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), BR-005

Answer: The Home note, else the README note of the top folder, else no tab.

## OQ-031: What happens when the workspace settings file cannot be read?

**Element:** UC-002 step 6
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), A8

Answer: The workspace opens with the user settings and the derived default format; the status bar names the file and the problem; the file stays as it is.

## OQ-032: May a directory inside a known workspace be opened as a workspace?

**Element:** UC-002 step 4
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), A5, A6

Answer: The system offers to open the outer workspace instead or the directory anyway.

## OQ-033: How does the user choose the workspace to open?

**Element:** UC-002 steps 2–3
**Raised by:** /use-case-spec UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), steps 2–3, A1, A2, BR-006

Answer: From the recently opened workspaces, most recent first, or any other directory; a directory named at start opens directly.

## OQ-034: Do the notes of a workspace inside another workspace belong to both?

**Element:** UC-002 A5, UC-001 BR-006
**Raised by:** spec review UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), BR-003, BR-007

Answer: No; a folder that is a known workspace of its own is left out of the workspace around it.

## OQ-035: Can the home directory be opened as a workspace?

**Element:** UC-002 step 4
**Raised by:** spec review UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), A11, BR-007

Answer: No; it holds the user settings file and all of the user's files, so the system asks for another directory.

## OQ-036: What happens when the Underline folder cannot be written or read?

**Element:** UC-002 step 8
**Raised by:** spec review UC-002, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-002](use_cases/UC-002-open-workspace.md), A12

Answer: The workspace opens anyway; the status bar says once that its tabs and recent notes cannot be remembered or could not be read.

## OQ-037: In which folder is a new note created?

**Element:** UC-003 step 2
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), step 2, BR-006

Answer: In the folder of the note in the active tab (the top folder without one); the user may choose another folder.

## OQ-038: What does a new note contain?

**Element:** UC-003 step 5
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-004

Answer: Only its title as heading, like the Home note.

## OQ-039: How does the title of a new note become its file name?

**Element:** UC-003 BR-001
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-001, BR-002

Answer: Runs of spaces become one dash; characters a file name cannot hold are refused.

## OQ-040: May a single note get another note format than the default format?

**Element:** UC-003 step 3
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-005

Answer: Yes; the default format is preselected and stays unchanged.

## OQ-041: What happens when a note with the same title exists in the folder?

**Element:** UC-003 step 4
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), A2, A3, BR-003

Answer: The system refuses and offers to open the existing note; case and note format do not make a title different.

## OQ-042: When is a new note written to the disk?

**Element:** UC-003 step 5
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-007

Answer: At once, when it is created; later changes by autosave.

## OQ-043: May a new folder be created together with a note?

**Element:** UC-003 step 3
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-006, A4

Answer: Yes, also several levels deep, inside the workspace; not hidden and not inside another workspace.

## OQ-044: In which editing view does a new note open?

**Element:** UC-003 step 6
**Raised by:** /use-case-spec UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), step 6

Answer: In the facade.

## OQ-045: Are 'Pre-Release' and 'Pre Release' two titles?

**Element:** UC-003 BR-001
**Raised by:** spec review UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), BR-001, BR-003, step 2

Answer: No; both give the file name Pre-Release.md. The check compares file names, the dialog shows the file name, and a dash is shown as a space as in GitHub wikis.

## OQ-046: May a note be created in a folder that git ignores?

**Element:** UC-003 step 4
**Raised by:** spec review UC-003, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-003](use_cases/UC-003-create-note.md), A4, BR-006

Answer: No; such a folder is refused like a hidden one, because the note would not be listed.

## OQ-047: Do 250 characters always fit into a file name?

**Element:** UC-003 BR-002
**Raised by:** implementation of UC-003, 2026-11-06
**Blocking:** No
**Status:** Open
**Assumed:** The limit stays at 250 characters. A title of letters such as "ü" or "中" can still be too long for the
file system (Linux and macOS allow 255 bytes per file name, which can be fewer than 250 such letters); then A5
applies and the status bar says that the file name is too long.

Readings: (a) keep 250 characters and report the rare case through A5; (b) lower the limit so that every title fits
on every file system (about 60 characters of any script); (c) count the limit in the units of the file system and
say so in the message.

## OQ-048: What does a click on a note in the note overview open?

**Element:** UC-004 step 2
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), step 2, A1, BR-001

Answer: A tab of its own; a note that is open already gets its tab activated.

## OQ-049: In which editing view does an existing note open?

**Element:** UC-004 step 2
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-003

Answer: In the facade; a tab keeps the view the user switched it to, also after a restart.

## OQ-050: How is a local link followed, given a click also places the cursor?

**Element:** UC-004 A2
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-006

Answer: By Ctrl+click (Cmd+click on macOS) in both views; pointing at a link shows where it leads.

## OQ-051: What happens with links to web addresses and other files?

**Element:** UC-004 A4
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A4, BR-007

Answer: Web addresses open in the default browser after the user agreed once in the session; other files of the workspace open with the system's program.

## OQ-052: What does following a link to a missing note do?

**Element:** UC-004 A3
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A3

Answer: It offers to create the note through UC-003, with title and folder from the link filled in.

## OQ-053: What happens when an open note is changed outside Underline?

**Element:** UC-004 A7
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A7, A8, A9, BR-009

Answer: The tab shows the new content within 1 second and keeps its reading position; a deleted note stays open, marked as deleted.

## OQ-054: What happens with a note that cannot be read?

**Element:** UC-004 A9
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A10

Answer: The status bar says why; no tab opens; the file is left as it is.

## OQ-055: How many tabs may be open?

**Element:** UC-004 step 2, Entity: OPEN_TAB.position
**Raised by:** /use-case-spec UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A6, BR-008

Answer: 100; one more closes the tab used longest ago.

## OQ-056: Does the agent cursor belong to reading a note?

**Element:** UC-004 A11, FR-057, FR-058
**Raised by:** spec review UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), use case diagram traceability

Answer: No; it moves to UC-019 (and UC-018), whose agent causes it. UC-004 keeps only that outside changes appear within 1 second.

## OQ-057: What happens when a note is changed outside while a change of the user is not stored yet?

**Element:** UC-004 A7
**Raised by:** spec review UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), A9, BR-002

Answer: Neither version is overwritten: the tab keeps the user's text, the outside version is kept as a copy next to the note, and the status bar offers to compare them.

## OQ-058: Are images from web addresses loaded?

**Element:** UC-004 BR-004
**Raised by:** spec review UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-004

Answer: Yes, as on GitHub; offline a placeholder names the address.

## OQ-059: Where does a new tab appear?

**Element:** UC-004 step 2
**Raised by:** spec review UC-004, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), step 2, BR-008

Answer: Right of the active tab.

## OQ-060: How far does undo reach?

**Element:** UC-005 A5
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-004

Answer: Per tab, across switches of the editing view, until the tab is closed; not after a restart.

## OQ-061: How is a block edited that the facade cannot format?

**Element:** UC-005 A4
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A4, BR-008

Answer: Its markup is edited in place, inside a frame, in the facade.

## OQ-062: What does the Vim input mode cover?

**Element:** UC-005 A6, FR-024
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-009

Answer: A common subset (modes, motions, operators, counts, undo, search, :q, :w), in both editing views.

## OQ-063: What happens when a dropped file's name is taken?

**Element:** UC-005 A10
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A10

Answer: A file with the same content is reused; otherwise the copy gets the next free number.

## OQ-064: In which form does copy on select copy from the facade?

**Element:** UC-005 A8, FR-031
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-011

Answer: As markup of the note format and as formatted text; from the source view as markup.

## OQ-065: What happens when autosave cannot store a note?

**Element:** UC-005 A14, FR-044
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A14, BR-013

Answer: The tab keeps the changes, marked as not stored; the status bar says why until it is stored; the system retries; closing asks first.

## OQ-066: Which format-specific options does the facade offer first?

**Element:** UC-005 A3, FR-012
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-007

Answer: For AsciiDoc: captions for images and tables, admonition blocks, and ids; none beyond the shared ones for Markdown.

## OQ-067: What happens when a note deleted outside is edited?

**Element:** UC-005 step 1, UC-004 A8
**Raised by:** /use-case-spec UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A15

Answer: The note is stored again at its place and listed again; the status bar says so.

## OQ-068: What does pasting formatted text do?

**Element:** UC-005 trigger
**Raised by:** spec review UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A17, BR-014

Answer: In the facade, formatting the note format can hold is kept as its markup; the source view and Ctrl/Cmd+Shift+V paste plain text.

## OQ-069: What does pasting an image do?

**Element:** UC-005 trigger
**Raised by:** spec review UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A18, BR-014

Answer: It is stored as a PNG next to the note, named after the note and the time, and inserted like a dropped image.

## OQ-070: How can a selection be replaced by pasting while copy on select is on?

**Element:** UC-005 A8, FR-031
**Raised by:** spec review UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-011

Answer: Pasting over a selection pastes what was on the clipboard before the selection was made.

## OQ-071: What happens with very large dropped files?

**Element:** UC-005 A9
**Raised by:** spec review UC-005, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), A19, BR-012

Answer: Above 10 MB the system asks whether to copy the file or only link to its place on this computer; such a link is followed by UC-004 A4.

## OQ-072: How is a diagram edited in the facade, where it is shown as a picture?

**Element:** UC-005 BR-008, FR-036, FR-037
**Raised by:** implementation of diagrams, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-008

Answer: While the cursor is in the diagram, its source is shown below the picture and edited there; the picture
follows each change. Without the cursor only the picture is shown.

## OQ-073: How is an image or diagram enlarged, and how is the enlarged view used?

**Element:** FR-039, UC-004 step 5
**Raised by:** implementation of enlarging, 2026-11-06
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-011

Answer: A double click or a magnifier at the corner of the picture opens it over the whole window, where it is
zoomed, moved, and fitted again; escape, a click beside it, or its close button returns to the note at the same place.

## OQ-074: How is a table edited in the facade?

**Element:** UC-005 BR-008, UC-004 step 3
**Raised by:** implementation of tables, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-008

Answer: As a diagram: the table is shown formatted, and while the cursor is in it, its markup is shown below it and
edited there; editing the cells in place may follow later.

## OQ-075: How are sections folded, and do both editing views share it?

**Element:** FR-027, UC-004 step 5 and 6
**Raised by:** implementation of folding, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-012

Answer: By an arrow before each heading and by keys for the section at the cursor; the facade and the source view of a
tab share the folded sections.

## OQ-076: How does the minimap look, and can it be hidden?

**Element:** FR-030, UC-004 step 4
**Raised by:** implementation of the minimap, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-004](use_cases/UC-004-read-note.md), BR-013

Answer: Bars in the shape of the lines and blocks rather than tiny text; a click jumps, dragging moves along; a switch
hides it for all tabs.

## OQ-077: What is a line of the Vim keys in the facade, and where does yanked text go?

**Element:** UC-005 A6, BR-009, FR-024
**Raised by:** implementation of the Vim input mode, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-005](use_cases/UC-005-edit-note.md), BR-009

Answer: In the facade a line is a block (paragraph, list item, heading); yanked and deleted text goes into a register
of its own, and `"+` uses the clipboard.

## OQ-078: How does the user change settings, and give a workspace a value of its own?

**Element:** UC-010, FR-070
**Raised by:** /use-case-spec UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: In a settings window with two parts, "For me" and "This workspace"; in the workspace part each setting shows whether it uses its own value, and "As for me" removes that value again.

## OQ-079: Do the quick switches of the toolbar stay next to the settings, and are they stored?

**Element:** UC-010 A3, FR-071
**Raised by:** /use-case-spec UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: They stay, and their state is stored at once like any other setting, where the setting comes from.

## OQ-080: What happens when a settings file has an error?

**Element:** UC-010 A6
**Raised by:** /use-case-spec UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: The last settings that could be read stay in effect (the built-in defaults at start), the status bar names file, line, and error, and the file is not written until it is corrected.

## OQ-081: Is the visibility of the minimap a setting that is kept?

**Element:** UC-010 BR-001, FR-030
**Raised by:** /use-case-spec UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: Yes, a setting of its own, shown by default.

## OQ-082: May a workspace settings file, often shared through git, switch the MCP server on?

**Element:** UC-010 BR-008, FR-059, NFR-016
**Raised by:** spec review UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: No; only the user settings switch it on. A workspace can only switch it off, and a workspace file that switches it on is not followed.

## OQ-083: Does "This workspace" say that its values are shared through git?

**Element:** UC-010 BR-007
**Raised by:** spec review UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: Yes, when the workspace is a git repository that does not ignore its workspace settings file.

## OQ-084: Where do line numbers apply?

**Element:** UC-010 BR-001, FR-028, FR-029
**Raised by:** spec review UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: In the source view only; the facade's lines are not lines of the file.

## OQ-085: How are the settings opened?

**Element:** UC-010 BR-006, NFR-011
**Raised by:** spec review UC-010, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-010](use_cases/UC-010-adjust-settings.md)

Answer: With Ctrl+, (Cmd+, on macOS) and a settings button in the top bar.

## OQ-086: How are the links to a renamed note updated?

**Element:** UC-006 BR-003, BR-007, FR-005
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: At once, without asking; the status bar reports how many links in how many notes were updated.

## OQ-087: Can renaming also move the note to another folder?

**Element:** UC-006 BR-001, FR-005
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: Yes; one dialog for title and folder, as for a new note, since a move needs the same link updates.

## OQ-088: Can renaming change the note format?

**Element:** UC-006 BR-002, FR-004
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: No; the note format stays. Converting a note is not renaming.

## OQ-089: Does a link text that shows the old title change?

**Element:** UC-006 BR-003
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: Yes, when it shows exactly the old title; any other link text stays.

## OQ-090: How does the user start renaming?

**Element:** UC-006 step 1, NFR-011
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: From the context menu of the note in the note overview or of its tab, and with F2 for the note in the active tab.

## OQ-091: What happens to the links of a note that moves to another folder?

**Element:** UC-006 BR-005
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: The linked notes and files stay where they are; the note's relative links are changed so they still lead to them.

## OQ-092: Does the first heading change with the title?

**Element:** UC-006 BR-004
**Raised by:** /use-case-spec UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: Yes, when it shows exactly the old title; otherwise the content stays.

## OQ-093: What happens when the Home note is renamed?

**Element:** UC-006 BR-009
**Raised by:** spec review UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: The dialog warns that the wiki then has no start page; the user may still confirm.

## OQ-094: Can a rename be undone?

**Element:** UC-006 BR-010
**Raised by:** spec review UC-006, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-006](use_cases/UC-006-rename-note.md)

Answer: No separate undo; renaming back updates the links again, and git can revert it like any other change.

## OQ-095: Can a whole folder be renamed or moved, with the links to its notes updated?

**Element:** UC-006 BR-011, FR-005
**Raised by:** spec review UC-006, 2026-11-07
**Blocking:** No
**Status:** Open

No requirement covers renaming folders. Renaming them by hand outside Underline breaks the links to their notes.

## OQ-096: Where does a deleted note go?

**Element:** UC-007 BR-004, FR-006
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: Into the deleted notes of the workspace, kept by Underline itself in the Underline folder, not the trash of the system.

## OQ-097: What does the confirmation say about links to the note?

**Element:** UC-007 BR-001, BR-002
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: It names how many links in how many notes lead to the note; the links stay unchanged.

## OQ-098: What happens to images and files only the deleted note uses?

**Element:** UC-007 BR-003
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: They stay where they are.

## OQ-099: How does the user start deleting?

**Element:** UC-007 step 1, BR-007, NFR-011
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: From the context menu of the note overview and of the tab, and with the Delete key in the note overview.

## OQ-100: How long are deleted notes kept?

**Element:** UC-007 BR-005
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: 30 days; older ones are removed when the workspace is opened, and the user can empty them earlier.

## OQ-101: How is a deleted note restored?

**Element:** UC-007 A4, A5, BR-006
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: From the list of deleted notes at the end of the note overview, and with an undo in the status bar right after deleting; it comes back at its place.

## OQ-102: Are deleted notes shared through git?

**Element:** UC-007 BR-004
**Raised by:** /use-case-spec UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: No; they are kept on this computer only, and git sees the note as deleted.

## OQ-103: What happens when the Home note is deleted?

**Element:** UC-007 BR-010
**Raised by:** spec review UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: The question warns that the wiki then has no start page; the user may still confirm.

## OQ-104: What if the deleted note cannot be kept because the Underline folder cannot be written?

**Element:** UC-007 A8
**Raised by:** spec review UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: The user is asked a second time whether to delete the note for good; otherwise nothing changes.

## OQ-105: Can several notes be deleted at once?

**Element:** UC-007 BR-011, FR-006
**Raised by:** spec review UC-007, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-007](use_cases/UC-007-delete-note.md)

Answer: No; one note at a time.

## OQ-106: Where does the user search?

**Element:** UC-008 step 1, BR-007
**Raised by:** /use-case-spec UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: In a search field above the note overview, with Ctrl+Shift+F; while it holds text, the overview shows the found notes.

## OQ-107: What does the search look at?

**Element:** UC-008 BR-001
**Raised by:** /use-case-spec UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: Titles, folders, and the text of the notes in their markup; title and folder matches come first.

## OQ-108: How are the found notes shown?

**Element:** UC-008 step 2, BR-004, A5
**Raised by:** /use-case-spec UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: Each note with its title, folder, and number of places, and the lines found with the words marked; choosing one opens the note at that place.

## OQ-109: How is the search text read?

**Element:** UC-008 BR-002
**Raised by:** /use-case-spec UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: All words must occur, in any order and case; words in double quotes are one sequence; no further options.

## OQ-110: Do accents and umlauts count in a search?

**Element:** UC-008 BR-002
**Raised by:** spec review UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: Yes; only upper and lower case do not matter.

## OQ-111: Are the words found marked in the opened note?

**Element:** UC-008 step 4, BR-008
**Raised by:** spec review UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: Yes, every occurrence in both editing views while the search field holds text.

## OQ-112: Is finding within the active note part of UC-008?

**Element:** UC-008 A6, BR-009
**Raised by:** spec review UC-008, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-008](use_cases/UC-008-find-note.md)

Answer: Yes: Ctrl+F with the same rules, Enter/F3 for the next and Shift+Enter/Shift+F3 for the previous occurrence.

## OQ-113: Where is a note exported to?

**Element:** UC-009 step 2, BR-007
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: To a place chosen in the file dialog of the system, proposing the title with the extension and the folder of the last export.

## OQ-114: What happens to content the other note format has no markup for?

**Element:** UC-009 A2, BR-002
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: The closest counterpart is written, and the report says which blocks were only approximated.

## OQ-115: How do images and diagrams get into the HTML export?

**Element:** UC-009 BR-003
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: Inside the one HTML file, so it stands alone.

## OQ-116: How is the PDF set up?

**Element:** UC-009 BR-004
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: Paper size by the region of the system, portrait, usual margins, page numbers, light look; no settings.

## OQ-117: How does the HTML export look?

**Element:** UC-009 BR-003
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: Light and plain as on GitHub, dark where the reader's browser asks for it, whatever the theme of Underline.

## OQ-118: What happens to links to other notes in HTML and PDF?

**Element:** UC-009 BR-005
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: Their text stays but they are no links; web addresses and links to headings of the note stay links.

## OQ-119: How does the user start an export?

**Element:** UC-009 step 1, BR-008
**Raised by:** /use-case-spec UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: From the context menus of the note overview and the tab, and with Ctrl+Shift+E for the active note.

## OQ-120: Can several notes or a folder be exported at once?

**Element:** UC-009 BR-010
**Raised by:** spec review UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: No; one note at a time.

## OQ-121: Is there a conversion that replaces the note by the other note format?

**Element:** UC-009 BR-010
**Raised by:** spec review UC-009, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-009](use_cases/UC-009-export-note.md)

Answer: No; the note stays unchanged, and an export into the workspace adds a new note.

## OQ-122: Should a folder or several notes be exportable as one document?

**Element:** UC-009 BR-010, FR-014 to FR-016
**Raised by:** spec review UC-009, 2026-11-07
**Blocking:** No
**Status:** Open

Open. No requirement asks for it yet.

## OQ-123: Should a note be convertible to the other note format in place, with the links following it?

**Element:** UC-009 BR-010, FR-014
**Raised by:** spec review UC-009, 2026-11-07
**Blocking:** No
**Status:** Open

Open. No requirement asks for it yet.

## OQ-124: Where does the user see which notes changed?

**Element:** UC-011 steps 2 and 4, BR-003
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: Marks in the note overview and a list of changes in the tool panel.

## OQ-125: How does the tool panel show the diff of the active note?

**Element:** UC-011 step 6, BR-004
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: As lines of the markup removed and added, with a few lines around them; choosing a line shows it in the note.

## OQ-126: Against what are the changes shown?

**Element:** UC-011 BR-001
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: Against the last commit, unsaved text included; there is no staging.

## OQ-127: Where does the current branch show?

**Element:** UC-011 step 1
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: In the status bar, with the number of changed notes and files; choosing it opens the changes.

## OQ-128: Are files that are not notes listed among the changes?

**Element:** UC-011 step 4, A5, BR-002
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: Yes, below the notes, with a diff for text files only.

## OQ-129: Can the changes of a note be discarded?

**Element:** UC-011 A6, BR-005
**Raised by:** /use-case-spec UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: Yes, after a question; a new note goes to the deleted notes.

## OQ-130: Are the changed words within a line marked?

**Element:** UC-011 BR-004
**Raised by:** spec review UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: Yes; the words removed and added are marked more strongly than the line.

## OQ-131: How does the diff treat changes of spaces or line endings only?

**Element:** UC-011 BR-004
**Raised by:** spec review UC-011, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-011](use_cases/UC-011-review-changes.md)

Answer: They are shown as git records them, with a note that only spaces or the line ending differ.

## OQ-132: What does a commit hold?

**Element:** UC-012 BR-001, BR-004
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: Every change listed, chosen by default; single notes and files can be left out and stay changed.

## OQ-133: Where is the commit message written?

**Element:** UC-012 step 2, BR-003
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: In a field above the list of changes in the tool panel, committed with a button or Ctrl+Enter.

## OQ-134: Does Underline propose a commit message?

**Element:** UC-012 BR-002
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: Yes, made from the titles of the changed notes on this computer; the user may rewrite it.

## OQ-135: Does a commit push to the remote?

**Element:** UC-012 BR-007
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: No; the next synchronization (UC-014) does.

## OQ-136: What happens when git does not know the user's name and email?

**Element:** UC-012 A3, BR-005
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: Underline asks for them and keeps them in the user's own git settings for every repository.

## OQ-137: Can the last commit be amended?

**Element:** UC-012 A6, BR-006
**Raised by:** /use-case-spec UC-012, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-012](use_cases/UC-012-commit-changes.md)

Answer: Yes, while it is not on the remote yet.

## OQ-138: Where does the user create and switch branches?

**Element:** UC-013 step 1, BR-002
**Raised by:** /use-case-spec UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: In a menu at the branch in the status bar, which also opens the changes.

## OQ-139: What happens to changes that are not committed when switching?

**Element:** UC-013 step 4, A3, BR-003
**Raised by:** /use-case-spec UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: They go along when they can, as git carries them; otherwise the switch is refused, offering to commit first.

## OQ-140: Which branches does the list show?

**Element:** UC-013 BR-001, A4
**Raised by:** /use-case-spec UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: Local branches first, then branches of the remote without a local one; choosing such a branch creates a local one following it.

## OQ-141: Are deleting and renaming branches part of UC-013?

**Element:** UC-013 BR-007
**Raised by:** /use-case-spec UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: No.

## OQ-142: Should Underline delete and rename branches?

**Element:** UC-013 BR-007, FR-050
**Raised by:** /use-case-spec UC-013, 2026-11-07
**Blocking:** No
**Status:** Open

Open. No requirement asks for it yet; git does it.

## OQ-143: Can a new branch start from another branch than the current one?

**Element:** UC-013 BR-008
**Raised by:** spec review UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: No; it always starts from the current commit with the current changes.

## OQ-144: Does a new branch go to the remote?

**Element:** UC-013 A1, BR-009
**Raised by:** spec review UC-013, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-013](use_cases/UC-013-manage-branches.md)

Answer: Yes, with the next synchronization, which then follows it.

## OQ-145: Are changes that are not committed committed before a sync?

**Element:** UC-014 step 2, BR-002
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: Yes, with the message UC-012 proposes; changes the user left out stay out.

## OQ-146: How are the remote's changes joined with local commits?

**Element:** UC-014 BR-003
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: As the user's git setup says, by merging by default.

## OQ-147: How are pull and push offered on demand?

**Element:** UC-014 step 1, A1
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: "Sync now" in the status bar, and Pull and Push in the branch menu.

## OQ-148: What does the status bar show of the sync?

**Element:** UC-014 BR-004
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: Syncing, the time of the last sync, offline or failed with the reason, and commits waiting to be pushed or pulled.

## OQ-149: What happens when the remote asks to sign in and git has no sign-in?

**Element:** UC-014 A5, BR-005
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: The status bar says so and how to set up a sign-in in git; Underline asks for no password.

## OQ-150: When does Underline sync besides the interval?

**Element:** UC-014 BR-001
**Raised by:** /use-case-spec UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: When the workspace opens, after every commit, and when Underline closes.

## OQ-151: Are automatic commits before a sync marked as such?

**Element:** UC-014 step 2, BR-002
**Raised by:** spec review UC-014, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-014](use_cases/UC-014-synchronize-with-remote.md)

Answer: Yes; their message starts with `Auto: `.

## OQ-152: How are the two versions of a note in conflict shown?

**Element:** UC-015 step 3, BR-002
**Raised by:** /use-case-spec UC-015, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-015](use_cases/UC-015-resolve-sync-conflict.md)

Answer: Each conflicting place as a block of Here and Remote side by side in the note's tab; the rest of the note is joined already.

## OQ-153: How is a conflicting place resolved?

**Element:** UC-015 step 4, A1
**Raised by:** /use-case-spec UC-015, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-015](use_cases/UC-015-resolve-sync-conflict.md)

Answer: Here, Remote, Both, or edited freely; or one side for the whole note.

## OQ-154: Must a conflict be resolved at once?

**Element:** UC-015 A4, A5
**Raised by:** /use-case-spec UC-015, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-015](use_cases/UC-015-resolve-sync-conflict.md)

Answer: No; it can wait, while commits, branch switches, and syncs wait too; the sync can be aborted.

## OQ-155: What happens with files that are not notes in a conflict?

**Element:** UC-015 A2
**Raised by:** /use-case-spec UC-015, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-015](use_cases/UC-015-resolve-sync-conflict.md)

Answer: One version is chosen for the whole file; the other is kept as a copy next to it.

## OQ-156: What may an agent do with a note in conflict?

**Element:** UC-015 BR-007, UC-019
**Raised by:** spec review UC-015, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-019](use_cases/UC-019-write-note-as-agent.md)

An agent writing a note through the MCP server (UC-019) could write into a note with open conflict places. To be decided with UC-019: refuse, read only, or allow.

Answer: An agent's change is refused until the user has resolved the conflicts; reading goes on.

## OQ-157: Which workspace does an installed MCP server serve?

**Element:** UC-016 BR-001
**Raised by:** /use-case-spec UC-016, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-016](use_cases/UC-016-install-mcp-server.md)

Answer: One entry per workspace, with that workspace's folder.

## OQ-158: Where does MCP install write the configuration?

**Element:** UC-016 BR-002
**Raised by:** /use-case-spec UC-016, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-016](use_cases/UC-016-install-mcp-server.md)

Answer: Into the tool's configuration for the user, never into the workspace.

## OQ-159: Where are MCP install and the configuration snippet offered?

**Element:** UC-016 step 1
**Raised by:** /use-case-spec UC-016, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-016](use_cases/UC-016-install-mcp-server.md)

Answer: In the section Agents of the settings.

## OQ-160: What happens on install while the MCP server is off?

**Element:** UC-016 A1, BR-005
**Raised by:** /use-case-spec UC-016, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-016](use_cases/UC-016-install-mcp-server.md)

Answer: Install asks whether to switch it on.

## OQ-161: Does Underline offer MCP install on its own?

**Element:** UC-016 BR-007
**Raised by:** spec review UC-016, 2026-11-07
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-016](use_cases/UC-016-install-mcp-server.md)

Answer: No; only in the section Agents of the settings.

## OQ-162: What does an agent experience while the MCP server is switched off?

**Element:** UC-017 step 7, A2, BR-001
**Raised by:** /use-case-spec UC-017, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: The server answers but refuses every request about notes, giving the reason.

## OQ-163: Where can agent access be switched besides the settings?

**Element:** UC-017 step 2, BR-002
**Raised by:** /use-case-spec UC-017, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: With an entry in the status bar, which changes the user setting.

## OQ-164: Does Underline show whether an agent is connected?

**Element:** UC-017 step 5, A3, A4, BR-004
**Raised by:** /use-case-spec UC-017, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: Yes, with the names the tools give when they connect.

## OQ-165: May an agent reach the notes while the Underline window is closed?

**Element:** UC-017 A5, BR-007
**Raised by:** spec review UC-017, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: Yes, as the stored setting says.

## OQ-166: Is there a read-only level of agent access?

**Element:** UC-017 A6, BR-002, FR-059
**Raised by:** spec review UC-017, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: Yes: off, read only, read and write; FR-059, UC-010, and the entity model follow it.

## OQ-167: What can an agent look up besides listing and reading notes?

**Element:** UC-018 steps 3–4, A4, A5, BR-004
**Raised by:** /use-case-spec UC-018, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-018](use_cases/UC-018-look-up-notes.md)

Answer: It can also search the notes, by the rules of the search of UC-008.

## OQ-168: What does an agent get when it reads a note?

**Element:** UC-018 step 6, A1
**Raised by:** /use-case-spec UC-018, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-018](use_cases/UC-018-look-up-notes.md)

Answer: The text in its note format, with the format and the number of lines; on request only a range of lines.

## OQ-169: Does an agent read changes the user has not saved yet?

**Element:** UC-018 BR-003
**Raised by:** /use-case-spec UC-018, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-018](use_cases/UC-018-look-up-notes.md)

Answer: No, it reads the notes as stored; unsaved changes are saved within a second.

## OQ-170: Does the user see where an agent reads?

**Element:** UC-018 step 7, BR-006, FR-057
**Raised by:** /use-case-spec UC-018, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-018](use_cases/UC-018-look-up-notes.md)

Answer: Yes, the agent cursor marks the lines read in an open note, as for writing.

## OQ-171: Does the agent cursor show while an agent reads at read only?

**Element:** UC-018 BR-006, UC-017 BR-005, FR-058
**Raised by:** spec review UC-018, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-018](use_cases/UC-018-look-up-notes.md), [UC-017](use_cases/UC-017-control-agent-access.md)

Answer: Yes; it shows at every level that allows what the agent does, and is hidden only while access is off or no agent is connected.

## OQ-172: How does an agent change a note?

**Element:** UC-019 step 1, A1, BR-003
**Raised by:** /use-case-spec UC-019, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-019](use_cases/UC-019-write-note-as-agent.md)

Answer: It creates a note, replaces a passage it quotes exactly, inserts text before a line or at the end, or replaces the whole text.

## OQ-173: What happens to an agent's change while the user is typing in the note?

**Element:** UC-019 A5, A6, BR-004
**Raised by:** /use-case-spec UC-019, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-019](use_cases/UC-019-write-note-as-agent.md)

Answer: It is merged into the tab; when it touches a line the user changed and has not saved, it is refused.

## OQ-174: Can the user undo an agent's change?

**Element:** UC-019 A11, BR-007
**Raised by:** /use-case-spec UC-019, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-019](use_cases/UC-019-write-note-as-agent.md)

Answer: Yes, in the open tab, each change of an agent as one step.

## OQ-175: Does Underline report an agent's change to a note that is not open?

**Element:** UC-019 A1, BR-010
**Raised by:** spec review UC-019, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-019](use_cases/UC-019-write-note-as-agent.md)

Answer: Yes, briefly in the status bar, with the agent's name, the note, and an offer to open it.

## OQ-176: Where are the todos of a workspace kept?

**Element:** UC-020 BR-004
**Raised by:** /use-case-spec UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: In the workspace, shared through git like the notes.

## OQ-177: Where does the user manage todos?

**Element:** UC-020 steps 1–2
**Raised by:** /use-case-spec UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: In the section "Todos" of the tool panel.

## OQ-178: Can a todo be deleted?

**Element:** UC-020 A5
**Raised by:** /use-case-spec UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: Yes, with "Undo" offered in the status bar; done todos are otherwise kept and can be reopened.

## OQ-179: May agents work with todos?

**Element:** UC-020 A8
**Raised by:** /use-case-spec UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: Yes, as agent access allows: reading at read only, also changing at read and write.

## OQ-180: What happens when the same todo was changed on two computers?

**Element:** UC-020 A10, BR-008
**Raised by:** spec review UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: It is a conflict the user resolves as for notes (UC-015), choosing for each todo which version holds; todos changed on one side only are joined without asking.

## OQ-181: What happens when two computers give a new todo the same number?

**Element:** UC-020 A11, BR-002
**Raised by:** spec review UC-020, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-020](use_cases/UC-020-manage-todos.md)

Answer: The todo shared first keeps it; the one created here gets the next free number, and the status bar says so.

## OQ-182: Where does the user work through the todo list?

**Element:** UC-021 steps 1–2, BR-006
**Raised by:** /use-case-spec UC-021, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-021](use_cases/UC-021-work-through-todo-list.md)

Answer: In a tab "Todos" of its own, a table of the todos, in addition to the section in the tool panel.

## OQ-183: How are todos sorted by due date or urgency?

**Element:** UC-021 BR-001, BR-002, BR-005
**Raised by:** /use-case-spec UC-021, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-021](use_cases/UC-021-work-through-todo-list.md)

Answer: By the one, then the other; the choice is kept for the workspace.

## OQ-184: Which filters does the todo list offer?

**Element:** UC-021 A1, BR-003
**Raised by:** /use-case-spec UC-021, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-021](use_cases/UC-021-work-through-todo-list.md)

Answer: Open, Done, and All, as FR-067 says.

## OQ-185: Are overdue todos shown?

**Element:** UC-021 A2, BR-004
**Raised by:** /use-case-spec UC-021, 2026-11-09
**Blocking:** No
**Status:** Resolved
**Resolved in:** [UC-021](use_cases/UC-021-work-through-todo-list.md)

Answer: Yes, marked in the list, and counted in the status bar, which opens the list.

## OQ-186: What happens to several empty lines the user enters between two paragraphs in the facade?

**Element:** UC-005 step 1, BR-002, BR-003
**Raised by:** Implementation of UC-005, 2026-11-09
**Blocking:** No
**Status:** Open
**Assumed:** Each Enter opens a visible empty line, so the cursor moves down at once; once the user types text there,
the empty lines in between are shown as one paragraph gap, as the note format renders them, and stay in the markup.

Readings: (a) as assumed, the facade shows what the note format renders; (b) the facade keeps every empty line
visible, unlike the rendered note; (c) the facade stores only one empty line between paragraphs and drops the others.

## OQ-187: Does the editor show where the cursor is and how much is selected?

**Element:** UC-005 step 1, UC-010 A3
**Raised by:** Implementation of UC-005, 2026-11-09
**Blocking:** No
**Status:** Open
**Assumed:** Yes, as the user asked: a status line under the editor shows the line and column of the cursor in the
markup and the number of selected characters, or of selected lines when the selection spans several lines; the quick
switches for the input mode, the minimap, and copy on select sit at its right end, in both editing views.

UC-005 and UC-010 should take this in: UC-010 A3 and TC-010-06 still place the quick switches in the toolbar. In the
facade the line and column count in the markup, not in the rendered text.

With Vim or Nano keys the same line also shows, at its left, the key map and its mode in a color of the theme, what is
typed but not done yet, and the most important commands as far as they fit; UC-005 A6, A7, BR-009, BR-010 and FR-026
still name the status bar of the window for this.

## OQ-188: Where are the commands of the window, and can the user hide them?

**Element:** UC-010 BR-001, BR-006, A3; UC-001; UC-002
**Raised by:** Implementation, 2026-11-09
**Blocking:** No
**Status:** Open
**Assumed:** As the user asked: a menu bar at the top of the window holds the menus File (new note, open workspace,
new workspace), Workspace (wiki project address), and Settings. A burger button at its left end shows or hides it;
hidden, the name of the workspace stands in its place. It starts hidden, and the switch is stored like a quick switch
(setting "Menu bar").

UC-010 should list the setting in BR-001 and name the burger button as a quick switch; UC-001 and UC-002 should name
the File menu as the place to open and create a workspace.

## OQ-189: Which searches mark their hits, where, and how does a search end?

**Element:** UC-008 steps 1–2, A2, BR-007, BR-008; UC-005 BR-009, BR-010; FR-030; UC-010 BR-001, BR-004
**Raised by:** Implementation, 2026-11-09
**Blocking:** No
**Status:** Open
**Assumed:** As the user asked:
- The search of the workspace is an overlay over the window, a field with the found notes and lines under it, opened
  with a magnifier in the top bar or Ctrl+Shift+F; the note overview no longer holds the search field. A click opens a
  place and keeps the overlay; Enter opens the chosen place and closes it; Escape or a click outside closes it, which
  ends the search.
- The hits of every search are marked in the note, in both editing views, and in the minimap: the search of the
  workspace, the find field of the note, and the last search of the Vim keys (`/`) or the Nano keys (`Ctrl+W`). Escape
  in Vim's normal mode or `:noh` ends the Vim search, Escape ends the Nano search.
- The minimap works like a sieve while there are hits: the note in it is veiled and only the found words shine through,
  at their place in the line, in the theme's color for hits. They blink; the setting "Search hits in the minimap" shows
  them steadily instead. It starts blinking.
- Hits are marked in a color of the theme, not one color for all themes.
- The menu bar has a menu "Theme" with every theme.

UC-008 should take in the overlay and the other searches; UC-010 BR-001 should list the new setting.

## OQ-190: Does replacing in the facade also change markup and link addresses?

**Element:** UC-005 BR-017
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Only the text the facade shows; markup and link addresses are replaced only in the source view.

Readings: (a) as assumed; (b) the facade replaces in the markup too and shows each hidden place in a frame; (c) replacing is offered only in the source view.

## OQ-191: How long can a replacement in the workspace be undone at once?

**Element:** UC-022 A6
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Until another status bar report replaces the offer, as for deleting (UC-007 A4); afterwards through the snapshots (UC-026).

Readings: (a) as assumed; (b) for a fixed time such as 30 seconds; (c) until Underline is closed, through an undo command of its own.

Implemented as assumed (2026-11-13, package 8a): since the snapshots exist, every note a replacement changes keeps
its content from before as a snapshot with the cause Replace in workspace (UC-022 BR-005), so it can be restored
after the offer is gone. What the undo itself replaces is OQ-289.

## OQ-192: Does the replace match the search text as one text or as words in any order?

**Element:** UC-022 BR-002
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** As one text, spaces included; the words-in-any-order rule of the search (UC-008 BR-002) does not apply to replacing, also not in the find field of the note.

A replacement needs one place to put the new text at, so words in any order cannot be replaced. The search field shows the same text with another meaning when the user switches from finding to replacing.

## OQ-193: Which notation of regular expressions do the searches accept?

**Element:** Glossary: Regular expression
**Raised by:** /requirements, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The usual notation of programming editors (character classes, groups, quantifiers, anchors, `\d`, `\w`), with `$1` to `$9` and `$0` in the replacement, as VS Code accepts it; no look-behind.

The notation is visible to the user and decides which searches work the same on every platform (NFR-013).

## OQ-194: Are the search switches kept per field and remembered?

**Element:** UC-008 BR-011
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The search field of the workspace and the find field of the note each keep their own switches until Underline is closed; nothing is remembered after a restart (BR-010).

Readings: (a) as assumed; (b) one set of switches for every search; (c) remembered in the user settings.

## OQ-195: Which keys open replacing, and how do they meet the Vim and Nano keys?

**Element:** UC-022 BR-008
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Ctrl+H (Cmd+Alt+F on macOS) replaces in the note and Ctrl+Shift+H (Cmd+Shift+H) in the workspace, as in VS Code; where the input mode gives the key a meaning (Ctrl+H is Backspace in the Vim insert mode), the input mode keeps it and the command palette reaches the replace.

Alt+C, Alt+W, and Alt+R switch the search switches; on macOS Option with a letter types a character in some keyboard layouts.

## OQ-196: Does go to note open with Ctrl+P or Ctrl+O, and does it match words or scattered letters?

**Element:** UC-004 BR-014
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Ctrl+P (Cmd+P on macOS), as in VS Code; every typed word occurs in the title or the folder, also as part of a word, as in the search of the workspace.

Obsidian uses Ctrl+O; VS Code matches scattered letters in order (`plnts` finds Plane Tickets). Ctrl+P is printing in many programs; Underline prints through the PDF export.

## OQ-197: Does the command palette show commands that are not available now?

**Element:** UC-023 A2
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Yes, after the others, marked as not available; choosing one says why and runs nothing.

Readings: (a) as assumed, so the user learns the command exists; (b) hidden, as VS Code does.

## OQ-198: Are the recently run commands remembered after a restart?

**Element:** UC-023 BR-004
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No; they are kept until Underline is closed.

Readings: (a) as assumed; (b) remembered for the user across restarts.

## OQ-199: Do the backlinks also list unlinked mentions of the note's title?

**Element:** UC-004 BR-015
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No; only real local links count. Unlinked mentions are not part of this release.
Implemented as assumed (2026-11-12, package 3): the backlinks list only local links.

Obsidian lists notes that name the title without a link and offers to link them. The research comparison lists it as missing.

## OQ-200: Does link completion start with `[[` in the facade?

**Element:** UC-005 A23
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Yes: `[[` only starts the completion and is replaced by a normal link of the note format; Escape keeps `[[` as typed text. In the source view it starts after `](`, `xref:`, or `<<`, and with Ctrl+Space.
Implemented as assumed (2026-11-12, package 3); Ctrl+Space in the facade, where no place of a link shows, see
OQ-247.

C-003 rules out wiki links in the stored note; the question is only whether `[[` may serve as a key to start the completion.

## OQ-201: Does a completed link name the note with or without its extension?

**Element:** UC-005 BR-018
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** As the formatting toolbar writes a link to a note (UC-005 A2); the specification of the toolbar does not say it either.
Implemented as assumed (2026-11-12, package 3): always with the extension, as the toolbar writes it (reading (a)
in effect); a place with spaces, see OQ-248.

GitHub repositories need the extension, GitHub wikis do not (UC-004 BR-005 accepts both). Readings: (a) always with the extension; (b) without it in a workspace that is a GitHub wiki; (c) as the most links of the workspace do.

## OQ-202: Which characters does close pairs close, and can it be switched off?

**Element:** UC-005 BR-019
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Brackets, double quotes, and backticks always; `**` and `~~` within a line; a single `*`, `_` (and `#` in AsciiDoc) only around a selection; single quotes never, because of apostrophes. A setting "Close pairs" switches it, on at first.
Implemented as assumed (2026-11-12, package 4), with the details of OQ-252.

Readings for the markup: (a) as assumed; (b) every markup character closes; (c) markup never closes, only brackets and quotes.

## OQ-203: Which keys move and duplicate lines?

**Element:** UC-005 BR-020
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Alt+Up and Alt+Down move; Ctrl+Shift+D (Cmd+Shift+D on macOS) duplicates.
Implemented as assumed (2026-11-12, package 4); in the Vim keys' normal mode the keys stay Vim's. The command palette
has "Move line up", "Move line down", and "Duplicate line" in every input mode. Which block the facade moves past, see
OQ-255.

VS Code duplicates with Shift+Alt+Down; GNOME Text Editor and Obsidian differ too. Ctrl+Shift+J opens the daily note (UC-025, OQ-219).

## OQ-204: What do Tab and Shift+Tab do outside a list item?

**Element:** UC-005 BR-021
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** They keep the meaning they have today; the specification does not state it yet.
Implemented as assumed (2026-11-12, package 4): in the source view Tab writes four spaces, as before; Shift+Tab takes
up to one tab stop of space from the start of the lines it touches (OQ-256); in the facade both do nothing (M-09.13).
Inside a list item that cannot go deeper or higher nothing happens either.

Readings: (a) Tab inserts indentation in the source view and does nothing in the facade; (b) Tab moves the focus out of the editor for keyboard users (NFR-011); (c) Tab indents the paragraph.

## OQ-205: Are the current line and the matching bracket highlighted in the facade too, and can they be switched off?

**Element:** UC-005 BR-022
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Only in the source view, always on, with no setting.
Implemented as assumed (2026-11-12, package 4): the colours are mixed from the active theme's palette (the current
line a shade of the text colour, a matching pair outlined in the link colour, a bracket without a counterpart
underlined with a wave in the colour of found places), so the mark is told by its shape too.

In the facade a line of the markup is not a line on screen; the current block could be highlighted instead.

## OQ-206: Which languages does the spell check offer, and which is the default?

**Element:** FR-100
**Raised by:** /requirements, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** English (US), English (UK), German (Germany), German (Switzerland), French, Italian, and Spanish; the default is the language of the system when it is one of these, otherwise off.

Each language needs a dictionary that comes with Underline (NFR-014), which makes the installation larger.

Implemented in part (2026-11-13, package 8b): offered are Off, English (US), English (UK), French and Spanish, the
languages whose dictionaries are licensed permissively (ADR-030). German (Germany), German (Switzerland) and Italian
wait for the decision of OQ-299; until then they are not offered. The default is the language of the system when it
is one of the offered languages, read as OQ-298 says, otherwise off.

## OQ-207: Can a note choose its own spell check language?

**Element:** UC-005 BR-023
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No; the language comes from the settings of the user or the workspace.

Readings: (a) as assumed; (b) a note attribute `lang` sets the language of one note; (c) several languages checked at once.

Implemented as assumed (2026-11-13, package 8b): the language comes from the settings of the workspace, else of the
user, else the system (OQ-206).

## OQ-208: Is there also a dictionary of the workspace, shared through git?

**Element:** Entity: DICTIONARY_WORD
**Raised by:** /entity-model, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No; only the user dictionary, which counts in every workspace and every language.

A word list of the workspace would let a team share project words; it would be a file in the workspace.

Implemented as assumed (2026-11-13, package 8b): only the user dictionary, a file next to the user settings file
(ADR-030). A dictionary of the workspace is not implemented; whether there is one is left to the user. It would change
FR-099 and UC-005 A28 (where an added word goes), and would be the same kind of file in the workspace, shared through
git.

## OQ-209: Which key switches the focus mode, and does it stay on after a restart?

**Element:** UC-010 A12
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Ctrl+Alt+F (Cmd+Ctrl+F on macOS); it is a setting like any other and stays on after a restart. It also hides the menu bar of OQ-188.

VS Code uses Ctrl+K Z for its zen mode and leaves it on a restart; Cmd+Ctrl+F is full screen on macOS.

## OQ-210: Does the editor zoom change images and diagrams, and is it a setting of the workspace too?

**Element:** UC-010 BR-009
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Only text, code, and formulas grow; images and diagrams keep their size. It is stored like a quick switch, so a workspace may hold its own value.

Readings: (a) as assumed; (b) everything in the note grows; (c) the whole window grows, as the zoom of a browser.

## OQ-211: What does soft wrap change in the facade?

**Element:** UC-010 BR-011
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Paragraphs, lists, and headings always wrap in the facade; tables, code blocks, and frames of markup follow soft wrap; on at first.

Today every line wraps (research comparison).

## OQ-212: What does the text statistics count?

**Element:** UC-004 BR-020
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The text the facade shows, code blocks included, without markup, front matter, link addresses, and the sources of diagrams and formulas; reading time at 200 words a minute, rounded up.

Readings for code: (a) counted, as assumed; (b) left out, as prose only.

## OQ-213: How are tags written in an AsciiDoc note?

**Element:** UC-004 BR-016
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Only in the document attribute `tags`, written `:tags: a, b`; `#word` in the text is no tag, because `#` marks highlighted text in AsciiDoc.

Readings: (a) as assumed; (b) `#word` in the text too, where it does not form highlighted text; (c) a role such as `[.tag]#word#`.

## OQ-214: How is a tag shown when the notes write it in different cases?

**Element:** Entity: TAG.name
**Raised by:** /entity-model, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Tags that differ only in case are one tag, shown as most notes write it.

Readings: (a) as assumed; (b) in lower case; (c) as the first note found writes it.

## OQ-215: Which kinds of values do note attributes have, and can nested values be changed?

**Element:** UC-024 BR-002
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Text, List, Number, Date, Yes or no, and Nested; nested values are shown as markup and changed only in the source view. AsciiDoc holds only Text and List.

Obsidian offers the same kinds without Nested and refuses notes with nested values.

## OQ-216: How are note attributes shown in the facade?

**Element:** UC-004 BR-004
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The front matter of a Markdown note is shown as a table at the start of the note, as GitHub shows it; document attributes of an AsciiDoc note are not shown in the facade, as on GitHub, only in the note properties.

Today the facade shows the front matter as text (research comparison).

## OQ-217: Are templates ordinary notes in every other respect?

**Element:** UC-003 BR-008
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Yes: they are listed, searched, linked, carry tags, and count as backlinks like any note; only their placeholders stay unfilled. The folder is `templates` unless the workspace settings name another.
Implemented as assumed (2026-11-13, package 6b): a template is a listed note in the template folder or below it, so a
missing, hidden or outside folder holds none (OQ-274); the dialog "New note" offers them by title, a deeper one with
its folders after it ("Retro (work)").

Readings: (a) as assumed; (b) the template folder is left out of the note overview, the search, the backlinks, and the tags. The placeholders are `{{title}}`, `{{date}}`, `{{time}}`, and `{{cursor}}`; others such as `{{yesterday}}` or a date format are not offered.

## OQ-218: Can the user choose the date format of the daily note title?

**Element:** UC-025 BR-001
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No; the title is always `2026-11-11`.
Implemented as assumed (2026-11-13, package 6b), with the local date of the computer; how the overview shows the
title, see OQ-272.

Obsidian lets the user choose a format and folders by month.

## OQ-219: Which keys open the daily note and the previous and next one?

**Element:** UC-025 BR-007
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Ctrl+Alt+D, Ctrl+Alt+Left, and Ctrl+Alt+Right (Cmd+Alt on macOS).
Not as assumed (2026-11-13, package 6b review), for the user to confirm: Ctrl+Alt+Left and Ctrl+Alt+Right switch
workspaces on many Linux desktops, Ctrl+Alt+D shows the desktop in XFCE, Cinnamon and Unity, Cmd+Option+D hides the
Dock on macOS, and Ctrl+Alt is AltGr on Windows, so on many systems the keys would never reach Underline or would type
characters. Implemented instead: Ctrl+Shift+J (Cmd+Shift+J) opens the daily note of today, Ctrl+Shift+PageUp and
Ctrl+Shift+PageDown (Cmd+Shift) the previous and the next one. They are free among the keys of the window and of the
editors (in the source view Shift+PageUp selects a page, with Ctrl it does nothing; the Vim and Nano keys have no
Ctrl+Shift+J), and the window takes them wherever the keyboard is, but not over a dialog or the palette. UC-025 BR-007
and TC-025-01, TC-025-04 and TC-025-11 name the new keys. The palette has "Daily note", "Previous daily note", "Next
daily note" and "New note from template…"; the top bar has a button for the daily note (OQ-276).

Readings: (a) as implemented; (b) the keys of the spec, Ctrl+Alt+D and Ctrl+Alt+arrows; (c) other keys, such as
Alt+Shift+Left and Right (which select by syntax in the source view and by word on macOS).

Ctrl+Alt with the arrow keys switches desktops on some Linux desktops, and Cmd+Alt+D hides the Dock on macOS.

## OQ-220: How does the split view divide the window, and is its width remembered?

**Element:** UC-004 BR-018
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Two panes side by side, no more and not one above the other; they start equally wide; the width the user drags is not remembered after a restart, the panes and their tabs are.
Implemented as assumed (2026-11-13, package 7a): each pane keeps at least 280 px (and 15 percent) of the width; the
border is dragged with the pointer or, with the keyboard on it, moved with Left and Right (Shift for larger steps), Home
and End, and a double click or Enter makes both equally wide again. What happens in a window too narrow for two panes,
how the active pane shows and where the tool panel stands, see OQ-278.

VS Code allows any number of groups in any direction.

## OQ-221: Do two tabs of the same note share their undo history?

**Element:** UC-005 BR-016
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Yes; undo in either takes back the last change of the note.
Implemented as assumed (2026-11-13, package 7a): typing on one side after typing on the other starts a new undo step,
so undo on either side takes back the other side's last typing first (TC-005-41 step 2). Where the second tab's cursor
starts, see OQ-280.

UC-005 BR-004 gives each tab its own history; with one note in two tabs that would let undo in one tab take back text the other tab typed later.

## OQ-222: How are formulas of AsciiDoc notes written and shown?

**Element:** UC-004 BR-019
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** `latexmath:[...]`, `[latexmath]` blocks, and `stem` when the note sets `:stem: latexmath` are typeset; AsciiMath (`:stem:` without a value) is shown as its source.

Implemented as assumed (2026-11-13, package 7b): `latexmath:[…]` is a formula in every AsciiDoc note, `stem:[…]` and a
`[stem]` block only while the header (the lines from the start up to the first blank line) sets `:stem: latexmath`; a
`[latexmath]` or such `[stem]` block is one delimited by `++++`. `:stem:` without a value, `:stem: asciimath`, and a
`stem` set below the header leave `stem:[…]` and `[stem]` blocks as their markup, as Asciidoctor reads `stem` only in
the header.

GitHub does not typeset formulas of AsciiDoc files, so showing them typeset departs from C-004; Markdown follows GitHub (C-003).

## OQ-223: How often are snapshots taken while editing?

**Element:** UC-026 BR-001
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Before the first change after the newest snapshot is older than 10 minutes, of the content before that change; a change from outside is kept only for notes open in a tab, because only their content is known.

Readings: (a) as assumed; (b) at every autosave, thinned out over time; (c) only when the tab is closed.

Implemented as assumed (2026-11-13, package 8a): the Editing snapshot is taken when the change is stored (the
autosave, storing at once, or closing the tab), of the content the note held before; see OQ-290. Changes from
another program are kept only for notes open in a tab; git's changes are kept for every note they touch (OQ-293).

## OQ-224: How long and how many snapshots are kept?

**Element:** UC-026 BR-002
**Raised by:** /use-case-spec, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** 30 days, as the deleted notes; no limit of number or size, and no setting.

A note edited every day for 30 days may get several hundred snapshots; a size limit per workspace could be added.

Implemented as assumed (2026-11-13, package 8a): a snapshot older than 30 days is no longer listed and is removed
when the workspace is opened; there is no limit of number or size and no setting, and the space the snapshots take
is not shown anywhere (OQ-294).

## OQ-225: Are the thresholds of the replace in the workspace right?

**Element:** NFR-017
**Raised by:** /requirements, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Places shown within 2 seconds for 95% of search texts in 10,000 notes; replacing in 1,000 notes within 10 seconds.

## OQ-226: Is 200 milliseconds the right threshold for suggestions?

**Element:** NFR-018
**Raised by:** /requirements, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Go to note, the command palette, and link completion follow each key within 200 milliseconds in 10,000 notes.
Measured for link completion (2026-11-12, package 3): about 1 to 2 milliseconds a key in 10,000 notes, 32 for the
first key, which takes in the notes.

## OQ-227: Is 2 seconds the right threshold for backlinks and tags?

**Element:** NFR-019
**Raised by:** /requirements, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Backlinks, outgoing links, and tags reflect a stored change within 2 seconds in 10,000 notes.
Measured for the backlinks and outgoing links (2026-11-12, package 3, see the report of the package): a change of
one note of 10,000 shows well within 2 seconds once all notes were read; the first lists of a workspace of 10,000
notes come after every note was read once.

## OQ-228: Which texts do finding and replacing show?

**Element:** UC-005 A20–A22, UC-008 A7, A9, A10, UC-022 steps 2–6, A1, A2, A5–A7
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** As the rest of the window words its texts:
- Find field of the note: "Replace with:" under "Find in note:", the buttons "Replace" and "Replace all", and the
  button "Replace in this note" (a chevron) before the label; while it replaces, "No occurrence" when the note holds none (while it only
  finds, "0 found" as before) and "{n} replaced" after "Replace all".
- Search switches: "Match case", "Whole word", "Regular expression", shown as "Aa", "ab" and ".*", each with its key
  ("Alt+C", "Alt+W", "Alt+R").
- What is wrong: "Not a valid regular expression: {reason}." and "The replacement names ${n}, but the expression has
  no group {n}.", in the find field and in the overlay.
- Replace in the workspace: the chevron "Replace in notes" before the search field, the field "Replace with", the
  button "Replace {k} places in {m} notes" ("Replace" while nothing is chosen, "Replacing…" while it runs), "{n}
  places in {m} notes" and "{k} of {n} places chosen", per note "Replace in this note", the key legend "Space: leave
  out · Ctrl+Enter: replace in this note" (Cmd+Enter on macOS) under the list, the places named "line {n}: '{old}'
  becomes '{new}'", "Type what to replace, then what replaces it.", "No note holds “{text}”.
  There is nothing to replace.", and for many places "The first 10,000 places are shown; {n} more are not shown and
  are replaced too."
- Status bar: "Replaced {n} places in {m} notes" with "Undo"; "…; {title} was changed meanwhile and is left as it
  is"; "…; {title} could not be changed: {reason}"; "Nothing was replaced: every place is left out"; when no note could be changed, "Nothing was replaced" followed by the reasons ("; {title} could not be changed: {reason}"); "The places
  changed while you looked at them; look at them again before you replace"; after the undo "Took back the
  replacement in {m} notes", with "…; {title} was changed since and is left as it is".

## OQ-229: Where does the replace of the workspace show, and how does it close?

**Element:** UC-022 step 1, A8, BR-008; UC-008 A10
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** In the search overlay of the workspace (OQ-189): "Replace in notes" before the search field, or Ctrl+Shift+H
(Cmd+Shift+H), adds the field "Replace with" and shows the places instead of the found notes; the search text and the
search switches stay the same. Escape or "Replace in notes" again closes the replace, and the overlay shows the search of
the workspace with its text (A8); a second Escape closes the overlay. With a search text already typed, Ctrl+Shift+H
puts the keyboard into the field "Replace with". While it replaces, a click outside the overlay or Tab out of it does
not close it, so the replacement and the places left out are not lost. Tab goes from the search field to the field
"Replace with", then to the button, the list, and the search switches.

Readings: (a) as assumed; (b) a panel or a tab of its own beside the note; (c) the search field itself turns into the
replace without a second field.

## OQ-230: What do `$$` and a `$` before another character write in a replacement?

**Element:** UC-022 BR-003
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** With regular expression, `$$` writes one `$`, and a `$` followed by anything but a digit or `$` is written
as typed, so `${1}` and `$name` are written as they stand; `$10` is group 1 followed by `0`. A group that took no part
in the place found (as `b` in `(a)|(b)` when `a` was found) writes nothing. Without regular expression, the
replacement is written exactly as typed.

## OQ-231: Is an occurrence that runs across markup found when replacing in the facade?

**Element:** UC-005 BR-017
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** No: in the facade, replacing finds an occurrence only where its text stands in the note as the facade shows
it, without markup between its characters; "the **pl**an" is not found as "the plan", and a character written with an
escape is not found either. The source view finds and replaces them in the markup.

Readings: (a) as assumed; (b) such an occurrence is found and its markup is removed by the replacement; (c) it is found
and the replacement keeps the markup around its first part.

## OQ-232: What happens to a note that changed between showing its places and replacing?

**Element:** UC-022 step 5, A4, BR-004
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** When the user replaces before the list shows the note's new places, the note is replaced only when it
still holds exactly the places shown; otherwise it is left as it is and named in the status bar ("was changed
meanwhile"), and its places are found again. A note with places that are not shown (A7) counts as changed as soon as
its text changed.

## OQ-233: How do whole word and a regular expression work together, and does an expression that matches nothing find a place?

**Element:** UC-008 BR-011, UC-022 BR-002
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** With both on, a place the expression finds counts only where it is not part of a longer word, by the
same rule as for a plain text (letters and digits belong to a word). An expression that matches an empty text there,
such as `^`, `x*` or `(|b)`, finds no place at that spot, so nothing is inserted between characters; where it can
match more, as `(|b)` before a `b`, the search tries the next character and does not find the longer place there
either.

## OQ-234: Do the search and the replace of the workspace share their search switches?

**Element:** UC-022 step 1, UC-008 BR-010, BR-011
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Yes: the search of the workspace and its replace are one field with one set of switches, so the switches
stay as they were when the replace opens or closes. The find field of the note keeps its own switches and its own
replacement until Underline closes (OQ-194).

## OQ-235: Which keys move through the places of the replace and leave one out?

**Element:** UC-022 BR-008
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Down in either field moves into the list of places, which is one stop for Tab; in it Up and Down move,
Space leaves the chosen note or place out or takes it in again, Ctrl+Enter (Cmd+Enter) replaces in the chosen note
(A3), and Up at the top goes back to the field. Ctrl+Alt+Enter (Cmd+Alt+Enter) replaces anywhere in the overlay, as
"Replace all" does in the find field; Enter in the search field goes to the field "Replace with" and replaces nothing.

## OQ-236: Can the Vim and Nano input modes replace in the note?

**Element:** UC-005 BR-017, OQ-195
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Not until the command palette exists (UC-023): Ctrl+H, like Ctrl+F, opens the find field only with the
input mode Normal, and the Vim and Nano keys keep their own search and `:s`. The replace of the workspace
(Ctrl+Shift+H) works in every input mode.

## OQ-237: Which commands does the command palette offer, and which menu does it name for each?

**Element:** UC-023 BR-001, BR-002, step 2
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The palette names a place for every command, also for those that are in no menu, and matches it like
a menu's name:
- File: New note…, Open workspace…, New workspace…; Theme: Theme… (asks for the theme, A3); Settings: Settings….
- View: Show the menu or Hide the menu (the burger button), Command palette.
- Search: Search notes, Replace in notes.
- Note, for the active note: Go to note…, Rename…, Delete…, Export…, Close tab, Find in note, Replace in note, Find
  next, Find previous; Note › Export: the kinds of export of the active note (AsciiDoc… or Markdown…, HTML…, PDF…).
- Editing view: Show facade view, Show source view. Formatting: the buttons of the toolbar by what they do (Format as
  heading, Format as bold, Format as italic, Format as code, Format as list, Format as quote, Insert link…, Insert
  image…, Insert table).
- Status line: Show the minimap or Hide the minimap, Turn on copy on select or Turn off copy on select (the switches
  name what they do now), Choose the input mode… (A3).
- Git: Show changes, Branches, Commit (Amend while amending), Sync now, Abort the sync.
- Todos: Open the todo list. Agents: Agent access… (asks for the level, A3), Settings of this workspace….

Left out: the field "Wiki project URL" of the Workspace menu (it asks for a text, not a choice), the buttons that act
on one changed file, one todo or one place of a conflict, "Undo" and "Open" of the status bar's reports, and the keys
that move the keyboard between parts of the window (Tab, F6, Shift+F10, Escape). Keys of the editor that change the
text (select all, undo, redo, folding) count as keys of typing in the text.

A command keeps the name of its menu entry where it has one (BR-001). The switch of the editing view, the toolbar's
buttons and the status line's switches and choice have no menu entry; their buttons say only "Facade", "B" or
"Minimap", so the palette names them by what they do (after the UX review, 2026-11-12).

UC-023 BR-001 should list the places; whether the left-out ones belong in the palette is for the user to decide.

## OQ-238: Which texts do go to note and the command palette show?

**Element:** UC-004 A12, A13; UC-023 steps 1–2, A1–A3
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** As the rest of the window words its texts:
- The field is named "Go to note", "Command palette", or, while a command asks for a choice, the command's name
  without "…" ("Theme", "Choose the input mode", "Agent access"); the name also stands visibly above the field. Empty,
  go to note hints "Go to note (type > for commands)".
- Backspace in the empty field of a command's choices goes back to the commands.
- A key that runs its command only in one place is followed by "in the note" or "outside the note" (OQ-243).
- The top bar has a button "Command palette" after the magnifier, its hint "Command palette  Ctrl+Shift+P" and
  "Go to note  Ctrl+P" on a second line; the screen without a workspace shows "Command palette" with its key under
  "New workspace…".
- A note shows its title and, weaker, its folder. A command shows its name, weaker its menu, "not available" when it
  cannot run now, and its key at the right edge. A choice shows a round mark, filled for the one in effect, and
  "current" after it.
- "No note matches “{text}”." with the entry "New note “{text}”…" (A13); the entry also stands last under the notes
  that match whenever something is typed, and screen readers hear "{n} notes, or New note “{text}”…". The reason a
  command is not available stands on a line of its own under the chosen row and is its description for screen
  readers. "No command matches “{text}”." (A1); "No
  choice matches “{text}”."; "No workspace is open." in go to note without a workspace; "The workspace holds no notes
  yet." for an empty workspace.
- Why a command cannot run (A2), marked "⚠": "No workspace is open.", "No note is open.", "The workspace is no git
  repository.", git's own problem when git cannot be asked, "The workspace has no remote to sync with.", "A sync is
  running.", "Resolve the conflicts first.", "No sync waits for its conflicts.", "The find field is not open.",
  "Nothing is chosen.", "A synchronization left notes in conflict; resolve them before committing.", and "Open a
  workspace to connect agents to it."

## OQ-239: How does go to note list a large workspace?

**Element:** UC-004 A12 step 2, BR-014; NFR-018
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** It lists the first 50 notes that match and 50 more each time the list is scrolled near its end or the
keyboard comes within five entries of it, with "Showing {k} of {n} notes. Scroll for more." under the list, as the
search of the workspace does.

## OQ-240: Where does Ctrl+P open go to note in the Vim and Nano input modes?

**Element:** UC-004 BR-014; UC-008 BR-009
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Everywhere in the window except while the keyboard is in the text of a note with the input mode Vim or
Nano, where Ctrl+P is a key of that input mode. Ctrl+F of the find field, by contrast, opens only with the input mode
Normal (UC-008 BR-009).

Readings: (a) as assumed; (b) only with the input mode Normal, wherever the keyboard is, as Ctrl+F.

Implemented and tested as (a) for Vim and Nano (2026-11-12). Neither Ctrl+P nor Ctrl+Shift+P and F1 open while a
dialog that waits for an answer is open (Rename, Delete, Export, New note, the settings…), as Ctrl+Shift+E does not.

## OQ-241: Does the export of the command palette ask for the kind in the field or in its window?

**Element:** UC-023 A3, BR-005; UC-009 step 1
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Both ways exist as in the menus: "Export…" (the tab's menu, Ctrl+Shift+E) opens the window "Export note",
which asks for the kind as from its key (BR-005), and each kind of the note overview's submenu "Export" is a command
of its own ("PDF…" under "Note › Export"), which asks only where to save. No command asks for the kind in the
palette's field, although A3 names the note format of an export as an example.

## OQ-242: What does "starts with what was typed" mean for several words?

**Element:** UC-004 BR-014; UC-023 BR-002
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** The whole text typed, upper and lower case aside and runs of spaces read as one: "plane ti" puts "Plane
Tickets" in the first group; "tick plane" matches it but in the last group.

## OQ-243: Which command does Ctrl+Shift+H run, the heading of the toolbar or the replace in the workspace?

**Element:** UC-005 A2 (toolbar keys), UC-022 BR-008
**Raised by:** Implementation, 2026-11-11
**Blocking:** No
**Status:** Open
**Assumed:** Reading (b), the interim answer below.

Readings: (a) as since the replace of the workspace came: with a workspace open, Ctrl+Shift+H opens the replace in
the workspace from anywhere, so the heading of the toolbar has no key that works; (b) the heading keeps Ctrl+Shift+H
in the text of a note, and the replace takes it elsewhere; (c) one of them gets another key.

**Interim answer** (decided by the coordinator of the implementation, 2026-11-12, for the user to confirm): reading
(b). While the keyboard is in the text of a note (source view or facade), Ctrl+Shift+H (Cmd+Shift+H) is the
toolbar's heading, as before the replace of the workspace came; everywhere else, also in the search overlay, it opens
"Replace in notes" or, while it is open, closes it again. The command palette shows the key at both commands, each
with its place: "in the note" and "outside the note".

## OQ-244: Do pictures, included files and written-out web addresses count as links of a note?

**Element:** UC-004 BR-015; FR-078, FR-079; entity NOTE_LINK
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** A picture (`![…](…)`, `image:`) and an included file (`include::`) are no links: they are neither
outgoing links nor backlinks, also when the file is missing. A web address written out as it is (`<https://…>`, a bare
address starting with `http://`, `https://` or `mailto:`, which GitHub and GitLab show as a link) is an outgoing
link; an address without its scheme (`www.example.org`, an e-mail address) is none. A link by reference
(`[text][label]`, `[label]`) counts once, at the line of its definition (`[label]: place`), where its place is
written; its uses are not listed. A line such as `[Note]: remember the milk` (text, not a place and a title), a
footnote (`[^1]: …`) and a definition inside a paragraph are no links. A formula (`$$` … `$$`) holds no links.

Readings: (a) as assumed; (b) pictures and included files are outgoing links too, so a missing picture shows as a
broken link; (c) a link by reference counts at each place it is used.

## OQ-245: Is a link that leads outside the workspace an outgoing link, and is it broken?

**Element:** UC-004 BR-005, BR-015
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** It is listed as a link to a file, with its place as written, and not marked as broken: it is no local link
(BR-005), and only a local link can be broken. A link to a file on this computer by its full place is marked as broken
when the file does not exist. Besides addresses with `://`, the places starting with `mailto:`, `tel:`, `sms:` or
`urn:` are addresses for another program, never notes; other words before a `:` are not (a note may be named
`Note: x`, and `C:` is a drive on Windows). A `://` after the `#` of a place belongs to the heading.

Readings: (a) as assumed; (b) it is marked, since following it does nothing; (c) it is not listed.

## OQ-246: In which form does link completion write a link started with `xref:` or `<<`?

**Element:** UC-005 A23 step 3, BR-018
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** In the form the user started, with the place the toolbar would write: `xref:place[text]` or
`<<place,text>>`, keeping a text the user gave. A link started in the facade, or with `](` in the source view, is
written as the toolbar writes it (`[text](place)`, `link:place[text]`).

Readings: (a) as assumed; (b) always as the toolbar writes it, replacing the `xref:` or `<<` the user typed.

## OQ-247: What does Ctrl+Space do in the facade and outside the place of a link?

**Element:** UC-005 A23 (trigger)
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** In the facade, which shows no places of links, Ctrl+Space starts the completion at the cursor as `[[`
does, and the link chosen is inserted there. In the source view, Ctrl+Space outside the place of a link does nothing.
Ctrl+Space is Ctrl on macOS too, and it opens the list again after Escape closed it for the same link. As an input
method may take Ctrl+Space, the command palette has "Complete link" (menu Note), which does the same.

Readings: (a) as assumed; (b) in the facade, Ctrl+Space on the text of a link completes its place; (c) Ctrl+Space
does nothing in the facade.

## OQ-248: How are places with spaces or brackets written in a link?

**Element:** UC-005 A2, BR-018; UC-004 BR-005
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** The toolbar writes the place as it is, such as `[Plan v2](../a b/Plan v2.md)`; GitHub and GitLab do not
read such a Markdown link as a link, so it is then neither followed, nor an outgoing link, nor a backlink. Link
completion writes the place relative to the note, with the extension, and percent-encodes what would end the place or
start a heading: `%`, `#`, spaces, brackets and angle brackets (`../a%20b/C%23.md`), so its links always lead to their
note. Its form is `[text](place)` in Markdown, `link:place[text]` in AsciiDoc, or `xref:place[text]` and
`<<place,text>>` when the user started so (OQ-246), with `#` and the anchor of a heading. A title or heading taken as
the link's text has its brackets escaped (`Plan \[v2\]`); a text the user typed stays as typed. Headings of the same
name are told apart as GitHub and GitLab number them (`intro`, `intro-1`); an anchor is matched with its escapes
decoded and upper and lower case alike.

Readings: (a) as assumed (the completion encodes, the toolbar does not); (b) the toolbar encodes too; (c) neither
does.

## OQ-249: What does the tool panel show of a backlink and of an outgoing link, and where does the cursor go?

**Element:** UC-004 BR-015, A14
**Raised by:** Implementation, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:**
- "Backlinks (N)" and "Outgoing links (N)" follow "Figures", open at first, "None" when empty and "Looking for
  links…" until the first lists of the note come. N counts the links, not the notes.
- A backlink: the title of the note that holds it, weaker its folder; under it each link with its line number and its
  line, at most 40 characters before the link and 60 after it, the link marked. Screen readers hear the line number,
  the line and the note ("4 [the plan](Plan.md) first, in Ideas").
- An outgoing link: its text (its place when it has none) and, weaker, where it leads: the place of the note in the
  workspace with `#` and the heading, the file, or the web address; a broken one says "broken" and is underlined as an
  error. Above the list: "Ctrl+click or Ctrl+Enter follows a link." (Cmd on macOS).
- Choosing a backlink opens its note with the cursor at the start of the link, nothing selected. When the note
  changed so the link is no longer at its place, the note opens without moving the cursor and the status bar says
  "{title} changed since the backlinks were listed; the link is no longer at its place".
- Choosing an outgoing link puts the cursor at its start and the keyboard into the text; with Ctrl (Cmd on macOS)
  held, by click or with Enter, it is followed. When the text of the note changed since the list was made, nothing is
  followed and the status bar says "{title} changed since its links were listed; choose the link again"; the list
  follows the change.

## OQ-250: How does the user follow a link by keyboard?

**Element:** UC-004 A2 to A4, A14; NFR-011
**Raised by:** UX review of package 3, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** Interim answer (decided by the coordinator of the implementation, 2026-11-12, for the user to confirm):
the command palette has "Follow link" (menu Note), which follows the link at the cursor in either view, as a
Ctrl+click there does; in the outgoing links of the tool panel, Ctrl+Enter (Cmd+Enter) on a link follows it. The text
itself gets no key for it, as Ctrl+Enter enlarges a picture in the facade. The cursor has to be on the text of the
link (in the source view between its brackets), as for a click.

Readings: (a) as assumed; (b) a key of its own in the text, such as Alt+Enter; (c) Ctrl+Enter follows a link in the
text and enlarges a picture only when the cursor is in one.

## OQ-251: Does link completion match the place of a note too?

**Element:** UC-005 A23 step 1, BR-018; UC-004 BR-014
**Raised by:** App test of package 3, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** Yes: besides the title and the folder, as go to note matches, link completion matches the place of the
note with its extension (`travel/Packing.md`), so typing a place as a link names it (`Hub.md`, `travel/Pack`,
`Meeting-0`) finds the note. The order is that of go to note: by the title first. Go to note itself matches title and
folder only.

Escape in the source view with the input mode Normal and no list shown leaves the text for the note's tab, as before
(M-08.10, egui); with the list shown, Escape only closes the list.

Readings: (a) as assumed; (b) only title and folder, as go to note.

## OQ-252: How exactly does close pairs behave around fences, line starts, and the facade?

**Element:** UC-005 BR-019, A25; FR-081
**Raised by:** Implementation of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 4):

- A backtick typed right after two backticks is typed as it is, so ```` ``` ```` opening a code fence gets three
  backticks, not four.
- `**` and `~~` close only after text on their line (within a line), so `**` and `***` at the start of a line are
  typed as they are; the condition for the closing char (end of the line, a space, or a closing bracket after the
  cursor) holds for them too.
- Besides `*`, `_` (and `#` in AsciiDoc), a single `~` encloses a selection too, as the markup of the pair `~~`.
- The facade closes brackets and quotes only, written as the facade writes typed chars (`\[\]` in Markdown); inline
  markup closes only in the source view. Around a selection within one block the facade encloses with brackets and
  quotes as typed chars, and with `*`, `_`, `~` (and `#` in AsciiDoc) as markup, so the selection is formatted; in code
  the markup replaces the selection as typed text does.
- The closing chars close pairs wrote are forgotten once the cursor is put elsewhere (a click, an arrow key); after
  that a closing char typed before one of them is typed, not moved over.

Readings for the fence: (a) as assumed; (b) every backtick closes, as BR-019 reads, and the user deletes the fourth.
For the facade: (a) as assumed; (b) inline markup closes in the facade too, around a selection.

## OQ-253: How are list items indented in the cases BR-021 does not name?

**Element:** UC-005 BR-021, A27; FR-083
**Raised by:** Implementation of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 4):

- An AsciiDoc item written with `-` has a single level and stays where it is; `*` and `.` get one more or one less.
- An item that changes the marker (`. b` below `* a` in AsciiDoc) counts as a list of its own: it has no item before
  it on its level.
- Outdenting an item that has items after it on its level makes those its own items (the text below it keeps its
  indent), as the lines of the markup say.
- The further lines of an item, and lines directly after it that continue its paragraph, move with it; a blank line
  ends an item unless the line after it is indented below the item.
- An ordered item keeps its number; the numbers are not counted anew.
- A tab counts to the next tab stop (every four columns). Indenting writes spaces after the space a line already
  starts with; outdenting a line that starts with a tab writes its remaining space as spaces.
- An AsciiDoc item on the fifth level (`*****`) is not indented: a sixth marker char would make it plain text; nor is
  an item whose own items would reach a sixth level.

Readings for the following items: (a) as assumed; (b) they are outdented along, keeping the item's place in the list.

## OQ-254: How does "Go to line" ask, and what does it accept?

**Element:** UC-004 A16, A17, BR-017; FR-086
**Raised by:** Implementation of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 4, after review): Ctrl+G (Cmd+G on macOS) and the palette's "Go to
line…" ask in the field of the command palette, titled "Go to line", which says "The note has {n} lines. Add a column
after a colon, such as 120:8." in the source view and "The note has {n} lines of markup." in the facade. Line or column
0 is "Lines and columns start at 1."; an empty field and anything but digits and one colon are "A line number is
expected, such as 120 or 120:8."; the text stays and nothing moves. A line beyond the last says "Went to the last
line." in the status bar, a column beyond the end of the line "Went to the end of the line.". In the facade the column
is used where the facade shows that place of the markup, else the nearest place it shows. A line after the last block
(the empty line after the last line break) goes to the end of the last block. Ctrl+G without an open note says "Open
a note first." in the status bar.

Cmd+G is "Find next" in many macOS programs; F3 is "Find next" here (UC-008). Readings: (a) as assumed; (b) a dialog
of its own; (c) another key on macOS, such as Ctrl+G.

## OQ-255: Past which block does the facade move a block?

**Element:** UC-005 BR-020, A26; FR-082
**Raised by:** Implementation of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 4): a block moves past the unit next to it on its level: a list item
past the item before or after it with that item's own items, a block that is no list item past a whole item of the
outer list. A list item past an item of an outer level becomes, by its indent, an item of the item it now follows.
What stands between the two (a blank line, or none) stays between them; where a block would join the one next to it
(a paragraph after a list item, two paragraphs), a blank line is added. A copy of a list item goes on the next line,
a copy of another block after a blank line, with the note's line break. An AsciiDoc `+` line belongs to the block
after it.

Readings: (a) as assumed; (b) a block moves past one block only, never past a whole item with its items; (c) a list
item never leaves its list.

## OQ-256: What does Shift+Tab do outside a list item?

**Element:** UC-005 BR-021, A27; OQ-204
**Raised by:** Review of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** In the source view, Shift+Tab takes up to one tab stop (four columns) of space from the start of every
line the selection touches, as one step for undo, and does nothing on lines without space; it never writes space. In
the facade it does nothing. Before package 4 it wrote four spaces, as Tab does.

Readings: (a) as assumed; (b) nothing outside a list; (c) four spaces, as before.

## OQ-257: Does a closing quote typed after the pairs were forgotten write a new pair?

**Element:** UC-005 BR-019, A25
**Raised by:** Review of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** Yes: `"` and `` ` `` open and close alike, so once the cursor was put elsewhere (OQ-252), a `"` typed
before a space or at the end of a line writes `""` even where the user means to close a quote typed earlier.

Readings: (a) as assumed; (b) a quote right after a word char is never paired; (c) count the quotes of the line, and
pair only when their number is even.

## OQ-258: Are Tab and Shift+Tab inverse around a fence that is not closed?

**Element:** UC-005 BR-021; BR-019
**Raised by:** Review of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** No guarantee: lines after an opening fence (```` ``` ````, `~~~`, `----`) that is never closed count as
code up to the end of the note and hold no list items, so Tab may find a list where Shift+Tab then finds none, or the
other way round, once a fence is typed or deleted between them.

Readings: (a) as assumed; (b) a fence that is not closed is no fence for these keys.

## OQ-259: What is "Duplicate line" called in the facade?

**Element:** UC-005 A26, BR-020; UC-023 BR-001
**Raised by:** Review of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** "Duplicate line", "Move line up", and "Move line down" in both views, the names of A26; in the facade they
act on the block, as BR-020 says.

Readings: (a) as assumed; (b) "Duplicate line or block" (and "Move line or block up"); (c) the name follows the view
shown.

## OQ-260: Do Vim's `>>` and `<<` indent list items as Tab and Shift+Tab do?

**Element:** UC-005 BR-021, BR-009
**Raised by:** App test of package 4, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** No: in the source view `>>` and `<<` are those of the Vim keys (`codemirror-vim`), which shift a line by
two spaces whatever it holds, so `<<` does nothing on an AsciiDoc item and `>>` writes two spaces before it. Tab and
Shift+Tab in insert mode, and the palette's "Indent list item" and "Outdent list item", follow BR-021 in every mode.
Routing `>>` and `<<` to BR-021 would need the Vim keys to ask the editor core whether the line is a list item before
falling back to their own shift, with counts and the dot command; that was left out.

Readings: (a) as assumed; (b) on a list item `>>` and `<<` follow BR-021, elsewhere Vim's shift.

## OQ-261: What exactly do the text statistics count as a word and a character?

**Element:** UC-004 BR-020; FR-090; OQ-212
**Raised by:** Implementation of package 5, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 5):

- A character is what a reader sees as one character: a letter with an accent written as two code points, an emoji
  with its skin tone, or a family of emoji is one character. Spaces count, line breaks do not.
- A typographic apostrophe (`’`) joins a word as `'` does, so "it’s" is one word.
- A run of Chinese or Japanese characters without spaces is one word, as the rule "a run of letters and digits"
  reads.
- An address the facade shows as the text of its own link (a bare `https://…`) is counted as shown, by the word rule:
  `https://example.org` is two words.
- Blocks the facade shows as their markup (an AsciiDoc attribute entry or comment, an HTML block) are markup and not
  counted; the indentation of an indented code block is shown and counted.
- An image counts its caption, not its alternative text; a caption of a table or an image is part of a selection only
  when the whole block is selected, so a part of a table counts the cells selected but not the caption.
- A selection counts a character of several code points when its first one is selected, so selecting everything
  counts exactly the note; a note with `\r\n` line breaks counts as with `\n`.
- Code blocks in the languages `math`, `stem`, `latexmath` and `asciimath` are formula sources and not counted; since
  package 7b (FR-098) neither are block formulas nor inline ones (`$…$`, `latexmath:[…]`), and a word does not run
  across an inline formula.
- Without words the reading time is "0 minutes".

Readings for CJK: (a) as assumed; (b) every CJK character is a word, as most word counters do.

## OQ-262: What does the focus mode hide besides the overview and the tool panel, and what if a command needs them?

**Element:** UC-010 A12, BR-010; FR-088; OQ-209
**Raised by:** Implementation of package 5, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 5, after review): the toolbars are the top bar (with the menu bar,
OQ-209) and the row above the note with the switch "Facade" / "Source" and the formatting toolbar. The status line
under the note stays, as it shows the place of the cursor and the mode of the Vim and Nano keys; it holds the quick
switches of A3 and a switch "Focus mode", so the mode can be seen and left with the mouse. Entering it, and starting
Underline in it, the status bar says "Focus mode on. Press Ctrl+Alt+F to leave."; the keyboard, if it was in a hidden
part, goes into the note. "Show changes" and Ctrl+Shift+G, whose list is in the tool panel, end the focus mode and
show the changes; every other command keeps the focus mode. On macOS the key is Cmd+Shift+. instead of Cmd+Ctrl+F of
OQ-209, which is the system's full screen (Cmd+Alt+F replaces in the note, OQ-195).

Readings: (a) as assumed; (b) the status line under the note is hidden too; (c) a command that shows something in a
hidden part shows that part for the moment and keeps the focus mode.

## OQ-263: How are the editor zoom's commands named and when do their keys apply?

**Element:** UC-010 A11, BR-006, BR-009; FR-087
**Raised by:** Implementation of package 5, 2026-11-12
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-12, package 5): the palette names them "Zoom in", "Zoom out" and "Reset zoom"
(menu View), and the status bar says "Editor zoom: 120%" after each key, also after Ctrl+0 at 100 percent; a step
beyond the limits changes nothing and says "Editor zoom: 300% (largest)" or "Editor zoom: 50% (smallest)". A zoom
changed in the settings window updates that status. Ctrl+= zooms in like Ctrl+Plus, for keyboards whose plus needs
Shift; Ctrl+0 is also told by the key's place, where it types another character (AZERTY). The keys apply wherever the
keyboard is, but not while a dialog or the command palette is open; the enlarged picture keeps Plus, Minus and 0 for
its own zoom. Like the other quick switches, a zoom back to 100 percent and a focus mode turned off are written to the
settings file as their values, not removed. One notch of the mouse wheel with Ctrl held is one step; the small turns of a touchpad are added up to a
notch. The settings window offers the editor zoom as "Editor zoom (percent)", whose arrow keys step by 10; a value
between two steps, typed or written in a file, is taken as the nearest step.

Readings: (a) as assumed; (b) the keys zoom the enlarged picture too while it is open.

## OQ-264: Which document attributes of an AsciiDoc note are lists?

**Element:** UC-024 BR-002; Entity: NOTE_ATTRIBUTE.kind; OQ-213
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a): AsciiDoc does not tell a list from a text, so the attributes
`tags` and `keywords` are lists, their items separated by commas, and every other attribute is a text. A list added
under another name is written with commas and read back as a text. An entry that unsets an attribute (`:name!:`) is no
note attribute and stays as it is; a value continued on the next line with ` \` is shown as its markup and changed
only in the source view, as a nested value (A6). The header ends at the first empty line; comment lines and the author
and revision lines below the title belong to it, and a new attribute goes after the last attribute entry, else after
the title and those lines. After review: a value or an item ending in `\` is refused ("An AsciiDoc value cannot end
in \."), as is a new attribute after an entry ending in `\` (it would be taken in); an item holding a comma is
refused; a name whose lower case is no name (`İx`) is refused; a byte order mark before the header is skipped.

Readings: (a) as assumed; (b) every value with a comma is a list; (c) the kind the user chose is remembered.

## OQ-265: Are the tags of the note listed in lower case or as the notes write them?

**Element:** UC-004 BR-016; TC-004-32; OQ-214
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a): the tool panel lists the tags of the active note in lower case,
each once, those of the attribute `tags` first, then those of the text in the order of the note, as TC-004-32 and
TC-024-11 expect ("travel" for `tags: [Travel]`). This differs from OQ-214's assumption ("as most notes write it"),
which is about the tags of the workspace; no list of the workspace's tags is shown yet. Choosing a tag searches for
it in lower case.

Readings: (a) as assumed; (b) as the active note writes it, the first way it writes it; (c) as most notes of the
workspace write it (OQ-214).

## OQ-266: What do kinds, quoting, and the last attribute mean in an unusual front matter?

**Element:** UC-024 BR-002, BR-003, A2, A6; OQ-215
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a), the front matter being edited as lines, never written out from
a parsed value:

- Kinds: `true` and `false` are yes or no; a number is digits with at most one point (no exponent, as YAML 1.1,
  which GitHub reads, and YAML 1.2 differ on `1e5`); a date is told by its form. A text that either version reads
  as something else (`yes`, `no`, `on`, `off`, `y`, `1e5`, `1_000`, `1:30`, `0b101`, `0x1F`, `2026-11-11 10:00`,
  `<<`) is written in double quotes; the line separators U+2028 and U+2029, the byte order mark, U+FFFE and U+FFFF
  are written escaped.
- A mapping indented as a whole is read; a new name takes its indentation. `name:` without a value is an empty text; `null` and `~` are texts as
  written.
- A value written over several lines (a block scalar `|` or `>`, a text continued on the next line), a value with an
  anchor, an alias or a YAML tag, fields in braces, and a list holding lists or fields are nested: shown as markup and
  changed only in the source view (A6). Removing such an attribute removes its lines.
- A text that would read back as another kind or holds YAML's signs (`true`, `12`, `2026-01-01`, `a: b`, `#x`, an
  empty text) is written in double quotes; other texts plainly. A comment after a value stays when the value changes.
- A list written one item per line stays so when it changes; one in brackets stays in brackets.
- The kind of an existing attribute cannot be changed in the note properties; it is removed and added again.
- A nested attribute can be removed from the note properties (its × button), which removes all its lines, an
  indented comment among them; only changing it needs the source view.
- Removing the last attribute removes the front matter only when nothing but empty lines is left in it; a comment
  keeps it, so no comment is lost. An empty front matter (`---` twice) holds no attributes and takes a new one.
- Twice the same name, a front matter holding a list or a text instead of names, and every other YAML error make it
  unreadable (A5). The reason is the YAML reader's own, in English (for example "could not find expected ':'"); the
  line is the one where the part in error starts.

Readings: (a) as assumed; (b) removing the last attribute always removes the whole front matter, comments included.

## OQ-267: How does the search by tag show what it found?

**Element:** UC-008 A8, BR-012, BR-004; TC-008-21
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a): a word `#` followed by no tag (`#42`, `#`) is searched as an
ordinary word of text. A note found by a tag is listed among the notes found by their text, never first as found by
its title. The places found are the lines holding a matching `#tag` and, for the attribute `tags`, the line or lines
of its value, marked whole; with other words, also their lines. In the open note, the find marks of the search mark
the search text as written (`#travel`), so a tag of the attribute `tags` is not marked there. The search by tag also
answers the search of an agent (UC-018), as it shares the search of the workspace. Tags in the text of a note are
found by the facade's reading of the note: a tag in the label of a link and in a table counts; since package 7b a
formula holds no tags (FR-098).

Readings: (a) as assumed; (b) the open note marks the tags found, also in the attribute `tags`.

## OQ-268: How does the facade show a front matter, and what do exports make of it?

**Element:** UC-004 BR-004; UC-009; OQ-216
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a): the facade shows the front matter as a table, the names as its
header row and the values below, a list as its items joined by commas and a nested value as its markup; a click shows
the markup below the table to be edited there, as for other tables. A front matter that cannot be read (A5) and an
empty one are shown as their markup. The table stays a table when the note opens with the cursor in it; its markup
shows once the user clicks or types in the facade. An export to Markdown keeps the front matter as it is; one to
AsciiDoc writes the attributes as document attributes below the document title (at the start without one), an
attribute AsciiDoc cannot hold (nested, a name or a value it cannot write) as comment lines, and a front matter that
cannot be read as a comment block below the title, all marked as approximated; HTML and PDF show the table. The
export writes `\n` line breaks throughout, as for every block. The text statistics do not count it (OQ-212).

Readings: (a) as assumed; (b) the facade hides the front matter, as it is shown in the note properties.

## OQ-269: How are the fields of the note attributes confirmed and how do they look?

**Element:** UC-024 step 3, BR-002, BR-007, A7
**Raised by:** Implementation of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As implemented (2026-11-13, package 6a, after review): the note properties hold, below the text
statistics, a part "Tags (N)" ("None" without tags; each tag a button named "#tag, search notes tagged tag") and
then "Attributes"; while another note's are read, the last ones stay dimmed and not usable. The search overlay says
"#tag finds the notes with that tag." while its field is empty. A refusal is shown below its field in the theme's
error colour with a sign; the field is marked; typing again or Escape takes the message away. After a remove the
keyboard goes to the next attribute (the one before, or "Add attribute"), the status says "Removed attribute X.
Undo with Ctrl+Z.", and Ctrl+Z in the panel undoes in the note unless a field holds text typed and not confirmed.
The remove button follows its field. Dates say "Dates are written year-month-day in the file."; a new list says
"Items, separated by commas.". The form of a new attribute puts each label above its field in a narrow panel. A value is written on Enter or when
its field is left; Escape gives up what was typed; a switch is written at once. A number is typed in a text field (so
A4 can say what is wrong), a date in the date field of the system, with its calendar where the webview has one. A
list shows a field per item with a remove button, and a field "Add item"; emptying an item removes it. "Add
attribute" opens a form with Name, Kind and Value (for a list, items separated by commas), Add and Cancel; the
palette's "Add note attribute" opens it with the keyboard in the name, and "Search by tag…" asks for a tag of the
active note. A change is written through the editing view shown; with the note in no view it says "Show the note to
change its attributes."

Readings: (a) as assumed; (b) a value is written only on Enter.

## OQ-270: May the attribute `tags` hold an item that is no tag?

**Element:** UC-024 BR-005; UC-004 BR-016
**Raised by:** App test of package 6a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** No: an item written into the attribute `tags` from the note properties must be a tag (a `#` before it
and a `/` after it are allowed and dropped when read); otherwise nothing is written and the properties say
"“two words” is no tag: a tag holds letters, digits, _, - and /, and not only digits." An item that is no tag
written in the source view stays in the note and is shown as an item, but is no tag of the note.

Readings: (a) as assumed; (b) any item is allowed and only the tags among them count; (c) an item that is no tag
is shown marked in the note properties.

## OQ-271: What do "Previous daily note" and "Next daily note" do without a daily note in the active tab?

**Element:** UC-025 A2
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Nothing opens; the status bar says "This note is not a daily note. Press Ctrl+Shift+J to open today's."
(the key as the platform names it). A note is a daily note when it lies directly in the daily note folder (upper and
lower case ignored) and its title is a date (UC-025 BR-001, BR-005); daily notes in folders below the daily note folder
are not stepped through. A step that opens a daily note clears an earlier report of the status bar.

Readings: (a) as assumed; (b) the step starts from the date of today; (c) the newest or oldest daily note opens.

## OQ-272: How do the note overview and the tab show the title of a daily note?

**Element:** UC-025 BR-001; UC-003 BR-001; TC-025-01
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Decided as an interim reading for the user to confirm (package 6b review, 2026-11-13), following the
reviewed test cases TC-025-01 and TC-025-04: a title that is exactly a date, `2026-11-11`, keeps its dashes everywhere
(note overview, tab, dialogs, status bar), as an exception to UC-003 BR-001; every other title shows a dash as a space.
This holds for any note so named, also outside the daily note folder.

Readings: (a) as assumed; (b) the rule of UC-003 BR-001 without exception ("2026 11 11"); (c) daily notes are shown
with the date written out ("11 November 2026").

## OQ-273: How does the setting "Daily note template" name a template?

**Element:** UC-010 BR-001; UC-025 BR-003
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** By the place of the note in the workspace, with or without its extension (`templates/Day.md`,
`templates/Day`); it may name any listed note of the workspace, also one outside the template folder. The settings
window offers "The template titled Daily" (no value of its own) and the templates of the template folder by their
place; a value that names no note is shown as "{value} (not found)" and gives TC-025-05. Without a value, the template
titled Daily is the one nearest the top of the template folder, upper and lower case ignored.

Readings: (a) as assumed; (b) the setting names a template by its title; (c) only templates of the template folder can
be named.

## OQ-274: What happens with a template folder or daily note folder that is not allowed?

**Element:** UC-010 BR-001; UC-003 BR-008; UC-025 A4
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Reading (b), after the package 6b review: the settings window refuses a folder that lies outside the
workspace, is hidden, holds a name not allowed, or (for the template folder) is the top folder; the field keeps what
was typed and says under it why ("Daily note folder not changed: the folder `.days` is hidden"). An emptied field
removes the value of the workspace, so the built-in default applies; the third column says "default" or "own". A
folder written into the settings file by hand is not checked there: a template folder that cannot hold templates
offers none, and "New note" says "No templates: notes in the folder {folder} are offered here."; a daily note folder
that is not allowed is said when the daily note is asked for (A4), with "Open settings" in the status bar. A folder
that is a link to another folder cannot take a note, as it may lead outside the workspace (UC-003 A4). Template
folders and daily note folders are matched without regard to upper and lower case, as note names are (UC-003 BR-003).

Readings: (a) any folder is taken and only said when it is used; (b) as assumed; (c) "New note" says why a template
folder offers no templates.

## OQ-275: Placeholders in detail

**Element:** UC-003 BR-008; TC-025-01; TC-003-17
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** A placeholder is matched exactly, in lower case and without spaces (`{{ title }}` and `{{Title}}` stay as
written); a template with several `{{cursor}}` loses all of them and the cursor goes to the first. Placeholders are
filled in the whole template, front matter and document attributes included. Decided after the package 6b review so
that both test cases hold: a line that holds only `{{cursor}}` right after an empty line takes the place of that empty
line, so TC-025-01 gives one empty line above "Done" with the cursor on it; after a line with text (TC-003-17) it stays
a line of its own, which becomes empty, with the cursor on it.

Readings: (a) as assumed; (b) a line that holds only `{{cursor}}` is always dropped and the cursor goes to its place;
(c) the template is copied as it is, which gives two empty lines in TC-025-01.

## OQ-276: Names and places of the commands of templates and daily notes

**Element:** UC-025 step 1, BR-007; UC-003 A7; UC-023 BR-001; TC-025-11
**Raised by:** Implementation of package 6b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The command palette has "Daily note" (Ctrl+Shift+J, named as TC-025-11 runs it), "Previous daily note"
(Ctrl+Shift+PageUp), "Next daily note" (Ctrl+Shift+PageDown) in the group Note, and "New note from template…" in the
group File, which opens "New note" with the keyboard on "Template:". The "toolbar" of UC-025 step 1 is the top bar: a
calendar button "Daily note" after the command palette's, with the hint "Daily note  Ctrl+Shift+J" in the form of the
palette button's, disabled without a workspace. The menu bar's File menu is unchanged. In "New note", "Template:"
stands between the folder and the note format, which it locks with the hint "The template sets the note format.";
without templates it is shown disabled. In the settings, the three settings stand under the heading "Templates and
daily note", each with a line that says what it does.

Readings: (a) as assumed; (b) the File menu lists the new commands too; (c) the template is chosen before the title.

## OQ-277: Keys and places to open to the side and to move between the panes

**Element:** UC-004 A18, BR-018; UC-023 BR-001; FR-096
**Raised by:** Implementation of package 7a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** "Open to the side" opens the note of the active tab to the side with Ctrl+\ (Cmd+\ on macOS), as A18
names it; on a keyboard where that sign needs AltGr (German, Swiss, French) the key at the same place does it, and
Ctrl+Shift+S (Cmd+Shift+S) does it on every keyboard. "Focus the other pane" (Ctrl+F6, Cmd+F6) goes to the other pane;
F6 alone still goes from region to region within a pane. "Focus the left pane" and "Focus the right pane" (Ctrl+1,
Ctrl+2, Cmd on macOS) stay as well; where a desktop takes these keys, Ctrl+F6 remains. "End the split view" has
Ctrl+Shift+M (Cmd+Shift+M). All of them are in the group View of the command palette; "Open to the side: link at the
cursor" (no key) is in the group Note and ranks after "Open to the side" when "side" is typed. To the side from the
note overview and from a tab: "Open to the side" at the end of their context menus, naming its key on the active
note's entry; the tab's menu also has "End the split view" while the split view shows. From go to note: Ctrl+Enter
(Cmd+Enter) on the note chosen, which the field names ("Ctrl+Enter opens to the side"). From a link: Ctrl+Shift+click
(Cmd+Shift+click); the hint on a link still says only "Ctrl+click to follow". A note a link followed to the side offers
to create (A3) opens to the side once created. The keys are taken wherever the keyboard is, but not over a dialog or
the palette; without the split view the pane keys do nothing. No input mode, no key of the editors and no desktop of
Linux, Windows or macOS gives them a meaning, and none is an AltGr combination.

Readings: (a) as assumed; (b) only the keys of the spec and Ctrl+1/Ctrl+2; (c) the hint on a link also names
Ctrl+Shift+click.

## OQ-278: How does the window show the split view and its active pane?

**Element:** UC-004 BR-018; UC-010 BR-010; FR-096
**Raised by:** Implementation of package 7a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Each pane has its own tab bar, its switch "Facade" / "Source" with the formatting toolbar, its minimap and
its status line under the note; the find field stands above the note of the active pane only. The tool panel stands
right of both panes, over their whole height, also without the split view (before, the tab bar reached over the tool
panel). The active pane is marked by a line of 3 px in the focus colour above its tab bar, and the active tab of the
other pane is dimmed; for screen readers each pane is a group "Left pane" / "Right pane", the active one current, and
a change of the active pane is told ("Right pane, Trip"). A click, or the keyboard, anywhere in a pane makes it the
active pane. A pane is at least 280 px wide; the border (12 px to grab, a line of 1 px) is dragged, moved with the
arrow keys, and set back to equal widths with a double click or Enter. When the panes do not fit at that width side by
side, only the active pane shows, with the line "The window is too narrow for both panes. Ctrl+1 and Ctrl+2 switch
between them."; when the tool panel does not fit beside two panes, it is hidden and a switch at the end of the right
pane's tab bar shows it. That tab bar ends with a button "End the split view" too. A tab whose note is open in the
other pane too carries "⇆" with the hint "Also open in the other pane". In a pane below 360 px the formatting toolbar
wraps into a second row; below 420 px the switches of the status line fold into a button "Switches" with a menu, so the
line stays one row; the find field narrows before its buttons wrap. "End the split view" puts the keyboard into the
note left active and says "Split view ended.". Dropped files go into the note of the active pane. In the focus mode
both panes keep their tab bars and status lines (TC-010-19).

Readings: (a) as assumed; (b) the tool panel stays under the tab bars and only the notes are split; (c) a split view that
does not fit is refused, with a message, instead of showing one pane.

## OQ-279: Where does the tab "Todos" stand in the split view?

**Element:** UC-021 BR-005; UC-004 BR-018
**Raised by:** Implementation of package 7a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** In the left pane only, after its note tabs. Showing it makes the left pane the active pane and shows the
todo list in place of its note; the right pane keeps its note. With the right pane made active again, the tool panel
shows the sections of the right pane's note and the commands act on it, while the left pane keeps the todo list.
Ending the split view keeps it where it is.

Readings: (a) as assumed; (b) in the active pane; (c) it can be opened in either pane like a note.

## OQ-280: Where does the cursor of the second tab of a note start, and what keeps each tab?

**Element:** UC-005 BR-016; UC-004 A19, BR-003
**Raised by:** Implementation of package 7a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The tab the note opens in on the other side starts with the cursor where the first tab has it, nothing
folded, at the start of the note. Each tab keeps its cursor, selection, folded sections and reading position also
while it switches between the source view and the facade, until it is closed; a change made on the other side moves
its cursor along. The Vim register, the last search of the Vim and Nano keys, and the lines Nano cut are the
window's, so a yank on one side is put on the other; the mode of the keys is each tab's. After a restart both start
as a single tab does.

Readings: (a) as assumed; (b) the second tab starts at the start of the note; (c) it starts where the first one is
scrolled to.

## OQ-281: The source view on both sides

**Element:** UC-005 BR-016; FR-097; UC-004 BR-004
**Raised by:** Implementation of package 7a, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** A note may show its source view in both panes (each tab keeps the view the user switches it to). The code
and markup colours then follow the lines of the source view scrolled last; lines that only the other side shows keep
the colours they had and get them anew once that side is scrolled. The colours are not made for the lines of both
views together (package 7a review: left as it is, recorded here).

Readings: (a) as assumed; (b) the colours are made for the lines both source views show; (c) the second tab of a note always
shows the other editing view and cannot be switched.

## OQ-282: Does `$22 and $2+2$` hold a formula, as GitHub reads it?

**Element:** UC-004 BR-019; FR-098; C-003
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As the Markdown parser reads the dollar (ADR-029): an opening `$` is followed by no space, a closing one is
preceded by no space and followed by no digit, `\$` is a dollar, `$$…$$` within a paragraph is a display formula in
its line, and `` $`…`$ `` is a formula. So "5$ or 6$" stays text, and `$22 and $2+2$` is the text "$22 and " and the
formula `2+2`. GitHub's own reading of the last case could not be checked offline.

Readings: (a) as assumed; (b) as Pandoc reads it, one formula `22 and $2+2`; (c) no formula at all.

## OQ-283: Is a `$` typed in the facade written as it is?

**Element:** UC-005 BR-008; FR-098
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Yes. A `$` typed in the text of a Markdown note is written without a backslash, so a price stays `5$` in
the markup, and typing `$x$` makes a formula, which is typeset once the cursor leaves it. Other formatting characters
(`*`, `_`, `` ` ``) are still escaped. Inside a formula everything typed is its source, without escapes.

Readings: (a) as assumed; (b) a typed `$` is escaped (`\$`) like the other formatting characters, so a formula is
written only in the source view.

Package 7b review (2026-11-13): the reviewers recommend keeping (a).

## OQ-284: How is a formula with an error exported?

**Element:** UC-009 BR-002, BR-003, A3; TC-009-18; FR-098
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** In HTML and PDF it is its source as code (a code block for a block formula, inline code for an inline
one). In the other note format it is its source as well, not a formula of that notation: a literal block (`....`) or
a code block without a language, and inline code (`` `+…+` `` or `` `…` ``). It is not reported as missing in the status
bar, as A3 names only diagrams and images.

Readings: (a) as assumed; (b) in the other note format it is written as a formula all the same (`latexmath:[…]`, a
`[latexmath]` block), with its error; (c) it is also reported as missing (A3).

A `$$…$$` within a paragraph goes to AsciiDoc as a formula in the line: AsciiDoc has no display formula in a line, and
writing the display style into the source would change the formula on the way back.

## OQ-285: What does the frame below a block formula show?

**Element:** UC-005 BR-008; UC-004 BR-019, A21; FR-098
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Its whole markup, as for a table or an image: `$$…$$`, the fences of a code block of the language `math`,
or `[latexmath]` with its `++++` lines, so the formula can also be made text again in the facade. The frame shows while
the cursor is in the formula, until the formula is typeset, and with an error, whose reason stands above it. A changed
block formula keeps its last picture until the new one comes.

Readings: (a) as assumed; (b) only the source between the delimiters, as for a diagram.

## OQ-286: How large is a formula next to the text?

**Element:** UC-004 BR-019; FR-098; C-003; UC-010 BR-009
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** An em of the formula is an em of the text around it (ADR-029), so it grows with the editor zoom; a block
formula is set in the size of body text. KaTeX, which GitLab uses, sets formulas 1.21 times larger than the text; GitHub
(MathJax) sets them about as large as the text.

Readings: (a) as assumed; (b) 1.21 times the text, as KaTeX does.

Package 7b review (2026-11-13): the reviewers recommend keeping (a).

## OQ-287: Is an inline formula shown typeset while its source is edited?

**Element:** UC-005 BR-008; TC-005-42; FR-098
**Raised by:** Implementation of package 7b, 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** No. While the cursor is in an inline formula (after its first character of markup and before its end), its
markup stands in its place in the code font and is edited there; the typeset formula shows again, with every change in
it, once the cursor leaves it. A click on the typeset formula puts the cursor at the start of its source. A block
formula stays typeset above the frame with its markup and follows each change.

Readings: (a) as assumed; (b) the typeset inline formula also shows above or below the line while its source is
edited.

Package 7b review (2026-11-13): the reviewers recommend keeping (a).

## OQ-288: Joining an AsciiDoc document title with the line after it

**Element:** UC-005 BR-016; FR-097
**Raised by:** Package 7b review (test monkey), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** As it is: an edit that joins the document title of an AsciiDoc note with the line after it may show other
blocks in the facade until the note is parsed whole again (the next edit near the start, or opening the note). Found
by the test monkey outside of formulas; not changed in package 7b. Lines that a lone carriage return, U+000B, U+000C,
U+0085, U+2028 or U+2029 parts are now parsed whole after every edit near them, so they show as a full parse does.

Readings: (a) as assumed; (b) every edit in the first two blocks of an AsciiDoc note parses the note whole.

## OQ-289: What does the undo of a replacement in the workspace keep?

**Element:** UC-022 A6, BR-005; UC-026 BR-001
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The undo of the status bar gives each note back the content it had before the replacement, and keeps
what it replaces as a snapshot with the cause Restore, as restoring a snapshot does (UC-026 step 6).

Readings: (a) as assumed; (b) the undo keeps nothing, as the replaced text follows from the snapshot before it; (c)
a cause of its own, such as "Undo replace".

## OQ-290: When exactly is the Editing snapshot taken?

**Element:** UC-026 BR-001; OQ-223
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** When a change made in Underline is stored (at most 2 seconds after it, NFR-005), of the content the
note's file held before; its time is when it is taken, not the moment of the first key. Taking it never holds up
typing or storing. A tab asks for it at most once each 10 minutes, so when the newest snapshot held the same content
then, the next one waits for the next ask. A newest snapshot dated later than now (a clock set back) counts as old.

Readings: (a) as assumed; (b) at the first key itself, of the text the tab showed before it, also when it is never
stored.

## OQ-291: What happens when a snapshot cannot be taken?

**Element:** UC-026 BR-001, failure postconditions
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** A restore, a replacement in the workspace and an agent's change of a note not open are not made
without their snapshot; the status bar or the agent's answer says why. For editing, a change from another program,
an agent's change of an open note, git and a resolved conflict the change goes on: the status bar says "No
snapshot of {note} could be taken: {why}", because the user's text or git's work must not be held up by it.

Readings: (a) as assumed; (b) nothing changes a note without its snapshot; (c) a snapshot that cannot be taken is
only logged.

Several that fail together are said as one report, with their number and the first reason; a report that offers an
undo, or of a note that could not be stored, is not replaced by it. Snapshots that cannot follow a renamed, deleted or
restored note stay where they were (with a deleted note: in its folder among the deleted notes, removed with it after
30 days), and the status says so after the report of the rename, deletion or restore.

Known limit (ADR-031): the window and an MCP server running without it on the same workspace do not wait for each
other when taking snapshots. Each may take the same content once, so a snapshot can come twice; none is ever
overwritten or lost. No snapshot is read, written, moved or removed through a symbolic link.

## OQ-292: How does the whole snapshot show code, pictures and diagrams?

**Element:** UC-026 A2
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The whole snapshot is shown as the facade lays out its blocks, read only: headings, text, lists,
quotes and tables as in the facade; code without its colours; pictures, diagrams and formulas as their
placeholders ("Loading …", "Rendering …") or their markup, as nothing is rendered for a text that is no note.

Readings: (a) as assumed; (b) everything rendered as in the facade, with the cost of rendering diagrams and loading
pictures for each snapshot shown.

## OQ-293: Which notes get a Git snapshot, and when?

**Element:** UC-026 BR-001 (cause Git)
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Before a pull (in a sync or alone) and before a switch to another branch, every note whose file differs
between the branch and what is pulled or switched to keeps its content as it is now, open in a tab or not; a
resolution of a conflict keeps the note with its conflicting places before each choice made in it. Aborting a sync
and "Keep here"/"Keep remote" of a file that is no note take no snapshot. A note changed by git while open is also
seen as changed from outside; that snapshot is not taken again, as it holds the same content.

Readings: (a) as assumed; (b) only notes open in a tab, as for other programs (OQ-223); (c) one snapshot before the
first choice of a resolution only.

## OQ-294: How much space may the snapshots take, and does the user see it?

**Element:** UC-026 BR-002; OQ-224; NFR-014
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** No limit and no display: each snapshot is a whole copy of the note, removed after 30 days. A note of
100 KB edited all day may keep about 5 MB a day (one snapshot each 10 minutes).

Readings: (a) as assumed; (b) a limit per workspace (for example 100 MB), the oldest snapshots removed first; (c)
the space shown in the settings of the workspace, with a button to remove all snapshots.

Recommended by the reviewers of package 8a: no limit now; the dialog shows "Kept on this computer for 30 days" below
the list. Later, the settings of the workspace show the space the snapshots take and offer "Remove all snapshots".
Snapshots are never dropped without the user knowing.

## OQ-295: Where do "Snapshots…" stand in the context menus, and is there a key?

**Element:** UC-026 step 1, BR-006
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** "Snapshots…" follows "Export" in the context menu of a note in the overview and of a tab, a line
separates both from "Open to the side", and the command palette has "Note: Snapshots…" for the active note, also in
the menu "Note" of the menu bar; it has no key of its own (as the reviewers of package 8a recommend).

Readings: (a) as assumed; (b) a key such as Ctrl+Alt+H ("history").

## OQ-296: What becomes of the snapshots of a note deleted or renamed by another program?

**Element:** UC-026 BR-003
**Raised by:** Package 8a (snapshots), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The snapshots follow a note renamed or deleted in Underline (by the user or an agent). A note deleted or
renamed by another program leaves its snapshots under its old name: they are listed again when a note of that name
appears, and removed after 30 days.

Readings: (a) as assumed; (b) a note that appears where one disappeared at the same moment takes its snapshots
along, as a rename.

## OQ-297: How are the causes of snapshots named in the list?

**Element:** UC-026 step 2, BR-001; entity SNAPSHOT.cause
**Raised by:** Package 8a review (UX), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The list names what a snapshot was taken before: "Before editing", "Before an outside change", "Before an
agent's change", "Before a replace in the workspace", "Before a git change", "Before a restore". The specification
names the causes Editing, Outside change, Agent change, Replace in workspace, Git and Restore; the test cases keep
those names for the causes, which the list shows in these words.

Readings: (a) as assumed; (b) the list shows the cause names of BR-001 as they are.

## OQ-298: Which language of the system counts as one of the offered spell check languages?

**Element:** FR-100, UC-010 BR-001 (built-in default of the spell check language)
**Raised by:** Package 8b (spell check), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** English of the United States and English without a region count as English (US), English of the
United Kingdom as English (UK); French and Spanish of any region as French and Spanish. English of another region
(Canada, Australia, India, …) and every other language give no default: the spell check is off until the user chooses
a language.

Readings: (a) as assumed; (b) English of the Commonwealth countries counts as English (UK), of Canada as English
(US); (c) only the exact languages of the list count.

## OQ-299: Do the German and Italian dictionaries come with Underline?

**Element:** FR-100, NFR-014, C-008; OQ-206
**Raised by:** Package 8b (spell check), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Not yet: Underline offers no German (Germany), German (Switzerland) or Italian spell check, because their
dictionaries (igerman98 / frami, the Italian dictionary of LibreOffice) are licensed GPL-2.0 or GPL-3.0 only, and
Underline is MIT (ADR-030).

Readings: (a) as assumed; (b) they come with Underline as separate, unchanged files next to the program ("mere
aggregation"), with their GPL texts and an offer of their source, and the bundle is "MIT and others"; (c) the user
downloads them once, from within Underline or by hand, into a folder Underline reads.

TC-005-40 and TC-010-21 name German (Switzerland) in their preconditions; until this is decided they are tested with
English (US) and English (UK), and a language without a dictionary says "No dictionary for {language}" and marks
nothing.

## OQ-300: What does the user dictionary keep of a word, and where?

**Element:** Entity DICTIONARY_WORD (added_at); UC-005 A28
**Raised by:** Package 8b (spell check), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** The words alone, one per line, sorted, in the file `underline-words.txt` next to the user settings file
(ADR-030), so the user can edit and share it as text. When a word was added is not kept. A word added in one Underline
counts in another running at the same time within a second; a word added with a capital counts only with the capital,
one added in small letters also with a capital at the start of a sentence.

Readings: (a) as assumed; (b) each line also holds the date the word was added, as the entity says; (c) the words
live in the settings file.

A words file that cannot be read (not UTF-8, no permission, larger than 2 MB) is never written over: a word added
then counts until Underline closes, and the status bar says why (package 8b review).

## OQ-301: Are the words of a table marked in the facade?

**Element:** UC-005 BR-023 ("in both editing views")
**Raised by:** Package 8b (spell check), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Yes: the cells of a table, and the values of the front matter shown as a table, carry the marks in the
facade as in the source view (changed by the review of package 8b; see OQ-303 for the front matter). While a block is
edited in the facade, the marks in that block are taken away until the check after the rest of 250 ms brings them
back; in the source view they move with the typing.

Readings: (a) as assumed; (b) the cells of a table in the facade carry the marks too; (c) the marks of the edited
block move with the typing in the facade as they do in the source view.

## OQ-302: Which entry of the spell menu has the keyboard, and in which order are its entries?

**Element:** UC-005 A28 step 1
**Raised by:** Package 8b (spell check), 2026-11-13
**Blocking:** No
**Status:** Open
**Assumed:** Decided by the review of package 8b (UX): the menu waits up to about 100 ms for the corrections. It shows
the word as a header, a line saying how many corrections there are ("3 corrections", "No corrections found",
"Corrections unavailable"), the corrections, the likeliest first, a line, and "Add "{word}" to dictionary" last. When
the corrections came in time, the first has the keyboard; without corrections "Add …" has it. When they did not, the
menu opens with "Looking for corrections…" and the keyboard on the menu itself; when they come, the first takes the
keyboard unless the user pressed a key or moved the pointer since. The line under the header stays, so the entries do
not move; screen readers hear it through a polite live region. A correction changes only the letters that differ, so
markup inside the word (`**Under**lne`, `[Under](…)lne`, `te**h`) stays; it is made only while the text is the one the
word was marked in, otherwise the status bar says "The text changed since the word was marked; the correction was not
made". F7 and Shift+F7 go to the next and the previous misspelled word, "Show corrections for the word at the cursor"
opens the menu from the command palette.

Readings: (a) as assumed; (b) the menu waits up to about 100 ms for the corrections and the first correction has the
keyboard; (c) the first correction takes the keyboard when it arrives, as long as the user pressed no key.

## OQ-303: Are the values of the front matter spell checked?

**Element:** UC-005 BR-023 ("names of note attributes" are not checked), UC-024 BR-001
**Raised by:** Package 8b review (rust-reviewer, ux-reviewer), 2026-11-16
**Blocking:** No
**Status:** Open
**Assumed:** Yes, in both editing views, as BR-023 leaves out only the names: a value shown as it is written (bare, in
quotes or in brackets) is checked and marked in the facade's front matter table as in the source view. The value of
`tags`, and values the facade shows otherwise (nested values, values with escapes), are not checked in either view,
so both views always mark the same words.

Readings: (a) as assumed; (b) no value of the front matter is checked; (c) only text values are checked, not lists.

