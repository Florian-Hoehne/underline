# Test Cases: Create Note

**Use Case:** [UC-003 Create Note](../use_cases/UC-003-create-note.md)  
**Requirements:** [FR-003, FR-004, FR-018, FR-021, FR-071, NFR-006, NFR-013, C-002](../requirements.md)

## Coverage

| Test case | Flow / rule                                         | Requirements          |
|-----------|-----------------------------------------------------|-----------------------|
| TC-003-01 | Main success scenario, Markdown in the top folder   | FR-003, FR-018, FR-021 |
| TC-003-02 | Folder of the active note preselected               | BR-006                |
| TC-003-03 | Other note format for one note                      | BR-005, FR-004        |
| TC-003-04 | Default format in a workspace without settings file | BR-005                |
| TC-003-05 | New folders several levels deep                     | BR-006                |
| TC-003-06 | Title becomes the file name                         | BR-001                |
| TC-003-07 | A1 Titles that are not allowed                      | BR-002, NFR-013       |
| TC-003-08 | A1 Title and place too long                         | BR-002                |
| TC-003-09 | A2/A3 Note exists                                   | BR-003                |
| TC-003-10 | A2 Same file name from another title                | BR-001, BR-003        |
| TC-003-11 | A2 File appears before the note is written          | Step 5, BR-003        |
| TC-003-12 | A4 Folders that cannot take a note                  | BR-006                |
| TC-003-13 | A5 Note cannot be created                           | NFR-006               |
| TC-003-14 | A6 Cancel at each point                             | —                     |
| TC-003-15 | Written at once and remembered after a crash        | BR-007, FR-071        |
| TC-003-16 | A new note in a git repository                      | BR-007                |

## TC-003-01: Create a Markdown note in the top folder

**Flow:** Main success scenario  
**Preconditions:** The Markdown workspace `notes` is open with no tab open. It holds `Home.md` and `Ideas.md`.

| Step | Action                                     | Expected result                                                                                      |
|------|--------------------------------------------|------------------------------------------------------------------------------------------------------|
| 1    | Ask to create a new note.                  | The system asks for a title; the top folder and Markdown are preselected.                            |
| 2    | Type the title `Meeting Notes`.            | The dialog shows the file name `Meeting-Notes.md`.                                                   |
| 3    | Confirm.                                   | `Meeting Notes` is open in a tab in the facade, with the cursor below the title; the status bar reports that the note was created. |
| 4    | Look at the note overview.                 | `Meeting Notes` is listed first.                                                                     |
| 5    | Look at `Meeting-Notes.md` with another program. | It holds only the heading `# Meeting Notes`.                                                   |

## TC-003-02: The folder of the active note is preselected

**Flow:** Step 2, BR-006  
**Preconditions:** The workspace `notes` holds `guide/Install.md`, which is open in the active tab.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Ask to create a new note.                      | The folder `guide` is preselected.                    |
| 2    | Enter the title `Upgrade` and confirm.         | The note `guide/Upgrade.md` is created and opened.    |
| 3    | Ask to create a new note, choose the top folder, enter `Changelog`, and confirm. | The note `Changelog.md` is created in the top folder. |

## TC-003-03: Another note format for one note

**Flow:** BR-005, FR-004  
**Preconditions:** The workspace `notes` has Markdown recorded as default format in its workspace settings file.

| Step | Action                                                     | Expected result                                                 |
|------|------------------------------------------------------------|-----------------------------------------------------------------|
| 1    | Ask to create a new note.                                  | Markdown is preselected.                                        |
| 2    | Choose AsciiDoc, enter `Spec`, and confirm.                | The note `Spec.adoc` holds only the heading `= Spec`; it is shown as an AsciiDoc note. |
| 3    | Ask to create another note.                                | Markdown is preselected again.                                  |
| 4    | Look at the workspace settings file.                       | It still names Markdown as default format.                      |

## TC-003-04: Default format in a workspace without a settings file

**Flow:** BR-005, UC-002 BR-002  
**Preconditions:** The workspace `docs` has no workspace settings file and holds `A.adoc` and `B.md`, so its derived default format is Markdown.

