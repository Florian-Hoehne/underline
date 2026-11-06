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
    widest: f32,
    /// Until when this frame may color lines.
    deadline: Option<std::time::Instant>,
    /// Whether a line was shown without its colors this frame.
    uncolored: bool,
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
            widest: 0.0,
            deadline: None,
            uncolored: false,
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

    /// Puts the cursor at the end of the text and the keyboard focus on the
    /// view, as for a new note (UC-003 step 6).
    pub fn focus_end(&mut self) {
        self.set_cursor(self.doc.len_chars());
        self.focus = true;
    }

    /// Draws the view and handles its input. Returns whether the text changed.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let font = FontId::monospace(FONT_SIZE);
        let row_height = ui.fonts(|f| f.row_height(&font));
        let (foreground, background) = self.highlight.colors();
        let foreground = color(foreground);
        let mut changed = false;
        self.deadline = Some(std::time::Instant::now() + COLORING_BUDGET);
        self.uncolored = false;

        egui::ScrollArea::both()
            .id_salt(self.id.with("scroll"))
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let lines = self.doc.len_lines();
                let size = Vec2::new(
                    (self.widest + 2.0 * PADDING).max(ui.available_width()),
                    lines as f32 * row_height + 2.0 * PADDING,
                );
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let response = ui.interact(rect, self.id, Sense::click_and_drag());
                let origin = rect.min + Vec2::splat(PADDING);
                if std::mem::take(&mut self.focus) {
                    response.request_focus();
                }

                // Pointer: place the cursor, extend the selection by dragging.
                if response.is_pointer_button_down_on() || response.clicked() {
                    response.request_focus();
                    if let Some(pos) = response.interact_pointer_pos() {
                        let at = self.char_at(ui, &font, origin, row_height, pos);
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
                }

                let painter = ui.painter_at(rect.intersect(viewport.translate(rect.min.to_vec2())));
                painter.rect_filled(painter.clip_rect(), 0.0, color(background));
                let first = ((viewport.min.y - PADDING) / row_height).floor().max(0.0) as usize;
                let last =
                    (((viewport.max.y - PADDING) / row_height).ceil() as usize + 1).min(lines);
                let (selection_start, selection_end) = self.selection();
                let selection_color = ui.visuals().selection.bg_fill;
                for line in first..last {
                    let galley = self.galley(ui, &font, line);
                    let top = origin + Vec2::new(0.0, line as f32 * row_height);
                    self.widest = self.widest.max(galley.size().x);
                    // Selection behind the text.
                    let line_start = self.doc.rope().line_to_char(line);
                    let line_len = self.doc.line_text(line).chars().count();
                    let from = selection_start.max(line_start);
                    let to = selection_end.min(line_start + line_len + 1);
                    if selection_start != selection_end && from < to {
                        let x0 = galley
                            .pos_from_ccursor(CCursor::new(from - line_start))
                            .min
                            .x;
                        let x1 = if to > line_start + line_len {
                            galley.size().x + 6.0
                        } else {
                            galley.pos_from_ccursor(CCursor::new(to - line_start)).min.x
                        };
                        painter.rect_filled(
                            Rect::from_min_max(
                                top + Vec2::new(x0, 0.0),
                                top + Vec2::new(x1, row_height),
                            ),
                            0.0,
                            selection_color,
                        );
                    }
                    painter.galley(top, galley, foreground);
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

    fn selection(&self) -> (usize, usize) {
        (self.cursor.min(self.anchor), self.cursor.max(self.anchor))
    }

    fn selected_text(&self) -> String {
        let (start, end) = self.selection();
        self.doc.rope().slice(start..end).to_string()
    }

    /// Replaces the selection by `text` (UC-005 step 1).
    fn insert(&mut self, text: &str) {
        let (start, end) = self.selection();
        if start == end && text.is_empty() {
            return;
        }
        let lines_before = self.doc.len_lines();
        let line = self.doc.replace(start..end, text);
        self.cursor = start + text.chars().count();
        self.anchor = self.cursor;
        let added = self.doc.len_lines() as isize - lines_before as isize;
        self.highlight.edited(line, added, self.doc.len_lines());
        self.scroll_to_cursor = true;
    }

    fn undo(&mut self) -> bool {
        let result = self.doc.undo();
        self.after_history(result)
    }

    fn redo(&mut self) -> bool {
        let result = self.doc.redo();
        self.after_history(result)
    }

    fn after_history(&mut self, result: Option<(usize, usize)>) -> bool {
        let Some((cursor, line)) = result else {
            return false;
        };
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
        let key = hasher.finish();
        if let Some(galley) = self.galleys.get(&key) {
            return Arc::clone(galley);
        }
        if self.galleys.len() > MAX_CACHED_GALLEYS {
            self.galleys.clear();
        }
        let (foreground, _) = self.highlight.colors();
        let job = layout_job(&text, &spans, font, color(foreground));
        let galley = ui.fonts(|f| f.layout_job(job));
        self.galleys.insert(key, Arc::clone(&galley));
        galley
    }

    fn char_at(
        &mut self,
        ui: &egui::Ui,
        font: &FontId,
        origin: Pos2,
        row_height: f32,
        pos: Pos2,
    ) -> usize {
        let lines = self.doc.len_lines();
        let line = (((pos.y - origin.y) / row_height).floor().max(0.0) as usize)
            .min(lines.saturating_sub(1));
        let galley = self.galley(ui, font, line);
        let column = galley
            .cursor_from_pos(Vec2::new(pos.x - origin.x, row_height / 2.0))
            .ccursor
            .index;
        self.doc.rope().line_to_char(line) + column
    }

    fn cursor_rect(&mut self, ui: &egui::Ui, font: &FontId, origin: Pos2, row_height: f32) -> Rect {
        let line = self.doc.rope().char_to_line(self.cursor);
        let column = self.cursor - self.doc.rope().line_to_char(line);
        let galley = self.galley(ui, font, line);
        let x = galley.pos_from_ccursor(CCursor::new(column)).min.x;
        let top = origin + Vec2::new(x, line as f32 * row_height);
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
    job.wrap.max_width = f32::INFINITY;
    job
}

fn color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
