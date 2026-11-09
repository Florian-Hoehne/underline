//! UC-020: the todos of a workspace, one YAML file each in `.todos/`
//! (ADR-019), shared through git with the notes.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::atomic::{write_atomic, write_new_atomic};
use crate::conflict::Joining;
use crate::git::{Git, GitError};
use crate::note::NoteId;

/// The folder of the todos in the workspace root (ADR-019).
pub const TODOS_FOLDER: &str = ".todos";
/// UC-020 BR-001: the longest title.
pub const MAX_TODO_TITLE: usize = 200;
const NUMBERS_FILE: &str = ".numbers";
const ATTRIBUTES_FILE: &str = ".gitattributes";
/// ADR-019: git never joins two versions of a todo line by line.
const ATTRIBUTES: &str = "*.yaml -merge\n";

/// How pressing a todo is (UC-020 BR-001).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl Urgency {
    pub const ALL: [Urgency; 4] = [
        Urgency::Low,
        Urgency::Normal,
        Urgency::High,
        Urgency::Critical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Urgency::Low => "Low",
            Urgency::Normal => "Normal",
            Urgency::High => "High",
            Urgency::Critical => "Critical",
        }
    }
}

impl FromStr for Urgency {
    type Err = String;
    fn from_str(text: &str) -> Result<Self, String> {
        Urgency::ALL
            .into_iter()
            .find(|u| u.label().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| format!("the urgency is low, normal, high, or critical, not `{text}`"))
    }
}

/// Whether a todo is still to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    #[default]
    Open,
    Done,
}

/// A day without a time (UC-020 BR-003), written `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(jiff::civil::Date);

impl Day {
    /// Today in the computer's time zone.
    pub fn today() -> Self {
        Day(jiff::Zoned::now().date())
    }

    /// The first day of this day's month (BR-003, the calendar).
    pub fn month_start(self) -> Day {
        Day(self.0.first_of_month())
    }

    /// The same day `months` months later (or earlier), within the month.
    pub fn add_months(self, months: i32) -> Day {
        self.0
            .checked_add(jiff::Span::new().months(months))
            .map(Day)
            .unwrap_or(self)
    }

    /// The weekday, 0 for Monday.
    pub fn monday_offset(self) -> u8 {
        self.0.weekday().to_monday_zero_offset() as u8
    }

    pub fn days_in_month(self) -> u8 {
        self.0.days_in_month() as u8
    }

    /// Day `day` of this day's month.
    pub fn with_day(self, day: u8) -> Day {
        self.0
            .with()
            .day(day as i8)
            .build()
            .map(Day)
            .unwrap_or(self)
    }

    /// How many days lie between `earlier` and this day.
    pub fn days_since(self, earlier: Day) -> i64 {
        earlier
            .0
            .until(self.0)
            .map(|span| i64::from(span.get_days()))
            .unwrap_or(0)
    }

    /// The month and year, such as "November 2026".
    pub fn month_title(self) -> String {
        self.0.strftime("%B %Y").to_string()
    }
}

impl FromStr for Day {
    type Err = TodoError;
    fn from_str(text: &str) -> Result<Self, TodoError> {
        let text = text.trim();
        let valid = text.len() == 10 && text.as_bytes()[4] == b'-' && text.as_bytes()[7] == b'-';
        valid
            .then(|| jiff::civil::Date::from_str(text).ok())
            .flatten()
            .map(Day)
            .ok_or_else(|| TodoError::Day(text.to_owned()))
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.strftime("%Y-%m-%d"))
    }
}

