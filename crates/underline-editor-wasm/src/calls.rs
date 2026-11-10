//! ADR-025: the editor's tab as the webview calls it, without wasm-bindgen:
//! every argument and every answer that is more than a number or a flag is
//! JSON of the DTOs in `bindings.ts`, so the native golden test drives the
//! very calls the webview makes.
//!
//! No call panics, whatever its arguments: offsets past the end or inside a
//! character are clamped by the editor, a revision that is not the text's is
//! refused, and JSON that is not the DTO is a [`CallError`] (the tests in
//! `hostile.rs`).
//!
//! **The clock.** The calls that find the sections and the Nano hits again
//! at most every 300 ms (`views`) take the window's time in milliseconds,
//! `performance.now()`; `setNow` sets it before an input. One tab has one
//! clock with one epoch, and only the window feeds it: the session's clock
//! (`millis` in `underline-session`) feeds the session's own tabs, never a
//! tab in the webview. The clock never goes back.
//!
//! Calls that only read (`state`, `facadeState`, the keys) still take the
//! tab mutably: they set the keys of the view first, and the facade's state
//! parses the blocks when they are not of the text.

use serde::de::DeserializeOwned;
use serde::Serialize;
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_editor_core::sync::{Answer, Change, Client};
use underline_editor_view::editor_view::{
    ClipAction, ClipContentView, Clipped, EditorInput, Insertion, TabViews, VimChange, VimState,
};
use underline_editor_view::facade_view::{FacadeAt, FacadeInput, RowsView};
use underline_editor_view::sync_view::{EnrichView, RemoteApplied, SyncAnswer, SyncBatch};
use underline_editor_view::{CachedViews, EditorTab, FacadeCall};
use underline_markup::NoteFormat;

/// Why a call of the webview could not be answered.
#[derive(Debug, thiserror::Error)]
pub enum CallError {
    /// An argument is not the JSON of the DTO it should be.
    #[error("{what} is not valid: {reason}")]
    Invalid {
        /// What the argument is, such as "The input".
        what: &'static str,
        /// What serde said of it.
        reason: serde_json::Error,
    },
    /// The name of a note format that is none.
    #[error("{0} is no note format")]
    NoFormat(String),
    /// The name of an input mode that is none.
    #[error("{0} is no input mode")]
    NoMode(String),
    /// The answer could not be written as JSON.
    #[error("The answer could not be written: {0}")]
    Answer(serde_json::Error),
    /// A call of the sync before it was started (`startSync`).
    #[error("The sync of this tab was not started")]
    NotSyncing,
}

/// `value` as JSON.
fn json<T: Serialize>(value: &T) -> Result<String, CallError> {
    serde_json::to_string(value).map_err(CallError::Answer)
}

/// The DTO of the JSON `text`, which is `what`.
fn parse<T: DeserializeOwned>(text: &str, what: &'static str) -> Result<T, CallError> {
    serde_json::from_str(text).map_err(|reason| CallError::Invalid { what, reason })
}

/// The note format named `name` (`FormatName` in `bindings.ts`).
fn format_of(name: &str) -> Result<NoteFormat, CallError> {
    match name {
        "Markdown" => Ok(NoteFormat::Markdown),
        "AsciiDoc" => Ok(NoteFormat::AsciiDoc),
        _ => Err(CallError::NoFormat(name.to_owned())),
    }
}

/// The input mode of the settings named `name` (`InputModeName` of the
/// window: `Normal`, `Vim` or `Nano`).
fn mode_of(name: &str) -> Result<KeysInputMode, CallError> {
    match name {
        "Normal" => Ok(KeysInputMode::Normal),
        "Vim" => Ok(KeysInputMode::Vim),
        "Nano" => Ok(KeysInputMode::Nano),
        _ => Err(CallError::NoMode(name.to_owned())),
    }
}

/// One open tab in the webview (ADR-025): the editor's tab, the views of
/// the facade made from what the backend sent, and what the window set.
#[derive(Debug)]
pub struct Core {
    note: String,
    mac: bool,
    tab: EditorTab,
    views: CachedViews,
    /// ADR-025: the window's side of the sync, once started.
    client: Option<Client>,
    /// The input mode of the settings.
    setting: KeysInputMode,
    /// The facade is shown, not the source view.
    facade: bool,
}

