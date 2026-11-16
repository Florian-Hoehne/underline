//! UC-004, M-09, ADR-021, ADR-024, ADR-025: the facade of an open note in
//! the session. The window's editor core makes the facade's blocks itself;
//! the session makes them too, with its highlighting, diagrams and images,
//! only for what the core cannot make: the colours of code and the pictures
//! of diagrams and images, which go to the window by their content
//! (`ed-enrich`, `docs/migration/variante-b.md` section 2.3).
//!
//! Parsing and making the views never run while the session is held
//! (ADR-024): the tick hands a snapshot of the text (a rope, copied in no
//! time) to a thread of its own (`build.rs`) and takes the result in with a
//! later tick, but only while it is still for the text the tab has, and
//! only if no later result was taken in already. Only a diagram that became
//! ready is worked in under the session, as it changes one figure.

mod build;
mod recolor;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use underline_core::{EditingView, NoteId, PaneSide};
use underline_editor_core::facade::splices;
use underline_editor_view::views::{
    colors_key, diagram_key, diagram_language, formula_key, refresh_formulas,
};
use underline_editor_view::{FacadeModel, Versioned, CODE_COLORS_KEPT};
use underline_facade::blocks::BlockKind;
use underline_facade::highlight::HighlightSet;

use self::build::{diagram_figure, Built, Context, Job, Looks};
pub(super) use self::build::{CodeColorCache, ImageCache};
use super::diagrams::SvgDiagrams;
use super::formulas::SvgFormulas;
use super::tab::Tab;
use super::Session;
use crate::assets::{percent_decode, request_path, Asset, AssetError, AssetPlan};
use crate::facade_view::{BlockKindView, BlockView, CodeColors, FigureView, FormulaView};
use crate::sync_view::EnrichView;
use crate::workspace_view::EditingViewName;
use crate::{Event, UiError, UiErrorKind};

#[cfg(test)]
use self::build::views_of;

/// ADR-024: one change of the views, in the chain of versions `ed-enrich`
/// follows.
type Splice = underline_editor_core::facade::Splice<BlockView>;

/// How the backend makes the facade of one tab: its jobs, the look and the
/// diagrams the views were made with, the pictures and colours that
/// follow. The blocks and their views as the window has them are the
/// editor's ([`FacadeModel`], `tab.edit.facade`, ADR-025).
#[derive(Debug, Default)]
pub(super) struct Facade {
    /// The look and the diagrams' generation the views were made with.
    looks: Option<Looks>,
    diagrams_seen: u64,
    /// FR-098: the formulas' generation the views were made with.
    formulas_seen: u64,
    /// M-09.7: the last picture of each diagram block, shown while its
    /// changed source renders (`last_pictures` of the egui facade).
    last_pictures: HashMap<usize, String>,
    /// The facade is shown; the tick keeps it current only then. The blocks
    /// stay while the source view is shown, so switching back is quick.
    shown: bool,
    /// The number of the next job, and of the job taken in last.
    next_seq: u64,
    committed: u64,
    /// A job of the tick runs.
    building: bool,
    /// Code blocks sent without their colours, which the tick makes
    /// (ADR-024).
    recolor: Vec<usize>,
    /// ADR-025: what the window was sent of the blocks' colours and
    /// figures (`ed-enrich`).
    sent: Sent,
}

/// ADR-025: what the window was sent of the facade's colours and figures,
/// by their keys, and in which look.
#[derive(Debug, Default)]
struct Sent {
    looks: Option<Looks>,
    /// The version of the views looked at last, so the tick looks again
    /// only after they changed.
    version: Option<u32>,
    colors: HashMap<String, Option<CodeColors>>,
    diagrams: HashMap<String, FigureView>,
    images: HashMap<String, FigureView>,
    formulas: HashMap<String, FormulaView>,
}

/// What making the views needs of the session.
struct Tools {
    looks: Looks,
    colors: Option<Arc<HighlightSet>>,
    dark: bool,
    diagrams: Option<Arc<SvgDiagrams>>,
    /// FR-098: the formulas, drawn in the theme's text colour.
    formulas: Option<Arc<SvgFormulas>>,
    color: underline_facade::themes::Rgb,
}

