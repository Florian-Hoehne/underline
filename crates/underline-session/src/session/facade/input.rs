//! UC-005, ADR-024: the inputs of the facade in the session. Each input is
//! one command: the selection the window shows is taken first, the input
//! goes to the Vim or Nano keys and what they leave to the facade's rules
//! (`underline_editor_core::facade`), and the answer carries only the blocks
//! that changed, with the cursor. Highlighting code and the sections follow
//! from the tick (ADR-024).
//!
//! The rules, the reparse and which views are made anew after an input are
//! the editor core's (ADR-025); this is their adapter to the session: it
//! maps the window's inputs, makes the views ([`Making`]) and answers with
//! the patch.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use ropey::Rope;
use underline_core::{InputMode, NoteId};
use underline_editor_core::facade::{
    carry_spans, gap_at, moved_indices, parse_all, refresh, splices, Before, FacadeEdit,
    FacadeTarget, Gap, Gathered, MakeViews, Part, Rows,
};
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_editor_core::{KeyInput, Keys, Modifiers, SourceEditor};
use underline_facade::blocks::Block;

use super::build::{colors_key, diagram_language, remember_colors, Context, Maker, WebWalker};
use super::{context, events, not_open, Facade, Splice, Tools};
use crate::editor_view::{revision, units, ClipAction, KeysStatusView};
use crate::facade_view::{
    BlockKindView, BlockView, BlocksView, CodeColors, FacadeAt, FacadeInput, FacadePatch, GapView,
    RowsView,
};
use crate::session::editing::{after_input, fold_key};
use crate::session::tab::Tab;
use crate::session::Session;
use crate::{Event, Platform, UiError};

/// FR-026: the status of the Vim or Nano keys `keys`; `None` with the usual
/// keys.
pub(super) fn keys_status(keys: &Keys) -> Option<KeysStatusView> {
    keys.status()
        .map(|status| KeysStatusView::of(&status, keys.prompting()))
}

/// What `input` is to the Vim and Nano keys.
fn key_input(input: &FacadeInput, mac: bool) -> Option<KeyInput<'_>> {
    match input {
        FacadeInput::Text { text } => Some(KeyInput::Text(text)),
        FacadeInput::Key {
            key,
            ctrl,
            alt,
            shift,
            meta,
        } => Some(KeyInput::Key {
            key: (*key).into(),
            pressed: true,
            modifiers: Modifiers::from_web(*ctrl, *alt, *shift, *meta, mac),
        }),
        FacadeInput::Tool { .. } => None,
    }
}

