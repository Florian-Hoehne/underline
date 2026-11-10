//! UC-005, ADR-024: the inputs of the facade in the session. Each input is
//! one command: the facade is brought up to date with the text, and the
//! editor's tab (`underline_editor_view::EditorTab`, ADR-025) takes the
//! selection the window shows, gives the input to the Vim or Nano keys and
//! what they leave to the facade's rules, and answers with only the blocks
//! that changed, with the cursor. Highlighting code and the sections follow
//! from the tick (ADR-024).
//!
//! This is the adapter of the editor's tab to the session: the keys of the
//! settings, the views made with the session's highlighting, diagrams and
//! images ([`Making`]), the autosave clock and the tab marks.

use std::collections::HashMap;
use std::ops::Range;

use ropey::Rope;
use underline_core::NoteId;
use underline_editor_core::facade::{moved_indices, MakeViews, Part};
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_editor_view::views::{carry_colors, diagram_language, ViewMaker, WebWalker};
use underline_editor_view::{EditorTab, FacadeCall, FacadeModel, Handled, Versioned};
use underline_facade::blocks::{Block, BlockKind};

use super::build::{colors_key, remember_colors, Context, Maker};
use super::{context, not_open, Facade, Splice};
use crate::editor_view::{ClipAction, KeysStatusView};
use crate::facade_view::{BlockKindView, BlockView, FacadeAt, FacadeInput, FacadePatch, RowsView};
use crate::session::editing::{after_input, keys_mode, millis};
use crate::session::tab::Tab;
use crate::session::Session;
use crate::{Platform, UiError};

/// ADR-025: makes the views of the blocks the editor core's [`refresh`]
/// asks for, with the session's highlighting, diagrams and images.
///
/// [`refresh`]: underline_editor_core::facade::refresh
struct Making<'a> {
    ctx: &'a Context,
    /// M-09.7: the last picture of each diagram block.
    last_pictures: &'a mut HashMap<usize, String>,
    /// Code blocks sent without their colours, which the tick makes.
    recolor: &'a mut Vec<usize>,
}

impl MakeViews for Making<'_> {
    type View = BlockView;

    fn all(&mut self, blocks: &[Block], text: &str, rope: &Rope) -> Vec<BlockView> {
        let mut maker = Maker::new(self.ctx, self.last_pictures.clone());
        let views = maker.views(blocks, text, rope);
        *self.last_pictures = maker.last_pictures;
        views
    }

    fn some(
        &mut self,
        blocks: &[Block],
        range: Range<usize>,
        part: Part<'_>,
        rope: &Rope,
    ) -> Vec<BlockView> {
        let mut walker = WebWalker::part(part.text, part.from, part.origin);
        let mut maker = Maker::new(self.ctx, std::mem::take(self.last_pictures));
        maker.colour_now = false;
        let views = ViewMaker::new(&mut maker, blocks).views_of_range(range, &mut walker, rope);
        *self.last_pictures = maker.last_pictures;
        views
    }

    fn blocks_moved(&mut self, replaced: Range<usize>, grown: isize) {
        let last = std::mem::take(self.last_pictures);
        *self.last_pictures = moved_indices(last, replaced, grown);
    }

    fn made(&mut self, index: usize, view: &mut BlockView, old: Option<&BlockView>) {
        let code = matches!(view.kind, BlockKindView::Code { .. }) && view.figure.is_none();
        if !code || view.colors.is_some() || self.ctx.colors.is_none() {
            return;
        }
        self.recolor.push(index);
        if let Some(old) = old.filter(|old| old.kind == view.kind) {
            view.colors = carry_colors(old, view);
        }
    }
}

/// Makes all views of a note anew with the session's highlighting,
/// diagrams and images, colouring code at once.
struct AllViews<'a> {
    ctx: &'a Context,
    last_pictures: HashMap<usize, String>,
}

