//! The app state between two starts (ADR-007): the known workspaces, their
//! open tabs, and when their notes were last opened (FR-071, UC-001).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::note::NoteId;

/// File name of the state file in the user's data directory.
pub const STATE_FILE_NAME: &str = "state.yaml";
/// A change is written this long after it happened, together with the changes
/// that follow within that time (ADR-007: at most 1 second, NFR-009).
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

/// Everything ADR-007 keeps between two starts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub workspaces: Vec<WorkspaceState>,
}

/// One known workspace (entity WORKSPACE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub directory: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_opened: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<TabState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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

/// Something the user should be told about the state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateNotice {
    /// The file could not be read; it was set aside under this name and
    /// Underline starts with an empty state.
    SetAside(PathBuf),
}

impl std::fmt::Display for StateNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateNotice::SetAside(path) => write!(
                f,
                "the remembered workspaces and tabs could not be read and were set aside as {}",
                path.display()
            ),
        }
    }
}

/// The app state with the file it is kept in.
#[derive(Debug)]
pub struct StateStore {
    /// `None` keeps the state in memory only, for example when the platform
    /// names no data directory.
    path: Option<PathBuf>,
    state: AppState,
    changed_at: Option<Instant>,
}

impl StateStore {
    /// `state.yaml` in the user's data directory (ADR-007).
    pub fn default_path() -> Option<PathBuf> {
        directories::BaseDirs::new()
            .map(|dirs| dirs.data_dir().join("underline").join(STATE_FILE_NAME))
    }

