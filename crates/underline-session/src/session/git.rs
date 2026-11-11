//! Feature F12 of the migration: UC-011 Review Changes and UC-012 Commit
//! Changes in the session (inventory M-19.1 to M-19.5, M-05.3).
//!
//! Ported from `changes_ui`, `change_action`, `commit_changes`,
//! `show_identity`, `change_marks`, `refresh_changes`, `track_changes`,
//! `start_discard`, `show_discard` and `discard_change` of
//! `apps/underline-desktop/src/app.rs`; the texts are egui's. What git is
//! asked and how a diff is made stays in `underline_core::changes`.
//!
//! git runs as a program (ADR-006) and never while the session is held
//! (inventory D-16, fixed):
//! - the changes are asked by the [`ChangeTracker`] on its own thread, at
//!   the start, every [`CHANGES_EVERY`] and after the notes changed; the tick
//!   takes in its answers;
//! - the diff of the chosen file or the active note is made on a thread of
//!   its own, at most every [`DIFF_EVERY`] while the text changes, from a
//!   copy of the tab's text (the mirror, ADR-025);
//! - committing ([`CommitJob`]), keeping the user's name ([`IdentityJob`]),
//!   the question before discarding ([`DiscardPlan`]) and the committed
//!   content a discard puts back ([`DiscardJob`]) are prepared under the
//!   session, run without it, and finished under it again.
//!
//! What the window does first (ADR-025, `flushed`): a commit, the question
//! before discarding and the discard itself run after the window sent its
//! edits, so the mirrors hold the tabs' texts with every key.
//!
//! The jobs run git with literal pathspecs and tell a file that was not
//! committed from a failure of git (`runner.rs`, review F12).
//!
//! The branch button of the status bar (M-04.1), the branch menu, the sync
//! after a commit and the actions of conflicts come with feature F13 (hooks
//! below); `in_conflict` stays its stub until then.

mod runner;

pub(super) use runner::GitSetup;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ropey::Rope;
use underline_core::{
    changed_lines, diff, now, proposed_message, write_atomic, BranchState, ChangeKind,
    ChangeTracker, Changes, DiscardError, FileChange, GitError, Hunk, LineKind, NoteId,
    OnlyDiffers,
};
use underline_editor_core::utf16::WebOffsets;

use super::Session;
use crate::editor_view::units;
use crate::git_view::{
    BranchView, ChangeKindName, ChangeView, ChangesView, CommitAnswer, DiffBody, DiffLineKind,
    DiffLineView, DiffPartView, DiffView, DiscardQuestion, HunkView,
};
use crate::Event;

/// UC-011 A3: how often git is asked again for the changes, so commits made
/// outside show up (egui: 3 s).
pub(super) const CHANGES_EVERY: Duration = Duration::from_secs(3);

/// UC-011 A3, M-19.3: how often the diff of a changing text is made again,
/// at most (egui: 700 ms).
pub(super) const DIFF_EVERY: Duration = Duration::from_millis(700);

/// UC-012 A1.
pub const NEEDS_MESSAGE: &str = "A commit needs a message.";

/// UC-012 A2.
pub const NOTHING_CHOSEN: &str = "Nothing is chosen.";

/// A diff job that has not answered after this long is given up, so a job
/// whose thread died never stops the diff (review F12).
const DIFF_GIVEN_UP: Duration = Duration::from_secs(30);

/// Review F12, new text (A-135): the diff job stopped unexpectedly.
const DIFF_STOPPED: &str = "it stopped unexpectedly";

/// "1 note", "2 notes" (egui's `count`).
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{n} {thing}s")
    }
}

/// The diff shown, and what it was made from (egui's `ShownDiff`).
#[derive(Debug)]
struct ShownDiff {
    /// The number of the job that made it; the window is told a new one.
    seq: u64,
    /// The file and the revision of the tab's text and the answer of git
    /// it was made for.
    made_for: MadeFor,
    made_at: Instant,
    view: DiffView,
}

/// The file, the revision of the tab's text and the answer of git a diff
/// is made for.
type MadeFor = (String, Option<u32>, u64);

/// The committed content of a file in one commit, kept for the next diff:
/// the place, the commit, the content.
type CommittedCache = (String, String, Option<Vec<u8>>);

/// What a diff job sends back: its number, the diff (`None` when it
/// panicked) and the committed content it read.
type MadeDiff = (u64, Option<DiffView>, Option<CommittedCache>);

/// The diff job that runs.
#[derive(Debug)]
struct Running {
    seq: u64,
    made_for: MadeFor,
    started: Instant,
}

/// The jobs that make the diff, one at a time.
#[derive(Debug)]
struct Diffs {
    shown: Option<ShownDiff>,
    running: Option<Running>,
    next: u64,
    /// The committed content the last job read (review F12).
    cache: Option<CommittedCache>,
    send: Sender<MadeDiff>,
    made: Receiver<MadeDiff>,
}

impl Default for Diffs {
    fn default() -> Self {
        let (send, made) = mpsc::channel();
        Self {
            shown: None,
            running: None,
            next: 1,
            cache: None,
            send,
            made,
        }
    }
}

