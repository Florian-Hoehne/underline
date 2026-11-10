//! UC-005 Edit Note, ADR-025: one open tab as the window holds it: the text
//! with its cursor and undo ([`SourceEditor`]), the Vim and Nano keys, the
//! folds, the facade's blocks, and the answers to the window's inputs in
//! both views ([`EditorPatch`], [`FacadePatch`]).
//!
//! Moved here from `underline-session` (`session/editing.rs`,
//! `session/facade/input.rs`; `docs/migration/variante-b.md` step B3b), so
//! the same code answers the window in the backend today and in the
//! webview (WASM) with step B5. What only the host knows stays with it:
//! whether the note is stored, the highlighting, the backend's figures
//! (behind [`MakeViews`]); every answer carries the [`Outcome`] the host
//! needs for them.

use std::ops::Range;
use std::sync::Arc;

use underline_editor_core::facade::{
    gap_at, parse_all, refresh, splices, Before, FacadeEdit, FacadeTarget, Gap, Gathered,
    MakeViews, Rows, Splice,
};
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_editor_core::sync::{apply_remote, Incoming, OutOfRange};
use underline_editor_core::{
    EditorCommand, InputStatus, KeyInput, Keys, KeysMode, Modifiers, Outcome, SourceEditor,
    SourceKeys, Tool,
};
use underline_markup::blocks::Block;
use underline_markup::formatting::apply_format;
use underline_markup::NoteFormat;

use crate::editor_view::{
    revision, units, ClipAction, EditorInput, EditorPatch, EditorState, FoundView, KeyName,
    KeysStatusView, TextChange, ToolName, VimChange, VimModeName, VimState,
};
use crate::facade_view::{
    BlockView, BlocksView, FacadeAt, FacadeInput, FacadePatch, FacadeState, GapView, RowsView,
};
use crate::folds::Folds;
use crate::CachedViews;

/// How often the marked hits of the Nano search are found again while the
/// text changes: 300 ms.
pub const FOUND_EVERY_MS: f64 = 300.0;

/// One change of the facade's blocks for the window, with the version it
/// applies to.
pub type Versioned = (u32, Splice<BlockView>);

/// The facade's blocks of the text and their views as the window has them
/// (M-09, ADR-024).
#[derive(Debug, Clone, Default)]
pub struct FacadeModel {
    /// The blocks of the text of revision `parsed`.
    pub blocks: Arc<Vec<Block>>,
    /// The revision of the text the blocks are of.
    pub parsed: Option<u64>,
    /// The views as the window has them.
    pub views: Vec<BlockView>,
    /// The version of `views`, counted by every change sent.
    pub version: u32,
    /// Markdown: the note may hold link reference definitions, so an edit
    /// parses it whole.
    pub refs: bool,
}

impl FacadeModel {
    /// Numbers `changes` in the chain of versions.
    pub fn versioned(&mut self, changes: Vec<Splice<BlockView>>) -> Vec<Versioned> {
        changes
            .into_iter()
            .map(|splice| {
                let base = self.version;
                self.version = self.version.wrapping_add(1);
                (base, splice)
            })
            .collect()
    }
}

/// What one input did: the answer for the window, and what the host takes
/// from it (the edits for its highlighting and its autosave clock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handled<P> {
    /// The answer for the window. Its `unstored` is the host's to set.
    pub patch: P,
    /// The edits of the text and whether the cursor moved.
    pub outcome: Outcome,
    /// The captioned figures and tables changed, so those after them are
    /// numbered anew (the host makes its views anew).
    pub recaptioned: bool,
}

/// What a facade input needs of the host besides the input.
#[derive(Debug)]
pub struct FacadeCall<'a, M> {
    /// The note, by its id.
    pub note: &'a str,
    /// The selection the window shows.
    pub at: FacadeAt,
    /// Whether Cmd is the command key (macOS).
    pub mac: bool,
    /// Changes of the blocks made before the input (the facade brought up to
    /// date), sent first.
    pub earlier: Vec<Versioned>,
    /// What makes the views of the blocks the input changed.
    pub views: &'a mut M,
}

/// What one input did, gathered over the keys it went through.
#[derive(Debug, Default)]
struct Done {
    outcome: Outcome,
    commands: Vec<EditorCommand>,
    request: Option<ToolName>,
}

impl Done {
    fn take(&mut self, outcome: Outcome) {
        self.outcome.moved |= outcome.moved;
        self.outcome.changes.extend(outcome.changes);
    }
}

