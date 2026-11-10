//! The session of one window: what was given on the command line, the app
//! state, the settings, the open workspace with its overview and tabs, and
//! the status message, driven by [`Session::tick`] (ADR-022).

mod diagrams;
pub(crate) use diagrams::SvgDiagrams;
mod editing;
mod embed;
mod facade;
mod indexes;
mod links;
mod notes;
mod search;
mod sync;
mod tab;
#[cfg(test)]
mod window;
mod workspace;

pub use self::embed::{EmbedDone, EmbedPlan};
pub use self::links::Followed;
pub use self::sync::{KeepCopy, Synced};
pub use self::workspace::{CreatePlan, OpenPlan, PreparedCreate, PreparedOpen};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use underline_core::{
    Effective, Git, NoteId, NoteSaver, OwnWrites, StateError, StateNotice, StateStore,
};
use underline_facade::Registry;
use underline_wiki::WikiRemote;

use crate::editor_view::TabPropertiesView;
use crate::settings::theme_names;
use crate::workspace_view::{TabsView, WorkspaceView};
use crate::{Event, Platform, SettingName, SettingsState, StartInfo, StatusOffer, StatusView};

use self::editing::Colors;
use self::workspace::OpenWorkspace;

/// Wakes the tick at once (ADR-022), for example when the workspace
/// watcher has changes. Without one set, waking does nothing.
#[derive(Clone)]
pub(crate) struct Wake(Arc<dyn Fn() + Send + Sync>);

impl Wake {
    pub(crate) fn wake(&self) {
        (self.0)();
    }
}

impl Default for Wake {
    fn default() -> Self {
        Self(Arc::new(|| {}))
    }
}

impl std::fmt::Debug for Wake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Wake")
    }
}

/// The state of one window.
///
/// The background services and the editor are ported here from the egui
/// app feature by feature.
pub struct Session {
    /// The authority for the folder. The window only gets a display string of
    /// it (`StartInfo::given_dir`) and never sends a path back.
    given_dir: Option<PathBuf>,
    /// Whether the workspace to start with was opened (M-01.1).
    started: bool,
    /// The known workspaces (M-22.1) and the remembered tabs (M-22.2).
    state: StateStore,
    /// The user and workspace settings (UC-010).
    settings: SettingsState,
    /// The one status message of the status bar (M-04.7).
    status: String,
    /// The installed git program (ADR-006).
    git: Git,
    /// The format modules, which write a new workspace's Home note.
    formats: Registry,
    /// The files Underline writes itself, which the watcher leaves out; the
    /// note saver records into the same.
    own_writes: OwnWrites,
    /// Stores the notes atomically on a worker thread (ADR-005).
    saver: NoteSaver,
    /// The highlighting of the theme in effect (ADR-010).
    colors: Option<Colors>,
    /// The highlighting being made on a thread of its own, for this theme.
    colors_coming: Option<(underline_core::Theme, std::sync::mpsc::Receiver<Colors>)>,
    /// ADR-025: tabs whose mirror the backend changed; the window is told
    /// to fetch the change (`ed-changed`).
    changed: Vec<NoteId>,
    /// ADR-025: the window is asked to send its edits now (`ed-flush`), for
    /// one note or for all.
    flushes: Vec<(Option<NoteId>, bool)>,
    /// Wakes the tick when a background service has news.
    wake: Wake,
    /// The open workspace (UC-002).
    open: Option<OpenWorkspace>,
    /// M-09.7: the diagrams of the facade as SVG (ADR-009), started with
    /// the first facade.
    diagrams: Option<Arc<diagrams::SvgDiagrams>>,
    /// M-09: what the facades share: the colours of code blocks, what was
    /// found of image files, and the results of the tick's jobs.
    facade: facade::Shared,
    /// M-16.5: the jobs that make the tool panel's lists.
    index_jobs: indexes::IndexJobs,
    /// UC-004 A4: the user agreed to open web addresses; asked once until
    /// Underline closes.
    web_allowed: bool,
    /// UC-004 A4: the web address "Open web address" asks about.
    web_asked: Option<String>,
    /// UC-007 A4: the undo offered right after deleting, while the status
    /// shows the report of the deletion.
    undo_delete: Option<notes::UndoDelete>,
    /// UC-007 A6: the deleted note "Restore note" asks a new place for.
    restoring: Option<notes::Restoring>,
    /// UC-008: the search of the overlay, kept when another workspace opens.
    search: search::Search,
    /// The workspace or the tab bar may have changed since the window was
    /// told; their views are built only then, not every tick.
    views_dirty: bool,
    /// What the window was told last, so only changes become events.
    told: Told,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("given_dir", &self.given_dir)
            .field("status", &self.status)
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

/// The state the window was told last.
#[derive(Debug)]
struct Told {
    status: String,
    /// What the status bar offered with it.
    offer: Option<StatusOffer>,
    /// The settings in effect; the view is built only when they changed.
    settings: Effective,
    width_generation: u32,
    workspace: Option<WorkspaceView>,
    /// The overview by the generation of the workspace it was taken from.
    overview: Option<(PathBuf, u64)>,
    /// The deleted notes, likewise (M-05.6).
    deleted: Option<(PathBuf, u64)>,
    tabs: TabsView,
    /// The properties section of the tool panel (M-16.2).
    properties: Option<TabPropertiesView>,
    /// The number of the tool panel's lists last sent (M-16.5).
    indexes: Option<u64>,
}

impl Session {
    /// The session of a window started with the optional workspace folder
    /// `given_dir` (M-01.1): the app state from the user's data directory,
    /// kept in memory where there is none (M-01.3), and the user settings
    /// from the home directory (UC-010).
    #[must_use]
    pub fn load(given_dir: Option<PathBuf>, now: Instant) -> Self {
        let (state, notice) = match StateStore::default_path() {
            Some(path) => StateStore::load(path),
            None => (StateStore::in_memory(), None),
        };
        Self::new(given_dir, state, notice, SettingsState::load(now))
    }

