# Test Cases: Edit Note

**Use Case:** [UC-005 Edit Note](../use_cases/UC-005-edit-note.md)  
**Requirements:** [FR-008, FR-009, FR-010, FR-011, FR-012, FR-021, FR-023, FR-024, FR-025, FR-026, FR-031, FR-032, FR-033, FR-034, FR-044, NFR-002, NFR-005, NFR-006, NFR-007, NFR-011, NFR-012](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements                 |
|-----------|------------------------------------------------------|------------------------------|
| TC-005-01 | Main success scenario, edit in the source view       | FR-008, FR-044, FR-021       |
| TC-005-02 | Main success scenario, edit in the facade            | FR-009, FR-011, BR-006       |
| TC-005-03 | Facade changes only the edited block                 | NFR-007, BR-002              |
| TC-005-04 | Encoding and line endings kept                       | BR-002                       |
| TC-005-05 | Autosave timing and the moments it stores at once    | NFR-005, BR-001              |
| TC-005-06 | Never half written                                   | NFR-006                      |
| TC-005-07 | Typing latency in a long note                        | NFR-002, BR-003              |
| TC-005-08 | A1 Switch the editing view                           | FR-010                       |
| TC-005-09 | A2 Formatting toolbar in both formats and views      | FR-023, BR-005, NFR-011      |
| TC-005-10 | A3 AsciiDoc-specific options                         | FR-012, BR-007               |
| TC-005-11 | A4 Block shown as markup                             | BR-008                       |
| TC-005-12 | A5 Undo and redo across views                        | BR-004                       |
| TC-005-13 | Undo after a change from outside                     | BR-004                       |
| TC-005-14 | A6 Vim input mode                                    | FR-024, FR-026, BR-009       |
| TC-005-15 | A7 Nano input mode                                   | FR-025, FR-026, BR-010       |
| TC-005-16 | Normal input mode keys                               | BR-015                       |
| TC-005-17 | A8 Copy on select, and pasting over a selection      | FR-031, FR-032, BR-011       |
| TC-005-18 | A9 Drop images and files from outside                | FR-033, BR-012               |
| TC-005-19 | A10 Dropped file name is taken                       | BR-012                       |
| TC-005-20 | A11 Drop a file of the workspace                     | BR-012                       |
| TC-005-21 | A12 Drop a link                                      | FR-034                       |
| TC-005-22 | A13 Something that cannot be dropped                 | —                            |
| TC-005-23 | A14 Note cannot be stored                            | BR-013                       |
| TC-005-24 | A15 Note deleted outside                             | —                            |
| TC-005-25 | A16 Note changed outside during an edit              | UC-004 A9                    |
| TC-005-26 | A17 Paste formatted and plain text                   | BR-014                       |
| TC-005-27 | A18 Paste an image                                   | BR-014                       |
| TC-005-28 | A19 Large file                                       | BR-012                       |

## TC-005-01: Edit in the source view

**Flow:** Main success scenario  
**Preconditions:** `Plan.md` holds `# Plan` and is open in the source view.

| Step | Action                                            | Expected result                                                             |
|------|---------------------------------------------------|-----------------------------------------------------------------------------|
| 1    | Type `\n\n- **first** step` at the end.           | The markup appears as typed.                                                |
| 2    | Wait 2 seconds.                                   | The status bar reports that `Plan` was saved.                               |
| 3    | Look at `Plan.md` with another program.           | It holds `# Plan`, an empty line, and `- **first** step`.                   |

## TC-005-02: Edit in the facade

**Flow:** Main success scenario, BR-006  
**Preconditions:** `Guide.adoc` holds `= Guide` and a paragraph `Hello.`; it is open in the facade.

| Step | Action                                                  | Expected result                                                                |
|------|---------------------------------------------------------|--------------------------------------------------------------------------------|
| 1    | Look at the facade.                                     | The format indicator shows AsciiDoc.                                           |
| 2    | Type ` World` after `Hello.`, select `World`, and make it bold with the toolbar. | `World` is shown bold.                                |
| 3    | Wait 2 seconds and look at `Guide.adoc`.                | The paragraph is `Hello. *World*`, in AsciiDoc markup.                         |
| 4    | Repeat steps 2 and 3 with a Markdown note.              | The paragraph is `Hello. **World**`; the format indicator shows Markdown.      |

## TC-005-03: The facade changes only the edited block

**Flow:** BR-002, NFR-007  
**Preconditions:** `Mixed.md` uses `*` for some bullets and `-` for others, `__bold__` instead of `**bold**`, a table with unaligned columns, two empty lines between some paragraphs, and trailing spaces in one line.

| Step | Action                                                       | Expected result                                                                     |
|------|--------------------------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Open `Mixed` in the facade and change one word in the third paragraph. | —                                                                         |
| 2    | Wait 2 seconds and compare `Mixed.md` with its version before. | Only the third paragraph differs; bullets, `__bold__`, the table, empty lines, and trailing spaces elsewhere are unchanged byte for byte. |
| 3    | Switch the tab to the source view and back without typing.   | The file is unchanged.                                                              |

## TC-005-04: Encoding and line endings are kept

**Flow:** BR-002  
**Preconditions:** `Windows.md` uses Windows line endings (CR LF) and holds `Größe`.

| Step | Action                                             | Expected result                                                                 |
|------|----------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Add a new line `Änderung` in either view and wait 2 seconds. | `Windows.md` still uses CR LF for every line, the new one included; `Größe` and `Änderung` are stored correctly. |

## TC-005-05: Autosave timing

**Flow:** Step 4, BR-001  
**Preconditions:** `Plan` is open.

| Step | Action                                                                  | Expected result                                                     |
|------|-------------------------------------------------------------------------|---------------------------------------------------------------------|
| 1    | Type a word and watch `Plan.md` with another program.                   | The word is in the file at most 2 seconds after the last keystroke. |
| 2    | Type for 10 seconds without pause.                                      | The file is updated during typing, each time within 2 seconds of a keystroke. |
| 3    | Type a word and close the tab at once.                                  | The word is in the file.                                            |
| 4    | Type a word and end Underline at once by its menu.                      | The word is in the file.                                            |
| 5    | Type a word and open another workspace at once (UC-002).                | The word is in the file.                                            |
| 6    | Type a word, end Underline by force after 1 second.                      | After a restart, `Plan.md` holds the note with or without that word, never part of it. |

## TC-005-06: Never half written

**Flow:** NFR-006  
**Preconditions:** `Big.md` holds 10,000 lines.

| Step | Action                                                                 | Expected result                                                         |
|------|------------------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Change the first line and cut the power of the machine while the note is being stored. | —                                                       |
| 2    | Start the machine and look at `Big.md`.                                | It holds either the old or the new content, complete.                    |

## TC-005-07: Typing latency in a long note

**Flow:** BR-003, NFR-002  
**Preconditions:** `Big.md` holds 10,000 lines with code blocks, tables, and a diagram.

| Step | Action                                                    | Expected result                                          |
|------|-----------------------------------------------------------|----------------------------------------------------------|
| 1    | Type 200 characters at line 5,000 in the source view.     | Every character appears within 16 milliseconds.          |
| 2    | Repeat in the facade.                                     | Every character appears within 16 milliseconds.          |

## TC-005-08: Switch the editing view

**Flow:** A1, FR-010  
**Preconditions:** `Links.md` holds a paragraph with the link `[Docs](https://example.org/docs)`; it is open in the source view.

| Step | Action                                                         | Expected result                                                         |
|------|----------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Select the word `Docs` and switch to the facade.               | `Docs` is selected in the facade.                                       |
| 2    | Put the cursor inside `example.org` and switch to the facade.  | The cursor is at the link `Docs`, the nearest place the facade shows.   |
| 3    | Switch back to the source view.                                | The cursor is in the link markup.                                       |

## TC-005-09: Formatting toolbar

**Flow:** A2, BR-005, NFR-011  
**Preconditions:** `Tools.md` and `Tools.adoc` hold the line `word`; each is open.

| Step | Action                                                                      | Expected result                                                                     |
|------|-----------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | In `Tools.md` (facade), select `word` and apply heading, bold, italic, code, list, and quote in turn, undoing each. | Each is shown formatted; the file holds `# word`, `**word**`, `*word*`, `` `word` ``, `- word`, `> word` in turn. |
| 2    | Repeat step 1 in `Tools.adoc`.                                              | The file holds `= word` (a section), `*word*`, `_word_`, `` `word` ``, `* word`, and a quote block in turn. |
| 3    | Repeat steps 1 and 2 in the source view.                                    | The same markup is inserted and shown as markup.                                    |
| 4    | With nothing selected, insert a table.                                      | A table of 2 columns and 2 rows is inserted at the cursor in the markup of the format. |
| 5    | Choose link and choose the note `Plan`.                                     | A link to `Plan` is inserted, relative to the note.                                 |
| 6    | Choose image and choose `photo.png` from outside the workspace.             | `photo.png` is copied next to the note and inserted as an image.                    |
| 7    | Reach and apply every toolbar function with the keyboard only.              | Each function can be reached and applied; its key is shown next to it.             |

## TC-005-10: AsciiDoc-specific options

**Flow:** A3, BR-007  
**Preconditions:** `Spec.adoc` holds an image, a table, and a paragraph; `Spec.md` holds the same in Markdown. Both are open in the facade.

| Step | Action                                                        | Expected result                                                                  |
|------|---------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | In `Spec.adoc`, give the image the caption `Overview`.        | The caption is shown; the file holds `.Overview` before the image.               |
| 2    | Give the table the caption `Costs`.                           | The file holds `.Costs` before the table.                                        |
| 3    | Turn the paragraph into a warning admonition.                 | It is shown as a warning; the file holds `WARNING:` before it.                   |
| 4    | Give the table the id `costs`.                                | The file holds `[#costs]` before the table.                                      |
| 5    | Look for these options in `Spec.md`.                          | They are not offered.                                                            |

## TC-005-11: A block shown as markup

**Flow:** A4, BR-008  
**Preconditions:** `Raw.md` holds a paragraph, the HTML `<details><summary>More</summary>Hidden</details>`, and a second paragraph.

| Step | Action                                                         | Expected result                                                                  |
|------|----------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Open `Raw` in the facade.                                      | Both paragraphs are formatted; the HTML is shown as markup in a frame.           |
| 2    | Change `Hidden` to `Shown` inside the frame and wait 2 seconds. | The file holds the HTML with `Shown`; the paragraphs are unchanged.             |

## TC-005-12: Undo and redo across views

**Flow:** A5, BR-004  
**Preconditions:** `Plan` and `Ideas` are open.

| Step | Action                                                          | Expected result                                                   |
|------|-----------------------------------------------------------------|-------------------------------------------------------------------|
| 1    | In `Plan` (facade), type `one`, switch to the source view, type `two`. | —                                                          |
| 2    | Undo twice.                                                     | `two`, then `one` are taken back; the file follows within 2 seconds. |
| 3    | Redo once.                                                      | `one` is back.                                                    |
| 4    | Switch to `Ideas` and undo.                                     | Only changes of `Ideas` are taken back; `Plan` is unchanged.      |
| 5    | Close `Plan`, open it again, and undo.                          | Nothing is taken back.                                            |

## TC-005-13: Undo after a change from outside

**Flow:** BR-004, UC-004 A7  
**Preconditions:** `Plan` is open; all changes are stored.

| Step | Action                                                          | Expected result                                                  |
|------|-----------------------------------------------------------------|------------------------------------------------------------------|
| 1    | Type `mine`, wait 2 seconds, then add `theirs` with another program. | The tab shows `mine` and `theirs`.                         |
| 2    | Undo.                                                           | Nothing is taken back; `theirs` and `mine` stay.                 |

## TC-005-14: Vim input mode

**Flow:** A6, BR-009  
**Preconditions:** The Vim input mode is chosen (UC-010). `Vim.md` holds the lines `alpha`, `beta`, `gamma`, open in the source view.

| Step | Action                                             | Expected result                                                             |
|------|----------------------------------------------------|-----------------------------------------------------------------------------|
| 1    | Look at the status bar.                            | It shows the normal mode and its most important commands.                  |
| 2    | Type `j`, `dd`.                                    | `beta` is deleted.                                                          |
| 3    | Type `p`.                                          | `beta` is put below `gamma`.                                                |
| 4    | Type `u`, then `Ctrl+R`.                           | The put is undone, then redone.                                             |
| 5    | Type `gg`, `2dd`.                                  | Two lines are deleted.                                                      |
| 6    | Type `i`, `new`, `Esc`.                            | The status bar shows insert mode while typing, then normal mode; `new` is inserted. |
| 7    | Type `v`, `e`, `y`, then `/new` and `n`.           | Visual mode selects, yank copies; search finds `new`.                      |
| 8    | Switch to the facade and repeat steps 2 to 4.      | The keys work the same.                                                     |
| 9    | Type `:w`.                                         | The note is stored at once.                                                 |
| 10   | Type `:q`.                                         | The tab is closed.                                                          |

## TC-005-15: Nano input mode

**Flow:** A7, BR-010  
**Preconditions:** The Nano input mode is chosen. `Nano.md` holds three lines, open.

| Step | Action                                           | Expected result                                                      |
|------|--------------------------------------------------|----------------------------------------------------------------------|
| 1    | Look at the status bar.                          | It shows the most important Nano commands.                           |
| 2    | Press `Ctrl+K` on the second line, move down, press `Ctrl+U`. | The line is cut and pasted below.                       |
| 3    | Press `Ctrl+W`, type a word, press Enter.        | The cursor moves to that word.                                       |
| 4    | Undo and redo with the Nano keys.                | The last change is undone and redone.                                |
| 5    | Repeat steps 2 to 4 in the facade.               | The keys work the same.                                              |
| 6    | Press `Ctrl+X`.                                  | The tab is closed.                                                   |

## TC-005-16: Normal input mode keys

**Flow:** BR-015  
**Preconditions:** The normal input mode is chosen. `Plan` is open. The test runs on Linux or Windows with `Ctrl` and on macOS with `Cmd`.

| Step | Action                                      | Expected result                                      |
|------|---------------------------------------------|------------------------------------------------------|
| 1    | Select all with `Ctrl+A`, copy with `Ctrl+C`. | The whole note is on the clipboard.                |
| 2    | Type `x`, undo with `Ctrl+Z`, redo with `Ctrl+Shift+Z`, undo again, redo with `Ctrl+Y`. | Each works as named. |
| 3    | Select a word, cut with `Ctrl+X`, paste with `Ctrl+V` elsewhere. | The word moves.                              |
| 4    | Make a word bold with the key shown next to the bold function. | The word is bold.                              |

## TC-005-17: Copy on select, and pasting over a selection

**Flow:** A8, BR-011, FR-032  
**Preconditions:** Copy on select is on. `Plan.md` holds the paragraph `Use **bold** here.` and the word `target`; it is open in the facade. The clipboard holds `REPLACEMENT`.

| Step | Action                                                                  | Expected result                                                                  |
|------|-------------------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Select `target` and paste.                                              | `target` is replaced by `REPLACEMENT`; afterwards the clipboard holds `target`.  |
| 2    | Select `Use bold here` in the facade.                                   | Without a further command, the clipboard holds `Use **bold** here` as markup and as formatted text. |
| 3    | Paste into another Markdown note.                                       | The paragraph appears with `bold` bold.                                          |
| 4    | Paste into a mail program.                                              | The text appears with `bold` bold.                                               |
| 5    | Select the same text in the source view.                                | The clipboard holds the markup `Use **bold** here`.                              |
| 6    | Switch copy on select off (UC-010) and select a word.                   | The clipboard is unchanged.                                                      |

## TC-005-18: Drop images and files from outside

**Flow:** A9, BR-012  
**Preconditions:** `guide/Setup.md` is open in the facade. Outside the workspace lie `shot.png`, `logo.svg`, and `manual.pdf`.

| Step | Action                                             | Expected result                                                                         |
|------|----------------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | Drop `shot.png` into the middle of a paragraph.    | `guide/shot.png` is created; the image is shown at the drop position; the note refers to `shot.png`. |
| 2    | Drop `logo.svg`.                                   | It is copied and shown as an image.                                                     |
| 3    | Drop `manual.pdf`.                                 | It is copied to `guide/manual.pdf` and inserted as a link named `manual.pdf`.           |
| 4    | Drop `shot.png` into a code block.                 | Only `shot.png` is inserted, as text.                                                   |
| 5    | Look at the original files.                        | They are unchanged where they were.                                                     |
| 6    | Push the workspace to a GitHub wiki and open the page. | The images and the link work there as in Underline.                                 |

## TC-005-19: The dropped file name is taken

**Flow:** A10  
**Preconditions:** The folder of the open note holds `diagram.png`. Outside lie a `diagram.png` with the same content and one with other content.

| Step | Action                                       | Expected result                                                                  |
|------|----------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Drop the `diagram.png` with the same content. | Nothing is copied; the existing `diagram.png` is inserted.                      |
| 2    | Drop the one with other content.             | It is copied as `diagram-2.png` and inserted; the status bar says so.            |
| 3    | Drop it again.                               | Nothing is copied; `diagram-2.png` is inserted, since its content is the same.   |

## TC-005-20: Drop a file of the workspace

**Flow:** A11  
**Preconditions:** `guide/Setup.md` is open. The workspace holds `img/arch.png` and `Plan.md`.

| Step | Action                                                    | Expected result                                                         |
|------|-----------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Drop `img/arch.png` from the file manager into the note.  | Nothing is copied; the image is inserted as `../img/arch.png`.          |
| 2    | Drop `Plan.md`.                                           | A link `../Plan.md` is inserted.                                        |

## TC-005-21: Drop a link

**Flow:** A12, FR-034  
**Preconditions:** A browser shows a page with the link text `Rust Book` to `https://doc.rust-lang.org/book/`.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Drag the link into the note.                   | A link `Rust Book` to that address is inserted at the drop position.    |
| 2    | Drag the address bar of the browser into the note. | A link is inserted whose text is the address.                       |

## TC-005-22: Something that cannot be dropped

**Flow:** A13  
**Preconditions:** A folder `photos` and a file `secret.png` that the user may not read lie outside the workspace.

| Step | Action                    | Expected result                                                            |
|------|---------------------------|----------------------------------------------------------------------------|
| 1    | Drop `photos`.            | The status bar says that folders cannot be dropped; the note is unchanged. |
| 2    | Drop `secret.png`.        | The status bar says that the file cannot be read; nothing is copied.       |

## TC-005-23: The note cannot be stored

**Flow:** A14, BR-013  
**Preconditions:** `Plan` is open from a removable drive.

| Step | Action                                                       | Expected result                                                                    |
|------|--------------------------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Remove the drive and type `important`.                       | Within 2 seconds the tab is marked as not stored and the status bar says why; the message stays. |
| 2    | Type more and wait 30 seconds.                               | The text stays in the tab; the mark stays.                                         |
| 3    | Try to close the tab.                                        | The system asks whether to close anyway and lose the changes; cancel.             |
| 4    | Insert the drive again.                                      | Within 10 seconds the note is stored with all changes; the mark goes; the status bar reports that it was saved. |

## TC-005-24: The note was deleted outside

**Flow:** A15  
**Preconditions:** `Old` is open; `Old.md` was deleted with another program, so the tab is marked as deleted (UC-004 A8).

| Step | Action                         | Expected result                                                                       |
|------|--------------------------------|---------------------------------------------------------------------------------------|
| 1    | Type a word into `Old`.        | Within 2 seconds `Old.md` exists again with the content; the mark goes; `Old` is listed again; the status bar says so. |

## TC-005-25: The note is changed outside during an edit

**Flow:** A16  
**Preconditions:** `Plan` is open.

| Step | Action                                                                      | Expected result                                                                  |
|------|-----------------------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Type `mine`; within the next second, add `theirs` with another program.      | Both versions are kept as TC-004-21 describes; nothing is lost.                 |

## TC-005-26: Paste formatted and plain text

**Flow:** A17, BR-014  
**Preconditions:** The clipboard holds formatted text from a web page: a heading, a paragraph with bold and a link, a bulleted list, a table, and text in a color. `Paste.md` and `Paste.adoc` are open.

| Step | Action                                      | Expected result                                                                         |
|------|---------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | Paste into `Paste.md` in the facade.        | Heading, bold, link, list, and table are kept and stored as Markdown; the color is dropped. |
| 2    | Paste into `Paste.adoc` in the facade.      | The same is stored as AsciiDoc.                                                         |
| 3    | Paste into `Paste.md` in the source view.   | The plain text is pasted.                                                               |
| 4    | Paste as plain text in the facade.          | The plain text is pasted, unformatted.                                                  |

## TC-005-27: Paste an image

**Flow:** A18, BR-014  
**Preconditions:** The clipboard holds a screenshot. `guide/Plan.md` is open; the time is 2026-11-06 14:30.

| Step | Action                                     | Expected result                                                                     |
|------|--------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Paste.                                     | `guide/Plan-2026-11-06-1430.png` is created and shown at the cursor.                |
| 2    | Paste again within the same minute.        | `guide/Plan-2026-11-06-1430-2.png` is created and shown.                            |

## TC-005-28: Large file

**Flow:** A19  
**Preconditions:** Outside the workspace lies `talk.mp4` of 500 MB. The workspace is a git repository.

| Step | Action                                     | Expected result                                                                       |
|------|--------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Drop `talk.mp4`.                           | The system says the file has 500 MB and asks whether to copy it or link to it.        |
| 2    | Cancel.                                    | Nothing changes.                                                                      |
| 3    | Drop it again and choose to link.          | A link to `talk.mp4` at its place on this computer is inserted; nothing is copied; git shows no new file. |
| 4    | Follow the link (UC-004 A4).               | The video opens in the program the system uses for it.                                 |
| 5    | Drop it again and choose to copy.          | `talk.mp4` is copied next to the note and inserted as a link.                         |
| 6    | Drop a file of 5 MB.                       | It is copied without asking.                                                          |
