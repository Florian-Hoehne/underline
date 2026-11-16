# Use Case: Restore Snapshot

## Overview

**Use Case ID:** UC-026  
**Use Case Name:** Restore Snapshot  
**Primary Actor:** User  
**Goal:** The user sees the earlier versions Underline kept of a note, compares one with the note, and gives the note that content back, also in a workspace that is no git repository and after the undo history is gone.  
**Trigger:** The user asks for the snapshots of a note.  
**Status:** Reviewed  

**Requirements:** [FR-101, FR-102, NFR-005, NFR-006, NFR-011, NFR-014, NFR-015](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).
- The note is listed in the note overview.

## Main Success Scenario

1. The user asks for the snapshots of a note, from the context menu of the note in the note overview or of its tab, or with the command palette for the active note (UC-023).
2. The system lists the snapshots of the note, the newest first, each with the day and time it was taken, its cause (BR-001), and how many lines it differs from the note as it is now.
3. The user chooses a snapshot.
4. The system shows the differences between the snapshot and the note as it is now, lines taken away and lines added marked as in the diff of the workspace (UC-011), and changes nothing.
5. The user restores the snapshot.
6. The system takes a snapshot of the note as it is now with the cause Restore, gives the note the content of the chosen snapshot, also in its tab when it is open, and stores it.
7. The system reports in the status bar that the note was restored to the snapshot of that day and time, with an offer to undo.

## Alternative Flows

### A1: No snapshots

**Trigger:** The note has no snapshot (step 2)  
**Flow:**

1. The system says that the note has no snapshot yet and when snapshots are taken (BR-001).
2. Use case ends.

### A2: Look at the whole snapshot

**Trigger:** The user asks to see the snapshot itself instead of its differences (step 4)  
**Flow:**

1. The system shows the whole snapshot rendered as in the facade, read only; the user can select and copy from it.
2. Use case continues at step 5, or ends when the user closes it.

### A3: Undo the restore

**Trigger:** The user chooses to undo in the status bar report (step 7), which offers it until another report replaces it  
**Flow:**

1. The system gives the note the content of the snapshot taken in step 6.
2. Use case ends.

### A4: Note changes while the snapshots are shown

**Trigger:** The note is changed in a tab, by another program, or by an agent (step 4)  
**Flow:**

1. The system shows the differences to the new content and updates the list of snapshots.
2. Use case continues at step 4.

### A5: Note cannot be stored

**Trigger:** The restored note cannot be stored, for example because the permission is gone or the disk is full (step 6)  
**Flow:**

1. The system handles it as any change that cannot be stored (UC-005 A14): the tab keeps the restored content, marked as not stored, and the status bar says why.
2. Use case ends.

### A6: User cancels

**Trigger:** The user closes the snapshots without restoring one (step 3, step 5)  
**Flow:**

1. The system changes nothing.
2. Use case ends.

## Postconditions

### Success Postconditions

- The note holds the content of the chosen snapshot and is stored.
- A snapshot with the cause Restore holds the content the note had before, so the restore can be taken back.
- The other snapshots of the note are unchanged.

### Failure Postconditions

- No snapshot is changed or removed by looking at it or restoring it.
- A note is never half written, and its content from before the restore is kept as a snapshot before it changes.

## Business Rules

### BR-001: When snapshots are taken

The system takes a snapshot of a note, with its content and the time, for each of these causes:

| Cause                | When                                                                                                       |
|----------------------|------------------------------------------------------------------------------------------------------------|
| Editing              | Before the first change of a note in Underline when its newest snapshot is older than 10 minutes, of the content before that change |
| Outside change       | When a note open in a tab is changed by another program, of the content the tab showed before (UC-004 A7)  |
| Agent change         | Before an agent changes a note (UC-019), of the content before                                              |
| Replace in workspace | Before a replacement in the workspace changes a note (UC-022)                                               |
| Git                  | Before a pull, a git sync, a switch of branch, or the resolution of a conflict changes a note (UC-013, UC-014, UC-015) |
| Restore              | Before a snapshot is restored (step 6)                                                                      |

No snapshot is taken when the content is the same as in the newest snapshot of the note.

### BR-002: Kept for 30 days, on this computer

A snapshot is kept for 30 days and then removed. Snapshots are kept in the Underline folder of the workspace, on this computer only, like the deleted notes (UC-007 BR-004): git ignores them, and they are not listed in the note overview, not searched, and not read or written by agents. Taking and keeping them needs no network connection (NFR-014), and no snapshot leaves the computer (NFR-015).

### BR-003: Renamed and deleted notes

The snapshots of a note follow it when it is renamed or moved (UC-006). When a note is deleted (UC-007), its snapshots are kept with it among the deleted notes and come back when it is restored; when the deleted note is removed for good, its snapshots go with it.

### BR-004: Restoring is a change

Restoring gives the note the whole content of the snapshot; it is a change of the note like any other: stored at once, never half written (NFR-006), shown to a git repository as a change to commit (UC-012), and one change of the tab's history, which undo takes back (UC-005 BR-004). The note keeps its title, folder, and note format.

### BR-005: Independent of git

Snapshots are taken in every workspace, whether it is a git repository or not. They do not replace the history of git: commits stay in the git repository (UC-011, UC-012), snapshots only on this computer.

### BR-006: Keyboard

The list of snapshots, the differences, restoring, and undoing can be reached by keyboard alone (NFR-011).