/// M-08.8: Ctrl/Cmd+Shift+[ folds (`Some(true)`), Ctrl/Cmd+Shift+]
/// expands (`Some(false)`).
#[must_use]
pub fn fold_key(key: KeyName, modifiers: Modifiers) -> Option<bool> {
    if !(modifiers.command && modifiers.shift) {
        return None;
    }
    match key {
        KeyName::BracketLeft => Some(true),
        KeyName::BracketRight => Some(false),
        _ => None,
    }
}

/// What `input` of the source view is to the Nano keys.
fn source_key_input(input: &EditorInput, mac: bool) -> Option<KeyInput<'_>> {
    match input {
        EditorInput::Text { text } => Some(KeyInput::Text(text)),
        EditorInput::Key {
            key,
            ctrl,
            alt,
            shift,
            meta,
            ..
        } => Some(KeyInput::Key {
            key: (*key).into(),
            pressed: true,
            modifiers: Modifiers::from_web(*ctrl, *alt, *shift, *meta, mac),
        }),
        EditorInput::Tool { .. } => None,
    }
}

/// What `input` of the facade is to the Vim and Nano keys.
fn facade_key_input(input: &FacadeInput, mac: bool) -> Option<KeyInput<'_>> {
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

/// An input the keys of the input mode left to the facade's own rules.
fn facade_plain_input(edit: &mut FacadeEdit<'_>, input: FacadeInput, mac: bool) {
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

/// FR-026: the status of the Vim or Nano keys `keys`; `None` with the usual
/// keys.
#[must_use]
pub fn keys_status(keys: &Keys) -> Option<KeysStatusView> {
    keys.status()
        .map(|status| KeysStatusView::of(&status, keys.prompting()))
}

/// FR-026: the status of the Nano keys of `keys`; `None` in the other
/// modes.
fn nano_status(keys: &Keys) -> Option<KeysStatusView> {
    (keys.mode == KeysInputMode::Nano)
        .then(|| keys.status())
        .flatten()
        .map(|status| KeysStatusView::of(&status, keys.prompting()))
}

/// M-09.11: the gap for the window.
#[must_use]
pub fn gap_view(gap: Gap) -> GapView {
    GapView {
        before: units(gap.before),
        lines: units(gap.lines),
    }
}

/// M-09.3: the format indicator.
#[must_use]
pub fn indicator(format: NoteFormat) -> &'static str {
    match format {
        NoteFormat::Markdown => "Markdown",
        NoteFormat::AsciiDoc => "AsciiDoc",
    }
}

/// `n` as an `i32` for a shift; a note is never that long.
fn signed(n: i64) -> i32 {
    i32::try_from(n).unwrap_or(if n < 0 { i32::MIN } else { i32::MAX })
}

/// The changes `changes` of the facade of `note`, whose blocks are of
/// revision `rev`, as the window takes them (`ed-blocks`).
#[must_use]
pub fn blocks_views(note: &str, rev: u64, changes: Vec<Versioned>) -> Vec<BlocksView> {
    changes
        .into_iter()
        .map(|(base, splice)| BlocksView {
            note: note.to_owned(),
            rev: revision(rev),
            base,
            version: base.wrapping_add(1),
            from: units(splice.from),
            removed: units(splice.removed),
            blocks: splice.blocks,
            shift: signed(splice.shift),
            line_shift: signed(splice.line_shift),
        })
        .collect()
}

/// One open tab as the window holds it (ADR-025).
#[derive(Debug)]
pub struct EditorTab {
    /// The note's format.
    pub format: NoteFormat,
    /// The text with its cursor and undo.
    pub editor: SourceEditor,
    /// The Vim and Nano keys of the view shown.
    pub keys: Keys,
    /// The facade's blocks and their views.
    pub facade: FacadeModel,
    /// M-09.14: what Underline last put on the clipboard from this tab,
    /// which pasting in the facade brings back as markup (`own_markup` of
    /// egui).
    pub own_markup: Option<String>,
    pub(crate) folds: Folds,
    /// The revision and the search of the hits the window was told of last.
    found_told: Option<(u64, Option<String>)>,
    /// When the hits were found last, in milliseconds of the host's clock.
    found_at: Option<f64>,
    /// The host's clock, in milliseconds ([`EditorTab::set_now`]).
    pub(crate) now_ms: f64,
}

impl EditorTab {
    /// A tab holding `text` in `format`, the cursor at its start.
    #[must_use]
    pub fn new(text: &str, format: NoteFormat) -> Self {
        Self {
            format,
            editor: SourceEditor::new(text),
            keys: Keys::default(),
            facade: FacadeModel::default(),
            own_markup: None,
            folds: Folds::default(),
            found_told: None,
            found_at: None,
            now_ms: 0.0,
        }
    }

