# Test Cases: Rename Note

**Use Case:** [UC-006 Rename Note](../use_cases/UC-006-rename-note.md)  
**Requirements:** [FR-004, FR-005, FR-017, FR-018, NFR-006, NFR-007, NFR-011, NFR-013](../requirements.md)

## Coverage

| Test case | Flow / rule                                              | Requirements             |
|-----------|----------------------------------------------------------|--------------------------|
| TC-006-01 | Main success scenario: new title, links updated          | FR-005, BR-003, BR-007   |
| TC-006-02 | Move to another folder, links in and out of the note     | FR-005, BR-001, BR-005   |
| TC-006-03 | Link text and the rest of a linking note                 | BR-003, NFR-007          |
| TC-006-04 | First heading                                            | BR-004                   |
| TC-006-05 | AsciiDoc note and AsciiDoc links, format stays           | FR-004, BR-002, BR-003   |
| TC-006-06 | Only upper and lower case changed                        | BR-001                   |
| TC-006-07 | A1 Title not allowed                                     | BR-001, NFR-013          |
| TC-006-08 | A2 Note exists                                           | BR-001                   |
| TC-006-09 | A3 Folder not allowed                                    | BR-001                   |
| TC-006-10 | A4 Nothing changed                                       | —                        |
| TC-006-11 | A5 Note cannot be renamed                                | BR-008, NFR-006          |
| TC-006-12 | A6 A linking note cannot be updated                      | BR-008                   |
| TC-006-13 | A7 Linking note open with unsaved changes                | —                        |
| TC-006-14 | A8 User cancels                                          | —                        |
| TC-006-15 | Open tab, remembered tabs, and recent notes              | FR-017, FR-018           |
| TC-006-16 | Home note                                                | BR-009                   |
| TC-006-17 | Renaming back, and the rename in git                     | BR-007, BR-010           |
| TC-006-18 | Todo links follow                                        | BR-006                   |
| TC-006-19 | Keyboard alone                                           | NFR-011                  |

## TC-006-01: Rename a note, links updated

**Flow:** Main success scenario, BR-003, BR-007  
**Preconditions:** The workspace `notes` holds `Plan.md` with the heading `# Plan` and a section `## Goals`, and these notes linking to it: `Ideas.md` with `[the plan](Plan.md)`, `Week.md` with `[goals](Plan#goals)`, and `team/Review.md` with `[plan](../Plan.md)`. `Plan.md` is open in the active tab.

| Step | Action                                             | Expected result                                                                                              |
|------|----------------------------------------------------|--------------------------------------------------------------------------------------------------------------|
| 1    | Press F2.                                          | A dialog shows the title "Plan" selected, the top folder, and the file name `Plan.md`.                       |
| 2    | Type "Road Map".                                   | The dialog shows the file name `Road-Map.md`.                                                               |
| 3    | Confirm.                                           | The note overview and the tab show "Road Map"; the note stays open where it was.                             |
| 4    | Look at the status bar.                            | It reports that the note was renamed to "Road Map" and that 3 links in 3 notes were updated.                 |
| 5    | Look at the workspace folder with another program. | `Road-Map.md` exists, `Plan.md` does not.                                                                   |
| 6    | Open `Ideas.md`, `Week.md`, and `team/Review.md`.  | They hold `[the plan](Road-Map.md)`, `[goals](Road-Map#goals)`, and `[plan](../Road-Map.md)`.               |
| 7    | Follow each of the three links.                    | Each opens "Road Map", the second at the section Goals.                                                     |

## TC-006-02: Move a note to another folder

**Flow:** Main success scenario, BR-001, BR-005  
**Preconditions:** `Plan.md` in the top folder links to `[ideas](Ideas.md)`, `[picture](img/chart.png)`, `[section](#goals)`, `[self](Plan.md#goals)`, and `[site](https://example.org)`. `Ideas.md` links to `[plan](Plan.md)`. The folder `projects/2027` does not exist.

| Step | Action                                                    | Expected result                                                                       |
|------|-----------------------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Rename `Plan` from the context menu of the note overview, keep the title, and choose the new folder `projects/2027`. | The dialog shows `projects/2027/Plan.md`.  |
| 2    | Confirm.                                                  | `projects/2027/Plan.md` exists and the folders were created; `Plan.md` is gone.       |
| 3    | Open the moved note in the source view.                   | It links to `../../Ideas.md`, `../../img/chart.png`, `#goals`, `Plan.md#goals`, and `https://example.org`. |
| 4    | Look at `img/chart.png`.                                  | It is still in `img`, unchanged.                                                      |
| 5    | Open `Ideas.md`.                                          | It links to `projects/2027/Plan.md`.                                                  |
| 6    | Follow every link of the moved note.                      | Each leads where it led before the move; the picture is shown.                        |

## TC-006-03: Link text and the rest of a linking note

**Flow:** BR-003, NFR-007  
**Preconditions:** `Getting-Started.md` exists. `Index.md` holds `[Getting Started](Getting-Started.md)`, `[start here](Getting-Started.md)`, the plain text "Getting Started", a code block holding `Getting-Started.md`, and a link `[Other](Other.md)`. A copy of `Index.md` is kept.

