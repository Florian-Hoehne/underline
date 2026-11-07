# Test Cases: Delete Note

**Use Case:** [UC-007 Delete Note](../use_cases/UC-007-delete-note.md)  
**Requirements:** [FR-006, FR-017, FR-018, NFR-006, NFR-011](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements           |
|-----------|------------------------------------------------------|------------------------|
| TC-007-01 | Main success scenario from the note overview         | FR-006, BR-001, BR-004 |
| TC-007-02 | Delete the note of an open tab with unsaved changes  | FR-018, BR-008         |
| TC-007-03 | Links stay and lead nowhere                          | BR-001, BR-002         |
| TC-007-04 | Files of the note stay                               | BR-003                 |
| TC-007-05 | A1 User cancels                                      | —                      |
| TC-007-06 | A2 Note is linked by a todo                          | BR-009                 |
| TC-007-07 | A3 Note cannot be deleted                            | BR-008, NFR-006        |
| TC-007-08 | A4 Undo right after deleting                         | BR-006                 |
| TC-007-09 | A5 Restore from the deleted notes                    | BR-006                 |
| TC-007-10 | A6 Place of a restored note is taken                 | BR-006, UC-006 BR-001  |
| TC-007-11 | A7 Empty the deleted notes                           | BR-005                 |
| TC-007-12 | A8 Deleted notes cannot be kept                      | BR-004                 |
| TC-007-13 | Deleted notes are not shared and not listed          | BR-004                 |
| TC-007-14 | Thirty days                                          | BR-005                 |
| TC-007-15 | Remembered tabs and recently opened notes            | FR-017, FR-018         |
| TC-007-16 | Home note                                            | BR-010                 |
| TC-007-17 | Delete key and keyboard alone                        | BR-007, NFR-011        |

## TC-007-01: Delete a note from the note overview

**Flow:** Main success scenario, BR-001, BR-004  
**Preconditions:** The workspace `notes` is a git repository with nothing to commit and holds `Old-Ideas.md`, which no note links to and no tab shows.

| Step | Action                                                 | Expected result                                                                                     |
|------|--------------------------------------------------------|-----------------------------------------------------------------------------------------------------|
| 1    | Choose Delete in the context menu of "Old Ideas" in the note overview. | A question names "Old Ideas" and says that no links lead to it.                       |
| 2    | Confirm.                                               | "Old Ideas" is gone from the note overview; the status bar reports it deleted and offers to undo.   |
| 3    | Look at the workspace folder with another program.      | `Old-Ideas.md` is gone.                                                                            |
| 4    | Look at the changes to commit.                          | `Old-Ideas.md` is deleted; nothing else changed, the deleted notes do not show up.                  |
| 5    | Open the deleted notes at the end of the note overview. | "Old Ideas" is listed with the top folder and the time of step 2.                                   |

## TC-007-02: Delete the note of an open tab with unsaved changes

**Flow:** Main success scenario steps 4–5, BR-008  
**Preconditions:** `Draft.md` is open in the active tab of three; the user has just typed "last words" and autosave has not run yet.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Choose Delete in the context menu of the tab and confirm. | The tab closes; the tab right of it becomes active.          |
| 2    | Restore "Draft" from the deleted notes.         | `Draft.md` is back and ends with "last words".                          |

## TC-007-03: Links stay and lead nowhere

**Flow:** BR-001, BR-002  
**Preconditions:** `Plan.md` exists; `Ideas.md` links to it twice and `team/Review.md` once.

| Step | Action                                         | Expected result                                                                      |
|------|------------------------------------------------|--------------------------------------------------------------------------------------|
| 1    | Ask to delete "Plan".                          | The question says that 3 links in 2 notes lead to it.                                |
| 2    | Confirm, then open `Ideas.md`.                  | Its links are unchanged.                                                             |
| 3    | Follow one of the links.                        | Underline offers to create the note "Plan" (UC-004 A3).                              |
| 4    | Decline, restore "Plan", and follow the link again. | The restored note opens.                                                        |

## TC-007-04: Files of the note stay

**Flow:** BR-003  
**Preconditions:** `Trip.md` shows `img/beach.png`, which no other note uses.

| Step | Action                         | Expected result                    |
|------|--------------------------------|------------------------------------|
| 1    | Delete "Trip" and confirm.     | `Trip.md` is gone.                 |
| 2    | Look at the folder `img`.      | `img/beach.png` is still there.    |

## TC-007-05: User cancels

**Flow:** A1  
**Preconditions:** `Plan.md` is open in a tab.

| Step | Action                              | Expected result                                                      |
|------|-------------------------------------|----------------------------------------------------------------------|
| 1    | Ask to delete "Plan" and cancel.    | The question closes; the note, its tab, and the overview are unchanged. |
| 2    | Ask again and press Escape.         | The same.                                                            |

## TC-007-06: Note is linked by a todo

**Flow:** A2, BR-009  
**Preconditions:** The todo "Prepare talk" links to `Talk.md` (UC-020).

| Step | Action                      | Expected result                                                                  |
|------|-----------------------------|----------------------------------------------------------------------------------|
| 1    | Ask to delete "Talk".       | Underline says the note cannot be deleted while "Prepare talk" links to it.      |
| 2    | Look at the workspace.       | `Talk.md` is unchanged.                                                          |

## TC-007-07: Note cannot be deleted

**Flow:** A3, BR-008, NFR-006  
**Preconditions:** `locked/Plan.md` is in a folder without permission to write; it is open in a tab.

| Step | Action                              | Expected result                                                                  |
|------|-------------------------------------|----------------------------------------------------------------------------------|
| 1    | Delete "Plan" and confirm.          | The status bar says the note could not be deleted and why.                       |
| 2    | Look at the workspace and the tab.  | `locked/Plan.md` is unchanged, its tab is open, and the deleted notes do not list it. |

## TC-007-08: Undo right after deleting

**Flow:** A4, BR-006  
**Preconditions:** `Plan.md` is open in the second of three tabs.

| Step | Action                                 | Expected result                                                             |
|------|----------------------------------------|-----------------------------------------------------------------------------|
| 1    | Delete "Plan" and confirm.             | The tab closes; the status bar offers to undo.                              |
| 2    | Choose undo.                           | `Plan.md` is back unchanged, open in a tab again, listed in the note overview. |
| 3    | Open the deleted notes.                 | "Plan" is not listed.                                                       |
| 4    | Delete "Plan" again, then open another note. | The status bar reports the note opened; the undo is no longer offered.  |

## TC-007-09: Restore from the deleted notes

**Flow:** A5, BR-006  
**Preconditions:** "Old" (top folder) was deleted yesterday, "guide/Setup" today; the folder `guide` was removed afterwards.

| Step | Action                                   | Expected result                                                              |
|------|------------------------------------------|------------------------------------------------------------------------------|
| 1    | Open the deleted notes.                   | "Setup" in `guide` is listed first, then "Old", each with the time of deleting. |
| 2    | Restore "Setup".                          | `guide/Setup.md` is back with its content; the folder `guide` was created.   |
| 3    | Look at the note overview and the tabs.    | "Setup" is listed; no tab was opened.                                       |
| 4    | Look at the deleted notes.                 | Only "Old" is listed.                                                       |

## TC-007-10: Place of a restored note is taken

**Flow:** A6, BR-006  
**Preconditions:** "Plan" was deleted; a new note "plan" was created in the top folder since.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Restore the deleted "Plan".                    | Underline says a note with that title exists and asks for a title and folder. |
| 2    | Enter "Plan Old" and confirm.                   | `Plan-Old.md` holds the deleted content; `plan.md` is unchanged.        |
| 3    | Restore another taken note and cancel.          | It stays among the deleted notes.                                       |

## TC-007-11: Empty the deleted notes

**Flow:** A7, BR-005  
**Preconditions:** Three notes are among the deleted notes.

| Step | Action                                             | Expected result                                                  |
|------|----------------------------------------------------|------------------------------------------------------------------|
| 1    | Remove one of them for good and confirm.           | It is no longer listed; the other two are.                        |
| 2    | Ask to empty the deleted notes.                     | The question says 2 notes will be removed for good.              |
| 3    | Cancel.                                             | Both are still listed.                                           |
| 4    | Empty them again and confirm.                       | No deleted notes are listed.                                     |

## TC-007-12: Deleted notes cannot be kept

**Flow:** A8, BR-004  
**Preconditions:** The Underline folder of the workspace cannot be written. `Plan.md` exists.

| Step | Action                            | Expected result                                                                  |
|------|-----------------------------------|----------------------------------------------------------------------------------|
| 1    | Delete "Plan" and confirm.        | Underline says the note cannot be restored afterwards and asks again.            |
| 2    | Cancel.                           | `Plan.md` is unchanged.                                                          |
| 3    | Delete again and confirm twice.   | `Plan.md` is gone; the status bar reports it deleted without offering to undo.   |

## TC-007-13: Deleted notes are not shared and not listed

**Flow:** BR-004  
**Preconditions:** The workspace is a git repository with a remote; "Plan" was deleted.

| Step | Action                                                     | Expected result                                                    |
|------|------------------------------------------------------------|--------------------------------------------------------------------|
| 1    | Look at the note overview.                                  | "Plan" is not listed among the notes.                              |
| 2    | Commit and push all changes, then clone the remote elsewhere and open the clone. | The clone holds no `Plan.md`, and its deleted notes are empty. |

## TC-007-14: Thirty days

**Flow:** BR-005  
**Preconditions:** Among the deleted notes, "Old" was deleted 31 days ago and "Recent" 29 days ago.

| Step | Action                              | Expected result                                  |
|------|-------------------------------------|--------------------------------------------------|
| 1    | Open the workspace.                 | The deleted notes list "Recent", not "Old".      |

## TC-007-15: Remembered tabs and recently opened notes

**Flow:** Main success scenario step 5, FR-017, FR-018  
**Preconditions:** `Plan.md` is open in a tab and is the most recently opened note.

| Step | Action                            | Expected result                                                 |
|------|-----------------------------------|-----------------------------------------------------------------|
| 1    | Delete "Plan" and confirm.        | The tab closes.                                                 |
| 2    | Restart Underline.                | No tab of "Plan" opens and the note overview does not list it.  |
| 3    | Restore "Plan".                   | It is listed in the note overview, not first.                   |

## TC-007-16: Home note

**Flow:** BR-010  
**Preconditions:** The workspace holds `Home.md` and `Plan.md`.

| Step | Action                     | Expected result                                                            |
|------|----------------------------|----------------------------------------------------------------------------|
| 1    | Ask to delete "Home".      | The question warns that the wiki then has no start page.                   |
| 2    | Confirm.                   | `Home.md` is deleted.                                                      |
| 3    | Ask to delete "Plan".      | No warning about a start page.                                             |

## TC-007-17: Delete key and keyboard alone

**Flow:** Main success scenario step 1, BR-007, NFR-011  
**Preconditions:** `Plan.md` is open in a tab; the note overview lists "Plan" and "Ideas".

| Step | Action                                                    | Expected result                                               |
|------|-----------------------------------------------------------|---------------------------------------------------------------|
| 1    | In the tab, select a word and press Delete.               | The word is deleted; no question appears.                     |
| 2    | Move the focus to "Ideas" in the note overview with the keyboard and press Delete. | The question to delete "Ideas" appears.  |
| 3    | Confirm with Enter.                                        | "Ideas" is deleted.                                           |
