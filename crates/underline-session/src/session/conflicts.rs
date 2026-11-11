//! Feature F13 of the migration: UC-015 Resolve Sync Conflict in the
//! session (inventory M-19.9, M-19.10, M-04.2), with the todos in conflict
//! of UC-020 A10 and A11.
//!
//! Ported from `conflict_bar`, `conflicts`, `in_conflict`,
//! `resolve_active`, `check_resolved`, `after_resolving`, `reload_tabs`,
//! `show_abort_confirm`, the conflict actions of `change_action` and the
//! todo conflicts of `poll_todos` and `todo_actions` of
//! `apps/underline-desktop/src/app.rs`; the texts are egui's. How places
//! are found and replaced, and how git takes a side, completes or aborts,
//! stays in `underline_core::conflict` and `underline_core::todos`.
//!
//! git never runs while the session is held (A-123):
//! - how the stopped sync joins (merge or rebase) and the todos in conflict
//!   are asked on a thread of their own whenever git answered anew while a
//!   merge waits (egui: every frame, and every answer for the todos);
//! - marking notes resolved, taking a side, keeping or deleting, keeping a
//!   todo's version and completing the joining run one after the other on
//!   threads of their own ([`Step`]); the tick takes in what they did;
//! - aborting is prepared under the session, runs without it, and is
//!   finished under it again (`AbortJob`).
//!
//! A choice for a place changes the open note's text as the backend does
//! (`backend_edit`, `docs/migration/variante-b.md` section 3.6: not sealed,
//! like egui's `apply_edits`); the window sent its edits first (`flushed`,
//! ADR-025), so the places are those of the text with every key.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

use ropey::Rope;
use underline_core::{
    conflict_places, now, resolve_place, todo_number_of, ChangeKind, Changes, Choice,
    CompleteError, ConflictPlace, FileChange, Git, Joining, NoteId, Side, Todo, TodoConflict,
};
use underline_editor_core::utf16::WebOffsets;

use super::branches::Pause;
use super::remote::redact;
use super::Session;
use crate::editor_view::units;
use crate::remote_view::{
    ChoiceName, ConflictBarView, ConflictPlaceView, SideName, TodoConflictRowView, TodoConflictView,
};
use crate::Event;

impl From<ChoiceName> for Choice {
    fn from(choice: ChoiceName) -> Self {
        match choice {
            ChoiceName::Here => Self::Here,
            ChoiceName::Remote => Self::Remote,
            ChoiceName::Both => Self::Both,
        }
    }
}

impl From<SideName> for Side {
    fn from(side: SideName) -> Self {
        match side {
            SideName::Here => Self::Here,
            SideName::Remote => Self::Remote,
        }
    }
}

/// The todos in conflict, and the numbers given anew as (old, new).
type TodoAnswer = Result<(Vec<TodoConflict>, Vec<(u64, u64)>), String>;

/// What the thread found out about a stopped sync.
#[derive(Debug)]
struct Probe {
    root: PathBuf,
    /// git's answer it was asked for.
    generation: u64,
    joining: Option<Joining>,
    /// UC-020 A10, A11: `None` when no todo is in conflict.
    todos: Option<TodoAnswer>,
}

/// One step of resolving, run without the session, then the joining is
/// completed once nothing is in conflict any more (UC-015 step 7).
#[derive(Debug)]
pub(super) enum Step {
    /// Step 6: the notes saved without open places are marked resolved.
    Mark(Vec<String>),
    /// A2: a file that is not a note keeps a side; the other is copied.
    TakeSide { path: String, side: Side },
    /// A3: a note or file kept, or deleted before (`removed`, the note it
    /// was), is marked resolved.
    Keep {
        path: String,
        removed: bool,
        note: Option<NoteId>,
    },
    /// UC-020 A10: a todo keeps a version (`None`: stays deleted).
    Todo { number: u64, version: Option<Todo> },
    /// UC-020 A11: only the completion, after todos were numbered anew.
    Complete,
}

