//! UC-013 Manage Branches: the branches of a workspace, creating one, and
//! switching, through the installed git (ADR-006).

use std::path::Path;

use crate::git::{Git, GitError};

/// The branches of a workspace (UC-013 BR-001).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Branches {
    /// The local branches by name.
    pub local: Vec<String>,
    /// The branch the workspace is on, if it is on one.
    pub current: Option<String>,
    /// Branches of the remote without a local branch of the same name, as
    /// `remote/name`, as of the last synchronization.
    pub remote: Vec<String>,
}

/// Why a branch name cannot be used (UC-013 BR-005, A2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BranchNameProblem {
    #[error("a branch needs a name")]
    Empty,
    #[error("a branch name cannot hold `{0}`")]
    Character(char),
    #[error("a branch name cannot hold `{0}`")]
    Sequence(&'static str),
    #[error("a branch name cannot start with `{0}`")]
    Start(char),
    #[error("a branch name cannot end with `{0}`")]
    End(&'static str),
    #[error("`@` alone is not a branch name")]
    At,
    #[error("the branch `{0}` exists")]
    Exists(String),
}

/// UC-013 BR-005: the name typed, with spaces turned into `-`, or why git
/// would not take it.
pub fn branch_name(typed: &str) -> Result<String, BranchNameProblem> {
    let name = typed.split_whitespace().collect::<Vec<_>>().join("-");
    if name.is_empty() {
        return Err(BranchNameProblem::Empty);
    }
    if name == "@" {
        return Err(BranchNameProblem::At);
    }
    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || "~^:?*[\\".contains(*c))
    {
        return Err(BranchNameProblem::Character(c));
    }
    for sequence in ["..", "//", "@{"] {
        if name.contains(sequence) {
            return Err(BranchNameProblem::Sequence(sequence));
        }
    }
    for start in ['-', '.', '/'] {
        if name.starts_with(start) {
            return Err(BranchNameProblem::Start(start));
        }
    }
    for end in ["/", ".lock", "."] {
        if name.ends_with(end) {
            return Err(BranchNameProblem::End(end));
        }
    }
    if name.split('/').any(|part| part.starts_with('.')) {
        return Err(BranchNameProblem::Start('.'));
    }
    Ok(name)
}

/// Why a switch did not happen (UC-013 A3, A5).
#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    /// A3: changes not committed would be overwritten; their places.
    #[error("changes to {} would be overwritten", .0.join(", "))]
    WouldOverwrite(Vec<String>),
    #[error(transparent)]
    Git(#[from] GitError),
}