    /// The host's clock is at `now_ms` milliseconds; the sections and hits
    /// are found again at most every 300 ms of it.
    ///
    /// One rule for the clock: every method that takes `now_ms` sets it
    /// through here first, and so does the host before an input. The clock
    /// has one epoch per tab and one host feeding it: a tab in the session
    /// gets the session's `millis`, a tab in the webview the window's
    /// `performance.now()`; no tab ever gets both. It only goes forward: an
    /// earlier time keeps the later one, and a time that is no finite number
    /// is ignored.
    pub fn set_now(&mut self, now_ms: f64) {
        if now_ms.is_finite() && now_ms > self.now_ms {
            self.now_ms = now_ms;
        }
    }

    /// UC-004 A7: shows `text` instead of the current text; the cursor stays
    /// at its line and column, the undo history starts anew, and the folded
    /// sections stay folded where they are now (M-08.8).
    pub fn reload(&mut self, text: &str) {
        self.editor.reload(text);
        self.refold();
    }

    /// ADR-025: the backend's changes, moved over the window's own edits
    /// (`Client::acknowledge`), applied as egui applies them: an agent's
    /// change as an undo step of its own, a reload with a fresh history and
    /// the cursor at its line and column. The patch for the source view
    /// (one for all of them), or `None` when nothing came.
    ///
    /// # Errors
    ///
    /// A change does not fit the text; nothing more is applied, and the
    /// copies are to be synced whole.
    pub fn apply_incoming(
        &mut self,
        incoming: Vec<Incoming>,
    ) -> Result<Option<EditorPatch>, OutOfRange> {
        if incoming.is_empty() {
            return Ok(None);
        }
        let base = self.editor.revision();
        let mut outcome = Outcome::default();
        for change in incoming {
            match change {
                Incoming::Edits { set, sealed } => {
                    let applied = apply_remote(&mut self.editor, &set, sealed)?;
                    self.refold_after(&applied);
                    outcome.changes.extend(applied.changes);
                }
                Incoming::Reload { text } => {
                    let doc = self.editor.document();
                    let old = self.editor.web_at_char(doc.len_chars());
                    // A reload replaces the whole text: it touches every line
                    // from the first.
                    let line = 0;
                    self.reload(&text);
                    let insert = self.editor.web_text();
                    outcome.changes.push(underline_editor_core::Applied {
                        from: 0,
                        to: old,
                        insert,
                        line,
                        added_lines: 0,
                        history: true,
                    });
                }
            }
        }
        let mut patch = self.patch(base, &outcome);
        // The view stays where it is (egui `source_view.rs:441`).
        patch.scroll = false;
        Ok(Some(patch))
    }

    /// The colours, the folds and the patch for the window after `outcome`,
    /// which started at revision `base`; `unstored` is the host's.
    fn after_input(&mut self, base: u64, outcome: &Outcome) -> EditorPatch {
        self.refold_after(outcome);
        self.unfold_at_cursor();
        self.patch(base, outcome)
    }

    /// The patch for the window after `outcome`, which started at `base`.
    fn patch(&self, base: u64, outcome: &Outcome) -> EditorPatch {
        let (head, anchor) = self.editor.selection_web();
        EditorPatch {
            base: revision(base),
            rev: revision(self.editor.revision()),
            changes: outcome
                .changes
                .iter()
                .map(|change| TextChange {
                    from: units(change.from),
                    to: units(change.to),
                    insert: change.insert.clone(),
                })
                .collect(),
            anchor: units(anchor),
            head: units(head),
            scroll: outcome.moved || outcome.edited(),
            unstored: false,
            place: self.editor.cursor_place().label(),
            keys: nano_status(&self.keys),
            commands: Vec::new(),
            request: None,
        }
    }

    /// The answer of an input that did `done`, which started at `base`.
    fn handled(&mut self, base: u64, done: Done) -> Handled<EditorPatch> {
        let mut patch = self.after_input(base, &done.outcome);
        patch.commands = done.commands.into_iter().map(Into::into).collect();
        patch.request = done.request;
        Handled {
            patch,
            outcome: done.outcome,
            recaptioned: false,
        }
    }

    /// UC-005 A2, BR-005: applies the toolbar function `tool` at the
    /// selection, in the markup of the note's format; Link and Image ask
    /// first (feature F6).
    fn apply_tool(&mut self, tool: Tool, done: &mut Done) {
        let Some(format) = tool.format() else {
            done.request = Some(tool.into());
            return;
        };
        let text = self.editor.text();
        let edit = apply_format(&text, self.format, self.editor.selection_bytes(), &format);
        done.take(
            self.editor
                .replace_bytes(edit.range, &edit.text, edit.selection),
        );
    }