| Step | Action                                                    | Expected result                                       |
|------|-----------------------------------------------------------|-------------------------------------------------------|
| 1    | Create the note `C` and choose AsciiDoc.                  | `C.adoc` is created.                                  |
| 2    | Ask to create another note.                               | AsciiDoc is preselected, as most notes are AsciiDoc now. |
| 3    | Look at `docs`.                                           | It still has no workspace settings file.              |

## TC-003-05: New folders several levels deep

**Flow:** Step 5, BR-006  
**Preconditions:** The workspace `notes` has no folder `guide`.

| Step | Action                                                              | Expected result                                                |
|------|---------------------------------------------------------------------|----------------------------------------------------------------|
| 1    | Ask to create a new note, enter the folder `guide/setup` and the title `Linux`, and confirm. | `guide/setup/Linux.md` is created and opened.  |
| 2    | Look at the note overview.                                          | `Linux` is listed first.                                       |

## TC-003-06: The title becomes the file name

**Flow:** BR-001  
**Preconditions:** The Markdown workspace `notes` is open.

| Step | Action                                       | Expected result                                                          |
|------|----------------------------------------------|--------------------------------------------------------------------------|
| 1    | Create a note titled `  Getting   Started  `. | The file is `Getting-Started.md`; the tab and the overview show `Getting Started`. |
| 2    | Create a note titled `Pre-Release`.          | The file is `Pre-Release.md`; the tab and the overview show `Pre Release`. |
| 3    | Create a note titled `Über uns`.             | The file is `Über-uns.md`; the overview shows `Über uns`.                |

## TC-003-07: Titles that are not allowed

**Flow:** A1, BR-002, NFR-013  
**Preconditions:** The workspace `notes` is open. The test runs the same on Linux, Windows, and macOS.

| Step | Action                                                                 | Expected result                                                         |
|------|------------------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Ask to create a new note, leave the title empty, and confirm.          | The system says that the title is empty and keeps the dialog open.     |
| 2    | Enter `.hidden` and confirm.                                           | The system says that a title cannot start with a dot.                   |
| 3    | Enter `Draft.` and confirm.                                            | The system says that a title cannot end with a dot.                     |
| 4    | Enter in turn `a/b`, `a\b`, `a:b`, `a*b`, `a?b`, `a"b`, `a<b`, `a>b`, `a|b`, and confirm each. | Each is refused with the character named.       |
| 5    | Enter in turn `CON`, `nul`, `Com1`, `LPT9`, and confirm each.          | Each is refused as a name Windows reserves.                             |
| 6    | Enter `Console` and confirm.                                           | The note `Console.md` is created.                                       |
| 7    | Look at the workspace.                                                 | No other file or folder was created.                                    |

## TC-003-08: Title and place too long

**Flow:** A1, BR-002  
**Preconditions:** The workspace `notes` is open.

| Step | Action                                                                       | Expected result                                       |
|------|------------------------------------------------------------------------------|-------------------------------------------------------|
| 1    | Enter a title of 251 characters and confirm.                                 | The system says that the title is too long.           |
| 2    | Enter a title of exactly 250 characters and confirm.                         | The note is created.                                  |
| 3    | Enter a folder path of 300 characters and a title of 250 characters, and confirm. | The system says that the place of the note is too long; nothing is created. |

## TC-003-09: The note exists

**Flow:** A2, A3, BR-003  
**Preconditions:** The workspace `notes` holds `Plan.md` with the text `keep me`, and `guide/Spec.adoc`.

| Step | Action                                                                 | Expected result                                                         |
|------|------------------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Ask to create a note in the top folder titled `Plan` and confirm.      | The system says that the note exists and offers to open it.            |
| 2    | Change the title to `plan` and confirm.                                | The same message appears.                                               |
| 3    | Choose AsciiDoc and confirm.                                           | The same message appears.                                               |
| 4    | Accept the offer to open it.                                           | `Plan` is open in a tab and still holds `keep me`.                      |
| 5    | Create a note in the folder `guide` titled `spec` in Markdown.         | The system says that the note exists.                                   |
| 6    | Create a note in the top folder titled `Spec`.                         | `Spec.md` is created in the top folder.                                 |

## TC-003-10: The same file name from another title

**Flow:** A2, BR-001, BR-003  
**Preconditions:** The workspace `notes` holds `Pre-Release.md`.

