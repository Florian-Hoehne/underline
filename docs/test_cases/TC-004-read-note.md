# Test Cases: Read Note

**Use Case:** [UC-004 Read Note](../use_cases/UC-004-read-note.md)  
**Requirements:** [FR-004, FR-017, FR-018, FR-019, FR-027, FR-030, FR-035, FR-036, FR-037, FR-038, FR-039, FR-040, FR-041, FR-042, FR-043, FR-046, FR-071, FR-076, FR-078, FR-079, FR-086, FR-090, FR-091, FR-092, FR-096, FR-097, FR-098, NFR-004, NFR-005, NFR-007, NFR-008, NFR-014, NFR-015, NFR-018, NFR-019, C-003, C-004, C-005](../requirements.md)

## Coverage

| Test case | Flow / rule                                        | Requirements                    |
|-----------|----------------------------------------------------|---------------------------------|
| TC-004-01 | Main success scenario, open a note in a tab        | FR-017, FR-018, BR-003          |
| TC-004-02 | Markdown rendered as GitHub shows it               | C-003, FR-035, BR-004           |
| TC-004-03 | AsciiDoc rendered as GitHub shows it               | C-004, FR-004, BR-004           |
| TC-004-04 | Diagrams in the colors of the theme, offline       | FR-036, FR-037, FR-038, NFR-014, C-005 |
| TC-004-05 | Diagram rendering time                             | NFR-004                         |
| TC-004-06 | Images, enlarged; web images and offline           | FR-039, NFR-015, BR-004         |
| TC-004-07 | Tool panel: properties                             | FR-019, BR-010                  |
| TC-004-08 | Tool panel: outline, code index, figure index      | FR-041, FR-042, FR-043          |
| TC-004-09 | Minimap and folding                                | FR-027, FR-030, step 6          |
| TC-004-10 | Reading changes nothing                            | NFR-007, BR-002                 |
| TC-004-11 | A1 Note already open                               | BR-001                          |
| TC-004-12 | A2 Follow local links                              | FR-040, BR-005, BR-006          |
| TC-004-13 | A2 Link to a missing heading, a link outside       | BR-005                          |
| TC-004-14 | A3 Linked note does not exist                      | FR-040                          |
| TC-004-15 | A4 Links to web addresses and other files          | BR-007                          |
| TC-004-16 | A5 Switch and close tabs; remembered after restart | FR-018, FR-071                  |
| TC-004-17 | New tab right of the active tab                    | BR-008                          |
| TC-004-18 | A6 More than 100 tabs                              | BR-008                          |
| TC-004-19 | A7 Note changed outside                            | NFR-008, BR-009                 |
| TC-004-20 | A8 Note deleted outside                            | —                               |
| TC-004-21 | A9 Changed outside while a change is not stored    | FR-046, NFR-005, BR-002         |
| TC-004-22 | A10 Note cannot be read                            | —                               |
| TC-004-23 | A11 Diagram cannot be rendered                     | BR-002                          |
| TC-004-24 | Closing a tab stores pending changes               | NFR-005, BR-008                 |
| TC-004-25 | A12 Go to note                                     | FR-076, BR-014                  |
| TC-004-26 | Go to note: matching and order                     | BR-014                          |
| TC-004-27 | A13 No note matches in go to note                  | BR-014                          |
| TC-004-28 | Go to note in a large workspace                    | NFR-018, BR-014                 |
| TC-004-29 | Backlinks and A14 going to one                     | FR-078, BR-015                  |
| TC-004-30 | Outgoing links and broken links                    | FR-079, BR-015                  |
| TC-004-31 | Links follow changes                               | NFR-019, BR-015                 |
| TC-004-32 | Tags of the note and A15 search by tag             | FR-091, FR-092, BR-016          |
| TC-004-33 | A16 Go to line, A17 not a line number              | FR-086, BR-017                  |
| TC-004-34 | A18 Open in the split view                         | FR-096, BR-018                  |
| TC-004-35 | A19 The same note on both sides                    | FR-097, BR-001, BR-018          |
| TC-004-36 | A20 End the split view                             | BR-018                          |
| TC-004-37 | Split view remembered after a restart              | BR-018, BR-008                  |
| TC-004-38 | Formulas and A21 a formula with an error           | FR-098, BR-019, NFR-014         |
| TC-004-39 | Text statistics                                    | FR-090, BR-020                  |
| TC-004-40 | Front matter and note attributes shown             | BR-004, BR-010                  |

