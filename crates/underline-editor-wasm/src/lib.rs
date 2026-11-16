//! The editor of Underline in the webview (ADR-025): the editor's tab
//! (`underline_editor_view::EditorTab`) compiled to `wasm32-unknown-unknown`
//! with wasm-bindgen. A key in either view is answered here, without a call
//! to the backend.
//!
//! [`WasmTab`] is the only thing the window sees; `ui/src/lib/editor/core.ts`
//! is the only module that imports it and types its JSON with the DTOs of
//! `bindings.ts`. The calls themselves are in [`calls`], without
//! wasm-bindgen, so a native golden test drives the same calls the webview
//! makes (`docs/migration/variante-b.md`, step B3c).
//!
//! A panic would trap the WASM (`panic = abort`); [`start`] writes it to the
//! console first. The calls are tested never to panic (`hostile.rs`).

#![warn(missing_docs)]

pub mod calls;
#[cfg(test)]
mod golden;
#[cfg(test)]
mod hostile;
pub mod mirror;
#[cfg(test)]
mod sync_tests;

use wasm_bindgen::prelude::*;

use crate::calls::Core;
use crate::mirror::Mirror;

#[wasm_bindgen]
extern "C" {
    /// The browser's `console.error`.
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Runs once when the WASM is loaded: a panic is written to the console
/// before the WASM traps (`panic = abort`), so it is never silent. After a
/// trap the tab is not used again: `core.ts` lets it go and reports it
/// (ADR-025). The calls are meant never to panic (`hostile.rs`).
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        console_error(&format!("The editor's core failed: {info}"));
    }));
}

/// One open tab in the webview; see [`Core`] for what each call answers.
///
/// No call panics, whatever its arguments; an argument that is not the DTO
/// it should be throws a JavaScript `Error` with the reason. The clock is
/// the window's `performance.now()`, one epoch per tab, fed only by the
/// window: `setNow` before an input, `views(now)` after it.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmTab(Core);

#[wasm_bindgen]
impl WasmTab {
    /// The tab of `note` holding `text` in `format` (`FormatName`).
    ///
    /// # Errors
    ///
    /// `format` names no note format.
    #[wasm_bindgen(constructor)]
    pub fn new(note: &str, text: &str, format: &str, mac: bool) -> Result<WasmTab, JsError> {
        Ok(WasmTab(Core::open(note, text, format, mac)?))
    }

    /// The input mode of the settings (`Normal`, `Vim`, `Nano`).
    ///
    /// # Errors
    ///
    /// `mode` names no input mode.
    #[wasm_bindgen(js_name = setMode)]
    pub fn set_mode(&mut self, mode: &str) -> Result<(), JsError> {
        Ok(self.0.set_mode(mode)?)
    }

    /// UC-006 step 7: the note was renamed to `note`; what the tab reports
    /// names it so from now on.
    pub fn rename(&mut self, note: &str) {
        self.0.rename(note);
    }

    /// FR-097: the view `id` of the window shows the note from now on.
    #[wasm_bindgen(js_name = viewOpen)]
    pub fn view_open(&mut self, id: u32) {
        self.0.view_open(id);
    }

    /// FR-097: the view `id` is gone.
    #[wasm_bindgen(js_name = viewClose)]
    pub fn view_close(&mut self, id: u32) {
        self.0.view_close(id);
    }

    /// UC-005 BR-016: the calls from now on are those of the view `id`;
    /// `input` when the user acts in it.
    #[wasm_bindgen(js_name = useView)]
    pub fn use_view(&mut self, id: u32, input: bool) {
        self.0.use_view(id, input);
    }

    /// FR-097: `EditorPatch | null` that brings the source view of the view
    /// calling now from revision `base` to the text as it is, as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = sourceSince)]
    pub fn source_since(&mut self, base: u32) -> Result<String, JsError> {
        Ok(self.0.source_since(base)?)
    }

    /// The facade (`true`) or the source view is shown.
    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(&mut self, facade: bool) {
        self.0.set_view(facade);
    }

    /// UC-010 BR-001, UC-005 BR-019: close pairs is on or off.
    #[wasm_bindgen(js_name = setClosePairs)]
    pub fn set_close_pairs(&mut self, on: bool) {
        self.0.set_close_pairs(on);
    }

