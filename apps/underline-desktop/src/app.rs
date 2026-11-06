use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use underline_core::{
    check_directory, check_open_directory, check_readable_directory, create_workspace,
    heading_title, note_overview, now, outside_copy_path, user_settings_dir, Autosave,
    CreateProblem, DirectoryProblem, EditingView, Git, GitRepositoryOption, NewNote,
    NewNoteProblem, NewWorkspace, NoteFormat, NoteId, NoteSaver, OpenProblem, OverviewEntry,
    OwnWrites, SaveJob, StateNotice, StateStore, Surroundings, TabList, TabState, Workspace,
    WorkspaceEvent, WorkspaceWatcher, HOME_TITLE, STATE_WRITE_DELAY,
};
use underline_editor::SourceView;
use underline_facade::highlight::HighlightSet;
use underline_facade::Registry;
use underline_wiki::WikiRemote;

/// Main window: note overview on the left, editor in the middle, status bar
/// at the bottom. Without an open workspace it offers to open or create one
/// (UC-002 A2).
pub struct UnderlineApp {
    workspace: Option<Workspace>,
    formats: Registry,
    git: Git,
    overview: Vec<OverviewEntry>,
    /// State of the last git sync for the status bar (UC-002 step 9).
    git_state: Option<String>,
    /// The open tabs of the workspace (UC-004).
    tabs: TabList<OpenTab>,
    highlight: Arc<HighlightSet>,
    /// Stores changed notes in the background (UC-005 BR-001).
    saver: NoteSaver,
    /// A close that waits for the user, because changes could not be stored
    /// (UC-005 A14 step 3).
    close_confirm: Option<CloseConfirm>,
    /// The user agreed to close Underline although changes are not stored.
    close_anyway: bool,
    /// Watches the open workspace for changes from outside (ADR-011).
    watcher: Option<WorkspaceWatcher>,
    ctx: egui::Context,
    wiki_url: String,
    status: String,
    create: Option<CreateDialog>,
    open: Option<OpenDialog>,
    new_note: Option<NoteDialog>,
    /// Workspaces, tabs, and recent notes kept between starts (ADR-008).
    state: StateStore,
}

/// One open tab (UC-004): the note in its source view, with its autosave
/// clock (UC-005).
struct OpenTab {
    view: SourceView,
    path: PathBuf,
    title: String,
    autosave: Autosave,
    /// A background save of this tab is running.
    saving: bool,
    /// Why the last save failed (UC-005 A14).
    error: Option<String>,
    /// The note was deleted outside while open (UC-004 A8).
    deleted: bool,
    /// The version from outside last kept as a copy (UC-004 A9), so one
    /// change is kept once, however many events report it.
    kept_outside: Option<String>,
}

impl OpenTab {
    /// The label of the tab: its title, marked when not stored or deleted.
    fn label(&self) -> String {
        let mut label = self.title.clone();
        if self.error.is_some() {
            label.push_str(" ⚠");
        } else if self.autosave.is_unstored() {
            label.push_str(" •");
        }
        if self.deleted {
            label.push_str(" (deleted)");
        }
        label
    }
}

/// What waits for the user's answer before closing (UC-005 A14 step 3).
enum CloseConfirm {
    Tab(NoteId, String),
    App(String),
}

/// The dialog of UC-001 Create Workspace, one stage per question.
enum CreateDialog {
    /// Step 1: choose the directory; `problem` explains A1 or A7.
    Directory {
        path: String,
        problem: Option<String>,
        offer_open: Option<PathBuf>,
    },
    /// Step 3: default format and git repository option.
    Options {
        directory: PathBuf,
        format: NoteFormat,
        git: bool,
    },
    /// A3 step 1: optional address of the git remote.
    Remote {
        directory: PathBuf,
        format: NoteFormat,
        address: String,
    },
}

/// The dialog of UC-003 Create Note (steps 2–3).
struct NoteDialog {
    title: String,
    folder: String,
    format: NoteFormat,
    /// Why the last confirmation was refused (A1, A2, A4).
    problem: Option<String>,
    /// A2: the existing note the user may open instead (A3).
    existing: Option<NoteId>,
}

/// What the user did in the note dialog this frame.
enum NoteAction {
    None,
    Cancel,
    Create,
    OpenExisting(NoteId),
}

/// The dialog of UC-002 Open Workspace (steps 2–3).
struct OpenDialog {
    recent: Vec<PathBuf>,
    path: String,
    problem: Option<String>,
    offer: Option<OpenOffer>,
}

/// What the open dialog offers after a check of step 4.
enum OpenOffer {
    /// A3: create a new workspace in the missing directory (A4).
    Create(PathBuf),
    /// A5: open the outer workspace (A6) or the directory anyway.
    Outer { outer: PathBuf, inner: PathBuf },
}