impl Serialize for Day {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Day {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// One todo (UC-020 BR-001, entity TODO).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    /// The number, from the file name.
    #[serde(skip)]
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub urgency: Urgency,
    #[serde(default)]
    pub status: TodoStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Day>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<Day>,
    /// When it was created, local time with its offset.
    #[serde(default)]
    pub created: String,
    /// When it was marked done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<String>,
    /// The notes it is about, as listed (`Projects/Plan`), each once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    /// Keys written by a later version, kept as they are.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_norway::Value>,
}

/// Why a todo cannot be created, changed, or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TodoError {
    /// A3.
    #[error("a todo needs a title")]
    NoTitle,
    /// A3.
    #[error("the title of a todo has at most {MAX_TODO_TITLE} characters")]
    LongTitle,
    #[error("`{0}` is not a day; write it as YYYY-MM-DD")]
    Day(String),
    #[error("there is no todo #{0}")]
    NoSuchTodo(u64),
    /// A9.
    #[error("the todos cannot be stored: {0}")]
    Write(String),
    #[error("todo #{number} cannot be read: {reason}")]
    Read { number: u64, reason: String },
    #[error("{0}")]
    Git(String),
}

fn now_text() -> String {
    jiff::Zoned::now()
        .strftime("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

impl Todo {
    /// UC-020 step 4: a new todo, open and normal, without dates.
    pub fn new(number: u64, title: &str) -> Result<Self, TodoError> {
        Ok(Todo {
            number,
            title: check_title(title)?,
            urgency: Urgency::Normal,
            status: TodoStatus::Open,
            due: None,
            scheduled: None,
            created: now_text(),
            done: None,
            links: Vec::new(),
            other: BTreeMap::new(),
        })
    }

    pub fn is_done(&self) -> bool {
        self.status == TodoStatus::Done
    }

    /// UC-020 step 10, A4, BR-007: done with the time, or open again.
    pub fn set_done(&mut self, done: bool) {
        if done == self.is_done() {
            return;
        }
        self.status = if done {
            TodoStatus::Done
        } else {
            TodoStatus::Open
        };
        self.done = done.then(now_text);
    }

    /// A3: sets the title, if it is allowed.
    pub fn set_title(&mut self, title: &str) -> Result<(), TodoError> {
        self.title = check_title(title)?;
        Ok(())
    }

    /// FR-066: links the note `note`, once.
    pub fn link(&mut self, note: &NoteId) -> bool {
        if self.links_to(note) {
            return false;
        }
        self.links.push(note.as_str().to_owned());
        true
    }

    /// A2: removes the link to `note`.
    pub fn unlink(&mut self, note: &str) -> bool {
        let before = self.links.len();
        self.links.retain(|l| !l.eq_ignore_ascii_case(note));
        self.links.len() != before
    }

    pub fn links_to(&self, note: &NoteId) -> bool {
        self.links
            .iter()
            .any(|l| l.eq_ignore_ascii_case(note.as_str()))
    }

    /// BR-003: the scheduled date is after the due date, shown as a hint.
    pub fn scheduled_after_due(&self) -> bool {
        matches!((self.scheduled, self.due), (Some(s), Some(d)) if s > d)
    }

    /// How the todo is named in messages, such as `#12 Ship release`.
    pub fn name(&self) -> String {
        format!("#{} {}", self.number, self.title)
    }

    fn to_text(&self) -> Result<String, TodoError> {
        serde_norway::to_string(self).map_err(|e| TodoError::Write(e.to_string()))
    }

    fn from_text(number: u64, text: &str) -> Result<Self, TodoError> {
        let mut todo: Todo = serde_norway::from_str(text).map_err(|e| TodoError::Read {
            number,
            reason: e.to_string(),
        })?;
        todo.number = number;
        Ok(todo)
    }
}

/// UC-020 A3: a title of 1 to 200 characters, on one line.
fn check_title(title: &str) -> Result<String, TodoError> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return Err(TodoError::NoTitle);
    }
    if title.chars().count() > MAX_TODO_TITLE {
        return Err(TodoError::LongTitle);
    }
    Ok(title)
}

/// UC-021 BR-001, BR-002: the order of the todo list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoSort {
    #[default]
    Due,
    Urgency,
}

/// UC-021 BR-003: which todos the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoFilter {
    #[default]
    Open,
    Done,
    All,
}

impl TodoFilter {
    pub const ALL: [TodoFilter; 3] = [TodoFilter::Open, TodoFilter::Done, TodoFilter::All];

