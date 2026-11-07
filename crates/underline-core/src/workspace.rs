use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::git::Git;
use crate::note::{Note, NoteId};
use crate::settings::{WorkspaceSettings, SETTINGS_FILE_NAME};
use crate::state::{Timestamp, UNDERLINE_FOLDER};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("note `{0}` not found")]
    NotFound(NoteId),
    #[error("invalid note id `{0}`")]
    InvalidId(String),
    #[error("{} does not exist", .0.display())]
    DirectoryMissing(PathBuf),
    #[error("{} is a file, not a directory", .0.display())]
    NotADirectory(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// One note found in the workspace, with the note format of its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    pub id: NoteId,
    pub format: NoteFormat,
}

/// The notes of a workspace and the folders that could not be read
/// (UC-002 step 7, A13).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub notes: Vec<NoteEntry>,
    pub unreadable: Vec<PathBuf>,
}

/// A local directory whose Markdown and AsciiDoc files are shown as notes.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub(crate) root: PathBuf,
    /// The default format of the workspace settings file, if it names one.
    recorded_format: Option<NoteFormat>,
    settings_problem: Option<String>,
    excluded: Vec<PathBuf>,
    listing: Listing,
}

impl Workspace {
    /// UC-002 steps 6–7: opens the existing directory `root` as a workspace.
    ///
    /// Opening changes nothing in the directory (BR-001). The default format
    /// comes from the workspace settings file or is derived from the notes
    /// (BR-002); a settings file that cannot be read is reported through
    /// [`Workspace::settings_problem`] and otherwise ignored (A8).
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        Self::open_excluding(root, &[])
    }

    /// Like [`Workspace::open`], leaving out the folders of `known`
    /// workspaces that lie inside this one (UC-002 BR-003, BR-007).
    pub fn open_excluding(
        root: impl Into<PathBuf>,
        known: &[PathBuf],
    ) -> Result<Self, WorkspaceError> {
        let root = root.into();
        match fs::metadata(&root) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Err(WorkspaceError::DirectoryMissing(root))
            }
            Err(err) => return Err(err.into()),
            Ok(meta) if !meta.is_dir() => return Err(WorkspaceError::NotADirectory(root)),
            Ok(_) => {}
        }
        fs::read_dir(&root)?;

        let (settings, settings_problem) = match WorkspaceSettings::load(&root) {
            Ok(settings) => (settings, None),
            Err(err) => (WorkspaceSettings::default(), Some(err.to_string())),
        };
        let absolute_root = std::path::absolute(&root).unwrap_or_else(|_| root.clone());
        let excluded = known
            .iter()
            .filter_map(|dir| std::path::absolute(dir).ok())
            .filter(|dir| dir != &absolute_root && dir.starts_with(&absolute_root))
            .collect();
        let mut workspace = Self {
            root,
            recorded_format: settings.default_format,
            settings_problem,
            excluded,
            listing: Listing::default(),
        };
        workspace.rescan();
        Ok(workspace)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Note format given to new notes of this workspace: the one recorded in
    /// the workspace settings file, else the one most notes have (UC-002
    /// BR-002, UC-003 BR-005).
    pub fn default_format(&self) -> NoteFormat {
        self.recorded_format
            .unwrap_or_else(|| derived_format(&self.listing.notes))
    }

    /// Why the workspace settings file could not be read, if it could not
    /// (UC-002 A8).
    pub fn settings_problem(&self) -> Option<&str> {
        self.settings_problem.as_deref()
    }

    /// Reads the workspace settings file again after it changed (ADR-011);
    /// a file that cannot be read is reported as when opening (UC-002 A8).
    pub fn reload_settings(&mut self) {
        match WorkspaceSettings::load(&self.root) {
            Ok(settings) => {
                self.recorded_format = settings.default_format;
                self.settings_problem = None;
            }
            // UC-010 A6: the last settings that could be read stay.
            Err(err) => self.settings_problem = Some(err.to_string()),
        }
    }

    /// The notes found when the workspace was opened or last rescanned.
    pub fn listing(&self) -> &Listing {
        &self.listing
    }

    /// Looks for the notes again, for example after a note was saved.
    pub fn rescan(&mut self) {
        self.listing = self.scan();
    }

    /// Lists the notes in the workspace now, sorted by id.
    pub fn list(&self) -> Result<Vec<NoteId>, WorkspaceError> {
        Ok(self
            .scan()
            .notes
            .into_iter()
            .map(|entry| entry.id)
            .collect())
    }

    /// UC-002 BR-005: the note to open when the workspace is opened for the
    /// first time: Home, else README of the top folder, else none.
    pub fn first_note(&self) -> Option<NoteId> {
        ["home", "readme"].into_iter().find_map(|name| {
            self.listing
                .notes
                .iter()
                .find(|entry| {
                    entry.id.folder().is_none() && entry.id.as_str().eq_ignore_ascii_case(name)
                })
                .map(|entry| entry.id.clone())
        })
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
            None => self.root.join(note.id.file_name(self.default_format())),
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        write_atomic(&path, note.body.as_bytes())?;
        Ok(())
    }

    /// The file of note `id`: the existing one, or where a new note in the
    /// default format would be stored.
    pub fn note_path(&self, id: &NoteId) -> Result<PathBuf, WorkspaceError> {
        Ok(match self.existing_path(id)? {
            Some(path) => path,
            None => self.root.join(id.file_name(self.default_format())),
        })
    }

    /// The file of note `id`, if it exists in any known note format.
    fn existing_path(&self, id: &NoteId) -> Result<Option<PathBuf>, WorkspaceError> {
        if !id.is_valid() {
            return Err(WorkspaceError::InvalidId(id.as_str().to_owned()));
        }
        Ok(NoteFormat::ALL
            .into_iter()
            .map(|format| self.root.join(id.file_name(format)))
            .find(|path| path.is_file()))
    }

    /// UC-002 BR-003: every `.md` and `.adoc` file in the directory and its
    /// folders, except hidden files and folders, what git ignores, and
    /// folders that are workspaces of their own.
    fn scan(&self) -> Listing {
        let ignored: HashSet<PathBuf> = Git::new()
            .ignored_paths(&self.root)
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut listing = Listing::default();
        let mut seen = HashSet::new();
        let mut folders = vec![PathBuf::new()];
        while let Some(relative) = folders.pop() {
            let dir = self.root.join(&relative);
            let Ok(entries) = fs::read_dir(&dir) else {
                listing.unreadable.push(dir);
                continue;
            };
            for entry in entries {
                let Ok(entry) = entry else {
                    listing.unreadable.push(dir.clone());
                    continue;
                };
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if name.starts_with('.') {
                    continue;
                }
                let path = relative.join(name);
                if ignored.contains(&path) {
                    continue;
                }
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    let absolute = self.root.join(&path);
                    let excluded = self
                        .excluded
                        .iter()
                        .any(|dir| std::path::absolute(&absolute).ok().as_ref() == Some(dir));
                    if !excluded && !is_workspace_directory(&absolute) {
                        folders.push(path);
                    }
                    continue;
                }
                let Some((stem, extension)) = name.rsplit_once('.') else {
                    continue;
                };
                let Some(format) = NoteFormat::from_extension(extension) else {
                    continue;
                };
                let mut id = relative
                    .iter()
                    .map(|part| part.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                id.push(stem.to_owned());
                let id = NoteId::from_relative(&id.join("/"));
                if seen.insert(id.clone()) {
                    listing.notes.push(NoteEntry { id, format });
                }
            }
        }
        listing.notes.sort_by(|a, b| a.id.cmp(&b.id));
        listing.unreadable.sort();
        listing
    }
}

