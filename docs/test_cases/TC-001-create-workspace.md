# Test Cases: Create Workspace

**Use Case:** [UC-001 Create Workspace](../use_cases/UC-001-create-workspace.md)  
**Requirements:** [FR-001, FR-002, FR-070, FR-071, FR-072, NFR-014, C-002, C-006](../requirements.md)

## Coverage

| Test case | Flow / rule                          | Requirements             |
|-----------|--------------------------------------|--------------------------|
| TC-001-01 | Main success scenario, Markdown      | FR-001, C-002, C-006     |
| TC-001-02 | Main success scenario, AsciiDoc      | FR-001, C-002, BR-001    |
| TC-001-03 | Main success scenario, existing empty directory | FR-001        |
| TC-001-04 | Previous workspace closed (step 6)   | FR-071                   |
| TC-001-05 | A1 Directory holds files or folders  | FR-001, BR-002           |
| TC-001-06 | A1 Hidden content counts             | BR-002                   |
| TC-001-07 | A2 Open the directory instead        | FR-002                   |
| TC-001-08 | A3 Git repository without remote     | FR-072, BR-007           |
| TC-001-09 | A3 Git repository with remote, offline | FR-072, NFR-014, BR-004 |
| TC-001-10 | A4 Cancel at each point              | FR-001                   |
| TC-001-11 | A5 Workspace cannot be created       | FR-001                   |
| TC-001-12 | A6 Leftovers cannot be removed       | FR-001                   |
| TC-001-13 | A7 Directory inside a workspace      | BR-006                   |
| TC-001-14 | Directory fills up before confirming (step 5) | BR-002          |
| TC-001-15 | Default format applies to new notes only | BR-005, FR-070       |

## TC-001-01: Create a Markdown workspace in a new directory

**Flow:** Main success scenario  
**Preconditions:** Underline is running. The directory `notes-a` does not exist.

| Step | Action                                                              | Expected result                                                                                 |
|------|---------------------------------------------------------------------|-------------------------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace.                                      | The system asks for a directory.                                                                |
| 2    | Choose the new directory `notes-a`.                                 | The system asks for the default format with Markdown preselected and shows the git repository option switched off. |
| 3    | Confirm without changing anything.                                  | The directory `notes-a` exists.                                                                 |
| 4    | Look at the window.                                                 | The note overview lists Home; Home is open in a tab in the facade and shows only the heading Home; the status bar reports that the workspace was created. |
| 5    | Look at the directory `notes-a` with another program.               | It holds exactly `Home.md` and the workspace settings file `underline.yaml`, which names Markdown as default format. |
| 6    | Restart Underline.                                                  | The workspace `notes-a` opens as the last opened workspace.                                     |

## TC-001-02: Create an AsciiDoc workspace

**Flow:** Main success scenario, BR-001  
**Preconditions:** The directory `notes-b` does not exist.

| Step | Action                                                     | Expected result                                                                  |
|------|------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `notes-b`.        | Markdown is preselected as default format.                                       |
| 2    | Choose AsciiDoc and confirm.                               | Home is open in a tab in the facade.                                             |
| 3    | Look at the directory `notes-b` with another program.      | It holds exactly `Home.adoc` and `underline.yaml`, which names AsciiDoc as default format. |
| 4    | Ask to create another workspace in a new directory `notes-c`. | Markdown is preselected again; the earlier choice of AsciiDoc is not carried over. |

## TC-001-03: Create a workspace in an existing empty directory

**Flow:** Main success scenario, step 1  
**Preconditions:** The directory `notes-d` exists and is empty.

| Step | Action                                                         | Expected result                                            |
|------|----------------------------------------------------------------|------------------------------------------------------------|
| 1    | Ask to create a new workspace, choose `notes-d`, and confirm.  | The workspace is created and Home is open in a tab.        |
| 2    | Look at `notes-d` with another program.                        | It holds exactly `Home.md` and `underline.yaml`.           |

## TC-001-04: The previous workspace is closed and its tabs are kept

