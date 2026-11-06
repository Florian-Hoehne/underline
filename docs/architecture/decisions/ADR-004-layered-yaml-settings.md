# ADR-004: Layered YAML settings in underline.yaml

**Status:** Proposed
**Date:** 2026-11-06
**Requirements:** C-006, NFR-009

## Context

Settings live in `underline.yaml`: the user settings file in the home directory and the workspace settings file in
the workspace root, which overrides it (C-006). A changed setting must be written within 1 second (NFR-009).

## Decision

- Both files are YAML, read with serde into one settings struct; every key is optional.
- The effective settings are: built-in defaults, overlaid by the user settings file, overlaid by the workspace
  settings file, key by key.
- A change in the interface is written at once, atomically (ADR-005), to the file that holds the key (see OQ-004).
- Unknown keys are kept when a file is rewritten.

## Consequences

- Users can edit both files by hand and keep the workspace file in git.
- Rewriting a YAML file loses comments unless a comment-preserving writer is used.
