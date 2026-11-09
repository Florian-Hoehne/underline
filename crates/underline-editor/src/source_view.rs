//! The source view (UC-005, ADR-012): the markup of a note, highlighted, with
//! only the visible lines laid out.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{
    self, text::CCursor, text::LayoutJob, Color32, Event, EventFilter, FontId, Galley, Key, Pos2,
    Rect, Sense, Stroke, TextFormat, Vec2,
};
use underline_core::{LineSide, NoteFormat, Query};
use underline_facade::highlight::{DocumentHighlight, HighlightSet, Span};

use crate::blink::Blink;
use crate::clipboard::{ClipContent, ClipImage, SharedClipboard};
use crate::document::Document;
use crate::keys::{EditorCommand, InputMode, InputStatus, Keys, Target};
use crate::minimap::{self, Mark};
use crate::Tool;
use underline_facade::blocks::{link_at, parse_blocks};
use underline_facade::clip::html_to_markup;
use underline_facade::formatting::{apply_format, Format};
use underline_facade::sections::{section_at, sections, Section, SectionKey};

const FONT_SIZE: f32 = 14.0;
/// UC-019 BR-006, UC-018 BR-006: the agent cursor: the lines an agent read
/// or wrote last, marked behind the text, with the agent's name.
pub const AGENT_COLOR: Color32 = Color32::from_rgba_premultiplied(30, 90, 170, 60);
/// The bar and the name of the agent cursor.
pub const AGENT_BAR_COLOR: Color32 = Color32::from_rgb(40, 120, 220);

/// UC-005: the place of the cursor, line and column counted from 1 in
/// chars of the markup, and the size of the selection: its chars and the
/// lines it touches (0 when nothing is selected).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPlace {
    /// The line of the cursor, from 1.
    pub line: usize,
    /// The column of the cursor in chars, from 1.
    pub column: usize,
    /// How many chars are selected.
    pub selected_chars: usize,
    /// How many lines the selection touches; a line it only reaches at its
    /// start does not count.
    pub selected_lines: usize,
}

impl CursorPlace {
    /// The place as the status line shows it: "Ln 3, Col 5", then what is
    /// selected, as "· 4 chars" within one line and "· 3 lines" across
    /// several.
    #[must_use]
    pub fn label(&self) -> String {
        let place = format!("Ln {}, Col {}", self.line, self.column);
        match (self.selected_chars, self.selected_lines) {
            (0, _) => place,
            (1, _) => format!("{place} · 1 char"),
            // Within one line: the chars.
            (n, 0 | 1) => format!("{place} · {n} chars"),
            (_, lines) => format!("{place} · {lines} lines"),
        }
    }
}

/// The agent cursor of one agent in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMark {
    /// The lines marked, counted from 0, the end not included.
    pub lines: Range<usize>,
    pub name: String,
}

/// The mark of a word found by a search (UC-008 BR-008), behind the text.
pub const FOUND_COLOR: Color32 = Color32::from_rgba_premultiplied(110, 85, 0, 110);
/// How often the minimap is drawn anew while the text changes.
pub(crate) const MARKS_EVERY: std::time::Duration = std::time::Duration::from_millis(150);
/// How often typing finds the sections again while nothing is folded.
const SECTIONS_EVERY: std::time::Duration = std::time::Duration::from_millis(300);
const PADDING: f32 = 6.0;
const MAX_CACHED_GALLEYS: usize = 4096;
/// Time a frame may spend on coloring lines (ADR-010); lines not colored yet
/// are shown plain and colored in the next frames.
const COLORING_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);

/// The source view of one tab.
pub struct SourceView {
    id: egui::Id,
    doc: Document,
    highlight: DocumentHighlight,
    /// Cursor and selection anchor, as char indices.
    cursor: usize,
    anchor: usize,
    /// The blink of the cursor (UC-005).
    blink: Blink,
    scroll_to_cursor: bool,
    focus: bool,
    galleys: HashMap<u64, Arc<Galley>>,
    /// Until when this frame may color lines.
    deadline: Option<std::time::Instant>,
    /// Whether a line was shown without its colors this frame.
    uncolored: bool,
    /// Rows each line takes when wrapped at `layout_width`: exact for lines
    /// laid out, estimated from the character count for the others.
    rows: Vec<u32>,
    /// Top of each line from the top of the text; one more entry for the end.
    tops: Vec<f32>,
    layout_width: f32,
    layout_revision: Option<u64>,
    pub(crate) row_height: f32,
    /// An edit since the last layout: its first line and the lines it added
    /// (negative when removed), so only these rows are estimated again.
    pending_edit: Option<(usize, isize)>,
    /// A toolbar function that asks the user first (link, image), for the
    /// window to answer (UC-005 A2).
    request: Option<Tool>,
    /// A link the user followed with Ctrl+click (UC-004 A2, BR-006).
    followed: Option<String>,
    /// The folded sections, shared with the facade of the tab (UC-004
    /// BR-012).
    folds: HashSet<SectionKey>,
    /// The sections of the text, the revision they were found in, and
    /// when they were found.
    sections: Option<(u64, Vec<Section>)>,
    sections_found: std::time::Instant,
    /// Counts the changes of the folded sections, so views lay out again.
    fold_version: u64,
    /// The fold version the line tops were computed for.
    tops_folds: u64,
    /// Whether the minimap is shown (FR-030), its bars, the part of the
    /// content in view, a scroll offset it asked for, and the characters
    /// a wrapped row holds.
    minimap: bool,
    marks: Vec<Mark>,
    /// The bars are older than the layout, and when they were built.
    marks_dirty: bool,
    marks_built: std::time::Instant,
    visible: (f32, f32),
    scroll_to: Option<f32>,
    columns: usize,
    /// Where the text was drawn last, on the screen: the top left of its
    /// first line, and the part of the screen it covers.
    pub(crate) drawn: Option<(Pos2, Rect)>,
    /// The clipboard (ADR-014); without one, copying and pasting use egui's
    /// plain text.
    clipboard: Option<SharedClipboard>,
    /// Copy on select (FR-031, BR-011): whether it is on, the selection put
    /// on the clipboard last, and what the clipboard held before.
    copy_on_select: bool,
    copied_selection: Option<(usize, usize)>,
    before_selection: Option<ClipContent>,
    /// The markup Underline put on the clipboard last; pasted as it is.
    own_markup: Option<String>,
    /// When a paste was handled, so the release of its key does nothing.
    pasted_at: Option<std::time::Instant>,
    /// Whether the press of `V` was seen, so its release is typing.
    paste_key_down: bool,
    /// A pasted image for the window to store next to the note (A18).
    pasted_image: Option<ClipImage>,
    /// The input mode and its state, shared with the facade (FR-024 to
    /// FR-026), and what its keys ask of the window.
    pub(crate) keys: Keys,
    commands: Vec<EditorCommand>,
    /// Whether line numbers are shown, and on which side (UC-010).
    line_numbers: (bool, LineSide),
    /// UC-008 BR-008: the search whose words are marked, and their places
    /// for the revision of the text they were found in.
    search: Option<Query>,
    found: (Option<u64>, Vec<Range<usize>>),
    /// UC-019 BR-006: the agent cursors shown.
    agent_marks: Vec<AgentMark>,
}

/// What a paste brings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pasted {
    /// Plain text, to be inserted as typed.
    Text(String),
    /// Markup of the note format, to be inserted as it is.
    Markup(String),
    Image(ClipImage),
    Nothing,
}

