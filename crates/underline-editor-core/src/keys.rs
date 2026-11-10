//! The input modes (FR-024 to FR-026, UC-005 A6, A7, BR-009, BR-010,
//! BR-015): the Vim and Nano keys, the same in both editing views. The keys
//! work through [`Target`], which each view gives: moving by a shown
//! character or line, what a line is (a markup line in the source view, a
//! block in the facade, OQ-077), and changing the text.

use std::ops::Range;

use crate::input::{Key, KeyInput, Modifiers};

/// The set of keys the editor follows (FR-024, FR-025): the usual keys of
/// the platform, Vim, or Nano.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InputMode {
    /// The usual keys of the platform.
    #[default]
    Normal,
    /// The Vim keys (FR-024).
    Vim,
    /// The Nano keys (FR-025).
    Nano,
}

/// What the keys ask of the window: `:w` and `Ctrl+O` store, `:q` and
/// `Ctrl+X` close the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    /// Store the note (`:w`, `Ctrl+O`).
    Store,
    /// Close the tab (`:q`, `Ctrl+X`).
    Close,
}

/// FR-026: the key map an [`InputStatus`] is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMap {
    /// The Vim keys.
    Vim,
    /// The Nano keys.
    Nano,
}

impl KeyMap {
    /// The name the status line shows.
    pub fn label(self) -> &'static str {
        match self {
            KeyMap::Vim => "VIM",
            KeyMap::Nano => "NANO",
        }
    }
}

/// FR-026, BR-009: the Vim mode an [`InputStatus`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysMode {
    /// Normal mode, where keys move and act.
    Normal,
    /// Insert mode, where typed text goes into the note.
    Insert,
    /// Visual mode, selecting by character.
    Visual,
    /// Visual mode, selecting by line.
    VisualLine,
    /// A command line after `:` or `/`.
    Command,
}

impl KeysMode {
    /// The name the status line shows.
    pub fn label(self) -> &'static str {
        match self {
            KeysMode::Normal => "NORMAL",
            KeysMode::Insert => "INSERT",
            KeysMode::Visual => "VISUAL",
            KeysMode::VisualLine => "VISUAL LINE",
            KeysMode::Command => "COMMAND",
        }
    }
}

/// FR-026, UC-005: what the editor's status line shows of an input mode
/// other than the normal keys: the key map, its mode (Vim only), what is
/// typed but not done yet, and the most important commands as keys and
/// action. The commands come most important first: when the line is too
/// short, the last ones are left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputStatus {
    /// The key map in use.
    pub keymap: KeyMap,
    /// The Vim mode; `None` for Nano, which has none.
    pub mode: Option<KeysMode>,
    /// A count, operator, or command line typed in Vim, or the search
    /// prompt of Nano.
    pub pending: String,
    /// The commands, as (keys, action).
    pub hints: Vec<(&'static str, &'static str)>,
}

impl InputStatus {
    /// FR-026: the status of the Vim keys in `mode`, with `pending` typed
    /// but not done yet (a count, an operator, or the command line) and
    /// whether the hits of a search are `marked` (UC-008 BR-008).
    ///
    /// Shared by the Vim keys of this crate and by a view whose Vim keys
    /// come from elsewhere (the source view of the Tauri window,
    /// `codemirror-vim`), so the status line reads the same.
    #[must_use]
    pub fn vim(mode: KeysMode, pending: String, marked: bool) -> Self {
        let hints = match mode {
            KeysMode::Normal => marked
                .then_some(("Esc", "end search"))
                .into_iter()
                .chain([
                    ("i", "insert"),
                    ("v", "visual"),
                    (":w", "store"),
                    (":q", "close"),
                    ("dd", "delete line"),
                    ("yy", "yank"),
                    ("p", "put"),
                    ("u", "undo"),
                    ("Ctrl+R", "redo"),
                    ("/ n", "search"),
                ])
                .collect(),
            KeysMode::Insert => vec![("Esc", "normal mode")],
            KeysMode::Visual | KeysMode::VisualLine => vec![
                ("d", "delete"),
                ("y", "yank"),
                ("c", "change"),
                ("Esc", "normal mode"),
            ],
            KeysMode::Command => vec![("Enter", "run"), ("Esc", "cancel")],
        };
        let pending = match mode {
            KeysMode::Normal | KeysMode::Command => pending,
            _ => String::new(),
        };
        Self {
            keymap: KeyMap::Vim,
            mode: Some(mode),
            pending,
            hints,
        }
    }

    /// FR-026: the status of the Nano keys: the search `prompt` while it is
    /// open, else the commands, with "End search" first while the hits of a
    /// search are `marked` (UC-008 BR-008).
    #[must_use]
    pub fn nano(prompt: Option<&str>, marked: bool) -> Self {
        let (pending, hints) = match prompt {
            Some(prompt) => (
                format!("Search: {prompt}"),
                vec![("Enter", "find"), ("^C", "cancel")],
            ),
            None => (
                String::new(),
                marked
                    .then_some(("Esc", "End search"))
                    .into_iter()
                    .chain([
                        ("^O", "Store"),
                        ("^X", "Close"),
                        ("^K", "Cut line"),
                        ("^U", "Paste"),
                        ("^W", "Where is"),
                        ("M-U", "Undo"),
                        ("M-E", "Redo"),
                        ("^A/^E", "Line start/end"),
                    ])
                    .collect(),
            ),
        };
        Self {
            keymap: KeyMap::Nano,
            mode: None,
            pending,
            hints,
        }
    }

    /// All of it as one line of text, where colors cannot be shown.
    #[must_use]
    pub fn text(&self) -> String {
        let mut parts = vec![self.keymap.label().to_owned()];
        parts.extend(self.mode.map(|mode| mode.label().to_owned()));
        if !self.pending.is_empty() {
            parts.push(self.pending.clone());
        }
        parts.extend(
            self.hints
                .iter()
                .map(|(keys, what)| format!("{keys} {what}")),
        );
        parts.join("  ")
    }
}

