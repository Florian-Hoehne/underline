//! Feature F15 of the migration: UC-020 Manage Todos and UC-021 Work
//! Through Todo List in the session (inventory M-04.5, M-04.6, M-06.5,
//! M-16.3, M-18).
//!
//! Ported from `TodosView`, `todos_ui`, `todo_table`, `poll_todos`,
//! `todo_actions`, `undo_todo_delete`, `open_todo_list` and the todo parts
//! of the status bar and the tab bar of `apps/underline-desktop`; the texts
//! are egui's. The todos themselves (fields, numbers, files, sort, filter,
//! overdue) stay in `underline_core::todos` (ADR-019, ADR-005).
//!
//! No todo file is read or written while the session is held (like A-123),
//! except for the last attempt before another workspace opens or Underline
//! ends ([`Session::todos_settle`]):
//! - a change is made to the todo as shown under the session, which shows
//!   it at once (`pending`), then written by a [`TodoJob`] without the
//!   session, which reads the todos again after it; the session takes in
//!   what it did ([`Session::todo_finish`]);
//! - a thread of the workspace tries again every second what could not be
//!   stored (UC-020 A9) and reads the todos again when their files changed
//!   (A7, by their fingerprint), as egui's `poll_todos`; it stops with its
//!   workspace;
//! - every write goes through [`disk::Disk`]: one after the other, never
//!   an older change over a newer one, and never over a file that changed
//!   elsewhere since it was read (review F15, A-188); what was read before
//!   a change was made is not taken in after it.
//!
//! The todos in conflict after a sync (A10, A11) are F13's
//! (`conflicts.rs`); the window shows them in the section "Todos".

mod disk;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use underline_core::{
    links_summary, load_todos, overdue_count, overdue_days, todo_list, todos_fingerprint, Day,
    NoteId, Todo, TodoError, TodoFilter, TodoListState, Todos,
};

use self::disk::{Disk, Saved};
use super::{Session, Wake};
use crate::todos_view::{
    CalendarDayView, CalendarView, TodoActiveNoteView, TodoAnswer, TodoCandidateView,
    TodoCandidatesView, TodoChange, TodoFilterName, TodoLinkView, TodoSortName, TodoTableView,
    TodoView, TodosView,
};
use crate::{Event, StatusOffer};

/// UC-020 A7, A9: how often the todos are looked at (egui: every second).
pub const TODOS_EVERY: Duration = Duration::from_secs(1);

/// M-18.4, UC-020 A1: how many notes "Link a note…" offers.
const CANDIDATES: usize = 8;

/// The todo files when last read: name, time of change and size.
type Fingerprint = Vec<(String, Option<SystemTime>, u64)>;

/// The todos as read, with the fingerprint of their files taken before.
fn read(root: &Path) -> (Fingerprint, Todos) {
    let fingerprint = todos_fingerprint(root);
    (fingerprint, load_todos(root))
}

/// What a [`TodoJob`] does.
#[derive(Debug, Clone)]
enum Work {
    /// UC-020 steps 3–4: creates a todo with this title.
    Create(String),
    /// Steps 5–10, A1, A2, A4: stores the todo changed in the section,
    /// made from `base` (as read; `None` without a file).
    Change { todo: Todo, base: Option<Todo> },
    /// UC-021 step 7: stores the todo marked in the list; `before` is
    /// what "Undo" brings back.
    Mark {
        before: Todo,
        todo: Todo,
        base: Option<Todo>,
    },
    /// A5: deletes the todo.
    Delete(u64),
    /// A5 step 2: the deleted todo comes back, as a new file.
    UndoDelete(Todo),
    /// UC-021 step 8: the marked todo is as before, if it is still as
    /// marked.
    UndoMark { before: Todo, marked: Todo },
}

/// What a [`TodoJob`] did.
#[derive(Debug)]
enum Did {
    Saved(Saved),
    /// The todo created or deleted.
    Todo(Todo),
}

/// A todo written or deleted without the session (`todo_create`,
/// `todo_change`, `todo_mark`, `todo_delete`, `todo_undo`).
#[derive(Debug)]
pub struct TodoJob {
    root: PathBuf,
    seq: u64,
    disk: Disk,
    work: Work,
}

/// What a [`TodoJob`] did, with the todos as read after it.
#[derive(Debug)]
pub struct TodoDone {
    root: PathBuf,
    seq: u64,
    work: Work,
    result: Result<Did, TodoError>,
    read: (Fingerprint, Todos),
}

impl TodoJob {
    /// Writes the todo files (ADR-005: atomically, ADR-019: one file per
    /// todo), then reads the todos again.
    #[must_use]
    pub fn run(self) -> TodoDone {
        let Self {
            root,
            seq,
            disk,
            work,
        } = self;
        let result = match &work {
            Work::Create(title) => disk.create(&root, title, seq).map(Did::Todo),
            Work::Change { todo, base } | Work::Mark { todo, base, .. } => {
                disk.save(&root, todo, base.as_ref(), seq).map(Did::Saved)
            }
            // Review F15: only while it is still as marked, also not over a
            // later change of the user.
            Work::UndoMark { before, marked } => disk
                .save_over(&root, before, Some(marked), seq, true)
                .map(Did::Saved),
            Work::UndoDelete(todo) => disk.restore(&root, todo, seq).map(Did::Saved),
            Work::Delete(number) => disk.delete(&root, *number, seq).map(Did::Todo),
        };
        let read = read(&root);
        TodoDone {
            root,
            seq,
            work,
            result,
            read,
        }
    }
}

