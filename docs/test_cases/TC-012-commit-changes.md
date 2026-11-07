# Test Cases: Commit Changes

**Use Case:** [UC-012 Commit Changes](../use_cases/UC-012-commit-changes.md)  
**Requirements:** [FR-048, FR-051, NFR-006, NFR-011, NFR-014](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements          |
|-----------|----------------------------------------------------|-----------------------|
| TC-012-01 | Main success scenario: commit all changes          | FR-051, BR-001        |
| TC-012-02 | Leave out a change                                 | BR-004                |
| TC-012-03 | Proposed message                                   | BR-002                |
| TC-012-04 | Unsaved text is committed                          | BR-001                |
| TC-012-05 | Renamed and deleted notes                          | BR-001                |
| TC-012-06 | Message kept, long first line                      | BR-003                |
| TC-012-07 | A1 Empty message                                   | —                     |
| TC-012-08 | A2 Nothing chosen                                  | BR-004                |
| TC-012-09 | A3 Git does not know the user                      | BR-005                |
| TC-012-10 | A4 Commit fails                                    | BR-008                |
| TC-012-11 | A5 A note cannot be saved                          | NFR-006               |
| TC-012-12 | A6 Amend the last commit                           | BR-006                |
| TC-012-13 | A6 not offered once pushed                         | BR-006                |
| TC-012-14 | No network                                         | BR-007, NFR-014       |
| TC-012-15 | No commit during an unfinished merge               | BR-009                |
| TC-012-16 | Keyboard alone                                     | NFR-011               |

## TC-012-01: Commit all changes

**Flow:** Main success scenario, BR-001  
**Preconditions:** The workspace `notes` is a git repository on `main`; `Plan.md` and `Ideas.md` changed, `img/shot.png` is new.

| Step | Action                                         | Expected result                                                                 |
|------|------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+G.                             | The message field shows "Update Plan, Ideas and 1 other file"; all three changes are checked. |
| 2    | Type "Plan the trip" and press Ctrl+Enter.      | The status bar reports a commit with its short name holding 2 notes and 1 file. |
| 3    | Look at the list and the status bar.            | Nothing changed since the last commit; the field is empty; the branch shows no count. |
| 4    | Run `git log -1 --stat` in a terminal.          | The commit "Plan the trip" holds the three files.                              |

## TC-012-02: Leave out a change

**Flow:** BR-004  
**Preconditions:** `Plan.md` and `Ideas.md` changed.

| Step | Action                                              | Expected result                                                  |
|------|-----------------------------------------------------|------------------------------------------------------------------|
| 1    | Uncheck "Ideas" and commit with a message.           | The commit holds only `Plan.md`.                                |
| 2    | Look at the list.                                    | "Ideas" is still listed as changed, and checked again.          |
| 3    | Uncheck all, then check all.                          | All changes are unchecked, then checked.                        |

## TC-012-03: Proposed message

**Flow:** BR-002  
**Preconditions:** Five notes changed (A, B, C, D, E), "New" is new, "Old" deleted, "Draft" renamed to "Final", and two images changed.

| Step | Action                         | Expected result                                                                                       |
|------|--------------------------------|-------------------------------------------------------------------------------------------------------|
| 1    | Open the changes.               | The field proposes "Update A, B, C and 2 more notes; Add New; Delete Old; Rename Draft to Final and 2 other files". |
| 2    | Uncheck "New".                  | The proposal drops "Add New".                                                                        |
| 3    | Type a word, then uncheck "Old". | The typed message stays.                                                                             |

## TC-012-04: Unsaved text is committed

**Flow:** BR-001, step 5  
**Preconditions:** `Plan.md` is open; the user has just typed "last word" and autosave has not run.

| Step | Action                     | Expected result                                                           |
|------|----------------------------|---------------------------------------------------------------------------|
| 1    | Commit at once.            | `Plan.md` is saved first; the commit holds "last word".                  |

## TC-012-05: Renamed and deleted notes

**Flow:** BR-001  
**Preconditions:** "Draft" was renamed to "Final" (UC-006) and "Old" deleted (UC-007).

| Step | Action                                  | Expected result                                                         |
|------|-----------------------------------------|-------------------------------------------------------------------------|
| 1    | Commit both.                             | `git show --stat` lists `Draft.md => Final.md` and `Old.md` deleted.    |
| 2    | Rename another note, uncheck it, commit something else. | The renamed note stays listed as renamed, its old place too. |

## TC-012-06: Message kept, long first line

**Flow:** BR-003  
**Preconditions:** `Plan.md` changed.

| Step | Action                                                 | Expected result                                                    |
|------|--------------------------------------------------------|--------------------------------------------------------------------|
| 1    | Type a first line of 80 characters.                     | The field says many programs show only its start.                 |
| 2    | Open another note and come back to the changes.         | The message is still there.                                       |
| 3    | Add a second line and "  " before the message, then commit. | The commit's message has both lines, without the leading spaces. |

## TC-012-07: Empty message

**Flow:** A1  
**Preconditions:** `Plan.md` changed.

| Step | Action                                   | Expected result                                        |
|------|------------------------------------------|--------------------------------------------------------|
| 1    | Empty the field, type spaces, and commit. | No commit is made; the field says a message is needed. |

## TC-012-08: Nothing chosen

**Flow:** A2, BR-004  
**Preconditions:** `Plan.md` changed.

| Step | Action               | Expected result                                                    |
|------|----------------------|--------------------------------------------------------------------|
| 1    | Uncheck "Plan".       | The commit button cannot be used; the field says nothing is chosen. |

## TC-012-09: Git does not know the user

**Flow:** A3, BR-005  
**Preconditions:** git has no `user.name` and `user.email`; `Plan.md` changed.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Commit with a message.                          | Underline asks for name and email, saying they hold for every repository and show in the history. |
| 2    | Cancel.                                          | Nothing is committed; the message is kept.                             |
| 3    | Commit again, enter "Ada" and "ada@example.org". | The commit is made by Ada.                                             |
| 4    | Run `git config --global user.name`.             | It prints "Ada".                                                       |

## TC-012-10: Commit fails

**Flow:** A4, BR-008  
**Preconditions:** The repository has a pre-commit check that rejects every commit with "no commits today".

| Step | Action                     | Expected result                                                                     |
|------|----------------------------|-------------------------------------------------------------------------------------|
| 1    | Commit with a message.      | The status bar says the commit failed: "no commits today"; message and choice stay. |
| 2    | Run `git log`.              | No new commit.                                                                      |

## TC-012-11: A note cannot be saved

**Flow:** A5, NFR-006  
**Preconditions:** `locked/Plan.md` is open with unsaved changes, in a folder that cannot be written.

| Step | Action               | Expected result                                                           |
|------|----------------------|---------------------------------------------------------------------------|
| 1    | Commit.               | No commit is made; the status bar names "Plan" as not saved and why.      |

## TC-012-12: Amend the last commit

**Flow:** A6, BR-006  
**Preconditions:** The last commit "Plan the trip" holds `Plan.md` and is not pushed; `Ideas.md` changed since.

| Step | Action                                           | Expected result                                                          |
|------|--------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Choose to amend the last commit.                  | The field holds "Plan the trip".                                         |
| 2    | Change it to "Plan the trip, with ideas" and commit. | The status bar reports the last commit amended.                       |
| 3    | Run `git log -2 --stat`.                          | The last commit holds both notes with the new message, the same author and time; the commit before is unchanged. |

## TC-012-13: Amending not offered once pushed

**Flow:** A6, BR-006  
**Preconditions:** The last commit is on the remote branch.

| Step | Action               | Expected result                               |
|------|----------------------|-----------------------------------------------|
| 1    | Open the changes.    | Amending the last commit is not offered.     |

## TC-012-14: No network

**Flow:** BR-007, NFR-014  
**Preconditions:** No network connection; the workspace has a remote.

| Step | Action                                   | Expected result                                           |
|------|------------------------------------------|-----------------------------------------------------------|
| 1    | Commit a change, watching the network.    | The commit succeeds; Underline makes no connection.      |

## TC-012-15: No commit during an unfinished merge

**Flow:** BR-009  
**Preconditions:** A synchronization left `Plan.md` in conflict (UC-015).

| Step | Action               | Expected result                                                          |
|------|----------------------|--------------------------------------------------------------------------|
| 1    | Open the changes.    | No commit is offered; the panel points to resolving the conflict.        |

## TC-012-16: Keyboard alone

**Flow:** NFR-011  
**Preconditions:** `Plan.md` changed.

| Step | Action                                         | Expected result                                  |
|------|------------------------------------------------|--------------------------------------------------|
| 1    | Press Ctrl+Shift+G.                             | The focus is in the message field.               |
| 2    | Type a message and press Ctrl+Enter.            | The commit is made.                              |
