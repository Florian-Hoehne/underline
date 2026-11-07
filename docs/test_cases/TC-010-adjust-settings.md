# Test Cases: Adjust Settings

**Use Case:** [UC-010 Adjust Settings](../use_cases/UC-010-adjust-settings.md)  
**Requirements:** [FR-022, FR-028, FR-029, FR-030, FR-032, FR-059, FR-069, FR-070, FR-071, NFR-009, NFR-010, NFR-011, NFR-013, NFR-015, NFR-016, C-006](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements                    |
|-----------|------------------------------------------------------|---------------------------------|
| TC-010-01 | Main success scenario: a theme for me                | FR-022, FR-069, NFR-009, BR-004 |
| TC-010-02 | Every setting and its built-in default               | BR-001, BR-002                  |
| TC-010-03 | Line numbers and their side, in the source view only | FR-028, FR-029, BR-001          |
| TC-010-04 | A1 A value for this workspace only                   | FR-070, BR-002, BR-003          |
| TC-010-05 | A2 Back to the user setting                          | FR-070, BR-003                  |
| TC-010-06 | A3 Quick switches are stored where they come from    | FR-030, FR-032, FR-071, BR-003  |
| TC-010-07 | A4 Panel widths kept after a restart                 | FR-071                          |
| TC-010-08 | A5 Settings file changed outside                     | C-006                           |
| TC-010-09 | A6 Settings file with an error                       | —                               |
| TC-010-10 | A7 Settings cannot be stored                         | —                               |
| TC-010-11 | A8 Default format of a workspace                     | BR-001, UC-001 BR-005           |
| TC-010-12 | A9 No workspace open                                 | BR-002                          |
| TC-010-13 | The file keeps what the user wrote                   | BR-005, NFR-009                 |
| TC-010-14 | Shared workspace settings are pointed out            | BR-007                          |
| TC-010-15 | The MCP server only with the user's consent          | BR-008, FR-059, NFR-015, NFR-016 |
| TC-010-16 | Keyboard alone                                       | BR-006, NFR-011                 |
| TC-010-17 | Every theme in contrast, on every platform           | NFR-010, NFR-013, BR-004        |

## TC-010-01: Choose a theme for me

**Flow:** Main success scenario, BR-004  
**Preconditions:** The workspace `notes` is open with `Diagrams` in one tab and `Code` in another; `Diagrams` holds a PlantUML diagram and `Code` a highlighted code block. The user settings file sets no theme. The workspace settings file sets no theme.

| Step | Action                                          | Expected result                                                                                |
|------|-------------------------------------------------|------------------------------------------------------------------------------------------------|
| 1    | Open the settings.                              | They show "For me" and "This workspace"; the theme is Light in both, "This workspace" marked as using the user setting. |
| 2    | Choose the theme Nord in "For me".              | The window, both tabs, the code, and the diagram turn into Nord colors at once, without a restart. |
| 3    | Within 1 second, look at the user settings file with another program. | It names the theme Nord.                                                |
| 4    | Close the settings and restart Underline.       | Underline starts in Nord.                                                                      |

## TC-010-02: Every setting and its built-in default

**Flow:** BR-001, BR-002  
**Preconditions:** There is no user settings file. The workspace `notes`, created in Underline as Markdown, is open.

| Step | Action                             | Expected result                                                                                       |
|------|------------------------------------|-------------------------------------------------------------------------------------------------------|
| 1    | Open the settings and look at "For me". | Theme Light, input mode Normal, line numbers shown on the left, minimap shown, copy on select on, git sync every 5 minutes, MCP server off, note overview 200 and tool panel 240 pixels wide. |
| 2    | Look at "This workspace".          | The default format is Markdown, the workspace's own; every other setting uses the user setting.       |
| 3    | Look at the home directory.        | No user settings file was created by opening the settings.                                           |

## TC-010-03: Line numbers in the source view only

**Flow:** BR-001, FR-028, FR-029  
**Preconditions:** `Plan.md` is open in the source view. Line numbers are shown on the left.

| Step | Action                                            | Expected result                                                   |
|------|---------------------------------------------------|-------------------------------------------------------------------|
| 1    | Choose the line number side right.                | The line numbers move to the right of the text at once.           |
| 2    | Hide the line numbers.                            | No line numbers are shown; the text uses their room.              |
| 3    | Switch the tab to the facade.                     | The facade shows no line numbers, whatever the setting.           |
| 4    | Show the line numbers again and switch back to the source view. | They are shown on the right.                       |

## TC-010-04: A value for this workspace only

**Flow:** A1, BR-002, BR-003  
**Preconditions:** The theme for me is Light. The workspaces `notes` and `project` exist; `notes` is open. Neither workspace settings file sets a theme.

| Step | Action                                                  | Expected result                                                                   |
|------|---------------------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | In "This workspace", choose the theme Dracula.          | `notes` turns Dracula; the theme is marked as the workspace's own; "For me" notes that this workspace uses its own theme. |
| 2    | Look at both settings files.                            | The workspace settings file of `notes` names Dracula; the user settings file is unchanged. |
| 3    | Open `project`.                                         | `project` is shown in Light.                                                      |
| 4    | Open `notes` again.                                     | `notes` is shown in Dracula.                                                      |
| 5    | In "For me", choose the theme Gruvbox while `notes` is open. | `notes` stays Dracula; the user settings file names Gruvbox.                 |

## TC-010-05: Back to the user setting

**Flow:** A2, BR-003  
**Preconditions:** As at the end of TC-010-04: `notes` uses its own theme Dracula; the theme for me is Gruvbox.

| Step | Action                                            | Expected result                                                         |
|------|---------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Choose "As for me" on the theme in "This workspace". | `notes` turns Gruvbox at once; the theme is marked as using the user setting. |
| 2    | Look at the workspace settings file of `notes`.   | It no longer names a theme; everything else in it is unchanged.         |

## TC-010-06: Quick switches are stored where they come from

**Flow:** A3, BR-003  
**Preconditions:** `notes` is open. Its workspace settings file sets the input mode Vim; the user settings file sets copy on select on and nothing else.

| Step | Action                                                | Expected result                                                               |
|------|-------------------------------------------------------|-------------------------------------------------------------------------------|
| 1    | Switch copy on select off in the toolbar.             | Within 1 second the user settings file says that copy on select is off.       |
| 2    | Switch the input mode to Nano in the toolbar.         | Within 1 second the workspace settings file of `notes` names Nano; the user settings file does not name an input mode. |
| 3    | Hide the minimap in the toolbar.                      | The minimap disappears in every tab; the user settings file says it is hidden. |
| 4    | Restart Underline.                                    | `notes` opens with copy on select off, Nano keys, and no minimap.             |

## TC-010-07: Panel widths kept after a restart

**Flow:** A4  
**Preconditions:** `notes` is open with the note overview and the tool panel at their default widths.

| Step | Action                                               | Expected result                                                  |
|------|------------------------------------------------------|------------------------------------------------------------------|
| 1    | Drag the edge of the note overview to make it wider and let go. | Within 1 second the user settings file holds the new width. |
| 2    | Drag the tool panel narrower and let go.             | Within 1 second the user settings file holds its new width.      |
| 3    | Restart Underline.                                   | Both panels have the widths they had.                            |

## TC-010-08: Settings file changed outside

**Flow:** A5  
**Preconditions:** `notes` is open in the theme Light; the settings are open.

| Step | Action                                                                      | Expected result                                                   |
|------|-----------------------------------------------------------------------------|-------------------------------------------------------------------|
| 1    | With another program, set the theme Solarized Dark in the user settings file. | Within 1 second the window turns Solarized Dark and the settings show it. |
| 2    | With another program, set the input mode Vim in the workspace settings file of `notes`. | Within 1 second the status bar shows the Vim normal mode; "This workspace" marks the input mode as its own. |

## TC-010-09: Settings file with an error

**Flow:** A6  
**Preconditions:** `notes` is open in the theme Nord, set in the user settings file, which also holds a comment.

| Step | Action                                                                  | Expected result                                                          |
|------|-------------------------------------------------------------------------|--------------------------------------------------------------------------|
| 1    | With another program, change the theme in the user settings file to `Purple`. | The window stays Nord; the status bar names the user settings file, the line of the theme, and that `Purple` is no theme. |
| 2    | Choose the theme Dracula in the settings.                               | The window turns Dracula; the status bar says that the change cannot be stored until the file is corrected. |
| 3    | Look at the user settings file.                                         | It still holds `Purple` and the comment, unchanged.                      |
| 4    | Correct the theme in the file to `Light`.                               | Within 1 second the window turns Light and the status bar no longer reports the error. |
| 5    | Damage the user settings file so it cannot be read at all, and restart Underline. | Underline starts with the built-in defaults and reports the file, line, and error. |

## TC-010-10: Settings cannot be stored

**Flow:** A7  
**Preconditions:** `notes` is open. The user may not write the user settings file.

| Step | Action                              | Expected result                                                                  |
|------|-------------------------------------|----------------------------------------------------------------------------------|
| 1    | Choose the theme Ubuntu.            | The window turns Ubuntu; the status bar says that the setting could not be stored and why. |
| 2    | Allow writing the file again and hide the minimap. | Within 1 second the file holds both the theme Ubuntu and the hidden minimap. |

## TC-010-11: Default format of a workspace

**Flow:** A8  
**Preconditions:** The workspace `wiki` was not created in Underline and has no workspace settings file; it holds three Markdown notes and one AsciiDoc note.

| Step | Action                                                    | Expected result                                                                 |
|------|-----------------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Open the settings and look at "This workspace".           | The default format shows Markdown, derived from the notes.                      |
| 2    | Choose AsciiDoc.                                          | A workspace settings file appears in `wiki` naming AsciiDoc; the notes are unchanged. |
| 3    | Create a new note (UC-003).                               | AsciiDoc is preselected.                                                        |
| 4    | Choose "Follow the notes".                                | The workspace settings file no longer names a default format; Markdown is shown as derived. |

## TC-010-12: No workspace open

**Flow:** A9  
**Preconditions:** Underline runs with no workspace open.

| Step | Action             | Expected result                                      |
|------|--------------------|------------------------------------------------------|
| 1    | Open the settings. | Only "For me" is shown.                              |
| 2    | Choose the theme Dark. | The window turns Dark; the user settings file names it. |

## TC-010-13: The file keeps what the user wrote

**Flow:** BR-005  
**Preconditions:** The workspace settings file of `notes`, kept in git, holds a comment, the default format, a setting Underline does not know, and the theme Nord, in that order.

| Step | Action                                         | Expected result                                                                          |
|------|------------------------------------------------|------------------------------------------------------------------------------------------|
| 1    | In "This workspace", choose the theme Dracula. | Within 1 second the file names Dracula.                                                  |
| 2    | Look at the git diff of the file.              | Exactly one line changed; the comment, the order, and the unknown setting are as before. |

## TC-010-14: Shared workspace settings are pointed out

**Flow:** BR-007  
**Preconditions:** `team` is a git repository whose workspace settings file is not ignored; `private` is a workspace that is no git repository.

| Step | Action                                       | Expected result                                                                   |
|------|----------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Open `team` and the settings.                | "This workspace" says that its values are shared with everyone who uses the repository. |
| 2    | Open `private` and the settings.             | "This workspace" shows no such note.                                              |

## TC-010-15: The MCP server only with the user's consent

**Flow:** BR-008  
**Preconditions:** The MCP server is off for me. The repository `cloned`, taken from someone else, has a workspace settings file that switches the MCP server on.

| Step | Action                                                   | Expected result                                                                     |
|------|----------------------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Open `cloned`.                                           | The MCP server stays off; the status bar says that the workspace settings ask for it and are not followed. |
| 2    | Look at "This workspace" in the settings.                | The MCP server can only be switched off there.                                      |
| 3    | Switch the MCP server on in "For me".                    | It is on for every workspace except one that switches it off.                       |
| 4    | In "This workspace" of `notes`, switch it off.           | It is off while `notes` is open and on in other workspaces.                         |

## TC-010-16: Keyboard alone

**Flow:** BR-006, NFR-011  
**Preconditions:** `notes` is open; the mouse is not used.

| Step | Action                                                              | Expected result                                               |
|------|---------------------------------------------------------------------|---------------------------------------------------------------|
| 1    | Press `Ctrl+,` (`Cmd+,` on macOS).                                  | The settings open with the first setting focused.             |
| 2    | Move to "This workspace", change the theme, and choose "As for me" with the keyboard. | Each change applies as with the mouse.        |
| 3    | Close the settings with the keyboard.                               | The settings close; the focus is back in the note.            |

## TC-010-17: Every theme in contrast, on every platform

**Flow:** BR-004, NFR-010, NFR-013  
**Preconditions:** A note with body text, a link, a code block, a diagram, and a table is open; the test runs on Linux, Windows, and macOS.

| Step | Action                                  | Expected result                                                                                         |
|------|-----------------------------------------|---------------------------------------------------------------------------------------------------------|
| 1    | Choose each of the eleven themes in turn. | Body text has a contrast of at least 4.5:1 and the controls of at least 3:1 in every theme; code and diagram follow the theme. |
| 2    | Repeat on the other two platforms.      | Every setting is offered and works the same.                                                            |