impl Facade {
    /// ADR-025: whether a tab shows the facade of the note (`facade` its
    /// part of the note's tab); the tick keeps it current only then, and
    /// keeps it while the source view shows.
    pub(super) fn show(facade: &mut Option<Self>, shown: bool) {
        match facade {
            Some(facade) => facade.shown = shown,
            None if shown => {
                *facade = Some(Self {
                    shown: true,
                    ..Self::default()
                });
            }
            None => {}
        }
    }

    /// Whether the views of `model` must be made anew for revision `rev` in
    /// `looks`.
    fn stale(&self, model: &FacadeModel, rev: u64, looks: Looks) -> bool {
        model.parsed != Some(rev) || self.looks != Some(looks)
    }

    /// The job that makes the views of `tab`'s text.
    fn job(&mut self, tab: &Tab, root: &std::path::Path, tools: &Tools, session: &Shared) -> Job {
        self.next_seq += 1;
        let rev = tab.edit.editor.revision();
        let model = &tab.edit.facade;
        Job {
            seq: self.next_seq,
            rev,
            generation: tools.diagrams.as_deref().map_or(0, SvgDiagrams::generation),
            formulas_generation: tools.formulas.as_deref().map_or(0, SvgFormulas::generation),
            rope: tab.edit.editor.document().rope().clone(),
            format: tab.edit.format,
            blocks: (model.parsed == Some(rev)).then(|| Arc::clone(&model.blocks)),
            last_pictures: self.last_pictures.clone(),
            ctx: context(tab, root, tools, session),
        }
    }

    /// Takes in what a job made into `model`, unless a later job's result
    /// was taken in already; the changes for the window, each with the
    /// version it applies to.
    fn commit(&mut self, model: &mut FacadeModel, built: Built) -> Option<Vec<Versioned>> {
        if built.seq <= self.committed {
            return None;
        }
        self.committed = built.seq;
        let changes = splices(&model.views, &built.views);
        model.views = built.views;
        model.blocks = built.blocks;
        model.parsed = Some(built.rev);
        self.looks = Some(built.looks);
        self.diagrams_seen = built.generation;
        self.formulas_seen = built.formulas_generation;
        self.last_pictures = built.last_pictures;
        if let Some(refs) = built.refs {
            model.refs = refs;
        }
        Some(model.versioned(changes))
    }

    /// M-09.7: the figures of the diagram blocks of `model` that changed
    /// since the window was told, worked into the views; the other views
    /// stay.
    fn figures(&mut self, model: &mut FacadeModel, tools: &Tools) -> Vec<Versioned> {
        let mut changes = self.formulas(model, tools);
        let generation = tools.diagrams.as_deref().map_or(0, SvgDiagrams::generation);
        if generation == self.diagrams_seen {
            return model.versioned(changes);
        }
        self.diagrams_seen = generation;
        let count = model.blocks.len();
        for index in 0..count.min(model.views.len()) {
            let Some(language) = diagram_language(&model.blocks[index].kind) else {
                continue;
            };
            let view = &model.views[index];
            let code: String = view.runs.iter().map(|r| r.text.as_str()).collect();
            let figure = diagram_figure(
                tools.diagrams.as_deref(),
                &mut self.last_pictures,
                count,
                (index, language, &code, tools.dark),
            );
            let view = &mut model.views[index];
            if view.figure.as_ref() != Some(&figure) {
                view.figure = Some(figure);
                changes.push(Splice {
                    from: index,
                    removed: 1,
                    blocks: vec![view.clone()],
                    shift: 0,
                    line_shift: 0,
                });
            }
        }
        model.versioned(changes)
    }

