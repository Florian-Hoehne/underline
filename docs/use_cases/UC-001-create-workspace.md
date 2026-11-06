# Use Case: Create Workspace

## Overview

**Use Case ID:** UC-001  
**Use Case Name:** Create Workspace  
**Primary Actor:** User  
**Goal:** The user has a new, empty workspace in the default format of their choice, optionally as a git repository, and can start writing in its Home note.  
**Trigger:** The user asks to create a new workspace.  
**Status:** Draft  

**Requirements:** [FR-001, FR-072, NFR-014, C-002, C-006](../requirements.md)

## Preconditions

_None — no workspace or other state needs to exist before._

## Main Success Scenario

1. The user chooses a directory for the new workspace: a new directory or an existing empty one.
2. The system checks that the directory is new or empty.
3. The system asks for the default format, with Markdown preselected, and offers to create the workspace as a git repository.
4. The user confirms a default format and leaves the git repository option off.
5. The system creates the directory if it does not exist yet and records the workspace with its default format in the workspace settings file.
6. The system creates the Home note in the default format.
7. The system opens the workspace, lists the Home note in the note overview, opens it in a tab, and reports in the status bar that the workspace was created.

## Alternative Flows

### A1: Directory holds files

**Trigger:** The chosen directory is not empty (step 2)  
**Flow:**

1. The system tells the user that the directory already holds files and offers to open it as a workspace instead.
2. The user chooses another directory.
3. Use case continues at step 2.

### A2: Open the directory instead

**Trigger:** The user accepts the offer to open the non-empty directory as a workspace (step 2)  
**Flow:**

1. The system opens the directory as an existing workspace (UC-002).
2. Use case ends.

### A3: Create as git repository

**Trigger:** The user switches the git repository option on (step 4)  
**Flow:**

1. The system asks for the address of a git remote; the address is optional.
2. The user enters a git remote address or leaves it empty and confirms.
3. The system creates the directory if it does not exist yet, makes it a git repository, links the git remote when an address was given, and records the workspace with its default format in the workspace settings file.
4. Use case continues at step 6.

### A4: User cancels

**Trigger:** The user cancels instead of confirming (step 4)  
**Flow:**

1. The system closes the dialog without creating anything.
2. Use case ends.

### A5: Directory cannot be written

**Trigger:** The system cannot create the directory or write into it (step 5)  
**Flow:**

1. The system reports in the status bar that the workspace could not be created and why, for example missing permission.
2. The system removes whatever it had created for this workspace.
3. Use case ends.

## Postconditions

### Success Postconditions

- The workspace exists in the chosen directory with the chosen default format, recorded in its workspace settings file.
- The workspace holds the Home note in the default format, and it is open in a tab.
- When the user chose the git repository option, the workspace is a git repository, linked to the git remote when an address was given.
- The workspace is the open workspace and is remembered as the last opened one.

### Failure Postconditions

- A directory that already held files is never changed.
- No half-set-up workspace remains: either the workspace with its Home note exists, or nothing that Underline created for it is left.

## Business Rules

### BR-001: Markdown preselected

The default format offered for a new workspace is Markdown; the user may choose AsciiDoc instead. The choice is asked every time a workspace is created.

### BR-002: New or empty directory only

A workspace is created only in a directory that does not exist yet or holds no files and folders. A directory with content is opened as a workspace (UC-002), never created anew.

### BR-003: Home note

Every new workspace starts with one note titled Home, empty except for its title as heading, stored in the default format. Home is the first page of a GitHub or GitLab wiki, so a workspace pushed to a wiki shows it as the start page.

### BR-004: Git remote not contacted

The git remote address is recorded as given. The system does not contact the git remote while creating the workspace; the first git sync (UC-014) shows whether it is reachable.

### BR-005: Default format applies to new notes only

The default format decides the note format of notes created later (UC-003). Changing it afterwards in the settings (UC-010) never converts existing notes.