/// What the user did in a dialog this frame.
enum DialogAction {
    None,
    Cancel,
    Check(String),
    AskRemote(PathBuf, NoteFormat),
    OpenInstead(PathBuf),
    Create(NewWorkspace),
}

/// What the user did in the open dialog this frame.
enum OpenAction {
    None,
    Cancel,
    Open(PathBuf),
    OpenAnyway(PathBuf),
    CreateAt(PathBuf),
}

impl UnderlineApp {
    /// UC-002 A1/A2: opens `given`, else the last opened workspace, else
    /// offers to open or create one.
    pub fn new(
        ctx: egui::Context,
        given: Option<PathBuf>,
        state: StateStore,
        notice: Option<StateNotice>,
    ) -> Self {
        let mut app = Self {
            workspace: None,
            formats: Registry::default(),
            git: Git::new(),
            overview: Vec::new(),
            git_state: None,
            tabs: TabList::default(),
            highlight: Arc::new(HighlightSet::new(true)),
            saver: NoteSaver::new(OwnWrites::default()),
            close_confirm: None,
            close_anyway: false,
            watcher: None,
            ctx,
            wiki_url: String::new(),
            status: String::new(),
            create: None,
            open: None,
            new_note: None,
            state,
        };
        if let Some(notice) = notice {
            app.status = format!("Note: {notice}");
        }
        let start = given.or_else(|| app.state.last_workspace().map(Path::to_owned));
        if let Some(dir) = start {
            app.request_open(&dir);
        }
        app
    }

    fn surroundings(&self) -> Surroundings {
        let mut known_workspaces = self.state.recent_workspaces();
        if let Some(workspace) = &self.workspace {
            known_workspaces.push(workspace.root().to_owned());
        }
        Surroundings {
            known_workspaces,
            user_settings_dir: user_settings_dir(),
        }
    }

    fn open_dialog(
        &self,
        path: String,
        problem: Option<String>,
        offer: Option<OpenOffer>,
    ) -> OpenDialog {
        OpenDialog {
            recent: self.state.recent_workspaces(),
            path,
            problem,
            offer,
        }
    }

    /// UC-002 step 4: checks the chosen directory and opens it, or shows the
    /// dialog with what stands in the way (A3, A5, A7, A11).
    fn request_open(&mut self, dir: &Path) {
        let path = dir.display().to_string();
        match check_open_directory(dir, &self.surroundings()) {
            Ok(dir) => self.open_directory(&dir),
            Err(OpenProblem::Missing(missing)) => {
                // A3: a remembered workspace that is gone is forgotten.
                self.state.forget_workspace(&missing);
                let problem = format!(
                    "{} does not exist. Choose another directory or create a new workspace there.",
                    missing.display()
                );
                self.open =
                    Some(self.open_dialog(path, Some(problem), Some(OpenOffer::Create(missing))));
            }
            Err(OpenProblem::InsideWorkspace {
                directory,
                workspace,
            }) => {
                let problem = format!(
                    "{} lies inside the workspace {}.",
                    directory.display(),
                    workspace.display()
                );
                let offer = OpenOffer::Outer {
                    outer: workspace,
                    inner: directory,
                };
                self.open = Some(self.open_dialog(path, Some(problem), Some(offer)));
            }
            Err(problem) => {
                let problem = format!("{problem}. Choose another directory.");
                self.open = Some(self.open_dialog(path, Some(problem), None));
            }
        }
    }

    /// UC-002 steps 5–9 for a directory that passed the checks.
    fn open_directory(&mut self, dir: &Path) {
        // A9: the workspace is already open.
        if self.workspace.as_ref().map(|w| underline_root(w.root())) == Some(underline_root(dir)) {
            self.open = None;
            return;
        }
        // UC-005 BR-001: the notes of the open workspace are stored first.
        if let Err(failures) = self.flush_all() {
            self.open = None;
            self.status = format!(
                "The workspace stays open, because changes could not be stored: {}",
                failures.join("; ")
            );
            return;
        }
        match Workspace::open_excluding(dir, &self.state.recent_workspaces()) {
            Ok(workspace) => {
                self.open = None;
                self.switch_to(workspace);
                let workspace = self.workspace.as_ref().expect("just opened");
                let mut messages = vec![format!("Opened {}", workspace.root().display())];
                if let Some(problem) = workspace.settings_problem() {
                    messages.push(format!("using the user settings, because {problem}"));
                }
                if !workspace.listing().unreadable.is_empty() {
                    let unreadable: Vec<_> = workspace
                        .listing()
                        .unreadable
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    messages.push(format!("could not read {}", unreadable.join(", ")));
                }
                self.status = messages.join("; ");
                self.show_notices();
            }
            Err(err) => {
                let problem = format!("{err}. Choose another directory.");
                self.open = Some(self.open_dialog(dir.display().to_string(), Some(problem), None));
            }
        }
    }

