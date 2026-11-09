//! The editor logic of Underline without a user interface (ADR-012): the
//! text of a note as a rope with its undo rules ([`Document`]), the Vim and
//! Nano keys behind the [`Target`] trait ([`Keys`]), and the functions of
//! the formatting toolbar ([`Tool`]).
//!
//! It was moved out of the egui widget `underline-editor`, which now builds
//! on it, so that the same logic can run elsewhere as well
//! (`docs/migration/plan.md`, sections 1.2, 3, and 6). The crate depends on
//! no UI toolkit and builds for `wasm32-unknown-unknown`; keys come in as
//! its own [`KeyInput`], which each UI makes from its events.

#![warn(missing_docs)]

pub mod document;
pub mod input;
pub mod keys;
pub mod toolbar;

pub use document::{Document, Edit};
pub use input::{Key, KeyInput, Modifiers};
pub use keys::{EditorCommand, InputMode, InputStatus, KeyMap, Keys, KeysMode, Target};
pub use toolbar::Tool;
