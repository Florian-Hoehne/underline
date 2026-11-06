# Use Case: Create Note

## Overview

**Use Case ID:** UC-003  
**Use Case Name:** Create Note  
**Primary Actor:** User  
**Goal:** The user has a new note with the title they chose, in the folder and note format they chose, open in a tab and ready to write in.  
**Trigger:** The user asks to create a new note.  
**Status:** Draft  

**Requirements:** [FR-003, FR-004, FR-018, FR-021, FR-071, NFR-006, C-002](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user asks to create a new note.
2. The system asks for the title of the note, with the folder of the note in the active tab preselected (the top folder when no tab is open) and the default format of the workspace preselected as note format.
3. The user enters a title, keeps or changes the folder and the note format, and confirms.
4. The system checks that the title is allowed, that the folder lies inside the workspace, and that the folder holds no note with that title.
5. The system creates the folder if it does not exist yet and creates the note, holding only its title as heading, in the chosen note format.
6. The system lists the note in the note overview as the most recently opened note, opens it in a tab in the facade with the cursor below the title, and reports in the status bar that the note was created.

## Alternative Flows

### A1: Title not allowed

**Trigger:** The title is empty, starts with a dot, holds a character that a file name cannot hold, or is too long (step 4)  
**Flow:**

1. The system tells the user why the title is not allowed and keeps what they entered.
2. The user changes the title.
3. Use case continues at step 4.

### A2: Note exists

**Trigger:** The chosen folder already holds a note with that title, in either note format and whatever its upper and lower case (step 4)  
**Flow:**

1. The system tells the user that the note exists and offers to open it instead.
2. The user changes the title or the folder.
3. Use case continues at step 4.

### A3: Open the existing note

**Trigger:** The user accepts the offer to open the existing note (A2 step 1)  
**Flow:**

1. The system opens the existing note in a tab, unchanged.
2. Use case ends.

### A4: Folder not allowed

**Trigger:** The chosen folder lies outside the workspace, is hidden, or is a workspace of its own (step 4)  
**Flow:**

1. The system tells the user why the folder cannot take the note and keeps what they entered.
2. The user changes the folder.
3. Use case continues at step 4.

### A5: Note cannot be created

**Trigger:** The system cannot create the folder or the note, for example for missing permission or a full disk (step 5)  
**Flow:**

1. The system reports in the status bar that the note could not be created and why.
2. The system removes the folders it created for the note.
3. Use case ends.

### A6: User cancels

**Trigger:** The user cancels instead of confirming (step 3, A1 step 2, A2 step 2, A4 step 2)  
**Flow:**

1. The system closes the dialog without creating anything.
2. Use case ends.

## Postconditions

### Success Postconditions

- The workspace holds the new note in the chosen folder and note format, holding only its title as heading.
- The note is open in a tab in the facade, listed first in the note overview, and remembered as the active tab.
- The default format of the workspace is unchanged.

### Failure Postconditions

- No note and no folder is created, and no existing note is changed.

## Business Rules

### BR-001: Title and file name

The title is stored as the file name of the note, with every run of spaces replaced by one dash and the extension of the note format added: the title "Getting Started" in Markdown is stored as `Getting-Started.md`. The note overview and the tab show the title with spaces again. Spaces at the start and the end of the title are dropped.

### BR-002: Allowed titles

A title is allowed when it is not empty, does not start with a dot, holds none of the characters `/ \ : * ? " < > |`, and has at most 250 characters.

### BR-003: One note per title and folder

A folder never holds two notes with the same title. Titles that differ only in upper and lower case count as the same, and so do two notes with the same title in different note formats.

### BR-004: Content of a new note

A new note holds only its title as heading, in the markup of its note format, like the Home note of a new workspace (UC-001 BR-003).

### BR-005: Note format

The default format of the workspace is preselected (FR-003). The user may choose the other note format for this one note; the default format of the workspace stays as it is.

### BR-006: Folder

The folder of the note in the active tab is preselected. The user may choose any other folder of the workspace or name a new one, also several levels deep; missing folders are created with the note. A folder outside the workspace, a hidden folder, and a folder that is a workspace of its own (UC-002 BR-007) cannot take a note.

### BR-007: Written at once

The note is written to the disk as soon as it is created, never half written (NFR-006), so it survives a crash and a git repository sees it at once. Later changes are saved by autosave (UC-005).
