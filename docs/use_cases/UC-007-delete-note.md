# Use Case: Delete Note

## Overview

**Use Case ID:** UC-007  
**Use Case Name:** Delete Note  
**Primary Actor:** User  
**Goal:** A note the user no longer needs is gone from the workspace after they confirmed it, and can be restored for 30 days.  
**Trigger:** The user asks to delete a note.  
**Status:** Reviewed  

**Requirements:** [FR-006, FR-017, FR-018, NFR-006, NFR-011](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).
- The note is listed in the note overview.

## Main Success Scenario

1. The user asks to delete a note, from the context menu of the note in the note overview or of its tab, or with the Delete key for the note chosen in the note overview.
2. The system asks the user to confirm, naming the note and how many links in how many other notes lead to it and will lead nowhere afterwards, and warns when the note is the Home note of the workspace (BR-010).
3. The user confirms.
4. The system saves the note if it has unsaved changes and moves it with its content into the deleted notes of the workspace, noting its place and when it was deleted.
5. The system closes the tab of the note if one is open, removes the note from the note overview and from the remembered tabs and recently opened notes, and reports in the status bar that the note was deleted, with an offer to undo it.

## Alternative Flows

### A1: User cancels

**Trigger:** The user cancels instead of confirming (step 3)  
**Flow:**

1. The system closes the question without changing anything.
2. Use case ends.

### A2: Note is linked by a todo

**Trigger:** A todo links to the note (step 2)  
**Flow:**

1. The system says that the note cannot be deleted while todos link to it and names those todos.
2. Use case ends.

### A3: Note cannot be deleted

**Trigger:** The system cannot save, move, or remove the note, for example for missing permission or because another program holds the file (step 4)  
**Flow:**

1. The system reports in the status bar that the note could not be deleted and why.
2. The note stays in the workspace as it was, and its tab stays open.
3. Use case ends.

### A4: Undo right after deleting

**Trigger:** The user chooses to undo in the status bar report (step 5), which offers it until another report replaces it  
**Flow:**

1. The system restores the note as in A5 and opens it again in a tab if it was open.
2. Use case ends.

### A5: Restore a deleted note

**Trigger:** The user opens the deleted notes from the end of the note overview and chooses to restore one (any time)  
**Flow:**

1. The system shows the deleted notes of the workspace, most recently deleted first, with their title, folder, and when they were deleted.
2. The user chooses a note and asks to restore it.
3. The system puts the note back at its place with its content, creating its folder if it is gone, lists it in the note overview, and removes it from the deleted notes.
4. Use case ends.

### A6: Place of a restored note is taken

**Trigger:** The folder of the note to restore holds a note with the same file name by now (A5 step 3)  
**Flow:**

1. The system says that a note with that title exists and asks for another title or folder, as when renaming (UC-006 BR-001).
2. The user enters a title and folder and confirms, or cancels and the note stays deleted.
3. Use case continues at A5 step 3 with the new title and folder.

### A7: Empty the deleted notes

**Trigger:** The user asks to empty the deleted notes, or to remove one of them for good (A5 step 1)  
**Flow:**

1. The system asks the user to confirm, naming how many notes will be removed for good.
2. When the user confirms, the system removes them for good.
3. Use case ends.

### A8: Deleted notes cannot be kept

**Trigger:** The Underline folder of the workspace cannot be written, so the note cannot be kept among the deleted notes (step 4)  
**Flow:**

1. The system says that the note cannot be restored afterwards and asks the user to confirm deleting it for good.
2. When the user confirms, the system saves the note if needed and removes it for good; use case continues at step 5 without the offer to undo.
3. When the user cancels, nothing is changed; use case ends.

## Postconditions

### Success Postconditions

- The note is no longer in the workspace and not listed in the note overview; when the workspace is a git repository, git sees it as deleted.
- The note is listed in the deleted notes of the workspace with its content, place, and the time it was deleted.
- No tab shows the note, and it is neither among the remembered tabs nor among the recently opened notes.
- Links to the note in other notes are unchanged.

### Failure Postconditions

- The note is in the workspace as it was, and no other file is changed.

## Business Rules

### BR-001: Confirmation

A note is deleted only after the user confirmed it (FR-006). The question names the note and the number of links in other notes of the workspace that lead to it (UC-006 BR-003 says which links lead to a note); with none, it says so.

### BR-002: Links stay

Links to the deleted note are not changed. Following one afterwards offers to create the note (UC-004 A3), and restoring the note makes them lead to it again.

### BR-003: Files of the note

Only the note is deleted. Images and other files it links to stay where they are, whether or not other notes use them.

### BR-004: Deleted notes of the workspace

Deleted notes are kept in the workspace, in the Underline folder, on this computer only: git ignores them, so for git and for other computers the note is deleted. They are not listed in the note overview, not searched, and not read or written by agents.

### BR-005: Thirty days

A deleted note is kept for 30 days. Notes deleted longer ago are removed for good when the workspace is opened. The user can remove them earlier (A7).

### BR-006: Restored as it was

A restored note comes back at its place, with the content it had when it was deleted, under the same title, unless that place is taken (A6). Its tab, remembered tabs, and recently opened notes are not restored, except by the undo right after deleting (A4).

### BR-007: Delete key

The Delete key deletes the note chosen in the note overview while the note overview has the keyboard focus; in a tab it deletes text as always.

### BR-008: Never half done

The note is in the workspace or in the deleted notes, never in neither and never half written (NFR-006): it is removed from the workspace only once it is completely kept among the deleted notes.

### BR-009: Todo links

A note that a todo links to cannot be deleted until the todo links are removed (entity model, TODO_LINK).

### BR-010: Home note

The Home note of the workspace (UC-001 BR-003) is the start page of a GitHub wiki. When it is deleted, the question warns that the wiki then has no start page; the user may still confirm.

### BR-011: One note at a time

One note is deleted at a time (FR-006); there is no deleting of several notes or of folders at once.

