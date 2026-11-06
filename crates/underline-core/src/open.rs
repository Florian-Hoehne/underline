//! UC-002 Open Workspace: the checks of step 4 before a directory is opened.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::create::{absolute, enclosing_workspace, Surroundings};

/// Why a directory cannot be opened as it was chosen (UC-002 step 4).
#[derive(Debug, thiserror::Error)]
pub enum OpenProblem {
    /// A3: the directory does not exist; a new workspace may be created there.
    #[error("{} does not exist", .0.display())]
    Missing(PathBuf),
    #[error("{} is a file, not a directory", .0.display())]
    NotADirectory(PathBuf),
    /// A7: the directory cannot be read.
    #[error("{} cannot be read: {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },
    /// A11: the home directory holds the user settings file and all of the
    /// user's files.
    #[error("the home directory {} cannot be a workspace, because it holds the user settings file and all of your files", .0.display())]
    HomeDirectory(PathBuf),
    /// A5: the directory lies inside a known workspace.
    #[error("{} lies inside the workspace {}", .directory.display(), .workspace.display())]
    InsideWorkspace {
        directory: PathBuf,
        workspace: PathBuf,
    },
}

/// UC-002 step 4: checks that `directory` exists, can be read, is not the
/// home directory, and does not lie inside a known workspace, in this order.
/// Returns the absolute directory to open.
pub fn check_open_directory(
    directory: &Path,
    around: &Surroundings,
) -> Result<PathBuf, OpenProblem> {
    let directory = check_readable_directory(directory, around)?;
    if let Some(workspace) = enclosing_workspace(&directory, around) {
        return Err(OpenProblem::InsideWorkspace {
            directory,
            workspace,
        });
    }
    Ok(directory)
}

/// The checks of step 4 without the one for an enclosing workspace, for a
/// directory the user chose to open anyway (UC-002 A5 step 2).
pub fn check_readable_directory(
    directory: &Path,
    around: &Surroundings,
) -> Result<PathBuf, OpenProblem> {
    let directory = absolute(directory);
    match fs::metadata(&directory) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(OpenProblem::Missing(directory))
        }
        Err(source) => {
            return Err(OpenProblem::Unreadable {
                path: directory,
                source,
            })
        }
        Ok(meta) if !meta.is_dir() => return Err(OpenProblem::NotADirectory(directory)),
        Ok(_) => {}
    }
    if let Err(source) = fs::read_dir(&directory) {
        return Err(OpenProblem::Unreadable {
            path: directory,
            source,
        });
    }
    if around.user_settings_dir.as_deref().map(absolute).as_deref() == Some(directory.as_path()) {
        return Err(OpenProblem::HomeDirectory(directory));
    }
    Ok(directory)
}

/// Test cases of UC-002 for the checks of step 4.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SETTINGS_FILE_NAME;
    use crate::state::UNDERLINE_FOLDER;

    fn sandbox(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("underline-uc002-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// TC-002-13: a missing directory is reported, not created (A3).
    #[test]
    fn uc002_tc13_missing_directory() {
        let root = sandbox("tc13");
        let fresh = root.join("fresh");
        assert!(matches!(
            check_open_directory(&fresh, &Surroundings::default()),
            Err(OpenProblem::Missing(_))
        ));
        assert!(!fresh.exists());
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-14: a directory inside a workspace is reported with the outer
    /// workspace (A5), whether it is remembered or marked on disk.
    #[test]
    fn uc002_tc14_directory_inside_a_workspace() {
        let root = sandbox("tc14");
        let marked = root.join("marked");
        fs::create_dir_all(marked.join(UNDERLINE_FOLDER)).unwrap();
        fs::create_dir_all(marked.join("inner")).unwrap();
        let remembered = root.join("remembered");
        fs::create_dir_all(remembered.join("inner")).unwrap();
        let around = Surroundings {
            known_workspaces: vec![remembered.clone()],
            user_settings_dir: None,
        };

        for (inner, outer) in [
            (marked.join("inner"), &marked),
            (remembered.join("inner"), &remembered),
        ] {
            match check_open_directory(&inner, &around) {
                Err(OpenProblem::InsideWorkspace { workspace, .. }) => {
                    assert_eq!(&workspace, outer)
                }
                other => panic!("expected InsideWorkspace, got {other:?}"),
            }
            assert!(check_readable_directory(&inner, &around).is_ok());
        }
        assert!(check_open_directory(&marked, &around).is_ok());
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-19: the home directory is refused (A11), and the user settings
    /// file there makes no workspace of the folders below it (BR-007).
    #[test]
    fn uc002_tc19_home_directory_is_refused() {
        let home = sandbox("tc19");
        fs::write(home.join(SETTINGS_FILE_NAME), "theme: Nord\n").unwrap();
        fs::create_dir_all(home.join("notes")).unwrap();
        let around = Surroundings {
            known_workspaces: Vec::new(),
            user_settings_dir: Some(home.clone()),
        };
        assert!(matches!(
            check_open_directory(&home, &around),
            Err(OpenProblem::HomeDirectory(_))
        ));
        assert!(check_open_directory(&home.join("notes"), &around).is_ok());
        assert!(!home.join(UNDERLINE_FOLDER).exists());
        fs::remove_dir_all(&home).unwrap();
    }

    /// TC-002-15: an unreadable directory is reported (A7). Skipped where
    /// permissions do not apply (as root).
    #[cfg(unix)]
    #[test]
    fn uc002_tc15_unreadable_directory() {
        use std::os::unix::fs::PermissionsExt;
        let root = sandbox("tc15");
        let locked = root.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read_dir(&locked).is_ok();
        let result = check_open_directory(&locked, &Surroundings::default());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert!(matches!(result, Err(OpenProblem::Unreadable { .. })));
        }
        fs::remove_dir_all(&root).unwrap();
    }
}
