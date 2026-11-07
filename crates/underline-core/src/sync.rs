//! UC-014 Synchronize with Remote: commit what changed, join the remote's
//! changes, push, through the installed git and the user's own sign-in
//! (ADR-006). A sync runs in the background (BR-008).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::changes::{proposed_message, FileChange};
use crate::git::{Git, GitError};

/// What a sync does (UC-014 step 1, A1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Commit, pull, push.
    Full,
    /// A1: commit and pull, no push.
    Pull,
    /// A1: commit and push, without fetching first.
    Push,
}

/// How a sync ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Steps 3–5: commits pulled and pushed; `committed` is the automatic
    /// commit's short name, if one was made.
    Synced {
        pulled: usize,
        pushed: usize,
        committed: Option<String>,
    },
    /// A3: these notes and files are in conflict (UC-015).
    Conflict(Vec<String>),
    /// A11: changes left out are in the way of the remote's changes.
    InTheWay(Vec<String>),
    /// A1 step 2: the remote holds commits the local branch lacks.
    PushRejected,
    /// A4.
    Offline(String),
    /// A5: signing in to this remote failed.
    SignIn(String),
    /// A7: git's reason.
    Failed(String),
    /// A10: why no sync is possible.
    NotPossible(String),
}

/// How far the branch and its remote branch are apart (UC-014 BR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Distance {
    /// Commits waiting to be pushed, and to be pulled.
    pub ahead: usize,
    pub behind: usize,
}

/// The automatic commit's message (UC-014 BR-002).
pub const AUTO_PREFIX: &str = "Auto: ";

impl Git {
    /// UC-014 steps 2–5: commits the changes not left out (BR-002), then
    /// pulls and pushes as `mode` says.
    pub fn sync(&self, dir: &Path, mode: SyncMode, left_out: &HashSet<String>) -> SyncOutcome {
        match self.sync_steps(dir, mode, left_out) {
            Ok(outcome) | Err(outcome) => outcome,
        }
    }