## TC-004-01: Open a note in a tab

**Flow:** Main success scenario  
**Preconditions:** The workspace `notes` is open with `Home` in the only tab. It holds `Plan.md`, which was never opened.

| Step | Action                                 | Expected result                                                                                   |
|------|----------------------------------------|---------------------------------------------------------------------------------------------------|
| 1    | Choose `Plan` in the note overview.    | `Plan` opens in a new tab in the facade and is the active tab; `Home` stays open in its tab.      |
| 2    | Look at the note overview.             | `Plan` is listed first, `Home` second.                                                            |

## TC-004-02: Markdown is rendered as GitHub shows it

**Flow:** Step 3, BR-004, C-003  
**Preconditions:** The note `Showcase.md` holds headings of levels 1 to 3, a paragraph with bold, italic, strikethrough, and inline code, a bulleted and a numbered list, a checklist with one checked and one open item, a table with three columns, a quote, a bare web address, and a code block marked `rust`.

| Step | Action                                                   | Expected result                                                                                     |
|------|----------------------------------------------------------|-----------------------------------------------------------------------------------------------------|
| 1    | Open `Showcase`.                                         | Every element is shown as GitHub shows the same file: headings, emphasis, strikethrough, lists, checklist with its marks, table, quote, and the bare address as a link. |
| 2    | Look at the code block.                                  | It is highlighted as Rust in the colors of the active theme.                                        |
| 3    | Switch the theme from Light to Dark (UC-010).            | The highlighted code changes to the colors of the Dark theme.                                       |

## TC-004-03: AsciiDoc is rendered as GitHub shows it

**Flow:** Step 3, BR-004, C-004, FR-004  
**Preconditions:** The note `Guide.adoc` holds a document title, sections of levels 1 and 2, a paragraph with bold and italic, a bulleted list, a table, an admonition `NOTE:`, an image with a caption, and a source block marked `python`.

| Step | Action           | Expected result                                                                                |
|------|------------------|------------------------------------------------------------------------------------------------|
| 1    | Open `Guide`.    | Every element is shown as GitHub shows the same file, the image with its caption; the code is highlighted as Python. |

## TC-004-04: Diagrams in the colors of the theme, offline

**Flow:** Step 3, BR-004  
**Preconditions:** The note `Diagrams.md` holds a PlantUML sequence diagram and a Mermaid flowchart as code blocks. The machine has no network connection. The theme is Light.

| Step | Action                                     | Expected result                                                          |
|------|--------------------------------------------|--------------------------------------------------------------------------|
| 1    | Open `Diagrams`.                           | Both diagrams are shown as pictures, in the colors of the Light theme.   |
| 2    | Switch the theme to Nord (UC-010).         | Both diagrams are shown again in the colors of the Nord theme.           |

## TC-004-05: Diagram rendering time

**Flow:** BR-004, NFR-004  
**Preconditions:** The note `Big Diagram.md` holds 50 paragraphs of text and a Mermaid diagram of 200 elements.

| Step | Action                         | Expected result                                                         |
|------|--------------------------------|-------------------------------------------------------------------------|
| 1    | Open `Big Diagram`.            | The text is readable at once; the diagram appears within 2 seconds.     |

## TC-004-06: Images, enlarged; web images and offline

**Flow:** Steps 3 and 5, BR-004  
**Preconditions:** The note `Pictures.md` shows `img/local.png` from the workspace and an image from a web address.

