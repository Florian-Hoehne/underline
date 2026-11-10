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

use wasm_bindgen::prelude::*;

use crate::calls::Core;

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

    /// `Clipped<EditorPatch>` as JSON.
    ///
    /// # Errors
    ///
    /// `action` is not a `ClipAction`.
    pub fn clipboard(&mut self, action: &str, text: Option<String>) -> Result<String, JsError> {
        Ok(self.0.clipboard(action, text.as_deref())?)
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
        text: Option<String>,
    ) -> Result<String, JsError> {
        Ok(self
            .0
            .facade_clipboard(at, action, plain, text.as_deref())?)
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
}
