//! UC-015 Resolve Sync Conflict: the conflicting places git leaves in a
//! note, choosing a side for each, and completing or aborting the joining
//! of a sync, through the installed git (ADR-006).

use std::fs;
use std::ops::Range;
use std::path::Path;

use crate::git::{Git, GitError};

/// A side of a conflict (UC-015 BR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// This computer's version.
    Here,
    /// The version that came with the sync.
    Remote,
}

/// What the user chooses for a place (UC-015 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Here,
    Remote,
    /// Here first, then remote.
    Both,
}

/// How a sync is joining the remote's changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joining {
    Merge,
    /// A6: local commits put on top of the remote's, one by one.
    Rebase,
}

/// A conflicting place of a note (UC-015 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictPlace {
    /// The bytes of the whole place, markers included.
    pub range: Range<usize>,
    pub here: String,
    pub remote: String,
}

/// UC-015 step 3: the conflicting places of `text`, as git marks them;
/// `joining` says which marked side is this computer's.
pub fn conflict_places(text: &str, joining: Joining) -> Vec<ConflictPlace> {
    let mut places = Vec::new();
    let mut at = 0;
    let mut open: Option<(usize, String, Option<String>, bool)> = None;
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        match &mut open {
            None if content.starts_with("<<<<<<<") => {
                open = Some((start, String::new(), None, false));
            }
            None => {}
            Some((_, _, None, in_base)) if content.starts_with("|||||||") => {
                *in_base = true;
            }
            Some((_, _, second, in_base)) if content.starts_with("=======") && second.is_none() => {
                *second = Some(String::new());
                *in_base = false;
            }
            Some((begin, first, Some(second), _)) if content.starts_with(">>>>>>>") => {
                let (ours, theirs) = (std::mem::take(first), std::mem::take(second));
                let (here, remote) = match joining {
                    Joining::Merge => (ours, theirs),
                    Joining::Rebase => (theirs, ours),
                };
                places.push(ConflictPlace {
                    range: *begin..at,
                    here,
                    remote,
                });
                open = None;
            }
            Some((_, _, _, true)) => {}
            Some((_, first, None, false)) => first.push_str(line),
            Some((_, _, Some(second), _)) => second.push_str(line),
        }
    }
    places
}

/// UC-015 step 5: `text` with the place `place` replaced by `choice`.
pub fn resolve_place(text: &str, place: &ConflictPlace, choice: Choice) -> String {
    let chosen = match choice {
        Choice::Here => place.here.clone(),
        Choice::Remote => place.remote.clone(),
        Choice::Both => {
            let mut both = place.here.clone();
            if !both.is_empty() && !both.ends_with('\n') {
                both.push('\n');
            }
            both.push_str(&place.remote);
            both
        }
    };
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..place.range.start]);
    out.push_str(&chosen);
    out.push_str(&text[place.range.end..]);
    out
}

