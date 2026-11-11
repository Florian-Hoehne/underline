//! How the session's git jobs run git (ADR-006): the installed program
//! with the environment of the session's `Git` (tests point it at a
//! configuration of their own), every pathspec taken literally
//! (`GIT_LITERAL_PATHSPECS`), so a note named `[a].md` or `:(top)x.md` never
//! stands for other files (review F12).
//!
//! `underline_core::Git` is shared with the egui app and stays as it is;
//! what the jobs need beyond it (the committed content as bytes, telling a
//! file that was not committed from a failure of git, unstaging, writing
//! the name and email together) is here.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

use underline_core::{Git, GitError};

/// The environment every git job runs with.
#[derive(Debug, Clone, Default)]
pub(in crate::session) struct GitSetup {
    env: Vec<(OsString, OsString)>,
}

/// What a git command gave: whether it succeeded, its output and its
/// error text.
struct Ran {
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl GitSetup {
    /// A setup whose git commands get `env` too.
    #[cfg(test)]
    pub(in crate::session) fn with_env(env: Vec<(OsString, OsString)>) -> Self {
        Self { env }
    }

    /// The `Git` of `underline_core` with this environment and literal
    /// pathspecs, for the change tracker and committing.
    pub(in crate::session) fn git(&self) -> Git {
        self.env
            .iter()
            .fold(Git::new(), |git, (key, value)| git.env(key, value))
            .env("GIT_LITERAL_PATHSPECS", "1")
    }

    fn raw(&self, dir: &Path, args: &[&str]) -> Result<Ran, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(|err| match err.kind() {
                io::ErrorKind::NotFound => GitError::NotInstalled.to_string(),
                _ => GitError::Io(err).to_string(),
            })?;
        Ok(Ran {
            success: output.status.success(),
            code: output.status.code(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }

    /// Runs git; its output, or the failure as `underline_core` words it.
    pub(in crate::session) fn run(&self, dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
        let ran = self.raw(dir, args)?;
        if ran.success {
            Ok(ran.stdout)
        } else {
            Err(GitError::Failed {
                command: args.first().copied().unwrap_or_default().to_owned(),
                message: ran.stderr,
            }
            .to_string())
        }
    }

    /// Whether git's settings for the workspace at `dir` hold `key` (S7,
    /// review F13: the user's own ssh command).
    pub(in crate::session) fn has_config(&self, dir: &Path, key: &str) -> bool {
        self.raw(dir, &["config", "--get", key])
            .is_ok_and(|ran| ran.success)
    }

    /// The last commit, or `None` when there is none yet (UC-011 A7).
    fn head(&self, dir: &Path) -> Result<Option<String>, String> {
        // Not `HEAD^{commit}`: that says nothing when the commit's object is
        // missing, which must be a failure, not "no commit yet".
        let ran = self.raw(dir, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
        if ran.success {
            return Ok(Some(String::from_utf8_lossy(&ran.stdout).trim().to_owned()));
        }
        // `--verify --quiet` exits with 1 and says nothing when HEAD names
        // no commit yet; anything else is a failure of git.
        if ran.code == Some(1) && ran.stderr.is_empty() {
            return Ok(None);
        }
        Err(GitError::Failed {
            command: "rev-parse".into(),
            message: ran.stderr,
        }
        .to_string())
    }

    /// The commit `HEAD` names now, for the cache of the diff.
    pub(in crate::session) fn head_commit(&self, dir: &Path) -> Result<Option<String>, String> {
        self.head(dir)
    }

    /// The content `path` (relative to the workspace at `dir`) has in the
    /// commit `head`, as bytes; `None` when it is not in it. A failure of
    /// git is an error, never "not committed" (review F12).
    pub(in crate::session) fn committed_in(
        &self,
        dir: &Path,
        head: Option<&str>,
        path: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let Some(head) = head else {
            return Ok(None);
        };
        let listed = self.run(dir, &["ls-tree", "-z", head, "--", path])?;
        let listed = String::from_utf8_lossy(&listed);
        let Some(entry) = listed.split('\0').find(|entry| !entry.is_empty()) else {
            return Ok(None);
        };
        // `<mode> <type> <object>\t<path>`
        let fields: Vec<&str> = entry
            .split('\t')
            .next()
            .unwrap_or_default()
            .split(' ')
            .collect();
        match fields.as_slice() {
            [_, "blob", object] => self.run(dir, &["cat-file", "blob", object]).map(Some),
            _ => Ok(None),
        }
    }

    /// [`GitSetup::committed_in`] the last commit.
    pub(in crate::session) fn committed(
        &self,
        dir: &Path,
        path: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        let head = self.head(dir)?;
        self.committed_in(dir, head.as_deref(), path)
    }

    /// UC-011 A6: the index of `paths` is set back to the last commit, so
    /// nothing staged of them is left (a failed commit stages its changes).
    pub(in crate::session) fn unstage(&self, dir: &Path, paths: &[&str]) -> Result<(), String> {
        let mut args = if self.head(dir)?.is_some() {
            vec!["reset", "--quiet", "HEAD", "--"]
        } else {
            vec!["rm", "--cached", "--quiet", "--ignore-unmatch", "--"]
        };
        args.extend_from_slice(paths);
        self.run(dir, &args).map(drop)
    }

    /// UC-012 A3, BR-005: keeps `name` and `email` in the user's own git
    /// settings; when the email cannot be kept, the name is put back as it
    /// was, so the settings are never half changed (review F12).
    pub(in crate::session) fn set_identity(
        &self,
        dir: &Path,
        name: &str,
        email: &str,
    ) -> Result<(), String> {
        let before = self.raw(dir, &["config", "--global", "--get", "user.name"])?;
        self.run(dir, &["config", "--global", "user.name", name])?;
        if let Err(err) = self.run(dir, &["config", "--global", "user.email", email]) {
            let back = if before.success {
                let old = String::from_utf8_lossy(&before.stdout)
                    .trim_end()
                    .to_owned();
                self.run(dir, &["config", "--global", "user.name", &old])
            } else {
                self.run(dir, &["config", "--global", "--unset", "user.name"])
            };
            if let Err(back) = back {
                eprintln!("UC-012 A3: the name could not be put back: {back}");
            }
            return Err(err);
        }
        Ok(())
    }
}