    pub fn label(self) -> &'static str {
        match self {
            TodoFilter::Open => "Open",
            TodoFilter::Done => "Done",
            TodoFilter::All => "All",
        }
    }

    pub fn keeps(self, todo: &Todo) -> bool {
        match self {
            TodoFilter::Open => !todo.is_done(),
            TodoFilter::Done => todo.is_done(),
            TodoFilter::All => true,
        }
    }
}

/// UC-021 steps 2–4, A1, BR-001 to BR-003: the todos `filter` keeps, in
/// the order of `sort`.
pub fn todo_list(todos: &[Todo], sort: TodoSort, filter: TodoFilter) -> Vec<&Todo> {
    let mut list: Vec<&Todo> = todos.iter().filter(|t| filter.keeps(t)).collect();
    // Without a due date last; the most urgent first.
    let due = |t: &Todo| (t.due.is_none(), t.due);
    let urgency = |t: &Todo| std::cmp::Reverse(t.urgency);
    match sort {
        TodoSort::Due => list.sort_by_key(|t| (due(t), urgency(t), t.number)),
        TodoSort::Urgency => list.sort_by_key(|t| (urgency(t), due(t), t.number)),
    }
    list
}

/// UC-021 BR-004: how many days an open todo is overdue on `today`.
pub fn overdue_days(todo: &Todo, today: Day) -> Option<i64> {
    let due = todo.due.filter(|d| *d < today && !todo.is_done())?;
    Some(today.days_since(due))
}

/// UC-021 A2, BR-004: how many todos are overdue on `today`.
pub fn overdue_count(todos: &[Todo], today: Day) -> usize {
    todos
        .iter()
        .filter(|t| overdue_days(t, today).is_some())
        .count()
}

/// UC-021 BR-009: up to two linked notes and how many more, such as
/// "Plan, Ideas +2".
pub fn links_summary(todo: &Todo) -> String {
    let titles: Vec<String> = todo
        .links
        .iter()
        .take(2)
        .map(|l| NoteId::from_relative(l).title())
        .collect();
    let mut text = titles.join(", ");
    if todo.links.len() > 2 {
        text.push_str(&format!(" +{}", todo.links.len() - 2));
    }
    text
}

/// UC-021 BR-005: the todo list of a workspace as this computer keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TodoListState {
    /// The tab "Todos" is open, and it is the active tab.
    pub open: bool,
    pub active: bool,
    pub sort: TodoSort,
    pub filter: TodoFilter,
}

impl TodoListState {
    pub fn is_default(&self) -> bool {
        *self == TodoListState::default()
    }
}

/// The todos folder of the workspace at `root`.
pub fn todos_folder(root: &Path) -> PathBuf {
    root.join(TODOS_FOLDER)
}

fn todo_path(root: &Path, number: u64) -> PathBuf {
    todos_folder(root).join(format!("{number}.yaml"))
}

/// The path of todo `number` relative to the workspace, as git names it.
pub fn todo_relative_path(number: u64) -> String {
    format!("{TODOS_FOLDER}/{number}.yaml")
}

/// The todo number of a path relative to the workspace, if it is a todo.
pub fn todo_number_of(path: &str) -> Option<u64> {
    path.strip_prefix(TODOS_FOLDER)?
        .strip_prefix('/')?
        .strip_suffix(".yaml")?
        .parse()
        .ok()
}

fn write_error(err: io::Error) -> TodoError {
    TodoError::Write(err.to_string())
}

/// Creates the folder and its git attributes (ADR-019).
fn ensure_folder(root: &Path) -> Result<PathBuf, TodoError> {
    let folder = todos_folder(root);
    fs::create_dir_all(&folder).map_err(write_error)?;
    let attributes = folder.join(ATTRIBUTES_FILE);
    if fs::read_to_string(&attributes).ok().as_deref() != Some(ATTRIBUTES) {
        write_atomic(&attributes, ATTRIBUTES.as_bytes()).map_err(write_error)?;
    }
    Ok(folder)
}