impl SourceView {
    pub fn new(id: impl Hash, text: &str, format: NoteFormat, set: Arc<HighlightSet>) -> Self {
        let doc = Document::new(text);
        let mut highlight = DocumentHighlight::new(set, format);
        highlight.invalidate_from(0, doc.len_lines());
        Self {
            id: egui::Id::new(id),
            doc,
            highlight,
            cursor: 0,
            anchor: 0,
            scroll_to_cursor: false,
            blink: Blink::default(),
            focus: false,
            galleys: HashMap::new(),
            deadline: None,
            uncolored: false,
            rows: Vec::new(),
            tops: vec![0.0],
            layout_width: 0.0,
            layout_revision: None,
            row_height: FONT_SIZE,
            pending_edit: None,
            request: None,
            followed: None,
            folds: HashSet::new(),
            sections: None,
            sections_found: std::time::Instant::now(),
            fold_version: 0,
            tops_folds: 0,
            minimap: true,
            marks: Vec::new(),
            marks_dirty: true,
            marks_built: std::time::Instant::now(),
            visible: (0.0, 0.0),
            scroll_to: None,
            columns: 80,
            drawn: None,
            clipboard: None,
            copy_on_select: true,
            copied_selection: None,
            before_selection: None,
            own_markup: None,
            pasted_at: None,
            paste_key_down: false,
            pasted_image: None,
            keys: Keys::default(),
            commands: Vec::new(),
            line_numbers: (true, LineSide::Left),
            search: None,
            found: (None, Vec::new()),
            agent_marks: Vec::new(),
        }
    }

    /// UC-018 BR-006, UC-019 BR-006: shows the agent cursors `marks`.
    pub fn set_agent_marks(&mut self, marks: Vec<AgentMark>) {
        self.agent_marks = marks;
    }

    /// The agent cursors shown, as char ranges of whole lines, with names.
    pub fn agent_mark_ranges(&self) -> Vec<(Range<usize>, String)> {
        let rope = self.doc.rope();
        let lines = rope.len_lines();
        self.agent_marks
            .iter()
            .filter(|m| m.lines.start < lines)
            .map(|m| {
                let end = m.lines.end.max(m.lines.start + 1).min(lines);
                (
                    rope.line_to_char(m.lines.start)..rope.line_to_char(end),
                    m.name.clone(),
                )
            })
            .collect()
    }

    /// UC-019 step 4, BR-007: an agent's change, as one undo step of its own;
    /// the cursor and the selection stay at the same place of the content,
    /// and the view does not scroll.
    pub fn apply_agent_edit(&mut self, range: Range<usize>, text: &str) {
        self.doc.seal();
        self.apply_edits(&[(range, text.to_owned())]);
        self.doc.seal();
    }

    /// UC-008 BR-008: marks every occurrence of the words of `query`, or
    /// none without one.
    pub fn set_search(&mut self, query: Option<Query>) {
        if self.search != query {
            self.search = query;
            self.found.0 = None;
        }
    }

    /// The occurrences of the search words in the text, as char ranges,
    /// sorted; worked out again when the text changed.
    pub fn found_ranges(&mut self) -> &[std::ops::Range<usize>] {
        let revision = self.doc.revision();
        if self.found.0 != Some(revision) {
            let ranges = match &self.search {
                Some(query) => {
                    let text = self.doc.text();
                    let rope = self.doc.rope();
                    query
                        .occurrences(&text)
                        .into_iter()
                        .map(|r| rope.byte_to_char(r.start)..rope.byte_to_char(r.end))
                        .collect()
                }
                None => Vec::new(),
            };
            self.found = (Some(revision), ranges);
        }
        &self.found.1
    }

    /// Selects the chars in `range` and scrolls them into view, as a place
    /// found (UC-008 step 4, A6).
    pub fn select_chars(&mut self, range: std::ops::Range<usize>) {
        let len = self.doc.len_chars();
        self.anchor = range.start.min(len);
        self.cursor = range.end.min(len);
        self.scroll_to_cursor = true;
    }

    /// The selection as char indices, start before end.
    pub fn selection_range(&self) -> std::ops::Range<usize> {
        let (start, end) = self.selection();
        start..end
    }

