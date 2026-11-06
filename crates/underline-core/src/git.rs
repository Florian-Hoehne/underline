//! git operations through the git program installed on the machine (ADR-006).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is not installed or not on the PATH; install git and try again")]
    NotInstalled,
    #[error(
        "git has no user name and email to make a commit with; set them with \
         `git config --global user.name \"Your Name\"` and \
         `git config --global user.email you@example.com`"
    )]
    NoIdentity,
    #[error("`git {command}` failed: {message}")]
    Failed { command: String, message: String },
    #[error("cannot run git: {0}")]
    Io(io::Error),
}

/// Runs the installed `git` program, so the user's own git configuration
/// applies: credential helper, SSH agent, default branch, and identity.
#[derive(Debug, Clone)]
pub struct Git {
    program: PathBuf,
    pub(crate) env: Vec<(OsString, OsString)>,
}

impl Default for Git {
    fn default() -> Self {
        Self::new()
    }
}

impl Git {
    /// The `git` found on the `PATH`.
    pub fn new() -> Self {
        Self::with_program("git")
    }

    /// A specific git program, for example in tests.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            env: Vec::new(),
        }
    }

    /// Sets an environment variable for every git command, for example to
    /// point tests at their own git configuration.
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// The default branch the user's git setup names (`init.defaultBranch`),
    /// if it names one.
    pub fn configured_default_branch(&self, dir: &Path) -> Result<Option<String>, GitError> {
        match self.run(dir, &["config", "--get", "init.defaultBranch"]) {
            Ok(out) => Ok(Some(out.trim().to_owned()).filter(|b| !b.is_empty())),
            // `git config --get` exits with 1 when the key is not set.
            Err(GitError::Failed { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Makes `dir` a git repository whose first branch is `branch`.
    pub fn init(&self, dir: &Path, branch: &str) -> Result<(), GitError> {
        self.run(dir, &["init", "--quiet", "--initial-branch", branch])
            .map(drop)
    }

    /// Links the git remote `name` with `address`, recorded as given.
    pub fn add_remote(&self, dir: &Path, name: &str, address: &str) -> Result<(), GitError> {
        self.run(dir, &["remote", "add", name, address]).map(drop)
    }

    /// Commits exactly `files` (paths relative to `dir`) with `message`.
    pub fn commit_files(&self, dir: &Path, files: &[&str], message: &str) -> Result<(), GitError> {
        let mut add = vec!["add", "--"];
        add.extend_from_slice(files);
        self.run(dir, &add)?;
        self.run(dir, &["commit", "--quiet", "--message", message])
            .map(drop)
    }

    /// The branch the repository at `dir` is on.
    pub fn current_branch(&self, dir: &Path) -> Result<String, GitError> {
        self.run(dir, &["symbolic-ref", "--short", "HEAD"])
            .map(|out| out.trim().to_owned())
    }

    /// Paths, relative to `dir`, that the git repository around `dir` is told
    /// to ignore; a folder stands for everything in it. Empty when `dir` is
    /// not in a git repository (UC-002 BR-003).
    pub fn ignored_paths(&self, dir: &Path) -> Result<Vec<PathBuf>, GitError> {
        let args = [
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ];
        match self.run(dir, &args) {
            Ok(out) => Ok(out
                .split('\0')
                .map(|entry| entry.trim_end_matches('/'))
                .filter(|entry| !entry.is_empty())
                .map(PathBuf::from)
                .collect()),
            Err(GitError::Failed { .. }) => Ok(Vec::new()),
            Err(err) => Err(err),
        }
    }

    /// The git remotes of the repository at `dir`, or `None` when `dir` is
    /// not in a git repository or git is not installed (UC-002 step 9).
    pub fn remotes(&self, dir: &Path) -> Option<Vec<String>> {
        self.run(dir, &["remote"])
            .ok()
            .map(|out| out.lines().map(str::to_owned).collect())
    }

    fn run(&self, dir: &Path, args: &[&str]) -> Result<String, GitError> {
        let output = Command::new(&self.program)
            .args(args)
            .current_dir(dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(|err| match err.kind() {
                io::ErrorKind::NotFound => GitError::NotInstalled,
                _ => GitError::Io(err),
            })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if is_identity_problem(&message) {
            return Err(GitError::NoIdentity);
        }
        Err(GitError::Failed {
            command: args.first().copied().unwrap_or_default().to_owned(),
            message,
        })
    }
}

fn is_identity_problem(stderr: &str) -> bool {
    [
        "Author identity unknown",
        "Please tell me who you are",
        "empty ident name",
        "no email was given",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;

    /// A git that reads only its own configuration in `dir`, so tests do not
    /// depend on the machine's git setup.
    pub(crate) fn isolated_git(dir: &Path, config: &str) -> Git {
        fs::create_dir_all(dir).unwrap();
        let config_file = dir.join("gitconfig");
        fs::write(&config_file, config).unwrap();
        Git::new()
            .env("GIT_CONFIG_GLOBAL", &config_file)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", dir)
    }

    #[test]
    fn missing_program_is_not_installed() {
        let git = Git::with_program("underline-no-such-git");
        let err = git.init(&std::env::temp_dir(), "main").unwrap_err();
        assert!(matches!(err, GitError::NotInstalled));
    }

    #[test]
    fn reads_the_configured_default_branch() {
        let dir = std::env::temp_dir().join(format!("underline-git-branch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let git = isolated_git(&dir, "[init]\n\tdefaultBranch = trunk\n");
        assert_eq!(
            git.configured_default_branch(&dir).unwrap().as_deref(),
            Some("trunk")
        );
        let git = isolated_git(&dir, "");
        assert_eq!(git.configured_default_branch(&dir).unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
