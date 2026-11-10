//! UC-001 Create Workspace and UC-002 Open Workspace in the session: the
//! checks of the Open and Create dialogs, switching to a workspace, the note
//! overview, the tab list with what is remembered of it, and the changes
//! from outside that the overview and the tabs show (ADR-011).
//!
//! Ported from `apps/underline-desktop/src/app.rs`: `request_open`,
//! `open_directory`, `switch_to`, `check_new_directory`,
//! `create_workspace`, `refresh_overview`, `open_note`, `open_tab`,
//! `persist_tabs`, `close_tab` and `handle_outside_changes`.
//!
//! Feature F3 adds storing the tabs' changes before a switch or a close
//! and the open tabs' text in the changes from outside. Left for later
//! features, with a hook where they go: the syncer (F13), the searcher
//! (F9), the change tracker (F12), the deleted notes (F8) and the window
//! presence (F14).
//!
//! Opening and creating a workspace read the directory, list its notes and
//! may run git while the session is locked, as the egui app does them in
//! its frame. The commands run them on the blocking pool, so the window
//! stays responsive; only the tick waits meanwhile. That is accepted for
//! now (parity with egui) and not split into a read without the lock.

use std::path::{Path, PathBuf};

use underline_core::{
    check_directory, check_open_directory, check_readable_directory, create_workspace, now,
    outside_copy_path, user_settings_dir, CreateError, CreateProblem, DirectoryProblem,
    EditingView, Git, GitRepositoryOption, NewWorkspace, NoteId, OpenProblem, OverviewEntry,
    Surroundings, TabList, TabState, TodoListState, Workspace, WorkspaceEvent, WorkspaceWatcher,
    HOME_TITLE,
};

use super::tab::Tab;
use super::Session;
use crate::workspace_view::{
    CreateAnswer, EditingViewName, NewWorkspaceRequest, OpenAnswer, OpenOffer, OverviewItem,
    StartOpened, TabView, TabsView, TodoTabView, WorkspaceSnapshot, WorkspaceView,
};

/// The status when the system delivers no change events (ADR-011).
const POLLING_STATUS: &str = "Changes from outside may show up later in the note overview; this system delivers no change events";

/// The open workspace with its overview, tabs and watcher.
pub(super) struct OpenWorkspace {
    pub(super) workspace: Workspace,
    pub(super) tabs: TabList<Tab>,
    /// Watches the workspace for changes from outside (ADR-011).
    watcher: WorkspaceWatcher,
    /// UC-002 step 7: the note overview, recently opened first.
    overview: Vec<OverviewEntry>,
    /// Grows with every change of `overview`, so only a change is sent.
    pub(super) overview_generation: u64,
    /// UC-021 BR-005: the tab "Todos", its sort and filter.
    todo_list: TodoListState,
}

impl OpenWorkspace {
    pub(super) fn root(&self) -> &Path {
        self.workspace.root()
    }
}

impl std::fmt::Debug for OpenWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenWorkspace")
            .field("root", &self.workspace.root())
            .field("tabs", &self.tabs.len())
            .field("overview", &self.overview.len())
            .finish_non_exhaustive()
    }
}

/// The form two workspace directories are compared in.
fn underline_root(dir: &Path) -> PathBuf {
    std::path::absolute(dir).unwrap_or_else(|_| dir.to_owned())
}

/// M-03.3: the name of the workspace at `root`: its folder name.
fn workspace_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

/// UC-001 A1, A7: the directory stage again, saying why the directory was
/// refused.
fn directory_answer(path: String, problem: &DirectoryProblem) -> CreateAnswer {
    let offer = match problem {
        // A1: offer to open it instead (A2).
        DirectoryProblem::NotEmpty(dir) => Some(display(dir)),
        _ => None,
    };
    let hint = match problem {
        DirectoryProblem::NotEmpty(_) => " Choose another directory or open it as a workspace.",
        DirectoryProblem::InsideWorkspace { .. } => " Choose another directory.",
        _ => "",
    };
    CreateAnswer::Directory {
        path,
        problem: format!("{problem}.{hint}"),
        offer,
    }
}

/// UC-001 A5, A6: why the workspace could not be created, and what could
/// not be removed afterwards.
fn create_failure_status(err: &CreateError) -> String {
    let mut status = format!("The workspace could not be created: {err}");
    if !err.leftovers.is_empty() {
        let left: Vec<_> = err.leftovers.iter().map(|p| display(p)).collect();
        status.push_str(&format!("; could not remove: {}", left.join(", ")));
    }
    status
}

