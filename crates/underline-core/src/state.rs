//! The state Underline keeps between two starts (ADR-008): per user the known
//! workspaces and the recently run commands of the palette (user-wide, in
//! every workspace: UC-023 BR-004, OQ-198), per workspace its open tabs, the
//! width of its split view (OQ-220), its tab "Todos" and when its notes were
//! last opened (FR-071, UC-001, UC-002).
//!
//! A field of a state file whose value has an unexpected form is read
//! loosely ([`Loose`]): it falls back to its default, and the rest of the
//! file still counts.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::panes::PaneSide;
use crate::todos::TodoListState;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::loose::Loose;
use crate::note::NoteId;

/// File name of both state files: in the user's data directory and in the
/// Underline folder of a workspace.
pub const STATE_FILE_NAME: &str = "state.yaml";
/// The hidden folder of a workspace that holds its state (ADR-008).
pub const UNDERLINE_FOLDER: &str = ".underline";
/// Content of the `.gitignore` in the Underline folder: git ignores the whole
/// folder, without a change to the repository's own `.gitignore`.
pub const UNDERLINE_FOLDER_GITIGNORE: &str = "*\n";
/// A change is written this long after it happened, together with the changes
/// that follow within that time (ADR-008: at most 1 second, NFR-009).
pub const STATE_WRITE_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("cannot write {}: {source}", .path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("cannot store the app state: {0}")]
    Encode(String),
}

/// Milliseconds since 1970-01-01 UTC.
pub type Timestamp = u64;

/// The current time as a [`Timestamp`].
pub fn now() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// UC-023 BR-004, OQ-198: how many recently run commands the user's state
/// keeps.
pub const RECENT_COMMANDS: usize = 20;
/// The longest id of a command kept; a longer one is no command's.
const COMMAND_ID_MAX: usize = 100;

/// What the user's data directory keeps: the known workspaces and the
/// recently run commands of the palette.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub workspaces: Vec<KnownWorkspace>,
    /// UC-023 BR-004, OQ-198: the ids of the commands run from the
    /// palette, the most recent first, at most [`RECENT_COMMANDS`]; for the
    /// user, in every workspace. A state file without them (written before)
    /// or with entries that are no id reads as far as it can.
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "lenient_commands"
    )]
    pub recent_commands: Vec<String>,
}

/// UC-023 BR-004, OQ-198: whether `id` can be a command's id
/// (`file.newNote`): short, of letters, digits, `.`, `-` and `_`.
fn is_command_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= COMMAND_ID_MAX
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// OQ-198: the recently run commands of a state file: the entries that are
/// ids, without repeats, at most [`RECENT_COMMANDS`]; anything else reads
/// as none, so the rest of the file still counts.
fn lenient_commands<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    Loose::read(deserializer, |value| {
        let Loose::List(entries) = value else {
            return Vec::new();
        };
        let mut ids: Vec<String> = Vec::new();
        for entry in entries {
            if let Loose::Text(id) = entry {
                if is_command_id(&id) && !ids.contains(&id) && ids.len() < RECENT_COMMANDS {
                    ids.push(id);
                }
            }
        }
        ids
    })
}

/// One known workspace (entity WORKSPACE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownWorkspace {
    pub directory: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened: Option<Timestamp>,
}

/// What the Underline folder of a workspace keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceState {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<TabState>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NoteState>,
    /// UC-021 BR-005: the tab "Todos", its sort and its filter.
    #[serde(default, skip_serializing_if = "TodoListState::is_default")]
    pub todo_list: TodoListState,
    /// UC-004 BR-018, OQ-220: the share of the split view's width the left
    /// pane takes, in tenths of a percent (500 is half); `None` (a state
    /// file written before, or a value that is no number) is half. Kept
    /// within [`SPLIT_SHARE_LEAST`] and its counterpart.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_split_share"
    )]
    pub split_share: Option<u16>,
}

/// UC-004 BR-018, OQ-220, OQ-278: the least share of the split view each
/// pane keeps, in tenths of a percent (15 percent); the window keeps at
/// least 280 px besides.
pub const SPLIT_SHARE_LEAST: u16 = 150;
/// OQ-220: the share of the split view's left pane without one remembered:
/// half.
pub const SPLIT_SHARE_EVEN: u16 = 500;

/// OQ-220: `share` (tenths of a percent of the left pane) within the least
/// share of each pane.
#[must_use]
pub fn split_share_within(share: f64) -> u16 {
    if !share.is_finite() {
        return SPLIT_SHARE_EVEN;
    }
    let least = f64::from(SPLIT_SHARE_LEAST);
    let most = 1000.0 - least;
    // Within 150..=850, so the cast loses nothing.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let within = share.clamp(least, most).round() as u16;
    within
}

/// OQ-220: the remembered share of a state file, within the least share of
/// each pane; anything that is no number reads as none, so the rest of the
/// file still counts.
fn lenient_split_share<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u16>, D::Error> {
    Loose::read(deserializer, |value| match value {
        Loose::Number(share) if share.is_finite() => Some(split_share_within(share)),
        _ => None,
    })
}

/// One open tab (entity OPEN_TAB).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabState {
    pub note: NoteId,
    /// UC-004 BR-018: the pane of the split view the tab is in; the left one
    /// without the split view (and in a state file written before it).
    #[serde(default, skip_serializing_if = "is_left")]
    pub pane: PaneSide,
    /// Place of the tab from the left of its pane, starting at 1.
    pub position: u32,
    pub editing_view: EditingView,
    pub active: bool,
}

/// The left pane is not written, so a state file without the split view
/// stays as it was.
fn is_left(pane: &PaneSide) -> bool {
    *pane == PaneSide::Left
}

/// The editing view a tab shows its note in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditingView {
    Source,
    Facade,
}

/// When a note was last opened (entity NOTE.last_opened).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteState {
    pub note: NoteId,
    pub last_opened: Timestamp,
}

