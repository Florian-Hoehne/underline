//! UC-005 Edit Note in the session: the text of every open tab lives here
//! (ADR-024), and the window's source view sends each input as one command
//! and gets a patch back. Around it: the autosave (M-10.8), storing at once
//! (M-10.9), closing a tab or Underline after storing (M-06.3, M-21.4), the
//! highlighting sent after the input (M-08.2) and the properties of the
//! tool panel (M-16.2). Feature F4 adds the Nano keys (M-11.3), the
//! formatting toolbar (M-10.2), folding (M-08.8), the status line's place
//! and keys (M-11.4), the edits of the window's Vim keys (A-01) and the
//! marked hits of the Nano search (UC-008 BR-008).
//!
//! What answers an input is the editor's tab (`underline_editor_view::
//! EditorTab`, ADR-025), which the window runs itself with variant B; the
//! session adds what only it knows: the keys of the settings, the autosave
//! clock, the highlighting and the tab marks.
//!
//! Ported from `apps/underline-desktop/src/app.rs`: `flush_tab_now`,
//! `flush_all`, `close_tab`, `autosave`, `key_command` and the close
//! confirmation, and from `crates/underline-editor/src/source_view.rs` the
//! clipboard keys of the source view.

use std::ops::Range;
use std::sync::{mpsc, Arc, OnceLock};
use std::time::{Duration, Instant};

use underline_core::{InputMode, NoteId, SaveJob, SaveResult, Theme};
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_editor_core::{utf16, Outcome};
use underline_editor_view::Handled;
use underline_facade::highlight::{DocumentHighlight, HighlightSet, Span};

use super::tab::Tab;
use super::workspace::OpenWorkspace;
use super::Session;
use crate::editor_view::{
    revision, units, CloseQuestion, EditorInput, EditorPatch, EditorState, HighlightView,
    KeysStatusView, SpanStyle, TabPropertiesView, VimChange, VimState,
};
use crate::{Event, Platform, UiError, UiErrorKind};

/// How long one tick may spend on colouring lines (ADR-010); the egui app
/// spends 4 ms a frame. Lines not done yet are done in the next tick, which
/// follows at once.
const COLORING_BUDGET: Duration = Duration::from_millis(4);

/// How long storing at once waits for a background save that runs; the
/// saver writes one note at a time, atomically, so this is far more than a
/// save takes.
const SETTLE_AT_MOST: Duration = Duration::from_secs(5);

/// The highlighting set for one theme (ADR-010), shared by the tabs.
pub(super) struct Colors {
    theme: Theme,
    set: Arc<HighlightSet>,
}

/// ADR-025: the editor's clock: milliseconds of `at` since a moment well
/// before the session began (the editor runs in the webview too, which has
/// no `Instant`).
pub(super) fn millis(at: Instant) -> f64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = *EPOCH.get_or_init(|| {
        let now = Instant::now();
        now.checked_sub(Duration::from_secs(3600)).unwrap_or(now)
    });
    at.saturating_duration_since(epoch).as_secs_f64() * 1000.0
}

/// The tab of `note` in the open workspace, its editor's clock at now.
fn tab_in<'a>(open: &'a mut Option<OpenWorkspace>, note: &NoteId) -> Result<&'a mut Tab, UiError> {
    let open = open.as_mut().ok_or_else(|| not_open(note))?;
    let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
    let tab = open.tabs.get_mut(index);
    tab.edit.set_now(millis(Instant::now()));
    Ok(tab)
}

/// The colours, the autosave clock and the tab mark follow an input the
/// editor handled (`outcome`); the patch's `unstored` is set, and whether
/// the tab's mark changed is returned.
pub(super) fn after_input(tab: &mut Tab, outcome: &Outcome, unstored: &mut bool) -> bool {
    let mut marked = false;
    // ADR-025: the mirror takes the edits, and the highlighting follows it.
    let edited = tab.sync_local();
    if outcome.edited() || edited {
        marked = tab.changed(Instant::now());
    }
    *unstored = tab.autosave.is_unstored();
    marked
}