/// What opening a directory needs, taken from the session (A-22).
#[derive(Debug, Clone)]
pub struct OpenPlan {
    surroundings: Surroundings,
    recent: Vec<PathBuf>,
    /// The root of the workspace open now.
    open_root: Option<PathBuf>,
}

/// A directory checked and read without the session, to be opened by
/// [`Session::finish_open`].
#[derive(Debug)]
pub struct PreparedOpen(Prepared);

#[derive(Debug)]
enum Prepared {
    /// UC-002 A9: the workspace is open already.
    AlreadyOpen,
    /// The workspace, read.
    Opened(Workspace),
    /// What stands in the way (A3, A5, A7, A11).
    Problem {
        path: String,
        problem: String,
        offer: Option<OpenOffer>,
        /// A3: a remembered workspace that is gone, to be forgotten.
        forget: Option<PathBuf>,
    },
}

impl OpenPlan {
    /// UC-002 step 4: checks `dir` and reads the workspace in it; with
    /// `anyway`, the check for an enclosing workspace is left out. Reads
    /// the directory, so it runs without the session.
    #[must_use]
    pub fn prepare(&self, dir: &Path, anyway: bool) -> PreparedOpen {
        let checked = if anyway {
            check_readable_directory(dir, &self.surroundings).map_err(|problem| {
                let text = format!("{problem}. Choose another directory.");
                problem_of(display(dir), text, None, None)
            })
        } else {
            check_open_directory(dir, &self.surroundings).map_err(|problem| match problem {
                OpenProblem::Missing(missing) => {
                    let text = format!(
                        "{} does not exist. Choose another directory or create a new workspace there.",
                        missing.display()
                    );
                    let offer = OpenOffer::Create {
                        directory: display(&missing),
                    };
                    problem_of(display(dir), text, Some(offer), Some(missing))
                }
                OpenProblem::InsideWorkspace {
                    directory,
                    workspace,
                } => {
                    let text = format!(
                        "{} lies inside the workspace {}.",
                        directory.display(),
                        workspace.display()
                    );
                    let offer = OpenOffer::Outer {
                        outer: display(&workspace),
                        inner: display(&directory),
                    };
                    problem_of(display(dir), text, Some(offer), None)
                }
                problem => {
                    let text = format!("{problem}. Choose another directory.");
                    problem_of(display(dir), text, None, None)
                }
            })
        };
        let dir = match checked {
            Ok(dir) => dir,
            Err(prepared) => return prepared,
        };
        if self
            .open_root
            .as_deref()
            .is_some_and(|root| underline_root(root) == underline_root(&dir))
        {
            return PreparedOpen(Prepared::AlreadyOpen);
        }
        match Workspace::open_excluding(&dir, &self.recent) {
            Ok(workspace) => PreparedOpen(Prepared::Opened(workspace)),
            Err(err) => problem_of(
                display(&dir),
                format!("{err}. Choose another directory."),
                None,
                None,
            ),
        }
    }
}

fn problem_of(
    path: String,
    problem: String,
    offer: Option<OpenOffer>,
    forget: Option<PathBuf>,
) -> PreparedOpen {
    PreparedOpen(Prepared::Problem {
        path,
        problem,
        offer,
        forget,
    })
}

/// What creating a workspace needs, taken from the session (A-22).
#[derive(Debug, Clone)]
pub struct CreatePlan {
    request: NewWorkspace,
    home: String,
    surroundings: Surroundings,
    git: Git,
}

/// A workspace created, or why not, without the session.
#[derive(Debug)]
pub struct PreparedCreate {
    directory: PathBuf,
    created: Result<Workspace, CreateError>,
}

impl CreatePlan {
    /// UC-001 step 5: creates the workspace with its Home note. Writes the
    /// directory and may run git, so it runs without the session.
    #[must_use]
    pub fn prepare(self) -> PreparedCreate {
        let created = create_workspace(&self.request, &self.home, &self.surroundings, &self.git);
        PreparedCreate {
            directory: self.request.directory,
            created,
        }
    }
}