    /// A store that never writes.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            state: AppState::default(),
            changed_at: None,
        }
    }

    /// Reads the state file at `path` and drops workspaces and notes that are
    /// gone. A missing file gives an empty state; a file that cannot be read
    /// is set aside, and the notice says so.
    pub fn load(path: PathBuf) -> (Self, Option<StateNotice>) {
        let (state, notice) = match fs::read_to_string(&path) {
            Ok(text) if text.trim().is_empty() => (AppState::default(), None),
            Ok(text) => match serde_norway::from_str(&text) {
                Ok(state) => (state, None),
                Err(_) => (AppState::default(), set_aside(&path)),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => (AppState::default(), None),
            Err(_) => (AppState::default(), set_aside(&path)),
        };
        let mut store = Self {
            path: Some(path),
            state,
            changed_at: None,
        };
        store.prune();
        (store, notice)
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The workspace opened most recently, if it still exists.
    pub fn last_workspace(&self) -> Option<&Path> {
        self.state
            .workspaces
            .iter()
            .filter(|w| w.last_opened.is_some())
            .max_by_key(|w| w.last_opened)
            .map(|w| w.directory.as_path())
    }

    /// The remembered tabs of the workspace at `directory`, left to right.
    pub fn tabs(&self, directory: &Path) -> Vec<TabState> {
        let mut tabs = self
            .find(directory)
            .map(|w| w.tabs.clone())
            .unwrap_or_default();
        tabs.sort_by_key(|tab| tab.position);
        tabs
    }

    /// The note of the active tab of the workspace at `directory`.
    pub fn active_note(&self, directory: &Path) -> Option<NoteId> {
        self.find(directory)?
            .tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.note.clone())
    }

    /// When the note `note` of the workspace at `directory` was last opened.
    pub fn note_last_opened(&self, directory: &Path, note: &NoteId) -> Option<Timestamp> {
        self.find(directory)?
            .notes
            .iter()
            .find(|n| &n.note == note)
            .map(|n| n.last_opened)
    }

    /// Records that the workspace at `directory` was opened (UC-001
    /// postcondition: remembered as the last opened one). Its tabs stay.
    pub fn workspace_opened(&mut self, directory: &Path, at: Timestamp) {
        self.entry(directory).last_opened = Some(at);
        self.mark_changed();
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
        let workspace = self.entry(directory);
        workspace.tabs = vec![TabState {
            note: note.clone(),
            position: 1,
            editing_view: view,
            active: true,
        }];
        match workspace.notes.iter_mut().find(|n| &n.note == note) {
            Some(entry) => entry.last_opened = at,
            None => workspace.notes.push(NoteState {
                note: note.clone(),
                last_opened: at,
            }),
        }
        self.mark_changed();
    }

    /// Whether a change waits to be written.
    pub fn has_unwritten_changes(&self) -> bool {
        self.changed_at.is_some()
    }

    /// Writes the state when a change has waited [`STATE_WRITE_DELAY`];
    /// returns whether it wrote.
    pub fn write_if_due(&mut self, now: Instant) -> Result<bool, StateError> {
        match self.changed_at {
            Some(changed) if now.duration_since(changed) >= STATE_WRITE_DELAY => {
                self.write().map(|()| true)
            }
            _ => Ok(false),
        }
    }

    /// Writes the state now, atomically (ADR-005), if anything changed.
    pub fn write(&mut self) -> Result<(), StateError> {
        if self.changed_at.is_none() {
            return Ok(());
        }
        if let Some(path) = &self.path {
            let text = serde_norway::to_string(&self.state)
                .map_err(|err| StateError::Encode(err.to_string()))?;
            let write = || -> io::Result<()> {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                write_atomic(path, text.as_bytes())
            };
            write().map_err(|source| StateError::Write {
                path: path.clone(),
                source,
            })?;
        }
        self.changed_at = None;
        Ok(())
    }

    /// Drops workspaces whose directory is gone and notes whose file is gone
    /// (ADR-007).
    fn prune(&mut self) {
        let before = self.state.clone();
        self.state.workspaces.retain(|w| w.directory.is_dir());
        for workspace in &mut self.state.workspaces {
            let dir = workspace.directory.clone();
            let exists = |note: &NoteId| {
                NoteFormat::ALL
                    .into_iter()
                    .any(|format| dir.join(note.file_name(format)).is_file())
            };
            workspace.notes.retain(|n| exists(&n.note));
            workspace.tabs.retain(|t| exists(&t.note));
            for (index, tab) in workspace.tabs.iter_mut().enumerate() {
                tab.position = u32::try_from(index + 1).unwrap_or(u32::MAX);
            }
            if !workspace.tabs.is_empty() && !workspace.tabs.iter().any(|t| t.active) {
                workspace.tabs[0].active = true;
            }
        }
        if self.state != before {
            self.mark_changed();
        }
    }

    fn mark_changed(&mut self) {
        self.changed_at.get_or_insert_with(Instant::now);
    }

    fn find(&self, directory: &Path) -> Option<&WorkspaceState> {
        let key = key(directory);
        self.state.workspaces.iter().find(|w| w.directory == key)
    }

    fn entry(&mut self, directory: &Path) -> &mut WorkspaceState {
        let key = key(directory);
        let index = match self
            .state
            .workspaces
            .iter()
            .position(|w| w.directory == key)
        {
            Some(index) => index,
            None => {
                self.state.workspaces.push(WorkspaceState {
                    directory: key,
                    last_opened: None,
                    tabs: Vec::new(),
                    notes: Vec::new(),
                });
                self.state.workspaces.len() - 1
            }
        };
        &mut self.state.workspaces[index]
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

        let (store, _) = StateStore::load(sandbox.state_file());
        assert_eq!(store.last_workspace(), Some(new.as_path()));
        assert_eq!(store.active_note(&new), Some(home()));
        assert_eq!(store.tabs(&new)[0].editing_view, EditingView::Facade);
        assert_eq!(store.note_last_opened(&new, &home()), Some(2_001));
        // TC-001-04: the previous workspace keeps its tab.
        assert_eq!(store.active_note(&old), Some(NoteId::from_title("A")));
    }

    /// ADR-007: a change is written after the write delay, not before.
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

    /// ADR-007: workspaces and notes that are gone are dropped at start.
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

        let (store, _) = StateStore::load(sandbox.state_file());
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

    /// ADR-007: a file that cannot be read is set aside and the state starts
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