/// What a view gives the keys. Places are char indices of the note's text.
pub trait Target {
    /// The length of the text in chars.
    fn len(&self) -> usize;
    /// Whether the note has no text.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The char at `at`, if there is one.
    fn char_at(&self, at: usize) -> Option<char>;
    /// Cursor and anchor of the selection.
    fn cursor(&self) -> (usize, usize);
    /// Sets the cursor and the anchor of the selection.
    fn set_cursor(&mut self, cursor: usize, anchor: usize);
    /// One shown character to the left (the facade skips markup).
    fn left(&mut self, at: usize) -> usize;
    /// One shown character to the right (the facade skips markup).
    fn right(&mut self, at: usize) -> usize;
    /// One shown line up.
    fn up(&mut self, at: usize) -> usize;
    /// One shown line down.
    fn down(&mut self, at: usize) -> usize;
    /// The text of the line at `at`, without what ends it.
    fn line(&mut self, at: usize) -> Range<usize>;
    /// The line at `at` with what separates it from the next one.
    fn whole_line(&mut self, at: usize) -> Range<usize>;
    /// What separates the line at `at` from the next: a line break, or an
    /// empty line between blocks (a line break between list items).
    fn separator(&mut self, at: usize) -> &'static str;
    /// The text in `range`.
    fn text(&self, range: Range<usize>) -> String;
    /// Replaces the text in `range` with `text`.
    fn replace(&mut self, range: Range<usize>, text: &str);
    /// Undoes the last change.
    fn undo(&mut self);
    /// Redoes the last undone change.
    fn redo(&mut self);
    /// Starts a new line below or above the line at `at` and puts the
    /// cursor on it (a list continues with its marker in the facade).
    fn open_line(&mut self, at: usize, below: bool);
    /// The next place of `pattern` after `from`, or before it.
    fn find(&self, pattern: &str, from: usize, forward: bool) -> Option<usize>;
    /// The text on the clipboard, if it holds text.
    fn clipboard_read(&mut self) -> Option<String>;
    /// Puts `text` on the clipboard.
    fn clipboard_write(&mut self, text: &str);
    /// Asks the window for `command`.
    fn command(&mut self, command: EditorCommand);
}

/// The state of the keys of one tab, shared by its two views.
#[derive(Debug, Default)]
pub struct Keys {
    /// The key map in use.
    pub mode: InputMode,
    vim: Vim,
    nano: Nano,
}

impl Keys {
    /// Handles `events` in the input mode; gives back the events the view
    /// handles as usual (typing in insert mode, for example), unchanged and
    /// in order, and whether the text changed. `input` tells what an event
    /// is to the keys; `None` for events they do not know.
    pub fn handle<E>(
        &mut self,
        events: Vec<E>,
        input: impl Fn(&E) -> Option<KeyInput<'_>>,
        target: &mut dyn Target,
    ) -> (Vec<E>, bool) {
        match self.mode {
            InputMode::Normal => (events, false),
            InputMode::Vim => self.vim.handle(events, &input, target),
            InputMode::Nano => self.nano.handle(events, &input, target),
        }
    }

    /// UC-008 BR-008: the pattern of the last search of the Vim or Nano
    /// keys while its hits are marked, which the views mark like any
    /// search.
    pub fn search(&self) -> Option<&str> {
        match self.mode {
            InputMode::Normal => None,
            InputMode::Vim => self.vim.search.as_deref().filter(|_| self.vim.marked),
            InputMode::Nano => self.nano.search.as_deref().filter(|_| self.nano.marked),
        }
    }

    /// Whether the search prompt of the Nano keys is open: it takes every
    /// key and every text until Enter, Escape or Ctrl+C.
    #[must_use]
    pub fn prompting(&self) -> bool {
        self.mode == InputMode::Nano && self.nano.prompt.is_some()
    }

    /// FR-026: the active mode and its most important commands, for the
    /// editor's status line (UC-005).
    pub fn status(&self) -> Option<InputStatus> {
        match self.mode {
            InputMode::Normal => None,
            InputMode::Vim => Some(self.vim.status()),
            InputMode::Nano => Some(self.nano.status()),
        }
    }
}

// ---------------------------------------------------------------- Vim ----

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum VimMode {
    #[default]
    Normal,
    Insert,
    /// Visual mode from `start`; by lines with `V`.
    Visual {
        start: usize,
        lines: bool,
    },
    /// A command line: `:` or `/` and what is typed after it.
    Command(String),
}

#[derive(Debug, Clone, Default)]
struct Register {
    text: String,
    lines: bool,
}

#[derive(Debug, Default)]
struct Vim {
    mode: VimMode,
    /// The cursor in visual mode, which the selection shows inclusively.
    visual_cursor: usize,
    count: String,
    /// An operator waiting for its motion: `d`, `c`, `y`, with its count.
    operator: Option<(char, usize)>,
    /// A key waiting for the next: `g`, `r`, `"`, or `i`/`a` of a text
    /// object.
    pending: Option<char>,
    register: Register,
    /// `"+` was typed: the next yank or put uses the clipboard.
    clipboard: bool,
    search: Option<String>,
    /// UC-008 BR-008: whether the hits of `search` are marked; Escape or
    /// `:noh` ends the marking, the next search or `n` starts it again.
    marked: bool,
}

/// Where a motion leads, and how an operator takes it.
struct Motion {
    to: usize,
    lines: bool,
    inclusive: bool,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Class {
    Space,
    Word,
    Punctuation,
}

fn class(c: Option<char>) -> Class {
    match c {
        None => Class::Space,
        Some(c) if c.is_whitespace() => Class::Space,
        Some(c) if c.is_alphanumeric() || c == '_' => Class::Word,
        Some(_) => Class::Punctuation,
    }
}

impl Vim {
    fn handle<E>(
        &mut self,
        events: Vec<E>,
        input: &dyn Fn(&E) -> Option<KeyInput<'_>>,
        t: &mut dyn Target,
    ) -> (Vec<E>, bool) {
        let mut through = Vec::new();
        let mut changed = false;
        for event in events {
            if self.mode == VimMode::Insert {
                match input(&event) {
                    Some(KeyInput::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    }) => {
                        self.mode = VimMode::Normal;
                        let (cursor, _) = t.cursor();
                        let start = t.line(cursor).start;
                        let at = if cursor > start {
                            t.left(cursor)
                        } else {
                            cursor
                        };
                        t.set_cursor(at, at);
                    }
                    _ => through.push(event),
                }
                continue;
            }
            match input(&event) {
                Some(KeyInput::Text(text)) => {
                    for c in text.chars() {
                        changed |= self.char(c, t);
                    }
                }
                Some(KeyInput::Key {
                    key,
                    pressed: true,
                    modifiers,
                }) => changed |= self.key(key, modifiers, t),
                // Copying keeps working; pasting is `p`.
                Some(KeyInput::Copy) => through.push(event),
                _ => {}
            }
        }
        (through, changed)
    }