impl Session {
    /// M-01.1, UC-002 A1/A2: opens the directory given on the command line,
    /// else the workspace opened last; without either the window offers to
    /// open or create one. Only the first call opens anything.
    ///
    /// The same as [`Session::plan_start`], [`OpenPlan::prepare`] and
    /// [`Session::finish_start`] in a row; the window's backend runs the
    /// middle step without holding the session.
    pub fn start_workspace(&mut self) -> StartOpened {
        let prepared = self
            .plan_start()
            .map(|(plan, dir)| plan.prepare(&dir, false));
        self.finish_start(prepared)
    }

    /// M-01.1: what opening the workspace to start with needs, and its
    /// directory; `None` when there is none or it was asked for before.
    pub fn plan_start(&mut self) -> Option<(OpenPlan, PathBuf)> {
        if std::mem::replace(&mut self.started, true) {
            return None;
        }
        let dir = self
            .given_dir
            .clone()
            .or_else(|| self.state.last_workspace().map(Path::to_owned))?;
        Some((self.plan_open(), dir))
    }

    /// M-01.1: opens the workspace to start with, as prepared.
    pub fn finish_start(&mut self, prepared: Option<PreparedOpen>) -> StartOpened {
        let answer = match prepared {
            Some(prepared) => self.finish_open(prepared),
            None => OpenAnswer::Done,
        };
        let snapshot = self.snapshot();
        // The window takes the snapshot as it is, so the tick sends only
        // what changes after it; an event that arrives while `ws_start`
        // runs is newer than the snapshot, and the window keeps it.
        self.told.workspace.clone_from(&snapshot.workspace);
        self.told.overview = self
            .open
            .as_ref()
            .map(|open| (open.root().to_owned(), open.overview_generation));
        self.told.tabs = snapshot.tabs.clone();
        self.views_dirty = false;
        StartOpened { answer, snapshot }
    }

    /// UC-002 step 2: the recently opened workspaces, the most recent first.
    #[must_use]
    pub fn recent_workspaces(&self) -> Vec<String> {
        self.state
            .recent_workspaces()
            .iter()
            .map(|dir| display(dir))
            .collect()
    }

