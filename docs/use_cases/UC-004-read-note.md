# Use Case: Read Note

## Overview

**Use Case ID:** UC-004  
**Use Case Name:** Read Note  
**Primary Actor:** User  
**Goal:** The user reads a note in a tab, rendered with its code, diagrams, and images, finds their way through it and to the notes it links to, and sees how it is stored.  
**Trigger:** The user chooses a note in the note overview, follows a local link, or a workspace opens with remembered tabs.  
**Status:** Reviewed  

**Requirements:** [FR-004, FR-017, FR-018, FR-019, FR-027, FR-030, FR-035, FR-036, FR-037, FR-038, FR-039, FR-040, FR-041, FR-042, FR-043, FR-046, FR-071, NFR-004, NFR-005, NFR-007, NFR-008, NFR-014, NFR-015, C-003, C-004, C-005](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user chooses a note in the note overview.
2. The system opens the note in a new tab in the facade, placed right of the active tab, makes it the active tab, and lists it first in the note overview.
3. The system shows the note as its note format is shown on GitHub and GitLab: headings, paragraphs, lists, checklists, tables, links, and images, code blocks highlighted for their language, and PlantUML and Mermaid diagrams as pictures, all in the colors of the active theme.
4. The system shows in the tool panel the properties of the note (its place in the workspace, note format, size, and last change), its outline, its code index, and its figure index, and beside the editor a minimap of the note.
5. The user reads the note: they scroll, jump to a heading, a code block, or a figure through the tool panel or to a place through the minimap, fold and expand sections, and enlarge diagrams and images.
6. The system keeps the reading position and the folded sections of the tab while it is open, and remembers the open tabs and the active tab of the workspace.

## Alternative Flows

### A1: Note already open

**Trigger:** The chosen note is open in a tab already (step 2)  
**Flow:**

1. The system makes that tab the active tab, at its reading position, and lists the note first in the note overview.
2. Use case continues at step 4.

### A2: Follow a local link

**Trigger:** The user follows a local link in the active note (step 5)  
**Flow:**

1. When the link points to a heading of the same note, the system scrolls to that heading; use case continues at step 5.
2. Otherwise the system opens the linked note, at the heading the link names if it names one; use case continues at step 2 with that note, or at A1 when it is open already, or at A10 when it cannot be read.
3. When the named heading does not exist, the system shows the note from its start and says in the status bar that the heading was not found.

### A3: Linked note does not exist

**Trigger:** The followed local link points to a note that does not exist (A2 step 2)  
**Flow:**

1. The system says that the note does not exist and offers to create it.
2. When the user accepts, the system creates the note (UC-003) with the title and the folder of the link filled in.
3. Use case ends.

### A4: Link to something else

**Trigger:** The followed link points to a web address, to a file of the workspace that is not a note, or to a file on this computer that a note links to on purpose (UC-005 A19) (step 5)  
**Flow:**

1. For a web address the system asks, the first time in a session, whether to open web addresses, and opens it in the default browser once the user agrees.
2. For a file of the workspace, or a file on this computer linked by its full place, the system opens it with the program the system uses for that kind of file; when the file does not exist, it says so in the status bar.
3. Use case continues at step 5.

### A5: Switch or close a tab

**Trigger:** The user chooses another tab, or closes a tab (step 5)  
**Flow:**

1. The system makes the chosen tab the active tab, at its reading position.
2. When a tab is closed, the system removes it and makes the tab next to it the active tab; with the last tab closed, no note is shown.
3. The system remembers the open tabs and the active tab; use case continues at step 4 with the active note, or ends when no tab is open.

### A6: Too many tabs

**Trigger:** A note is opened while 100 tabs are open (step 2)  
**Flow:**

1. The system closes the tab that was used longest ago and says so in the status bar.
2. Use case continues at step 2.

### A7: Note changed outside

**Trigger:** An open note is changed by another program, by git, or by an agent, and all changes made to it in Underline are stored (any step)  
**Flow:**

1. Within 1 second the system shows the new content in the tab, keeps the reading position as far as the content allows, and says in the status bar that the note was changed outside.
2. Use case continues at the step it was in.

### A8: Note deleted outside

**Trigger:** The file of an open note is deleted or moved by another program or by git (any step)  
**Flow:**

1. The system keeps the tab open with the last content, marks it as deleted, and removes the note from the note overview.
2. Use case continues at the step it was in.

### A9: Note changed outside while a change is not stored yet

**Trigger:** An open note is changed outside Underline while it holds a change the user made that autosave has not stored yet (any step)  
**Flow:**

1. The system overwrites neither version: the tab keeps the user's text, which autosave stores as usual, and the version from outside is kept as a copy next to the note, whose title names the note and the time of the change.
2. The system says in the status bar that the note was changed outside and offers to show both versions side by side (FR-046).
3. Use case continues at the step it was in.

### A10: Note cannot be read

**Trigger:** The note cannot be read, or its file holds no readable text (step 2)  
**Flow:**

1. The system says in the status bar why the note cannot be read and opens no tab; the file is left as it is.
2. Use case ends.

### A11: Diagram cannot be rendered

**Trigger:** A diagram of the note has an error in its source (step 3)  
**Flow:**

1. The system shows the source of that diagram with the error in its place and renders the rest of the note.
2. Use case continues at step 4.

## Postconditions

### Success Postconditions

- The note is open in its own tab, which is the active tab, and is listed first in the note overview.
- The open tabs, their order and editing views, and the active tab are remembered for the next time the workspace is opened. Tabs opened again when a workspace opens (UC-002 step 8) do not change the order of the note overview.
- The note and its file are unchanged.

### Failure Postconditions

- No file is created, changed, or removed, except the copy of an outside version kept in A9; a note that cannot be read gets no tab.

## Business Rules

### BR-001: One tab per note

A note is open in at most one tab of its workspace. Opening a note that is open already makes its tab the active tab.

### BR-002: Reading changes nothing

Opening, reading, and closing a note never changes its file (NFR-007), in either editing view, also when a diagram cannot be rendered or a link is followed. A change from outside never overwrites a change of the user, nor the other way round (A9).

### BR-003: Editing view per tab

A note opens in the facade. A tab keeps the editing view the user switches it to (UC-005), also after a restart.

### BR-004: Rendering

A note is rendered as the GitHub and GitLab web interfaces show its note format (C-003, C-004). Code blocks are highlighted for the language they name; PlantUML and Mermaid diagrams written in the note (C-005) are shown as pictures. Code and diagrams follow the colors of the active theme (FR-038). Rendering needs no network connection (NFR-014). An image from a web address is loaded and shown as GitHub shows it; without a network connection a placeholder names its address. Loading it sends no note content (NFR-015). The text of a note is readable at once; a diagram of up to 200 elements appears within 2 seconds (NFR-004).

### BR-005: Local links

A local link names the place of a note or file of the workspace relative to the note that holds it, as GitHub and GitLab read it, with or without the extension of the note format. It may name a heading of the note, and a link with only a heading points into the same note. A relative link that leads outside the workspace is not a local link and is not followed; a link to a file on this computer by its full place, as a large file can be linked (UC-005 A19), is followed like a link to another file (BR-007).

### BR-006: Following links

A link is followed by a click with the Ctrl key held (the Cmd key on macOS), in both editing views; a plain click places the cursor. Pointing at a link shows where it leads and how to follow it.

### BR-007: Other links

A web address opens in the default browser after the user agreed once in the session (A4). A file of the workspace that is not a note opens with the program the system uses for that kind of file.

### BR-008: Tabs

At most 100 tabs are open in a workspace. A new tab is placed right of the active tab. Opening one more than 100 closes the tab that was used longest ago. Closing a tab first stores any change autosave has not stored yet (NFR-005), so closing never loses a change.

### BR-009: Changes from outside

A change to an open note from another program, from git, or from an agent appears in its tab within 1 second (NFR-008). The tab keeps its reading position as far as the new content allows. The agent cursor that shows where an agent reads or writes belongs to UC-018 and UC-019.

### BR-010: Note properties

The properties of the active note are its place in the workspace with its file name, its note format, its size, and when it was last saved (FR-019).