impl Core {
    /// The tab of `note` holding `text` in the format named `format`; `mac`
    /// when Cmd is the command key.
    ///
    /// # Errors
    ///
    /// `format` names no note format.
    pub fn open(note: &str, text: &str, format: &str, mac: bool) -> Result<Self, CallError> {
        Ok(Self {
            note: note.to_owned(),
            mac,
            tab: EditorTab::new(text, format_of(format)?),
            views: CachedViews::default(),
            client: None,
            setting: KeysInputMode::Normal,
            facade: false,
        })
    }

    /// UC-006 step 7: the note is `note` now; what the tab reports names
    /// it so (the window's views take only their note's blocks).
    pub fn rename(&mut self, note: &str) {
        note.clone_into(&mut self.note);
    }

    /// M-11: the input mode of the settings, by name.
    ///
    /// # Errors
    ///
    /// `mode` names no input mode.
    pub fn set_mode(&mut self, mode: &str) -> Result<(), CallError> {
        self.setting = mode_of(mode)?;
        Ok(())
    }

    /// M-07.2: the facade (`facade`) or the source view is shown.
    pub fn set_view(&mut self, facade: bool) {
        self.facade = facade;
    }

    /// The theme named `theme`, dark or not; what was sent for another
    /// theme is let go.
    pub fn set_looks(&mut self, theme: &str, dark: bool) {
        self.views = CachedViews::new(theme, dark);
    }

    /// ADR-025: the sync starts: the text is the backend's at revision
    /// `rev` (`ed_mirror`), and the edits from now on are sent.
    pub fn start_sync(&mut self, rev: u32) {
        self.tab.editor.record_edits();
        self.tab.editor.take_edits();
        self.client = Some(Client::new(&self.tab.editor.text(), rev));
    }

    /// The editor's edits since the last call go to the client.
    fn record(&mut self) {
        let edits = self.tab.editor.take_edits();
        if let Some(client) = &mut self.client {
            for edit in &edits {
                if client.local(&Change::from(edit)).is_err() {
                    // Never expected: the client follows the editor. The
                    // next answer finds the copies apart.
                    break;
                }
            }
        }
    }

    fn client(&mut self) -> Result<&mut Client, CallError> {
        self.record();
        self.client.as_mut().ok_or(CallError::NotSyncing)
    }

    /// `SyncBatch | null`: the edits to send (`ed_sync`), unless a batch is
    /// on its way or there is nothing to send.
    ///
    /// # Errors
    ///
    /// The sync was not started, or the answer could not be written.
    pub fn take_outbox(&mut self) -> Result<String, CallError> {
        let batch = self.client()?.take_outbox();
        json(&batch.as_ref().map(SyncBatch::from))
    }

    /// `RemoteApplied`: takes in the backend's answer (`SyncAnswer`) to the
    /// last batch; the patch for the source view, or that the window sends
    /// its whole text (`ed_resync`).
    ///
    /// # Errors
    ///
    /// The sync was not started, or `answer` is not a `SyncAnswer`.
    pub fn acknowledge(&mut self, answer: &str) -> Result<String, CallError> {
        let answer: SyncAnswer = parse(answer, "The answer")?;
        let acknowledged = self.client()?.acknowledge(&Answer::from(&answer));
        let desync = |reason: String| RemoteApplied {
            patch: None,
            desync: true,
            reason: Some(reason),
        };
        let applied = match acknowledged {
            Ok(incoming) => match self.tab.apply_incoming(incoming) {
                Ok(patch) => RemoteApplied {
                    patch,
                    desync: false,
                    reason: None,
                },
                Err(error) => desync(format!("a change of the backend does not fit: {error}")),
            },
            Err(error) => desync(format!("{error:?}")),
        };
        // The backend has what it sent.
        self.tab.editor.take_edits();
        json(&applied)
    }