    /// UC-004 A16 step 1: how many lines the note has.
    #[wasm_bindgen(js_name = lineCount)]
    #[must_use]
    pub fn line_count(&self) -> u32 {
        self.0.line_count()
    }

    /// UC-004 BR-020 (FR-090): `TextStatisticsView` JSON of the note and,
    /// while text is selected, of the selection.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = textStatistics)]
    pub fn text_statistics(&mut self) -> Result<String, JsError> {
        Ok(self.0.text_statistics()?)
    }

    /// UC-024 step 2, FR-091: `NoteHeadView` JSON, the note attributes and
    /// the tags of the note.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = noteHead)]
    pub fn note_head(&mut self) -> Result<String, JsError> {
        Ok(self.0.note_head()?)
    }

    /// UC-024 step 4: `AttributeEdited<EditorPatch>` JSON of `change`
    /// (`AttributeChangeView`) in the source view.
    ///
    /// # Errors
    ///
    /// `change` is not an `AttributeChangeView`.
    pub fn attribute(&mut self, change: &str) -> Result<String, JsError> {
        Ok(self.0.attribute(change)?)
    }

    /// UC-024 step 4: `AttributeEdited<FacadePatch>` JSON of `change`
    /// (`AttributeChangeView`) at the facade's selection `at`.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeAttribute)]
    pub fn facade_attribute(&mut self, at: &str, change: &str) -> Result<String, JsError> {
        Ok(self.0.facade_attribute(at, change)?)
    }

    /// UC-004 BR-017: `string | null` JSON, what the status bar says when
    /// `target` (`LineTargetView`) lies beyond the note.
    ///
    /// # Errors
    ///
    /// `target` is not a `LineTargetView`.
    #[wasm_bindgen(js_name = lineNote)]
    pub fn line_note(&self, target: &str) -> Result<String, JsError> {
        Ok(self.0.line_note(target)?)
    }

    /// UC-004 A16, BR-017: `EditorPatch` of the source view's cursor gone
    /// to `target` (`LineTargetView`).
    ///
    /// # Errors
    ///
    /// `target` is not a `LineTargetView`.
    #[wasm_bindgen(js_name = goToLine)]
    pub fn go_to_line(&mut self, target: &str) -> Result<String, JsError> {
        Ok(self.0.go_to_line(target)?)
    }

    /// UC-004 A16, BR-017: `FacadePatch` of the facade's cursor gone to the
    /// line of `target` (`LineTargetView`) from the selection `at`.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeGoToLine)]
    pub fn facade_go_to_line(&mut self, at: &str, target: &str) -> Result<String, JsError> {
        Ok(self.0.facade_go_to_line(at, target)?)
    }

    /// The theme named `theme`, dark or not.
    #[wasm_bindgen(js_name = setLooks)]
    pub fn set_looks(&mut self, theme: &str, dark: bool) {
        self.0.set_looks(theme, dark);
    }

    /// The window's clock, `performance.now()` in milliseconds; never back.
    #[wasm_bindgen(js_name = setNow)]
    pub fn set_now(&mut self, now_ms: f64) {
        self.0.set_now(now_ms);
    }

