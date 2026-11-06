use serde::{Deserialize, Serialize};

/// The markup a note is stored in, given by its file extension (FR-004, C-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteFormat {
    /// Markdown as GitHub shows it, stored as `.md`. The preselected default
    /// format of a new workspace (UC-001 BR-001).
    #[default]
    Markdown,
    /// AsciiDoc, stored as `.adoc`.
    AsciiDoc,
}

impl NoteFormat {
    pub const ALL: [NoteFormat; 2] = [NoteFormat::Markdown, NoteFormat::AsciiDoc];

    /// File extension without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            NoteFormat::Markdown => "md",
            NoteFormat::AsciiDoc => "adoc",
        }
    }

    /// The note format of a file extension, if it is one Underline knows.
    pub fn from_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| format.extension() == extension)
    }

    /// Name shown to the user.
    pub fn name(self) -> &'static str {
        match self {
            NoteFormat::Markdown => "Markdown",
            NoteFormat::AsciiDoc => "AsciiDoc",
        }
    }
}

impl std::fmt::Display for NoteFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_round_trips() {
        for format in NoteFormat::ALL {
            assert_eq!(NoteFormat::from_extension(format.extension()), Some(format));
        }
        assert_eq!(NoteFormat::from_extension("txt"), None);
    }
}