| Step | Action                                         | Expected result                                                                                                  |
|------|------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| 1    | Rename "Getting Started" to "First Steps".     | The status bar reports 2 links in 1 note.                                                                        |
| 2    | Compare `Index.md` with the copy.              | Only the two links differ: `[First Steps](First-Steps.md)` and `[start here](First-Steps.md)`. The plain text, the code block, the other link, line ends, and spacing are unchanged. |

## TC-006-04: First heading

**Flow:** BR-004  
**Preconditions:** `Plan.md` begins with `# Plan`; `Notes.md` begins with `# My notes on things`.

| Step | Action                              | Expected result                                                   |
|------|-------------------------------------|-------------------------------------------------------------------|
| 1    | Rename "Plan" to "Road Map".         | The note begins with `# Road Map`; the rest is unchanged.         |
| 2    | Rename "Notes" to "Journal".         | The note still begins with `# My notes on things`; nothing else changed. |

## TC-006-05: AsciiDoc note, format stays

**Flow:** BR-002, BR-003, FR-004  
**Preconditions:** `Setup.adoc` begins with `= Setup`. `Guide.adoc` links to it with `xref:Setup.adoc[Setup]` and `<<Setup.adoc#install,install>>`; `Intro.md` links to it with `[setup](Setup.adoc)`.

| Step | Action                                    | Expected result                                                                                    |
|------|-------------------------------------------|----------------------------------------------------------------------------------------------------|
| 1    | Open the rename dialog for `Setup`.       | The dialog offers no choice of note format; the file name ends in `.adoc`.                         |
| 2    | Rename it to "Installation".              | `Installation.adoc` exists and begins with `= Installation`.                                       |
| 3    | Open `Guide.adoc` and `Intro.md`.         | They hold `xref:Installation.adoc[Installation]`, `<<Installation.adoc#install,install>>`, and `[setup](Installation.adoc)`. |

## TC-006-06: Only upper and lower case changed

**Flow:** BR-001  
**Preconditions:** `road-map.md` exists and `Ideas.md` links to `[road map](road-map.md)`.

| Step | Action                          | Expected result                                                                   |
|------|---------------------------------|-----------------------------------------------------------------------------------|
| 1    | Rename "road map" to "Road Map". | No "note exists" message; the file is named `Road-Map.md`, also on Windows and macOS. |
| 2    | Open `Ideas.md`.                 | It holds `[Road Map](Road-Map.md)`.                                               |

## TC-006-07: Title not allowed

**Flow:** A1, BR-001, NFR-013  
**Preconditions:** `Plan.md` exists.

| Step | Action                                              | Expected result                                                              |
|------|-----------------------------------------------------|------------------------------------------------------------------------------|
| 1    | Open the rename dialog and enter `Plan: Q1`.         | The dialog says a title cannot hold `:` and keeps the entry; nothing is renamed. |
| 2    | Enter `CON`, then an empty title, then `.hidden`.    | Each time the dialog says why the title is not allowed.                      |
| 3    | Enter `Plan Q1` and confirm.                         | The note is renamed to `Plan-Q1.md`.                                         |

## TC-006-08: Note exists

**Flow:** A2, BR-001  
**Preconditions:** The top folder holds `Plan.md`, `Ideas.md`, and `Notes.adoc`.

| Step | Action                                              | Expected result                                                                 |
|------|-----------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Rename `Plan` to "ideas".                            | The dialog says a note with that title exists in the folder; nothing changes.   |
| 2    | Rename it to "Notes".                                | Same message, although the existing note is AsciiDoc.                           |
| 3    | Change the title to "Plans" and confirm.             | The note is renamed to `Plans.md`; `Ideas.md` and `Notes.adoc` are unchanged.   |

## TC-006-09: Folder not allowed

**Flow:** A3, BR-001  
**Preconditions:** The workspace is a git repository that ignores the folder `build`; it holds a hidden folder `.drafts` and a folder `other` that is a workspace of its own.

| Step | Action                                                      | Expected result                                                   |
|------|-------------------------------------------------------------|-------------------------------------------------------------------|
| 1    | Rename `Plan` into the folder `build`.                       | The dialog says the folder is ignored by git; nothing changes.    |
| 2    | Choose `.drafts`, then `other`, then `../outside`.           | Each time the dialog says why the folder cannot take the note.    |

## TC-006-10: Nothing changed

**Flow:** A4  
**Preconditions:** `Plan.md` exists; its time of last change is noted.

| Step | Action                                      | Expected result                                                  |
|------|---------------------------------------------|------------------------------------------------------------------|
| 1    | Open the rename dialog and confirm at once. | The dialog closes; the status bar reports nothing.               |
| 2    | Look at `Plan.md` and the notes linking to it. | Their contents and times of last change are unchanged.        |

## TC-006-11: Note cannot be renamed

