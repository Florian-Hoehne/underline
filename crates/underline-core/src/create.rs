//! UC-001 Create Workspace: a new workspace that holds only its Home note, in
//! the default format the user chose, optionally as a git repository.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::git::{Git, GitError};
use crate::note::NoteId;
use crate::settings::{SettingsError, WorkspaceSettings, SETTINGS_FILE_NAME};
use crate::workspace::{is_workspace_directory, Workspace, WorkspaceError};

/// Title of the note every new workspace starts with (UC-001 BR-003).
pub const HOME_TITLE: &str = "Home";
/// Title of the first commit of a workspace created as a git repository
/// (UC-001 BR-007).
pub const FIRST_COMMIT_MESSAGE: &str = "Create workspace";
/// Name the git remote is recorded under (UC-001 BR-004).
pub const REMOTE_NAME: &str = "origin";
/// Branch used when the user's git setup names no default branch (BR-007).
pub const FALLBACK_BRANCH: &str = "main";

/// What the user chose for a new workspace (UC-001 steps 1–4, A3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkspace {
    pub directory: PathBuf,
    pub default_format: NoteFormat,
    /// `Some` when the user switched the git repository option on.
    pub git: Option<GitRepositoryOption>,
}

/// The git repository option of UC-001 A3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitRepositoryOption {
    /// Address of the git remote; recorded as given, never contacted.
    pub remote: Option<String>,
}

/// Why a directory cannot take a new workspace (UC-001 step 2).
#[derive(Debug, thiserror::Error)]
pub enum DirectoryProblem {
    /// A1: the directory holds files or folders, hidden ones included (BR-002).
    #[error("{} already holds files or folders", .0.display())]
    NotEmpty(PathBuf),
    /// A7: the directory lies inside an existing workspace (BR-006).
    #[error("{} lies inside the workspace {}", .directory.display(), .workspace.display())]
    InsideWorkspace {
        directory: PathBuf,
        workspace: PathBuf,
    },
    #[error("{} is a file, not a directory", .0.display())]
    NotADirectory(PathBuf),
    #[error("cannot look into {}: {source}", .path.display())]
    Unreadable { path: PathBuf, source: io::Error },
}

/// What a directory is checked against besides its own content.
#[derive(Debug, Clone, Default)]
pub struct Surroundings {
    /// Workspaces Underline knows, for example the open one.
    pub known_workspaces: Vec<PathBuf>,
    /// Directory of the user settings file, whose `underline.yaml` does not
    /// mark a workspace.
    pub user_settings_dir: Option<PathBuf>,
}

/// UC-001 step 2: checks that `directory` is new or empty and does not lie
/// inside an existing workspace.
pub fn check_directory(directory: &Path, around: &Surroundings) -> Result<(), DirectoryProblem> {
    let directory = absolute(directory);
    if let Some(workspace) = enclosing_workspace(&directory, around) {
        return Err(DirectoryProblem::InsideWorkspace {
            directory,
            workspace,
        });
    }
    check_new_or_empty(&directory)
}

fn check_new_or_empty(directory: &Path) -> Result<(), DirectoryProblem> {
    match fs::metadata(directory) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(DirectoryProblem::Unreadable {
                path: directory.to_owned(),
                source,
            })
        }
        Ok(meta) if !meta.is_dir() => {
            return Err(DirectoryProblem::NotADirectory(directory.to_owned()))
        }
        Ok(_) => {}
    }
    let mut entries = fs::read_dir(directory).map_err(|source| DirectoryProblem::Unreadable {
        path: directory.to_owned(),
        source,
    })?;
    match entries.next() {
        None => Ok(()),
        Some(_) => Err(DirectoryProblem::NotEmpty(directory.to_owned())),
    }
}

/// The known workspace `directory` lies inside, if any (UC-001 BR-006,
/// UC-002 BR-007): a remembered one, or an enclosing directory that holds a
/// workspace settings file or an Underline folder. The user's home directory
/// is never a workspace.
pub(crate) fn enclosing_workspace(directory: &Path, around: &Surroundings) -> Option<PathBuf> {
    for known in &around.known_workspaces {
        let known = absolute(known);
        if directory != known && directory.starts_with(&known) {
            return Some(known);
        }
    }
    let user_dir = around.user_settings_dir.as_deref().map(absolute);
    directory
        .ancestors()
        .skip(1)
        .filter(|ancestor| Some(*ancestor) != user_dir.as_deref())
        .find(|ancestor| is_workspace_directory(ancestor))
        .map(Path::to_owned)
}