    /// Whether every visible line has its colors (ADR-010).
    pub fn is_colored(&self) -> bool {
        !self.uncolored
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// UC-005: where the cursor is in the markup and how much is selected,
    /// for the status line under the editor.
    #[must_use]
    pub fn cursor_place(&self) -> CursorPlace {
        let rope = self.doc.rope();
        let line = rope.char_to_line(self.cursor);
        let (start, end) = self.selection();
        let selected_lines = if start == end {
            0
        } else {
            // A selection of whole lines ends at the start of the next one,
            // which is not selected.
            rope.char_to_line(end - 1) - rope.char_to_line(start) + 1
        };
        CursorPlace {
            line: line + 1,
            column: self.cursor - rope.line_to_char(line) + 1,
            selected_chars: end - start,
            selected_lines,
        }
    }

    /// Puts the cursor at char index `at` and scrolls it into view.
    pub fn set_cursor(&mut self, at: usize) {
        self.cursor = at.min(self.doc.len_chars());
        self.anchor = self.cursor;
        self.scroll_to_cursor = true;
    }

    /// Shows `text` instead of the current text, after a change from outside
    /// (UC-004 A7). The cursor stays at the same line and column as far as
    /// the new text allows; the undo history starts anew (UC-005 BR-004).
    pub fn reload(&mut self, text: &str) {
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        let format = self.highlight.format();
        let set = self.highlight.set();
        self.doc = self.doc.replaced_by(text);
        self.highlight = DocumentHighlight::new(set, format);
        self.highlight.invalidate_from(0, self.doc.len_lines());
        self.galleys.clear();
        self.layout_revision = None;
        self.cursor = self.at_line(line, column);
        self.anchor = self.cursor;
    }

    /// UC-006 A7: replaces byte ranges of the text, sorted and not
    /// overlapping, as a rename changes the links of an open note. The cursor
    /// and the selection stay at the same place of the content, and the view
    /// does not scroll.
    pub fn apply_edits(&mut self, edits: &[(std::ops::Range<usize>, String)]) {
        let (mut cursor, mut anchor) = (self.cursor, self.anchor);
        for (range, text) in edits.iter().rev() {
            let rope = self.doc.rope();
            let start = rope.byte_to_char(range.start.min(rope.len_bytes()));
            let end = rope.byte_to_char(range.end.min(rope.len_bytes()));
            self.replace(start..end, text);
            let inserted = text.chars().count();
            let shift = |at: usize| {
                if at >= end {
                    at + inserted - (end - start)
                } else if at > start {
                    start + inserted.min(at - start)
                } else {
                    at
                }
            };
            cursor = shift(cursor);
            anchor = shift(anchor);
        }
        self.scroll_to_cursor = false;
        self.set_cursor_anchor(cursor, anchor);
    }

    /// Puts the cursor at the end of the text and the keyboard focus on the
    /// view, as for a new note (UC-003 step 6).
    pub fn focus_end(&mut self) {
        self.set_cursor(self.doc.len_chars());
        self.focus = true;
    }

    /// UC-005 A2: applies a toolbar function at the selection, in the markup
    /// of the note's format; link and image are asked for first and come
    /// back through [`Self::take_request`].
    pub fn apply_tool(&mut self, tool: Tool) -> bool {
        match tool.format() {
            Some(format) => self.apply_format(&format),
            None => {
                self.request = Some(tool);
                false
            }
        }
    }

    /// Applies `format` at the selection (UC-005 A2, BR-005).
    pub fn apply_format(&mut self, format: &Format) -> bool {
        let text = self.doc.text();
        let rope = self.doc.rope();
        let (start, end) = self.selection();
        let bytes = rope.char_to_byte(start)..rope.char_to_byte(end);
        let edit = apply_format(&text, self.highlight.format(), bytes, format);
        let (from, to) = (
            rope.byte_to_char(edit.range.start),
            rope.byte_to_char(edit.range.end),
        );
        self.replace(from..to, &edit.text);
        let rope = self.doc.rope();
        let selection = (
            rope.byte_to_char(edit.selection.start.min(rope.len_bytes())),
            rope.byte_to_char(edit.selection.end.min(rope.len_bytes())),
        );
        self.anchor = selection.0;
        self.cursor = selection.1;
        true
    }

    /// The link the user followed, for the window to open (UC-004 A2).
    pub fn take_followed_link(&mut self) -> Option<String> {
        self.followed.take()
    }

    pub(crate) fn follow(&mut self, link: String) {
        self.followed = Some(link);
    }

    /// Puts the cursor at source byte `offset` and scrolls to it, as when a
    /// link leads to a heading (UC-004 A2).
    pub fn jump_to_byte(&mut self, offset: usize) {
        let rope = self.doc.rope();
        let at = rope.byte_to_char(offset.min(rope.len_bytes()));
        self.set_cursor(at);
    }

    /// The note format of the text.
    pub fn document_format(&self) -> NoteFormat {
        self.highlight.format()
    }

    /// The toolbar function waiting for the user's answer, if any.
    pub fn take_request(&mut self) -> Option<Tool> {
        self.request.take()
    }

    /// Puts the keyboard focus on the view at the cursor, as after a switch
    /// of the editing view (UC-005 A1).
    pub fn focus_cursor(&mut self) {
        self.focus = true;
        self.scroll_to_cursor = true;
    }

    /// The sections of the text (UC-004 BR-012), found again when it
    /// changed. While nothing is folded, typing finds them again at most
    /// every [`SECTIONS_EVERY`], so typing stays fast in long notes
    /// (NFR-002); until then the arrows may stand a little off.
    pub(crate) fn sections(&mut self) -> &[Section] {
        let revision = self.doc.revision();
        let stale = self.sections.as_ref().is_none_or(|(r, _)| *r != revision);
        let due = self.sections.is_none()
            || !self.folds.is_empty()
            || self.sections_found.elapsed() >= SECTIONS_EVERY;
        if stale && due {
            let found = sections(&self.doc.text(), self.highlight.format());
            self.sections = Some((revision, found));
            self.sections_found = std::time::Instant::now();
        }
        &self.sections.as_ref().expect("sections").1
    }

    /// The text revision the known sections were found in.
    pub(crate) fn sections_revision(&self) -> Option<u64> {
        self.sections.as_ref().map(|(r, _)| *r)
    }

    /// Whether the known sections are older than the text, so the view
    /// must look again soon.
    fn sections_stale(&self) -> bool {
        self.sections
            .as_ref()
            .is_some_and(|(r, _)| *r != self.doc.revision())
    }

    /// The bytes hidden by folded sections, in order and not overlapping.
    pub(crate) fn hidden(&mut self) -> Vec<Range<usize>> {
        if self.folds.is_empty() {
            return Vec::new();
        }
        let folds = self.folds.clone();
        let mut hidden: Vec<Range<usize>> = Vec::new();
        for section in self.sections() {
            if !folds.contains(&section.key) || section.body.is_empty() {
                continue;
            }
            match hidden.last_mut() {
                Some(last) if section.body.start <= last.end => {
                    last.end = last.end.max(section.body.end);
                }
                _ => hidden.push(section.body.clone()),
            }
        }
        hidden
    }

    pub(crate) fn is_folded(&self, key: &SectionKey) -> bool {
        self.folds.contains(key)
    }

    /// Counts the changes of the folded sections.
    pub fn fold_version(&self) -> u64 {
        self.fold_version
    }

    /// UC-004 BR-012: folds or expands `section`. A cursor inside the hidden
    /// part moves to the end of the heading, so the section stays folded.
    pub(crate) fn toggle_fold(&mut self, section: &Section) {
        if !self.folds.remove(&section.key) {
            self.folds.insert(section.key.clone());
            let rope = self.doc.rope();
            let byte = rope.char_to_byte(self.cursor);
            if section.body.contains(&byte) {
                let end = section
                    .heading
                    .end
                    .saturating_sub(1)
                    .max(section.heading.start);
                self.cursor = rope.byte_to_char(end.min(rope.len_bytes()));
                self.anchor = self.cursor;
            }
        }
        self.fold_version += 1;
    }

    /// Ctrl+Shift+[ folds, Ctrl+Shift+] expands the section at the cursor
    /// (UC-004 BR-012, NFR-011).
    pub(crate) fn fold_key(&mut self, key: Key, modifiers: egui::Modifiers) -> bool {
        if !(modifiers.command && modifiers.shift) {
            return false;
        }
        let fold = match key {
            Key::OpenBracket => true,
            Key::CloseBracket => false,
            _ => return false,
        };
        let byte = self.doc.rope().char_to_byte(self.cursor);
        let Some(section) = section_at(self.sections(), byte).cloned() else {
            return true;
        };
        if fold != self.is_folded(&section.key) {
            self.toggle_fold(&section);
        }
        true
    }

    /// UC-004 BR-012: a folded section the cursor is in expands.
    pub(crate) fn unfold_at_cursor(&mut self) {
        if self.folds.is_empty() {
            return;
        }
        let byte = self.doc.rope().char_to_byte(self.cursor);
        let open: Vec<SectionKey> = self
            .sections()
            .iter()
            .filter(|s| s.body.start <= byte && byte < s.body.end)
            .map(|s| s.key.clone())
            .collect();
        let before = self.folds.len();
        for key in open {
            self.folds.remove(&key);
        }
        if self.folds.len() != before {
            self.fold_version += 1;
        }
    }

    /// Whether `line` is hidden by a folded section: it has no height.
    fn is_hidden_line(&self, line: usize) -> bool {
        line + 1 < self.tops.len()
            && self.tops[line + 1] == self.tops[line]
            && self.rows.get(line).is_some_and(|r| *r > 0)
    }

    /// The nearest line from `line` in the direction of `step` (1 or -1)
    /// that is shown, or `line` when there is none.
    fn visible_line(&self, line: usize, step: isize) -> usize {
        let mut at = line as isize;
        let lines = self.doc.len_lines() as isize;
        while at >= 0 && at < lines && self.is_hidden_line(at as usize) {
            at += step;
        }
        if at < 0 || at >= lines {
            line
        } else {
            at as usize
        }
    }

    /// Shows or hides the minimap (FR-030).
    pub fn set_minimap(&mut self, shown: bool) {
        self.minimap = shown;
    }

    /// Chooses the input mode: normal, Vim, or Nano (FR-024, FR-025).
    pub fn set_input_mode(&mut self, mode: InputMode) {
        self.keys.mode = mode;
    }

    /// FR-026: the active input mode and its most important commands, for
    /// the editor's status line (UC-005).
    pub fn input_status(&self) -> Option<InputStatus> {
        self.keys.status()
    }

    /// What the keys asked of the window: store at once, close the tab.
    pub fn take_commands(&mut self) -> Vec<EditorCommand> {
        std::mem::take(&mut self.commands)
    }

    pub(crate) fn push_command(&mut self, command: EditorCommand) {
        self.commands.push(command);
    }

    /// Searches `pattern` in the text after (or before) char `from`, going
    /// round at the end; gives its char index.
    pub(crate) fn find(&self, pattern: &str, from: usize, forward: bool) -> Option<usize> {
        let text = self.doc.text();
        let rope = self.doc.rope();
        let at = rope.char_to_byte((from + usize::from(forward)).min(rope.len_chars()));
        let byte = if forward {
            text[at..]
                .find(pattern)
                .map(|i| at + i)
                .or_else(|| text.find(pattern))
        } else {
            text[..rope.char_to_byte(from)]
                .rfind(pattern)
                .or_else(|| text.rfind(pattern))
        }?;
        Some(rope.byte_to_char(byte))
    }

    /// Shows the line numbers or not, and on which side (FR-028, FR-029).
    pub fn set_line_numbers(&mut self, shown: bool, side: LineSide) {
        if self.line_numbers != (shown, side) {
            self.line_numbers = (shown, side);
            self.layout_revision = None;
        }
    }

    /// Colors the text with `set`, the colors of another theme (UC-010
    /// BR-004); every line is colored again.
    pub fn set_highlight_set(&mut self, set: Arc<HighlightSet>) {
        let format = self.highlight.format();
        self.highlight = DocumentHighlight::new(set, format);
        self.highlight.invalidate_from(0, self.doc.len_lines());
        self.galleys.clear();
    }

    /// Uses `clipboard` for copying and pasting with formats (ADR-014).
    pub fn set_clipboard(&mut self, clipboard: SharedClipboard) {
        self.clipboard = Some(clipboard);
    }

    /// Switches copy on select on or off (FR-031, FR-032).
    pub fn set_copy_on_select(&mut self, on: bool) {
        self.copy_on_select = on;
    }

    /// Whether a clipboard with formats is set.
    pub(crate) fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    pub(crate) fn set_pasted_image(&mut self, image: ClipImage) {
        self.pasted_image = Some(image);
    }

    /// An image pasted from the clipboard, for the window to store next to
    /// the note and insert (UC-005 A18).
    pub fn take_pasted_image(&mut self) -> Option<ClipImage> {
        self.pasted_image.take()
    }

    /// UC-005 A17, A18, BR-011: what pasting brings now. Over the selection
    /// copy on select made, it is what the clipboard held before it. With
    /// `rich`, HTML comes as markup of the note format (the facade), and
    /// the markup Underline copied comes as it is; otherwise plain text.
    pub(crate) fn paste(&mut self, rich: bool) -> Pasted {
        self.pasted_at = Some(std::time::Instant::now());
        let Some(clipboard) = self.clipboard.clone() else {
            return Pasted::Nothing;
        };
        let selection = self.selection();
        let content = match (&self.before_selection, self.copied_selection) {
            (Some(before), Some(copied)) if copied == selection && selection.0 != selection.1 => {
                before.clone()
            }
            _ => clipboard.lock().expect("clipboard").read(),
        };
        if let Some(text) = &content.text {
            if rich && self.own_markup.as_deref() == Some(text.as_str()) {
                return Pasted::Markup(text.clone());
            }
        }
        match (content.html, content.text, content.image) {
            (Some(html), _, _) if rich => {
                let markup = html_to_markup(&html, self.highlight.format());
                if markup.is_empty() {
                    Pasted::Nothing
                } else {
                    Pasted::Markup(markup)
                }
            }
            (_, Some(text), _) => Pasted::Text(text),
            (_, None, Some(image)) => Pasted::Image(image),
            _ => Pasted::Nothing,
        }
    }

    /// Whether a paste happened that egui did not report: egui takes the
    /// press of the paste key for itself and reports a paste only when the
    /// clipboard holds text, so the release of `V` without its press is a
    /// paste of an image alone (UC-005 A18). A typed `v` shows its press.
    pub(crate) fn unreported_paste(&mut self, ui: &egui::Ui) -> bool {
        let mut unreported = false;
        let events = ui.input(|i| i.events.clone());
        let reported = events.iter().any(|e| matches!(e, Event::Paste(_)));
        for event in events {
            if let Event::Key {
                key: Key::V | Key::Paste,
                pressed,
                ..
            } = event
            {
                if pressed {
                    self.paste_key_down = true;
                } else {
                    unreported |= !self.paste_key_down && !reported;
                    self.paste_key_down = false;
                }
            }
        }
        unreported
            && self
                .pasted_at
                .is_none_or(|at| at.elapsed() > std::time::Duration::from_millis(500))
    }

    /// Puts the selection on the clipboard: `markup`, and `html` for other
    /// programs when given. Remembers the markup, to paste it as it is.
    pub(crate) fn copy(&mut self, markup: String, html: Option<String>) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let content = ClipContent {
            text: Some(markup.clone()),
            html,
            image: None,
        };
        clipboard.lock().expect("clipboard").write(&content);
        self.own_markup = Some(markup);
    }

