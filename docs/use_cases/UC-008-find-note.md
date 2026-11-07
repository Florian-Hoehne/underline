# Use Case: Find Note

## Overview

**Use Case ID:** UC-008  
**Use Case Name:** Find Note  
**Primary Actor:** User  
**Goal:** The user finds the notes that hold what they look for, sees where, and opens one at that place, without knowing its name.  
**Trigger:** The user starts to type in the search field of the note overview.  
**Status:** Draft  

**Requirements:** [FR-007, FR-017, FR-018, NFR-002, NFR-003, NFR-011, NFR-013](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).

## Main Success Scenario

1. The user puts the focus into the search field above the note overview, by clicking it or with Ctrl+Shift+F (Cmd+Shift+F on macOS), and types what they look for.
2. While the user types, the system searches the titles, folders, and texts of all notes of the workspace and shows, instead of the notes, the notes that match: those whose title or folder matches first, then the others, each with its title, its folder, the number of places found, and the lines found with the words marked.
3. The user chooses a found line, or a found note.
4. The system opens the note in a tab, or activates its tab, with the cursor at the place found and the words found selected, and marks every occurrence of the words in the note (BR-008); for a found note it opens at the first place found.
5. The search field keeps what the user typed and the found notes stay listed, so the user can open the next one.

## Alternative Flows

### A1: Nothing found

**Trigger:** No note matches (step 2)  
**Flow:**

1. The system says in the note overview that no note holds what the user typed.
2. Use case continues at step 1.

### A2: Search cleared

**Trigger:** The user empties the search field or presses Escape in it (any step)  
**Flow:**

1. The system empties the search field, removes the marks of BR-008, and shows the note overview again.
2. Use case ends.

### A3: A note changes while found

**Trigger:** A found note is changed in a tab or outside, or a note is created, renamed, or deleted, while the search field holds text (step 2 to step 5)  
**Flow:**

1. The system searches again and updates the found notes, keeping the one the user chose marked if it still matches.
2. Use case continues at the step it left.

### A4: Note cannot be read

**Trigger:** A note of the workspace cannot be read during the search (step 2)  
**Flow:**

1. The system searches the other notes and says below the found notes how many notes could not be searched.
2. Use case continues at step 3.

### A5: Many places in one note

**Trigger:** A note holds more than five places found (step 2)  
**Flow:**

1. The system shows its first five lines found and says how many more places it holds.
2. Use case continues at step 3; choosing the note opens it at the first place.

### A6: Find in the active note

**Trigger:** The user presses Ctrl+F (Cmd+F on macOS) in a tab (any time)  
**Flow:**

1. The system shows a find field at the top of the tab, holding the selected text if there is one.
2. While the user types, the system marks every occurrence in the note by the rules of BR-002, selects the first one after the cursor, and shows how many there are.
3. Enter or F3 selects the next occurrence, Shift+Enter or Shift+F3 the previous one, wrapping around at the end and the start of the note.
4. Escape closes the find field, keeps the selection, and returns the focus to the text.
5. Use case ends.

## Postconditions

### Success Postconditions

- The chosen note is open in the active tab at the place found, with the words found selected.
- No note is changed by searching.

### Failure Postconditions

- No note is changed, and the tabs are as they were.

## Business Rules

### BR-001: What is searched

The search looks at the title and the folder of every note of the workspace and at its text as it is stored, in its markup, so a link address or a word in a code block is found too. For a note open in a tab, the text of the tab is searched, with changes not yet saved. Deleted notes (UC-007), hidden folders, and files that are not notes are not searched.

### BR-002: What matches

The search text is split into words at spaces. A note matches when every word occurs in its title, folder, or text, each word anywhere, in any order, also inside a longer word. Upper and lower case do not matter; accents and umlauts do, so `uber` does not find `über`. Words in double quotes are one word with their spaces, so `"release plan"` finds exactly that sequence. There are no further options.

### BR-003: Order of the found notes

Notes whose title or folder holds every word come first; among them, and among the others, the most recently opened note comes first, as in the note overview (UC-002 BR-008).

### BR-004: Places found

A place found is a line of the text that holds at least one of the words. A line is shown as it is stored, shortened around the words when it is long, with every word found marked.

### BR-005: Speed

The found notes appear within 1 second for 95% of searches in a workspace of 10,000 notes (NFR-003), and the search starts again with every key typed, so typing is never held up by a running search (NFR-002).

### BR-006: Only reading

Searching never changes a note, its tab, or the order of the note overview; only opening a found note counts as opening it (UC-004).

### BR-007: Keyboard

Ctrl+Shift+F (Cmd+Shift+F on macOS) puts the focus into the search field from anywhere in the window. With the focus in the field, the arrow keys move through the found notes and lines, Enter opens the chosen one, and Escape clears the search (NFR-011).

### BR-008: Marked occurrences

While the search field holds text, every occurrence of its words is marked in the open notes, in both editing views; the marks go when the search is cleared (A2). Marks change nothing in the note.

### BR-009: Find in the note

Finding in the active note (A6) follows the matching rules of BR-002 within that one note, also in both editing views. In the Vim and Nano input modes, their own keys keep their meaning, such as `/` in Vim (UC-005); Ctrl+F opens the find field where the input mode gives it no meaning of its own.

### BR-010: Nothing remembered

The search text is not remembered when Underline closes.

