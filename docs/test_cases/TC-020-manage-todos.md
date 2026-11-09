# Test Cases: Manage Todos

**Use Case:** [UC-020 Manage Todos](../use_cases/UC-020-manage-todos.md)  
**Requirements:** [FR-062, FR-063, FR-064, FR-065, FR-066, FR-068, NFR-006](../requirements.md)

## Coverage

| Test case | Flow / rule                                         | Requirements           |
|-----------|-----------------------------------------------------|------------------------|
| TC-020-01 | Main success scenario: create, fill, link, complete | FR-062 to FR-066, FR-068 |
| TC-020-02 | Fields and defaults                                  | BR-001, BR-003         |
| TC-020-03 | A1 Link another note, A2 Remove a link               | FR-066, BR-005         |
| TC-020-04 | A3 Title missing or too long                         | BR-001                 |
| TC-020-05 | A4 Reopen                                            | BR-007                 |
| TC-020-06 | A5 Delete and undo; numbers                          | BR-002                 |
| TC-020-07 | A6 Linked note renamed, deleted, missing             | BR-005                 |
| TC-020-08 | A7 Changed elsewhere                                 | BR-006                 |
| TC-020-09 | A8 Agent manages todos                               | UC-017 BR-002          |
| TC-020-10 | A9 Todos cannot be stored                            | NFR-006                |
| TC-020-11 | A10 The same todo changed on two computers           | BR-008                 |
| TC-020-12 | A11 The same number on two computers                 | BR-002                 |
| TC-020-13 | Kept with the workspace, joined per todo             | BR-004, BR-008         |

## TC-020-01: Create, fill, link, complete

**Flow:** Main success scenario  
**Preconditions:** The workspace `notes` has no todos; "Plan" is open in the active tab.

| Step | Action                                                         | Expected result                                                            |
|------|----------------------------------------------------------------|----------------------------------------------------------------------------|
| 1    | Open the section "Todos" of the tool panel.                     | No open todos; a field for a new todo.                                    |
| 2    | Type "Ship release" and press Enter.                            | "#1 Ship release", normal, without dates, in the list.                    |
| 3    | Open it; set due date 2026-11-20, urgency high, scheduled 2026-11-18. | Each shows at once; after closing and reopening Underline they are kept. |
| 4    | Choose "Link the open note".                                     | "Plan" is linked; choosing it opens Plan.                                 |
| 5    | Mark it done.                                                    | It leaves the open todos; it is stored as done with the time.            |

## TC-020-02: Fields and defaults

**Flow:** BR-001, BR-003  
**Preconditions:** A todo "#1" exists.

| Step | Action                                             | Expected result                                               |
|------|----------------------------------------------------|---------------------------------------------------------------|
| 1    | Look at a new todo.                                 | Open, normal urgency, no dates, a creation time.              |
| 2    | Choose each urgency in turn.                        | Low, normal, high, critical are offered and kept.             |
| 3    | Type the due date "2026-02-30".                     | Not taken: not a day; the field says so.                      |
| 4    | Set the scheduled date after the due date.          | Kept, with a hint.                                            |
| 5    | Remove both dates.                                  | The todo has no dates.                                        |

## TC-020-03: Link another note, remove a link

**Flow:** A1, A2, BR-005  
**Preconditions:** Notes "Plan", "Planning", "Release"; todo #1 linked to Plan.

| Step | Action                                           | Expected result                                       |
|------|--------------------------------------------------|-------------------------------------------------------|
| 1    | Choose "Link a note" and type "plan".             | Plan and Planning are offered.                        |
| 2    | Choose Planning.                                  | #1 links Plan and Planning.                           |
| 3    | Try to link Plan again.                           | Not offered again; each note once.                    |
| 4    | Remove the link to Plan.                          | #1 links Planning only; the note Plan is unchanged.   |

## TC-020-04: Title missing or too long

**Flow:** A3  
**Preconditions:** The section "Todos" is open.

| Step | Action                                   | Expected result                                              |
|------|------------------------------------------|--------------------------------------------------------------|
| 1    | Press Enter in the empty field.           | No todo; the field says a title is needed.                   |
| 2    | Enter 201 characters.                     | No todo; the field says the limit; the text stays.           |
| 3    | Clear the title of #1.                    | Not taken; the old title stays stored.                       |

## TC-020-05: Reopen

**Flow:** A4, BR-007  
**Preconditions:** #1 is done.

| Step | Action           | Expected result                                                  |
|------|------------------|------------------------------------------------------------------|
| 1    | Mark it open.     | Among the open todos again, without done time; fields and links kept. |