    /// FR-098: the formulas of `model` typeset since the views were made
    /// worked into the views; the changes for the window.
    fn formulas(&mut self, model: &mut FacadeModel, tools: &Tools) -> Vec<Splice> {
        let generation = tools.formulas.as_deref().map_or(0, SvgFormulas::generation);
        if generation == self.formulas_seen {
            return Vec::new();
        }
        self.formulas_seen = generation;
        let mut changes = Vec::new();
        let count = model.blocks.len().min(model.views.len());
        for index in 0..count {
            let block = &model.blocks[index];
            let has = matches!(block.kind, BlockKind::Formula { .. })
                || block.runs.iter().any(|run| run.style.formula.is_some())
                || matches!(&block.kind, BlockKind::Table(table)
                    if table.rows.iter().flatten().flatten().any(|run| run.style.formula.is_some()));
            if !has {
                continue;
            }
            let view = &mut model.views[index];
            let changed = refresh_formulas(block, view, index, &mut |_, source, display| {
                build::formula_view(
                    tools.formulas.as_deref(),
                    tools.looks.0,
                    tools.color,
                    source,
                    display,
                )
            });
            if changed {
                changes.push(Splice {
                    from: index,
                    removed: 1,
                    blocks: vec![view.clone()],
                    shift: 0,
                    line_shift: 0,
                });
            }
        }
        changes
    }

    /// ADR-025, `docs/migration/variante-b.md` section 2.3: what the backend
    /// made of the blocks of `model` (the colours of code, the figures of
    /// diagrams and images, by their content) that the window was not sent
    /// yet, in `looks`; `None` when there is nothing new. Code whose colours
    /// are still to come is left out.
    fn enrich(&mut self, model: &FacadeModel, looks: Looks, dark: bool) -> Option<Enriched> {
        if self.looks != Some(looks) || model.parsed.is_none() {
            // The views are not made in this look yet.
            return None;
        }
        let new_looks = self.sent.looks != Some(looks);
        if !new_looks && self.sent.version == Some(model.version) {
            return None;
        }
        // The window keeps what was sent since the last `clear`: past the
        // bound both start anew with the colours and figures of the blocks
        // now (ADR-025, `CODE_COLORS_KEPT`).
        let full = self.sent.colors.len()
            + self.sent.diagrams.len()
            + self.sent.images.len()
            + self.sent.formulas.len()
            >= CODE_COLORS_KEPT;
        let clear = new_looks || full;
        if clear {
            self.sent = Sent {
                looks: Some(looks),
                ..Sent::default()
            };
        }
        let mut out = Enriched {
            clear,
            ..Enriched::default()
        };
        let theme = looks.0.label();
        for (index, (block, view)) in model.blocks.iter().zip(&model.views).enumerate() {
            self.sent_formulas(theme, view, &mut out.formulas);
            let code = || {
                view.runs
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
            };
            match &block.kind {
                BlockKind::Code { language } => match diagram_language(&block.kind) {
                    Some(diagram) => {
                        let (Some(figure), key) =
                            (&view.figure, diagram_key(diagram, &code(), dark))
                        else {
                            continue;
                        };
                        if self.sent.diagrams.get(&key) != Some(figure) {
                            self.sent.diagrams.insert(key.clone(), figure.clone());
                            out.diagrams.push((key, figure.clone()));
                        }
                    }
                    None => {
                        if !looks.1 || self.recolor.contains(&index) {
                            continue;
                        }
                        let key = colors_key(looks.0.label(), language, &code());
                        if self.sent.colors.get(&key) != Some(&view.colors) {
                            self.sent.colors.insert(key.clone(), view.colors.clone());
                            out.colors.push((key, view.colors.clone()));
                        }
                    }
                },
                BlockKind::Image { target, .. } => {
                    let Some(figure) = &view.figure else {
                        continue;
                    };
                    let figure = FigureView {
                        caption: None,
                        ..figure.clone()
                    };
                    if self.sent.images.get(target) != Some(&figure) {
                        self.sent.images.insert(target.clone(), figure.clone());
                        out.images.push((target.clone(), figure));
                    }
                }
                _ => {}
            }
        }
        self.sent.version = Some(model.version);
        let any = out.clear
            || !out.colors.is_empty()
            || !out.diagrams.is_empty()
            || !out.images.is_empty()
            || !out.formulas.is_empty();
        any.then_some(out)
    }