| Step | Action                                        | Expected result                                                         |
|------|-----------------------------------------------|-------------------------------------------------------------------------|
| 1    | Type the title `Pre Release`.                 | The dialog shows the file name `Pre-Release.md`.                        |
| 2    | Confirm.                                      | The system says that the note exists and offers to open it.            |

## TC-003-11: A file appears before the note is written

**Flow:** Step 5, A2  
**Preconditions:** The workspace `notes` has no note `Race`.

| Step | Action                                                                                   | Expected result                                                      |
|------|------------------------------------------------------------------------------------------|----------------------------------------------------------------------|
| 1    | Ask to create a note titled `Race`; before confirming, write `Race.md` with the text `other program` with another program. | —                        |
| 2    | Confirm.                                                                                 | The system says that the note exists and offers to open it.         |
| 3    | Look at `Race.md`.                                                                       | It still holds `other program`.                                      |

## TC-003-12: Folders that cannot take a note

**Flow:** A4, BR-006  
**Preconditions:** The workspace `project` is a git repository that ignores `vendor/`. It holds the folder `sub`, which is a workspace of its own.

| Step | Action                                                     | Expected result                                                                   |
|------|------------------------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Create a note `X` in the folder `../outside`.              | The system says that the folder lies outside the workspace.                       |
| 2    | Create a note `X` in the folder `.cache`.                  | The system says that the folder is hidden.                                        |
| 3    | Create a note `X` in the folder `vendor`.                  | The system says that git ignores the folder, so the note would not be listed.    |
| 4    | Create a note `X` in the folder `sub`.                     | The system says that the folder is a workspace of its own.                        |
| 5    | Create a note `X` in the folder `docs/CON`.                | The system says that the folder name is reserved by Windows.                      |
| 6    | Look at the directory and the folders above it.            | No file or folder was created.                                                    |

## TC-003-13: The note cannot be created

**Flow:** A5, NFR-006  
**Preconditions:** The user may not write into the workspace `readonly`, which is open. In the workspace `notes`, the folder `full` cannot be written because the disk is full.

| Step | Action                                                              | Expected result                                                                  |
|------|---------------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Create a note `X` in `readonly`.                                    | The status bar reports that the note could not be created because of missing permission; no tab opens. |
| 2    | In `notes`, create a note `Y` in the new folder `full/deep`.        | The status bar reports that the note could not be created; the folder `full/deep` does not exist. |
| 3    | Look at both workspaces.                                            | No note or part of a note was left behind.                                       |

## TC-003-14: Cancel at each point

**Flow:** A6  
**Preconditions:** The workspace `notes` holds `Plan.md`.

| Step | Action                                                                 | Expected result                           |
|------|------------------------------------------------------------------------|-------------------------------------------|
| 1    | Ask to create a note, type `Draft`, and cancel.                        | No note `Draft` exists.                   |
| 2    | Ask again, enter `a:b`, confirm, and cancel after the message.         | Nothing was created.                      |
| 3    | Ask again, enter `Plan`, confirm, and cancel after the message.        | `Plan.md` is unchanged.                   |
| 4    | Ask again, enter the folder `.cache`, confirm, and cancel after the message. | No folder `.cache` exists.          |

## TC-003-15: Written at once and remembered after a crash

**Flow:** BR-007, FR-071  
**Preconditions:** The workspace `notes` is open.

| Step | Action                                                        | Expected result                                                         |
|------|---------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Create the note `Crash Test`.                                 | `Crash-Test.md` exists on the disk at once.                             |
| 2    | Wait 1 second and end Underline by force, without closing it. | —                                                                       |
| 3    | Start Underline without naming a directory.                   | `notes` opens with `Crash Test` in the active tab, listed first in the note overview. |

## TC-003-16: A new note in a git repository

**Flow:** BR-007  
**Preconditions:** The workspace `wiki` is a git repository with no uncommitted changes.

| Step | Action                                  | Expected result                                                       |
|------|-----------------------------------------|-----------------------------------------------------------------------|
| 1    | Create the note `New Page`.             | `New-Page.md` is created.                                             |
| 2    | Look at the git state of `wiki`.        | `New-Page.md` is the only untracked file; no other file changed.      |
