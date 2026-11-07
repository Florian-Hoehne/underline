//! UC-007 Delete Note: deleted notes are kept in the Underline folder of the
//! workspace for 30 days, on this computer only (BR-004, BR-005, ADR-008).
//!
//! Every deletion is a folder named by the time of deleting, holding the note
//! at the place it had in the workspace, so the place, the content, and the
//! time are kept together without a list that could get out of step.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic::write_new_atomic;
use crate::format::NoteFormat;
use crate::new_note::{NewNote, NewNoteProblem};
use crate::note::NoteId;
use crate::rename::Rename;
use crate::state::{underline_folder, Timestamp, UNDERLINE_FOLDER};
use crate::workspace::Workspace;

/// The folder of the deleted notes inside the Underline folder.
pub const DELETED_FOLDER: &str = "deleted";
/// How long a deleted note is kept, in milliseconds (UC-007 BR-005).
pub const KEEP_DELETED_MS: Timestamp = 30 * 24 * 60 * 60 * 1000;

/// A note among the deleted notes of a workspace (entity DELETED_NOTE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedNote {
    /// The note as it was named in the workspace.
    pub id: NoteId,
    pub format: NoteFormat,
    pub deleted_at: Timestamp,
    /// The folder of this deletion and the kept file in it.
    folder: PathBuf,
    file: PathBuf,
}

impl DeletedNote {
    /// The content of the note when it was deleted.
    pub fn content(&self) -> io::Result<String> {
        fs::read_to_string(&self.file)
    }
}

/// Why a note could not be deleted (UC-007 A3, A8).
#[derive(Debug, thiserror::Error)]
pub enum DeleteProblem {
    /// A8: the Underline folder cannot take the note.
    #[error("the note cannot be kept among the deleted notes: {0}")]
    CannotKeep(io::Error),
    /// A3.
    #[error("cannot {action}: {source}")]
    Io {
        action: &'static str,
        source: io::Error,
    },
}

impl Workspace {
    fn deleted_folder(&self) -> PathBuf {
        self.root().join(UNDERLINE_FOLDER).join(DELETED_FOLDER)
    }

    /// UC-007 step 4: moves the note `id` into the deleted notes, deleted at
    /// `at`. The note leaves the workspace only once it is kept completely
    /// (BR-008); when it cannot be kept, nothing is changed (A8).
    pub fn delete_note(
        &mut self,
        id: &NoteId,
        at: Timestamp,
    ) -> Result<DeletedNote, DeleteProblem> {
        let io = |action| move |source| DeleteProblem::Io { action, source };
        let format = self
            .format_of(id)
            .ok_or_else(|| io("find the note")(io::Error::from(io::ErrorKind::NotFound)))?;
        let name = id.file_name(format);
        let path = self.root().join(&name);
        let content = fs::read(&path).map_err(io("read the note"))?;
        let deleted = underline_folder(self.root())
            .map_err(DeleteProblem::CannotKeep)?
            .join(DELETED_FOLDER);
        let folder = (0..)
            .map(|n| match n {
                0 => deleted.join(at.to_string()),
                n => deleted.join(format!("{at}-{n}")),
            })
            .find(|folder| !folder.exists())
            .unwrap_or_else(|| deleted.join(at.to_string()));
        let file = folder.join(&name);
        let keep = fs::create_dir_all(file.parent().unwrap_or(&folder))
            .and_then(|()| write_new_atomic(&file, &content));
        if let Err(err) = keep {
            let _ = fs::remove_dir_all(&folder);
            return Err(DeleteProblem::CannotKeep(err));
        }
        if let Err(err) = fs::remove_file(&path) {
            let _ = fs::remove_dir_all(&folder);
            return Err(io("remove the note")(err));
        }
        self.rescan();
        Ok(DeletedNote {
            id: id.clone(),
            format,
            deleted_at: at,
            folder,
            file,
        })
    }

    /// UC-007 A8 step 2: removes the note `id` for good, without keeping it.
    pub fn delete_note_for_good(&mut self, id: &NoteId) -> Result<(), DeleteProblem> {
        let format = self.format_of(id).ok_or_else(|| DeleteProblem::Io {
            action: "find the note",
            source: io::Error::from(io::ErrorKind::NotFound),
        })?;
        fs::remove_file(self.root().join(id.file_name(format))).map_err(|source| {
            DeleteProblem::Io {
                action: "remove the note",
                source,
            }
        })?;
        self.rescan();
        Ok(())
    }