    /// An input the keys of the input mode left to the usual keys: typed
    /// text, the folding keys, the toolbar keys, then the editor's keys
    /// (`key` of the egui widget).
    fn source_plain_input(&mut self, input: EditorInput, mac: bool, done: &mut Done) {
        match input {
            EditorInput::Text { text } => done.take(self.editor.type_text(&text)),
            EditorInput::Tool { tool } => self.apply_tool(tool.into(), done),
            EditorInput::Key {
                key,
                ctrl,
                alt,
                shift,
                meta,
                page,
            } => {
                let modifiers = Modifiers::from_web(ctrl, alt, shift, meta, mac);
                if let Some(fold) = fold_key(key, modifiers) {
                    done.outcome.moved |= self.fold_key(fold);
                } else if let Some(tool) = Tool::for_key(key.into(), modifiers) {
                    self.apply_tool(tool, done);
                } else {
                    done.take(self.editor.key(key.into(), modifiers, page as usize));
                }
            }
        }
    }

    /// ADR-024: the text with its cursor, as the window starts showing it or
    /// shows it again; it is told the sections and hits again.
    pub fn state(&mut self, note: &str) -> EditorState {
        self.folds.told = None;
        self.found_told = None;
        let (head, anchor) = self.editor.selection_web();
        EditorState {
            note: note.to_owned(),
            rev: revision(self.editor.revision()),
            text: self.editor.web_text(),
            anchor: units(anchor),
            head: units(head),
            place: self.editor.cursor_place().label(),
            keys: nano_status(&self.keys),
        }
    }

    /// M-11.4, FR-026: the left part of the status line of the source view
    /// in the input mode `setting` of the settings: the Nano keys, or the
    /// window's Vim keys in the state `vim` (A-01); `None` with the usual
    /// keys.
    #[must_use]
    pub fn source_keys_status(
        &self,
        setting: KeysInputMode,
        vim: Option<VimState>,
    ) -> Option<KeysStatusView> {
        match setting {
            KeysInputMode::Normal => None,
            KeysInputMode::Nano => nano_status(&self.keys),
            KeysInputMode::Vim => {
                let vim = vim.unwrap_or(VimState {
                    mode: VimModeName::Normal,
                    pending: String::new(),
                    marked: false,
                });
                let mode: KeysMode = vim.mode.into();
                Some(KeysStatusView::of(
                    &InputStatus::vim(mode, vim.pending, vim.marked),
                    false,
                ))
            }
        }
    }

    /// UC-005 step 1, M-08.4, M-08.5, M-10.1: one input of the source view,
    /// in the mode of `keys`: the Nano keys take it first, what they leave
    /// goes to the usual keys (FR-025, BR-010, BR-015).
    pub fn source_input(&mut self, input: EditorInput, mac: bool) -> Handled<EditorPatch> {
        let base = self.editor.revision();
        let mut done = Done::default();
        let nano = self.keys.mode == KeysInputMode::Nano;
        let through = if nano && !matches!(input, EditorInput::Tool { .. }) {
            let mut target = SourceKeys::new(&mut self.editor);
            let (through, _) =
                self.keys
                    .handle(vec![input], |i| source_key_input(i, mac), &mut target);
            done.take(std::mem::take(&mut target.outcome));
            done.commands = target.commands;
            through
        } else {
            vec![input]
        };
        for input in through {
            self.source_plain_input(input, mac, &mut done);
        }
        self.handled(base, done)
    }

    /// A-01, ADR-024: the window's Vim keys changed the text of revision
    /// `base` by `changes` (each relative to the text after the ones before
    /// it) and left the selection at `(anchor, head)`. Each change is one
    /// edit, undone by the usual rules (M-10.1), and one marked `new_step`
    /// is not undone with the one before; `new_step` marks the start of an
    /// insert session without a change of its own.
    ///
    /// `None` when the text is no longer of revision `base`, or when a
    /// change does not fit the text (beyond its end, backwards, inside a
    /// surrogate pair): nothing is applied then. The patch carries no
    /// changes, as the window made them already.
    pub fn source_apply(
        &mut self,
        base: u32,
        changes: &[VimChange],
        (anchor, head): (u32, u32),
        new_step: bool,
    ) -> Option<Handled<EditorPatch>> {
        let start = self.editor.revision();
        if revision(start) != base {
            return None;
        }
        let checked: Vec<(usize, usize, &str)> = changes
            .iter()
            .map(|c| (c.from as usize, c.to as usize, c.insert.as_str()))
            .collect();
        if !self.editor.web_changes_fit(&checked) {
            return None;
        }
        if new_step {
            self.editor.seal();
        }
        let mut done = Done::default();
        for change in changes {
            if change.new_step {
                self.editor.seal();
            }
            done.take(self.editor.apply_web(
                change.from as usize,
                change.to as usize,
                &change.insert,
                None,
            ));
        }
        done.outcome.moved |= self
            .editor
            .set_selection_web(head as usize, anchor as usize);
        let mut handled = self.handled(start, done);
        handled.patch.changes.clear();
        Some(handled)
    }

