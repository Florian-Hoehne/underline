# Use Case: Edit Note

## Overview

**Use Case ID:** UC-005  
**Use Case Name:** Edit Note  
**Primary Actor:** User  
**Goal:** The user changes a note in the source view or in the facade, with the keys and tools they prefer, and every change is stored without them having to save.  
**Trigger:** The user types, formats, pastes, or drops something into the note of the active tab.  
**Status:** Draft  

**Requirements:** [FR-008, FR-009, FR-010, FR-011, FR-012, FR-021, FR-023, FR-024, FR-025, FR-026, FR-031, FR-033, FR-034, FR-044, NFR-002, NFR-005, NFR-006, NFR-007, NFR-011, NFR-012](../requirements.md)

## Preconditions

- A note is open in the active tab (UC-004).

## Main Success Scenario

1. The user changes the note in the active tab: in the source view they edit its markup, in the facade they edit its rendered content.
2. The system shows every typed character at once, in either editing view.
3. In the facade the system stores each change in the markup of the note format, changing only the edited block, and shows the format indicator with the note format of the note.
4. At most 2 seconds after the last change the system stores the note on the local disk, never half written, and reports in the status bar that the note was saved.
5. The user goes on editing; use case continues at step 1 until the user leaves the note.

## Alternative Flows

### A1: Switch the editing view

**Trigger:** The user switches the tab between the source view and the facade (any step)  
**Flow:**

1. The system shows the note in the other editing view, with the cursor and the selection at the same place of the content.
2. Use case continues at step 1.

### A2: Format with the toolbar

**Trigger:** The user chooses a function of the formatting toolbar: heading, bold, italic, code, list, quote, link, image, or table (step 1)  
**Flow:**

1. The system applies it to the selected text, or inserts it at the cursor when nothing is selected, in the markup of the note format; in the source view the user sees that markup.
2. Use case continues at step 2.

### A3: Format-specific option

**Trigger:** The user chooses an option that only the note format offers, in the facade (step 1)  
**Flow:**

1. The system applies the option, for example a caption for an image or a table, an admonition block, or an id that other places can link to, in the markup of the note format.
2. Use case continues at step 2.

### A4: Edit a block the facade cannot format

**Trigger:** The user edits a block in the facade that the facade shows as its markup (step 1)  
**Flow:**

1. The system lets the user edit the markup of that block in place, inside its frame, and keeps the content around it formatted.
2. Use case continues at step 2.

### A5: Undo and redo

**Trigger:** The user undoes or redoes a change (any step)  
**Flow:**

1. The system takes back the last change of the tab, or applies the last undone change again, in either editing view.
2. Use case continues at step 4.

### A6: Vim input mode

**Trigger:** The Vim input mode is chosen (UC-010) and the user edits (step 1)  
**Flow:**

1. The system follows the Vim keys of BR-009 in both editing views and shows the active Vim mode and its most important commands in the status bar.
2. When the user enters `:q`, the system closes the tab (UC-004 A5); `:w` is accepted and stores the note at once.
3. Use case continues at step 2.

### A7: Nano input mode

**Trigger:** The Nano input mode is chosen (UC-010) and the user edits (step 1)  
**Flow:**

1. The system follows the Nano keys of BR-010 in both editing views and shows the most important Nano commands in the status bar.
2. Use case continues at step 2.

### A8: Copy on select

**Trigger:** Copy on select is switched on (UC-010) and the user selects text, an image, or another element (step 1)  
**Flow:**

1. The system puts the selection on the clipboard as BR-011 says, without a further command.
2. Use case continues at step 1.

### A9: Drop an image or a file

**Trigger:** The user drags an image or another file from outside the workspace into the note (step 1)  
**Flow:**

1. The system copies the file into the folder of the note, keeping its name.
2. The system inserts it at the drop position: an image as an image, another file as a link, both by the name of the copy.
3. Use case continues at step 2.

### A10: Dropped file name is taken

**Trigger:** The folder of the note already holds a file with the name of the dropped file (A9 step 1)  
**Flow:**

1. When that file has the same content, the system copies nothing and inserts the existing file.
2. Otherwise the system copies the file under its name with the next free number added (`diagram-2.png`) and says so in the status bar.
3. Use case continues at A9 step 2.

### A11: Drop a file of the workspace

**Trigger:** The dropped file lies inside the workspace already (step 1)  
**Flow:**

1. The system copies nothing and inserts the file at the drop position by its place relative to the note, as an image or as a link.
2. Use case continues at step 2.

### A12: Drop a link

**Trigger:** The user drags a link, for example from a browser, into the note (step 1)  
**Flow:**

1. The system inserts it at the drop position as a link with the text the link carries, or with its address when it carries none.
2. Use case continues at step 2.

### A13: Something that cannot be dropped

