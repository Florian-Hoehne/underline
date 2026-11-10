//! The languages diagrams are written in, as code blocks name them. Their
//! rendering stays in the facade engine (ADR-009).

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