    fn key(&mut self, key: Key, modifiers: Modifiers, t: &mut dyn Target) -> bool {
        if let VimMode::Command(line) = &mut self.mode {
            match key {
                Key::Escape => self.mode = VimMode::Normal,
                Key::Backspace => {
                    line.pop();
                    if line.is_empty() {
                        self.mode = VimMode::Normal;
                    }
                }
                Key::Enter => {
                    let line = std::mem::take(line);
                    self.mode = VimMode::Normal;
                    return self.execute(&line, t);
                }
                _ => {}
            }
            return false;
        }
        match key {
            Key::Escape => {
                // UC-008 BR-008: Escape in normal mode, with nothing typed,
                // ends the search, so its hits are no longer marked.
                let idle = matches!(self.mode, VimMode::Normal)
                    && self.count.is_empty()
                    && self.operator.is_none()
                    && self.pending.is_none();
                if idle {
                    self.marked = false;
                }
                self.reset();
                if let VimMode::Visual { .. } = self.mode {
                    let at = self.visual_cursor;
                    t.set_cursor(at, at);
                }
                self.mode = VimMode::Normal;
                false
            }
            Key::R if modifiers.ctrl || modifiers.command => {
                for _ in 0..self.take_count() {
                    t.redo();
                }
                true
            }
            Key::ArrowLeft => self.char('h', t),
            Key::ArrowRight => self.char('l', t),
            Key::ArrowUp => self.char('k', t),
            Key::ArrowDown => self.char('j', t),
            _ => false,
        }
    }

    fn reset(&mut self) {
        self.count.clear();
        self.operator = None;
        self.pending = None;
        self.clipboard = false;
    }

    fn take_count(&mut self) -> usize {
        let n = self.count.parse().unwrap_or(1).max(1);
        self.count.clear();
        n
    }

    fn cursor(&self, t: &dyn Target) -> usize {
        match self.mode {
            VimMode::Visual { .. } => self.visual_cursor,
            _ => t.cursor().0,
        }
    }

    fn set(&mut self, at: usize, t: &mut dyn Target) {
        match self.mode {
            VimMode::Visual { start, lines } => {
                self.visual_cursor = at;
                if lines {
                    let (a, b) = (start.min(at), start.max(at));
                    let from = t.line(a).start;
                    let to = t.line(b).end;
                    t.set_cursor(to, from);
                } else if at >= start {
                    t.set_cursor((at + 1).min(t.len()), start);
                } else {
                    t.set_cursor(at, (start + 1).min(t.len()));
                }
            }
            _ => t.set_cursor(at, at),
        }
    }

    /// A typed character in normal or visual mode, or on the command line.
    fn char(&mut self, c: char, t: &mut dyn Target) -> bool {
        if let VimMode::Command(line) = &mut self.mode {
            line.push(c);
            return false;
        }
        let cursor = self.cursor(t);
        match self.pending.take() {
            Some('g') => {
                if c == 'g' {
                    let n = self.take_count();
                    let mut at = 0;
                    for _ in 1..n {
                        at = t.down(at);
                    }
                    return self.moved(
                        Motion {
                            to: at,
                            lines: true,
                            inclusive: false,
                        },
                        t,
                    );
                }
                self.reset();
                return false;
            }
            Some('r') => {
                self.count.clear();
                if cursor < t.len() && t.char_at(cursor).is_some_and(|ch| ch != '\n') {
                    t.replace(cursor..cursor + 1, &c.to_string());
                    t.set_cursor(cursor, cursor);
                    return true;
                }
                return false;
            }
            Some('"') => {
                self.clipboard = c == '+' || c == '*';
                return false;
            }
            Some(object @ ('i' | 'a')) => {
                if c == 'w' {
                    let range = self.word_object(cursor, object == 'a', t);
                    if let Some((op, _)) = self.operator.take() {
                        return self.operate(op, range, false, t);
                    }
                }
                self.reset();
                return false;
            }
            _ => {}
        }
        if c.is_ascii_digit() && !(c == '0' && self.count.is_empty()) {
            self.count.push(c);
            return false;
        }
        if self.operator.is_some() && matches!(c, 'i' | 'a') {
            self.pending = Some(c);
            return false;
        }
        // Keys that wait for the next; a count before them stays.
        if matches!(c, 'g' | 'r' | '"') {
            self.pending = Some(c);
            return false;
        }
        // Doubled operators work on lines: dd, cc, yy.
        if let Some((op, op_count)) = self.operator {
            if c == op {
                let n = self.take_count() * op_count;
                self.operator = None;
                let range = self.lines(cursor, n, t);
                return self.operate(op, range, true, t);
            }
        }
        let n = if self.count.is_empty() && self.operator.is_some() {
            1
        } else {
            self.take_count()
        };
        if let Some(motion) = self.motion(c, n, cursor, t) {
            return self.moved(motion, t);
        }
        let visual = matches!(self.mode, VimMode::Visual { .. });
        match c {
            'd' | 'c' | 'y' if visual => {
                let range = self.visual_range(t);
                let lines = matches!(self.mode, VimMode::Visual { lines: true, .. });
                self.mode = VimMode::Normal;
                self.operate(c, range, lines, t)
            }
            'x' if visual => self.char('d', t),
            'd' | 'c' | 'y' => {
                self.operator = Some((c, n));
                false
            }
            'D' | 'C' => {
                let end = t.line(cursor).end;
                self.operate(c.to_ascii_lowercase(), cursor..end, false, t)
            }
            'Y' => {
                let range = self.lines(cursor, n, t);
                self.operate('y', range, true, t)
            }
            'x' | 'X' => {
                let line = t.line(cursor);
                let range = if c == 'x' {
                    let mut end = cursor;
                    for _ in 0..n {
                        if end < line.end {
                            end = t.right(end).min(line.end).max(end + 1);
                        }
                    }
                    cursor..end
                } else {
                    let mut start = cursor;
                    for _ in 0..n {
                        if start > line.start {
                            start = t.left(start);
                        }
                    }
                    start..cursor
                };
                if range.is_empty() {
                    return false;
                }
                self.operate('d', range, false, t)
            }
            'p' | 'P' => self.put(c == 'p', n, t),
            'u' => {
                for _ in 0..n {
                    t.undo();
                }
                true
            }
            'i' => self.insert(cursor, t),
            'a' => {
                let line = t.line(cursor);
                let at = if cursor < line.end {
                    t.right(cursor)
                } else {
                    cursor
                };
                self.insert(at, t)
            }
            'I' => {
                let at = self.first_non_blank(cursor, t);
                self.insert(at, t)
            }
            'A' => {
                let at = t.line(cursor).end;
                self.insert(at, t)
            }
            'o' | 'O' => {
                t.open_line(cursor, c == 'o');
                self.mode = VimMode::Insert;
                true
            }
            'v' | 'V' => {
                let lines = c == 'V';
                self.mode = match self.mode {
                    VimMode::Visual { lines: l, .. } if l == lines => {
                        t.set_cursor(cursor, cursor);
                        VimMode::Normal
                    }
                    VimMode::Visual { start, .. } => VimMode::Visual { start, lines },
                    _ => VimMode::Visual {
                        start: cursor,
                        lines,
                    },
                };
                if let VimMode::Visual { .. } = self.mode {
                    self.set(cursor, t);
                }
                false
            }
            ':' | '/' => {
                self.mode = VimMode::Command(c.to_string());
                false
            }
            'n' | 'N' => {
                let Some(pattern) = self.search.clone() else {
                    return false;
                };
                self.marked = true;
                if let Some(at) = t.find(&pattern, cursor, c == 'n') {
                    self.set(at, t);
                }
                false
            }
            _ => {
                self.reset();
                false
            }
        }
    }

