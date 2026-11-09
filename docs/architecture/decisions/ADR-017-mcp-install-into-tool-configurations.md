# ADR-017: MCP install edits each agent tool's user configuration file directly

**Status:** Accepted
**Date:** 2026-11-08
**Requirements:** FR-060, FR-061, NFR-006, NFR-013, NFR-016, UC-016 BR-001 to BR-006

## Context

UC-016 adds one entry per workspace to the user configuration of six agent tools, removes it again, and shows per tool
whether it is found, installed, or installed for another place. Every other setting of the configuration must stay
as it was, and a broken or unwritable configuration must stay untouched. The six tools keep their user-level MCP
configuration in different files and formats:

| Tool               | File (user level)                                                                 | Format, key                         |
|--------------------|-----------------------------------------------------------------------------------|-------------------------------------|
| Claude Code        | `~/.claude.json`                                                                  | JSON, top-level `mcpServers`        |
| Claude Desktop     | macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`, Linux `~/.config/Claude/claude_desktop_config.json` | JSON, `mcpServers` |
| Cursor             | `~/.cursor/mcp.json` (`%USERPROFILE%\.cursor\mcp.json`)                           | JSON, `mcpServers`                  |
| VS Code (Copilot)  | the user profile's `mcp.json`: Linux `~/.config/Code/User/`, macOS `~/Library/Application Support/Code/User/`, Windows `%APPDATA%\Code\User\` | JSON, `servers` with `"type": "stdio"` |
| Codex              | `~/.codex/config.toml` (or `$CODEX_HOME/config.toml`)                             | TOML, table `[mcp_servers.<name>]`  |
| Gemini CLI         | `~/.gemini/settings.json`                                                         | JSON, `mcpServers`                  |

Some tools also offer a command (`claude mcp add`, `codex mcp add`, `gemini mcp add`, `code --add-mcp`), but not all,
not for removing in every case, and not with output meant to be read back.

## Decision

- **One mechanism: edit the file.** For every tool, Underline reads the user configuration file, changes only its own
  entry under the tool's key, and writes the file atomically (ADR-005). The tools' commands are not used, so install,
  remove, and the state shown work the same for all six and can be tested without the tools installed.
- **The entry** is named `underline-<folder name>` (`-2`, `-3`, … when taken by another workspace) and holds the full
  path of the running Underline program as `command` and `["mcp", "<workspace folder>"]` as `args`; for VS Code also
  `"type": "stdio"`. Two entries that hold the same command and folder are the same; the same name with another
  command or folder is "installed for another place" (UC-016 A2).
- **JSON** is read and written with `serde_json` with its `preserve_order` feature, so the other entries keep their
  order and content. A file holding comments or anything else `serde_json` cannot read is not written (UC-016 A6);
  JSON with comments, as VS Code allows, is refused with that reason rather than silently stripped.
- **TOML** (Codex) is edited with `toml_edit` (MIT or Apache-2.0), which keeps comments and formatting of everything
  else.
- **Found** means the tool's configuration file or its folder exists, or its program is on the `PATH`.
- **Copy configuration** (FR-061) puts the common `mcpServers` JSON form for the open workspace on the clipboard
  (ADR-014) and shows the command line.

## Consequences

- Install and remove touch exactly one entry; a broken file stays as it was.
- When a tool moves its configuration in a later version, the table above changes; each tool's place is one function
  with its own test.
- VS Code profiles other than the default and the Insiders build are not covered; their users use "Copy
  configuration".
- **Alternatives considered:** the tools' own commands where they exist (four tools, uneven support for removing and
  reading back, slower to test); a project file in the workspace (shared through git, ruled out by UC-016 BR-002).
