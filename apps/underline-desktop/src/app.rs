use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::settings::SettingsState;
use eframe::egui;
use underline_core::{
    asciidoc_anchor, check_directory, check_open_directory, check_readable_directory,
    create_workspace, dropped_link, embed_file, format_local_time, heading_anchor, heading_title,
    note_overview, now, outside_copy_path, relative, resolve_link, store_pasted_image,
    user_settings_dir, Autosave, CreateProblem, DeleteProblem, DeletedNote, DirectoryProblem,
    EditingView, EmbedError, Git, GitRepositoryOption, LinkTarget, NewNote, NewNoteProblem,
    NewWorkspace, NoteFormat, NoteId, NoteMove, NoteProperties, NoteSaver, OpenProblem,
    OverviewEntry, OwnWrites, Rename, SaveJob, SettingKey, StateNotice, StateStore, Surroundings,
    TabList, TabState, Theme, Workspace, WorkspaceEvent, WorkspaceWatcher, HOME_TITLE,
    STATE_WRITE_DELAY,
};
use underline_core::{InputMode as Keys, NoteEntry, Query, SearchRequest, SearchResult, Searcher};
use underline_editor::FOUND_COLOR;
use underline_editor::{
    ClipImage, EditorCommand, FacadeView, InputMode, SharedClipboard, SourceView, SystemClipboard,
    Tool,
};
use underline_facade::blocks::{heading_offset, parse_blocks};
use underline_facade::diagram::Diagrams;
use underline_facade::formatting::Format;
use underline_facade::highlight::HighlightSet;
use underline_facade::images::{encode_png, Images};
use underline_facade::index::{indexes, Indexes};
use underline_facade::rename::{links_to_note, rename_note, Edit, OpenNotes, Renamed};
use underline_facade::themes::Palette;
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
    /// Renders the diagrams of the open notes (FR-036, FR-037, ADR-009).
    diagrams: Arc<Diagrams>,
    /// Loads the images of the open notes (UC-004 step 3).
    images: Arc<Images>,
    /// The clipboard of the system, with HTML and images (ADR-014).
    clipboard: SharedClipboard,
    /// The user and workspace settings and those in effect (UC-010), and
    /// the theme the window shows now.
    settings: SettingsState,
    applied_theme: Option<Theme>,
    /// Whether the note overview and the tool panel were being resized.
    resizing: [bool; 2],
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
    rename: Option<RenameDialog>,
    /// UC-007: the question before deleting, the undo offered right after
    /// it, the deleted notes, and the dialogs to restore and remove them.
    delete: Option<DeleteDialog>,
    /// UC-008: the search text, the searcher of the workspace, its newest
    /// answer, what it was last asked, the found entry chosen with the
    /// keyboard, and whether the search field is to take the focus.
    search_text: String,
    searcher: Option<Searcher>,
    search_result: Option<SearchResult>,
    search_asked: Option<(String, u64)>,
    search_cursor: Option<usize>,
    focus_search: bool,
    /// Counts the changes of the note overview, so a search runs again.
    overview_generation: u64,
    /// UC-008 A6: the find field of the active tab.
    find: Option<FindBar>,
    /// The note chosen in the note overview while it has the keyboard.
    overview_chosen: Option<NoteId>,
    undo_delete: Option<UndoDelete>,
    deleted: Vec<DeletedNote>,
    restore: Option<RestoreDialog>,
    purge: Option<Purge>,
    /// The question a link or image tool asks (UC-005 A2).
    tool_dialog: Option<ToolDialog>,
    /// The user agreed to open web addresses in this session (UC-004 A4).
    web_allowed: bool,
    /// A web address waiting for that agreement.
    web_confirm: Option<String>,
    /// Workspaces, tabs, and recent notes kept between starts (ADR-008).
    state: StateStore,
}

/// One open tab (UC-004): the note in its source view, with its autosave
/// clock (UC-005).
struct OpenTab {
    view: SourceView,
    /// The facade of the same text (ADR-012).
    facade: FacadeView,
    /// The editing view the tab shows (UC-004 BR-003, FR-010).
    editing_view: EditingView,
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
    /// What the file held when Underline last read or stored it; a file
    /// that holds anything else was changed outside (ADR-011).
    on_disk: String,
    /// The text a running background save writes.
    saving_text: Option<String>,
    /// The next save stores a change Underline made, not the user, such as
    /// links updated by a rename (UC-006 A7); it does not replace the report
    /// in the status bar.
    quiet_save: bool,
    /// The lists of the tool panel and the revision of the text they were
    /// made from (FR-041 to FR-043).
    indexes: Option<(u64, Indexes)>,
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

/// What the link and image tools ask (UC-005 A2).
enum ToolDialog {
    /// A web address, or a note of the workspace.
    Link { address: String, filter: String },
    /// An image file; `large` is set when it is larger than 10 MB (A19).
    Image {
        path: String,
        large: Option<(PathBuf, u64)>,
    },
}

/// The answer of the link or image dialog.
enum ToolAnswer {
    Address(String),
    /// A dropped link with the text it carries (UC-005 A12).
    Link(String, Option<String>),
    Note(NoteId),
    /// A file to embed; `true` when the user agreed to copy a large one.
    File(PathBuf, bool),
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

/// The dialog of UC-006 Rename Note (steps 2–3).
struct RenameDialog {
    from: NoteId,
    title: String,
    folder: String,
    /// Why the last confirmation was refused (A1–A3).
    problem: Option<String>,
    /// The title is selected when the dialog opens.
    select: bool,
}

/// The question of UC-007 step 2, or of A8 when the note cannot be kept.
struct DeleteDialog {
    id: NoteId,
    /// Links in other notes that lead to the note, and in how many notes.
    links: usize,
    notes: usize,
    /// A8: why the note cannot be kept; the user confirms deleting for good.
    cannot_keep: Option<String>,
}

/// The find field of the active tab (UC-008 A6).
struct FindBar {
    text: String,
    focus: bool,
    /// The text the selection was last moved for.
    sought: String,
}

/// What the user did in the find field this frame.
#[derive(PartialEq)]
enum FindAction {
    None,
    Next,
    Previous,
    Close,
}

/// A found note or line chosen in the search results (UC-008 step 3).
struct FoundChoice {
    id: NoteId,
    /// The bytes of the first word found in the line, or `None` for a note.
    place: Option<std::ops::Range<usize>>,
}

/// The undo offered right after deleting (UC-007 A4), while the status bar
/// still shows the report.
struct UndoDelete {
    deleted: DeletedNote,
    reopen: bool,
    status: String,
}

/// UC-007 A6: the place of a note to restore is taken.
struct RestoreDialog {
    deleted: DeletedNote,
    reopen: bool,
    title: String,
    folder: String,
    problem: Option<String>,
}

/// UC-007 A7: what is about to be removed for good.
enum Purge {
    One(DeletedNote),
    All,
}

/// What the user did in the rename dialog this frame.
enum RenameAction {
    None,
    Cancel,
    Rename,
}

/// The open tabs as a rename sees them (UC-006 A7): a linking note that is
/// open is changed in its tab, which saves it with its next autosave.
struct TabEdits<'a> {
    tabs: &'a mut TabList<OpenTab>,
    /// The renamed note, whose tab is changed after the move.
    renamed: &'a NoteId,
}

impl OpenNotes for TabEdits<'_> {
    fn text(&self, id: &NoteId) -> Option<String> {
        if id == self.renamed {
            return None;
        }
        let index = self.tabs.find(id)?;
        Some(self.tabs.get(index).view.document().text())
    }

    fn edit(&mut self, id: &NoteId, edits: &[Edit]) {
        if let Some(index) = self.tabs.find(id) {
            let tab = self.tabs.get_mut(index);
            tab.view.apply_edits(edits);
            tab.quiet_save = !tab.autosave.is_unstored();
            tab.autosave.changed(Instant::now());
        }
    }
}

/// The choice of a found note, or of one of its lines.
fn choice(note: &underline_core::FoundNote, line: Option<usize>) -> FoundChoice {
    let line = line.or((!note.lines.is_empty()).then_some(0));
    FoundChoice {
        id: note.id.clone(),
        place: line
            .and_then(|l| note.lines.get(l))
            .map(|l| l.first.clone()),
    }
}