/// Why the conflicts cannot be completed (UC-015 A6, A7).
#[derive(Debug, thiserror::Error)]
pub enum CompleteError {
    /// A6: the next local commit conflicts too; its places.
    #[error("the next commit conflicts in {}", .0.join(", "))]
    MoreConflicts(Vec<String>),
    #[error(transparent)]
    Git(#[from] GitError),
}

impl Git {
    /// How the stopped sync is joining, if one is.
    pub fn joining(&self, dir: &Path) -> Option<Joining> {
        let exists = |name: &str| {
            self.run(dir, &["rev-parse", "--git-path", name])
                .ok()
                .is_some_and(|p| dir.join(p.trim()).exists())
        };
        if exists("rebase-merge") || exists("rebase-apply") {
            Some(Joining::Rebase)
        } else if self
            .run(dir, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
            .is_ok()
        {
            Some(Joining::Merge)
        } else {
            None
        }
    }

    /// UC-015 step 6, A3: records the note or file at `path` as resolved,
    /// as it is now (also when it is gone).
    pub fn mark_resolved(&self, dir: &Path, path: &str) -> Result<(), GitError> {
        self.run(dir, &["add", "--all", "--", path]).map(drop)
    }

    /// A2: keeps `side` of the file at `path` and the other version next to
    /// it as a copy; returns the copy's place.
    pub fn take_side(
        &self,
        dir: &Path,
        path: &str,
        side: Side,
        joining: Joining,
    ) -> Result<String, GitError> {
        // Git's stages: 2 is "ours", 3 is "theirs".
        let (here, remote) = match joining {
            Joining::Merge => (2, 3),
            Joining::Rebase => (3, 2),
        };
        let (keep, other, label) = match side {
            Side::Here => (here, remote, "remote"),
            Side::Remote => (remote, here, "here"),
        };
        let bytes = |stage: i32| self.run_bytes(dir, &["show", &format!(":{stage}:./{path}")]);
        let kept = bytes(keep)?;
        let copy = copy_name(path, label);
        if let Ok(other) = bytes(other) {
            fs::write(dir.join(&copy), other).map_err(GitError::Io)?;
        }
        fs::write(dir.join(path), kept).map_err(GitError::Io)?;
        self.mark_resolved(dir, path)?;
        Ok(copy)
    }

    /// UC-015 step 7, BR-004: completes the joining once nothing is in
    /// conflict.
    pub fn complete_joining(&self, dir: &Path) -> Result<(), CompleteError> {
        match self.joining(dir) {
            Some(Joining::Merge) => {
                let branch = self
                    .run(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"])
                    .map(|b| b.trim().to_owned())
                    .unwrap_or_else(|_| "the branch".into());
                let message = format!("Merge remote changes into {branch}");
                self.run(
                    dir,
                    &["commit", "--quiet", "--no-edit", "--message", &message],
                )?;
                Ok(())
            }
            Some(Joining::Rebase) => {
                let git = self.clone().env("GIT_EDITOR", "true");
                match git.run(dir, &["rebase", "--continue"]) {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        let open: Vec<String> = self
                            .run(dir, &["diff", "--name-only", "--diff-filter=U"])
                            .map(|o| o.lines().map(str::to_owned).collect())
                            .unwrap_or_default();
                        if open.is_empty() {
                            Err(err.into())
                        } else {
                            Err(CompleteError::MoreConflicts(open))
                        }
                    }
                }
            }
            None => Ok(()),
        }
    }

    /// UC-015 A5, BR-008: puts the workspace back as it was right before the
    /// sync joined; local commits and changes left out stay.
    pub fn abort_joining(&self, dir: &Path) -> Result<(), GitError> {
        match self.joining(dir) {
            Some(Joining::Merge) => self.run(dir, &["merge", "--abort"]).map(drop),
            Some(Joining::Rebase) => self.run(dir, &["rebase", "--abort"]).map(drop),
            None => Ok(()),
        }
    }
}

/// `img/shot.png` → `img/shot (remote).png`.
pub fn copy_name(path: &str, label: &str) -> String {
    let (folder, name) = match path.rsplit_once('/') {
        Some((folder, name)) => (format!("{folder}/"), name),
        None => (String::new(), path),
    };
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            format!("{folder}{stem} ({label}).{extension}")
        }
        _ => format!("{folder}{name} ({label})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::isolated_git;
    use crate::sync::{SyncMode, SyncOutcome};
    use std::collections::HashSet;
    use std::path::PathBuf;

    const MARKED: &str = "one\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> abc\nthree\n<<<<<<< HEAD\nA\n||||||| base\nB\n=======\nC\n>>>>>>> abc\n";

    #[test]
    fn uc015_places_and_choices() {
        let places = conflict_places(MARKED, Joining::Merge);
        assert_eq!(places.len(), 2);
        assert_eq!(
            (places[0].here.as_str(), places[0].remote.as_str()),
            ("mine\n", "theirs\n")
        );
        assert_eq!(
            (places[1].here.as_str(), places[1].remote.as_str()),
            ("A\n", "C\n")
        );
        let rebased = conflict_places(MARKED, Joining::Rebase);
        assert_eq!(rebased[0].here, "theirs\n");
        let text = resolve_place(MARKED, &places[1], Choice::Both);
        assert!(text.ends_with("three\nA\nC\n"), "{text}");
        let text = resolve_place(
            &text,
            &conflict_places(&text, Joining::Merge)[0],
            Choice::Remote,
        );
        assert_eq!(text, "one\ntheirs\nthree\nA\nC\n");
        assert!(conflict_places(&text, Joining::Merge).is_empty());
        assert_eq!(copy_name("img/shot.png", "remote"), "img/shot (remote).png");
        assert_eq!(copy_name("README", "here"), "README (here)");
    }

    struct Setup {
        base: PathBuf,
        git: Git,
    }

    impl Setup {
        fn new(name: &str, config: &str) -> Self {
            let base =
                std::env::temp_dir().join(format!("underline-uc015-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&base);
            let git = isolated_git(
                &base.join("home"),
                &format!("[user]\n\tname = T\n\temail = t@example.com\n[init]\n\tdefaultBranch = main\n{config}"),
            );
            git.run(&base, &["init", "--quiet", "--bare", "remote.git"])
                .unwrap();
            git.run(&base, &["clone", "--quiet", "remote.git", "a"])
                .unwrap();
            fs::write(base.join("a/Plan.md"), "one\ntwo\nthree\n").unwrap();
            fs::write(base.join("a/shot.png"), "png 1").unwrap();
            git.run(&base.join("a"), &["add", "-A"]).unwrap();
            git.run(&base.join("a"), &["commit", "--quiet", "-m", "init"])
                .unwrap();
            git.run(
                &base.join("a"),
                &["push", "--quiet", "-u", "origin", "main"],
            )
            .unwrap();
            git.run(&base, &["clone", "--quiet", "remote.git", "b"])
                .unwrap();
            let s = Self { base, git };
            s.write("b", "Plan.md", "one\nTWO from b\nthree\n");
            s.write("b", "shot.png", "png b");
            s.sync("b");
            s.write("a", "Plan.md", "one\ntwo from a\nthree\n");
            s.write("a", "shot.png", "png a");
            s
        }

        fn dir(&self, clone: &str) -> PathBuf {
            self.base.join(clone)
        }

        fn write(&self, clone: &str, path: &str, text: &str) {
            fs::write(self.dir(clone).join(path), text).unwrap();
        }

        fn read(&self, clone: &str, path: &str) -> String {
            fs::read_to_string(self.dir(clone).join(path)).unwrap()
        }

        fn sync(&self, clone: &str) -> SyncOutcome {
            self.git
                .sync(&self.dir(clone), SyncMode::Full, &HashSet::new())
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    /// TC-015-01 / TC-015-05: resolve a note and a file, complete, sync.
    #[test]
    fn uc015_tc01_tc05_resolve_and_complete() {
        let s = Setup::new("merge", "");
        assert!(matches!(s.sync("a"), SyncOutcome::Conflict(_)));
        let dir = s.dir("a");
        assert_eq!(s.git.joining(&dir), Some(Joining::Merge));
        let text = s.read("a", "Plan.md");
        let places = conflict_places(&text, Joining::Merge);
        assert_eq!(places[0].here, "two from a\n");
        assert_eq!(places[0].remote, "TWO from b\n");
        fs::write(
            dir.join("Plan.md"),
            resolve_place(&text, &places[0], Choice::Here),
        )
        .unwrap();
        s.git.mark_resolved(&dir, "Plan.md").unwrap();
        let copy = s
            .git
            .take_side(&dir, "shot.png", Side::Here, Joining::Merge)
            .unwrap();
        assert_eq!(copy, "shot (remote).png");
        assert_eq!(s.read("a", "shot.png"), "png a");
        assert_eq!(s.read("a", "shot (remote).png"), "png b");
        s.git.complete_joining(&dir).unwrap();
        assert_eq!(s.git.joining(&dir), None);
        assert!(matches!(s.sync("a"), SyncOutcome::Synced { .. }));
        let log = s
            .git
            .run(&dir, &["log", "--format=%s", "origin/main"])
            .unwrap();
        assert!(log.contains("Merge remote changes into main"), "{log}");
    }

    /// TC-015-08: abort puts the workspace back; the local commit stays.
    #[test]
    fn uc015_tc08_abort() {
        let s = Setup::new("abort", "");
        assert!(matches!(s.sync("a"), SyncOutcome::Conflict(_)));
        let dir = s.dir("a");
        s.git.abort_joining(&dir).unwrap();
        assert_eq!(s.git.joining(&dir), None);
        assert_eq!(s.read("a", "Plan.md"), "one\ntwo from a\nthree\n");
        let log = s.git.run(&dir, &["log", "-1", "--format=%s"]).unwrap();
        assert!(log.starts_with("Auto:"), "{log}");
    }

    /// TC-015-09: with rebasing, here is this computer's version, and
    /// completing goes on with the rebase.
    #[test]
    fn uc015_tc09_rebase() {
        let s = Setup::new("rebase", "[pull]\n\trebase = true\n");
        assert!(matches!(s.sync("a"), SyncOutcome::Conflict(_)));
        let dir = s.dir("a");
        assert_eq!(s.git.joining(&dir), Some(Joining::Rebase));
        let text = s.read("a", "Plan.md");
        let places = conflict_places(&text, Joining::Rebase);
        assert_eq!(places[0].here, "two from a\n");
        fs::write(
            dir.join("Plan.md"),
            resolve_place(&text, &places[0], Choice::Here),
        )
        .unwrap();
        s.git.mark_resolved(&dir, "Plan.md").unwrap();
        s.git
            .take_side(&dir, "shot.png", Side::Remote, Joining::Rebase)
            .unwrap();
        assert_eq!(s.read("a", "shot.png"), "png b");
        s.git.complete_joining(&dir).unwrap();
        assert_eq!(s.git.joining(&dir), None);
        assert!(matches!(s.sync("a"), SyncOutcome::Synced { .. }));
    }
}