/// What a [`Step`] did.
#[derive(Debug)]
pub(super) struct Resolved {
    root: PathBuf,
    step: Step,
    /// The step's result: the copy kept by [`Step::TakeSide`], or why it
    /// failed.
    pub(super) result: Result<Option<String>, String>,
    /// Whether the joining was completed, when nothing was in conflict.
    completed: Option<Result<(), CompleteError>>,
    /// git's newest answer about the changes.
    changes: Option<Changes>,
}

/// Runs `step` and, when nothing is in conflict any more, completes the
/// joining (egui's `after_resolving`).
pub(super) fn resolve(git: &Git, root: PathBuf, step: Step) -> Resolved {
    let joining = git.joining(&root).unwrap_or(Joining::Merge);
    let result = match &step {
        Step::Mark(paths) => {
            // BR-001 (A-145): only a file that holds no place any more is
            // marked, so a tab that has not shown git's places yet never
            // marks them resolved. egui: resolved when one could be marked.
            let marked = paths
                .iter()
                .filter(|path| {
                    std::fs::read_to_string(root.join(path)).is_ok_and(|text| !has_markers(&text))
                })
                .filter(|path| git.mark_resolved(&root, path).is_ok())
                .count();
            if marked > 0 {
                Ok(None)
            } else {
                Err(String::new())
            }
        }
        Step::TakeSide { path, side } => git
            .take_side(&root, path, *side, joining)
            .map(Some)
            .map_err(|e| e.to_string()),
        Step::Keep { path, .. } => git
            .mark_resolved(&root, path)
            .map(|()| None)
            .map_err(|e| e.to_string()),
        Step::Todo { number, version } => git
            .resolve_todo(&root, *number, version.as_ref())
            .map(|()| None)
            .map_err(|e| e.to_string()),
        Step::Complete => Ok(None),
    };
    if result.is_err() {
        return Resolved {
            root,
            step,
            result,
            completed: None,
            changes: None,
        };
    }
    let answer = git.changes(&root);
    let open = answer.as_ref().map_or(1, |c| conflicts_of(c).count());
    if open > 0 {
        return Resolved {
            root,
            step,
            result,
            completed: None,
            changes: answer.ok(),
        };
    }
    let completed = git.complete_joining(&root);
    let changes = git.changes(&root).ok().or(answer.ok());
    Resolved {
        root,
        step,
        result,
        completed: Some(completed),
        changes,
    }
}

/// UC-015 BR-001 (A-145): whether `text` still holds a place git marked,
/// or a line that starts like one of git's markers (stricter than egui's
/// `conflict_places`, which needs the whole block).
pub(super) fn has_markers(text: &str) -> bool {
    !conflict_places(text, Joining::Merge).is_empty()
        || text.lines().any(|l| {
            ["<<<<<<<", "=======", ">>>>>>>"]
                .iter()
                .any(|m| l.starts_with(m))
        })
}

/// The notes and files in conflict (UC-015 BR-005).
fn conflicts_of(changes: &Changes) -> impl Iterator<Item = &FileChange> {
    changes
        .files
        .iter()
        .filter(|f| matches!(f.kind, ChangeKind::Conflict { .. }))
}

/// UC-015: the session's part of the conflicts of the open workspace.
#[derive(Default)]
pub(super) struct Conflicts {
    /// The workspace whose conflicts these are.
    root: Option<PathBuf>,
    /// How the stopped sync joins, as the thread found out.
    joining: Option<Joining>,
    /// git's answer the thread was last asked about.
    probed: Option<u64>,
    /// The thread that asks, while it runs.
    probe: Option<Receiver<Probe>>,
    /// UC-020 A10: the todos in conflict.
    todos: Vec<TodoConflict>,
    /// The steps waiting for the one that runs.
    queue: VecDeque<Step>,
    /// The step that runs.
    running: Option<Receiver<Resolved>>,
    /// Step 6: the tabs looked at, by the revision of their text and git's
    /// answer, so a note that could not be marked is tried again only
    /// after either changed.
    checked: HashMap<NoteId, (u32, u64)>,
    /// The bar last made: of which note, revision, answer and joining.
    bar: Option<(NoteId, u32, u64, Joining, Option<ConflictBarView>)>,
    /// What the window was told last.
    told_bar: Option<Option<ConflictBarView>>,
    told_todos: Option<Vec<TodoConflictView>>,
}

