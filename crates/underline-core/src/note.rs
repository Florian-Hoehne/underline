use serde::{Deserialize, Serialize};

use crate::format::NoteFormat;

/// Identifies a note by its place in the workspace without the file
/// extension, with `/` between folders (`Home`, `guide/Getting-Started`).
/// A note at the top of the workspace is named by its page name, as wikis do.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NoteId(String);

impl NoteId {
    /// Builds the id of a note at the top of the workspace from a human
    /// title, using the wiki convention of replacing whitespace with `-`.
    pub fn from_title(title: &str) -> Self {
        let slug = title.split_whitespace().collect::<Vec<_>>().join("-");
        Self(slug)
    }

    /// Builds the id of the note at `path`, relative to the workspace,
    /// without extension and with `/` between folders.
    pub fn from_relative(path: &str) -> Self {
        Self(path.trim_matches('/').to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Title shown to the user: the page name with `-` turned into spaces.
    pub fn title(&self) -> String {
        self.page().replace('-', " ")
    }

    /// The folder of the note inside the workspace, `None` at the top.
    pub fn folder(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(folder, _)| folder)
    }

    /// Path of the note's file inside the workspace directory, in `format`.
    pub fn file_name(&self, format: NoteFormat) -> String {
        format!("{}.{}", self.0, format.extension())
    }

    /// Whether the id names a place inside the workspace: no empty, hidden,
    /// or parent segments and no backslashes.
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && !self.0.contains('\\')
            && self
                .0
                .split('/')
                .all(|segment| !segment.is_empty() && !segment.starts_with('.'))
    }

    fn page(&self) -> &str {
        self.0
            .rsplit_once('/')
            .map_or(self.0.as_str(), |(_, page)| page)
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single note; its note format follows its file in the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub body: String,
}

impl Note {
    pub fn new(title: &str, body: impl Into<String>) -> Self {
        Self {
            id: NoteId::from_title(title),
            body: body.into(),
        }
    }

    /// Title shown in the UI: the page name with `-` turned back into spaces.
    pub fn title(&self) -> String {
        self.id.title()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_round_trips_through_wiki_page_name() {
        let note = Note::new("Getting   Started", "");
        assert_eq!(note.id.as_str(), "Getting-Started");
        assert_eq!(
            note.id.file_name(NoteFormat::Markdown),
            "Getting-Started.md"
        );
        assert_eq!(
            note.id.file_name(NoteFormat::AsciiDoc),
            "Getting-Started.adoc"
        );
        assert_eq!(note.title(), "Getting Started");
    }

    /// UC-002 BR-003 / BR-004: notes in folders keep their folder.
    #[test]
    fn uc002_notes_in_folders_have_folder_and_title() {
        let id = NoteId::from_relative("guide/setup/Getting-Started");
        assert_eq!(id.title(), "Getting Started");
        assert_eq!(id.folder(), Some("guide/setup"));
        assert_eq!(
            id.file_name(NoteFormat::AsciiDoc),
            "guide/setup/Getting-Started.adoc"
        );
        assert_eq!(NoteId::from_title("Home").folder(), None);
        assert!(id.is_valid());
        for bad in ["", "../secret", "a//b", ".hidden/x", "a\\b"] {
            assert!(
                !NoteId::from_relative(bad).is_valid() || bad.is_empty(),
                "{bad}"
            );
        }
    }
}