**Flow:** Main success scenario, step 6  
**Preconditions:** Workspace `old` is open with the notes `A` and `B` in two tabs. The directory `notes-e` does not exist.

| Step | Action                                                      | Expected result                                                         |
|------|-------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Create a new workspace in `notes-e` with the defaults.      | The window shows `notes-e` with only the Home tab; `A` and `B` are no longer shown. |
| 2    | Open the workspace `old` again (UC-002).                    | The tabs `A` and `B` come back in the same order.                       |

## TC-001-05: Refuse a directory that holds files or folders

**Flow:** A1  
**Preconditions:** The directory `full` holds the file `readme.txt`. The directory `only-folder` holds just an empty folder `sub`. The directory `notes-f` does not exist.

| Step | Action                                              | Expected result                                                                                   |
|------|-----------------------------------------------------|---------------------------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `full`.    | The system says that the directory already holds files or folders and offers to open it as a workspace. |
| 2    | Choose another directory: `only-folder`.            | The same message and offer appear.                                                                |
| 3    | Choose another directory: `notes-f`, and confirm.   | The workspace is created in `notes-f`.                                                            |
| 4    | Look at `full` and `only-folder` with another program. | Both are unchanged.                                                                            |

## TC-001-06: Hidden content makes a directory non-empty

**Flow:** A1, BR-002  
**Preconditions:** The directory `hidden-file` holds only a hidden file `.DS_Store`. The directory `bare-git` holds only a git repository (`.git`).

| Step | Action                                                     | Expected result                                                         |
|------|------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `hidden-file`.    | The system says that the directory already holds files or folders.      |
| 2    | Choose `bare-git` instead.                                 | The system says the same and offers to open it as a workspace.          |
| 3    | Cancel.                                                    | Both directories are unchanged.                                         |

## TC-001-07: Open a non-empty directory instead

**Flow:** A2  
**Preconditions:** The directory `existing` holds the note `Ideas.md`.

| Step | Action                                                 | Expected result                                                                   |
|------|--------------------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `existing`.   | The system offers to open it as a workspace.                                      |
| 2    | Accept the offer.                                      | The workspace `existing` is open (UC-002) and the note overview lists `Ideas`.    |
| 3    | Look at `existing` with another program.               | No Home note was added; the directory holds what UC-002 leaves there.             |

## TC-001-08: Create a workspace as a git repository without a remote

**Flow:** A3, BR-007  
**Preconditions:** The user's git setup names `trunk` as default branch. The directory `notes-g` does not exist.

| Step | Action                                                                  | Expected result                                                                 |
|------|-------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace in `notes-g` and switch the git repository option on. | The system asks for the address of a git remote and marks it as optional. |
| 2    | Leave the address empty and confirm.                                    | Home is open in a tab in the facade; the status bar reports that the workspace was created. |
| 3    | Look at the git state of the workspace.                                 | The current branch is `trunk`; there are no uncommitted changes; the workspace has no git remote. |
| 4    | Look at the history of the workspace.                                   | It holds exactly one commit, titled "Create workspace", with `Home.md` and `underline.yaml`. |
| 5    | Repeat steps 1–3 in `notes-h` with a git setup that names no default branch. | The current branch is `main`.                                           |

## TC-001-09: Create a git workspace with a remote while offline

**Flow:** A3, BR-004, NFR-014  
**Preconditions:** The machine has no network connection. The directory `notes-i` does not exist.

| Step | Action                                                                                 | Expected result                                                          |
|------|----------------------------------------------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Ask to create a new workspace in `notes-i`, switch the git repository option on, enter the address `https://example.com/team/notes.git`, and confirm. | The workspace is created without delay or error, and Home is open in a tab. |
| 2    | Look at the git remotes of the workspace.                                              | It has one git remote named origin with exactly the entered address.     |
| 3    | Look at the history.                                                                   | It holds the single commit "Create workspace".                           |

## TC-001-10: Cancel at each point