| Step | Action                                                    | Expected result                                                            |
|------|-----------------------------------------------------------|----------------------------------------------------------------------------|
| 1    | Open `Pictures` with a network connection.                | Both images are shown.                                                     |
| 2    | Enlarge the local image, then the diagram of `Diagrams`.  | Each is shown enlarged so that its details can be read; closing the enlarged view returns to the note at the same place. |
| 3    | Disconnect the network and open `Pictures` again after a restart. | The local image is shown; in place of the web image a placeholder names its address. |

## TC-004-07: Properties of the active note

**Flow:** Step 4, BR-010  
**Preconditions:** The note `guide/Install.adoc` has 2,048 bytes and was last saved on 2026-11-01 at 09:30.

| Step | Action                  | Expected result                                                                                           |
|------|-------------------------|-----------------------------------------------------------------------------------------------------------|
| 1    | Open `Install`.         | The tool panel shows the place `guide/Install.adoc`, the note format AsciiDoc, the size 2 KB, and the last change 2026-11-01 09:30. |
| 2    | Switch to another tab.  | The tool panel shows the properties of that note.                                                         |

## TC-004-08: Outline, code index, and figure index

**Flow:** Steps 4 and 5  
**Preconditions:** The note `Long.md` has 300 lines with the headings `Intro`, `Setup`, `Setup > Linux`, and `Usage`, three code blocks, and two images and one diagram.

| Step | Action                                   | Expected result                                                                       |
|------|------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Open `Long`.                             | The outline lists the four headings with `Linux` below `Setup`; the code index lists three code blocks; the figure index lists two images and one diagram. |
| 2    | Click `Usage` in the outline.            | The tab scrolls to the heading `Usage`.                                               |
| 3    | Click the third code block in the code index. | The tab scrolls to that code block.                                              |
| 4    | Click the diagram in the figure index.   | The tab scrolls to the diagram.                                                       |

## TC-004-09: Minimap and folding

**Flow:** Steps 4 to 6, FR-027, FR-030  
**Preconditions:** The note `Long.md` of TC-004-08 is open in the facade, scrolled to the top.

| Step | Action                                                        | Expected result                                                         |
|------|---------------------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Look beside the editor.                                       | A minimap of the whole note marks the visible part.                     |
| 2    | Click near the end of the minimap.                            | The tab scrolls to that place; the marked part follows.                 |
| 3    | Fold the section `Setup`.                                     | Its content, `Linux` included, is hidden; `Usage` follows right below the heading `Setup`. |
| 4    | Switch to another tab and back.                               | The section is still folded and the reading position is the same.       |
| 5    | Switch the tab to the source view (UC-005) and fold `Usage`.  | The section is folded in the source view too.                           |
| 6    | Expand both sections.                                         | The full note is shown again.                                           |

## TC-004-10: Reading changes nothing

**Flow:** BR-002, NFR-007  
**Preconditions:** The workspace is a git repository without changes. It holds `Showcase.md`, `Guide.adoc`, and `Broken.md`, whose diagram has an error.

| Step | Action                                                                             | Expected result                     |
|------|------------------------------------------------------------------------------------|-------------------------------------|
| 1    | Open each of the three notes, scroll, fold, follow a link, switch each tab to the source view and back, and close the tabs. | —                |
| 2    | Look at the git state.                                                             | There are no changes; every file is byte for byte as before. |

## TC-004-11: The note is open already

**Flow:** A1, BR-001  
**Preconditions:** `Home` and `Plan` are open in tabs, `Home` is active; `Plan` is scrolled to its middle.

| Step | Action                               | Expected result                                                          |
|------|--------------------------------------|--------------------------------------------------------------------------|
| 1    | Choose `Plan` in the note overview.  | No new tab opens; the tab of `Plan` is active, still scrolled to its middle. |

## TC-004-12: Follow local links

**Flow:** A2, BR-005, BR-006  
**Preconditions:** The note `guide/Index.md` holds the links `[Install](Install.md)`, `[Upgrade](Upgrade)`, `[Home](../Home.md#welcome)`, and `[Below](#usage)`, and a heading `Usage`. The notes `guide/Install.md`, `guide/Upgrade.md`, and `Home.md` with the heading `Welcome` exist.