    fn insert(&mut self, at: usize, t: &mut dyn Target) -> bool {
        t.set_cursor(at, at);
        self.mode = VimMode::Insert;
        false
    }

    fn first_non_blank(&mut self, at: usize, t: &mut dyn Target) -> usize {
        let line = t.line(at);
        let mut pos = line.start;
        while pos < line.end && t.char_at(pos).is_some_and(|c| c == ' ' || c == '\t') {
            pos = t.right(pos);
        }
        pos
    }

    /// The motion of key `c`, `n` times from `from`.
    fn motion(&mut self, c: char, n: usize, from: usize, t: &mut dyn Target) -> Option<Motion> {
        let charwise = |to| Motion {
            to,
            lines: false,
            inclusive: false,
        };
        let mut at = from;
        Some(match c {
            'h' => {
                let start = t.line(from).start;
                for _ in 0..n {
                    if at > start {
                        at = t.left(at);
                    }
                }
                charwise(at)
            }
            'l' | ' ' => {
                let end = t.line(from).end;
                for _ in 0..n {
                    if at < end {
                        at = t.right(at).min(end);
                    }
                }
                charwise(at)
            }
            'j' | 'k' => {
                for _ in 0..n {
                    at = if c == 'j' { t.down(at) } else { t.up(at) };
                }
                Motion {
                    to: at,
                    lines: true,
                    inclusive: false,
                }
            }
            'w' | 'W' => {
                for _ in 0..n {
                    at = self.next_word(at, t);
                }
                // `cw` changes to the end of the word, as Vim does.
                if self.operator.is_some_and(|(op, _)| op == 'c') {
                    let mut end = from;
                    for _ in 0..n {
                        end = self.word_end(end, t);
                    }
                    return Some(Motion {
                        to: end,
                        lines: false,
                        inclusive: true,
                    });
                }
                // `dw` on the last word of a line keeps the line break.
                if self.operator.is_some() {
                    let end = t.line(from).end;
                    if at > end {
                        at = end;
                    }
                }
                charwise(at)
            }
            'b' | 'B' => {
                for _ in 0..n {
                    at = self.previous_word(at, t);
                }
                charwise(at)
            }
            'e' | 'E' => {
                for _ in 0..n {
                    at = self.word_end(at, t);
                }
                Motion {
                    to: at,
                    lines: false,
                    inclusive: true,
                }
            }
            '0' => charwise(t.line(from).start),
            '^' => charwise(self.first_non_blank(from, t)),
            '$' => {
                let line = t.line(from);
                let last = if line.end > line.start {
                    t.left(line.end).max(line.start)
                } else {
                    line.start
                };
                Motion {
                    to: last,
                    lines: false,
                    inclusive: line.end > line.start,
                }
            }
            'G' => {
                let to = if self.count_given(n) {
                    let mut at = 0;
                    for _ in 1..n {
                        at = t.down(at);
                    }
                    at
                } else {
                    // The last line, not the empty place after its break.
                    let end = t.len();
                    let last = if end > 0 && t.char_at(end - 1) == Some('\n') {
                        end - 1
                    } else {
                        end
                    };
                    t.line(last).start
                };
                Motion {
                    to,
                    lines: true,
                    inclusive: false,
                }
            }
            _ => return None,
        })
    }

    /// `G` with a count goes to that line; without, to the last.
    fn count_given(&self, n: usize) -> bool {
        n > 1
    }