impl std::fmt::Debug for Conflicts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conflicts")
            .field("joining", &self.joining)
            .field("todos", &self.todos.len())
            .field("queued", &self.queue.len())
            .field("running", &self.running.is_some())
            .finish_non_exhaustive()
    }
}

/// M-19.10, UC-020 A10: the values of a todo's six fields, as egui shows
/// them.
fn todo_fields(todo: Option<&Todo>) -> [String; 6] {
    let Some(t) = todo else {
        return std::array::from_fn(|_| "(deleted)".to_owned());
    };
    let none = || "–".to_owned();
    [
        t.title.clone(),
        t.due.map_or_else(none, |d| d.to_string()),
        t.scheduled.map_or_else(none, |d| d.to_string()),
        t.urgency.label().to_owned(),
        if t.is_done() { "done" } else { "open" }.to_owned(),
        if t.links.is_empty() {
            none()
        } else {
            t.links.join(", ")
        },
    ]
}

/// M-19.10: a todo in conflict, both versions field by field.
fn todo_view(conflict: &TodoConflict) -> TodoConflictView {
    let here = todo_fields(conflict.mine.as_ref());
    let remote = todo_fields(conflict.remote.as_ref());
    let names = ["Title", "Due", "Scheduled", "Urgency", "Status", "Notes"];
    TodoConflictView {
        number: u32::try_from(conflict.number).unwrap_or(u32::MAX),
        heading: format!(
            "Todo #{} was changed here and on the remote",
            conflict.number
        ),
        rows: names
            .iter()
            .zip(here.into_iter().zip(remote))
            .map(|(name, (here, remote))| TodoConflictRowView {
                name: (*name).to_owned(),
                differ: here != remote,
                here,
                remote,
            })
            .collect(),
    }
}

/// M-19.9, UC-015 step 3: the places of `rope`'s text, for the window.
fn places_view(rope: &Rope, places: &[ConflictPlace]) -> Vec<ConflictPlaceView> {
    let web = WebOffsets::of(rope);
    places
        .iter()
        .map(|place| {
            let start = place.range.start.min(rope.len_bytes());
            let at = rope.byte_to_char(start);
            ConflictPlaceView {
                line: units(rope.byte_to_line(start) + 1),
                offset: units(web.to_web(rope, at)),
                here: place.here.trim_end().to_owned(),
                remote: place.remote.trim_end().to_owned(),
            }
        })
        .collect()
}

/// The prepared abort of a stopped sync (UC-015 A5), run without the
/// session.
#[derive(Debug)]
pub struct AbortJob {
    git: Git,
    root: PathBuf,
}

/// What an abort gave.
#[derive(Debug)]
pub struct AbortOutcome {
    root: PathBuf,
    result: Result<(), String>,
}

impl AbortJob {
    /// UC-015 A5 step 2, BR-008: the workspace as it was right before the
    /// sync joined; local commits and changes left out stay.
    #[must_use]
    pub fn run(self) -> AbortOutcome {
        AbortOutcome {
            result: self
                .git
                .abort_joining(&self.root)
                .map_err(|e| e.to_string()),
            root: self.root,
        }
    }
}

impl Session {
    /// UC-015: a workspace was opened; the conflicts of the one left are
    /// forgotten (what was told stays, so the window hears they are gone).
    pub(super) fn track_conflicts(&mut self, root: &std::path::Path) {
        let told_bar = self.conflicts.told_bar.take();
        let told_todos = self.conflicts.told_todos.take();
        self.conflicts = Conflicts {
            root: Some(root.to_owned()),
            told_bar,
            told_todos,
            ..Conflicts::default()
        };
    }