    /// M-08.8: the arrow of the section whose heading is `line` (from 0)
    /// folds or expands it; a cursor in the hidden part moves to the
    /// heading.
    pub fn fold_line(&mut self, line: u32) -> Handled<EditorPatch> {
        let base = self.editor.revision();
        let moved = self.toggle_at_line(line as usize);
        let outcome = Outcome {
            changes: Vec::new(),
            moved,
        };
        Handled {
            patch: self.after_input(base, &outcome),
            outcome,
            recaptioned: false,
        }
    }

    /// M-08.6, ADR-014: copying gives the selected markup for the clipboard,
    /// `None` with nothing selected.
    #[must_use]
    pub fn copy(&self) -> (Option<String>, EditorPatch) {
        let text = Some(self.editor.selected_text()).filter(|t| !t.is_empty());
        let base = self.editor.revision();
        (text, self.patch(base, &Outcome::default()))
    }

    /// M-08.6: cutting gives the selected markup for the clipboard and takes
    /// it out of the text.
    pub fn cut(&mut self) -> (Option<String>, Handled<EditorPatch>) {
        let base = self.editor.revision();
        let (text, outcome) = self.editor.cut();
        let done = Done {
            outcome,
            ..Done::default()
        };
        (
            Some(text).filter(|t| !t.is_empty()),
            self.handled(base, done),
        )
    }

    /// M-08.6: pasting inserts `text`, the plain text of the clipboard;
    /// nothing without one. A paste ends a run of Nano cuts, as in egui
    /// (BR-010), and the open search prompt takes it (M-11.3).
    pub fn paste(&mut self, text: Option<&str>) -> Handled<EditorPatch> {
        let (through, _) = self.keys.handle(
            vec![()],
            |()| Some(KeyInput::Paste),
            &mut SourceKeys::new(&mut self.editor),
        );
        let base = self.editor.revision();
        let outcome = text
            .filter(|_| !through.is_empty())
            .map(|text| self.editor.type_text(text))
            .unwrap_or_default();
        let done = Done {
            outcome,
            ..Done::default()
        };
        self.handled(base, done)
    }

    /// M-08.3: the mouse (or the window's Vim keys) put the cursor and the
    /// selection at these web offsets of the revision `rev`. A selection
    /// made in an older revision is dropped, as its offsets no longer fit.
    /// A cursor in a folded section expands it (M-08.8). Gives the place for
    /// the status line (M-11.4).
    pub fn select(&mut self, rev: u32, anchor: u32, head: u32) -> String {
        if revision(self.editor.revision()) == rev {
            self.editor
                .set_selection_web(head as usize, anchor as usize);
            self.unfold_at_cursor();
        }
        self.editor.cursor_place().label()
    }

    /// M-12.2, UC-008 BR-008: the marked hits of the Nano search at
    /// `now_ms`, once they changed; while typing goes on with the same
    /// search, at most every [`FOUND_EVERY_MS`] (ADR-024). `None` while the
    /// window has them.
    pub fn found_view(&mut self, note: &str, now_ms: f64) -> Option<FoundView> {
        self.set_now(now_ms);
        let now_ms = self.now_ms;
        let rev = self.editor.revision();
        let search = self.keys.search().map(str::to_owned);
        let told = self.found_told.clone();
        let key = (rev, search);
        if told.as_ref() == Some(&key) {
            return None;
        }
        let same_search = told.as_ref().is_some_and(|(_, p)| *p == key.1);
        let recent = self
            .found_at
            .is_some_and(|at| (now_ms - at).max(0.0) < FOUND_EVERY_MS);
        if same_search && recent && key.1.is_some() {
            return None;
        }
        let all = key
            .1
            .as_deref()
            .map(|pattern| self.editor.find_all(pattern))
            .unwrap_or_default();
        let count = units(all.len());
        let ranges = all
            .into_iter()
            .take(FoundView::MAX)
            .map(|range| {
                let web = self.editor.to_web_range(range);
                (units(web.start), units(web.end))
            })
            .collect();
        // Nothing to tell when no hits were marked and none are.
        let was_marked = told.as_ref().is_some_and(|(_, p)| p.is_some());
        let view = (key.1.is_some() || was_marked).then(|| FoundView {
            note: note.to_owned(),
            rev: revision(rev),
            ranges,
            count,
        });
        self.found_told = Some(key);
        self.found_at = Some(now_ms);
        view
    }