    /// Applies a motion: moves the cursor, or hands the range to the
    /// waiting operator.
    fn moved(&mut self, motion: Motion, t: &mut dyn Target) -> bool {
        let from = self.cursor(t);
        if let Some((op, _)) = self.operator.take() {
            self.count.clear();
            if motion.lines {
                let (a, b) = (from.min(motion.to), from.max(motion.to));
                let start = t.whole_line(a).start;
                let mut end = t.whole_line(b).end;
                if end <= start {
                    end = t.whole_line(b).end;
                }
                let range = self.trim_last(start..end, t);
                return self.operate(op, range, true, t);
            }
            let range = if motion.to >= from {
                let end = if motion.inclusive {
                    (motion.to + 1).min(t.len())
                } else {
                    motion.to
                };
                from..end
            } else {
                motion.to..from
            };
            return self.operate(op, range, false, t);
        }
        self.set(motion.to, t);
        false
    }

    /// `n` lines from the one at `at`, with their separators.
    fn lines(&mut self, at: usize, n: usize, t: &mut dyn Target) -> Range<usize> {
        let start = t.whole_line(at).start;
        let mut end = t.whole_line(at).end;
        for _ in 1..n {
            if end >= t.len() {
                break;
            }
            end = t.whole_line(end).end;
        }
        self.trim_last(start..end, t)
    }

    /// The last lines of the note have no separator after them; deleting
    /// them takes the one before instead.
    fn trim_last(&mut self, range: Range<usize>, t: &mut dyn Target) -> Range<usize> {
        let ends_with_break = t.text(range.clone()).ends_with('\n');
        if range.end >= t.len() && range.start > 0 && !ends_with_break {
            let left = t.left(range.start);
            let before = t.line(left).end;
            return before..t.len();
        }
        range
    }

    /// `iw` and `aw`: the word at `at`, with the spaces after it for `aw`.
    fn word_object(&mut self, at: usize, around: bool, t: &mut dyn Target) -> Range<usize> {
        let kind = class(t.char_at(at));
        let line = t.line(at);
        let mut start = at;
        while start > line.start {
            let before = t.left(start);
            if class(t.char_at(before)) != kind {
                break;
            }
            start = before;
        }
        let mut end = at;
        while end < line.end && class(t.char_at(end)) == kind {
            end = t.right(end).min(line.end).max(end + 1);
        }
        if around {
            while end < line.end && class(t.char_at(end)) == Class::Space {
                end = t.right(end).min(line.end).max(end + 1);
            }
        }
        start..end
    }

    fn next_word(&mut self, at: usize, t: &mut dyn Target) -> usize {
        let len = t.len();
        let kind = class(t.char_at(at));
        let mut pos = at;
        // Over the rest of this word, then over the spaces after it.
        while pos < len && kind != Class::Space && class(t.char_at(pos)) == kind {
            let next = t.right(pos);
            if next == pos {
                return pos;
            }
            pos = next;
        }
        while pos < len && class(t.char_at(pos)) == Class::Space {
            let next = t.right(pos);
            if next == pos {
                return pos;
            }
            pos = next;
        }
        pos
    }

    fn previous_word(&mut self, at: usize, t: &mut dyn Target) -> usize {
        let mut pos = at;
        if pos == 0 {
            return 0;
        }
        pos = t.left(pos);
        while pos > 0 && class(t.char_at(pos)) == Class::Space {
            pos = t.left(pos);
        }
        let kind = class(t.char_at(pos));
        while pos > 0 {
            let before = t.left(pos);
            if class(t.char_at(before)) != kind {
                break;
            }
            pos = before;
        }
        pos
    }

    fn word_end(&mut self, at: usize, t: &mut dyn Target) -> usize {
        let len = t.len();
        let mut pos = t.right(at);
        while pos < len && class(t.char_at(pos)) == Class::Space {
            pos = t.right(pos);
        }
        let kind = class(t.char_at(pos));
        while pos < len {
            let next = t.right(pos);
            if next == pos || next >= len || class(t.char_at(next)) != kind {
                break;
            }
            pos = next;
        }
        pos.min(len.saturating_sub(1))
    }

    fn visual_range(&mut self, t: &mut dyn Target) -> Range<usize> {
        let VimMode::Visual { start, lines } = self.mode else {
            return 0..0;
        };
        let cursor = self.visual_cursor;
        let (a, b) = (start.min(cursor), start.max(cursor));
        if lines {
            let range = t.whole_line(a).start..t.whole_line(b).end;
            self.trim_last(range, t)
        } else {
            a..(b + 1).min(t.len())
        }
    }

    /// Applies operator `op` (delete, change, yank) to `range`.
    fn operate(&mut self, op: char, range: Range<usize>, lines: bool, t: &mut dyn Target) -> bool {
        self.count.clear();
        let text = t.text(range.clone());
        let stored = if lines {
            text.trim_matches('\n').to_owned()
        } else {
            text
        };
        if self.clipboard {
            t.clipboard_write(&stored);
            self.clipboard = false;
        } else {
            self.register = Register {
                text: stored,
                lines,
            };
        }
        self.mode = VimMode::Normal;
        match op {
            'y' => {
                t.set_cursor(range.start, range.start);
                false
            }
            'c' => {
                if lines {
                    // A changed line stays, empty, for typing.
                    let first = t.line(range.start.min(t.len()));
                    let content = first.start..range.end.min(t.len());
                    let keep_break = t.text(content.clone()).ends_with('\n');
                    let separator = t.separator(content.start);
                    t.replace(content.clone(), if keep_break { separator } else { "" });
                    t.set_cursor(content.start, content.start);
                } else {
                    t.replace(range.clone(), "");
                    t.set_cursor(range.start, range.start);
                }
                self.mode = VimMode::Insert;
                true
            }
            _ => {
                t.replace(range.clone(), "");
                let at = range.start.min(t.len());
                let at = if lines { t.line(at).start } else { at };
                t.set_cursor(at, at);
                true
            }
        }
    }

