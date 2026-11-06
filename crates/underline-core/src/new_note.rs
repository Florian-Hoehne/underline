//! UC-003 Create Note: a new note with the title, folder, and note format
//! the user chose, holding only its title as heading.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::atomic::write_new_atomic;
use crate::format::NoteFormat;
use crate::git::Git;
use crate::note::NoteId;
use crate::workspace::{is_workspace_directory, Workspace};

/// Longest title, and longest name of a folder (UC-003 BR-002).
pub const MAX_TITLE_CHARS: usize = 250;
/// Longest place of a note in the workspace, folders and extension included
/// (UC-003 BR-002, entity NOTE.path).
pub const MAX_PATH_CHARS: usize = 500;
/// Characters no file name may hold on any platform (UC-003 BR-002).
pub const FORBIDDEN_CHARACTERS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// What the user chose for a new note (UC-003 steps 2–3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    /// Folder inside the workspace with `/` between levels; empty for the top
    /// folder.
    pub folder: String,
    pub title: String,
    pub format: NoteFormat,
}

/// Why a title or a folder name is not allowed (UC-003 BR-002).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameProblem {
    #[error("is empty")]
    Empty,
    #[error("cannot start with a dot")]
    LeadingDot,
    #[error("cannot end with a dot")]
    TrailingDot,
    #[error("cannot hold the character `{0}`")]
    Character(char),
    #[error("cannot hold control characters")]
    ControlCharacter,
    #[error("is a name Windows reserves for devices")]
    Reserved,
    #[error("is longer than {MAX_TITLE_CHARS} characters")]
    TooLong,
}

/// Why a note cannot be created (UC-003 A1, A2, A4, A5).
#[derive(Debug, thiserror::Error)]
pub enum NewNoteProblem {
    /// A1.
    #[error("the title {0}")]
    Title(NameProblem),
    /// A4: a new or existing folder name breaks the rules of the title.
    #[error("the folder name `{name}` {problem}")]
    FolderName { name: String, problem: NameProblem },
    /// A1: the place of the note is too long.
    #[error("the place of the note is longer than {MAX_PATH_CHARS} characters")]
    PathTooLong,
    /// A4.
    #[error("the folder lies outside the workspace")]
    Outside,
    /// A4.
    #[error("the folder `{0}` is hidden")]
    Hidden(String),
    /// A4: the note would not be listed (UC-002 BR-003).
    #[error("git ignores the folder `{0}`, so the note would not be listed")]
    Ignored(String),
    /// A4.
    #[error("the folder `{0}` is a workspace of its own")]
    OwnWorkspace(String),
    /// A2: a note with this file name is there, in either format and case.
    #[error("the note `{}` exists", .0.title())]
    Exists(NoteId),
    /// A5.
    #[error("cannot {action}: {source}")]
    Io {
        action: &'static str,
        source: io::Error,
    },
}

/// UC-003 BR-001: the file name of a title without extension: spaces at the
/// ends dropped, every run of spaces turned into one dash.
pub fn page_name(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join("-")
}

/// UC-003 BR-001 / BR-004: the title as the note shows it in its heading:
/// spaces at the ends dropped, every run of spaces turned into one space.
pub fn heading_title(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// UC-003 BR-002: checks a title or a folder name.
pub fn check_name(name: &str) -> Result<(), NameProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if name.starts_with('.') {
        return Err(NameProblem::LeadingDot);
    }
    if name.ends_with('.') {
        return Err(NameProblem::TrailingDot);
    }
    if let Some(c) = name.chars().find(|c| FORBIDDEN_CHARACTERS.contains(c)) {
        return Err(NameProblem::Character(c));
    }
    if name.chars().any(char::is_control) {
        return Err(NameProblem::ControlCharacter);
    }
    if is_reserved(name) {
        return Err(NameProblem::Reserved);
    }
    if name.chars().count() > MAX_TITLE_CHARS {
        return Err(NameProblem::TooLong);
    }
    Ok(())
}

/// Windows reserves these device names, with any extension and in any case.
fn is_reserved(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|n| {
                n.len() == 1 && ('1'..='9').contains(&n.chars().next().unwrap_or('0'))
            })
        })
}