    /// UC-007 A5 step 1: the deleted notes, most recently deleted first.
    pub fn deleted_notes(&self) -> Vec<DeletedNote> {
        let mut notes = Vec::new();
        let Ok(entries) = fs::read_dir(self.deleted_folder()) else {
            return notes;
        };
        for entry in entries.filter_map(Result::ok) {
            let folder = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(deleted_at) = name.split('-').next().unwrap_or("").parse::<Timestamp>() else {
                continue;
            };
            let mut files = Vec::new();
            collect_files(&folder, &mut files);
            for file in files {
                let Ok(relative) = file.strip_prefix(&folder) else {
                    continue;
                };
                let relative = relative
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                let Some((stem, extension)) = relative.rsplit_once('.') else {
                    continue;
                };
                let Some(format) = NoteFormat::from_extension(extension) else {
                    continue;
                };
                notes.push(DeletedNote {
                    id: NoteId::from_relative(stem),
                    format,
                    deleted_at,
                    folder: folder.clone(),
                    file,
                });
            }
        }
        notes.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then(a.id.cmp(&b.id)));
        notes
    }

    /// UC-007 A5 step 3: puts the deleted note back at its place, or under
    /// the title and folder of `instead` when its place is taken (A6). A
    /// note that is there is never replaced.
    pub fn restore_note(
        &mut self,
        deleted: &DeletedNote,
        instead: Option<&Rename>,
    ) -> Result<NoteId, NewNoteProblem> {
        let content = deleted.content().map_err(|source| NewNoteProblem::Io {
            action: "read the deleted note",
            source,
        })?;
        let new = match instead {
            Some(rename) => NewNote {
                folder: rename.folder.clone(),
                title: rename.title.clone(),
                format: deleted.format,
            },
            None => NewNote {
                folder: deleted.id.folder().unwrap_or_default().to_owned(),
                title: deleted.id.title(),
                format: deleted.format,
            },
        };
        let id = self.create_note(&new, &content)?;
        let _ = self.forget_deleted(deleted);
        Ok(id)
    }

    /// UC-007 A7: removes a deleted note for good.
    pub fn forget_deleted(&self, deleted: &DeletedNote) -> io::Result<()> {
        fs::remove_file(&deleted.file)?;
        // Folders left empty by it go too, the deletion's own folder last.
        let mut dir = deleted.file.parent();
        while let Some(folder) = dir {
            if !folder.starts_with(&deleted.folder) || fs::remove_dir(folder).is_err() {
                break;
            }
            dir = folder.parent();
        }
        Ok(())
    }

    /// UC-007 BR-005: removes the notes deleted more than 30 days before
    /// `now`; returns how many.
    pub fn purge_deleted(&self, now: Timestamp) -> usize {
        self.deleted_notes()
            .iter()
            .filter(|d| now.saturating_sub(d.deleted_at) > KEEP_DELETED_MS)
            .filter(|d| self.forget_deleted(d).is_ok())
            .count()
    }
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let root =
                std::env::temp_dir().join(format!("underline-uc007-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            for (path, content) in files {
                let path = root.join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, content).unwrap();
            }
            Self(root)
        }

        fn open(&self) -> Workspace {
            Workspace::open(&self.0).unwrap()
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap()
        }

        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .args(args)
                .current_dir(&self.0)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .unwrap();
            String::from_utf8(out.stdout).unwrap()
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn id(s: &str) -> NoteId {
        NoteId::from_relative(s)
    }

    /// TC-007-01 / TC-007-13: the note leaves the workspace, is kept among
    /// the deleted notes, and git sees only the deleted note.
    #[test]
    fn uc007_tc01_tc13_deletes_and_keeps_the_note() {
        let sandbox = Sandbox::new(
            "tc01",
            &[("Old-Ideas.md", "# Old Ideas\n"), ("Home.md", "")],
        );
        sandbox.git(&["init", "--quiet"]);
        sandbox.git(&["add", "."]);
        sandbox.git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "--quiet",
            "-m",
            "init",
        ]);
        let mut workspace = sandbox.open();
        let deleted = workspace.delete_note(&id("Old-Ideas"), 1_000).unwrap();
        assert!(!sandbox.0.join("Old-Ideas.md").exists());
        assert!(!workspace
            .listing()
            .notes
            .iter()
            .any(|n| n.id == id("Old-Ideas")));
        assert_eq!(workspace.deleted_notes(), std::slice::from_ref(&deleted));
        assert_eq!(deleted.content().unwrap(), "# Old Ideas\n");
        assert_eq!(deleted.deleted_at, 1_000);
        assert_eq!(
            sandbox.git(&["status", "--porcelain", "--untracked-files=all"]),
            " D Old-Ideas.md\n"
        );
    }

    /// TC-007-04: only the note goes; its files stay.
    #[test]
    fn uc007_tc04_files_stay() {
        let sandbox = Sandbox::new(
            "tc04",
            &[("Trip.md", "![b](img/beach.png)"), ("img/beach.png", "png")],
        );
        let mut workspace = sandbox.open();
        workspace.delete_note(&id("Trip"), 1).unwrap();
        assert_eq!(sandbox.read("img/beach.png"), "png");
    }

    /// TC-007-08 / TC-007-09: restored at its place, its folder created
    /// again, and no longer among the deleted notes; most recent first.
    #[test]
    fn uc007_tc09_restores_at_its_place() {
        let sandbox = Sandbox::new(
            "tc09",
            &[
                ("Old.md", "old"),
                ("guide/Setup.md", "setup"),
                ("Pre-Release.md", "pre"),
            ],
        );
        let mut workspace = sandbox.open();
        workspace.delete_note(&id("Old"), 10).unwrap();
        workspace.delete_note(&id("guide/Setup"), 20).unwrap();
        workspace.delete_note(&id("Pre-Release"), 20).unwrap();
        fs::remove_dir(sandbox.0.join("guide")).unwrap();
        let deleted = workspace.deleted_notes();
        let ids: Vec<_> = deleted.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["Pre-Release", "guide/Setup", "Old"]);
        let setup = deleted.iter().find(|d| d.id == id("guide/Setup")).unwrap();
        assert_eq!(
            workspace.restore_note(setup, None).unwrap(),
            id("guide/Setup")
        );
        assert_eq!(sandbox.read("guide/Setup.md"), "setup");
        let pre = deleted.iter().find(|d| d.id == id("Pre-Release")).unwrap();
        assert_eq!(
            workspace.restore_note(pre, None).unwrap(),
            id("Pre-Release")
        );
        let left: Vec<_> = workspace
            .deleted_notes()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(left, [id("Old")]);
    }

    /// TC-007-10: a taken place is never replaced; another title restores.
    #[test]
    fn uc007_tc10_place_taken() {
        let sandbox = Sandbox::new("tc10", &[("Plan.md", "deleted content")]);
        let mut workspace = sandbox.open();
        workspace.delete_note(&id("Plan"), 5).unwrap();
        fs::write(sandbox.0.join("plan.md"), "new").unwrap();
        workspace.rescan();
        let deleted = workspace.deleted_notes().remove(0);
        assert!(matches!(
            workspace.restore_note(&deleted, None),
            Err(NewNoteProblem::Exists(_))
        ));
        assert_eq!(workspace.deleted_notes().len(), 1);
        let other = Rename {
            folder: String::new(),
            title: "Plan Old".into(),
        };
        workspace.restore_note(&deleted, Some(&other)).unwrap();
        assert_eq!(sandbox.read("Plan-Old.md"), "deleted content");
        assert_eq!(sandbox.read("plan.md"), "new");
        assert!(workspace.deleted_notes().is_empty());
    }

    /// TC-007-11 / TC-007-14: removed for good one by one, and after 30 days.
    #[test]
    fn uc007_tc11_tc14_removed_for_good() {
        let sandbox = Sandbox::new(
            "tc14",
            &[("Old.md", ""), ("Recent.md", ""), ("Gone.md", "")],
        );
        let mut workspace = sandbox.open();
        let day = 24 * 60 * 60 * 1000;
        let now = 100 * day;
        workspace.delete_note(&id("Old"), now - 31 * day).unwrap();
        workspace
            .delete_note(&id("Recent"), now - 29 * day)
            .unwrap();
        let gone = workspace.delete_note(&id("Gone"), now).unwrap();
        workspace.forget_deleted(&gone).unwrap();
        assert_eq!(workspace.purge_deleted(now), 1);
        let left: Vec<_> = workspace
            .deleted_notes()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(left, [id("Recent")]);
        let folders = fs::read_dir(workspace.deleted_folder()).unwrap().count();
        assert_eq!(folders, 1);
    }

    /// TC-007-07 / TC-007-12: a note that cannot be kept stays, and can then
    /// be removed for good.
    #[cfg(unix)]
    #[test]
    fn uc007_tc12_cannot_keep() {
        use std::os::unix::fs::PermissionsExt;
        let sandbox = Sandbox::new(
            "tc12",
            &[("Plan.md", "plan"), (".underline/.gitignore", "*\n")],
        );
        let folder = sandbox.0.join(UNDERLINE_FOLDER);
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
        let enforced = fs::write(folder.join("probe"), "").is_err();
        let mut workspace = sandbox.open();
        let result = workspace.delete_note(&id("Plan"), 1);
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        if enforced {
            assert!(
                matches!(result, Err(DeleteProblem::CannotKeep(_))),
                "{result:?}"
            );
            assert_eq!(sandbox.read("Plan.md"), "plan");
            workspace.delete_note_for_good(&id("Plan")).unwrap();
            assert!(!sandbox.0.join("Plan.md").exists());
        }
    }
}
