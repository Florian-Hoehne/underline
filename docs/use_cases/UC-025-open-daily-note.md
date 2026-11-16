# Use Case: Open Daily Note

## Overview

**Use Case ID:** UC-025  
**Use Case Name:** Open Daily Note  
**Primary Actor:** User  
**Goal:** The user has the daily note of today open and ready to write in, created from the daily note template the first time it is asked for that day.  
**Trigger:** The user asks for the daily note.  
**Status:** Reviewed  

**Requirements:** [FR-094, FR-095, FR-018, NFR-006, NFR-011, NFR-013](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user asks for the daily note, with its key, its button in the toolbar, or the command palette (UC-023).
2. The system finds no note titled with the date of today (BR-001) in the daily note folder of the workspace (BR-002).
3. The system creates the daily note in the daily note folder, from the daily note template with its placeholders filled in for today (BR-003), in the note format of the template, or, without a daily note template, holding only its title as heading in the default format of the workspace (BR-004).
4. The system opens the daily note in a tab in the facade, lists it first in the note overview, places the cursor where the template asks for it or else at the end of the note, and reports in the status bar that the daily note was created.

## Alternative Flows

### A1: Daily note exists

**Trigger:** The daily note folder holds a note titled with the date of today, in either note format (step 2)  
**Flow:**

1. The system opens that note, or makes its tab the active tab when it is open already, with the cursor at the end of the note (UC-004).
2. Use case ends.

### A2: Daily note of another day

**Trigger:** The user asks for the previous or the next daily note while a daily note is in the active tab (step 4)  
**Flow:**

1. The system opens the daily note of the nearest earlier or later date that exists in the daily note folder; no daily note is created for the days in between.
2. When there is none, the system says in the status bar that there is no earlier or later daily note.
3. Use case ends.

### A3: Daily note template not found

**Trigger:** The workspace settings name a daily note template that does not exist (step 3)  
**Flow:**

1. The system creates the daily note holding only its title as heading, in the default format of the workspace, and says in the status bar that the template was not found.
2. Use case continues at step 4.

### A4: Daily note folder not allowed

**Trigger:** The daily note folder lies outside the workspace, is hidden, is ignored by the git repository of the workspace, or is a workspace of its own (step 3)  
**Flow:**

1. The system says why the daily note cannot be created there, creates nothing, and offers to open the settings of the workspace (UC-010).
2. Use case ends.

### A5: Daily note cannot be created

**Trigger:** The system cannot create the folder or the note, for example for missing permission or a full disk (step 3)  
**Flow:**

1. The system reports in the status bar that the daily note could not be created and why, and removes the folders it created for it.
2. Use case ends.

## Postconditions

### Success Postconditions

- The daily note folder holds the daily note of today, created at most once that day, and it is open in the active tab and listed first in the note overview.
- Daily notes of other days are unchanged.

### Failure Postconditions

- No note and no folder is created, and no existing file is changed or replaced.

## Business Rules

### BR-001: Title of a daily note

The title of a daily note is its date as year, month, and day, `2026-11-11`, so the daily notes sort by date in every program and the file name is allowed on every platform (UC-003 BR-002, NFR-013). The date of today is the date of the clock of this computer in its time zone; the day changes at midnight.

### BR-002: Daily note folder

Daily notes are kept in the daily note folder of the workspace, `daily` unless the workspace settings name another (UC-010 BR-001). The folder is created with the first daily note. The rules for folders of new notes hold for it (UC-003 BR-006).

### BR-003: Daily note template

A new daily note starts from the daily note template: the template titled Daily in the template folder, unless the workspace settings name another (UC-010 BR-001). Its placeholders are filled in as for any template (UC-003 BR-008), with the date of the daily note as the date.

### BR-004: Note format

A daily note gets the note format of its template, or the default format of the workspace when it has no template (UC-003 BR-005).

### BR-005: A note like any other

A daily note is a note like any other: it can be edited, renamed, moved, deleted, searched, linked, and committed. A note renamed out of the date or moved out of the daily note folder is no longer found as a daily note, and asking for the daily note of that day creates a new one.

### BR-006: Created at most once

The daily note of a day is created only when the daily note folder holds no note with that title in either note format, so asking for it twice, also on two computers that share the workspace through git, never replaces a daily note. Two daily notes of the same day created on two computers before a git sync are joined as any other conflict (UC-015).

### BR-007: Keyboard

Ctrl+Shift+J (Cmd+Shift+J on macOS) opens the daily note of today; Ctrl+Shift+PageUp and Ctrl+Shift+PageDown
(Cmd+Shift+PageUp and Cmd+Shift+PageDown on macOS) open the previous and the next daily note (NFR-011, OQ-219).