/// Something the user should be told about a state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateNotice {
    /// The file could not be read; it was set aside under this name and
    /// Underline continues with an empty state.
    SetAside(PathBuf),
    /// The Underline folder of this workspace cannot be written, so its tabs
    /// and recently opened notes are not remembered (UC-002 A12). Given once
    /// per workspace.
    CannotRemember(PathBuf),
}

impl std::fmt::Display for StateNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateNotice::SetAside(path) => write!(
                f,
                "the remembered workspaces and tabs could not be read and were set aside as {}",
                path.display()
            ),
            StateNotice::CannotRemember(dir) => write!(
                f,
                "the tabs and recently opened notes of {} cannot be remembered, because its Underline folder cannot be written",
                dir.display()
            ),
        }
    }
}

/// The state of one workspace with whether it waits to be written.
#[derive(Debug)]
struct Loaded {
    state: WorkspaceState,
    changed_at: Option<Instant>,
    /// Writing failed before and the user was told (UC-002 A12).
    unwritable: bool,
}

/// The state with the files it is kept in.
#[derive(Debug)]
pub struct StateStore {
    /// Path of the user's state file; `None` keeps everything in memory.
    path: Option<PathBuf>,
    state: AppState,
    changed_at: Option<Instant>,
    workspaces: HashMap<PathBuf, Loaded>,
    notices: Vec<StateNotice>,
}

impl StateStore {
    /// `state.yaml` in the user's data directory (ADR-008).
    pub fn default_path() -> Option<PathBuf> {
        directories::BaseDirs::new()
            .map(|dirs| dirs.data_dir().join("underline").join(STATE_FILE_NAME))
    }