/// The absolute, lexically normalized form of `path`.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}

/// What went wrong while creating a workspace (UC-001 A5).
#[derive(Debug, thiserror::Error)]
pub enum CreateProblem {
    #[error(transparent)]
    Directory(#[from] DirectoryProblem),
    #[error("cannot {action}: {source}")]
    Io {
        action: &'static str,
        source: io::Error,
    },
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Open(#[from] WorkspaceError),
}

/// A workspace that could not be created, with what could not be removed
/// afterwards (UC-001 A5, A6).
#[derive(Debug, thiserror::Error)]
#[error("{problem}")]
pub struct CreateError {
    pub problem: CreateProblem,
    /// Directories or files Underline created but could not remove (A6).
    pub leftovers: Vec<PathBuf>,
}

impl CreateError {
    fn before_anything(problem: impl Into<CreateProblem>) -> Self {
        Self {
            problem: problem.into(),
            leftovers: Vec::new(),
        }
    }
}

/// UC-001 steps 5 and A3: creates the workspace described by `request`.
///
/// `home_note` is the content of the Home note in the default format; the
/// facade engine writes it (ADR-003). The directory is checked again first.
/// On any failure everything created for the workspace is removed, and what
/// cannot be removed is named in the error.
pub fn create_workspace(
    request: &NewWorkspace,
    home_note: &str,
    around: &Surroundings,
    git: &Git,
) -> Result<Workspace, CreateError> {
    check_directory(&request.directory, around).map_err(CreateError::before_anything)?;
    let directory = absolute(&request.directory);
    let created = Created::make_directory(&directory).map_err(CreateError::before_anything)?;

    match fill(&directory, request, home_note, git) {
        Ok(workspace) => Ok(workspace),
        Err(problem) => Err(CreateError {
            problem,
            leftovers: created.remove(),
        }),
    }
}

fn fill(
    directory: &Path,
    request: &NewWorkspace,
    home_note: &str,
    git: &Git,
) -> Result<Workspace, CreateProblem> {
    if let Some(option) = &request.git {
        let branch = git
            .configured_default_branch(directory)?
            .unwrap_or_else(|| FALLBACK_BRANCH.to_owned());
        git.init(directory, &branch)?;
        if let Some(address) = option.remote.as_deref().filter(|a| !a.trim().is_empty()) {
            git.add_remote(directory, REMOTE_NAME, address.trim())?;
        }
    }

    WorkspaceSettings {
        default_format: Some(request.default_format),
    }
    .write_new(directory)?;
    let home_file = NoteId::from_title(HOME_TITLE).file_name(request.default_format);
    write_atomic(&directory.join(&home_file), home_note.as_bytes()).map_err(|source| {
        CreateProblem::Io {
            action: "write the Home note",
            source,
        }
    })?;

    if request.git.is_some() {
        git.commit_files(
            directory,
            &[SETTINGS_FILE_NAME, &home_file],
            FIRST_COMMIT_MESSAGE,
        )?;
    }
    Ok(Workspace::open(directory)?)
}

/// What UC-001 created on disk, so it can be removed again (A5).
enum Created {
    /// The directory did not exist; this is the topmost directory created.
    NewDirectory(PathBuf),
    /// The directory existed and was empty.
    InEmptyDirectory(PathBuf),
}

impl Created {
    fn make_directory(directory: &Path) -> Result<Self, CreateProblem> {
        if directory.is_dir() {
            return Ok(Self::InEmptyDirectory(directory.to_owned()));
        }
        let top = directory
            .ancestors()
            .take_while(|ancestor| !ancestor.exists())
            .last()
            .unwrap_or(directory)
            .to_owned();
        fs::create_dir_all(directory).map_err(|source| CreateProblem::Io {
            action: "create the directory",
            source,
        })?;
        Ok(Self::NewDirectory(top))
    }

    /// Removes what was created and returns what could not be removed (A6).
    fn remove(self) -> Vec<PathBuf> {
        match self {
            Self::NewDirectory(top) => match fs::remove_dir_all(&top) {
                Ok(()) => Vec::new(),
                Err(_) if !top.exists() => Vec::new(),
                Err(_) => vec![top],
            },
            Self::InEmptyDirectory(directory) => {
                let Ok(entries) = fs::read_dir(&directory) else {
                    return vec![directory];
                };
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        let removed = if path.is_dir() {
                            fs::remove_dir_all(path)
                        } else {
                            fs::remove_file(path)
                        };
                        removed.is_err()
                    })
                    .collect()
            }
        }
    }
}