    /// `p` and `P`: the register after or before the cursor; lines below or
    /// above the line.
    fn put(&mut self, after: bool, n: usize, t: &mut dyn Target) -> bool {
        let register = if self.clipboard {
            self.clipboard = false;
            match t.clipboard_read() {
                Some(text) => Register {
                    lines: text.ends_with('\n'),
                    text: text.trim_end_matches('\n').to_owned(),
                },
                None => return false,
            }
        } else {
            self.register.clone()
        };
        if register.text.is_empty() {
            return false;
        }
        let cursor = t.cursor().0;
        let separator = t.separator(cursor);
        if register.lines {
            let block = vec![register.text.as_str(); n].join(separator);
            if after {
                let at = t.whole_line(cursor).end;
                if at >= t.len() && !t.text(0..t.len()).ends_with('\n') {
                    t.replace(at..at, &format!("{separator}{block}"));
                    let start = at + separator.chars().count();
                    t.set_cursor(start, start);
                } else if at >= t.len() {
                    // After a last line with a break: no empty line between
                    // lines of the source view, one between blocks.
                    let lead = &separator[1..];
                    t.replace(at..at, &format!("{lead}{block}\n"));
                    let start = at + lead.chars().count();
                    t.set_cursor(start, start);
                } else {
                    t.replace(at..at, &format!("{block}{separator}"));
                    t.set_cursor(at, at);
                }
            } else {
                let at = t.whole_line(cursor).start;
                t.replace(at..at, &format!("{block}{separator}"));
                t.set_cursor(at, at);
            }
        } else {
            let text = register.text.repeat(n);
            let line = t.line(cursor);
            let at = if after && cursor < line.end {
                t.right(cursor)
            } else {
                cursor
            };
            t.replace(at..at, &text);
            let end = at + text.chars().count();
            let last = end.saturating_sub(1).max(at);
            t.set_cursor(last, last);
        }
        true
    }

    /// `:w`, `:q`, `:wq`, `:x`, `/pattern`, and `:noh` (`:nohlsearch`),
    /// which ends the search (UC-008 BR-008).
    fn execute(&mut self, line: &str, t: &mut dyn Target) -> bool {
        if let Some(pattern) = line.strip_prefix('/') {
            if !pattern.is_empty() {
                self.search = Some(pattern.to_owned());
            }
            self.marked = self.search.is_some();
            let cursor = t.cursor().0;
            if let Some(at) = self.search.clone().and_then(|p| t.find(&p, cursor, true)) {
                t.set_cursor(at, at);
            }
            return false;
        }
        match line.trim_start_matches(':').trim() {
            "noh" | "nohl" | "nohlsearch" => self.marked = false,
            "w" => t.command(EditorCommand::Store),
            "q" | "q!" => t.command(EditorCommand::Close),
            "wq" | "x" => {
                t.command(EditorCommand::Store);
                t.command(EditorCommand::Close);
            }
            _ => {}
        }
        false
    }

    fn status(&self) -> InputStatus {
        let mode = match &self.mode {
            VimMode::Normal => KeysMode::Normal,
            VimMode::Insert => KeysMode::Insert,
            VimMode::Visual { lines: true, .. } => KeysMode::VisualLine,
            VimMode::Visual { lines: false, .. } => KeysMode::Visual,
            VimMode::Command(_) => KeysMode::Command,
        };
        let pending = match &self.mode {
            VimMode::Normal => self
                .count
                .chars()
                .chain(self.operator.map(|(op, _)| op))
                .chain(self.pending)
                .collect(),
            VimMode::Command(line) => line.clone(),
            _ => String::new(),
        };
        InputStatus::vim(mode, pending, self.search.is_some() && self.marked)
    }
}

// --------------------------------------------------------------- Nano ----

#[derive(Debug, Default)]
struct Nano {
    /// The lines cut, pasted with Ctrl+U; consecutive cuts add up.
    cut: String,
    cutting: bool,
    /// The search prompt of Ctrl+W, while it is open.
    prompt: Option<String>,
    search: Option<String>,
    /// UC-008 BR-008: whether the hits of `search` are marked; Escape or
    /// `:noh` ends the marking, the next search or `n` starts it again.
    marked: bool,
}

impl Nano {
    fn handle<E>(
        &mut self,
        events: Vec<E>,
        input: &dyn Fn(&E) -> Option<KeyInput<'_>>,
        t: &mut dyn Target,
    ) -> (Vec<E>, bool) {
        let mut through = Vec::new();
        let mut changed = false;
        // Alt with a letter may also arrive as text: it is dropped.
        let mut drop_text = false;
        for event in events {
            if let Some(prompt) = &mut self.prompt {
                match input(&event) {
                    Some(KeyInput::Text(text)) => prompt.push_str(text),
                    Some(KeyInput::Key {
                        key,
                        pressed: true,
                        modifiers,
                    }) => match key {
                        Key::Enter => {
                            let pattern = std::mem::take(prompt);
                            self.prompt = None;
                            if !pattern.is_empty() {
                                self.search = Some(pattern);
                            }
                            // Searching again marks the hits again.
                            self.marked = self.search.is_some();
                            let cursor = t.cursor().0;
                            if let Some(at) =
                                self.search.clone().and_then(|p| t.find(&p, cursor, true))
                            {
                                t.set_cursor(at, at);
                            }
                        }
                        Key::Escape => self.prompt = None,
                        Key::C if modifiers.ctrl => self.prompt = None,
                        Key::Backspace => {
                            prompt.pop();
                        }
                        _ => {}
                    },
                    _ => {}
                }
                continue;
            }
            let kind = input(&event);
            let Some(KeyInput::Key {
                key,
                pressed: true,
                modifiers,
            }) = kind
            else {
                if drop_text && matches!(kind, Some(KeyInput::Text(_))) {
                    continue;
                }
                if matches!(kind, Some(KeyInput::Text(_) | KeyInput::Paste)) {
                    self.cutting = false;
                }
                through.push(event);
                continue;
            };
            let (ctrl, alt) = (modifiers.ctrl || modifiers.mac_cmd, modifiers.alt);
            let cursor = t.cursor().0;
            let was_cutting = std::mem::take(&mut self.cutting);
            match (key, ctrl, alt) {
                (Key::K, true, false) => {
                    let range = t.whole_line(cursor);
                    let text = t.text(range.clone());
                    if was_cutting {
                        self.cut.push_str(&text);
                    } else {
                        self.cut = text;
                    }
                    let range =
                        if range.end >= t.len() && range.start > 0 && !self.cut.ends_with('\n') {
                            // The last line takes the break before it, and
                            // keeps one in what was cut.
                            let left = t.left(range.start);
                            let before = t.line(left).end;
                            self.cut.push('\n');
                            before..range.end
                        } else {
                            range
                        };
                    t.replace(range.clone(), "");
                    let at = t.line(range.start.min(t.len())).start;
                    t.set_cursor(at, at);
                    self.cutting = true;
                    changed = true;
                }
                (Key::U, true, false) => {
                    if !self.cut.is_empty() {
                        let at = t.whole_line(cursor).start;
                        t.replace(at..at, &self.cut);
                        let end = at + self.cut.chars().count();
                        t.set_cursor(end.min(t.len()), end.min(t.len()));
                        changed = true;
                    }
                }
                (Key::W, true, false) => self.prompt = Some(String::new()),
                // UC-008 BR-008: Escape ends the search and its marks.
                (Key::Escape, false, false) => self.marked = false,
                (Key::U, false, true) => {
                    t.undo();
                    changed = true;
                    drop_text = true;
                }
                (Key::E, false, true) => {
                    t.redo();
                    changed = true;
                    drop_text = true;
                }
                (Key::X, true, false) => t.command(EditorCommand::Close),
                (Key::O, true, false) | (Key::S, true, false) => t.command(EditorCommand::Store),
                (Key::A, true, false) => {
                    let at = t.line(cursor).start;
                    t.set_cursor(at, at);
                }
                (Key::E, true, false) => {
                    let at = t.line(cursor).end;
                    t.set_cursor(at, at);
                }
                _ => through.push(event),
            }
        }
        (through, changed)
    }