**Flow:** A4  
**Preconditions:** Workspace `old` is open. The directory `notes-j` does not exist.

| Step | Action                                                                                   | Expected result                                                 |
|------|------------------------------------------------------------------------------------------|-----------------------------------------------------------------|
| 1    | Ask to create a new workspace and cancel while choosing the directory.                   | The dialog closes; `old` is still open.                         |
| 2    | Ask again, choose `notes-j`, and cancel at the default format question.                  | The dialog closes; `notes-j` does not exist; `old` is still open. |
| 3    | Ask again, choose `notes-j`, switch the git repository option on, and cancel at the git remote address. | The dialog closes; `notes-j` does not exist; `old` is still open. |

## TC-001-11: Report a workspace that cannot be created

**Flow:** A5  
**Preconditions:** Workspace `old` is open. The directory `locked` exists, is empty, and the user may not write to it. The directory `parent-locked` may not be written to.

| Step | Action                                                                                  | Expected result                                                                                   |
|------|-----------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------|
| 1    | Create a new workspace in `locked` with the defaults.                                   | The status bar reports that the workspace could not be created because of missing permission.    |
| 2    | Look at `locked`.                                                                       | It is still empty and still exists.                                                               |
| 3    | Create a new workspace in the new directory `parent-locked/notes-k`.                    | The status bar reports that the workspace could not be created; `parent-locked/notes-k` does not exist. |
| 4    | Look at the window.                                                                     | `old` is still the open workspace with its tabs.                                                  |

## TC-001-12: Name leftovers that cannot be removed

**Flow:** A6  
**Preconditions:** Creating the workspace in the new directory `notes-l` fails after the directory and `underline.yaml` were written, and removing them fails as well (for example because the directory is made read-only in between).

| Step | Action                                             | Expected result                                                                                           |
|------|----------------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| 1    | Create a new workspace in `notes-l`.               | The status bar reports that the workspace could not be created and names `notes-l` as left behind.       |
| 2    | Look at `notes-l`.                                 | Exactly the leftovers named in the status bar exist.                                                      |

## TC-001-13: Refuse a directory inside an existing workspace

**Flow:** A7, BR-006  
**Preconditions:** A workspace exists in the directory `outer`. The directory `outer/inner` does not exist. The directory `notes-m` does not exist.

| Step | Action                                                 | Expected result                                                                              |
|------|--------------------------------------------------------|----------------------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `outer/inner`. | The system says that the directory lies inside the workspace `outer` and asks for another directory. |
| 2    | Choose `notes-m` and confirm.                          | The workspace is created in `notes-m`.                                                       |
| 3    | Look at `outer`.                                       | `outer/inner` does not exist; `outer` is unchanged.                                          |

## TC-001-14: The directory fills up before the user confirms

**Flow:** Step 5, A1  
**Preconditions:** The directory `notes-n` exists and is empty.

| Step | Action                                                                          | Expected result                                                                       |
|------|---------------------------------------------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Ask to create a new workspace and choose `notes-n`.                             | The system asks for the default format.                                               |
| 2    | Before confirming, put a file `draft.txt` into `notes-n` with another program.  | —                                                                                     |
| 3    | Confirm.                                                                        | The system says that the directory already holds files or folders and offers to open it. |
| 4    | Look at `notes-n`.                                                              | It holds only `draft.txt`.                                                            |

## TC-001-15: The default format applies to new notes only

**Flow:** BR-005  
**Preconditions:** A Markdown workspace was created in `notes-o` (TC-001-01).

| Step | Action                                                       | Expected result                                                   |
|------|--------------------------------------------------------------|-------------------------------------------------------------------|
| 1    | Change the default format of `notes-o` to AsciiDoc (UC-010). | The workspace settings file names AsciiDoc as default format.    |
| 2    | Create a new note `Plan` (UC-003).                           | The note is stored as `Plan.adoc`.                                |
| 3    | Look at the Home note.                                       | It is still `Home.md`, unchanged.                                 |
