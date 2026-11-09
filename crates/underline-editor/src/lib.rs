//! Editor widget of Underline (ADR-012): one rope per note, a source view
//! that lays out only the visible lines, and later the facade on the same
//! text.

mod blink;
mod clipboard;
mod document;
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
pub use document::{Document, Edit};
pub use facade_view::FacadeView;
pub use keys::{EditorCommand, InputMode, InputStatus, KeyMap, KeysMode};
pub use source_view::{
    found_color, AgentMark, CursorPlace, SourceView, AGENT_BAR_COLOR, AGENT_COLOR,
};
pub use toolbar::Tool;

#[cfg(test)]
mod prototype_benchmark;