    /// `EditorState` as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn state(&mut self) -> Result<String, JsError> {
        Ok(self.0.state()?)
    }

    /// FR-080: `LinkContextView | null`, the place of a link typed at the
    /// source view's cursor; `manual` for Ctrl+Space.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = linkContext)]
    pub fn link_context(&mut self, manual: bool) -> Result<String, JsError> {
        Ok(self.0.link_context(manual)?)
    }

    /// FR-080: `EditorPatch | null` of writing the link `choice`
    /// (`LinkChoice`) in the source view.
    ///
    /// # Errors
    ///
    /// `choice` is not a `LinkChoice`.
    #[wasm_bindgen(js_name = completeLink)]
    pub fn complete_link(&mut self, choice: &str) -> Result<String, JsError> {
        Ok(self.0.complete_link(choice)?)
    }

    /// FR-080: `LinkContextView | null` at the facade's selection `at`
    /// (`FacadeAt`); `manual` for Ctrl+Space.
    ///
    /// # Errors
    ///
    /// `at` is not a `FacadeAt`.
    #[wasm_bindgen(js_name = facadeLinkContext)]
    pub fn facade_link_context(&mut self, at: &str, manual: bool) -> Result<String, JsError> {
        Ok(self.0.facade_link_context(at, manual)?)
    }

    /// FR-080: `FacadePatch | null` of writing the link `choice`
    /// (`LinkChoice`) at the facade's selection `at` (`FacadeAt`).
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeCompleteLink)]
    pub fn facade_complete_link(&mut self, at: &str, choice: &str) -> Result<String, JsError> {
        Ok(self.0.facade_complete_link(at, choice)?)
    }

    /// `EditorPatch` of `input` (`EditorInput`), as JSON.
    ///
    /// # Errors
    ///
    /// `input` is not an `EditorInput`.
    pub fn input(&mut self, input: &str) -> Result<String, JsError> {
        Ok(self.0.input(input)?)
    }

    /// `EditorPatch | null` of the window's Vim changes, as JSON.
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
    ) -> Result<String, JsError> {
        Ok(self.0.apply(base, changes, anchor, head, new_step)?)
    }

    /// UC-008 BR-008, BR-011, M-13.2: marks the places of `spec` (JSON
    /// `FindSpec | null`) from now on.
    ///
    /// # Errors
    ///
    /// `spec` is not a `FindSpec` or `null`.
    #[wasm_bindgen(js_name = setSearch)]
    pub fn set_search(&mut self, spec: &str) -> Result<(), JsError> {
        Ok(self.0.set_search(spec)?)
    }

    /// UC-008 A6, M-13.4: `FindAnswer | null` of the step `step` (JSON
    /// `FindStep`) of the find field holding `spec` (JSON `FindSpec`).
    ///
    /// # Errors
    ///
    /// An argument is not what it should be.
    pub fn find(&mut self, spec: &str, step: &str) -> Result<String, JsError> {
        Ok(self.0.find(spec, step)?)
    }

    /// UC-005 A20, A21: `Replaced<EditorPatch>` of replacing the selected
    /// occurrence of `spec` (JSON `FindSpec`), or all, with `replacement`.
    ///
    /// # Errors
    ///
    /// `spec` is not a `FindSpec`.
    pub fn replace(&mut self, spec: &str, replacement: &str, all: bool) -> Result<String, JsError> {
        Ok(self.0.replace(spec, replacement, all)?)
    }

    /// UC-005 A20, A21, BR-017: `Replaced<FacadePatch>` of replacing in the
    /// text the facade shows.
    ///
    /// # Errors
    ///
    /// `spec` is not a `FindSpec`.
    #[wasm_bindgen(js_name = facadeReplace)]
    pub fn facade_replace(
        &mut self,
        spec: &str,
        replacement: &str,
        all: bool,
    ) -> Result<String, JsError> {
        Ok(self.0.facade_replace(spec, replacement, all)?)
    }

    /// M-13.4: the selected text (markup), for the find field.
    #[wasm_bindgen(js_name = selectedText)]
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.0.selected_text()
    }

    /// M-13.4, UC-005 A22: `FindStatus` of the find field, as JSON, with
    /// what is wrong with the replacement (JSON `string | null`).
    ///
    /// # Errors
    ///
    /// `replacement` is not a string or `null`.
    #[wasm_bindgen(js_name = findStatus)]
    pub fn find_status(&mut self, replacement: &str) -> Result<String, JsError> {
        Ok(self.0.find_status(replacement)?)
    }

    /// The place for the status line, as a JSON string.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn select(&mut self, rev: u32, anchor: u32, head: u32) -> Result<String, JsError> {
        Ok(self.0.select(rev, anchor, head)?)
    }

    /// `EditorPatch` of the fold arrow on `line`, as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn fold(&mut self, line: u32) -> Result<String, JsError> {
        Ok(self.0.fold(line)?)
    }

    /// `KeysStatusView | null` of the source view, as JSON.
    ///
    /// # Errors
    ///
    /// `vim` is not a `VimState` or `null`.
    #[wasm_bindgen(js_name = keysStatus)]
    pub fn keys_status(&mut self, vim: &str) -> Result<String, JsError> {
        Ok(self.0.keys_status(vim)?)
    }

    /// `Clipped<EditorPatch>` of `action` (`ClipAction`) with the clipboard's
    /// `content` (`ClipContentView | null`), as JSON.
    ///
    /// # Errors
    ///
    /// `action` is not a `ClipAction`, or `content` not a `ClipContentView`
    /// or `null`.
    pub fn clipboard(&mut self, action: &str, content: &str) -> Result<String, JsError> {
        Ok(self.0.clipboard(action, content)?)
    }

    /// `CopyOnSelectView` of copy on select (`on`), as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = copyOnSelect)]
    pub fn copy_on_select(&mut self, on: bool) -> Result<String, JsError> {
        Ok(self.0.copy_on_select(on)?)
    }

    /// Whether the selection is the one copy on select copied.
    #[wasm_bindgen(js_name = overCopied)]
    #[must_use]
    pub fn over_copied(&self) -> bool {
        self.0.over_copied()
    }

    /// `EditorPatch` of `insertion` (`Insertion`) in the source view, as
    /// JSON.
    ///
    /// # Errors
    ///
    /// `insertion` is not an `Insertion`.
    pub fn insert(&mut self, insertion: &str) -> Result<String, JsError> {
        Ok(self.0.insert(insertion)?)
    }

    /// `FacadeState` as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = facadeState)]
    pub fn facade_state(&mut self) -> Result<String, JsError> {
        Ok(self.0.facade_state()?)
    }

    /// `FacadeState` without its blocks, as JSON (the window holds them).
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = facadeHead)]
    pub fn facade_head(&mut self) -> Result<String, JsError> {
        Ok(self.0.facade_head()?)
    }

    /// `FacadePatch` as JSON.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeInput)]
    pub fn facade_input(
        &mut self,
        at: &str,
        input: &str,
        rows: &str,
        clipboard: Option<String>,
    ) -> Result<String, JsError> {
        Ok(self.0.facade_input(at, input, rows, clipboard.as_deref())?)
    }

    /// `Clipped<FacadePatch>` as JSON.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeClipboard)]
    pub fn facade_clipboard(
        &mut self,
        at: &str,
        action: &str,
        plain: bool,
        content: &str,
    ) -> Result<String, JsError> {
        Ok(self.0.facade_clipboard(at, action, plain, content)?)
    }

    /// `FacadePatch` of `insertion` (`Insertion`) at the selection `at`
    /// (`FacadeAt`), as JSON.
    ///
    /// # Errors
    ///
    /// An argument is not the DTO it should be.
    #[wasm_bindgen(js_name = facadeInsert)]
    pub fn facade_insert(&mut self, at: &str, insertion: &str) -> Result<String, JsError> {
        Ok(self.0.facade_insert(at, insertion)?)
    }

    /// `BlocksView[]` of the backend's colours and figures (`EnrichView`
    /// JSON), as JSON.
    ///
    /// # Errors
    ///
    /// `sent` is not an `EnrichView`.
    pub fn enrich(&mut self, sent: &str) -> Result<String, JsError> {
        Ok(self.0.enrich(sent)?)
    }

    /// `FacadePatch` after a change of the backend, as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = facadeRefresh)]
    pub fn facade_refresh(&mut self) -> Result<String, JsError> {
        Ok(self.0.facade_refresh()?)
    }

    /// `KeysStatusView | null` of the facade, as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = facadeKeys)]
    pub fn facade_keys(&mut self) -> Result<String, JsError> {
        Ok(self.0.facade_keys()?)
    }

    /// `TabViews` at `now_ms` (`performance.now()`), as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn views(&mut self, now_ms: f64) -> Result<String, JsError> {
        Ok(self.0.views(now_ms)?)
    }

    /// The whole text.
    #[must_use]
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// The hash of the text.
    #[must_use]
    pub fn hash(&self) -> String {
        self.0.hash()
    }

    /// ADR-025: the sync starts from the backend's text at `rev`.
    #[wasm_bindgen(js_name = startSync)]
    pub fn start_sync(&mut self, rev: u32) {
        self.0.start_sync(rev);
    }

    /// `SyncBatch | null` to send, as JSON.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    #[wasm_bindgen(js_name = takeOutbox)]
    pub fn take_outbox(&mut self) -> Result<String, JsError> {
        Ok(self.0.take_outbox()?)
    }

    /// `RemoteApplied` of the backend's `answer` (`SyncAnswer` JSON), as
    /// JSON.
    ///
    /// # Errors
    ///
    /// The sync was not started, or `answer` is not a `SyncAnswer`.
    pub fn acknowledge(&mut self, answer: &str) -> Result<String, JsError> {
        Ok(self.0.acknowledge(answer)?)
    }

    /// The backend rang: the next outbox fetches.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn ring(&mut self) -> Result<(), JsError> {
        Ok(self.0.ring()?)
    }

    /// The last `ed_sync` failed: its edits go again.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn failed(&mut self) -> Result<(), JsError> {
        Ok(self.0.failed()?)
    }

    /// After `ed_resync`: the backend took `sent`, the text sent, at `rev`;
    /// what was typed meanwhile stays to be sent.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn reset(&mut self, sent: &str, rev: u32) -> Result<(), JsError> {
        Ok(self.0.reset(sent, rev)?)
    }

    /// Whether a result of the backend for `rev` applies as it is.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    #[wasm_bindgen(js_name = inStep)]
    pub fn in_step(&mut self, rev: u32) -> Result<bool, JsError> {
        Ok(self.0.in_step(rev)?)
    }

    /// M-07.3: the backend's revision the window last synced with.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    #[wasm_bindgen(js_name = syncedRev)]
    pub fn synced_rev(&mut self) -> Result<u32, JsError> {
        Ok(self.0.synced_rev()?)
    }

    /// Whether nothing waits to be sent or fetched.
    ///
    /// # Errors
    ///
    /// The sync was not started.
    pub fn idle(&mut self) -> Result<bool, JsError> {
        Ok(self.0.idle()?)
    }
}