| Step | Action                                             | Expected result                                                            |
|------|----------------------------------------------------|----------------------------------------------------------------------------|
| 1    | Open `Index` and point at the link `Install`.      | The system shows that it leads to `guide/Install.md` and that Ctrl+click (Cmd+click on macOS) follows it. |
| 2    | Click the link without Ctrl.                       | The cursor is placed in the link; no tab opens.                            |
| 3    | Ctrl+click `Install`.                              | `Install` opens in a new tab.                                              |
| 4    | Go back to `Index` and Ctrl+click `Upgrade`.       | `Upgrade` opens, although the link has no extension.                       |
| 5    | Go back and Ctrl+click `Home`.                     | `Home` opens at the heading `Welcome`.                                     |
| 6    | Go back and Ctrl+click `Below`.                    | `Index` scrolls to its heading `Usage`; no tab opens.                      |
| 7    | Repeat step 3 in the source view of `Index`.       | The tab of `Install` becomes active; no second tab opens.                  |

## TC-004-13: Link to a missing heading, a link outside the workspace

**Flow:** A2, BR-005  
**Preconditions:** The note `Index.md` holds `[A](Home.md#nowhere)` and `[B](../../etc/passwd)`.

| Step | Action             | Expected result                                                                 |
|------|--------------------|---------------------------------------------------------------------------------|
| 1    | Ctrl+click `A`.    | `Home` opens at its start; the status bar says that the heading was not found.  |
| 2    | Ctrl+click `B`.    | Nothing opens; the status bar says that the link leads outside the workspace.   |

## TC-004-14: The linked note does not exist

**Flow:** A3  
**Preconditions:** The note `guide/Index.md` holds the link `[Roadmap](Roadmap.md)`; `guide/Roadmap.md` does not exist.

| Step | Action                             | Expected result                                                                            |
|------|------------------------------------|--------------------------------------------------------------------------------------------|
| 1    | Ctrl+click `Roadmap`.              | The system says that the note does not exist and offers to create it.                      |
| 2    | Accept.                            | The New note dialog (UC-003) opens with the title `Roadmap` and the folder `guide` filled in. |
| 3    | Confirm.                           | `guide/Roadmap.md` is created and opened; following the link again opens it.               |

## TC-004-15: Links to web addresses and other files

**Flow:** A4, BR-007  
**Preconditions:** The note `Links.md` holds two web addresses, a link to `docs/manual.pdf`, which exists, and a link to `docs/missing.pdf`, which does not.

| Step | Action                                    | Expected result                                                                |
|------|-------------------------------------------|--------------------------------------------------------------------------------|
| 1    | Ctrl+click the first web address.         | The system asks whether to open web addresses.                                 |
| 2    | Agree.                                    | The address opens in the default browser.                                      |
| 3    | Ctrl+click the second web address.        | It opens in the browser without asking.                                        |
| 4    | Restart Underline and Ctrl+click a web address. | The system asks again.                                                   |
| 5    | Ctrl+click `manual.pdf`.                  | It opens in the program the system uses for PDF files.                         |
| 6    | Ctrl+click `missing.pdf`.                 | The status bar says that the file does not exist.                              |

## TC-004-16: Switch and close tabs; remembered after a restart

**Flow:** A5, FR-071  
**Preconditions:** `Home`, `Plan`, and `Ideas` are open in this order, `Plan` active; `Ideas` is switched to the source view.

| Step | Action                                         | Expected result                                                           |
|------|------------------------------------------------|---------------------------------------------------------------------------|
| 1    | Choose the tab `Home`.                         | `Home` is active; the tool panel shows its properties.                    |
| 2    | Close the tab `Home`.                          | `Plan`, next to it, is active.                                            |
| 3    | Restart Underline.                             | `Plan` and `Ideas` are open in this order, `Plan` active, `Ideas` in the source view. |
| 4    | Close both tabs.                               | No note is shown; the note overview still lists all notes.                |
| 5    | Restart Underline.                             | No tab is open.                                                           |

