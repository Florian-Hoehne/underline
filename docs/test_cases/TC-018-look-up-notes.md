# Test Cases: Look Up Notes

**Use Case:** [UC-018 Look Up Notes](../use_cases/UC-018-look-up-notes.md)  
**Requirements:** [FR-054, FR-055, FR-057, FR-058, NFR-003, NFR-015, NFR-016](../requirements.md)

## Coverage

| Test case | Flow / rule                                   | Requirements           |
|-----------|-----------------------------------------------|------------------------|
| TC-018-01 | Main success scenario: list, search, read     | FR-054, FR-055         |
| TC-018-02 | Which notes are listed                        | BR-001, BR-009, FR-054 |
| TC-018-03 | A1 Only some lines                            | FR-055, BR-009         |
| TC-018-04 | A2 No such note                               | BR-002                 |
| TC-018-05 | A3 Same title in several folders              | BR-002                 |
| TC-018-06 | A4 Nothing found, A5 Many found               | BR-004                 |
| TC-018-07 | A6 Access off                                 | UC-017 BR-001          |
| TC-018-08 | A7 Note cannot be read                        | FR-055                 |
| TC-018-09 | A8 Note in conflict                           | FR-055                 |
| TC-018-10 | A9 One folder only                            | FR-054                 |
| TC-018-11 | As stored, without the window                 | BR-003, UC-017 BR-007  |
| TC-018-12 | Only reading                                  | BR-005                 |
| TC-018-13 | The agent cursor while reading                | BR-006, FR-057, FR-058 |
| TC-018-14 | Speed                                         | BR-007, NFR-003        |

## TC-018-01: List, search, read

**Flow:** Main success scenario  
**Preconditions:** Agent access is read only; the workspace holds `Plan.md` ("# Plan", "- ship the release") and `Projects/Ideas.adoc`; Claude Code is connected.

| Step | Action                                              | Expected result                                                                    |
|------|-----------------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Ask Claude Code to list the notes.                   | Plan (Markdown) and Projects/Ideas (AsciiDoc), each with title, folder, and format. |
| 2    | Ask it to search for `release`.                      | Plan, with line 2 "- ship the release".                                           |
| 3    | Ask it to read "Plan".                               | The text as stored, the format Markdown, and 2 lines.                             |

## TC-018-02: Which notes are listed

**Flow:** BR-001, BR-009  
**Preconditions:** The workspace holds `A.md` changed yesterday, `B.md` changed today, a deleted note `Old`, `.hidden/Secret.md`, `picture.png`, and a folder `Other` with its own `underline.yaml`.

| Step | Action               | Expected result                                                           |
|------|----------------------|---------------------------------------------------------------------------|
| 1    | List the notes.       | B, then A; neither Old, Secret, picture.png, nor any note of `Other`.     |
| 2    | Search for `Secret`.  | Nothing found.                                                            |

## TC-018-03: Only some lines

**Flow:** A1, BR-009  
**Preconditions:** `Long.md` has 500 lines.

| Step | Action                       | Expected result                                                     |
|------|------------------------------|---------------------------------------------------------------------|
| 1    | Read lines 10 to 12 of Long.  | Exactly lines 10, 11, 12 with their numbers; the note has 500 lines. |
| 2    | Read lines 498 to 510.        | Lines 498 to 500, and that the note ends at line 500.              |

## TC-018-04: No such note

**Flow:** A2, BR-002  
**Preconditions:** The workspace holds Plan, Planning, and Release.

| Step | Action             | Expected result                                               |
|------|--------------------|---------------------------------------------------------------|
| 1    | Read "Plann".       | There is no such note; Plan and Planning are named as closest. |
| 2    | Read "plan".        | The note Plan; case does not matter.                          |

## TC-018-05: Same title in several folders

**Flow:** A3, BR-002  
**Preconditions:** The workspace holds `Work/Plan.md` and `Home/Plan.md`.

| Step | Action                  | Expected result                                          |
|------|-------------------------|----------------------------------------------------------|
| 1    | Read "Plan".             | Both notes with their folders; neither is read.          |
| 2    | Read "Work/Plan".        | The note in Work.                                        |