    /// UC-015 BR-005: the notes and files in conflict, as git answered
    /// last.
    fn conflict_files(&self) -> Vec<FileChange> {
        self.git_changes()
            .map(|c| conflicts_of(c).cloned().collect())
            .unwrap_or_default()
    }

    /// For the tests: `step` waits to run.
    #[cfg(test)]
    pub(super) fn queue_step(&mut self, step: Step) {
        self.conflicts.queue.push_back(step);
    }

    /// Whether a step of resolving runs or waits.
    pub(super) fn resolving_conflicts(&self) -> bool {
        self.conflicts.running.is_some() || !self.conflicts.queue.is_empty()
    }

    /// M-04.2, UC-015 step 1: how many conflicts wait.
    pub(super) fn conflict_count(&self) -> usize {
        self.git_changes().map_or(0, |c| conflicts_of(c).count())
    }

    /// UC-015 BR-007: whether the note `id` is in conflict; it is then not
    /// renamed, deleted, exported or discarded.
    pub(super) fn in_conflict(&self, id: &NoteId) -> bool {
        self.git_changes()
            .is_some_and(|c| conflicts_of(c).any(|f| f.note().is_some_and(|(note, _)| &note == id)))
    }

    /// UC-015: the tick's part: how the stopped sync joins and the todos in
    /// conflict, what the steps of resolving did, the next step, and the
    /// notes whose places are all gone (step 6).
    pub(super) fn run_conflicts(&mut self) {
        self.take_probe();
        self.start_probe();
        self.take_resolved();
        self.check_resolved();
        self.next_step();
    }

    /// Asks how the stopped sync joins, and for the todos in conflict, when
    /// git answered anew while a merge waits (egui: every answer).
    fn start_probe(&mut self) {
        let generation = self.git_generation();
        if self.conflicts.probe.is_some() || self.conflicts.probed == Some(generation) {
            return;
        }
        let Some(changes) = self.git_changes() else {
            return;
        };
        let conflicted: Vec<String> = conflicts_of(changes).map(|f| f.path.clone()).collect();
        if !changes.merging && conflicted.is_empty() {
            self.conflicts.probed = Some(generation);
            self.conflicts.joining = None;
            self.conflicts.todos.clear();
            return;
        }
        let Some(root) = self.workspace_root().map(std::path::Path::to_owned) else {
            return;
        };
        let todo_paths: Vec<String> = conflicted
            .into_iter()
            .filter(|p| todo_number_of(p).is_some())
            .collect();
        let git = self.git_setup().git();
        let (send, answer) = mpsc::channel();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("underline-conflicts".into())
            .spawn(move || {
                let joining = git.joining(&root);
                let todos = (!todo_paths.is_empty()).then(|| {
                    git.todo_conflicts(&root, &todo_paths, joining.unwrap_or(Joining::Merge))
                        .map_err(|e| e.to_string())
                });
                let _ = send.send(Probe {
                    root,
                    generation,
                    joining,
                    todos,
                });
                wake.wake();
            });
        match spawned {
            Ok(_) => self.conflicts.probe = Some(answer),
            Err(error) => eprintln!("UC-015: the conflicts could not be looked at: {error}"),
        }
    }

