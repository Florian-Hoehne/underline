# Use Case: Commit Changes

## Overview

**Use Case ID:** UC-012  
**Use Case Name:** Commit Changes  
**Primary Actor:** User  
**Goal:** The changes the user chose are recorded in the history of the workspace with a message that says what changed and why.  
**Trigger:** The user asks to commit the changes of the workspace.  
**Status:** Reviewed  

**Requirements:** [FR-051, FR-048, NFR-006, NFR-011, NFR-014](../requirements.md)

## Preconditions

- A workspace that is a git repository is open, and git can be found on the computer (UC-011).
- At least one note or file changed since the last commit (UC-011).

## Main Success Scenario

1. The user opens the changes in the tool panel (UC-011).
2. The system shows a field for the commit message above the list of changes, holding a proposed message (BR-002), with every changed note and file chosen for the commit.
3. The user keeps or rewrites the message, and leaves out notes or files they do not want in this commit.
4. The user asks to commit, with the commit button or Ctrl+Enter (Cmd+Enter on macOS) in the message field.
5. The system saves the open notes that have changes not yet saved.
6. The system records the chosen changes with the message as a new commit on the current branch; the changes left out stay changed.
7. The system empties the message field, updates the list of changes and the branch in the status bar (UC-011), and reports in the status bar the commit with its short name and how many notes and files it holds.

## Alternative Flows

### A1: Empty message

**Trigger:** The message is empty or holds only spaces (step 4)  
**Flow:**

1. The system does not commit and says that a commit needs a message.
2. Use case continues at step 3.

### A2: Nothing chosen

**Trigger:** The user left out every change (step 3)  
**Flow:**

1. The system offers no commit and says that nothing is chosen.
2. Use case continues at step 3.

### A3: Git does not know the user

**Trigger:** git has no name and email to record the commit with (step 6)  
**Flow:**

1. The system asks for the name and the email to record commits with, saying they are kept for every repository of the user and show in the history.
2. The user enters both and confirms.
3. The system keeps them in the user's git settings and continues at step 6.
4. When the user cancels, nothing is committed and the message and choice are kept; use case ends.

### A4: Commit fails

**Trigger:** git refuses the commit, for example because a check of the repository rejects it or another git program is busy with the repository (step 6)  
**Flow:**

1. The system reports in the status bar that the commit failed and why, in git's words.
2. Nothing is committed; the message and the choice are kept.
3. Use case continues at step 3.

### A5: A note cannot be saved

**Trigger:** An open note with changes not yet saved cannot be saved (step 5)  
**Flow:**

1. The system does not commit and reports which note could not be saved and why (UC-005 A14).
2. The message and the choice are kept.
3. Use case ends.

### A6: Amend the last commit

**Trigger:** The user chooses to amend the last commit instead of making a new one (step 3)  
**Flow:**

1. The system offers this only while the last commit of the branch is not on the remote yet (BR-006); then it puts the message of the last commit into the field.
2. The user keeps or changes the message and the chosen changes, and asks to commit.
3. The system replaces the last commit by one that holds its changes and the chosen ones, with the message of the field.
4. Use case continues at step 7, reporting that the last commit was amended.

## Postconditions

### Success Postconditions

- The branch has a new commit (or an amended last commit) holding exactly the chosen changes with the message.
- The changes left out are still changed, and nothing of the notes is lost.
- Nothing was sent over the network.

### Failure Postconditions

- No commit is made; the notes, the message, and the choice are as they were.

## Business Rules

### BR-001: What a commit holds

By default every change listed (UC-011) is chosen: new, changed, deleted, and renamed notes, and other files. A renamed note is one change: choosing it records both its old place gone and its new place. A note open in a tab is committed with its changes not yet saved, which are saved first (step 5).

### BR-002: Proposed message

The proposed message names what changed by the titles of the notes, such as `Update Plan, Ideas`, `Add New`, `Delete Old`, or `Rename Draft to Final`, and joins the kinds with `; `. With more than three notes of a kind it names three and the count of the others (`Update Plan, Ideas, Trip and 4 more notes`); other files add `and 2 other files`. It follows the choice while the user has not typed. The proposal is made on this computer from the titles only; nothing leaves it.

### BR-003: Message

The message has a first line that sums up the commit and may have more lines. Spaces at its start and end are dropped. When the first line is longer than 72 characters, the field says that many programs show only the start of it; the commit is still possible. It is kept while the user looks at other notes, until a commit succeeds.

### BR-004: Choice

The user chooses the changes in the list of UC-011 with a check box each and can choose all or none at once. The choice is kept while the list changes, for the changes still listed; a change that appears after the list was shown is chosen.

### BR-005: The user's name and email

The name and email of A3 are kept in the user's own git settings, so they hold for every repository, as git keeps them. Underline changes no other git setting.

### BR-006: Amending

Only the last commit can be amended, and only while it is not on the branch of the remote: when the branch has no remote branch, or the remote branch does not hold the last commit. Amending keeps the time and author of the last commit.

### BR-007: No network

Committing works without a network connection and sends nothing (NFR-014); the commit reaches the remote with the next synchronization (UC-014).

### BR-008: The repository's checks

Checks the repository runs before a commit run as they do with git. When one rejects the commit, A4 applies.

### BR-009: Unfinished merge

While a synchronization left notes in conflict (UC-015), the commit that finishes it belongs to UC-015; this use case offers no commit until the conflicts are resolved.