/// Test cases TC-001-01 to TC-001-16 of UC-001, as far as they concern the
/// domain; the window part is checked by hand.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::isolated_git;
    use crate::note::Note;

    const MD_HOME: &str = "# Home\n";
    const ADOC_HOME: &str = "= Home\n";
    const IDENTITY: &str = "[user]\n\tname = Test\n\temail = test@example.com\n";

    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("underline-uc001-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn git(&self, config: &str) -> Git {
            isolated_git(&self.path("git-home"), config)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn request(directory: PathBuf, default_format: NoteFormat) -> NewWorkspace {
        NewWorkspace {
            directory,
            default_format,
            git: None,
        }
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn git_out(git: &Git, dir: &Path, args: &[&str]) -> String {
        let mut command = std::process::Command::new("git");
        command.args(args).current_dir(dir);
        for (key, value) in &git.env {
            command.env(key, value);
        }
        String::from_utf8(command.output().unwrap().stdout).unwrap()
    }

    /// TC-001-01: a Markdown workspace in a new directory holds exactly
    /// `Home.md` and `underline.yaml` naming Markdown.
    #[test]
    fn uc001_tc01_creates_markdown_workspace_in_new_directory() {
        let sandbox = Sandbox::new("tc01");
        let dir = sandbox.path("notes-a");
        let workspace = create_workspace(
            &request(dir.clone(), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap();

        assert_eq!(entries(&dir), ["Home.md", "underline.yaml"]);
        assert_eq!(fs::read_to_string(dir.join("Home.md")).unwrap(), MD_HOME);
        assert_eq!(workspace.default_format(), NoteFormat::Markdown);
        assert_eq!(workspace.list().unwrap(), [NoteId::from_title("Home")]);
    }

    /// TC-001-02: an AsciiDoc workspace holds `Home.adoc` and records AsciiDoc.
    #[test]
    fn uc001_tc02_creates_asciidoc_workspace() {
        let sandbox = Sandbox::new("tc02");
        let dir = sandbox.path("notes-b");
        let workspace = create_workspace(
            &request(dir.clone(), NoteFormat::AsciiDoc),
            ADOC_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap();

        assert_eq!(entries(&dir), ["Home.adoc", "underline.yaml"]);
        assert_eq!(
            WorkspaceSettings::load(&dir).unwrap().default_format,
            Some(NoteFormat::AsciiDoc)
        );
        assert_eq!(workspace.default_format(), NoteFormat::AsciiDoc);
    }

    /// TC-001-03: an existing empty directory takes a workspace.
    #[test]
    fn uc001_tc03_creates_workspace_in_existing_empty_directory() {
        let sandbox = Sandbox::new("tc03");
        let dir = sandbox.path("notes-d");
        fs::create_dir(&dir).unwrap();
        create_workspace(
            &request(dir.clone(), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap();
        assert_eq!(entries(&dir), ["Home.md", "underline.yaml"]);
    }

    /// TC-001-05: a directory holding a file or only a folder is refused and
    /// left unchanged (A1).
    #[test]
    fn uc001_tc05_refuses_directory_with_files_or_folders() {
        let sandbox = Sandbox::new("tc05");
        let full = sandbox.path("full");
        fs::create_dir(&full).unwrap();
        fs::write(full.join("readme.txt"), "x").unwrap();
        let only_folder = sandbox.path("only-folder");
        fs::create_dir_all(only_folder.join("sub")).unwrap();

        for dir in [&full, &only_folder] {
            let err = create_workspace(
                &request(dir.clone(), NoteFormat::Markdown),
                MD_HOME,
                &Surroundings::default(),
                &Git::new(),
            )
            .unwrap_err();
            assert!(matches!(
                err.problem,
                CreateProblem::Directory(DirectoryProblem::NotEmpty(_))
            ));
        }
        assert_eq!(entries(&full), ["readme.txt"]);
        assert_eq!(entries(&only_folder), ["sub"]);
    }

    /// TC-001-06: hidden content makes a directory non-empty (BR-002).
    #[test]
    fn uc001_tc06_hidden_content_counts() {
        let sandbox = Sandbox::new("tc06");
        let hidden = sandbox.path("hidden-file");
        fs::create_dir(&hidden).unwrap();
        fs::write(hidden.join(".DS_Store"), "").unwrap();
        let bare_git = sandbox.path("bare-git");
        fs::create_dir_all(bare_git.join(".git")).unwrap();

        for dir in [&hidden, &bare_git] {
            assert!(matches!(
                check_directory(dir, &Surroundings::default()),
                Err(DirectoryProblem::NotEmpty(_))
            ));
        }
    }

    /// TC-001-08: a git workspace without remote has one commit "Create
    /// workspace" with both files, on the configured or the `main` branch.
    #[test]
    fn uc001_tc08_creates_git_repository_with_first_commit() {
        let sandbox = Sandbox::new("tc08");
        for (name, config, branch) in [
            ("notes-g", "[init]\n\tdefaultBranch = trunk\n", "trunk"),
            ("notes-h", "", "main"),
        ] {
            let git = sandbox.git(&format!("{IDENTITY}{config}"));
            let dir = sandbox.path(name);
            let mut new = request(dir.clone(), NoteFormat::Markdown);
            new.git = Some(GitRepositoryOption::default());
            create_workspace(&new, MD_HOME, &Surroundings::default(), &git).unwrap();

            assert_eq!(git.current_branch(&dir).unwrap(), branch);
            assert_eq!(
                git_out(&git, &dir, &["log", "--format=%s"]),
                "Create workspace\n"
            );
            assert_eq!(
                git_out(&git, &dir, &["show", "--name-only", "--format="]),
                "Home.md\nunderline.yaml\n"
            );
            assert_eq!(git_out(&git, &dir, &["status", "--porcelain"]), "");
            assert_eq!(git_out(&git, &dir, &["remote"]), "");
        }
    }

    /// TC-001-09: the git remote is recorded as origin, as given, without
    /// contacting it (BR-004, NFR-014).
    #[test]
    fn uc001_tc09_records_remote_without_contacting_it() {
        let sandbox = Sandbox::new("tc09");
        let git = sandbox.git(IDENTITY);
        let dir = sandbox.path("notes-i");
        let address = "https://underline.invalid/team/notes.git";
        let mut new = request(dir.clone(), NoteFormat::Markdown);
        new.git = Some(GitRepositoryOption {
            remote: Some(address.to_owned()),
        });
        create_workspace(&new, MD_HOME, &Surroundings::default(), &git).unwrap();

        assert_eq!(git_out(&git, &dir, &["remote"]), "origin\n");
        assert_eq!(
            git_out(&git, &dir, &["remote", "get-url", "origin"]),
            format!("{address}\n")
        );
        assert_eq!(git_out(&git, &dir, &["rev-list", "--count", "HEAD"]), "1\n");
    }

    /// TC-001-11: a directory that cannot be created is reported and nothing
    /// is left behind (A5).
    #[test]
    fn uc001_tc11_reports_directory_that_cannot_be_created() {
        let sandbox = Sandbox::new("tc11");
        let blocker = sandbox.path("blocker");
        fs::write(&blocker, "a file where a directory should be").unwrap();
        let err = create_workspace(
            &request(blocker.join("notes-k"), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap_err();
        assert!(matches!(
            err.problem,
            CreateProblem::Io { .. }
                | CreateProblem::Directory(DirectoryProblem::Unreadable { .. })
        ));
        assert!(err.leftovers.is_empty());
        assert_eq!(
            fs::read_to_string(&blocker).unwrap(),
            "a file where a directory should be"
        );
    }

    /// TC-001-11 / A5: a failure after the directory was made removes the
    /// directory and every parent created for it.
    #[test]
    fn uc001_tc11_removes_created_directories_after_failure() {
        let sandbox = Sandbox::new("tc11b");
        let dir = sandbox.path("parent/notes-k");
        let mut new = request(dir, NoteFormat::Markdown);
        new.git = Some(GitRepositoryOption::default());
        let err = create_workspace(
            &new,
            MD_HOME,
            &Surroundings::default(),
            &Git::with_program("underline-no-such-git"),
        )
        .unwrap_err();
        assert!(matches!(
            err.problem,
            CreateProblem::Git(GitError::NotInstalled)
        ));
        assert!(err.leftovers.is_empty());
        assert!(!sandbox.path("parent").exists());
    }

    /// TC-001-13: a directory inside an existing workspace is refused (A7,
    /// BR-006), but the user settings file marks no workspace.
    #[test]
    fn uc001_tc13_refuses_directory_inside_workspace() {
        let sandbox = Sandbox::new("tc13");
        let outer = sandbox.path("outer");
        create_workspace(
            &request(outer.clone(), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap();

        let err = check_directory(&outer.join("inner"), &Surroundings::default()).unwrap_err();
        assert!(matches!(err, DirectoryProblem::InsideWorkspace { .. }));

        let known = Surroundings {
            known_workspaces: vec![sandbox.path("plain")],
            user_settings_dir: None,
        };
        assert!(matches!(
            check_directory(&sandbox.path("plain/../plain/inner"), &known),
            Err(DirectoryProblem::InsideWorkspace { .. })
        ));

        let home = Surroundings {
            known_workspaces: Vec::new(),
            user_settings_dir: Some(outer.clone()),
        };
        assert!(check_directory(&outer.join("inner"), &home).is_ok());
        assert!(!outer.join("inner").exists());
    }

    /// TC-001-14: the directory is checked again right before it is filled.
    #[test]
    fn uc001_tc14_checks_directory_again_before_creating() {
        let sandbox = Sandbox::new("tc14");
        let dir = sandbox.path("notes-n");
        fs::create_dir(&dir).unwrap();
        assert!(check_directory(&dir, &Surroundings::default()).is_ok());
        fs::write(dir.join("draft.txt"), "draft").unwrap();

        let err = create_workspace(
            &request(dir.clone(), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap_err();
        assert!(matches!(
            err.problem,
            CreateProblem::Directory(DirectoryProblem::NotEmpty(_))
        ));
        assert_eq!(entries(&dir), ["draft.txt"]);
    }

    /// TC-001-15: new notes take the default format; the Home note keeps its
    /// own (BR-005).
    #[test]
    fn uc001_tc15_new_notes_take_the_default_format() {
        let sandbox = Sandbox::new("tc15");
        let dir = sandbox.path("notes-o");
        create_workspace(
            &request(dir.clone(), NoteFormat::Markdown),
            MD_HOME,
            &Surroundings::default(),
            &Git::new(),
        )
        .unwrap();
        fs::write(dir.join(SETTINGS_FILE_NAME), "default_format: asciidoc\n").unwrap();

        let workspace = Workspace::open(&dir).unwrap();
        workspace.write(&Note::new("Plan", "= Plan\n")).unwrap();
        assert!(dir.join("Plan.adoc").is_file());
        assert_eq!(fs::read_to_string(dir.join("Home.md")).unwrap(), MD_HOME);
    }

    /// TC-001-16: without a git identity, or without git, nothing is left and
    /// the problem is named (A5).
    #[test]
    fn uc001_tc16_reports_missing_identity_and_missing_git() {
        let sandbox = Sandbox::new("tc16");
        let no_identity = sandbox.git("[user]\n\tuseConfigOnly = true\n");
        let mut new = request(sandbox.path("notes-p"), NoteFormat::Markdown);
        new.git = Some(GitRepositoryOption::default());

        let err =
            create_workspace(&new, MD_HOME, &Surroundings::default(), &no_identity).unwrap_err();
        assert!(matches!(
            err.problem,
            CreateProblem::Git(GitError::NoIdentity)
        ));
        assert!(err.to_string().contains("user.name"));
        assert!(!sandbox.path("notes-p").exists());

        let err = create_workspace(
            &new,
            MD_HOME,
            &Surroundings::default(),
            &Git::with_program("underline-no-such-git"),
        )
        .unwrap_err();
        assert!(matches!(
            err.problem,
            CreateProblem::Git(GitError::NotInstalled)
        ));
        assert!(!sandbox.path("notes-p").exists());
    }

    /// A5 in an existing empty directory: the directory stays, empty.
    #[test]
    fn uc001_a5_empties_an_existing_directory_again() {
        let sandbox = Sandbox::new("a5");
        let dir = sandbox.path("notes-q");
        fs::create_dir(&dir).unwrap();
        let mut new = request(dir.clone(), NoteFormat::Markdown);
        new.git = Some(GitRepositoryOption::default());
        let no_identity = sandbox.git("[user]\n\tuseConfigOnly = true\n");

        let err =
            create_workspace(&new, MD_HOME, &Surroundings::default(), &no_identity).unwrap_err();
        assert!(err.leftovers.is_empty());
        assert!(dir.is_dir());
        assert!(entries(&dir).is_empty());
    }
}
