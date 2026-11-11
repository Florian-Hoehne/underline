//! Feature F13 of the migration: UC-013 Manage Branches in the session
//! (inventory M-19.6, M-19.7).
//!
//! Ported from `open_branch_menu`, `show_branch_menu` and `switch_branch`
//! of `apps/underline-desktop/src/app.rs`; the texts are egui's. What git
//! is asked stays in `underline_core::branches`.
//!
//! git never runs while the session is held (A-123): listing the branches
//! ([`BranchesJob`]) and switching ([`SwitchJob`]) are prepared under the
//! session, run without it, and finished under it again. While a switch
//! runs, no note is stored in the background, so git never meets a file
//! half written (A-142). The window sent its edits first (`flushed`,
//! ADR-025), so the notes are stored with every key before the switch.

use std::path::PathBuf;

use underline_core::{branch_name, Branches, Git, NoteId, SwitchError};

use super::git::count;
use super::Session;
use crate::remote_view::{
    BranchEntryView, BranchMenuView, BranchNameView, BranchTarget, SwitchAnswer,
};

/// Why background saves wait (A-142): git works on the notes. Each reason
/// ends with its job: a switch or an abort when its command finishes (also
/// when its job failed, `resume_background_saves`), resolving when its
/// thread answered or ended, a sync when its outcome came or it was given
/// up (`remote.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Pause {
    /// A branch switch runs (UC-013 step 4).
    Switch,
    /// "Abort the sync" runs (UC-015 A5).
    Abort,
    /// A step of resolving conflicts runs (UC-015 steps 6–7).
    Resolve,
    /// A sync runs: it commits, pulls and may rebase (UC-014 steps 2–5).
    Sync,
}

/// M-19.6, UC-013 step 2: the branches to list, asked without the session.
#[derive(Debug)]
pub struct BranchesJob {
    git: Git,
    root: PathBuf,
}

/// What [`BranchesJob::run`] found.
#[derive(Debug)]
pub struct BranchesListed {
    root: PathBuf,
    branches: Result<Branches, String>,
}

impl BranchesJob {
    /// UC-013 step 2, BR-001: the local branches, the current one, and the
    /// remote's without a local one, as of the last sync; nothing is sent
    /// over the network.
    #[must_use]
    pub fn run(self) -> BranchesListed {
        BranchesListed {
            branches: self.git.branches(&self.root).map_err(|e| e.to_string()),
            root: self.root,
        }
    }
}

/// How a switch goes (UC-013 step 4, A1, A4).
#[derive(Debug)]
enum How {
    /// To the local branch.
    Switch,
    /// A4: a local branch following the remote branch of this name.
    Track(String),
    /// A1: a new branch from the current commit.
    Create,
}

/// UC-013 step 4: a switch to make without the session.
#[derive(Debug)]
pub struct SwitchJob {
    git: Git,
    root: PathBuf,
    /// The branch switched to.
    name: String,
    how: How,
    /// Step 6: the changes carried along.
    carried: usize,
}

/// What a switch gave.
#[derive(Debug)]
pub struct SwitchOutcome {
    root: PathBuf,
    name: String,
    carried: usize,
    result: Result<(), SwitchError>,
}

impl SwitchJob {
    /// UC-013 step 4, A1, A4, BR-003: switches with git, carrying along the
    /// changes that are not committed.
    #[must_use]
    pub fn run(self) -> SwitchOutcome {
        let result = match &self.how {
            How::Switch => self.git.switch_branch(&self.root, &self.name),
            How::Track(remote) => self.git.track_branch(&self.root, remote),
            How::Create => self.git.create_branch(&self.root, &self.name),
        };
        SwitchOutcome {
            root: self.root,
            name: self.name,
            carried: self.carried,
            result,
        }
    }
}

/// M-19.6, UC-013 A1, BR-005: "Branch: {clean}" for the name typed, or why
/// git would not take it.
#[must_use]
pub fn branch_name_view(typed: &str) -> BranchNameView {
    match branch_name(typed) {
        Ok(clean) => BranchNameView {
            text: format!("Branch: {clean}"),
            ok: true,
        },
        Err(problem) => BranchNameView {
            text: format!("{problem}."),
            ok: false,
        },
    }
}

impl Session {
    /// M-19.6, UC-013 step 1: the branch menu opens only in a workspace
    /// whose changes are tracked (egui: `tracker.is_some()`); the branches
    /// are listed without the session.
    #[must_use]
    pub fn branches_prepare(&self) -> Option<BranchesJob> {
        // A-143: only while git answers (not with "Changes cannot be shown").
        if !self.tracks_changes() || self.git_changes().is_none() {
            return None;
        }
        Some(BranchesJob {
            git: self.git_setup().git(),
            root: self.workspace_root()?.to_owned(),
        })
    }