/// UC-011, UC-012: the session's part of the git changes of the open
/// workspace.
#[derive(Default)]
pub(super) struct GitChanges {
    /// How the jobs run git (the environment of the session's `Git`).
    setup: GitSetup,
    /// The workspace whose changes these are.
    root: Option<PathBuf>,
    /// Asks git on its own thread; only for a workspace with `.git`.
    tracker: Option<ChangeTracker>,
    /// git's newest answer.
    changes: Option<Result<Changes, String>>,
    /// Grows with every answer.
    generation: u64,
    /// When git was asked last.
    asked: Option<Instant>,
    /// UC-012 BR-004: the changes left out of the commit, by place.
    left_out: HashSet<String>,
    /// UC-011 A4, A5: the file chosen in the list that has no tab.
    chosen: Option<FileChange>,
    /// The diff of the chosen file or the active note.
    diffs: Diffs,
    /// UC-011 A6: the change "Discard changes" asks about.
    discarding: Option<FileChange>,
    /// The view of the changes needs to be made again.
    dirty: bool,
    /// M-05.3: the letters of the overview, and their number of changes.
    pub(super) marks: HashMap<NoteId, char>,
    pub(super) marks_rev: u64,
    /// The answer of git the marks were made of.
    marks_generation: u64,
    /// What the window was told last: the changes, and the number of the
    /// diff (`None`: none).
    told_changes: Option<ChangesView>,
    told_diff: Option<u64>,
}

impl std::fmt::Debug for GitChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitChanges")
            .field("tracking", &self.tracker.is_some())
            .field("generation", &self.generation)
            .field("left_out", &self.left_out)
            .finish_non_exhaustive()
    }
}

impl GitChanges {
    /// The changes as git answered them, if it did.
    fn current(&self) -> Option<&Changes> {
        self.changes.as_ref().and_then(|c| c.as_ref().ok())
    }

    /// The change of `path`, as git answered last.
    fn change(&self, path: &str) -> Option<&FileChange> {
        self.current()?.files.iter().find(|f| f.path == path)
    }
}

/// M-04.1: where the workspace stands, as egui's status bar names it.
fn branch_view(branch: &BranchState) -> BranchView {
    match branch {
        BranchState::Branch(name) => BranchView {
            label: name.clone(),
            detached: false,
        },
        BranchState::NoCommit(name) => BranchView {
            label: format!("{name} (no commit yet)"),
            detached: false,
        },
        BranchState::Detached(commit) => BranchView {
            label: format!("not on a branch: {commit}"),
            detached: true,
        },
    }
}

/// The title of the note at `path` (with its old one), or the path.
fn change_name(change: &FileChange) -> String {
    match (change.note(), &change.kind) {
        (Some((id, _)), ChangeKind::Renamed { from }) => {
            let old = from.rsplit_once('.').map_or(from.as_str(), |(s, _)| s);
            format!("{} ← {}", id.title(), NoteId::from_relative(old).title())
        }
        (Some((id, _)), _) => match id.folder() {
            Some(folder) => format!("{} · {folder}", id.title()),
            None => id.title(),
        },
        (None, _) => change.path.clone(),
    }
}

fn kind_name(kind: &ChangeKind) -> ChangeKindName {
    match kind {
        ChangeKind::New => ChangeKindName::New,
        ChangeKind::Changed => ChangeKindName::Changed,
        ChangeKind::Deleted => ChangeKindName::Deleted,
        ChangeKind::Renamed { .. } => ChangeKindName::Renamed,
        ChangeKind::Conflict { .. } => ChangeKindName::Conflict,
    }
}

/// UC-011 steps 4, 6, A1, A9, UC-012 steps 2–3: the view of git's answer
/// with the commit's choice.
fn changes_view(answer: &Result<Changes, String>, left_out: &HashSet<String>) -> ChangesView {
    let changes = match answer {
        Ok(changes) => changes,
        Err(problem) => {
            return ChangesView {
                problem: Some(problem.clone()),
                branch: None,
                files: Vec::new(),
                merging: false,
                amendable: None,
                proposed: String::new(),
            }
        }
    };
    let chosen: Vec<&FileChange> = changes
        .files
        .iter()
        .filter(|c| !left_out.contains(&c.path))
        .collect();
    ChangesView {
        problem: None,
        branch: Some(branch_view(&changes.branch)),
        files: changes
            .files
            .iter()
            .map(|change| ChangeView {
                path: change.path.clone(),
                kind: kind_name(&change.kind),
                letter: change.kind.letter().to_string(),
                name: change_name(change),
                note: change.note().is_some(),
                chosen: !left_out.contains(&change.path),
            })
            .collect(),
        merging: changes.merging,
        amendable: changes.amendable.clone(),
        proposed: proposed_message(&chosen),
    }
}

/// What the diff is made of.
#[derive(Debug)]
enum DiffSource {
    /// The text of a tab (BR-001: with its changes not yet saved), of
    /// revision `rev`.
    Tab { note: NoteId, rev: u32, rope: Rope },
    /// A file chosen in the list (A4, A5), read by the job.
    File { kind: ChangeKind },
}

/// One diff to make without the session.
#[derive(Debug)]
struct DiffJob {
    setup: GitSetup,
    root: PathBuf,
    path: String,
    /// Where the file was at the last commit (a renamed note's old place).
    from: String,
    source: DiffSource,
    cache: Option<CommittedCache>,
}

/// The diff when git could not be asked (review F12, new text, A-135).
fn diff_problem(path: String, note: Option<NoteId>, rev: Option<u32>, why: &str) -> DiffView {
    DiffView {
        path,
        note: note.map(|n| n.as_str().to_owned()),
        rev,
        body: DiffBody::Problem {
            text: format!("The diff cannot be shown: {why}"),
        },
    }
}

