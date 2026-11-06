use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::note::{Note, NoteId};
use crate::settings::{SettingsError, WorkspaceSettings};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("note `{0}` not found")]
    NotFound(NoteId),
    #[error("invalid note id `{0}`")]
    InvalidId(String),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A local directory whose Markdown and AsciiDoc files are shown as notes.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    default_format: NoteFormat,
}

impl Workspace {
    /// Opens the workspace at `root`, creating the directory if needed, and
    /// reads its default format from the workspace settings file (Markdown
    /// when the file does not set one).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let default_format = WorkspaceSettings::load(&root)?
            .default_format
            .unwrap_or_default();
        Ok(Self {
            root,
            default_format,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Note format given to new notes of this workspace.
    pub fn default_format(&self) -> NoteFormat {
        self.default_format
    }

    /// Lists the notes in the workspace, sorted by id.
    pub fn list(&self) -> Result<Vec<NoteId>, WorkspaceError> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            let known = path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(NoteFormat::from_extension)
                .is_some();
            if !known {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !stem.starts_with('.') {
                    ids.push(NoteId::from_title(stem));
                }
            }
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    pub fn read(&self, id: &NoteId) -> Result<Note, WorkspaceError> {
        let Some(path) = self.existing_path(id)? else {
            return Err(WorkspaceError::NotFound(id.clone()));
        };
        match fs::read_to_string(&path) {
            Ok(body) => Ok(Note {
                id: id.clone(),
                body,
            }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Err(WorkspaceError::NotFound(id.clone()))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Saves a note atomically (ADR-005): into its existing file, or as a new
    /// file in the default format of the workspace.
    pub fn write(&self, note: &Note) -> Result<(), WorkspaceError> {
        let path = match self.existing_path(&note.id)? {
            Some(path) => path,
            None => self.root.join(note.id.file_name(self.default_format)),
        };
        write_atomic(&path, note.body.as_bytes())?;
        Ok(())
    }

    /// The file of note `id`, if it exists in any known note format.
    fn existing_path(&self, id: &NoteId) -> Result<Option<PathBuf>, WorkspaceError> {
        let name = id.as_str();
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(WorkspaceError::InvalidId(name.to_owned()));
        }
        Ok(NoteFormat::ALL
            .into_iter()
            .map(|format| self.root.join(id.file_name(format)))
            .find(|path| path.is_file()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(name: &str) -> Workspace {
        let dir = std::env::temp_dir().join(format!("underline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Workspace::open(dir).unwrap()
    }

    #[test]
    fn writes_lists_and_reads_notes() {
        let workspace = temp_workspace("roundtrip");
        workspace.write(&Note::new("Home", "# Welcome")).unwrap();
        workspace.write(&Note::new("Ideas", "- one")).unwrap();

        let ids = workspace.list().unwrap();
        assert_eq!(
            ids,
            vec![NoteId::from_title("Home"), NoteId::from_title("Ideas")]
        );
        assert_eq!(workspace.read(&ids[0]).unwrap().body, "# Welcome");
        fs::remove_dir_all(workspace.root()).unwrap();
    }

    #[test]
    fn lists_and_keeps_asciidoc_notes() {
        let workspace = temp_workspace("asciidoc");
        fs::write(workspace.root().join("Plan.adoc"), "= Plan\n").unwrap();
        let id = NoteId::from_title("Plan");
        assert_eq!(workspace.list().unwrap(), vec![id.clone()]);

        workspace
            .write(&Note::new("Plan", "= Plan\n\nMore.\n"))
            .unwrap();
        assert!(!workspace.root().join("Plan.md").exists());
        assert_eq!(workspace.read(&id).unwrap().body, "= Plan\n\nMore.\n");
        fs::remove_dir_all(workspace.root()).unwrap();
    }

    #[test]
    fn missing_note_is_not_found() {
        let workspace = temp_workspace("missing");
        let err = workspace.read(&NoteId::from_title("Nope")).unwrap_err();
        assert!(matches!(err, WorkspaceError::NotFound(_)));
        fs::remove_dir_all(workspace.root()).unwrap();
    }

    #[test]
    fn rejects_ids_that_escape_the_workspace() {
        let workspace = temp_workspace("escape");
        let err = workspace.write(&Note::new("../secret", "")).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidId(_)));
        fs::remove_dir_all(workspace.root()).unwrap();
    }
}
