# Test Cases: Work Through Todo List

**Use Case:** [UC-021 Work Through Todo List](../use_cases/UC-021-work-through-todo-list.md)  
**Requirements:** [FR-066, FR-067, FR-068](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements   |
|-----------|----------------------------------------------------|----------------|
| TC-021-01 | Main success scenario: sort, open note, mark done   | FR-066 to FR-068 |
| TC-021-02 | Sort by due date                                    | BR-001, BR-007 |
| TC-021-03 | Sort by urgency                                     | BR-002, BR-007 |
| TC-021-04 | A1 Filters                                          | BR-003, FR-067 |
| TC-021-05 | A2 Overdue                                          | BR-004         |
| TC-021-06 | A3 No todos                                         | BR-003         |
| TC-021-07 | A4 Change a todo                                    | FR-066         |
| TC-021-08 | A5 Changed elsewhere                                | UC-020 A7      |
| TC-021-09 | A6 Missing note                                     | FR-066         |
| TC-021-10 | Kept per workspace, one tab                         | BR-005, BR-006 |
| TC-021-11 | Keyboard                                            | BR-008         |
| TC-021-12 | Linked notes in a row                               | BR-009         |

## TC-021-01: Sort, open note, mark done

**Flow:** Main success scenario  
**Preconditions:** Open todos #1 "Ship" (due 2026-11-20, normal, linked to Plan), #2 "Fix" (due 2026-11-25, critical); today is 2026-11-10.

| Step | Action                                         | Expected result                                                        |
|------|------------------------------------------------|------------------------------------------------------------------------|
| 1    | Choose "Open list" in the section "Todos".      | The tab "Todos": #1, then #2, with all columns.                        |
| 2    | Choose to sort by urgency.                      | #2, then #1.                                                           |
| 3    | Choose "Plan" in the row of #1.                 | Plan opens in its own tab; the tab "Todos" stays.                      |
| 4    | Mark #1 done in its row.                        | #1 leaves the list; "Marked todo #1 Ship done" with "Undo".            |
| 5    | Choose "Undo".                                  | #1 is open and in the list again.                                      |

## TC-021-02: Sort by due date

**Flow:** BR-001, BR-007  
**Preconditions:** #1 due 11-20 normal; #2 due 11-20 high; #3 no due date, critical; #4 due 11-15 low.

| Step | Action                                   | Expected result          |
|------|------------------------------------------|--------------------------|
| 1    | Choose the column heading "Due".          | #4, #2, #1, #3.          |

## TC-021-03: Sort by urgency

**Flow:** BR-002, BR-007  
**Preconditions:** As in TC-021-02, plus #5 high without due date.

| Step | Action                                   | Expected result              |
|------|------------------------------------------|------------------------------|
| 1    | Choose the column heading "Urgency".      | #3, #2, #5, #1, #4.          |

## TC-021-04: Filters

**Flow:** A1, BR-003  
**Preconditions:** #1 open, #2 done.

| Step | Action                         | Expected result                                       |
|------|--------------------------------|-------------------------------------------------------|
| 1    | Look at the list.               | Filter "Open": #1 only.                               |
| 2    | Choose "Done".                  | #2 only, shown done with the time.                    |
| 3    | Reopen #2.                      | It leaves "Done".                                     |
| 4    | Choose "All".                   | #1 and #2.                                            |

## TC-021-05: Overdue

**Flow:** A2, BR-004  
**Preconditions:** Today is 2026-11-10; #1 due 11-08, #2 due 11-10, #3 due 11-09 but done.

| Step | Action                              | Expected result                                                   |
|------|-------------------------------------|-------------------------------------------------------------------|
| 1    | Look at the status bar.              | "1 todo overdue".                                                 |
| 2    | Choose it.                           | The tab "Todos" opens with "Open"; #1 is marked "2 days overdue"; #2 is not marked. |
| 3    | Mark #1 done.                        | The status bar no longer shows overdue todos.                     |

## TC-021-06: No todos

**Flow:** A3  
**Preconditions:** The workspace has no todos.

| Step | Action               | Expected result                                          |
|------|----------------------|----------------------------------------------------------|
| 1    | Open the tab "Todos". | "No open todos" and an offer to create one.             |

## TC-021-07: Change a todo

**Flow:** A4  
**Preconditions:** #1 and #2 open, sorted by due date, #1 first.

| Step | Action                                                 | Expected result                                           |
|------|--------------------------------------------------------|-----------------------------------------------------------|
| 1    | Choose the title of #1.                                 | The section "Todos" shows #1 with its fields.            |
| 2    | Set #1's due date after #2's.                           | In the tab, #1 moves below #2.                           |

## TC-021-08: Changed elsewhere

**Flow:** A5  
**Preconditions:** The tab "Todos" is open; an agent is connected with read and write.

| Step | Action                                  | Expected result                         |
|------|-----------------------------------------|-----------------------------------------|
| 1    | Let the agent create a todo due today.   | Within 1 s it is in the table.          |

## TC-021-09: Missing note

**Flow:** A6  
**Preconditions:** #1 links "Old", whose file was removed outside Underline.

| Step | Action                    | Expected result                            |
|------|---------------------------|--------------------------------------------|
| 1    | Choose "Old" in the row.   | The status bar says the note no longer exists. |

## TC-021-10: Kept per workspace, one tab

**Flow:** BR-005, BR-006  
**Preconditions:** Two workspaces A and B with todos.

| Step | Action                                                  | Expected result                                       |
|------|---------------------------------------------------------|-------------------------------------------------------|
| 1    | In A sort by urgency, filter "All"; close and reopen A.  | The tab "Todos" is back, by urgency, "All".           |
| 2    | Open B.                                                  | B's own choices (by default due date, "Open").        |
| 3    | Choose "Open list" again in A.                           | The open tab "Todos" is shown; no second one.         |
| 4    | Open as many notes as allowed.                           | The tab "Todos" does not count towards the limit.     |
| 5    | Look at the workspace's changes (UC-011).                | Sort and filter are not among them.                   |

## TC-021-11: Keyboard

**Flow:** BR-008  
**Preconditions:** The tab "Todos" shows #1 (linked to Plan) and #2.

| Step | Action               | Expected result                         |
|------|----------------------|-----------------------------------------|
| 1    | Press Down.           | #2 is chosen.                           |
| 2    | Press Up, then Enter. | Plan opens.                             |
| 3    | Back in the list, press Space on #1. | #1 is done.               |

## TC-021-12: Linked notes in a row

**Flow:** BR-009  
**Preconditions:** #1 links Plan, Ideas, Road Map, and Notes.

| Step | Action             | Expected result            |
|------|--------------------|----------------------------|
| 1    | Look at the row.    | "Plan, Ideas +2".          |
