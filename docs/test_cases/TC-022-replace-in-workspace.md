# Test Cases: Replace in Workspace

**Use Case:** [UC-022 Replace in Workspace](../use_cases/UC-022-replace-in-workspace.md)  
**Requirements:** [FR-074, FR-075, FR-101, NFR-002, NFR-006, NFR-011, NFR-013, NFR-017](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements            |
|-----------|------------------------------------------------------|-------------------------|
| TC-022-01 | Main success scenario: see, leave out, replace       | FR-074, BR-004, BR-005  |
| TC-022-02 | The search text is one text                          | BR-002                  |
| TC-022-03 | Match case and whole word                            | FR-075, BR-002          |
| TC-022-04 | Regular expression with groups                       | FR-075, BR-003          |
| TC-022-05 | A1 Nothing found                                     | —                       |
| TC-022-06 | A2 Regular expression not valid                      | —                       |
| TC-022-07 | A3 Replace in one note                               | BR-005                  |
| TC-022-08 | A4 A note changes while the places are shown         | —                       |
| TC-022-09 | A5 Note cannot be stored                             | BR-006, NFR-006         |
| TC-022-10 | A6 Undo the replacement                              | BR-005, FR-101          |
| TC-022-11 | A7 Many places                                       | BR-004                  |
| TC-022-12 | A8 User cancels                                      | BR-004                  |
| TC-022-13 | Open note with changes not yet saved; undo in the tab | BR-001, BR-005         |
| TC-022-14 | What is not replaced                                 | BR-001                  |
| TC-022-15 | Links are replaced as text                           | BR-006                  |
| TC-022-16 | Speed in a large workspace                           | BR-007, NFR-017, NFR-002 |
| TC-022-17 | Keyboard alone                                       | BR-008, NFR-011         |

## TC-022-01: See, leave out, and replace

**Flow:** Main success scenario, BR-004, BR-005  
**Preconditions:** The workspace `notes` holds `Trip.md` with "Book the ferry" and "ferry times", `Ideas.md` with "a ferry to Hydra", and `Log.md` with "ferry". No note is open.

| Step | Action                                                         | Expected result                                                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+H, type `ferry` as search text and `boat` as replacement. | "4 places in 3 notes"; each line shows before and after, such as "Book the ferry" → "Book the boat". |
| 2    | Leave out the note "Log".                                      | "Log" and its place are marked as left out.                                                              |
| 3    | Confirm.                                                       | The status bar says 3 places in 2 notes were replaced and offers to undo; the list no longer shows them; "Log" is still listed. |
| 4    | Look at the three files with another program.                  | `Trip.md` and `Ideas.md` hold "boat" instead of "ferry"; `Log.md` is unchanged.                         |
| 5    | Ask for the snapshots of "Trip" (UC-026).                      | The newest snapshot has the cause Replace in workspace and holds "ferry".                                |

## TC-022-02: The search text is one text

**Flow:** BR-002  
**Preconditions:** `A.md` holds "release plan", `B.md` holds "plan the release".

| Step | Action                                         | Expected result                       |
|------|------------------------------------------------|---------------------------------------|
| 1    | Search `release plan` for replacing.           | Only the place in "A" is found.       |

## TC-022-03: Match case and whole word

**Flow:** BR-002, FR-075  
**Preconditions:** `Words.md` holds "Plan plan planning".

| Step | Action                                                    | Expected result                        |
|------|-----------------------------------------------------------|----------------------------------------|
| 1    | Search `plan` with no switch on.                          | 3 places are found.                    |
| 2    | Switch match case on.                                     | 2 places: "plan" and "planning".       |
| 3    | Switch whole word on as well.                             | 1 place: "plan".                       |

## TC-022-04: Regular expression with groups

**Flow:** BR-003, FR-075  
**Preconditions:** `Dates.md` holds "Due 11.11.2026" and "Due 01.12.2026".

| Step | Action                                                                                  | Expected result                                      |
|------|-----------------------------------------------------------------------------------------|------------------------------------------------------|
| 1    | Switch regular expression on, search `(\d\d)\.(\d\d)\.(\d{4})`, replace with `$3-$2-$1`. | The places show "Due 2026-11-11" and "Due 2026-12-01" as after. |
| 2    | Confirm.                                                                                | `Dates.md` holds both dates in the new form.        |

## TC-022-05: Nothing found

**Flow:** A1  
**Preconditions:** No note holds "zebra".

| Step | Action                              | Expected result                                         |
|------|-------------------------------------|---------------------------------------------------------|
| 1    | Search `zebra` for replacing.       | The system says no note holds it; replacing is not possible. |

## TC-022-06: Regular expression not valid

**Flow:** A2  
**Preconditions:** As TC-022-04.

| Step | Action                                                  | Expected result                                               |
|------|---------------------------------------------------------|---------------------------------------------------------------|
| 1    | With regular expression on, search `(\d\d`.             | The system says the expression is not valid; no places are shown. |
| 2    | Search `(\d\d)` and replace with `$2`.                  | The system says the expression has no group 2; replacing is not possible. |

## TC-022-07: Replace in one note

**Flow:** A3, BR-005  
**Preconditions:** As TC-022-01.

| Step | Action                                                    | Expected result                                                  |
|------|-----------------------------------------------------------|------------------------------------------------------------------|
| 1    | Search `ferry`, replace with `boat`, and replace in "Ideas" only. | Only `Ideas.md` changes; the status bar offers to undo.     |
| 2    | Look at the list.                                         | "Trip" and "Log" are still listed with their places.             |
| 3    | Ask for the snapshots of "Ideas" and of "Trip" (UC-026).  | "Ideas" has a snapshot with the cause Replace in workspace holding "a ferry to Hydra"; "Trip" has none from the replacement. |

## TC-022-08: A note changes while the places are shown

**Flow:** A4  
**Preconditions:** As TC-022-01; `ferry` is searched for replacing and the place in "Log" is left out.

| Step | Action                                                     | Expected result                                          |
|------|------------------------------------------------------------|----------------------------------------------------------|
| 1    | Add "ferry port" to `Log.md` with another program.         | "Log" shows 2 places; the first stays left out, the new one is chosen. |
| 2    | Create the note "Port" holding "ferry".                    | "Port" appears in the list.                              |

## TC-022-09: Note cannot be stored

**Flow:** A5, BR-006, NFR-006  
**Preconditions:** As TC-022-01; `Ideas.md` may only be read.

| Step | Action                                            | Expected result                                                          |
|------|---------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Replace `ferry` with `boat` in every note.        | "Trip" and "Log" are changed; the status bar names "Ideas" as not changed and why. |
| 2    | Look at `Ideas.md`.                               | It is unchanged and whole.                                               |

## TC-022-10: Undo the replacement

**Flow:** A6, BR-005  
**Preconditions:** TC-022-01 step 3 was just done; then `Ideas.md` was changed with another program to "a boat to Hydra and back".

| Step | Action                                   | Expected result                                                                  |
|------|------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Choose undo in the status bar.           | `Trip.md` holds "ferry" again; `Ideas.md` is left as changed outside and named in the status bar. |

## TC-022-11: Many places

**Flow:** A7, BR-004  
**Preconditions:** 20 notes each hold "x" 600 times.

| Step | Action                                    | Expected result                                                          |
|------|-------------------------------------------|--------------------------------------------------------------------------|
| 1    | Search `x` for replacing with `y`.         | 10,000 places are shown, and the system says 2,000 more exist.           |
| 2    | Confirm.                                  | All 12,000 places hold `y`.                                              |

## TC-022-12: Cancel

**Flow:** A8, BR-004  
**Preconditions:** As TC-022-01.

| Step | Action                                                  | Expected result                                               |
|------|---------------------------------------------------------|---------------------------------------------------------------|
| 1    | Search `ferry`, type `boat`, and close the replace.     | No note changes; the search of the workspace holds `ferry`.  |

## TC-022-13: Open note with changes not yet saved; undo in the tab

**Flow:** BR-001, BR-005  
**Preconditions:** "Trip" is open; "ferry pier" was just typed into it and is not yet saved.

| Step | Action                                         | Expected result                                                    |
|------|------------------------------------------------|--------------------------------------------------------------------|
| 1    | Search `ferry` for replacing.                  | "Trip" shows the place in "ferry pier" too.                        |
| 2    | Replace with `boat` and confirm.               | The tab of "Trip" shows "boat pier"; the note is stored.           |
| 3    | In the tab of "Trip", press Ctrl+Z.            | "Trip" holds "ferry" in all its places again; the other notes stay replaced. |
| 4    | Ask for the snapshots of "Trip" (UC-026).      | A snapshot with the cause Replace in workspace holds "ferry pier", the text of the tab before the replacement, with the change not yet saved. |

## TC-022-14: What is not replaced

**Flow:** BR-001  
**Preconditions:** `ferry/Ferry-Log.md` holds no "ferry" in its text; the deleted note "Old" held "ferry"; `.hidden/F.md` and `img/ferry.txt` hold "ferry".

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Replace `ferry` with `boat` in every note.     | "Ferry Log" keeps its title and folder; the deleted note, `.hidden/F.md`, and `img/ferry.txt` are unchanged. |

## TC-022-15: Links are replaced as text

**Flow:** BR-006  
**Preconditions:** `Index.md` holds `[Plan](Plan.md)`; `Plan.md` exists.

| Step | Action                                         | Expected result                                                          |
|------|------------------------------------------------|--------------------------------------------------------------------------|
| 1    | Replace `Plan.md` with `Plan-2026.md`.         | `Index.md` holds `[Plan](Plan-2026.md)`; `Plan.md` is not renamed, and the link is a broken link. |

## TC-022-16: Speed in a large workspace

**Flow:** BR-007, NFR-017, NFR-002  
**Preconditions:** A workspace of 10,000 notes of 1 to 50 KB each; 1,000 of them hold "Acme".

| Step | Action                                                         | Expected result                                       |
|------|----------------------------------------------------------------|-------------------------------------------------------|
| 1    | Search 20 different texts for replacing, measuring until the places appear. | At least 19 of 20 appear within 2 seconds. |
| 2    | Replace `Acme` with `Acme Corp` in all notes, measuring.        | Done within 10 seconds.                               |
| 3    | Type a long search text at 10 characters a second.              | Every character appears in the field at once.         |

## TC-022-17: Keyboard alone

**Flow:** BR-008, NFR-011  
**Preconditions:** As TC-022-01; the focus is in a note.

| Step | Action                                                                        | Expected result                                  |
|------|-------------------------------------------------------------------------------|--------------------------------------------------|
| 1    | Press Ctrl+Shift+H, type `ferry`, Tab, `boat`.                                 | The places are shown.                            |
| 2    | Press Alt+W, move to the place in "Log" with the arrow keys, and leave it out. | Whole word is on; the place is left out.         |
| 3    | Confirm by keyboard.                                                          | The other places are replaced.                   |
