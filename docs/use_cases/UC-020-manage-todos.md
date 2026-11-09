# Use Case: Manage Todos

## Overview

**Use Case ID:** UC-020  
**Use Case Name:** Manage Todos  
**Primary Actor:** User  
**Secondary Actor:** Agent  
**Goal:** The user keeps the work they are heading towards as todos next to the notes, each with a title, a due date, an urgency, a scheduled date, and links to the notes it is about, and marks a todo done when it is done.  
**Trigger:** The user opens the section "Todos" of the tool panel and creates, changes, completes, or deletes a todo.  
**Status:** Draft  

**Requirements:** [FR-062, FR-063, FR-064, FR-065, FR-066, FR-068, NFR-006](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user opens the section "Todos" of the tool panel.
2. The system shows the open todos of the workspace, each with its number, title, due date, and urgency (the list itself, its order and filters, is UC-021).
3. The user types a title into the field for a new todo and confirms.
4. The system creates the todo: open, with the urgency normal and no dates, and the next free number (FR-062); it stores it at once and shows it in the list.
5. The user opens the todo and gives it a due date, an urgency, and a scheduled date (FR-063, FR-064, FR-065).
6. The system stores each change at once and shows it.
7. The user links the todo to the note open in the active tab (FR-066).
8. The system stores the link and shows the note's title in the todo; choosing it opens the note.
9. When the work is done, the user marks the todo as done (FR-068).
10. The system stores it as done with the time it was done; the todo leaves the list of open todos.

## Alternative Flows

### A1: Link another note

**Trigger:** The user wants to link a note that is not open (step 7)  
**Flow:**

1. The user chooses "Link a note" and types part of its title.
2. The system offers the notes whose titles match, as the note overview names them.
3. The user chooses one; the system stores the link.
4. Use case continues at step 9.

### A2: Remove a link

**Trigger:** The user removes a note from the todo (step 8)  
**Flow:**

1. The system removes the link only; the note stays as it is.
2. Use case continues at step 9.

### A3: Title missing or too long

**Trigger:** The title is empty or longer than 200 characters (step 3, step 5)  
**Flow:**

1. The system does not create or change the todo and says why; what the user typed stays in the field.
2. Use case continues at step 3.

### A4: Reopen

**Trigger:** The user marks a done todo as open again (step 10)  
**Flow:**

1. The system stores it as open, without the time it was done, and it is among the open todos again.
2. Use case ends.

### A5: Delete

**Trigger:** The user deletes a todo (steps 5 to 10)  
**Flow:**

1. The system removes the todo and its links at once, and the status bar offers "Undo" while it reports the deletion.
2. When the user chooses "Undo", the todo comes back with its number, fields, and links.
3. Use case ends.

### A6: Linked note renamed, moved, or deleted

**Trigger:** A linked note is renamed or moved (UC-006), or someone tries to delete it (UC-007)  
**Flow:**

1. A renamed or moved note stays linked under its new name (UC-006 BR-006).
2. A linked note cannot be deleted in Underline until its todo links are removed (UC-007 A2).
3. A linked note that disappeared outside Underline, for example by git, is shown in the todo as missing; choosing it says so, and the user can remove the link (A2).
4. Use case ends.

### A7: Changed elsewhere

**Trigger:** The todos changed outside this window, for example by a git sync or by an agent (step 2)  
**Flow:**

1. Within 1 second, the list shows the todos as they are now; a todo the user is changing keeps what the user typed.
2. Use case continues at step 2.

### A8: Agent manages todos

**Trigger:** A connected agent asks the MCP server for the todos, or to create, change, complete, or delete one (UC-017)  
**Flow:**

1. While agent access is read only, the agent can list and read todos; while it is read and write, it can also create, change, complete, reopen, link, and delete them, by the same rules as the user; while it is off, it is refused (UC-017 BR-001).
2. The list shows the agent's change within 1 second (A7), and the status bar names the agent and the todo.
3. Use case ends.

### A9: Todos cannot be stored

**Trigger:** The todos of the workspace cannot be written, for example for missing permission (steps 4, 6, 8, 10)  
**Flow:**

1. The system keeps the change in the window, marks the todo as not stored, says why, and tries again, as for notes (UC-005 A14).
2. Use case ends.

## Postconditions

### Success Postconditions

- The todo is stored in the workspace with its fields and links, whole (NFR-006), and shared through git with the notes.

### Failure Postconditions

- The stored todos are as they were; the window says what was not stored.

## Business Rules

### BR-001: Fields of a todo

A todo has a number, a title of 1 to 200 characters, an optional due date, an urgency (low, normal, high, or critical), an optional scheduled date, a status (open or done), the time it was created, and, while done, the time it was done (entity model, TODO). A new todo is open, normal, without dates.

### BR-002: Numbers

Each todo has a number within its workspace, shown as `#12`, that never changes and is never given to another todo of the workspace. A number freed by a deletion is not used again.

### BR-003: Dates

Due date and scheduled date are days without a time, entered by typing or by choosing in a calendar, and shown in the user's date format. A scheduled date after the due date is allowed and shown as a hint.

### BR-004: Kept with the workspace

The todos are kept in the workspace, so they travel with git sync (UC-014) and are part of the workspace's changes (UC-011) and commits (UC-012) like notes, and an agent or another computer sees them.

### BR-005: Links

A todo links to none, one, or several notes of its own workspace, each once. Links follow renamed and moved notes (UC-006 BR-006); a linked note cannot be deleted in Underline (UC-007 BR-009).

### BR-006: Stored at once

Every change of a todo is stored within 1 second, whole or not at all (NFR-006); there is nothing to save by hand.

### BR-007: Done

Marking a todo done records the time; reopening removes it. A done todo keeps its fields and links.
