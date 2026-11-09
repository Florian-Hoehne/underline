//! Spike F0 variant B: the owner in the webview, compiled to wasm32. Inputs
//! and patches cross as JSON strings, like the IPC of variant A does.

use f0_model::{FacadeInput, Owner, SourceInput};
use wasm_bindgen::prelude::*;

/// The owner of the text, living in the webview.
#[wasm_bindgen]
pub struct WasmOwner(Owner);

#[wasm_bindgen]
impl WasmOwner {
    /// An owner of `text` that keeps its edits for the backend's mirror.
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> WasmOwner {
        WasmOwner(Owner::new(text, true))
    }

    /// A source view input (JSON `SourceInput`), answered with a JSON
    /// `SourcePatch`.
    #[wasm_bindgen(js_name = sourceInput)]
    pub fn source_input(&mut self, input: &str) -> Result<String, JsError> {
        let input: SourceInput = serde_json::from_str(input)?;
        Ok(serde_json::to_string(&self.0.source_input(&input))?)
    }

    /// A facade input (JSON `FacadeInput`), answered with a JSON `FacadePatch`.
    #[wasm_bindgen(js_name = facadeInput)]
    pub fn facade_input(&mut self, input: &str) -> Result<String, JsError> {
        let input: FacadeInput = serde_json::from_str(input)?;
        Ok(serde_json::to_string(&self.0.facade_input(&input))?)
    }

    /// The HTML of every block.
    #[wasm_bindgen(js_name = renderAll)]
    pub fn render_all(&mut self) -> String {
        self.0.render_all()
    }

    /// The HTML of blocks `from..to`, as a JSON array.
    #[wasm_bindgen(js_name = renderRange)]
    pub fn render_range(&mut self, from: usize, to: usize) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.0.render_range(from, to))?)
    }

    /// The number of blocks.
    #[wasm_bindgen(js_name = blockCount)]
    pub fn block_count(&mut self) -> usize {
        self.0.block_count()
    }

    /// The whole text.
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// The FNV-1a hash of the text.
    pub fn hash(&self) -> String {
        f0_model::hash(&self.0.text())
    }

    /// The revision of the text.
    pub fn revision(&self) -> f64 {
        self.0.revision() as f64
    }

    /// The edits since the last call, as JSON `MirrorEdit[]`.
    #[wasm_bindgen(js_name = takeMirror)]
    pub fn take_mirror(&mut self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.0.take_mirror())?)
    }

    /// The benchmark positions, as JSON `BenchPosition[]`.
    #[wasm_bindgen(js_name = benchPositions)]
    pub fn bench_positions(&mut self) -> Result<String, JsError> {
        Ok(serde_json::to_string(&self.0.bench_positions())?)
    }

    /// The facade text of block `index`.
    #[wasm_bindgen(js_name = blockText)]
    pub fn block_text(&mut self, index: usize) -> String {
        self.0.block_text(index)
    }
}