impl Git {
    /// UC-013 step 2: the local branches, the current one, and the remote's
    /// branches without a local one.
    pub fn branches(&self, dir: &Path) -> Result<Branches, GitError> {
        let mut local: Vec<String> = self
            .run(
                dir,
                &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
            )?
            .lines()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        local.sort_by_key(|n| n.to_lowercase());
        let remote_refs = self.run(
            dir,
            &["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
        )?;
        let mut remote: Vec<String> = remote_refs
            .lines()
            .filter(|r| !r.is_empty() && !r.ends_with("/HEAD") && r.contains('/'))
            .filter(|r| {
                let name = r.split_once('/').map_or(*r, |(_, n)| n);
                !local.iter().any(|l| l == name)
            })
            .map(str::to_owned)
            .collect();
        remote.sort_by_key(|n| n.to_lowercase());
        remote.dedup_by(|a, b| a.split_once('/').map(|x| x.1) == b.split_once('/').map(|x| x.1));
        let current = self
            .run(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|b| b.trim().to_owned());
        Ok(Branches {
            local,
            current,
            remote,
        })
    }

    /// UC-013 step 4: switches to the local branch `name`, carrying along
    /// the changes that are not committed (BR-003).
    pub fn switch_branch(&self, dir: &Path, name: &str) -> Result<(), SwitchError> {
        self.switching(dir, &["switch", "--quiet", name])
    }

    /// UC-013 A1: creates the branch `name` at the current commit and
    /// switches to it; the changes stay as they are.
    pub fn create_branch(&self, dir: &Path, name: &str) -> Result<(), SwitchError> {
        self.switching(dir, &["switch", "--quiet", "--create", name])
    }

    /// UC-013 A4: a local branch following the remote branch `remote`
    /// (`origin/review`), switched to.
    pub fn track_branch(&self, dir: &Path, remote: &str) -> Result<(), SwitchError> {
        self.switching(dir, &["switch", "--quiet", "--track", remote])
    }

    fn switching(&self, dir: &Path, args: &[&str]) -> Result<(), SwitchError> {
        match self.run(dir, args) {
            Ok(_) => Ok(()),
            Err(GitError::Failed { message, .. }) if message.contains("would be overwritten") => {
                let files = message
                    .lines()
                    .filter(|l| l.starts_with('\t'))
                    .map(|l| l.trim().to_owned())
                    .collect();
                Err(SwitchError::WouldOverwrite(files))
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::isolated_git;
    use std::fs;

    struct Repo {
        dir: std::path::PathBuf,
        git: Git,
    }

    impl Repo {
        fn new(name: &str) -> Self {
            let base =
                std::env::temp_dir().join(format!("underline-uc013-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&base);
            let git = isolated_git(
                &base.join("home"),
                "[user]\n\tname = T\n\temail = t@example.com\n",
            );
            let dir = base.join("ws");
            fs::create_dir_all(&dir).unwrap();
            git.init(&dir, "main").unwrap();
            Self { dir, git }
        }

        fn write(&self, path: &str, text: &str) {
            fs::write(self.dir.join(path), text).unwrap();
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.dir.join(path)).unwrap()
        }

        fn commit(&self) {
            self.git.run(&self.dir, &["add", "-A"]).unwrap();
            self.git
                .run(&self.dir, &["commit", "--quiet", "-m", "c"])
                .unwrap();
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.dir.parent().unwrap());
        }
    }

    /// TC-013-01 / TC-013-03 / TC-013-05 / TC-013-07: create, switch, carry
    /// changes along, and refuse to overwrite them.
    #[test]
    fn uc013_tc01_tc03_tc05_tc07_switch() {
        let repo = Repo::new("switch");
        repo.write("Plan.md", "plan\n");
        repo.write("Ideas.md", "ideas\n");
        repo.commit();
        repo.git.create_branch(&repo.dir, "drafts").unwrap();
        repo.write("Plan.md", "draft plan\n");
        repo.write("Sketch.md", "sketch\n");
        repo.commit();
        repo.git.switch_branch(&repo.dir, "main").unwrap();
        assert_eq!(repo.read("Plan.md"), "plan\n");
        assert!(!repo.dir.join("Sketch.md").exists());
        let branches = repo.git.branches(&repo.dir).unwrap();
        assert_eq!(branches.local, ["drafts", "main"]);
        assert_eq!(branches.current.as_deref(), Some("main"));
        // Carried along.
        repo.write("Ideas.md", "ideas 2\n");
        repo.git.switch_branch(&repo.dir, "drafts").unwrap();
        assert_eq!(repo.read("Ideas.md"), "ideas 2\n");
        assert_eq!(repo.read("Plan.md"), "draft plan\n");
        // Would be overwritten.
        repo.write("Plan.md", "changed on drafts\n");
        match repo.git.switch_branch(&repo.dir, "main") {
            Err(SwitchError::WouldOverwrite(files)) => assert_eq!(files, ["Plan.md"]),
            other => panic!("{other:?}"),
        }
        assert_eq!(repo.read("Plan.md"), "changed on drafts\n");
        // A new branch keeps the changes.
        repo.git.create_branch(&repo.dir, "trip-ideas").unwrap();
        assert_eq!(repo.read("Plan.md"), "changed on drafts\n");
        assert!(matches!(
            repo.git.create_branch(&repo.dir, "drafts"),
            Err(SwitchError::Git(_))
        ));
    }

    /// TC-013-06: names git would not take.
    #[test]
    fn uc013_tc06_branch_names() {
        assert_eq!(branch_name(" trip  ideas ").unwrap(), "trip-ideas");
        assert_eq!(branch_name("feature/notes").unwrap(), "feature/notes");
        for bad in [
            "", "a..b", "-x", "x.lock", "a:b", "@", "a b~", ".x", "x/", "a//b", "a@{b", "a/.b",
        ] {
            assert!(branch_name(bad).is_err(), "{bad}");
        }
    }

    /// TC-013-08: a branch of the remote becomes a local one following it.
    #[test]
    fn uc013_tc08_remote_branch() {
        let origin = Repo::new("origin");
        origin.write("A.md", "a\n");
        origin.commit();
        origin.git.create_branch(&origin.dir, "review").unwrap();
        origin.git.switch_branch(&origin.dir, "main").unwrap();
        let clone = Repo::new("clone");
        fs::remove_dir_all(&clone.dir).unwrap();
        clone
            .git
            .run(
                clone.dir.parent().unwrap(),
                &["clone", "--quiet", origin.dir.to_str().unwrap(), "ws"],
            )
            .unwrap();
        let branches = clone.git.branches(&clone.dir).unwrap();
        assert_eq!(branches.local, ["main"]);
        assert_eq!(branches.remote, ["origin/review"]);
        clone.git.track_branch(&clone.dir, "origin/review").unwrap();
        let status = clone.git.run(&clone.dir, &["status", "-sb"]).unwrap();
        assert!(status.starts_with("## review...origin/review"), "{status}");
        assert!(clone.git.branches(&clone.dir).unwrap().remote.is_empty());
    }
}