impl MakeViews for AllViews<'_> {
    type View = BlockView;

    fn all(&mut self, blocks: &[Block], text: &str, rope: &Rope) -> Vec<BlockView> {
        let mut maker = Maker::new(self.ctx, std::mem::take(&mut self.last_pictures));
        let views = maker.views(blocks, text, rope);
        self.last_pictures = maker.last_pictures;
        views
    }

    fn some(
        &mut self,
        blocks: &[Block],
        range: Range<usize>,
        part: Part<'_>,
        rope: &Rope,
    ) -> Vec<BlockView> {
        let mut walker = WebWalker::part(part.text, part.from, part.origin);
        let mut maker = Maker::new(self.ctx, std::mem::take(&mut self.last_pictures));
        let views = ViewMaker::new(&mut maker, blocks).views_of_range(range, &mut walker, rope);
        self.last_pictures = maker.last_pictures;
        views
    }

    fn blocks_moved(&mut self, replaced: Range<usize>, grown: isize) {
        let last = std::mem::take(&mut self.last_pictures);
        self.last_pictures = moved_indices(last, replaced, grown);
    }

    fn made(&mut self, _index: usize, _view: &mut BlockView, _old: Option<&BlockView>) {}
}

impl Facade {
    /// Makes the blocks of `edit`'s text and their views anew, when they
    /// are not of its revision; the changes for the window.
    fn bring_up_to_date(&mut self, edit: &mut EditorTab, ctx: &Context) -> Vec<Versioned> {
        if edit.facade.parsed == Some(edit.editor.revision()) {
            return Vec::new();
        }
        let mut maker = AllViews {
            ctx,
            last_pictures: self.last_pictures.clone(),
        };
        let changes = edit.bring_facade_up_to_date(&mut maker);
        let mut last_pictures = maker.last_pictures;
        last_pictures.retain(|index, _| *index < edit.facade.blocks.len());
        self.looks = Some(ctx.looks);
        self.last_pictures = last_pictures;
        // A job of the tick that runs now is for an older text.
        self.committed = self.next_seq;
        changes
    }

    /// M-09.4, ADR-024: the code blocks of `model` sent without their
    /// colours get them; the changes for the window.
    pub(super) fn recolored(&mut self, model: &mut FacadeModel, ctx: &Context) -> Vec<Versioned> {
        let wanted = std::mem::take(&mut self.recolor);
        let Some(set) = ctx.colors.as_deref() else {
            return Vec::new();
        };
        let mut changes = Vec::new();
        let mut done = Vec::new();
        for index in wanted {
            if done.contains(&index) {
                continue;
            }
            done.push(index);
            let (Some(block), Some(view)) = (model.blocks.get(index), model.views.get(index))
            else {
                continue;
            };
            let BlockKind::Code { language } = &block.kind else {
                continue;
            };
            if diagram_language(&block.kind).is_some() {
                continue;
            }
            let code: String = view.runs.iter().map(|r| r.text.as_str()).collect();
            let key = colors_key(ctx.looks.0, language, &code);
            let known = ctx
                .code_colors
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&key)
                .cloned();
            let colors = match known {
                Some(colors) => colors,
                None => remember_colors(&ctx.code_colors, key, set, language, &code),
            };
            if view.colors != colors {
                let view = &mut model.views[index];
                view.colors = colors;
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
}

/// What a facade input starts from: the tab, its facade brought up to date,
/// what views are made with, and the changes that brought it up to date.
type Begun<'a> = (&'a mut Tab, Facade, Context, Vec<Versioned>);

impl Session {
    /// The keys of the facade: the input mode of the settings, with the
    /// editor core's Vim keys (the source view's are the window's, A-01).
    pub(in crate::session) fn facade_keys_mode(&self) -> KeysInputMode {
        keys_mode(self.settings.effective().input_mode)
    }

    /// M-11.4, FR-026: the left part of the status line while the facade
    /// shows the note of `note`, in the input mode of the settings.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn facade_keys_status(&mut self, note: &NoteId) -> Result<Option<KeysStatusView>, UiError> {
        let mode = self.facade_keys_mode();
        let open = self.open.as_mut().ok_or_else(|| not_open(note))?;
        let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
        let tab = open.tabs.get_mut(index);
        tab.edit.keys.mode = mode;
        Ok(tab.edit.facade_keys_status())
    }

