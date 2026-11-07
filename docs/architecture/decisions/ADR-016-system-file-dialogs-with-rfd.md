# ADR-016: The system's file dialogs through rfd

**Status:** Accepted
**Date:** 2026-11-07
**Requirements:** NFR-011, NFR-013, UC-009 step 2, BR-007

## Context

UC-009 lets the user choose where to save an export with the file dialog of the system, which also asks before
replacing a file. egui has no file dialogs; Underline so far asks for directories in text fields.

## Decision

- Underline shows the system's open and save dialogs through `rfd` 0.17 (MIT): the native dialogs on Windows and
  macOS, and on Linux the dialog of the desktop through the XDG desktop portal.
- The dialog runs off the window's thread, so the window keeps drawing while it is open; its answer comes back to the
  app as a message.
- Where no dialog can be shown, such as on a Linux system without a desktop portal, Underline falls back to a text
  field for the path in its own window, and asks itself before replacing a file.

## Consequences

- Exports, and later the choice of a workspace directory, use the dialog the user knows from other programs.
- On Linux the portal must run, which every common desktop provides; without it the fallback keeps the function
  usable.
- **Alternatives considered:** a file browser of our own in egui (not the system's dialog, as UC-009 asks; more code);
  `native-dialog` (fewer platforms kept up).
