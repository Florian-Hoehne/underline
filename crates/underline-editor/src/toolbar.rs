//! The formatting toolbar's functions (FR-023, UC-005 A2, NFR-011) on egui:
//! [`Tool`] and its keys live in `underline-editor-core`; this module gives
//! it egui's keys and the facade engine's formatting.

use eframe::egui::{Key, Modifiers};
pub use underline_editor_core::Tool;
use underline_facade::formatting::Format;

use crate::keys::{key_to_core, modifiers_to_core};

/// The formatting `tool` applies at once, or `None` when it asks first.
pub(crate) fn format(tool: Tool) -> Option<Format> {
    Some(match tool {
        Tool::Heading => Format::Heading,
        Tool::Bold => Format::Bold,
        Tool::Italic => Format::Italic,
        Tool::Code => Format::Code,
        Tool::List => Format::List,
        Tool::Quote => Format::Quote,
        Tool::Table => Format::Table,
        Tool::Link | Tool::Image => return None,
    })
}

/// The function a key press calls, if any (`Ctrl` is `Cmd` on macOS).
pub(crate) fn for_key(key: Key, modifiers: Modifiers) -> Option<Tool> {
    Tool::for_key(key_to_core(key), modifiers_to_core(modifiers))
}
