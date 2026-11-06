# ADR-009: Render PlantUML and Mermaid offline inside Underline

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** FR-036, FR-037, FR-038, NFR-004, NFR-013, NFR-014, C-005, C-008

## Context

Notes hold PlantUML and Mermaid diagrams as code blocks (C-005), which UC-004 shows as pictures in the colors of the
active theme (FR-036 to FR-038, UC-004 BR-004). Rendering must work without a network connection (NFR-014), the same
on Linux, Windows, and macOS (NFR-013), and a diagram of 200 elements must appear within 2 seconds while typing stays
fast (NFR-004, NFR-002). A diagram with an error shows its source and the error (UC-004 A11). Underline is MIT
licensed (C-008).

The usual tools do not fit: the official Mermaid CLI starts a headless browser, and PlantUML is a Java program.
In 2026 there are pure-Rust Mermaid renderers, and PlantUML publishes an official build compiled to JavaScript with
TeaVM (`@plantuml/core`, MIT licensed, about 5 MB with its Graphviz layout engine), which runs without Java, without a
server, and without a DOM (it supports Web Workers through `renderToString`).

## Decision

- **One renderer interface.** `crates/underline-facade` defines a `DiagramRenderer` trait: diagram source and theme
  colors in, SVG or a typed error out. The facade engine looks the renderer up by the language of the code block
  (`plantuml`, `puml`, `mermaid`). Renderers run on a worker thread, never on the UI thread, and their SVG is cached
  by source and theme, so typing elsewhere in the note never waits for a diagram.
- **Drawing.** The SVG is rasterized with `resvg`/`usvg` into an egui texture at the display scale, and again at a
  higher scale when the user enlarges the diagram (FR-039).
- **Mermaid:** a pure-Rust renderer crate behind the trait. Candidates are `mermaid-rs-renderer`, `rusty-mermaid`, and
  `fermaid`; the implementation picks one by these criteria, in this order: license compatible with MIT; coverage of
  the diagram types GitHub renders (at least flowchart, sequence, class, state, ER, gantt, pie); support for theme
  colors; recent releases. The choice and the measured coverage are added to this ADR when the crate is chosen.
- **PlantUML, primary:** the official TeaVM build `@plantuml/core` (pinned version, vendored into the repository with
  its license) runs in an embedded JavaScript engine (`rquickjs`, QuickJS) on the renderer thread, started on the
  first PlantUML diagram. `renderToString` returns the SVG; dark themes use its `dark` option, theme colors are passed
  as `skinparam` lines. `!include` is limited to files inside the workspace; includes from the web are not fetched and
  render as an error.
- **PlantUML, first step: a spike.** Before the renderer is built on it, a spike checks in a test that the TeaVM build
  runs in QuickJS: sequence, class, activity, state, and component diagrams render offline; a diagram of 200 elements
  renders within 2 seconds on a typical laptop after the engine is started; the engine starts within 3 seconds; the
  result is the same on Linux, Windows, and macOS.
- **PlantUML, fallback if the spike fails:** the MIT-licensed PlantUML jar is bundled and run with a Java found on the
  machine (`JAVA_HOME`, then the `PATH`). Without Java, a PlantUML diagram shows its source and says that Java is
  needed (UC-004 A11); Mermaid and everything else keep working. This fallback is then recorded here as the decision.
- **Errors.** Every renderer error (syntax error, too large, timeout after 10 seconds, missing engine) becomes a typed
  error that UC-004 A11 shows next to the source; the rest of the note renders.

## Consequences

- Rendering needs no network, no browser, and, with the primary path, no Java; the app grows by about 5 MB for
  PlantUML plus the Mermaid crate.
- Rendering through QuickJS is slower than native code; the cache and the worker thread keep it away from typing.
- The output of the Mermaid crate is close to, but not identical with, Mermaid.js; differences are accepted and
  tracked as issues of the chosen crate.
- PlantUML updates mean replacing the vendored build; its version is pinned and tested.
- **Alternatives considered:** Mermaid.js in a JavaScript engine (needs a DOM for measuring text); a hidden system
  webview for both (adds the browser engine ADR-001 avoided); a trimmed Java runtime bundled per platform (about
  40 to 50 MB per platform); a PlantUML server (needs a network or a running server).