## TC-020-06: Delete and undo; numbers

**Flow:** A5, BR-002  
**Preconditions:** Todos #1, #2, #3; #2 links Plan.

| Step | Action                         | Expected result                                                   |
|------|--------------------------------|-------------------------------------------------------------------|
| 1    | Delete #2.                      | Gone; the status bar reports it with "Undo".                     |
| 2    | Choose "Undo".                  | #2 is back with its fields and link to Plan.                     |
| 3    | Delete #3, then create a todo.  | The new todo is #4, not #3.                                      |

## TC-020-07: Linked note renamed, deleted, missing

**Flow:** A6, BR-005  
**Preconditions:** #1 links Plan.

| Step | Action                                         | Expected result                                                   |
|------|------------------------------------------------|-------------------------------------------------------------------|
| 1    | Rename Plan to "Project Plan" in "Work".        | #1 links Work/Project Plan.                                       |
| 2    | Delete Work/Project Plan in Underline.          | Refused; #1 is named (UC-007 A2).                                 |
| 3    | Delete its file outside Underline.              | #1 shows the link as missing; choosing it says so; it can be removed. |

## TC-020-08: Changed elsewhere

**Flow:** A7, BR-006  
**Preconditions:** The section "Todos" is open; the user is typing a new title for #1.

| Step | Action                                             | Expected result                                                     |
|------|----------------------------------------------------|---------------------------------------------------------------------|
| 1    | Change the due date of #2 from another program or an agent. | Within 1 s the list shows it.                              |
| 2    | Look at #1.                                         | What the user typed stays.                                         |

## TC-020-09: Agent manages todos

**Flow:** A8  
**Preconditions:** Claude Code is connected; todo #1 exists.

| Step | Action                                                  | Expected result                                                        |
|------|---------------------------------------------------------|------------------------------------------------------------------------|
| 1    | Access read only: ask the agent to list the todos, then to complete #1. | It lists them; completing is refused with the reason.      |
| 2    | Access read and write: ask it to create "Review" linked to Plan and complete #1. | Both happen; within 1 s the list shows them; the status bar names Claude Code. |
| 3    | Access off: ask it to list the todos.                    | Refused with the reason.                                               |

## TC-020-10: Todos cannot be stored

**Flow:** A9, NFR-006  
**Preconditions:** The todos of the workspace cannot be written.

| Step | Action                     | Expected result                                                         |
|------|----------------------------|-------------------------------------------------------------------------|
| 1    | Change the title of #1.     | Shown, marked not stored, with the reason; the stored todos unchanged. |
| 2    | Make them writable again.   | It is stored; the mark goes.                                           |

## TC-020-11: The same todo changed on two computers

**Flow:** A10, BR-008  
**Preconditions:** A and B share the workspace; #1 "Ship" with no due date.

| Step | Action                                                       | Expected result                                                         |
|------|--------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | On B, set #1's due date and sync. On A, set #1's urgency to high; sync. | A's sync stops: "1 conflict"; #1 is marked, both versions shown field by field. |
| 2    | Choose "Take remote".                                         | #1 has B's due date and normal urgency; the sync completes.             |
| 3    | Repeat step 1, then abort the sync.                           | #1 is A's version; nothing pushed.                                      |
| 4    | On B delete #2; on A change #2; sync on A.                    | A conflict for #2; "Keep mine" keeps it.                                |

## TC-020-12: The same number on two computers

**Flow:** A11, BR-002  
**Preconditions:** A and B have todos up to #11, both offline.

| Step | Action                                     | Expected result                                                 |
|------|--------------------------------------------|-----------------------------------------------------------------|
| 1    | B creates "Remote" (#12) and syncs.         | B has #12.                                                      |
| 2    | A creates "Mine" (#12) with a link, syncs.  | "Remote" is #12; "Mine" is #13 with its link; "Todo #12 is now #13". |

## TC-020-13: Kept with the workspace, joined per todo

**Flow:** BR-004, BR-008  
**Preconditions:** The workspace is a git repository with a remote.

| Step | Action                                                    | Expected result                                                     |
|------|-----------------------------------------------------------|---------------------------------------------------------------------|
| 1    | Create a todo and look at the changes (UC-011).            | The todos are among the changes; commit and sync take them along.   |
| 2    | On B change #1, on A change #2; sync both.                 | Both changes on both computers, without a conflict.                 |
| 3    | Look at the note overview.                                 | No todo is shown as a note.                                         |