    fn sync_steps(
        &self,
        dir: &Path,
        mode: SyncMode,
        left_out: &HashSet<String>,
    ) -> Result<SyncOutcome, SyncOutcome> {
        let failed = |e: GitError| SyncOutcome::Failed(e.to_string());
        let changes = self.changes(dir).map_err(failed)?;
        // A10.
        if changes.merging {
            return Err(SyncOutcome::NotPossible(
                "notes are in conflict; resolve them first".into(),
            ));
        }
        let Some(branch) = self
            .run(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|b| b.trim().to_owned())
        else {
            return Err(SyncOutcome::NotPossible(
                "the workspace is not on a branch".into(),
            ));
        };
        let remotes: Vec<String> = self
            .remotes(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !r.is_empty())
            .collect();
        if remotes.is_empty() {
            return Err(SyncOutcome::NotPossible(
                "the workspace has no remote".into(),
            ));
        }
        // Step 2, BR-002.
        let chosen: Vec<FileChange> = changes
            .files
            .iter()
            .filter(|c| !left_out.contains(&c.path))
            .cloned()
            .collect();
        let committed = if chosen.is_empty() {
            None
        } else {
            let refs: Vec<&FileChange> = chosen.iter().collect();
            let message = format!("{AUTO_PREFIX}{}", proposed_message(&refs));
            Some(
                self.commit_changes(dir, &chosen, &message, false)
                    .map_err(failed)?,
            )
        };
        let ssh = self.ssh_env(dir);
        // A6: a branch without a remote branch is pushed as a new one.
        let upstream = self
            .run(
                dir,
                &[
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}",
                ],
            )
            .ok();
        let Some(upstream) = upstream else {
            if mode == SyncMode::Pull {
                return Ok(SyncOutcome::Synced {
                    pulled: 0,
                    pushed: 0,
                    committed,
                });
            }
            let remote = remotes
                .iter()
                .find(|r| r.as_str() == "origin")
                .unwrap_or(&remotes[0])
                .clone();
            let pushed = self
                .run(dir, &["rev-list", "--count", "HEAD"])
                .ok()
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            self.network(
                dir,
                &ssh,
                &["push", "--quiet", "--set-upstream", &remote, &branch],
            )?;
            return Ok(SyncOutcome::Synced {
                pulled: 0,
                pushed,
                committed,
            });
        };
        let remote = upstream
            .trim()
            .split('/')
            .next()
            .unwrap_or("origin")
            .to_owned();
        let mut pulled = 0;
        if mode != SyncMode::Push {
            self.network(dir, &ssh, &["fetch", "--quiet", &remote])?;
            pulled = self.distance(dir).behind;
            if pulled > 0 {
                // BR-003: as the user's git setup says, merging by default.
                let mut pull = vec!["pull", "--quiet", "--no-edit"];
                if self.run(dir, &["config", "--get", "pull.rebase"]).is_err() {
                    pull.push("--no-rebase");
                }
                if let Err(outcome) = self.network(dir, &ssh, &pull) {
                    return Err(self.why_pull_failed(dir, outcome));
                }
            }
        }
        let mut pushed = 0;
        if mode != SyncMode::Pull {
            pushed = self.distance(dir).ahead;
            if pushed > 0 {
                if let Err(outcome) = self.network(dir, &ssh, &["push", "--quiet"]) {
                    return Err(match outcome {
                        SyncOutcome::Failed(m)
                            if m.contains("rejected") || m.contains("fetch first") =>
                        {
                            SyncOutcome::PushRejected
                        }
                        other => other,
                    });
                }
            }
        }
        Ok(SyncOutcome::Synced {
            pulled,
            pushed,
            committed,
        })
    }

    /// UC-014 BR-004: how far the branch and its remote branch are apart, as
    /// of the last fetch.
    pub fn distance(&self, dir: &Path) -> Distance {
        self.run(
            dir,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )
        .ok()
        .and_then(|out| {
            let mut parts = out.split_whitespace().map(|n| n.parse().unwrap_or(0));
            Some(Distance {
                ahead: parts.next()?,
                behind: parts.next()?,
            })
        })
        .unwrap_or_default()
    }

    /// A3, A11: why a pull did not join.
    fn why_pull_failed(&self, dir: &Path, outcome: SyncOutcome) -> SyncOutcome {
        let conflicts: Vec<String> = self
            .run(dir, &["diff", "--name-only", "--diff-filter=U"])
            .map(|out| out.lines().map(str::to_owned).collect())
            .unwrap_or_default();
        if !conflicts.is_empty() {
            return SyncOutcome::Conflict(conflicts);
        }
        match outcome {
            SyncOutcome::Failed(message) if message.contains("would be overwritten") => {
                SyncOutcome::InTheWay(
                    message
                        .lines()
                        .filter(|l| l.starts_with('\t'))
                        .map(|l| l.trim().to_owned())
                        .collect(),
                )
            }
            other => other,
        }
    }

    /// BR-008: ssh may never ask in a terminal; the user's own ssh command
    /// is kept when they set one.
    fn ssh_env(&self, dir: &Path) -> Option<String> {
        let own = std::env::var_os("GIT_SSH_COMMAND").is_some()
            || std::env::var_os("GIT_SSH").is_some()
            || self
                .run(dir, &["config", "--get", "core.sshCommand"])
                .is_ok();
        (!own).then(|| "ssh -o BatchMode=yes".to_owned())
    }

    /// A git command that talks to the remote, its failure sorted into A4,
    /// A5, or A7.
    fn network(&self, dir: &Path, ssh: &Option<String>, args: &[&str]) -> Result<(), SyncOutcome> {
        let git = match ssh {
            Some(command) => self.clone().env("GIT_SSH_COMMAND", command),
            None => self.clone(),
        };
        match git.run(dir, args) {
            Ok(_) => Ok(()),
            Err(GitError::Failed { message, .. }) => Err(classify(&message)),
            Err(err) => Err(SyncOutcome::Failed(err.to_string())),
        }
    }
}

/// Sorts git's complaint into the alternative flows of UC-014.
fn classify(message: &str) -> SyncOutcome {
    let lower = message.to_lowercase();
    let sign_in = [
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "permission denied (publickey",
        "invalid username or password",
        "host key verification failed",
    ];
    let offline = [
        "could not resolve host",
        "failed to connect",
        "connection refused",
        "network is unreachable",
        "connection timed out",
        "operation timed out",
        "could not connect",
        "unable to access",
        "temporary failure in name resolution",
    ];
    let line = message
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(message)
        .trim()
        .to_owned();
    if sign_in.iter().any(|s| lower.contains(s)) {
        SyncOutcome::SignIn(line)
    } else if offline.iter().any(|s| lower.contains(s)) {
        SyncOutcome::Offline(line)
    } else {
        SyncOutcome::Failed(message.trim().to_owned())
    }
}

/// A sync to run (UC-014 step 1).
#[derive(Debug, Clone)]
pub struct SyncRequest {
    pub mode: SyncMode,
    /// Changes the user left out of commits (BR-002).
    pub left_out: HashSet<String>,
}

