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
/// edits its own Vim keys make (`apply`, A-01).
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
    /// The revision the changes apply to. When the view's text has
    /// another revision, it asks for the whole text again (`state`).
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
    /// UC-023 BR-001: its name in the command palette, such as "Bold".
    pub name: String,
    /// UC-023 BR-003: the letter of its key with the command key, such as
    /// "B".
    pub key: String,
    /// The key is held with Shift.
    pub shift: bool,
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
                name: tool.name().to_owned(),
                key: tool.key().0.to_string(),
                shift: tool.key().1,
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

/// UC-008 BR-008, M-11.3, M-13.2: the marked hits of a tab, in UTF-16
/// units: the words of the workspace search (or of the find field, A6) and
/// those of the search of the Nano keys; empty when none are. At most
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
    /// D, for Ctrl/Cmd+Shift+D (UC-005 A26: duplicate the line).
    D,
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
            KeyName::D => Self::D,
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
    /// FR-082, FR-083: an action of the editing comfort, from the command
    /// palette; its keys come as [`EditorInput::Key`].
    Edit {
        /// The action.
        action: EditActionName,
    },
    /// OQ-260, UC-005 BR-021: Vim's `>>` and `<<` (`>` and `<` on a
    /// selection) of the source view's Vim keys on the lines from the one
    /// holding web offset `from` to the one holding `to`: on a list item
    /// the list rule of Tab and Shift+Tab, elsewhere Vim's shift; one step
    /// for undo.
    VimShift {
        /// A web offset in the first line.
        from: u32,
        /// A web offset in the last line.
        to: u32,
        /// `<`: outdent.
        outdent: bool,
        /// How many times over (the count of `>` in visual mode).
        times: u32,
    },
}

/// FR-082, FR-083, UC-005 A26, A27: an action of the editing comfort that
/// the command palette runs in the view shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum EditActionName {
    /// Move the line or block up (Alt+Up).
    MoveLineUp,
    /// Move the line or block down (Alt+Down).
    MoveLineDown,
    /// Duplicate the line or block (Ctrl+Shift+D).
    DuplicateLine,
    /// Indent the list items (Tab).
    Indent,
    /// Outdent the list items (Shift+Tab).
    Outdent,
}

impl From<EditActionName> for underline_editor_core::EditAction {
    fn from(action: EditActionName) -> Self {
        use underline_editor_core::EditAction;
        match action {
            EditActionName::MoveLineUp => EditAction::MoveLineUp,
            EditActionName::MoveLineDown => EditAction::MoveLineDown,
            EditActionName::DuplicateLine => EditAction::DuplicateLine,
            EditActionName::Indent => EditAction::Indent,
            EditActionName::Outdent => EditAction::Outdent,
        }
    }
}

/// FR-086, UC-004 A16: a line to go to, and a column in it, both from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineTargetView {
    /// The line.
    pub line: u32,
    /// The column; `None`: the start of the line.
    pub column: Option<u32>,
}

/// UC-004 BR-020 (FR-090, M-16.2): the text statistics the note properties
/// show: words, characters and reading time of the note, and the words and
/// characters of the selection while text is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextStatisticsView {
    /// The words of the text the facade shows.
    pub words: u32,
    /// Its characters, spaces counted.
    pub characters: u32,
    /// The reading time in whole minutes, rounded up.
    pub reading_minutes: u32,
    /// The selection's words and characters; `None` while nothing is
    /// selected.
    pub selection: Option<SelectionStatisticsView>,
}

/// UC-004 BR-020: the words and characters of the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SelectionStatisticsView {
    /// The words selected.
    pub words: u32,
    /// The characters selected, spaces counted.
    pub characters: u32,
}

/// UC-024 BR-002: what the value of a note attribute holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AttributeKindName {
    /// Any text on one line.
    Text,
    /// Items, a field per item.
    List,
    /// A whole or decimal number.
    Number,
    /// Year, month and day.
    Date,
    /// `true` or `false`, a switch.
    YesNo,
    /// Fields of its own: shown as its markup, changed only in the source
    /// view (A6).
    Nested,
}

