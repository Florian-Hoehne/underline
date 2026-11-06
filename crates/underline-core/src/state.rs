//! The state Underline keeps between two starts (ADR-008): per user the known
//! workspaces, per workspace its open tabs and when its notes were last
//! opened (FR-071, UC-001, UC-002).

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
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

/// What the user's data directory keeps: the known workspaces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub workspaces: Vec<KnownWorkspace>,
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
}

/// One open tab (entity OPEN_TAB).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabState {
    pub note: NoteId,
    /// Place of the tab from the left, starting at 1.
    pub position: u32,
    pub editing_view: EditingView,
    pub active: bool,
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

    /// The remembered tabs of the workspace at `directory`, left to right.
    pub fn tabs(&mut self, directory: &Path) -> Vec<TabState> {
        let mut tabs = self.workspace(directory).state.tabs.clone();
        tabs.sort_by_key(|tab| tab.position);
        tabs
    }

    /// The note of the active tab of the workspace at `directory`.
    pub fn active_note(&mut self, directory: &Path) -> Option<NoteId> {
        self.workspace(directory)
            .state
            .tabs
            .iter()
            .find(|tab| tab.active)
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
    for (index, tab) in state.tabs.iter_mut().enumerate() {
        tab.position = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
    if !state.tabs.is_empty() && !state.tabs.iter().any(|t| t.active) {
        state.tabs[0].active = true;
    }
    *state != before
}

/// Writes the state of a workspace into its Underline folder, creating the
/// folder and the `.gitignore` that makes git ignore it (ADR-008).
fn write_workspace_state(directory: &Path, state: &WorkspaceState) -> Result<(), StateError> {
    let folder = directory.join(UNDERLINE_FOLDER);
    let gitignore = folder.join(".gitignore");
    let prepare = || -> io::Result<()> {
        fs::create_dir_all(&folder)?;
        if fs::read_to_string(&gitignore).ok().as_deref() != Some(UNDERLINE_FOLDER_GITIGNORE) {
            write_atomic(&gitignore, UNDERLINE_FOLDER_GITIGNORE.as_bytes())?;
        }
        Ok(())
    };
    prepare().map_err(|source| StateError::Write {
        path: gitignore.clone(),
        source,
    })?;
    write_yaml(&folder.join(STATE_FILE_NAME), state)
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
}