/// What the thread of the workspace is asked every second.
#[derive(Debug)]
struct PollAsk {
    seq: u64,
    /// A9: the changes not stored, with what each was made from.
    unstored: Vec<(Todo, Option<Todo>)>,
    /// The fingerprint read last; `None` before the first read.
    before: Option<Fingerprint>,
}

/// What the thread found.
#[derive(Debug)]
struct Polled {
    seq: u64,
    /// A9: the todos that could be stored now.
    stored: Vec<Todo>,
    /// The todos that still cannot be, with the reason.
    refused: Vec<(u64, String)>,
    /// The fingerprint now, and the todos read when it changed.
    fingerprint: Fingerprint,
    todos: Option<Todos>,
}

/// The thread that looks at the todos of one workspace (M-18.9); it stops
/// with its workspace and never writes after that.
struct Poller {
    ask: Sender<PollAsk>,
    answer: Receiver<Polled>,
    stop: Arc<AtomicBool>,
    /// A look was asked for and not answered yet.
    waiting: bool,
}

impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Poller {
    fn start(root: PathBuf, disk: Disk, wake: Wake) -> Option<Self> {
        let (ask, asks) = mpsc::channel::<PollAsk>();
        let (send, answer) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name("underline-todos".into())
            .spawn(move || {
                while let Ok(ask) = asks.recv() {
                    let mut stored = Vec::new();
                    let mut refused = Vec::new();
                    for (todo, base) in ask.unstored {
                        if stopped.load(Ordering::SeqCst) {
                            return;
                        }
                        match disk.save(&root, &todo, base.as_ref(), ask.seq) {
                            Ok(Saved::Written) => stored.push(todo),
                            Ok(Saved::Refused(why)) => refused.push((todo.number, why)),
                            Err(err) => refused.push((todo.number, err.to_string())),
                            Ok(Saved::Skipped) => {}
                        }
                    }
                    if stopped.load(Ordering::SeqCst) {
                        return;
                    }
                    let fingerprint = todos_fingerprint(&root);
                    let todos =
                        (ask.before.as_ref() != Some(&fingerprint)).then(|| load_todos(&root));
                    let answered = send.send(Polled {
                        seq: ask.seq,
                        stored,
                        refused,
                        fingerprint,
                        todos,
                    });
                    if answered.is_err() {
                        return;
                    }
                    wake.wake();
                }
            });
        match spawned {
            Ok(_) => Some(Self {
                ask,
                answer,
                stop,
                waiting: false,
            }),
            Err(error) => {
                eprintln!("UC-020: the todos cannot be looked at: {error}");
                None
            }
        }
    }
}

/// What made the view: the todos' revision, today, the active note, the
/// tab "Todos" and the notes of the workspace.
type Built = (u64, Day, Option<NoteId>, TodoListState, u64);

/// A change being written: its number, the todo as changed (shown
/// meanwhile) and what it was made from.
#[derive(Debug, Clone)]
struct Pending {
    seq: u64,
    todo: Todo,
    base: Option<Todo>,
}

/// A9: a change that could not be stored, why, and what it was made from.
#[derive(Debug, Clone)]
struct Unstored {
    todo: Todo,
    why: String,
    base: Option<Todo>,
}

/// UC-020, UC-021: the session's part of the todos of the open workspace.
#[derive(Default)]
pub(super) struct TodoState {
    /// The workspace whose todos these are.
    root: Option<PathBuf>,
    /// The todos as read last; `None` before they were read.
    todos: Option<Todos>,
    fingerprint: Fingerprint,
    pending: HashMap<u64, Pending>,
    unstored: HashMap<u64, Unstored>,
    /// A5: the last deleted todo and the report that offers to undo.
    deleted: Option<(Todo, String)>,
    /// UC-021 step 8: the todo before it was marked in the list, and the
    /// report that offers to undo; `marked_to` is the todo as marked.
    marked: Option<(Todo, String)>,
    marked_to: Option<Todo>,
    /// Review F15: the todos this window deleted; a change of one that
    /// ends after the deletion is not kept as not stored.
    deleted_here: std::collections::HashSet<u64>,
    /// Review F15: the workspace to open once the user gave up the todos
    /// that cannot be stored ("Close anyway").
    pub(super) switch_waiting: Option<super::workspace::SwitchWaiting>,
    /// Review F15: "Changes not stored" before another workspace opens.
    pub(super) question: Option<crate::CloseQuestion>,
    /// When the thread was asked last.
    checked: Option<Instant>,
    poller: Option<Poller>,
    disk: Disk,
    /// The number of the next job or look.
    next: u64,
    /// What was read before the change `floor` was made is not taken in.
    floor: u64,
    /// The newest read that was taken in.
    taken: u64,
    /// Grows with every change of what is shown.
    rev: u64,
    /// Today, by the computer's calendar (UC-021 BR-004).
    today: Option<Day>,
    built: Option<(Built, TodosView)>,
    /// What the window was told last.
    told: Option<TodosView>,
}