    /// A session with the given parts; `notice` is what loading `state`
    /// reported (M-01.3).
    #[must_use]
    pub fn new(
        given_dir: Option<PathBuf>,
        state: StateStore,
        notice: Option<StateNotice>,
        settings: SettingsState,
    ) -> Self {
        let told = Told {
            status: String::new(),
            offer: None,
            settings: settings.effective().clone(),
            width_generation: settings.width_generation(),
            workspace: None,
            overview: None,
            deleted: None,
            tabs: TabsView::default(),
            properties: None,
            indexes: None,
        };
        let own_writes = OwnWrites::default();
        let mut session = Self {
            given_dir,
            started: false,
            state,
            settings,
            status: String::new(),
            git: Git::new(),
            formats: Registry::default(),
            saver: NoteSaver::new(own_writes.clone()),
            own_writes,
            colors: None,
            colors_coming: None,
            changed: Vec::new(),
            flushes: Vec::new(),
            wake: Wake::default(),
            open: None,
            diagrams: None,
            facade: facade::Shared::default(),
            index_jobs: indexes::IndexJobs::default(),
            web_allowed: false,
            web_asked: None,
            undo_delete: None,
            restoring: None,
            search: search::Search::default(),
            views_dirty: false,
            told,
        };
        if let Some(notice) = notice {
            session.status = format!("Note: {notice}");
        }
        session.take_settings_messages();
        session.told.status.clone_from(&session.status);
        session
    }

    /// Sets what wakes the tick when a background service of the session,
    /// such as the workspace watcher, has news (ADR-022). Services started
    /// before keep the earlier one.
    pub fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Wake(Arc::new(wake));
    }

    /// What the window needs to know when it starts (M-01).
    #[must_use]
    pub fn start_info(&self) -> StartInfo {
        let display = |path: &std::path::Path| path.to_string_lossy().into_owned();
        StartInfo {
            given_dir: self.given_dir.as_deref().map(display),
            last_workspace: self.state.last_workspace().map(display),
            recent_workspaces: self
                .state
                .recent_workspaces()
                .iter()
                .map(|p| display(p))
                .collect(),
            platform: Platform::current(),
            settings: self.settings.view(),
            themes: theme_names(),
            status: self.status.clone(),
            tools: crate::ToolView::all(),
            input_modes: underline_core::InputMode::ALL
                .iter()
                .map(|mode| mode.label().to_owned())
                .collect(),
        }
    }

