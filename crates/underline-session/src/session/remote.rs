//! Feature F13 of the migration: UC-014 Synchronize with Remote in the
//! session (inventory M-19.8, M-04.1 to M-04.3).
//!
//! Ported from `switch_to` (the syncer), `request_sync`, `run_syncs`, the
//! close request of `update` and the status bar of
//! `apps/underline-desktop/src/app.rs`; the texts are egui's. How a sync
//! commits, joins and pushes stays in `underline_core::sync`.
//!
//! git never runs while the session is held (A-123, A-140): whether the
//! workspace has a remote is asked on a thread of its own when it opens,
//! and the syncs run on the [`Syncer`]'s thread (UC-014 BR-008); the tick
//! takes in their outcomes. git runs with the user's own sign-in and never
//! asks in a terminal (`GIT_TERMINAL_PROMPT=0`, `ssh -o BatchMode=yes`,
//! BR-005, as in egui); a stalled transfer ends (S7, review F13).
//!
//! One git job on the notes at a time (S1, review F13): a sync waits while
//! a switch, an abort or a step of resolving runs, and they wait for it;
//! only one sync is asked of the syncer at a time, a second one waits
//! (A8). No note is stored in the background while a sync runs (A-142).
//!
//! Inventory D-06 (fixed): the state of the last sync is the sync part of
//! the status bar (UC-002 step 9); egui's `git_state` was never set.
//! Inventory D-08 (fixed): a sync does not start when the open notes could
//! not be stored first; the status says why (A-141).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use underline_core::{BranchState, Distance, Git, SyncMode, SyncOutcome, SyncRequest, Syncer};

use super::branches::Pause;
use super::git::{branch_view, count};
use super::Session;
use crate::remote_view::{GitBarView, SyncBarView, SyncModeName};
use crate::Event;

/// UC-014 A9: how long Underline waits for the last sync before closing.
pub const CLOSING_SYNC_AT_MOST: Duration = Duration::from_secs(10);

/// S7 (review F13): a sync that has not ended after this long is given up
/// and reported as failed. git itself cannot be stopped from here (the
/// syncer of `underline_core` owns the process); the network limits below
/// end a stalled transfer much earlier.
pub(super) const SYNC_GIVEN_UP: Duration = Duration::from_secs(10 * 60);

/// S7: the reason a sync given up shows.
const GIVEN_UP: &str = "it took longer than 10 minutes and was given up";

/// S7: an HTTP transfer slower than this many bytes per second ...
const LOW_SPEED_LIMIT: &str = "1000";
/// ... for this many seconds is ended by git.
const LOW_SPEED_TIME: &str = "60";
/// S7: ssh as egui runs it (never asks in a terminal), giving up connecting
/// after 30 s; only when the user set no ssh command of their own.
const SSH_COMMAND: &str = "ssh -o BatchMode=yes -o ConnectTimeout=30";