    /// A store that never reads or writes a file.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            state: AppState::default(),
            changed_at: None,
            workspaces: HashMap::new(),
            notices: Vec::new(),
        }
    }

    /// Reads the user's state file at `path` and drops workspaces that are
    /// gone. A missing file gives an empty state; a file that cannot be read
    /// is set aside, and the notice says so.
    pub fn load(path: PathBuf) -> (Self, Option<StateNotice>) {
        let (state, notice) = read_state::<AppState>(&path);
        let mut store = Self {
            path: Some(path),
            state,
            changed_at: None,
            workspaces: HashMap::new(),
            notices: Vec::new(),
        };
        let before = store.state.workspaces.len();
        store.state.workspaces.retain(|w| w.directory.is_dir());
        if store.state.workspaces.len() != before {
            store.changed_at = Some(Instant::now());
        }
        (store, notice)
    }

    /// The known workspaces, as kept in the user's data directory.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Notices about state files of workspaces read since the last call.
    pub fn take_notices(&mut self) -> Vec<StateNotice> {
        std::mem::take(&mut self.notices)
    }

    /// UC-002 step 2: the known workspaces, the most recently opened first.
    pub fn recent_workspaces(&self) -> Vec<PathBuf> {
        let mut known: Vec<_> = self.state.workspaces.iter().collect();
        known.sort_by_key(|w| std::cmp::Reverse(w.last_opened));
        known.into_iter().map(|w| w.directory.clone()).collect()
    }

    /// UC-002 A3: forgets a workspace whose directory is gone.
    pub fn forget_workspace(&mut self, directory: &Path) {
        let key = key(directory);
        let before = self.state.workspaces.len();
        self.state.workspaces.retain(|w| w.directory != key);
        self.workspaces.remove(&key);
        if self.state.workspaces.len() != before {
            self.changed_at.get_or_insert_with(Instant::now);
        }
    }

    /// The workspace opened most recently.
    pub fn last_workspace(&self) -> Option<&Path> {
        self.state
            .workspaces
            .iter()
            .filter(|w| w.last_opened.is_some())
            .max_by_key(|w| w.last_opened)
            .map(|w| w.directory.as_path())
    }

    /// The remembered tabs of the workspace at `directory`, those of the
    /// left pane first, each pane's left to right (UC-004 BR-018).
    pub fn tabs(&mut self, directory: &Path) -> Vec<TabState> {
        let mut tabs = self.workspace(directory).state.tabs.clone();
        tabs.sort_by_key(|tab| (tab.pane, tab.position));
        tabs
    }

    /// The note of the active tab of the workspace at `directory`: of the
    /// left pane, which is the active pane after a restart (BR-018).
    pub fn active_note(&mut self, directory: &Path) -> Option<NoteId> {
        self.workspace(directory)
            .state
            .tabs
            .iter()
            .find(|tab| tab.active && tab.pane == PaneSide::Left)
            .map(|tab| tab.note.clone())
    }

    /// When the note `note` of the workspace at `directory` was last opened.
    pub fn note_last_opened(&mut self, directory: &Path, note: &NoteId) -> Option<Timestamp> {
        self.workspace(directory)
            .state
            .notes
            .iter()
            .find(|n| &n.note == note)
            .map(|n| n.last_opened)
    }

    /// Records that the workspace at `directory` was opened (UC-001, UC-002:
    /// remembered as the last opened one). Its tabs stay.
    pub fn workspace_opened(&mut self, directory: &Path, at: Timestamp) {
        let key = key(directory);
        match self
            .state
            .workspaces
            .iter_mut()
            .find(|w| w.directory == key)
        {
            Some(known) => known.last_opened = Some(at),
            None => self.state.workspaces.push(KnownWorkspace {
                directory: key,
                last_opened: Some(at),
            }),
        }
        self.changed_at.get_or_insert_with(Instant::now);
    }

    /// Records that `note` was opened in the active tab of the workspace at
    /// `directory`.
    ///
    /// Underline shows one note at a time until tabs exist, so the workspace
    /// keeps exactly one tab, which is active.
    pub fn note_opened(
        &mut self,
        directory: &Path,
        note: &NoteId,
        view: EditingView,
        at: Timestamp,
    ) {
        let loaded = self.workspace(directory);
        loaded.state.tabs = vec![TabState {
            note: note.clone(),
            pane: PaneSide::Left,
            position: 1,
            editing_view: view,
            active: true,
        }];
        match loaded.state.notes.iter_mut().find(|n| &n.note == note) {
            Some(entry) => entry.last_opened = at,
            None => loaded.state.notes.push(NoteState {
                note: note.clone(),
                last_opened: at,
            }),
        }
        loaded.changed_at.get_or_insert_with(Instant::now);
    }

    /// Records when `note` was last opened, for the note overview (UC-004
    /// step 2), without changing the remembered tabs.
    pub fn note_viewed(&mut self, directory: &Path, note: &NoteId, at: Timestamp) {
        let loaded = self.workspace(directory);
        match loaded.state.notes.iter_mut().find(|n| &n.note == note) {
            Some(entry) => entry.last_opened = at,
            None => loaded.state.notes.push(NoteState {
                note: note.clone(),
                last_opened: at,
            }),
        }
        loaded.changed_at.get_or_insert_with(Instant::now);
    }

    /// UC-006 step 7: the note `from` of the workspace at `directory` is now
    /// named `to`; its remembered tab and when it was last opened follow it.
    pub fn note_renamed(&mut self, directory: &Path, from: &NoteId, to: &NoteId) {
        let loaded = self.workspace(directory);
        loaded.state.notes.retain(|n| &n.note != to);
        for entry in loaded.state.notes.iter_mut().filter(|n| &n.note == from) {
            entry.note = to.clone();
        }
        for tab in loaded.state.tabs.iter_mut().filter(|t| &t.note == from) {
            tab.note = to.clone();
        }
        loaded.changed_at.get_or_insert_with(Instant::now);
    }

    /// UC-007 step 5: the note `note` of the workspace at `directory` was
    /// deleted; it is no longer a remembered tab or recently opened note.
    pub fn note_deleted(&mut self, directory: &Path, note: &NoteId) {
        let loaded = self.workspace(directory);
        loaded.state.notes.retain(|n| &n.note != note);
        loaded.state.tabs.retain(|t| &t.note != note);
        loaded.changed_at.get_or_insert_with(Instant::now);
    }

    /// UC-021 BR-005: the todo list of the workspace at `directory`.
    pub fn todo_list(&mut self, directory: &Path) -> TodoListState {
        self.workspace(directory).state.todo_list
    }

    /// UC-021 BR-005: keeps the todo list of the workspace at `directory`.
    pub fn set_todo_list(&mut self, directory: &Path, list: TodoListState) {
        let loaded = self.workspace(directory);
        if loaded.state.todo_list != list {
            loaded.state.todo_list = list;
            loaded.changed_at.get_or_insert_with(Instant::now);
        }
    }

    /// UC-004 BR-018, OQ-220: the share of the split view the left pane of
    /// the workspace at `directory` takes, in tenths of a percent; half
    /// when none is remembered.
    pub fn split_share(&mut self, directory: &Path) -> u16 {
        self.workspace(directory)
            .state
            .split_share
            .unwrap_or(SPLIT_SHARE_EVEN)
    }

    /// UC-004 BR-018, OQ-220: remembers the share of the split view the
    /// left pane of the workspace at `directory` takes, within the least
    /// share of each pane; returns it.
    pub fn set_split_share(&mut self, directory: &Path, share: f64) -> u16 {
        let share = split_share_within(share);
        let loaded = self.workspace(directory);
        if loaded.state.split_share != Some(share) {
            loaded.state.split_share = Some(share);
            loaded.changed_at.get_or_insert_with(Instant::now);
        }
        share
    }

    /// UC-023 BR-004, OQ-198: the recently run commands, the most recent
    /// first.
    pub fn recent_commands(&self) -> &[String] {
        &self.state.recent_commands
    }

    /// UC-023 BR-004, OQ-198: the command `id` ran from the palette: it is
    /// the most recent one now; the oldest beyond [`RECENT_COMMANDS`] is
    /// forgotten. What is no command's id is not kept.
    pub fn command_ran(&mut self, id: &str) {
        if !is_command_id(id) {
            return;
        }
        let recent = &mut self.state.recent_commands;
        if recent.first().map(String::as_str) == Some(id) {
            return;
        }
        recent.retain(|known| known != id);
        recent.insert(0, id.to_owned());
        recent.truncate(RECENT_COMMANDS);
        self.changed_at.get_or_insert_with(Instant::now);
    }

    /// Remembers the open tabs of the workspace at `directory`, left to
    /// right (UC-004 step 6, A5); positions start at 1.
    pub fn set_tabs(&mut self, directory: &Path, tabs: Vec<TabState>) {
        let loaded = self.workspace(directory);
        if loaded.state.tabs != tabs {
            loaded.state.tabs = tabs;
            loaded.changed_at.get_or_insert_with(Instant::now);
        }
    }

    /// Whether a change waits to be written.
    pub fn has_unwritten_changes(&self) -> bool {
        self.changed_at.is_some() || self.workspaces.values().any(|w| w.changed_at.is_some())
    }

    /// Writes what has waited [`STATE_WRITE_DELAY`]; returns whether it wrote.
    pub fn write_if_due(&mut self, now: Instant) -> Result<bool, StateError> {
        let due = |changed: Option<Instant>| {
            changed.is_some_and(|c| now.duration_since(c) >= STATE_WRITE_DELAY)
        };
        let any_due = due(self.changed_at) || self.workspaces.values().any(|w| due(w.changed_at));
        if any_due {
            self.write()?;
        }
        Ok(any_due)
    }

    /// Writes every change now, atomically (ADR-005).
    pub fn write(&mut self) -> Result<(), StateError> {
        let Some(path) = self.path.clone() else {
            self.changed_at = None;
            self.workspaces
                .values_mut()
                .for_each(|w| w.changed_at = None);
            return Ok(());
        };
        for (directory, loaded) in &mut self.workspaces {
            if loaded.changed_at.take().is_some()
                && write_workspace_state(directory, &loaded.state).is_err()
                && !loaded.unwritable
            {
                // UC-002 A12: the workspace stays open; the user is told once.
                loaded.unwritable = true;
                self.notices
                    .push(StateNotice::CannotRemember(directory.clone()));
            }
        }
        if self.changed_at.is_some() {
            write_yaml(&path, &self.state)?;
            self.changed_at = None;
        }
        Ok(())
    }

    /// The state of the workspace at `directory`, read from its Underline
    /// folder the first time; notes that are gone are dropped.
    fn workspace(&mut self, directory: &Path) -> &mut Loaded {
        let key = key(directory);
        let persistent = self.path.is_some();
        let notices = &mut self.notices;
        self.workspaces.entry(key.clone()).or_insert_with(|| {
            if !persistent {
                return Loaded {
                    state: WorkspaceState::default(),
                    changed_at: None,
                    unwritable: false,
                };
            }
            let file = key.join(UNDERLINE_FOLDER).join(STATE_FILE_NAME);
            let (mut state, notice) = read_state::<WorkspaceState>(&file);
            notices.extend(notice);
            let pruned = prune(&key, &mut state);
            Loaded {
                state,
                changed_at: pruned.then(Instant::now),
                unwritable: false,
            }
        })
    }
}