impl std::fmt::Debug for TodoState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TodoState")
            .field("root", &self.root)
            .field("todos", &self.todos.as_ref().map(|t| t.todos.len()))
            .field("pending", &self.pending.len())
            .field("unstored", &self.unstored.len())
            .finish_non_exhaustive()
    }
}

impl TodoState {
    /// Review F15: the undo of a mark goes when the todo changes again.
    fn changed_since_mark(&mut self, number: u64) {
        if self.marked_to.as_ref().is_some_and(|t| t.number == number) {
            self.marked = None;
            self.marked_to = None;
        }
    }

    fn seq(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// The todo `number` as read last.
    fn read_version(&self, number: u64) -> Option<Todo> {
        self.todos.as_ref()?.get(number).cloned()
    }

    /// The todo `number` as shown: being written, not stored, or as read.
    fn shown(&self, number: u64) -> Option<Todo> {
        self.pending
            .get(&number)
            .map(|p| p.todo.clone())
            .or_else(|| self.unstored.get(&number).map(|u| u.todo.clone()))
            .or_else(|| self.read_version(number))
    }

    /// Every todo as shown, by number.
    fn shown_all(&self) -> Vec<Todo> {
        let mut all: Vec<Todo> = self
            .todos
            .iter()
            .flat_map(|t| t.todos.iter())
            .filter_map(|t| self.shown(t.number))
            .collect();
        for number in self.pending.keys().chain(self.unstored.keys()) {
            if !all.iter().any(|t| t.number == *number) {
                all.extend(self.shown(*number));
            }
        }
        all.sort_by_key(|t| t.number);
        all.dedup_by_key(|t| t.number);
        all
    }

    /// Takes in the todos read by the look or the job `seq`, unless a
    /// change was made since or a newer read was taken in.
    fn take_read(&mut self, seq: u64, fingerprint: Fingerprint, todos: Todos) {
        if seq < self.floor || seq <= self.taken {
            return;
        }
        self.taken = seq;
        self.fingerprint = fingerprint;
        if self.todos.as_ref() != Some(&todos) {
            self.todos = Some(todos);
            self.rev += 1;
        }
    }

    /// The change `seq` was written or failed: it is not shown as being
    /// written any more, unless a newer one is.
    fn written(&mut self, number: u64, seq: u64) {
        if self.pending.get(&number).is_some_and(|p| p.seq == seq) {
            self.pending.remove(&number);
            self.rev += 1;
        }
    }

    /// A9: `todo` is kept as not stored, unless a newer change of it is
    /// being written.
    fn keep_unstored(&mut self, todo: Todo, why: String, base: Option<Todo>) {
        // Review F15: a deletion of the todo wins, in whatever order the
        // jobs end.
        if self.pending.contains_key(&todo.number) || self.deleted_here.contains(&todo.number) {
            return;
        }
        self.unstored
            .insert(todo.number, Unstored { todo, why, base });
        self.rev += 1;
    }
}

/// "YYYY-MM-DD HH:MM" of a time stored as `YYYY-MM-DDTHH:MM:SS+zz:zz`.
fn minutes(time: &str) -> String {
    time.get(..16).unwrap_or(time).replace('T', " ")
}

/// UC-020 BR-003: a day typed (`YYYY-MM-DD`), or none when empty.
fn typed_day(text: &str) -> Result<Option<Day>, TodoError> {
    let text = text.trim();
    if text.is_empty() {
        Ok(None)
    } else {
        text.parse().map(Some)
    }
}

/// M-18.2, M-18.3, M-18.8: one todo for the window.
fn todo_view(
    todo: &Todo,
    unstored: Option<&str>,
    notes: &[NoteId],
    active: Option<&NoteId>,
    today: Day,
) -> TodoView {
    let overdue = match (todo.due, overdue_days(todo, today)) {
        (Some(due), Some(days)) => {
            let late = if days == 1 {
                "1 day overdue".to_owned()
            } else {
                format!("{days} days overdue")
            };
            Some(format!("{due}  {late}"))
        }
        _ => None,
    };
    let stamp = match &todo.done {
        Some(done) => format!("done {}", minutes(done)),
        None => format!("created {}", minutes(&todo.created)),
    };
    TodoView {
        number: u32::try_from(todo.number).unwrap_or(u32::MAX),
        title: todo.title.clone(),
        name: todo.name(),
        urgency: todo.urgency.into(),
        urgency_label: todo.urgency.label().to_owned(),
        done: todo.is_done(),
        due: todo.due.map(|d| d.to_string()),
        scheduled: todo.scheduled.map(|d| d.to_string()),
        overdue,
        scheduled_after_due: todo.scheduled_after_due(),
        links: todo
            .links
            .iter()
            .map(|link| TodoLinkView {
                link: link.clone(),
                title: NoteId::from_relative(link).title(),
                exists: notes.iter().any(|n| n.as_str().eq_ignore_ascii_case(link)),
            })
            .collect(),
        more_links: (todo.links.len() > 2).then(|| format!("+{}", todo.links.len() - 2)),
        links_summary: links_summary(todo),
        links_active: active.is_some_and(|note| todo.links_to(note)),
        stamp,
        done_at: todo.done.as_deref().map(minutes),
        unstored: unstored.map(str::to_owned),
    }
}

fn number_of(todo: &Todo) -> u32 {
    u32::try_from(todo.number).unwrap_or(u32::MAX)
}

/// M-18.3, UC-020 BR-003: the month of `day` (today without one), `step`
/// months on, Monday first, for the calendar "Choose a day".
#[must_use]
pub fn calendar_view(day: Option<&str>, step: i32) -> CalendarView {
    let today = Day::today();
    let month = day
        .and_then(|d| d.parse::<Day>().ok())
        .unwrap_or(today)
        .month_start()
        .add_months(step);
    CalendarView {
        title: month.month_title(),
        month: month.to_string(),
        lead: u32::from(month.monday_offset()),
        days: (1..=month.days_in_month())
            .map(|number| {
                let day = month.with_day(number);
                CalendarDayView {
                    day: day.to_string(),
                    label: u32::from(number),
                    today: day == today,
                }
            })
            .collect(),
    }
}

impl Session {
    /// UC-020, UC-021: a workspace was opened; the todos of the one left
    /// are forgotten (its thread stops), and those of this one are read at
    /// once by its own thread.
    pub(super) fn track_todos(&mut self, root: &Path) {
        let disk = Disk::default();
        let poller = Poller::start(root.to_owned(), disk.clone(), self.wake.clone());
        self.todos = TodoState {
            root: Some(root.to_owned()),
            disk,
            poller,
            ..TodoState::default()
        };
    }

