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
    /// UC-005: the place of the cursor for the status line, such as
    /// "Ln 3, Col 5 · 4 chars".
    pub place: String,
    /// FR-026: the status of the Nano keys; `None` in the other modes.
    pub keys: Option<KeysStatusView>,
}

/// One change of the text: the UTF-16 range `from..to` of the text before it
/// was replaced by `insert`. The window sends such changes too, for the
/// edits its own Vim keys make (`ed_apply`, A-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// A-01: one change the window's Vim keys made, like [`TextChange`], and
/// whether it starts a new undo step: the window marks the start of each
/// Vim command and of each insert session (M-10.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct VimChange {
    /// Where the replaced text starts, in UTF-16 units.
    pub from: u32,
    /// Where the replaced text ends.
    pub to: u32,
    /// The text put in its place.
    pub insert: String,
    /// It is not undone together with the change before it.
    pub new_step: bool,
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
    /// UC-005: the place of the cursor for the status line.
    pub place: String,
    /// FR-026: the status of the Nano keys after the input; `None` in the
    /// other modes.
    pub keys: Option<KeysStatusView>,
    /// M-10.9: what the Nano keys ask of the window, in order: store the
    /// note at once, close its tab.
    pub commands: Vec<EditorCommandName>,
    /// UC-005 A2: a toolbar function that asks first (Link, Image); the
    /// window opens its dialog (feature F6).
    pub request: Option<ToolName>,
}

/// M-10.9, UC-005 BR-009, BR-010: what the keys of the editor ask of the
/// window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum EditorCommandName {
    /// Store the note at once (`:w`, Ctrl+O, Ctrl+S).
    Store,
    /// Close the tab (`:q`, Ctrl+X).
    Close,
}

impl From<underline_editor_core::EditorCommand> for EditorCommandName {
    fn from(command: underline_editor_core::EditorCommand) -> Self {
        match command {
            underline_editor_core::EditorCommand::Store => Self::Store,
            underline_editor_core::EditorCommand::Close => Self::Close,
        }
    }
}

/// M-10.2, FR-023: a function of the formatting toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ToolName {
    /// Heading.
    Heading,
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Code.
    Code,
    /// List.
    List,
    /// Quote.
    Quote,
    /// Link: asks for a web address or a note first.
    Link,
    /// Image: asks for an image file first.
    Image,
    /// Table.
    Table,
}

impl From<ToolName> for underline_editor_core::Tool {
    fn from(tool: ToolName) -> Self {
        use underline_editor_core::Tool;
        match tool {
            ToolName::Heading => Tool::Heading,
            ToolName::Bold => Tool::Bold,
            ToolName::Italic => Tool::Italic,
            ToolName::Code => Tool::Code,
            ToolName::List => Tool::List,
            ToolName::Quote => Tool::Quote,
            ToolName::Link => Tool::Link,
            ToolName::Image => Tool::Image,
            ToolName::Table => Tool::Table,
        }
    }
}

impl From<underline_editor_core::Tool> for ToolName {
    fn from(tool: underline_editor_core::Tool) -> Self {
        use underline_editor_core::Tool;
        match tool {
            Tool::Heading => Self::Heading,
            Tool::Bold => Self::Bold,
            Tool::Italic => Self::Italic,
            Tool::Code => Self::Code,
            Tool::List => Self::List,
            Tool::Quote => Self::Quote,
            Tool::Link => Self::Link,
            Tool::Image => Self::Image,
            Tool::Table => Self::Table,
        }
    }
}

/// M-10.2: one button of the formatting toolbar, as the egui app labels it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ToolView {
    /// The function.
    pub tool: ToolName,
    /// The label of the button, such as "B" or "List".
    pub label: String,
    /// The hint shown on hover, with the key, such as "Bold (Ctrl+B)".
    pub hint: String,
}