/// Whether `dir` holds a workspace of its own: a workspace settings file or
/// an Underline folder (UC-002 BR-007).
pub fn is_workspace_directory(dir: &Path) -> bool {
    dir.join(SETTINGS_FILE_NAME).is_file() || dir.join(UNDERLINE_FOLDER).is_dir()
}

/// UC-002 BR-002: the note format most notes have, Markdown on a tie or
/// without notes.
fn derived_format(notes: &[NoteEntry]) -> NoteFormat {
    let asciidoc = notes
        .iter()
        .filter(|entry| entry.format == NoteFormat::AsciiDoc)
        .count();
    if asciidoc * 2 > notes.len() {
        NoteFormat::AsciiDoc
    } else {
        NoteFormat::Markdown
    }
}

/// One line of the note overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewEntry {
    pub id: NoteId,
    /// The title, followed by its folder when the title occurs more than once
    /// in the workspace (UC-002 BR-004).
    pub label: String,
}

/// UC-002 BR-008: the recently opened notes first, the most recent on top,
/// then all other notes by title from A to Z, ignoring case.
pub fn note_overview(
    notes: &[NoteEntry],
    mut last_opened: impl FnMut(&NoteId) -> Option<Timestamp>,
) -> Vec<OverviewEntry> {
    let mut titles = std::collections::HashMap::<String, usize>::new();
    for entry in notes {
        *titles.entry(entry.id.title().to_lowercase()).or_default() += 1;
    }
    let mut ordered: Vec<_> = notes
        .iter()
        .map(|entry| (last_opened(&entry.id), entry))
        .collect();
    ordered.sort_by(|(a_opened, a), (b_opened, b)| {
        b_opened
            .cmp(a_opened)
            .then_with(|| {
                a.id.title()
                    .to_lowercase()
                    .cmp(&b.id.title().to_lowercase())
            })
            .then_with(|| a.id.folder().cmp(&b.id.folder()))
    });
    ordered
        .into_iter()
        .map(|(_, entry)| {
            let title = entry.id.title();
            let repeated = titles.get(&title.to_lowercase()).copied().unwrap_or(0) > 1;
            let label = match (repeated, entry.id.folder()) {
                (true, Some(folder)) => format!("{title}  ({folder})"),
                (true, None) => format!("{title}  (top folder)"),
                _ => title,
            };
            OverviewEntry {
                id: entry.id.clone(),
                label,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("underline-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temp_workspace(name: &str) -> Workspace {
        Workspace::open(temp_dir(name)).unwrap()
    }

    fn files(root: &Path, files: &[(&str, &str)]) {
        for (path, content) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }

    fn ids(workspace: &Workspace) -> Vec<String> {
        workspace
            .listing()
            .notes
            .iter()
            .map(|entry| entry.id.as_str().to_owned())
            .collect()
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

    /// UC-002 A3 / BR-001: a missing directory is not created.
    #[test]
    fn uc002_tc13_missing_directory_is_not_created() {
        let dir = std::env::temp_dir().join(format!("underline-uc002-gone-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let err = Workspace::open(&dir).unwrap_err();
        assert!(matches!(err, WorkspaceError::DirectoryMissing(_)));
        assert!(!dir.exists());
    }

    /// TC-002-02: opening changes nothing in the directory (BR-001).
    #[test]
    fn uc002_tc02_opening_changes_nothing() {
        let root = temp_dir("uc002-tc02");
        files(&root, &[("Home.md", "# Home\n"), ("Setup.md", "x")]);
        let workspace = Workspace::open(&root).unwrap();
        assert_eq!(ids(&workspace), ["Home", "Setup"]);
        assert_eq!(workspace.first_note(), Some(NoteId::from_title("Home")));
        let mut names: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["Home.md", "Setup.md"]);
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-03: without Home the README of the top folder opens, without
    /// either no note (BR-005).
    #[test]
    fn uc002_tc03_first_note_is_home_else_readme_else_none() {
        let a = temp_dir("uc002-tc03a");
        files(
            &a,
            &[("README.md", ""), ("Guide.md", ""), ("sub/Home.md", "")],
        );
        assert_eq!(
            Workspace::open(&a).unwrap().first_note(),
            Some(NoteId::from_title("README"))
        );
        let b = temp_dir("uc002-tc03b");
        files(&b, &[("Guide.md", "")]);
        assert_eq!(Workspace::open(&b).unwrap().first_note(), None);
        let c = temp_dir("uc002-tc03c");
        files(&c, &[("home.md", ""), ("README.md", "")]);
        assert_eq!(
            Workspace::open(&c).unwrap().first_note(),
            Some(NoteId::from_title("home"))
        );
        for dir in [a, b, c] {
            fs::remove_dir_all(dir).unwrap();
        }
    }

    /// TC-002-04: the default format is derived from the notes (BR-002).
    #[test]
    fn uc002_tc04_derives_the_default_format() {
        type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], NoteFormat);
        let cases: [Case; 3] = [
            (
                "mostly",
                &[("A.adoc", ""), ("B.adoc", ""), ("C.md", "")],
                NoteFormat::AsciiDoc,
            ),
            ("tie", &[("A.adoc", ""), ("C.md", "")], NoteFormat::Markdown),
            ("empty", &[], NoteFormat::Markdown),
        ];
        for (name, notes, expected) in cases {
            let root = temp_dir(&format!("uc002-tc04-{name}"));
            files(&root, notes);
            let workspace = Workspace::open(&root).unwrap();
            assert_eq!(workspace.default_format(), expected, "{name}");
            workspace.write(&Note::new("New", "")).unwrap();
            assert!(root
                .join(NoteId::from_title("New").file_name(expected))
                .is_file());
            assert!(!root.join(SETTINGS_FILE_NAME).exists());
            fs::remove_dir_all(&root).unwrap();
        }
    }

    /// TC-002-05: which files are notes (BR-003).
    #[test]
    fn uc002_tc05_lists_only_the_notes_of_this_workspace() {
        let root = temp_dir("uc002-tc05");
        files(
            &root,
            &[
                ("Home.md", ""),
                ("guide/Install.adoc", ""),
                ("notes.txt", ""),
                (".draft.md", ""),
                (".cache/Old.md", ""),
                ("vendor/lib/README.md", ""),
                (".gitignore", "vendor/\n"),
                ("sub/underline.yaml", ""),
                ("sub/Inner.md", ""),
                ("other/Known.md", ""),
            ],
        );
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .status()
            .unwrap();
        assert!(status.success());

        let workspace = Workspace::open_excluding(&root, &[root.join("other")]).unwrap();
        assert_eq!(ids(&workspace), ["Home", "guide/Install"]);
        assert_eq!(workspace.listing().notes[1].format, NoteFormat::AsciiDoc);
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-06 / TC-002-07: overview order and folders for repeated titles
    /// (BR-004, BR-008).
    #[test]
    fn uc002_tc06_tc07_orders_the_note_overview() {
        let root = temp_dir("uc002-tc07");
        files(
            &root,
            &[
                ("Zebra.md", ""),
                ("apple.md", ""),
                ("Mango.md", ""),
                ("Berry.md", ""),
                ("a/Home.md", ""),
                ("b/Home.md", ""),
            ],
        );
        let workspace = Workspace::open(&root).unwrap();
        let opened = |id: &NoteId| match id.as_str() {
            "Mango" => Some(1),
            "Zebra" => Some(2),
            _ => None,
        };
        let labels: Vec<_> = note_overview(&workspace.listing().notes, opened)
            .into_iter()
            .map(|entry| entry.label)
            .collect();
        assert_eq!(
            labels,
            ["Zebra", "Mango", "apple", "Berry", "Home  (a)", "Home  (b)"]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-16: a broken workspace settings file is reported and left as
    /// it is; the default format is derived (A8).
    #[test]
    fn uc002_tc16_broken_settings_file_is_reported_and_kept() {
        let root = temp_dir("uc002-tc16");
        let broken = "default_format: [oops\n";
        files(
            &root,
            &[
                ("Home.adoc", ""),
                ("Plan.adoc", ""),
                (SETTINGS_FILE_NAME, broken),
            ],
        );
        let workspace = Workspace::open(&root).unwrap();
        assert!(workspace.settings_problem().is_some());
        assert_eq!(workspace.default_format(), NoteFormat::AsciiDoc);
        assert_eq!(
            fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
            broken
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-21: a folder that cannot be read is named; the other notes are
    /// listed (A13). Skipped where permissions do not apply (as root).
    #[cfg(unix)]
    #[test]
    fn uc002_tc21_names_unreadable_folders() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("uc002-tc21");
        files(
            &root,
            &[("Home.md", ""), ("Plan.md", ""), ("private/Secret.md", "")],
        );
        let private = root.join("private");
        fs::set_permissions(&private, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_dir(&private).is_ok() {
            fs::set_permissions(&private, fs::Permissions::from_mode(0o755)).unwrap();
            fs::remove_dir_all(&root).unwrap();
            return;
        }
        let workspace = Workspace::open(&root).unwrap();
        assert_eq!(ids(&workspace), ["Home", "Plan"]);
        assert_eq!(
            workspace.listing().unreadable,
            std::slice::from_ref(&private)
        );
        fs::set_permissions(&private, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-22 (NFR-003): 10,000 notes in 100 folders are listed within
    /// 1 second. Run with `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn uc002_tc22_lists_10000_notes_within_a_second() {
        let root = temp_dir("uc002-tc22");
        for folder in 0..100 {
            let dir = root.join(format!("f{folder}"));
            fs::create_dir_all(&dir).unwrap();
            for note in 0..100 {
                fs::write(dir.join(format!("Note-{note}.md")), "").unwrap();
            }
        }
        let start = std::time::Instant::now();
        let workspace = Workspace::open(&root).unwrap();
        let overview = note_overview(&workspace.listing().notes, |_| None);
        assert_eq!(overview.len(), 10_000);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            start.elapsed()
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