**Trigger:** The user drops a folder, or a file that cannot be read (step 1)  
**Flow:**

1. The system says in the status bar why it cannot be dropped and changes nothing.
2. Use case continues at step 1.

### A14: Note cannot be stored

**Trigger:** The system cannot store the note, for example because the disk is full, the permission is gone, or the drive was removed (step 4)  
**Flow:**

1. The system keeps every change in the tab, marks the tab as not stored, and says in the status bar why; the message stays until the note is stored.
2. The system tries again with every further change and every 10 seconds.
3. When the user closes the tab or Underline while the note is not stored, the system asks first whether to close anyway and lose the changes.
4. Once the note is stored, the system removes the mark and reports that it was saved; use case continues at step 5.

### A15: Note deleted outside

**Trigger:** The user edits a note that was deleted outside while it was open (UC-004 A8) (step 1)  
**Flow:**

1. The system stores the note again at its place, removes the mark as deleted, lists it in the note overview again, and says so in the status bar.
2. Use case continues at step 5.

### A16: Note changed outside

**Trigger:** The note is changed outside Underline while a change of the user is not stored yet (step 4)  
**Flow:**

1. The system keeps both versions (UC-004 A9).
2. Use case continues at step 4.

## Postconditions

### Success Postconditions

- Every change is stored on the local disk at most 2 seconds after it was made, in the markup of the note format.
- Blocks the user did not edit are unchanged, byte for byte.
- Dropped files are in the folder of the note and the note refers to them by their place.

### Failure Postconditions

- No change is lost silently: a change that cannot be stored stays in the tab, marked, until it is stored or the user agrees to lose it.
- A note is never half written.

## Business Rules

### BR-001: Autosave

A change is stored at most 2 seconds after the last change (NFR-005), and at once when the tab is closed, Underline is closed, the workspace is switched, or `:w` is entered. A stored note holds either its previous or its new content, never part of both (NFR-006). The user never has to save by hand.

### BR-002: Format fidelity

An edit in the facade changes only the paragraph, list, table, or other block that was edited; every other block stays byte for byte as it was (NFR-007). Switching the editing view changes nothing.

### BR-003: Both editing views

Every function of this use case works the same in the source view and the facade: typing, formatting, undo, the input modes, copy on select, and dropping. A typed character appears within 16 milliseconds in notes of up to 10,000 lines (NFR-002). Every function of the toolbar can be reached by keyboard alone (NFR-011).

### BR-004: Undo

Each tab has its own history of changes, which it keeps across switches of the editing view until the tab is closed; it does not survive a restart. An undone or redone change is stored like any other change.

### BR-005: Formatting toolbar

The toolbar offers heading, bold, italic, code, list, quote, link, image, and table (FR-023). It writes the markup of the note format of the active note, so the same function gives Markdown in a Markdown note and AsciiDoc in an AsciiDoc note.

### BR-006: Format indicator

The facade always shows the note format of the active note (FR-011), so the user knows which markup and options apply.

### BR-007: Format-specific options

In the first release the facade offers for AsciiDoc notes: captions for images and tables, admonition blocks (note, tip, important, caution, warning), and ids that cross references can point to. Markdown notes have no options beyond those both formats share. A further note format brings its options with it (NFR-012).

### BR-008: Blocks shown as markup

A block the facade cannot format, such as raw HTML in Markdown or an AsciiDoc block it does not know, is shown as its markup in a frame and edited there as markup. It is stored exactly as the user leaves it.

### BR-009: Vim keys

The Vim input mode offers the normal, insert, and visual mode; motions, the operators delete, change, yank, and put; counts; undo and redo; search with `/` and `n`; `:q` to close the tab; and `:w` to store at once. The status bar shows the active mode and its most important commands (FR-026).

### BR-010: Nano keys

The Nano input mode offers the Nano keys to move, cut and paste lines, search, undo, and redo, and `Ctrl+X` to close the tab. The status bar shows the most important commands (FR-026).

### BR-011: Copy on select

While copy on select is on, a selection is put on the clipboard as soon as it is made (FR-031). From the facade it is put there both as markup of the note format and as formatted text, so that pasting into a note keeps the markup and pasting into another program keeps the formatting; from the source view as its markup. Copy on select can be switched off (FR-032, UC-010).

### BR-012: Dropped files and links

A dropped file from outside the workspace is copied into the folder of the note (A9); a file of the workspace is referred to where it is (A11). Files of the kinds PNG, JPEG, GIF, SVG, and WebP are inserted as images, all others as links. The note refers to the file by its place relative to the note, so the note and its files can move together and look the same on GitHub and GitLab.

### BR-013: Changes that cannot be stored

A change that cannot be stored is never dropped silently (A14). The tab shows that it is not stored, the status bar says why, and the system keeps trying until the change is stored or the user agrees to lose it.