/// The todos of a workspace as stored, and those that cannot be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Todos {
    /// By number.
    pub todos: Vec<Todo>,
    pub unreadable: Vec<TodoError>,
}

impl Todos {
    pub fn get(&self, number: u64) -> Option<&Todo> {
        self.todos.iter().find(|t| t.number == number)
    }

    /// UC-007 A2: the todos that link to `note`.
    pub fn linking(&self, note: &NoteId) -> Vec<&Todo> {
        self.todos.iter().filter(|t| t.links_to(note)).collect()
    }
}

/// UC-020 step 2, A7: reads the todos of the workspace at `root`.
pub fn load_todos(root: &Path) -> Todos {
    let mut todos = Todos::default();
    let Ok(entries) = fs::read_dir(todos_folder(root)) else {
        return todos;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(number) = name
            .to_str()
            .and_then(|n| n.strip_suffix(".yaml"))
            .and_then(|n| n.parse::<u64>().ok())
        else {
            continue;
        };
        match fs::read_to_string(entry.path()) {
            Ok(text) => match Todo::from_text(number, &text) {
                Ok(todo) => todos.todos.push(todo),
                Err(err) => todos.unreadable.push(err),
            },
            Err(err) => todos.unreadable.push(TodoError::Read {
                number,
                reason: err.to_string(),
            }),
        }
    }
    todos.todos.sort_by_key(|t| t.number);
    todos
}

/// UC-020 A7: a fingerprint of the todo files, which changes whenever one of
/// them is created, changed, or removed.
pub fn todos_fingerprint(root: &Path) -> Vec<(String, Option<std::time::SystemTime>, u64)> {
    let Ok(entries) = fs::read_dir(todos_folder(root)) else {
        return Vec::new();
    };
    let mut found: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((
                e.file_name().to_string_lossy().into_owned(),
                meta.modified().ok(),
                meta.len(),
            ))
        })
        .collect();
    found.sort();
    found
}

/// BR-002: the highest number ever given in the workspace.
fn highest_number(root: &Path) -> u64 {
    let recorded = fs::read_to_string(todos_folder(root).join(NUMBERS_FILE))
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let files = fs::read_dir(todos_folder(root))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    e.file_name()
                        .to_str()?
                        .strip_suffix(".yaml")?
                        .parse::<u64>()
                        .ok()
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    recorded.max(files)
}

fn record_number(root: &Path, number: u64) -> Result<(), TodoError> {
    if highest_number(root) > number {
        return Ok(());
    }
    write_atomic(
        &todos_folder(root).join(NUMBERS_FILE),
        format!("{number}\n").as_bytes(),
    )
    .map_err(write_error)
}

/// UC-020 steps 3–4, FR-062: creates a todo titled `title` with the next free
/// number, stored at once.
pub fn create_todo(root: &Path, title: &str) -> Result<Todo, TodoError> {
    ensure_folder(root)?;
    let mut number = highest_number(root) + 1;
    let todo = Todo::new(number, title)?;
    loop {
        let mut numbered = todo.clone();
        numbered.number = number;
        match write_new_atomic(&todo_path(root, number), numbered.to_text()?.as_bytes()) {
            Ok(()) => {
                record_number(root, number)?;
                return Ok(numbered);
            }
            // Another process took the number in between.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(err) => return Err(write_error(err)),
        }
    }
}

/// UC-020 step 6, BR-006: stores `todo`, whole.
pub fn save_todo(root: &Path, todo: &Todo) -> Result<(), TodoError> {
    check_title(&todo.title)?;
    ensure_folder(root)?;
    write_atomic(&todo_path(root, todo.number), todo.to_text()?.as_bytes()).map_err(write_error)?;
    record_number(root, todo.number)
}

/// UC-020 A5: removes todo `number`; returns it so it can be brought back.
pub fn delete_todo(root: &Path, number: u64) -> Result<Todo, TodoError> {
    let path = todo_path(root, number);
    let text = fs::read_to_string(&path).map_err(|_| TodoError::NoSuchTodo(number))?;
    let todo = Todo::from_text(number, &text)?;
    record_number(root, number)?;
    fs::remove_file(&path).map_err(write_error)?;
    Ok(todo)
}

