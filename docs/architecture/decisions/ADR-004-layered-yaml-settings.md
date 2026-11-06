# ADR-004: Layered YAML settings in underline.yaml

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** C-006, NFR-009, FR-070, FR-071

## Context

Settings live in `underline.yaml`: the user settings file in the home directory and the workspace settings file in
the workspace root, which overrides it (C-006, FR-070). A changed setting must be written within 1 second (NFR-009).
Users edit both files by hand and keep the workspace file in git.

## Decision

- Both files are YAML, read with serde (`serde_norway`, the maintained fork of `serde_yaml`) into one settings
  struct; every key is optional.
- The effective settings are: built-in defaults, overlaid by the user settings file, overlaid by the workspace
  settings file, key by key.
- A change in the interface goes to the workspace settings file when that file already sets the key, otherwise to the
  user settings file (entity model, SETTINGS). It is written at once and atomically (ADR-005).
- A change rewrites only the line of the changed key, or appends the key when the file does not set it yet. Comments,
  key order, and unknown keys stay as the user wrote them. A file whose layout this cannot handle (for example a key
  written as a flow mapping) is rewritten in full and the status bar says that its comments were dropped.

## Consequences

- Hand-edited files stay as written, and a workspace file in git shows one-line diffs for interface changes.
- The line-level editor is our own code and needs tests for nested keys, quoting, and multi-line values.
- **Alternatives considered:** rewriting the whole file loses comments; TOML with `toml_edit` would keep comments but
  contradicts C-006.
