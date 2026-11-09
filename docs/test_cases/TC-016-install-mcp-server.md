# Test Cases: Install MCP Server

**Use Case:** [UC-016 Install MCP Server](../use_cases/UC-016-install-mcp-server.md)  
**Requirements:** [FR-059, FR-060, FR-061, NFR-006, NFR-013, NFR-015, NFR-016](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements             |
|-----------|----------------------------------------------------|--------------------------|
| TC-016-01 | Main success scenario: install into Claude Code    | FR-060, BR-002, BR-004   |
| TC-016-02 | Every tool                                         | FR-060, BR-003, NFR-013  |
| TC-016-03 | One entry per workspace                            | BR-001                   |
| TC-016-04 | Other settings kept                                | BR-002, NFR-006          |
| TC-016-05 | A1 MCP server switched off                         | BR-005, FR-059           |
| TC-016-06 | A2 Already installed, or for another place         | BR-006                   |
| TC-016-07 | A3 Remove                                          | —                        |
| TC-016-08 | A4 Copy the configuration                          | FR-061                   |
| TC-016-09 | A5 Tool not found                                  | —                        |
| TC-016-10 | A6 Configuration cannot be read or written         | —                        |
| TC-016-11 | The agent reaches the notes                        | BR-004, NFR-016          |
| TC-016-12 | Only when asked; other workspaces untouched        | BR-007, BR-008           |

## TC-016-01: Install into Claude Code

**Flow:** Main success scenario, BR-002, BR-004  
**Preconditions:** The workspace `notes` is open; the MCP server is on; Claude Code is installed and has no entry for Underline; the workspace is a git repository with nothing to commit.

| Step | Action                                         | Expected result                                                                  |
|------|------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Open the settings and the section "Agents".     | Claude Code is listed as "not installed".                                       |
| 2    | Choose "Install" for Claude Code.               | The report says `notes` is installed in Claude Code and it may need a restart; the list shows "installed". |
| 3    | Run `claude mcp list` in a terminal.             | It lists `underline-notes`, starting Underline's MCP server for the workspace's folder. |
| 4    | Look at the workspace's changes (UC-011).        | Nothing changed in the workspace.                                               |

## TC-016-02: Every tool

**Flow:** FR-060, BR-003, NFR-013  
**Preconditions:** Claude Code, Claude Desktop, Cursor, VS Code with Copilot, Codex, and Gemini CLI are installed; run on Linux, Windows, and macOS where each tool runs.

| Step | Action                                       | Expected result                                                         |
|------|----------------------------------------------|-------------------------------------------------------------------------|
| 1    | Install the workspace into each tool.         | Each reports installed.                                                |
| 2    | Restart each tool and look at its MCP servers. | Each lists `underline-notes` and can start it.                         |

## TC-016-03: One entry per workspace

**Flow:** BR-001  
**Preconditions:** Two workspaces `~/a/notes` and `~/b/notes`; Cursor is installed.

| Step | Action                                     | Expected result                                                |
|------|--------------------------------------------|----------------------------------------------------------------|
| 1    | Open `~/a/notes` and install into Cursor.   | Cursor has `underline-notes` for `~/a/notes`.                 |
| 2    | Open `~/b/notes` and install into Cursor.   | Cursor also has `underline-notes-2` for `~/b/notes`.          |

## TC-016-04: Other settings kept

**Flow:** BR-002, NFR-006  
**Preconditions:** Cursor's configuration holds two other MCP servers and other settings; a copy is kept.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Install into Cursor.            | The configuration equals the copy plus the one entry.                  |
| 2    | Remove it again (A3).           | The configuration means the same as the copy.                          |

## TC-016-05: MCP server switched off

**Flow:** A1, BR-005, FR-059  
**Preconditions:** Agent access is off (UC-017).

| Step | Action                                       | Expected result                                                         |
|------|----------------------------------------------|-------------------------------------------------------------------------|
| 1    | Choose "Install" for Claude Code.             | Underline says agents reach notes only while agent access is on, and asks whether to allow reading, or reading and writing. |
| 2    | Choose "Install only".                        | It installs; the report says agent access is off; the setting stays off. |
| 3    | Remove, install again, and allow reading.     | It installs and agent access is read only in the user settings.        |

## TC-016-06: Already installed, or for another place

**Flow:** A2, BR-006  
**Preconditions:** Claude Code has `underline-notes` for this workspace.

| Step | Action                                                       | Expected result                                                      |
|------|--------------------------------------------------------------|----------------------------------------------------------------------|
| 1    | Choose "Install" again.                                       | Nothing changes; it says installed.                                 |
| 2    | Move the Underline program and open the section again.        | Claude Code shows "installed for another place".                     |
| 3    | Choose "Install" and agree to replace.                        | The entry starts the moved Underline; nothing else changed.         |

## TC-016-07: Remove

**Flow:** A3  
**Preconditions:** `underline-notes` is in Codex with another server next to it.

| Step | Action                         | Expected result                                                  |
|------|--------------------------------|------------------------------------------------------------------|
| 1    | Choose "Remove" for Codex.      | Codex no longer lists `underline-notes`; the other server stays. |

## TC-016-08: Copy the configuration

**Flow:** A4, FR-061  
**Preconditions:** The workspace `notes` is open.

| Step | Action                                  | Expected result                                                         |
|------|-----------------------------------------|-------------------------------------------------------------------------|
| 1    | Choose "Copy configuration".             | The report says it was copied; the command is shown.                  |
| 2    | Paste it into a generic MCP client's configuration and start the client. | The client reaches the workspace's notes.   |

## TC-016-09: Tool not found

**Flow:** A5  
**Preconditions:** Gemini CLI is not installed.

| Step | Action                    | Expected result                                                        |
|------|---------------------------|------------------------------------------------------------------------|
| 1    | Open the section "Agents". | Gemini CLI shows "not found"; "Install" cannot be chosen for it.      |

## TC-016-10: Configuration cannot be read or written

**Flow:** A6  
**Preconditions:** Cursor's configuration holds broken content; Claude Desktop's cannot be written.

| Step | Action                              | Expected result                                                      |
|------|-------------------------------------|----------------------------------------------------------------------|
| 1    | Install into Cursor.                 | Nothing changes; Underline says it cannot read it and names its place. |
| 2    | Install into Claude Desktop.         | Nothing changes; Underline says it cannot write it and names its place. |

## TC-016-11: The agent reaches the notes

**Flow:** BR-004, NFR-016  
**Preconditions:** Installed into Claude Code; the MCP server is on.

| Step | Action                                             | Expected result                                                   |
|------|----------------------------------------------------|-------------------------------------------------------------------|
| 1    | Ask Claude Code to list the notes of the workspace. | It lists them (UC-018).                                          |
| 2    | Look at the network ports Underline listens on.     | None.                                                            |

## TC-016-12: Only when asked; other workspaces untouched

**Flow:** BR-007, BR-008  
**Preconditions:** Claude Code is installed; Cursor has `underline-old` for a workspace that was deleted.

| Step | Action                                             | Expected result                                                    |
|------|----------------------------------------------------|--------------------------------------------------------------------|
| 1    | Open several workspaces for the first time.         | Underline never suggests installing on its own.                   |
| 2    | Install and remove `notes` in Cursor.               | `underline-old` is still there.                                   |
