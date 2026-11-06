use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::note::{Note, NoteId};

#[derive(Debug, thiserror::Error)]
pub enum NotebookError {
    #[error("note `{0}` not found")]
    NotFound(NoteId),
    #[error("invalid note id `{0}`")]
    InvalidId(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A directory of Markdown notes, laid out like a wiki repository.
#[derive(Debug, Clone)]
pub struct Notebook {
    root: PathBuf,
}

impl Notebook {
    /// Opens a notebook at `root`, creating the directory if needed.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, NotebookError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lists the notes in the notebook, sorted by id.
    pub fn list(&self) -> Result<Vec<NoteId>, NotebookError> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(NoteId::from_title(stem));
            }
        }
        ids.sort();
        Ok(ids)
    }

    pub fn read(&self, id: &NoteId) -> Result<Note, NotebookError> {
        let path = self.path_of(id)?;
        match fs::read_to_string(&path) {
            Ok(body) => Ok(Note {
                id: id.clone(),
                body,
            }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err(NotebookError::NotFound(id.clone()))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn write(&self, note: &Note) -> Result<(), NotebookError> {
        fs::write(self.path_of(&note.id)?, &note.body)?;
        Ok(())
    }

    fn path_of(&self, id: &NoteId) -> Result<PathBuf, NotebookError> {
        let name = id.as_str();
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(NotebookError::InvalidId(name.to_owned()));
        }
        Ok(self.root.join(id.file_name()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_notebook(name: &str) -> Notebook {
        let dir = std::env::temp_dir().join(format!("underline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Notebook::open(dir).unwrap()
    }

    #[test]
    fn writes_lists_and_reads_notes() {
        let notebook = temp_notebook("roundtrip");
        notebook.write(&Note::new("Home", "# Welcome")).unwrap();
        notebook.write(&Note::new("Ideas", "- one")).unwrap();

        let ids = notebook.list().unwrap();
        assert_eq!(
            ids,
            vec![NoteId::from_title("Home"), NoteId::from_title("Ideas")]
        );
        assert_eq!(notebook.read(&ids[0]).unwrap().body, "# Welcome");
        fs::remove_dir_all(notebook.root()).unwrap();
    }

    #[test]
    fn missing_note_is_not_found() {
        let notebook = temp_notebook("missing");
        let err = notebook.read(&NoteId::from_title("Nope")).unwrap_err();
        assert!(matches!(err, NotebookError::NotFound(_)));
        fs::remove_dir_all(notebook.root()).unwrap();
    }

    #[test]
    fn rejects_ids_that_escape_the_notebook() {
        let notebook = temp_notebook("escape");
        let err = notebook.write(&Note::new("../secret", "")).unwrap_err();
        assert!(matches!(err, NotebookError::InvalidId(_)));
        fs::remove_dir_all(notebook.root()).unwrap();
    }
}
