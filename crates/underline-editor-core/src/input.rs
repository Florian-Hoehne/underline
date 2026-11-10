//! The input the keys of the editor read (FR-024, FR-025, NFR-011), in a
//! form of their own, so that no UI toolkit is needed: each UI turns its
//! events into a [`KeyInput`] and gives the events the keys leave to its
//! view as usual.

/// A key the editor's keys tell apart; every other key is [`Key::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// The Escape key.
    Escape,
    /// The Enter key.
    Enter,
    /// The Backspace key.
    Backspace,
    /// The left arrow key.
    ArrowLeft,
    /// The right arrow key.
    ArrowRight,
    /// The up arrow key.
    ArrowUp,
    /// The down arrow key.
    ArrowDown,
    /// The Delete key.
    Delete,
    /// The Tab key.
    Tab,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The Page Up key.
    PageUp,
    /// The Page Down key.
    PageDown,
    /// The A key.
    A,
    /// The B key.
    B,
    /// The C key.
    C,
    /// The D key.
    D,
    /// The E key.
    E,
    /// The F key.
    F,
    /// The G key.
    G,
    /// The H key.
    H,
    /// The I key.
    I,
    /// The J key.
    J,
    /// The K key.
    K,
    /// The L key.
    L,
    /// The M key.
    M,
    /// The N key.
    N,
    /// The O key.
    O,
    /// The P key.
    P,
    /// The Q key.
    Q,
    /// The R key.
    R,
    /// The S key.
    S,
    /// The T key.
    T,
    /// The U key.
    U,
    /// The V key.
    V,
    /// The W key.
    W,
    /// The X key.
    X,
    /// The Y key.
    Y,
    /// The Z key.
    Z,
    /// A key none of the keys handle.
    Other,
}

/// The modifier keys held with a key.
///
/// `command` is the platform's command key: `Ctrl` on Linux and Windows,
/// `Cmd` on macOS. `mac_cmd` is the `Cmd` key on macOS only. A UI fills them
/// in as its platform reports them, so "Ctrl means Cmd on macOS" holds.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// The Alt key.
    pub alt: bool,
    /// The Ctrl key.
    pub ctrl: bool,
    /// The Shift key.
    pub shift: bool,
    /// The `Cmd` key on macOS; always `false` elsewhere.
    pub mac_cmd: bool,
    /// `Ctrl` off macOS (Linux, Windows), `Cmd` on macOS, as egui reports it.
    pub command: bool,
}

impl Modifiers {
    /// The modifiers of a key event as a webview reports them
    /// (`ctrlKey`, `altKey`, `shiftKey`, `metaKey`), on macOS when `mac`.
    ///
    /// They follow egui exactly (plan section 14): `command` is `Ctrl` off
    /// macOS and `Cmd` (`metaKey`) on macOS, and `mac_cmd` is set on macOS
    /// only.
    #[must_use]
    pub fn from_web(ctrl: bool, alt: bool, shift: bool, meta: bool, mac: bool) -> Self {
        Self {
            alt,
            ctrl,
            shift,
            mac_cmd: mac && meta,
            command: if mac { meta } else { ctrl },
        }
    }
}

/// One input event as the keys see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput<'a> {
    /// Typed text.
    Text(&'a str),
    /// A key pressed (`pressed`) or let go.
    Key {
        /// The key the event is for.
        key: Key,
        /// `true` for a press, `false` for a release.
        pressed: bool,
        /// The modifier keys held with it.
        modifiers: Modifiers,
    },
    /// The platform's copy (`Ctrl+C` or `Cmd+C`).
    Copy,
    /// The platform's paste, whatever it pastes.
    Paste,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plan section 14: off macOS, Ctrl is the command key and `mac_cmd`
    /// stays off, whatever the Meta (Super) key does.
    #[test]
    fn web_modifiers_off_macos_take_ctrl_as_command() {
        let ctrl = Modifiers::from_web(true, false, false, false, false);
        assert!(ctrl.command && ctrl.ctrl && !ctrl.mac_cmd);
        let meta = Modifiers::from_web(false, false, false, true, false);
        assert!(!meta.command && !meta.mac_cmd);
    }

    /// Plan section 14: on macOS, Cmd is the command key and `mac_cmd`;
    /// Ctrl is only Ctrl.
    #[test]
    fn web_modifiers_on_macos_take_cmd_as_command() {
        let cmd = Modifiers::from_web(false, false, true, true, true);
        assert!(cmd.command && cmd.mac_cmd && cmd.shift && !cmd.ctrl);
        let ctrl = Modifiers::from_web(true, true, false, false, true);
        assert!(!ctrl.command && !ctrl.mac_cmd && ctrl.ctrl && ctrl.alt);
    }
}