    /// The tab of `note` with what its facade's views are made with, and the
    /// facade brought up to date with its text.
    fn facade_begin(&mut self, note: &NoteId) -> Result<Begun<'_>, UiError> {
        let mode = self.facade_keys_mode();
        let tools = self.facade_tools();
        let open = self.open.as_mut().ok_or_else(|| not_open(note))?;
        let root = open.root().to_owned();
        let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
        let tab = open.tabs.get_mut(index);
        tab.edit.keys.mode = mode;
        tab.edit.set_now(millis(std::time::Instant::now()));
        let ctx = context(tab, &root, &tools, &self.facade);
        let mut facade = tab.facade.take().unwrap_or_default();
        facade.shown = true;
        let changes = facade.bring_up_to_date(&mut tab.edit, &ctx);
        Ok((tab, facade, ctx, changes))
    }

    /// The patch of `handled` for the window, after the session took it in;
    /// the facade goes back to `tab`.
    fn facade_took(
        tab: &mut Tab,
        mut facade: Facade,
        handled: Handled<FacadePatch>,
        marked: &mut bool,
    ) -> FacadePatch {
        if handled.recaptioned {
            // The figures and tables after them are numbered anew.
            facade.looks = None;
        }
        tab.facade = Some(facade);
        let mut patch = handled.patch;
        *marked |= after_input(tab, &handled.outcome, &mut patch.unstored);
        patch
    }

    /// UC-005, M-09.10 to M-09.13, M-11.2, M-11.3, M-10.1: one input of the
    /// facade at the selection `at` the window shows. `rows` are the rows
    /// the window measured for the cursor (M-09.13), `clipboard` the system
    /// clipboard's text when the Vim keys asked for it. Only the edit, the
    /// blocks around it and the patch are worked out here (ADR-024).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn facade_input(
        &mut self,
        note: &NoteId,
        at: FacadeAt,
        input: FacadeInput,
        rows: Option<RowsView>,
        clipboard: Option<String>,
    ) -> Result<FacadePatch, UiError> {
        let mac = Platform::current() == Platform::MacOs;
        let (tab, mut facade, ctx, earlier) = self.facade_begin(note)?;
        let mut making = Making {
            ctx: &ctx,
            last_pictures: &mut facade.last_pictures,
            recolor: &mut facade.recolor,
        };
        let call = FacadeCall {
            note: note.as_str(),
            at,
            mac,
            earlier,
            views: &mut making,
        };
        let handled = tab
            .edit
            .facade_input(call, input, rows, clipboard.as_deref());
        let mut marked = false;
        let patch = Self::facade_took(tab, facade, handled, &mut marked);
        // M-06.1: the tab is marked " •" now.
        self.views_dirty |= marked;
        Ok(patch)
    }

    /// M-09.14, M-08.6: copy, cut or paste in the facade at the selection
    /// `at`. Copying and cutting give the selected markup for the
    /// clipboard, which pasting it brings back as markup; any other text
    /// is pasted as typed, escaped, and so is everything with `plain`
    /// (Ctrl+Shift+V). `text` is what the window's backend read from the
    /// clipboard before it took the session. Formatted text from other
    /// programs (HTML) comes with feature F6.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn facade_clipboard(
        &mut self,
        note: &NoteId,
        at: FacadeAt,
        action: ClipAction,
        plain: bool,
        text: Option<&str>,
    ) -> Result<(Option<String>, FacadePatch), UiError> {
        let mac = Platform::current() == Platform::MacOs;
        let (tab, mut facade, ctx, earlier) = self.facade_begin(note)?;
        let mut making = Making {
            ctx: &ctx,
            last_pictures: &mut facade.last_pictures,
            recolor: &mut facade.recolor,
        };
        let call = FacadeCall {
            note: note.as_str(),
            at,
            mac,
            earlier,
            views: &mut making,
        };
        let (copied, handled) = tab.edit.facade_clipboard(call, action, plain, text);
        let mut marked = false;
        let patch = Self::facade_took(tab, facade, handled, &mut marked);
        self.views_dirty |= marked;
        Ok((copied, patch))
    }
}

#[cfg(test)]
mod tests;
