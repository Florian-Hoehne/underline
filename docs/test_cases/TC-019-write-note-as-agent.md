# Test Cases: Write Note as Agent

**Use Case:** [UC-019 Write Note as Agent](../use_cases/UC-019-write-note-as-agent.md)  
**Requirements:** [FR-056, FR-057, FR-058, NFR-006, NFR-008, NFR-015, NFR-016](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements            |
|-----------|----------------------------------------------------|-------------------------|
| TC-019-01 | Main success scenario: replace a passage           | FR-056, FR-057, NFR-008 |
| TC-019-02 | Insert before a line, at the end, replace all      | FR-056, BR-003          |
| TC-019-03 | A1 Create a note                                    | FR-056, BR-010          |
| TC-019-04 | A2 Note exists, A3 Title not allowed                | FR-056                  |
| TC-019-05 | A4 Passage not found or not unique                  | BR-003                  |
| TC-019-06 | A5 The user changed the same place                  | BR-004                  |
| TC-019-07 | A6 The user changed elsewhere                       | BR-004                  |
| TC-019-08 | A7 Note in conflict                                 | BR-005                  |
| TC-019-09 | A8 Read only or off                                 | UC-017 BR-002           |
| TC-019-10 | A9 Note cannot be written, A10 No such note         | NFR-006                 |
| TC-019-11 | A11 Undo the agent's change                         | BR-007                  |
| TC-019-12 | Format and line breaks                              | BR-001                  |
| TC-019-13 | Only notes' text                                    | BR-002                  |
| TC-019-14 | The agent cursor while writing                      | BR-006, FR-057, FR-058  |
| TC-019-15 | Without the window                                  | BR-004, UC-017 BR-007   |
| TC-019-16 | Changes to notes not open                           | BR-010                  |

## TC-019-01: Replace a passage

**Flow:** Main success scenario  
**Preconditions:** Access is read and write; `Plan.md` holds "# Plan", "- draft", "- ship"; "Plan" is open in a tab, the user's cursor in line 3; Claude Code is connected.

| Step | Action                                                       | Expected result                                                                    |
|------|--------------------------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Let Claude Code replace "- draft" with "- final draft".       | The answer says Plan changed in line 2; the file holds "- final draft".           |
| 2    | Look at the tab within 1 second.                              | Line 2 reads "- final draft", marked by the agent cursor "Claude Code"; the user's cursor is still in line 3; no scrolling. |

## TC-019-02: Insert and replace all

**Flow:** Main success scenario, BR-003  
**Preconditions:** `Plan.md` holds three lines.

| Step | Action                                            | Expected result                                  |
|------|---------------------------------------------------|--------------------------------------------------|
| 1    | Insert "- review" before line 3.                   | It is line 3; the old line 3 is line 4.          |
| 2    | Insert "- done" before line 99.                    | It is the last line.                             |
| 3    | Replace the whole text with "# Plan\n".            | The note holds exactly that.                     |

## TC-019-03: Create a note

**Flow:** A1, BR-010  
**Preconditions:** A Markdown workspace; no note "Ideas".

| Step | Action                                                    | Expected result                                                                    |
|------|-----------------------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Create "Projects/Ideas" with "# Ideas".                    | `Projects/Ideas.md` with that text; the answer names "Projects/Ideas".             |
| 2    | Look at the window.                                        | Within 1 s the overview lists it; no tab opens; the status bar says "Claude Code created Projects/Ideas" with "Open". |
| 3    | Create "Notes" as AsciiDoc.                                | `Notes.adoc`.                                                                      |

## TC-019-04: Note exists, title not allowed

**Flow:** A2, A3  
**Preconditions:** "Plan" exists.

| Step | Action                         | Expected result                                       |
|------|--------------------------------|-------------------------------------------------------|
| 1    | Create "Plan".                  | Refused, the note exists; Plan is unchanged.          |
| 2    | Create "A:B" and ".hidden/X".   | Each refused with the rule broken; nothing created.   |

## TC-019-05: Passage not found or not unique

**Flow:** A4, BR-003  
**Preconditions:** `Plan.md` holds "- todo" twice and "- ship" once.

| Step | Action                                | Expected result                                      |
|------|---------------------------------------|------------------------------------------------------|
| 1    | Replace "- zebra".                     | Refused: not found; nothing changes.                 |
| 2    | Replace "- todo".                      | Refused: it occurs 2 times; nothing changes.         |
| 3    | Replace "- SHIP".                      | Refused: not found (exact, case counts).             |

## TC-019-06: The user changed the same place

**Flow:** A5, BR-004  
**Preconditions:** "Plan" is open; the user types in line 2 and the change is not saved yet.

| Step | Action                                              | Expected result                                                  |
|------|-----------------------------------------------------|------------------------------------------------------------------|
| 1    | Let the agent replace a passage in line 2.           | Refused: the user is editing that place; the tab and file unchanged by the agent. |
| 2    | Let the agent insert before line 3.                  | Refused (right after a changed line).                           |
| 3    | Let the agent replace the whole text.                | Refused.                                                        |
| 4    | Wait for the autosave, read again, and retry step 1. | The change is made.                                             |

## TC-019-07: The user changed elsewhere

**Flow:** A6, BR-004  
**Preconditions:** "Plan" with 10 lines is open; the user typed in line 2, unsaved.

| Step | Action                                         | Expected result                                                              |
|------|------------------------------------------------|------------------------------------------------------------------------------|
| 1    | Let the agent replace a passage in line 8.      | The tab shows both changes; the file holds both; the user's cursor stays.    |

## TC-019-08: Note in conflict

**Flow:** A7, BR-005  
**Preconditions:** `Plan.md` holds a place in conflict.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Let the agent change Plan.      | Refused: resolve the conflicts first; nothing changes.                  |
| 2    | Let it read Plan.               | It reads it (UC-018 A8).                                                |
| 3    | Resolve the conflict, retry.    | The change is made.                                                     |

## TC-019-09: Read only or off

**Flow:** A8  
**Preconditions:** Access is read only, then off.

| Step | Action                          | Expected result                                      |
|------|---------------------------------|------------------------------------------------------|
| 1    | Change and create a note.        | Refused: Underline allows reading only.              |
| 2    | Switch off; change again.        | Refused: access is switched off.                     |

## TC-019-10: Not writable, no such note

**Flow:** A9, A10, NFR-006  
**Preconditions:** `Plan.md` is not writable; the workspace holds `Work/Todo.md` and `Home/Todo.md`.

| Step | Action                     | Expected result                                          |
|------|----------------------------|----------------------------------------------------------|
| 1    | Change Plan.                | Refused with the reason; Plan as it was.                 |
| 2    | Change "Plann".             | No such note; Plan named as closest.                     |
| 3    | Change "Todo".              | Both notes named; nothing changes.                       |

## TC-019-11: Undo the agent's change

**Flow:** A11, BR-007  
**Preconditions:** "Plan" is open; the user typed "x" in line 1 and it was saved.

| Step | Action                                    | Expected result                                                           |
|------|-------------------------------------------|---------------------------------------------------------------------------|
| 1    | Let the agent replace a passage in line 3. | The change shows.                                                         |
| 2    | Press Ctrl+Z once.                         | Only the agent's change is taken back; "x" stays; within 1 s it is saved. |
| 3    | Let the agent read Plan.                   | The text without the agent's change.                                      |

## TC-019-12: Format and line breaks

**Flow:** BR-001  
**Preconditions:** `Old.adoc` with Windows line breaks.

| Step | Action                                              | Expected result                                               |
|------|-----------------------------------------------------|---------------------------------------------------------------|
| 1    | Insert "* item\none more" at the end of Old.          | Stored as given, in AsciiDoc, with Windows line breaks throughout. |

## TC-019-13: Only notes' text

**Flow:** BR-002  
**Preconditions:** An agent is connected.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Look at the tools Underline offers.             | None renames, moves, or deletes notes or changes settings. |

## TC-019-14: The agent cursor while writing

**Flow:** BR-006, FR-057, FR-058  
**Preconditions:** "Plan" is open; access read and write.

| Step | Action                                   | Expected result                                      |
|------|------------------------------------------|------------------------------------------------------|
| 1    | Let the agent change line 2.              | The agent cursor marks line 2 with its name.         |
| 2    | Wait 10 seconds.                          | It is hidden.                                        |
| 3    | Change again, then quit the agent tool.   | Within 5 s it is hidden.                             |

## TC-019-15: Without the window

**Flow:** BR-004, UC-017 BR-007  
**Preconditions:** Underline is closed; access read and write.

| Step | Action                          | Expected result                                 |
|------|---------------------------------|-------------------------------------------------|
| 1    | Let the agent change Plan.       | The file holds the change.                      |
| 2    | Open Underline and Plan.         | The tab shows it.                               |

## TC-019-16: Changes to notes not open

**Flow:** BR-010  
**Preconditions:** "Plan" and "Ideas" are not open.

| Step | Action                                     | Expected result                                                         |
|------|--------------------------------------------|-------------------------------------------------------------------------|
| 1    | Let the agent change Plan.                  | "Claude Code changed Plan" with "Open"; Plan is among the changes (UC-011). |
| 2    | Let it change Plan and Ideas in a row.      | One report naming both.                                                |
| 3    | Choose "Open".                              | The note opens in a tab.                                               |