    /// The tick's part (M-18.9): takes in what the thread found, and asks
    /// it again a second after it was asked (egui's `poll_todos`): what
    /// could not be stored is tried again (UC-020 A9), and the todos are
    /// read again when their files changed (A7), so a change by an agent, a
    /// sync or another program shows within a second.
    pub(super) fn todos_tick(&mut self, now: Instant) {
        self.take_todo_poll();
        let due = self
            .todos
            .checked
            .is_none_or(|at| now.saturating_duration_since(at) >= TODOS_EVERY);
        let busy = self.todos.poller.as_ref().is_none_or(|p| p.waiting);
        if busy || !due {
            return;
        }
        self.todos.checked = Some(now);
        // UC-021 BR-004: a new day may make todos overdue.
        let today = Day::today();
        if self.todos.today != Some(today) {
            self.todos.today = Some(today);
            self.todos.rev += 1;
        }
        let seq = self.todos.seq();
        let state = &mut self.todos;
        let ask = PollAsk {
            seq,
            unstored: state
                .unstored
                .values()
                .map(|u| (u.todo.clone(), u.base.clone()))
                .collect(),
            before: state.todos.is_some().then(|| state.fingerprint.clone()),
        };
        if let Some(poller) = &mut state.poller {
            poller.waiting = poller.ask.send(ask).is_ok();
        }
    }

    fn take_todo_poll(&mut self) {
        let Some(poller) = &mut self.todos.poller else {
            return;
        };
        let polled = match poller.answer.try_recv() {
            Ok(polled) => polled,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.todos.poller = None;
                return;
            }
        };
        poller.waiting = false;
        let state = &mut self.todos;
        for todo in polled.stored {
            if state
                .unstored
                .get(&todo.number)
                .is_some_and(|u| u.todo == todo)
            {
                state.unstored.remove(&todo.number);
                state.rev += 1;
            }
        }
        for (number, why) in polled.refused {
            if let Some(unstored) = state.unstored.get_mut(&number) {
                if unstored.why != why {
                    unstored.why = why;
                    state.rev += 1;
                }
            }
        }
        match polled.todos {
            Some(todos) => state.take_read(polled.seq, polled.fingerprint, todos),
            None if polled.seq >= state.floor => state.fingerprint = polled.fingerprint,
            None => {}
        }
    }

    /// Whether the todos of the open workspace were read once.
    #[must_use]
    pub fn todos_read(&self) -> bool {
        self.todos.todos.is_some()
    }

