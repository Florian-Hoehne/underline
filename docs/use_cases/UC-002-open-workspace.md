# Use Case: Open Workspace

## Overview

**Use Case ID:** UC-002  
**Use Case Name:** Open Workspace  
**Primary Actor:** User  
**Goal:** The user works on the notes of an existing directory, for example a git repository, with its notes listed and the notes they had open shown again.  
**Trigger:** The user asks to open a workspace, or starts Underline.  
**Status:** Draft  

**Requirements:** [FR-002, FR-004, FR-017, FR-018, FR-020, FR-070, FR-071, NFR-001, NFR-003, C-002, C-006](../requirements.md)

## Preconditions

_None — a workspace may be open or not._

## Main Success Scenario

1. The user asks to open a workspace.
2. The system lists the recently opened workspaces, the most recently opened first, and offers to choose any other directory.
3. The user chooses a recently opened workspace or a directory.
4. The system checks that the directory exists, can be read, is not the home directory, and does not lie inside a known workspace.
5. The system closes the workspace that was open before, if any, keeping its open tabs for the next time it is opened.
6. The system reads the workspace settings file of the directory, if there is one, and takes the default format from it or derives it from the notes.
7. The system lists the notes of the workspace in the note overview: the recently opened ones first, the most recent on top, then the others by title from A to Z.
8. The system opens the tabs the workspace had open the last time, leaving out notes that are gone, or, when it is opened for the first time, the first note.
9. The system remembers the workspace as the last opened one, shows its directory and, for a git repository with a git remote, the state of the last git sync in the status bar, and reports that the workspace was opened.

## Alternative Flows

### A1: Started with a directory

**Trigger:** The user starts Underline and names a directory to open (instead of step 1)  
**Flow:**

1. Use case continues at step 4 with that directory.

### A2: Started without a directory

**Trigger:** The user starts Underline without naming a directory (instead of step 1)  
**Flow:**

1. When a workspace was opened before and still exists, use case continues at step 4 with the last opened workspace.
2. Otherwise the system offers to open a workspace or to create one (UC-001); use case continues at step 2, or UC-001 starts.

### A3: Directory does not exist

**Trigger:** The chosen directory does not exist (step 4)  
**Flow:**

1. The system tells the user that the directory does not exist and offers to create a new workspace there instead; a recently opened workspace that is gone is removed from the list. This check comes before the others of step 4.
2. The user chooses another directory.
3. Use case continues at step 4.

### A4: Create a workspace instead

**Trigger:** The user accepts the offer to create a new workspace in the directory that does not exist (A3 step 1)  
**Flow:**

1. The system creates the workspace in that directory (UC-001, from its step 2, which also refuses a directory inside a workspace).
2. Use case ends.

### A5: Directory inside a workspace

**Trigger:** The chosen directory lies inside the directory of a known workspace (step 4)  
**Flow:**

1. The system tells the user that the directory lies inside that workspace and offers to open that workspace instead or to open the directory anyway.
2. The user chooses to open the directory anyway.
3. Use case continues at step 5. From now on the outer workspace leaves this directory out of its notes (BR-003).

### A6: Open the outer workspace

**Trigger:** The user accepts the offer to open the workspace the directory lies in (A5 step 1)  
**Flow:**

1. Use case continues at step 5 with the directory of that workspace.

### A7: Directory cannot be read

**Trigger:** The system cannot read the chosen directory, for example for missing permission (step 4)  
**Flow:**

1. The system tells the user that the directory cannot be read and why.
2. The user chooses another directory.
3. Use case continues at step 4.

### A8: Workspace settings file cannot be read

**Trigger:** The workspace settings file exists but cannot be read, for example after a faulty hand edit (step 6)  
**Flow:**

1. The system uses the user settings and the derived default format for this workspace and leaves the file as it is.
2. The system names the file and the problem in the status bar.
3. Use case continues at step 7.

### A9: Workspace already open