/// FR-086, UC-004 A16, A17: `LineTargetAnswer` JSON for what the user
/// typed in "Go to line".
///
/// # Errors
///
/// The answer could not be written.
#[wasm_bindgen(js_name = lineTarget)]
pub fn line_target(text: &str) -> Result<String, JsError> {
    Ok(calls::line_target(text)?)
}

/// ADR-025: the backend's mirror of one note, for the mock backend of the
/// dev server; see [`Mirror`].
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmMirror(Mirror);

#[wasm_bindgen]
impl WasmMirror {
    /// The mirror of `text` at revision 1.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> WasmMirror {
        WasmMirror(Mirror::new(text))
    }

    /// The revision.
    pub fn rev(&self) -> u32 {
        self.0.rev()
    }

    /// The text.
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// `{ answer: SyncAnswer, reverted: string | null }` JSON to `batch`
    /// (`SyncBatch` JSON); `reverted` is the version a late batch took the
    /// place of (UC-004 A9).
    ///
    /// # Errors
    ///
    /// `batch` is not a `SyncBatch`.
    pub fn receive(&mut self, batch: &str) -> Result<String, JsError> {
        Ok(self.0.receive(batch)?)
    }

    /// A change of the backend's own, in chars; the new revision, or
    /// `undefined` when it does not fit.
    #[wasm_bindgen(js_name = backendEdit)]
    pub fn backend_edit(
        &mut self,
        at: u32,
        remove: u32,
        insert: &str,
        sealed: bool,
    ) -> Option<u32> {
        self.0
            .backend_edit(at as usize, remove as usize, insert, sealed)
    }

    /// `EnrichView` JSON for the mock: the figures of the images from
    /// `images` (JSON object by target), every diagram with `diagram`, every
    /// formula with the placeholder `formula` (FR-098).
    ///
    /// # Errors
    ///
    /// An argument is not the JSON it should be.
    // A wasm-bindgen export takes plain arguments; this one serves only the
    // mock backend.
    #[allow(clippy::too_many_arguments)]
    pub fn enrichment(
        &self,
        note: &str,
        markdown: bool,
        theme: &str,
        dark: bool,
        images: &str,
        diagram: &str,
        formula: &str,
    ) -> Result<String, JsError> {
        Ok(self
            .0
            .enrichment(note, markdown, (theme, dark), images, diagram, formula)?)
    }

    /// A change from outside; the new revision.
    pub fn reload(&mut self, text: &str) -> u32 {
        self.0.reload(text)
    }

    /// `SyncAnswer` JSON after the window's `text` was taken.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn resync(&mut self, text: &str) -> Result<String, JsError> {
        Ok(self.0.resync(text)?)
    }
}

