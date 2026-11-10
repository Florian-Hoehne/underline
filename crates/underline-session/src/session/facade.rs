//! UC-004, M-09, ADR-021, ADR-024: the facade of an open note in the
//! session. The blocks of the facade engine (`underline_facade::blocks`) are
//! kept per tab and sent to the window as [`BlockView`]s: the whole facade
//! when it opens (`fa_open`), afterwards only the blocks that changed
//! (`ed-blocks`).
//!
//! Parsing and making the views never run while the session is held, so a
//! keystroke never waits for them (ADR-024): the tick hands a snapshot of
//! the text (a rope, copied in no time) to a thread of its own
//! (`build.rs`) and takes the result in with a later tick, but only while
//! it is still for the text the tab has, and only if no later result was
//! taken in already. `fa_open` makes its views the same way, outside the
//! session (`facade_prepare`, `FacadePrep::build`, `facade_finish`). Only a
//! diagram that became ready is worked in under the session, as it changes
//! one figure.

mod build;
mod input;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use underline_core::{EditingView, NoteId};
use underline_editor_core::facade::splices;
use underline_editor_view::views::diagram_language;
use underline_editor_view::{blocks_views, FacadeModel, Versioned};
use underline_facade::highlight::HighlightSet;

use self::build::{diagram_figure, Built, Context, Job, Looks};
pub(super) use self::build::{CodeColorCache, ImageCache};
use super::diagrams::SvgDiagrams;
use super::tab::Tab;
use super::Session;
use crate::assets::{percent_decode, request_path, Asset, AssetError, AssetPlan};
use crate::facade_view::{BlockView, FacadeState};
use crate::workspace_view::EditingViewName;
use crate::{Event, UiError, UiErrorKind};

#[cfg(test)]
use self::build::views_of;

/// ADR-024: one change of the window's blocks (`ed-blocks`).
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
}

/// What making the views needs of the session.
struct Tools {
    looks: Looks,
    colors: Option<Arc<HighlightSet>>,
    dark: bool,
    diagrams: Option<Arc<SvgDiagrams>>,
}

impl Facade {
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
        let generation = tools.diagrams.as_deref().map_or(0, SvgDiagrams::generation);
        if generation == self.diagrams_seen {
            return Vec::new();
        }
        self.diagrams_seen = generation;
        let count = model.blocks.len();
        let mut changes = Vec::new();
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

    /// Whether the facade is shown.
    pub(super) fn shown(&self) -> bool {
        self.shown
    }
}

