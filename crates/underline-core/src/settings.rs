//! The settings files `underline.yaml` (C-006, ADR-004).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;

/// File name of the user settings file and of every workspace settings file.
pub const SETTINGS_FILE_NAME: &str = "underline.yaml";

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("{path} is not valid: {message}")]
    Invalid { path: PathBuf, message: String },
}

/// The settings of one workspace settings file. Every key is optional; an
/// unset key means that the user setting applies. Keys Underline does not
/// know are ignored when reading.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceSettings {
    /// Note format given to new notes of the workspace (UC-001, BR-005).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_format: Option<NoteFormat>,
}

impl WorkspaceSettings {
    /// Path of the workspace settings file of the workspace at `root`.
    pub fn path(root: &Path) -> PathBuf {
        root.join(SETTINGS_FILE_NAME)
    }

    /// Reads the workspace settings file of `root`; a missing file gives
    /// empty settings.
    pub fn load(root: &Path) -> Result<Self, SettingsError> {
        let path = Self::path(root);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(SettingsError::Read { path, source }),
        };
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_norway::from_str(&text).map_err(|err| SettingsError::Invalid {
            path,
            message: err.to_string(),
        })
    }

    /// Writes a new workspace settings file for `root`, atomically (ADR-005).
    ///
    /// Only for a workspace that has no settings file yet (UC-001): changing an
    /// existing file edits single keys so the user's comments stay (ADR-004).
    pub fn write_new(&self, root: &Path) -> Result<PathBuf, SettingsError> {
        let path = Self::path(root);
        let text = serde_norway::to_string(self).map_err(|err| SettingsError::Invalid {
            path: path.clone(),
            message: err.to_string(),
        })?;
        write_atomic(&path, text.as_bytes()).map_err(|source| SettingsError::Write {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }
}

/// The directory that holds the user settings file: the user's home
/// directory (C-006).
pub fn user_settings_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("underline-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_format_round_trips() {
        let dir = temp_dir("roundtrip");
        let settings = WorkspaceSettings {
            default_format: Some(NoteFormat::AsciiDoc),
        };
        settings.write_new(&dir).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "default_format: asciidoc\n"
        );
        assert_eq!(WorkspaceSettings::load(&dir).unwrap(), settings);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_and_unknown_keys_are_accepted() {
        let dir = temp_dir("unknown");
        assert_eq!(
            WorkspaceSettings::load(&dir).unwrap(),
            WorkspaceSettings::default()
        );
        fs::write(
            dir.join(SETTINGS_FILE_NAME),
            "# mine\ntheme: Nord\ndefault_format: markdown\n",
        )
        .unwrap();
        assert_eq!(
            WorkspaceSettings::load(&dir).unwrap().default_format,
            Some(NoteFormat::Markdown)
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
