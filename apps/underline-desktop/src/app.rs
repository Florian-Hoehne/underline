use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;
use underline_core::{
    check_directory, create_workspace, now, user_settings_dir, CreateProblem, DirectoryProblem,
    EditingView, Git, GitRepositoryOption, NewWorkspace, Note, NoteFormat, NoteId, StateNotice,
    StateStore, Surroundings, Workspace, HOME_TITLE, STATE_WRITE_DELAY,
};
use underline_facade::Registry;
use underline_wiki::WikiRemote;

/// Main window: note overview on the left, editor in the middle.
pub struct UnderlineApp {
    workspace: Workspace,
    formats: Registry,
    pages: Vec<NoteId>,
    current: Option<Note>,
    new_page: String,
    wiki_url: String,
    status: String,
    create: Option<CreateDialog>,
    /// Workspaces, tabs, and recent notes kept between starts (ADR-007).
    state: StateStore,
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

/// What the user did in the create dialog this frame.
enum DialogAction {
    None,
    Cancel,
    Check(String),
    AskRemote(PathBuf, NoteFormat),
    OpenInstead(PathBuf),
    Create(NewWorkspace),
}

impl UnderlineApp {
    pub fn new(workspace: Workspace, state: StateStore, notice: Option<StateNotice>) -> Self {
        let mut app = Self {
            workspace,
            formats: Registry::default(),
            pages: Vec::new(),
            current: None,
            new_page: String::new(),
            wiki_url: String::new(),
            status: String::new(),
            create: None,
            state,
        };
        let workspace = app.workspace.clone();
        app.switch_to(workspace);
        if let Some(notice) = notice {
            app.status = format!("Note: {notice}");
        }
        app
    }

    fn surroundings(&self) -> Surroundings {
        Surroundings {
            known_workspaces: vec![self.workspace.root().to_owned()],
            user_settings_dir: user_settings_dir(),
        }
    }

    /// Makes `workspace` the open one; the previous one is closed (UC-001 step 6).
    ///
    /// The workspace is remembered as the last opened one, and the note of its
    /// remembered active tab is opened again (ADR-007).
    fn switch_to(&mut self, workspace: Workspace) {
        self.workspace = workspace;
        self.current = None;
        self.state.workspace_opened(self.workspace.root(), now());
        self.refresh();
        if let Some(note) = self.state.active_note(self.workspace.root()) {
            self.open(&note);
        }
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
        let home = self.formats.new_note(request.default_format, HOME_TITLE);
        match create_workspace(&request, &home, &self.surroundings(), &Git::new()) {
            Ok(workspace) => {
                let root = workspace.root().display().to_string();
                self.switch_to(workspace);
                self.open(&NoteId::from_title(HOME_TITLE));
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

    /// Writes the app state when a change is due (ADR-007, NFR-009).
    fn write_state(&mut self, ctx: &egui::Context) {
        if let Err(err) = self.state.write_if_due(Instant::now()) {
            self.status = err.to_string();
        }
        if self.state.has_unwritten_changes() {
            ctx.request_repaint_after(STATE_WRITE_DELAY);
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
                        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
                    ui.label("The remote is not contacted now; the first git sync shows whether it is reachable.");
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let remote = Some(address.trim().to_owned()).filter(|a| !a.is_empty());
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
            // A2: open the non-empty directory as an existing workspace (UC-002).
            DialogAction::OpenInstead(dir) => {
                self.create = None;
                match Workspace::open(&dir) {
                    Ok(workspace) => {
                        self.switch_to(workspace);
                        self.status = format!("Opened {}", dir.display());
                    }
                    Err(err) => self.status = err.to_string(),
                }
            }
            DialogAction::Create(request) => self.create_workspace(request),
        }
    }

    fn refresh(&mut self) {
        match self.workspace.list() {
            Ok(pages) => self.pages = pages,
            Err(err) => self.status = err.to_string(),
        }
    }

    fn open(&mut self, id: &NoteId) {
        match self.workspace.read(id) {
            Ok(note) => {
                // Tabs and the facade come with later use cases; until then the
                // one open note is the active tab, shown as source.
                self.state
                    .note_opened(self.workspace.root(), &note.id, EditingView::Source, now());
                self.current = Some(note);
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn save(&mut self) {
        if let Some(note) = &self.current {
            self.status = match self.workspace.write(note) {
                Ok(()) => format!("Saved {}", note.id),
                Err(err) => err.to_string(),
            };
            self.refresh();
        }
    }
}

impl eframe::App for UnderlineApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.state.write();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_create_dialog(ctx);
        self.write_state(ctx);

        egui::TopBottomPanel::top("wiki").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("New workspace…").clicked() && self.create.is_none() {
                    self.create = Some(CreateDialog::Directory {
                        path: String::new(),
                        problem: None,
                        offer_open: None,
                    });
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
            ui.label(format!(
                "{}  |  {}",
                self.workspace.root().display(),
                self.status
            ));
        });

        egui::SidePanel::left("pages").show(ctx, |ui| {
            ui.heading("Pages");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_page);
                if ui.button("New").clicked() && !self.new_page.trim().is_empty() {
                    self.current = Some(Note::new(&self.new_page, ""));
                    self.new_page.clear();
                }
            });
            ui.separator();
            let mut selected = None;
            for id in &self.pages {
                let active = self.current.as_ref().is_some_and(|n| &n.id == id);
                if ui.selectable_label(active, id.as_str()).clicked() {
                    selected = Some(id.clone());
                }
            }
            if let Some(id) = selected {
                self.open(&id);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(title) = self.current.as_ref().map(Note::title) else {
                ui.label("Select or create a page.");
                return;
            };
            ui.horizontal(|ui| {
                ui.heading(title);
                if ui.button("Save").clicked() {
                    self.save();
                }
            });
            if let Some(note) = &mut self.current {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut note.body).code_editor(),
                );
            }
        });
    }
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