    /// The backend rang (`ed-changed`, `ed-flush`): the next outbox fetches.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn ring(&mut self) -> Result<(), CallError> {
        self.client()?.ring();
        Ok(())
    }

    /// The last `ed_sync` failed: its edits go again with the next outbox.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn failed(&mut self) -> Result<(), CallError> {
        self.client()?.failed();
        Ok(())
    }

    /// After `ed_resync`: the backend took `sent`, the text sent, at `rev`.
    /// What was typed while it travelled stays to be sent.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn reset(&mut self, sent: &str, rev: u32) -> Result<(), CallError> {
        let now = self.tab.editor.text();
        self.client()?.reset(sent, rev, &now);
        Ok(())
    }

    /// Whether the window shows the backend's text of revision `rev` and
    /// nothing of its own (a result of the backend for `rev` applies).
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn in_step(&mut self, rev: u32) -> Result<bool, CallError> {
        Ok(self.client()?.in_step(rev))
    }

    /// M-07.3: the backend's revision the window last synced with.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn synced_rev(&mut self) -> Result<u32, CallError> {
        Ok(self.client()?.synced())
    }

    /// Whether nothing waits: no batch on its way, no edits to send, no
    /// fetch asked for.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn idle(&mut self) -> Result<bool, CallError> {
        Ok(self.client()?.idle())
    }

    /// The window's clock, `performance.now()` in milliseconds
    /// ([`EditorTab::set_now`]); set before an input. An earlier time or one
    /// that is no finite number is ignored.
    pub fn set_now(&mut self, now_ms: f64) {
        self.tab.set_now(now_ms);
    }

    /// The keys of the source view: the Nano keys in the Nano mode, else
    /// the usual keys; the source view's Vim keys are the window's (A-01).
    fn source_keys(&mut self) {
        self.tab.keys.mode = match self.setting {
            KeysInputMode::Nano => KeysInputMode::Nano,
            KeysInputMode::Normal | KeysInputMode::Vim => KeysInputMode::Normal,
        };
    }

    /// The keys of the facade: those of the settings.
    fn facade_keys_mode(&mut self) {
        self.tab.keys.mode = self.setting;
    }

    /// `EditorState`: the text with its cursor, as the source view shows it.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn state(&mut self) -> Result<String, CallError> {
        self.source_keys();
        json(&self.tab.state(&self.note))
    }

    /// `EditorPatch` of one input of the source view (`EditorInput`).
    ///
    /// # Errors
    ///
    /// `input` is not an `EditorInput`.
    pub fn input(&mut self, input: &str) -> Result<String, CallError> {
        let input: EditorInput = parse(input, "The input")?;
        self.source_keys();
        json(&self.tab.source_input(input, self.mac).patch)
    }

    /// `EditorPatch | null`: the changes `changes` (`VimChange[]`) of the
    /// window's Vim keys on revision `base`, the selection left at
    /// `anchor`, `head`.
    ///
    /// # Errors
    ///
    /// `changes` is not a `VimChange[]`.
    pub fn apply(
        &mut self,
        base: u32,
        changes: &str,
        anchor: u32,
        head: u32,
        new_step: bool,
    ) -> Result<String, CallError> {
        let changes: Vec<VimChange> = parse(changes, "The changes")?;
        let handled = self
            .tab
            .source_apply(base, &changes, (anchor, head), new_step);
        json(&handled.map(|handled| handled.patch))
    }

    /// The place for the status line (a JSON string) after the selection
    /// was set at `anchor`, `head` of revision `rev`.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn select(&mut self, rev: u32, anchor: u32, head: u32) -> Result<String, CallError> {
        json(&self.tab.select(rev, anchor, head))
    }

    /// `EditorPatch` after the fold arrow of the heading on `line`.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn fold(&mut self, line: u32) -> Result<String, CallError> {
        json(&self.tab.fold_line(line).patch)
    }

    /// `KeysStatusView | null` of the source view, with the window's Vim
    /// keys in the state `vim` (`VimState | null`).
    ///
    /// # Errors
    ///
    /// `vim` is not a `VimState` or `null`.
    pub fn keys_status(&mut self, vim: &str) -> Result<String, CallError> {
        let vim: Option<VimState> = parse(vim, "The Vim state")?;
        self.source_keys();
        json(&self.tab.source_keys_status(self.setting, vim))
    }

    /// `Clipped<EditorPatch>` of copying, cutting or pasting what the
    /// clipboard holds (`content`: `ClipContentView | null`) in the source
    /// view (`action`: `ClipAction`).
    ///
    /// # Errors
    ///
    /// `action` is not a `ClipAction`, or `content` not a
    /// `ClipContentView` or `null`.
    pub fn clipboard(&mut self, action: &str, content: &str) -> Result<String, CallError> {
        let action: ClipAction = parse(action, "The clipboard action")?;
        let content: Option<ClipContentView> = parse(content, "The clipboard's content")?;
        self.source_keys();
        let (copied, image, patch) = match action {
            ClipAction::Copy => {
                let (copied, patch) = self.tab.copy();
                (copied, false, patch)
            }
            ClipAction::Cut => {
                let (copied, handled) = self.tab.cut();
                (copied, false, handled.patch)
            }
            ClipAction::Paste => {
                let (image, handled) = self.tab.paste(content.as_ref());
                (None, image, handled.patch)
            }
        };
        json(&Clipped {
            copied,
            html: None,
            image,
            patch,
        })
    }

    /// `CopyOnSelectView`: FR-031, M-10.7, copy on select (`on`) of the
    /// view shown, once the selection settled.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn copy_on_select(&mut self, on: bool) -> Result<String, CallError> {
        json(&self.tab.copy_on_select(on, self.facade))
    }

    /// BR-011: whether the selection is the one copy on select copied.
    #[must_use]
    pub fn over_copied(&self) -> bool {
        self.tab.over_copied()
    }

    /// `EditorPatch` of putting `insertion` (`Insertion`) into the text at
    /// the source view's selection (M-10.3 to M-10.6).
    ///
    /// # Errors
    ///
    /// `insertion` is not an `Insertion`.
    pub fn insert(&mut self, insertion: &str) -> Result<String, CallError> {
        let insertion: Insertion = parse(insertion, "The insertion")?;
        self.source_keys();
        json(&self.tab.source_insert(insertion).patch)
    }

    /// `FacadeState`: the whole facade, its blocks made when they are not
    /// of the text.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn facade_state(&mut self) -> Result<String, CallError> {
        self.facade_keys_mode();
        self.tab.bring_facade_up_to_date(&mut self.views);
        json(&self.tab.facade_state(&self.note))
    }

    /// `FacadeState` without its blocks (`blocks` empty), its blocks made
    /// when they are not of the text: when the window holds the blocks of
    /// its `version`, it takes those again instead of all of them through
    /// JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn facade_head(&mut self) -> Result<String, CallError> {
        self.facade_keys_mode();
        self.tab.bring_facade_up_to_date(&mut self.views);
        json(&self.tab.facade_head(&self.note))
    }

    /// `FacadePatch` of one input of the facade (`FacadeInput`) at the
    /// selection `at` (`FacadeAt`), with the rows (`RowsView | null`) and
    /// the clipboard's text the Vim keys asked for.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    pub fn facade_input(
        &mut self,
        at: &str,
        input: &str,
        rows: &str,
        clipboard: Option<&str>,
    ) -> Result<String, CallError> {
        let at: FacadeAt = parse(at, "The selection")?;
        let input: FacadeInput = parse(input, "The input")?;
        let rows: Option<RowsView> = parse(rows, "The measure of the rows")?;
        self.facade_keys_mode();
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: &self.note,
            at,
            mac: self.mac,
            earlier,
            views: &mut self.views,
        };
        json(&self.tab.facade_input(call, input, rows, clipboard).patch)
    }

    /// `Clipped<FacadePatch>` of copying, cutting or pasting what the
    /// clipboard holds (`content`: `ClipContentView | null`) in the facade
    /// at the selection `at` (`FacadeAt`); `plain` pastes as typed.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    pub fn facade_clipboard(
        &mut self,
        at: &str,
        action: &str,
        plain: bool,
        content: &str,
    ) -> Result<String, CallError> {
        let at: FacadeAt = parse(at, "The selection")?;
        let action: ClipAction = parse(action, "The clipboard action")?;
        let content: Option<ClipContentView> = parse(content, "The clipboard's content")?;
        self.facade_keys_mode();
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: &self.note,
            at,
            mac: self.mac,
            earlier,
            views: &mut self.views,
        };
        let clipped = self
            .tab
            .facade_clipboard(call, action, plain, content.as_ref());
        json(&Clipped {
            copied: clipped.copied,
            html: clipped.html,
            image: clipped.image,
            patch: clipped.handled.patch,
        })
    }

    /// `FacadePatch` of putting `insertion` (`Insertion`) into the text at
    /// the facade's selection `at` (`FacadeAt`, M-10.3 to M-10.6).
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    pub fn facade_insert(&mut self, at: &str, insertion: &str) -> Result<String, CallError> {
        let at: FacadeAt = parse(at, "The selection")?;
        let insertion: Insertion = parse(insertion, "The insertion")?;
        self.facade_keys_mode();
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: &self.note,
            at,
            mac: self.mac,
            earlier,
            views: &mut self.views,
        };
        json(&self.tab.facade_insert(call, insertion).patch)
    }

    /// `BlocksView[]`: the facade's blocks take what the backend sent of
    /// colours and figures (`EnrichView`, `ed-enrich`).
    ///
    /// # Errors
    ///
    /// `sent` is not an `EnrichView`.
    pub fn enrich(&mut self, sent: &str) -> Result<String, CallError> {
        let sent: EnrichView = parse(sent, "The enrichment")?;
        self.views.take(&sent);
        json(&self.tab.facade_enriched(&self.note, &mut self.views))
    }

    /// `FacadePatch`: the facade after a change of the backend, its blocks
    /// made anew where needed.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn facade_refresh(&mut self) -> Result<String, CallError> {
        self.facade_keys_mode();
        json(&self.tab.facade_refresh(&self.note, &mut self.views))
    }

    /// `KeysStatusView | null` of the facade.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn facade_keys(&mut self) -> Result<String, CallError> {
        self.facade_keys_mode();
        json(&self.tab.facade_keys_status())
    }

    /// `TabViews`: the sections and the Nano hits at `now_ms` (the window's
    /// `performance.now()`, which also sets the clock), once they changed,
    /// for the view shown.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn views(&mut self, now_ms: f64) -> Result<String, CallError> {
        if self.facade {
            self.facade_keys_mode();
        } else {
            self.source_keys();
        }
        let sections = self.tab.sections_view(&self.note, now_ms);
        let found = self.tab.found_view(&self.note, now_ms);
        json(&TabViews { sections, found })
    }

    /// The whole text, its line breaks as they are.
    #[must_use]
    pub fn text(&self) -> String {
        self.tab.editor.text()
    }

    /// The hash of the text (`underline_editor_core::sync::hash`).
    #[must_use]
    pub fn hash(&self) -> String {
        underline_editor_core::sync::hash(&self.tab.editor.text())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::Core;

    fn parsed(text: &str) -> Value {
        serde_json::from_str(text).expect("JSON")
    }

    /// ADR-025, M-07.2: the facade shown again takes the blocks it drew;
    /// its head is the whole facade without them, of the same version.
    #[test]
    fn adr025_the_facades_head_is_its_state_without_the_blocks() {
        let mut core = Core::open("Home", "# Home\n\nText\n", "Markdown", false).expect("a format");
        let whole = parsed(&core.facade_state().expect("the state"));
        let mut head = parsed(&core.facade_head().expect("the head"));

        assert_eq!(head["blocks"], Value::Array(Vec::new()));
        assert!(!whole["blocks"].as_array().expect("blocks").is_empty());
        head["blocks"] = whole["blocks"].clone();
        assert_eq!(head, whole);
    }

    /// M-07.3: the window sends the revision it is in step with along with
    /// the place of a followed link; there is none before the sync started.
    #[test]
    fn m07_3_the_synced_revision_is_the_one_the_sync_started_with() {
        let mut core = Core::open("Home", "# Home\n", "Markdown", false).expect("a format");
        assert!(core.synced_rev().is_err());

        core.start_sync(7);

        assert_eq!(core.synced_rev().expect("syncing"), 7);
    }

    /// UC-006 step 7, M-14.2: a renamed note's tab reports its views under
    /// the new name, so the facade of the new tab takes them.
    #[test]
    fn uc006_m14_2_a_renamed_tab_names_its_new_note() {
        let mut core = Core::open("Plan", "# Plan\n", "Markdown", false).expect("a format");

        core.rename("work/Roadmap");

        let state = parsed(&core.facade_state().expect("the state"));
        assert_eq!(state["note"], "work/Roadmap");
    }
}