/// "This file is not text; it is {kind}." (UC-011 A5).
fn not_text(path: String, kind: &ChangeKind) -> DiffView {
    let kind = match kind {
        ChangeKind::New => "new",
        ChangeKind::Deleted => "deleted",
        _ => "changed",
    };
    DiffView {
        path,
        note: None,
        rev: None,
        body: DiffBody::NotText {
            text: format!("This file is not text; it is {kind}."),
        },
    }
}

impl DiffJob {
    /// The committed content of `from`, from the cache when the last commit
    /// is the one it was read from.
    fn committed(&mut self) -> Result<Option<Vec<u8>>, String> {
        let head = self.setup.head_commit(&self.root)?;
        let key = head.clone().unwrap_or_default();
        if let Some((path, commit, content)) = &self.cache {
            if *path == self.from && *commit == key {
                return Ok(content.clone());
            }
        }
        let content = self
            .setup
            .committed_in(&self.root, head.as_deref(), &self.from)?;
        self.cache = Some((self.from.clone(), key, content.clone()));
        Ok(content)
    }

    /// UC-011 step 6, A4, A5, BR-004: the diff against the last commit,
    /// three lines around each changed part (egui's `track_changes`). A
    /// failure of git says so instead of showing every line as added
    /// (review F12).
    fn run(mut self) -> (DiffView, Option<CommittedCache>) {
        let committed = self.committed();
        let cache = self.cache.take();
        let view = self.view(committed);
        (view, cache)
    }

    fn view(self, committed: Result<Option<Vec<u8>>, String>) -> DiffView {
        let (note, rev, text, rope) = match self.source {
            DiffSource::Tab { note, rev, rope } => {
                (Some(note), Some(rev), Some(rope.to_string()), Some(rope))
            }
            DiffSource::File { ref kind } => {
                let text = match kind {
                    ChangeKind::Deleted => Some(String::new()),
                    _ => std::fs::read(self.root.join(&self.path))
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok()),
                };
                let Some(text) = text else {
                    return not_text(self.path, kind);
                };
                (None, None, Some(text), None)
            }
        };
        let old = match committed {
            Ok(None) => String::new(),
            Ok(Some(bytes)) => match String::from_utf8(bytes) {
                Ok(old) => old,
                Err(_) => {
                    let mut view = not_text(self.path, &ChangeKind::Changed);
                    view.note = note.map(|n| n.as_str().to_owned());
                    view.rev = rev;
                    return view;
                }
            },
            Err(why) => return diff_problem(self.path, note, rev, &why),
        };
        let hunks = diff(&old, text.as_deref().unwrap_or_default(), 3);
        let body = if hunks.is_empty() {
            DiffBody::Unchanged
        } else {
            DiffBody::Hunks {
                hunks: hunks_view(&hunks, rope.as_ref()),
            }
        };
        DiffView {
            path: self.path,
            note: note.map(|n| n.as_str().to_owned()),
            rev,
            body,
        }
    }
}

/// M-19.3: the hunks as the window shows them; with the tab's `rope`, each
/// line knows where a click puts the cursor (UC-011 steps 7–8).
fn hunks_view(hunks: &[Hunk], rope: Option<&Rope>) -> Vec<HunkView> {
    let web = rope.map(WebOffsets::of);
    let number = |n: Option<usize>| n.map_or_else(|| "    ".to_owned(), |n| format!("{n:>4}"));
    hunks
        .iter()
        .map(|hunk| HunkView {
            lines: hunk
                .lines
                .iter()
                .map(|line| {
                    let (kind, sign) = match line.kind {
                        LineKind::Same => (DiffLineKind::Same, " "),
                        LineKind::Removed => (DiffLineKind::Removed, "-"),
                        LineKind::Added => (DiffLineKind::Added, "+"),
                    };
                    let mut parts = Vec::new();
                    let mut at = 0;
                    for word in &line.words {
                        if word.start > at {
                            parts.push(DiffPartView {
                                text: line.text[at..word.start].to_owned(),
                                strong: false,
                            });
                        }
                        parts.push(DiffPartView {
                            text: line.text[word.clone()].to_owned(),
                            strong: true,
                        });
                        at = word.end;
                    }
                    if at < line.text.len() || parts.is_empty() {
                        parts.push(DiffPartView {
                            text: line.text[at..].to_owned(),
                            strong: false,
                        });
                    }
                    let offset = match (rope, &web, line.new.or(line.old)) {
                        (Some(rope), Some(web), Some(n)) => {
                            let line = n.saturating_sub(1).min(rope.len_lines().saturating_sub(1));
                            Some(units(web.to_web(rope, rope.line_to_char(line))))
                        }
                        _ => None,
                    };
                    DiffLineView {
                        kind,
                        numbers: format!("{} {} {sign} ", number(line.old), number(line.new)),
                        parts,
                        only: line.only.map(|only| {
                            match only {
                                OnlyDiffers::Spaces => "  (only spaces differ)",
                                OnlyDiffers::LineEnding => "  (only the line ending differs)",
                            }
                            .to_owned()
                        }),
                        offset,
                    }
                })
                .collect(),
        })
        .collect()
}

/// UC-012 step 6: a commit to make without the session.
#[derive(Debug)]
pub struct CommitJob {
    git: underline_core::Git,
    root: PathBuf,
    chosen: Vec<FileChange>,
    message: String,
    amend: bool,
}