    fn status(&self) -> InputStatus {
        InputStatus::nano(self.prompt.as_deref(), self.search.is_some() && self.marked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note of plain lines in memory, as the source view treats it.
    #[derive(Default)]
    struct Lines {
        text: Vec<char>,
        cursor: usize,
        anchor: usize,
        undo: Vec<Vec<char>>,
        clipboard: Option<String>,
        commands: Vec<EditorCommand>,
    }

    impl Lines {
        fn new(text: &str) -> Self {
            Self {
                text: text.chars().collect(),
                ..Default::default()
            }
        }
        fn string(&self) -> String {
            self.text.iter().collect()
        }
    }

    impl Target for Lines {
        fn len(&self) -> usize {
            self.text.len()
        }
        fn char_at(&self, at: usize) -> Option<char> {
            self.text.get(at).copied()
        }
        fn cursor(&self) -> (usize, usize) {
            (self.cursor, self.anchor)
        }
        fn set_cursor(&mut self, cursor: usize, anchor: usize) {
            self.cursor = cursor.min(self.len());
            self.anchor = anchor.min(self.len());
        }
        fn left(&mut self, at: usize) -> usize {
            at.saturating_sub(1)
        }
        fn right(&mut self, at: usize) -> usize {
            (at + 1).min(self.len())
        }
        fn up(&mut self, at: usize) -> usize {
            let line = self.line(at);
            if line.start == 0 {
                return at;
            }
            let previous = self.line(line.start - 1);
            (previous.start + (at - line.start)).min(previous.end)
        }
        fn down(&mut self, at: usize) -> usize {
            let whole = self.whole_line(at);
            if whole.end >= self.len() {
                return at;
            }
            let next = self.line(whole.end);
            (next.start + (at - whole.start)).min(next.end)
        }
        fn line(&mut self, at: usize) -> Range<usize> {
            let at = at.min(self.len());
            let start = self.text[..at]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |i| i + 1);
            let end = self.text[at..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(self.len(), |i| at + i);
            start..end
        }
        fn whole_line(&mut self, at: usize) -> Range<usize> {
            let line = self.line(at);
            line.start..(line.end + 1).min(self.len())
        }
        fn separator(&mut self, _at: usize) -> &'static str {
            "\n"
        }
        fn text(&self, range: Range<usize>) -> String {
            self.text[range.start.min(self.len())..range.end.min(self.len())]
                .iter()
                .collect()
        }
        fn replace(&mut self, range: Range<usize>, text: &str) {
            self.undo.push(self.text.clone());
            self.text.splice(range.clone(), text.chars());
            self.cursor = range.start + text.chars().count();
            self.anchor = self.cursor;
        }
        fn undo(&mut self) {
            if let Some(text) = self.undo.pop() {
                self.text = text;
            }
        }
        fn redo(&mut self) {}
        fn open_line(&mut self, at: usize, below: bool) {
            let line = self.line(at);
            if below {
                self.replace(line.end..line.end, "\n");
            } else {
                self.replace(line.start..line.start, "\n");
                self.set_cursor(line.start, line.start);
            }
        }
        fn find(&self, pattern: &str, from: usize, _forward: bool) -> Option<usize> {
            let text = self.string();
            let start: usize = text.chars().take(from + 1).map(char::len_utf8).sum();
            let byte = text[start..]
                .find(pattern)
                .map(|i| start + i)
                .or_else(|| text.find(pattern))?;
            Some(text[..byte].chars().count())
        }
        fn clipboard_read(&mut self) -> Option<String> {
            self.clipboard.clone()
        }
        fn clipboard_write(&mut self, text: &str) {
            self.clipboard = Some(text.to_owned());
        }
        fn command(&mut self, command: EditorCommand) {
            self.commands.push(command);
        }
    }

    /// An event of a test: typed text or a key pressed.
    enum Ev {
        Text(String),
        Key(Key),
    }

    fn input(event: &Ev) -> Option<KeyInput<'_>> {
        Some(match event {
            Ev::Text(text) => KeyInput::Text(text),
            Ev::Key(key) => KeyInput::Key {
                key: *key,
                pressed: true,
                modifiers: Modifiers::default(),
            },
        })
    }