/// Drops notes and tabs whose file is gone; returns whether anything changed.
fn prune(directory: &Path, state: &mut WorkspaceState) -> bool {
    let before = state.clone();
    let exists = |note: &NoteId| {
        NoteFormat::ALL
            .into_iter()
            .any(|format| directory.join(note.file_name(format)).is_file())
    };
    state.notes.retain(|n| exists(&n.note));
    state.tabs.retain(|t| exists(&t.note));
    state.tabs.sort_by_key(|tab| (tab.pane, tab.position));
    // UC-004 BR-018: places and the active tab count per pane.
    for side in [PaneSide::Left, PaneSide::Right] {
        let mut position = 0;
        let mut first = None;
        let mut any_active = false;
        for (index, tab) in state.tabs.iter_mut().enumerate() {
            if tab.pane == side {
                position += 1;
                tab.position = position;
                first.get_or_insert(index);
                any_active |= tab.active;
            }
        }
        if let (Some(first), false) = (first, any_active) {
            state.tabs[first].active = true;
        }
    }
    *state != before
}

/// Writes the state of a workspace into its Underline folder, creating the
/// folder and the `.gitignore` that makes git ignore it (ADR-008).
fn write_workspace_state(directory: &Path, state: &WorkspaceState) -> Result<(), StateError> {
    let folder = underline_folder(directory).map_err(|source| StateError::Write {
        path: directory.join(UNDERLINE_FOLDER).join(".gitignore"),
        source,
    })?;
    write_yaml(&folder.join(STATE_FILE_NAME), state)
}

/// The Underline folder of the workspace at `directory`, created with the
/// `.gitignore` that makes git ignore it when it is missing (ADR-008).
pub(crate) fn underline_folder(directory: &Path) -> io::Result<PathBuf> {
    let folder = directory.join(UNDERLINE_FOLDER);
    let gitignore = folder.join(".gitignore");
    fs::create_dir_all(&folder)?;
    if fs::read_to_string(&gitignore).ok().as_deref() != Some(UNDERLINE_FOLDER_GITIGNORE) {
        write_atomic(&gitignore, UNDERLINE_FOLDER_GITIGNORE.as_bytes())?;
    }
    Ok(folder)
}

fn write_yaml<T: Serialize>(path: &Path, value: &T) -> Result<(), StateError> {
    let text = serde_norway::to_string(value).map_err(|err| StateError::Encode(err.to_string()))?;
    let write = || -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        write_atomic(path, text.as_bytes())
    };
    write().map_err(|source| StateError::Write {
        path: path.to_owned(),
        source,
    })
}

/// Reads a state file; a missing or empty file gives the default, a file that
/// cannot be read is set aside.
fn read_state<T: DeserializeOwned + Default>(path: &Path) -> (T, Option<StateNotice>) {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => (T::default(), None),
        Ok(text) => match serde_norway::from_str(&text) {
            Ok(state) => (state, None),
            Err(_) => (T::default(), set_aside(path)),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => (T::default(), None),
        Err(_) => (T::default(), set_aside(path)),
    }
}

/// The form a workspace directory is remembered under.
fn key(directory: &Path) -> PathBuf {
    std::path::absolute(directory).unwrap_or_else(|_| directory.to_owned())
}