/// UC-024 step 2: a note attribute as the note properties show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NoteAttributeView {
    /// Its name as the note writes it.
    pub name: String,
    /// What its value holds.
    pub kind: AttributeKindName,
    /// The value as shown; for a nested value its markup.
    pub value: String,
    /// The items of a list.
    pub items: Vec<String>,
    /// The line of the note it starts on, from 1.
    pub line: u32,
    /// Where it starts, as a web offset of the text (A6: shown there).
    pub offset: u32,
}

/// UC-024 A5: why the front matter cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HeadProblemView {
    /// The line of the note, from 1.
    pub line: u32,
    /// What is wrong, in a few words.
    pub reason: String,
    /// The text of the front matter.
    pub markup: String,
    /// Where the line in error starts, as a web offset of the text.
    pub offset: u32,
}

/// UC-024 step 2, FR-091: the note attributes and the tags of a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NoteHeadView {
    /// The attributes in the order of the note.
    pub attributes: Vec<NoteAttributeView>,
    /// A5: why the front matter cannot be read; then no attributes.
    pub problem: Option<HeadProblemView>,
    /// UC-004 BR-016: the tags of the note, in lower case, each once: those
    /// of the attribute `tags` first, then those of the text.
    pub tags: Vec<String>,
    /// An AsciiDoc note, which holds only texts and lists (BR-002).
    pub asciidoc: bool,
}

/// UC-024 step 3: a value given for a note attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttributeValueView {
    /// A text on one line.
    Text {
        /// The text.
        text: String,
    },
    /// A list.
    List {
        /// Its items, in order.
        items: Vec<String>,
    },
    /// A number as typed.
    Number {
        /// The number, such as `2.5`.
        text: String,
    },
    /// A date as typed.
    Date {
        /// The date, `2026-11-11`.
        text: String,
    },
    /// Yes or no.
    YesNo {
        /// Whether it is yes.
        yes: bool,
    },
}

/// UC-024 step 3: what the user does to a note attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttributeChangeView {
    /// Adds an attribute after the last one.
    Add {
        /// Its name, as typed.
        name: String,
        /// Its value.
        value: AttributeValueView,
    },
    /// Gives an attribute a new value.
    Set {
        /// Its name.
        name: String,
        /// The new value.
        value: AttributeValueView,
    },
    /// Removes an attribute.
    Remove {
        /// Its name.
        name: String,
    },
}

/// UC-024 step 4, A3, A4: what a change of a note attribute did: the patch
/// of the view, or why nothing was written.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AttributeEdited<P> {
    /// The change of the text, for the view; `None` when nothing was written.
    pub patch: Option<P>,
    /// Why nothing was written, as the note properties say it.
    pub problem: Option<String>,
    /// The field the refusal is about (A3: the name, A4: the value).
    pub about: Option<AttributeFieldName>,
}

/// UC-024 A3, A4: the field a refused change is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AttributeFieldName {
    /// The name of a new attribute.
    Name,
    /// The value.
    Value,
    /// The note as a whole.
    Note,
}

/// UC-004 A16 step 2, A17: what the field of "Go to line" holds: the line
/// it names, or what is wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineTargetAnswer {
    /// The line and column; `None` when the text is no line number.
    pub target: Option<LineTargetView>,
    /// A17: why it is no line number, in the words the field shows.
    pub problem: Option<String>,
}

impl LineTargetAnswer {
    /// UC-004 A16, A17: the answer for `text` as the user typed it.
    #[must_use]
    pub fn of(text: &str) -> Self {
        match underline_editor_core::comfort::goto::LineTarget::parse(text) {
            Ok(target) => Self {
                target: Some(LineTargetView {
                    line: units(target.line),
                    column: target.column.map(units),
                }),
                problem: None,
            },
            Err(error) => Self {
                target: None,
                problem: Some(error.to_string()),
            },
        }
    }
}