    /// M-19.6, UC-013 step 2, A7: what the branch menu lists; `None` when
    /// another workspace opened meanwhile. The local branches are kept to
    /// tell a new branch's name that is taken (A2).
    pub fn branches_listed(&mut self, listed: BranchesListed) -> Option<BranchMenuView> {
        if self.workspace_root() != Some(listed.root.as_path()) {
            return None;
        }
        let merging = self.git_changes().is_some_and(|c| c.merging);
        let (branches, problem) = match listed.branches {
            Ok(branches) => (branches, None),
            Err(problem) => (Branches::default(), Some(super::remote::redact(&problem))),
        };
        let current = branches.current.clone();
        let entries = branches
            .local
            .iter()
            .map(|name| (name, false))
            .chain(branches.remote.iter().map(|name| (name, true)))
            .map(|(name, remote)| {
                let is_current = current.as_deref() == Some(name.as_str());
                let label = match (is_current, remote) {
                    (true, _) => format!("{name}  (current)"),
                    (_, true) => format!("{name}  (on the remote)"),
                    _ => name.clone(),
                };
                BranchEntryView {
                    name: name.clone(),
                    label,
                    current: is_current,
                    remote,
                }
            })
            .collect();
        self.branches_listed = Some(branches);
        Some(BranchMenuView {
            branches: entries,
            problem,
            merging,
        })
    }

    /// UC-013 steps 3–4, A1, A2, A4, A6, A7, M-19.7: a branch chosen in the
    /// menu, once the window sent its edits. The open notes are stored
    /// first ("Did not switch, because …"); a new branch's name must be
    /// allowed and not taken. The job switches without the session;
    /// [`Session::finish_switch`] shows the branch's notes.
    ///
    /// # Errors
    ///
    /// The answer when nothing is switched: the menu closes (the status
    /// says why, or the current branch was chosen), or the name is taken
    /// (A2) or not allowed.
    pub fn switch_prepare(&mut self, target: &BranchTarget) -> Result<SwitchJob, SwitchAnswer> {
        let Some(changes) = self.git_changes() else {
            return Err(SwitchAnswer::Done);
        };
        // A7: no switch while a merge waits.
        if changes.merging {
            return Err(SwitchAnswer::Done);
        }
        let carried = changes.files.len();
        // S1 (review F13): one git job on the notes at a time.
        if self.sync_running() || self.git_job_busy() {
            self.status =
                "Did not switch, because a sync is running; try again when it ended".into();
            return Err(SwitchAnswer::Done);
        }
        let listed = self.branches_listed.as_ref();
        let (name, how) = match target {
            BranchTarget::Local { name } => {
                // S6 (review F13): only a branch the menu listed; so a name
                // such as `--detach` never reaches git.
                let Some(listed) = listed.filter(|b| b.local.contains(name)) else {
                    return Err(SwitchAnswer::Done);
                };
                if listed.current.as_deref() == Some(name.as_str()) {
                    return Err(SwitchAnswer::Done);
                }
                (name.clone(), How::Switch)
            }
            BranchTarget::Remote { name } => {
                if !listed.is_some_and(|b| b.remote.contains(name)) {
                    return Err(SwitchAnswer::Done);
                }
                let local = name.split_once('/').map_or(name.as_str(), |(_, n)| n);
                (local.to_owned(), How::Track(name.clone()))
            }
            BranchTarget::New { name } => {
                let clean = branch_name(name).map_err(|problem| SwitchAnswer::Invalid {
                    problem: format!("{problem}."),
                })?;
                if listed.is_some_and(|b| b.local.contains(&clean)) {
                    // A2: taken; offered to switch to it instead.
                    return Err(SwitchAnswer::Exists {
                        problem: format!(
                            "The branch {clean} exists; choose it in the list to switch to it."
                        ),
                        name: clean,
                    });
                }
                (clean, How::Create)
            }
        };
        // A6: open notes are stored first.
        if let Err(failures) = self.flush_all() {
            self.status = format!("Did not switch, because {}", failures.join("; "));
            return Err(SwitchAnswer::Done);
        }
        let Some(root) = self.workspace_root().map(std::path::Path::to_owned) else {
            return Err(SwitchAnswer::Done);
        };
        self.pause_saves(Pause::Switch);
        Ok(SwitchJob {
            git: self.git_setup().git(),
            root,
            name,
            how,
            carried,
        })
    }

