# Test Cases: Run Command

**Use Case:** [UC-023 Run Command](../use_cases/UC-023-run-command.md)  
**Requirements:** [FR-077, NFR-011, NFR-013, NFR-018](../requirements.md)

## Coverage

| Test case | Flow / rule                                     | Requirements           |
|-----------|-------------------------------------------------|------------------------|
| TC-023-01 | Main success scenario: find and run a command   | FR-077, BR-003, BR-005 |
| TC-023-02 | Matching and order                              | BR-002, BR-004         |
| TC-023-03 | Every command is offered                        | BR-001                 |
| TC-023-04 | A1 No command matches                           | —                      |
| TC-023-05 | A2 Command not available now                    | BR-002                 |
| TC-023-06 | A3 Command asks for a choice                    | BR-005                 |
| TC-023-07 | A4 Switch to go to note                         | BR-007                 |
| TC-023-08 | A5 User cancels                                 | —                      |
| TC-023-09 | In the Vim and Nano input modes                 | BR-007                 |
| TC-023-10 | Speed in a large workspace                      | BR-006, NFR-018        |
| TC-023-11 | Keys shown per platform                         | BR-003, NFR-013        |

## TC-023-01: Find and run a command

**Flow:** Main success scenario, BR-003, BR-005  
**Preconditions:** The workspace `notes` is open; "Plan" is open in the active tab with the focus in its text; the minimap is shown.

| Step | Action                                   | Expected result                                                                       |
|------|------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+P and type `mini`.      | The command palette lists the command that shows or hides the minimap, with its menu and its key, chosen. |
| 2    | Press Enter.                             | The palette closes; the minimap is hidden; the focus is back in the text of "Plan".   |
| 3    | Press F1 and type `export pdf`.          | The command to export the note to PDF is listed first.                               |
| 4    | Press Enter.                             | The file dialog of the export asks where to save "Plan" (UC-009).                    |

## TC-023-02: Matching and order

**Flow:** BR-002, BR-004  
**Preconditions:** No command was ever run from the palette.

| Step | Action                                             | Expected result                                                         |
|------|----------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Open the command palette and type `NOTE NEW`.      | The command to create a new note is listed; case and order do not matter. |
| 2    | Type `theme` instead.                              | Commands whose name starts with "Theme" come before those that only hold the word. |
| 3    | Run the command to show the minimap, then open the palette with an empty field. | That command is listed first.                       |
| 4    | Restart Underline and open the palette with an empty field. | The command to show the minimap is still listed first.          |
| 5    | Open another workspace and open the palette with an empty field. | The command to show the minimap is listed first there too.  |

## TC-023-03: Every command is offered

**Flow:** BR-001  
**Preconditions:** A workspace that is a git repository is open with a note in the active tab.

| Step | Action                                                                                | Expected result                                       |
|------|---------------------------------------------------------------------------------------|-------------------------------------------------------|
| 1    | For each command of the menus, the toolbar, the tool panel, and the context menus of the note overview, the tab, and the note, search its name. | Each one is found, under the same name as in its menu. |
| 2    | Search `dd`.                                                                          | No Vim command is listed.                             |

## TC-023-04: No command matches

**Flow:** A1  
**Preconditions:** —

| Step | Action                                | Expected result                                 |
|------|---------------------------------------|-------------------------------------------------|
| 1    | Open the palette and type `xyzzy`.    | The palette says no command matches.            |

## TC-023-05: Command not available now

**Flow:** A2, BR-002  
**Preconditions:** A workspace that is no git repository is open; no tab is open.

| Step | Action                                    | Expected result                                                                     |
|------|-------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Type `commit`.                            | The commit command is listed after the others, marked as not available.             |
| 2    | Choose it.                                | The palette stays open, nothing runs, and it says the workspace is no git repository. |
| 3    | Type `export`.                            | The export commands are marked as not available because no note is open.           |

## TC-023-06: Command asks for a choice

**Flow:** A3, BR-005  
**Preconditions:** The theme is Light.

| Step | Action                                    | Expected result                                                  |
|------|-------------------------------------------|------------------------------------------------------------------|
| 1    | Run the command to choose the theme.      | The field lists the themes, Light marked as current.             |
| 2    | Type `nord` and press Enter.              | The theme is Nord; the palette closes; the setting is stored (UC-010). |

## TC-023-07: Switch to go to note

**Flow:** A4, BR-007  
**Preconditions:** The workspace holds "Plan".

| Step | Action                                                  | Expected result                                       |
|------|---------------------------------------------------------|-------------------------------------------------------|
| 1    | Open the palette, type `plan`, then delete the `>`.     | The field is go to note holding `plan`; "Plan" is listed. |
| 2    | Type `>` at the start.                                  | The field is the command palette again.              |

## TC-023-08: Cancel

**Flow:** A5  
**Preconditions:** "Plan" is open with the cursor in line 3.

| Step | Action                                            | Expected result                                          |
|------|---------------------------------------------------|----------------------------------------------------------|
| 1    | Open the palette, type `mini`, and press Escape.  | Nothing runs; the cursor is in line 3 of "Plan".        |
| 2    | Open it again and click outside it.               | The same.                                                |

## TC-023-09: In the Vim and Nano input modes

**Flow:** BR-007  
**Preconditions:** The input mode is Vim, normal mode, in "Plan".

| Step | Action                              | Expected result                         |
|------|-------------------------------------|-----------------------------------------|
| 1    | Press Ctrl+Shift+P.                 | The command palette opens.              |
| 2    | Switch to Nano and press F1.        | The command palette opens.              |

## TC-023-10: Speed in a large workspace

**Flow:** BR-006, NFR-018  
**Preconditions:** A workspace of 10,000 notes.

| Step | Action                                                     | Expected result                                       |
|------|------------------------------------------------------------|-------------------------------------------------------|
| 1    | Type 20 different texts, measuring until the list follows. | Each list appears within 200 milliseconds of the key. |

## TC-023-11: Keys shown per platform

**Flow:** BR-003, NFR-013  
**Preconditions:** Underline runs on Linux and on macOS.

| Step | Action                                           | Expected result                                            |
|------|--------------------------------------------------|------------------------------------------------------------|
| 1    | On Linux, search the command to find in the note. | Its key shows `Ctrl+F`.                                   |
| 2    | On macOS, the same.                              | Its key shows `Cmd+F`.                                     |