**Flow:** A5, BR-008, NFR-006  
**Preconditions:** `Plan.md` exists, `Ideas.md` links to it, and the folder `locked` exists without permission to write.

| Step | Action                                                    | Expected result                                                                  |
|------|-----------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Rename `Plan` into the new folder `locked/sub`.            | The status bar says the note could not be renamed and why.                       |
| 2    | Look at the workspace.                                     | `Plan.md` is unchanged, `locked/sub` does not exist, `Ideas.md` still links to `Plan.md`. |

## TC-006-12: A linking note cannot be updated

**Flow:** A6, BR-008  
**Preconditions:** `Ideas.md` and `Week.md` link to `Plan.md`; `Week.md` is read-only.

| Step | Action                         | Expected result                                                                              |
|------|--------------------------------|----------------------------------------------------------------------------------------------|
| 1    | Rename "Plan" to "Road Map".    | The note is renamed; `Ideas.md` links to `Road-Map.md`.                                       |
| 2    | Look at the status bar.         | It reports 1 link updated and names `Week` as still linking to the old title.                |
| 3    | Look at `Week.md`.              | It is unchanged.                                                                             |

## TC-006-13: Linking note open with unsaved changes

**Flow:** A7  
**Preconditions:** `Ideas.md` links to `Plan.md` and is open in a tab; the user has just typed "draft" at its end and autosave has not run yet.

| Step | Action                                    | Expected result                                                                                    |
|------|-------------------------------------------|----------------------------------------------------------------------------------------------------|
| 1    | Rename "Plan" to "Road Map" from the note overview. | The tab of `Ideas` shows the updated link, the typed "draft", and the cursor where it was. |
| 2    | Wait 2 seconds and look at `Ideas.md` with another program. | It holds the updated link and "draft".                                     |

## TC-006-14: User cancels

**Flow:** A8  
**Preconditions:** `Plan.md` exists.

| Step | Action                                             | Expected result                                       |
|------|----------------------------------------------------|-------------------------------------------------------|
| 1    | Open the rename dialog, type "Road Map", and cancel. | The dialog closes; `Plan.md` and all links are unchanged. |
| 2    | Open the dialog, enter `Plan:`, then cancel.        | The dialog closes; nothing is changed.                 |

## TC-006-15: Open tab, remembered tabs, and recent notes

**Flow:** Main success scenario step 7, FR-017, FR-018  
**Preconditions:** `Plan.md` is open in the second of three tabs, scrolled to its end, and is the most recently opened note.

| Step | Action                                    | Expected result                                                                       |
|------|-------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Rename it to "Road Map" from its tab's context menu. | The second tab shows "Road Map", still scrolled to the end; no tab closed or opened. |
| 2    | Look at the note overview.                | "Road Map" is listed first; "Plan" is not listed.                                     |
| 3    | Restart Underline.                        | The three tabs open again, the second showing "Road Map".                             |

## TC-006-16: Home note

**Flow:** BR-009  
**Preconditions:** The workspace holds `Home.md`.

| Step | Action                                  | Expected result                                                                 |
|------|-----------------------------------------|---------------------------------------------------------------------------------|
| 1    | Open the rename dialog for `Home`.      | The dialog warns that the wiki then has no start page.                          |
| 2    | Rename it to "Start" and confirm.       | The note is renamed to `Start.md`; links to it are updated.                     |
| 3    | Open the rename dialog for `Plan`.      | No warning is shown.                                                            |

## TC-006-17: Renaming back, and the rename in git

**Flow:** BR-007, BR-010  
**Preconditions:** The workspace is a git repository with nothing to commit. `Ideas.md` links to `Plan.md`.

| Step | Action                                    | Expected result                                                                          |
|------|-------------------------------------------|------------------------------------------------------------------------------------------|
| 1    | Rename "Plan" to "Road Map".              | The changes to commit show `Plan.md` renamed to `Road-Map.md` and `Ideas.md` changed.    |
| 2    | Rename "Road Map" back to "Plan".         | The status bar reports 1 link updated.                                                   |
| 3    | Look at the changes to commit.            | There are none: every file is as it was before step 1.                                   |

## TC-006-18: Todo links follow

**Flow:** BR-006  
**Preconditions:** A todo links to `Plan.md` (UC-020).

| Step | Action                       | Expected result                                                  |
|------|------------------------------|------------------------------------------------------------------|
| 1    | Rename "Plan" to "Road Map". | The todo links to "Road Map"; following its link opens the note. |

## TC-006-19: Keyboard alone

**Flow:** Main success scenario step 1, NFR-011  
**Preconditions:** `Plan.md` is listed in the note overview and open in the active tab.

| Step | Action                                                           | Expected result                                           |
|------|------------------------------------------------------------------|-----------------------------------------------------------|
| 1    | Press F2, type "Road Map", press Enter.                           | The note is renamed.                                      |
| 2    | Move the focus to the note overview with the keyboard, select "Road Map", open its context menu with the keyboard, and choose to rename. | The rename dialog opens. |
| 3    | Move between title and folder with Tab and press Escape.          | The dialog closes without a change.                       |