    /// Makes `workspace` the open one; the previous one is closed and keeps
    /// its tabs (UC-001 step 6, UC-002 step 5). The remembered active tab is
    /// opened again, or the first note (UC-002 step 8, BR-005).
    fn switch_to(&mut self, workspace: Workspace) {
        let root = workspace.root().to_owned();
        self.git_state = self.git.remotes(&root).and_then(|remotes| {
            // UC-014 brings git sync; until then no sync has run.
            (!remotes.is_empty()).then(|| "git: no sync yet".to_owned())
        });
        let wake = self.ctx.clone();
        let watcher = WorkspaceWatcher::start(
            &root,
            self.saver.own_writes().clone(),
            self.git.clone(),
            move || wake.request_repaint(),
        );
        let polling = watcher.is_polling();
        self.watcher = Some(watcher);
        self.workspace = Some(workspace);
        self.tabs = TabList::default();
        self.state.workspace_opened(&root, now());
        // UC-004: the remembered tabs come back in their order; restoring
        // them does not change the order of the note overview.
        let remembered = self.state.tabs(&root);
        for tab in &remembered {
            self.open_tab(&tab.note, false);
        }
        let active = remembered.iter().find(|t| t.active).map(|t| t.note.clone());
        match active.and_then(|note| self.tabs.find(&note)) {
            Some(index) => self.tabs.activate(index),
            None if self.tabs.is_empty() => {
                if let Some(note) = self.workspace.as_ref().and_then(Workspace::first_note) {
                    self.open_note(&note);
                }
            }
            None => {}
        }
        self.persist_tabs();
        self.refresh_overview();
        if polling {
            self.status =
                "Changes from outside may show up later in the note overview; this system delivers no change events"
                    .to_owned();
        }
    }