/// UC-008 step 2, A1, A4, A5: the found notes in place of the note
/// overview; returns the note or line the user clicked. `cursor` is the
/// entry chosen with the keyboard.
fn found_list(
    ui: &mut egui::Ui,
    result: Option<&SearchResult>,
    text: &str,
    cursor: Option<(usize, Option<usize>)>,
) -> Option<FoundChoice> {
    let Some(result) = result else {
        ui.weak("Searching…");
        return None;
    };
    let mut clicked = None;
    if result.found.is_empty() {
        ui.weak(format!("No note holds “{}”.", text.trim()));
    }
    for (n, note) in result.found.iter().enumerate() {
        let places = match note.places {
            0 => "in the title or folder".to_owned(),
            1 => "1 place".to_owned(),
            p => format!("{p} places"),
        };
        let header = ui
            .selectable_label(
                cursor == Some((n, None)),
                egui::RichText::new(note.id.title()).strong(),
            )
            .on_hover_text(format!(
                "{} · {places}",
                note.id.folder().unwrap_or("top folder")
            ));
        if header.clicked() {
            clicked = Some(choice(note, None));
        }
        if cursor == Some((n, None)) {
            header.scroll_to_me(None);
        }
        ui.weak(format!(
            "{}{places}",
            note.id
                .folder()
                .map(|f| format!("{f} · "))
                .unwrap_or_default()
        ));
        for (l, line) in note.lines.iter().enumerate() {
            let mut job = egui::text::LayoutJob::default();
            let font = egui::TextStyle::Small.resolve(ui.style());
            let color = ui.visuals().text_color();
            let mut at = 0;
            for mark in &line.marks {
                job.append(
                    &line.shown[at..mark.start],
                    if at == 0 { 8.0 } else { 0.0 },
                    egui::TextFormat::simple(font.clone(), color),
                );
                job.append(
                    &line.shown[mark.clone()],
                    0.0,
                    egui::TextFormat {
                        background: FOUND_COLOR,
                        ..egui::TextFormat::simple(font.clone(), color)
                    },
                );
                at = mark.end;
            }
            job.append(
                &line.shown[at..],
                0.0,
                egui::TextFormat::simple(font.clone(), color),
            );
            job.wrap.max_rows = 2;
            job.wrap.max_width = ui.available_width();
            let row = ui.selectable_label(cursor == Some((n, Some(l))), job);
            if row.clicked() {
                clicked = Some(choice(note, Some(l)));
            }
            if cursor == Some((n, Some(l))) {
                row.scroll_to_me(None);
            }
        }
        if note.places > note.lines.len() {
            ui.weak(format!("    and {} more", note.places - note.lines.len()));
        }
        ui.add_space(4.0);
    }
    if result.unreadable > 0 {
        ui.separator();
        ui.weak(format!(
            "{} could not be searched.",
            count(result.unreadable, "note")
        ));
    }
    clicked
}

