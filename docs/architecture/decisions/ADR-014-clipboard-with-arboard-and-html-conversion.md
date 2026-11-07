# ADR-014: The clipboard through arboard, with HTML converted to and from markup

**Status:** Accepted
**Date:** 2026-11-07
**Requirements:** FR-031, FR-032, NFR-013, NFR-015, UC-005 A17, A18, BR-011, BR-014

## Context

Pasting into the facade keeps the formatting the note format can hold (UC-005 A17), a pasted image becomes a PNG file
next to the note (A18), copying from the facade puts both markup and formatted text on the clipboard, and copy on
select needs to read what was on the clipboard before a selection (BR-011). egui hands an application only plain text
on paste and writes only plain text on copy; it offers no HTML and no images. All of this must work the same on
Linux, Windows, and macOS (NFR-013), and nothing leaves the machine (NFR-015).

## Decision

- **Clipboard access.** The editor crate reads and writes the system clipboard through `arboard` 3 (MIT or Apache-2.0),
  which egui's window integration already uses: plain text, HTML with a plain-text alternative, and images. Access
  sits behind a small `Clipboard` trait, so the views are tested with a clipboard in memory.
- **Pasting.** On a paste in the facade, HTML on the clipboard is converted to markup of the note format; without
  HTML, or with `Ctrl+Shift+V` (`Cmd+Shift+V`), the plain text is pasted as typed text. The source view always pastes
  plain text. When the clipboard holds an image and no text, the image is encoded as PNG and stored next to the note
  (`Plan-2026-11-06-1430.png`, then `-2`, ...) and inserted like a dropped image. When the clipboard's text is exactly
  what Underline itself copied as markup, that markup is pasted as it is.
- **HTML to markup.** An own converter in the facade crate reads the HTML with `scraper` (html5ever, the HTML parser of
  Servo; ISC) and writes headings, paragraphs, bold, italic, code, links, images, lists, quotes, code blocks, and
  tables as Markdown or AsciiDoc; everything else (colors, fonts, layout) becomes plain text. Script and style
  elements are dropped.
- **Markup to HTML.** Copying from the facade writes HTML made from the facade's blocks of the selection (their runs
  and styles), so other programs receive the formatting; the plain-text alternative is the markup of the selection.
- **Copy on select.** When a selection is made, the clipboard's content is read once and kept; the selection is then
  written to the clipboard. Pasting over that selection pastes the kept content (BR-011). A switch turns copy on
  select off until the settings of UC-010 hold it.

## Consequences

- Rich pasting and images work on all three platforms without platform code of our own.
- The converters cover the elements the note formats can hold; unusual HTML loses its formatting but keeps its text.
- On Linux the clipboard belongs to the program that wrote it; Underline keeps its clipboard content alive while it
  runs, as `arboard` does.
- **Alternatives considered:** `htmd` for HTML to Markdown (no AsciiDoc, and two converters to keep in step);
  platform clipboard crates per system (more code for the same result); waiting for egui to support rich clipboard
  content (not planned).
