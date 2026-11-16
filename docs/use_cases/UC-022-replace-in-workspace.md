# Use Case: Replace in Workspace

## Overview

**Use Case ID:** UC-022  
**Use Case Name:** Replace in Workspace  
**Primary Actor:** User  
**Goal:** The user replaces a text in the notes of the workspace in one step, after seeing every place it changes, and can take the replacement back.  
**Trigger:** The user asks to replace in the workspace.  
**Status:** Reviewed  

**Requirements:** [FR-074, FR-075, FR-101, NFR-002, NFR-006, NFR-011, NFR-013, NFR-017](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user opens the replace of the workspace, with Ctrl+Shift+H (Cmd+Shift+H on macOS) or from the search of the workspace (UC-008), types the search text and the replacement, and chooses the search switches; the search text and the search switches of the search of the workspace are taken over.
2. While the user types, the system finds every place in the notes of the workspace that matches the search text with the search switches (BR-001, BR-002) and shows the notes with their places, each place with its line as it is and as it will be after the replacement, and how many places in how many notes it found.
3. The user looks through the places and leaves out the notes and the places that shall not change; at first every place is chosen.
4. The user confirms the replacement.
5. The system takes a snapshot of every note it is going to change (BR-005), replaces the chosen places, and stores every changed note whole; a note open in a tab changes in its tab.
6. The system reports in the status bar how many places in how many notes were replaced, with an offer to undo, and searches again, so the replaced places are no longer listed.

## Alternative Flows

### A1: Nothing found

**Trigger:** No note holds the search text (step 2)  
**Flow:**

1. The system says that no note holds the search text and offers no replacement.
2. Use case continues at step 1.

### A2: Regular expression not valid

**Trigger:** The search switch regular expression is on and the search text is no valid regular expression, or the replacement names a group the expression does not have (step 2)  
**Flow:**

1. The system says what is wrong with the search text or the replacement, shows no places, and offers no replacement.
2. Use case continues at step 1.

### A3: Replace in one note

**Trigger:** The user replaces the chosen places of one note only (step 3)  
**Flow:**

1. The system takes a snapshot of that note, replaces its chosen places, stores it, and reports it in the status bar with an offer to undo.
2. Use case continues at step 6.

### A4: A note changes while the places are shown

**Trigger:** A note of the workspace is changed in a tab, by another program, or by an agent, or a note is created, renamed, or deleted, before the user confirms (step 3)  
**Flow:**

1. The system finds the places of that note again and updates the list; a place the user left out stays left out while it is still found.
2. Use case continues at step 3.

### A5: Note cannot be stored

**Trigger:** A changed note cannot be stored, for example because the permission is gone or the disk is full (step 5)  
**Flow:**

1. The system leaves that note as it was, replaces in the other notes, and says in the status bar which notes could not be changed and why.
2. Use case continues at step 6.

### A6: Undo the replacement

**Trigger:** The user chooses to undo in the status bar report (step 6), which offers it until another report replaces it  
**Flow:**

1. The system gives every changed note back the content of the snapshot taken before the replacement, unless the note has changed since; such a note is left as it is and named in the status bar.
2. Use case ends.

### A7: Many places

**Trigger:** The search text is found at more than 10,000 places (step 2)  
**Flow:**

1. The system shows the first 10,000 places and says how many more there are; the replacement still changes every chosen place, the ones not shown included.
2. Use case continues at step 3.

### A8: User cancels

**Trigger:** The user closes the replace of the workspace without confirming (step 3)  
**Flow:**

1. The system changes nothing; the search text stays in the search of the workspace (UC-008).
2. Use case ends.

## Postconditions

### Success Postconditions

- Every chosen place in the notes of the workspace holds the replacement, and every other place and note is unchanged.
- A snapshot of each changed note holds its content from before the replacement (UC-026).
- Every changed note is stored, and a git repository shows the changed notes as changes to commit (UC-012).

### Failure Postconditions

- A note is either fully replaced or not changed at all; it is never half written.
- No note changes before the user has seen its places and confirmed.

## Business Rules

### BR-001: What is replaced

Only the text of the notes of the workspace is searched and replaced, as it is stored, in its markup, as the search of the workspace reads it (UC-008 BR-001): for a note open in a tab, the text of the tab with changes not yet saved. Titles and folders are not changed; renaming is UC-006. Deleted notes, snapshots, hidden folders, and files that are not notes are not touched.

### BR-002: What matches

The search text is one text, spaces included, not a set of words in any order as in the search of the workspace (UC-008 BR-002). The search switches apply as in UC-008 BR-011: without match case, upper and lower case do not matter; with whole word, a place matches only where it is not part of a longer word; with regular expression, the search text is a regular expression. A place found never spans two lines unless the regular expression matches a line end.

### BR-003: The replacement

The replacement is written as typed. With the search switch regular expression, `$1` to `$9` in the replacement stand for the parts of the place found in the matching parentheses of the expression, and `$0` for the whole place found. Without match case, the replacement is written as typed, not adapted to the case of the place found. An empty replacement removes the places found.

### BR-004: Seen before it changes

Nothing changes before the user confirms, and the user sees every place that will change, with its line before and after, except as A7 says. Places the user left out stay as they are.

### BR-005: Snapshots and undo

Before a note is changed by a replacement, the system takes a snapshot of it with the cause Replace in workspace (UC-026 BR-001). The status bar offers to undo the whole replacement until another report replaces it (A6); later, each note can be restored from its snapshot (UC-026). In a note that is open in a tab, the replacement also counts as one change of the tab's history, so undo in that tab takes it back for that note (UC-005 BR-004).

### BR-006: Note by note, never half written

Each changed note is stored whole, as autosave stores it (NFR-006). A note that cannot be stored does not stop the others (A5). Links are not updated as when renaming (UC-006 BR-003): a replacement that changes the place a link names changes it as text.

### BR-007: Speed

The places found appear within 2 seconds for 95% of search texts in a workspace of 10,000 notes, and replacing in 1,000 notes is done within 10 seconds (NFR-017). Typing in the fields is never held up by a running search (NFR-002).

### BR-008: Keyboard

Ctrl+Shift+H (Cmd+Shift+H on macOS) opens the replace of the workspace from anywhere in the window. Every part of it can be used by keyboard alone: the fields, the search switches (Alt+C match case, Alt+W whole word, Alt+R regular expression), moving through the places, leaving one out, replacing in one note, and confirming (NFR-011).

### BR-009: Seen by others

A replaced note is changed like any other change of the user: a git repository shows it as a change to commit (UC-012), and a connected agent reads the new content (UC-018).