    /// ADR-011: takes in the changes made outside Underline (UC-004 A7–A9).
    fn handle_outside_changes(&mut self) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        watcher.set_open_notes(self.tabs.iter().map(|(_, t)| t.path.clone()).collect());
        let polling = watcher.is_polling();
        let events = watcher.poll();
        if polling {
            self.ctx.request_repaint_after(Duration::from_secs(1));
        }
        let mut rescan = false;
        for event in events {
            match event {
                WorkspaceEvent::NoteChanged(path) => {
                    rescan |= self.outside_change(&path);
                }
                WorkspaceEvent::NoteRemoved(path) => {
                    rescan = true;
                    if let Some((_, tab)) = self.tabs.iter_mut().find(|(_, t)| t.path == path) {
                        // UC-004 A8: the tab stays open, marked.
                        tab.deleted = true;
                        self.status = format!("{} was deleted outside", tab.title);
                    }
                }
                WorkspaceEvent::SettingsChanged => {
                    if let Some(workspace) = &mut self.workspace {
                        workspace.reload_settings();
                        if let Some(problem) = workspace.settings_problem() {
                            self.status = format!("Using the user settings, because {problem}");
                        }
                    }
                }
                WorkspaceEvent::Rescan => {
                    rescan = true;
                    let paths: Vec<PathBuf> =
                        self.tabs.iter().map(|(_, t)| t.path.clone()).collect();
                    for path in paths {
                        if path.exists() {
                            self.outside_change(&path);
                        } else if let Some((_, tab)) =
                            self.tabs.iter_mut().find(|(_, t)| t.path == path)
                        {
                            tab.deleted = true;
                        }
                    }
                }
            }
        }
        if rescan {
            if let Some(workspace) = &mut self.workspace {
                workspace.rescan();
            }
            self.refresh_overview();
        }
    }

    /// A note's file changed outside: reload its tab (UC-004 A7), or keep
    /// both versions when the tab holds an unstored change (A9). Returns
    /// whether the note overview needs a rescan.
    fn outside_change(&mut self, path: &Path) -> bool {
        let Some((_, tab)) = self.tabs.iter_mut().find(|(_, t)| t.path == path) else {
            return true;
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return false;
        };
        let was_deleted = std::mem::take(&mut tab.deleted);
        if content == tab.view.document().text() {
            return was_deleted;
        }
        if tab.autosave.is_unstored() || tab.saving {
            // UC-004 A9: neither version is overwritten.
            if tab.kept_outside.as_deref() == Some(content.as_str()) {
                return false;
            }
            tab.kept_outside = Some(content.clone());
            let copy = outside_copy_path(path, SystemTime::now());
            match self.saver.save_now(&copy, &content) {
                Ok(()) => {
                    let name = copy.file_name().unwrap_or_default().to_string_lossy();
                    self.status = format!(
                        "{} was changed outside while you were editing; your text stays, the other version is kept as {name}",
                        tab.title
                    );
                }
                Err(err) => {
                    self.status = format!(
                        "{} was changed outside; the other version could not be kept: {err}",
                        tab.title
                    );
                }
            }
            return true;
        }
        tab.view.reload(&content);
        self.status = format!("{} was changed outside", tab.title);
        was_deleted
    }

    fn show_notices(&mut self) {
        for notice in self.state.take_notices() {
            self.status = format!("Note: {notice}");
        }
    }

    /// UC-002 step 7: the note overview, recently opened first (BR-008).
    fn refresh_overview(&mut self) {
        let Some(workspace) = &self.workspace else {
            self.overview.clear();
            return;
        };
        let root = workspace.root().to_owned();
        let notes = workspace.listing().notes.clone();
        let state = &mut self.state;
        self.overview = note_overview(&notes, |id| state.note_last_opened(&root, id));
    }

    /// UC-001 step 2: check the chosen directory, then ask for the options.
    fn check_new_directory(&mut self, path: String) {
        let directory = PathBuf::from(path.trim());
        self.create = Some(match check_directory(&directory, &self.surroundings()) {
            Ok(()) => CreateDialog::Options {
                directory,
                format: NoteFormat::default(),
                git: false,
            },
            Err(problem) => directory_dialog(path, &problem),
        });
    }

    /// UC-001 step 5 / A3: create the workspace and open its Home note.
    fn create_workspace(&mut self, request: NewWorkspace) {
        if let Err(failures) = self.flush_all() {
            self.create = None;
            self.status = format!(
                "No workspace was created, because changes could not be stored: {}",
                failures.join("; ")
            );
            return;
        }
        let home = self.formats.new_note(request.default_format, HOME_TITLE);
        match create_workspace(&request, &home, &self.surroundings(), &self.git) {
            Ok(workspace) => {
                let root = workspace.root().display().to_string();
                self.switch_to(workspace);
                self.open_note(&NoteId::from_title(HOME_TITLE));
                self.refresh_overview();
                self.status = format!("Workspace created in {root}");
                self.create = None;
            }
            // A1 after the check of step 5: the directory filled up meanwhile.
            Err(err) if matches!(err.problem, CreateProblem::Directory(_)) => {
                let CreateProblem::Directory(problem) = &err.problem else {
                    unreachable!()
                };
                let path = request.directory.display().to_string();
                self.create = Some(directory_dialog(path, problem));
            }
            // A5 and A6.
            Err(err) => {
                self.status = format!("The workspace could not be created: {err}");
                if !err.leftovers.is_empty() {
                    let left: Vec<_> = err
                        .leftovers
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    self.status
                        .push_str(&format!("; could not remove: {}", left.join(", ")));
                }
                self.create = None;
            }
        }
    }

    /// Writes the app state when a change is due (ADR-008, NFR-009).
    fn write_state(&mut self, ctx: &egui::Context) {
        if let Err(err) = self.state.write_if_due(Instant::now()) {
            self.status = err.to_string();
        }
        self.show_notices();
        if self.state.has_unwritten_changes() {
            ctx.request_repaint_after(STATE_WRITE_DELAY);
        }
    }

    fn show_open_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.open else {
            return;
        };
        let mut action = OpenAction::None;
        egui::Window::new("Open workspace")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if !dialog.recent.is_empty() {
                    ui.label("Recently opened:");
                    for dir in &dialog.recent {
                        if ui.button(dir.display().to_string()).clicked() {
                            action = OpenAction::Open(dir.clone());
                        }
                    }
                    ui.separator();
                }
                ui.label("Another directory:");
                let edit = ui.text_edit_singleline(&mut dialog.path);
                if dialog.path.is_empty() && dialog.problem.is_none() && dialog.recent.is_empty() {
                    edit.request_focus();
                }
                if let Some(problem) = &dialog.problem {
                    ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
                }
                match &dialog.offer {
                    Some(OpenOffer::Create(dir)) => {
                        if ui.button("Create a new workspace there").clicked() {
                            action = OpenAction::CreateAt(dir.clone());
                        }
                    }
                    Some(OpenOffer::Outer { outer, inner }) => {
                        if ui.button(format!("Open {}", outer.display())).clicked() {
                            action = OpenAction::OpenAnyway(outer.clone());
                        }
                        if ui
                            .button(format!("Open {} anyway", inner.display()))
                            .clicked()
                        {
                            action = OpenAction::OpenAnyway(inner.clone());
                        }
                    }
                    None => {}
                }
                ui.horizontal(|ui| {
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button("Open").clicked() || enter) && !dialog.path.trim().is_empty() {
                        action = OpenAction::Open(PathBuf::from(dialog.path.trim()));
                    }
                    if ui.button("Cancel").clicked() {
                        action = OpenAction::Cancel;
                    }
                });
            });

        match action {
            OpenAction::None => {}
            // A10: the open workspace stays open.
            OpenAction::Cancel => self.open = None,
            OpenAction::Open(dir) => self.request_open(&dir),
            // A5 step 2 and A6: skip only the check for an enclosing workspace.
            OpenAction::OpenAnyway(dir) => {
                match check_readable_directory(&dir, &self.surroundings()) {
                    Ok(dir) => self.open_directory(&dir),
                    Err(problem) => {
                        let problem = format!("{problem}. Choose another directory.");
                        self.open =
                            Some(self.open_dialog(dir.display().to_string(), Some(problem), None));
                    }
                }
            }
            // A4: UC-001 from its step 2.
            OpenAction::CreateAt(dir) => {
                self.open = None;
                self.check_new_directory(dir.display().to_string());
            }
        }
    }

    fn show_create_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.create else {
            return;
        };
        let mut action = DialogAction::None;
        egui::Window::new("New workspace")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| match dialog {
                CreateDialog::Directory {
                    path,
                    problem,
                    offer_open,
                } => {
                    ui.label("Directory for the new workspace (new or empty):");
                    let edit = ui.text_edit_singleline(path);
                    if path.is_empty() && problem.is_none() {
                        edit.request_focus();
                    }
                    if let Some(problem) = problem {
                        ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
                    }
                    if let Some(dir) = offer_open {
                        if ui.button("Open it as a workspace instead").clicked() {
                            action = DialogAction::OpenInstead(dir.clone());
                        }
                    }
                    ui.horizontal(|ui| {
                        let enter =
                            edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (ui.button("Next").clicked() || enter) && !path.trim().is_empty() {
                            action = DialogAction::Check(path.clone());
                        }
                        if ui.button("Cancel").clicked() {
                            action = DialogAction::Cancel;
                        }
                    });
                }
                CreateDialog::Options {
                    directory,
                    format,
                    git,
                } => {
                    ui.label(format!("Directory: {}", directory.display()));
                    ui.label("Default format of new notes:");
                    for option in NoteFormat::ALL {
                        ui.radio_value(format, option, option.name());
                    }
                    ui.checkbox(git, "Create as git repository");
                    ui.horizontal(|ui| {
                        if *git {
                            if ui.button("Next").clicked() {
                                action = DialogAction::AskRemote(directory.clone(), *format);
                            }
                        } else if ui.button("Create").clicked() {
                            action = DialogAction::Create(NewWorkspace {
                                directory: directory.clone(),
                                default_format: *format,
                                git: None,
                            });
                        }
                        if ui.button("Cancel").clicked() {
                            action = DialogAction::Cancel;
                        }
                    });
                }
                CreateDialog::Remote {
                    directory,
                    format,
                    address,
                } => {
                    ui.label("Address of the git remote (optional):");
                    ui.text_edit_singleline(address);
                    ui.label(
                        "The remote is not contacted now; the first git sync shows whether it is reachable.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let remote =
                                Some(address.trim().to_owned()).filter(|a| !a.is_empty());
                            action = DialogAction::Create(NewWorkspace {
                                directory: directory.clone(),
                                default_format: *format,
                                git: Some(GitRepositoryOption { remote }),
                            });
                        }
                        if ui.button("Cancel").clicked() {
                            action = DialogAction::Cancel;
                        }
                    });
                }
            });

        match action {
            DialogAction::None => {}
            // A4: nothing is created.
            DialogAction::Cancel => self.create = None,
            DialogAction::Check(path) => self.check_new_directory(path),
            DialogAction::AskRemote(directory, format) => {
                self.create = Some(CreateDialog::Remote {
                    directory,
                    format,
                    address: String::new(),
                });
            }
            // A2: open the non-empty directory as an existing workspace
            // (UC-002, from its step 4).
            DialogAction::OpenInstead(dir) => {
                self.create = None;
                self.request_open(&dir);
            }
            DialogAction::Create(request) => self.create_workspace(request),
        }
    }

    /// UC-004 step 2: opens `id` in a tab, or activates its tab (A1), and
    /// lists it first in the note overview.
    fn open_note(&mut self, id: &NoteId) {
        if self.open_tab(id, true) {
            self.persist_tabs();
            self.refresh_overview();
        }
    }

    /// Opens `id` in a tab; with `viewed`, records it as the most recently
    /// opened note. Returns whether a tab shows the note now.
    fn open_tab(&mut self, id: &NoteId, viewed: bool) -> bool {
        let Some(workspace) = &self.workspace else {
            return false;
        };
        let root = workspace.root().to_owned();
        if let Some(index) = self.tabs.find(id) {
            self.tabs.activate(index);
        } else {
            let (note, path) = match (workspace.read(id), workspace.note_path(id)) {
                (Ok(note), Ok(path)) => (note, path),
                (Err(err), _) | (_, Err(err)) => {
                    // UC-004 A10: no tab for a note that cannot be read.
                    self.status = format!("{} cannot be read: {err}", id.title());
                    return false;
                }
            };
            let format = workspace
                .listing()
                .notes
                .iter()
                .find(|entry| entry.id == note.id)
                .map_or(workspace.default_format(), |entry| entry.format);
            let highlight = Arc::clone(&self.highlight);
            let opened = self.tabs.open(id.clone(), || OpenTab {
                view: SourceView::new(note.id.as_str(), &note.body, format, highlight),
                path,
                title: note.title(),
                autosave: Autosave::default(),
                saving: false,
                error: None,
                deleted: false,
                kept_outside: None,
            });
            // UC-004 A6: the tab used longest ago was closed; its changes
            // are stored first.
            if let Some((closed, mut tab)) = opened.closed {
                let _ = self.flush_tab_now(&mut tab);
                self.status = format!("Closed the tab {}, used longest ago", closed.title());
            }
        }
        if viewed {
            self.state.note_viewed(&root, id, now());
        }
        true
    }

    /// Remembers the open tabs and the active one (UC-004 step 6).
    fn persist_tabs(&mut self) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let active = self.tabs.active_index();
        let tabs = (0..self.tabs.len())
            .map(|i| TabState {
                note: self.tabs.note(i).clone(),
                position: u32::try_from(i + 1).unwrap_or(u32::MAX),
                editing_view: EditingView::Source,
                active: Some(i) == active,
            })
            .collect();
        let root = workspace.root().to_owned();
        self.state.set_tabs(&root, tabs);
    }

    /// Stores a tab's changes now (UC-005 BR-001).
    fn flush_tab_now(&self, tab: &mut OpenTab) -> Result<(), String> {
        if !tab.autosave.is_unstored() {
            return Ok(());
        }
        let started = Instant::now();
        match self.saver.save_now(&tab.path, &tab.view.document().text()) {
            Ok(()) => {
                tab.autosave.stored(started);
                tab.error = None;
                tab.deleted = false;
                Ok(())
            }
            Err(err) => {
                tab.autosave.failed(started);
                tab.error = Some(err.clone());
                Err(format!("{}: {err}", tab.title))
            }
        }
    }

    /// Stores every tab's changes now; returns the failures.
    fn flush_all(&mut self) -> Result<(), Vec<String>> {
        let mut tabs = std::mem::take(&mut self.tabs);
        let failures: Vec<String> = tabs
            .iter_mut()
            .filter_map(|(_, tab)| self.flush_tab_now(tab).err())
            .collect();
        self.tabs = tabs;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }

    /// UC-004 A5: closes a tab, storing its changes first; when they cannot
    /// be stored, asks first (UC-005 A14 step 3).
    fn close_tab(&mut self, id: &NoteId, force: bool) {
        let Some(index) = self.tabs.find(id) else {
            return;
        };
        let mut tabs = std::mem::take(&mut self.tabs);
        let result = self.flush_tab_now(tabs.get_mut(index));
        self.tabs = tabs;
        if let (Err(err), false) = (result, force) {
            self.close_confirm = Some(CloseConfirm::Tab(id.clone(), err));
            return;
        }
        self.tabs.close(index);
        self.persist_tabs();
    }

    /// UC-005 step 4: stores due tabs in the background and takes in the
    /// outcomes (FR-021, A14, A15).
    fn autosave(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let mut busy = false;
        for (_, tab) in self.tabs.iter_mut() {
            if !tab.saving && tab.autosave.is_due(now) {
                self.saver.save(SaveJob {
                    path: tab.path.clone(),
                    text: tab.view.document().text(),
                    started: now,
                });
                tab.saving = true;
            }
            busy |= tab.saving || tab.autosave.is_unstored();
        }
        for result in self.saver.poll() {
            let Some((_, tab)) = self.tabs.iter_mut().find(|(_, t)| t.path == result.path) else {
                continue;
            };
            tab.saving = false;
            match result.result {
                Ok(()) => {
                    tab.autosave.stored(result.started);
                    tab.error = None;
                    if std::mem::take(&mut tab.deleted) {
                        // UC-005 A15: a note deleted outside is stored again.
                        self.status = format!("Stored {} again at its place", tab.title);
                    } else {
                        self.status = format!("Saved {}", tab.title);
                    }
                }
                Err(err) => {
                    tab.autosave.failed(Instant::now());
                    self.status = format!("{} could not be stored: {err}", tab.title);
                    tab.error = Some(err);
                }
            }
        }
        if busy {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// UC-005 A14 step 3: asks before closing with changes not stored.
    fn show_close_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.close_confirm else {
            return;
        };
        let (what, error) = match confirm {
            CloseConfirm::Tab(id, err) => (format!("the tab {}", id.title()), err.clone()),
            CloseConfirm::App(err) => ("Underline".to_owned(), err.clone()),
        };
        let mut answer = None;
        egui::Window::new("Changes not stored")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Changes could not be stored: {error}"));
                ui.label(format!("Close {what} anyway and lose these changes?"));
                ui.horizontal(|ui| {
                    if ui.button("Keep open").clicked() {
                        answer = Some(false);
                    }
                    if ui.button("Close anyway").clicked() {
                        answer = Some(true);
                    }
                });
            });
        match (answer, self.close_confirm.take()) {
            (Some(true), Some(CloseConfirm::Tab(id, _))) => self.close_tab(&id, true),
            (Some(true), Some(CloseConfirm::App(_))) => {
                self.close_anyway = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (None, confirm) => self.close_confirm = confirm,
            (Some(_), _) => {}
        }
    }

    /// UC-003 step 2: the folder of the active note and the default format
    /// are preselected.
    fn start_new_note(&mut self) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let folder = self
            .tabs
            .active()
            .and_then(|(note, _)| note.folder())
            .unwrap_or_default()
            .to_owned();
        self.new_note = Some(NoteDialog {
            title: String::new(),
            folder,
            format: workspace.default_format(),
            problem: None,
            existing: None,
        });
    }

    fn show_note_dialog(&mut self, ctx: &egui::Context) {
        let (Some(dialog), Some(workspace)) = (&mut self.new_note, &self.workspace) else {
            return;
        };
        let mut action = NoteAction::None;
        egui::Window::new("New note")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Title:");
                let edit = ui.text_edit_singleline(&mut dialog.title);
                if dialog.title.is_empty() && dialog.problem.is_none() {
                    edit.request_focus();
                }
                ui.label("Folder (empty for the top folder, new folders are created):");
                ui.text_edit_singleline(&mut dialog.folder);
                ui.horizontal(|ui| {
                    ui.label("Note format:");
                    for option in NoteFormat::ALL {
                        ui.radio_value(&mut dialog.format, option, option.name());
                    }
                });
                // Step 2: the file name the note will get, while the user types.
                let new = NewNote {
                    folder: dialog.folder.clone(),
                    title: dialog.title.clone(),
                    format: dialog.format,
                };
                if let Ok(id) = workspace.plan_note(&new) {
                    ui.weak(format!("File: {}", id.file_name(dialog.format)));
                }
                if let Some(problem) = &dialog.problem {
                    ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
                }
                if let Some(existing) = &dialog.existing {
                    if ui.button("Open the existing note").clicked() {
                        action = NoteAction::OpenExisting(existing.clone());
                    }
                }
                ui.horizontal(|ui| {
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Create").clicked() || enter {
                        action = NoteAction::Create;
                    }
                    if ui.button("Cancel").clicked() {
                        action = NoteAction::Cancel;
                    }
                });
            });

        match action {
            NoteAction::None => {}
            // A6: nothing is created.
            NoteAction::Cancel => self.new_note = None,
            // A3: the existing note opens unchanged.
            NoteAction::OpenExisting(id) => {
                self.new_note = None;
                self.open_note(&id);
                self.refresh_overview();
            }
            NoteAction::Create => self.create_note(),
        }
    }

    /// UC-003 steps 4–6.
    fn create_note(&mut self) {
        let (Some(dialog), Some(workspace)) = (&mut self.new_note, &mut self.workspace) else {
            return;
        };
        let new = NewNote {
            folder: dialog.folder.clone(),
            title: dialog.title.clone(),
            format: dialog.format,
        };
        let content = self
            .formats
            .new_note(new.format, &heading_title(&new.title));
        match workspace.create_note(&new, &content) {
            Ok(id) => {
                self.new_note = None;
                self.open_note(&id);
                if let Some((_, tab)) = self.tabs.active_mut() {
                    tab.view.focus_end();
                }
                self.status = format!("Created the note {}", id.title());
            }
            Err(NewNoteProblem::Exists(id)) => {
                dialog.problem = Some(format!(
                    "The note {} exists. Change the title or the folder, or open it.",
                    id.title()
                ));
                dialog.existing = Some(id);
            }
            // A5: reported in the status bar; nothing is left behind.
            Err(problem @ NewNoteProblem::Io { .. }) => {
                self.new_note = None;
                self.status = format!("The note could not be created: {problem}");
            }
            // A1 and A4: the dialog keeps what the user entered.
            Err(problem) => {
                let mut text = problem.to_string();
                if let Some(first) = text.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                dialog.problem = Some(format!("{text}."));
                dialog.existing = None;
            }
        }
    }

    fn start_create(&mut self) {
        if self.create.is_none() {
            self.open = None;
            self.create = Some(CreateDialog::Directory {
                path: String::new(),
                problem: None,
                offer_open: None,
            });
        }
    }

    fn start_open(&mut self) {
        if self.open.is_none() {
            self.create = None;
            self.open = Some(self.open_dialog(String::new(), None, None));
        }
    }
}