/// Renames a state file that cannot be read to `state.yaml.damaged`.
fn set_aside(path: &Path) -> Option<StateNotice> {
    let mut damaged = path.as_os_str().to_owned();
    damaged.push(".damaged");
    let damaged = PathBuf::from(damaged);
    fs::rename(path, &damaged)
        .ok()
        .map(|()| StateNotice::SetAside(damaged))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("underline-state-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn workspace(&self, name: &str, notes: &[&str]) -> PathBuf {
            let dir = self.0.join(name);
            fs::create_dir_all(&dir).unwrap();
            for note in notes {
                fs::write(dir.join(note), "").unwrap();
            }
            dir
        }

        fn state_file(&self) -> PathBuf {
            self.0.join("data").join(STATE_FILE_NAME)
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn home() -> NoteId {
        NoteId::from_title("Home")
    }

    /// UC-001 postcondition / TC-001-01 step 6: the new workspace is
    /// remembered as the last opened one, with its Home tab, after a restart.
    #[test]
    fn uc001_remembers_last_opened_workspace_and_tab_across_restart() {
        let sandbox = Sandbox::new("restart");
        let old = sandbox.workspace("old", &["A.md"]);
        let new = sandbox.workspace("notes-a", &["Home.md"]);

        let (mut store, notice) = StateStore::load(sandbox.state_file());
        assert_eq!(notice, None);
        store.workspace_opened(&old, 1_000);
        store.note_opened(&old, &NoteId::from_title("A"), EditingView::Source, 1_001);
        store.workspace_opened(&new, 2_000);
        store.note_opened(&new, &home(), EditingView::Facade, 2_001);
        store.write().unwrap();

        // ADR-008: tabs and recent notes live in the workspace, ignored by git.
        let folder = new.join(UNDERLINE_FOLDER);
        assert!(folder.join(STATE_FILE_NAME).is_file());
        assert_eq!(
            fs::read_to_string(folder.join(".gitignore")).unwrap(),
            UNDERLINE_FOLDER_GITIGNORE
        );
        assert!(!fs::read_to_string(sandbox.state_file())
            .unwrap()
            .contains("Home"));

        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.last_workspace(), Some(new.as_path()));
        assert_eq!(store.active_note(&new), Some(home()));
        assert_eq!(store.tabs(&new)[0].editing_view, EditingView::Facade);
        assert_eq!(store.note_last_opened(&new, &home()), Some(2_001));
        // TC-001-04: the previous workspace keeps its tab.
        assert_eq!(store.active_note(&old), Some(NoteId::from_title("A")));
    }

    /// TC-021-10, UC-021 BR-005: the todo list is kept per workspace, in
    /// the Underline folder, across a restart.
    #[test]
    fn uc021_tc10_todo_list_kept_per_workspace() {
        use crate::todos::{TodoFilter, TodoSort};
        let sandbox = Sandbox::new("todo-list");
        let a = sandbox.workspace("a", &["A.md"]);
        let b = sandbox.workspace("b", &["B.md"]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        let list = TodoListState {
            open: true,
            active: true,
            sort: TodoSort::Urgency,
            filter: TodoFilter::All,
            pane: PaneSide::Left,
        };
        store.set_todo_list(&a, list);
        store.write().unwrap();
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.todo_list(&a), list);
        assert_eq!(store.todo_list(&b), TodoListState::default());
        assert!(
            fs::read_to_string(a.join(UNDERLINE_FOLDER).join(STATE_FILE_NAME))
                .unwrap()
                .contains("urgency")
        );
    }

    /// OQ-279, TC-021-10 step 5: the pane of the tab "Todos" is kept; a
    /// state file without one (written before) reads as the left pane, and
    /// the left pane is not written.
    #[test]
    fn oq279_tc021_10_todo_tab_pane_kept_and_left_by_default() {
        let sandbox = Sandbox::new("todo-pane");
        let a = sandbox.workspace("a", &["A.md"]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        let mut list = TodoListState {
            open: true,
            active: true,
            pane: PaneSide::Right,
            ..TodoListState::default()
        };
        store.set_todo_list(&a, list);
        store.write().unwrap();
        let file = a.join(UNDERLINE_FOLDER).join(STATE_FILE_NAME);
        assert!(fs::read_to_string(&file).unwrap().contains("pane: right"));
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.todo_list(&a), list);

        list.pane = PaneSide::Left;
        store.set_todo_list(&a, list);
        store.write().unwrap();
        assert!(!fs::read_to_string(&file).unwrap().contains("pane"));

        let old: WorkspaceState =
            serde_norway::from_str("todo_list:\n  open: true\n  active: true\n").unwrap();
        assert_eq!(old.todo_list.pane, PaneSide::Left);
    }

    /// OQ-220, UC-004 BR-018, TC-004-37 step 4: the width of the split view
    /// is kept per workspace across a restart; without one it is half.
    #[test]
    fn oq220_tc004_37_split_share_kept_per_workspace() {
        let sandbox = Sandbox::new("split-share");
        let a = sandbox.workspace("a", &["A.md"]);
        let b = sandbox.workspace("b", &["B.md"]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.split_share(&a), SPLIT_SHARE_EVEN);
        assert_eq!(store.set_split_share(&a, 623.4), 623);
        store.write().unwrap();
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.split_share(&a), 623);
        assert_eq!(store.split_share(&b), 500);
        assert!(
            fs::read_to_string(a.join(UNDERLINE_FOLDER).join(STATE_FILE_NAME))
                .unwrap()
                .contains("split_share: 623")
        );
    }

    /// OQ-220: a share beyond the least width of a pane is kept within it
    /// (15 percent each); one that is no number is half.
    #[test]
    fn oq220_split_share_kept_within_the_least_share() {
        assert_eq!(split_share_within(0.0), SPLIT_SHARE_LEAST);
        assert_eq!(split_share_within(-5.0), 150);
        assert_eq!(split_share_within(990.0), 850);
        assert_eq!(split_share_within(f64::NAN), 500);
        assert_eq!(split_share_within(f64::INFINITY), 500);
        assert_eq!(split_share_within(499.6), 500);
    }

    /// OQ-220, UC-004 step 6: a state file written before the width was
    /// remembered, or with a width that is out of bounds or no number,
    /// still gives its tabs; the width is half, or within the bounds.
    #[test]
    fn oq220_old_and_odd_state_files_keep_their_tabs() {
        let tabs = "tabs:\n- note: Plan\n  position: 1\n  editing_view: source\n  active: true\n";
        let old: WorkspaceState = serde_norway::from_str(tabs).unwrap();
        assert_eq!(old.split_share, None);
        assert_eq!(old.tabs.len(), 1);
        for (value, share) in [
            ("2000", Some(850)),
            ("-1", Some(150)),
            ("612.7", Some(613)),
            ("wide", None),
            ("[1, 2]", None),
            ("~", None),
        ] {
            let state: WorkspaceState =
                serde_norway::from_str(&format!("{tabs}split_share: {value}\n")).unwrap();
            assert_eq!(state.split_share, share, "{value}");
            assert_eq!(state.tabs.len(), 1, "{value}");
        }
        let written = serde_norway::to_string(&old).unwrap();
        assert!(!written.contains("split_share"));
    }

    /// OQ-198, UC-023 BR-004, TC-023-02 step 4: the recently run commands
    /// are the user's, kept across a restart, the most recent first, at
    /// most 20, each once.
    #[test]
    fn oq198_tc023_02_recent_commands_kept_for_the_user() {
        let sandbox = Sandbox::new("recent-commands");
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert!(store.recent_commands().is_empty());
        store.command_ran("file.newNote");
        store.command_ran("view.toggleFocus");
        store.command_ran("file.newNote");
        assert_eq!(
            store.recent_commands(),
            ["file.newNote", "view.toggleFocus"]
        );
        store.write().unwrap();
        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(
            store.recent_commands(),
            ["file.newNote", "view.toggleFocus"]
        );

        for n in 0..30 {
            store.command_ran(&format!("test.command{n}"));
        }
        assert_eq!(store.recent_commands().len(), RECENT_COMMANDS);
        assert_eq!(store.recent_commands()[0], "test.command29");
        assert_eq!(store.recent_commands()[19], "test.command10");
    }

    /// OQ-198: what is no command's id is not kept, and running the most
    /// recent command again changes nothing to write.
    #[test]
    fn oq198_recent_commands_take_only_ids() {
        let mut store = StateStore::in_memory();
        store.command_ran("");
        store.command_ran("has space");
        store.command_ran("../../etc");
        store.command_ran(&"x".repeat(101));
        assert!(store.recent_commands().is_empty());
        store.command_ran("file.newNote");
        store.write().unwrap();
        store.command_ran("file.newNote");
        assert!(!store.has_unwritten_changes());
    }

    /// OQ-198: a state file written before, or with recent commands that
    /// are odd, still gives its workspaces; what is no id is dropped, and
    /// repeats and entries beyond 20 too.
    #[test]
    fn oq198_old_and_odd_state_files_keep_their_workspaces() {
        let workspaces = "workspaces:\n- directory: /notes\n";
        let old: AppState = serde_norway::from_str(workspaces).unwrap();
        assert!(old.recent_commands.is_empty());
        assert!(!serde_norway::to_string(&old)
            .unwrap()
            .contains("recent_commands"));
        let many: Vec<String> = (0..25).map(|n| format!("c.n{n}")).collect();
        for (value, expected) in [
            ("7", Vec::new()),
            ("wide", Vec::new()),
            (
                "[file.newNote, 3, {a: b}, 'bad id', file.newNote, view.x]",
                vec!["file.newNote".to_owned(), "view.x".to_owned()],
            ),
            (&format!("[{}]", many.join(", ")), many[..20].to_vec()),
        ] {
            let state: AppState =
                serde_norway::from_str(&format!("{workspaces}recent_commands: {value}\n")).unwrap();
            assert_eq!(state.recent_commands, expected, "{value}");
            assert_eq!(state.workspaces.len(), 1, "{value}");
        }
    }

    /// TC-006-15: the tab and the recent note follow a renamed note.
    #[test]
    fn uc006_tc15_tab_and_recent_note_follow_the_rename() {
        let sandbox = Sandbox::new("rename");
        let dir = sandbox.workspace("w", &["Plan.md", "Road-Map.md"]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        let (plan, road) = (NoteId::from_title("Plan"), NoteId::from_title("Road Map"));
        store.note_viewed(&dir, &road, 5);
        store.note_opened(&dir, &plan, EditingView::Source, 10);
        store.note_renamed(&dir, &plan, &road);
        assert_eq!(store.active_note(&dir), Some(road.clone()));
        assert_eq!(store.note_last_opened(&dir, &road), Some(10));
        assert_eq!(store.note_last_opened(&dir, &plan), None);
    }

    /// ADR-008: a change is written after the write delay, not before.
    #[test]
    fn writes_changes_after_the_delay() {
        let sandbox = Sandbox::new("delay");
        let dir = sandbox.workspace("w", &[]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        store.workspace_opened(&dir, 1);

        let start = Instant::now();
        assert!(!store.write_if_due(start).unwrap() || start.elapsed() >= STATE_WRITE_DELAY);
        assert!(store.write_if_due(start + STATE_WRITE_DELAY * 2).unwrap());
        assert!(sandbox.state_file().is_file());
        assert!(!store.has_unwritten_changes());
        assert!(!store.write_if_due(start + STATE_WRITE_DELAY * 4).unwrap());
    }

    /// ADR-008: workspaces and notes that are gone are dropped at start.
    #[test]
    fn drops_missing_workspaces_and_notes_at_start() {
        let sandbox = Sandbox::new("prune");
        let kept = sandbox.workspace("kept", &["Plan.adoc", "Gone.md"]);
        let removed = sandbox.workspace("removed", &[]);
        let (mut store, _) = StateStore::load(sandbox.state_file());
        store.workspace_opened(&removed, 2);
        store.workspace_opened(&kept, 1);
        store.note_opened(&kept, &NoteId::from_title("Plan"), EditingView::Source, 3);
        store.note_opened(&kept, &NoteId::from_title("Gone"), EditingView::Source, 4);
        store.write().unwrap();
        fs::remove_dir_all(&removed).unwrap();
        fs::remove_file(kept.join("Gone.md")).unwrap();

        let (mut store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.last_workspace(), Some(kept.as_path()));
        assert_eq!(store.state().workspaces.len(), 1);
        assert_eq!(
            store.note_last_opened(&kept, &NoteId::from_title("Plan")),
            Some(3)
        );
        assert_eq!(
            store.note_last_opened(&kept, &NoteId::from_title("Gone")),
            None
        );
        assert_eq!(store.active_note(&kept), None);
    }

    /// ADR-008: a file that cannot be read is set aside and the state starts
    /// empty.
    #[test]
    fn sets_a_damaged_file_aside() {
        let sandbox = Sandbox::new("damaged");
        let file = sandbox.state_file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "workspaces: [this is not: valid").unwrap();

        let (store, notice) = StateStore::load(file.clone());
        assert_eq!(store.state(), &AppState::default());
        let Some(StateNotice::SetAside(aside)) = notice else {
            panic!("expected a notice");
        };
        assert!(aside.is_file());
        assert!(!file.exists());
    }

    #[test]
    fn in_memory_store_never_writes() {
        let mut store = StateStore::in_memory();
        store.workspace_opened(Path::new("/nowhere"), 1);
        store.write().unwrap();
        assert!(!store.has_unwritten_changes());
    }

    /// UC-021 BR-005, UC-004 step 6, OQ-279 (test-monkey): a pane value of the
    /// tab "Todos" that is no word (a number, null, a list, a map) must not
    /// lose the rest of the workspace's state file, like an unknown word.
    #[test]
    fn oq279_todo_pane_of_any_type_keeps_the_rest_of_the_state() {
        let tabs = "tabs:\n- note: Plan\n  position: 1\n  editing_view: source\n  active: false\nsplit_share: 600\n";
        let mut lost = Vec::new();
        for value in [
            "sideways", "5", "~", "true", "[left]", "{a: b}", "1.5", "''",
        ] {
            let text = format!("{tabs}todo_list:\n  open: true\n  pane: {value}\n");
            match serde_norway::from_str::<WorkspaceState>(&text) {
                Ok(state) => {
                    assert_eq!(state.tabs.len(), 1, "{value}");
                    assert_eq!(state.split_share, Some(600), "{value}");
                }
                Err(_) => lost.push(value),
            }
        }
        assert!(lost.is_empty(), "whole state file lost for pane: {lost:?}");
    }

    /// UC-004 step 6, OQ-220 (test-monkey): the same for the pane of an open
    /// tab, which has the same type.
    #[test]
    fn oq220_tab_pane_of_any_type_keeps_the_rest_of_the_state() {
        let mut lost = Vec::new();
        for value in ["5", "~", "true", "[right]", "{a: b}"] {
            let text = format!(
                "tabs:\n- note: Plan\n  position: 1\n  editing_view: source\n  active: true\n  pane: {value}\nsplit_share: 600\n"
            );
            if serde_norway::from_str::<WorkspaceState>(&text).is_err() {
                lost.push(value);
            }
        }
        assert!(lost.is_empty(), "whole state file lost for pane: {lost:?}");
    }

    /// OQ-220 (test-monkey): odd shares of every YAML type give a state, in
    /// bounds, and never lose the tabs.
    #[test]
    fn oq220_split_share_hostile_values() {
        let tabs = "tabs:\n- note: Plan\n  position: 1\n  editing_view: source\n  active: true\n";
        let mut lost = Vec::new();
        for value in [
            "99999999999999999999999999",
            "-99999999999999999999999999",
            "1e400",
            "-1e400",
            ".inf",
            "-.inf",
            ".nan",
            "0x1F4",
            "0o764",
            "'600'",
            "true",
            "null",
            "''",
            "-0.0",
            "4.9e-324",
            "18446744073709551615",
            "-9223372036854775808",
            "{a: 1}",
            "!!str 5",
            "&a 700",
        ] {
            let text = format!("{tabs}split_share: {value}\n");
            match serde_norway::from_str::<WorkspaceState>(&text) {
                Ok(state) => {
                    assert_eq!(state.tabs.len(), 1, "{value}");
                    if let Some(share) = state.split_share {
                        assert!((150..=850).contains(&share), "{value}: {share}");
                    }
                }
                Err(_) => lost.push(value),
            }
        }
        assert!(
            lost.is_empty(),
            "whole state file lost for split_share: {lost:?}"
        );
    }

    /// OQ-198 (test-monkey): hostile recent commands never lose the rest.
    #[test]
    fn oq198_recent_commands_hostile_values() {
        let workspaces = "workspaces:\n- directory: /notes\n";
        let mut lost = Vec::new();
        let huge: Vec<String> = (0..50_000).map(|n| format!("c.n{}", n % 30)).collect();
        for value in [
            "[\"a\\0b\", \"ok.id\", \"\\u0000\"]".to_owned(),
            "[\"ok.id\\n\"]".to_owned(),
            "[\"ok.id\", \"ok.id\", \"OK.ID\"]".to_owned(),
            "[~, ~, null, 1.5, [a], {a: 1}]".to_owned(),
            "[\"\\u00e4.x\", \"\\u202e.x\"]".to_owned(),
            "{a: b}".to_owned(),
            "~".to_owned(),
            "''".to_owned(),
            "99999999999999999999999999".to_owned(),
            "[ok.id, 99999999999999999999999999, -99999999999999999999999999]".to_owned(),
            format!("[{}]", huge.join(", ")),
        ] {
            let text = format!("{workspaces}recent_commands: {value}\n");
            let Ok(state) = serde_norway::from_str::<AppState>(&text) else {
                lost.push(value[..value.len().min(40)].to_owned());
                continue;
            };
            assert_eq!(state.workspaces.len(), 1);
            assert!(state.recent_commands.len() <= RECENT_COMMANDS);
            for id in &state.recent_commands {
                assert!(is_command_id(id), "{id:?}");
            }
            let mut sorted = state.recent_commands.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), state.recent_commands.len());
        }
        assert!(
            lost.is_empty(),
            "whole state file lost for recent_commands: {lost:?}"
        );
    }
}