impl ToolView {
    /// All buttons, in the order of the toolbar.
    #[must_use]
    pub fn all() -> Vec<Self> {
        underline_editor_core::Tool::ALL
            .iter()
            .map(|&tool| Self {
                tool: tool.into(),
                label: tool.label().to_owned(),
                hint: tool.hint().to_owned(),
            })
            .collect()
    }
}

/// FR-026: a mode of the Vim keys, as the window's Vim keys report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum VimModeName {
    /// NORMAL.
    Normal,
    /// INSERT.
    Insert,
    /// VISUAL.
    Visual,
    /// VISUAL LINE.
    VisualLine,
    /// COMMAND: a command line after `:` or `/`.
    Command,
}

impl From<VimModeName> for underline_editor_core::KeysMode {
    fn from(mode: VimModeName) -> Self {
        use underline_editor_core::KeysMode;
        match mode {
            VimModeName::Normal => KeysMode::Normal,
            VimModeName::Insert => KeysMode::Insert,
            VimModeName::Visual => KeysMode::Visual,
            VimModeName::VisualLine => KeysMode::VisualLine,
            VimModeName::Command => KeysMode::Command,
        }
    }
}

/// FR-026, A-01: what the window's Vim keys (`codemirror-vim`) are doing,
/// for the status line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct VimState {
    /// The mode.
    pub mode: VimModeName,
    /// What is typed but not done yet, such as `2d` or `"+`.
    pub pending: String,
    /// The hits of a search are marked (UC-008 BR-008).
    pub marked: bool,
}

/// One command of the status line: its keys and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KeyHintView {
    /// The keys, such as ":w" or "^O".
    pub keys: String,
    /// What they do, such as "store".
    pub action: String,
}

/// FR-026, M-11.4: the left part of the editor's status line with the Vim
/// or Nano keys: the badge, what is typed but not done, and the commands,
/// the most important first (the window leaves out from the end what does
/// not fit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KeysStatusView {
    /// "VIM" or "NANO".
    pub keymap: String,
    /// The Vim mode, such as "NORMAL" or "VISUAL LINE"; `None` for Nano.
    pub mode: Option<String>,
    /// Which of the palette's mode colours the badge has: 0 normal (and
    /// Nano), 1 insert, 2 visual, 3 command (`PaletteView::mode_colors`).
    pub color: u32,
    /// What is typed but not done yet, or the Nano search prompt.
    pub pending: String,
    /// The commands.
    pub hints: Vec<KeyHintView>,
    /// The Nano search prompt is open and takes every key.
    pub prompting: bool,
}

impl KeysStatusView {
    /// The view of `status`; `prompting` while the Nano prompt is open.
    #[must_use]
    pub fn of(status: &underline_editor_core::InputStatus, prompting: bool) -> Self {
        use underline_editor_core::KeysMode;
        Self {
            keymap: status.keymap.label().to_owned(),
            mode: status.mode.map(|mode| mode.label().to_owned()),
            color: match status.mode {
                Some(KeysMode::Insert) => 1,
                Some(KeysMode::Visual | KeysMode::VisualLine) => 2,
                Some(KeysMode::Command) => 3,
                Some(KeysMode::Normal) | None => 0,
            },
            pending: status.pending.clone(),
            hints: status
                .hints
                .iter()
                .map(|(keys, action)| KeyHintView {
                    keys: (*keys).to_owned(),
                    action: (*action).to_owned(),
                })
                .collect(),
            prompting,
        }
    }
}

/// M-08.8, UC-004 BR-012: one section of the note, as the source view folds
/// it: lines counted from 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    /// The line of the heading.
    pub line: u32,
    /// The last line folding hides; `line` when the section has nothing to
    /// fold.
    pub last: u32,
    /// The section is folded.
    pub folded: bool,
}

/// M-08.8, M-12.2: the sections of the note shown, sent after the input
/// that changed them (ADR-024), and when a section was folded or expanded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SectionsView {
    /// The note.
    pub note: String,
    /// The revision of the text the sections are of.
    pub rev: u32,
    /// The sections, in order.
    pub sections: Vec<SectionView>,
}

