use serde::{Deserialize, Serialize};

/// Identifies a note by its page name, as wikis do (`Home`, `Getting-Started`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NoteId(String);

impl NoteId {
    /// Builds an id from a human title, using the wiki convention of
    /// replacing whitespace with `-`.
    pub fn from_title(title: &str) -> Self {
        let slug = title.split_whitespace().collect::<Vec<_>>().join("-");
        Self(slug)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// File name of the note inside a notebook directory.
    pub fn file_name(&self) -> String {
        format!("{}.md", self.0)
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single Markdown note.
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
        self.id.as_str().replace('-', " ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_round_trips_through_wiki_page_name() {
        let note = Note::new("Getting   Started", "");
        assert_eq!(note.id.as_str(), "Getting-Started");
        assert_eq!(note.id.file_name(), "Getting-Started.md");
        assert_eq!(note.title(), "Getting Started");
    }
}
