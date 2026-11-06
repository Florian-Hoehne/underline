//! Mermaid through `merman`, a Rust implementation of Mermaid that is
//! checked against Mermaid's own fixtures (ADR-009): no browser, no network.

use std::time::Duration;

use merman::{
    Engine, MermaidConfig, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
    SvgRequest,
};

use super::{DiagramError, DiagramRenderer};

/// The Mermaid renderer, one engine for each of the light and dark theme.
pub struct Mermaid {
    light: Renderer,
    dark: Renderer,
    timeout: Duration,
}

impl Default for Mermaid {
    fn default() -> Self {
        Self::new()
    }
}

impl Mermaid {
    pub fn new() -> Self {
        let themed = |theme: &str| {
            // Labels as SVG text: the picture is rasterized without a
            // browser, which HTML labels would need.
            let config = MermaidConfig::from_value(serde_json::json!({
                "theme": theme,
                "htmlLabels": false,
                "flowchart": { "htmlLabels": false },
                "class": { "htmlLabels": false },
                "state": { "htmlLabels": false },
            }));
            Renderer::new().with_engine(Engine::new().with_site_config(config))
        };
        Self {
            light: themed("default"),
            dark: themed("dark"),
            timeout: super::plantuml::TIMEOUT,
        }
    }
}

impl DiagramRenderer for Mermaid {
    fn render(&mut self, source: &str, dark: bool) -> Result<String, DiagramError> {
        let renderer = if dark { &self.dark } else { &self.light };
        let control = OperationControl::new().with_deadline(self.timeout);
        let output = renderer.render(RenderRequest::svg(source, control, SvgRequest::default()));
        match output {
            Ok(RenderOutput::Svg(Some(svg))) => Ok(svg.svg().to_string()),
            Ok(_) | Err(RenderError::NoDiagram) => Err(DiagramError::Syntax {
                line: None,
                message: "no Mermaid diagram was found".into(),
            }),
            Err(RenderError::Parse(diagnostic)) => {
                let line = diagnostic
                    .terminal_diagnostic_details()
                    .span
                    .map(|span| line_of(source, span.start));
                Err(DiagramError::Syntax {
                    line,
                    message: diagnostic.terminal_safe_message(),
                })
            }
            Err(RenderError::Cancelled(_)) => Err(DiagramError::TimedOut(self.timeout.as_secs())),
            Err(RenderError::ResourceLimitExceeded(_)) => Err(DiagramError::TooLarge),
            Err(error) => Err(DiagramError::Engine(error.to_string())),
        }
    }
}

/// The line, counted from 1, of byte `offset` in `source`.
fn line_of(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    source.as_bytes()[..offset]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::rasterize;

    /// TC-004-04, FR-037: the diagram types GitHub shows render offline,
    /// light and dark, and can be rasterized.
    #[test]
    fn uc004_tc04_mermaid_renders_offline() {
        let mut mermaid = Mermaid::new();
        let diagrams = [
            "flowchart TD\n  A[Open] --> B{Exists?}\n  B -- yes --> C[Show]\n  B -- no --> D[Create]",
            "sequenceDiagram\n  actor User\n  User->>Underline: type\n  Underline-->>User: saved",
            "classDiagram\n  class Note {\n    +title() String\n  }\n  Workspace *-- Note",
            "stateDiagram-v2\n  [*] --> Clean\n  Clean --> Dirty: edit",
            "erDiagram\n  WORKSPACE ||--o{ NOTE : holds",
            "gantt\n  dateFormat YYYY-MM-DD\n  section Editor\n  Facade :a1, 2026-11-01, 10d",
            "pie title Formats\n  \"Markdown\" : 70\n  \"AsciiDoc\" : 30",
        ];
        for source in diagrams {
            for dark in [false, true] {
                let svg = mermaid
                    .render(source, dark)
                    .unwrap_or_else(|e| panic!("{source}: {e}"));
                assert!(svg.contains("<svg"), "{source}");
                assert!(!svg.contains("<foreignObject"), "HTML labels in {source}");
                let picture = rasterize(&svg, 1.0).unwrap_or_else(|e| panic!("{source}: {e}"));
                assert!(picture.width > 20 && picture.height > 20, "{source}");
            }
        }
    }

    /// TC-004-23: a source with an error gives the error and its line.
    #[test]
    fn uc004_tc23_mermaid_syntax_error() {
        let mut mermaid = Mermaid::new();
        match mermaid.render("flowchart TD\n  A --> B\n  B --> -->", false) {
            Err(DiagramError::Syntax { line, message }) => {
                assert!(!message.is_empty());
                assert_eq!(line, Some(3), "{message}");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            mermaid.render("just some words", false),
            Err(DiagramError::Syntax { .. })
        ));
    }

    /// TC-004-05, NFR-004: a diagram of 200 elements renders and rasterizes
    /// within 2 seconds (`cargo test --release -p underline-facade -- --ignored`).
    #[test]
    #[ignore]
    fn tc004_05_mermaid_200_elements_in_time() {
        let mut source = String::from("flowchart TD\n");
        for i in 1..100 {
            source.push_str(&format!(
                "  N{}[Node {}] --> N{i}[Node {i}]\n",
                (i - 1) / 2,
                (i - 1) / 2
            ));
        }
        let mut mermaid = Mermaid::new();
        for dark in [false, true] {
            let start = std::time::Instant::now();
            let svg = mermaid.render(&source, dark).unwrap();
            let picture = rasterize(&svg, 2.0).unwrap();
            let took = start.elapsed();
            println!(
                "200 elements: {took:?}, {}x{} px",
                picture.width, picture.height
            );
            assert!(took < std::time::Duration::from_secs(2));
        }
    }
}