/// What making views needs of the session and the tab.
fn context(tab: &Tab, root: &std::path::Path, tools: &Tools, session: &Shared) -> Context {
    Context {
        looks: tools.looks,
        colors: tools.colors.clone(),
        dark: tools.dark,
        diagrams: tools.diagrams.clone(),
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

/// The events of `changes` of the facade of `note`, whose blocks are of
/// revision `rev`.
fn events(note: &NoteId, rev: u64, changes: Vec<Versioned>) -> Vec<Event> {
    blocks_views(note.as_str(), rev, changes)
        .into_iter()
        .map(Event::Blocks)
        .collect()
}

fn not_open(note: &NoteId) -> UiError {
    UiError::new(
        UiErrorKind::Invalid,
        format!("The tab {} is not open", note.title()),
    )
}

/// `fa_open`, first step: the facade as it is, or the job that makes it.
#[derive(Debug)]
pub struct FacadePrep(Prep);

#[derive(Debug)]
enum Prep {
    Ready(FacadeState),
    Build(NoteId, Box<Job>),
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("seq", &self.seq)
            .finish_non_exhaustive()
    }
}

/// `fa_open`, second step: what was made, to take in.
pub struct FacadeBuilt(Done);

enum Done {
    Ready(FacadeState),
    Built(NoteId, Box<Built>),
}

impl FacadePrep {
    /// Makes the views if they must be made; runs without the session.
    #[must_use]
    pub fn build(self) -> FacadeBuilt {
        FacadeBuilt(match self.0 {
            Prep::Ready(state) => Done::Ready(state),
            Prep::Build(note, job) => Done::Built(note, Box::new(build::run(*job))),
        })
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

    /// What the facade is made with: the colours for code (once ready), the
    /// theme's darkness for diagrams, and the diagrams.
    fn facade_tools(&mut self) -> Tools {
        let colors = self.colors();
        let theme = self.settings.effective().theme;
        Tools {
            looks: (theme, colors.is_some()),
            colors,
            dark: underline_facade::themes::palette(theme).dark,
            diagrams: self.svg_diagrams(),
        }
    }

    /// M-09, ADR-024, first step of `fa_open`: the facade of the tab of
    /// `note` is shown from now on; its views as they are, or the job that
    /// makes them, which [`FacadePrep::build`] runs without the session.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn facade_prepare(&mut self, note: &NoteId) -> Result<FacadePrep, UiError> {
        let tools = self.facade_tools();
        let open = self.open.as_mut().ok_or_else(|| not_open(note))?;
        let root = open.root().to_owned();
        let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
        let tab = open.tabs.get_mut(index);
        // The source view is not shown; the sections are sent again.
        tab.viewport = None;
        tab.highlight_told = None;
        tab.edit.forget_sections();
        let mut facade = tab.facade.take().unwrap_or_default();
        facade.shown = true;
        let prep = if facade.stale(&tab.edit.facade, tab.edit.editor.revision(), tools.looks) {
            // The tick starts no job of its own meanwhile.
            facade.building = true;
            Prep::Build(
                note.clone(),
                Box::new(facade.job(tab, &root, &tools, &self.facade)),
            )
        } else {
            // The window gets all blocks; the changed figures need no event.
            facade.figures(&mut tab.edit.facade, &tools);
            Prep::Ready(tab.edit.facade_state(note.as_str()))
        };
        tab.facade = Some(facade);
        Ok(FacadePrep(prep))
    }

    /// M-09, ADR-024, last step of `fa_open`: takes in what was made, unless
    /// a later result was taken in meanwhile, and gives the whole facade
    /// with the cursor. From now on the tick sends the blocks that change
    /// (`ed-blocks`), and the sections for the fold arrows.
    ///
    /// # Errors
    ///
    /// The note's tab was closed meanwhile.
    pub fn facade_finish(&mut self, built: FacadeBuilt) -> Result<FacadeState, UiError> {
        let (note, built) = match built.0 {
            Done::Ready(state) => return Ok(state),
            Done::Built(note, built) => (note, built),
        };
        let open = self.open.as_mut().ok_or_else(|| not_open(&note))?;
        let index = open.tabs.find(&note).ok_or_else(|| not_open(&note))?;
        let tab = open.tabs.get_mut(index);
        let facade = tab.facade.get_or_insert_with(Facade::default);
        facade.building = false;
        // The window takes all blocks; a stale text is followed up by the
        // tick, with its changes.
        let _ = facade.commit(&mut tab.edit.facade, *built);
        Ok(tab.edit.facade_state(note.as_str()))
    }

    /// M-09, ADR-024: `fa_open` in one go (holding the session throughout).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn facade_state(&mut self, note: &NoteId) -> Result<FacadeState, UiError> {
        let prep = self.facade_prepare(note)?;
        self.facade_finish(prep.build())
    }

    /// M-07.2, UC-004 BR-003: the tab of `note` shows `view` from now on; it
    /// is remembered with the tabs (M-22.2).
    pub fn set_editing_view(&mut self, note: &NoteId, view: EditingViewName) {
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(index) = open.tabs.find(note) else {
            return;
        };
        let tab = open.tabs.get_mut(index);
        let view = EditingView::from(view);
        if tab.editing_view == view {
            return;
        }
        tab.editing_view = view;
        if let (EditingView::Source, Some(facade)) = (view, tab.facade.as_mut()) {
            // Kept, but no longer kept current until it is shown again.
            facade.shown = false;
        }
        self.persist_tabs();
    }

    /// ADR-024: the tick's part of the facade: the results of the jobs are
    /// taken in where they are still for the tab's text, a new job starts
    /// when the active tab's text or look changed, and a diagram that became
    /// ready changes its figure. Gives the blocks that changed.
    pub(super) fn facade_events(&mut self) -> Vec<Event> {
        let mut out = self.take_built();
        let shows_facade = self
            .open
            .as_ref()
            .and_then(|open| open.tabs.active())
            .is_some_and(|(_, tab)| tab.facade.as_ref().is_some_and(Facade::shown));
        if !shows_facade {
            return out;
        }
        let tools = self.facade_tools();
        let wake = self.wake.clone();
        let Some(open) = self.open.as_mut() else {
            return out;
        };
        let root = open.root().to_owned();
        let Some((note, tab)) = open.tabs.active_mut() else {
            return out;
        };
        let rev = tab.edit.editor.revision();
        let Some(mut facade) = tab.facade.take() else {
            return out;
        };
        if facade.building {
            // The running job brings the figures too.
        } else if facade.stale(&tab.edit.facade, rev, tools.looks) {
            let job = facade.job(tab, &root, &tools, &self.facade);
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
        } else if let Some(parsed) = tab.edit.facade.parsed {
            out.extend(events(
                note,
                parsed,
                facade.figures(&mut tab.edit.facade, &tools),
            ));
            if !facade.recolor.is_empty() {
                let ctx = context(tab, &root, &tools, &self.facade);
                out.extend(events(
                    note,
                    parsed,
                    facade.recolored(&mut tab.edit.facade, &ctx),
                ));
            }
        }
        tab.facade = Some(facade);
        out
    }

    /// Takes in the results of the tick's jobs that are still for the text
    /// of their tab; the others are dropped, and the next tick starts anew.
    fn take_built(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
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
            if let Some(changes) = facade.commit(&mut tab.edit.facade, built) {
                out.extend(events(&note, rev, changes));
            }
        }
        out
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