## TC-004-17: A new tab is placed right of the active tab

**Flow:** Step 2, BR-008  
**Preconditions:** `A`, `B`, and `C` are open in this order, `A` active.

| Step | Action                               | Expected result                         |
|------|--------------------------------------|-----------------------------------------|
| 1    | Choose `D` in the note overview.     | The tabs are `A`, `D`, `B`, `C`; `D` is active. |

## TC-004-18: More than 100 tabs

**Flow:** A6, BR-008  
**Preconditions:** 100 tabs are open; `Oldest` was used longest ago.

| Step | Action                                      | Expected result                                                             |
|------|---------------------------------------------|-----------------------------------------------------------------------------|
| 1    | Open one more note.                         | It opens in a tab; the tab of `Oldest` is closed and the status bar says so; 100 tabs are open. |

## TC-004-19: The note is changed outside

**Flow:** A7, BR-009, NFR-008  
**Preconditions:** `Plan` is open and scrolled to its heading `Budget`; all changes are stored.

| Step | Action                                                         | Expected result                                                                  |
|------|----------------------------------------------------------------|----------------------------------------------------------------------------------|
| 1    | Add a paragraph above `Budget` with another program.           | Within 1 second the tab shows the new paragraph and stays at `Budget`; the status bar says that the note was changed outside. |
| 2    | Change `Plan` with `git checkout` to an older version.         | Within 1 second the tab shows the older version.                                 |

## TC-004-20: The note is deleted outside

**Flow:** A8  
**Preconditions:** `Old` is open in a tab.

| Step | Action                                     | Expected result                                                                         |
|------|--------------------------------------------|-----------------------------------------------------------------------------------------|
| 1    | Delete `Old.md` with another program.      | The tab stays open with the last content, marked as deleted; `Old` is no longer listed in the note overview. |
| 2    | Restart Underline.                         | No tab of `Old` is open.                                                                |

## TC-004-21: Changed outside while a change is not stored yet

**Flow:** A9, BR-002, FR-046  
**Preconditions:** `Plan` is open in the facade.

| Step | Action                                                                                  | Expected result                                                                                  |
|------|-----------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| 1    | Type `mine` and, within the next second, add `theirs` to `Plan.md` with another program. | The tab keeps `mine`; the status bar says that the note was changed outside and offers to show both versions. |
| 2    | Wait 2 seconds and look at the workspace.                                               | `Plan.md` holds `mine`; a copy next to it, whose title names `Plan` and the time of the change, holds `theirs`. |
| 3    | Accept the offer.                                                                       | Both versions are shown side by side.                                                            |

## TC-004-22: The note cannot be read

**Flow:** A10  
**Preconditions:** `Locked.md` cannot be read by the user. `Binary.md` holds the bytes of an image.

| Step | Action                     | Expected result                                                           |
|------|----------------------------|---------------------------------------------------------------------------|
| 1    | Choose `Locked`.           | The status bar says that the note cannot be read for missing permission; no tab opens. |
| 2    | Choose `Binary`.           | The status bar says that the note holds no readable text; no tab opens.   |
| 3    | Look at both files.        | They are unchanged.                                                       |

## TC-004-23: A diagram cannot be rendered

**Flow:** A11  
**Preconditions:** `Broken.md` holds a paragraph, a Mermaid diagram with a syntax error, and a correct PlantUML diagram.

| Step | Action               | Expected result                                                                               |
|------|----------------------|-----------------------------------------------------------------------------------------------|
| 1    | Open `Broken`.       | The paragraph and the PlantUML diagram are rendered; in place of the Mermaid diagram its source is shown with the error. |
| 2    | Look at `Broken.md`. | It is unchanged.                                                                              |

## TC-004-24: Closing a tab stores pending changes