    /// FR-098: the formulas of `view` typeset, or with their error, that
    /// the window was not sent yet, by key, into `out`; one still being
    /// typeset is sent once it is done.
    fn sent_formulas(
        &mut self,
        theme: &str,
        view: &BlockView,
        out: &mut Vec<(String, FormulaView)>,
    ) {
        let block = match &view.kind {
            BlockKindView::Formula { formula } => Some(formula),
            _ => None,
        };
        let cells = match &view.kind {
            BlockKindView::Table { table } => table.rows.iter().flatten().flatten().collect(),
            _ => Vec::new(),
        };
        let inline = view
            .runs
            .iter()
            .chain(cells)
            .filter_map(|run| run.formula.as_ref());
        for formula in block.into_iter().chain(inline) {
            if formula.picture.is_none() && formula.error.is_none() {
                continue;
            }
            let key = formula_key(theme, formula.display, &formula.source);
            if self.sent.formulas.get(&key) != Some(formula) {
                self.sent.formulas.insert(key.clone(), formula.clone());
                out.push((key, formula.clone()));
            }
        }
    }

    /// ADR-025: the window's facade starts anew (a new core): everything is
    /// sent again.
    pub(super) fn forget_sent(&mut self) {
        self.sent = Sent::default();
    }

    /// Whether the facade is shown.
    pub(super) fn shown(&self) -> bool {
        self.shown
    }
}

/// What [`Facade::enrich`] found new.
#[derive(Debug, Default)]
struct Enriched {
    clear: bool,
    colors: Vec<(String, Option<CodeColors>)>,
    diagrams: Vec<(String, FigureView)>,
    images: Vec<(String, FigureView)>,
    formulas: Vec<(String, FormulaView)>,
}

/// What making views needs of the session and the tab.
fn context(tab: &Tab, root: &std::path::Path, tools: &Tools, session: &Shared) -> Context {
    Context {
        looks: tools.looks,
        colors: tools.colors.clone(),
        dark: tools.dark,
        diagrams: tools.diagrams.clone(),
        formulas: tools.formulas.clone(),
        color: tools.color,
        code_colors: Arc::clone(&session.code_colors),
        images: Arc::clone(&session.images),
        root: root.to_owned(),
        note: tab.path.clone(),
    }
}

/// What the facades of a session share: the colours of code blocks, what
/// was found of image files, and the way back for the tick's jobs.
pub(super) struct Shared {
    code_colors: Arc<Mutex<CodeColorCache>>,
    images: Arc<Mutex<ImageCache>>,
    send: Sender<(NoteId, Built)>,
    built: Receiver<(NoteId, Built)>,
}

impl Default for Shared {
    fn default() -> Self {
        let (send, built) = mpsc::channel();
        Self {
            code_colors: Arc::default(),
            images: Arc::default(),
            send,
            built,
        }
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("facade::Shared")
    }
}

impl Session {
    /// The diagrams of the facade (ADR-009), started with the first facade.
    /// Should they not start, every diagram says it cannot be rendered.
    fn svg_diagrams(&mut self) -> Option<Arc<SvgDiagrams>> {
        if self.diagrams.is_none() {
            match SvgDiagrams::start(self.wake.clone()) {
                Ok(diagrams) => self.diagrams = Some(Arc::new(diagrams)),
                Err(error) => self.status = format!("Diagrams cannot be rendered: {error}"),
            }
        }
        self.diagrams.clone()
    }

    /// FR-098, ADR-029: the formulas of the facade, on a queue of their
    /// own, started with the first facade. Should they not start, every
    /// formula says it cannot be typeset.
    fn svg_formulas(&mut self) -> Option<Arc<SvgFormulas>> {
        if self.formulas.is_none() {
            match SvgFormulas::start(self.wake.clone()) {
                Ok(formulas) => self.formulas = Some(Arc::new(formulas)),
                Err(error) => self.status = format!("Formulas cannot be typeset: {error}"),
            }
        }
        self.formulas.clone()
    }