/// FR-076, FR-077 (UC-004 BR-014, UC-023 BR-002): the notes of go to note
/// or the commands of the command palette, ranked against what the user
/// typed with the rule of `underline_markup::suggest`, so the window ranks
/// without a call to the backend and both overlays follow one rule.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct WasmSuggestions(underline_markup::suggest::Suggestions);

#[wasm_bindgen]
impl WasmSuggestions {
    /// The suggestions, each as its name and its context at the same
    /// index (a note's title and folder, a command's name and menu), in the
    /// order they are listed with an empty field; a missing context is
    /// `""`, and contexts beyond the last name are not read.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(names: Vec<String>, contexts: Vec<String>) -> WasmSuggestions {
        WasmSuggestions(underline_markup::suggest::Suggestions::new(
            names
                .iter()
                .enumerate()
                .map(|(at, name)| (name.as_str(), contexts.get(at).map_or("", String::as_str))),
        ))
    }

    /// How many suggestions there are.
    #[must_use]
    pub fn len(&self) -> u32 {
        u32::try_from(self.0.len()).unwrap_or(u32::MAX)
    }

    /// Whether there is none.
    #[wasm_bindgen(js_name = isEmpty)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The indices of the suggestions that match `typed`, best first; the
    /// indices in `last` follow all others (commands not available now);
    /// an index in `last` that names no suggestion is left aside.
    #[must_use]
    pub fn rank(&self, typed: &str, last: Vec<u32>) -> Vec<u32> {
        let last: std::collections::HashSet<usize> = last
            .into_iter()
            .filter_map(|at| usize::try_from(at).ok())
            .collect();
        self.0
            .rank(typed, |at| last.contains(&at))
            .into_iter()
            .map(|at| u32::try_from(at).unwrap_or(u32::MAX))
            .collect()
    }
}

