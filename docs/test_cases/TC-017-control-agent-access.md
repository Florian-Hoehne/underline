# Test Cases: Control Agent Access

**Use Case:** [UC-017 Control Agent Access](../use_cases/UC-017-control-agent-access.md)  
**Requirements:** [FR-057, FR-058, FR-059, NFR-015, NFR-016](../requirements.md)

## Coverage

| Test case | Flow / rule                                       | Requirements          |
|-----------|---------------------------------------------------|-----------------------|
| TC-017-01 | Main success scenario: allow, connect, switch off | FR-059, BR-001        |
| TC-017-02 | The three levels in the status bar and settings   | BR-002                |
| TC-017-03 | Off by default; old settings understood           | BR-002                |
| TC-017-04 | Taking effect without restarting the tool         | BR-003                |
| TC-017-05 | A1 Lowered in the workspace                       | BR-002                |
| TC-017-06 | A2 Agent connects while access is off             | BR-001                |
| TC-017-07 | A3 Agent disconnects                              | BR-004, FR-058        |
| TC-017-08 | A4 Several agents                                 | BR-004                |
| TC-017-09 | A5 Underline not running                          | BR-007                |
| TC-017-10 | A6 Read only                                      | BR-002                |
| TC-017-11 | The agent cursor follows access                   | BR-005, FR-057, FR-058 |
| TC-017-12 | Local only                                        | BR-006, NFR-016       |

## TC-017-01: Allow, connect, switch off

**Flow:** Main success scenario, BR-001  
**Preconditions:** The workspace `notes` is open; agent access is off; Claude Code has `underline-notes` installed (UC-016).

| Step | Action                                               | Expected result                                                        |
|------|------------------------------------------------------|------------------------------------------------------------------------|
| 1    | Look at the status bar.                               | "Agents: off".                                                        |
| 2    | Choose "Read and write" in the menu of that entry.    | "Agents: read and write"; the user settings hold it.                  |
| 3    | Start Claude Code and ask it to list the notes.       | It lists them; the status bar shows "Agent connected: Claude Code".   |
| 4    | Choose "Off" and ask Claude Code to read "Plan".      | It answers that access is switched off in Underline; "Agents: off".   |

## TC-017-02: The three levels in the status bar and settings

**Flow:** BR-002  
**Preconditions:** A workspace is open.

| Step | Action                                         | Expected result                                                    |
|------|------------------------------------------------|--------------------------------------------------------------------|
| 1    | Choose "Read only" in the status bar menu.      | The settings window shows agent access "Read only" under "For me". |
| 2    | Choose "Read and write" in the settings.        | The status bar shows "Agents: read and write".                    |

## TC-017-03: Off by default; old settings understood

**Flow:** BR-002  
**Preconditions:** No user settings file; a second user settings file from an earlier version holds the MCP server switched on.

| Step | Action                                     | Expected result                                       |
|------|--------------------------------------------|-------------------------------------------------------|
| 1    | Start Underline without a settings file.    | "Agents: off".                                       |
| 2    | Start it with the earlier file.             | "Agents: read and write".                            |

## TC-017-04: Taking effect without restarting the tool

**Flow:** BR-003  
**Preconditions:** Claude Code is connected; access is read and write.

| Step | Action                                      | Expected result                                               |
|------|---------------------------------------------|---------------------------------------------------------------|
| 1    | Choose "Off" and within 1 s ask for a list.  | The list is refused, without restarting Claude Code.         |
| 2    | Choose "Read and write" and ask again.       | The list comes.                                              |

## TC-017-05: Lowered in the workspace

**Flow:** A1, BR-002  
**Preconditions:** The user settings allow read and write; the workspace settings set agent access to read only.

| Step | Action                                     | Expected result                                                          |
|------|--------------------------------------------|--------------------------------------------------------------------------|
| 1    | Look at the status bar and open its menu.   | "Agents: read only in this workspace"; "Read and write" is not offered; an entry opens the workspace settings. |
| 2    | Choose "Read only" for me, then set the workspace to read and write in its file. | The status bar says the workspace file is not followed; "Agents: read only". |

## TC-017-06: Agent connects while access is off

**Flow:** A2, BR-001  
**Preconditions:** Access is off.

| Step | Action                                         | Expected result                                                      |
|------|------------------------------------------------|----------------------------------------------------------------------|
| 1    | Start Claude Code and ask it to list the notes. | It answers access is switched off; the status bar says an agent asked for access. |
| 2    | Ask Claude Code which tools Underline offers.   | It names them.                                                      |
| 3    | Choose "Read and write" and ask again.          | The list comes.                                                     |

## TC-017-07: Agent disconnects

**Flow:** A3, BR-004, FR-058  
**Preconditions:** Claude Code is connected.

| Step | Action                    | Expected result                                                       |
|------|---------------------------|-----------------------------------------------------------------------|
| 1    | Quit Claude Code.          | Within 5 s, the status bar shows "Agents: read and write" without a name. |
| 2    | Kill its process instead.  | The same, within 5 s.                                                |

## TC-017-08: Several agents

**Flow:** A4, BR-004  
**Preconditions:** Access is read and write; Claude Code and Cursor are installed.

| Step | Action                              | Expected result                                                   |
|------|-------------------------------------|-------------------------------------------------------------------|
| 1    | Connect both.                        | "Agents connected: Claude Code, Cursor".                         |
| 2    | Choose "Off" and ask both to read.   | Both are refused.                                                |

## TC-017-09: Underline not running

**Flow:** A5, BR-007  
**Preconditions:** Underline is closed; access is read and write, then off.

| Step | Action                                     | Expected result                         |
|------|--------------------------------------------|-----------------------------------------|
| 1    | Ask Claude Code to read "Plan".             | It reads it.                            |
| 2    | Set access off in the settings file, ask again. | It is refused.                     |

## TC-017-10: Read only

**Flow:** A6, BR-002  
**Preconditions:** Access is read only; Claude Code is connected.

| Step | Action                                     | Expected result                                                         |
|------|--------------------------------------------|-------------------------------------------------------------------------|
| 1    | Ask Claude Code to list and read notes.     | It does.                                                               |
| 2    | Ask it to change "Plan" and create "New".   | Both are refused with the reason that Underline allows reading only; nothing changes. |

## TC-017-11: The agent cursor follows access

**Flow:** BR-005, FR-057, FR-058  
**Preconditions:** Claude Code is connected and writing in "Plan", which is open (UC-019).

| Step | Action                     | Expected result                         |
|------|----------------------------|-----------------------------------------|
| 1    | Look at the tab.            | The agent cursor shows.                 |
| 2    | Choose "Read only".         | The agent cursor of the writing is hidden; when Claude Code reads "Plan" next, it shows the lines read (UC-018). |
| 3    | Choose "Off".               | It is hidden.                           |

## TC-017-12: Local only

**Flow:** BR-006, NFR-016  
**Preconditions:** Access is read and write.

| Step | Action                                   | Expected result                         |
|------|------------------------------------------|-----------------------------------------|
| 1    | Look at the ports Underline listens on.   | None.                                  |
