# Use Case: Review Changes

## Overview

**Use Case ID:** UC-011  
**Use Case Name:** Review Changes  
**Primary Actor:** User  
**Goal:** The user knows the branch they work on, which notes and files changed since the last commit, and what changed in a note, and can take back the changes of a note.  
**Trigger:** The user opens a workspace that is a git repository, changes a note, or looks at the changes.  
**Status:** Draft  

**Requirements:** [FR-017, FR-047, FR-048, FR-049, NFR-002, NFR-011, NFR-013](../requirements.md)

## Preconditions

- A workspace that is a git repository is open (UC-001 or UC-002), and git can be found on the computer.

## Main Success Scenario

1. The system shows in the status bar the current branch and how many notes and files changed since the last commit.
2. The system marks every changed note in the note overview as new or changed.
3. The user opens the changes, from the status bar or the tool panel.
4. The system lists in the tool panel the changed notes, then the other changed files, each marked as new, changed, deleted, or renamed.
5. The user chooses a changed note in the list, or a note is active in a tab.
6. The system opens the note if it is not open and shows in the tool panel the diff of the active note against the last commit: the changed parts with the lines removed and the lines added, a few unchanged lines around them, and the line numbers.
7. The user chooses a line of the diff.
8. The system shows that line in the note, in the editing view of its tab.

## Alternative Flows

### A1: No changes

**Trigger:** Nothing changed since the last commit (step 1, step 4)  
**Flow:**

1. The status bar shows the branch with no count, and the list says that nothing changed since the last commit.
2. Use case ends.

### A2: Active note unchanged

**Trigger:** The active note has no changes since the last commit (step 6)  
**Flow:**

1. The tool panel says that the note did not change since the last commit.
2. Use case continues at step 5.

### A3: Changes while looking

**Trigger:** The user types in a note, or a note or file is changed, created, renamed, or deleted, in Underline or outside, while the changes are shown (any step)  
**Flow:**

1. The system updates the count, the marks, the list, and the diff, at the latest one second after the typing stopped.
2. Use case continues at the step it left.

### A4: Deleted note chosen

**Trigger:** The user chooses a deleted note in the list (step 5)  
**Flow:**

1. The system shows its diff, every line removed, without opening a tab.
2. Use case continues at step 5.

### A5: Other file chosen

**Trigger:** The user chooses a file that is not a note in the list (step 5)  
**Flow:**

1. For a text file, the system shows its diff as for a note; for any other file, it says the file is new, changed, or deleted, without a diff.
2. Use case continues at step 5.

### A6: Discard the changes of a note

**Trigger:** The user asks to discard the changes of a note, from the list or the diff (step 4, step 6)  
**Flow:**

1. The system asks the user to confirm, naming the note and how many lines will be lost.
2. When the user confirms, the system puts the note back as it was at the last commit: a changed note gets its committed content, also in its open tab; a new note goes to the deleted notes (UC-007); a deleted note comes back; a renamed note gets its old name back.
3. The system updates the count, marks, list, and diff, and reports in the status bar what it discarded.
4. When the user cancels, nothing is changed.
5. Use case continues at step 4.

### A7: No commit yet

**Trigger:** The repository has no commit yet (step 1)  
**Flow:**

1. The status bar shows the branch to be created by the first commit, and every note and file counts as new.
2. Use case continues at step 2.

### A8: No branch

**Trigger:** The workspace is not on a branch, but on a commit of its own, for example during a rebase (step 1)  
**Flow:**

1. The status bar shows the short name of the commit instead of a branch, marked as not on a branch.
2. Use case continues at step 2.

### A9: Git cannot be asked

**Trigger:** Git cannot be found, or fails to answer, for example because the repository is damaged (step 1)  
**Flow:**

1. The status bar says that the changes cannot be shown and why; marks, list, and diff are not shown.
2. Underline keeps working with the notes as with a workspace without git.
3. Use case ends.

## Postconditions

### Success Postconditions

- The status bar, the marks, the list, and the diff show the state of the workspace against the last commit.
- Looking at changes changes nothing; only discarding (A6) changes the notes it names.

### Failure Postconditions

- No note or file is changed.

## Business Rules

### BR-001: Against the last commit

Changes are shown against the last commit of the current branch, as a commit would record them: there is no step of choosing which changes go into a commit. The text of a tab counts with its changes not yet saved.

### BR-002: What is shown

Notes and other files git sees are shown: files git ignores, the Underline folder (UC-001), and the deleted notes (UC-007) are not. Notes come first, then the other files, each group in the order of the note overview and by name.

### BR-003: Marks

A note is marked as new (not in the last commit), changed, or renamed in the note overview and in the list; a deleted note is listed only in the list, since the note overview no longer holds it. The marks are letters with a color, so they can be told apart without color too.

### BR-004: Diff

The diff compares lines of the markup, as git does: removed lines first, then added lines, each with its line number, three unchanged lines around each changed part, and the parts joined when they come close. A note renamed and changed shows the changes of its content.

### BR-005: Discarding

Discarding takes back all changes of one note since the last commit, after the user confirmed it. What it takes back is kept where it can be: a new note goes to the deleted notes (UC-007); the previous content of a changed note is not kept, which the question says. Files that are not notes are not discarded in Underline.

### BR-006: Speed

The count, the marks, and the list appear within one second of opening a workspace of 10,000 notes, and typing is never held up by finding the changes (NFR-002).

### BR-007: Keyboard

The changes are reachable with the keyboard: a shortcut (Ctrl+Shift+G, Cmd+Shift+G on macOS) opens them in the tool panel; the arrow keys move through the list and the diff, and Enter opens the chosen note or line (NFR-011).
