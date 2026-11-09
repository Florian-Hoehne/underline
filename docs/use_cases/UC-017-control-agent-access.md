# Use Case: Control Agent Access

## Overview

**Use Case ID:** UC-017  
**Use Case Name:** Control Agent Access  
**Primary Actor:** User  
**Secondary Actor:** Agent  
**Goal:** An agent reaches the user's notes only as far as the user allows it, off, read only, or read and write, and the user sees at any time what is allowed and which agent is connected.  
**Trigger:** The user changes agent access, or an agent connects or disconnects.  
**Status:** Draft  

**Requirements:** [FR-057, FR-058, FR-059, NFR-015, NFR-016](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002) for the status bar; the switch itself is a setting (UC-010) and holds whether Underline runs or not.

## Main Success Scenario

1. The system shows in the status bar what agents may do with the notes of the workspace: "Agents: off", "Agents: read only", or "Agents: read and write", and, while agents are connected, their names.
2. The user chooses "Read and write" or "Read only", in the menu of that entry in the status bar or with the agent access setting (UC-010).
3. The system keeps the choice in the user settings and shows it.
4. An agent tool starts Underline's MCP server for the workspace (UC-016) and connects.
5. The system shows "Agent connected:" with the name the tool gives, and the agent reads and writes notes (UC-018, UC-019).
6. The user chooses "Off".
7. From the agent's next request on, within one second, the MCP server refuses every request about notes with the reason that access is switched off in Underline; the system hides the agent cursor (FR-058) and shows "Agents: off".

## Alternative Flows

### A1: Switched off in the workspace

**Trigger:** The workspace settings switch the MCP server off (UC-010 BR-008) (step 2)  
**Flow:**

1. The status bar shows "Agents: off in this workspace" (or "read only in this workspace" when the workspace lowers it so); its menu offers only what the workspace allows and opens the settings of the workspace (UC-010) for more.
2. Use case ends.

### A2: Agent connects while access is off

**Trigger:** An agent tool starts the MCP server while access is off (step 4)  
**Flow:**

1. The MCP server answers the agent, but refuses every request about notes with the reason, so the agent can tell its user.
2. The status bar shows "Agents: off" and, while the agent stays, that an agent asked for access.
3. When the user switches access on, the agent's next request is served.
4. Use case continues at step 5.

### A3: Agent disconnects

**Trigger:** The agent tool ends the MCP server, for example when it is closed (step 5)  
**Flow:**

1. Within a few seconds, the status bar no longer names the agent and shows "Agents: on"; its cursor is hidden (FR-058).
2. Use case continues at step 4 whenever an agent connects again.

### A4: Several agents

**Trigger:** More than one agent tool is connected to the workspace (step 5)  
**Flow:**

1. The status bar names them all, such as "Agents connected: Claude Code, Cursor".
2. Switching off (step 6) applies to all of them.
3. Use case continues at step 5.

### A5: Underline not running

**Trigger:** An agent tool starts the MCP server while the Underline window is closed (step 4)  
**Flow:**

1. The MCP server follows the setting as it is stored: it serves the notes when access is on and refuses them when it is off.
2. Use case ends.

### A6: Read only

**Trigger:** An agent asks to create or change a note while access is read only (step 5)  
**Flow:**

1. The MCP server refuses the change, with the reason that Underline allows reading only; listing and reading go on.
2. Use case continues at step 5.

## Postconditions

### Success Postconditions

- Agents reach the notes exactly while access is on; the setting holds the user's choice.
- The status bar shows the choice and the agents connected.

### Failure Postconditions

- When the choice cannot be stored (UC-010 A7), access stays as it was and the status bar says so.

## Business Rules

### BR-001: Off means no notes

While access is off, the MCP server lists, reads, and writes no note and tells the agent why; nothing of the notes leaves the computer through it (NFR-015). An agent still learns which tools Underline offers, so it can explain the refusal.

### BR-002: The choice is the user's

Agent access has three levels: off, read only (listing and reading notes), and read and write (also creating and changing notes, UC-019). Only the user settings raise it (UC-010 BR-008); a workspace can only lower it. The menu in the status bar changes the user setting, as "For me" in the settings does. It is off by default.

### BR-003: Taking effect

A change of the switch takes effect for running MCP servers from their next request, within one second, without restarting the agent tool; a request already being served finishes.

### BR-004: Connected agents

An agent counts as connected while an MCP server started by an agent tool runs for the workspace; it is named by the name the tool gives when it connects, or "an agent" when it gives none. A server that ends without saying so stops counting within five seconds.

### BR-005: The agent cursor

The agent cursor (FR-057, UC-019) shows only while access is read and write and that agent is connected (FR-058).

### BR-007: Without the window

The MCP server follows the stored choice also while the Underline window is closed (A5), so an agent can work with the notes when the user allowed it, whether Underline runs or not.

### BR-006: Local only

Whatever the switch, an agent reaches the notes only through the MCP server on this computer (NFR-016).