#[cfg(test)]
mod suggestion_tests {
    use super::WasmSuggestions;

    /// TC-004-26, TC-023-05: the window's ranking is the rule's.
    #[test]
    fn tc_004_26_the_wasm_ranks_as_the_rule() {
        let notes = WasmSuggestions::new(
            vec!["Release Plan".into(), "Plan".into(), "Plane Tickets".into()],
            vec![String::new(), String::new(), "travel".into()],
        );
        assert_eq!(notes.len(), 3);
        assert!(!notes.is_empty());
        assert_eq!(notes.rank("plan", Vec::new()), [1, 2, 0]);
        assert_eq!(notes.rank("plan", vec![1]), [2, 0, 1]);
        assert_eq!(notes.rank("travel", Vec::new()), [2]);
    }

    /// Review: whatever the window gives, the ranking answers without a
    /// panic (a panic would trap the WASM, `hostile.rs`).
    #[test]
    fn hostile_arguments_rank_without_a_panic() {
        // More contexts than names: the extra ones are not read.
        let more = WasmSuggestions::new(vec!["Plan".into()], vec!["a".into(), "b".into()]);
        assert_eq!(more.len(), 1);
        assert_eq!(more.rank("b", Vec::new()), Vec::<u32>::new());
        // Fewer contexts, empty names, duplicates.
        let odd = WasmSuggestions::new(
            vec![String::new(), "Plan".into(), "Plan".into(), "\"".into()],
            vec!["x".into()],
        );
        assert_eq!(odd.rank("", Vec::new()), [0, 1, 2, 3]);
        assert_eq!(odd.rank("plan", vec![1]), [2, 1]);
        assert_eq!(odd.rank("x", Vec::new()), [0]);
        // Indices out of range, also the largest, are left aside.
        assert_eq!(odd.rank("plan", vec![u32::MAX, 99, 2, 2]), [1, 2]);
        // Quotes, open or alone, and characters whose lower case is longer.
        for typed in [
            "\"", "\"\"", "\"plan", "plan\"", "İ", "ß", "Σ", "\u{301}", "  ", "\u{0}",
        ] {
            let _ = odd.rank(typed, vec![0]);
        }
        let unicode = WasmSuggestions::new(
            vec!["İstanbul".into(), "ΟΔΟΣ".into(), "e\u{301}te\u{301}".into()],
            Vec::new(),
        );
        assert_eq!(unicode.rank("i\u{307}st", Vec::new()), [0]);
        assert_eq!(unicode.rank("οδοσ", Vec::new()), [1]);
        assert_eq!(unicode.rank("\u{301}t", Vec::new()), [2]);
    }

    #[test]
    fn tc_023_04_missing_contexts_are_empty() {
        let commands = WasmSuggestions::new(vec!["New note…".into()], Vec::new());
        assert_eq!(commands.rank("new", vec![7]), [0]);
        assert!(commands.rank("xyzzy", Vec::new()).is_empty());
        assert!(WasmSuggestions::default().is_empty());
    }
}