/// Runs `work` on the facade's text and blocks, gathering into `gathered`
/// (ADR-025: the edit borrows the one [`Gathered`] of the input).
fn with_edit<R>(
    editor: &mut SourceEditor,
    facade: &mut Facade,
    format: underline_core::NoteFormat,
    gathered: &mut Gathered,
    extra: (Option<Rows>, Option<&str>),
    work: impl FnOnce(&mut FacadeEdit<'_>) -> R,
) -> R {
    let blocks = Arc::make_mut(&mut facade.blocks);
    let mut edit = FacadeEdit::new(editor, blocks, format, &mut facade.refs, gathered)
        .with_rows(extra.0)
        .with_clipboard(extra.1);
    work(&mut edit)
}

/// An input the keys of the input mode left to the facade's own rules.
fn plain_input(edit: &mut FacadeEdit<'_>, input: FacadeInput, mac: bool) {
    match input {
        FacadeInput::Text { text } => edit.type_text(&text),
        FacadeInput::Tool { tool } => edit.apply_tool(tool.into()),
        FacadeInput::Key {
            key,
            ctrl,
            alt,
            shift,
            meta,
        } => edit.key(key.into(), Modifiers::from_web(ctrl, alt, shift, meta, mac)),
    }
}

/// The UTF-16 units of the text of `view`'s runs.
fn units_of(view: &BlockView) -> Vec<u16> {
    view.runs
        .iter()
        .flat_map(|run| run.text.encode_utf16())
        .collect()
}

/// M-09.4, ADR-024: the colours of a code block whose text changed from
/// `old` to `new`, carried over until the tick colours it anew: the colours
/// before the change stay, those after it move with the text
/// ([`carry_spans`]).
fn carry_colors(old: &BlockView, new: &BlockView) -> Option<CodeColors> {
    let colors = old.colors.as_ref()?;
    Some(CodeColors {
        spans: carry_spans(&units_of(old), &units_of(new), &colors.spans),
        styles: colors.styles.clone(),
    })
}

/// M-09.11: the gap for the window.
pub(super) fn gap_view(gap: Gap) -> GapView {
    GapView {
        before: units(gap.before),
        lines: units(gap.lines),
    }
}

/// ADR-025: makes the views of the blocks the editor core's [`refresh`]
/// asks for, with the session's highlighting, diagrams and images.
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
        let mut maker = Maker::new(self.ctx, blocks, self.last_pictures.clone());
        let views = maker.views(text, rope);
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
        let mut maker = Maker::new(self.ctx, blocks, std::mem::take(self.last_pictures));
        maker.colour_now = false;
        let views = maker.views_of_range(range, &mut walker, rope);
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

/// What an input of the facade starts from: the tab, its facade brought up
/// to date, what views are made with, and the changes that brought it up to
/// date.
type Begun<'a> = (&'a mut Tab, Facade, Context, Vec<(u32, Splice)>);

impl Facade {
    /// Makes the blocks of `editor`'s text and their views anew, when they
    /// are not of its revision; the changes for the window.
    fn bring_up_to_date(
        &mut self,
        editor: &SourceEditor,
        format: underline_core::NoteFormat,
        ctx: &Context,
    ) -> Vec<(u32, Splice)> {
        let rev = editor.revision();
        if self.parsed == Some(rev) {
            return Vec::new();
        }
        let text = editor.text();
        let (blocks, refs) = parse_all(&text, format);
        let mut maker = Maker::new(ctx, &blocks, self.last_pictures.clone());
        let views = maker.views(&text, editor.document().rope());
        let mut last_pictures = maker.last_pictures;
        last_pictures.retain(|index, _| *index < blocks.len());
        let changes = splices(&self.views, &views);
        self.views = views;
        self.blocks = Arc::new(blocks);
        self.parsed = Some(rev);
        self.looks = Some(ctx.looks);
        self.refs = refs;
        self.last_pictures = last_pictures;
        // A job of the tick that runs now is for an older text.
        self.committed = self.next_seq;
        self.versioned(changes)
    }

    /// ADR-024: the views of the blocks `dirty` made anew after an input,
    /// the others moved (the editor core's [`refresh`]); the change for the
    /// window.
    fn refresh(
        &mut self,
        editor: &SourceEditor,
        ctx: &Context,
        dirty: Range<usize>,
        before: Before,
    ) -> Vec<(u32, Splice)> {
        let mut making = Making {
            ctx,
            last_pictures: &mut self.last_pictures,
            recolor: &mut self.recolor,
        };
        let refreshed = refresh(
            &mut self.views,
            &self.blocks,
            editor,
            &mut making,
            dirty,
            before,
        );
        if refreshed.recaptioned {
            // The figures and tables after them are numbered anew.
            self.looks = None;
        }
        self.versioned(refreshed.splices)
    }

