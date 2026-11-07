# Use Case: Install MCP Server

## Overview

**Use Case ID:** UC-016  
**Use Case Name:** Install MCP Server  
**Primary Actor:** User  
**Secondary Actor:** Agent tool (Claude Code, Claude Desktop, Cursor, VS Code with Copilot, Codex, Gemini CLI)  
**Goal:** The user's agent tool knows how to reach the notes of the open workspace through Underline's MCP server, without the user editing any configuration by hand.  
**Trigger:** The user opens the section "Agents" of the settings.  
**Status:** Draft  

**Requirements:** [FR-059, FR-060, FR-061, NFR-006, NFR-013, NFR-015, NFR-016](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user opens the section "Agents" of the settings (UC-010).
2. The system lists the agent tools Underline can install into: Claude Code, Claude Desktop, Cursor, VS Code (Copilot), Codex, and Gemini CLI, each with whether it was found on this computer and whether the open workspace is installed in it.
3. The user chooses "Install" for a tool that was found.
4. The system adds an entry for the open workspace to that tool's configuration for the user, keeping everything else the configuration holds.
5. The system reports that the workspace is installed in the tool, and that the tool may have to be restarted to see it.

## Alternative Flows

### A1: MCP server switched off

**Trigger:** The MCP server is switched off in the settings (UC-010 BR-008) (step 3)  
**Flow:**

1. The system says that an agent reaches the notes only while the MCP server is on, and asks whether to switch it on for the user.
2. When the user agrees, the system switches it on (UC-017) and continues at step 4; otherwise it continues at step 4 and the report says the server is off.

### A2: Already installed

**Trigger:** The tool's configuration holds an entry of the same name (step 4)  
**Flow:**

1. When the entry already serves this workspace with this Underline, the system changes nothing and says it is installed.
2. When it serves another place or another Underline, for example after Underline moved, the system asks to replace it; when the user agrees, it replaces only that entry.
3. Use case continues at step 5.

### A3: Remove

**Trigger:** The user chooses "Remove" for a tool in which the workspace is installed (step 3)  
**Flow:**

1. The system removes only the entry of this workspace from the tool's configuration, keeping everything else.
2. The system reports it.
3. Use case ends.

### A4: Copy the configuration

**Trigger:** The user chooses "Copy configuration" to connect another agent tool by hand (step 3)  
**Flow:**

1. The system puts on the clipboard a ready configuration for the open workspace in the form most agent tools take, and shows the command it holds, for tools that ask for a command.
2. The system reports that it was copied.
3. Use case ends.

### A5: Tool not found

**Trigger:** A tool was not found on this computer (step 2)  
**Flow:**

1. The list shows it as not found, and "Install" cannot be chosen for it; "Copy configuration" stays possible.
2. Use case continues at step 3.

### A6: Configuration cannot be read or written

**Trigger:** The tool's configuration holds something that cannot be read, or cannot be written, for example for missing permission (step 4, A3 step 1)  
**Flow:**

1. The system changes nothing and says why, naming the configuration's place, so the user can fix it or use A4.
2. Use case ends.

## Postconditions

### Success Postconditions

- The tool's configuration holds one entry for the open workspace, which starts Underline's MCP server for it; everything else in the configuration is as it was.
- Nothing is written into the workspace, so nothing of it is shared through git.

### Failure Postconditions

- The tool's configuration is as it was.

## Business Rules

### BR-001: One entry per workspace

Each workspace gets an entry of its own, named `underline-` and the workspace's folder name, such as `underline-notes`; a second workspace of the same folder name gets the next free number (`underline-notes-2`). An agent so always knows which notes it reaches.

### BR-002: The user's configuration

The entry goes into the tool's configuration for the user, never into a file of the workspace, so it is not shared through git. Every other entry and setting of that configuration stays as it was, and the configuration is written whole or not at all (NFR-006).

### BR-003: Which tools

The list holds the tools FR-060 names. A tool counts as found when its configuration for the user or the tool itself is found on this computer, on each operating system where the tool runs (NFR-013).

### BR-004: The entry starts the local server

The entry starts Underline's MCP server on this computer for the workspace's folder; it opens no network connection (NFR-016). It holds no secret, since none is needed.

### BR-005: The MCP server stays the user's choice

Installing never switches the MCP server on without asking (A1); whether an agent reaches the notes is decided by the switch of UC-017 at every moment, whatever is installed.

### BR-006: State shown

For each tool, the list shows "not found", "not installed", "installed", or "installed for another place" (A2), checked again whenever the section is opened.
