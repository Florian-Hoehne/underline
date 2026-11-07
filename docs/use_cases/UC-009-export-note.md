# Use Case: Export Note

## Overview

**Use Case ID:** UC-009  
**Use Case Name:** Export Note  
**Primary Actor:** User  
**Goal:** The user has a copy of a note in the other note format, as a standalone HTML file, or as a PDF file, with its diagrams and images, at a place of their choice.  
**Trigger:** The user asks to export a note.  
**Status:** Reviewed  

**Requirements:** [FR-014, FR-015, FR-016, NFR-006, NFR-011, NFR-013, NFR-014, NFR-015](../requirements.md)

## Preconditions

- A workspace is open (UC-001 or UC-002).
- The note is listed in the note overview.

## Main Success Scenario

1. The user asks to export a note, from the context menu of the note in the note overview or of its tab, choosing the other note format, HTML, or PDF; or with Ctrl+Shift+E (Cmd+Shift+E on macOS) for the note in the active tab, which first asks for one of the three.
2. The system asks where to save the export with the file dialog of the system, proposing the title of the note with the extension of the export and the folder of the last export, or the home folder the first time.
3. The user chooses the place and confirms.
4. The system turns the note, as its tab shows it with changes not yet saved, into the export (BR-002 to BR-004), with its diagrams and images, and writes it to the chosen place.
5. The system reports in the status bar that the note was exported and where, with an offer to open the file with the program the system uses for it.

## Alternative Flows

### A1: User cancels

**Trigger:** The user cancels the choice of the export or the file dialog (step 1, step 3)  
**Flow:**

1. The system writes nothing.
2. Use case ends.

### A2: Content without counterpart

**Trigger:** The note holds content the other note format has no markup for, such as an AsciiDoc admonition or a table cell spanning several columns in Markdown (step 4, export to the other note format)  
**Flow:**

1. The system writes the closest counterpart (BR-002).
2. After the export, the status bar report says how many blocks were only approximated, and the system offers to show them.
3. When the user asks, the system lists those blocks with their line in the note.
4. Use case ends.

### A3: Diagram or image cannot be shown

**Trigger:** A diagram cannot be rendered, or an image cannot be read or, for a web image, loaded (step 4, export to HTML or PDF)  
**Flow:**

1. The system writes the export with the diagram's source as a code block, or with the image's alternative text, in its place.
2. The status bar report says which diagrams and images are missing in the export.
3. Use case ends.

### A4: Export cannot be written

**Trigger:** The chosen place cannot be written, for example for missing permission or a full disk (step 4)  
**Flow:**

1. The system reports in the status bar that the note could not be exported and why.
2. Nothing is left at the chosen place: an existing file there is as it was.
3. Use case ends.

### A5: Export onto the note itself

**Trigger:** The chosen place is the file of the note or of another note open in a tab (step 3)  
**Flow:**

1. The system says that a note cannot be replaced by an export and asks for another place.
2. Use case continues at step 2.

### A6: Export into the workspace

**Trigger:** An export to the other note format is saved in a folder of the workspace (step 4)  
**Flow:**

1. The system writes it as for any other place; it is a new note of the workspace and is listed in the note overview.
2. Use case continues at step 5.

## Postconditions

### Success Postconditions

- The export is at the chosen place, complete.
- The note is unchanged, and so is every other note (A6 adds one).

### Failure Postconditions

- The note is unchanged and nothing is written at the chosen place.

## Business Rules

### BR-001: What is exported

The note is exported as its tab shows it, with changes not yet saved; a note that is not open is exported as it is stored. The export holds the whole note, folded sections included (UC-004 BR-012).

### BR-002: Export to the other note format

The note is written in the markup of the other note format (FR-014): headings, paragraphs, emphasis, lists, checklists, tables, links, images, code blocks, diagrams, quotes, and breaks keep their meaning. Where the other format has no counterpart, the closest one is written, such as an admonition as a quote starting with its label in bold, and the block counts as approximated (A2). The relative places of links and images are changed so they lead to the same notes and files from the place of the export, as for a moved note (UC-006 BR-005).

### BR-003: Export to HTML

The HTML file stands alone (FR-015): images and diagrams are inside it, so it can be sent as one file and read in any browser without a network connection. It looks light and plain, as GitHub shows a note, and dark where the reader's browser asks for a dark look, whatever the theme of Underline. Code is highlighted as in Underline (ADR-010).

### BR-004: Export to PDF

The PDF file has the paper size of the system's region, A4 or US Letter, in portrait with usual margins and page numbers at the foot of every page (FR-016). It looks light, whatever the theme of Underline. Headings do not stand alone at the foot of a page, and an image or diagram is not split across pages; one larger than a page is scaled down to fit. There are no settings.

### BR-005: Links in HTML and PDF

Web addresses and links to headings of the same note stay links. A link to another note or file of the workspace keeps its text but is no link in the export, since the export stands alone.

### BR-006: Offline and local

Exporting needs no network connection (NFR-014), and nothing of the note leaves the computer (NFR-015), except web images the note itself shows, which are loaded as when reading the note (ADR-013).

### BR-007: Place of the export

The file dialog proposes the title of the note as file name: `Getting-Started.adoc`, `Getting-Started.html`, `Getting-Started.pdf`. When the chosen file exists, the file dialog of the system asks whether to replace it. The export is written whole or not at all (NFR-006); a file it replaces is as it was until the export is complete.

### BR-008: Keyboard

Ctrl+Shift+E (Cmd+Shift+E on macOS) asks for the kind of export of the note in the active tab; the arrow keys and Enter choose it (NFR-011).

### BR-009: Text and title

In the HTML and the PDF export, text stays text: it can be selected, copied, and searched, and every character the note shows appears, also umlauts and other scripts. Both carry the title of the note as the title of the document.

### BR-010: One note, unchanged

One note is exported at a time; exporting several notes or a folder at once is not part of this use case (OQ-122). Exporting never changes or replaces the note; turning a note into the other note format in place is not part of it either (OQ-123).

