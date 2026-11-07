# Test Cases: Resolve Sync Conflict

**Use Case:** [UC-015 Resolve Sync Conflict](../use_cases/UC-015-resolve-sync-conflict.md)  
**Requirements:** [FR-045, FR-046, NFR-006, NFR-011](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements           |
|-----------|----------------------------------------------------|------------------------|
| TC-015-01 | Main success scenario: resolve place by place      | FR-046, BR-004         |
| TC-015-02 | Here, Remote, Both, and editing                    | BR-002, BR-003         |
| TC-015-03 | Marks and count                                    | BR-005                 |
| TC-015-04 | A1 One side for the whole note                     | —                      |
| TC-015-05 | A2 File that is not a note                         | —                      |
| TC-015-06 | A3 Deleted on one side                             | —                      |
| TC-015-07 | A4 Later, and after a restart                      | BR-001, NFR-006        |
| TC-015-08 | A5 Abort                                           | BR-008                 |
| TC-015-09 | A6 Commit by commit                                | BR-004                 |
| TC-015-10 | A7 Completing fails                                | —                      |
| TC-015-11 | Other use cases wait                               | BR-007                 |
| TC-015-12 | Keyboard alone                                     | BR-006, NFR-011        |

## TC-015-01: Resolve place by place

**Flow:** Main success scenario, BR-004  
**Preconditions:** The workspace on `main` follows `origin/main`. `Plan.md` has 20 lines; here line 3 and line 15 were changed and committed; on the remote the same two lines were changed differently, and line 10 too. A sync stopped with "Plan" in conflict (UC-014 A3).

| Step | Action                                        | Expected result                                                                     |
|------|-----------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Open "Plan".                                   | Two blocks "Here" / "Remote" at lines 3 and 15; line 10 holds the remote's change already; "2 places open". |
| 2    | Choose "Here" at the first block.              | It holds this computer's line 3; "1 place open".                                   |
| 3    | Choose "Remote" at the second block.           | It holds the remote's line 15; the note is marked as changed, no longer in conflict. |
| 4    | Look at the status bar and the remote.         | It reports the conflicts resolved; `git log origin/main` shows "Merge remote changes into main". |

## TC-015-02: Here, Remote, Both, and editing

**Flow:** BR-002, BR-003  
**Preconditions:** "Plan" in conflict at three places; the tab shows the facade.

| Step | Action                                          | Expected result                                                         |
|------|-------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Look at a block.                                 | Both versions are formatted, each with who changed it and when.        |
| 2    | Choose "Both" at the first place.                | The place holds here's version, then the remote's.                     |
| 3    | Switch to the source view.                       | The two open places show both versions as markup.                      |
| 4    | Delete the remote's version of one place by hand and keep editing until no block is left. | The note is marked as resolved. |

## TC-015-03: Marks and count

**Flow:** BR-005  
**Preconditions:** A sync stopped with "Plan" and "Ideas" in conflict.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Look at the note overview.      | Both carry a C in the warning color.                                    |
| 2    | Open the changes (UC-011).      | Both are listed first, marked C.                                        |
| 3    | Look at the status bar.          | It says 2 conflicts wait.                                              |

## TC-015-04: One side for the whole note

**Flow:** A1  
**Preconditions:** "Plan" in conflict at three places.

| Step | Action                    | Expected result                                                       |
|------|---------------------------|-----------------------------------------------------------------------|
| 1    | Choose "All: Remote".     | All three places hold the remote's version; the note is resolved.    |

## TC-015-05: File that is not a note

**Flow:** A2  
**Preconditions:** `img/shot.png` was changed here and on the remote.

| Step | Action                         | Expected result                                                           |
|------|--------------------------------|---------------------------------------------------------------------------|
| 1    | Choose `img/shot.png` in the changes. | Both pictures are shown, with "Here" and "Remote".               |
| 2    | Choose "Here".                  | `img/shot.png` is this computer's picture; `img/shot (remote).png` holds the remote's. |

## TC-015-06: Deleted on one side

**Flow:** A3  
**Preconditions:** "Old" was deleted here and changed on the remote; "Trip" changed here and deleted on the remote.

| Step | Action                         | Expected result                                                           |
|------|--------------------------------|---------------------------------------------------------------------------|
| 1    | Open "Old".                     | Its remote version shows, saying it was deleted here.                    |
| 2    | Choose "Keep".                  | "Old" stays with the remote's version.                                   |
| 3    | Open "Trip" and choose "Delete". | "Trip" is in the deleted notes (UC-007) and gone from the workspace.    |

## TC-015-07: Later, and after a restart

**Flow:** A4, BR-001, NFR-006  
**Preconditions:** "Plan" in conflict at two places; one is resolved.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Type in "Ideas" and in "Plan" outside the block. | Both save as usual.                                                   |
| 2    | Try to commit, to switch branch, and to sync.   | Each says the conflicts must be resolved first.                         |
| 3    | Restart Underline.                              | "Plan" is still in conflict with one open place; the resolved one stays resolved. |

## TC-015-08: Abort

**Flow:** A5, BR-008  
**Preconditions:** A sync stopped with "Plan" in conflict; "Ideas" has a change left out of commits; there is one local commit.

| Step | Action                                 | Expected result                                                              |
|------|----------------------------------------|------------------------------------------------------------------------------|
| 1    | Choose "Abort the sync" and cancel.     | Nothing changes.                                                            |
| 2    | Choose it again and confirm.            | "Plan" is as before the sync; the local commit and the change to "Ideas" stay; no conflict is marked. |
| 3    | Sync again.                             | The same conflict comes up.                                                 |

## TC-015-09: Commit by commit

**Flow:** A6, BR-004  
**Preconditions:** `git config pull.rebase true`; two local commits each conflict with the remote.

| Step | Action                                | Expected result                                                            |
|------|---------------------------------------|----------------------------------------------------------------------------|
| 1    | Resolve the conflicts shown.           | The conflicts of the second commit come up, naming it.                    |
| 2    | Resolve them.                          | The local commits are on top of the remote's; the sync pushes.            |

## TC-015-10: Completing fails

**Flow:** A7  
**Preconditions:** A check of the repository rejects every commit; "Plan" in conflict.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Resolve the last place.         | The status bar says completing failed and why; "Plan" stays resolved.  |
| 2    | Abort the sync.                 | The workspace is as before the sync.                                   |

## TC-015-11: Other use cases wait

**Flow:** BR-007  
**Preconditions:** "Plan" in conflict.

| Step | Action                         | Expected result                                                       |
|------|--------------------------------|-----------------------------------------------------------------------|
| 1    | Export "Plan" to HTML.          | The export says the note must be resolved first.                     |
| 2    | Rename "Plan", then delete it.  | Both wait until the conflicts are resolved, saying so.              |

## TC-015-12: Keyboard alone

**Flow:** BR-006, NFR-011  
**Preconditions:** "Plan" in conflict at three places; the cursor is at the top.

| Step | Action                         | Expected result                                       |
|------|--------------------------------|-------------------------------------------------------|
| 1    | Press F7.                       | The cursor is at the first open place.                |
| 2    | Press Alt+H, F7, Alt+R, F7, Alt+B. | The three places hold here, remote, and both.     |
| 3    | Press Shift+F7.                 | Nothing is open; the cursor stays.                   |