**Flow:** BR-008, NFR-005  
**Preconditions:** `Plan` is open.

| Step | Action                                                        | Expected result                                       |
|------|---------------------------------------------------------------|-------------------------------------------------------|
| 1    | Type `last words` and close the tab at once.                  | —                                                     |
| 2    | Look at `Plan.md` with another program.                       | It holds `last words`.                                |

## TC-004-25: Go to note

**Flow:** A12, BR-014, FR-076  
**Preconditions:** The workspace `notes` holds `Plan.md`, `travel/Packing.md`, and `Ideas.md`; "Ideas" is open with the focus in its text; the input mode is Normal.

| Step | Action                                   | Expected result                                                                    |
|------|------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Press Ctrl+P and type `pack`.            | Go to note lists "Packing" with its folder `travel`, chosen.                       |
| 2    | Press Enter.                             | "Packing" opens in a new tab right of "Ideas" and is listed first in the note overview. |
| 3    | Press Ctrl+P, type `ide`, and press Enter. | The tab of "Ideas" becomes the active tab (A1).                                  |

## TC-004-26: Go to note: matching and order

**Flow:** BR-014  
**Preconditions:** `Plan.md` (opened yesterday), `Release-Plan.md` (opened today), `travel/Plane-Tickets.md`, and `Explanation.md` (never opened); `Über-uns.md`.

| Step | Action                          | Expected result                                                                       |
|------|---------------------------------|---------------------------------------------------------------------------------------|
| 1    | Type `plan`.                    | "Plan" and "Plane Tickets" (title starts with it) come first, the recently opened first, then "Release Plan" (a word starts with it), then "Explanation". |
| 2    | Type `travel tick`.             | Only "Plane Tickets" (folder and title).                                              |
| 3    | Type `uber`, then `ÜBER`.       | Nothing for `uber`; "Über uns" for `ÜBER`.                                            |
| 4    | Empty the field.                | The notes are listed in the order of the note overview.                               |

## TC-004-27: No note matches in go to note

**Flow:** A13  
**Preconditions:** No note title holds "Budget".

| Step | Action                                        | Expected result                                                    |
|------|-----------------------------------------------|--------------------------------------------------------------------|
| 1    | Press Ctrl+P and type `Budget 2027`.          | Go to note says no note matches and offers to create "Budget 2027". |
| 2    | Accept.                                       | The dialog to create a note opens with the title "Budget 2027" (UC-003). |

## TC-004-28: Go to note in a large workspace

**Flow:** BR-014, NFR-018  
**Preconditions:** A workspace of 10,000 notes.

| Step | Action                                                      | Expected result                                      |
|------|-------------------------------------------------------------|------------------------------------------------------|
| 1    | Type 20 different texts into go to note, measuring each key. | The list follows within 200 milliseconds of each key. |

## TC-004-29: Backlinks and going to one

**Flow:** A14, BR-015, FR-078  
**Preconditions:** `Plan.md` has the heading "Goals". `Ideas.md` holds `[the plan](Plan.md)` on line 4 and `[goals](Plan.md#goals)` on line 9; `travel/Trip.adoc` holds `link:../Plan.md[Plan]` on line 2; `Code.md` holds `` `[x](Plan.md)` `` in a code span; `Plan.md` holds `[top](#goals)`.

| Step | Action                                            | Expected result                                                                       |
|------|---------------------------------------------------|---------------------------------------------------------------------------------------|
| 1    | Open "Plan" and look at the backlinks.            | The section says 3; "Ideas" with lines 4 and 9 and "Trip" with line 2, each with the link marked; "Code" and "Plan" itself are not listed. |
| 2    | Choose the backlink of line 9 of "Ideas".         | "Ideas" opens with the cursor on the link in line 9.                                 |

## TC-004-30: Outgoing links and broken links