    /// What the thread found: how the sync joins, and the todos in
    /// conflict; todos created on both sides were numbered anew (UC-020
    /// A11), which the status says, and the joining may complete.
    fn take_probe(&mut self) {
        let Some(answer) = &self.conflicts.probe else {
            return;
        };
        let probe = match answer.try_recv() {
            Ok(probe) => probe,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.conflicts.probe = None;
                return;
            }
        };
        self.conflicts.probe = None;
        if self.conflicts.root.as_ref() != Some(&probe.root) {
            return;
        }
        self.conflicts.probed = Some(probe.generation);
        self.conflicts.joining = probe.joining;
        match probe.todos {
            None => self.conflicts.todos.clear(),
            Some(Ok((conflicts, renumbered))) => {
                self.conflicts.todos = conflicts;
                if !renumbered.is_empty() {
                    // A11: the todos created here got new numbers.
                    let moved: Vec<String> = renumbered
                        .iter()
                        .map(|(old, new)| format!("Todo #{old} is now #{new}"))
                        .collect();
                    self.status = moved.join("; ");
                    self.conflicts.queue.push_back(Step::Complete);
                }
            }
            Some(Err(err)) => {
                self.status = format!("The todos in conflict cannot be read: {err}");
            }
        }
    }

    /// Starts the next step of resolving, when none runs.
    fn next_step(&mut self) {
        // S1 (review F13): one git job on the notes at a time.
        // Round 2: only a sync git runs holds a step back; one waiting runs
        // after the steps.
        if self.conflicts.running.is_some()
            || self.sync_in_flight()
            || self.saves_paused.contains(&Pause::Switch)
            || self.saves_paused.contains(&Pause::Abort)
        {
            return;
        }
        let Some(step) = self.conflicts.queue.pop_front() else {
            return;
        };
        let Some(root) = self.workspace_root().map(std::path::Path::to_owned) else {
            self.conflicts.queue.clear();
            return;
        };
        let git = self.git_setup().git();
        let (send, answer) = mpsc::channel();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("underline-resolve".into())
            .spawn(move || {
                let resolved = resolve(&git, root, step);
                let _ = send.send(resolved);
                wake.wake();
            });
        match spawned {
            Ok(_) => {
                self.conflicts.running = Some(answer);
                self.pause_saves(Pause::Resolve);
            }
            Err(error) => {
                eprintln!("UC-015: resolving could not start: {error}");
                self.status = format!("The conflicts could not be resolved: {error}");
            }
        }
    }

    /// Takes in what a step did (egui's `change_action`, `todo_actions`
    /// and `after_resolving`).
    fn take_resolved(&mut self) {
        let Some(answer) = &self.conflicts.running else {
            return;
        };
        let resolved = match answer.try_recv() {
            Ok(resolved) => resolved,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.conflicts.running = None;
                self.resume_saves(Pause::Resolve);
                eprintln!("UC-015: a step of resolving stopped unexpectedly");
                return;
            }
        };
        self.conflicts.running = None;
        self.resume_saves(Pause::Resolve);
        if self.workspace_root() != Some(resolved.root.as_path()) {
            return;
        }
        self.step_done(&resolved.step, resolved.result);
        if let Some(changes) = resolved.changes {
            self.take_git_changes(changes);
        }
        self.refresh_changes();
        match resolved.completed {
            None => {}
            Some(Ok(())) => {
                self.git_report("The conflicts are resolved; syncing".into());
                // The joining is complete: the sync goes on (step 7).
                self.reload_tabs();
                self.refresh_overview();
                self.request_sync(underline_core::SyncMode::Full, false);
            }
            // A6: the next local commit conflicts too.
            Some(Err(CompleteError::MoreConflicts(files))) => {
                self.git_report(format!(
                    "The next commit conflicts too: {}",
                    files.join(", ")
                ));
                self.reload_tabs();
                self.refresh_overview();
            }
            // A7.
            Some(Err(CompleteError::Git(err))) => {
                self.status = format!("Completing the sync failed: {}", redact(&err.to_string()));
            }
        }
    }

    /// What the step's result says in the status, and what follows it.
    fn step_done(&mut self, step: &Step, result: Result<Option<String>, String>) {
        match (step, result) {
            (Step::TakeSide { path, .. }, Ok(copy)) => {
                self.git_report(format!(
                    "Kept one version of {path}; the other is {}",
                    copy.unwrap_or_default()
                ));
            }
            (Step::TakeSide { path, .. } | Step::Keep { path, .. }, Err(err)) => {
                self.status = format!("{path} could not be resolved: {}", redact(&err));
            }
            (Step::Keep { removed, note, .. }, Ok(_)) => {
                if *removed {
                    if let (Some(id), Some(root)) =
                        (note, self.workspace_root().map(std::path::Path::to_owned))
                    {
                        self.state.note_deleted(&root, id);
                    }
                    self.refresh_deleted();
                }
            }
            (Step::Todo { number, .. }, Ok(_)) => {
                self.conflicts.todos.retain(|c| c.number != *number);
            }
            (Step::Todo { number, .. }, Err(err)) => {
                self.status = format!("Todo #{number} could not be resolved: {}", redact(&err));
            }
            (Step::Mark(_) | Step::Complete, _) => {}
        }
    }

    /// UC-015 step 6: an open note in conflict whose text holds no place
    /// any more (chosen, or edited away by hand) is stored and marked
    /// resolved (egui's `check_resolved`, every frame while conflicts
    /// wait).
    fn check_resolved(&mut self) {
        if self.conflicts.running.is_some() || !self.conflicts.queue.is_empty() {
            return;
        }
        let generation = self.git_generation();
        let notes: Vec<(NoteId, String)> = self
            .conflict_files()
            .into_iter()
            .filter(|c| matches!(c.kind, ChangeKind::Conflict { deleted: None }))
            .filter_map(|c| c.note().map(|(id, _)| (id, c.path)))
            .collect();
        if notes.is_empty() {
            self.conflicts.checked.clear();
            return;
        }
        let mut done = Vec::new();
        let mut stale = Vec::new();
        for (id, path) in notes {
            let Some(open) = &self.open else {
                return;
            };
            let Some(index) = open.tabs.find(&id) else {
                continue;
            };
            let tab = open.tabs.get(index);
            let key = (tab.mirror.rev(), generation);
            if self.conflicts.checked.get(&id) == Some(&key) {
                continue;
            }
            self.conflicts.checked.insert(id.clone(), key);
            // The side does not matter for whether a place is left.
            if has_markers(&tab.mirror.text()) {
                continue;
            }
            // S5 (review F13): a tab that holds no change of its own but
            // shows no place while its file has them shows an older text;
            // it takes the file's first and is never stored over it.
            let file_marked = std::fs::read_to_string(&tab.path).is_ok_and(|t| has_markers(&t));
            if file_marked && !tab.autosave.is_unstored() {
                stale.push(tab.path.clone());
                continue;
            }
            done.push((id, path));
        }
        for path in stale {
            self.take_file_version(&path, false);
        }
        if done.is_empty() {
            return;
        }
        // A background save of the note must not land after this one.
        self.settle_saves();
        let mut paths = Vec::new();
        let Some(open) = &mut self.open else {
            return;
        };
        let root = open.root().to_owned();
        for (id, path) in done {
            let Some(index) = open.tabs.find(&id) else {
                continue;
            };
            let tab = open.tabs.get_mut(index);
            let unstored = tab.autosave.is_unstored();
            if tab.flush(&self.saver).is_ok() {
                paths.push(path);
            }
            if unstored {
                tab.read_properties(&root);
                self.views_dirty = true;
            }
        }
        if !paths.is_empty() {
            self.conflicts.queue.push_back(Step::Mark(paths));
        }
    }

    /// UC-015 steps 4–5, A1, M-19.9: the place `place` of the open note
    /// `note` (every open place with `None`, "All: Here"/"All: Remote") is
    /// replaced by `choice`, as the backend changes an open note; the note
    /// is stored and marked resolved by the tick once no place is left
    /// (step 6). The places are those of the backend's text, which holds
    /// every key (`flushed`).
    ///
    /// `rev` is the revision of the bar the choice was made in; when the
    /// text changed since (a key, a choice made twice), nothing is replaced
    /// (S3, review F13): the bar comes anew.
    pub fn conflict_choose(
        &mut self,
        note: &NoteId,
        rev: u32,
        place: Option<usize>,
        choice: ChoiceName,
    ) {
        if !self.in_conflict(note) {
            return;
        }
        let joining = self.conflicts.joining.unwrap_or(Joining::Merge);
        let Some(text) = self.open.as_ref().and_then(|open| {
            open.tabs.find(note).and_then(|index| {
                let tab = open.tabs.get(index);
                (tab.mirror.rev() == rev).then(|| tab.mirror.text())
            })
        }) else {
            return;
        };
        let places = conflict_places(&text, joining);
        let chosen: Vec<ConflictPlace> = match place {
            Some(index) => places.get(index).cloned().into_iter().collect(),
            None => places,
        };
        let choice = Choice::from(choice);
        let edits: Vec<(std::ops::Range<usize>, String)> = chosen
            .iter()
            .map(|p| {
                let resolved = resolve_place(
                    &text[p.range.clone()],
                    &ConflictPlace {
                        range: 0..p.range.len(),
                        here: p.here.clone(),
                        remote: p.remote.clone(),
                    },
                    choice,
                );
                (p.range.clone(), resolved)
            })
            .collect();
        if edits.is_empty() {
            return;
        }
        if let Err(error) = self.backend_edit(note, &edits, false) {
            eprintln!("UC-015: the choice could not be made: {}", error.message);
        }
    }

    /// UC-015 A2, M-19.2: "Keep here" or "Keep remote" of a file in
    /// conflict that is not a note: the chosen version stays, the other is
    /// kept next to it as a copy; the status says where.
    pub fn conflict_take_side(&mut self, path: &str, side: SideName) {
        let fits = self.conflict_files().iter().any(|c| {
            c.path == path
                && c.note().is_none()
                && matches!(c.kind, ChangeKind::Conflict { deleted: None })
        });
        if fits {
            self.conflicts.queue.push_back(Step::TakeSide {
                path: path.to_owned(),
                side: side.into(),
            });
        }
    }

    /// UC-015 A3, M-19.2: "Keep" or "Delete" of a note or file deleted on
    /// one side and changed on the other. "Delete" moves a note to the
    /// deleted notes (UC-007) and closes its tab, another file goes; then
    /// it is marked resolved.
    pub fn conflict_keep(&mut self, path: &str, keep: bool) {
        let Some(change) = self.conflict_files().into_iter().find(|c| {
            c.path == path && matches!(c.kind, ChangeKind::Conflict { deleted: Some(_) })
        }) else {
            return;
        };
        let note = change.note().map(|(id, _)| id);
        if !keep {
            if let Err(err) = self.remove_conflicted(&change, note.as_ref()) {
                self.status = format!("{path} could not be resolved: {err}");
                return;
            }
        }
        self.conflicts.queue.push_back(Step::Keep {
            path: path.to_owned(),
            removed: !keep,
            note,
        });
    }

    /// A3 "Delete": the note goes to the deleted notes and its tab closes
    /// (so no save brings it back), another file is removed.
    fn remove_conflicted(
        &mut self,
        change: &FileChange,
        note: Option<&NoteId>,
    ) -> Result<(), String> {
        self.settle_saves();
        let open = self.open.as_mut().ok_or_else(String::new)?;
        let root = open.root().to_owned();
        let file = root.join(&change.path);
        match note {
            Some(id) if file.exists() => {
                // S9 (review F13): the deleted note holds the latest text.
                if let Some(index) = open.tabs.find(id) {
                    open.tabs.get_mut(index).flush(&self.saver)?;
                }
                open.workspace
                    .delete_note(id, now())
                    .map_err(|e| e.to_string())?;
                if let Some(index) = open.tabs.find(id) {
                    open.tabs.close(index);
                }
                self.persist_tabs();
                self.views_dirty = true;
                Ok(())
            }
            _ => match std::fs::remove_file(&file) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.to_string()),
                _ => Ok(()),
            },
        }
    }

    /// M-19.10, UC-020 A10 step 3: "Keep mine" or "Take remote" of the todo
    /// `number`; then it is marked resolved.
    pub fn todo_conflict_resolve(&mut self, number: u32, mine: bool) {
        let number = u64::from(number);
        let Some(conflict) = self.conflicts.todos.iter().find(|c| c.number == number) else {
            return;
        };
        let version = if mine {
            conflict.mine.clone()
        } else {
            conflict.remote.clone()
        };
        self.conflicts
            .queue
            .push_back(Step::Todo { number, version });
    }

    /// UC-015 A5 step 2, M-19.10: "Abort the sync" confirmed, once the
    /// window sent its edits: prepared here, run without the session;
    /// `None` when no sync waits for its conflicts. No note is stored in
    /// the background meanwhile.
    pub fn abort_prepare(&mut self) -> Option<AbortJob> {
        if !self.git_changes().is_some_and(|c| c.merging) {
            return None;
        }
        // S1 (review F13): not while a sync or a step of resolving runs.
        if self.sync_running() || self.resolving_conflicts() {
            self.status = "The sync could not be aborted: git is still working on it".into();
            return None;
        }
        let root = self.workspace_root()?.to_owned();
        self.pause_saves(Pause::Abort);
        Some(AbortJob {
            git: self.git_setup().git(),
            root,
        })
    }

    /// UC-015 A5 step 3: the tabs show the notes as before the sync and the
    /// status says "The sync was aborted; the workspace is as before it",
    /// or why it could not be.
    pub fn finish_abort(&mut self, outcome: AbortOutcome) {
        self.resume_saves(Pause::Abort);
        if self.workspace_root() != Some(outcome.root.as_path()) {
            return;
        }
        match outcome.result {
            Ok(()) => {
                self.reload_tabs();
                self.refresh_overview();
                // App-tester F13: the stopped sync's state is no news any more.
                self.forget_stopped_sync();
                self.git_report("The sync was aborted; the workspace is as before it".into());
            }
            Err(err) => {
                self.status = format!("The sync could not be aborted: {}", redact(&err));
            }
        }
        self.refresh_changes();
    }

    /// M-19.9, M-19.10: the conflict bar of the active note and the todos
    /// in conflict, when they changed.
    pub(super) fn conflict_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        let bar = self.conflict_bar();
        // No bar and no todo in conflict were told from the start.
        if self.conflicts.told_bar.clone().unwrap_or_default() != bar {
            self.conflicts.told_bar = Some(bar.clone());
            events.push(Event::Conflict(bar));
        }
        let todos: Vec<TodoConflictView> = self.conflicts.todos.iter().map(todo_view).collect();
        if self.conflicts.told_todos.clone().unwrap_or_default() != todos {
            self.conflicts.told_todos = Some(todos.clone());
            events.push(Event::TodoConflicts(todos));
        }
        events
    }

    /// UC-015 steps 3, 5, BR-006: the open places of the active note while
    /// it is in conflict and the sync's joining is known; made again only
    /// when its text, git's answer or the joining changed.
    fn conflict_bar(&mut self) -> Option<ConflictBarView> {
        let joining = self.conflicts.joining?;
        let generation = self.git_generation();
        let (id, rev) = {
            let open = self.open.as_ref()?;
            let (id, tab) = open.tabs.active()?;
            (id.clone(), tab.mirror.rev())
        };
        if !self.in_conflict(&id) {
            return None;
        }
        if let Some((note, r, g, j, view)) = &self.conflicts.bar {
            if *note == id && *r == rev && *g == generation && *j == joining {
                return view.clone();
            }
        }
        let open = self.open.as_ref()?;
        let tab = open.tabs.get(open.tabs.find(&id)?);
        let rope = tab.mirror.rope();
        let places = conflict_places(&rope.to_string(), joining);
        let view = (!places.is_empty()).then(|| ConflictBarView {
            note: id.as_str().to_owned(),
            rev,
            places: places_view(rope, &places),
        });
        self.conflicts.bar = Some((id, rev, generation, joining, view.clone()));
        view
    }
}

#[cfg(test)]
impl Session {
    /// For the tests: whether a step of resolving runs or waits, or the
    /// joining is still asked about.
    pub(super) fn resolving(&self) -> bool {
        self.conflicts.running.is_some()
            || !self.conflicts.queue.is_empty()
            || self.conflicts.probe.is_some()
    }
}
