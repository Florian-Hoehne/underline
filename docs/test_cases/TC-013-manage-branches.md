# Test Cases: Manage Branches

**Use Case:** [UC-013 Manage Branches](../use_cases/UC-013-manage-branches.md)  
**Requirements:** [FR-018, FR-047, FR-050, NFR-006, NFR-011, NFR-014](../requirements.md)

## Coverage

| Test case | Flow / rule                                      | Requirements          |
|-----------|--------------------------------------------------|-----------------------|
| TC-013-01 | Main success scenario: switch branch             | FR-050, BR-004        |
| TC-013-02 | The list and its filter                          | BR-001, BR-002        |
| TC-013-03 | Changes carried along                            | BR-003                |
| TC-013-04 | Open tabs after a switch                         | BR-004, FR-018        |
| TC-013-05 | A1 Create a branch                               | BR-008                |
| TC-013-06 | A2 Name not allowed or taken                     | BR-005                |
| TC-013-07 | A3 Changes would be overwritten                  | BR-003                |
| TC-013-08 | A4 Remote branch chosen                          | BR-001                |
| TC-013-09 | A5 Switch fails                                  | —                     |
| TC-013-10 | A6 A note cannot be saved                        | NFR-006               |
| TC-013-11 | A7 Unfinished merge                              | —                     |
| TC-013-12 | A8 User cancels                                  | —                     |
| TC-013-13 | No network                                       | BR-001, NFR-014       |
| TC-013-14 | Keyboard alone                                   | BR-006, NFR-011       |

## TC-013-01: Switch branch

**Flow:** Main success scenario, BR-004  
**Preconditions:** The workspace is on `main` with nothing to commit; the branch `drafts` holds `Plan.md` with the line "draft plan" and the note `Sketch.md`, which `main` does not hold.

| Step | Action                                         | Expected result                                                                |
|------|------------------------------------------------|--------------------------------------------------------------------------------|
| 1    | Click the branch in the status bar.             | A menu lists `main` (marked as current) and `drafts`.                         |
| 2    | Choose `drafts`.                                 | The status bar shows `drafts` and reports the switch with no changes carried. |
| 3    | Look at the note overview and the tab of "Plan". | "Sketch" is listed; "Plan" shows "draft plan".                                |
| 4    | Switch back to `main`.                           | "Sketch" is no longer listed; "Plan" shows its text on `main`.               |

## TC-013-02: The list and its filter

**Flow:** BR-001, BR-002  
**Preconditions:** Local branches `main`, `drafts`, `Trip-2027`; the remote has `main`, `drafts`, and `review` (known from the last synchronization).

| Step | Action                         | Expected result                                                                  |
|------|--------------------------------|----------------------------------------------------------------------------------|
| 1    | Open the branch menu.           | `drafts`, `main` (current), `Trip-2027`, then `review` marked as on the remote; then "New branch…" and "Show changes". |
| 2    | Type "TRIP" in the filter.      | Only `Trip-2027` is listed.                                                     |
| 3    | Choose "Show changes".          | The changes open in the tool panel (UC-011).                                    |

## TC-013-03: Changes carried along

**Flow:** BR-003  
**Preconditions:** On `main`, `Ideas.md` changed and not committed; `drafts` holds the same `Ideas.md` as `main`.

| Step | Action                     | Expected result                                                      |
|------|----------------------------|----------------------------------------------------------------------|
| 1    | Switch to `drafts`.         | The report says 1 change was carried along; "Ideas" is marked changed and holds the change. |

## TC-013-04: Open tabs after a switch

**Flow:** BR-004, FR-018  
**Preconditions:** On `drafts`, tabs of "Plan" and "Sketch" are open; "Plan" has a word typed and not yet saved; `main` holds the same committed "Plan" as `drafts` and does not hold "Sketch".

| Step | Action               | Expected result                                                                         |
|------|----------------------|-----------------------------------------------------------------------------------------|
| 1    | Switch to `main`.    | "Plan" was saved first and its tab still shows the typed word, carried along; the tab of "Sketch" closes and the report names it. |

## TC-013-05: Create a branch

**Flow:** A1, BR-008  
**Preconditions:** On `main`, `Plan.md` changed and not committed; the workspace has a remote.