/// The folder levels the user entered, or why they cannot be used.
fn folder_levels(folder: &str) -> Result<Vec<String>, NewNoteProblem> {
    let folder = folder.trim();
    if folder.starts_with('/') || folder.starts_with('\\') {
        return Err(NewNoteProblem::Outside);
    }
    let mut levels = Vec::new();
    for level in folder.split('/').map(str::trim).filter(|l| !l.is_empty()) {
        if level == ".." {
            return Err(NewNoteProblem::Outside);
        }
        if level == "." {
            continue;
        }
        match check_name(level) {
            Ok(()) => levels.push(level.to_owned()),
            Err(NameProblem::LeadingDot) => return Err(NewNoteProblem::Hidden(level.to_owned())),
            Err(problem) => {
                return Err(NewNoteProblem::FolderName {
                    name: level.to_owned(),
                    problem,
                })
            }
        }
    }
    Ok(levels)
}

impl Workspace {
    /// UC-003 step 4: checks the title, the folder, and that the folder holds
    /// no note with that file name. Returns the id the note will get; the
    /// dialog shows its file name while the user types (step 2).
    pub fn plan_note(&self, new: &NewNote) -> Result<NoteId, NewNoteProblem> {
        check_name(&new.title).map_err(NewNoteProblem::Title)?;
        let levels = folder_levels(&new.folder)?;
        let page = page_name(&new.title);
        let mut parts = levels.clone();
        parts.push(page.clone());
        let id = NoteId::from_relative(&parts.join("/"));
        if id.file_name(new.format).chars().count() > MAX_PATH_CHARS {
            return Err(NewNoteProblem::PathTooLong);
        }

        let mut dir = self.root.clone();
        for (depth, level) in levels.iter().enumerate() {
            dir.push(level);
            if dir.is_dir() && is_workspace_directory(&dir) {
                return Err(NewNoteProblem::OwnWorkspace(levels[..=depth].join("/")));
            }
        }
        if !levels.is_empty() && Git::new().is_ignored(&self.root, &id.file_name(new.format)) {
            return Err(NewNoteProblem::Ignored(levels.join("/")));
        }
        if let Some(existing) = existing_note(&dir, &page) {
            let mut parts = levels;
            parts.push(existing);
            return Err(NewNoteProblem::Exists(NoteId::from_relative(
                &parts.join("/"),
            )));
        }
        Ok(id)
    }

    /// UC-003 step 5: creates the note with `content`, creating missing
    /// folders, without ever replacing a file that is there. On failure the
    /// folders created for the note are removed again (A5).
    pub fn create_note(&mut self, new: &NewNote, content: &str) -> Result<NoteId, NewNoteProblem> {
        let id = self.plan_note(new)?;
        let path = self.root.join(id.file_name(new.format));
        let folder = path.parent().unwrap_or(&self.root).to_owned();
        let created = topmost_missing(&self.root, &folder);
        let result = fs::create_dir_all(&folder)
            .map_err(|source| NewNoteProblem::Io {
                action: "create the folder",
                source,
            })
            .and_then(|()| {
                write_new_atomic(&path, content.as_bytes()).map_err(|source| {
                    if source.kind() == io::ErrorKind::AlreadyExists {
                        NewNoteProblem::Exists(id.clone())
                    } else {
                        NewNoteProblem::Io {
                            action: "write the note",
                            source,
                        }
                    }
                })
            });
        if result.is_err() {
            if let Some(created) = created {
                let _ = fs::remove_dir_all(created);
            }
            return result.map(|()| id);
        }
        self.rescan();
        Ok(id)
    }
}

/// The file name (without extension) of a note in `dir` whose name matches
/// `page`, ignoring case and note format (UC-003 BR-003).
fn existing_note(dir: &Path, page: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    entries.filter_map(Result::ok).find_map(|entry| {
        let name = entry.file_name();
        let name = name.to_str()?;
        let (stem, extension) = name.rsplit_once('.')?;
        (NoteFormat::from_extension(extension).is_some()
            && stem.to_lowercase() == page.to_lowercase())
        .then(|| stem.to_owned())
    })
}

