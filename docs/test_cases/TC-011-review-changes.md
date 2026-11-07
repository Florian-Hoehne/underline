# Test Cases: Review Changes

**Use Case:** [UC-011 Review Changes](../use_cases/UC-011-review-changes.md)  
**Requirements:** [FR-017, FR-047, FR-048, FR-049, NFR-002, NFR-011, NFR-013](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements            |
|-----------|------------------------------------------------------|-------------------------|
| TC-011-01 | Main success scenario: branch, count, marks, diff    | FR-047, FR-048, FR-049  |
| TC-011-02 | New, changed, deleted, and renamed notes             | FR-048, BR-003          |
| TC-011-03 | Unsaved text counts                                  | BR-001                  |
| TC-011-04 | Diff with context, joined parts, and words           | FR-049, BR-004          |
| TC-011-05 | Spaces and line endings                              | BR-004                  |
| TC-011-06 | What is not shown                                    | BR-002                  |
| TC-011-07 | A1 No changes                                        | —                       |
| TC-011-08 | A2 Active note unchanged                             | —                       |
| TC-011-09 | A3 Changes while looking                             | —                       |
| TC-011-10 | A4 Deleted note chosen                               | —                       |
| TC-011-11 | A5 Other file chosen                                 | BR-002                  |
| TC-011-12 | A6 Discard a changed note                            | BR-005                  |
| TC-011-13 | A6 Discard a new, a deleted, and a renamed note      | BR-005                  |
| TC-011-14 | A7 No commit yet                                     | —                       |
| TC-011-15 | A8 No branch                                         | —                       |
| TC-011-16 | A9 Git cannot be asked                               | —                       |
| TC-011-17 | Speed in a large workspace                           | BR-006, NFR-002         |
| TC-011-18 | Keyboard alone                                       | BR-007, NFR-011         |

## TC-011-01: Branch, count, marks, and diff

**Flow:** Main success scenario  
**Preconditions:** The workspace `notes` is a git repository on the branch `main` with everything committed. `Plan.md` has 20 lines.

| Step | Action                                                     | Expected result                                                                     |
|------|------------------------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Open the workspace.                                         | The status bar shows `main` and no count; no note is marked.                       |
| 2    | Open `Plan`, change line 10 and wait 2 seconds.             | The status bar shows `main` with 1 change; "Plan" is marked as changed.            |
| 3    | Click the branch in the status bar.                         | The tool panel lists "Plan" as changed.                                            |
| 4    | Look at the diff below the list.                            | The old line 10 is shown as removed and the new one as added, with lines 7–9 and 11–13 around them and their line numbers. |
| 5    | Click the added line of the diff.                           | The note shows line 10 with the cursor on it.                                      |

## TC-011-02: New, changed, deleted, and renamed notes

**Flow:** BR-003  
**Preconditions:** As TC-011-01; `Old.md` and `Draft.md` are committed.

| Step | Action                                                     | Expected result                                                          |
|------|------------------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Create the note "New" (UC-003).                             | "New" is marked as new in the overview and the list.                    |
| 2    | Delete "Old" (UC-007).                                      | The list shows "Old" as deleted; the overview does not list it.         |
| 3    | Rename "Draft" to "Final" (UC-006).                         | "Final" is marked as renamed from "Draft".                              |
| 4    | Look at the marks without color, for example in a screenshot in grey. | New, changed, deleted, and renamed can be told apart by their letters. |

## TC-011-03: Unsaved text counts

**Flow:** BR-001  
**Preconditions:** As TC-011-01; autosave is held back by a note in a folder that cannot be written.

| Step | Action                                         | Expected result                                                  |
|------|------------------------------------------------|------------------------------------------------------------------|
| 1    | Type a word in a note that cannot be saved.    | Within a second, the note counts as changed and its diff shows the word. |

## TC-011-04: Diff with context, joined parts, and words

**Flow:** BR-004  
**Preconditions:** `Long.md` has 60 committed lines.

| Step | Action                                                       | Expected result                                                         |
|------|--------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Change one word in line 5, line 9, and line 40.               | The diff shows two parts: lines 2–12 together, and lines 37–43.        |
| 2    | Look at line 5 in the diff.                                   | The old and the new word are marked more strongly than the rest of the line. |

## TC-011-05: Spaces and line endings

**Flow:** BR-004  
**Preconditions:** `Plan.md` is committed with Unix line endings.

| Step | Action                                                      | Expected result                                                         |
|------|-------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Add two spaces at the end of line 3.                         | Line 3 shows as changed, with a note that only spaces differ.          |
| 2    | With another program, give line 7 a Windows line ending.     | Line 7 shows as changed, with a note that only the line ending differs. |

## TC-011-06: What is not shown

**Flow:** BR-002  
**Preconditions:** The repository ignores `build/`; a note was deleted (UC-007); tabs were opened, so the Underline folder changed.

| Step | Action                                  | Expected result                                                                   |
|------|-----------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Write `build/out.md`.                    | It is not listed.                                                                |
| 2    | Look at the list.                         | Nothing of the Underline folder or of the deleted notes is listed.               |

## TC-011-07: No changes

**Flow:** A1  
**Preconditions:** Everything is committed.

| Step | Action                      | Expected result                                                        |
|------|-----------------------------|------------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+G.          | The list says nothing changed since the last commit.                  |

## TC-011-08: Active note unchanged

**Flow:** A2  
**Preconditions:** `Plan.md` changed, `Ideas.md` did not.

| Step | Action                        | Expected result                                                    |
|------|-------------------------------|--------------------------------------------------------------------|
| 1    | Activate the tab of "Ideas".  | The tool panel says the note did not change since the last commit. |

## TC-011-09: Changes while looking

**Flow:** A3  
**Preconditions:** The changes are shown; only "Plan" changed.

| Step | Action                                                      | Expected result                                       |
|------|-------------------------------------------------------------|-------------------------------------------------------|
| 1    | Change `Ideas.md` with another program.                      | Within a second, "Ideas" is listed and marked.        |
| 2    | Commit everything with git in a terminal.                    | Within a second, the list says nothing changed.       |
| 3    | Type in "Plan" for 10 seconds without a pause.               | Typing is never held up; the diff follows within a second after the typing stops. |

## TC-011-10: Deleted note chosen

**Flow:** A4  
**Preconditions:** "Old" was deleted.

| Step | Action                        | Expected result                                                  |
|------|-------------------------------|------------------------------------------------------------------|
| 1    | Choose "Old" in the list.     | Its diff shows every line removed; no tab opens.                 |

## TC-011-11: Other file chosen

**Flow:** A5, BR-002  
**Preconditions:** `img/shot.png` was added and `underline.yaml` changed.

| Step | Action                              | Expected result                                                    |
|------|-------------------------------------|--------------------------------------------------------------------|
| 1    | Look at the list.                    | Both are listed below the notes as other files.                   |
| 2    | Choose `underline.yaml`.             | Its diff is shown.                                                 |
| 3    | Choose `img/shot.png`.               | It says the file is new, without a diff.                           |

## TC-011-12: Discard a changed note

**Flow:** A6, BR-005  
**Preconditions:** `Plan.md` is open with 3 lines changed since the last commit, 1 of them not yet saved.

| Step | Action                                               | Expected result                                                         |
|------|------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Ask to discard the changes of "Plan".                 | The question names "Plan" and 3 changed lines, and says they cannot be restored. |
| 2    | Cancel.                                                | Nothing changes.                                                       |
| 3    | Ask again and confirm.                                 | The tab and the file hold the committed content; "Plan" is no longer marked. |

## TC-011-13: Discard a new, a deleted, and a renamed note

**Flow:** A6, BR-005  
**Preconditions:** "New" was created, "Old" deleted, and "Draft" renamed to "Final" since the last commit.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Discard "New" and confirm.      | "New" is gone and listed among the deleted notes (UC-007).              |
| 2    | Discard "Old" and confirm.      | "Old" is back with its committed content.                               |
| 3    | Discard "Final" and confirm.    | The note is "Draft" again with its committed content.                   |
| 4    | Look at the list.               | Nothing changed since the last commit.                                  |

## TC-011-14: No commit yet

**Flow:** A7  
**Preconditions:** A workspace created with a new git repository and no commit, holding `Home.md`.

| Step | Action               | Expected result                                                         |
|------|----------------------|-------------------------------------------------------------------------|
| 1    | Open the workspace.  | The status bar shows the branch of the first commit; "Home" is new.     |

## TC-011-15: No branch

**Flow:** A8  
**Preconditions:** The repository is checked out on a commit, not a branch.

| Step | Action               | Expected result                                                                  |
|------|----------------------|----------------------------------------------------------------------------------|
| 1    | Open the workspace.  | The status bar shows the short name of the commit, marked as not on a branch.    |

## TC-011-16: Git cannot be asked

**Flow:** A9  
**Preconditions:** The `.git` folder of the workspace is damaged.

| Step | Action                         | Expected result                                                              |
|------|--------------------------------|------------------------------------------------------------------------------|
| 1    | Open the workspace.             | The status bar says the changes cannot be shown and why; no marks show.     |
| 2    | Open and edit a note.            | Everything works as in a workspace without git.                            |

## TC-011-17: Speed in a large workspace

**Flow:** BR-006, NFR-002  
**Preconditions:** A committed workspace of 10,000 notes, 50 of them changed.

| Step | Action                                     | Expected result                                                  |
|------|--------------------------------------------|------------------------------------------------------------------|
| 1    | Open the workspace and time the count.      | The count, the marks, and the list appear within 1 second.      |
| 2    | Type in a note.                             | Every character appears at once.                                 |

## TC-011-18: Keyboard alone

**Flow:** BR-007, NFR-011  
**Preconditions:** "Plan" and "Ideas" changed.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Press Ctrl+Shift+G.                             | The changes show in the tool panel, the first chosen. |
| 2    | Press the down arrow and Enter.                 | "Ideas" opens with its diff.                          |
| 3    | Move into the diff with the keyboard and press Enter on a line. | The note shows that line.             |