**Trigger:** The chosen directory, or the outer workspace chosen in A6, is the workspace that is open (step 4, A6)  
**Flow:**

1. The system keeps the workspace and its tabs as they are and closes the dialog.
2. Use case ends.

### A10: User cancels

**Trigger:** The user cancels instead of choosing (step 3, A3 step 2, A5 step 2, A7 step 2, A11 step 2)  
**Flow:**

1. The system closes the dialog; the workspace that was open stays open.
2. Use case ends.

### A11: Home directory

**Trigger:** The chosen directory is the user's home directory (step 4)  
**Flow:**

1. The system tells the user that the home directory cannot be a workspace, because it holds the user settings file and all of the user's files.
2. The user chooses another directory.
3. Use case continues at step 4.

### A12: Remembered tabs cannot be kept

**Trigger:** The system cannot write the Underline folder, for example in a directory the user may only read, or cannot read what it remembered there (step 8)  
**Flow:**

1. The system opens the workspace as if for the first time when it cannot read what it remembered, and sets the unreadable record aside.
2. The system says once in the status bar that the tabs and recently opened notes of this workspace cannot be remembered, or that what was remembered could not be read.
3. Use case continues at step 9.

### A13: Some files cannot be read

**Trigger:** A folder or note of the workspace cannot be read while the notes are listed (step 7)  
**Flow:**

1. The system lists the other notes and names in the status bar what it could not read.
2. Use case continues at step 8.

## Postconditions

### Success Postconditions

- The chosen directory is the open workspace, with its notes listed in the note overview, the recently opened ones first.
- The tabs the workspace had open the last time are open again, or, at the first opening, its first note.
- The workspace is remembered as the last opened one and listed first among the recently opened workspaces.
- The workspace that was open before is closed, and its open tabs are remembered.
- The notes and settings files of the directory are unchanged; only the Underline folder may have been added.

### Failure Postconditions

- When no workspace could be opened, the workspace that was open before stays open with its tabs.
- No note, settings file, or directory is created, changed, or removed.

## Business Rules

### BR-001: Opening changes nothing

Opening a workspace never adds, changes, or removes a note or a settings file in its directory; no workspace settings file is written, one appears only when the user changes a workspace setting (UC-010). The only thing opening adds is the Underline folder, in which the system remembers the open tabs and recently opened notes of the workspace; a git repository ignores that folder by itself, so it never shows as a change.

### BR-002: Derived default format

A workspace whose settings file names no default format gives new notes the note format most of its notes have. On a tie, or when it holds no notes, the default format is Markdown.

### BR-003: Which files are notes

Every Markdown (`.md`) and AsciiDoc (`.adoc`) file in the workspace directory and its folders is a note, except hidden files, files in hidden folders, files the git repository of the workspace is told to ignore, and files in a folder that is a known workspace of its own (BR-007). So every note belongs to exactly one workspace. Each note keeps the note format of its file extension (FR-004).

### BR-004: Titles unique per folder

Two notes in the same folder never have the same title; notes in different folders may. When a title occurs more than once in the workspace, the note overview shows the folder next to it.

### BR-005: First note

A workspace opened for the first time opens its Home note in a tab. Without a Home note it opens the README note of the top folder; without either it opens no tab.

### BR-006: Recently opened workspaces

The system remembers every workspace the user opens or creates, with when it was last opened. A workspace whose directory is gone is no longer listed. The tabs and recently opened notes of a workspace are remembered in its Underline folder and go wherever the directory goes.

### BR-007: Known workspaces

A directory is a known workspace when the system remembers it (BR-006), or when it holds a workspace settings file or an Underline folder. The user settings file in the home directory makes no workspace, and the home directory itself can never be a workspace.

### BR-008: Order of the note overview

The note overview lists the recently opened notes first, the most recently opened on top, and then all other notes by title from A to Z. A title that occurs more than once is shown with its folder (BR-004).