    /// FR-031, BR-011: a finished selection goes to the clipboard at once;
    /// what the clipboard held before is kept for pasting over it. `html`
    /// makes the formatted text of the selection.
    pub(crate) fn copy_on_select(
        &mut self,
        ui: &egui::Ui,
        html: impl FnOnce(&mut Self) -> Option<String>,
    ) {
        let selection = self.selection();
        if selection.0 == selection.1 {
            self.copied_selection = None;
            self.before_selection = None;
            return;
        }
        let selecting = ui.input(|i| i.pointer.any_down());
        if !self.copy_on_select
            || self.clipboard.is_none()
            || selecting
            || self.copied_selection == Some(selection)
        {
            return;
        }
        if self.copied_selection.is_none() {
            let before = self
                .clipboard
                .as_ref()
                .map(|c| c.lock().expect("clipboard").read());
            self.before_selection = before;
        }
        let markup = self.selected_text();
        let html = html(self);
        self.copy(markup, html);
        self.copied_selection = Some(selection);
    }

    /// Draws the view with its minimap and handles their input. Returns
    /// whether the text changed.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let (editor, strip) = minimap::split(ui, self.minimap);
        let changed = ui
            .allocate_new_ui(egui::UiBuilder::new().max_rect(editor), |ui| {
                self.show_editor(ui)
            })
            .inner;
        if let Some(strip) = strip {
            if self.marks_dirty {
                if self.marks.is_empty() || self.marks_built.elapsed() >= MARKS_EVERY {
                    self.build_marks();
                } else {
                    ui.ctx().request_repaint_after(MARKS_EVERY);
                }
            }
            let content = self.tops.last().copied().unwrap_or(0.0) + 2.0 * PADDING;
            let (_, background) = self.highlight.colors();
            if let Some(offset) = minimap::show(
                ui,
                self.id,
                strip,
                &self.marks,
                content,
                self.visible,
                color(background).gamma_multiply(0.9),
            ) {
                self.scroll_to = Some(offset);
                ui.ctx().request_repaint();
            }
        }
        changed
    }

    /// Draws the view itself.
    fn show_editor(&mut self, ui: &mut egui::Ui) -> bool {
        let font = FontId::monospace(FONT_SIZE);
        let row_height = ui.fonts(|f| f.row_height(&font));
        let char_width = ui.fonts(|f| f.glyph_width(&font, 'x'));
        let (foreground, background) = self.highlight.colors();
        let foreground = color(foreground);
        let numbers = foreground.gamma_multiply(0.45);
        let mut changed = false;
        self.deadline = Some(std::time::Instant::now() + COLORING_BUDGET);
        self.uncolored = false;
        self.unfold_at_cursor();

        let mut area = egui::ScrollArea::vertical()
            .id_salt(self.id.with("scroll"))
            .auto_shrink(false);
        if let Some(offset) = self.scroll_to.take() {
            area = area.vertical_scroll_offset(offset);
        }
        area.show_viewport(ui, |ui, viewport| {
            self.visible = (viewport.min.y, viewport.max.y);
            let lines = self.doc.len_lines();
            // Line numbers left or right, or none (FR-028, FR-029); the fold
            // arrows stay on the left.
            let digits = lines.max(1).ilog10() as f32 + 1.0;
            let number_width = if self.line_numbers.0 {
                digits * char_width + PADDING
            } else {
                0.0
            };
            let right = self.line_numbers.1 == LineSide::Right;
            let gutter = PADDING + FOLD_ARROW + if right { 0.0 } else { number_width };
            let width = ui.available_width();
            let right_room = if right { number_width } else { 0.0 };
            let wrap_width = (width - gutter - right_room - 2.0 * PADDING).max(char_width * 8.0);
            self.update_layout(wrap_width, char_width, row_height);
            if self.tops_folds != self.fold_version {
                self.recompute_tops();
            }

            let size = Vec2::new(width, self.tops[lines] + 2.0 * PADDING);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let response = ui.interact(rect, self.id, Sense::click_and_drag());
            let origin = rect.min + Vec2::new(gutter, PADDING);
            self.drawn = Some((
                origin,
                rect.intersect(viewport.translate(rect.min.to_vec2())),
            ));
            if std::mem::take(&mut self.focus) {
                response.request_focus();
            }

            // Pointer: place the cursor, extend the selection by dragging.
            // UC-004 BR-006: Ctrl+click (Cmd+click) follows a link.
            if response.clicked() && ui.input(|i| i.modifiers.command) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let at = self.char_at(ui, &font, origin, pos);
                    let byte = self.doc.rope().char_to_byte(at);
                    let blocks = parse_blocks(&self.doc.text(), self.highlight.format());
                    if let Some(link) = link_at(&blocks, byte) {
                        self.followed = Some(link);
                    }
                }
            }
            if response.is_pointer_button_down_on() || response.clicked() {
                response.request_focus();
                if let Some(pos) = response.interact_pointer_pos() {
                    let at = self.char_at(ui, &font, origin, pos);
                    let extend = response.dragged() || ui.input(|i| i.modifiers.shift);
                    self.cursor = at;
                    if !extend {
                        self.anchor = at;
                    }
                }
            }

            if response.has_focus() {
                let vim_escape = self.keys.mode == InputMode::Vim;
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        self.id,
                        EventFilter {
                            tab: true,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            // Vim needs Esc for itself (BR-009).
                            escape: vim_escape,
                        },
                    )
                });
                let page = (viewport.height() / row_height).floor().max(1.0) as usize;
                changed = self.handle_keys(ui, page);
                self.unfold_at_cursor();
                if changed {
                    self.update_layout(wrap_width, char_width, row_height);
                }
                if self.tops_folds != self.fold_version {
                    self.recompute_tops();
                }
            }

            let painter = ui.painter_at(rect.intersect(viewport.translate(rect.min.to_vec2())));
            painter.rect_filled(painter.clip_rect(), 0.0, color(background));
            let lines = self.doc.len_lines();
            let first = self.line_at(viewport.min.y - PADDING);
            let last = (self.line_at(viewport.max.y - PADDING) + 1).min(lines);
            let (selection_start, selection_end) = self.selection();
            let selection_color = ui.visuals().selection.bg_fill;
            let found = self.found_ranges().to_vec();
            let mut exact_changed = false;
            // UC-004 BR-012: the arrows of the section headings shown.
            let heading_lines: HashMap<usize, Section> = {
                let rope = self.doc.rope().clone();
                self.sections()
                    .iter()
                    .map(|s| (rope.byte_to_line(s.heading.start), s.clone()))
                    .filter(|(line, _)| (first..last).contains(line))
                    .collect()
            };
            let pointer = ui.input(|i| i.pointer.hover_pos());
            let mut toggle = None;
            for line in first..last {
                if self.is_hidden_line(line) {
                    continue;
                }
                if let Some(section) = heading_lines.get(&line) {
                    let top = origin.y + self.tops[line];
                    let arrow = Rect::from_min_size(
                        Pos2::new(rect.min.x + 2.0, top),
                        Vec2::new(FOLD_ARROW - 2.0, row_height),
                    );
                    let folded = self.is_folded(&section.key);
                    let hovered = pointer
                        .is_some_and(|p| p.y >= top && p.y < top + row_height && rect.contains(p));
                    let click = ui.interact(arrow, self.id.with(("fold", line)), Sense::click());
                    if folded || hovered || click.hovered() {
                        paint_fold_arrow(&painter, arrow, folded, numbers);
                    }
                    if click.clicked() {
                        toggle = Some(section.clone());
                    }
                }
                let galley = self.galley(ui, &font, line);
                let rows = galley.rows.len().max(1) as u32;
                if self.rows[line] != rows {
                    self.rows[line] = rows;
                    exact_changed = true;
                }
                let top = origin + Vec2::new(0.0, self.tops[line]);
                if self.line_numbers.0 {
                    let x = if right {
                        rect.right() - PADDING
                    } else {
                        origin.x - PADDING
                    };
                    painter.text(
                        Pos2::new(x, top.y),
                        egui::Align2::RIGHT_TOP,
                        (line + 1).to_string(),
                        font.clone(),
                        numbers,
                    );
                }
                // Selection and found words behind the text, row by row.
                let line_start = self.doc.rope().line_to_char(line);
                let line_len = self.doc.line_text(line).chars().count();
                let paint_span = |from: usize, to: usize, fill: Color32| {
                    let from = from.max(line_start);
                    let to = to.min(line_start + line_len + 1);
                    if from >= to {
                        return;
                    }
                    let start = galley.pos_from_ccursor(CCursor::new(from - line_start));
                    let end = if to > line_start + line_len {
                        let last = galley.pos_from_ccursor(CCursor::new(line_len));
                        Rect::from_min_size(last.min + Vec2::new(6.0, 0.0), last.size())
                    } else {
                        galley.pos_from_ccursor(CCursor::new(to - line_start))
                    };
                    let row_of = |y: f32| (y / row_height).round() as usize;
                    let (first_row, last_row) = (row_of(start.min.y), row_of(end.min.y));
                    for row in first_row..=last_row {
                        let x0 = if row == first_row { start.min.x } else { 0.0 };
                        let x1 = if row == last_row {
                            end.min.x
                        } else {
                            galley
                                .rows
                                .get(row)
                                .map_or(wrap_width, |r| r.rect.max.x + 6.0)
                        };
                        let y = row as f32 * row_height;
                        painter.rect_filled(
                            Rect::from_min_max(
                                top + Vec2::new(x0, y),
                                top + Vec2::new(x1, y + row_height),
                            ),
                            0.0,
                            fill,
                        );
                    }
                };
                // UC-008 BR-008: the words of the search, marked.
                let first_found = found.partition_point(|r| r.end <= line_start);
                for range in found[first_found..]
                    .iter()
                    .take_while(|r| r.start <= line_start + line_len)
                {
                    paint_span(range.start, range.end, FOUND_COLOR);
                }
                if selection_start != selection_end {
                    paint_span(selection_start, selection_end, selection_color);
                }
                // UC-019 BR-006: the agent cursor behind the lines.
                if let Some(mark) = self.agent_marks.iter().find(|m| {
                    m.lines.contains(&line) || (m.lines.is_empty() && m.lines.start == line)
                }) {
                    let height = rows as f32 * row_height;
                    painter.rect_filled(
                        Rect::from_min_size(top, Vec2::new(wrap_width, height)),
                        0.0,
                        AGENT_COLOR,
                    );
                    painter.line_segment(
                        [top + Vec2::new(-3.0, 0.0), top + Vec2::new(-3.0, height)],
                        Stroke::new(3.0_f32, AGENT_BAR_COLOR),
                    );
                    if line == mark.lines.start || line == first {
                        painter.text(
                            top + Vec2::new(galley.rect.right() + 16.0, 0.0),
                            egui::Align2::LEFT_TOP,
                            &mark.name,
                            egui::FontId::proportional(font.size * 0.8),
                            AGENT_BAR_COLOR,
                        );
                    }
                }
                let folded_heading = heading_lines
                    .get(&line)
                    .is_some_and(|s| self.is_folded(&s.key));
                if folded_heading {
                    let end = galley.rect.right_bottom();
                    painter.text(
                        top + Vec2::new(end.x + 8.0, end.y),
                        egui::Align2::LEFT_BOTTOM,
                        "…",
                        font.clone(),
                        numbers,
                    );
                }
                painter.galley(top, galley, foreground);
            }
            if let Some(section) = toggle {
                self.toggle_fold(&section);
                self.recompute_tops();
                ui.ctx().request_repaint();
            }
            if exact_changed {
                self.recompute_tops();
                ui.ctx().request_repaint();
            }

            // Cursor.
            let cursor_rect = self.cursor_rect(ui, &font, origin, row_height);
            if response.has_focus() {
                if self.blink.shown(ui, (self.cursor, self.anchor)) {
                    painter.line_segment(
                        [cursor_rect.center_top(), cursor_rect.center_bottom()],
                        Stroke::new(2.0_f32, foreground),
                    );
                }
                ui.ctx()
                    .output_mut(|o| o.ime = Some(egui::output::IMEOutput { rect, cursor_rect }));
            }
            if std::mem::take(&mut self.scroll_to_cursor) {
                ui.scroll_to_rect(cursor_rect.expand(row_height), None);
            }
        });
        // FR-031: the source view copies the markup of a selection.
        self.copy_on_select(ui, |_| None);
        if self.uncolored {
            ui.ctx().request_repaint();
        }
        if self.sections_stale() {
            ui.ctx().request_repaint_after(SECTIONS_EVERY);
        }
        changed
    }

    /// Estimates the rows of every line for `wrap_width` when the width or
    /// the text changed; lines laid out later correct their estimate.
    fn update_layout(&mut self, wrap_width: f32, char_width: f32, row_height: f32) {
        let revision = self.doc.revision();
        let same_width = (self.layout_width - wrap_width).abs() < 0.5;
        if self.layout_revision == Some(revision) && same_width {
            return;
        }
        let columns = (wrap_width / char_width).floor().max(1.0) as usize;
        self.columns = columns;
        let rope = self.doc.rope();
        let lines = self.doc.len_lines();
        let estimate = |line: usize| {
            let chars = rope.line(line).len_chars().saturating_sub(1);
            chars.div_ceil(columns).max(1) as u32
        };
        match self.pending_edit.take() {
            // Only the edited lines are estimated again; the others shift.
            Some((line, added)) if same_width && self.layout_revision.is_some() => {
                let line = line.min(self.rows.len());
                if added > 0 {
                    let at = (line + 1).min(self.rows.len());
                    self.rows
                        .splice(at..at, std::iter::repeat_n(1, added as usize));
                } else if added < 0 {
                    let from = (line + 1).min(self.rows.len());
                    let to = (from + added.unsigned_abs()).min(self.rows.len());
                    self.rows.drain(from..to);
                }
                self.rows.resize(lines, 1);
                let last = (line as isize + added.max(0)) as usize;
                for l in line..=last.min(lines.saturating_sub(1)) {
                    self.rows[l] = estimate(l);
                }
            }
            _ => {
                self.rows.clear();
                self.rows.extend((0..lines).map(estimate));
            }
        }
        self.layout_width = wrap_width;
        self.layout_revision = Some(revision);
        self.row_height = row_height;
        self.recompute_tops();
    }

    /// Remembers an edit at `line` that added `added` lines, for the next
    /// layout; a second edit before it falls back to a full estimate.
    fn note_edit(&mut self, line: usize, added: isize) {
        if self.pending_edit.is_some() || self.layout_revision.is_none() {
            self.pending_edit = None;
            self.layout_revision = None;
        } else {
            self.pending_edit = Some((line, added));
        }
    }

    fn recompute_tops(&mut self) {
        // Lines of folded sections take no room (UC-004 BR-012).
        let hidden_lines: Vec<Range<usize>> = {
            let hidden = self.hidden();
            let rope = self.doc.rope();
            hidden
                .iter()
                .filter(|r| !r.is_empty())
                .map(|r| {
                    let first = rope.byte_to_line(r.start.min(rope.len_bytes()));
                    let last = rope.byte_to_line((r.end - 1).min(rope.len_bytes()));
                    first..last + 1
                })
                .collect()
        };
        self.tops_folds = self.fold_version;
        self.marks_dirty = true;
        self.tops.clear();
        self.tops.reserve(self.rows.len() + 1);
        let mut y = 0.0;
        let mut hidden = hidden_lines.iter().peekable();
        for (line, rows) in self.rows.iter().enumerate() {
            self.tops.push(y);
            while hidden.peek().is_some_and(|r| r.end <= line) {
                hidden.next();
            }
            if hidden.peek().is_some_and(|r| r.contains(&line)) {
                continue;
            }
            y += *rows as f32 * self.row_height;
        }
        self.tops.push(y);
    }

    /// FR-030: the bars of the minimap, one per shown line, as long as the
    /// line, headings stronger.
    fn build_marks(&mut self) {
        let heading_lines: HashSet<usize> = {
            let rope = self.doc.rope().clone();
            self.sections()
                .iter()
                .map(|s| rope.byte_to_line(s.heading.start))
                .collect()
        };
        let (foreground, _) = self.highlight.colors();
        let (text, strong) = (color(foreground).gamma_multiply(0.4), color(foreground));
        let mut marks = Vec::with_capacity(self.rows.len());
        let rope = self.doc.rope();
        for (line, slice) in rope
            .lines()
            .enumerate()
            .take(self.tops.len().saturating_sub(1))
        {
            let height = self.tops[line + 1] - self.tops[line];
            let chars = slice.len_chars().saturating_sub(1);
            if height > 0.0 && chars > 0 {
                marks.push(Mark {
                    top: PADDING + self.tops[line],
                    height: height * 0.7,
                    width: (chars as f32 / self.columns.max(1) as f32).min(1.0),
                    color: if heading_lines.contains(&line) {
                        strong
                    } else {
                        text
                    },
                });
            }
        }
        self.marks = marks;
        self.marks_dirty = false;
        self.marks_built = std::time::Instant::now();
    }

    /// The line at height `y` from the top of the text.
    fn line_at(&self, y: f32) -> usize {
        let lines = self.rows.len();
        self.tops[..lines]
            .partition_point(|top| *top <= y)
            .saturating_sub(1)
            .min(lines.saturating_sub(1))
    }

    /// Applies the keys, text, and clipboard events of this frame.
    fn handle_keys(&mut self, ui: &mut egui::Ui, page: usize) -> bool {
        let raw_events = ui.input(|i| i.events.clone());
        let mut changed = false;
        // FR-024, FR-025: each event goes to the Vim or Nano keys first, in
        // order; what they leave is handled as usual right after.
        for raw in raw_events {
            let events = if self.keys.mode == InputMode::Normal {
                vec![raw]
            } else {
                let mut keys = std::mem::take(&mut self.keys);
                let (rest, edited) = keys.handle(vec![raw], &mut SourceTarget(self));
                self.keys = keys;
                changed |= edited;
                self.scroll_to_cursor = true;
                rest
            };
            for event in events {
                match event {
                    Event::Text(text) => {
                        self.insert(&text);
                        changed = true;
                    }
                    // UC-005 A17, BR-014: the source view pastes plain text.
                    Event::Paste(text) => {
                        match self.clipboard.is_some().then(|| self.paste(false)) {
                            Some(Pasted::Text(text) | Pasted::Markup(text)) => self.insert(&text),
                            Some(Pasted::Image(image)) => self.pasted_image = Some(image),
                            Some(Pasted::Nothing) => {}
                            None => self.insert(&text),
                        }
                        changed = true;
                    }
                    Event::Ime(egui::ImeEvent::Commit(text)) => {
                        self.insert(&text);
                        changed = true;
                    }
                    Event::Copy | Event::Cut => {
                        let text = self.selected_text();
                        if !text.is_empty() {
                            if self.clipboard.is_some() {
                                self.copy(text, None);
                            } else {
                                ui.ctx().copy_text(text);
                            }
                            if event == Event::Cut {
                                self.insert("");
                                changed = true;
                            }
                        }
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        changed |= self.key(key, modifiers, page);
                    }
                    _ => {}
                }
            }
        }
        // UC-005 A18: a paste egui did not report, as of an image alone.
        if self.clipboard.is_some() && self.unreported_paste(ui) {
            match self.paste(false) {
                Pasted::Image(image) => self.pasted_image = Some(image),
                Pasted::Text(text) | Pasted::Markup(text) => {
                    self.insert(&text);
                    changed = true;
                }
                Pasted::Nothing => {}
            }
        }
        changed
    }

    fn key(&mut self, key: Key, modifiers: egui::Modifiers, page: usize) -> bool {
        if self.fold_key(key, modifiers) {
            return false;
        }
        if let Some(tool) = Tool::for_key(key, modifiers) {
            return self.apply_tool(tool);
        }
        let rope_len = self.doc.len_chars();
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        let mut target = None;
        match key {
            Key::Backspace => {
                if self.cursor == self.anchor && self.cursor > 0 {
                    self.anchor = self.cursor - 1;
                }
                self.insert("");
                return true;
            }
            Key::Delete => {
                if self.cursor == self.anchor && self.cursor < rope_len {
                    self.anchor = self.cursor + 1;
                }
                self.insert("");
                return true;
            }
            Key::Enter => {
                self.insert("\n");
                return true;
            }
            Key::Tab => {
                self.insert("    ");
                return true;
            }
            Key::Z if modifiers.command && modifiers.shift => return self.redo(),
            Key::Z if modifiers.command => return self.undo(),
            Key::Y if modifiers.command => return self.redo(),
            Key::A if modifiers.command => {
                self.anchor = 0;
                self.cursor = rope_len;
                return false;
            }
            Key::ArrowLeft => target = Some(self.cursor.saturating_sub(1)),
            Key::ArrowRight => target = Some((self.cursor + 1).min(rope_len)),
            // Folded lines are stepped over.
            Key::ArrowUp => {
                let up = self.visible_line(line.saturating_sub(1), -1);
                target = Some(self.at_line(up, column));
            }
            Key::ArrowDown => {
                let down = self.visible_line(line + 1, 1);
                target = Some(self.at_line(down, column));
            }
            Key::PageUp => target = Some(self.at_line(line.saturating_sub(page), column)),
            Key::PageDown => target = Some(self.at_line(line + page, column)),
            Key::Home if modifiers.command => target = Some(0),
            Key::End if modifiers.command => target = Some(rope_len),
            Key::Home => target = Some(self.doc.rope().line_to_char(line)),
            Key::End => target = Some(self.at_line(line, usize::MAX)),
            _ => {}
        }
        if let Some(at) = target {
            self.cursor = at;
            if !modifiers.shift {
                self.anchor = at;
            }
            self.scroll_to_cursor = true;
        }
        false
    }

    /// The char index at `column` of `line`, clamped to the line.
    fn at_line(&self, line: usize, column: usize) -> usize {
        let line = line.min(self.doc.len_lines().saturating_sub(1));
        let length = self.doc.line_text(line).chars().count();
        self.doc.rope().line_to_char(line) + column.min(length)
    }

    pub(crate) fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    pub(crate) fn selected_text(&self) -> String {
        let (start, end) = self.selection();
        self.doc.rope().slice(start..end).to_string()
    }

    /// Replaces the selection by `text` (UC-005 step 1).
    fn insert(&mut self, text: &str) {
        let (start, end) = self.selection();
        self.replace(start..end, text);
    }

    /// Inserts `text` as it is at the cursor, replacing the selection, as a
    /// dropped file's name in a code block (UC-005 A9 step 2).
    pub fn insert_text(&mut self, text: &str) {
        self.insert(text);
    }

    /// UC-005 A9: puts the cursor at the place under `pos` on the screen,
    /// where something is dropped. False when `pos` is not over the text.
    pub fn place_cursor_at(&mut self, ui: &egui::Ui, pos: Pos2) -> bool {
        let Some((origin, area)) = self.drawn else {
            return false;
        };
        if !area.contains(pos) {
            return false;
        }
        let font = FontId::monospace(FONT_SIZE);
        let at = self.char_at(ui, &font, origin, pos);
        self.cursor = at;
        self.anchor = at;
        true
    }

    /// Replaces the chars in `range` by `text` and puts the cursor after it;
    /// the facade edits the note through this (ADR-012).
    pub(crate) fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
        let (start, end) = (range.start, range.end);
        if start == end && text.is_empty() {
            return;
        }
        let lines_before = self.doc.len_lines();
        let line = self.doc.replace(start..end, text);
        self.cursor = start + text.chars().count();
        self.anchor = self.cursor;
        let added = self.doc.len_lines() as isize - lines_before as isize;
        self.highlight.edited(line, added, self.doc.len_lines());
        self.note_edit(line, added);
        self.scroll_to_cursor = true;
    }

    /// Cursor and selection anchor, as char indices (shared by both views).
    pub(crate) fn cursor_anchor(&self) -> (usize, usize) {
        (self.cursor, self.anchor)
    }

    pub(crate) fn set_cursor_anchor(&mut self, cursor: usize, anchor: usize) {
        let len = self.doc.len_chars();
        self.cursor = cursor.min(len);
        self.anchor = anchor.min(len);
    }

    /// The theme colors of the view: text and background.
    pub(crate) fn colors(&self) -> ([u8; 4], [u8; 4]) {
        self.highlight.colors()
    }

    pub(crate) fn format(&self) -> NoteFormat {
        self.highlight.format()
    }

    pub(crate) fn highlight_set(&self) -> Arc<HighlightSet> {
        self.highlight.set()
    }

    pub(crate) fn undo(&mut self) -> bool {
        let result = self.doc.undo();
        self.after_history(result)
    }

    pub(crate) fn redo(&mut self) -> bool {
        let result = self.doc.redo();
        self.after_history(result)
    }

    fn after_history(&mut self, result: Option<(usize, usize)>) -> bool {
        let Some((cursor, line)) = result else {
            return false;
        };
        // Undo and redo can change any number of lines: estimate everything.
        self.layout_revision = None;
        self.pending_edit = None;
        self.cursor = cursor;
        self.anchor = cursor;
        self.highlight.invalidate_from(line, self.doc.len_lines());
        self.scroll_to_cursor = true;
        true
    }

    /// The laid-out text of `line`, cached by its text and colors.
    fn galley(&mut self, ui: &egui::Ui, font: &FontId, line: usize) -> Arc<Galley> {
        let doc = &self.doc;
        let spans = match self
            .highlight
            .spans_by(line, |i| doc.line(i), self.deadline)
        {
            Some(spans) => spans,
            None => {
                self.uncolored = true;
                Arc::new(Vec::new())
            }
        };
        let text = doc.line_text(line);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        spans.hash(&mut hasher);
        self.layout_width.to_bits().hash(&mut hasher);
        let key = hasher.finish();
        if let Some(galley) = self.galleys.get(&key) {
            return Arc::clone(galley);
        }
        if self.galleys.len() > MAX_CACHED_GALLEYS {
            self.galleys.clear();
        }
        let (foreground, _) = self.highlight.colors();
        let mut job = layout_job(&text, &spans, font, color(foreground));
        job.wrap.max_width = self.layout_width;
        let galley = ui.fonts(|f| f.layout_job(job));
        self.galleys.insert(key, Arc::clone(&galley));
        galley
    }

    fn char_at(&mut self, ui: &egui::Ui, font: &FontId, origin: Pos2, pos: Pos2) -> usize {
        let line = self.line_at(pos.y - origin.y);
        let galley = self.galley(ui, font, line);
        let column = galley
            .cursor_from_pos(pos - origin - Vec2::new(0.0, self.tops[line]))
            .ccursor
            .index;
        self.doc.rope().line_to_char(line) + column
    }

    fn cursor_rect(&mut self, ui: &egui::Ui, font: &FontId, origin: Pos2, row_height: f32) -> Rect {
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        let galley = self.galley(ui, font, line);
        let at = galley.pos_from_ccursor(CCursor::new(column)).min;
        let top = origin + Vec2::new(at.x, self.tops[line.min(self.tops.len() - 1)] + at.y);
        Rect::from_min_size(top, Vec2::new(2.0, row_height))
    }
}