    /// M-09: parses the blocks of the text and makes all their views anew
    /// with `views`, when they are not of the text's revision; the changes
    /// for the window (none when they were).
    pub fn bring_facade_up_to_date<M: MakeViews<View = BlockView>>(
        &mut self,
        views: &mut M,
    ) -> Vec<Versioned> {
        let rev = self.editor.revision();
        if self.facade.parsed == Some(rev) {
            return Vec::new();
        }
        let text = self.editor.text();
        let (blocks, refs) = parse_all(&text, self.format);
        let made = views.all(&blocks, &text, self.editor.document().rope());
        let changes = splices(&self.facade.views, &made);
        self.facade.views = made;
        self.facade.blocks = Arc::new(blocks);
        self.facade.parsed = Some(rev);
        self.facade.refs = refs;
        self.facade.versioned(changes)
    }

    /// ADR-025: the facade after the text changed without an input of the
    /// facade (a change of the backend): its blocks made anew where needed,
    /// with the cursor as it is now; the view does not scroll.
    pub fn facade_refresh<M: MakeViews<View = BlockView>>(
        &mut self,
        note: &str,
        views: &mut M,
    ) -> FacadePatch {
        let base = self.editor.revision();
        let changes = self.bring_facade_up_to_date(views);
        let rev = self.editor.revision();
        let source = self.patch(base, &Outcome::default());
        FacadePatch {
            base: revision(base),
            rev: revision(rev),
            blocks: blocks_views(note, rev, changes),
            anchor: source.anchor,
            head: source.head,
            gap: gap_at(
                self.editor.document().rope(),
                self.editor.cursor(),
                &self.facade.blocks,
            )
            .map(gap_view),
            scroll: false,
            unstored: false,
            place: source.place,
            keys: keys_status(&self.keys),
            commands: Vec::new(),
            request: None,
            copied: None,
            wants_clipboard: self.keys.wants_clipboard(),
        }
    }

    /// ADR-025: the facade's views take what the backend sent of colours and
    /// figures (`ed-enrich`); the changes for the window, each block in
    /// place. None while the blocks are not of the text (they take it when
    /// they are made).
    pub fn facade_enriched(&mut self, note: &str, views: &mut CachedViews) -> Vec<BlocksView> {
        let rev = self.editor.revision();
        if self.facade.parsed != Some(rev) {
            return Vec::new();
        }
        let changed = views.enrich(&self.facade.blocks, &mut self.facade.views);
        let splices = changed
            .into_iter()
            .map(|index| Splice {
                from: index,
                removed: 1,
                blocks: vec![self.facade.views[index].clone()],
                shift: 0,
                line_shift: 0,
            })
            .collect();
        let changes = self.facade.versioned(splices);
        blocks_views(note, rev, changes)
    }

    /// M-11.4, FR-026: the left part of the status line while the facade
    /// shows the note.
    #[must_use]
    pub fn facade_keys_status(&self) -> Option<KeysStatusView> {
        keys_status(&self.keys)
    }

    /// The whole facade of `note` for the window, with the cursor.
    #[must_use]
    pub fn facade_state(&self, note: &str) -> FacadeState {
        let mut state = self.facade_head(note);
        state.blocks.clone_from(&self.facade.views);
        state
    }

    /// ADR-025: the facade of `note` without its blocks: the window still
    /// holds the blocks of this `version` and takes them again.
    #[must_use]
    pub fn facade_head(&self, note: &str) -> FacadeState {
        let (head, anchor) = self.editor.selection_web();
        let facade = &self.facade;
        FacadeState {
            note: note.to_owned(),
            rev: revision(facade.parsed.unwrap_or_else(|| self.editor.revision())),
            version: facade.version,
            format: indicator(self.format).to_owned(),
            blocks: Vec::new(),
            anchor: units(anchor),
            head: units(head),
            place: self.editor.cursor_place().label(),
            gap: (facade.parsed == Some(self.editor.revision()))
                .then(|| {
                    gap_at(
                        self.editor.document().rope(),
                        self.editor.cursor(),
                        &facade.blocks,
                    )
                })
                .flatten()
                .map(gap_view),
            keys: keys_status(&self.keys),
        }
    }

