# Use Case: Look Up Notes

## Overview

**Use Case ID:** UC-018  
**Use Case Name:** Look Up Notes  
**Primary Actor:** Agent  
**Secondary Actor:** User  
**Goal:** An agent finds out which notes the workspace holds, finds the notes that concern its work, and reads them, in their note format, so it can use them; the user sees in an open note where the agent reads.  
**Trigger:** The agent asks Underline's MCP server for the notes, for a search, or for a note.  
**Status:** Draft  

**Requirements:** [FR-054, FR-055, FR-057, FR-058, NFR-003, NFR-015, NFR-016](../requirements.md)

## Preconditions

- The agent is connected to the MCP server of the workspace (UC-016, UC-017).
- Agent access is read only or read and write (UC-017 BR-002).

## Main Success Scenario

1. The agent asks for the notes of the workspace.
2. The system answers with every note of the workspace, each with its title, its folder, and its note format, the most recently changed first (BR-009).
3. The agent searches the notes for words.
4. The system answers with the notes that match, each with the lines found and their line numbers, by the rules of the search of UC-008.
5. The agent asks to read a note, naming it as listed.
6. The system answers with the note's text as it is stored, in its note format, with the note format and the number of its lines.
7. While the note is open in a tab, the system shows the agent cursor at the lines read, with the agent's name (FR-057).

## Alternative Flows

### A1: Only some lines

**Trigger:** The agent asks for a range of lines of a note, for example of a long one (step 5)  
**Flow:**

1. The system answers with those lines only, with their line numbers and the number of lines the note has.
2. When the range goes beyond the end of the note, the system answers with the lines there are and says where the note ends.
3. Use case continues at step 7.

### A2: No such note

**Trigger:** The agent names a note the workspace does not hold (step 5)  
**Flow:**

1. The system answers that there is no such note, and names up to five notes whose titles come closest, so the agent can choose.
2. Use case continues at step 5.

### A3: Same title in several folders

**Trigger:** The agent names a note by its title alone, and notes of that title are in several folders (step 5)  
**Flow:**

1. The system answers with the notes of that title and their folders, and reads none.
2. Use case continues at step 5.

### A4: Nothing found

**Trigger:** No note matches the words (step 4)  
**Flow:**

1. The system answers that no note matches, so the agent can search for other words.
2. Use case continues at step 3.

### A5: Many found

**Trigger:** More notes match, or more lines in a note, than one answer holds (step 4)  
**Flow:**

1. The system answers with the first 50 notes, each with its first 3 lines found, in the order of UC-008 BR-003, and says how many more notes and lines there are, so the agent can search more narrowly or read the note.
2. Use case continues at step 5.

### A6: Access off

**Trigger:** Agent access is off when the agent asks (steps 1, 3, 5)  
**Flow:**

1. The system refuses with the reason that access is switched off in Underline (UC-017 BR-001); nothing of the notes is answered.
2. Use case ends.

### A7: Note cannot be read

**Trigger:** The note's file cannot be read, for example for missing permission or because it was deleted in between (step 5)  
**Flow:**

1. The system answers with the reason and changes nothing.
2. Use case ends.

### A8: Note in conflict

**Trigger:** The note holds places in conflict from a git sync (UC-015) (step 5)  
**Flow:**

1. The system answers with the note as it is stored, with both versions of each place, and says that the note holds conflicts the user has not resolved yet.
2. Use case continues at step 7.

### A9: One folder only

**Trigger:** The agent asks for the notes of one folder (step 1)  
**Flow:**

1. The system answers with the notes of that folder and of the folders in it, as in step 2; for a folder that does not exist, it says so.
2. Use case continues at step 3.

## Postconditions

### Success Postconditions

- The agent has the list, the places found, or the text it asked for, as stored at the moment of asking.
- Nothing in the workspace has changed.

### Failure Postconditions

- The agent has the reason; nothing in the workspace has changed.

## Business Rules

### BR-001: Which notes

The notes of the workspace are those of the note overview (UC-002): every Markdown and AsciiDoc file of the workspace, also in folders. Deleted notes (UC-007), hidden folders, files that are not notes, and folders that are workspaces of their own are not listed, searched, or read.

### BR-002: Naming a note

A note is named by its folder and title as listed, such as `Projects/Plan`; the title alone is enough while it is unique in the workspace (A3). Upper and lower case of the name do not matter.

### BR-003: As stored

The agent gets every note as it is stored. Changes the user has typed but not yet saved are not part of it; they are saved within a second (UC-005), and the agent sees them from its next request on. So the agent gets the same, whether the Underline window is open or not (UC-017 BR-007).

### BR-004: The search

The search follows UC-008 BR-002 to BR-004: every word must occur in the title, the folder, or the text, in any order and any case; words in double quotes are one; lines found are given as stored. It searches the stored text (BR-003).

### BR-005: Only reading

Looking up changes no note, no setting, and nothing the window remembers, such as the recently opened notes; the note overview is not reordered by it.

### BR-006: The agent cursor while reading

While a note the agent reads is open in a tab, the agent cursor marks the lines read and names the agent (FR-057), until the agent turns to another note or place, or for at most 10 seconds after the last request about it; it never moves the user's cursor or scroll position. It shows at every level of access that lets the agent read, read only included, and is hidden while access is off or no agent is connected (FR-058, UC-017 BR-005). Several agents each have their own, named.

### BR-007: Speed

The list and the search answer within 1 second in a workspace of 10,000 notes (NFR-003); reading a note answers within 1 second.

### BR-008: Local only

Notes reach only the agent connected through the MCP server on this computer (NFR-015, NFR-016).

### BR-009: Order and lines

The list gives the most recently changed note first, so an agent sees the current work first; the window's order of recently opened notes stays the user's (BR-005). Lines are counted from 1, as the line numbers of the source view show them (UC-005).