| Step | Action                                         | Expected result                                                                   |
|------|------------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Open the menu, type "trip ideas", choose "New branch…". | The name field holds `trip-ideas`; the dialog says it starts from the current commit with the current changes and goes to the remote with the next synchronization. |
| 2    | Confirm.                                         | The status bar shows `trip-ideas`; "Plan" is still marked changed.               |
| 3    | Run `git log -1` on both branches.               | Both point to the same commit.                                                    |

## TC-013-06: Name not allowed or taken

**Flow:** A2, BR-005  
**Preconditions:** A local branch `drafts` exists.

| Step | Action                                  | Expected result                                                         |
|------|-----------------------------------------|-------------------------------------------------------------------------|
| 1    | Create a branch named `a..b`.            | Underline says why the name is not allowed and keeps it.               |
| 2    | Try `-x`, `x.lock`, `a:b`, `@`, and an empty name. | Each time the reason is shown.                              |
| 3    | Try `drafts`.                            | Underline says it exists and offers to switch to it.                   |

## TC-013-07: Changes would be overwritten

**Flow:** A3, BR-003  
**Preconditions:** On `main`, `Plan.md` changed and not committed; `drafts` holds a different `Plan.md`.

| Step | Action                     | Expected result                                                                  |
|------|----------------------------|----------------------------------------------------------------------------------|
| 1    | Switch to `drafts`.         | Underline does not switch, names "Plan", and offers to commit first or stay.    |
| 2    | Choose to stay.             | The workspace is on `main`; "Plan" keeps its change.                            |

## TC-013-08: Remote branch chosen

**Flow:** A4, BR-001  
**Preconditions:** The remote has `review`, there is no local `review`.

| Step | Action                     | Expected result                                                              |
|------|----------------------------|------------------------------------------------------------------------------|
| 1    | Choose `review`.            | A local `review` following the remote's is created and the workspace is on it. |
| 2    | Run `git status -sb`.       | It shows `review...origin/review`.                                           |

## TC-013-09: Switch fails

**Flow:** A5  
**Preconditions:** Another git program holds the repository's lock (`.git/index.lock` exists).

| Step | Action               | Expected result                                                       |
|------|----------------------|-----------------------------------------------------------------------|
| 1    | Switch to `drafts`.  | The status bar says the switch failed, in git's words; still on `main`. |

## TC-013-10: A note cannot be saved

**Flow:** A6, NFR-006  
**Preconditions:** `locked/Plan.md` is open with unsaved changes in a folder that cannot be written.

| Step | Action               | Expected result                                                    |
|------|----------------------|--------------------------------------------------------------------|
| 1    | Switch to `drafts`.  | Underline does not switch and names "Plan" as not saved and why.  |

## TC-013-11: Unfinished merge

**Flow:** A7  
**Preconditions:** A synchronization left `Plan.md` in conflict (UC-015).

| Step | Action               | Expected result                                                         |
|------|----------------------|-------------------------------------------------------------------------|
| 1    | Open the branch menu. | The branches are listed; switching and creating are not offered; it says to resolve the conflicts first. |

## TC-013-12: User cancels

**Flow:** A8  
**Preconditions:** On `main`.

| Step | Action                                   | Expected result          |
|------|------------------------------------------|--------------------------|
| 1    | Open the menu and press Escape.           | Nothing changes.        |
| 2    | Choose "New branch…", type a name, cancel. | No branch is created.  |

## TC-013-13: No network

**Flow:** BR-001, NFR-014  
**Preconditions:** No network connection; the workspace has a remote.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Open the menu, create a branch, switch back.    | All works; Underline makes no connection.            |

## TC-013-14: Keyboard alone

**Flow:** BR-006, NFR-011  
**Preconditions:** Branches `main` (current) and `drafts`.

| Step | Action                                   | Expected result                                       |
|------|------------------------------------------|-------------------------------------------------------|
| 1    | Press Ctrl+Shift+B.                       | The menu opens with the focus in the filter field.   |
| 2    | Press the down arrow to `drafts` and Enter. | The workspace switches to `drafts`.                |
| 3    | Press Ctrl+Shift+B and Escape.            | The menu closes.                                     |
