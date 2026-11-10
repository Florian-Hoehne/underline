//! The formatting toolbar's functions and their keys (FR-023, UC-005 A2,
//! NFR-011), the same in both editing views. What each function does to the
//! markup is the facade engine's; [`Tool::format`] maps a [`Tool`] to it.

use underline_markup::formatting::Format;

use crate::input::{Key, Modifiers};

/// One function of the formatting toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// The heading function (`H`).
    Heading,
    /// The bold function (`B`).
    Bold,
    /// The italic function (`I`).
    Italic,
    /// The code function (`</>`).
    Code,
    /// The list function.
    List,
    /// The quote function.
    Quote,
    /// Asks for a web address or a note (UC-005 A2).
    Link,
    /// Asks for an image file (UC-005 A2).
    Image,
    /// The table function.
    Table,
}

impl Tool {
    /// M-10.2: the formatting the function applies at once, or `None` when
    /// it asks first (Link and Image, UC-005 A2), as `toolbar::format` of
    /// the egui widget.
    #[must_use]
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

    /// All functions, in the order of the toolbar.
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
    #[must_use]
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
    #[must_use]
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

    /// The function a key press calls, if any (`Ctrl` is `Cmd` on macOS).
    #[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-023, NFR-011: the toolbar keys need the command key (`Ctrl`, or
    /// `Cmd` on macOS), with Shift where the hint says so.
    #[test]
    fn uc005_toolbar_keys_need_the_command_key() {
        let command = Modifiers {
            command: true,
            ..Modifiers::default()
        };
        let shifted = Modifiers {
            shift: true,
            ..command
        };
        let ctrl_only = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        assert_eq!(Tool::for_key(Key::B, command), Some(Tool::Bold));
        assert_eq!(Tool::for_key(Key::I, command), Some(Tool::Italic));
        assert_eq!(Tool::for_key(Key::I, shifted), Some(Tool::Image));
        assert_eq!(Tool::for_key(Key::H, shifted), Some(Tool::Heading));
        assert_eq!(Tool::for_key(Key::H, command), None);
        assert_eq!(Tool::for_key(Key::B, Modifiers::default()), None);
        // On macOS Ctrl alone is not the command key.
        assert_eq!(Tool::for_key(Key::B, ctrl_only), None);
        assert_eq!(Tool::for_key(Key::Other, command), None);
    }
}
