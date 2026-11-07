# Use Case: Adjust Settings

## Overview

**Use Case ID:** UC-010  
**Use Case Name:** Adjust Settings  
**Primary Actor:** User  
**Goal:** Underline looks and behaves as the user wants it, in every workspace by default and differently in a workspace where they want that, and keeps it after a restart.  
**Trigger:** The user opens the settings, uses a quick switch of the toolbar, or changes the size of a panel.  
**Status:** Draft  

**Requirements:** [FR-022, FR-028, FR-029, FR-030, FR-032, FR-069, FR-070, FR-071, NFR-009, NFR-010, NFR-011, NFR-013, C-006](../requirements.md)

## Preconditions

- Underline is running. The settings of a workspace (A1, A2) can be changed only while a workspace is open.

## Main Success Scenario

1. The user opens the settings.
2. The system shows the settings in two parts: "For me", the user settings that apply in every workspace, and "This workspace", the settings of the open workspace. Each setting shows its value, and in "This workspace" whether the workspace has a value of its own or uses the user setting.
3. The user changes a setting in "For me".
4. The system applies the change at once to every open tab and to the whole window (BR-004), unless the open workspace has a value of its own for that setting, and stores it in the user settings file within 1 second (BR-005).
5. The user closes the settings; the changes stay.

## Alternative Flows

### A1: Change a setting for this workspace

**Trigger:** The user changes a setting in "This workspace" (step 3)  
**Flow:**

1. The system gives the workspace a value of its own for that setting, applies it at once, and stores it in the workspace settings file within 1 second; the setting is now marked as the workspace's own.
2. Use case continues at step 5.

### A2: Use the user setting again in this workspace

**Trigger:** The user chooses "As for me" on a setting that the workspace has a value of its own for (step 3)  
**Flow:**

1. The system removes the workspace's value from the workspace settings file, applies the user setting at once, and marks the setting as using the user setting.
2. Use case continues at step 5.

### A3: Quick switch in the toolbar

**Trigger:** The user switches the minimap, copy on select, or the input mode in the toolbar (step 1)  
**Flow:**

1. The system applies the change at once and stores it like a change in the settings: in the workspace settings file when the workspace has a value of its own for it, otherwise in the user settings file (BR-003).
2. Use case ends.

### A4: Panel resized

**Trigger:** The user drags the edge of the note overview or of the tool panel (step 1)  
**Flow:**

1. The system stores the new width like a quick switch (A3) once the user lets go, so the panel has that width after a restart.
2. Use case ends.

### A5: Settings file changed outside

**Trigger:** The user settings file or the workspace settings file is changed by another program, for example by hand or by git (any step)  
**Flow:**

1. The system reads the file again and applies its settings within 1 second, as if they had been changed in the settings; an open settings window shows the new values.
2. Use case continues where it was.

### A6: Settings file has an error

**Trigger:** A settings file that is read (at start, when a workspace is opened, or in A5) cannot be understood, or holds a value that is not allowed (any step)  
**Flow:**

1. The system keeps the settings it had before, or the built-in defaults when it had none, and says in the status bar which file and which line hold the error and what is wrong.
2. The system does not write that file until it can be read again, so the user's text is not lost; a change in the interface meanwhile applies until Underline is closed and the status bar says that it cannot be stored yet.
3. When the file is corrected, the system continues at A5.

### A7: Settings cannot be stored

**Trigger:** A settings file cannot be written, for example for missing permission or a full disk (step 4, A1 to A4)  
**Flow:**

1. The system applies the change until Underline is closed, says in the status bar that it could not be stored and why, and tries again with the next change.
2. Use case continues at step 5.

### A8: Change the default format

**Trigger:** The user changes the default format in "This workspace" (A1)  
**Flow:**

1. The system stores it in the workspace settings file; notes created afterwards get the new note format (UC-003). Existing notes keep their format and are not converted (UC-001 BR-005).
2. Use case continues at step 5.

### A9: No workspace open

**Trigger:** The user opens the settings while no workspace is open (step 1)  
**Flow:**

1. The system shows only "For me".
2. Use case continues at step 3.

### A10: User cancels

**Trigger:** The user closes the settings without a change (step 3)  
**Flow:**

1. Nothing is changed.
2. Use case ends.

## Postconditions

### Success Postconditions

- The changed setting is in effect in every open tab and in the whole window.
- It is stored in the user settings file or the workspace settings file within 1 second and is in effect again after a restart.
- Everything else in that file stays as the user wrote it (BR-005).

### Failure Postconditions

- When a settings file has an error (A6), it is left as it is and the settings in effect are the last ones that could be read.
- When a settings file cannot be written (A7), the change is in effect until Underline is closed.

## Business Rules

### BR-001: The settings

| Setting                | Values                                                                 | Built-in default |
|------------------------|------------------------------------------------------------------------|------------------|
| Theme                  | Light, Dark, GitHub Light, GitHub Dark, Solarized Light, Solarized Dark, Night Owl, Ubuntu, Nord, Dracula, Gruvbox | Light |
| Input mode             | Normal, Vim, Nano                                                      | Normal           |
| Line numbers           | Shown or hidden                                                        | Shown            |
| Line number side       | Left or right                                                          | Left             |
| Minimap                | Shown or hidden                                                        | Shown            |
| Copy on select         | On or off                                                              | On               |
| Git sync interval      | 1 to 60 minutes                                                        | 5 minutes        |
| MCP server             | On or off                                                              | Off              |
| Note overview width    | 100 to 1,000 pixels                                                    | 200 pixels       |
| Tool panel width       | 100 to 1,000 pixels                                                    | 240 pixels       |
| Default format         | Markdown or AsciiDoc; only for a workspace (UC-001 BR-005)             | —                |

What git sync and the MCP server do is described in UC-014 and UC-017; this use case only sets them.

### BR-002: Two levels

The user settings apply in every workspace. A workspace can give a setting a value of its own, which applies in that workspace only (FR-070); for every other setting it uses the user setting. The default format exists only for a workspace. A setting the user never set has its built-in default.

### BR-003: Where a change is stored

A change in "For me" is stored in the user settings file, a change in "This workspace" in the workspace settings file. A change made outside the settings window (a quick switch, A3, or a panel width, A4) is stored in the workspace settings file when the workspace has a value of its own for that setting, otherwise in the user settings file, so it applies where the user sees it come from.

### BR-004: In effect at once

A changed setting is in effect at once, without a restart, in every open tab and in both editing views. A new theme also recolors highlighted code and rendered diagrams (FR-038). Every theme reaches the contrast of NFR-010.

### BR-005: Stored at once, as written

A changed setting is written to its settings file within 1 second (NFR-009), never half written. Only that setting changes in the file; the user's comments, the order of the settings, and settings Underline does not know stay as they were, so a workspace settings file in git shows the change as one changed line. A workspace settings file appears only when the workspace gets a value of its own for the first time (UC-002 BR-001).

### BR-006: Keyboard

The settings can be opened and every setting changed by keyboard alone (NFR-011).
