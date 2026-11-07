# Use Case: Resolve Sync Conflict

## Overview

**Use Case ID:** UC-015  
**Use Case Name:** Resolve Sync Conflict  
**Primary Actor:** User  
**Goal:** Notes changed both here and on the remote in the same places hold what the user chose from both versions, and the sync completes without a change lost.  
**Trigger:** A sync stopped because notes or files are in conflict (UC-014 A3).  
**Status:** Draft  

**Requirements:** [FR-046, FR-045, NFR-006, NFR-011](../requirements.md)

## Preconditions

- A sync of the workspace stopped with notes or files in conflict (UC-014 A3); both versions are kept.

## Main Success Scenario

1. The system marks the notes and files in conflict in the note overview and lists them first among the changes (UC-011), and the status bar says how many conflicts wait.
2. The user opens a note in conflict.
3. The system shows the note with every conflicting place as a block of two versions, "Here" and "Remote", side by side, or one below the other in a narrow window; the rest of the note holds both sides' changes already joined.
4. For each conflicting place, the user chooses "Here", "Remote", "Both" (here first, then remote), or edits the place freely.
5. The system replaces the place with the choice and shows how many places of the note are still open.
6. When no place of the note is open, the system saves the note and marks it as resolved.
7. When every note and file in conflict is resolved, the system completes the joining of the sync and continues the sync (UC-014 step 5), and reports in the status bar that the conflicts are resolved.

## Alternative Flows

### A1: One side for the whole note

**Trigger:** The user chooses "All: Here" or "All: Remote" for the note (step 4)  
**Flow:**

1. The system resolves every open place of the note with that side.
2. Use case continues at step 6.

### A2: File that is not a note

**Trigger:** A file that is not a note, such as an image, is in conflict (step 2)  
**Flow:**

1. The system shows both versions where it can, such as both pictures, and offers "Here" or "Remote" for the whole file.
2. The user chooses one.
3. The system keeps the chosen version at the file's place and the other one next to it as a copy, named with "(here)" or "(remote)" before the extension, such as `shot (remote).png`.
4. Use case continues at step 7.

### A3: Deleted on one side

**Trigger:** A note or file was deleted on one side and changed on the other (step 2)  
**Flow:**

1. The system shows the changed version and says on which side it was deleted.
2. The user chooses "Keep" or "Delete".
3. For "Keep", the note stays with the changed version; for "Delete", it goes to the deleted notes (UC-007) on this computer and is deleted in the workspace.
4. Use case continues at step 7.

### A4: Later

**Trigger:** The user leaves conflicts open (step 2 to step 6)  
**Flow:**

1. The marks stay; the user reads and writes notes as usual, also notes in conflict, whose open places stay blocks.
2. Commits (UC-012), switching branches (UC-013), and syncs (UC-014) wait until the conflicts are resolved; the status bar keeps saying how many wait.
3. The open conflicts stay when Underline is closed and come back when the workspace is opened.
4. Use case continues at step 2 whenever the user goes on.

### A5: Abort

**Trigger:** The user chooses "Abort the sync" in the changes or the status bar (any step)  
**Flow:**

1. The system asks the user to confirm, saying the workspace goes back to how it was before the sync and resolved places are lost.
2. When the user confirms, the system puts the workspace back as it was right before the sync joined the remote's changes; the local commits and the changes left out stay; the remote's commits are not taken.
3. The system reports it; the next sync meets the same conflicts again unless something changed.
4. Use case ends.

### A6: Commit by commit

**Trigger:** The sync puts local commits on top of the remote's, as the user's git setup says (UC-014 BR-003), and a later local commit conflicts too (step 7)  
**Flow:**

1. The system shows the conflicts of that commit, as of step 1, naming the commit.
2. Use case continues at step 2.

### A7: Completing fails

**Trigger:** git refuses to complete the joining, for example because a check of the repository rejects it (step 7)  
**Flow:**

1. The system reports why, in git's words; the resolved notes stay as they are.
2. The user can try again from the changes, or abort (A5).
3. Use case ends.

## Postconditions

### Success Postconditions

- Every note and file in conflict holds what the user chose; no version was lost without the user choosing it, and copies of files keep the version not chosen.
- The joining is completed and the sync went on, so the remote holds the result.

### Failure Postconditions

- After A5, the workspace is as it was before the sync; nothing of the notes is lost.
- After A4 or A7, the conflicts stay and nothing else is changed.

## Business Rules

### BR-001: Both versions kept

A sync never overwrites a change silently (FR-046): until the user chooses, both versions of every conflicting place are kept in the note, and a note in conflict is never committed with the conflict in it.

### BR-002: Blocks of two versions

A conflicting place shows "Here" (this computer's version) and "Remote" (the version that came with the sync), each with who changed it and when, if known. In the facade both versions are shown formatted; in the source view as markup. Between the blocks, the note is as both sides' changes made it.

### BR-003: Resolved

A note is resolved when no block of two versions is left, whether the user chose for each place, chose for the whole note, or edited the blocks away in the source view. Its mark changes from conflict to changed.

### BR-004: Completing

When the last conflict is resolved, the joining is completed as git records it: by a commit that joins both sides with the message `Merge remote changes into <branch>`, or by going on with putting the local commits on top (A6). Then the sync pushes as usual (UC-014).

### BR-005: Marks

A note or file in conflict is marked with the letter C in the note overview and the changes, in the warning color, and listed first among the changes.

### BR-006: Keyboard

In a note with conflicts, F7 and Shift+F7 move to the next and previous open place; Alt+H, Alt+R, and Alt+B choose "Here", "Remote", and "Both" for the place at the cursor (NFR-011).