impl From<LineTargetView> for underline_editor_core::comfort::goto::LineTarget {
    fn from(view: LineTargetView) -> Self {
        let wide = |n: u32| usize::try_from(n).unwrap_or(usize::MAX).max(1);
        Self {
            line: wide(view.line),
            column: view.column.map(wide),
        }
    }
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

/// M-08.6, M-09.14, M-10.6, ADR-025: what copying, cutting or pasting in
/// the webview's editor gives: the markup for the clipboard, if any, with
/// the HTML of the facade's selection, and the patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Clipped<P> {
    /// The markup for the system clipboard; `None` with nothing selected
    /// or when pasting.
    pub copied: Option<String>,
    /// UC-005 BR-011: the facade's selection as HTML, for other programs,
    /// put on the clipboard with `copied`; `None` in the source view.
    pub html: Option<String>,
    /// UC-005 A18, M-10.6: the paste found only an image; the window has
    /// the backend store it next to the note and inserts it then
    /// ([`Insertion::Image`]).
    pub image: bool,
    /// The patch for the view.
    pub patch: P,
}

/// UC-005 A17, A18, ADR-014: what the clipboard holds, as the backend read
/// it for a paste (`clip_read`). Over the selection copy on select made it
/// is what the clipboard held before (BR-011).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ClipContentView {
    /// The plain text, with `\n` line endings; `None` without text.
    pub text: Option<String>,
    /// The HTML of other programs as markup of the note's format
    /// (`html_to_markup`), when the facade asked for it; empty when the
    /// HTML held nothing the format keeps.
    pub markup: Option<String>,
    /// An image and nothing else (a screenshot).
    pub image: bool,
}

/// FR-031, BR-011: a selection copy on select puts on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SelectionCopy {
    /// The selected markup.
    pub text: String,
    /// The facade's selection as HTML; `None` in the source view.
    pub html: Option<String>,
    /// The first copy since the selection was made: the backend keeps what
    /// the clipboard held before, for pasting over the selection.
    pub first: bool,
}

/// FR-031, BR-011: what copy on select asks of the backend after an input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CopyOnSelectView {
    /// The selection to put on the clipboard, if any.
    pub copy: Option<SelectionCopy>,
    /// The copied selection is gone: the backend lets go of what the
    /// clipboard held before it.
    pub forget: bool,
}

/// UC-005 A2, A9 to A13, A18: what goes into the note at the cursor,
/// replacing the selection, in the markup of the note's format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Insertion {
    /// M-10.3: a link to `address`; its text is `label`, else the
    /// selection, else the address.
    Link {
        /// The web address or the place of a note.
        address: String,
        /// The link's text.
        label: Option<String>,
    },
    /// M-10.6: a picture at `place`, on a paragraph of its own.
    Image {
        /// Its place, relative to the note.
        place: String,
    },
    /// M-10.4, A9 to A11: a file put into the note: a picture when
    /// `image`, else a link labelled with its file name.
    File {
        /// Its place, relative to the note, with `/`.
        place: String,
        /// Whether it is inserted as a picture (BR-012).
        image: bool,
    },
    /// M-10.5, A9 step 2: a dropped file: as [`Insertion::File`], but in a
    /// code block only its place.
    Dropped {
        /// Its place, relative to the note, with `/`.
        place: String,
        /// Whether it is inserted as a picture (BR-012).
        image: bool,
    },
    /// M-10.5: text as it is, such as the space between dropped items.
    Text {
        /// The text.
        text: String,
    },
    /// UC-005 A28 step 2: a correction of the spell check in place of the
    /// selected word, as it is and as one change undone alone.
    Correction {
        /// The corrected word.
        text: String,
    },
}

/// M-08.8, M-12.2, ADR-025: what the webview's editor tells its view after
/// the inputs (`views`); each only once it changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TabViews {
    /// The sections for the fold arrows.
    pub sections: Option<SectionsView>,
    /// The marked hits: the words of the searches and of the Nano search.
    pub found: Option<FoundView>,
}

/// UC-008 A6, M-13.4: where a step of the find field moves the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FindStep {
    /// The text changed: the first hit from the selection on.
    First,
    /// Enter, F3, "Next": the next hit, wrapping around.
    Next,
    /// Shift+Enter, Shift+F3, "Previous": the hit before, wrapping around.
    Previous,
}