fn layout_job(text: &str, spans: &[Span], font: &FontId, plain: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut end = 0;
    for span in spans {
        let (start, stop) = (span.start.min(text.len()), span.end.min(text.len()));
        if start < end
            || start >= stop
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(stop)
        {
            continue;
        }
        if start > end {
            job.append(
                &text[end..start],
                0.0,
                TextFormat::simple(font.clone(), plain),
            );
        }
        let format = TextFormat {
            font_id: font.clone(),
            color: color(span.color),
            italics: span.italic,
            ..Default::default()
        };
        job.append(&text[start..stop], 0.0, format);
        end = stop;
    }
    if end < text.len() {
        job.append(&text[end..], 0.0, TextFormat::simple(font.clone(), plain));
    }
    job
}

/// Room for the fold arrows left of the line numbers.
const FOLD_ARROW: f32 = 14.0;

/// UC-004 BR-012: the arrow of a section, pointing right when folded and
/// down when expanded.
pub(crate) fn paint_fold_arrow(painter: &egui::Painter, rect: Rect, folded: bool, color: Color32) {
    let c = rect.center();
    let r = 4.0;
    let points = if folded {
        vec![
            c + Vec2::new(-r * 0.6, -r),
            c + Vec2::new(r * 0.9, 0.0),
            c + Vec2::new(-r * 0.6, r),
        ]
    } else {
        vec![
            c + Vec2::new(-r, -r * 0.6),
            c + Vec2::new(r, -r * 0.6),
            c + Vec2::new(0.0, r * 0.9),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

fn color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The source view for the Vim and Nano keys: a line is a line of markup.
struct SourceTarget<'a>(&'a mut SourceView);

impl Target for SourceTarget<'_> {
    fn len(&self) -> usize {
        self.0.doc.len_chars()
    }

    fn char_at(&self, at: usize) -> Option<char> {
        let rope = self.0.doc.rope();
        (at < rope.len_chars()).then(|| rope.char(at))
    }

    fn cursor(&self) -> (usize, usize) {
        (self.0.cursor, self.0.anchor)
    }

    fn set_cursor(&mut self, cursor: usize, anchor: usize) {
        let len = self.len();
        self.0.cursor = cursor.min(len);
        self.0.anchor = anchor.min(len);
        self.0.scroll_to_cursor = true;
    }

    fn left(&mut self, at: usize) -> usize {
        at.saturating_sub(1)
    }

    fn right(&mut self, at: usize) -> usize {
        (at + 1).min(self.len())
    }

    fn up(&mut self, at: usize) -> usize {
        let rope = self.0.doc.rope();
        let line = rope.char_to_line(at);
        let column = at - rope.line_to_char(line);
        let up = self.0.visible_line(line.saturating_sub(1), -1);
        self.0.at_line(up, column)
    }

    fn down(&mut self, at: usize) -> usize {
        let rope = self.0.doc.rope();
        let line = rope.char_to_line(at);
        let column = at - rope.line_to_char(line);
        let down = self
            .0
            .visible_line((line + 1).min(self.0.doc.len_lines() - 1), 1);
        self.0.at_line(down, column)
    }

    fn line(&mut self, at: usize) -> Range<usize> {
        let rope = self.0.doc.rope();
        let line = rope.char_to_line(at.min(rope.len_chars()));
        let start = rope.line_to_char(line);
        start..start + self.0.doc.line_text(line).chars().count()
    }

    fn whole_line(&mut self, at: usize) -> Range<usize> {
        let rope = self.0.doc.rope();
        let line = rope.char_to_line(at.min(rope.len_chars()));
        let start = rope.line_to_char(line);
        let end = if line + 1 < rope.len_lines() {
            rope.line_to_char(line + 1)
        } else {
            rope.len_chars()
        };
        start..end
    }

    fn separator(&mut self, _at: usize) -> &'static str {
        "\n"
    }

    fn text(&self, range: Range<usize>) -> String {
        let len = self.len();
        self.0
            .doc
            .rope()
            .slice(range.start.min(len)..range.end.min(len))
            .to_string()
    }

    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.0.replace(range, text);
    }

    fn undo(&mut self) {
        self.0.undo();
    }

    fn redo(&mut self) {
        self.0.redo();
    }

    fn open_line(&mut self, at: usize, below: bool) {
        let line = self.line(at);
        if below {
            self.0.replace(line.end..line.end, "\n");
        } else {
            self.0.replace(line.start..line.start, "\n");
            self.set_cursor(line.start, line.start);
        }
    }

    fn find(&self, pattern: &str, from: usize, forward: bool) -> Option<usize> {
        self.0.find(pattern, from, forward)
    }

    fn clipboard_read(&mut self) -> Option<String> {
        match self.0.paste(false) {
            Pasted::Text(text) | Pasted::Markup(text) => Some(text),
            _ => None,
        }
    }

    fn clipboard_write(&mut self, text: &str) {
        self.0.copy(text.to_owned(), None);
    }

    fn command(&mut self, command: EditorCommand) {
        self.0.push_command(command);
    }
}
