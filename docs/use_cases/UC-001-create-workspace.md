# Use Case: Create Workspace

## Overview

**Use Case ID:** UC-001  
**Use Case Name:** Create Workspace  
**Primary Actor:** User  
**Goal:** The user has a new workspace that holds only its Home note besides its workspace settings file and the Underline folder, in the default format of their choice, optionally as a git repository, and can start writing in the Home note.  
**Trigger:** The user asks to create a new workspace.  
**Status:** Reviewed  

**Requirements:** [FR-001, FR-002, FR-070, FR-071, FR-072, NFR-014, C-002, C-006](../requirements.md)

## Preconditions

_None — no workspace or other state needs to exist before._

## Main Success Scenario

1. The user chooses a directory for the new workspace: a new directory or an existing empty one.
2. The system checks that the directory is new or empty and does not lie inside an existing workspace.
3. The system asks for the default format, with Markdown preselected, and offers to create the workspace as a git repository.
4. The user confirms a default format and leaves the git repository option off.
5. The system checks again that the directory is new or empty, creates the directory if it does not exist yet, records the default format in the workspace settings file, and creates the Home note in the default format.
6. The system closes the workspace that was open before, if any, keeping its open tabs for the next time it is opened.
7. The system opens the new workspace, lists the Home note in the note overview, opens it in a tab in the facade, remembers the workspace as the last opened one, and reports in the status bar that the workspace was created.

## Alternative Flows

### A1: Directory holds files or folders

**Trigger:** The chosen directory is not empty (step 2 or step 5)  
**Flow:**

1. The system tells the user that the directory already holds files or folders and offers to open it as a workspace instead.
2. The user chooses another directory.
3. Use case continues at step 2.

### A2: Open the directory instead

**Trigger:** The user accepts the offer to open the non-empty directory as a workspace (A1 step 1)  
**Flow:**

1. The system opens the directory as an existing workspace (UC-002, from its step 4).
2. Use case ends.

### A3: Create as git repository

**Trigger:** The user switches the git repository option on (step 4)  
**Flow:**

1. The system asks for the address of a git remote; the address is optional.
2. The user enters a git remote address or leaves it empty and confirms.
3. The system checks again that the directory is new or empty, creates the directory if it does not exist yet, makes it a git repository on the branch the user's git setup names as default, links the git remote when an address was given, records the default format in the workspace settings file, and creates the Home note in the default format.
4. The system commits the Home note and the workspace settings file as the first commit, titled "Create workspace".
5. Use case continues at step 6.

### A4: User cancels

**Trigger:** The user cancels instead of choosing a directory (step 1), instead of confirming (step 4), or instead of confirming the git remote address (A3 step 2)  
**Flow:**

1. The system closes the dialog without creating anything.
2. Use case ends.

### A5: Workspace cannot be created

**Trigger:** The system cannot create the directory, the workspace settings file, or the Home note (step 5), or the git repository, the git remote link, or the first commit (A3 steps 3–4)  
**Flow:**

1. The system reports in the status bar that the workspace could not be created and why, for example missing permission, a full disk, git not being installed, or git having no user name and email to make the first commit with; for a git problem it says how to fix it in git.
2. The system removes whatever it had created for this workspace.
3. Use case ends.

### A6: Leftovers cannot be removed

**Trigger:** The system cannot remove what it had created (A5 step 2)  
**Flow:**

1. The system names in the status bar the directory or files that could not be removed, so the user can delete them by hand.
2. Use case ends.

### A7: Directory inside a workspace

**Trigger:** The chosen directory lies inside the directory of an existing workspace (step 2)  
**Flow:**

1. The system tells the user that the directory lies inside that workspace and asks for another directory.
2. The user chooses another directory.
3. Use case continues at step 2.

## Postconditions

### Success Postconditions

- The workspace exists in the chosen directory with the chosen default format, recorded in its workspace settings file.
- The workspace holds the Home note in the default format, and it is open in a tab in the facade.
- When the user chose the git repository option, the workspace is a git repository on the user's default branch, its first commit holds the Home note and the workspace settings file, and it is linked to the git remote when an address was given.
- The workspace that was open before is closed, and its open tabs are remembered.
- The new workspace is the open workspace and is remembered as the last opened one; its open tab is remembered in its Underline folder, which git ignores.

### Failure Postconditions

- A directory that already held files or folders is never changed.
- No half-set-up workspace remains: either the workspace with its Home note exists, or nothing that Underline created for it is left, or the status bar names what is left (A6).
- The workspace that was open before stays open when creating the new one fails or is cancelled.

## Business Rules

### BR-001: Markdown preselected

The default format offered for a new workspace is Markdown; the user may choose AsciiDoc instead. The choice is asked every time a workspace is created.

### BR-002: New or empty directory only

A workspace is created only in a directory that does not exist yet or holds no files and folders. Hidden files and folders count as content too, so a directory holding only a git repository is not empty. A directory with content is opened as a workspace (UC-002), never created anew.

### BR-003: Home note

Every new workspace starts with one note titled Home, empty except for its title as heading, stored in the default format. Home is the first page of a GitHub wiki, so a workspace pushed to a GitHub wiki shows it as the start page.

### BR-004: Git remote named origin, not contacted

The git remote address is recorded as given, under the name origin. The system does not contact the git remote while creating the workspace; the first git sync (UC-014) shows whether it is reachable. When the git remote already holds a history of its own, the first git sync overwrites neither side and tells the user; a user who wants to work on an existing repository opens a copy of it instead (UC-002).

### BR-005: Default format applies to new notes only

The default format decides the note format of notes created later (UC-003). Changing it afterwards in the settings (UC-010) never converts existing notes.

### BR-006: No nested workspaces

A workspace is never created inside the directory of an existing workspace, so no note belongs to two workspaces.

### BR-007: First commit

A workspace created as a git repository starts with one commit, titled "Create workspace", that holds the Home note and the workspace settings file, on the branch the user's git setup names as default (main when it names none). The first git sync can push it without a further step.