/// UC-020 A5 step 2: brings a deleted todo back with its number.
pub fn restore_todo(root: &Path, todo: &Todo) -> Result<(), TodoError> {
    save_todo(root, todo)
}

/// UC-006 BR-006: the todo links to `from` follow the note to `to`; returns
/// how many todos changed.
pub fn relink_todos(root: &Path, from: &NoteId, to: &NoteId) -> Result<usize, TodoError> {
    let mut changed = 0;
    for mut todo in load_todos(root).todos {
        let mut moved = false;
        for link in &mut todo.links {
            if link.eq_ignore_ascii_case(from.as_str()) {
                *link = to.as_str().to_owned();
                moved = true;
            }
        }
        if moved {
            save_todo(root, &todo)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// The todos in conflict, and the numbers given anew as (old, new).
pub type TodoConflicts = (Vec<TodoConflict>, Vec<(u64, u64)>);

/// A todo changed on both sides of a sync (UC-020 A10).
#[derive(Debug, Clone, PartialEq)]
pub struct TodoConflict {
    pub number: u64,
    /// This computer's version and the remote's; `None` where deleted.
    pub mine: Option<Todo>,
    pub remote: Option<Todo>,
}

impl Git {
    /// The version of `path` at git's stage `stage`, if there is one.
    fn stage(&self, root: &Path, stage: u8, path: &str) -> Option<String> {
        self.run(root, &["show", &format!(":{stage}:{path}")]).ok()
    }

    /// UC-020 A10, A11, ADR-019: the todos in conflict among the files
    /// `conflicted` (relative paths). Two new todos of one number (A11) are
    /// solved at once: this computer's todo gets the next free number; the
    /// renumbering is returned as (old, new).
    pub fn todo_conflicts(
        &self,
        root: &Path,
        conflicted: &[String],
        joining: Joining,
    ) -> Result<TodoConflicts, TodoError> {
        let (mine_stage, remote_stage) = match joining {
            Joining::Merge => (2, 3),
            Joining::Rebase => (3, 2),
        };
        let mut conflicts = Vec::new();
        let mut renumbered = Vec::new();
        for path in conflicted {
            let Some(number) = todo_number_of(path) else {
                continue;
            };
            let read = |stage| {
                self.stage(root, stage, path)
                    .map(|text| Todo::from_text(number, &text))
                    .transpose()
            };
            let mine = read(mine_stage)?;
            let remote = read(remote_stage)?;
            let added_on_both = self.stage(root, 1, path).is_none();
            match (mine, remote) {
                (Some(mut mine), Some(remote)) if added_on_both && mine != remote => {
                    let new = highest_number(root).max(number) + 1;
                    mine.number = new;
                    save_todo(root, &mine)?;
                    save_todo(root, &remote)?;
                    self.mark_resolved(root, path)
                        .and_then(|()| self.mark_resolved(root, &todo_relative_path(new)))
                        .map_err(|e| TodoError::Git(e.to_string()))?;
                    renumbered.push((number, new));
                }
                (mine, remote) => conflicts.push(TodoConflict {
                    number,
                    mine,
                    remote,
                }),
            }
        }
        Ok((conflicts, renumbered))
    }

    /// UC-020 A10 step 3: keeps `version` of todo `number` (`None` keeps it
    /// deleted) and marks it resolved.
    pub fn resolve_todo(
        &self,
        root: &Path,
        number: u64,
        version: Option<&Todo>,
    ) -> Result<(), TodoError> {
        let path = todo_relative_path(number);
        match version {
            Some(todo) => {
                let mut todo = todo.clone();
                todo.number = number;
                save_todo(root, &todo)?;
            }
            None => {
                let _ = fs::remove_file(todo_path(root, number));
            }
        }
        self.mark_resolved(root, &path)
            .map_err(|e: GitError| TodoError::Git(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("underline-todos-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// TC-020-01, TC-020-02: created, filled, linked, done; stored at once.
    #[test]
    fn uc020_tc01_tc02_create_fill_link_complete() {
        let root = temp("main");
        let mut todo = create_todo(&root, "  Ship   release ").unwrap();
        assert_eq!((todo.number, todo.title.as_str()), (1, "Ship release"));
        assert_eq!(
            (todo.urgency, todo.status),
            (Urgency::Normal, TodoStatus::Open)
        );
        assert!(todo.due.is_none() && todo.scheduled.is_none() && !todo.created.is_empty());
        todo.due = Some("2026-11-20".parse().unwrap());
        todo.scheduled = Some("2026-11-18".parse().unwrap());
        todo.urgency = "high".parse().unwrap();
        assert!(todo.link(&NoteId::from_relative("Plan")));
        assert!(!todo.link(&NoteId::from_relative("plan")));
        save_todo(&root, &todo).unwrap();
        let stored = load_todos(&root);
        assert_eq!(stored.todos, vec![todo.clone()]);
        let text = fs::read_to_string(root.join(".todos/1.yaml")).unwrap();
        assert!(
            text.contains("due: 2026-11-20") && text.contains("- Plan"),
            "{text}"
        );
        assert_eq!(
            fs::read_to_string(root.join(".todos/.gitattributes")).unwrap(),
            "*.yaml -merge\n"
        );

        todo.set_done(true);
        assert!(todo.is_done() && todo.done.is_some());
        // A4: reopened.
        todo.set_done(false);
        assert!(!todo.is_done() && todo.done.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    /// TC-020-02, TC-020-04: days and titles that are not allowed.
    #[test]
    fn uc020_tc02_tc04_fields_checked() {
        assert!("2026-02-30".parse::<Day>().is_err());
        assert!("20.11.2026".parse::<Day>().is_err());
        assert_eq!(
            "2026-02-28".parse::<Day>().unwrap().to_string(),
            "2026-02-28"
        );
        assert!("urgent".parse::<Urgency>().is_err());
        let root = temp("titles");
        assert_eq!(create_todo(&root, "  "), Err(TodoError::NoTitle));
        assert_eq!(
            create_todo(&root, &"x".repeat(201)),
            Err(TodoError::LongTitle)
        );
        let mut todo = create_todo(&root, &"x".repeat(200)).unwrap();
        assert_eq!(todo.set_title(""), Err(TodoError::NoTitle));
        assert_eq!(todo.title.len(), 200);
        todo.due = Some("2026-11-10".parse().unwrap());
        todo.scheduled = Some("2026-11-12".parse().unwrap());
        assert!(todo.scheduled_after_due());
        fs::remove_dir_all(root).unwrap();
    }

    /// TC-020-06, BR-002: deleted and brought back; numbers not given again.
    #[test]
    fn uc020_tc06_delete_undo_numbers() {
        let root = temp("delete");
        for title in ["one", "two", "three"] {
            create_todo(&root, title).unwrap();
        }
        let mut two = load_todos(&root).get(2).cloned().unwrap();
        two.link(&NoteId::from_relative("Plan"));
        save_todo(&root, &two).unwrap();
        let deleted = delete_todo(&root, 2).unwrap();
        assert!(load_todos(&root).get(2).is_none());
        restore_todo(&root, &deleted).unwrap();
        assert_eq!(load_todos(&root).get(2), Some(&two));
        delete_todo(&root, 3).unwrap();
        assert_eq!(create_todo(&root, "four").unwrap().number, 4);
        assert_eq!(delete_todo(&root, 9), Err(TodoError::NoSuchTodo(9)));
        fs::remove_dir_all(root).unwrap();
    }

    /// TC-020-07: links follow a renamed note; todos linking a note.
    #[test]
    fn uc020_tc07_links_follow_rename() {
        let root = temp("rename");
        let mut todo = create_todo(&root, "Ship").unwrap();
        todo.link(&NoteId::from_relative("Plan"));
        todo.link(&NoteId::from_relative("Other"));
        save_todo(&root, &todo).unwrap();
        let plan = NoteId::from_relative("Plan");
        assert_eq!(load_todos(&root).linking(&plan).len(), 1);
        let moved = NoteId::from_relative("Work/Project-Plan");
        assert_eq!(relink_todos(&root, &plan, &moved).unwrap(), 1);
        let todos = load_todos(&root);
        assert_eq!(todos.todos[0].links, ["Work/Project-Plan", "Other"]);
        assert!(todos.linking(&plan).is_empty());
        assert!(todo.unlink("plan") && !todo.unlink("plan"));
        fs::remove_dir_all(root).unwrap();
    }

    /// Unknown keys stay; unreadable files are reported, not lost.
    fn todo(number: u64, due: Option<&str>, urgency: Urgency) -> Todo {
        let mut todo = Todo::new(number, "x").unwrap();
        todo.due = due.map(|d| d.parse().unwrap());
        todo.urgency = urgency;
        todo
    }

    fn numbers(list: &[&Todo]) -> Vec<u64> {
        list.iter().map(|t| t.number).collect()
    }

    /// TC-021-02, TC-021-03, TC-021-04: sorted and filtered.
    #[test]
    fn uc021_tc02_tc03_tc04_sort_and_filter() {
        let mut todos = vec![
            todo(1, Some("2026-11-20"), Urgency::Normal),
            todo(2, Some("2026-11-20"), Urgency::High),
            todo(3, None, Urgency::Critical),
            todo(4, Some("2026-11-15"), Urgency::Low),
            todo(5, None, Urgency::High),
        ];
        let open = TodoFilter::Open;
        assert_eq!(
            numbers(&todo_list(&todos, TodoSort::Due, open)),
            [4, 2, 1, 3, 5]
        );
        assert_eq!(
            numbers(&todo_list(&todos, TodoSort::Urgency, open)),
            [3, 2, 5, 1, 4]
        );
        todos[1].set_done(true);
        assert_eq!(
            numbers(&todo_list(&todos, TodoSort::Due, open)),
            [4, 1, 3, 5]
        );
        assert_eq!(
            numbers(&todo_list(&todos, TodoSort::Due, TodoFilter::Done)),
            [2]
        );
        assert_eq!(todo_list(&todos, TodoSort::Due, TodoFilter::All).len(), 5);
    }

    /// TC-021-05: overdue before today only, and only while open.
    #[test]
    fn uc021_tc05_overdue() {
        let today: Day = "2026-11-10".parse().unwrap();
        let mut todos = vec![
            todo(1, Some("2026-11-08"), Urgency::Normal),
            todo(2, Some("2026-11-10"), Urgency::Normal),
            todo(3, Some("2026-11-09"), Urgency::Normal),
            todo(4, None, Urgency::Normal),
        ];
        todos[2].set_done(true);
        assert_eq!(overdue_days(&todos[0], today), Some(2));
        assert_eq!(overdue_days(&todos[1], today), None);
        assert_eq!(overdue_count(&todos, today), 1);
        let year: Day = "2025-11-10".parse().unwrap();
        assert_eq!(today.days_since(year), 365);
    }

    /// TC-021-12: up to two linked notes and how many more.
    #[test]
    fn uc021_tc12_links_summary() {
        let mut todo = todo(1, None, Urgency::Normal);
        assert_eq!(links_summary(&todo), "");
        for note in ["Plan", "Ideas", "Road-Map", "Work/Notes"] {
            todo.link(&NoteId::from_relative(note));
        }
        assert_eq!(links_summary(&todo), "Plan, Ideas +2");
    }

    #[test]
    fn calendar_days() {
        let day: Day = "2026-11-18".parse().unwrap();
        assert_eq!(day.month_start().to_string(), "2026-11-01");
        assert_eq!(day.month_start().monday_offset(), 6);
        assert_eq!(day.days_in_month(), 30);
        assert_eq!(day.add_months(-11).to_string(), "2025-12-18");
        assert_eq!(day.with_day(3).to_string(), "2026-11-03");
        assert_eq!(day.month_title(), "November 2026");
    }

    #[test]
    fn unknown_keys_kept_and_broken_files_reported() {
        let root = temp("keys");
        fs::create_dir_all(root.join(".todos")).unwrap();
        fs::write(root.join(".todos/5.yaml"), "title: Later\ncolor: blue\n").unwrap();
        fs::write(root.join(".todos/6.yaml"), "title: [\n").unwrap();
        fs::write(root.join(".todos/notes.txt"), "x").unwrap();
        let todos = load_todos(&root);
        assert_eq!(todos.todos.len(), 1);
        assert_eq!(todos.unreadable.len(), 1);
        save_todo(&root, &todos.todos[0]).unwrap();
        assert!(fs::read_to_string(root.join(".todos/5.yaml"))
            .unwrap()
            .contains("color: blue"));
        assert_eq!(create_todo(&root, "next").unwrap().number, 7);
        assert_eq!(todo_number_of(".todos/12.yaml"), Some(12));
        assert_eq!(todo_number_of("Plan.md"), None);
        fs::remove_dir_all(root).unwrap();
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(
            out.status.success() || args[0] == "merge",
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn conflicted(dir: &Path) -> Vec<String> {
        let out = std::process::Command::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// TC-020-11, TC-020-12, TC-020-13: todos join per todo; one todo
    /// changed on both sides is a conflict; two new todos of one number are
    /// renumbered.
    #[test]
    fn uc020_tc11_tc12_tc13_joining_todos() {
        let root = temp("git");
        git(&root, &["init", "-q", "-b", "main"]);
        let one = create_todo(&root, "Ship").unwrap();
        let two = create_todo(&root, "Review").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "base"]);
        git(&root, &["checkout", "-qb", "remote"]);
        let mut remote_one = one.clone();
        remote_one.due = Some("2026-12-01".parse().unwrap());
        save_todo(&root, &remote_one).unwrap();
        let mut remote_two = two.clone();
        remote_two.urgency = Urgency::Low;
        save_todo(&root, &remote_two).unwrap();
        let remote_new = create_todo(&root, "Remote").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "remote"]);
        git(&root, &["checkout", "-q", "main"]);
        let mut mine_one = one.clone();
        mine_one.urgency = Urgency::High;
        save_todo(&root, &mine_one).unwrap();
        // .numbers says 2 here; both sides add #3.
        let mut mine_new = create_todo(&root, "Mine").unwrap();
        mine_new.link(&NoteId::from_relative("Plan"));
        save_todo(&root, &mine_new).unwrap();
        assert_eq!(mine_new.number, remote_new.number);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "mine"]);
        git(&root, &["merge", "-q", "remote"]);

        let files = conflicted(&root);
        assert!(files.contains(&".todos/1.yaml".to_owned()), "{files:?}");
        assert!(!files.contains(&".todos/2.yaml".to_owned()), "{files:?}");
        let (conflicts, renumbered) = Git::new()
            .todo_conflicts(&root, &files, Joining::Merge)
            .unwrap();
        assert_eq!(renumbered, vec![(3, 4)]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].mine.as_ref().unwrap().urgency, Urgency::High);
        assert_eq!(conflicts[0].remote.as_ref().unwrap().due, remote_one.due);
        // Changed on one side only: taken over.
        assert_eq!(load_todos(&root).get(2).unwrap().urgency, Urgency::Low);
        let todos = load_todos(&root);
        assert_eq!(todos.get(3).unwrap().title, "Remote");
        assert_eq!(todos.get(4).unwrap().title, "Mine");
        assert_eq!(todos.get(4).unwrap().links, ["Plan"]);

        Git::new()
            .resolve_todo(&root, 1, conflicts[0].remote.as_ref())
            .unwrap();
        assert_eq!(load_todos(&root).get(1).unwrap().due, remote_one.due);
        assert!(conflicted(&root).iter().all(|f| !f.starts_with(".todos/")));
        fs::remove_dir_all(root).unwrap();
    }
}
