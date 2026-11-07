# Test Cases: Export Note

**Use Case:** [UC-009 Export Note](../use_cases/UC-009-export-note.md)  
**Requirements:** [FR-014, FR-015, FR-016, NFR-006, NFR-011, NFR-013, NFR-014, NFR-015](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements              |
|-----------|------------------------------------------------------|---------------------------|
| TC-009-01 | Main success scenario: Markdown to AsciiDoc          | FR-014, BR-002, BR-007    |
| TC-009-02 | AsciiDoc to Markdown, links of the export            | FR-014, BR-002            |
| TC-009-03 | HTML that stands alone                               | FR-015, BR-003, BR-009    |
| TC-009-04 | HTML in the reader's dark mode                       | BR-003                    |
| TC-009-05 | PDF with pages                                       | FR-016, BR-004, BR-009    |
| TC-009-06 | Links in HTML and PDF                                | BR-005                    |
| TC-009-07 | Unsaved changes and folded sections                  | BR-001                    |
| TC-009-08 | A1 User cancels                                      | —                         |
| TC-009-09 | A2 Content without counterpart                       | BR-002                    |
| TC-009-10 | A3 Diagram or image cannot be shown                  | —                         |
| TC-009-11 | A4 Export cannot be written                          | BR-007, NFR-006           |
| TC-009-12 | A5 Export onto the note itself                       | —                         |
| TC-009-13 | A6 Export into the workspace                         | —                         |
| TC-009-14 | Offline, nothing leaves the computer                 | BR-006, NFR-014, NFR-015  |
| TC-009-15 | Replacing an existing file                           | BR-007                    |
| TC-009-16 | Keyboard alone                                       | BR-008, NFR-011           |
| TC-009-17 | The same on every platform                           | NFR-013                   |

## TC-009-01: Export Markdown to AsciiDoc

**Flow:** Main success scenario, BR-002, BR-007  
**Preconditions:** `Getting-Started.md` holds a heading, a paragraph with bold and italic text and a link to a web address, a nested list, a checklist, a table, a code block in Rust, a Mermaid diagram, an image `img/shot.png`, and a quote. Nothing was exported before.

| Step | Action                                                        | Expected result                                                                 |
|------|---------------------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Choose Export → AsciiDoc in the context menu of the note.      | The file dialog proposes `Getting-Started.adoc` in the home folder.             |
| 2    | Choose the folder `~/out` and confirm.                         | The status bar says the note was exported to `~/out/Getting-Started.adoc` and offers to open it. |
| 3    | Open the file in Underline as a note and in Asciidoctor.       | Every block shows with the same meaning; the image link leads to the image in the workspace. |
| 4    | Export the note again.                                         | The file dialog proposes `~/out`.                                              |
| 5    | Look at `Getting-Started.md`.                                  | It is unchanged.                                                               |

## TC-009-02: Export AsciiDoc to Markdown

**Flow:** BR-002  
**Preconditions:** `guide/Setup.adoc` holds sections, `xref:Install.adoc[Install]`, `image::img/a.png[A]`, a PlantUML block, and a source block.

| Step | Action                                     | Expected result                                                                     |
|------|--------------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Export to Markdown into the top folder of another directory `~/out`. | `~/out/Setup.md` holds headings, the code block, a `plantuml` block, and an image. |
| 2    | Look at its link and image places.         | They lead from `~/out` to `Install.adoc` and `img/a.png` in the workspace.         |
| 3    | Open it on GitHub or in a Markdown viewer. | It shows as the note does in Underline.                                            |

## TC-009-03: HTML that stands alone

**Flow:** FR-015, BR-003, BR-009  
**Preconditions:** `Trip.md` holds an image, a PlantUML diagram, a Mermaid diagram, a highlighted code block, and a table.

| Step | Action                                                       | Expected result                                                                 |
|------|--------------------------------------------------------------|---------------------------------------------------------------------------------|
| 1    | Export to HTML as `~/out/Trip.html`.                          | Only `Trip.html` is written; no folder of files.                               |
| 2    | Copy the file to another computer without a network and open it in a browser. | Images, both diagrams, highlighted code, and the table show; the tab of the browser says "Trip". |
| 3    | Select and copy a sentence.                                    | The text is copied as text.                                                    |
| 4    | Search the page for a word with the browser.                   | It is found.                                                                   |

## TC-009-04: HTML in the reader's dark mode

**Flow:** BR-003  
**Preconditions:** Underline uses the theme Dracula. `Trip.html` was exported.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Open `Trip.html` with the system in light mode. | It looks light and plain, not in Dracula colors.    |
| 2    | Switch the system to dark mode and reload.      | It looks dark, with readable text and code.          |

## TC-009-05: PDF with pages

**Flow:** FR-016, BR-004, BR-009  
**Preconditions:** The region of the system is Germany. `Long.md` holds 8 pages of text, headings throughout, a tall diagram, a large image, and the words "Grüße" and "日本語".

| Step | Action                                  | Expected result                                                                     |
|------|-----------------------------------------|-------------------------------------------------------------------------------------|
| 1    | Export to PDF.                           | The status bar reports the PDF.                                                    |
| 2    | Open it in a PDF reader.                 | A4 pages, portrait, with margins and page numbers at the foot; the document title is "Long". |
| 3    | Look at the foot of every page.          | No heading stands there without the text that follows it.                         |
| 4    | Look at the diagram and the image.        | Neither is split across pages; the image larger than a page fits one page.         |
| 5    | Search for "Grüße" and "日本語".          | Both are found and shown correctly.                                               |
| 6    | Switch the region to the United States and export again. | The pages are US Letter.                                     |

## TC-009-06: Links in HTML and PDF

**Flow:** BR-005  
**Preconditions:** `Plan.md` holds `[site](https://example.org)`, `[goals](#goals)` with a section Goals, and `[ideas](Ideas.md)`.

| Step | Action                                 | Expected result                                                         |
|------|----------------------------------------|-------------------------------------------------------------------------|
| 1    | Export to HTML and open it.             | "site" opens the web address, "goals" jumps to Goals, "ideas" is plain text. |
| 2    | Export to PDF and open it.              | The same.                                                               |

## TC-009-07: Unsaved changes and folded sections

**Flow:** BR-001  
**Preconditions:** `Plan.md` is open; the section "Details" is folded; the user has just typed "new line" and autosave has not run.

| Step | Action                    | Expected result                                                  |
|------|---------------------------|------------------------------------------------------------------|
| 1    | Export to HTML.           | The export holds "new line" and the whole section "Details".     |

## TC-009-08: User cancels

**Flow:** A1  
**Preconditions:** `Plan.md` exists.

| Step | Action                                        | Expected result                          |
|------|-----------------------------------------------|------------------------------------------|
| 1    | Press Ctrl+Shift+E and Escape.                | Nothing is written.                      |
| 2    | Choose Export → PDF and cancel the file dialog. | Nothing is written; no report.         |

## TC-009-09: Content without counterpart

**Flow:** A2, BR-002  
**Preconditions:** `Guide.adoc` holds a `NOTE:` admonition on line 5 and a table with a cell spanning two columns on line 12.

| Step | Action                                   | Expected result                                                                   |
|------|------------------------------------------|-----------------------------------------------------------------------------------|
| 1    | Export to Markdown.                       | The report says 2 blocks were only approximated and offers to show them.         |
| 2    | Ask to show them.                         | Line 5 and line 12 are listed.                                                   |
| 3    | Open the export.                          | The admonition is a quote starting with "**Note:**"; the table keeps its text.   |

## TC-009-10: Diagram or image cannot be shown

**Flow:** A3  
**Preconditions:** `Broken.md` holds a Mermaid diagram with a syntax error and an image `img/missing.png` with the text "Plan", which does not exist.

| Step | Action                     | Expected result                                                                      |
|------|----------------------------|--------------------------------------------------------------------------------------|
| 1    | Export to HTML.            | The export holds the diagram source as code and "Plan" in place of the image.        |
| 2    | Look at the status bar.    | It names the diagram and the image as missing in the export.                         |

## TC-009-11: Export cannot be written

**Flow:** A4, BR-007, NFR-006  
**Preconditions:** `~/locked` cannot be written; `~/out/Plan.pdf` exists from an earlier export; the disk of `~/out` is full.

| Step | Action                                       | Expected result                                                         |
|------|----------------------------------------------|-------------------------------------------------------------------------|
| 1    | Export to PDF into `~/locked`.               | The status bar says the note could not be exported and why.             |
| 2    | Export to PDF as `~/out/Plan.pdf` and agree to replace it. | The status bar reports the failure; the earlier `Plan.pdf` is unchanged. |

## TC-009-12: Export onto the note itself

**Flow:** A5  
**Preconditions:** `Plan.adoc` and `Ideas.md` exist; `Ideas` is open in a tab.

| Step | Action                                              | Expected result                                                  |
|------|-----------------------------------------------------|------------------------------------------------------------------|
| 1    | Export `Plan` to Markdown, choosing `Ideas.md` in the workspace. | Underline says a note cannot be replaced by an export and asks for another place. |
| 2    | Look at `Ideas.md`.                                  | It is unchanged.                                                 |

## TC-009-13: Export into the workspace

**Flow:** A6  
**Preconditions:** `Plan.md` is in the top folder of the workspace.

| Step | Action                                             | Expected result                                                  |
|------|----------------------------------------------------|------------------------------------------------------------------|
| 1    | Export to AsciiDoc as `archive/Plan.adoc` in the workspace. | The note overview lists the new note "Plan" in `archive`.   |
| 2    | Look at `Plan.md`.                                  | It is unchanged.                                                 |

## TC-009-14: Offline, nothing leaves the computer

**Flow:** BR-006, NFR-014, NFR-015  
**Preconditions:** The computer has no network connection. `Trip.md` holds a PlantUML and a Mermaid diagram and a local image.

| Step | Action                                             | Expected result                                       |
|------|----------------------------------------------------|-------------------------------------------------------|
| 1    | Export to the other note format, to HTML, and to PDF. | All three succeed, with diagrams and the image.     |
| 2    | Watch the network connections of Underline during step 1 with a network monitor. | No connection is made.  |

## TC-009-15: Replacing an existing file

**Flow:** BR-007  
**Preconditions:** `~/out/Plan.html` exists.

| Step | Action                                            | Expected result                                  |
|------|---------------------------------------------------|--------------------------------------------------|
| 1    | Export `Plan` to HTML as `~/out/Plan.html`.       | The file dialog asks whether to replace it.      |
| 2    | Decline.                                           | The old file is unchanged.                      |
| 3    | Export again and agree.                            | `~/out/Plan.html` holds the new export.          |

## TC-009-16: Keyboard alone

**Flow:** BR-008, NFR-011  
**Preconditions:** `Plan.md` is open in the active tab.

| Step | Action                                                  | Expected result                                         |
|------|---------------------------------------------------------|---------------------------------------------------------|
| 1    | Press Ctrl+Shift+E.                                      | Underline asks for AsciiDoc, HTML, or PDF.              |
| 2    | Choose PDF with the arrow keys and Enter.                | The file dialog opens with `Plan.pdf`.                  |
| 3    | Confirm with Enter.                                      | The PDF is written.                                     |

## TC-009-17: The same on every platform

**Flow:** NFR-013  
**Preconditions:** The same workspace on Linux, Windows, and macOS.

| Step | Action                                            | Expected result                                                  |
|------|---------------------------------------------------|------------------------------------------------------------------|
| 1    | Export `Trip.md` to all three kinds on each system. | The exports are alike; each system shows its own file dialog. |
