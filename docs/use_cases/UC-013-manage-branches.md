# Use Case: Manage Branches

## Overview

**Use Case ID:** UC-013  
**Use Case Name:** Manage Branches  
**Primary Actor:** User  
**Goal:** The user works on the line of notes they chose: a new branch they created, or another branch they switched to, without losing a change.  
**Trigger:** The user opens the branch menu in the status bar.  
**Status:** Draft  

**Requirements:** [FR-050, FR-047, FR-018, NFR-006, NFR-011, NFR-014](../requirements.md)

## Preconditions

- A workspace that is a git repository is open, and git can be found on the computer (UC-011).

## Main Success Scenario

1. The user opens the branch menu by choosing the branch in the status bar, or with Ctrl+Shift+B (Cmd+Shift+B on macOS).
2. The system lists the local branches, the current one marked, then the branches of the remote that have no local branch, with a field to filter them by name, an entry to create a new branch, and an entry to show the changes (UC-011).
3. The user chooses another branch.
4. The system saves the open notes with changes not yet saved and switches the workspace to the chosen branch, carrying along the changes that are not committed (BR-003).
5. The system shows the notes of the branch: the note overview lists them, open tabs show their notes as the branch holds them, and tabs of notes the branch does not hold are closed (BR-004).
6. The system shows the new branch in the status bar and reports in the status bar that it switched, and how many changes it carried along.

## Alternative Flows

### A1: Create a branch

**Trigger:** The user chooses to create a new branch (step 3)  
**Flow:**

1. The system asks for the name of the new branch, filled with the text of the filter field, and says it starts from the current commit with the current changes and goes to the remote with the next synchronization.
2. The user enters a name and confirms.
3. The system checks the name (BR-005), creates the branch from the current commit, and switches to it; the changes that are not committed stay as they are.
4. Use case continues at step 6.

### A2: Name not allowed or taken

**Trigger:** The name breaks the rules of BR-005, or a local branch has that name (A1 step 3)  
**Flow:**

1. The system says why, keeps what the user entered, and offers to switch to the existing branch when it is taken.
2. Use case continues at A1 step 2.

### A3: Changes would be overwritten

**Trigger:** A change that is not committed touches a note or file that differs on the chosen branch, so it cannot be carried along (step 4)  
**Flow:**

1. The system does not switch and names the notes and files in the way.
2. The system offers to commit the changes first (UC-012) or to stay.
3. Use case ends.

### A4: Remote branch chosen

**Trigger:** The user chooses a branch of the remote that has no local branch (step 3)  
**Flow:**

1. The system creates a local branch of the same name that follows the remote branch, as the last synchronization knows it, and switches to it.
2. Use case continues at step 5.

### A5: Switch fails

**Trigger:** git refuses to switch for another reason, for example because another git program is busy with the repository (step 4)  
**Flow:**

1. The system reports in the status bar that it could not switch and why, in git's words.
2. The workspace stays on its branch, and nothing is changed.
3. Use case ends.

### A6: A note cannot be saved

**Trigger:** An open note with changes not yet saved cannot be saved (step 4)  
**Flow:**

1. The system does not switch and reports which note could not be saved and why (UC-005 A14).
2. Use case ends.

### A7: Unfinished merge

**Trigger:** A synchronization left notes in conflict (UC-015) (step 2)  
**Flow:**

1. The system lists the branches but offers no switch and no new branch, and says that the conflicts must be resolved first.
2. Use case ends.

### A8: User cancels

**Trigger:** The user closes the menu or cancels the name (step 3, A1 step 2)  
**Flow:**

1. Nothing is changed.
2. Use case ends.

## Postconditions

### Success Postconditions

- The workspace is on the chosen or new branch, and every change that was not committed is still there.
- The note overview, the open tabs, the changes (UC-011), and the status bar show the branch.
- Nothing was sent over the network.

### Failure Postconditions

- The workspace is on its branch as before, with every note and change as it was.

## Business Rules

### BR-001: The list

Local branches come first, the current one marked, by name; then the branches of the remote without a local branch of the same name, by name, marked as on the remote. The filter field narrows the list to names that hold its text, ignoring case. The list knows the remote's branches as of the last synchronization (UC-014); opening it sends nothing.

### BR-002: The menu

The menu holds the list, the entry to create a branch, and the entry to show the changes (UC-011 step 3), which the branch in the status bar opened before.

### BR-003: Changes carried along

Changes that are not committed go along to the chosen branch, as git carries them, when the branch does not differ in the same notes and files (A3). Nothing that is not committed is ever lost by switching.

### BR-004: Open tabs

After a switch, a tab whose note the branch holds shows it as the branch holds it, with the changes carried along; a tab whose note the branch does not hold is closed, and the report names it. The remembered tabs and recent notes stay as they are (UC-002).

### BR-005: Branch names

A branch name is not empty, holds no spaces, no control characters, and none of `~ ^ : ? * [ \`, does not start with `-` or `.`, does not end with `/` or `.lock`, holds no `..`, `//`, or `@{`, and is not `@`, as git requires. Spaces typed into the name become `-`, as the field shows while typing.

### BR-006: Keyboard

Ctrl+Shift+B (Cmd+Shift+B on macOS) opens the menu with the focus in the filter field; the arrow keys move through the list, Enter switches to the chosen branch, and Escape closes the menu (NFR-011).

### BR-007: Not in scope

Deleting and renaming branches are not part of this use case (OQ-142); git does them.

### BR-008: Starting point

A new branch always starts from the current commit, with the changes that are not committed; there is no choice of another starting point.

### BR-009: To the remote

When the workspace has a remote, a new branch goes to it with the next synchronization (UC-014), which then follows the remote branch, so nothing stays only on this computer.

