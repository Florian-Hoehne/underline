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

    /// The facade (`true`) or the source view is shown.
    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(&mut self, facade: bool) {
        self.0.set_view(facade);
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

    /// UC-008 BR-008, M-13.2: marks the words of `text` (JSON
    /// `string | null`) from now on.
    ///
    /// # Errors
    ///
    /// `text` is not a string or `null`.
    #[wasm_bindgen(js_name = setSearch)]
    pub fn set_search(&mut self, text: &str) -> Result<(), JsError> {
        Ok(self.0.set_search(text)?)
    }

    /// UC-008 A6, M-13.4: `FindAnswer | null` of the step `step` (JSON
    /// `FindStep`) of the find field holding `text` (a JSON string).
    ///
    /// # Errors
    ///
    /// An argument is not what it should be.
    pub fn find(&mut self, text: &str, step: &str) -> Result<String, JsError> {
        Ok(self.0.find(text, step)?)
    }

    /// M-13.4: the selected text (markup), for the find field.
    #[wasm_bindgen(js_name = selectedText)]
    #[must_use]
    pub fn selected_text(&self) -> String {
        self.0.selected_text()
    }

    /// M-13.4: `FindStatus` of the find field, as JSON.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    #[wasm_bindgen(js_name = findStatus)]
    pub fn find_status(&mut self) -> Result<String, JsError> {
        Ok(self.0.find_status()?)
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
    /// `images` (JSON object by target), every diagram with `diagram`.
    ///
    /// # Errors
    ///
    /// An argument is not the JSON it should be.
    pub fn enrichment(
        &self,
        note: &str,
        markdown: bool,
        theme: &str,
        dark: bool,
        images: &str,
        diagram: &str,
    ) -> Result<String, JsError> {
        Ok(self
            .0
            .enrichment(note, markdown, (theme, dark), images, diagram)?)
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
