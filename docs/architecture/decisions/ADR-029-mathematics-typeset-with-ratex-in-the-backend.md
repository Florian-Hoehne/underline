# ADR-029: Typeset mathematics with RaTeX in the backend, as SVG for the facade and both exports

**Status:** Proposed
**Date:** 2026-11-13
**Requirements:** C-003, NFR-013, NFR-014, NFR-002, C-008, FR-098

## Context

Formulas written as GitHub reads them are shown as typeset mathematics in the facade and in the exports (FR-098;
UC-004 BR-019, UC-005, UC-009; OQ-222).

- **Markdown:** `$…$`, `` $`…`$ ``, `$$…$$` and fenced ```` ```math ```` blocks.
- **AsciiDoc:** `latexmath:[…]`, `stem:[…]` while the note sets `:stem: latexmath`, and `[latexmath]` or `[stem]`
  blocks. AsciiMath is shown as its source.
- **A formula with an error** shows its source and the reason (UC-004 A21).

The constraints are:

- The formula looks the same on WebKitGTK, WebView2 and WKWebView (NFR-013).
- It works offline (NFR-014). The webview loads nothing from the network (ADR-023).
- It looks as it does on GitHub and GitLab (C-003).
- Typing stays within 16 ms (NFR-002).
- The library fits the MIT license (C-008).

The PDF export is set by Typst as a library (ADR-015). It needs the same formulas.

RaTeX is a pure-Rust port of KaTeX (MIT). It is already linked: `merman` with the feature `complete-svg` (ADR-009)
turns on its `math` feature. `Cargo.lock` therefore holds `ratex-parser`, `ratex-layout`, `ratex-types`, `ratex-svg`
and `ratex-font` 0.1.15. The KaTeX fonts (OFL-1.1) are embedded through `ratex-svg/embed-fonts`.

## Decision

- **Parsing, in the WASM core (`underline-markup`):**
  - Markdown: comrak's options `math_dollars` and `math_code` are turned on. These are options, not cargo
    features, so the WASM file does not grow by a crate.
  - A paragraph that holds only one display formula is a block formula.
  - AsciiDoc: the own parser (ADR-003) learns the inline macros and the blocks named above.
  - The facade gets a block kind and an inline run "formula" (source, display or inline).
  - Comrak's math is inline, so `$$` never reaches beyond its paragraph and does not force a full parse in the
    incremental reparse. A part of an AsciiDoc note parsed alone is told whether the header sets `:stem:`.
- **Typesetting, in the backend:**
  - `ratex-parser`, `ratex-layout`, `ratex-types` and `ratex-svg` are used at exactly the version merman uses
    (0.1.15, pinned with `=`). They are updated together with merman, so the build holds one copy.
  - Each formula goes through parse, layout (display or text style, the theme's text colour) and SVG. Glyphs are
    embedded as paths, so the SVG needs no fonts and looks alike on every platform.
  - The renderer sits behind a `MathRenderer` trait, like the diagram renderer.
  - Formulas render on their own queue, not behind PlantUML. They follow the cache, the revision tags and the
    recolouring of ADR-009 and ADR-025. The cache key is the source, display or inline, and the theme.
  - Rendering never runs in a key handler.
- **Facade:**
  - The SVG is served through `ul-asset://` and shown as `<img>`.
  - Its height is `(height + depth)` em and an inline formula sits at `vertical-align: -depth` em, so it scales with
    the editor zoom.
  - Its alternative text is the source.
  - Until the SVG arrives, the formula shows its source.
  - An error shows the source and the parser's reason.
- **HTML export:** the same SVG is written inline, as diagrams are (ADR-015), with `role="img"` and the source as its
  label. It is drawn in `currentColor`, so it follows the page's light or dark scheme; the PDF uses black.
- **PDF export:**
  - The same SVG goes into the Typst world as an image.
  - An inline formula is boxed at its baseline.
  - A block formula is centred.
- **Export to the other note format:** the markup writer maps the syntax (`$…$` ↔ `latexmath:[…]`, ```` ```math ````
  ↔ `[latexmath]` block).

## Consequences

- **Agreement:** one engine typesets the facade, the HTML export and the PDF export. A formula that typesets in the
  facade typesets the same in both exports.
- **Measured** (release, Linux, RaTeX 0.1.15):
  - 0.17 ms per typical formula on average, 0.74 ms for the first one.
  - Errors are typed and carry a position.
  - The SVG of a formula is 3 to 23 KB.
- **Size:** the app binary and the frontend bundle grow by almost nothing, because RaTeX and its fonts are already
  linked. The WASM file grows by the formula parsing, 48 KB (1.8 %). The HTML export grows by the SVG of each formula
  (about 9 KB on average).
- **Formulas are pictures:**
  - Their text cannot be selected in the facade; copying gives the source.
  - In the PDF they cannot be searched.
- **Risk: RaTeX is young** (0.1.x, mostly one author), and its version is bound to merman's.
  - The `MathRenderer` trait keeps a switch open: KaTeX in the existing QuickJS (ADR-009), or KaTeX in the frontend
    for the facade only.
- **C-003:**
  - GitHub typesets with MathJax, GitLab with KaTeX.
  - Commands only MathJax knows (`\bbox`, `\class`, `\require`) are shown as errors (A21).
  - Comrak's dollar rules are checked against GitHub's in tests (`$22 and $2+2$`, `$$` within a paragraph).
- **`\text{}` with CJK or rare characters** falls back to system fonts and can differ per platform.
- **Alternatives considered:**
  - **KaTeX in the frontend** (about 560 KB with fonts):
    - It would give selectable, crisp text.
    - It does not reach the PDF, so a second engine would be needed, and the facade and the PDF could disagree.
    - It stays the fallback for the facade if `<img>` formulas prove unacceptable.
  - **MathML Core rendered by the webview** (with Temml, pulldown-latex or math-core):
    - WebKit's MathML differs from Chromium's in stretchy operators and spacing.
    - It needs an OpenType MATH font, which is often missing on Linux.
    - It needs a second path for Typst. Against NFR-013.
  - **`mitex`** (LaTeX to Typst math, Apache-2.0):
    - Its crates.io release is stale.
    - Its output needs mitex's Typst scope vendored into the world.
    - Its coverage differs from KaTeX.
  - **`tex2typst-rs`:** GPL-3.0, against C-008.
  - **MathJax:** too large for the bundle or for QuickJS.
  - **RaTeX in the WASM core:** it would grow the 2.7 MB file, and typesetting is not on the key path.
