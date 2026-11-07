# Use Case: Rename Note

## Overview

**Use Case ID:** UC-006  
**Use Case Name:** Rename Note  
**Primary Actor:** User  
**Goal:** The note has the title and the folder the user chose, and every local link to it, in other notes and in todos, still leads to it.  
**Trigger:** The user asks to rename a note.  
**Status:** Draft  

**Requirements:** [FR-005, FR-004, FR-017, FR-018, NFR-006, NFR-007, NFR-011, NFR-013](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).
- The note is listed in the note overview.

## Main Success Scenario

1. The user asks to rename a note, from the context menu of the note in the note overview or of its tab, or with F2 for the note in the active tab.
2. The system asks for the new title and folder, with the current title selected and the current folder preselected, keeps the note format, and shows the file name the note will get while the user types.
3. The user enters a title, keeps or changes the folder, and confirms.
4. The system checks that the title is allowed, that the folder can take a note, and that the folder holds no other note with that file name.
5. The system saves the note if it has unsaved changes, renames and moves it without ever replacing a file that is there, and changes its first heading to the new title when it showed exactly the old title.
6. The system updates every local link to the note in the other notes of the workspace and every todo link to it; when the note moved to another folder, it also updates the local links in the note itself so they lead to the same notes and files as before.
7. The system shows the new title in the note overview and in the tab, keeps the note open where it was, and reports in the status bar that the note was renamed and how many links in how many notes were updated.

## Alternative Flows

### A1: Title not allowed

**Trigger:** The title breaks a rule of UC-003 BR-002 (step 4)  
**Flow:**

1. The system tells the user why the title is not allowed and keeps what they entered.
2. The user changes the title.
3. Use case continues at step 4.

### A2: Note exists

**Trigger:** The chosen folder already holds another note with that file name, in either note format and whatever its upper and lower case (step 4), or such a file appears there before the note is renamed (step 5)  
**Flow:**

1. The system tells the user that a note with that title exists in the folder.
2. The user changes the title or the folder.
3. Use case continues at step 4.

### A3: Folder not allowed

**Trigger:** The chosen folder lies outside the workspace, is hidden, is ignored by the git repository of the workspace, or is a workspace of its own (step 4)  
**Flow:**

1. The system tells the user why the folder cannot take the note and keeps what they entered.
2. The user changes the folder.
3. Use case continues at step 4.

### A4: Nothing changed

**Trigger:** The user confirms the current title and folder unchanged (step 3)  
**Flow:**

1. The system closes the dialog and changes nothing.
2. Use case ends.

### A5: Note cannot be renamed

**Trigger:** The system cannot rename or move the note or create its new folder, for example for missing permission, a full disk, or because another program holds the file (step 5)  
**Flow:**

1. The system reports in the status bar that the note could not be renamed and why.
2. The system removes the folders it created for the note and leaves the note and all links as they were.
3. Use case ends.

### A6: A linking note cannot be updated

**Trigger:** A note that links to the renamed note cannot be read or written, for example for missing permission (step 6)  
**Flow:**

1. The system updates all other links and keeps the note renamed.
2. The system reports in the status bar which notes still hold links to the old title.
3. Use case continues at step 7.

### A7: Linking note is open with unsaved changes

**Trigger:** A note that links to the renamed note is open in a tab and has changes that are not saved yet (step 6)  
**Flow:**

1. The system updates the links in the open tab, keeps the user's changes and the cursor, and saves the note with the next autosave (UC-005).
2. Use case continues at step 6 with the next note.

### A8: User cancels

**Trigger:** The user cancels instead of confirming (step 3, A1 step 2, A2 step 2, A3 step 2)  
**Flow:**

1. The system closes the dialog without changing anything.
2. Use case ends.

## Postconditions

### Success Postconditions

- The note has the new title as file name, in the chosen folder, in its note format, with its content unchanged except for its first heading (BR-004) and, after a move, its local links (BR-005).
- Every local link and todo link that led to the note leads to it under its new name, at the same heading.
- A tab that showed the note shows it under the new title, and the remembered tabs and recently opened notes name the new title.

### Failure Postconditions

- The note keeps its title, folder, and content, no link is changed, and no other file is changed or replaced.

## Business Rules

### BR-001: Title, file name, and folder

The rules for titles, file names, and folders of a new note hold for the new title and folder (UC-003 BR-001, BR-002, BR-006): missing folders are created, and a folder that cannot take a new note cannot take a renamed one. A rename that changes only upper and lower case of the title is allowed; the note does not count as its own namesake (UC-003 BR-003).

### BR-002: Note format stays

Renaming never changes the note format: the extension of the file name stays `.md` or `.adoc` (FR-004). Converting a note to the other note format is not renaming.

### BR-003: Links that are updated

Every local link (UC-004 BR-005) in the notes of the workspace that leads to the renamed note is changed to lead to it under its new title and folder, in the markup of the note format of the linking note, keeping the heading it names and whether it was written with or without the extension. When the text of the link shows exactly the old title, it shows the new title; any other link text stays as it is. Links to other notes and files, and anything that is not a link, stay unchanged, so the linking notes change only where the links are (NFR-007). Links in places the system cannot see, such as other repositories or web pages, cannot be updated.

### BR-004: First heading

When the first heading of the note shows exactly the old title, as it does in a new note (UC-003 BR-004), it is changed to the new title. Any other first heading, and the rest of the content, stays as it is.

### BR-005: Links of a moved note

When the note moves to another folder, the notes and files it links to stay where they are. Its local links, which name places relative to the note, are changed so they lead to the same notes and files from the new folder; links to headings of the note itself, web addresses, and links by full place stay unchanged.

### BR-006: Todo links

Todo links follow the renamed note, so a todo stays linked to it (entity model, TODO_LINK).

### BR-007: Report

The status bar reports the new title and how many links in how many notes were updated, so the user sees what changed without being asked beforehand. When the workspace is a git repository, the rename and the updated notes show up as changes to commit (UC-012).

### BR-008: Never half done

The note is renamed as a whole or not at all (NFR-006): its old file disappears only when the new one is complete. Each linking note is written whole, as autosave writes it. When some linking notes cannot be written (A6), the rename stays, and the status bar names those notes so the user can fix their links.