#[cfg(test)]
mod uc002_tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-state-uc002-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// TC-002-01 / TC-002-13: the recently opened workspaces, most recent
    /// first; a gone one is forgotten.
    #[test]
    fn uc002_tc01_tc13_lists_and_forgets_recent_workspaces() {
        let root = sandbox("recent");
        let (alpha, beta) = (root.join("alpha"), root.join("beta"));
        fs::create_dir_all(&alpha).unwrap();
        fs::create_dir_all(&beta).unwrap();
        let (mut store, _) = StateStore::load(root.join("data").join(STATE_FILE_NAME));
        store.workspace_opened(&alpha, 1);
        store.workspace_opened(&beta, 2);
        assert_eq!(store.recent_workspaces(), [beta.clone(), alpha.clone()]);
        store.workspace_opened(&alpha, 3);
        assert_eq!(store.recent_workspaces(), [alpha.clone(), beta.clone()]);
        store.forget_workspace(&beta);
        assert_eq!(store.recent_workspaces(), [alpha]);
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-20: an Underline folder that cannot be written is reported
    /// once, and the store keeps working (A12).
    #[test]
    fn uc002_tc20_reports_an_unwritable_underline_folder_once() {
        let root = sandbox("unwritable");
        let workspace = root.join("readonly");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("Home.md"), "").unwrap();
        // A file where the folder should be cannot be written into, also as root.
        fs::write(workspace.join(UNDERLINE_FOLDER), "").unwrap();
        let (mut store, _) = StateStore::load(root.join("data").join(STATE_FILE_NAME));

        store.workspace_opened(&workspace, 1);
        store.note_opened(
            &workspace,
            &NoteId::from_title("Home"),
            EditingView::Source,
            2,
        );
        store.write().unwrap();
        assert_eq!(
            store.take_notices(),
            [StateNotice::CannotRemember(workspace.clone())]
        );
        store.note_opened(
            &workspace,
            &NoteId::from_title("Home"),
            EditingView::Source,
            3,
        );
        store.write().unwrap();
        assert!(store.take_notices().is_empty());
        assert_eq!(store.last_workspace(), Some(workspace.as_path()));
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-002-20: a damaged record in the Underline folder is set aside and
    /// the workspace opens as if for the first time (A12).
    #[test]
    fn uc002_tc20_sets_a_damaged_workspace_record_aside() {
        let root = sandbox("damaged");
        let workspace = root.join("damaged");
        fs::create_dir_all(workspace.join(UNDERLINE_FOLDER)).unwrap();
        fs::write(workspace.join("Home.md"), "").unwrap();
        let record = workspace.join(UNDERLINE_FOLDER).join(STATE_FILE_NAME);
        fs::write(&record, "tabs: [broken").unwrap();
        let (mut store, _) = StateStore::load(root.join("data").join(STATE_FILE_NAME));

        assert_eq!(store.active_note(&workspace), None);
        let notices = store.take_notices();
        assert!(matches!(notices.as_slice(), [StateNotice::SetAside(_)]));
        assert!(!record.exists());
        fs::remove_dir_all(&root).unwrap();
    }
}

