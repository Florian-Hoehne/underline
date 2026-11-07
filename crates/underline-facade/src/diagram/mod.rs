//! Diagrams of a note shown as pictures (FR-036 to FR-038, UC-004 A11,
//! ADR-009): one [`DiagramRenderer`] per diagram language turns the source
//! of a diagram into SVG, which is rasterized for the facade. Rendering runs
//! on a worker thread with a cache ([`Diagrams`]), so typing never waits for
//! a diagram (NFR-004).

mod mermaid;
mod plantuml;
mod raster;
mod service;

pub use mermaid::Mermaid;
pub use plantuml::PlantUml;
pub use raster::{rasterize, Picture};
pub(crate) use service::renderer;
pub use service::{DiagramState, Diagrams};

/// The languages diagrams are written in (C-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagramLanguage {
    PlantUml,
    Mermaid,
}

impl DiagramLanguage {
    /// The diagram language a code block names, if it names one.
    pub fn from_code_language(language: &str) -> Option<Self> {
        match language.to_ascii_lowercase().as_str() {
            "plantuml" | "puml" => Some(Self::PlantUml),
            "mermaid" => Some(Self::Mermaid),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::PlantUml => "PlantUML",
            Self::Mermaid => "Mermaid",
        }
    }
}

/// Why a diagram could not be rendered (UC-004 A11).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiagramError {
    /// The source has an error; `line` counts from 1 in the diagram source.
    #[error("{}{message}", .line.map(|l| format!("Line {l}: ")).unwrap_or_default())]
    Syntax {
        line: Option<usize>,
        message: String,
    },
    #[error("the diagram took longer than {0} seconds to render")]
    TimedOut(u64),
    #[error("the diagram is too large to show")]
    TooLarge,
    /// The renderer of the language is not available.
    #[error("{0}")]
    NotAvailable(String),
    /// The rendering engine failed.
    #[error("the diagram could not be rendered: {0}")]
    Engine(String),
}

/// Turns the source of a diagram into SVG (ADR-009).
pub trait DiagramRenderer {
    fn render(&mut self, source: &str, dark: bool) -> Result<String, DiagramError>;
}