/// What a commit gave.
#[derive(Debug)]
pub struct CommitOutcome {
    root: PathBuf,
    result: Result<String, GitError>,
    notes: usize,
    files: usize,
    amend: bool,
}

impl CommitJob {
    /// UC-012 step 6, A6, BR-008: records the chosen changes with git; the
    /// repository's checks run as they do with git.
    #[must_use]
    pub fn run(self) -> CommitOutcome {
        let result = self
            .git
            .commit_changes(&self.root, &self.chosen, &self.message, self.amend);
        let notes = self.chosen.iter().filter(|c| c.note().is_some()).count();
        CommitOutcome {
            root: self.root,
            result,
            notes,
            files: self.chosen.len() - notes,
            amend: self.amend,
        }
    }
}

/// UC-012 A3: the user's name and email to keep without the session.
#[derive(Debug)]
pub struct IdentityJob {
    setup: GitSetup,
    root: PathBuf,
    name: String,
    email: String,
}

impl IdentityJob {
    /// UC-012 A3 step 3, BR-005: keeps them in the user's own git settings,
    /// for every repository; never only the name (review F12).
    ///
    /// # Errors
    ///
    /// git could not keep them; the text says why.
    pub fn run(self) -> Result<(), String> {
        self.setup.set_identity(&self.root, &self.name, &self.email)
    }
}

/// UC-011 A6 step 1: what the question before discarding needs, made
/// without the session.
#[derive(Debug)]
pub struct DiscardPlan {
    setup: GitSetup,
    root: PathBuf,
    change: FileChange,
    /// The tab's text, if the note has a tab; else the file is read.
    text: Option<String>,
}

/// Why the changes of a note cannot be discarded; the status says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscardRefused {
    title: String,
    why: String,
}

impl DiscardPlan {
    /// M-19.5: "Discard the changes of {title} since the last commit?" and
    /// what is lost, with the lines that differ from the last commit.
    ///
    /// # Errors
    ///
    /// git could not tell the committed content (review F12): no question
    /// that would claim the whole note is lost.
    pub fn question(self) -> Result<DiscardQuestion, DiscardRefused> {
        let title = discard_title(&self.change);
        let from = match &self.change.kind {
            ChangeKind::Renamed { from } => from.clone(),
            _ => self.change.path.clone(),
        };
        let consequence = match self.change.kind {
            ChangeKind::New => "The note is new; it goes to the deleted notes.".to_owned(),
            ChangeKind::Deleted => "The note comes back as it was committed.".to_owned(),
            _ => {
                let old = match self.setup.committed(&self.root, &from) {
                    Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
                    Ok(None) => {
                        return Err(DiscardRefused {
                            title,
                            why: DiscardError::NotCommitted(from).to_string(),
                        })
                    }
                    Err(why) => return Err(DiscardRefused { title, why }),
                };
                let new = self.text.unwrap_or_else(|| {
                    std::fs::read_to_string(self.root.join(&self.change.path)).unwrap_or_default()
                });
                let lines = changed_lines(&diff(&old, &new, 0));
                format!(
                    "{} will be lost and cannot be restored.",
                    count(lines, "changed line")
                )
            }
        };
        Ok(DiscardQuestion {
            path: self.change.path.clone(),
            question: format!("Discard the changes of {title} since the last commit?"),
            consequence,
        })
    }
}

/// The title of the note of `change`, else its place.
fn discard_title(change: &FileChange) -> String {
    change
        .note()
        .map_or_else(|| change.path.clone(), |(id, _)| id.title())
}

/// UC-011 A6 step 2: the index set back and the committed content to put
/// back, without the session.
#[derive(Debug)]
pub struct DiscardJob {
    setup: GitSetup,
    root: PathBuf,
    change: FileChange,
}

/// What [`DiscardJob::run`] did and fetched: the committed content as
/// bytes (`None` for a new note), or why not.
#[derive(Debug)]
pub struct DiscardFetched {
    root: PathBuf,
    change: FileChange,
    committed: Result<Option<Vec<u8>>, String>,
}

impl DiscardJob {
    /// Sets the index of the note (and of its old place) back to the last
    /// commit, so nothing of it stays staged (review F12), and fetches the
    /// content the note had at the last commit (at its old place when
    /// renamed), byte for byte.
    #[must_use]
    pub fn run(self) -> DiscardFetched {
        let committed = self.fetch();
        DiscardFetched {
            root: self.root,
            change: self.change,
            committed,
        }
    }

    fn fetch(&self) -> Result<Option<Vec<u8>>, String> {
        let path = self.change.path.as_str();
        let from = match &self.change.kind {
            ChangeKind::Renamed { from } => Some(from.as_str()),
            _ => None,
        };
        let mut paths = vec![path];
        paths.extend(from);
        self.setup.unstage(&self.root, &paths)?;
        if self.change.kind == ChangeKind::New {
            return Ok(None);
        }
        let from = from.unwrap_or(path);
        match self.setup.committed(&self.root, from)? {
            Some(bytes) => Ok(Some(bytes)),
            None => Err(DiscardError::NotCommitted(from.to_owned()).to_string()),
        }
    }
}

/// `restore_committed` of `underline_core::changes` with the content
/// fetched before, byte for byte: writes it at `path` of the workspace at
/// `root`.
fn write_committed(root: &Path, path: &str, content: &[u8]) -> Result<(), String> {
    let target = root.join(path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| DiscardError::Io(e).to_string())?;
    }
    write_atomic(&target, content).map_err(|e| DiscardError::Io(e).to_string())
}