/// M-13.4: what the find field says: the hit selected, counted from 1, if
/// the selection is one exactly, and how many there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FindStatus {
    /// "{at} of {count}"; `None`: "{count} found".
    pub at: Option<u32>,
    /// How many hits there are.
    pub count: u32,
    /// UC-008 A9, UC-005 A22: what is wrong with the search text (an
    /// expression that is not valid), or with the replacement; nothing is
    /// found or replaced then.
    pub problem: Option<String>,
}

/// FR-075, UC-008 BR-011: the search switches of a field, each off at
/// first; Alt+C, Alt+W and Alt+R switch them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchSwitches {
    /// Upper and lower case must match as typed.
    pub match_case: bool,
    /// Only where the text is not part of a longer word.
    pub whole_word: bool,
    /// The search text is one regular expression.
    pub regex: bool,
}

impl From<underline_markup::search::Switches> for SearchSwitches {
    fn from(switches: underline_markup::search::Switches) -> Self {
        Self {
            match_case: switches.match_case,
            whole_word: switches.whole_word,
            regex: switches.regex,
        }
    }
}

impl From<SearchSwitches> for underline_markup::search::Switches {
    fn from(switches: SearchSwitches) -> Self {
        Self {
            match_case: switches.match_case,
            whole_word: switches.whole_word,
            regex: switches.regex,
        }
    }
}

/// UC-008 BR-008, A6, A7, UC-005 BR-017: what a tab marks and finds: the
/// search text with its switches; while the find field is open for
/// replacing, the text is one text (OQ-192), and in the facade only the
/// text the facade shows counts (OQ-190).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FindSpec {
    /// The search text as typed.
    pub text: String,
    /// The search switches.
    pub switches: SearchSwitches,
    /// The text is one text, spaces included, not words in any order.
    pub one_text: bool,
    /// Only the text the facade shows, not its markup and link addresses.
    pub shown_only: bool,
}

/// UC-005 A20, A21, FR-073: what replacing in the find field did: the
/// patch of the view, how many occurrences were replaced, and what the
/// find field says now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Replaced<P> {
    /// The change of the text and the selection, for the view.
    pub patch: P,
    /// How many occurrences were replaced (0: the selection moved to the
    /// next one, or nothing could be replaced).
    pub replaced: u32,
    /// What the find field says now.
    pub status: FindStatus,
}

/// UC-008 A6, M-13.4: the hit a step of the find field selected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FindAnswer {
    /// The revision of the text (the core's own).
    pub rev: u32,
    /// The start of the hit, in UTF-16 units.
    pub anchor: u32,
    /// Its end, where the cursor is.
    pub head: u32,
    /// The place for the status line (M-11.4).
    pub place: String,
    /// What the find field says now.
    pub status: FindStatus,
}

/// FR-080, UC-005 A23 step 1: the place of a link the user is typing at
/// the cursor, for link completion: the window lists the notes that match
/// `typed` as go to note does (BR-014), or after a `#` the headings of a
/// note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkContextView {
    /// The revision of the tab's text it was found in; the choice names it.
    pub rev: u32,
    /// Where the link starts, in web units; the list shows below it.
    pub from: u32,
    /// What was typed of the place before the cursor, without a `#` and
    /// what follows it.
    pub typed: String,
    /// What was typed after a `#`: the headings are listed then.
    pub heading: Option<String>,
    /// The note `typed` names as a place relative to the note, without
    /// its extension (`travel/Packing`); `None` when nothing is typed.
    pub named: Option<String>,
}

/// FR-080, UC-005 A23 steps 2 and 3: the note or the heading chosen in link
/// completion, which the tab writes as a link of the note format (BR-018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkChoice {
    /// The revision of the text the completion was asked in; in another,
    /// nothing is written.
    pub rev: u32,
    /// The chosen note; `None` for a heading of the note itself.
    pub note: Option<String>,
    /// Its note format (`Markdown`, `AsciiDoc`).
    pub format: Option<String>,
    /// The anchor of the chosen heading.
    pub anchor: Option<String>,
    /// The link's text when the user gave none: the note's title or the
    /// heading.
    pub label: String,
}
