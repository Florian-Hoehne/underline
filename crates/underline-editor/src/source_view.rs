//! The source view (UC-005, ADR-012): the markup of a note, highlighted, with
//! only the visible lines laid out.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{
    self, text::CCursor, text::LayoutJob, Color32, Event, EventFilter, FontId, Galley, Key, Pos2,
    Rect, Sense, Stroke, TextFormat, Vec2,
};
use underline_core::NoteFormat;
use underline_facade::highlight::{DocumentHighlight, HighlightSet, Span};

use crate::document::Document;
use crate::Tool;
use underline_facade::blocks::{link_at, parse_blocks};
use underline_facade::formatting::{apply_format, Format};

const FONT_SIZE: f32 = 14.0;
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
    row_height: f32,
    /// An edit since the last layout: its first line and the lines it added
    /// (negative when removed), so only these rows are estimated again.
    pending_edit: Option<(usize, isize)>,
    /// A toolbar function that asks the user first (link, image), for the
    /// window to answer (UC-005 A2).
    request: Option<Tool>,
    /// A link the user followed with Ctrl+click (UC-004 A2, BR-006).
    followed: Option<String>,
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
        }
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
        self.doc = Document::new(text);
        self.highlight = DocumentHighlight::new(set, format);
        self.highlight.invalidate_from(0, self.doc.len_lines());
        self.galleys.clear();
        self.layout_revision = None;
        self.cursor = self.at_line(line, column);
        self.anchor = self.cursor;
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

    /// Draws the view and handles its input. Returns whether the text changed.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let font = FontId::monospace(FONT_SIZE);
        let row_height = ui.fonts(|f| f.row_height(&font));
        let char_width = ui.fonts(|f| f.glyph_width(&font, 'x'));
        let (foreground, background) = self.highlight.colors();
        let foreground = color(foreground);
        let numbers = foreground.gamma_multiply(0.45);
        let mut changed = false;
        self.deadline = Some(std::time::Instant::now() + COLORING_BUDGET);
        self.uncolored = false;

        egui::ScrollArea::vertical()
            .id_salt(self.id.with("scroll"))
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let lines = self.doc.len_lines();
                // Line numbers on the left (FR-028; their settings come with
                // UC-010).
                let digits = lines.max(1).ilog10() as f32 + 1.0;
                let gutter = digits * char_width + 2.0 * PADDING;
                let width = ui.available_width();
                let wrap_width = (width - gutter - 2.0 * PADDING).max(char_width * 8.0);
                self.update_layout(wrap_width, char_width, row_height);

                let size = Vec2::new(width, self.tops[lines] + 2.0 * PADDING);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let response = ui.interact(rect, self.id, Sense::click_and_drag());
                let origin = rect.min + Vec2::new(gutter, PADDING);
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
                    ui.memory_mut(|m| {
                        m.set_focus_lock_filter(
                            self.id,
                            EventFilter {
                                tab: true,
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                escape: false,
                            },
                        )
                    });
                    let page = (viewport.height() / row_height).floor().max(1.0) as usize;
                    changed = self.handle_keys(ui, page);
                    if changed {
                        self.update_layout(wrap_width, char_width, row_height);
                    }
                }

                let painter = ui.painter_at(rect.intersect(viewport.translate(rect.min.to_vec2())));
                painter.rect_filled(painter.clip_rect(), 0.0, color(background));
                let lines = self.doc.len_lines();
                let first = self.line_at(viewport.min.y - PADDING);
                let last = (self.line_at(viewport.max.y - PADDING) + 1).min(lines);
                let (selection_start, selection_end) = self.selection();
                let selection_color = ui.visuals().selection.bg_fill;
                let mut exact_changed = false;
                for line in first..last {
                    let galley = self.galley(ui, &font, line);
                    let rows = galley.rows.len().max(1) as u32;
                    if self.rows[line] != rows {
                        self.rows[line] = rows;
                        exact_changed = true;
                    }
                    let top = origin + Vec2::new(0.0, self.tops[line]);
                    painter.text(
                        Pos2::new(origin.x - PADDING, top.y),
                        egui::Align2::RIGHT_TOP,
                        (line + 1).to_string(),
                        font.clone(),
                        numbers,
                    );
                    // Selection behind the text, row by row.
                    let line_start = self.doc.rope().line_to_char(line);
                    let line_len = self.doc.line_text(line).chars().count();
                    let from = selection_start.max(line_start);
                    let to = selection_end.min(line_start + line_len + 1);
                    if selection_start != selection_end && from < to {
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
                                selection_color,
                            );
                        }
                    }
                    painter.galley(top, galley, foreground);
                }
                if exact_changed {
                    self.recompute_tops();
                    ui.ctx().request_repaint();
                }

                // Cursor.
                let cursor_rect = self.cursor_rect(ui, &font, origin, row_height);
                if response.has_focus() {
                    painter.line_segment(
                        [cursor_rect.center_top(), cursor_rect.center_bottom()],
                        Stroke::new(2.0_f32, foreground),
                    );
                    ui.ctx().output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput { rect, cursor_rect })
                    });
                }
                if std::mem::take(&mut self.scroll_to_cursor) {
                    ui.scroll_to_rect(cursor_rect.expand(row_height), None);
                }
            });
        if self.uncolored {
            ui.ctx().request_repaint();
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
                        .splice(at..at, std::iter::repeat(1).take(added as usize));
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
        self.tops.clear();
        self.tops.reserve(self.rows.len() + 1);
        let mut y = 0.0;
        for rows in &self.rows {
            self.tops.push(y);
            y += *rows as f32 * self.row_height;
        }
        self.tops.push(y);
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
        let events = ui.input(|i| i.events.clone());
        let mut changed = false;
        for event in events {
            match event {
                Event::Text(text) | Event::Paste(text) => {
                    self.insert(&text);
                    changed = true;
                }
                Event::Ime(egui::ImeEvent::Commit(text)) => {
                    self.insert(&text);
                    changed = true;
                }
                Event::Copy => {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                }
                Event::Cut => {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                        self.insert("");
                        changed = true;
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
        changed
    }

    fn key(&mut self, key: Key, modifiers: egui::Modifiers, page: usize) -> bool {
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
            Key::ArrowUp => target = Some(self.at_line(line.saturating_sub(1), column)),
            Key::ArrowDown => target = Some(self.at_line(line + 1, column)),
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

fn color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