    /// UC-013 steps 5–6, A3, A5, BR-004, M-19.7: after the switch, tabs of
    /// notes the branch does not hold close, the others show their notes
    /// as the branch holds them (a tab typed in meanwhile keeps its text
    /// and the branch's version as a copy, UC-004 A9), the notes are listed
    /// again, and the status says "Switched to X; carried along N
    /// change(s); closed A, B, not on this branch". A3 keeps the menu open
    /// with the notes in the way; A5 says git's reason.
    pub fn finish_switch(&mut self, outcome: SwitchOutcome) -> SwitchAnswer {
        self.resume_saves(Pause::Switch);
        if self.workspace_root() != Some(outcome.root.as_path()) {
            return SwitchAnswer::Done;
        }
        let name = outcome.name;
        match outcome.result {
            Ok(()) => {
                self.branches_listed = None;
                let (closed, kept, kept_copy) = self.follow_branch();
                // UC-004 A9: a copy kept is the news to tell; it said so.
                if !kept_copy {
                    let mut status = format!("Switched to {name}");
                    if outcome.carried > 0 {
                        status.push_str(&format!(
                            "; carried along {}",
                            count(outcome.carried, "change")
                        ));
                    }
                    if !closed.is_empty() {
                        status.push_str(&format!(
                            "; closed {}, not on this branch",
                            closed.join(", ")
                        ));
                    }
                    if !kept.is_empty() {
                        // M1 (review F13, new text): never dropped.
                        status.push_str(&format!(
                            "; kept {} open with changes not stored, not on this branch; they are stored again",
                            kept.join(", ")
                        ));
                    }
                    self.git_report(status);
                }
                self.after_switch();
                // K1 (review F13).
                self.refresh_changes();
                SwitchAnswer::Done
            }
            // A3.
            Err(SwitchError::WouldOverwrite(files)) => SwitchAnswer::InTheWay {
                text: format!(
                    "Changes not committed would be overwritten: {}. Commit them first, or stay.",
                    files.join(", ")
                ),
            },
            // A5.
            Err(SwitchError::Git(err)) => {
                self.status = format!(
                    "Could not switch to {name}: {}",
                    super::remote::redact(&err.to_string())
                );
                self.refresh_changes();
                SwitchAnswer::Done
            }
        }
    }

    /// UC-013 step 5, BR-004: tabs of notes the branch does not hold close;
    /// the others show the notes as the branch holds them, quietly as in
    /// egui. A tab whose note is gone but holds a change not stored (typed
    /// while git switched) stays open, marked deleted, and is stored again
    /// at its place (M1, review F13: no text is dropped; as UC-004 A8 and
    /// UC-005 A15). Returns the titles of the tabs closed and kept, and
    /// whether a tab typed in meanwhile kept its text and the branch's
    /// version as a copy (UC-004 A9, which the status then says).
    fn follow_branch(&mut self) -> (Vec<String>, Vec<String>, bool) {
        let Some(open) = &mut self.open else {
            return (Vec::new(), Vec::new(), false);
        };
        let gone: Vec<NoteId> = open
            .tabs
            .iter()
            .filter(|(_, tab)| !tab.path.exists())
            .map(|(id, _)| id.clone())
            .collect();
        let mut closed = Vec::new();
        let mut kept = Vec::new();
        for id in gone {
            let Some(index) = open.tabs.find(&id) else {
                continue;
            };
            let tab = open.tabs.get_mut(index);
            if tab.autosave.is_unstored() || tab.saving {
                tab.deleted = true;
                kept.push(id.title());
            } else {
                open.tabs.close(index);
                closed.push(id.title());
            }
        }
        let paths: Vec<PathBuf> = open
            .tabs
            .iter()
            .filter(|(_, t)| t.path.exists())
            .map(|(_, t)| t.path.clone())
            .collect();
        let said = self.status.clone();
        for path in paths {
            self.take_file_version(&path, false);
        }
        let kept_copy = self.status != said;
        (closed, kept, kept_copy)
    }

    /// The notes of the branch are listed again and the tabs remembered.
    fn after_switch(&mut self) {
        if let Some(open) = &mut self.open {
            open.workspace.rescan();
        }
        self.persist_tabs();
        self.refresh_overview();
        self.views_dirty = true;
    }

    /// Background saves wait for `why` (A-142).
    pub(super) fn pause_saves(&mut self, why: Pause) {
        self.saves_paused.insert(why);
    }

    /// Background saves no longer wait for `why`.
    pub(super) fn resume_saves(&mut self, why: Pause) {
        self.saves_paused.remove(&why);
    }

    /// Whether git switches, aborts or resolves on the notes now, or a step
    /// of resolving waits: a sync waits for it (S1, review F13).
    pub(super) fn git_job_busy(&self) -> bool {
        self.saves_paused.contains(&Pause::Switch)
            || self.saves_paused.contains(&Pause::Abort)
            || self.resolving_conflicts()
    }

    /// A switch or an abort ended without its outcome (its job stopped
    /// unexpectedly): background saves go on.
    pub fn resume_background_saves(&mut self) {
        self.resume_saves(Pause::Switch);
        self.resume_saves(Pause::Abort);
    }

    /// Sets the status to a git report the background's "Saved {t}" does
    /// not replace (like A-113).
    pub(super) fn git_report(&mut self, text: String) {
        self.status.clone_from(&text);
        self.git_report = Some(text);
    }
}