/// The topmost folder between `root` and `folder` that does not exist yet.
fn topmost_missing(root: &Path, folder: &Path) -> Option<PathBuf> {
    folder
        .ancestors()
        .take_while(|dir| *dir != root && !dir.exists())
        .last()
        .map(Path::to_owned)
}

/// Test cases TC-003-01 to TC-003-16 of UC-003, as far as they concern the
/// domain; the dialog is checked by hand.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{WorkspaceSettings, SETTINGS_FILE_NAME};
    use std::process::Command;

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let root =
                std::env::temp_dir().join(format!("underline-uc003-{name}-{}", std::process::id()));
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

    fn note(folder: &str, title: &str, format: NoteFormat) -> NewNote {
        NewNote {
            folder: folder.to_owned(),
            title: title.to_owned(),
            format,
        }
    }

    fn heading(new: &NewNote) -> String {
        match new.format {
            NoteFormat::Markdown => format!("# {}\n", new.title.trim()),
            NoteFormat::AsciiDoc => format!("= {}\n", new.title.trim()),
        }
    }

    fn create(workspace: &mut Workspace, new: &NewNote) -> Result<NoteId, NewNoteProblem> {
        workspace.create_note(new, &heading(new))
    }

    /// TC-003-01: a Markdown note in the top folder holds only its heading.
    #[test]
    fn uc003_tc01_creates_markdown_note_in_top_folder() {
        let sandbox = Sandbox::new("tc01", &[("Home.md", ""), ("Ideas.md", "")]);
        let mut workspace = sandbox.open();
        let new = note("", "Meeting Notes", NoteFormat::Markdown);
        let planned = workspace.plan_note(&new).unwrap();
        assert_eq!(planned.file_name(new.format), "Meeting-Notes.md");

        let id = create(&mut workspace, &new).unwrap();
        assert_eq!(id, planned);
        assert_eq!(sandbox.read("Meeting-Notes.md"), "# Meeting Notes\n");
        assert!(workspace.listing().notes.iter().any(|e| e.id == id));
    }

    /// TC-003-02 / TC-003-05: notes in a chosen folder, also in new folders
    /// several levels deep.
    #[test]
    fn uc003_tc02_tc05_creates_notes_in_folders() {
        let sandbox = Sandbox::new("tc02", &[("guide/Install.md", "")]);
        let mut workspace = sandbox.open();
        create(
            &mut workspace,
            &note("guide", "Upgrade", NoteFormat::Markdown),
        )
        .unwrap();
        create(
            &mut workspace,
            &note("guide/setup/", "Linux", NoteFormat::Markdown),
        )
        .unwrap();
        assert_eq!(sandbox.read("guide/Upgrade.md"), "# Upgrade\n");
        assert_eq!(sandbox.read("guide/setup/Linux.md"), "# Linux\n");
    }

    /// TC-003-03: another format for one note; the recorded default stays.
    #[test]
    fn uc003_tc03_other_format_keeps_recorded_default() {
        let sandbox = Sandbox::new(
            "tc03",
            &[(SETTINGS_FILE_NAME, "default_format: markdown\n")],
        );
        let mut workspace = sandbox.open();
        create(&mut workspace, &note("", "Spec", NoteFormat::AsciiDoc)).unwrap();
        assert_eq!(sandbox.read("Spec.adoc"), "= Spec\n");
        assert_eq!(workspace.default_format(), NoteFormat::Markdown);
        assert_eq!(
            WorkspaceSettings::load(&sandbox.0).unwrap().default_format,
            Some(NoteFormat::Markdown)
        );
    }

    /// TC-003-04: a derived default format follows the new note.
    #[test]
    fn uc003_tc04_derived_default_follows_the_notes() {
        let sandbox = Sandbox::new("tc04", &[("A.adoc", ""), ("B.md", "")]);
        let mut workspace = sandbox.open();
        assert_eq!(workspace.default_format(), NoteFormat::Markdown);
        create(&mut workspace, &note("", "C", NoteFormat::AsciiDoc)).unwrap();
        assert_eq!(workspace.default_format(), NoteFormat::AsciiDoc);
        assert!(!sandbox.0.join(SETTINGS_FILE_NAME).exists());
    }

    /// TC-003-06: the title becomes the file name (BR-001).
    #[test]
    fn uc003_tc06_title_becomes_file_name() {
        assert_eq!(page_name("  Getting   Started  "), "Getting-Started");
        assert_eq!(page_name("Pre-Release"), "Pre-Release");
        assert_eq!(page_name("Über uns"), "Über-uns");
        assert_eq!(heading_title("  Meeting   Notes "), "Meeting Notes");
        assert_eq!(NoteId::from_title("Pre-Release").title(), "Pre Release");
        let sandbox = Sandbox::new("tc06", &[]);
        let mut workspace = sandbox.open();
        create(&mut workspace, &note("", "Über uns", NoteFormat::Markdown)).unwrap();
        assert!(sandbox.0.join("Über-uns.md").is_file());
    }

    /// TC-003-07: titles that are not allowed, the same on every platform.
    #[test]
    fn uc003_tc07_refuses_titles_that_are_not_allowed() {
        assert_eq!(check_name("  "), Err(NameProblem::Empty));
        assert_eq!(check_name(".hidden"), Err(NameProblem::LeadingDot));
        assert_eq!(check_name("Draft."), Err(NameProblem::TrailingDot));
        for c in FORBIDDEN_CHARACTERS {
            assert_eq!(
                check_name(&format!("a{c}b")),
                Err(NameProblem::Character(c))
            );
        }
        assert_eq!(check_name("a\tb"), Err(NameProblem::ControlCharacter));
        for reserved in ["CON", "nul", "Com1", "LPT9", "aux.backup"] {
            assert_eq!(
                check_name(reserved),
                Err(NameProblem::Reserved),
                "{reserved}"
            );
        }
        for allowed in ["Console", "COM10", "LPT0", "Notes 2026"] {
            assert_eq!(check_name(allowed), Ok(()), "{allowed}");
        }
        let sandbox = Sandbox::new("tc07", &[]);
        let mut workspace = sandbox.open();
        assert!(matches!(
            create(&mut workspace, &note("", "a:b", NoteFormat::Markdown)),
            Err(NewNoteProblem::Title(NameProblem::Character(':')))
        ));
        assert_eq!(fs::read_dir(&sandbox.0).unwrap().count(), 0);
    }

    /// TC-003-08: title and place too long.
    #[test]
    fn uc003_tc08_refuses_titles_and_places_that_are_too_long() {
        let sandbox = Sandbox::new("tc08", &[]);
        let mut workspace = sandbox.open();
        let long = "x".repeat(MAX_TITLE_CHARS + 1);
        assert!(matches!(
            workspace.plan_note(&note("", &long, NoteFormat::Markdown)),
            Err(NewNoteProblem::Title(NameProblem::TooLong))
        ));
        let exact = "y".repeat(MAX_TITLE_CHARS);
        create(&mut workspace, &note("", &exact, NoteFormat::Markdown)).unwrap();
        let folder = ["f".repeat(150), "g".repeat(150)].join("/");
        assert!(matches!(
            workspace.plan_note(&note(&folder, &exact, NoteFormat::Markdown)),
            Err(NewNoteProblem::PathTooLong)
        ));
    }

    /// TC-003-09 / TC-003-10: an existing note, whatever its case, format, or
    /// typed title, is found and left as it is (A2, BR-003).
    #[test]
    fn uc003_tc09_tc10_refuses_existing_notes() {
        let sandbox = Sandbox::new(
            "tc09",
            &[
                ("Plan.md", "keep me"),
                ("guide/Spec.adoc", ""),
                ("Pre-Release.md", ""),
            ],
        );
        let mut workspace = sandbox.open();
        for (folder, title, format) in [
            ("", "Plan", NoteFormat::Markdown),
            ("", "plan", NoteFormat::Markdown),
            ("", "Plan", NoteFormat::AsciiDoc),
            ("guide", "spec", NoteFormat::Markdown),
            ("", "Pre Release", NoteFormat::Markdown),
        ] {
            match create(&mut workspace, &note(folder, title, format)) {
                Err(NewNoteProblem::Exists(_)) => {}
                other => panic!("{folder}/{title}: expected Exists, got {other:?}"),
            }
        }
        match workspace.plan_note(&note("", "plan", NoteFormat::Markdown)) {
            Err(NewNoteProblem::Exists(id)) => assert_eq!(id, NoteId::from_title("Plan")),
            other => panic!("expected Exists, got {other:?}"),
        }
        assert_eq!(sandbox.read("Plan.md"), "keep me");
        create(&mut workspace, &note("", "Spec", NoteFormat::Markdown)).unwrap();
    }

    /// TC-003-11: a file that appears after the check is never replaced.
    #[test]
    fn uc003_tc11_never_replaces_a_file_that_appears() {
        let sandbox = Sandbox::new("tc11", &[]);
        let mut workspace = sandbox.open();
        let new = note("", "Race", NoteFormat::Markdown);
        workspace.plan_note(&new).unwrap();
        fs::write(sandbox.0.join("Race.md"), "other program").unwrap();
        assert!(matches!(
            create(&mut workspace, &new),
            Err(NewNoteProblem::Exists(_))
        ));
        assert_eq!(sandbox.read("Race.md"), "other program");
    }

    /// TC-003-12: folders that cannot take a note (A4).
    #[test]
    fn uc003_tc12_refuses_folders_that_cannot_take_a_note() {
        let sandbox = Sandbox::new(
            "tc12",
            &[(".gitignore", "vendor/\n"), ("sub/underline.yaml", "")],
        );
        sandbox.git(&["init", "--quiet"]);
        let mut workspace = sandbox.open();
        type Expect = fn(&NewNoteProblem) -> bool;
        let cases: [(&str, Expect); 6] = [
            ("../outside", |p| matches!(p, NewNoteProblem::Outside)),
            ("/etc", |p| matches!(p, NewNoteProblem::Outside)),
            (".cache", |p| matches!(p, NewNoteProblem::Hidden(_))),
            ("vendor/lib", |p| matches!(p, NewNoteProblem::Ignored(_))),
            ("sub", |p| matches!(p, NewNoteProblem::OwnWorkspace(_))),
            ("docs/CON", |p| {
                matches!(
                    p,
                    NewNoteProblem::FolderName {
                        problem: NameProblem::Reserved,
                        ..
                    }
                )
            }),
        ];
        for (folder, expected) in cases {
            match create(&mut workspace, &note(folder, "X", NoteFormat::Markdown)) {
                Err(problem) => assert!(expected(&problem), "{folder}: {problem:?}"),
                Ok(id) => panic!("{folder}: created {id}"),
            }
        }
        let mut names: Vec<_> = fs::read_dir(&sandbox.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, [".git", ".gitignore", "sub"]);
    }

    /// TC-003-13: a note that cannot be created leaves no folder behind (A5).
    #[test]
    fn uc003_tc13_leaves_nothing_behind_on_failure() {
        let sandbox = Sandbox::new("tc13", &[("full", "a file where a folder should be")]);
        let mut workspace = sandbox.open();
        let result = create(
            &mut workspace,
            &note("new/full/deep", "Y", NoteFormat::Markdown),
        );
        assert!(result.is_ok());
        let blocked = create(
            &mut workspace,
            &note("full/deep", "Y", NoteFormat::Markdown),
        );
        assert!(
            matches!(blocked, Err(NewNoteProblem::Io { .. })),
            "{blocked:?}"
        );
        assert_eq!(sandbox.read("full"), "a file where a folder should be");
        assert!(!sandbox.0.join("full/deep").exists());
    }

    /// TC-003-16: in a git repository the new note is the only new file.
    #[test]
    fn uc003_tc16_new_note_is_the_only_untracked_file() {
        let sandbox = Sandbox::new("tc16", &[("Home.md", "")]);
        sandbox.git(&["init", "--quiet"]);
        sandbox.git(&["add", "Home.md"]);
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
        create(&mut workspace, &note("", "New Page", NoteFormat::Markdown)).unwrap();
        assert_eq!(
            sandbox.git(&["status", "--porcelain", "--untracked-files=all"]),
            "?? New-Page.md\n"
        );
    }
}
