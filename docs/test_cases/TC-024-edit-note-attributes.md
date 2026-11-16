# Test Cases: Edit Note Attributes

**Use Case:** [UC-024 Edit Note Attributes](../use_cases/UC-024-edit-note-attributes.md)  
**Requirements:** [FR-091, FR-093, NFR-005, NFR-006, NFR-007, NFR-011, C-003, C-004](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements            |
|-----------|------------------------------------------------------|-------------------------|
| TC-024-01 | Main success scenario in a Markdown note             | FR-093, BR-001, BR-002  |
| TC-024-02 | Main success scenario in an AsciiDoc note            | FR-093, BR-001, BR-004, C-004 |
| TC-024-03 | Only the note attribute changes                      | BR-003, NFR-007         |
| TC-024-04 | Kinds of values                                      | BR-002                  |
| TC-024-05 | A1 First note attribute, A2 last one removed         | BR-001                  |
| TC-024-06 | A3 Name not allowed or taken                         | BR-004                  |
| TC-024-07 | A4 Value does not fit its kind                       | BR-002                  |
| TC-024-08 | A5 Head of the note cannot be read                   | —                       |
| TC-024-09 | A6 Nested value                                      | BR-002                  |
| TC-024-10 | A7 Note changed outside                              | —                       |
| TC-024-11 | Tags as a note attribute                             | FR-091, BR-005          |
| TC-024-12 | Stored and undone like any edit                      | BR-006, NFR-005         |
| TC-024-13 | Keyboard alone                                       | BR-007, NFR-011         |

## TC-024-01: Change a note attribute of a Markdown note

**Flow:** Main success scenario, BR-001, BR-002  
**Preconditions:** `Plan.md` starts with a front matter holding `status: draft` and `due: 2026-12-01`, followed by `# Plan`; it is open in the facade.

| Step | Action                                                  | Expected result                                                                  |
|------|---------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Look at the note properties.                            | Below place, format, size, last change, and text statistics: "status" as text "draft" and "due" as the date 1 December 2026. |
| 2    | Look at the facade.                                     | The front matter is shown as a table at the start of the note, as GitHub shows it. |
| 3    | Change "status" to `final`.                             | The table in the facade shows "final" at once.                                  |
| 4    | Wait 2 seconds and look at `Plan.md`.                   | The front matter holds `status: final`; everything else is unchanged.           |

## TC-024-02: Change a note attribute of an AsciiDoc note

**Flow:** Main success scenario, BR-001, BR-004, C-004  
**Preconditions:** `Guide.adoc` holds `= Guide` and below it `:author: Ann`.

| Step | Action                                                    | Expected result                                                     |
|------|-----------------------------------------------------------|---------------------------------------------------------------------|
| 1    | Add the note attribute `Edition` with the value `2`.      | It is shown as "edition".                                           |
| 2    | Look at `Guide.adoc`.                                     | It holds `:author: Ann` and below it `:edition: 2`, below `= Guide`. |

## TC-024-03: Only the note attribute changes

**Flow:** BR-003, NFR-007  
**Preconditions:** The front matter of `Mixed.md` holds a comment line, `tags: [a, b]`, an empty line, and `title:  "Mixed"` with two spaces.

| Step | Action                                                         | Expected result                                                         |
|------|----------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Add the note attribute `owner` with the value `Ann`.           | —                                                                       |
| 2    | Compare `Mixed.md` with its version before.                    | Only the line `owner: Ann` after the last attribute is new; the comment, the empty line, and the spaces are unchanged. |

## TC-024-04: Kinds of values

**Flow:** BR-002  
**Preconditions:** `Kinds.md` has a front matter with `title: Kinds`, `tags: [a, b]`, `weight: 2.5`, `due: 2026-12-01`, `done: false`.

| Step | Action                         | Expected result                                                                     |
|------|--------------------------------|-------------------------------------------------------------------------------------|
| 1    | Look at the note properties.   | A text field, a field per tag, a number field, a date with a calendar, and a switch. |
| 2    | Switch "done" on.              | `Kinds.md` holds `done: true`.                                                     |

## TC-024-05: First note attribute, last one removed

**Flow:** A1, A2, BR-001  
**Preconditions:** `Plain.md` holds only `# Plain`; `Plain.adoc` holds only `= Plain`.

| Step | Action                                                   | Expected result                                                   |
|------|----------------------------------------------------------|-------------------------------------------------------------------|
| 1    | In "Plain" (Markdown), add `status` with `draft`.        | The note starts with a front matter holding `status: draft`, then `# Plain`. |
| 2    | Remove `status`.                                         | The front matter is gone; the note holds only `# Plain` again.    |
| 3    | In "Plain" (AsciiDoc), add `status` with `draft`, then remove it. | First `:status: draft` stands below `= Plain`; afterwards only `= Plain` is left. |

## TC-024-06: Name not allowed or taken

**Flow:** A3, BR-004  
**Preconditions:** `Plan.md` as in TC-024-01.

| Step | Action                                        | Expected result                                               |
|------|-----------------------------------------------|---------------------------------------------------------------|
| 1    | Add an attribute with the name `my status`.   | The system says the name holds a space; nothing is written.   |
| 2    | Add one named `status`.                       | The system says the note has that attribute already.          |
| 3    | Add one with an empty name.                   | The system says a name is needed.                             |

## TC-024-07: Value does not fit its kind

**Flow:** A4, BR-002  
**Preconditions:** `Kinds.md` as in TC-024-04.

| Step | Action                                  | Expected result                                                 |
|------|-----------------------------------------|-----------------------------------------------------------------|
| 1    | Type `soon` as the value of "weight".   | The system says a number is expected; the file is unchanged.   |
| 2    | Type `2026-13-01` as the value of "due". | The system says a date is expected; the file is unchanged.     |

## TC-024-08: Head of the note cannot be read

**Flow:** A5  
**Preconditions:** The front matter of `Broken.md` holds the line `just text` without a name.

| Step | Action                         | Expected result                                                                  |
|------|--------------------------------|----------------------------------------------------------------------------------|
| 1    | Look at the note properties.   | The front matter is shown as text with the line and the reason; there are no fields; an offer to show the source view. |
| 2    | Look at `Broken.md`.           | It is unchanged.                                                                 |

## TC-024-09: Nested value

**Flow:** A6, BR-002  
**Preconditions:** The front matter of `Nested.md` holds `author:` with `name: Ann` and `mail: ann@example.org` indented below it, and `status: draft`.

| Step | Action                         | Expected result                                                                   |
|------|--------------------------------|-----------------------------------------------------------------------------------|
| 1    | Look at the note properties.   | "author" is shown as its markup, marked as changeable only in the source view; "status" is a text field. |
| 2    | Change "status" to `final`.    | Only that line changes; "author" is unchanged.                                    |

## TC-024-10: Note changed outside

**Flow:** A7  
**Preconditions:** `Plan.md` as in TC-024-01; the user is typing `fin` into "status".

| Step | Action                                                         | Expected result                                                          |
|------|----------------------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Change "due" to `2027-01-15` with another program.              | The note properties show the new due date; "status" still holds `fin`.  |
| 2    | Complete `final` and confirm.                                  | `Plan.md` holds `status: final` and `due: 2027-01-15`.                  |

## TC-024-11: Tags as a note attribute

**Flow:** BR-005, FR-091  
**Preconditions:** `Trip.md` holds a front matter `tags: [travel]` and the text "see #greece".

| Step | Action                                         | Expected result                                                              |
|------|------------------------------------------------|------------------------------------------------------------------------------|
| 1    | Look at the tags in the tool panel.            | "travel" and "greece".                                                      |
| 2    | Add the item `ferry` to the attribute "tags".  | `Trip.md` holds `tags: [travel, ferry]`; the tags in the tool panel are travel, ferry, greece. |
| 3    | Look at the attribute "tags".                  | "greece" is not one of its items; it stays in the text.                     |

## TC-024-12: Stored and undone like any edit

**Flow:** BR-006, NFR-005  
**Preconditions:** `Plan.md` as in TC-024-01, open in the source view.

| Step | Action                                         | Expected result                                                 |
|------|------------------------------------------------|-----------------------------------------------------------------|
| 1    | Change "status" to `final` and wait 2 seconds. | The source view shows `status: final`; the note is stored.      |
| 2    | Press Ctrl+Z in the tab.                       | `status: draft` is back, in the source view and the note properties. |

## TC-024-13: Keyboard alone

**Flow:** BR-007, NFR-011  
**Preconditions:** `Plan.md` as in TC-024-01; the focus is in the note.

| Step | Action                                                                   | Expected result                         |
|------|--------------------------------------------------------------------------|-----------------------------------------|
| 1    | Move the focus to the note properties and with Tab to the value of "status"; type `final` and Enter. | `status: final` is stored. |
| 2    | Tab to the remove button of "due" and press Enter.                       | "due" is removed from the note.         |