/// `n` things, as "1 link" or "3 links".
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{n} {thing}s")
    }
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
            diagrams: {
                let ctx = ctx.clone();
                Arc::new(Diagrams::start(move || ctx.request_repaint()))
            },
            clipboard: SystemClipboard::shared(),
            settings: SettingsState::load(),
            applied_theme: None,
            resizing: [false; 2],
            images: {
                let ctx = ctx.clone();
                Arc::new(Images::start(move || ctx.request_repaint()))
            },
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
            rename: None,
            delete: None,
            search_text: String::new(),
            searcher: None,
            search_result: None,
            search_asked: None,
            search_cursor: None,
            focus_search: false,
            overview_generation: 0,
            find: None,
            overview_chosen: None,
            undo_delete: None,
            deleted: Vec::new(),
            restore: None,
            purge: None,
            tool_dialog: None,
            web_allowed: false,
            web_confirm: None,
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
        let wake = self.ctx.clone();
        self.searcher = Some(Searcher::start(root.clone(), move || {
            wake.request_repaint()
        }));
        self.search_result = None;
        self.search_asked = None;
        self.find = None;
        self.settings.open_workspace(&root, &self.git);
        self.workspace = Some(workspace);
        self.tabs = TabList::default();
        self.state.workspace_opened(&root, now());
        // UC-004: the remembered tabs come back in their order; restoring
        // them does not change the order of the note overview.
        let remembered = self.state.tabs(&root);
        for tab in &remembered {
            self.open_tab(&tab.note, false, tab.editing_view);
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
        // UC-007 BR-005: notes deleted more than 30 days ago go for good.
        if let Some(workspace) = &self.workspace {
            workspace.purge_deleted(now());
        }
        self.undo_delete = None;
        self.refresh_deleted();
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
                    self.settings.reload_workspace();
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
        // Only a file that holds something else than Underline last read or
        // stored was changed outside; unstored edits make no difference.
        if content == tab.on_disk || tab.saving_text.as_deref() == Some(content.as_str()) {
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
        tab.on_disk = content;
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
        self.overview_generation += 1;
    }

    /// UC-008 step 2, A3: asks the searcher again when the search text,
    /// the notes, or a text open in a tab changed, takes in its answer, and
    /// marks the words in the open notes (BR-008).
    fn run_search(&mut self) {
        let text = self.search_text.trim().to_owned();
        let global = (!text.is_empty()).then(|| Query::parse(&text));
        let active = self.tabs.active_index();
        let find = self
            .find
            .as_ref()
            .filter(|f| !f.text.trim().is_empty())
            .map(|f| Query::parse(&f.text));
        for (index, (_, tab)) in self.tabs.iter_mut().enumerate() {
            let query = match &find {
                Some(query) if Some(index) == active => Some(query.clone()),
                _ => global.clone(),
            };
            tab.view.set_search(query);
        }
        let (Some(searcher), Some(workspace)) = (&self.searcher, &self.workspace) else {
            return;
        };
        if let Some(result) = searcher.poll() {
            if result.text == text {
                self.search_result = Some(result);
            }
        }
        if text.is_empty() {
            self.search_result = None;
            self.search_asked = None;
            self.search_cursor = None;
            return;
        }
        let revisions = self
            .tabs
            .iter()
            .fold(self.overview_generation, |sum, (_, tab)| {
                sum.wrapping_mul(31)
                    .wrapping_add(tab.view.document().revision())
            });
        let stamp = (text.clone(), revisions);
        if self.search_asked.as_ref() == Some(&stamp) {
            return;
        }
        let formats: std::collections::HashMap<&NoteId, NoteFormat> = workspace
            .listing()
            .notes
            .iter()
            .map(|n| (&n.id, n.format))
            .collect();
        let notes = self
            .overview
            .iter()
            .filter_map(|entry| {
                formats.get(&entry.id).map(|format| NoteEntry {
                    id: entry.id.clone(),
                    format: *format,
                })
            })
            .collect();
        let open = self
            .tabs
            .iter()
            .map(|(id, tab)| (id.clone(), tab.view.document().text()))
            .collect();
        searcher.search(SearchRequest { text, notes, open });
        self.search_asked = Some(stamp);
    }

    /// UC-008 step 4: opens a found note at the place found.
    fn open_found(&mut self, choice: FoundChoice) {
        self.open_note(&choice.id);
        let Some((id, tab)) = self.tabs.active_mut() else {
            return;
        };
        if *id != choice.id {
            return;
        }
        if let Some(place) = choice.place {
            let rope = tab.view.document().rope();
            let len = rope.len_bytes();
            let chars =
                rope.byte_to_char(place.start.min(len))..rope.byte_to_char(place.end.min(len));
            tab.view.select_chars(chars);
        }
        match tab.editing_view {
            EditingView::Facade => tab.facade.request_focus(),
            EditingView::Source => tab.view.focus_cursor(),
        }
    }

    /// UC-008 A6: moves the selection of the active tab to an occurrence of
    /// the find text: the first from the cursor on when the text changed,
    /// else the next or the previous one, wrapping around.
    fn find_in_note(&mut self, action: FindAction) {
        let Some(find) = &mut self.find else {
            return;
        };
        if action == FindAction::Close {
            self.find = None;
            if let Some((_, tab)) = self.tabs.active_mut() {
                match tab.editing_view {
                    EditingView::Facade => tab.facade.request_focus(),
                    EditingView::Source => tab.view.focus_cursor(),
                }
            }
            return;
        }
        let changed = find.sought != find.text;
        if !changed && action == FindAction::None {
            return;
        }
        find.sought = find.text.clone();
        let query = Query::parse(&find.text);
        let Some((_, tab)) = self.tabs.active_mut() else {
            return;
        };
        tab.view.set_search((!query.is_empty()).then_some(query));
        let found = tab.view.found_ranges().to_vec();
        if found.is_empty() {
            return;
        }
        let at = tab.view.selection_range().start;
        let chosen = match action {
            FindAction::Previous => found.iter().rev().find(|r| r.start < at).or(found.last()),
            FindAction::Next => found.iter().find(|r| r.start > at).or(found.first()),
            _ => found.iter().find(|r| r.start >= at).or(found.first()),
        };
        if let Some(range) = chosen.cloned() {
            tab.view.select_chars(range);
            if tab.editing_view == EditingView::Facade {
                tab.facade.scroll_to_cursor();
            }
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
        if self.open_tab(id, true, EditingView::Facade) {
            self.persist_tabs();
            self.refresh_overview();
        }
    }

    /// Opens `id` in a tab; with `viewed`, records it as the most recently
    /// opened note. Returns whether a tab shows the note now.
    fn open_tab(&mut self, id: &NoteId, viewed: bool, editing_view: EditingView) -> bool {
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
            let root = workspace.root().to_path_buf();
            let clipboard = Arc::clone(&self.clipboard);
            let opened = self.tabs.open(id.clone(), || OpenTab {
                view: {
                    let mut view = SourceView::new(note.id.as_str(), &note.body, format, highlight);
                    view.set_clipboard(clipboard);
                    view
                },
                facade: FacadeView::new(note.id.as_str())
                    .with_diagrams(Arc::clone(&self.diagrams))
                    .with_images(Arc::clone(&self.images), root, path.clone()),
                editing_view,
                path,
                title: note.title(),
                autosave: Autosave::default(),
                saving: false,
                error: None,
                deleted: false,
                kept_outside: None,
                on_disk: note.body.clone(),
                saving_text: None,
                quiet_save: false,
                indexes: None,
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
                editing_view: self.tabs.get(i).editing_view,
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
        let text = tab.view.document().text();
        match self.saver.save_now(&tab.path, &text) {
            Ok(()) => {
                tab.on_disk = text;
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
                let text = tab.view.document().text();
                tab.saving_text = Some(text.clone());
                self.saver.save(SaveJob {
                    path: tab.path.clone(),
                    text,
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
                    if let Some(text) = tab.saving_text.take() {
                        tab.on_disk = text;
                    }
                    if std::mem::take(&mut tab.quiet_save) {
                        // UC-006 A7: the rename's report stays.
                    } else if std::mem::take(&mut tab.deleted) {
                        // UC-005 A15: a note deleted outside is stored again.
                        self.status = format!("Stored {} again at its place", tab.title);
                    } else {
                        self.status = format!("Saved {}", tab.title);
                    }
                }
                Err(err) => {
                    tab.saving_text = None;
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

    /// UC-004 A2–A4: follows a link of the note at `note`.
    fn follow_link(&mut self, link: &str, note: &Path) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        match resolve_link(workspace.root(), note, link) {
            LinkTarget::Heading(anchor) => self.jump_to_heading(&anchor),
            LinkTarget::Note { id, heading } => {
                self.open_note(&id);
                if let Some(anchor) = heading {
                    self.jump_to_heading(&anchor);
                }
            }
            // A3: the missing note is offered for creation (UC-003).
            LinkTarget::MissingNote { folder, title } => {
                self.start_new_note();
                if let Some(dialog) = &mut self.new_note {
                    dialog.folder = folder;
                    dialog.title = title.clone();
                    dialog.problem = Some(format!("{title} does not exist yet. Create it?"));
                }
            }
            LinkTarget::Web(address) => {
                if self.web_allowed {
                    self.open_with_system(&address);
                } else {
                    self.web_confirm = Some(address);
                }
            }
            LinkTarget::File(path) => {
                if path.exists() {
                    self.open_with_system(&path.display().to_string());
                } else {
                    self.status = format!("{} does not exist", path.display());
                }
            }
            LinkTarget::Outside => {
                self.status = format!("{link} leads outside the workspace and is not followed");
            }
        }
    }

    /// Scrolls the active tab to the heading with `anchor` (UC-004 A2).
    fn jump_to_heading(&mut self, anchor: &str) {
        let Some((_, tab)) = self.tabs.active_mut() else {
            return;
        };
        let text = tab.view.document().text();
        let format = tab.view.document_format();
        let blocks = parse_blocks(&text, format);
        let slug = match format {
            NoteFormat::Markdown => heading_anchor,
            NoteFormat::AsciiDoc => asciidoc_anchor,
        };
        match heading_offset(&blocks, anchor, slug) {
            Some(offset) => {
                tab.view.jump_to_byte(offset);
                tab.facade.request_focus();
            }
            None => self.status = format!("The heading {anchor} was not found"),
        }
    }

    /// UC-004 step 4: the tool panel with the properties, outline, code
    /// index, and figure index of the active note; a click on an entry
    /// scrolls the tab to it (FR-019, FR-041 to FR-043).
    fn show_tool_panel(&mut self, ctx: &egui::Context) {
        let mut tool_width = None;
        let root = self.workspace.as_ref().map(|w| w.root().to_path_buf());
        let Some((_, tab)) = self.tabs.active_mut() else {
            return;
        };
        let revision = tab.view.document().revision();
        if tab
            .indexes
            .as_ref()
            .is_none_or(|(made, _)| *made != revision)
        {
            let blocks = tab.facade.current_blocks(&tab.view);
            tab.indexes = Some((revision, indexes(blocks)));
        }
        let properties = root.as_deref().and_then(|root| {
            NoteProperties::read(root, &tab.path, tab.view.document_format()).ok()
        });
        let mut jump = None;
        let width = self.settings.effective.tool_panel_width as f32;
        let generation = self.settings.width_generation;
        let panel = egui::SidePanel::right(egui::Id::new(("tools", generation)))
            .resizable(true)
            .width_range(100.0..=1000.0)
            .default_width(width)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::CollapsingHeader::new("Properties")
                        .default_open(true)
                        .show(ui, |ui| match &properties {
                            Some(p) => {
                                egui::Grid::new("properties").num_columns(2).show(ui, |ui| {
                                    for (name, value) in [
                                        ("Place", p.place.clone()),
                                        ("Format", p.format.name().to_owned()),
                                        ("Size", p.size_text()),
                                        ("Last change", p.changed_text()),
                                    ] {
                                        ui.weak(name);
                                        ui.label(value);
                                        ui.end_row();
                                    }
                                });
                            }
                            None => {
                                ui.weak("Not saved yet");
                            }
                        });
                    let Some((_, lists)) = &tab.indexes else {
                        return;
                    };
                    for (title, entries) in [
                        ("Outline", &lists.outline),
                        ("Code", &lists.code),
                        ("Figures", &lists.figures),
                    ] {
                        egui::CollapsingHeader::new(format!("{title} ({})", entries.len()))
                            .id_salt(title)
                            .default_open(true)
                            .show(ui, |ui| {
                                if entries.is_empty() {
                                    ui.weak("None");
                                }
                                for entry in entries {
                                    ui.horizontal(|ui| {
                                        ui.add_space(entry.depth as f32 * 14.0);
                                        let link = ui.selectable_label(false, &entry.label);
                                        if link.clicked() {
                                            jump = Some(entry.offset);
                                        }
                                    });
                                }
                            });
                    }
                });
            });
        let resizing = ctx.is_being_dragged(egui::Id::new(("tools", generation)).with("__resize"));
        let released = std::mem::replace(&mut self.resizing[1], resizing) && !resizing;
        let shown = panel.response.rect.width().round();
        if released && (shown - width).abs() > 1.0 {
            tool_width = Some((shown as u32).clamp(100, 1000));
        }
        if let Some(offset) = jump {
            match tab.editing_view {
                EditingView::Facade => tab.facade.reveal(offset),
                EditingView::Source => {
                    tab.view.jump_to_byte(offset);
                    tab.view.focus_cursor();
                }
            }
        }
        if let Some(width) = tool_width {
            self.settings
                .quick_change(SettingKey::ToolPanelWidth, width.to_string());
        }
    }

    /// UC-010: the settings in effect, applied to the window and every tab:
    /// the theme at once (BR-004), the user settings file watched (A5),
    /// the settings window, Ctrl+, (BR-006), and their messages.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        self.settings.poll();
        let theme = self.settings.effective.theme;
        if self.applied_theme != Some(theme) {
            self.applied_theme = Some(theme);
            ctx.set_visuals(visuals(self.settings.palette()));
            self.highlight = Arc::new(self.highlight.with_theme(theme));
            for (_, tab) in self.tabs.iter_mut() {
                tab.view.set_highlight_set(Arc::clone(&self.highlight));
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.settings.open = !self.settings.open;
        }
        let derived = self.workspace.as_ref().map(|w| w.default_format());
        self.settings.show_window(ctx, derived);
        for message in self.settings.take_messages() {
            self.status = message;
        }
    }

    /// UC-004 BR-007: opens an address or file with the system's program.
    fn open_with_system(&mut self, target: &str) {
        if let Err(err) = open::that_detached(target) {
            self.status = format!("{target} could not be opened: {err}");
        }
    }

    /// UC-004 A4: asks once per session before opening web addresses.
    fn show_web_confirm(&mut self, ctx: &egui::Context) {
        let Some(address) = self.web_confirm.clone() else {
            return;
        };
        let mut answer = None;
        egui::Window::new("Open web address")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Open {address} in your browser?"));
                ui.label("Underline asks this once until it is closed.");
                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        answer = Some(false);
                    }
                });
            });
        match answer {
            Some(true) => {
                self.web_confirm = None;
                self.web_allowed = true;
                self.open_with_system(&address);
            }
            Some(false) => self.web_confirm = None,
            None => {}
        }
    }

    /// UC-005 A2: asks for the address of a link or the file of an image.
    fn show_tool_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.tool_dialog else {
            return;
        };
        let notes: Vec<OverviewEntry> = self.overview.clone();
        let mut done = None;
        let mut cancel = false;
        match dialog {
            ToolDialog::Link { address, filter } => {
                egui::Window::new("Insert link")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("Web address:");
                        let edit = ui.text_edit_singleline(address);
                        if address.is_empty() && filter.is_empty() {
                            edit.request_focus();
                        }
                        ui.label("Or a note of the workspace:");
                        ui.text_edit_singleline(filter);
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                let needle = filter.to_lowercase();
                                for entry in notes
                                    .iter()
                                    .filter(|e| e.label.to_lowercase().contains(&needle))
                                    .take(50)
                                {
                                    if ui.selectable_label(false, &entry.label).clicked() {
                                        done = Some(ToolAnswer::Note(entry.id.clone()));
                                    }
                                }
                            });
                        ui.horizontal(|ui| {
                            let enter =
                                edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (ui.button("Insert").clicked() || enter)
                                && !address.trim().is_empty()
                            {
                                done = Some(ToolAnswer::Address(address.trim().to_owned()));
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
            }
            ToolDialog::Image { path, large } => {
                egui::Window::new("Insert image")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| match large {
                        // A19: a large file is copied or only linked.
                        Some((file, bytes)) => {
                            ui.label(format!(
                                "{} has {} MB. Copy it into the workspace, or only link to where it is (it is then not part of the workspace and not pushed with git)?",
                                file.display(),
                                *bytes / (1024 * 1024)
                            ));
                            ui.horizontal(|ui| {
                                if ui.button("Copy").clicked() {
                                    done = Some(ToolAnswer::File(file.clone(), true));
                                }
                                if ui.button("Link to its place").clicked() {
                                    done = Some(ToolAnswer::Address(file.display().to_string()));
                                }
                                if ui.button("Cancel").clicked() {
                                    cancel = true;
                                }
                            });
                        }
                        None => {
                            ui.label("Image file:");
                            let edit = ui.text_edit_singleline(path);
                            if path.is_empty() {
                                edit.request_focus();
                            }
                            ui.horizontal(|ui| {
                                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if (ui.button("Insert").clicked() || enter) && !path.trim().is_empty() {
                                    done = Some(ToolAnswer::File(PathBuf::from(path.trim()), false));
                                }
                                if ui.button("Cancel").clicked() {
                                    cancel = true;
                                }
                            });
                        }
                    });
            }
        }
        if cancel {
            self.tool_dialog = None;
        }
        if let Some(answer) = done {
            self.answer_tool(answer);
        }
    }

    /// What the Vim and Nano keys ask: store the active note at once, or
    /// close its tab (UC-005 BR-009, BR-010).
    fn key_command(&mut self, command: EditorCommand) {
        let Some(index) = self.tabs.active_index() else {
            return;
        };
        match command {
            EditorCommand::Store => {
                let mut tabs = std::mem::take(&mut self.tabs);
                if let Some((_, tab)) = tabs.iter_mut().nth(index) {
                    tab.quiet_save = false;
                    tab.autosave.changed(Instant::now());
                    self.status = match self.flush_tab_now(tab) {
                        Ok(()) => format!("Stored {}", tab.title),
                        Err(err) => format!("{} could not be stored: {err}", tab.title),
                    };
                }
                self.tabs = tabs;
            }
            EditorCommand::Close => {
                let id = self.tabs.iter().nth(index).map(|(id, _)| id.clone());
                if let Some(id) = id {
                    self.close_tab(&id, false);
                }
            }
        }
    }

    /// UC-005 A18, BR-014: stores a pasted image as a PNG next to the note,
    /// named after the note and the time, and inserts it at the cursor.
    fn paste_image(&mut self, image: ClipImage) {
        let Some((_, tab)) = self.tabs.active_mut() else {
            return;
        };
        let stored = encode_png(image.width, image.height, &image.rgba)
            .map_err(|e| e.to_string())
            .and_then(|png| {
                store_pasted_image(&tab.path, &png, SystemTime::now()).map_err(|e| e.to_string())
            });
        match stored {
            Ok(name) => {
                if tab.view.apply_format(&Format::Image(name.clone())) {
                    tab.quiet_save = false;
                    tab.autosave.changed(Instant::now());
                }
                self.status = format!("Pasted image stored as {name}");
            }
            Err(err) => self.status = format!("The pasted image could not be stored: {err}"),
        }
    }

    /// UC-005 A9 to A13, BR-012: inserts what was dropped at the cursor,
    /// which the editor put at the drop position: a file from outside is
    /// copied next to the note, a file of the workspace is referred to, a
    /// link becomes a link; in a code block a file is inserted by its name
    /// only. A folder or a file that cannot be read is refused in the
    /// status bar.
    fn drop_items(&mut self, items: Vec<egui::DroppedFile>) {
        let Some(root) = self.workspace.as_ref().map(|w| w.root().to_owned()) else {
            return;
        };
        for (n, item) in items.into_iter().enumerate() {
            let path = item
                .path
                .clone()
                .or_else(|| (!item.name.is_empty()).then(|| PathBuf::from(&item.name)));
            let Some(path) = path else {
                self.status = "This cannot be dropped into a note".into();
                continue;
            };
            if n > 0 {
                if let Some((_, tab)) = self.tabs.active_mut() {
                    tab.view.insert_text(" ");
                }
            }
            if let Some((address, label)) = dropped_link(&path) {
                self.answer_tool(ToolAnswer::Link(address, label));
                continue;
            }
            let Some((_, tab)) = self.tabs.active_mut() else {
                return;
            };
            if tab.facade.cursor_in_code(&tab.view) {
                match embed_file(&root, &tab.path, &path, false) {
                    Ok(embedded) => {
                        tab.view.insert_text(&embedded.place);
                        tab.quiet_save = false;
                        tab.autosave.changed(Instant::now());
                        if embedded.renamed {
                            self.status =
                                format!("Copied as {}, since the name was taken", embedded.place);
                        }
                    }
                    Err(EmbedError::Large { .. }) => {
                        self.answer_tool(ToolAnswer::File(path, false));
                    }
                    Err(err) => self.status = err.to_string(),
                }
                continue;
            }
            self.answer_tool(ToolAnswer::File(path, false));
        }
    }

    /// Applies the answer of the link or image dialog to the active tab.
    fn answer_tool(&mut self, answer: ToolAnswer) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let root = workspace.root().to_owned();
        let format = match answer {
            ToolAnswer::Address(address) => Format::Link {
                address,
                label: None,
            },
            ToolAnswer::Link(address, label) => Format::Link { address, label },
            ToolAnswer::Note(id) => {
                let (Ok(target), Some((_, tab))) = (workspace.note_path(&id), self.tabs.active())
                else {
                    return;
                };
                let dir = tab.path.parent().unwrap_or(&root).to_owned();
                let dir = std::path::absolute(&dir).unwrap_or(dir);
                let target = std::path::absolute(&target).unwrap_or(target);
                Format::Link {
                    address: relative(&dir, &target),
                    label: Some(id.title()),
                }
            }
            ToolAnswer::File(file, allow_large) => {
                let Some((_, tab)) = self.tabs.active() else {
                    return;
                };
                match embed_file(&root, &tab.path, &file, allow_large) {
                    Ok(embedded) => {
                        if embedded.renamed {
                            self.status =
                                format!("Copied as {}, since the name was taken", embedded.place);
                        }
                        if embedded.image {
                            Format::Image(embedded.place)
                        } else {
                            let label = embedded.place.rsplit('/').next().map(str::to_owned);
                            Format::Link {
                                address: embedded.place,
                                label,
                            }
                        }
                    }
                    Err(EmbedError::Large { path, bytes }) => {
                        self.tool_dialog = Some(ToolDialog::Image {
                            path: String::new(),
                            large: Some((path, bytes)),
                        });
                        return;
                    }
                    Err(err) => {
                        self.status = err.to_string();
                        self.tool_dialog = None;
                        return;
                    }
                }
            }
        };
        self.tool_dialog = None;
        if let Some((_, tab)) = self.tabs.active_mut() {
            if tab.view.apply_format(&format) {
                tab.quiet_save = false;
                tab.autosave.changed(Instant::now());
            }
            match tab.editing_view {
                EditingView::Facade => tab.facade.request_focus(),
                EditingView::Source => tab.view.focus_cursor(),
            }
        }
        if let Some(workspace) = &mut self.workspace {
            workspace.rescan();
        }
        self.refresh_overview();
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
                    tab.facade.request_focus();
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

    /// UC-006 step 1: asks for the new title and folder of `id`.
    fn start_rename(&mut self, id: NoteId) {
        if self.rename.is_some() || self.workspace.is_none() {
            return;
        }
        self.rename = Some(RenameDialog {
            title: id.title(),
            folder: id.folder().unwrap_or_default().to_owned(),
            from: id,
            problem: None,
            select: true,
        });
    }

    /// UC-006 steps 2–3.
    fn show_rename_dialog(&mut self, ctx: &egui::Context) {
        let (Some(dialog), Some(workspace)) = (&mut self.rename, &self.workspace) else {
            return;
        };
        let mut action = RenameAction::None;
        egui::Window::new("Rename note")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Title:");
                let mut title = egui::TextEdit::singleline(&mut dialog.title).show(ui);
                if std::mem::take(&mut dialog.select) {
                    // Step 2: the current title is selected.
                    title.response.request_focus();
                    let all = egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(dialog.title.chars().count()),
                    );
                    title.state.cursor.set_char_range(Some(all));
                    title.state.store(ui.ctx(), title.response.id);
                }
                ui.label("Folder (empty for the top folder, new folders are created):");
                let folder = ui.text_edit_singleline(&mut dialog.folder);
                // Step 2: the file name the note will get, while the user types.
                let rename = Rename {
                    folder: dialog.folder.clone(),
                    title: dialog.title.clone(),
                };
                if let Ok(Some(planned)) = workspace.plan_rename(&dialog.from, &rename) {
                    ui.weak(format!("File: {}", planned.to_file()));
                }
                // BR-009: the start page of a GitHub wiki.
                if dialog.from.as_str() == HOME_TITLE {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "This is the Home note, the start page of a GitHub wiki. Renamed or moved, the wiki has no start page.",
                    );
                }
                if let Some(problem) = &dialog.problem {
                    ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
                }
                ui.horizontal(|ui| {
                    let enter = (title.response.lost_focus() || folder.lost_focus())
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Rename").clicked() || enter {
                        action = RenameAction::Rename;
                    }
                    if ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        action = RenameAction::Cancel;
                    }
                });
            });
        match action {
            RenameAction::None => {}
            // A8: nothing is changed.
            RenameAction::Cancel => self.rename = None,
            RenameAction::Rename => self.rename_note(),
        }
    }

    /// UC-006 steps 4–7.
    fn rename_note(&mut self) {
        let Some(dialog) = &self.rename else {
            return;
        };
        let from = dialog.from.clone();
        let rename = Rename {
            folder: dialog.folder.clone(),
            title: dialog.title.clone(),
        };
        // Step 5: unsaved changes of the note are saved first.
        if let Some(index) = self.tabs.find(&from) {
            let mut tabs = std::mem::take(&mut self.tabs);
            let result = self.flush_tab_now(tabs.get_mut(index));
            self.tabs = tabs;
            if let Err(err) = result {
                self.rename = None;
                self.status = format!("The note could not be renamed, because {err}");
                return;
            }
        }
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        let planned = match workspace.plan_rename(&from, &rename) {
            Ok(Some(planned)) => planned,
            // A4: nothing changed.
            Ok(None) => {
                self.rename = None;
                return;
            }
            Err(problem) => return self.refuse_rename(problem),
        };
        let mut open = TabEdits {
            tabs: &mut self.tabs,
            renamed: &from,
        };
        match rename_note(workspace, &planned, &mut open) {
            Ok(renamed) => self.renamed(&planned, renamed),
            Err(problem) => self.refuse_rename(problem),
        }
    }

    /// UC-006 A1–A3 keep the dialog with what the user entered; A5 reports
    /// in the status bar.
    fn refuse_rename(&mut self, problem: NewNoteProblem) {
        let Some(dialog) = &mut self.rename else {
            return;
        };
        match problem {
            NewNoteProblem::Exists(id) => {
                dialog.problem = Some(format!(
                    "A note named {} exists in that folder. Change the title or the folder.",
                    id.title()
                ));
            }
            problem @ NewNoteProblem::Io { .. } => {
                self.rename = None;
                self.status = format!("The note could not be renamed: {problem}");
            }
            problem => {
                let mut text = problem.to_string();
                if let Some(first) = text.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                dialog.problem = Some(format!("{text}."));
            }
        }
    }

    /// UC-006 step 7: the tab, the remembered state, the note overview, and
    /// the report (BR-007, A6).
    fn renamed(&mut self, planned: &NoteMove, renamed: Renamed) {
        self.rename = None;
        let Some(workspace) = &self.workspace else {
            return;
        };
        let root = workspace.root().to_owned();
        if let Some(index) = self.tabs.find(&planned.from) {
            let tab = self.tabs.get_mut(index);
            tab.view.apply_edits(&renamed.own_edits);
            tab.on_disk = tab.view.document().text();
            tab.path = root.join(planned.to_file());
            tab.facade.set_note_file(tab.path.clone());
            tab.title = planned.to.title();
            tab.deleted = false;
            self.tabs.rename(&planned.from, planned.to.clone());
        }
        self.state.note_renamed(&root, &planned.from, &planned.to);
        self.persist_tabs();
        self.refresh_overview();
        let mut status = format!(
            "Renamed {} to {}; updated {} in {}",
            planned.from.title(),
            planned.to.title(),
            count(renamed.links, "link"),
            count(renamed.notes, "note"),
        );
        if !renamed.not_updated.is_empty() {
            let names: Vec<String> = renamed.not_updated.iter().map(NoteId::title).collect();
            status.push_str(&format!(
                "; these notes could not be updated and still link to the old title: {}",
                names.join(", ")
            ));
        }
        self.status = status;
    }

    /// UC-007 steps 1–2: asks before deleting `id`.
    fn start_delete(&mut self, id: NoteId) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        if self.delete.is_some() {
            return;
        }
        let (links, notes) = links_to_note(workspace, &id);
        self.delete = Some(DeleteDialog {
            id,
            links,
            notes,
            cannot_keep: None,
        });
    }

    fn show_delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.delete else {
            return;
        };
        let (mut confirm, mut cancel) = (false, false);
        egui::Window::new("Delete note")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Delete the note {}?", dialog.id.title()));
                if dialog.links == 0 {
                    ui.label("No links in other notes lead to it.");
                } else {
                    ui.label(format!(
                        "{} in {} lead to it and will lead nowhere.",
                        count(dialog.links, "link"),
                        count(dialog.notes, "note")
                    ));
                }
                // BR-010: the start page of a GitHub wiki.
                if dialog.id.as_str() == HOME_TITLE {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "This is the Home note, the start page of a GitHub wiki. Deleted, the wiki has no start page.",
                    );
                }
                match &dialog.cannot_keep {
                    // A8: the second question.
                    Some(reason) => ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!("The note cannot be kept among the deleted notes ({reason}), so it cannot be restored. Delete it for good?"),
                    ),
                    None => ui.weak("It is kept among the deleted notes for 30 days."),
                };
                ui.horizontal(|ui| {
                    let label = if dialog.cannot_keep.is_some() {
                        "Delete for good"
                    } else {
                        "Delete"
                    };
                    confirm = ui.button(label).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    cancel = ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape));
                });
            });
        if cancel {
            // A1: nothing is changed.
            self.delete = None;
        } else if confirm {
            self.delete_note();
        }
    }

    /// UC-007 steps 4–5, A3, A8.
    fn delete_note(&mut self) {
        let Some(dialog) = &self.delete else {
            return;
        };
        let id = dialog.id.clone();
        let for_good = dialog.cannot_keep.is_some();
        // Step 4: unsaved changes are saved first, so they are kept too.
        if let Some(index) = self.tabs.find(&id) {
            let mut tabs = std::mem::take(&mut self.tabs);
            let result = self.flush_tab_now(tabs.get_mut(index));
            self.tabs = tabs;
            if let Err(err) = result {
                self.delete = None;
                self.status = format!("The note could not be deleted, because {err}");
                return;
            }
        }
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        let result = if for_good {
            workspace.delete_note_for_good(&id).map(|()| None)
        } else {
            workspace.delete_note(&id, now()).map(Some)
        };
        match result {
            Ok(deleted) => {
                self.delete = None;
                self.deleted_note(&id, deleted);
            }
            Err(DeleteProblem::CannotKeep(err)) => {
                if let Some(dialog) = &mut self.delete {
                    dialog.cannot_keep = Some(err.to_string());
                }
            }
            Err(problem) => {
                // A3: the note and its tab stay.
                self.delete = None;
                self.status = format!("The note could not be deleted: {problem}");
            }
        }
    }

    /// UC-007 step 5: the tab closes, the note is forgotten, and the report
    /// offers to undo when the note is kept.
    fn deleted_note(&mut self, id: &NoteId, deleted: Option<DeletedNote>) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let root = workspace.root().to_owned();
        let reopen = match self.tabs.find(id) {
            Some(index) => {
                self.tabs.close(index);
                true
            }
            None => false,
        };
        self.state.note_deleted(&root, id);
        self.persist_tabs();
        self.refresh_overview();
        self.refresh_deleted();
        self.status = format!("Deleted {}", id.title());
        self.undo_delete = deleted.map(|deleted| UndoDelete {
            deleted,
            reopen,
            status: self.status.clone(),
        });
    }

    fn refresh_deleted(&mut self) {
        self.deleted = self
            .workspace
            .as_ref()
            .map(Workspace::deleted_notes)
            .unwrap_or_default();
    }

    /// UC-007 A4, A5: puts a deleted note back at its place; asks for
    /// another title or folder when it is taken (A6).
    fn restore_deleted(&mut self, deleted: DeletedNote, reopen: bool) {
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        match workspace.restore_note(&deleted, None) {
            Ok(id) => self.restored(&id, reopen),
            Err(NewNoteProblem::Exists(existing)) => {
                self.restore = Some(RestoreDialog {
                    title: deleted.id.title(),
                    folder: deleted.id.folder().unwrap_or_default().to_owned(),
                    problem: Some(format!(
                        "A note named {} exists in that folder. Choose another title or folder.",
                        existing.title()
                    )),
                    deleted,
                    reopen,
                });
            }
            Err(problem) => {
                self.status = format!("The note could not be restored: {problem}");
            }
        }
    }

    fn restored(&mut self, id: &NoteId, reopen: bool) {
        self.restore = None;
        self.undo_delete = None;
        if reopen {
            self.open_note(id);
        }
        self.refresh_overview();
        self.refresh_deleted();
        self.status = format!("Restored {}", id.title());
    }

    /// UC-007 A6.
    fn show_restore_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.restore else {
            return;
        };
        let (mut confirm, mut cancel) = (false, false);
        egui::Window::new("Restore note")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Title:");
                let title = ui.text_edit_singleline(&mut dialog.title);
                ui.label("Folder (empty for the top folder, new folders are created):");
                let folder = ui.text_edit_singleline(&mut dialog.folder);
                if let Some(problem) = &dialog.problem {
                    ui.colored_label(ui.visuals().warn_fg_color, problem.as_str());
                }
                ui.horizontal(|ui| {
                    let enter = (title.lost_focus() || folder.lost_focus())
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    confirm = ui.button("Restore").clicked() || enter;
                    cancel = ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape));
                });
            });
        if cancel {
            // A6 step 2: the note stays deleted.
            self.restore = None;
            return;
        }
        if !confirm {
            return;
        }
        let Some(workspace) = &mut self.workspace else {
            return;
        };
        let instead = Rename {
            folder: dialog.folder.clone(),
            title: dialog.title.clone(),
        };
        match workspace.restore_note(&dialog.deleted, Some(&instead)) {
            Ok(id) => {
                let reopen = dialog.reopen;
                self.restored(&id, reopen);
            }
            Err(NewNoteProblem::Exists(existing)) => {
                dialog.problem = Some(format!(
                    "A note named {} exists in that folder. Choose another title or folder.",
                    existing.title()
                ));
            }
            Err(problem) => {
                let mut text = problem.to_string();
                if let Some(first) = text.get_mut(0..1) {
                    first.make_ascii_uppercase();
                }
                dialog.problem = Some(format!("{text}."));
            }
        }
    }

    /// UC-007 A7: asks before removing deleted notes for good.
    fn show_purge_dialog(&mut self, ctx: &egui::Context) {
        let Some(purge) = &self.purge else {
            return;
        };
        let n = match purge {
            Purge::One(_) => 1,
            Purge::All => self.deleted.len(),
        };
        let (mut confirm, mut cancel) = (false, false);
        egui::Window::new("Remove deleted notes")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Remove {} for good? They cannot be restored afterwards.",
                    count(n, "deleted note")
                ));
                ui.horizontal(|ui| {
                    confirm = ui.button("Remove").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    cancel = ui.button("Cancel").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape));
                });
            });
        if cancel {
            self.purge = None;
        }
        if !confirm {
            return;
        }
        let Some(workspace) = &self.workspace else {
            return;
        };
        let targets: Vec<DeletedNote> = match self.purge.take() {
            Some(Purge::One(deleted)) => vec![deleted],
            _ => self.deleted.clone(),
        };
        let failed = targets
            .iter()
            .filter(|d| workspace.forget_deleted(d).is_err())
            .count();
        if self
            .undo_delete
            .as_ref()
            .is_some_and(|u| targets.contains(&u.deleted))
        {
            self.undo_delete = None;
        }
        self.refresh_deleted();
        self.status = if failed == 0 {
            format!("Removed {} for good", count(targets.len(), "deleted note"))
        } else {
            format!("{} could not be removed", count(failed, "deleted note"))
        };
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
        self.apply_settings(ctx);
        self.show_close_confirm(ctx);
        self.show_open_dialog(ctx);
        self.show_create_dialog(ctx);
        self.show_note_dialog(ctx);
        self.show_rename_dialog(ctx);
        self.show_delete_dialog(ctx);
        self.show_restore_dialog(ctx);
        self.show_purge_dialog(ctx);
        self.show_tool_dialog(ctx);
        // UC-008 BR-007: Ctrl+Shift+F into the search field; A6: Ctrl+F finds
        // in the active note where the input mode gives it no meaning.
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            )
        }) {
            self.focus_search = true;
        } else if self.settings.effective.input_mode == Keys::Normal
            && self.tabs.active().is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F))
        {
            let selected = self.tabs.active().map(|(_, tab)| {
                let range = tab.view.selection_range();
                tab.view.document().rope().slice(range).to_string()
            });
            let text = selected
                .filter(|s| !s.is_empty() && !s.contains('\n'))
                .or_else(|| self.find.as_ref().map(|f| f.text.clone()))
                .unwrap_or_default();
            self.find = Some(FindBar {
                text,
                focus: true,
                sought: String::new(),
            });
        }
        self.run_search();
        // UC-006 step 1: F2 renames the note of the active tab.
        if self.rename.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2))
        {
            if let Some((id, _)) = self.tabs.active() {
                self.start_rename(id.clone());
            }
        }
        self.show_web_confirm(ctx);
        self.write_state(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open workspace…").clicked() {
                    self.start_open();
                }
                if ui.button("New workspace…").clicked() {
                    self.start_create();
                }
                // UC-010 BR-006: the settings button.
                if ui
                    .button("Settings")
                    .on_hover_text(if cfg!(target_os = "macos") {
                        "Cmd+,"
                    } else {
                        "Ctrl+,"
                    })
                    .clicked()
                {
                    self.settings.open = !self.settings.open;
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
            let mut undo = false;
            ui.horizontal(|ui| {
                ui.label(parts.join("  |  "));
                // UC-007 A4: offered while the report of the deletion shows.
                match &self.undo_delete {
                    Some(offer) if offer.status == self.status => {
                        undo = ui.small_button("Undo").clicked();
                    }
                    _ => {}
                }
            });
            if self
                .undo_delete
                .as_ref()
                .is_some_and(|offer| offer.status != self.status)
            {
                self.undo_delete = None;
            }
            if undo {
                if let Some(offer) = self.undo_delete.take() {
                    self.restore_deleted(offer.deleted, offer.reopen);
                }
            }
            // FR-026: the input mode and its most important commands.
            if let Some(status) = self
                .tabs
                .active()
                .and_then(|(_, tab)| tab.view.input_status())
            {
                ui.label(egui::RichText::new(status).monospace());
            }
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

        let overview_width = self.settings.effective.overview_width as f32;
        let generation = self.settings.width_generation;
        let pages = egui::SidePanel::left(egui::Id::new(("pages", generation)))
            .resizable(true)
            .width_range(100.0..=1000.0)
            .default_width(overview_width)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Notes");
                    if ui.button("New note…").clicked() {
                        self.start_new_note();
                    }
                });
                // UC-008 step 1: the search field above the note overview.
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.search_text)
                        .hint_text(if cfg!(target_os = "macos") {
                            "Search notes  Cmd+Shift+F"
                        } else {
                            "Search notes  Ctrl+Shift+F"
                        })
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut self.focus_search) {
                    field.request_focus();
                }
                let searching = !self.search_text.trim().is_empty();
                let mut found_choice = None;
                // BR-007: arrows, Enter, and Escape in the search field.
                let entries: Vec<(usize, Option<usize>)> = self
                    .search_result
                    .iter()
                    .flat_map(|r| r.found.iter().enumerate())
                    .flat_map(|(n, note)| {
                        std::iter::once((n, None))
                            .chain((0..note.lines.len()).map(move |l| (n, Some(l))))
                    })
                    .collect();
                if field.has_focus() || field.lost_focus() {
                    let (down, up, enter, escape) = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::ArrowDown),
                            i.key_pressed(egui::Key::ArrowUp),
                            i.key_pressed(egui::Key::Enter),
                            i.key_pressed(egui::Key::Escape),
                        )
                    });
                    if escape {
                        // A2: the search is cleared.
                        self.search_text.clear();
                        self.search_cursor = None;
                    } else if !entries.is_empty() {
                        let last = entries.len() - 1;
                        if down {
                            self.search_cursor =
                                Some(self.search_cursor.map_or(0, |c| (c + 1).min(last)));
                            field.request_focus();
                        }
                        if up {
                            self.search_cursor =
                                Some(self.search_cursor.map_or(0, |c| c.saturating_sub(1)));
                            field.request_focus();
                        }
                        if enter {
                            let (n, line) = entries[self.search_cursor.unwrap_or(0).min(last)];
                            found_choice = self
                                .search_result
                                .as_ref()
                                .map(|r| choice(&r.found[n], line));
                        }
                    }
                }
                ui.separator();
                let mut selected = None;
                let mut renaming = None;
                let mut deleting = None;
                let mut restoring = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if searching {
                        if let Some(clicked) = found_list(
                            ui,
                            self.search_result.as_ref(),
                            &self.search_text,
                            self.search_cursor.and_then(|c| entries.get(c).copied()),
                        ) {
                            found_choice = Some(clicked);
                        }
                    }
                    for entry in self.overview.iter().filter(|_| !searching) {
                        let active = self.tabs.active().is_some_and(|(n, _)| n == &entry.id);
                        let label = ui.selectable_label(active, &entry.label);
                        if label.clicked() {
                            selected = Some(entry.id.clone());
                            // The keyboard leaves the editor for the overview.
                            label.request_focus();
                        }
                        // UC-006 step 1: the context menu of the note.
                        label.context_menu(|ui| {
                            if ui.button("Rename…").clicked() {
                                renaming = Some(entry.id.clone());
                                ui.close_menu();
                            }
                            if ui.button("Delete…").clicked() {
                                deleting = Some(entry.id.clone());
                                ui.close_menu();
                            }
                        });
                        // UC-007 BR-007: the Delete key in the note overview.
                        if label.has_focus() {
                            self.overview_chosen = Some(entry.id.clone());
                        }
                    }
                    // UC-007 A5: the deleted notes, at the end of the overview.
                    if !self.deleted.is_empty() && !searching {
                        ui.separator();
                        egui::CollapsingHeader::new(format!(
                            "Deleted notes ({})",
                            self.deleted.len()
                        ))
                        .id_salt("deleted notes")
                        .show(ui, |ui| {
                            for deleted in &self.deleted {
                                let time = std::time::UNIX_EPOCH
                                    + Duration::from_millis(deleted.deleted_at);
                                ui.label(deleted.id.title()).on_hover_text(format!(
                                    "{} · deleted {}",
                                    deleted.id.folder().unwrap_or("top folder"),
                                    format_local_time(time)
                                ));
                                ui.horizontal(|ui| {
                                    ui.weak(format_local_time(time));
                                    if ui.small_button("Restore").clicked() {
                                        restoring = Some(deleted.clone());
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        self.purge = Some(Purge::One(deleted.clone()));
                                    }
                                });
                            }
                            if ui.button("Empty the deleted notes").clicked() {
                                self.purge = Some(Purge::All);
                            }
                        });
                    }
                });
                if let Some(id) = selected {
                    self.open_note(&id);
                    self.refresh_overview();
                    self.overview_chosen = Some(id);
                }
                if let Some(id) = renaming {
                    self.start_rename(id);
                }
                if let Some(id) = deleting {
                    self.start_delete(id);
                }
                if let Some(deleted) = restoring {
                    self.restore_deleted(deleted, false);
                }
                if let Some(found) = found_choice {
                    self.open_found(found);
                }
            });
        // UC-007 BR-007: the Delete key deletes the note chosen in the note
        // overview while the overview has the keyboard: chosen by a click or
        // by moving the focus there, until the user clicks elsewhere or
        // another widget takes the keyboard.
        let pointer_elsewhere = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pos| !pages.response.rect.contains(pos))
        });
        let other_focus = ctx
            .memory(|m| m.focused())
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|focused| !pages.response.rect.contains(focused.rect.center()));
        if pointer_elsewhere || other_focus {
            self.overview_chosen = None;
        }
        if self.delete.is_none()
            && self.rename.is_none()
            && self.overview_chosen.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
        {
            if let Some(id) = self.overview_chosen.clone() {
                self.start_delete(id);
            }
        }
        // UC-010 A4: a width the user dragged is kept once they let go.
        let resizing = ctx.is_being_dragged(egui::Id::new(("pages", generation)).with("__resize"));
        let released = std::mem::replace(&mut self.resizing[0], resizing) && !resizing;
        let shown = pages.response.rect.width().round();
        if released && (shown - overview_width).abs() > 1.0 {
            self.settings.quick_change(
                SettingKey::OverviewWidth,
                (shown as u32).clamp(100, 1000).to_string(),
            );
        }

        self.show_tool_panel(ctx);

        let mut persist = false;
        let mut ask_tool = None;
        let mut followed = None;
        let mut dropped = Vec::new();
        let mut pasted_image = None;
        let mut quick: Vec<(SettingKey, String)> = Vec::new();
        let mut key_commands = Vec::new();
        egui::CentralPanel::default().show(ctx, |ui| {
            // UC-004 A5: the tab bar, left to right.
            let mut activate = None;
            let mut close = None;
            let mut renaming = None;
            let mut deleting = None;
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
                            // UC-006 step 1: the context menu of the tab.
                            label.context_menu(|ui| {
                                if ui.button("Rename…  F2").clicked() {
                                    renaming = Some(id.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Delete…").clicked() {
                                    deleting = Some(id.clone());
                                    ui.close_menu();
                                }
                            });
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
            if let Some(id) = renaming {
                self.start_rename(id);
            }
            if let Some(id) = deleting {
                self.start_delete(id);
            }
            ui.separator();
            // UC-008 A6: the find field of the active tab.
            let mut find_action = None;
            let found_count =
                self.find
                    .as_ref()
                    .and_then(|_| self.tabs.active_mut())
                    .map(|(_, tab)| {
                        let selection = tab.view.selection_range();
                        let found = tab.view.found_ranges();
                        let at = found.iter().position(|r| *r == selection);
                        (at, found.len())
                    });
            if let Some(find) = &mut self.find {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Find in note:");
                    let edit =
                        ui.add(egui::TextEdit::singleline(&mut find.text).desired_width(200.0));
                    if std::mem::take(&mut find.focus) {
                        edit.request_focus();
                    }
                    match found_count {
                        Some((Some(at), n)) => ui.weak(format!("{} of {n}", at + 1)),
                        Some((None, n)) => ui.weak(format!("{n} found")),
                        None => ui.weak(""),
                    };
                    let keys = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::Enter),
                            i.key_pressed(egui::Key::F3),
                            i.modifiers.shift,
                            i.key_pressed(egui::Key::Escape),
                        )
                    });
                    let in_field = edit.has_focus() || edit.lost_focus();
                    let previous = ui
                        .small_button("Previous")
                        .on_hover_text("Shift+Enter, Shift+F3")
                        .clicked();
                    let next = ui.small_button("Next").on_hover_text("Enter, F3").clicked();
                    let close = ui.small_button("×").on_hover_text("Escape").clicked();
                    find_action = Some(if close || (in_field && keys.3) {
                        FindAction::Close
                    } else if previous || ((in_field && keys.0) || keys.1) && keys.2 {
                        find.focus = in_field;
                        FindAction::Previous
                    } else if next || (in_field && keys.0) || keys.1 {
                        find.focus = in_field;
                        FindAction::Next
                    } else {
                        FindAction::None
                    });
                });
            }
            if let Some(action) = find_action {
                self.find_in_note(action);
            }
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
            // FR-010: switch the editing view at any time; the cursor stays
            // at its place, since both views share it (UC-005 A1).
            let mut switched = false;
            let mut tool_used = None;
            ui.horizontal(|ui| {
                for (view, label) in [
                    (EditingView::Facade, "Facade"),
                    (EditingView::Source, "Source"),
                ] {
                    if ui
                        .selectable_label(tab.editing_view == view, label)
                        .clicked()
                        && tab.editing_view != view
                    {
                        tab.editing_view = view;
                        switched = true;
                    }
                }
                ui.separator();
                // FR-023: the formatting toolbar, the same in both views.
                for tool in Tool::ALL {
                    if ui.button(tool.label()).on_hover_text(tool.hint()).clicked() {
                        tool_used = Some(tool);
                    }
                }
                ui.separator();
                // UC-010 A3: quick switches, stored like any setting.
                let effective = self.settings.effective.clone();
                let mut minimap = effective.show_minimap;
                if ui
                    .toggle_value(&mut minimap, "Minimap")
                    .on_hover_text("Show or hide the minimap beside the editor")
                    .changed()
                {
                    quick.push((SettingKey::ShowMinimap, minimap.to_string()));
                }
                let mut mode = effective.input_mode;
                egui::ComboBox::from_id_salt("input-mode")
                    .selected_text(format!("Keys: {}", mode.label()))
                    .show_ui(ui, |ui| {
                        for each in InputMode::ALL {
                            ui.selectable_value(&mut mode, *each, each.label());
                        }
                    });
                if mode != effective.input_mode {
                    quick.push((SettingKey::InputMode, mode.label().to_owned()));
                }
                let mut copy = effective.copy_on_select;
                if ui
                    .toggle_value(&mut copy, "Copy on select")
                    .on_hover_text("Put a selection on the clipboard as soon as it is made")
                    .changed()
                {
                    quick.push((SettingKey::CopyOnSelect, copy.to_string()));
                }
            });
            if let Some(tool) = tool_used {
                if tab.view.apply_tool(tool) {
                    tab.quiet_save = false;
                    tab.autosave.changed(Instant::now());
                }
                switched = true;
            }
            if let Some(tool) = tab.view.take_request() {
                ask_tool = Some(tool);
            }
            if switched {
                match tab.editing_view {
                    EditingView::Facade => tab.facade.request_focus(),
                    EditingView::Source => tab.view.focus_cursor(),
                }
            }
            let effective = &self.settings.effective;
            tab.facade.set_minimap(effective.show_minimap);
            tab.view.set_minimap(effective.show_minimap);
            tab.view.set_copy_on_select(effective.copy_on_select);
            tab.view.set_input_mode(effective.input_mode);
            tab.view
                .set_line_numbers(effective.show_line_numbers, effective.line_number_side);
            let changed = match tab.editing_view {
                EditingView::Facade => tab.facade.show(ui, &mut tab.view),
                EditingView::Source => tab.view.show(ui),
            };
            persist |= switched;
            // UC-005 A9 to A13: what is dropped goes where the pointer is.
            let drops = ui.ctx().input(|i| i.raw.dropped_files.clone());
            if !drops.is_empty() {
                if let Some(pos) = ui.ctx().input(|i| i.pointer.latest_pos()) {
                    match tab.editing_view {
                        EditingView::Facade => {
                            tab.facade.place_cursor_at(ui, &mut tab.view, pos);
                        }
                        EditingView::Source => {
                            tab.view.place_cursor_at(ui, pos);
                        }
                    }
                }
                dropped = drops;
            }
            if let Some(image) = tab.view.take_pasted_image() {
                pasted_image = Some(image);
            }
            key_commands = tab.view.take_commands();
            if let Some(link) = tab.view.take_followed_link() {
                followed = Some((link, tab.path.clone()));
            }
            if changed {
                tab.quiet_save = false;
                tab.autosave.changed(Instant::now());
            }
        });
        if persist {
            self.persist_tabs();
        }
        if let Some((link, note)) = followed {
            self.follow_link(&link, &note);
        }
        if !dropped.is_empty() {
            self.drop_items(dropped);
        }
        if let Some(image) = pasted_image {
            self.paste_image(image);
        }
        for (key, value) in quick {
            self.settings.quick_change(key, value);
        }
        // UC-005 A6, A7: `:w` and Ctrl+O store at once, `:q` and Ctrl+X
        // close the tab.
        for command in key_commands {
            self.key_command(command);
        }
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            self.status = "Drop to insert at the place under the pointer".into();
        }
        if let Some(tool) = ask_tool {
            self.tool_dialog = Some(match tool {
                Tool::Image => ToolDialog::Image {
                    path: String::new(),
                    large: None,
                },
                _ => ToolDialog::Link {
                    address: String::new(),
                    filter: String::new(),
                },
            });
        }
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

/// The look of the window in the colors of a theme (FR-022, UC-010 BR-004).
fn visuals(palette: &Palette) -> egui::Visuals {
    let color = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let mut visuals = if palette.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = color(palette.panel);
    visuals.window_fill = color(palette.panel);
    visuals.extreme_bg_color = color(palette.background);
    visuals.faint_bg_color = color(palette.code_background);
    visuals.code_bg_color = color(palette.code_background);
    visuals.hyperlink_color = color(palette.link);
    visuals.override_text_color = Some(color(palette.foreground));
    visuals.selection.bg_fill = color(palette.chosen());
    visuals.widgets.inactive.weak_bg_fill = color(palette.button());
    visuals.selection.stroke.color = color(palette.chosen_text());
    visuals
}
