# Use Case: Write Note as Agent

## Overview

**Use Case ID:** UC-019  
**Use Case Name:** Write Note as Agent  
**Primary Actor:** Agent  
**Secondary Actor:** User  
**Goal:** An agent creates a note or changes part of a note through Underline's MCP server, in the note's format, and the user sees the change and where the agent writes in the open note, without losing anything typed.  
**Trigger:** The agent asks the MCP server to create a note or to change one.  
**Status:** Draft  

**Requirements:** [FR-056, FR-057, FR-058, NFR-006, NFR-008, NFR-015, NFR-016](../requirements.md)

## Preconditions

- The agent is connected to the MCP server of the workspace (UC-016, UC-017).
- Agent access is read and write (UC-017 BR-002).

## Main Success Scenario

1. The agent asks to change a note it has read (UC-018), naming the note and one of these changes: replace a passage it quotes exactly, insert text before a line or at the end, or replace the whole text.
2. The system finds the note and the place of the change, and stores the changed note whole (NFR-006).
3. The system answers that the note was changed, with the lines that now hold the change.
4. While the note is open in a tab, the tab shows the change within 1 second (NFR-008), and the agent cursor marks the changed lines with the agent's name (FR-057); the user's cursor, selection, and scroll position stay where they were.
5. The user goes on reading or editing; the agent's change is one step the user can undo in the tab.

## Alternative Flows

### A1: Create a note

**Trigger:** The agent asks to create a note with a title, a folder if any, and its text (step 1)  
**Flow:**

1. The system creates the note by the rules of UC-003 BR-001 to BR-006, in the workspace's default format unless the agent names the other note format, with the text the agent gives.
2. The system answers that the note was created, with its name as listed (UC-018 BR-002).
3. The note appears in the note overview within 1 second; it is not opened in a tab, and the status bar reports it (BR-010).
4. Use case ends.

### A2: Note exists

**Trigger:** The agent asks to create a note whose title is taken in that folder (A1 step 1)  
**Flow:**

1. The system refuses with the reason and changes nothing, so the agent can change the existing note instead.
2. Use case ends.

### A3: Title or folder not allowed

**Trigger:** The title or folder breaks UC-003 BR-002 or BR-006 (A1 step 1)  
**Flow:**

1. The system refuses with the rule broken and changes nothing.
2. Use case ends.

### A4: Passage not found or not unique

**Trigger:** The passage to replace does not occur in the note as stored, or occurs more than once (step 2)  
**Flow:**

1. The system refuses and says whether the passage was not found or how often it occurs, so the agent can read the note again and quote more of it.
2. Use case ends.

### A5: The user changed the same place

**Trigger:** The note is open in a tab with changes not yet saved, and the agent's change touches a line the user changed (step 2)  
**Flow:**

1. The system refuses with the reason that the user is editing that place, and changes nothing.
2. Use case ends; the agent reads the note again (UC-018) once the user's changes are saved.

### A6: The user changed elsewhere

**Trigger:** The note is open in a tab with changes not yet saved, at other lines than the agent's change (step 2)  
**Flow:**

1. The system puts the agent's change into the tab, keeping what the user typed, and stores the note with both.
2. Use case continues at step 3.

### A7: Note in conflict

**Trigger:** The note holds places in conflict from a git sync (UC-015) (step 2)  
**Flow:**

1. The system refuses with the reason that the user has to resolve the conflicts first, and changes nothing; reading goes on (UC-018 A8).
2. Use case ends.

### A8: Read only or off

**Trigger:** Agent access is read only or off (step 1, A1)  
**Flow:**

1. The system refuses with the reason (UC-017 A6, BR-001) and changes nothing.
2. Use case ends.

### A9: Note cannot be written

**Trigger:** The note's file or folder cannot be written, for example for missing permission or a full disk (step 2, A1)  
**Flow:**

1. The system refuses with the reason; the note is as it was.
2. Use case ends.

### A10: No such note

**Trigger:** The agent names a note the workspace does not hold, or a title several notes have (step 1)  
**Flow:**

1. The system answers as in UC-018 A2 and A3 and changes nothing.
2. Use case ends.

### A11: The user undoes the change

**Trigger:** The user undoes in the tab after the agent's change (step 5)  
**Flow:**

1. The tab takes back the agent's change as one step, without the user's own changes around it, and the note is saved so (UC-005).
2. The agent sees the note without its change from its next reading on.
3. Use case ends.

## Postconditions

### Success Postconditions

- The note holds the agent's change, stored whole, and everything else as before, including the user's own changes.
- An open tab shows it; the change is listed among the workspace's changes (UC-011) like any other.

### Failure Postconditions

- The note is as it was; the agent has the reason.

## Business Rules

### BR-001: The note's format

The agent writes the text in the note's format, Markdown or AsciiDoc, as stored; Underline stores it as given and converts nothing, except that line breaks follow those the note already has.

### BR-002: What an agent may change

An agent creates notes and changes their text. It does not rename, move, or delete notes, and changes no settings and nothing outside the notes of the workspace.

### BR-003: Exact passages

A passage to replace must occur exactly once in the note as stored, character for character; a line to insert before is counted from 1 (UC-018 BR-009), and a line after the last one inserts at the end.

### BR-004: The user's typing comes first

What the user typed is never lost through an agent: a change that touches a line the user changed and has not saved yet is refused (A5); others are merged into the tab (A6), and the note is then stored with both, the user's changes included. A change touches a line when it replaces or removes it, or inserts right before or after it; replacing the whole text touches every line. While the Underline window is closed, there is nothing unsaved, and the change is stored directly (UC-017 BR-007).

### BR-005: Notes in conflict

A note holding places in conflict (UC-015) is not changed by an agent until the user has resolved them (OQ-156).

### BR-006: The agent cursor while writing

While the note is open, the agent cursor marks the lines of the agent's last change and names the agent, until the agent turns to another note or place, or for at most 10 seconds after the change, as while reading (UC-018 BR-006). It shows only while access is read and write and the agent is connected (UC-017 BR-005, FR-058); it never moves the user's cursor or scroll position.

### BR-007: One step to undo

Each change of an agent is one step of the tab's undo, apart from the user's own typing (A11). A change made while the note is not open cannot be undone in a tab; git keeps the earlier version once committed (UC-012).

### BR-008: Speed

The agent gets its answer within 1 second; an open tab shows the change within 1 second (NFR-008).

### BR-009: Local only

Changes come only from an agent connected through the MCP server on this computer (NFR-016).

### BR-010: Changes to notes not open

When an agent creates or changes a note that is not open in a tab, the status bar says so with the agent's name and the note, such as "Claude Code changed Plan", and offers to open it; several changes in a row are reported together. The note is also listed among the workspace's changes (UC-011).
