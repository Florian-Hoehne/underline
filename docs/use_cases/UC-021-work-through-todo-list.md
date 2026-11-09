# Use Case: Work Through Todo List

## Overview

**Use Case ID:** UC-021  
**Use Case Name:** Work Through Todo List  
**Primary Actor:** User  
**Goal:** The user sees the todos of the workspace in one list, sorted by due date or urgency and filtered by open or done, notices what is overdue, opens the notes a todo is about, and marks todos done one after the other.  
**Trigger:** The user opens the tab "Todos", or chooses the overdue todos in the status bar.  
**Status:** Draft  

**Requirements:** [FR-066, FR-067, FR-068](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user opens the tab "Todos", from the section "Todos" of the tool panel (UC-020).
2. The system shows the open todos of the workspace in a table, one row each, with number, title, due date, urgency, scheduled date, and linked notes, sorted by due date.
3. The user chooses to sort by urgency.
4. The system sorts the rows by urgency, the most urgent first, and keeps the choice for the workspace.
5. The user chooses a linked note in a row (FR-066).
6. The system opens the note in a tab of its own; the tab "Todos" stays open.
7. When the work is done, the user marks the todo done in its row (FR-068).
8. The system stores it as done (UC-020 step 10), and the row leaves the list of open todos.

## Alternative Flows

### A1: Show done or all todos

**Trigger:** The user chooses the filter "Done" or "All" instead of "Open" (step 2)  
**Flow:**

1. The system shows only the done todos, or all todos, in the sort chosen; done todos are shown as done, with the time they were done.
2. Marking a done todo open again (UC-020 A4) keeps it in the list under "All" and removes it under "Done".
3. The system keeps the filter for the workspace.
4. Use case continues at step 3.

### A2: Overdue todos

**Trigger:** Open todos have a due date before today (step 2)  
**Flow:**

1. The rows of those todos are marked as overdue, with how many days.
2. The status bar shows how many todos are overdue, such as "2 todos overdue"; choosing it opens the tab "Todos" with the filter "Open".
3. Use case continues at step 3.

### A3: No todos

**Trigger:** No todo matches the filter (step 2)  
**Flow:**

1. The table says there are no open (or done) todos and offers to create one (UC-020 step 3).
2. Use case ends.

### A4: Change a todo

**Trigger:** The user chooses the title of a todo in a row (step 5)  
**Flow:**

1. The system shows the todo in the section "Todos" of the tool panel with its fields (UC-020 step 5), where the user changes it.
2. The row shows the change at once; when the change moves the todo in the sort, the row moves.
3. Use case continues at step 5.

### A5: Changed elsewhere

**Trigger:** The todos change outside the tab, in the section "Todos", by a git sync, or by an agent (UC-020 A7, A8)  
**Flow:**

1. Within 1 second, the table shows the todos as they are now, in the sort and filter chosen.
2. Use case continues at step 3.

### A6: Missing note

**Trigger:** The user chooses a linked note that no longer exists (step 5)  
**Flow:**

1. The system says so, as in UC-020 A6.
2. Use case continues at step 5.

## Postconditions

### Success Postconditions

- The todos the user marked are stored as done; the sort and filter chosen are kept for the workspace.

### Failure Postconditions

- The todos are as they were.

## Business Rules

### BR-001: Sort by due date

Sorted by due date, the earliest due date comes first and todos without one come last; among todos due the same day, the more urgent comes first, and then the lower number.

### BR-002: Sort by urgency

Sorted by urgency, critical comes first, then high, normal, and low; among todos of the same urgency, the earlier due date comes first, todos without one after them, and then the lower number.

### BR-003: Filters

The filters are "Open" (the default), "Done", and "All".

### BR-004: Overdue

An open todo is overdue when its due date is before today, by the computer's calendar; a todo due today is not. The status bar shows the number of overdue todos only while there is at least one.

### BR-005: Kept per workspace

The sort and the filter last chosen are kept for each workspace on this computer, as the open tabs are (UC-002), and are not shared through git. The tab "Todos" is restored with the other tabs when the workspace is opened again.

### BR-006: One tab

There is at most one tab "Todos" per window; opening it again shows the open one.