    /// The workspace, overview and tabs as the window shows them now.
    #[must_use]
    pub fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            workspace: self.workspace_view(),
            overview: self.overview_items(),
            tabs: self.tabs_view(),
        }
    }

    /// M-03.3, M-04.7: the open workspace.
    pub(super) fn workspace_view(&self) -> Option<WorkspaceView> {
        self.open.as_ref().map(|open| {
            let root = open.workspace.root();
            WorkspaceView {
                root: display(root),
                name: workspace_name(root),
            }
        })
    }

    /// M-05.2: the note overview.
    pub(super) fn overview_items(&self) -> Vec<OverviewItem> {
        self.open
            .iter()
            .flat_map(|open| &open.overview)
            .map(|entry| OverviewItem {
                note: entry.id.as_str().to_owned(),
                label: entry.label.clone(),
            })
            .collect()
    }

    /// M-06.1: the tab bar.
    pub(super) fn tabs_view(&self) -> TabsView {
        let Some(open) = &self.open else {
            return TabsView::default();
        };
        let todos = open.todo_list;
        let active = open.tabs.active().map(|(note, _)| note.as_str().to_owned());
        TabsView {
            tabs: open
                .tabs
                .iter()
                .map(|(note, tab)| TabView {
                    note: note.as_str().to_owned(),
                    title: tab.title.clone(),
                    label: tab.label(),
                    path: display(&tab.path),
                    view: EditingViewName::from(tab.editing_view),
                    save_error: tab.error.clone(),
                })
                .collect(),
            active,
            todo_list: TodoTabView {
                open: todos.open,
                active: todos.active,
            },
        }
    }

    /// UC-001 BR-006, UC-002 BR-007: what a directory is checked against.
    ///
    /// The home directory is the folder of the user settings file the
    /// session reads, and only without one the system's home directory.
    /// egui always asks the system; in production both are the same, and
    /// taking the settings' folder keeps the check and the settings file in
    /// step and lets tests use a home directory of their own (paritaet.md
    /// A-14).
    fn surroundings(&self) -> Surroundings {
        let mut known_workspaces = self.state.recent_workspaces();
        if let Some(open) = &self.open {
            known_workspaces.push(open.workspace.root().to_owned());
        }
        Surroundings {
            known_workspaces,
            user_settings_dir: self.settings.user_settings_dir().or_else(user_settings_dir),
        }
    }

    fn open_problem(&self, path: String, problem: String, offer: Option<OpenOffer>) -> OpenAnswer {
        OpenAnswer::Problem {
            path,
            problem,
            offer,
            recent: self.recent_workspaces(),
        }
    }

    /// M-21.1, UC-002 step 4: checks the chosen directory and opens it, or
    /// says what stands in the way (A3, A5, A7, A11). With `anyway`, only
    /// the check for an enclosing workspace is left out (A5 step 2, A6).
    ///
    /// The same as [`Session::plan_open`], [`OpenPlan::prepare`] and
    /// [`Session::finish_open`] in a row.
    pub fn open_chosen(&mut self, dir: &Path, anyway: bool) -> OpenAnswer {
        let prepared = self.plan_open().prepare(dir, anyway);
        self.finish_open(prepared)
    }

    /// What opening a directory needs from the session: taken while it is
    /// held, so the checks and the reading of the directory can run without
    /// it (A-22).
    #[must_use]
    pub fn plan_open(&self) -> OpenPlan {
        OpenPlan {
            surroundings: self.surroundings(),
            recent: self.state.recent_workspaces(),
            open_root: self.open.as_ref().map(|open| open.root().to_owned()),
        }
    }

    /// UC-002 steps 5–9: opens a directory that passed the checks, or says
    /// what stood in the way. The open notes are stored first (UC-005
    /// BR-001); when that fails, the workspace stays open.
    pub fn finish_open(&mut self, prepared: PreparedOpen) -> OpenAnswer {
        let workspace = match prepared.0 {
            Prepared::AlreadyOpen => return OpenAnswer::Done,
            Prepared::Problem {
                path,
                problem,
                offer,
                forget,
            } => {
                // A3: a remembered workspace that is gone is forgotten.
                if let Some(missing) = forget {
                    self.state.forget_workspace(&missing);
                }
                return self.open_problem(path, problem, offer);
            }
            Prepared::Opened(workspace) => workspace,
        };
        // A9: the workspace is already open (it may have been opened while
        // this one was read).
        if self.open.as_ref().is_some_and(|open| {
            underline_root(open.workspace.root()) == underline_root(workspace.root())
        }) {
            return OpenAnswer::Done;
        }
        if let Err(failures) = self.flush_all() {
            self.status = format!(
                "The workspace stays open, because changes could not be stored: {}",
                failures.join("; ")
            );
            return OpenAnswer::Done;
        }
        self.switch_to(workspace);
        let Some(open) = &self.open else {
            return OpenAnswer::Done;
        };
        let workspace = &open.workspace;
        let mut messages = vec![format!("Opened {}", workspace.root().display())];
        if let Some(problem) = workspace.settings_problem() {
            messages.push(format!("using the user settings, because {problem}"));
        }
        let unreadable = &workspace.listing().unreadable;
        if !unreadable.is_empty() {
            let unreadable: Vec<_> = unreadable.iter().map(|p| display(p)).collect();
            messages.push(format!("could not read {}", unreadable.join(", ")));
        }
        self.status = messages.join("; ");
        self.show_state_notices();
        // In the egui app the settings' messages reach the status one frame
        // later, so they win.
        self.take_settings_messages();
        OpenAnswer::Done
    }

    /// M-21.3, UC-001 step 6, UC-002 steps 5–8: makes `workspace` the open
    /// one. The workspace open before is closed and keeps its tabs; the
    /// remembered tabs come back, or the first note (BR-005); notes deleted
    /// more than 30 days ago go for good (UC-007 BR-005).
    fn switch_to(&mut self, workspace: Workspace) {
        let root = workspace.root().to_owned();
        // Closing the workspace open before:
        // - F14 (inventory D-05, fixed there): remove its window presence
        //   here, as the egui app does only when it ends.
        // - F13: stop its syncer; drop the old `Syncer` here.
        // Opening this one:
        // - F13: start a `Syncer` when it has a git remote, then a full sync
        //   (UC-014); with it the git state of the status bar (D-06).
        // - F9: start a `Searcher` for the workspace (UC-008).
        // - F12: start a `ChangeTracker` when it is a git repository
        //   (UC-011).
        let wake = self.wake.clone();
        let watcher = WorkspaceWatcher::start(
            &root,
            self.own_writes.clone(),
            self.git.clone(),
            move || wake.wake(),
        );
        let polling = watcher.is_polling();
        self.settings.open_workspace(&root, &self.git);
        self.state.workspace_opened(&root, now());
        // UC-021 BR-005: the tab "Todos", its sort and filter come back.
        let todo_list = self.state.todo_list(&root);
        self.open = Some(OpenWorkspace {
            workspace,
            tabs: TabList::default(),
            watcher,
            overview: Vec::new(),
            overview_generation: 0,
            todo_list,
        });
        // UC-004: the remembered tabs come back in their order; restoring
        // them does not change the order of the note overview.
        let remembered = self.state.tabs(&root);
        for tab in &remembered {
            self.open_tab(&tab.note, false, tab.editing_view);
        }
        let active = remembered.iter().find(|t| t.active).map(|t| &t.note);
        let found = self
            .open
            .as_ref()
            .and_then(|open| active.and_then(|note| open.tabs.find(note)));
        match found {
            Some(index) => {
                if let Some(open) = &mut self.open {
                    open.tabs.activate(index);
                }
            }
            None if self.open.as_ref().is_some_and(|open| open.tabs.is_empty()) => {
                let first = self
                    .open
                    .as_ref()
                    .and_then(|open| open.workspace.first_note());
                if let Some(note) = first {
                    self.open_note(&note);
                }
            }
            None => {}
        }
        self.persist_tabs();
        self.refresh_overview();
        if let Some(open) = &self.open {
            open.workspace.purge_deleted(now());
        }
        // F8: the deleted notes of the overview are read here (M-05.6).
        self.note_polling(polling);
    }

    /// ADR-011: says so when the system delivers no change events and the
    /// watcher polls instead.
    fn note_polling(&mut self, polling: bool) {
        if polling {
            POLLING_STATUS.clone_into(&mut self.status);
        }
    }

    /// M-21.2, UC-001 step 2: checks the directory typed in the first stage
    /// of the Create dialog.
    #[must_use]
    pub fn check_new_directory(&self, path: &str) -> CreateAnswer {
        let directory = PathBuf::from(path.trim());
        match check_directory(&directory, &self.surroundings()) {
            Ok(()) => CreateAnswer::Options {
                directory: display(&directory),
            },
            Err(problem) => directory_answer(path.to_owned(), &problem),
        }
    }

    /// M-21.2, UC-001 step 5, A3: creates the workspace, opens it with its
    /// Home note and says so; a failure says why and what is left (A5, A6).
    ///
    /// The same as [`Session::plan_create`], [`CreatePlan::prepare`] and
    /// [`Session::finish_create`] in a row.
    pub fn create_workspace(&mut self, request: NewWorkspaceRequest) -> CreateAnswer {
        match self.plan_create(request) {
            Ok(plan) => {
                let prepared = plan.prepare();
                self.finish_create(prepared)
            }
            Err(answer) => answer,
        }
    }

    /// UC-001 step 5: stores the open notes first (UC-005 BR-001) and takes
    /// what creating the workspace needs from the session, so creating it
    /// can run without the session (A-22). When the notes cannot be stored,
    /// the answer is that no workspace was created.
    ///
    /// # Errors
    ///
    /// The answer for the window when nothing is to be created.
    pub fn plan_create(
        &mut self,
        request: NewWorkspaceRequest,
    ) -> Result<CreatePlan, CreateAnswer> {
        if let Err(failures) = self.flush_all() {
            self.status = format!(
                "No workspace was created, because changes could not be stored: {}",
                failures.join("; ")
            );
            return Err(CreateAnswer::Done);
        }
        let remote = Some(request.remote.trim().to_owned()).filter(|a| !a.is_empty());
        let request = NewWorkspace {
            directory: PathBuf::from(request.directory),
            default_format: request.format.into(),
            git: request.git.then_some(GitRepositoryOption { remote }),
        };
        let home = self.formats.new_note(request.default_format, HOME_TITLE);
        Ok(CreatePlan {
            request,
            home,
            surroundings: self.surroundings(),
            git: self.git.clone(),
        })
    }

    /// UC-001 step 6, A1, A5, A6: opens the created workspace with its Home
    /// note, or says why it could not be created.
    pub fn finish_create(&mut self, prepared: PreparedCreate) -> CreateAnswer {
        match prepared.created {
            Ok(workspace) => {
                let root = display(workspace.root());
                self.switch_to(workspace);
                self.open_note(&NoteId::from_title(HOME_TITLE));
                self.refresh_overview();
                self.status = format!("Workspace created in {root}");
                self.take_settings_messages();
                CreateAnswer::Done
            }
            // A1 after the check of step 5: the directory filled up meanwhile.
            Err(err) => match &err.problem {
                CreateProblem::Directory(problem) => {
                    directory_answer(display(&prepared.directory), problem)
                }
                // A5 and A6.
                _ => {
                    self.status = create_failure_status(&err);
                    CreateAnswer::Done
                }
            },
        }
    }

    /// M-05.2, UC-004 step 2: opens the note `note` in a tab, or activates
    /// its tab (A1), and lists it first in the note overview.
    pub fn open_note(&mut self, note: &NoteId) {
        if self.open_tab(note, true, EditingView::Facade) {
            self.persist_tabs();
            self.refresh_overview();
        }
    }

    /// Opens `id` in a tab; with `viewed`, records it as the most recently
    /// opened note. Returns whether a tab shows the note now.
    fn open_tab(&mut self, id: &NoteId, viewed: bool, editing_view: EditingView) -> bool {
        let Some(open) = &mut self.open else {
            return false;
        };
        let root = open.workspace.root().to_owned();
        if viewed && open.todo_list.active {
            // UC-021 step 6: the note shows; the tab "Todos" stays open.
            open.todo_list.active = false;
            self.state.set_todo_list(&root, open.todo_list);
        }
        if let Some(index) = open.tabs.find(id) {
            open.tabs.activate(index);
        } else {
            let (note, path) = match (open.workspace.read(id), open.workspace.note_path(id)) {
                (Ok(note), Ok(path)) => (note, path),
                (Err(err), _) | (_, Err(err)) => {
                    // UC-004 A10: no tab for a note that cannot be read.
                    self.status = format!("{} cannot be read: {err}", id.title());
                    return false;
                }
            };
            let format = open
                .workspace
                .listing()
                .notes
                .iter()
                .find(|entry| entry.id == note.id)
                .map_or(open.workspace.default_format(), |entry| entry.format);
            let title = note.title();
            let opened = open.tabs.open(id.clone(), || {
                let mut tab = Tab::new(path, title, format, editing_view, note.body);
                tab.read_properties(&root);
                tab
            });
            // UC-004 A6, M-06.6: the tab used longest ago was closed; its
            // changes are stored first. Feature F16 fixes that a failure is
            // ignored here, as in the egui app (inventory D-07).
            if let Some((closed, mut tab)) = opened.closed {
                let _ = tab.flush(&self.saver);
                self.status = format!("Closed the tab {}, used longest ago", closed.title());
            }
        }
        if viewed {
            self.state.note_viewed(&root, id, now());
        }
        true
    }

    /// M-06.2, UC-004 A5: makes the tab of `note` the active one, records
    /// it as the most recently opened note and moves it up in the overview.
    pub fn activate_tab(&mut self, note: &NoteId) {
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(index) = open.tabs.find(note) else {
            return;
        };
        let root = open.workspace.root().to_owned();
        if open.todo_list.active {
            open.todo_list.active = false;
            self.state.set_todo_list(&root, open.todo_list);
        }
        open.tabs.activate(index);
        // M-16.2: the properties of the note shown are read again.
        open.tabs.get_mut(index).read_properties(&root);
        self.state.note_viewed(&root, note, now());
        self.persist_tabs();
        self.refresh_overview();
    }

    /// M-22.2, UC-004 step 6: remembers the open tabs and the active one.
    pub(super) fn persist_tabs(&mut self) {
        let Some(open) = &self.open else {
            return;
        };
        let active = open.tabs.active_index();
        let tabs = open
            .tabs
            .iter()
            .enumerate()
            .map(|(i, (note, tab))| TabState {
                note: note.clone(),
                position: u32::try_from(i + 1).unwrap_or(u32::MAX),
                editing_view: tab.editing_view,
                active: Some(i) == active,
            })
            .collect();
        let root = open.workspace.root().to_owned();
        self.state.set_tabs(&root, tabs);
        // The tab bar changed; the watcher's poller follows the open notes.
        self.views_dirty = true;
        if let Some(open) = &mut self.open {
            open.watcher
                .set_open_notes(open.tabs.iter().map(|(_, t)| t.path.clone()).collect());
        }
    }

    /// UC-002 step 7: the note overview, recently opened first (BR-008).
    fn refresh_overview(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let root = open.workspace.root().to_owned();
        let state = &mut self.state;
        let overview = underline_core::note_overview(&open.workspace.listing().notes, |id| {
            state.note_last_opened(&root, id)
        });
        if overview != open.overview {
            open.overview = overview;
            open.overview_generation += 1;
        }
        // F12: the git changes are asked again here (`refresh_changes`).
    }

    /// ADR-011: takes in the changes made outside Underline (UC-004 A7, A8,
    /// UC-010 A5) and lists the notes again when needed.
    pub(super) fn handle_outside_changes(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let events = open.watcher.poll();
        if events.is_empty() {
            return;
        }
        // A tab may be marked deleted or not any more.
        self.views_dirty = true;
        let mut rescan = false;
        for event in events {
            match event {
                WorkspaceEvent::NoteChanged(path) => rescan |= self.outside_change(&path),
                WorkspaceEvent::NoteRemoved(path) => {
                    rescan = true;
                    let Some(open) = &mut self.open else {
                        return;
                    };
                    let root = open.workspace.root().to_owned();
                    if let Some((_, tab)) = open.tabs.iter_mut().find(|(_, t)| t.path == path) {
                        // UC-004 A8: the tab stays open, marked.
                        tab.deleted = true;
                        tab.read_properties(&root);
                        self.status = format!("{} was deleted outside", tab.title);
                    }
                }
                WorkspaceEvent::SettingsChanged => {
                    self.settings.reload_workspace();
                    self.take_settings_messages();
                    let Some(open) = &mut self.open else {
                        return;
                    };
                    open.workspace.reload_settings();
                    if let Some(problem) = open.workspace.settings_problem() {
                        self.status = format!("Using the user settings, because {problem}");
                    }
                }
                WorkspaceEvent::Rescan => {
                    rescan = true;
                    let paths: Vec<PathBuf> = self
                        .open
                        .iter()
                        .flat_map(|open| open.tabs.iter().map(|(_, t)| t.path.clone()))
                        .collect();
                    for path in paths {
                        if path.exists() {
                            self.outside_change(&path);
                        } else if let Some(open) = &mut self.open {
                            if let Some((_, tab)) =
                                open.tabs.iter_mut().find(|(_, t)| t.path == path)
                            {
                                tab.deleted = true;
                            }
                        }
                    }
                }
            }
        }
        if rescan {
            if let Some(open) = &mut self.open {
                open.workspace.rescan();
            }
            self.refresh_overview();
        }
    }

    /// A note's file changed outside: its tab shows it again (UC-004 A7),
    /// or both versions are kept when the tab holds a change not stored
    /// yet (A9). Returns whether the note overview needs a rescan.
    fn outside_change(&mut self, path: &Path) -> bool {
        let Some(open) = &mut self.open else {
            return false;
        };
        let root = open.workspace.root().to_owned();
        let Some((note, tab)) = open.tabs.iter_mut().find(|(_, t)| t.path == path) else {
            return true;
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return false;
        };
        let was_deleted = std::mem::take(&mut tab.deleted);
        // Only a file that holds something else than Underline last read or
        // stored was changed outside; unstored edits make no difference.
        if content == tab.on_disk || tab.saving_text.as_deref() == Some(content.as_str()) {
            return was_deleted;
        }
        tab.read_properties(&root);
        if tab.autosave.is_unstored() || tab.saving {
            // UC-004 A9: neither version is overwritten.
            if tab.kept_outside.as_deref() == Some(content.as_str()) {
                return false;
            }
            tab.kept_outside = Some(content.clone());
            let copy = outside_copy_path(path, std::time::SystemTime::now());
            self.status = match self.saver.save_now(&copy, &content) {
                Ok(()) => {
                    let name = copy.file_name().unwrap_or_default().to_string_lossy();
                    format!(
                        "{} was changed outside while you were editing; your text stays, the other version is kept as {name}",
                        tab.title
                    )
                }
                Err(err) => format!(
                    "{} was changed outside; the other version could not be kept: {err}",
                    tab.title
                ),
            };
            return true;
        }
        tab.reload(content);
        self.replaced.push(note.clone());
        self.changed.push(note.clone());
        self.status = format!("{} was changed outside", tab.title);
        was_deleted
    }
}

#[cfg(test)]
mod tests;