#[cfg(test)]
mod git_tests {
    use super::*;
    use std::process::Command;

    /// ADR-008 / TC-001-08: in a git repository the Underline folder never
    /// shows as an untracked file, and the repository's .gitignore is untouched.
    #[test]
    fn uc001_underline_folder_is_ignored_by_git_by_itself() {
        let dir = std::env::temp_dir().join(format!("underline-state-git-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let workspace = dir.join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("Home.md"), "# Home\n").unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(&workspace)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .unwrap();
            String::from_utf8(out.stdout).unwrap()
        };
        git(&["init", "--quiet"]);

        let (mut store, _) = StateStore::load(dir.join("data").join(STATE_FILE_NAME));
        store.workspace_opened(&workspace, 1);
        store.note_opened(
            &workspace,
            &NoteId::from_title("Home"),
            EditingView::Source,
            2,
        );
        store.write().unwrap();

        assert!(workspace
            .join(UNDERLINE_FOLDER)
            .join(STATE_FILE_NAME)
            .is_file());
        assert_eq!(
            git(&["status", "--porcelain", "--untracked-files=all"]),
            "?? Home.md\n"
        );
        assert!(!workspace.join(".gitignore").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-004 step 6, BR-018: a tab of the right pane is remembered with its
    /// pane; a state file written before the split view reads as the left
    /// pane; places and the active tab count per pane.
    #[test]
    fn uc004_br018_tabs_are_remembered_with_their_pane() {
        let old: WorkspaceState = serde_norway::from_str(
            "tabs:\n- note: A\n  position: 1\n  editing_view: facade\n  active: true\n",
        )
        .unwrap();
        assert_eq!(old.tabs[0].pane, PaneSide::Left);
        let written = serde_norway::to_string(&old).unwrap();
        assert!(!written.contains("pane"), "{written}");
        let tab = |note: &str, pane: PaneSide, position: u32, active: bool| TabState {
            note: NoteId::from_title(note),
            pane,
            position,
            editing_view: EditingView::Source,
            active,
        };
        let mut state = WorkspaceState {
            tabs: vec![
                tab("B", PaneSide::Right, 7, false),
                tab("A", PaneSide::Left, 3, true),
                tab("C", PaneSide::Right, 9, false),
            ],
            ..WorkspaceState::default()
        };
        let dir =
            std::env::temp_dir().join(format!("underline-state-panes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["A", "B", "C"] {
            std::fs::write(dir.join(format!("{name}.md")), "").unwrap();
        }
        assert!(prune(&dir, &mut state));
        let summary: Vec<(String, PaneSide, u32, bool)> = state
            .tabs
            .iter()
            .map(|t| (t.note.to_string(), t.pane, t.position, t.active))
            .collect();
        assert_eq!(
            summary,
            [
                ("A".to_owned(), PaneSide::Left, 1, true),
                ("B".to_owned(), PaneSide::Right, 1, true),
                ("C".to_owned(), PaneSide::Right, 2, false),
            ]
        );
        let written = serde_norway::to_string(&state).unwrap();
        assert!(written.contains("pane: right"), "{written}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