    fn run(text: &str, cursor: usize, spec: &str) -> Lines {
        let mut lines = Lines::new(text);
        lines.set_cursor(cursor, cursor);
        let mut keys = Keys {
            mode: InputMode::Vim,
            ..Keys::default()
        };
        let mut events = Vec::new();
        let mut rest = spec;
        while let Some(c) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("<Esc>") {
                events.push(Ev::Key(Key::Escape));
                rest = after;
                continue;
            }
            events.push(Ev::Text(c.to_string()));
            rest = &rest[c.len_utf8()..];
        }
        for event in events {
            let (through, _) = keys.handle(vec![event], input, &mut lines);
            // Insert mode: the view types what the keys let through.
            for event in through {
                if let Ev::Text(text) = event {
                    let at = lines.cursor;
                    lines.replace(at..at, &text);
                }
            }
        }
        lines
    }

    /// BR-010, BR-015: Nano lets through, unchanged and in order, what it
    /// does not handle; Alt+U undoes, and the text Alt+U may also send is
    /// dropped.
    #[test]
    fn uc005_br010_nano_lets_other_events_through() {
        #[derive(Debug, PartialEq)]
        enum Ev {
            Text(&'static str),
            Key(Key, Modifiers),
            Pointer(u8),
        }
        fn input(event: &Ev) -> Option<KeyInput<'_>> {
            match event {
                Ev::Text(text) => Some(KeyInput::Text(text)),
                Ev::Key(key, modifiers) => Some(KeyInput::Key {
                    key: *key,
                    pressed: true,
                    modifiers: *modifiers,
                }),
                Ev::Pointer(_) => None,
            }
        }
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        let mut lines = Lines::new("one\n");
        lines.replace(3..3, "!");
        let mut keys = Keys {
            mode: InputMode::Nano,
            ..Keys::default()
        };
        let (through, changed) = keys.handle(
            vec![
                Ev::Pointer(1),
                Ev::Text("a"),
                Ev::Key(Key::Other, Modifiers::default()),
                Ev::Key(Key::U, alt),
                Ev::Text("u"),
                Ev::Pointer(2),
            ],
            input,
            &mut lines,
        );
        assert!(changed);
        assert_eq!(lines.string(), "one\n");
        assert_eq!(
            through,
            vec![
                Ev::Pointer(1),
                Ev::Text("a"),
                Ev::Key(Key::Other, Modifiers::default()),
                Ev::Pointer(2),
            ]
        );
    }

    /// BR-009: counts, change, text objects, and the keys around them.
    #[test]
    fn uc005_br009_vim_commands() {
        assert_eq!(run("one two three\n", 0, "3x").string(), " two three\n");
        assert_eq!(run("one two three\n", 0, "d2w").string(), "three\n");
        assert_eq!(
            run("one two three\n", 4, "ciwTWO<Esc>").string(),
            "one TWO three\n"
        );
        assert_eq!(run("one two three\n", 4, "daw").string(), "one three\n");
        assert_eq!(run("one two three\n", 4, "D").string(), "one \n");
        assert_eq!(
            run("one\ntwo\nthree\n", 4, "ccnew<Esc>").string(),
            "one\nnew\nthree\n"
        );
        assert_eq!(run("abc\n", 1, "rX").string(), "aXc\n");
        assert_eq!(run("one\ntwo\n", 0, "oin<Esc>").string(), "one\nin\ntwo\n");
        assert_eq!(run("one\ntwo\n", 4, "Oin<Esc>").string(), "one\nin\ntwo\n");
        assert_eq!(run("  one\n", 4, "Ix<Esc>").string(), "  xone\n");
        assert_eq!(run("one\n", 0, "A!<Esc>").string(), "one!\n");
        assert_eq!(run("a\nb\nc\nd\n", 2, "Vjd").string(), "a\nd\n");
        assert_eq!(run("a\nb\nc\n", 0, "3Gx").string(), "a\nb\n\n");
        assert_eq!(run("one two\n", 0, "wvey$p").string(), "one twotwo\n");
        assert_eq!(run("a\nb\nc\n", 0, "Gdd").string(), "a\nb\n");
        // "+ uses the clipboard; the register stays as it was.
        let clip = run("one two\n", 0, "\"+yw");
        assert_eq!(clip.clipboard.as_deref(), Some("one "));
        let mut lines = run("x\n", 0, "u");
        lines.clipboard = Some("pasted".into());
        assert_eq!(lines.string(), "x\n");
    }

    /// FR-026: the status line of the Vim keys of another view (the
    /// window's `codemirror-vim`) reads as that of these keys.
    #[test]
    fn uc005_fr026_status_of_vim_and_nano_for_any_view() {
        let normal = InputStatus::vim(KeysMode::Normal, "2d".into(), false);
        assert_eq!(
            normal.text(),
            Keys {
                mode: InputMode::Vim,
                ..Keys::default()
            }
            .status()
            .unwrap()
            .text()
            .replacen("NORMAL", "NORMAL  2d", 1)
        );
        let marked = InputStatus::vim(KeysMode::Normal, String::new(), true);
        assert_eq!(marked.hints[0], ("Esc", "end search"));
        let visual = InputStatus::vim(KeysMode::Visual, "x".into(), true);
        assert_eq!(visual.pending, "");
        assert_eq!(visual.hints[0], ("d", "delete"));
        let command = InputStatus::vim(KeysMode::Command, ":wq".into(), false);
        assert_eq!(command.text(), "VIM  COMMAND  :wq  Enter run  Esc cancel");
        let prompt = InputStatus::nano(Some("ab"), true);
        assert_eq!(prompt.text(), "NANO  Search: ab  Enter find  ^C cancel");
        assert_eq!(
            InputStatus::nano(None, true).hints[0],
            ("Esc", "End search")
        );
    }

    /// M-11.3: the search prompt of the Nano keys takes every key.
    #[test]
    fn m11_3_nano_prompting() {
        let mut keys = Keys {
            mode: InputMode::Nano,
            ..Keys::default()
        };
        let mut lines = Lines::new("ab\n");
        assert!(!keys.prompting());
        let ctrl_w = KeyInput::Key {
            key: Key::W,
            pressed: true,
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
        };
        keys.handle(vec![ctrl_w], |e| Some(*e), &mut lines);
        assert!(keys.prompting());
        keys.mode = InputMode::Normal;
        assert!(!keys.prompting(), "only in the Nano mode");
    }
}
