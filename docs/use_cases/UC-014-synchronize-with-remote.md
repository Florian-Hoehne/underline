# Use Case: Synchronize with Remote

## Overview

**Use Case ID:** UC-014  
**Use Case Name:** Synchronize with Remote  
**Primary Actor:** User  
**Secondary Actor:** Git remote  
**Goal:** The notes of the workspace are backed up on its remote and hold the changes made elsewhere, at a regular interval and whenever the user asks.  
**Trigger:** The sync interval passed, the workspace was opened, a commit was made, Underline is closing, or the user asks to sync, pull, or push.  
**Status:** Draft  

**Requirements:** [FR-020, FR-045, FR-052, FR-053, NFR-002, NFR-006, NFR-014, NFR-015](../requirements.md)

## Preconditions

- A workspace that is a git repository with a remote is open, on a branch, and git can be found on the computer (UC-011).

## Main Success Scenario

1. A sync starts: the interval of the settings passed since the last one (UC-010), the workspace was opened, a commit was made (UC-012), Underline is closing, or the user chose "Sync now" in the status bar.
2. The system saves the open notes with changes not yet saved and commits the changes that are not committed, with the proposed message of UC-012 (BR-002).
3. The system shows "Syncing…" in the status bar and fetches the changes of the remote branch.
4. The system joins the remote's changes with the local commits, as the user's git setup says (BR-003).
5. The system pushes the local commits to the remote branch.
6. The system shows the notes as they are now: the note overview, the open tabs, and the changes (UC-011).
7. The system shows in the status bar when the last sync succeeded, such as "Synced 2 min ago".

## Alternative Flows

### A1: Pull only, push only

**Trigger:** The user chooses "Pull" or "Push" in the branch menu (UC-013) (step 1)  
**Flow:**

1. For "Pull", the system runs steps 2–4 and 6–7 without pushing; for "Push", it runs step 2 and then pushes, without fetching first.
2. When the remote holds commits the local branch lacks, a push is refused by the remote; the system says so and offers to sync.
3. Use case ends.

### A2: Nothing to do

**Trigger:** Neither the local branch nor the remote branch has anything new (step 3)  
**Flow:**

1. The system pushes nothing and updates the time of the last sync.
2. Use case continues at step 7.

### A3: Conflict

**Trigger:** A note or file was changed both locally and on the remote in the same lines, so the changes cannot be joined (step 4)  
**Flow:**

1. The system keeps both versions and stops the sync before pushing.
2. The status bar says which notes are in conflict; UC-015 continues.
3. Use case ends.

### A4: No network

**Trigger:** The remote cannot be reached (step 3, step 5)  
**Flow:**

1. The status bar shows "Offline, will retry", with the time of the last successful sync.
2. The local commits stay; the next sync tries again.
3. Use case ends.

### A5: Sign-in missing

**Trigger:** The remote asks to sign in and git has no saved sign-in for it (step 3, step 5)  
**Flow:**

1. The status bar says that signing in to the remote failed, and how to set up a sign-in in git (BR-005).
2. Underline asks for no password; the next sync tries again.
3. Use case ends.

### A6: New branch

**Trigger:** The current branch has no branch on the remote yet, for example a branch created in UC-013 (step 3)  
**Flow:**

1. The system skips fetching and joining, pushes the branch to the remote as a new branch of the same name, and from then on follows it.
2. Use case continues at step 6.

### A7: Sync fails otherwise

**Trigger:** git refuses for another reason, for example because the remote refuses the push or another git program is busy with the repository (step 3 to step 5)  
**Flow:**

1. The status bar shows "Sync failed" with the reason in git's words, in a warning color.
2. Nothing of the notes is lost; the local commits stay; the next sync tries again.
3. Use case ends.

### A8: A sync is running

**Trigger:** A sync is asked for while one runs (step 1)  
**Flow:**

1. The system does not start a second one; when the running sync ends, it starts the asked one once.
2. Use case ends.

### A9: Closing

**Trigger:** Underline is closing (step 1)  
**Flow:**

1. The system runs the sync and closes when it ends, or after 10 seconds at the latest; the status bar says it is syncing before closing.
2. When the sync fails or takes longer, Underline closes anyway: the notes are saved and committed on this computer, and the next start syncs them.
3. Use case ends.

### A10: No sync possible

**Trigger:** The workspace has no remote, is not on a branch (UC-011 A8), or a merge waits for its conflicts (UC-015) (step 1)  
**Flow:**

1. The system does not sync; "Sync now" is not offered, and the status bar says why when the user asks.
2. Use case ends.

## Postconditions

### Success Postconditions

- The local branch and the remote branch hold the same commits; every saved change of the workspace is in them.
- The status bar shows the time of this sync.

### Failure Postconditions

- No note and no commit is lost; the notes are as they were, or, after A3, both versions are kept (UC-015).
- The status bar says why the sync did not succeed.

## Business Rules

### BR-001: When it syncs

A workspace syncs every few minutes as the settings say (UC-010: 1 to 60, 5 by default), counted from the end of the last sync; when it is opened; after every commit made in Underline; when Underline closes; and when the user asks. A sync never runs while the user waits for it in a dialog.

### BR-002: Committing before a sync

Before a sync, the changes that are not committed are committed with the message UC-012 proposes, so they are backed up. Changes the user left out in the commit field (UC-012 BR-004) stay out until the user commits them or chooses them again. No commit is made when nothing changed, or while a merge waits (UC-015).

### BR-003: Joining

The remote's changes are joined with the local commits as the user's git setup says: by merging, or by putting the local commits on top of the remote's when git is set up to do so. Changes to different notes, and to different lines of one note, are joined without asking; changes to the same lines are a conflict (A3, UC-015).

### BR-004: Status bar

The status bar shows the state of the last sync next to the branch (FR-020): "Syncing…", "Synced 2 min ago", "Offline, will retry", or "Sync failed: …" in a warning color; and, when the branches differ, how many commits wait to be pushed (↑) and how many to be pulled (↓). "Sync now" sits next to it.

### BR-005: Signing in

Underline signs in to the remote only through git and the sign-in the user set up for git, such as a saved token or an SSH key; it asks for no password and keeps none. A failed sign-in names the remote and points to setting up a sign-in in git.

### BR-006: Network

A sync is the only time the workspace's notes leave the computer, and only to the workspace's remote (NFR-015). Everything else works without a network (NFR-014).

### BR-007: The remote

A branch syncs with the remote branch it follows. A new branch goes to the remote the workspace has, or, with several, to the one named `origin`, else the first.

### BR-008: Typing goes on

A sync runs in the background: typing, saving, and reading go on while it runs (NFR-002). When it changes a note that is open, the tab shows the note as it is now, as for a change from outside (UC-004 A7).