    /// The status message (M-04.7).
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The root of the open workspace, if one is open.
    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.open.as_ref().map(OpenWorkspace::root)
    }

    /// The settings of the window (UC-010).
    #[must_use]
    pub fn settings(&self) -> &SettingsState {
        &self.settings
    }

    /// UC-010 A3, A4, M-20.5: a setting changed outside the settings window,
    /// such as the theme from the Theme menu (M-03.6) or the burger button
    /// (M-03.1): in effect at once and stored at once.
    pub fn quick_setting(&mut self, key: SettingName, value: String) {
        self.settings.quick_change(key.into(), value);
        self.take_settings_messages();
    }

    /// M-03.5: the wiki project URL of the Workspace menu, checked. The
    /// status says which repository it maps to, or why it does not. Nothing
    /// is stored or connected (inventory D-13).
    pub fn check_wiki(&mut self, url: &str) {
        self.status = match WikiRemote::from_project_url(url) {
            Ok(remote) => format!("Wiki repository: {}", remote.clone_url()),
            Err(err) => err.to_string(),
        };
    }

    /// The work of one tick at `now` (ADR-022): the changes from outside
    /// the workspace watcher reports are taken in (ADR-011), the user
    /// settings file is looked at every 500 ms (M-20.6), due notes are
    /// stored in the background (M-10.8), the app state is written once a
    /// change is due (M-22.1), the colours of the lines the source view
    /// shows are worked out (M-08.2), the facade's colours and figures
    /// for the window's editor core (M-09, `ed-enrich`), and the outline,
    /// code and figures of the tool panel (M-16.5); the answer of the
    /// workspace search is taken in and the search started again when its
    /// notes changed (UC-008 A3, M-13.1). Returns what changed
    /// for the window.
    pub fn tick(&mut self, now: Instant) -> Vec<Event> {
        self.handle_outside_changes();
        self.settings.poll(now);
        self.take_settings_messages();
        self.autosave(now);
        if let Err(err) = self.state.write_if_due(now) {
            self.status = err.to_string();
        }
        self.show_state_notices();
        let mut events = self.take_events();
        events.extend(self.highlight_event(now));
        events.extend(self.facade_events());
        events.extend(self.indexes_events(now));
        events.extend(self.search_events());
        events
    }

    /// M-22.1: writes the app state now, as when the app ends. A failure
    /// becomes the status too.
    ///
    /// # Errors
    ///
    /// The state file could not be written.
    pub fn write_state_now(&mut self) -> Result<(), StateError> {
        let written = self.state.write();
        if let Err(err) = &written {
            self.status = err.to_string();
        }
        self.show_state_notices();
        written
    }

    /// The changes since the window was told last, as events.
    pub fn take_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        if self.settings.effective() != &self.told.settings
            || self.settings.width_generation() != self.told.width_generation
        {
            self.told.settings = self.settings.effective().clone();
            self.told.width_generation = self.settings.width_generation();
            events.push(Event::Settings(Box::new(self.settings.view())));
        }
        let dirty = std::mem::take(&mut self.views_dirty);
        if dirty {
            let workspace = self.workspace_view();
            if workspace != self.told.workspace {
                self.told.workspace.clone_from(&workspace);
                events.push(Event::Workspace(workspace));
            }
        }
        let overview = self
            .open
            .as_ref()
            .map(|open| (open.root().to_owned(), open.overview_generation));
        if overview != self.told.overview {
            self.told.overview = overview;
            events.push(Event::Overview(self.overview_items()));
        }
        if dirty {
            let tabs = self.tabs_view();
            if tabs != self.told.tabs {
                self.told.tabs = tabs.clone();
                events.push(Event::Tabs(tabs));
            }
        }
        // M-16.2: built when the tabs may have changed, or when the window
        // has none for the active tab yet, as after the workspace started.
        let active = self
            .open
            .as_ref()
            .and_then(|open| open.tabs.active())
            .map(|(note, _)| note.as_str());
        let told_note = self.told.properties.as_ref().map(|p| p.note.as_str());
        if dirty || active != told_note {
            let properties = self.properties_view();
            if properties != self.told.properties {
                self.told.properties.clone_from(&properties);
                events.push(Event::Properties(properties));
            }
        }
        let deleted = self
            .open
            .as_ref()
            .map(|open| (open.root().to_owned(), open.deleted_generation));
        if deleted != self.told.deleted {
            self.told.deleted = deleted;
            events.push(Event::Deleted(self.deleted_view()));
        }
        events.extend(self.sync_events());
        // UC-007 A4: the undo is offered while the report of the deletion
        // shows; another report ends it.
        if self
            .undo_delete
            .as_ref()
            .is_some_and(|offer| offer.status != self.status)
        {
            self.undo_delete = None;
        }
        let offer = self.undo_delete.as_ref().map(|_| StatusOffer::UndoDelete);
        if self.status != self.told.status || offer != self.told.offer {
            self.told.status.clone_from(&self.status);
            self.told.offer = offer;
            events.push(Event::Status(StatusView {
                text: self.status.clone(),
                offer,
            }));
        }
        events
    }

    /// The settings' messages become the status, the last one wins.
    fn take_settings_messages(&mut self) {
        if let Some(message) = self.settings.take_messages().pop() {
            self.status = message;
        }
    }

    fn show_state_notices(&mut self) {
        if let Some(notice) = self.state.take_notices().pop() {
            self.status = format!("Note: {notice}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    use underline_core::{Theme, SETTINGS_FILE_NAME, STATE_WRITE_DELAY};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-session-session-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A session with its user settings file in `dir` and the state in
    /// memory.
    fn session_in(dir: &Path, given: Option<PathBuf>) -> Session {
        Session::new(
            given,
            StateStore::in_memory(),
            None,
            SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
        )
    }

    #[test]
    fn m01_1_start_info_passes_the_given_dir_along() {
        let dir = temp_dir("given");
        let session = session_in(&dir, Some(PathBuf::from("notes")));

        let info = session.start_info();

        assert_eq!(info.given_dir.as_deref(), Some("notes"));
        assert_eq!(info.platform, Platform::current());
        assert_eq!(info.themes.len(), 11);
        assert_eq!(info.settings.theme, "Light");
        assert_eq!(info.status, "");
    }

    #[test]
    fn m01_1_start_info_without_a_dir() {
        let dir = temp_dir("no-dir");

        let info = session_in(&dir, None).start_info();

        assert_eq!(info.given_dir, None);
        assert_eq!(info.last_workspace, None);
        assert!(info.recent_workspaces.is_empty());
    }

    #[test]
    fn m01_3_a_notice_about_the_state_file_is_the_status() {
        let dir = temp_dir("notice");
        let notice = StateNotice::SetAside(PathBuf::from("/data/state.yaml.damaged"));

        let session = Session::new(
            None,
            StateStore::in_memory(),
            Some(notice),
            SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
        );

        assert_eq!(
            session.status(),
            "Note: the remembered workspaces and tabs could not be read and were set aside as /data/state.yaml.damaged"
        );
        assert_eq!(session.start_info().status, session.status());
    }

    #[test]
    fn m01_3_a_damaged_state_file_is_set_aside_and_noted() {
        let dir = temp_dir("damaged");
        let path = dir.join("state.yaml");
        fs::write(&path, "workspaces: [unclosed").unwrap();
        let (state, notice) = StateStore::load(path.clone());

        let session = Session::new(
            None,
            state,
            notice,
            SettingsState::with_user_file(None, Instant::now()),
        );

        assert!(
            session.status().starts_with("Note: "),
            "{}",
            session.status()
        );
        assert!(dir.join("state.yaml.damaged").exists());
    }

    #[test]
    fn m22_1_recent_and_last_workspace_come_from_the_state() {
        let dir = temp_dir("recent");
        let older = dir.join("older");
        let newer = dir.join("newer");
        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&newer).unwrap();
        let path = dir.join("state.yaml");
        fs::write(
            &path,
            format!(
                "workspaces:\n- directory: {}\n  last_opened: 1\n- directory: {}\n  last_opened: 2\n",
                older.display(),
                newer.display()
            ),
        )
        .unwrap();
        let (state, notice) = StateStore::load(path);

        let session = Session::new(
            None,
            state,
            notice,
            SettingsState::with_user_file(None, Instant::now()),
        );
        let info = session.start_info();

        assert_eq!(info.last_workspace, Some(newer.display().to_string()));
        assert_eq!(
            info.recent_workspaces,
            [newer.display().to_string(), older.display().to_string()]
        );
    }

    #[test]
    fn m22_1_the_tick_writes_the_state_once_a_change_is_due() {
        let dir = temp_dir("write-due");
        let path = dir.join("state.yaml");
        let gone = dir.join("gone");
        fs::write(
            &path,
            format!(
                "workspaces:\n- directory: {}\n  last_opened: 1\n",
                gone.display()
            ),
        )
        .unwrap();
        let (state, notice) = StateStore::load(path.clone());
        let mut session = Session::new(
            None,
            state,
            notice,
            SettingsState::with_user_file(None, Instant::now()),
        );

        session.tick(Instant::now() + STATE_WRITE_DELAY + Duration::from_millis(10));

        let written = fs::read_to_string(&path).unwrap();
        assert!(!written.contains("gone"), "{written}");
    }

    #[test]
    fn m22_1_a_due_change_is_written_once_and_not_again() {
        let dir = temp_dir("written-once");
        let path = dir.join("state.yaml");
        let mut state = StateStore::load(path.clone()).0;
        state.workspace_opened(&dir, 1);
        let mut session = Session::new(
            None,
            state,
            None,
            SettingsState::with_user_file(None, Instant::now()),
        );
        let due = Instant::now() + STATE_WRITE_DELAY + Duration::from_millis(10);

        session.tick(due);
        assert!(path.exists());
        fs::remove_file(&path).unwrap();
        session.tick(due + Duration::from_secs(1));

        assert!(!path.exists(), "an unchanged state is not written again");
    }

    #[test]
    fn m22_1_write_state_now_writes_a_change_before_it_is_due() {
        let dir = temp_dir("write-now");
        let path = dir.join("state.yaml");
        let mut state = StateStore::load(path.clone()).0;
        state.workspace_opened(&dir, 1);
        let mut session = Session::new(
            None,
            state,
            None,
            SettingsState::with_user_file(None, Instant::now()),
        );

        session.write_state_now().unwrap();

        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains(&dir.display().to_string()));
        assert_eq!(session.status(), "");
    }

    #[test]
    fn m22_1_a_failed_write_at_the_end_is_returned_and_the_status() {
        let dir = temp_dir("write-fails");
        // A file where the state's folder should be.
        fs::write(dir.join("data"), "").unwrap();
        let mut state = StateStore::load(dir.join("data").join("state.yaml")).0;
        state.workspace_opened(&dir, 1);
        let mut session = Session::new(
            None,
            state,
            None,
            SettingsState::with_user_file(None, Instant::now()),
        );

        let written = session.write_state_now();

        let err = written.unwrap_err();
        assert_eq!(session.status(), err.to_string());
    }

    #[test]
    fn m03_5_a_wiki_project_url_names_its_repository() {
        let dir = temp_dir("wiki-ok");
        let mut session = session_in(&dir, None);

        session.check_wiki("https://github.com/owner/repo");

        assert_eq!(
            session.status(),
            "Wiki repository: https://github.com/owner/repo.wiki.git"
        );
    }

    #[test]
    fn m03_5_a_wrong_wiki_url_says_why() {
        let dir = temp_dir("wiki-err");
        let mut session = session_in(&dir, None);

        session.check_wiki("owner/repo");

        assert_eq!(
            session.status(),
            "`owner/repo` is not a GitHub or GitLab project URL"
        );
    }

    #[test]
    fn uc010_m03_6_choosing_a_theme_applies_and_stores_it() {
        let dir = temp_dir("theme");
        let mut session = session_in(&dir, None);

        session.quick_setting(SettingName::Theme, "Solarized Dark".into());

        assert_eq!(session.settings().effective().theme, Theme::SolarizedDark);
        assert_eq!(
            fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "theme: Solarized Dark\n"
        );
        let events = session.take_events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], Event::Settings(view) if view.theme == "Solarized Dark" && view.palette.dark)
        );
    }

    #[test]
    fn uc010_m03_1_the_burger_button_switches_the_menu_bar() {
        let dir = temp_dir("menu-bar");
        let mut session = session_in(&dir, None);
        assert!(!session.settings().effective().show_menu_bar);

        session.quick_setting(SettingName::ShowMenuBar, "true".into());

        assert!(session.settings().effective().show_menu_bar);
        assert_eq!(
            fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "show_menu_bar: true\n"
        );
    }

    #[test]
    fn uc010_m20_5_a_refused_value_becomes_the_status() {
        let dir = temp_dir("refused-value");
        let mut session = session_in(&dir, None);

        session.quick_setting(SettingName::Theme, "Purple".into());

        assert!(!session.status().is_empty());
        let events = session.take_events();
        assert_eq!(
            events,
            [Event::Status(StatusView {
                text: session.status().to_owned(),
                offer: None,
            })]
        );
    }

    #[test]
    fn adr022_nothing_changed_gives_no_event() {
        let dir = temp_dir("quiet");
        let mut session = session_in(&dir, None);

        assert!(session.tick(Instant::now()).is_empty());
        session.check_wiki("https://github.com/owner/repo");
        assert_eq!(session.take_events().len(), 1);
        assert!(session.take_events().is_empty());
    }

    #[test]
    fn uc010_m20_6_the_tick_reads_a_changed_user_file() {
        let dir = temp_dir("tick-poll");
        let start = Instant::now();
        let mut session = Session::new(
            None,
            StateStore::in_memory(),
            None,
            SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), start),
        );
        fs::write(dir.join(SETTINGS_FILE_NAME), "theme: Nord\n").unwrap();

        assert!(session.tick(start + Duration::from_millis(100)).is_empty());
        let events = session.tick(start + Duration::from_millis(500));

        assert_eq!(session.settings().effective().theme, Theme::Nord);
        assert!(matches!(&events[..], [Event::Settings(view)] if view.theme == "Nord"));
    }
}