    /// M-09.4, ADR-024: the code blocks sent without their colours get
    /// them; the changes for the window.
    pub(super) fn recolored(&mut self, ctx: &Context) -> Vec<(u32, Splice)> {
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
            let (Some(block), Some(view)) = (self.blocks.get(index), self.views.get(index)) else {
                continue;
            };
            let underline_facade::blocks::BlockKind::Code { language } = &block.kind else {
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
                let view = &mut self.views[index];
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
        self.versioned(changes)
    }
}

/// The changes for the window as the views of `ed-blocks`.
fn blocks_views(note: &NoteId, rev: u64, changes: Vec<(u32, Splice)>) -> Vec<BlocksView> {
    events(note, rev, changes)
        .into_iter()
        .filter_map(|event| match event {
            Event::Blocks(view) => Some(view),
            _ => None,
        })
        .collect()
}

impl Session {
    /// The keys of the facade: the input mode of the settings, with the
    /// editor core's Vim keys (the source view's are the window's, A-01).
    pub(in crate::session) fn facade_keys_mode(&self) -> KeysInputMode {
        match self.settings.effective().input_mode {
            InputMode::Normal => KeysInputMode::Normal,
            InputMode::Vim => KeysInputMode::Vim,
            InputMode::Nano => KeysInputMode::Nano,
        }
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
        tab.keys.mode = mode;
        Ok(keys_status(&tab.keys))
    }

    /// The tab of `note` with what its facade's views are made with, the
    /// facade brought up to date with its text, and the selection `at` the
    /// window showed taken in.
    ///
    /// ADR-024: the selection is taken in only when `at.rev` is the
    /// revision of the text. A stale `at.rev` (the window has not taken in
    /// the patch of an earlier input yet) drops the window's selection: its
    /// web units would point into another text, so the input applies at the
    /// backend's own cursor, where the earlier input left it.
    fn facade_begin(&mut self, note: &NoteId, at: FacadeAt) -> Result<Begun<'_>, UiError> {
        let mode = self.facade_keys_mode();
        let tools: Tools = self.facade_tools();
        let open = self.open.as_mut().ok_or_else(|| not_open(note))?;
        let root = open.root().to_owned();
        let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
        let tab = open.tabs.get_mut(index);
        tab.keys.mode = mode;
        let ctx = context(tab, &root, &tools, &self.facade);
        let mut facade = tab.facade.take().unwrap_or_default();
        facade.shown = true;
        let changes = facade.bring_up_to_date(&tab.editor, tab.format, &ctx);
        if revision(tab.editor.revision()) == at.rev {
            tab.editor
                .set_selection_web(at.head as usize, at.anchor as usize);
        }
        Ok((tab, facade, ctx, changes))
    }

