# Test Cases: Open Workspace

**Use Case:** [UC-002 Open Workspace](../use_cases/UC-002-open-workspace.md)  
**Requirements:** [FR-002, FR-004, FR-017, FR-018, FR-020, FR-070, FR-071, NFR-001, NFR-003, C-002, C-006](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements           |
|-----------|----------------------------------------------------|------------------------|
| TC-002-01 | Main success scenario, recently opened workspace   | FR-002, FR-018, FR-071 |
| TC-002-02 | Main success scenario, cloned repository, first time | FR-002, BR-001, BR-005 |
| TC-002-03 | First note: README, or none                        | BR-005                 |
| TC-002-04 | Derived default format                             | BR-002, FR-070         |
| TC-002-05 | Which files are notes                              | BR-003, FR-004         |
| TC-002-06 | Same title in two folders                          | BR-004                 |
| TC-002-07 | Order of the note overview (A to Z ignores case)   | BR-008, FR-017         |
| TC-002-08 | Remembered tab of a note that is gone              | Step 8                 |
| TC-002-09 | Previous workspace closed, tabs kept               | Step 5, FR-071         |
| TC-002-10 | Status bar with directory and git sync state       | Step 9, FR-020         |
| TC-002-11 | A1 Started with a directory                        | FR-002                 |
| TC-002-12 | A2 Started without a directory                     | FR-071                 |
| TC-002-13 | A3/A4 Directory does not exist                     | BR-006                 |
| TC-002-14 | A5/A6 Directory inside a workspace                 | BR-003, BR-007         |
| TC-002-15 | A7 Directory cannot be read                        | —                      |
| TC-002-16 | A8 Workspace settings file cannot be read          | FR-070, C-006          |
| TC-002-17 | A9 Workspace already open                          | —                      |
| TC-002-18 | A10 Cancel at each point                           | —                      |
| TC-002-19 | A11 Home directory                                 | BR-007, C-006          |
| TC-002-20 | A12 Remembered tabs cannot be kept                 | FR-071                 |
| TC-002-21 | A13 Some files cannot be read                      | —                      |
| TC-002-22 | Large workspace                                    | NFR-001, NFR-003       |

## TC-002-01: Open a recently opened workspace

**Flow:** Main success scenario  
**Preconditions:** The workspaces `alpha` and `beta` exist and were opened before, `beta` last. `alpha` had the notes `Plan` and `Ideas`, with `Plan` open in a tab. `beta` is open.

| Step | Action                                     | Expected result                                                                                      |
|------|--------------------------------------------|------------------------------------------------------------------------------------------------------|
| 1    | Ask to open a workspace.                   | The system lists `beta` first and `alpha` second, and offers to choose another directory.           |
| 2    | Choose `alpha`.                            | The note overview lists the notes of `alpha`; `Plan` is open in a tab as before.                    |
| 3    | Look at the status bar.                    | It shows the directory of `alpha` and reports that the workspace was opened.                        |
| 4    | Ask to open a workspace again.             | `alpha` is listed first now, `beta` second.                                                          |

## TC-002-02: Open a cloned repository for the first time

**Flow:** Main success scenario, BR-001, BR-005  
**Preconditions:** The directory `wiki` is a git repository with no uncommitted changes. It holds `Home.md` and `Setup.md` and no workspace settings file. Underline has never opened it.

| Step | Action                                                  | Expected result                                                                         |
|------|---------------------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | Ask to open a workspace and choose the directory `wiki`. | The note overview lists `Home` and `Setup`; `Home` is open in a tab.                  |
| 2    | Look at `wiki` with another program.                    | No workspace settings file was added; `Home.md` and `Setup.md` are unchanged; the hidden Underline folder exists. |
| 3    | Look at the git state of `wiki`.                        | There are no changes and no untracked files.                                            |

## TC-002-03: First note is README, or none

**Flow:** BR-005  
**Preconditions:** The directory `docs-a` holds `README.md` and `Guide.md` but no Home note. The directory `docs-b` holds only `Guide.md`. Neither was opened before.

| Step | Action                    | Expected result                                       |
|------|---------------------------|-------------------------------------------------------|
| 1    | Open the directory `docs-a`. | `README` is open in a tab.                         |
| 2    | Open the directory `docs-b`. | No tab is open; the note overview lists `Guide`.   |

## TC-002-04: Derive the default format

**Flow:** Step 6, BR-002  
**Preconditions:** None of the directories has a workspace settings file. `mostly-adoc` holds two `.adoc` notes and one `.md` note. `tie` holds one of each. `empty` holds no notes.

| Step | Action                                            | Expected result                         |
|------|---------------------------------------------------|-----------------------------------------|
| 1    | Open `mostly-adoc` and create a note `New` (UC-003). | `New` is stored as `New.adoc`.        |
| 2    | Open `tie` and create a note `New`.               | `New` is stored as `New.md`.            |
| 3    | Open `empty` and create a note `New`.             | `New` is stored as `New.md`.            |
| 4    | Look at the three directories.                    | None of them has a workspace settings file. |

## TC-002-05: Which files are notes

**Flow:** Step 7, BR-003  
**Preconditions:** The git repository `project` holds `Home.md`, `guide/Install.adoc`, `notes.txt`, `.draft.md`, `.cache/Old.md`, `vendor/lib/README.md` where the repository ignores `vendor/`, and a folder `sub` that is a workspace of its own with `sub/Inner.md`.

| Step | Action                    | Expected result                                                                                                  |
|------|---------------------------|------------------------------------------------------------------------------------------------------------------|
| 1    | Open the directory `project`. | The note overview lists exactly `Home` and `Install`.                                                        |
| 2    | Open `Install`.           | It is shown as an AsciiDoc note.                                                                                  |

## TC-002-06: Same title in two folders

**Flow:** BR-004  
**Preconditions:** The directory `team` holds `a/Home.md`, `b/Home.md`, and `Plan.md`.

| Step | Action                  | Expected result                                                                       |
|------|-------------------------|---------------------------------------------------------------------------------------|
| 1    | Open the directory `team`. | The note overview lists both `Home` notes, each with its folder `a` or `b`; `Plan` is listed without a folder. |
| 2    | Open the `Home` note of `b`. | The tab shows the content of `b/Home.md`.                                         |

## TC-002-07: Order of the note overview

**Flow:** Step 7, BR-008  
**Preconditions:** The workspace `order` holds the notes `Zebra`, `apple`, `Mango`, and `Berry`. Earlier, `Mango` was opened, then `Zebra`.

| Step | Action                | Expected result                                                  |
|------|-----------------------|------------------------------------------------------------------|
| 1    | Open the workspace `order`. | The note overview lists `Zebra`, `Mango`, `apple`, `Berry` in this order. |

## TC-002-08: A remembered tab of a note that is gone

**Flow:** Step 8  
**Preconditions:** The workspace `gone` had the note `Draft` open in a tab. `Draft.md` was deleted with another program afterwards; the workspace also holds `Home.md`.

| Step | Action                 | Expected result                                                         |
|------|------------------------|-------------------------------------------------------------------------|
| 1    | Open the workspace `gone`. | No tab shows `Draft`; the note overview does not list it; no error is reported. |

## TC-002-09: The previous workspace is closed and its tabs are kept

**Flow:** Step 5  
**Preconditions:** Workspace `old` is open with the note `A` in a tab. Workspace `other` exists.

| Step | Action                         | Expected result                                      |
|------|--------------------------------|------------------------------------------------------|
| 1    | Open the workspace `other`.    | The window shows `other`; `A` is no longer shown.    |
| 2    | Open the workspace `old` again. | `A` is open in a tab again.                         |

## TC-002-10: Status bar with directory and git sync state

**Flow:** Step 9, FR-020  
**Preconditions:** The workspace `synced` is a git repository with a git remote. The workspace `plain` is not a git repository.

| Step | Action                     | Expected result                                                                 |
|------|----------------------------|---------------------------------------------------------------------------------|
| 1    | Open the workspace `synced`. | The status bar shows its directory and the state of the last git sync, or that no git sync has run yet. |
| 2    | Open the workspace `plain`.  | The status bar shows its directory and no git sync state.                     |

## TC-002-11: Start with a directory

**Flow:** A1  
**Preconditions:** The workspace `beta` was opened last. The directory `alpha` is a workspace.

| Step | Action                                     | Expected result                                       |
|------|--------------------------------------------|-------------------------------------------------------|
| 1    | Start Underline and name the directory `alpha`. | `alpha` opens directly, without the choice of a workspace. |
| 2    | Restart Underline without naming a directory. | `alpha` opens, as the last opened workspace.       |

## TC-002-12: Start without a directory

**Flow:** A2  
**Preconditions:** Underline was never used on this machine, or every workspace it knew is gone.

| Step | Action                                         | Expected result                                                          |
|------|------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Start Underline without naming a directory.    | The system offers to open a workspace or to create one; no directory was created. |
| 2    | Choose to open a workspace and choose the directory `wiki`. | `wiki` opens.                                               |
| 3    | Restart Underline without naming a directory.  | `wiki` opens directly.                                                   |

## TC-002-13: The directory does not exist

**Flow:** A3, A4  
**Preconditions:** The workspace `moved` was opened before; its directory was deleted afterwards while Underline was running. The directory `fresh` does not exist.

| Step | Action                                          | Expected result                                                                                    |
|------|-------------------------------------------------|----------------------------------------------------------------------------------------------------|
| 1    | Ask to open a workspace and choose `moved` from the recently opened workspaces. | The system says that the directory does not exist and offers to create a new workspace there. |
| 2    | Ask to open a workspace again.                  | `moved` is no longer listed.                                                                       |
| 3    | Choose the directory `fresh`.                   | The same message and offer appear; `fresh` was not created.                                        |
| 4    | Accept the offer.                               | The system asks for the default format (UC-001 step 3); after confirming, `fresh` holds a new workspace. |

## TC-002-14: A directory inside a workspace

**Flow:** A5, A6, BR-003  
**Preconditions:** The workspace `outer` holds `Home.md` and the folder `inner` with `Inner.md`. `outer` was opened before.

| Step | Action                                                    | Expected result                                                                         |
|------|-----------------------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | Ask to open a workspace and choose `outer/inner`.         | The system says that the directory lies inside `outer` and offers to open `outer` instead or `inner` anyway. |
| 2    | Choose to open `outer` instead.                           | `outer` opens; its note overview lists `Home` and `Inner`.                              |
| 3    | Ask again, choose `outer/inner`, and open it anyway.       | `inner` opens as a workspace; its note overview lists `Inner`.                          |
| 4    | Open `outer` again.                                        | Its note overview lists only `Home`.                                                    |

## TC-002-15: The directory cannot be read

**Flow:** A7  
**Preconditions:** The user may not read the directory `locked`. Workspace `old` is open.

| Step | Action                                        | Expected result                                                        |
|------|-----------------------------------------------|------------------------------------------------------------------------|
| 1    | Ask to open a workspace and choose `locked`.  | The system says that the directory cannot be read because of missing permission. |
| 2    | Cancel.                                       | `old` is still open with its tabs.                                     |

## TC-002-16: The workspace settings file cannot be read

**Flow:** A8  
**Preconditions:** The user settings file sets the theme Nord. The directory `broken` holds `Home.adoc`, `Plan.adoc`, and a workspace settings file whose content was damaged by a hand edit.

| Step | Action                         | Expected result                                                                               |
|------|--------------------------------|-----------------------------------------------------------------------------------------------|
| 1    | Open the directory `broken`.   | The workspace opens with the theme Nord; the status bar names the workspace settings file and the problem. |
| 2    | Create a note `New` (UC-003).  | `New` is stored as `New.adoc`, the derived default format.                                    |
| 3    | Look at the workspace settings file. | It is unchanged.                                                                         |

## TC-002-17: The workspace is already open

**Flow:** A9  
**Preconditions:** Workspace `alpha` is open with the notes `Plan` and `Ideas` in tabs, `Ideas` active.

| Step | Action                                       | Expected result                                             |
|------|----------------------------------------------|-------------------------------------------------------------|
| 1    | Ask to open a workspace and choose `alpha`.  | The dialog closes; the tabs are unchanged and `Ideas` is still active. |

## TC-002-18: Cancel at each point

**Flow:** A10  
**Preconditions:** Workspace `old` is open. The directory `missing` does not exist. The workspace `outer` holds the folder `inner`.

| Step | Action                                                                  | Expected result                         |
|------|-------------------------------------------------------------------------|-----------------------------------------|
| 1    | Ask to open a workspace and cancel at the choice.                       | `old` is still open.                    |
| 2    | Ask again, choose `missing`, and cancel at the offer to create it.      | `old` is still open; `missing` does not exist. |
| 3    | Ask again, choose `outer/inner`, and cancel at the offer.               | `old` is still open.                    |
| 4    | Ask again, choose the home directory, and cancel.                       | `old` is still open.                    |

## TC-002-19: The home directory is refused

**Flow:** A11  
**Preconditions:** The user's home directory holds the user settings file.

| Step | Action                                               | Expected result                                                           |
|------|------------------------------------------------------|---------------------------------------------------------------------------|
| 1    | Ask to open a workspace and choose the home directory. | The system says that the home directory cannot be a workspace and asks for another directory. |
| 2    | Look at the home directory.                          | No Underline folder was added.                                            |

## TC-002-20: Remembered tabs cannot be kept

**Flow:** A12  
**Preconditions:** The user may read but not change the directory `readonly`, which holds `Home.md`. The workspace `damaged` holds `Home.md` and `Plan.md`; what Underline remembered in its Underline folder was damaged.

| Step | Action                                  | Expected result                                                                                         |
|------|-----------------------------------------|---------------------------------------------------------------------------------------------------------|
| 1    | Open the directory `readonly`.          | The workspace opens with `Home` in a tab; the status bar says once that its tabs and recently opened notes cannot be remembered. |
| 2    | Open `Home` and another note in turn.   | The message is not repeated.                                                                            |
| 3    | Open the workspace `damaged`.           | It opens as if for the first time, with `Home` in a tab; the status bar says that what was remembered could not be read. |

## TC-002-21: Some files cannot be read

**Flow:** A13  
**Preconditions:** The workspace `partly` holds `Home.md`, `Plan.md`, and the folder `private`, which the user may not read.

| Step | Action                      | Expected result                                                                     |
|------|-----------------------------|-------------------------------------------------------------------------------------|
| 1    | Open the workspace `partly`. | The note overview lists `Home` and `Plan`; the status bar names the folder `private` as not readable. |

## TC-002-22: Large workspace

**Flow:** Main success scenario, NFR-001, NFR-003  
**Preconditions:** The workspace `medium` holds 1,000 notes and the workspace `big` 10,000 notes in 100 folders, on a machine with an SSD. Each was opened before with one note in a tab; `medium` was opened last.

| Step | Action                                          | Expected result                                                         |
|------|-------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Start Underline without naming a directory.     | The note that was open in `medium` is shown within 1 second (NFR-001). |
| 2    | Open the workspace `big`.                       | Its note overview lists its notes within 1 second (NFR-003).            |
