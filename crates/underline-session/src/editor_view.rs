//! What the window and the session exchange about the text of an open note
//! (ADR-024): the backend owns the text, every input is one command, and the
//! answer is a patch. Offsets are UTF-16 code units, as JavaScript counts
//! them (`underline_editor_core::utf16`); revisions and offsets are `u32`,
//! as TypeScript numbers carry them exactly.

use serde::{Deserialize, Serialize};

/// The revision of a text as the window sees it: the low 32 bits of the
/// document's revision. Only compared for equality, so wrapping is harmless.
#[must_use]
pub fn revision(revision: u64) -> u32 {
    u32::try_from(revision & u64::from(u32::MAX)).unwrap_or(u32::MAX)
}

/// An offset or a count for the window; a text longer than `u32::MAX` UTF-16
/// units is not opened in practice.
#[must_use]
pub fn units(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The text of an open note as the window starts showing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EditorState {
    /// The note, by its id.
    pub note: String,
    /// The revision of `text`; every patch names the revision it applies to.
    pub rev: u32,
    /// The whole markup.
    pub text: String,
    /// The other end of the selection, in UTF-16 units.
    pub anchor: u32,
    /// The cursor, in UTF-16 units.
    pub head: u32,
}

/// One change of the text: the UTF-16 range `from..to` of the text before it
/// was replaced by `insert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
    /// Where the replaced text starts.
    pub from: u32,
    /// Where the replaced text ends.
    pub to: u32,
    /// The text put in its place.
    pub insert: String,
}

/// The answer to an input (ADR-024): the changes of the text, one after the
/// other, and where the cursor and the selection are now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EditorPatch {
    /// The revision the changes apply to. When the window's text has
    /// another revision, it asks for the whole text again (`ed_open`).
    pub base: u32,
    /// The revision after the changes.
    pub rev: u32,
    /// The changes, each relative to the text after the ones before it.
    pub changes: Vec<TextChange>,
    /// The other end of the selection, in UTF-16 units.
    pub anchor: u32,
    /// The cursor, in UTF-16 units.
    pub head: u32,
    /// The cursor moved by the input and is scrolled into view.
    pub scroll: bool,
    /// M-06.1: the note holds changes that are not stored yet. The tab bar
    /// shows it from the `tabs` event; the patch carries it so a view does
    /// not have to wait for the tick.
    pub unstored: bool,
}

/// A key the source view hands to the editor (M-08.4, M-10.1), named as
/// the webview's `KeyboardEvent.key` names it; letters in capitals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum KeyName {
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Enter.
    Enter,
    /// Tab.
    Tab,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// A, for Ctrl/Cmd+A.
    A,
    /// Y, for Ctrl/Cmd+Y.
    Y,
    /// Z, for Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z.
    Z,
}

impl From<KeyName> for underline_editor_core::Key {
    fn from(key: KeyName) -> Self {
        match key {
            KeyName::Backspace => Self::Backspace,
            KeyName::Delete => Self::Delete,
            KeyName::Enter => Self::Enter,
            KeyName::Tab => Self::Tab,
            KeyName::ArrowLeft => Self::ArrowLeft,
            KeyName::ArrowRight => Self::ArrowRight,
            KeyName::ArrowUp => Self::ArrowUp,
            KeyName::ArrowDown => Self::ArrowDown,
            KeyName::Home => Self::Home,
            KeyName::End => Self::End,
            KeyName::PageUp => Self::PageUp,
            KeyName::PageDown => Self::PageDown,
            KeyName::A => Self::A,
            KeyName::Y => Self::Y,
            KeyName::Z => Self::Z,
        }
    }
}

/// One input of the source view (UC-005, M-08.4, M-08.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EditorInput {
    /// Typed text, or the finished text of an input method.
    Text {
        /// The text.
        text: String,
    },
    /// A key with the modifiers the webview reported; the backend makes the
    /// platform's command key of them (Ctrl, or Cmd on macOS).
    Key {
        /// The key.
        key: KeyName,
        /// `ctrlKey`.
        ctrl: bool,
        /// `altKey`.
        alt: bool,
        /// `shiftKey`.
        shift: bool,
        /// `metaKey` (Cmd on macOS).
        meta: bool,
        /// How many lines Page Up and Page Down move: the lines the view
        /// shows.
        page: u32,
    },
}

/// The clipboard keys of the source view (M-08.6, ADR-014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ClipAction {
    /// The selected markup goes to the clipboard as plain text.
    Copy,
    /// Copy, then the selection goes.
    Cut,
    /// The plain text of the clipboard replaces the selection.
    Paste,
}

/// How one kind of highlighted text looks (ADR-010).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SpanStyle {
    /// The colour, `#rrggbb`.
    pub color: String,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
}

/// M-08.2: the highlighting of the lines the source view shows, sent after
/// the input that changed them (ADR-024), never with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HighlightView {
    /// The note.
    pub note: String,
    /// The revision of the text the colours are for; the window drops them
    /// for another revision, as newer ones follow.
    pub rev: u32,
    /// The first line, counted from 0.
    pub from: u32,
    /// For each line from `from` on: `[start, end, style]`, with start and
    /// end in UTF-16 units of the line and `style` an index into `styles`.
    /// Text in the plain colour of the theme has no span.
    pub lines: Vec<Vec<(u32, u32, u32)>>,
    /// The looks the spans refer to.
    pub styles: Vec<SpanStyle>,
}

/// M-16.2, UC-004 BR-010: the properties of a stored note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NotePropertiesView {
    /// "Place": the place in the workspace, such as `guide/Install.adoc`.
    pub place: String,
    /// "Format": "Markdown" or "AsciiDoc".
    pub format: String,
    /// "Size", such as `2 KB`.
    pub size: String,
    /// "Last change", local time to the minute.
    pub last_change: String,
}

/// M-16.2: the properties section of the tool panel for the active tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabPropertiesView {
    /// The note of the active tab.
    pub note: String,
    /// `None` when the file cannot be read: "Not saved yet".
    pub properties: Option<NotePropertiesView>,
}

/// M-06.3, M-21.4, UC-005 A14 step 3: "Changes not stored": closing failed
/// to store, and the user decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CloseQuestion {
    /// The note of the tab to close; `None` when Underline closes.
    pub tab: Option<String>,
    /// "the tab {title}" or "Underline", for "Close {what} anyway and lose
    /// these changes?".
    pub what: String,
    /// Why, for "Changes could not be stored: {error}".
    pub error: String,
}
