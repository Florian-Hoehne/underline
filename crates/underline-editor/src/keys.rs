//! The input modes (FR-024 to FR-026, UC-005 A6, A7, BR-009, BR-010,
//! BR-015) on egui: the Vim and Nano keys live in `underline-editor-core`;
//! this module turns egui's events, keys, and modifiers into its
//! [`KeyInput`], and the input mode of the settings into its own.

use eframe::egui::{self, Event};
pub use underline_core::InputMode;
pub use underline_editor_core::keys::{
    EditorCommand, InputMode as KeysInputMode, InputStatus, KeyMap, Keys, KeysMode, Target,
};
use underline_editor_core::{Key, KeyInput, Modifiers};

/// What `event` is to the Vim and Nano keys; `None` for events they leave
/// to the view (pointer, IME, cut, and the like).
pub(crate) fn to_core(event: &Event) -> Option<KeyInput<'_>> {
    Some(match event {
        Event::Text(text) => KeyInput::Text(text),
        Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } => KeyInput::Key {
            key: key_to_core(*key),
            pressed: *pressed,
            modifiers: modifiers_to_core(*modifiers),
        },
        Event::Copy => KeyInput::Copy,
        Event::Paste(_) => KeyInput::Paste,
        _ => return None,
    })
}

/// The settings' input mode as the keys know it.
pub(crate) fn mode_to_core(mode: InputMode) -> KeysInputMode {
    match mode {
        InputMode::Normal => KeysInputMode::Normal,
        InputMode::Vim => KeysInputMode::Vim,
        InputMode::Nano => KeysInputMode::Nano,
    }
}

/// egui's modifiers as they are: `command` is `Ctrl`, or `Cmd` on macOS.
pub(crate) fn modifiers_to_core(modifiers: egui::Modifiers) -> Modifiers {
    Modifiers {
        alt: modifiers.alt,
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        mac_cmd: modifiers.mac_cmd,
        command: modifiers.command,
    }
}

/// egui's key as the keys know it; keys they do not handle are
/// [`Key::Other`].
pub(crate) fn key_to_core(key: egui::Key) -> Key {
    use egui::Key as K;
    match key {
        K::Escape => Key::Escape,
        K::Enter => Key::Enter,
        K::Backspace => Key::Backspace,
        K::ArrowLeft => Key::ArrowLeft,
        K::ArrowRight => Key::ArrowRight,
        K::ArrowUp => Key::ArrowUp,
        K::ArrowDown => Key::ArrowDown,
        K::A => Key::A,
        K::B => Key::B,
        K::C => Key::C,
        K::D => Key::D,
        K::E => Key::E,
        K::F => Key::F,
        K::G => Key::G,
        K::H => Key::H,
        K::I => Key::I,
        K::J => Key::J,
        K::K => Key::K,
        K::L => Key::L,
        K::M => Key::M,
        K::N => Key::N,
        K::O => Key::O,
        K::P => Key::P,
        K::Q => Key::Q,
        K::R => Key::R,
        K::S => Key::S,
        K::T => Key::T,
        K::U => Key::U,
        K::V => Key::V,
        K::W => Key::W,
        K::X => Key::X,
        K::Y => Key::Y,
        K::Z => Key::Z,
        _ => Key::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-024, FR-025: egui's events reach the keys as they are, and what
    /// the keys do not know stays with the view.
    #[test]
    fn uc005_egui_events_reach_the_keys_unchanged() {
        let modifiers = egui::Modifiers {
            alt: true,
            ctrl: false,
            shift: true,
            mac_cmd: true,
            command: true,
        };
        let key = Event::Key {
            key: egui::Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        assert_eq!(
            to_core(&key),
            Some(KeyInput::Key {
                key: Key::K,
                pressed: true,
                modifiers: Modifiers {
                    alt: true,
                    ctrl: false,
                    shift: true,
                    mac_cmd: true,
                    command: true,
                },
            })
        );
        assert_eq!(to_core(&Event::Text("ä".into())), Some(KeyInput::Text("ä")));
        assert_eq!(to_core(&Event::Copy), Some(KeyInput::Copy));
        assert_eq!(to_core(&Event::Paste("x".into())), Some(KeyInput::Paste));
        assert_eq!(to_core(&Event::Cut), None);
        assert_eq!(key_to_core(egui::Key::F5), Key::Other);
        assert_eq!(mode_to_core(InputMode::Nano), KeysInputMode::Nano);
    }

    /// FR-024: the default input mode of the settings is the default mode
    /// of the keys.
    #[test]
    fn uc005_default_input_modes_agree() {
        assert_eq!(mode_to_core(InputMode::default()), KeysInputMode::default());
    }
}