/// The patch of `handled` for the window, after the session took it in.
fn took(tab: &mut Tab, handled: Handled<EditorPatch>, marked: &mut bool) -> EditorPatch {
    let mut patch = handled.patch;
    *marked |= after_input(tab, &handled.outcome, &mut patch.unstored);
    patch
}

fn not_open(note: &NoteId) -> UiError {
    UiError::new(
        UiErrorKind::Invalid,
        format!("The tab {} is not open", note.title()),
    )
}

/// The text of `line` of `rope` with its line ending; empty past the end.
fn rope_line(rope: &ropey::Rope, line: usize) -> String {
    if line < rope.len_lines() {
        rope.line(line).to_string()
    } else {
        String::new()
    }
}

/// `#rrggbb` of an RGBA colour.
fn hex([r, g, b, _]: [u8; 4]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The keys of the input mode `mode` of the settings, for the editor core.
pub(super) fn keys_mode(mode: InputMode) -> KeysInputMode {
    match mode {
        InputMode::Normal => KeysInputMode::Normal,
        InputMode::Vim => KeysInputMode::Vim,
        InputMode::Nano => KeysInputMode::Nano,
    }
}

impl Session {
    fn tab_mut(&mut self, note: &NoteId) -> Result<&mut Tab, UiError> {
        tab_in(&mut self.open, note)
    }

    /// The keys of the source view in the session: the Nano keys in the
    /// Nano mode, else the usual keys; the Vim keys of the source view are
    /// the window's (A-01), which send what they leave as usual inputs.
    fn source_keys_mode(&self) -> KeysInputMode {
        match self.settings.effective().input_mode {
            InputMode::Nano => KeysInputMode::Nano,
            InputMode::Normal | InputMode::Vim => KeysInputMode::Normal,
        }
    }

    /// ADR-024: the text of the tab of `note`, with its cursor, as the
    /// window starts showing it or shows it again.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_state(&mut self, note: &NoteId) -> Result<EditorState, UiError> {
        let mode = self.source_keys_mode();
        let tab = self.tab_mut(note)?;
        tab.edit.keys.mode = mode;
        Ok(tab.edit.state(note.as_str()))
    }

    /// M-11.4, FR-026: the left part of the status line in the input mode
    /// of the settings: the Nano keys of the tab of `note`, or the window's
    /// Vim keys in the state `vim` (A-01); `None` with the usual keys.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_keys_status(
        &mut self,
        note: &NoteId,
        vim: Option<VimState>,
    ) -> Result<Option<KeysStatusView>, UiError> {
        let setting = keys_mode(self.settings.effective().input_mode);
        let mode = self.source_keys_mode();
        let tab = self.tab_mut(note)?;
        tab.edit.keys.mode = mode;
        Ok(tab.edit.source_keys_status(setting, vim))
    }

    /// UC-005 step 1, M-08.4, M-08.5, M-10.1: one input of the source view
    /// in the Normal input mode. Only the edit and the patch are worked out
    /// here; the colours follow with the next tick (ADR-024).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_input(
        &mut self,
        note: &NoteId,
        input: EditorInput,
    ) -> Result<EditorPatch, UiError> {
        let mac = Platform::current() == Platform::MacOs;
        let mode = self.source_keys_mode();
        let tab = self.tab_mut(note)?;
        tab.edit.keys.mode = mode;
        let handled = tab.edit.source_input(input, mac);
        let mut marked = false;
        let patch = took(tab, handled, &mut marked);
        // M-06.1: the tab is marked " •" now.
        self.views_dirty |= marked;
        Ok(patch)
    }

    /// A-01, ADR-024: the window's Vim keys changed the text of revision
    /// `base` by `changes` (each relative to the text after the ones before
    /// it) and left the cursor at `head`, the anchor at `anchor`
    /// ([`EditorTab::source_apply`](underline_editor_view::EditorTab::source_apply)).
    ///
    /// `None` when the text is no longer of revision `base`, or when a
    /// change does not fit the text: nothing is applied then, and the window
    /// shows the text again (`ed_open`).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_apply(
        &mut self,
        note: &NoteId,
        base: u32,
        changes: &[VimChange],
        selection: (u32, u32),
        new_step: bool,
    ) -> Result<Option<EditorPatch>, UiError> {
        let tab = self.tab_mut(note)?;
        let Some(handled) = tab.edit.source_apply(base, changes, selection, new_step) else {
            return Ok(None);
        };
        let mut marked = false;
        let patch = took(tab, handled, &mut marked);
        self.views_dirty |= marked;
        Ok(Some(patch))
    }

    /// M-08.8: the arrow of the section whose heading is `line` (from 0)
    /// folds or expands it; a cursor in the hidden part moves to the
    /// heading.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_fold(&mut self, note: &NoteId, line: u32) -> Result<EditorPatch, UiError> {
        let tab = self.tab_mut(note)?;
        let handled = tab.edit.fold_line(line);
        let mut marked = false;
        Ok(took(tab, handled, &mut marked))
    }

    /// M-08.6, ADR-014: copying gives the selected markup for the clipboard,
    /// `None` with nothing selected. The window's backend writes it to the
    /// clipboard after the session is let go, so the clipboard never holds
    /// the session up.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_copy(&mut self, note: &NoteId) -> Result<(Option<String>, EditorPatch), UiError> {
        let tab = self.tab_mut(note)?;
        let (text, mut patch) = tab.edit.copy();
        patch.unstored = tab.autosave.is_unstored();
        Ok((text, patch))
    }

    /// M-08.6: cutting gives the selected markup for the clipboard and takes
    /// it out of the text.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_cut(&mut self, note: &NoteId) -> Result<(Option<String>, EditorPatch), UiError> {
        let tab = self.tab_mut(note)?;
        let (text, handled) = tab.edit.cut();
        let mut marked = false;
        let patch = took(tab, handled, &mut marked);
        self.views_dirty |= marked;
        Ok((text, patch))
    }

    /// M-08.6: pasting inserts `text`, the plain text the window's backend
    /// read from the clipboard before it took the session; nothing without
    /// one. An image on the clipboard comes with feature F6 (M-10.6).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_paste(
        &mut self,
        note: &NoteId,
        text: Option<&str>,
    ) -> Result<EditorPatch, UiError> {
        let mode = self.source_keys_mode();
        let tab = self.tab_mut(note)?;
        tab.edit.keys.mode = mode;
        let handled = tab.edit.paste(text);
        let mut marked = false;
        let patch = took(tab, handled, &mut marked);
        self.views_dirty |= marked;
        Ok(patch)
    }

    /// M-08.3: the mouse (or the window's Vim keys) put the cursor and the
    /// selection at these UTF-16 offsets of the revision `rev`. A selection
    /// made in an older revision is dropped, as its offsets no longer fit.
    /// A cursor in a folded section expands it (M-08.8). Gives the place
    /// for the status line (M-11.4).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_select(
        &mut self,
        note: &NoteId,
        rev: u32,
        anchor: u32,
        head: u32,
    ) -> Result<String, UiError> {
        let tab = self.tab_mut(note)?;
        Ok(tab.edit.select(rev, anchor, head))
    }

    /// M-08.1, M-08.2: the source view shows the lines `from..to` of the
    /// note; their colours are sent with the next tick, also when the
    /// window had them already, as after it showed the text anew.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_viewport(&mut self, note: &NoteId, from: u32, to: u32) -> Result<(), UiError> {
        let tab = self.tab_mut(note)?;
        tab.viewport = Some(from as usize..(to as usize).max(from as usize + 1));
        tab.highlight_told = None;
        Ok(())
    }

    /// M-10.9, UC-005 BR-009, BR-010: `Store` stores the note at once and
    /// says so: "Stored {t}" or "{t} could not be stored: {err}".
    pub fn store_tab(&mut self, note: &NoteId) {
        self.settle_saves();
        let Some(open) = &mut self.open else {
            return;
        };
        let Some(index) = open.tabs.find(note) else {
            return;
        };
        let root = open.workspace.root().to_owned();
        let tab = open.tabs.get_mut(index);
        tab.autosave.changed(Instant::now());
        self.status = match tab.flush(&self.saver) {
            Ok(()) => format!("Stored {}", tab.title),
            Err(err) => format!("{} could not be stored: {err}", tab.title),
        };
        tab.read_properties(&root);
        self.views_dirty = true;
    }

    /// M-06.3, UC-004 A5: closes the tab of `note`, storing its changes
    /// first. When they cannot be stored, the tab stays and the user is
    /// asked (UC-005 A14 step 3); with `force` ("Close anyway") it closes
    /// all the same.
    pub fn close_tab(&mut self, note: &NoteId, force: bool) -> Option<CloseQuestion> {
        self.settle_saves();
        let open = self.open.as_mut()?;
        let index = open.tabs.find(note)?;
        let flushed = open.tabs.get_mut(index).flush(&self.saver);
        self.views_dirty = true;
        if let (Err(error), false) = (flushed, force) {
            return Some(CloseQuestion {
                tab: Some(note.as_str().to_owned()),
                what: format!("the tab {}", note.title()),
                error,
            });
        }
        if let Some(open) = &mut self.open {
            open.tabs.close(index);
        }
        self.persist_tabs();
        None
    }

    /// M-21.4, UC-005 BR-001: Underline is to close; every change is stored
    /// first. When that fails, the window stays open and the user is asked
    /// (A14 step 3). The sync before closing comes with feature F13.
    pub fn request_close(&mut self) -> Option<CloseQuestion> {
        let failures = self.flush_all().err()?;
        Some(CloseQuestion {
            tab: None,
            what: "Underline".to_owned(),
            error: failures.join("; "),
        })
    }

    /// M-21.4, M-22.1: Underline ends: what can be stored is stored, then
    /// the app state is written. The window presence is removed with
    /// feature F14.
    ///
    /// # Errors
    ///
    /// What could not be stored or written, one text each, for the log.
    pub fn end(&mut self) -> Result<(), Vec<String>> {
        let mut problems = self.flush_all().err().unwrap_or_default();
        if let Err(err) = self.write_state_now() {
            problems.push(err.to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// UC-005 BR-001: stores every tab's changes now; returns the failures,
    /// each naming its tab.
    pub(super) fn flush_all(&mut self) -> Result<(), Vec<String>> {
        self.settle_saves();
        let Some(open) = &mut self.open else {
            return Ok(());
        };
        let root = open.workspace.root().to_owned();
        let mut failures = Vec::new();
        for (_, tab) in open.tabs.iter_mut() {
            let unstored = tab.autosave.is_unstored();
            if let Err(failure) = tab.flush(&self.saver) {
                failures.push(failure);
            }
            if unstored {
                tab.read_properties(&root);
                self.views_dirty = true;
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }

    /// UC-005 step 4, M-10.8: stores the due tabs in the background and
    /// takes in the outcomes (FR-021, A14, A15).
    pub(super) fn autosave(&mut self, now: Instant) {
        let Some(open) = &mut self.open else {
            return;
        };
        for (_, tab) in open.tabs.iter_mut() {
            if !tab.saving && tab.autosave.is_due(now) {
                let text = tab.mirror.text();
                tab.saving_text = Some(text.clone());
                self.saver.save(SaveJob {
                    path: tab.path.clone(),
                    text,
                    started: now,
                });
                tab.saving = true;
            }
        }
        let results = self.saver.poll();
        self.take_save_results(results);
    }

    /// Takes in the outcomes of background saves (UC-005 A14, A15).
    fn take_save_results(&mut self, results: Vec<SaveResult>) {
        let Some(open) = &mut self.open else {
            return;
        };
        let root = open.workspace.root().to_owned();
        for result in results {
            let Some((_, tab)) = open.tabs.iter_mut().find(|(_, t)| t.path == result.path) else {
                continue;
            };
            tab.saving = false;
            self.views_dirty = true;
            match result.result {
                Ok(()) => {
                    tab.autosave.stored(result.started);
                    tab.error = None;
                    if let Some(text) = tab.saving_text.take() {
                        tab.on_disk = text;
                    }
                    tab.read_properties(&root);
                    // F8: a save of links a rename changed keeps the
                    // rename's report (UC-006 A7, `quiet_save`).
                    if std::mem::take(&mut tab.deleted) {
                        // UC-005 A15: a note deleted outside is stored again.
                        self.status = format!("Stored {} again at its place", tab.title);
                    } else {
                        self.status = format!("Saved {}", tab.title);
                        // F12: the git changes are asked again here
                        // (`refresh_changes`).
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
    }

    /// Waits until no background save runs any more, at most
    /// [`SETTLE_AT_MOST`], so a save of an older text never lands on disk
    /// after the newer one stored now (UC-005 BR-001).
    pub(super) fn settle_saves(&mut self) {
        let start = Instant::now();
        while self.saving() && start.elapsed() < SETTLE_AT_MOST {
            let results = self.saver.poll();
            if results.is_empty() {
                std::thread::sleep(Duration::from_millis(1));
            } else {
                self.take_save_results(results);
            }
        }
    }

    /// Whether a background save of a tab runs.
    fn saving(&self) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.tabs.iter().any(|(_, tab)| tab.saving))
    }

    /// M-16.2: the properties section for the active tab, if a note tab is
    /// active.
    pub(super) fn properties_view(&self) -> Option<TabPropertiesView> {
        let open = self.open.as_ref()?;
        let (note, tab) = open.tabs.active()?;
        Some(tab.properties_view(note.as_str()))
    }

    /// UC-010 BR-004: starts making the colours of the theme in effect, so
    /// they are ready when the first note shows. They are made on a thread
    /// of their own, never while the session is held, as loading the
    /// grammars takes a while.
    pub fn warm_colors(&mut self) {
        let _ = self.colors();
    }

    /// The colours of the theme in effect, once made; `None` while they are
    /// being made (the lines stay plain meanwhile). After the theme changed
    /// they are made anew and every tab's colours start anew.
    pub(super) fn colors(&mut self) -> Option<Arc<HighlightSet>> {
        let theme = self.settings.effective().theme;
        if let Some((_, coming)) = &self.colors_coming {
            if let Ok(made) = coming.try_recv() {
                self.colors_coming = None;
                self.colors = Some(made);
                if let Some(open) = &mut self.open {
                    for (_, tab) in open.tabs.iter_mut() {
                        tab.highlight = None;
                        tab.highlight_told = None;
                    }
                }
            }
        }
        if let Some(colors) = self.colors.as_ref().filter(|c| c.theme == theme) {
            return Some(Arc::clone(&colors.set));
        }
        if self.colors_coming.as_ref().is_none_or(|(t, _)| *t != theme) {
            let base = self.colors.as_ref().map(|c| Arc::clone(&c.set));
            let wake = self.wake.clone();
            let (send, coming) = mpsc::channel();
            let made = std::thread::Builder::new()
                .name("underline-colors".into())
                .spawn(move || {
                    let set = base.map_or_else(
                        || HighlightSet::for_theme(theme),
                        |base| base.with_theme(theme),
                    );
                    let _ = send.send(Colors {
                        theme,
                        set: Arc::new(set),
                    });
                    wake.wake();
                });
            if made.is_ok() {
                self.colors_coming = Some((theme, coming));
            }
        }
        None
    }

    /// M-08.2, ADR-024: the colours of the lines the active tab's source
    /// view shows, once they changed; at most [`COLORING_BUDGET`] of work a
    /// tick, the rest in the next ones.
    pub(super) fn highlight_event(&mut self, now: Instant) -> Option<Event> {
        let has_viewport = self
            .open
            .as_ref()
            .and_then(|open| open.tabs.active())
            .is_some_and(|(_, tab)| tab.viewport.is_some());
        if !has_viewport {
            return None;
        }
        let wake = self.wake.clone();
        let told = {
            let open = self.open.as_ref()?;
            let (_, tab) = open.tabs.active()?;
            let wanted = tab.viewport.clone()?;
            tab.highlight_told.as_ref() == Some(&(u64::from(tab.mirror.rev()), wanted))
        };
        // The colours of a new theme reset `highlight_told`.
        let set = self.colors()?;
        let open = self.open.as_mut()?;
        let (note, tab) = open.tabs.active_mut()?;
        if told && tab.highlight_told.is_some() {
            return None;
        }
        // ADR-025: the colours carry the mirror's revision; the window
        // shows them when its core is in step with it.
        let rev = u64::from(tab.mirror.rev());
        let lines = tab.mirror.rope().len_lines();
        let wanted = tab.viewport.clone()?;
        let shown = wanted.start.min(lines)..wanted.end.min(lines);
        let format = tab.edit.format;
        let highlight = tab.highlight.get_or_insert_with(|| {
            let mut highlight = DocumentHighlight::new(set, format);
            highlight.invalidate_from(0, lines);
            highlight
        });
        let (foreground, _) = highlight.colors();
        let doc = tab.mirror.rope();
        let deadline = Some(now + COLORING_BUDGET);
        let mut colored: Vec<Arc<Vec<Span>>> = Vec::with_capacity(shown.len());
        for line in shown.clone() {
            if let Some(spans) = highlight.spans_by(line, |i| rope_line(doc, i), deadline) {
                colored.push(spans);
            } else {
                // Out of time: the next tick goes on where this one stopped.
                wake.wake();
                return None;
            }
        }
        let view = highlight_view(
            note,
            rev,
            &shown,
            &colored,
            |i| rope_line(doc, i).trim_end_matches(['\n', '\r']).to_owned(),
            foreground,
        );
        tab.highlight_told = Some((rev, wanted));
        Some(Event::Highlight(view))
    }
}

impl Session {
    /// M-08.8, M-12.2, UC-008 BR-008: after the inputs, the sections of the
    /// active tab's note and the marked hits of its Nano search, once they
    /// changed (ADR-024: never within the keystroke command).
    pub(super) fn editor_events(&mut self, now: Instant) -> Vec<Event> {
        let source_mode = self.source_keys_mode();
        let facade_mode = self.facade_keys_mode();
        let Some(open) = self.open.as_mut() else {
            return Vec::new();
        };
        let Some((note, tab)) = open.tabs.active_mut() else {
            return Vec::new();
        };
        let facade_shown = tab
            .facade
            .as_ref()
            .is_some_and(super::facade::Facade::shown);
        if tab.viewport.is_none() && !facade_shown {
            // Neither the source view nor the facade shows the note.
            return Vec::new();
        }
        // The keys of the view shown: the facade has the editor core's Vim.
        tab.edit.keys.mode = if facade_shown {
            facade_mode
        } else {
            source_mode
        };
        let mut events = Vec::new();
        let now = millis(now);
        events.extend(
            tab.edit
                .sections_view(note.as_str(), now)
                .map(Event::Sections),
        );
        events.extend(tab.edit.found_view(note.as_str(), now).map(Event::Found));
        events
    }
}

/// The colours of the lines `shown` for the window: spans in UTF-16 units
/// of each line, those in the plain colour of the theme left out.
fn highlight_view(
    note: &NoteId,
    rev: u64,
    shown: &Range<usize>,
    colored: &[Arc<Vec<Span>>],
    text: impl Fn(usize) -> String,
    foreground: [u8; 4],
) -> HighlightView {
    let mut styles: Vec<SpanStyle> = Vec::new();
    let mut lines = Vec::with_capacity(colored.len());
    for (line, spans) in shown.clone().zip(colored) {
        let text = text(line);
        let length = utf16::len_utf16(&text);
        let mut out = Vec::new();
        for span in spans.iter() {
            if span.color == foreground && !span.bold && !span.italic {
                continue;
            }
            let start = utf16::byte_to_utf16(&text, span.start).min(length);
            let end = utf16::byte_to_utf16(&text, span.end).min(length);
            if start >= end {
                continue;
            }
            let style = SpanStyle {
                color: hex(span.color),
                bold: span.bold,
                italic: span.italic,
            };
            let index = styles.iter().position(|s| *s == style).unwrap_or_else(|| {
                styles.push(style);
                styles.len() - 1
            });
            out.push((units(start), units(end), units(index)));
        }
        lines.push(out);
    }
    HighlightView {
        note: note.as_str().to_owned(),
        rev: revision(rev),
        from: units(shown.start),
        lines,
        styles,
    }
}

#[cfg(test)]
mod tests;