## TC-018-06: Nothing found, many found

**Flow:** A4, A5, BR-004  
**Preconditions:** 80 notes hold the word `todo`, one of them in 10 lines.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Search for `zebra`.             | No note matches.                                                       |
| 2    | Search for `todo`.              | 50 notes, each with at most 3 lines; 30 more notes and the further lines are counted. |
| 3    | Search for `"ship the"` and `SHIP`. | Exactly the sequence; case does not matter.                       |

## TC-018-07: Access off

**Flow:** A6, UC-017 BR-001  
**Preconditions:** Agent access is off.

| Step | Action                         | Expected result                                          |
|------|--------------------------------|----------------------------------------------------------|
| 1    | List, search, and read.         | Each is refused with the reason; no note text is given.  |

## TC-018-08: Note cannot be read

**Flow:** A7  
**Preconditions:** `Plan.md` cannot be read for missing permission.

| Step | Action                                  | Expected result                         |
|------|-----------------------------------------|-----------------------------------------|
| 1    | Read "Plan".                             | The reason is given; nothing changes.   |
| 2    | Delete `Ideas.md` outside, read "Ideas". | There is no such note.                  |

## TC-018-09: Note in conflict

**Flow:** A8  
**Preconditions:** `Plan.md` holds a place in conflict after a git sync (UC-015).

| Step | Action          | Expected result                                                           |
|------|-----------------|---------------------------------------------------------------------------|
| 1    | Read "Plan".     | The note with both versions of the place, and that it holds unresolved conflicts. |

## TC-018-10: One folder only

**Flow:** A9  
**Preconditions:** The workspace holds `Plan.md`, `Projects/Ideas.md`, `Projects/2026/Q1.md`.

| Step | Action                        | Expected result                       |
|------|-------------------------------|---------------------------------------|
| 1    | List the notes of Projects.    | Projects/Ideas and Projects/2026/Q1.  |
| 2    | List the notes of Nowhere.     | The folder does not exist.            |

## TC-018-11: As stored, without the window

**Flow:** BR-003, UC-017 BR-007  
**Preconditions:** "Plan" is open in a tab.

| Step | Action                                                     | Expected result                                   |
|------|------------------------------------------------------------|---------------------------------------------------|
| 1    | Type "new line" and at once ask the agent to read "Plan".   | The stored text, possibly without "new line".     |
| 2    | Wait 1 second and read again.                               | The text with "new line".                         |
| 3    | Close Underline and read again.                             | The same text.                                    |

## TC-018-12: Only reading

**Flow:** BR-005  
**Preconditions:** The note overview lists Plan, then Ideas, as recently opened.

| Step | Action                                  | Expected result                                               |
|------|-----------------------------------------|---------------------------------------------------------------|
| 1    | List, search, and read Ideas by agent.   | No file in the workspace changes; the overview order stays.   |

## TC-018-13: The agent cursor while reading

**Flow:** BR-006, FR-057, FR-058  
**Preconditions:** "Long" is open in a tab, scrolled to its top; access is read only; Claude Code is connected.

| Step | Action                                        | Expected result                                                         |
|------|-----------------------------------------------|-------------------------------------------------------------------------|
| 1    | Let Claude Code read lines 200 to 210 of Long. | The agent cursor marks lines 200 to 210 with "Claude Code"; the tab does not scroll; the user's cursor stays. |
| 2    | Wait 10 seconds.                               | The agent cursor is hidden.                                            |
| 3    | Let it read again, then choose "Off".          | The agent cursor is hidden at once.                                    |
| 4    | Let a second agent read the same lines.        | (Access read only again.) Each agent has its own named cursor.        |

## TC-018-14: Speed

**Flow:** BR-007, NFR-003  
**Preconditions:** A workspace of 10,000 notes.

| Step | Action                            | Expected result           |
|------|-----------------------------------|---------------------------|
| 1    | List, search for a word, read.     | Each answers within 1 s.  |