/// UC-008 BR-008, M-11.3: the hits of the search of the Nano keys while
/// they are marked, in UTF-16 units; empty when none are. At most
/// [`FoundView::MAX`] hits are sent; `count` says how many there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundView {
    /// The note.
    pub note: String,
    /// The revision of the text the hits are in.
    pub rev: u32,
    /// The hits, `[from, to]`, the first [`FoundView::MAX`] of them.
    pub ranges: Vec<(u32, u32)>,
    /// How many hits there are in all.
    pub count: u32,
}

impl FoundView {
    /// The most hits sent to the window: enough for the minimap's sieve,
    /// and a bounded event however often the pattern occurs.
    pub const MAX: usize = 10_000;
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
    /// Escape (M-11.3: ends the marking of the Nano search, cancels its
    /// prompt).
    Escape,
    /// `[`, for Ctrl/Cmd+Shift+[ (M-08.8: fold).
    BracketLeft,
    /// `]`, for Ctrl/Cmd+Shift+] (M-08.8: expand).
    BracketRight,
    /// A, for Ctrl/Cmd+A (and Nano's Ctrl+A).
    A,
    /// B, for Ctrl/Cmd+B (M-10.2).
    B,
    /// C, for Nano's Ctrl+C.
    C,
    /// E, for Ctrl/Cmd+E (M-10.2) and Nano's Ctrl+E and Alt+E.
    E,
    /// H, for Ctrl/Cmd+Shift+H (M-10.2).
    H,
    /// I, for Ctrl/Cmd+I and Ctrl/Cmd+Shift+I (M-10.2).
    I,
    /// K, for Ctrl/Cmd+K (M-10.2) and Nano's Ctrl+K.
    K,
    /// L, for Ctrl/Cmd+Shift+L (M-10.2).
    L,
    /// O, for Nano's Ctrl+O.
    O,
    /// Q, for Ctrl/Cmd+Shift+Q (M-10.2).
    Q,
    /// R, for Vim's Ctrl+R in the facade (M-11.2).
    R,
    /// S, for Nano's Ctrl+S.
    S,
    /// T, for Ctrl/Cmd+Shift+T (M-10.2).
    T,
    /// U, for Nano's Ctrl+U and Alt+U.
    U,
    /// W, for Nano's Ctrl+W.
    W,
    /// X, for Nano's Ctrl+X.
    X,
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
            KeyName::Escape => Self::Escape,
            // The folding keys are the session's (M-08.8).
            KeyName::BracketLeft | KeyName::BracketRight => Self::Other,
            KeyName::A => Self::A,
            KeyName::B => Self::B,
            KeyName::C => Self::C,
            KeyName::E => Self::E,
            KeyName::H => Self::H,
            KeyName::I => Self::I,
            KeyName::K => Self::K,
            KeyName::L => Self::L,
            KeyName::O => Self::O,
            KeyName::Q => Self::Q,
            KeyName::R => Self::R,
            KeyName::S => Self::S,
            KeyName::T => Self::T,
            KeyName::U => Self::U,
            KeyName::W => Self::W,
            KeyName::X => Self::X,
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
    /// M-10.2: a button of the formatting toolbar.
    Tool {
        /// The function.
        tool: ToolName,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// M-08.6, M-09.14, ADR-025: what copying, cutting or pasting in the
/// webview's editor gives: the markup for the clipboard, if any, and the
/// patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Clipped<P> {
    /// The markup for the system clipboard; `None` with nothing selected
    /// or when pasting.
    pub copied: Option<String>,
    /// The patch for the view.
    pub patch: P,
}

/// M-08.8, M-12.2, ADR-025: what the webview's editor tells its view after
/// the inputs, as the backend's tick did (`ed-sections`, `ed-found`); each
/// only once it changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabViews {
    /// The sections for the fold arrows.
    pub sections: Option<SectionsView>,
    /// The marked hits of the Nano search.
    pub found: Option<FoundView>,
}