**Flow:** BR-015, FR-079, A14  
**Preconditions:** `Hub.md` holds, in this order, `[Plan](Plan.md)`, `[Gone](Gone.md)`, `[Goals](Plan.md#nothing)`, `[Logo](img/logo.png)`, and `<https://example.org>`; `Plan.md` exists without a heading "nothing"; `img/logo.png` exists; `Gone.md` does not.

| Step | Action                                         | Expected result                                                                        |
|------|------------------------------------------------|----------------------------------------------------------------------------------------|
| 1    | Open "Hub" and look at the outgoing links.     | The section says 5 and lists them in this order; "Gone" and "Goals" are marked as broken links; the web address is not marked. |
| 2    | Choose "Plan".                                 | The cursor is on the link to "Plan" in "Hub".                                          |
| 3    | Choose "Plan" with the Ctrl key held.           | "Plan" opens (A2).                                                                     |

## TC-004-31: Links follow changes

**Flow:** BR-015, NFR-019  
**Preconditions:** As TC-004-29, "Plan" is open.

| Step | Action                                                               | Expected result                                                  |
|------|----------------------------------------------------------------------|------------------------------------------------------------------|
| 1    | Add `[plan](Plan.md)` to `Log.md` with another program.              | Within 2 seconds "Log" appears among the backlinks of "Plan".    |
| 2    | Open "Ideas" in another tab, delete its link of line 4, and go back to "Plan" without waiting for autosave. | The backlinks of "Ideas" show only line 9. |
| 3    | Create `Gone.md`, then look at the outgoing links of "Hub" (TC-004-30). | "Gone" is no longer marked as broken.                         |

## TC-004-32: Tags of the note and search by tag

**Flow:** BR-016, A15, FR-091, FR-092  
**Preconditions:** `Trip.md` has the front matter `tags: [Travel]` and the text "Ferry to #hydra and #Project/alpha, issue #12, C#, `#code`, [link](#ferry)"; `Guide.adoc` has `:tags: travel, docs` and the text "a #highlight# word".

| Step | Action                                     | Expected result                                                          |
|------|--------------------------------------------|--------------------------------------------------------------------------|
| 1    | Open "Trip" and look at the tags.          | travel, hydra, project/alpha; not 12, not C#, not code, not ferry.      |
| 2    | Look at the facade.                        | "#hydra" is shown as plain text.                                         |
| 3    | Open "Guide" and look at the tags.         | travel and docs; "highlight" is no tag.                                  |
| 4    | Choose the tag "travel".                   | The search of the workspace finds "Trip" and "Guide" (UC-008 A8).        |

## TC-004-33: Go to line

**Flow:** A16, A17, BR-017, FR-086  
**Preconditions:** `Long.md` has 300 lines; line 120 is in a paragraph that starts on line 118, in the section "Results", which is folded. The note is open in the source view.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Press Ctrl+G.                                  | The system asks for a line number and shows that the note has 300 lines. |
| 2    | Enter `120:8`.                                 | "Results" expands; the cursor is in line 120 at column 8, in view.      |
| 3    | Switch to the facade, press Ctrl+G, enter `120`. | The cursor is in the paragraph that holds line 120, at the start of that line. |
| 4    | Press Ctrl+G and enter `999`; then press Ctrl+G and enter `5:400`, line 5 being 20 characters long. | The cursor goes to line 300, then to the end of line 5. |
| 5    | Press Ctrl+G and enter `abc`.                  | The system says a line number is expected; the cursor stays.           |

## TC-004-34: Open in the split view

**Flow:** A18, BR-018, FR-096  
**Preconditions:** "Plan" and "Ideas" are open in the left pane; "Plan" is active.

| Step | Action                                                    | Expected result                                                            |
|------|-----------------------------------------------------------|----------------------------------------------------------------------------|
| 1    | Open "Trip" to the side from the note overview.           | The middle shows two equal panes; "Plan" and "Ideas" stay left; "Trip" opens right; the right pane is active and the tool panel shows "Trip". |
| 2    | Choose "Packing" in the note overview.                    | "Packing" opens in the right pane.                                        |
| 3    | Click into "Plan" and choose "Log" in the note overview.  | "Log" opens in the left pane.                                              |
| 4    | Drag the border between the panes.                        | The panes change their width.                                              |

