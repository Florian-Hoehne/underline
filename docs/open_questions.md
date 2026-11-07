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