impl Session {
    /// UC-011 step 1: a workspace was opened at `root`; its changes are
    /// tracked when it is a git repository (egui: `.git` in its folder).
    pub(super) fn track_changes(&mut self, root: &Path) {
        // UC-012 BR-004: the choice holds for this workspace only.
        let left_out = if self.changes.root.as_deref() == Some(root) {
            std::mem::take(&mut self.changes.left_out)
        } else {
            HashSet::new()
        };
        let setup = std::mem::take(&mut self.changes.setup);
        let marks_rev = self.changes.marks_rev + 1;
        // What the window was told stays, so it hears that the changes and
        // the diff of the workspace left are gone.
        let told_changes = self.changes.told_changes.take();
        let told_diff = self.changes.told_diff.take();
        self.changes = GitChanges {
            setup,
            root: Some(root.to_owned()),
            left_out,
            marks_rev,
            dirty: true,
            told_changes,
            told_diff,
            ..GitChanges::default()
        };
        if root.join(".git").exists() {
            let wake = self.wake.clone();
            // The tracker's `git status` takes no lock of the index, so it
            // never makes a commit fail (git's optional locks).
            let git = self.changes.setup.git().env("GIT_OPTIONAL_LOCKS", "0");
            self.changes.tracker = Some(ChangeTracker::start(git, root.to_owned(), move || {
                wake.wake();
            }));
            self.changes.asked = Some(Instant::now());
        }
    }

    /// UC-011 A3: asks git again for the changes.
    pub(super) fn refresh_changes(&mut self) {
        if let Some(tracker) = &self.changes.tracker {
            tracker.refresh();
            self.changes.asked = Some(Instant::now());
        }
    }

    /// Whether the open workspace is a git repository whose changes are
    /// tracked (M-16.4).
    #[must_use]
    pub fn tracks_changes(&self) -> bool {
        self.changes.tracker.is_some()
    }

    /// M-05.3, UC-011 step 2: the letters of the changed notes, and `M` for
    /// a tab with changes not stored yet, only in a git workspace.
    fn change_marks(&self) -> HashMap<NoteId, char> {
        let mut marks: HashMap<NoteId, char> = self
            .changes
            .current()
            .iter()
            .flat_map(|c| &c.files)
            .filter_map(|f| f.note().map(|(id, _)| (id, f.kind.letter())))
            .collect();
        if self.changes.tracker.is_some() {
            if let Some(open) = &self.open {
                for (id, tab) in open.tabs.iter() {
                    if tab.autosave.is_unstored() {
                        marks.entry(id.clone()).or_insert('M');
                    }
                }
            }
        }
        marks
    }

    /// M-05.3: the marks are made again when git answered anew or a tab's
    /// changes were stored or made (`views_dirty`); a change of them sends
    /// the overview again.
    pub(super) fn update_marks(&mut self, dirty: bool) {
        if !dirty && self.changes.marks_generation == self.changes.generation {
            return;
        }
        self.changes.marks_generation = self.changes.generation;
        let marks = self.change_marks();
        if marks != self.changes.marks {
            self.changes.marks = marks;
            self.changes.marks_rev += 1;
        }
    }

    /// UC-011 step 1, A3, M-19.2, M-19.3: the tick's part of the changes:
    /// git's answers are taken in, git is asked again every
    /// [`CHANGES_EVERY`], the diff is made again when the text or the
    /// answer changed (at most every [`DIFF_EVERY`]), and the window is told
    /// what changed (`changes`, `diff`).
    pub(super) fn git_events(&mut self, now: Instant) -> Vec<Event> {
        let mut events = Vec::new();
        if let Some(tracker) = &self.changes.tracker {
            if let Some(answer) = tracker.poll() {
                // UC-012 BR-004: the choice is kept for the changes still
                // listed.
                if let Ok(changes) = &answer {
                    self.changes
                        .left_out
                        .retain(|p| changes.files.iter().any(|f| &f.path == p));
                }
                self.changes.changes = Some(answer);
                self.changes.generation += 1;
                self.changes.dirty = true;
            }
            if self
                .changes
                .asked
                .is_none_or(|at| now.saturating_duration_since(at) >= CHANGES_EVERY)
            {
                self.refresh_changes();
            }
        }
        self.take_diffs(now);
        self.start_diff(now);
        if std::mem::take(&mut self.changes.dirty) {
            let view = self
                .changes
                .changes
                .as_ref()
                .filter(|_| self.changes.tracker.is_some())
                .map(|answer| changes_view(answer, &self.changes.left_out));
            if view != self.changes.told_changes {
                self.changes.told_changes.clone_from(&view);
                events.push(Event::Changes(view));
            }
        }
        let shown = self.changes.diffs.shown.as_ref().map(|d| d.seq);
        if shown != self.changes.told_diff {
            self.changes.told_diff = shown;
            let diff = self.changes.diffs.shown.as_ref().map(|d| d.view.clone());
            events.push(Event::Diff(diff));
        }
        events
    }