## TC-004-35: The same note on both sides

**Flow:** A19, BR-001, BR-018, FR-097  
**Preconditions:** "Plan" is open in the facade in the left pane; no split view.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Press Ctrl+\ in "Plan".                        | "Plan" is shown in the left pane in the facade and in the right pane in the source view. |
| 2    | Type `hello` in the source view.               | The facade shows `hello` at once (UC-005 BR-016).                       |
| 3    | Choose "Plan" in the note overview while the right pane is active. | Its tab in the right pane is the active tab; no third tab opens. |

## TC-004-36: End the split view

**Flow:** A20, BR-018  
**Preconditions:** Left pane: "Plan", "Ideas"; right pane: "Plan", "Trip".

| Step | Action                                     | Expected result                                                                 |
|------|--------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Ask to end the split view.                 | One pane with "Plan", "Ideas", "Trip" in this order.                           |
| 2    | Split again with "Trip" to the side, then close "Trip" in the right pane. | The split view ends; the left pane is unchanged.          |

## TC-004-37: Split view remembered after a restart

**Flow:** BR-018, BR-008  
**Preconditions:** Left pane: "Plan" (active), "Ideas"; right pane: "Plan" in the source view (active), "Trip"; 99 tabs open in the left pane in another try.

| Step | Action                                         | Expected result                                                         |
|------|------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Restart Underline.                             | Both panes come back with their tabs, editing views, and active tabs; the left pane is active. |
| 2    | With 99 tabs left and 1 right, open one more note. | The tab used longest ago is closed (A6): 100 tabs in both panes together. |

## TC-004-38: Formulas

**Flow:** BR-019, A21, FR-098, NFR-014  
**Preconditions:** The network is off. `Math.md` holds "Energy $E=mc^2$ and price 5$ or 6$", a block `$$\sum_{i=1}^n i$$`, a code block of the language `math` with `\sqrt{2}`, and `$$\frac{1}{$$`. `Math.adoc` sets `:stem: latexmath` and holds `stem:[a^2]` and `latexmath:[b^2]`. The theme is Dark.

| Step | Action                     | Expected result                                                                         |
|------|----------------------------|-----------------------------------------------------------------------------------------|
| 1    | Open `Math.md`.            | `E=mc^2` is typeset inline; "5$ or 6$" stays text; the sum and the root are typeset as blocks in the colors of Dark; `\frac{1}{` is shown as its source with the error. |
| 2    | Switch to the source view. | The markup is shown.                                                                    |
| 3    | Open `Math.adoc`.          | Both formulas are typeset inline.                                                       |
| 4    | Look at both files.        | They are unchanged.                                                                     |

## TC-004-39: Text statistics

**Flow:** BR-020, FR-090  
**Preconditions:** `Count.md` holds `# Title`, the paragraph "It's a well-known [fact](https://example.org/a-b-c)." and a code block with `let x = 1;`.

| Step | Action                                             | Expected result                                                         |
|------|----------------------------------------------------|-------------------------------------------------------------------------|
| 1    | Open "Count" and look at the note properties.      | 8 words (Title, It's, a, well-known, fact, let, x, 1), the number of characters of the text shown without markup and link address, and a reading time of 1 minute. |
| 2    | Select "well-known fact".                          | The note properties also show 2 words and 15 characters of the selection. |
| 3    | Add 400 words.                                     | Within 1 second the reading time shows 3 minutes.                       |

## TC-004-40: Front matter and note attributes shown

**Flow:** BR-004, BR-010  
**Preconditions:** `Plan.md` starts with a front matter holding `status: draft`.

| Step | Action                              | Expected result                                                          |
|------|-------------------------------------|--------------------------------------------------------------------------|
| 1    | Open "Plan".                        | The facade shows a table with "status" and "draft" at the start; the note properties show "status" as a note attribute (UC-024). |
