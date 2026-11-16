# Use Case: Read Note

## Overview

**Use Case ID:** UC-004  
**Use Case Name:** Read Note  
**Primary Actor:** User  
**Goal:** The user reads a note in a tab, rendered with its code, diagrams, and images, finds their way through it and to the notes it links to, and sees how it is stored.  
**Trigger:** The user chooses a note in the note overview, follows a local link, or a workspace opens with remembered tabs.  
**Status:** Reviewed  

**Requirements:** [FR-004, FR-017, FR-018, FR-019, FR-027, FR-030, FR-035, FR-036, FR-037, FR-038, FR-039, FR-040, FR-041, FR-042, FR-043, FR-046, FR-071, FR-076, FR-078, FR-079, FR-086, FR-090, FR-091, FR-092, FR-096, FR-097, FR-098, NFR-004, NFR-005, NFR-007, NFR-008, NFR-014, NFR-015, NFR-018, NFR-019, C-003, C-004, C-005](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user chooses a note in the note overview.
2. The system opens the note in a new tab in the facade, placed right of the active tab, makes it the active tab, and lists it first in the note overview.
3. The system shows the note as its note format is shown on GitHub and GitLab: headings, paragraphs, lists, checklists, tables, links, and images, code blocks highlighted for their language, PlantUML and Mermaid diagrams as pictures, formulas typeset as mathematics (BR-019), and the front matter of a Markdown note as a table, all in the colors of the active theme.
4. The system shows in the tool panel the note properties (its place in the workspace, note format, size, last change, text statistics, and note attributes), its outline, its code index, its figure index, its backlinks, its outgoing links, and its tags, and beside the editor a minimap of the note.
5. The user reads the note: they scroll, jump to a heading, a code block, a figure, or a link through the tool panel or to a place through the minimap, fold and expand sections, and enlarge diagrams and images.
6. The system keeps the reading position and the folded sections of the tab while it is open, and remembers the open tabs, their panes, and the active tab of each pane of the workspace.

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

### A12: Go to note

**Trigger:** The user opens go to note with Ctrl+P (Cmd+P on macOS) instead of choosing a note in the note overview (step 1)  
**Flow:**

1. The user types a part of the title or the folder of a note.
2. While the user types, the system lists the notes that match (BR-014), each with its title and its folder, the best match first and chosen.
3. The user chooses a note with the arrow keys and Enter, or with a click.
4. Use case continues at step 2 with that note, or at A1 when it is open already.

### A13: No note matches in go to note

**Trigger:** No note matches what the user typed in go to note (step 1)  
**Flow:**

1. The system says that no note matches and offers to create a note with the typed text as its title.
2. When the user accepts, the system creates the note (UC-003) with that title filled in; use case ends.
3. Otherwise use case continues at A12 step 1.

### A14: Go to a backlink or an outgoing link

**Trigger:** The user chooses a backlink or an outgoing link in the tool panel (step 5)  
**Flow:**

1. For a backlink, the system opens the linking note with the cursor on the link; use case continues at step 2 with that note, or at A1 when it is open already.
2. For an outgoing link, the system puts the cursor on the link in the active note; with the Ctrl key held (the Cmd key on macOS), it follows the link as in A2 to A4.
3. Use case continues at step 5.

### A15: Search by a tag of the note

**Trigger:** The user chooses a tag of the note in the tool panel (step 5)  
**Flow:**

1. The system searches the workspace for the notes that carry that tag (UC-008 A8).
2. Use case ends.

### A16: Go to line

**Trigger:** The user asks to go to a line with Ctrl+G (Cmd+G on macOS) or the command palette (UC-023) (step 5)  
**Flow:**

1. The system asks for a line number, showing the number of lines of the note; the user may add a column after a colon, such as `120:8`.
2. The user enters the number and confirms.
3. The system moves the cursor to that line and column of the markup and puts it in view, by the rules of BR-017.
4. Use case continues at step 5.

### A17: Line number not valid

**Trigger:** The user enters something that is no line number when going to a line (step 5)  
**Flow:**

1. The system says that a line number is expected, keeps what the user typed, and moves nothing.
2. When the user corrects it, the system goes on as in A16 step 3; when the user cancels, nothing moves.
3. Use case continues at step 5.

### A18: Open in the split view

**Trigger:** The user opens a note to the side, from the note overview, go to note, a link, or the tab of the active note, or splits the middle of the window with Ctrl+\ (Cmd+\ on macOS) (step 1)  
**Flow:**

1. When the split view is not shown, the system divides the middle of the window into two panes side by side; the open tabs stay in the left pane.
2. The system opens the note in a tab of the other pane, or makes its tab there the active tab, and makes that pane the active pane (BR-018).
3. Use case continues at step 3.

### A19: The same note on both sides

**Trigger:** The note the user opens to the side is the note of the active tab (step 1)  
**Flow:**

1. The system shows the note in the other pane in the other editing view, so its source view and its facade are side by side; each side follows every change made on the other at once (UC-005 BR-016).
2. Use case continues at step 4.

### A20: End the split view

**Trigger:** The user closes the last tab of a pane, or asks to end the split view (step 5)  
**Flow:**

1. The system shows one pane again: the tabs of the right pane join the left pane after its own tabs, and a note open on both sides keeps only the tab of the left pane.
2. Use case continues at step 4 with the active note.

### A21: Formula cannot be typeset

**Trigger:** A formula of the note has an error in its notation (step 3)  
**Flow:**

1. The system shows the source of that formula with the error in its place and renders the rest of the note.
2. Use case continues at step 4.

## Postconditions

### Success Postconditions

- The note is open in its own tab, which is the active tab, and is listed first in the note overview.
- The open tabs, their panes, order and editing views, and the active tab of each pane are remembered for the next time the workspace is opened. Tabs opened again when a workspace opens (UC-002 step 8) do not change the order of the note overview.
- The note and its file are unchanged.

### Failure Postconditions

- No file is created, changed, or removed, except the copy of an outside version kept in A9; a note that cannot be read gets no tab.

## Business Rules

### BR-001: One tab per note

A note is open in at most one tab of each pane of its workspace. Opening a note that is open already in the active pane makes its tab the active tab; only opening it to the side (A18) gives it a tab in the other pane too.

### BR-002: Reading changes nothing

Opening, reading, and closing a note never changes its file (NFR-007), in either editing view, also when a diagram cannot be rendered or a link is followed. A change from outside never overwrites a change of the user, nor the other way round (A9).

### BR-003: Editing view per tab

A note opens in the facade. A tab keeps the editing view the user switches it to (UC-005), also after a restart.

### BR-004: Rendering

A note is rendered as the GitHub and GitLab web interfaces show its note format (C-003, C-004). Code blocks are highlighted for the language they name; PlantUML and Mermaid diagrams written in the note (C-005) are shown as pictures. Code, diagrams, and formulas follow the colors of the active theme (FR-038). The front matter of a Markdown note is shown as a table of its note attributes at the start of the note, as GitHub shows it (UC-024 BR-001). Rendering needs no network connection (NFR-014). An image from a web address is loaded and shown as GitHub shows it; without a network connection a placeholder names its address. Loading it sends no note content (NFR-015). The text of a note is readable at once; a diagram of up to 200 elements appears within 2 seconds (NFR-004).

### BR-005: Local links

A local link names the place of a note or file of the workspace relative to the note that holds it, as GitHub and GitLab read it, with or without the extension of the note format. It may name a heading of the note, and a link with only a heading points into the same note. A relative link that leads outside the workspace is not a local link and is not followed; a link to a file on this computer by its full place, as a large file can be linked (UC-005 A19), is followed like a link to another file (BR-007).

### BR-006: Following links

A link is followed by a click with the Ctrl key held (the Cmd key on macOS), in both editing views; a plain click places the cursor. Pointing at a link shows where it leads and how to follow it.

### BR-007: Other links

A web address opens in the default browser after the user agreed once in the session (A4). A file of the workspace that is not a note opens with the program the system uses for that kind of file.

### BR-008: Tabs

At most 100 tabs are open in a workspace, both panes of the split view together. A new tab is placed right of the active tab. Opening one more than 100 closes the tab that was used longest ago. Closing a tab first stores any change autosave has not stored yet (NFR-005), so closing never loses a change.

### BR-009: Changes from outside

A change to an open note from another program, from git, or from an agent appears in its tab within 1 second (NFR-008). The tab keeps its reading position as far as the new content allows. The agent cursor that shows where an agent reads or writes belongs to UC-018 and UC-019.

### BR-010: Note properties

The note properties of the active note are its place in the workspace with its file name, its note format, its size, when it was last saved (FR-019), its text statistics (BR-020), and its note attributes, which can be changed there (UC-024).

### BR-011: Enlarged view

A double click on an image or a diagram, or the magnifier shown at its corner while pointing at it, opens it enlarged
over the whole window (FR-039). There it can be zoomed with the mouse wheel or the keys for larger and smaller,
moved by dragging, and fitted to the window again. The escape key, a click beside it, or its close button closes
it; the note is then shown at the same place as before. A single click puts the cursor into it, as in UC-005 BR-008.

### BR-012: Folding

A section is a heading with everything below it up to the next heading of the same or a higher level; folding it
hides everything but its heading, its own sections included (FR-027). An arrow before each heading folds and expands
its section, shown while pointing at the heading and always at a folded one, in the facade before the heading and in
the source view beside its line number; the keyboard folds and expands the section the cursor is in. A folded heading
shows that something is hidden behind it. When the cursor moves into a folded section, the section expands. Both
editing views of a tab share the folded sections, which stay folded while the tab is open (step 6).

### BR-013: Minimap

The minimap is a narrow strip beside the editor that shows the whole note scaled down, each line or block as a bar in
the shape and color of what it is, headings stronger than text, and code, tables, and figures as areas; the visible
part is marked (FR-030). A click puts that place in view, and dragging moves the view along. It shows the note as the
active editing view shows it, folded sections left out. A switch hides it and shows it again, for all tabs.

### BR-014: Go to note

A note matches in go to note when every word typed occurs in its title or its folder, in any order, also as a part of a word; upper and lower case do not matter, accents do, as in the search of the workspace (UC-008 BR-002). Notes whose title starts with what was typed come first, then those with a word of the title that starts with it, then the others; within each group, the order of the note overview applies (UC-002 BR-008). With an empty field, go to note lists the notes in the order of the note overview. The notes that match appear within 200 milliseconds of a key in a workspace of 10,000 notes (NFR-018). Ctrl+P (Cmd+P on macOS) opens go to note where the input mode gives that key no meaning of its own, as for the find field (UC-008 BR-009); the command palette reaches it in every input mode (UC-023 BR-007).

### BR-015: Backlinks and outgoing links

The backlinks of the active note are the local links (BR-005) in the other notes of the workspace that lead to it or to one of its headings, grouped by the note that holds them, in the order of the note overview, each with its line and the link marked (FR-078). A link of a note to itself is no backlink. The outgoing links of the active note are all its links in the order of the note: local links, links to other files of the workspace, and web addresses, each with its text and where it leads (FR-079). A local link whose note, file, or heading does not exist is a broken link and is marked as such; whether a web address can be reached is not checked. Text in code blocks, diagrams, and formulas holds no links. For notes open in a tab, the text of the tab counts, with changes not yet saved. Both lists follow every stored change of any note within 2 seconds in a workspace of 10,000 notes (NFR-019); the section of each shows how many there are.

### BR-016: Tags

A note carries a tag (FR-091) in these places:

- In the text of a Markdown note: `#` followed directly by the tag, such as `#project` or `#project/alpha`, where the `#` does not follow a letter, a digit, or `&`. A `#` followed by a space starts a heading, not a tag. Code, link addresses, diagrams, and formulas hold no tags.
- In the note attribute `tags` of a Markdown or an AsciiDoc note (UC-024 BR-005). An AsciiDoc note carries tags only there, because `#` marks highlighted text in AsciiDoc.

A tag holds letters, digits, `_`, `-`, and `/`, and at least one character that is not a digit, so `#1` or `#2026` is no tag. Upper and lower case do not matter: `#Project` and `#project` are the same tag. A `/` makes a tag a part of the tag before it, so `project/alpha` also counts as `project` when searching (UC-008 BR-012). A tag in the text is shown as the note format shows it on GitHub and GitLab, as plain text (C-003). The tool panel lists the tags of the active note by name; they follow every stored change within 2 seconds (NFR-019).

### BR-017: Go to line

A line number counts the lines of the markup of the note, as the line numbers of the source view do, starting at 1 (FR-086). In the source view, the cursor goes to the start of that line, or to the column given after a colon; in the facade, it goes to the block that holds that line, at the place of that line within the block where the facade shows it. A number beyond the last line goes to the last line, and a column beyond the end of the line goes to its end. A folded section that holds the line is expanded (BR-012). Ctrl+G (Cmd+G on macOS) asks for the line where the input mode gives that key no meaning of its own, as for the find field (UC-008 BR-009); the Vim input mode has its own `:N` (UC-005 BR-009).

### BR-018: Split view

The split view divides the middle of the window into a left and a right pane, each with its own tabs and its own active tab (FR-096); they start equally wide, and the border between them can be dragged. The pane the user last worked in is the active pane: the tool panel shows its active note, and a note chosen in the note overview, in go to note, or by a followed link opens there. Opening to the side opens the note in the other pane. A note may be open in both panes at once, one tab in each, which is how its source view and its facade are seen side by side (FR-097). The panes, their tabs, and their active tabs are remembered like the tabs (step 6); after a restart, the left pane is the active pane. The focus mode (UC-010 A12) hides the side panels but keeps both panes.

### BR-019: Formulas

A formula is written in the notation GitHub reads (FR-098, C-003): in a Markdown note inline between `$` and `$` or between `` $` `` and `` `$ ``, and as a block between two lines `$$` or in a code block of the language `math`, decided as GitHub decides whether a `$` starts a formula. In an AsciiDoc note, a formula is written inline as `latexmath:[...]` and as a block marked `[latexmath]`, or with `stem` when the note sets the document attribute `stem` to `latexmath` (C-004). The facade shows it typeset, inline in its line or centered as a block, in the colors of the active theme; the source view shows its markup. Typesetting needs no network connection (NFR-014). A formula with an error is shown as its source with the error (A21).

### BR-020: Text statistics

The text statistics of a note count the text the facade shows (FR-090): its markup, front matter, link addresses, and the sources of diagrams and formulas are not counted; the text of code blocks is. A word is a run of letters and digits, with `'` and `-` inside it belonging to the word. Characters are counted with spaces. The reading time is the number of words divided by 200, rounded up to whole minutes, and at least one minute for a note with words. While text is selected, the note properties also show the words and characters of the selection. The statistics follow every change within 1 second.

