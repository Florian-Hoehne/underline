//! The formatting toolbar's functions and their keys (FR-023, UC-005 A2,
//! NFR-011), the same in both editing views.

use eframe::egui::{Key, Modifiers};
use underline_facade::formatting::Format;

/// One function of the formatting toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Heading,
    Bold,
    Italic,
    Code,
    List,
    Quote,
    /// Asks for a web address or a note (UC-005 A2).
    Link,
    /// Asks for an image file (UC-005 A2).
    Image,
    Table,
}

impl Tool {
    pub const ALL: [Tool; 9] = [
        Tool::Heading,
        Tool::Bold,
        Tool::Italic,
        Tool::Code,
        Tool::List,
        Tool::Quote,
        Tool::Link,
        Tool::Image,
        Tool::Table,
    ];

    /// The label of its button.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Heading => "H",
            Tool::Bold => "B",
            Tool::Italic => "I",
            Tool::Code => "</>",
            Tool::List => "List",
            Tool::Quote => "Quote",
            Tool::Link => "Link",
            Tool::Image => "Image",
            Tool::Table => "Table",
        }
    }

    /// The name and key shown next to the button (NFR-011).
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Heading => "Heading (Ctrl+Shift+H)",
            Tool::Bold => "Bold (Ctrl+B)",
            Tool::Italic => "Italic (Ctrl+I)",
            Tool::Code => "Code (Ctrl+E)",
            Tool::List => "List (Ctrl+Shift+L)",
            Tool::Quote => "Quote (Ctrl+Shift+Q)",
            Tool::Link => "Link (Ctrl+K)",
            Tool::Image => "Image (Ctrl+Shift+I)",
            Tool::Table => "Table (Ctrl+Shift+T)",
        }
    }

    /// The formatting it applies at once, or `None` when it asks first.
    pub fn format(self) -> Option<Format> {
        Some(match self {
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
    pub fn for_key(key: Key, modifiers: Modifiers) -> Option<Tool> {
        if !modifiers.command {
            return None;
        }
        Some(match (key, modifiers.shift) {
            (Key::B, false) => Tool::Bold,
            (Key::I, false) => Tool::Italic,
            (Key::E, false) => Tool::Code,
            (Key::K, false) => Tool::Link,
            (Key::H, true) => Tool::Heading,
            (Key::L, true) => Tool::List,
            (Key::Q, true) => Tool::Quote,
            (Key::I, true) => Tool::Image,
            (Key::T, true) => Tool::Table,
            _ => return None,
        })
    }
}
