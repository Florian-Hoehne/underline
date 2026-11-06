# ADR-002: Offer the MCP server over the stdio transport

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** NFR-016, NFR-015, NFR-008

## Context

Only an agent the user runs on the same machine may reach the MCP server (NFR-016), and note content leaves the
machine only through it or git (NFR-015). Changes the agent writes must appear in the open tab within 1 second
(NFR-008). The existing code already serves a workspace with `underline mcp <workspace dir>`.

## Decision

- The MCP server implements the Model Context Protocol over the stdio transport: JSON-RPC 2.0, one message per line
  on stdin and stdout. It opens no network port.
- An agent's tool starts it as `underline mcp <workspace dir>`; MCP install writes exactly this command into the
  tool's configuration.
- The server reads and writes notes only through the facade engine (ADR-003), so the agent's edits keep the note
  format.
- The running app watches the workspace files and reloads a changed note into its tab, which also carries the
  agent's edits and the agent cursor position.

## Consequences

- No authentication or port management is needed; the operating system's process boundary protects the notes.
- An agent on another machine cannot connect; a remote transport would need a new ADR.
- The MCP server process and the window are separate processes, so the app learns of agent edits from file changes
  and needs a small channel (for example a file in the user's cache directory) for the agent cursor.
- **Alternatives considered:** Streamable HTTP on localhost would let the window host the server directly but opens a
  port that NFR-016 rules out.