/// Runs syncs in the background, one at a time (A8, BR-008).
pub struct Syncer {
    requests: Sender<SyncRequest>,
    results: Receiver<(SyncMode, SyncOutcome, Distance)>,
}

impl Syncer {
    /// Starts the syncer of the workspace at `dir`; `wake` is called when a
    /// sync ended.
    pub fn start(git: Git, dir: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, inbox) = channel::<SyncRequest>();
        let (outbox, results) = channel();
        std::thread::Builder::new()
            .name("underline-sync".into())
            .spawn(move || {
                while let Ok(mut request) = inbox.recv() {
                    // A8: requests that piled up run once.
                    while let Ok(newer) = inbox.try_recv() {
                        if newer.mode == SyncMode::Full || request.mode != SyncMode::Full {
                            request = newer;
                        }
                    }
                    let outcome = git.sync(&dir, request.mode, &request.left_out);
                    let distance = git.distance(&dir);
                    if outbox.send((request.mode, outcome, distance)).is_err() {
                        return;
                    }
                    wake();
                }
            })
            .expect("the sync thread starts");
        Self { requests, results }
    }

    pub fn request(&self, request: SyncRequest) {
        let _ = self.requests.send(request);
    }

    /// The syncs that ended since the last call.
    pub fn poll(&self) -> Vec<(SyncMode, SyncOutcome, Distance)> {
        self.results.try_iter().collect()
    }

    /// Waits up to `timeout` for a sync to end, as when closing (A9).
    pub fn wait(&self, timeout: std::time::Duration) -> Option<(SyncMode, SyncOutcome, Distance)> {
        self.results.recv_timeout(timeout).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::isolated_git;
    use std::fs;

    struct Setup {
        base: PathBuf,
        git: Git,
    }

    impl Setup {
        /// A bare remote and two clones of it, `a` and `b`.
        fn new(name: &str, config: &str) -> Self {
            let base =
                std::env::temp_dir().join(format!("underline-uc014-{name}-{}", std::process::id()));
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
            fs::write(base.join("a/Ideas.md"), "ideas\n").unwrap();
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
            Self { base, git }
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

        fn log(&self, clone: &str, range: &str) -> String {
            self.git
                .run(&self.dir(clone), &["log", "--format=%s", range])
                .unwrap()
        }

        fn sync(&self, clone: &str, mode: SyncMode) -> SyncOutcome {
            self.git.sync(&self.dir(clone), mode, &HashSet::new())
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    /// TC-014-01 / TC-014-02 / TC-014-06 / TC-014-08: changes committed as
    /// Auto, joined by merging by default, pushed; nothing to do after.
    #[test]
    fn uc014_tc01_tc02_tc06_tc08_sync() {
        let s = Setup::new("sync", "");
        s.write("b", "Ideas.md", "ideas from b\n");
        assert!(matches!(
            s.sync("b", SyncMode::Full),
            SyncOutcome::Synced { pushed: 1, .. }
        ));
        s.write("a", "Plan.md", "one\ntwo\nthree\nfour\n");
        let outcome = s.sync("a", SyncMode::Full);
        assert!(
            matches!(
                outcome,
                SyncOutcome::Synced {
                    pulled: 1,
                    pushed: 2,
                    committed: Some(_)
                }
            ),
            "{outcome:?}"
        );
        assert_eq!(s.read("a", "Ideas.md"), "ideas from b\n");
        let log = s.log("a", "origin/main");
        assert!(log.contains("Auto: Update Plan"), "{log}");
        assert!(log.contains("Merge"), "merged by default: {log}");
        assert_eq!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::Synced {
                pulled: 0,
                pushed: 0,
                committed: None
            }
        );
        assert_eq!(s.git.distance(&s.dir("a")), Distance::default());
    }

    /// TC-014-06: with `pull.rebase`, local commits go on top.
    #[test]
    fn uc014_tc06_rebase_when_set_up() {
        let s = Setup::new("rebase", "[pull]\n\trebase = true\n");
        s.write("b", "Ideas.md", "ideas from b\n");
        s.sync("b", SyncMode::Full);
        s.write("a", "Plan.md", "one\ntwo\nthree\nfour\n");
        assert!(matches!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::Synced { .. }
        ));
        let log = s.log("a", "origin/main");
        assert!(!log.contains("Merge"), "{log}");
        assert!(log.starts_with("Auto: Update Plan"), "{log}");
    }

    /// TC-014-03 / TC-014-17: a change left out stays out, and is named
    /// when it is in the way.
    #[test]
    fn uc014_tc03_tc17_left_out() {
        let s = Setup::new("left", "");
        s.write("a", "Plan.md", "one\ntwo\nthree\nlocal\n");
        s.write("a", "Ideas.md", "ideas 2\n");
        let left: HashSet<String> = ["Ideas.md".to_owned()].into();
        assert!(matches!(
            s.git.sync(&s.dir("a"), SyncMode::Full, &left),
            SyncOutcome::Synced { pushed: 1, .. }
        ));
        assert_eq!(s.git.changes(&s.dir("a")).unwrap().files.len(), 1);
        s.write("b", "Ideas.md", "ideas from b\n");
        s.sync("b", SyncMode::Full);
        match s.git.sync(&s.dir("a"), SyncMode::Full, &left) {
            SyncOutcome::InTheWay(files) => assert_eq!(files, ["Ideas.md"]),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.read("a", "Ideas.md"), "ideas 2\n");
    }

    /// TC-014-07: pull only, push only, and a push the remote refuses.
    #[test]
    fn uc014_tc07_pull_and_push() {
        let s = Setup::new("pullpush", "");
        s.write("b", "Ideas.md", "ideas from b\n");
        s.sync("b", SyncMode::Full);
        s.write("a", "Plan.md", "one\ntwo\nthree\nfour\n");
        assert_eq!(s.sync("a", SyncMode::Push), SyncOutcome::PushRejected);
        assert!(matches!(
            s.sync("a", SyncMode::Pull),
            SyncOutcome::Synced {
                pulled: 1,
                pushed: 0,
                ..
            }
        ));
        assert_eq!(s.git.distance(&s.dir("a")).ahead, 2);
        assert!(matches!(
            s.sync("a", SyncMode::Push),
            SyncOutcome::Synced { pushed: 2, .. }
        ));
    }

    /// TC-014-09: the same lines changed on both sides are a conflict.
    #[test]
    fn uc014_tc09_conflict() {
        let s = Setup::new("conflict", "");
        s.write("b", "Plan.md", "one\nTWO from b\nthree\n");
        s.sync("b", SyncMode::Full);
        s.write("a", "Plan.md", "one\ntwo from a\nthree\n");
        assert_eq!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::Conflict(vec!["Plan.md".into()])
        );
        assert!(s.git.changes(&s.dir("a")).unwrap().merging);
        assert!(matches!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::NotPossible(_)
        ));
    }

    /// TC-014-12: a new branch goes to the remote and is followed.
    #[test]
    fn uc014_tc12_new_branch() {
        let s = Setup::new("branch", "");
        s.git.create_branch(&s.dir("a"), "trip").unwrap();
        s.write("a", "Plan.md", "trip plan\n");
        assert!(matches!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::Synced { .. }
        ));
        let status = s.git.run(&s.dir("a"), &["status", "-sb"]).unwrap();
        assert!(status.starts_with("## trip...origin/trip"), "{status}");
    }

    /// TC-014-10 / TC-014-16: an unreachable remote is offline; without a
    /// remote no sync is possible.
    #[test]
    fn uc014_tc10_tc16_offline_and_not_possible() {
        let s = Setup::new("offline", "");
        s.git
            .run(
                &s.dir("a"),
                &[
                    "remote",
                    "set-url",
                    "origin",
                    "http://127.0.0.1:9/remote.git",
                ],
            )
            .unwrap();
        s.write("a", "Plan.md", "changed\n");
        assert!(matches!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::Offline(_)
        ));
        // The automatic commit stays.
        assert_eq!(s.git.distance(&s.dir("a")).ahead, 1);
        s.git
            .run(&s.dir("a"), &["remote", "remove", "origin"])
            .unwrap();
        assert!(matches!(
            s.sync("a", SyncMode::Full),
            SyncOutcome::NotPossible(_)
        ));
    }

    #[test]
    fn complaints_sorted() {
        assert!(matches!(
            classify("fatal: could not read Username for 'https://x': terminal prompts disabled"),
            SyncOutcome::SignIn(_)
        ));
        assert!(matches!(
            classify("git@x: Permission denied (publickey)."),
            SyncOutcome::SignIn(_)
        ));
        assert!(matches!(
            classify("fatal: unable to access 'https://x/': Could not resolve host: x"),
            SyncOutcome::Offline(_)
        ));
        assert!(matches!(
            classify("remote: protected branch"),
            SyncOutcome::Failed(_)
        ));
    }
}
