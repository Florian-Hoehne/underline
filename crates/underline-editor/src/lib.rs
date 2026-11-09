//! Editor widget of Underline (ADR-012): one rope per note, a source view
//! that lays out only the visible lines, and later the facade on the same
//! text. The logic without egui (rope and undo, Vim and Nano keys, toolbar
//! functions) is in `underline-editor-core` (`docs/migration/plan.md`,
//! section 3); this crate draws it on egui and re-exports what the app uses.

mod blink;
mod clipboard;
mod enlarged;
mod facade_view;
pub mod fonts;
mod keys;
mod minimap;
mod source_view;
mod toolbar;

pub use clipboard::{
    ClipContent, ClipImage, Clipboard, MemoryClipboard, SharedClipboard, SystemClipboard,
};
pub use facade_view::FacadeView;
pub use keys::{EditorCommand, InputMode, InputStatus, KeyMap, KeysMode};
pub use source_view::{
    found_color, AgentMark, CursorPlace, SourceView, AGENT_BAR_COLOR, AGENT_COLOR,
};
pub use toolbar::Tool;
pub use underline_editor_core::{Document, Edit};

#[cfg(test)]
mod prototype_benchmark;