    /// What the facade is made with: the colours for code (once ready), the
    /// theme's darkness for diagrams, the diagrams, and the formulas in the
    /// theme's text colour.
    fn facade_tools(&mut self) -> Tools {
        let colors = self.colors();
        let theme = self.settings.effective().theme;
        let palette = underline_facade::themes::palette(theme);
        Tools {
            looks: (theme, colors.is_some()),
            colors,
            dark: palette.dark,
            diagrams: self.svg_diagrams(),
            formulas: self.svg_formulas(),
            color: palette.foreground,
        }
    }

    /// M-07.2, UC-004 BR-003: the tab of `note` shows `view` from now on; it
    /// is remembered with the tabs (M-22.2). The tab of the active pane,
    /// else of the pane that has one (BR-018).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn set_editing_view(
        &mut self,
        note: &NoteId,
        view: EditingViewName,
    ) -> Result<(), UiError> {
        let side = self.pane_of(note);
        self.set_editing_view_in(side, note, view)
    }

    /// M-07.2, UC-004 BR-003, UC-005 BR-016: the tab of `note` in the pane
    /// `side` shows `view` from now on, remembered with the tabs; a tab of
    /// the note in the other pane keeps its own view.
    ///
    /// # Errors
    ///
    /// The pane has no tab of the note.
    pub fn set_editing_view_in(
        &mut self,
        side: PaneSide,
        note: &NoteId,
        view: EditingViewName,
    ) -> Result<(), UiError> {
        let not_open = || {
            UiError::new(
                UiErrorKind::Invalid,
                format!("The tab {} is not open", note.title()),
            )
        };
        let open = self.open.as_mut().ok_or_else(not_open)?;
        if !open.panes.has(side, note) || open.tabs.find(note).is_none() {
            return Err(not_open());
        }
        let view = EditingView::from(view);
        let changed = open.panes.set_view(side, note, view);
        // ADR-025: the tick makes the blocks' colours and figures for the
        // window's facade (`ed-enrich`) while a tab shows it; the blocks
        // stay while only the source view shows, so switching back is quick.
        open.sync_active();
        if view == EditingView::Facade {
            self.wake.wake();
        }
        if changed {
            self.persist_tabs();
        }
        Ok(())
    }

    /// ADR-024, ADR-025: the tick's part of the facade: the results of the
    /// jobs are taken in where they are still for the tab's text, a new job
    /// starts when the active tab's text or look changed, a diagram that
    /// became ready changes its figure, and code gets its colours. The
    /// window's editor core makes the blocks itself; what it gets is their
    /// colours and figures (`ed-enrich`).
    pub(super) fn facade_events(&mut self) -> Vec<Event> {
        self.take_built();
        let mut out = Vec::new();
        // UC-004 BR-018: the facade each pane shows now (its active tab
        // shows it), the active pane's first; a facade tab not shown is not
        // kept current.
        let shown: Vec<NoteId> = self.open.as_ref().map_or_else(Vec::new, |open| {
            open.panes
                .shown_facades()
                .into_iter()
                .filter(|note| {
                    open.tabs.find(note).is_some_and(|index| {
                        open.tabs
                            .get(index)
                            .facade
                            .as_ref()
                            .is_some_and(Facade::shown)
                    })
                })
                .collect()
        });
        if shown.is_empty() {
            return out;
        }
        let tools = self.facade_tools();
        for note in shown {
            self.facade_event(&note, &tools, &mut out);
        }
        out
    }

    /// [`Session::facade_events`] for the facade of `note`.
    fn facade_event(&mut self, note: &NoteId, tools: &Tools, out: &mut Vec<Event>) {
        let wake = self.wake.clone();
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let root = open.root().to_owned();
        let Some(index) = open.tabs.find(note) else {
            return;
        };
        let tab = open.tabs.get_mut(index);
        let rev = tab.edit.editor.revision();
        let Some(mut facade) = tab.facade.take() else {
            return;
        };
        if facade.building {
            // The running job brings the figures too.
        } else if facade.stale(&tab.edit.facade, rev, tools.looks) {
            let job = facade.job(tab, &root, tools, &self.facade);
            let send = self.facade.send.clone();
            let note = note.clone();
            facade.building = true;
            let spawned = std::thread::Builder::new()
                .name("underline-facade".into())
                .spawn(move || {
                    let built = build::run(job);
                    let _ = send.send((note, built));
                    wake.wake();
                });
            if let Err(error) = spawned {
                facade.building = false;
                self.status = format!("The facade could not be made: {error}");
            }
        } else if tab.edit.facade.parsed.is_some() {
            // The changes count the views' versions, which `enrich` follows.
            facade.figures(&mut tab.edit.facade, tools);
            if !facade.recolor.is_empty() {
                let ctx = context(tab, &root, tools, &self.facade);
                facade.recolored(&mut tab.edit.facade, &ctx);
            }
            if let Some(found) = facade.enrich(&tab.edit.facade, tools.looks, tools.dark) {
                out.push(Event::Enrich(Box::new(EnrichView {
                    note: note.as_str().to_owned(),
                    rev: tab.mirror.rev(),
                    theme: tools.looks.0.label().to_owned(),
                    dark: tools.dark,
                    clear: found.clear,
                    colors: found.colors,
                    diagrams: found.diagrams,
                    images: found.images,
                    formulas: found.formulas,
                })));
            }
        }
        tab.facade = Some(facade);
    }

    /// Takes in the results of the tick's jobs that are still for the text
    /// of their tab; the others are dropped, and the next tick starts anew.
    fn take_built(&mut self) {
        while let Ok((note, built)) = self.facade.built.try_recv() {
            let Some(open) = self.open.as_mut() else {
                continue;
            };
            let Some(index) = open.tabs.find(&note) else {
                continue;
            };
            let tab = open.tabs.get_mut(index);
            let rev = tab.edit.editor.revision();
            let Some(facade) = tab.facade.as_mut() else {
                continue;
            };
            facade.building = false;
            if built.rev != rev {
                continue;
            }
            let _ = facade.commit(&mut tab.edit.facade, built);
        }
    }

    /// ADR-023: what the app's own protocol serves for the request path
    /// `path` (`/file/…` or `/diagram/….svg`, a query left out), as a plan
    /// resolved without the session ([`AssetPlan::resolve`]).
    ///
    /// # Errors
    ///
    /// [`AssetError::Refused`] for every path outside what it serves,
    /// [`AssetError::NotFound`] when there are no diagrams yet.
    pub fn asset_plan(&self, path: &str) -> Result<AssetPlan, AssetError> {
        let path = request_path(path);
        if let Some(rest) = path.strip_prefix("diagram/") {
            let name = rest.strip_suffix(".svg").ok_or(AssetError::Refused)?;
            let diagrams = self.diagrams.as_ref().ok_or(AssetError::NotFound)?;
            return Ok(AssetPlan::diagram(Arc::clone(diagrams), name));
        }
        // FR-098, ADR-029: a typeset formula.
        if let Some(rest) = path.strip_prefix("formula/") {
            let name = rest.strip_suffix(".svg").ok_or(AssetError::Refused)?;
            let formulas = self.formulas.as_ref().ok_or(AssetError::NotFound)?;
            return Ok(AssetPlan::formula(Arc::clone(formulas), name));
        }
        if let Some(rest) = path.strip_prefix("file/") {
            let root = self.workspace_root().ok_or(AssetError::Refused)?;
            let relative = percent_decode(rest).ok_or(AssetError::Refused)?;
            return Ok(AssetPlan::file(root.to_owned(), relative));
        }
        Err(AssetError::Refused)
    }

    /// [`Session::asset_plan`], resolved at once (for tests and tools).
    ///
    /// # Errors
    ///
    /// As [`Session::asset_plan`] and [`AssetPlan::resolve`].
    pub fn asset(&self, path: &str) -> Result<Asset, AssetError> {
        self.asset_plan(path)?.resolve()
    }
}

#[cfg(test)]
mod tests;