    /// The patch after an input that started at `before`, with the changes
    /// `changes` of the blocks made before it.
    fn facade_finish_input(
        note: &NoteId,
        tab: &mut Tab,
        mut facade: Facade,
        ctx: &Context,
        mut changes: Vec<(u32, Splice)>,
        before: Before,
        gathered: Gathered,
    ) -> (FacadePatch, bool) {
        let rev = tab.editor.revision();
        facade.parsed = Some(rev);
        if let Some(dirty) = gathered.dirty.range() {
            changes.extend(facade.refresh(&tab.editor, ctx, dirty, before));
        }
        if let Some(copied) = &gathered.copied {
            tab.own_markup = Some(copied.clone());
        }
        let (patch, marked) = after_input(tab, before.rev, &gathered.outcome);
        let gap = gap_at(
            tab.editor.document().rope(),
            tab.editor.cursor(),
            &facade.blocks,
        )
        .map(gap_view);
        let patch = FacadePatch {
            base: revision(before.rev),
            rev: revision(rev),
            blocks: blocks_views(note, rev, changes),
            anchor: patch.anchor,
            head: patch.head,
            gap,
            scroll: patch.scroll,
            unstored: patch.unstored,
            place: patch.place,
            keys: keys_status(&tab.keys),
            commands: gathered.commands.into_iter().map(Into::into).collect(),
            request: gathered.request.map(Into::into),
            copied: gathered.copied,
            wants_clipboard: tab.keys.wants_clipboard(),
        };
        tab.facade = Some(facade);
        (patch, marked)
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
        let (tab, mut facade, ctx, changes) = self.facade_begin(note, at)?;
        let before = Before::of(&tab.editor, facade.blocks.len());
        let rows = rows.map(|rows| Rows {
            at: tab.editor.char_at_web(rows.at as usize),
            up: rows.up.map(|up| tab.editor.char_at_web(up as usize)),
            down: rows.down.map(|down| tab.editor.char_at_web(down as usize)),
            start: tab.editor.char_at_web(rows.start as usize),
            end: tab.editor.char_at_web(rows.end as usize),
        });
        let format = tab.format;
        let mut gathered = Gathered::default();
        // FR-024, FR-025: the Vim or Nano keys take the input first; what
        // they leave goes to the facade's rules right after.
        let through = if tab.keys.mode != KeysInputMode::Normal
            && !matches!(input, FacadeInput::Tool { .. })
        {
            let Tab { keys, editor, .. } = &mut *tab;
            with_edit(
                editor,
                &mut facade,
                format,
                &mut gathered,
                (rows, clipboard.as_deref()),
                |edit| {
                    let mut target = FacadeTarget { edit };
                    keys.handle(vec![input], |i| key_input(i, mac), &mut target)
                        .0
                },
            )
        } else {
            vec![input]
        };
        for input in through {
            if let FacadeInput::Key {
                key,
                ctrl,
                alt,
                shift,
                meta,
            } = &input
            {
                let modifiers = Modifiers::from_web(*ctrl, *alt, *shift, *meta, mac);
                if let Some(fold) = fold_key(*key, modifiers) {
                    // M-08.8: the folds are the tab's, for both views.
                    let moved = tab.fold_key(fold);
                    gathered.moved(moved);
                    continue;
                }
            }
            with_edit(
                &mut tab.editor,
                &mut facade,
                format,
                &mut gathered,
                (rows, clipboard.as_deref()),
                |edit| plain_input(edit, input, mac),
            );
        }
        let (patch, marked) =
            Self::facade_finish_input(note, tab, facade, &ctx, changes, before, gathered);
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
        let (tab, mut facade, ctx, changes) = self.facade_begin(note, at)?;
        let before = Before::of(&tab.editor, facade.blocks.len());
        let format = tab.format;
        let mut gathered = Gathered::default();
        let mut copied = None;
        match action {
            ClipAction::Copy | ClipAction::Cut => {
                let selected = tab.editor.selected_text();
                if !selected.is_empty() {
                    tab.own_markup = Some(selected.clone());
                    if action == ClipAction::Cut {
                        let range = tab.editor.selection();
                        with_edit(
                            &mut tab.editor,
                            &mut facade,
                            format,
                            &mut gathered,
                            (None, None),
                            |edit| edit.replace(range, ""),
                        );
                    }
                    copied = Some(selected);
                }
            }
            ClipAction::Paste => {
                let own = tab.own_markup.clone();
                let Tab { keys, editor, .. } = &mut *tab;
                with_edit(
                    editor,
                    &mut facade,
                    format,
                    &mut gathered,
                    (None, None),
                    |edit| {
                        // A paste ends a run of Nano cuts; Vim's normal mode
                        // drops it, as in egui.
                        let through = keys
                            .handle(
                                vec![()],
                                |()| Some(KeyInput::Paste),
                                &mut FacadeTarget { edit: &mut *edit },
                            )
                            .0;
                        let Some(text) = text.filter(|_| !through.is_empty()) else {
                            return;
                        };
                        if !plain && own.as_deref() == Some(text) {
                            edit.paste_markup(text);
                        } else {
                            edit.type_text(text);
                        }
                    },
                );
            }
        }
        let (patch, marked) =
            Self::facade_finish_input(note, tab, facade, &ctx, changes, before, gathered);
        self.views_dirty |= marked;
        Ok((copied, patch))
    }
}

#[cfg(test)]
mod tests;
