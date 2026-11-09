# ADR-019: Todos as one YAML file each in `.todos/`, never merged line by line

**Status:** Proposed
**Date:** 2026-11-09
**Requirements:** FR-062 to FR-068, NFR-006, UC-020 BR-002, BR-004, BR-006, BR-008, A10, A11, ADR-004, ADR-005, ADR-006, ADR-008

## Context

UC-020 keeps todos in the workspace, shared through git like the notes (BR-004). A sync joins them todo by todo:
todos changed on one side only are taken over, a todo changed on both sides is a conflict the user resolves field by
field (A10, BR-008), and a new todo whose number the remote took first gets the next free number (A11). The Underline
folder `.underline/` is ignored by git (ADR-008), so it cannot hold them. They must never be listed as notes
(UC-020 BR-004), and every change is stored whole within a second (BR-006, NFR-006).

## Decision

- **One file per todo** in the hidden folder `.todos/` of the workspace root: `.todos/12.yaml` for todo #12. Hidden
  folders are not notes (UC-002 BR-003), so no todo is shown as a note, also in a GitHub wiki.
- **YAML** through `serde_norway`, as the settings (ADR-004): `title`, `urgency`, `status`, `due`, `scheduled`
  (`YYYY-MM-DD`), `created`, `done` (RFC 3339 local time), and `links`, the notes as listed (`Projects/Plan`).
  Unknown keys are kept when the file is written again.
- **Written atomically** (ADR-005) on every change, by the window and by the MCP server alike; the window watches
  `.todos/` like the notes (ADR-011) and reads a changed file again (UC-020 A7).
- **No line merge.** Underline keeps `.todos/.gitattributes` with `*.yaml -merge`, so git never joins two versions of
  one todo line by line: a todo changed on both sides is always a conflict (A10), while todos changed on one side are
  different files and join without one (BR-008).
- **Conflicts.** After a sync stops, a conflicted `.todos/*.yaml` is a todo conflict, not a note conflict: the
  window reads this computer's and the remote's version from git (`git show :2:` and `:3:`) and shows them field by
  field; "Keep mine" or "Take remote" writes that version and marks the file resolved (`git add`). A todo deleted on
  one side and changed on the other is offered the same way. Aborting the sync is UC-015 A4 (ADR-006).
- **Numbers.** The next number is one more than the highest number of any todo file and of the numbers recorded as
  freed in `.todos/.numbers` (one line, the highest number ever given), so deleted numbers are not given again
  (BR-002). A conflict where both sides added the same file (A11) is solved by moving this computer's todo to the
  next free number and taking the remote's file; the status bar reports the new number.

## Consequences

- The changes list (UC-011) shows a changed todo as `.todos/12.yaml`; the section "Todos" names it by title.
- Many todos are many small files, which git handles well; a todo is read in microseconds.
- Older versions of Underline would list nothing of it, since the folder is hidden.
- **Alternatives considered:** one file for all todos (every concurrent change would conflict or merge line by line,
  against BR-008); todos inside the notes as Markdown task lists (no stable number, fields, or links per todo, and
  AsciiDoc has no common form); a database file (binary, not reviewable in git, always conflicting).