    /// Takes in the diff the job made, if it is the one that runs; a job
    /// that panicked shows why, and one that never answers is given up
    /// after [`DIFF_GIVEN_UP`] (review F12).
    fn take_diffs(&mut self, now: Instant) {
        while let Ok((seq, view, cache)) = self.changes.diffs.made.try_recv() {
            let diffs = &mut self.changes.diffs;
            if cache.is_some() {
                diffs.cache = cache;
            }
            let Some(running) = diffs.running.take_if(|running| running.seq == seq) else {
                continue;
            };
            let view = view.unwrap_or_else(|| {
                let (path, rev, _) = &running.made_for;
                diff_problem(path.clone(), None, *rev, DIFF_STOPPED)
            });
            diffs.shown = Some(ShownDiff {
                seq,
                made_for: running.made_for,
                made_at: running.started,
                view,
            });
        }
        let diffs = &mut self.changes.diffs;
        if diffs
            .running
            .as_ref()
            .is_some_and(|running| now.saturating_duration_since(running.started) >= DIFF_GIVEN_UP)
        {
            eprintln!("M-19.3: the diff job did not answer; it is given up");
            diffs.running = None;
        }
    }

    /// UC-011 step 6, A3: starts the diff of the file chosen in the list,
    /// else of the active note, when it is not of its text and answer yet.
    /// A diff of another file than the one to show is not shown meanwhile
    /// (review F12).
    fn start_diff(&mut self, now: Instant) {
        let Some(open) = self
            .open
            .as_ref()
            .filter(|_| self.changes.tracker.is_some())
        else {
            self.changes.diffs.shown = None;
            return;
        };
        let root = open.root().to_owned();
        let active = open.tabs.active().map(|(id, tab)| {
            (
                id.clone(),
                id.file_name(tab.edit.format),
                tab.mirror.rev(),
                tab,
            )
        });
        let (path, source, rev) = match (&self.changes.chosen, active) {
            (Some(change), active)
                if active.as_ref().is_none_or(|(_, p, ..)| *p != change.path) =>
            {
                let source = DiffSource::File {
                    kind: change.kind.clone(),
                };
                (change.path.clone(), source, None)
            }
            (_, Some((note, path, rev, tab))) => {
                let rope = tab.mirror.rope().clone();
                (path, DiffSource::Tab { note, rev, rope }, Some(rev))
            }
            _ => {
                self.changes.diffs.shown = None;
                return;
            }
        };
        let diffs = &mut self.changes.diffs;
        if diffs.shown.as_ref().is_some_and(|d| d.view.path != path) {
            diffs.shown = None;
        }
        if diffs.running.is_some() {
            return;
        }
        let made_for = (path.clone(), rev, self.changes.generation);
        if let Some(shown) = &diffs.shown {
            if shown.made_for == made_for {
                return;
            }
            // A3: while typing, at most every DIFF_EVERY.
            if now.saturating_duration_since(shown.made_at) < DIFF_EVERY {
                return;
            }
        }
        let from = match self.changes.change(&path).map(|c| &c.kind) {
            Some(ChangeKind::Renamed { from }) => from.clone(),
            _ => path.clone(),
        };
        let job = DiffJob {
            setup: self.changes.setup.clone(),
            root,
            path,
            from,
            source,
            cache: self.changes.diffs.cache.clone(),
        };
        let diffs = &mut self.changes.diffs;
        let seq = diffs.next;
        diffs.next += 1;
        let send = diffs.send.clone();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("underline-diff".into())
            .spawn(move || {
                let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.run()));
                let made = match made {
                    Ok((view, cache)) => (seq, Some(view), cache),
                    Err(_) => {
                        eprintln!("M-19.3: making the diff panicked");
                        (seq, None, None)
                    }
                };
                // The session may be gone; then nobody waits for the diff.
                let _ = send.send(made);
                wake.wake();
            });
        match spawned {
            Ok(_) => {
                diffs.running = Some(Running {
                    seq,
                    made_for,
                    started: now,
                });
            }
            Err(error) => eprintln!("M-19.3: the diff could not be made: {error}"),
        }
    }

    /// UC-011 step 5, A4, A5, M-19.2: a click on the change of `path` in the
    /// list: a note opens in a tab (its diff follows the tab); a deleted
    /// note or another file is shown without a tab.
    pub fn choose_change(&mut self, path: &str) {
        let Some(change) = self.changes.change(path).cloned() else {
            return;
        };
        match (&change.kind, change.note()) {
            (ChangeKind::Deleted, _) | (_, None) => self.changes.chosen = Some(change),
            (_, Some((id, _))) => {
                self.changes.chosen = None;
                self.open_note(&id);
            }
        }
        self.changes.diffs.shown = None;
    }

    /// UC-012 step 3, BR-004, M-19.1: the change of `path` is chosen for the
    /// commit or left out; without a path, all ("All") or none ("None").
    pub fn choose_for_commit(&mut self, path: Option<&str>, chosen: bool) {
        match (path, chosen) {
            (Some(path), true) => {
                self.changes.left_out.remove(path);
            }
            (Some(path), false) => {
                self.changes.left_out.insert(path.to_owned());
            }
            (None, true) => self.changes.left_out.clear(),
            (None, false) => {
                self.changes.left_out = self
                    .changes
                    .current()
                    .iter()
                    .flat_map(|c| &c.files)
                    .map(|f| f.path.clone())
                    .collect();
            }
        }
        self.changes.dirty = true;
    }

    /// UC-012 steps 4–5, A1, A2, A5, M-19.4: a commit of the chosen
    /// changes with `message` (`amend`: the last commit amended, A6), once
    /// the window sent its edits. The open notes are stored first. The job
    /// runs git without the session; [`Session::finish_commit`] reports it.
    ///
    /// The choice is that of git's last answer, which may be up to
    /// [`CHANGES_EVERY`] old, as in egui: a change that appeared since is
    /// not committed (its file stays changed), one that went is left to
    /// git, which commits nothing for it.
    ///
    /// # Errors
    ///
    /// The answer when nothing is committed: a refusal for the commit box
    /// (A1, A2), or nothing more (A5 and the rest say so in the status).
    pub fn commit_prepare(
        &mut self,
        message: &str,
        amend: bool,
    ) -> Result<CommitJob, CommitAnswer> {
        let Some(changes) = self.changes.current() else {
            return Err(CommitAnswer::Done);
        };
        // BR-009: no commit during a merge.
        if changes.merging {
            return Err(CommitAnswer::Done);
        }
        let message = message.trim().to_owned();
        if message.is_empty() {
            return Err(CommitAnswer::Problem {
                problem: NEEDS_MESSAGE.to_owned(),
            });
        }
        let chosen: Vec<FileChange> = changes
            .files
            .iter()
            .filter(|c| !self.changes.left_out.contains(&c.path))
            .cloned()
            .collect();
        let amend = amend && changes.amendable.is_some();
        if chosen.is_empty() && !amend {
            return Err(CommitAnswer::Problem {
                problem: NOTHING_CHOSEN.to_owned(),
            });
        }
        // Step 5, A5: open notes are stored first.
        if let Err(failures) = self.flush_all() {
            self.status = format!("Nothing was committed, because {}", failures.join("; "));
            return Err(CommitAnswer::Done);
        }
        let Some(root) = self.workspace_root().map(Path::to_owned) else {
            return Err(CommitAnswer::Done);
        };
        Ok(CommitJob {
            git: self.changes.setup.git(),
            root,
            chosen,
            message,
            amend,
        })
    }

    /// UC-012 step 7, A3, A4: "Committed {id} holding N note(s) and M
    /// file(s)" (or "Amended the last commit, now {id}…"), and the changes
    /// asked again; git not knowing the user asks for the name (A3).
    pub fn finish_commit(&mut self, outcome: CommitOutcome) -> CommitAnswer {
        if self.workspace_root() != Some(outcome.root.as_path()) {
            // Another workspace opened meanwhile.
            return CommitAnswer::Done;
        }
        match outcome.result {
            Ok(id) => {
                let mut held = vec![];
                if outcome.notes > 0 {
                    held.push(count(outcome.notes, "note"));
                }
                if outcome.files > 0 {
                    held.push(count(outcome.files, "file"));
                }
                let held = if held.is_empty() {
                    String::new()
                } else {
                    format!(" holding {}", held.join(" and "))
                };
                self.status = if outcome.amend {
                    format!("Amended the last commit, now {id}{held}")
                } else {
                    format!("Committed {id}{held}")
                };
                self.changes.left_out.clear();
                self.changes.dirty = true;
                self.refresh_changes();
                // F13: UC-014 BR-001, a commit is synced right after
                // (`request_sync(SyncMode::Full, false)`).
                CommitAnswer::Committed
            }
            // A3: git does not know the user.
            Err(GitError::NoIdentity) => CommitAnswer::Identity,
            // A4.
            Err(err) => {
                self.status = format!("The commit failed: {err}");
                CommitAnswer::Done
            }
        }
    }

    /// UC-012 A3 step 2, M-19.4: "Commit" in "Your name for commits": the
    /// name and email to keep, when the name is not empty and the email has
    /// an `@` (egui enables the button only then), and neither holds a line
    /// break or a NUL, which would break the git settings (review F12);
    /// `None` otherwise or without a workspace.
    #[must_use]
    pub fn identity_prepare(&self, name: &str, email: &str) -> Option<IdentityJob> {
        let (name, email) = (name.trim(), email.trim());
        let broken = |text: &str| text.contains(['\n', '\r', '\0']);
        if name.is_empty() || !email.contains('@') || broken(name) || broken(email) {
            return None;
        }
        Some(IdentityJob {
            setup: self.changes.setup.clone(),
            root: self.workspace_root()?.to_owned(),
            name: name.to_owned(),
            email: email.to_owned(),
        })
    }

    /// UC-012 A3: the name could not be kept: "Your name could not be
    /// kept: {err}", which is returned for the dialog too.
    pub fn identity_failed(&mut self, err: &str) -> String {
        self.status = format!("Your name could not be kept: {err}");
        self.status.clone()
    }

    /// UC-011 A6 step 1, M-19.5: "Discard…" of the change of `path` (a note
    /// that is not in conflict): what the question needs, made without the
    /// session; `None` for anything else. The window sent its edits first,
    /// so the lines lost count the tab's text. A question asked before and
    /// cancelled is replaced.
    pub fn discard_start(&mut self, path: &str) -> Option<DiscardPlan> {
        self.changes.discarding = None;
        let change = self.changes.change(path)?.clone();
        let (id, _) = change.note()?;
        if matches!(change.kind, ChangeKind::Conflict { .. }) {
            return None;
        }
        let open = self.open.as_ref()?;
        let text = open
            .tabs
            .find(&id)
            .map(|index| open.tabs.get(index).mirror.text());
        self.changes.discarding = Some(change.clone());
        Some(DiscardPlan {
            setup: self.changes.setup.clone(),
            root: open.root().to_owned(),
            change,
            text,
        })
    }

    /// UC-011 A6: the question could not be asked; the status says why
    /// ("The changes of {t} could not be discarded: {why}").
    pub fn discard_refused(&mut self, refused: &DiscardRefused) {
        self.changes.discarding = None;
        self.status = format!(
            "The changes of {} could not be discarded: {}",
            refused.title, refused.why
        );
    }

    /// UC-011 A6 step 2, M-19.5: "Discard" of the question about `path`,
    /// once the window sent its edits. The change must still be what was
    /// asked about (review F12, else the status says so); a renamed note
    /// whose old place is taken now is refused. The job sets the index back
    /// and fetches the committed content without the session; then
    /// [`Session::finish_discard`] puts it back, or moves a new note to the
    /// deleted notes. A note in conflict is refused (the status says so).
    pub fn discard_prepare(&mut self, path: &str) -> Option<DiscardJob> {
        let asked = self
            .changes
            .discarding
            .take()
            .filter(|change| change.path == path)?;
        let (id, _) = asked.note()?;
        let title = id.title();
        let now = self.changes.change(path).cloned();
        if now.as_ref() != Some(&asked) {
            self.discarded(
                &title,
                Err("it changed since the question; choose Discard… again".into()),
            );
            return None;
        }
        let root = self.workspace_root()?.to_owned();
        match &asked.kind {
            ChangeKind::Conflict { .. } => {
                self.discarded(&title, Err("it is in conflict; resolve it first".into()));
                None
            }
            ChangeKind::Renamed { from } if root.join(from).exists() => {
                let why = format!("{from} exists");
                self.discarded(&title, Err(why));
                None
            }
            _ => Some(DiscardJob {
                setup: self.changes.setup.clone(),
                root,
                change: asked,
            }),
        }
    }

    /// UC-011 A6 step 2: a new note goes to the deleted notes (UC-007); its
    /// tab closes.
    fn discard_new(&mut self, id: &NoteId) -> Result<(), String> {
        // A save of the tab must not bring the file back afterwards.
        self.settle_saves();
        let open = self.open.as_mut().ok_or_else(String::new)?;
        let root = open.root().to_owned();
        open.workspace
            .delete_note(id, now())
            .map_err(|err| err.to_string())?;
        if let Some(index) = open.tabs.find(id) {
            open.tabs.close(index);
        }
        self.state.note_deleted(&root, id);
        Ok(())
    }

    /// UC-011 A6 steps 2–3: puts back the committed content fetched, byte
    /// for byte: a changed or deleted note gets it, also in its open tab
    /// (which shows it as a reload, `docs/migration/variante-b.md` section
    /// 3.6); a renamed note goes back to its old place and its tab closes; a
    /// new note goes to the deleted notes.
    pub fn finish_discard(&mut self, fetched: DiscardFetched) {
        if self.workspace_root() != Some(fetched.root.as_path()) {
            return;
        }
        let change = fetched.change;
        let Some((id, _)) = change.note() else {
            return;
        };
        let title = id.title();
        let root = fetched.root;
        let result = match fetched.committed {
            Err(why) => Err(why),
            Ok(None) => self.discard_new(&id),
            Ok(Some(content)) => {
                // A save of the tab must not land after the committed content.
                self.settle_saves();
                match &change.kind {
                    ChangeKind::Renamed { from } => write_committed(&root, from, &content)
                        .and_then(|()| {
                            std::fs::remove_file(root.join(&change.path)).map_err(|e| e.to_string())
                        })
                        .map(|()| {
                            if let Some(open) = &mut self.open {
                                if let Some(index) = open.tabs.find(&id) {
                                    open.tabs.close(index);
                                }
                            }
                        }),
                    _ => write_committed(&root, &change.path, &content).map(|()| {
                        let text = String::from_utf8_lossy(&content).into_owned();
                        self.reload_discarded(&id, text);
                    }),
                }
            }
        };
        self.discarded(&title, result);
    }

    /// The open tab of `id` shows the committed `content` again, as stored.
    fn reload_discarded(&mut self, id: &NoteId, content: String) {
        let Some(open) = &mut self.open else {
            return;
        };
        let root = open.root().to_owned();
        let Some(index) = open.tabs.find(id) else {
            return;
        };
        let tab = open.tabs.get_mut(index);
        tab.reload(content);
        tab.autosave = underline_core::Autosave::default();
        tab.saving_text = None;
        tab.error = None;
        tab.deleted = false;
        tab.quiet_save = false;
        tab.read_properties(&root);
        self.changed.push(id.clone());
    }

    /// UC-011 A6 step 3: after discarding (or failing to, maybe half way),
    /// the notes are read again and the status says what happened.
    fn discarded(&mut self, title: &str, result: Result<(), String>) {
        if let Some(open) = &mut self.open {
            open.workspace.rescan();
        }
        self.persist_tabs();
        self.refresh_deleted();
        self.refresh_overview();
        match result {
            Ok(()) => {
                self.changes.chosen = None;
                self.status = format!("Discarded the changes of {title}");
            }
            Err(err) => {
                self.status = format!("The changes of {title} could not be discarded: {err}");
            }
        }
    }
}

#[cfg(test)]
impl Session {
    /// For the tests: git gets `env` from now on (an isolated
    /// configuration), here and in the git jobs.
    pub(super) fn use_git_env(&mut self, env: &[(&str, &std::ffi::OsStr)]) {
        self.git = env
            .iter()
            .fold(underline_core::Git::new(), |git, (key, value)| {
                git.env(key, value)
            });
        self.changes.setup = GitSetup::with_env(
            env.iter()
                .map(|(key, value)| ((*key).into(), (*value).to_owned()))
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests;