/// S8 (review F13): `scheme://user:token@host` becomes `scheme://host`, so
/// a token in a remote's address never reaches the status, a hover or the
/// log.
#[must_use]
pub(super) fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace() || "'\"<>".contains(c))
            .unwrap_or(tail.len());
        let host = &tail[..end];
        match host.rfind('@') {
            Some(user) => out.push_str(&host[user + 1..]),
            None => out.push_str(host),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// A8: requests that pile up run once; a full sync takes the place of a
/// pull or a push (as the `Syncer` merges them).
fn merged(queued: Option<SyncMode>, mode: SyncMode) -> SyncMode {
    match queued {
        Some(SyncMode::Full) => SyncMode::Full,
        _ => mode,
    }
}

/// "just now", "N min ago", "N h ago" (egui's `ago`).
fn ago(since: Duration) -> String {
    let secs = since.as_secs();
    match secs {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

impl From<SyncModeName> for SyncMode {
    fn from(mode: SyncModeName) -> Self {
        match mode {
            SyncModeName::Full => Self::Full,
            SyncModeName::Pull => Self::Pull,
            SyncModeName::Push => Self::Push,
        }
    }
}

/// S7: the git of the syncer: HTTP transfers that stall end, and ssh gives
/// up connecting, unless the user set these themselves.
fn sync_git(git: Git, own_ssh: bool) -> Git {
    let mut git = git;
    for (key, value) in [
        ("GIT_HTTP_LOW_SPEED_LIMIT", LOW_SPEED_LIMIT),
        ("GIT_HTTP_LOW_SPEED_TIME", LOW_SPEED_TIME),
    ] {
        if std::env::var_os(key).is_none() {
            git = git.env(key, value);
        }
    }
    // `core.sshCommand` given through the environment, so
    // `underline_core::sync` takes it for the user's own and keeps it.
    if !own_ssh && std::env::var_os("GIT_CONFIG_COUNT").is_none() {
        git = git
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "core.sshCommand")
            .env("GIT_CONFIG_VALUE_0", SSH_COMMAND);
    }
    git
}

/// UC-014: the session's part of the syncs of the open workspace.
#[derive(Default)]
pub(super) struct Remote {
    /// The workspace whose syncs these are.
    root: Option<PathBuf>,
    /// The answer whether the workspace has a remote, and whether the user
    /// set an ssh command of their own, asked when it opened.
    probe: Option<Receiver<(bool, bool)>>,
    /// Runs the syncs, only for a workspace with `.git` and a remote.
    syncer: Option<Syncer>,
    /// A sync was asked of the syncer and has not ended; only one is asked
    /// at a time (S4, review F13).
    running: bool,
    /// Syncs asked of the syncer whose outcome has not come (round 2 of
    /// review F13): also one given up, whose git still runs. No new sync is
    /// asked, and saves wait, until it is 0.
    in_flight: u32,
    /// When the running sync was asked for (S7).
    started: Option<Instant>,
    /// A8, S1: the sync asked for while one runs or git works on the notes
    /// otherwise; it starts once that ended.
    queued: Option<SyncMode>,
    /// When the last sync ended (BR-001: the interval counts from here).
    ended: Option<Instant>,
    /// When the last sync succeeded (step 7).
    succeeded: Option<Instant>,
    /// How the last sync ended.
    last: Option<SyncOutcome>,
    /// BR-004: how far the branch and its remote branch are apart.
    distance: Distance,
    /// A9: Underline closes once the sync ended, or at this time.
    closing: Option<Instant>,
    /// What the status bar was told last.
    told: Option<GitBarView>,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Remote")
            .field("root", &self.root)
            .field("syncing", &self.syncer.is_some())
            .field("running", &self.running)
            .field("queued", &self.queued)
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl Remote {
    /// M-04.3: the state of the syncs as egui's status bar shows it; the
    /// reasons redacted (S8).
    fn bar(&self, conflicts: usize, now: Instant) -> Option<SyncBarView> {
        self.syncer.as_ref()?;
        let since = self
            .succeeded
            .map(|at| ago(now.saturating_duration_since(at)));
        let reason = |text: &str| Some(redact(text));
        let (text, warning, hover) = match (&self.last, self.running) {
            (_, true) => ("Syncing…".to_owned(), false, None),
            (None, false) => match since {
                // After "Abort the sync" (app-tester F13).
                Some(since) => (format!("Synced {since}"), false, None),
                None => ("Not synced yet".to_owned(), false, None),
            },
            (Some(SyncOutcome::Synced { .. }), _) => {
                (format!("Synced {}", since.unwrap_or_default()), false, None)
            }
            (Some(SyncOutcome::Offline(why)), _) => (
                match &since {
                    Some(since) => format!("Offline, will retry (synced {since})"),
                    None => "Offline, will retry".to_owned(),
                },
                false,
                reason(why),
            ),
            (Some(SyncOutcome::Failed(why)), _) => ("Sync failed".to_owned(), true, reason(why)),
            (Some(SyncOutcome::SignIn(why)), _) => {
                ("Sync failed: signing in".to_owned(), true, reason(why))
            }
            (Some(SyncOutcome::Conflict(files)), _) => (
                "Sync stopped: conflict".to_owned(),
                true,
                Some(files.join(", ")),
            ),
            (Some(SyncOutcome::InTheWay(files)), _) => {
                ("Sync stopped".to_owned(), true, Some(files.join(", ")))
            }
            (Some(SyncOutcome::PushRejected), _) => ("Push refused".to_owned(), true, None),
            (Some(SyncOutcome::NotPossible(why)), _) => ("No sync".to_owned(), false, reason(why)),
        };
        let Distance { ahead, behind } = self.distance;
        Some(SyncBarView {
            text,
            warning,
            hover,
            to_push: (ahead > 0).then(|| format!("{ahead} to push")),
            to_pull: (behind > 0).then(|| format!("{behind} to pull")),
            running: self.running,
            // UC-014 A10: no sync while conflicts wait.
            can_sync: !self.running && conflicts == 0,
        })
    }
}

impl Session {
    /// UC-014 step 1: a workspace was opened at `root`. The syncs of the
    /// workspace left end; whether this one has a remote (and whether the
    /// user set an ssh command of their own, S7) is asked on a thread of
    /// its own, and with one it syncs first thing (BR-001).
    pub(super) fn track_remote(&mut self, root: &Path) {
        let told = self.remote.told.take();
        self.remote = Remote {
            root: Some(root.to_owned()),
            told,
            ..Remote::default()
        };
        if !root.join(".git").exists() {
            return;
        }
        let (send, answer) = mpsc::channel();
        let setup = self.git_setup();
        let git = setup.git();
        let dir = root.to_owned();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("underline-remote".into())
            .spawn(move || {
                let has_remote = git
                    .remotes(&dir)
                    .is_some_and(|remotes| remotes.iter().any(|r| !r.is_empty()));
                let own_ssh = std::env::var_os("GIT_SSH_COMMAND").is_some()
                    || std::env::var_os("GIT_SSH").is_some()
                    || setup.has_config(&dir, "core.sshCommand");
                // The session may have moved on; then nobody waits.
                let _ = send.send((has_remote, own_ssh));
                wake.wake();
            });
        match spawned {
            Ok(_) => self.remote.probe = Some(answer),
            Err(error) => eprintln!("UC-014: the remote could not be looked for: {error}"),
        }
    }

    /// The answer whether the workspace has a remote: with one, the syncer
    /// starts and syncs (BR-001: when the workspace is opened).
    fn take_remote_probe(&mut self) {
        let Some(probe) = &self.remote.probe else {
            return;
        };
        let (has_remote, own_ssh) = match probe.try_recv() {
            Ok(answer) => answer,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => (false, false),
        };
        self.remote.probe = None;
        let Some(root) = self.remote.root.clone() else {
            return;
        };
        if !has_remote || self.workspace_root() != Some(root.as_path()) {
            return;
        }
        let wake = self.wake.clone();
        let git = sync_git(self.git_setup().git(), own_ssh);
        self.remote.syncer = Some(Syncer::start(git, root, move || {
            wake.wake();
        }));
        self.views_dirty = true;
        self.request_sync(SyncMode::Full, false);
    }

    /// Whether the open workspace syncs with a remote (M-04.3).
    #[must_use]
    pub fn syncs(&self) -> bool {
        self.remote.syncer.is_some()
    }

    /// Whether a sync runs (also one given up whose git still runs) or
    /// waits to (S1).
    pub(super) fn sync_running(&self) -> bool {
        self.remote.in_flight > 0 || self.remote.queued.is_some()
    }

    /// Whether git runs a sync now (not one only waiting to).
    pub(super) fn sync_in_flight(&self) -> bool {
        self.remote.in_flight > 0
    }

    /// UC-014 step 1, A1, A8, A10, M-19.8: a sync after the open notes are
    /// stored; `asked` when the user asked ("Sync now", "Pull", "Push"),
    /// so a refusal is reported. The window sent its edits first
    /// (`flushed`, ADR-025). While a sync runs, or git switches, aborts or
    /// resolves on the notes, it waits and starts once that ended (S1,
    /// S4); piled requests run once.
    ///
    /// Inventory D-08 (fixed, A-141): when a note cannot be stored, no sync
    /// starts and the status says why; egui synced anyway and showed
    /// "Syncing…".
    pub fn request_sync(&mut self, mode: SyncMode, asked: bool) {
        if self.remote.syncer.is_none() {
            if asked {
                self.status = "This workspace has no remote to sync with".into();
            }
            return;
        }
        let refusal = match self.git_changes() {
            Some(c) if c.merging => Some("notes are in conflict; resolve them first"),
            Some(c) if matches!(c.branch, BranchState::Detached(_)) => {
                Some("the workspace is not on a branch")
            }
            _ => None,
        };
        if let Some(reason) = refusal {
            if asked {
                self.status = format!("No sync, because {reason}");
            }
            return;
        }
        if self.remote.in_flight > 0 || self.git_job_busy() {
            self.remote.queued = Some(merged(self.remote.queued, mode));
            return;
        }
        // Step 2: open notes are stored first, then the syncer commits them.
        if let Err(failures) = self.flush_all() {
            self.status = format!(
                "Not everything could be saved before the sync: {}",
                failures.join("; ")
            );
            // D-08: the next interval tries again.
            self.remote.ended = Some(Instant::now());
            return;
        }
        let left_out = self.left_out();
        if let Some(syncer) = &self.remote.syncer {
            syncer.request(SyncRequest { mode, left_out });
        }
        self.remote.in_flight += 1;
        self.remote.running = true;
        self.remote.started = Some(Instant::now());
        self.pause_saves(Pause::Sync);
    }

    /// UC-014: the tick's part of the syncs: whether the workspace has a
    /// remote, the outcomes of syncs (steps 6–7, A3 to A11), a sync given
    /// up (S7), the one waiting (A8, S1), the interval (BR-001) and the end
    /// of the last sync before closing (A9).
    pub(super) fn run_syncs(&mut self, now: Instant) {
        self.take_remote_probe();
        let Some(syncer) = &self.remote.syncer else {
            return;
        };
        let outcomes = syncer.poll();
        for (_, outcome, distance) in outcomes {
            self.sync_done(now);
            self.remote.distance = distance;
            // Round 2 of review F13: a sync waiting since before a conflict
            // (or changes in the way) would only say "No sync" over it.
            if matches!(outcome, SyncOutcome::Conflict(_) | SyncOutcome::InTheWay(_)) {
                self.remote.queued = None;
            }
            self.sync_ended(&outcome, now);
            self.remote.last = Some(outcome);
            self.refresh_changes();
        }
        // S7: given up, only for the status bar; its git still runs, so
        // saves keep waiting and no other git job starts (round 2).
        if self.remote.running
            && self
                .remote
                .started
                .is_some_and(|at| now.saturating_duration_since(at) >= SYNC_GIVEN_UP)
        {
            eprintln!("UC-014: a sync did not end in time; it is given up");
            self.remote.running = false;
            self.remote.ended = Some(now);
            self.remote.last = Some(SyncOutcome::Failed(GIVEN_UP.into()));
        }
        if self.remote.in_flight == 0 && !self.git_job_busy() {
            if let Some(mode) = self.remote.queued.take() {
                self.request_sync(mode, false);
            }
        }
        if self.remote.closing.is_some() {
            return;
        }
        // BR-001: every few minutes after the last sync ended.
        let minutes = u64::from(self.settings.effective().git_sync_interval.max(1));
        let interval = Duration::from_secs(minutes * 60);
        if !self.sync_running()
            && self
                .remote
                .ended
                .is_some_and(|ended| now.saturating_duration_since(ended) >= interval)
        {
            self.request_sync(SyncMode::Full, false);
        }
    }

    /// The outcome of a sync asked of the syncer came.
    fn sync_done(&mut self, now: Instant) {
        self.remote.in_flight = self.remote.in_flight.saturating_sub(1);
        self.remote.ended = Some(now);
        if self.remote.in_flight == 0 {
            self.remote.running = false;
            self.remote.started = None;
            self.resume_saves(Pause::Sync);
        }
    }

    /// What a sync's outcome says in the status, redacted (S8), and the
    /// notes as they are now (step 6, BR-008; S5: also after a conflict or
    /// changes in the way, so no tab shows an older text).
    fn sync_ended(&mut self, outcome: &SyncOutcome, now: Instant) {
        match outcome {
            SyncOutcome::Synced { .. } => {
                self.remote.succeeded = Some(now);
                self.reload_tabs();
                self.refresh_overview();
            }
            SyncOutcome::Conflict(files) => {
                self.reload_tabs();
                self.refresh_overview();
                self.git_report(format!(
                    "The sync stopped: {} changed here and on the remote in the same lines",
                    files.join(", ")
                ));
            }
            SyncOutcome::InTheWay(files) => {
                self.reload_tabs();
                self.refresh_overview();
                self.git_report(format!(
                    "The sync stopped: changes you left out of commits are in the way of the remote's: {}. Commit or discard them.",
                    files.join(", ")
                ));
            }
            SyncOutcome::PushRejected => {
                self.git_report(
                    "The remote has commits this branch lacks; sync to join them first".into(),
                );
            }
            SyncOutcome::SignIn(reason) => {
                self.git_report(format!(
                    "Signing in to the remote failed ({}). Set up a sign-in for git, such as a saved token or an SSH key.",
                    redact(reason)
                ));
            }
            SyncOutcome::NotPossible(reason) => {
                self.status = format!("No sync, because {}", redact(reason));
            }
            SyncOutcome::Offline(_) | SyncOutcome::Failed(_) => {}
        }
    }

    /// App-tester F13: after "Abort the sync", the status bar no longer
    /// says "Sync stopped: conflict".
    pub(super) fn forget_stopped_sync(&mut self) {
        if matches!(self.remote.last, Some(SyncOutcome::Conflict(_))) {
            self.remote.last = None;
        }
    }

    /// UC-014 step 6, BR-008: the open tabs show their notes as the files
    /// hold them now, as for a change from outside (UC-004 A7); a tab with
    /// a change not stored keeps it and the other version as a copy (A9).
    /// The notes are listed again.
    pub(super) fn reload_tabs(&mut self) {
        let paths: Vec<PathBuf> = self
            .open
            .iter()
            .flat_map(|open| open.tabs.iter().map(|(_, t)| t.path.clone()))
            .collect();
        for path in paths {
            if path.exists() {
                self.outside_change(&path);
            }
        }
        if let Some(open) = &mut self.open {
            open.workspace.rescan();
        }
        self.views_dirty = true;
    }

    /// UC-014 A9: Underline is to close and every note was stored: with a
    /// remote, a last sync is asked for and, when it starts or waits, the
    /// status says "Syncing before closing…"; `true` then, and Underline
    /// closes once [`Session::closing_sync_over`]. `false` without a remote,
    /// when no sync can start (K5, review F13), or when a last sync was
    /// asked for already (a second close goes through, as in egui).
    pub fn closing_sync(&mut self, now: Instant) -> bool {
        if self.remote.syncer.is_none() || self.remote.closing.is_some() {
            return false;
        }
        self.request_sync(SyncMode::Full, false);
        if !self.sync_running() {
            return false;
        }
        self.remote.closing = Some(now + CLOSING_SYNC_AT_MOST);
        self.status = "Syncing before closing…".into();
        true
    }

    /// UC-014 A9: the last sync before closing ended (also one that waited
    /// for another, S4), or it took [`CLOSING_SYNC_AT_MOST`]; Underline
    /// closes now.
    #[must_use]
    pub fn closing_sync_over(&self, now: Instant) -> bool {
        self.remote
            .closing
            .is_none_or(|deadline| !self.sync_running() || now >= deadline)
    }

    /// M-04.1 to M-04.3: the git part of the status bar, when it changed.
    pub(super) fn git_bar_event(&mut self, now: Instant) -> Option<Event> {
        let view = self.git_bar(now);
        // Nothing to show was told from the start.
        if self.remote.told.clone().unwrap_or_default() == view {
            return None;
        }
        self.remote.told = Some(view.clone());
        Some(Event::GitBar(view))
    }

    fn git_bar(&self, now: Instant) -> GitBarView {
        let conflicts = self.conflict_count();
        let (branch, problem) = match self.git_answer() {
            Some(Ok(changes)) => {
                let branch = branch_view(&changes.branch).label;
                let text = match changes.files.len() {
                    0 => format!("Branch {branch}"),
                    n => format!("Branch {branch} · {}", count(n, "change")),
                };
                (Some(text), None)
            }
            Some(Err(err)) => (
                None,
                Some(format!("Changes cannot be shown: {}", redact(err))),
            ),
            None => (None, None),
        };
        GitBarView {
            branch,
            problem,
            conflicts: match conflicts {
                0 => None,
                1 => Some("1 conflict".to_owned()),
                n => Some(format!("{n} conflicts")),
            },
            sync: self.remote.bar(conflicts, now),
        }
    }
}

#[cfg(test)]
impl Session {
    /// For the tests: the running sync started `ago` before now (S7).
    pub(super) fn sync_started_before(&mut self, ago: Duration) {
        self.remote.started = Some(Instant::now() - ago);
    }
}

#[cfg(test)]
mod tests;