    /// Review F15 (UC-020 A9, BR-006): the last attempt to store what is
    /// being written or could not be stored, before another workspace
    /// opens or Underline ends; it waits for a write that runs. Returns
    /// what still cannot be stored, as "Todo #n could not be stored: …";
    /// those stay shown "not stored".
    pub(super) fn todos_settle(&mut self) -> Result<(), Vec<String>> {
        let Some(root) = self.todos.root.clone() else {
            return Ok(());
        };
        let mut waiting: HashMap<u64, (Todo, Option<Todo>)> = self
            .todos
            .unstored
            .iter()
            .map(|(n, u)| (*n, (u.todo.clone(), u.base.clone())))
            .collect();
        // A change being written is newer than one not stored.
        for (number, pending) in &self.todos.pending {
            waiting.insert(*number, (pending.todo.clone(), pending.base.clone()));
        }
        let mut numbers: Vec<u64> = waiting.keys().copied().collect();
        numbers.sort_unstable();
        let mut failures = Vec::new();
        for number in numbers {
            let Some((todo, base)) = waiting.remove(&number) else {
                continue;
            };
            let seq = self.todos.seq();
            let why = match self.todos.disk.save(&root, &todo, base.as_ref(), seq) {
                Ok(Saved::Written | Saved::Skipped) => {
                    self.todos.pending.remove(&number);
                    self.todos.unstored.remove(&number);
                    self.todos.rev += 1;
                    continue;
                }
                Ok(Saved::Refused(why)) => why,
                Err(err) => err.to_string(),
            };
            failures.push(format!("Todo #{number} could not be stored: {why}"));
            self.todos.pending.remove(&number);
            self.todos.keep_unstored(todo, why, base);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }

    /// Review F15: the changes of the todos that cannot be stored are
    /// given up ("Close anyway"); nothing being written or tried again
    /// writes them after this.
    pub(super) fn give_up_todos(&mut self) {
        let numbers: Vec<u64> = self
            .todos
            .pending
            .keys()
            .chain(self.todos.unstored.keys())
            .copied()
            .collect();
        for number in numbers {
            let seq = self.todos.seq();
            self.todos.disk.forget(number, seq);
        }
        self.todos.pending.clear();
        self.todos.unstored.clear();
        self.todos.rev += 1;
    }

    /// A job for `work` on the open workspace; what was read before it is
    /// not taken in after it.
    fn todo_job(&mut self, work: Work) -> Option<TodoJob> {
        let root = self.todos.root.clone()?;
        let seq = self.todos.seq();
        self.todos.floor = seq;
        // Shown at once; a change not stored before is part of it.
        let shown = match &work {
            Work::Change { todo, base } | Work::Mark { todo, base, .. } => {
                Some((todo.clone(), base.clone()))
            }
            Work::UndoMark { before, marked } => Some((before.clone(), Some(marked.clone()))),
            _ => None,
        };
        if let Some((todo, base)) = shown {
            self.todos.unstored.remove(&todo.number);
            self.todos
                .pending
                .insert(todo.number, Pending { seq, todo, base });
            self.todos.rev += 1;
        }
        Some(TodoJob {
            root,
            seq,
            disk: self.todos.disk.clone(),
            work,
        })
    }

    /// M-18.1, UC-020 steps 3–4, A3: a new todo titled `title`; the title
    /// is checked here, the todo is created by the job.
    ///
    /// # Errors
    ///
    /// The answer at once: [`TodoAnswer::Problem`] for a title that is
    /// empty or longer than 200 characters, [`TodoAnswer::Failed`] without
    /// a workspace.
    pub fn todo_create_start(&mut self, title: &str) -> Result<TodoJob, TodoAnswer> {
        if let Err(err) = Todo::new(0, title) {
            return Err(TodoAnswer::Problem {
                problem: err.to_string(),
            });
        }
        self.todo_job(Work::Create(title.to_owned()))
            .ok_or(TodoAnswer::Failed)
    }

    /// The note `note` as the workspace lists it.
    fn listed_note(&self, note: &str) -> Option<NoteId> {
        let id = NoteId::from_relative(note);
        if !id.is_valid() {
            return None;
        }
        self.open
            .as_ref()?
            .workspace
            .listing()
            .notes
            .iter()
            .find_map(|entry| {
                entry
                    .id
                    .as_str()
                    .eq_ignore_ascii_case(id.as_str())
                    .then(|| entry.id.clone())
            })
    }

    /// M-18.2 to M-18.4 (UC-020 steps 5–10, A1, A2, A4): `change` made to
    /// the todo `number` as shown; it shows at once and is stored by the
    /// job, made from the todo as read last.
    ///
    /// # Errors
    ///
    /// The answer at once: [`TodoAnswer::Problem`] for a title or a day
    /// that is not allowed, or a note that is not in the workspace (shown
    /// under the fields), [`TodoAnswer::Done`] when nothing changed or the
    /// todo is gone.
    pub fn todo_change_start(
        &mut self,
        number: u32,
        change: TodoChange,
    ) -> Result<TodoJob, TodoAnswer> {
        let number = u64::from(number);
        let Some(mut todo) = self.todos.shown(number) else {
            return Err(TodoAnswer::Done);
        };
        let before = todo.clone();
        let changed: Result<(), String> = match change {
            TodoChange::Title { title } => todo.set_title(&title).map_err(|e| e.to_string()),
            TodoChange::Due { day } => typed_day(&day)
                .map(|day| todo.due = day)
                .map_err(|e| e.to_string()),
            TodoChange::Scheduled { day } => typed_day(&day)
                .map(|day| todo.scheduled = day)
                .map_err(|e| e.to_string()),
            TodoChange::Urgency { urgency } => {
                todo.urgency = urgency.into();
                Ok(())
            }
            TodoChange::Done { done } => {
                todo.set_done(done);
                Ok(())
            }
            // Review F15 (new text, Q-117): only a note of the workspace.
            TodoChange::Link { note } => match self.listed_note(&note) {
                Some(listed) => {
                    todo.link(&listed);
                    Ok(())
                }
                None => Err(format!("the note {note} is not in this workspace")),
            },
            TodoChange::Unlink { link } => {
                todo.unlink(&link);
                Ok(())
            }
        };
        if let Err(problem) = changed {
            return Err(TodoAnswer::Problem { problem });
        }
        if todo == before {
            return Err(TodoAnswer::Done);
        }
        let base = self.todos.read_version(number);
        self.todos.changed_since_mark(number);
        self.todo_job(Work::Change { todo, base })
            .ok_or(TodoAnswer::Done)
    }

    /// M-18.7, M-18.8 (UC-021 steps 7–8): the todo `number` marked done
    /// (`done`) or open again in the tab "Todos", with "Undo" after it;
    /// `None` when it is gone or already so.
    pub fn todo_mark_start(&mut self, number: u32, done: bool) -> Option<TodoJob> {
        let number = u64::from(number);
        let before = self.todos.shown(number)?;
        if before.is_done() == done {
            return None;
        }
        let mut todo = before.clone();
        todo.set_done(done);
        let base = self.todos.read_version(number);
        self.todos.changed_since_mark(number);
        self.todo_job(Work::Mark { before, todo, base })
    }

    /// M-18.5 (UC-020 A5): "Delete" of the todo `number`, without asking.
    /// A todo that has no file (only a change not stored) goes at once
    /// (review F15).
    pub fn todo_delete_start(&mut self, number: u32) -> Option<TodoJob> {
        let number = u64::from(number);
        self.todos.root.as_ref()?;
        self.todos.changed_since_mark(number);
        if self.todos.read_version(number).is_none() {
            let todo = self.todos.shown(number)?;
            // Review F15: nothing being written or tried again writes it
            // after "Deleted todo".
            let seq = self.todos.seq();
            self.todos.disk.forget(number, seq);
            self.todos.deleted_here.insert(number);
            self.todos.pending.remove(&number);
            self.todos.unstored.remove(&number);
            self.todos.rev += 1;
            self.status = format!("Deleted todo {}", todo.name());
            self.todos.deleted = Some((todo, self.status.clone()));
            return None;
        }
        self.todos.deleted_here.insert(number);
        self.todo_job(Work::Delete(number))
    }

    /// M-04.6 (UC-020 A5 step 2, UC-021 step 8): "Undo" of the status bar
    /// after a todo was deleted or marked, while its report shows; `None`
    /// once another report took its place.
    pub fn todo_undo_start(&mut self) -> Option<TodoJob> {
        let status = self.status.clone();
        if let Some((todo, _)) = self.todos.deleted.take_if(|(_, report)| *report == status) {
            self.todos.deleted_here.remove(&todo.number);
            return self.todo_job(Work::UndoDelete(todo));
        }
        let (before, _) = self.todos.marked.take_if(|(_, report)| *report == status)?;
        let marked = self.todos.marked_to.take()?;
        self.todo_job(Work::UndoMark { before, marked })
    }

    /// Takes in what a [`TodoJob`] did: the status says it (egui's texts),
    /// a change that could not be stored is kept and shown "not stored"
    /// (UC-020 A9), and the todos read after it show.
    pub fn todo_finish(&mut self, done: TodoDone) -> TodoAnswer {
        let TodoDone {
            root,
            seq,
            work,
            result,
            read,
        } = done;
        if self.todos.root.as_ref() != Some(&root) {
            return TodoAnswer::Done;
        }
        let answer = self.todo_report(work, result, seq);
        let (fingerprint, todos) = read;
        self.todos.take_read(seq, fingerprint, todos);
        answer
    }

    fn todo_report(&mut self, work: Work, result: Result<Did, TodoError>, seq: u64) -> TodoAnswer {
        // What could not be written, and why.
        let failed = |result: &Result<Did, TodoError>| match result {
            Ok(Did::Saved(Saved::Refused(why))) => Some(why.clone()),
            Err(err) => Some(err.to_string()),
            Ok(_) => None,
        };
        match work {
            // UC-020 steps 3–4, A3.
            Work::Create(_) => match result {
                Ok(Did::Todo(todo)) => {
                    self.status = format!("Created todo {}", todo.name());
                    TodoAnswer::Done
                }
                Err(err @ (TodoError::NoTitle | TodoError::LongTitle)) => TodoAnswer::Problem {
                    problem: err.to_string(),
                },
                Err(err) => {
                    self.status = format!("The todo could not be created: {err}");
                    TodoAnswer::Failed
                }
                Ok(Did::Saved(_)) => TodoAnswer::Done,
            },
            // Steps 5–10, A2, A4; A9: kept, shown "not stored", tried again
            // every second.
            Work::Change { todo, base } => {
                self.todos.written(todo.number, seq);
                match result {
                    Ok(Did::Saved(Saved::Refused(why))) => {
                        self.status = format!("Todo #{} could not be stored: {why}", todo.number);
                        self.todos.keep_unstored(todo, why, base);
                        TodoAnswer::Done
                    }
                    Err(err @ TodoError::Write(_)) => {
                        let why = err.to_string();
                        self.status = format!("Todo #{} could not be stored: {why}", todo.number);
                        self.todos.keep_unstored(todo, why, base);
                        TodoAnswer::Done
                    }
                    Err(err) => TodoAnswer::Problem {
                        problem: err.to_string(),
                    },
                    Ok(_) => TodoAnswer::Done,
                }
            }
            // UC-021 step 8: with "Undo". As egui, a mark that could not be
            // stored is not kept (A-180).
            Work::Mark { before, todo, .. } => {
                self.todos.written(todo.number, seq);
                if let Some(why) = failed(&result) {
                    self.status = format!("Todo #{} could not be stored: {why}", todo.number);
                    return TodoAnswer::Failed;
                }
                self.status = format!(
                    "Marked todo {} {}",
                    todo.name(),
                    if todo.is_done() { "done" } else { "open" }
                );
                self.todos.marked = Some((before, self.status.clone()));
                self.todos.marked_to = Some(todo);
                TodoAnswer::Done
            }
            // A5: with "Undo".
            Work::Delete(number) => match result {
                Ok(Did::Todo(todo)) => {
                    self.todos.pending.remove(&number);
                    self.todos.unstored.remove(&number);
                    self.todos.rev += 1;
                    self.status = format!("Deleted todo {}", todo.name());
                    self.todos.deleted = Some((todo, self.status.clone()));
                    TodoAnswer::Done
                }
                Err(err) => {
                    self.todos.deleted_here.remove(&number);
                    self.status = format!("The todo could not be deleted: {err}");
                    TodoAnswer::Failed
                }
                Ok(Did::Saved(_)) => TodoAnswer::Done,
            },
            Work::UndoDelete(todo) => {
                if let Some(why) = failed(&result) {
                    self.status = format!("Todo {} could not be brought back: {why}", todo.name());
                    return TodoAnswer::Failed;
                }
                self.status = format!("Todo {} is back", todo.name());
                TodoAnswer::Done
            }
            Work::UndoMark { before, .. } => {
                self.todos.written(before.number, seq);
                if let Some(why) = failed(&result) {
                    self.status = format!("Todo #{} could not be stored: {why}", before.number);
                    return TodoAnswer::Failed;
                }
                self.status = format!("Todo {} is as before", before.name());
                TodoAnswer::Done
            }
        }
    }

    /// M-04.6: "Undo" is offered while the report of a deleted or marked
    /// todo shows; another report ends it.
    pub(super) fn todo_offer(&mut self) -> Option<StatusOffer> {
        let status = &self.status;
        let todos = &mut self.todos;
        todos.deleted.take_if(|(_, report)| report != status);
        if todos
            .marked
            .take_if(|(_, report)| report != status)
            .is_some()
        {
            todos.marked_to = None;
        }
        if todos.deleted.is_some() {
            Some(StatusOffer::UndoTodoDelete)
        } else if todos.marked.is_some() {
            Some(StatusOffer::UndoTodoMark)
        } else {
            None
        }
    }

    /// D-04 (fixed, like A-83): the report of a deleted or marked todo with
    /// its "Undo", which "Saved {t}" does not replace.
    pub(super) fn todo_report_kept(&self) -> bool {
        let shows = |offer: &Option<(Todo, String)>| {
            offer
                .as_ref()
                .is_some_and(|(_, report)| *report == self.status)
        };
        shows(&self.todos.deleted) || shows(&self.todos.marked)
    }

    /// M-18.4, M-18.8 (UC-020 step 8, A6, UC-021 step 5, BR-008): a linked
    /// note opens in a tab; one that no longer exists is said so.
    pub fn todo_open_link(&mut self, link: &str) {
        let found = self.open.as_ref().and_then(|open| {
            open.workspace
                .listing()
                .notes
                .iter()
                .find(|entry| entry.id.as_str().eq_ignore_ascii_case(link))
                .map(|entry| entry.id.clone())
        });
        match found {
            Some(note) => {
                self.open_note(&note);
            }
            None => {
                self.status = format!("The note {link} no longer exists; remove the link with ×");
            }
        }
    }

    /// M-18.4, UC-020 A1: the notes "Link a note…" offers for `query`: up
    /// to 8 whose title holds it, in the order of the workspace's notes,
    /// not linked by the todo `number` yet.
    #[must_use]
    pub fn todo_candidates(&self, number: u32, query: &str) -> TodoCandidatesView {
        let query = query.trim().to_lowercase();
        let (Some(open), Some(todo), false) = (
            self.open.as_ref(),
            self.todos.shown(u64::from(number)),
            query.is_empty(),
        ) else {
            return TodoCandidatesView {
                notes: Vec::new(),
                none: None,
            };
        };
        let notes: Vec<TodoCandidateView> = open
            .workspace
            .listing()
            .notes
            .iter()
            .map(|entry| &entry.id)
            .filter(|note| !todo.links_to(note) && note.title().to_lowercase().contains(&query))
            .take(CANDIDATES)
            .map(|note| TodoCandidateView {
                note: note.as_str().to_owned(),
                label: note.as_str().replace('-', " "),
            })
            .collect();
        let none = notes.is_empty().then(|| "No note matches".to_owned());
        TodoCandidatesView { notes, none }
    }

    /// M-06.5, M-04.5 (UC-021 step 1, A2, BR-006): shows the tab "Todos",
    /// opening it if needed; the overdue todos of the status bar show it
    /// with the filter "Open".
    pub fn show_todo_list(&mut self, filter: Option<TodoFilterName>) {
        self.change_todo_list(|list| {
            list.open = true;
            list.active = true;
            if let Some(filter) = filter {
                list.filter = filter.into();
            }
        });
    }

    /// M-06.5: "×" or a middle click closes the tab "Todos".
    pub fn close_todo_list(&mut self) {
        self.change_todo_list(|list| {
            list.open = false;
            list.active = false;
        });
    }

    /// M-18.6, M-18.8 (UC-021 steps 3–4, A1, BR-005, BR-007): the sort and
    /// the filter chosen in the tab "Todos", kept for the workspace.
    pub fn set_todo_list(&mut self, sort: TodoSortName, filter: TodoFilterName) {
        self.change_todo_list(|list| {
            list.sort = sort.into();
            list.filter = filter.into();
        });
    }

    /// UC-021 BR-005: changes and remembers the tab "Todos".
    fn change_todo_list(&mut self, change: impl FnOnce(&mut TodoListState)) {
        let Some(open) = &mut self.open else {
            return;
        };
        let before = open.todo_list;
        change(&mut open.todo_list);
        if open.todo_list == before {
            return;
        }
        let root = open.workspace.root().to_owned();
        self.state.set_todo_list(&root, open.todo_list);
        self.views_dirty = true;
    }

    /// M-16.3, M-18, M-04.5: the todos for the window, when they changed;
    /// nothing before they were read.
    pub(super) fn todos_event(&mut self) -> Option<Event> {
        let view = self.todos_view()?;
        if self.todos.told.as_ref() == Some(&view) {
            return None;
        }
        self.todos.told = Some(view.clone());
        Some(Event::Todos(Box::new(view)))
    }

    /// The view, made again only when what it shows changed.
    fn todos_view(&mut self) -> Option<TodosView> {
        let open = self.open.as_ref()?;
        self.todos.todos.as_ref()?;
        let list = open.todo_list;
        let active = open
            .tabs
            .active()
            .filter(|_| !(list.open && list.active))
            .map(|(note, _)| note.clone());
        let today = self.todos.today.unwrap_or_else(Day::today);
        let built: Built = (
            self.todos.rev,
            today,
            active.clone(),
            list,
            open.overview_generation,
        );
        if let Some((made, view)) = &self.todos.built {
            if *made == built {
                return Some(view.clone());
            }
        }
        let notes: Vec<NoteId> = open
            .workspace
            .listing()
            .notes
            .iter()
            .map(|entry| entry.id.clone())
            .collect();
        let view = self.make_todos_view(&notes, active.as_ref(), list, today);
        self.todos.built = Some((built, view.clone()));
        Some(view)
    }

    fn make_todos_view(
        &self,
        notes: &[NoteId],
        active: Option<&NoteId>,
        list: TodoListState,
        today: Day,
    ) -> TodosView {
        let state = &self.todos;
        // Numbers beyond u32 are not shown (review F15): the window counts in u32.
        let all: Vec<Todo> = state
            .shown_all()
            .into_iter()
            .filter(|t| u32::try_from(t.number).is_ok())
            .collect();
        let views = all
            .iter()
            .map(|todo| {
                let unstored = state.unstored.get(&todo.number).map(|u| u.why.as_str());
                todo_view(todo, unstored, notes, active, today)
            })
            .collect();
        // M-18.1: the next due first, those without a due day last.
        let mut open: Vec<&Todo> = all.iter().filter(|t| !t.is_done()).collect();
        open.sort_by_key(|t| (t.due.is_none(), t.due, t.number));
        let done: Vec<u32> = all.iter().filter(|t| t.is_done()).map(number_of).collect();
        let open_count = u32::try_from(open.len()).unwrap_or(u32::MAX);
        let overdue = match overdue_count(&all, today) {
            0 => None,
            1 => Some("1 todo overdue".to_owned()),
            n => Some(format!("{n} todos overdue")),
        };
        let rows: Vec<u32> = todo_list(&all, list.sort, list.filter)
            .into_iter()
            .map(number_of)
            .collect();
        // UC-021 A3.
        let empty = rows.is_empty().then(|| {
            match list.filter {
                TodoFilter::Open => "No open todos. Type a title above to create one.",
                TodoFilter::Done => "No done todos.",
                TodoFilter::All => "No todos. Type a title above to create one.",
            }
            .to_owned()
        });
        TodosView {
            heading: format!("Todos ({open_count})"),
            open_count,
            todos: views,
            open: open.into_iter().map(number_of).collect(),
            done,
            unreadable: state
                .todos
                .iter()
                .flat_map(|t| t.unreadable.iter())
                .map(ToString::to_string)
                .collect(),
            overdue,
            active_note: active.map(|note| TodoActiveNoteView {
                note: note.as_str().to_owned(),
                link_label: format!("Link {}", note.title()),
            }),
            table: TodoTableView {
                sort: list.sort.into(),
                filter: list.filter.into(),
                rows,
                empty,
                done_heading: if list.filter == TodoFilter::Open {
                    String::new()
                } else {
                    "Done".to_owned()
                },
            },
        }
    }
}

#[cfg(test)]
mod tests;