    /// Runs `work` on the facade's text and blocks, gathering into
    /// `gathered` (ADR-025: the edit borrows the one [`Gathered`]).
    fn with_edit<R>(
        &mut self,
        gathered: &mut Gathered,
        (rows, clipboard): (Option<Rows>, Option<&str>),
        work: impl FnOnce(&mut FacadeEdit<'_>, &mut Keys) -> R,
    ) -> R {
        let blocks = Arc::make_mut(&mut self.facade.blocks);
        let mut edit = FacadeEdit::new(
            &mut self.editor,
            blocks,
            self.format,
            &mut self.facade.refs,
            gathered,
        )
        .with_rows(rows)
        .with_clipboard(clipboard);
        work(&mut edit, &mut self.keys)
    }

    /// ADR-024: the selection `at` the window showed is taken in only when
    /// `at.rev` is the revision of the text. A stale `at.rev` (the window has
    /// not taken in the patch of an earlier input yet) drops it: its web
    /// units would point into another text, so the input applies at the
    /// tab's own cursor, where the earlier input left it.
    fn take_selection(&mut self, at: FacadeAt) {
        if revision(self.editor.revision()) == at.rev {
            self.editor
                .set_selection_web(at.head as usize, at.anchor as usize);
        }
    }

    /// UC-005, M-09.10 to M-09.13, M-11.2, M-11.3, M-10.1: one input of the
    /// facade at the selection `call.at`, whose blocks are of the text's
    /// revision. `rows` are the rows the window measured for the cursor
    /// (M-09.13), `clipboard` the system clipboard's text when the Vim keys
    /// asked for it. Only the edit, the blocks around it and the patch are
    /// worked out (ADR-024).
    pub fn facade_input<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        input: FacadeInput,
        rows: Option<RowsView>,
        clipboard: Option<&str>,
    ) -> Handled<FacadePatch> {
        self.take_selection(call.at);
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let rows = rows.map(|rows| Rows {
            at: self.editor.char_at_web(rows.at as usize),
            up: rows.up.map(|up| self.editor.char_at_web(up as usize)),
            down: rows.down.map(|down| self.editor.char_at_web(down as usize)),
            start: self.editor.char_at_web(rows.start as usize),
            end: self.editor.char_at_web(rows.end as usize),
        });
        let mac = call.mac;
        let mut gathered = Gathered::default();
        // FR-024, FR-025: the Vim or Nano keys take the input first; what
        // they leave goes to the facade's rules right after.
        let through = if self.keys.mode != KeysInputMode::Normal
            && !matches!(input, FacadeInput::Tool { .. })
        {
            self.with_edit(&mut gathered, (rows, clipboard), |edit, keys| {
                let mut target = FacadeTarget { edit };
                keys.handle(vec![input], |i| facade_key_input(i, mac), &mut target)
                    .0
            })
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
                    let moved = self.fold_key(fold);
                    gathered.moved(moved);
                    continue;
                }
            }
            self.with_edit(&mut gathered, (rows, clipboard), |edit, _| {
                facade_plain_input(edit, input, mac);
            });
        }
        self.facade_finish(call, before, gathered)
    }

    /// M-09.14, M-08.6: copy, cut or paste in the facade at the selection
    /// `call.at`. Copying and cutting give the selected markup for the
    /// clipboard (the first of the answer), which pasting it brings back as
    /// markup; any other text is pasted as typed, escaped, and so is
    /// everything with `plain` (Ctrl+Shift+V). `text` is the clipboard's
    /// text.
    pub fn facade_clipboard<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        action: ClipAction,
        plain: bool,
        text: Option<&str>,
    ) -> (Option<String>, Handled<FacadePatch>) {
        self.take_selection(call.at);
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let mut gathered = Gathered::default();
        let mut copied = None;
        match action {
            ClipAction::Copy | ClipAction::Cut => {
                let selected = self.editor.selected_text();
                if !selected.is_empty() {
                    self.own_markup = Some(selected.clone());
                    if action == ClipAction::Cut {
                        let range = self.editor.selection();
                        self.with_edit(&mut gathered, (None, None), |edit, _| {
                            edit.replace(range, "");
                        });
                    }
                    copied = Some(selected);
                }
            }
            ClipAction::Paste => {
                let own = self.own_markup.clone();
                self.with_edit(&mut gathered, (None, None), |edit, keys| {
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
                });
            }
        }
        (copied, self.facade_finish(call, before, gathered))
    }

    /// The patch after a facade input that started at `before`, with the
    /// changes of the blocks `call.earlier` made before it.
    fn facade_finish<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        before: Before,
        gathered: Gathered,
    ) -> Handled<FacadePatch> {
        let FacadeCall {
            note,
            earlier: mut changes,
            views,
            ..
        } = call;
        let rev = self.editor.revision();
        self.facade.parsed = Some(rev);
        let mut recaptioned = false;
        if let Some(dirty) = gathered.dirty.range() {
            let (refreshed, again) = self.refresh_views(views, dirty, before);
            changes.extend(refreshed);
            recaptioned = again;
        }
        if let Some(copied) = &gathered.copied {
            self.own_markup = Some(copied.clone());
        }
        let source = self.after_input(before.rev, &gathered.outcome);
        let gap = gap_at(
            self.editor.document().rope(),
            self.editor.cursor(),
            &self.facade.blocks,
        )
        .map(gap_view);
        let patch = FacadePatch {
            base: revision(before.rev),
            rev: revision(rev),
            blocks: blocks_views(note, rev, changes),
            anchor: source.anchor,
            head: source.head,
            gap,
            scroll: source.scroll,
            unstored: false,
            place: source.place,
            keys: keys_status(&self.keys),
            commands: gathered.commands.into_iter().map(Into::into).collect(),
            request: gathered.request.map(Into::into),
            copied: gathered.copied,
            wants_clipboard: self.keys.wants_clipboard(),
        };
        Handled {
            patch,
            outcome: gathered.outcome,
            recaptioned,
        }
    }

    /// ADR-024: the views of the blocks `dirty` made anew after an input,
    /// the others moved (the editor core's [`refresh`]); the changes for the
    /// window, and whether the captions are to be numbered anew.
    fn refresh_views<M: MakeViews<View = BlockView>>(
        &mut self,
        views: &mut M,
        dirty: Range<usize>,
        before: Before,
    ) -> (Vec<Versioned>, bool) {
        let refreshed = refresh(
            &mut self.facade.views,
            &self.facade.blocks,
            &self.editor,
            views,
            dirty,
            before,
        );
        (
            self.facade.versioned(refreshed.splices),
            refreshed.recaptioned,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-005 step 1, ADR-025: a typed character is one change of the
    /// window's text, with the cursor after it; whether the note is stored
    /// is the host's to say.
    #[test]
    fn uc005_typing_answers_with_the_change_and_the_cursor() {
        let mut tab = EditorTab::new("ab\n", NoteFormat::Markdown);
        tab.editor.set_selection(1, 1);

        let handled = tab.source_input(
            EditorInput::Text {
                text: "😀".into()
            },
            false,
        );

        assert_eq!(
            handled.patch.changes,
            [TextChange {
                from: 1,
                to: 1,
                insert: "😀".into()
            }]
        );
        assert_eq!((handled.patch.anchor, handled.patch.head), (3, 3));
        assert_eq!(handled.patch.base + 1, handled.patch.rev);
        assert!(!handled.patch.unstored);
        assert!(handled.outcome.edited());
        assert_eq!(tab.editor.text(), "a😀b\n");
    }

    /// A-01: changes of the window's Vim keys made on another revision are
    /// not applied.
    #[test]
    fn a01_vim_changes_of_another_revision_are_refused() {
        let mut tab = EditorTab::new("ab\n", NoteFormat::Markdown);
        let base = revision(tab.editor.revision());
        let change = VimChange {
            from: 0,
            to: 1,
            insert: "x".into(),
            new_step: true,
        };

        assert!(tab
            .source_apply(base + 1, std::slice::from_ref(&change), (1, 1), false)
            .is_none());
        let handled = tab.source_apply(base, &[change], (1, 1), false).unwrap();
        assert!(handled.patch.changes.is_empty(), "the window made them");
        assert_eq!(tab.editor.text(), "xb\n");
    }

    /// ADR-025: the clock only goes forward, and what is no finite time is
    /// ignored.
    #[test]
    fn adr025_the_clock_goes_only_forward() {
        let mut tab = EditorTab::new("", NoteFormat::Markdown);

        tab.set_now(500.0);
        tab.set_now(100.0);
        tab.set_now(f64::NAN);
        tab.set_now(f64::INFINITY);

        assert!((tab.now_ms - 500.0).abs() < f64::EPSILON);
    }

    /// ADR-025 (review B5b): after a change of the backend the facade takes
    /// only the blocks that changed, in the chain of versions, without
    /// scrolling; nothing more when nothing changed again.
    #[test]
    fn adr025_the_facade_after_a_change_of_the_backend() {
        let mut tab = EditorTab::new("# A\n\nOne\n\nTwo\n", NoteFormat::Markdown);
        let mut views = crate::CachedViews::new("Light", false);
        tab.bring_facade_up_to_date(&mut views);
        let before = tab.facade_state("A");
        let at = "# A\n\nOne".len();
        let _ = tab.editor.replace(at..at, "!");

        let patch = tab.facade_refresh("A", &mut views);

        assert!(!patch.scroll);
        assert_eq!(patch.blocks.len(), 1);
        let change = &patch.blocks[0];
        assert_eq!(change.base, before.version);
        assert_eq!((change.from, change.removed), (1, 1));
        assert_eq!(change.blocks[0].runs[0].text, "One!");
        assert!(tab.facade_refresh("A", &mut views).blocks.is_empty());
    }
}
