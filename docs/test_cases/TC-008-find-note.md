# Test Cases: Find Note

**Use Case:** [UC-008 Find Note](../use_cases/UC-008-find-note.md)  
**Requirements:** [FR-007, FR-017, FR-018, NFR-002, NFR-003, NFR-011, NFR-013](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements            |
|-----------|----------------------------------------------------|-------------------------|
| TC-008-01 | Main success scenario: find and open at the place  | FR-007, BR-004, BR-008  |
| TC-008-02 | All words, any order, any case                     | BR-002                  |
| TC-008-03 | Words in double quotes                             | BR-002                  |
| TC-008-04 | Accents count                                      | BR-002                  |
| TC-008-05 | Titles and folders first                           | BR-001, BR-003          |
| TC-008-06 | Markup, unsaved text, and what is not searched     | BR-001                  |
| TC-008-07 | A1 Nothing found                                   | —                       |
| TC-008-08 | A2 Search cleared                                  | BR-008                  |
| TC-008-09 | A3 A note changes while found                      | —                       |
| TC-008-10 | A4 Note cannot be read                             | —                       |
| TC-008-11 | A5 Many places in one note                         | BR-004                  |
| TC-008-12 | A6 Find in the active note                         | BR-009                  |
| TC-008-13 | Find in the note with the Vim and Nano keys        | BR-009                  |
| TC-008-14 | Searching changes nothing                          | BR-006                  |
| TC-008-15 | Speed in a large workspace                         | BR-005, NFR-002, NFR-003 |
| TC-008-16 | Keyboard alone                                     | BR-007, NFR-011         |
| TC-008-17 | Nothing remembered                                 | BR-010                  |

## TC-008-01: Find a note and open it at the place

**Flow:** Main success scenario, BR-004, BR-008  
**Preconditions:** The workspace `notes` holds `Trip.md` with the line "Book the ferry to Hydra" on line 12, and `Ideas.md` without the word "ferry". `Trip.md` is not open; the facade is the default view.

| Step | Action                                   | Expected result                                                                                      |
|------|------------------------------------------|------------------------------------------------------------------------------------------------------|
| 1    | Click the search field and type "ferry".  | The note overview shows "Trip" with its folder, "1 place", and the line "Book the ferry to Hydra" with "ferry" marked; "Ideas" is not listed. |
| 2    | Click the found line.                     | "Trip" opens in a tab, scrolled to that line, with "ferry" selected.                                |
| 3    | Look at the note.                         | Every "ferry" of the note is marked.                                                                 |
| 4    | Look at the search field.                 | It still holds "ferry" and "Trip" is still listed.                                                  |

## TC-008-02: All words, any order, any case

**Flow:** BR-002  
**Preconditions:** `A.md` holds "release plan for Q1", `B.md` holds "plan the RELEASES", `C.md` holds "release notes".

| Step | Action               | Expected result                                       |
|------|----------------------|-------------------------------------------------------|
| 1    | Search "Plan release". | "A" and "B" are found, "C" is not.                  |
| 2    | Search "leas".        | All three are found.                                 |

## TC-008-03: Words in double quotes

**Flow:** BR-002  
**Preconditions:** As TC-008-02.

| Step | Action                     | Expected result                       |
|------|----------------------------|---------------------------------------|
| 1    | Search `"release plan"`.   | Only "A" is found.                    |
| 2    | Search `"plan the" q1`.    | Nothing is found (A1).                |

## TC-008-04: Accents count

**Flow:** BR-002  
**Preconditions:** `Uns.md` holds "Über uns".

| Step | Action          | Expected result           |
|------|-----------------|---------------------------|
| 1    | Search "uber".  | Nothing is found.         |
| 2    | Search "ÜBER".  | "Uns" is found.           |

## TC-008-05: Titles and folders first

**Flow:** BR-001, BR-003  
**Preconditions:** `travel/Packing.md` holds no "travel"; `Budget.md` holds "travel costs" and was opened more recently than `Packing`; `Travel-Log.md` holds "travel" and was opened longest ago.

| Step | Action             | Expected result                                                                                  |
|------|--------------------|--------------------------------------------------------------------------------------------------|
| 1    | Search "travel".   | The order is "Packing" (folder `travel`) and "Travel Log" (title) before "Budget"; within the first two, the more recently opened first. |

## TC-008-06: Markup, unsaved text, and what is not searched

**Flow:** BR-001  
**Preconditions:** `Links.md` holds `[site](https://example.org/kiwi)`; `Code.md` holds a code block with `let kiwi = 1;`; `Draft.md` is open and the user has just typed "mango", not yet saved; "Old" holding "kiwi" was deleted (UC-007); `.hidden/K.md` and `img/kiwi.png` exist.

| Step | Action           | Expected result                                       |
|------|------------------|-------------------------------------------------------|
| 1    | Search "kiwi".   | "Links" and "Code" are found; "Old", `.hidden/K.md`, and `kiwi.png` are not. |
| 2    | Search "mango".  | "Draft" is found.                                     |

## TC-008-07: Nothing found

**Flow:** A1  
**Preconditions:** No note holds "zebra".

| Step | Action           | Expected result                                                  |
|------|------------------|------------------------------------------------------------------|
| 1    | Search "zebra".  | The note overview says no note holds "zebra".                    |
| 2    | Type a space and "x". | The search runs again; still nothing is found.              |

## TC-008-08: Search cleared

**Flow:** A2, BR-008  
**Preconditions:** "ferry" was searched and `Trip.md` opened from it, with its occurrences marked.

| Step | Action                              | Expected result                                                   |
|------|-------------------------------------|-------------------------------------------------------------------|
| 1    | Press Escape in the search field.    | The field is empty, the note overview shows the notes, the marks in "Trip" are gone. |
| 2    | Search again and delete every letter. | The same.                                                        |

## TC-008-09: A note changes while found

**Flow:** A3  
**Preconditions:** "ferry" is searched; only "Trip" is found.

| Step | Action                                                          | Expected result                                        |
|------|-----------------------------------------------------------------|--------------------------------------------------------|
| 1    | Write "ferry times" into `Ideas.md` with another program.        | "Ideas" appears among the found notes.                 |
| 2    | In the tab of "Trip", delete the word "ferry".                   | "Trip" is no longer found.                             |
| 3    | Rename "Ideas" to "Plans" (UC-006).                               | "Plans" is found instead of "Ideas".                  |

## TC-008-10: Note cannot be read

**Flow:** A4  
**Preconditions:** `Secret.md` cannot be read; `Trip.md` holds "ferry".

| Step | Action           | Expected result                                                         |
|------|------------------|-------------------------------------------------------------------------|
| 1    | Search "ferry".  | "Trip" is found; below it, the overview says 1 note could not be searched. |

## TC-008-11: Many places in one note

**Flow:** A5, BR-004  
**Preconditions:** `Log.md` holds "deploy" on 8 lines, one of them 400 characters long with "deploy" near its end.

| Step | Action                   | Expected result                                                                 |
|------|--------------------------|---------------------------------------------------------------------------------|
| 1    | Search "deploy".         | "Log" shows "8 places", its first 5 lines, and "3 more".                        |
| 2    | Look at the long line.   | It is shortened around "deploy", which is visible and marked.                  |
| 3    | Click the note "Log".    | It opens at the first place found.                                             |

## TC-008-12: Find in the active note

**Flow:** A6, BR-009  
**Preconditions:** `Trip.md` is open in the facade with "ferry" three times; the cursor is between the first and the second.

| Step | Action                                   | Expected result                                                            |
|------|------------------------------------------|----------------------------------------------------------------------------|
| 1    | Press Ctrl+F and type "FERRY".            | The find field shows "3"; all three are marked; the second one is selected. |
| 2    | Press Enter, then F3.                     | The third, then the first one is selected.                                 |
| 3    | Press Shift+Enter.                        | The third one is selected.                                                 |
| 4    | Press Escape.                             | The find field closes; the third "ferry" stays selected; typing goes into the text. |
| 5    | Switch to the source view and repeat steps 1–2. | The same, in the markup.                                            |
| 6    | Select "Hydra" and press Ctrl+F.          | The find field holds "Hydra".                                              |

## TC-008-13: Find in the note with the Vim and Nano keys

**Flow:** BR-009  
**Preconditions:** `Trip.md` is open; the input mode is Vim, normal mode.

| Step | Action                                   | Expected result                                               |
|------|------------------------------------------|---------------------------------------------------------------|
| 1    | Type `/ferry` and Enter.                  | Vim's own search moves to the next "ferry"; no find field opens. |
| 2    | Press Ctrl+F.                             | Vim's meaning applies (page down); no find field opens.        |
| 3    | Switch the input mode to Normal and press Ctrl+F. | The find field opens.                                |

## TC-008-14: Searching changes nothing

**Flow:** BR-006  
**Preconditions:** The note overview lists "Ideas" first; the times of last change of all notes are noted.

| Step | Action                                    | Expected result                                                |
|------|-------------------------------------------|----------------------------------------------------------------|
| 1    | Search several words and clear the search. | The note overview lists "Ideas" first again.                  |
| 2    | Look at the notes.                         | No note has a new time of last change.                        |

## TC-008-15: Speed in a large workspace

**Flow:** BR-005, NFR-002, NFR-003  
**Preconditions:** A workspace of 10,000 notes of 1 to 50 KB each.

| Step | Action                                                         | Expected result                                               |
|------|----------------------------------------------------------------|---------------------------------------------------------------|
| 1    | Search 20 different words, measuring until the found notes appear. | At least 19 of the 20 searches answer within 1 second.     |
| 2    | Type a long search text quickly.                                | Every typed character appears in the field at once.           |

## TC-008-16: Keyboard alone

**Flow:** BR-007, NFR-011  
**Preconditions:** A note is open with the focus in its text; "ferry" is in `Trip.md` and `Ideas.md`.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Press Ctrl+Shift+F and type "ferry".            | The focus is in the search field; both notes are found. |
| 2    | Press the down arrow twice and Enter.           | The second found line opens.                         |
| 3    | Press Ctrl+Shift+F, then Escape.                | The search is cleared.                               |

## TC-008-17: Nothing remembered

**Flow:** BR-010  
**Preconditions:** "ferry" is searched.

| Step | Action               | Expected result                                       |
|------|----------------------|-------------------------------------------------------|
| 1    | Restart Underline.   | The search field is empty and the note overview shows the notes. |