impl eframe::App for UnderlineApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.flush_all();
        let _ = self.state.write();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // UC-005 BR-001: closing Underline stores every change first; when
        // that fails, the user is asked (A14 step 3).
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_anyway {
            if let Err(failures) = self.flush_all() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_confirm = Some(CloseConfirm::App(failures.join("; ")));
            }
        }
        self.handle_outside_changes();
        self.show_close_confirm(ctx);
        self.show_open_dialog(ctx);
        self.show_create_dialog(ctx);
        self.show_note_dialog(ctx);
        self.write_state(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open workspace…").clicked() {
                    self.start_open();
                }
                if ui.button("New workspace…").clicked() {
                    self.start_create();
                }
                ui.separator();
                ui.label("Wiki project URL");
                ui.text_edit_singleline(&mut self.wiki_url);
                if ui.button("Check").clicked() {
                    self.status = match WikiRemote::from_project_url(&self.wiki_url) {
                        Ok(remote) => format!("Wiki repository: {}", remote.clone_url()),
                        Err(err) => err.to_string(),
                    };
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let mut parts = Vec::new();
            if let Some(workspace) = &self.workspace {
                parts.push(workspace.root().display().to_string());
            }
            if let Some(git_state) = &self.git_state {
                parts.push(git_state.clone());
            }
            if !self.status.is_empty() {
                parts.push(self.status.clone());
            }
            ui.label(parts.join("  |  "));
        });

        if self.workspace.is_none() {
            // UC-002 A2 step 2: offer to open or create a workspace.
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.heading("No workspace is open");
                    if ui.button("Open workspace…").clicked() {
                        self.start_open();
                    }
                    if ui.button("New workspace…").clicked() {
                        self.start_create();
                    }
                });
            });
            return;
        }

        egui::SidePanel::left("pages").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Notes");
                if ui.button("New note…").clicked() {
                    self.start_new_note();
                }
            });
            ui.separator();
            let mut selected = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in &self.overview {
                    let active = self.tabs.active().is_some_and(|(n, _)| n == &entry.id);
                    if ui.selectable_label(active, &entry.label).clicked() {
                        selected = Some(entry.id.clone());
                    }
                }
            });
            if let Some(id) = selected {
                self.open_note(&id);
                self.refresh_overview();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // UC-004 A5: the tab bar, left to right.
            let mut activate = None;
            let mut close = None;
            egui::ScrollArea::horizontal()
                .id_salt("tabs")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let active = self.tabs.active_index();
                        for (index, (id, tab)) in self.tabs.iter().enumerate() {
                            let label = ui
                                .selectable_label(Some(index) == active, tab.label())
                                .on_hover_text(tab.path.display().to_string());
                            if label.clicked() {
                                activate = Some(index);
                            }
                            if label.middle_clicked() || ui.small_button("×").clicked() {
                                close = Some(id.clone());
                            }
                            ui.separator();
                        }
                    });
                });
            if let Some(index) = activate {
                self.tabs.activate(index);
                let id = self.tabs.note(index).clone();
                if let Some(workspace) = &self.workspace {
                    let root = workspace.root().to_owned();
                    self.state.note_viewed(&root, &id, now());
                }
                self.persist_tabs();
                self.refresh_overview();
            }
            if let Some(id) = close {
                self.close_tab(&id, false);
            }
            ui.separator();
            let Some((_, tab)) = self.tabs.active_mut() else {
                ui.label("Select or create a note.");
                return;
            };
            if let Some(err) = &tab.error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("Not stored: {err}. Underline keeps trying."),
                );
            }
            if tab.view.show(ui) {
                tab.autosave.changed(Instant::now());
            }
        });
        self.autosave(ctx);
    }
}

/// The form two workspace directories are compared in.
fn underline_root(dir: &Path) -> PathBuf {
    std::path::absolute(dir).unwrap_or_else(|_| dir.to_owned())
}

/// The directory stage again, explaining why the directory was refused.
fn directory_dialog(path: String, problem: &DirectoryProblem) -> CreateDialog {
    let offer_open = match problem {
        // A1: offer to open it instead (A2).
        DirectoryProblem::NotEmpty(dir) => Some(dir.clone()),
        _ => None,
    };
    let hint = match problem {
        DirectoryProblem::NotEmpty(_) => " Choose another directory or open it as a workspace.",
        DirectoryProblem::InsideWorkspace { .. } => " Choose another directory.",
        _ => "",
    };
    CreateDialog::Directory {
        path,
        problem: Some(format!("{problem}.{hint}")),
        offer_open,
    }
}
