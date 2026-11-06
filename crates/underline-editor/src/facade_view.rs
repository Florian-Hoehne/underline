//! The facade (FR-009, UC-005, ADR-012): the note shown formatted, block by
//! block. Every edit is turned into an edit of the markup at the source
//! position the facade character stands for, on the same text and cursor as
//! the source view, so both views always agree and blocks that are not edited
//! keep every byte (NFR-007).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{
    self, text::CCursor, text::LayoutJob, Color32, Event, EventFilter, FontId, Galley, Key, Pos2,
    Rect, Sense, Stroke, TextFormat, Vec2,
};
use underline_core::{resolve_link, LinkTarget, NoteFormat};
use underline_facade::blocks::{insert_typed, link_at, parse_blocks, Block, BlockKind, ListMarker};
use underline_facade::diagram::{DiagramLanguage, DiagramState, Diagrams, Picture};
use underline_facade::highlight::highlight_code;
use underline_facade::images::{ImageError, ImageSource, ImageState, Images};

use crate::enlarged::{Enlarged, Shown};
use crate::{fonts, SourceView, Tool};

const PADDING: f32 = 12.0;
const BLOCK_GAP: f32 = 8.0;
const BODY_SIZE: f32 = 15.0;
const CODE_SIZE: f32 = 14.0;

/// The facade of one tab; it shows and edits the text of a [`SourceView`].
pub struct FacadeView {
    id: egui::Id,
    blocks: Vec<Block>,
    parsed: Option<u64>,
    /// Laid-out blocks by what they show, so an edit lays out only the
    /// blocks it changed.
    galleys: HashMap<u64, Arc<Galley>>,
    /// Top of each block, and the end.
    tops: Vec<f32>,
    width: f32,
    focus: bool,
    scroll_to_cursor: bool,
    /// Renders the diagrams (FR-036, FR-037); without it they show as code.
    diagrams: Option<Arc<Diagrams>>,
    /// Loads the images (UC-004 step 3); without it they show as markup.
    images: Option<Arc<Images>>,
    /// The workspace and the file of the note, to find its images.
    place: Option<(PathBuf, PathBuf)>,
    /// The diagrams rendered and images loaded when the layout was made.
    diagram_generation: (u64, u64),
    /// The diagram block holding the cursor, whose source is shown below
    /// its picture (UC-005 BR-008).
    editing: Option<usize>,
    /// Textures of the pictures shown, by picture.
    textures: HashMap<usize, (Arc<Picture>, egui::TextureHandle)>,
    /// The last picture of each diagram block, shown while its changed
    /// source is rendered.
    last_pictures: HashMap<usize, Arc<Picture>>,
    /// The picture shown enlarged over the window (FR-039).
    enlarged: Option<Enlarged>,
}

/// What a block shows besides its text: the marker before it, in chars, and
/// for a diagram its picture or message above the text.
struct Laid {
    galley: Arc<Galley>,
    prefix: usize,
    /// Where the text starts in the block.
    offset: Vec2,
    /// The height of the whole block.
    height: f32,
    /// Whether the text is shown (a diagram hides its source).
    text_shown: bool,
    figure: Option<Figure>,
    /// The caption of an image: its top in the block, and its text.
    caption: Option<(f32, Arc<Galley>)>,
}

/// What a diagram block shows above its source.
enum Figure {
    Picture(egui::TextureId, Vec2),
    /// A message, wrapped to the width of the facade.
    Message(Arc<Galley>),
}

impl FacadeView {
    pub fn new(id: impl Hash) -> Self {
        Self {
            id: egui::Id::new(id).with("facade"),
            blocks: Vec::new(),
            parsed: None,
            galleys: HashMap::new(),
            tops: vec![0.0],
            width: 0.0,
            focus: false,
            scroll_to_cursor: false,
            diagrams: None,
            images: None,
            place: None,
            diagram_generation: (0, 0),
            editing: None,
            textures: HashMap::new(),
            last_pictures: HashMap::new(),
            enlarged: None,
        }
    }

    /// The same facade, showing diagrams as pictures rendered by `diagrams`.
    pub fn with_diagrams(mut self, diagrams: Arc<Diagrams>) -> Self {
        self.diagrams = Some(diagrams);
        self
    }

    /// The same facade, showing images loaded by `images` for the note in
    /// the file `note` of the workspace at `root`.
    pub fn with_images(mut self, images: Arc<Images>, root: PathBuf, note: PathBuf) -> Self {
        self.images = Some(images);
        self.place = Some((root, note));
        self
    }

    /// Whether block `index` is shown as a picture: a diagram or an image.
    fn is_figure(&self, index: usize) -> bool {
        match &self.blocks[index].kind {
            BlockKind::Image { .. } => self.images.is_some(),
            kind => self.diagrams.is_some() && diagram_language(kind).is_some(),
        }
    }

    /// The picture block holding the cursor, if any; a change of it changes
    /// the layout, as does a picture that became ready.
    fn update_editing(&mut self, source: &SourceView) {
        let editing = self
            .cursor_position(source)
            .map(|(index, _)| index)
            .filter(|&index| self.is_figure(index));
        if editing != self.editing {
            self.editing = editing;
            self.tops.clear();
        }
        let generation = (
            self.diagrams.as_ref().map_or(0, |d| d.generation()),
            self.images.as_ref().map_or(0, |i| i.generation()),
        );
        if generation != self.diagram_generation {
            self.diagram_generation = generation;
            self.tops.clear();
        }
    }

    /// Puts the keyboard focus on the facade at the next frame.
    pub fn request_focus(&mut self) {
        self.focus = true;
        self.scroll_to_cursor = true;
    }

    /// Draws the facade of `source`'s text and handles its input. Returns
    /// whether the text changed.
    pub fn show(&mut self, ui: &mut egui::Ui, source: &mut SourceView) -> bool {
        let (foreground, background) = source.colors();
        let theme = Theme {
            text: rgba(foreground),
            strong: ui.visuals().strong_text_color(),
            link: ui.visuals().hyperlink_color,
            code_bg: ui.visuals().code_bg_color,
            weak: rgba(foreground).gamma_multiply(0.55),
        };
        self.reparse(source);
        self.update_editing(source);
        let mut changed = false;

        egui::ScrollArea::vertical()
            .id_salt(self.id.with("scroll"))
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let width = (ui.available_width() - 2.0 * PADDING).max(80.0);
                self.layout_all(ui, source, &theme, width);
                let size = Vec2::new(
                    ui.available_width(),
                    self.tops[self.blocks.len()] + 2.0 * PADDING,
                );
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let response = ui.interact(rect, self.id, Sense::click_and_drag());
                let origin = rect.min + Vec2::splat(PADDING);
                if std::mem::take(&mut self.focus) {
                    response.request_focus();
                }

                // UC-004 BR-006: pointing at a link shows where it leads;
                // Ctrl+click (Cmd+click) follows it.
                if let Some(pos) = response.hover_pos() {
                    if let Some(link) = self.link_at_pos(ui, source, &theme, origin, pos) {
                        let key = if cfg!(target_os = "macos") {
                            "Cmd"
                        } else {
                            "Ctrl"
                        };
                        response
                            .clone()
                            .on_hover_text_at_pointer(format!("{link}\n{key}+click to follow"));
                        if response.clicked() && ui.input(|i| i.modifiers.command) {
                            source.follow(link);
                        }
                    }
                }
                // UC-004 BR-011: a double click on a picture enlarges it.
                if response.double_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        if let Some((index, _)) = self.picture_at(ui, source, &theme, origin, pos) {
                            self.enlarge(ui, index);
                        }
                    }
                }
                if response.is_pointer_button_down_on() || response.clicked() {
                    response.request_focus();
                    if let Some(pos) = response.interact_pointer_pos() {
                        if let Some(at) = self.char_at(ui, source, &theme, origin, pos) {
                            let (_, anchor) = source.cursor_anchor();
                            let extend = response.dragged() || ui.input(|i| i.modifiers.shift);
                            source.set_cursor_anchor(at, if extend { anchor } else { at });
                        }
                    }
                }

                if response.has_focus() && self.enlarged.is_none() {
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
                    changed = self.handle_input(ui, source, &theme);
                    if changed {
                        self.reparse(source);
                    }
                    self.update_editing(source);
                    self.layout_all(ui, source, &theme, width);
                }

                let painter = ui.painter_at(rect.intersect(viewport.translate(rect.min.to_vec2())));
                painter.rect_filled(painter.clip_rect(), 0.0, rgba(background));
                let first = self.block_at(viewport.min.y - PADDING);
                let last = (self.block_at(viewport.max.y - PADDING) + 1).min(self.blocks.len());
                let (sel_start, sel_end) = source.selection();
                let rope = source.document().rope().clone();
                for index in first..last {
                    let laid = self.laid(ui, source, &theme, index);
                    let block_top = origin + Vec2::new(0.0, self.tops[index]);
                    let top = block_top + laid.offset;
                    let block = &self.blocks[index];
                    match &laid.figure {
                        Some(Figure::Picture(texture, size)) => {
                            painter.image(
                                *texture,
                                Rect::from_min_size(block_top, *size),
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                        Some(Figure::Message(message)) => {
                            painter.galley(block_top, Arc::clone(message), theme.weak);
                        }
                        None => {}
                    }
                    if let Some((caption_top, caption)) = &laid.caption {
                        painter.galley(
                            block_top + Vec2::new(0.0, *caption_top),
                            Arc::clone(caption),
                            theme.text,
                        );
                    }
                    match &block.kind {
                        BlockKind::Code { .. } | BlockKind::Markup | BlockKind::Image { .. }
                            if laid.text_shown =>
                        {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    top - Vec2::splat(4.0),
                                    Vec2::new(width + 8.0, laid.galley.size().y + 8.0),
                                ),
                                4.0,
                                theme.code_bg,
                            );
                        }
                        BlockKind::Quote(depth) => {
                            for d in 0..*depth {
                                let x = top.x + 4.0 + d as f32 * 14.0;
                                painter.line_segment(
                                    [
                                        Pos2::new(x, top.y),
                                        Pos2::new(x, top.y + laid.galley.size().y),
                                    ],
                                    Stroke::new(3.0_f32, theme.weak),
                                );
                            }
                        }
                        BlockKind::Admonition(_) => {
                            let x = top.x + 4.0;
                            painter.line_segment(
                                [
                                    Pos2::new(x, top.y),
                                    Pos2::new(x, top.y + laid.galley.size().y),
                                ],
                                Stroke::new(3.0_f32, theme.link),
                            );
                        }
                        BlockKind::ThematicBreak => {
                            let y = top.y + 8.0;
                            painter.line_segment(
                                [Pos2::new(top.x, y), Pos2::new(top.x + width, y)],
                                Stroke::new(1.0_f32, theme.weak),
                            );
                        }
                        _ => {}
                    }
                    // Selection behind the text.
                    if sel_start != sel_end && laid.text_shown {
                        let to_char =
                            |offset: usize| rope.byte_to_char(offset.min(rope.len_bytes()));
                        let count = block.text().chars().count();
                        let mut from = None;
                        let mut to = None;
                        for i in 0..=count {
                            let at = to_char(block.source_offset(i));
                            if from.is_none() && at >= sel_start {
                                from = Some(i);
                            }
                            if at <= sel_end {
                                to = Some(i);
                            }
                        }
                        if let (Some(from), Some(to)) = (from, to) {
                            if from < to {
                                paint_selection(
                                    &painter,
                                    &laid,
                                    top,
                                    from,
                                    to,
                                    ui.visuals().selection.bg_fill,
                                );
                            }
                        }
                    }
                    if laid.text_shown {
                        painter.galley(top, Arc::clone(&laid.galley), theme.text);
                    }
                }

                // UC-004 BR-011: the magnifier at the corner of the picture
                // under the pointer enlarges it.
                // The pointer is asked directly: over the button, the facade
                // itself is not hovered.
                let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| {
                    rect.contains(*p)
                        && ui.layer_id() == ui.ctx().layer_id_at(*p).unwrap_or(ui.layer_id())
                });
                if let Some(pos) = pointer.filter(|_| self.enlarged.is_none()) {
                    if let Some((index, picture)) = self.picture_at(ui, source, &theme, origin, pos)
                    {
                        let button = Rect::from_min_size(
                            picture.right_top() + Vec2::new(-30.0, 6.0),
                            Vec2::splat(24.0),
                        );
                        let magnifier =
                            ui.interact(button, self.id.with("magnifier"), Sense::click());
                        let hovered = magnifier.hovered();
                        painter.rect_filled(
                            button,
                            4.0,
                            Color32::from_black_alpha(if hovered { 200 } else { 140 }),
                        );
                        let glass = button.center() - Vec2::splat(2.0);
                        painter.circle_stroke(glass, 6.0, Stroke::new(2.0_f32, Color32::WHITE));
                        painter.line_segment(
                            [glass + Vec2::splat(4.5), glass + Vec2::splat(9.0)],
                            Stroke::new(2.5_f32, Color32::WHITE),
                        );
                        let magnifier = magnifier.on_hover_text("Enlarge (double click)");
                        if magnifier.clicked() {
                            self.enlarge(ui, index);
                        }
                    }
                }
                if let Some(enlarged) = &mut self.enlarged {
                    let open =
                        enlarged.show(ui.ctx(), self.diagrams.as_deref(), self.images.as_deref());
                    if !open {
                        self.enlarged = None;
                        response.request_focus();
                    }
                }

                // Cursor, and the format indicator (FR-011).
                if let Some(cursor_rect) = self.cursor_rect(ui, source, &theme, origin) {
                    if response.has_focus() {
                        painter.line_segment(
                            [cursor_rect.center_top(), cursor_rect.center_bottom()],
                            Stroke::new(2.0_f32, theme.text),
                        );
                        ui.ctx().output_mut(|o| {
                            o.ime = Some(egui::output::IMEOutput { rect, cursor_rect })
                        });
                    }
                    if std::mem::take(&mut self.scroll_to_cursor) {
                        ui.scroll_to_rect(cursor_rect.expand(24.0), None);
                    }
                }
                let indicator = match source.format() {
                    NoteFormat::Markdown => "Markdown",
                    NoteFormat::AsciiDoc => "AsciiDoc",
                };
                painter.text(
                    Pos2::new(rect.right() - 6.0, viewport.min.y + rect.top() + 4.0),
                    egui::Align2::RIGHT_TOP,
                    indicator,
                    FontId::proportional(11.0),
                    theme.weak,
                );
            });
        changed
    }

    /// Parses the blocks again when the text changed.
    fn reparse(&mut self, source: &SourceView) {
        let revision = source.document().revision();
        if self.parsed != Some(revision) {
            self.blocks = parse_blocks(&source.document().text(), source.format());
            self.parsed = Some(revision);
            self.tops.clear();
        }
    }

    /// Lays out every block (from the cache when unchanged) and computes the
    /// tops of the blocks.
    fn layout_all(&mut self, ui: &egui::Ui, source: &SourceView, theme: &Theme, width: f32) {
        if (self.width - width).abs() > 0.5 {
            self.width = width;
            self.galleys.clear();
            self.tops.clear();
        }
        if self.tops.len() == self.blocks.len() + 1 {
            return;
        }
        let mut tops = Vec::with_capacity(self.blocks.len() + 1);
        let mut y = 0.0;
        for index in 0..self.blocks.len() {
            tops.push(y);
            let laid = self.laid(ui, source, theme, index);
            let extra = match self.blocks[index].kind {
                BlockKind::ThematicBreak => 16.0,
                BlockKind::Code { .. } | BlockKind::Markup | BlockKind::Image { .. } => 8.0,
                _ => 0.0,
            };
            y += laid.height + BLOCK_GAP + extra;
        }
        tops.push(y);
        self.tops = tops;
    }

    fn block_at(&self, y: f32) -> usize {
        let count = self.blocks.len();
        if count == 0 {
            return 0;
        }
        self.tops[..count]
            .partition_point(|top| *top <= y)
            .saturating_sub(1)
            .min(count - 1)
    }

    /// The laid-out block `index`, cached by what it shows; the layout is
    /// built only when the cache misses.
    fn laid(&mut self, ui: &egui::Ui, source: &SourceView, theme: &Theme, index: usize) -> Laid {
        let block = &self.blocks[index];
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        block.kind.hash(&mut hasher);
        for run in &block.runs {
            run.text.hash(&mut hasher);
            run.style.hash(&mut hasher);
        }
        self.width.to_bits().hash(&mut hasher);
        let key = hasher.finish();
        let galley = match self.galleys.get(&key) {
            Some(galley) => Arc::clone(galley),
            None => {
                let (job, _) = block_job(block, theme, self.width, source);
                if self.galleys.len() > 8192 {
                    self.galleys.clear();
                }
                let galley = ui.fonts(|f| f.layout_job(job));
                self.galleys.insert(key, Arc::clone(&galley));
                galley
            }
        };
        let prefix = prefix_chars(&block.kind);
        let mut laid = Laid {
            height: galley.size().y,
            galley,
            prefix,
            offset: Vec2::ZERO,
            text_shown: true,
            figure: None,
            caption: None,
        };
        if self.is_figure(index) {
            self.figure(ui, theme, index, &mut laid);
        }
        laid
    }

    /// What a picture block shows now: its picture, or why not, and
    /// whether its markup must be shown.
    fn picture_state(&self, ui: &egui::Ui, index: usize) -> (Result<Arc<Picture>, String>, bool) {
        let scale = ui.ctx().pixels_per_point();
        let block = &self.blocks[index];
        match &block.kind {
            BlockKind::Image { target, alt, .. } => {
                let Some(images) = &self.images else {
                    return (Err(alt.clone()), true);
                };
                let source = match image_source(self.place.as_ref(), target) {
                    Ok(source) => source,
                    Err(reason) => return (Err(reason), false),
                };
                match images.get(&source, scale) {
                    ImageState::Ready(picture) => (Ok(picture), false),
                    ImageState::Pending => (Err(format!("Loading {target}…")), false),
                    ImageState::Failed(ImageError::NotFound(_)) => (
                        Err(format!(
                            "The image {alt} cannot be shown: {target} was not found"
                        )),
                        false,
                    ),
                    ImageState::Failed(error) => (
                        Err(format!("The image {alt} cannot be shown: {error}")),
                        false,
                    ),
                }
            }
            kind => {
                let (Some(diagrams), Some(language)) = (&self.diagrams, diagram_language(kind))
                else {
                    return (Err(String::new()), true);
                };
                let dark = ui.visuals().dark_mode;
                match diagrams.get(language, &block.text(), dark, scale) {
                    DiagramState::Ready(picture) => (Ok(picture), false),
                    DiagramState::Pending => (
                        Err(format!("Rendering the {} diagram…", language.name())),
                        false,
                    ),
                    // UC-004 A11: the source with the error.
                    DiagramState::Failed(error) => (
                        Err(format!(
                            "This {} diagram cannot be shown: {error}",
                            language.name()
                        )),
                        true,
                    ),
                }
            }
        }
    }

    /// FR-036, FR-037, UC-004 step 3, UC-005 BR-008: a diagram or image
    /// block shows its picture (an image with its caption), and its markup
    /// below it while the cursor is in it. While a changed source renders,
    /// the last picture stays; a diagram that cannot be rendered shows its
    /// source with the error (UC-004 A11), an image that cannot be shown
    /// says why, naming its address (BR-004).
    fn figure(&mut self, ui: &egui::Ui, theme: &Theme, index: usize, laid: &mut Laid) {
        let (state, failed) = self.picture_state(ui, index);
        let pending = matches!(&state, Err(m) if m.ends_with('…'));
        let picture = match state {
            Ok(picture) => {
                self.last_pictures.insert(index, Arc::clone(&picture));
                Ok(picture)
            }
            Err(message) if pending => self.last_pictures.get(&index).cloned().ok_or(message),
            Err(message) => {
                self.last_pictures.remove(&index);
                Err(message)
            }
        };
        let width = self.width;
        let layout = |text: String, color: Color32, size: f32| {
            ui.fonts(|f| f.layout(text, FontId::proportional(size), color, width))
        };
        let (figure, mut size) = match picture {
            Ok(picture) => {
                let [w, h] = picture.size();
                let shrink = (width / w).min(1.0);
                let size = Vec2::new(w * shrink, h * shrink);
                (Figure::Picture(self.texture(ui, &picture), size), size)
            }
            Err(message) => {
                let color = if failed || !pending {
                    ui.visuals().error_fg_color
                } else {
                    theme.weak
                };
                let galley = layout(message, color, 13.0);
                let size = galley.size();
                (Figure::Message(galley), size)
            }
        };
        // TC-004-02: a caption below the picture, numbered as Asciidoctor
        // numbers figures.
        laid.caption = None;
        if let BlockKind::Image {
            caption: Some(caption),
            ..
        } = &self.blocks[index].kind
        {
            let number = self.blocks[..=index]
                .iter()
                .filter(|b| {
                    matches!(
                        &b.kind,
                        BlockKind::Image {
                            caption: Some(_),
                            ..
                        }
                    )
                })
                .count();
            let galley = layout(format!("Figure {number}. {caption}"), theme.text, 14.0);
            let top = size.y + 6.0;
            size.y = top + galley.size().y;
            laid.caption = Some((top, galley));
        }
        laid.text_shown = failed || self.editing == Some(index);
        laid.offset = Vec2::new(0.0, size.y + if laid.text_shown { 12.0 } else { 0.0 });
        laid.height = laid.offset.y
            + if laid.text_shown {
                laid.galley.size().y
            } else {
                0.0
            };
        laid.figure = Some(figure);
    }

    /// The picture block under `pos` and where its picture is.
    fn picture_at(
        &mut self,
        ui: &egui::Ui,
        source: &SourceView,
        theme: &Theme,
        origin: Pos2,
        pos: Pos2,
    ) -> Option<(usize, Rect)> {
        if self.blocks.is_empty() {
            return None;
        }
        let index = self.block_at(pos.y - origin.y);
        let laid = self.laid(ui, source, theme, index);
        let Some(Figure::Picture(_, size)) = laid.figure else {
            return None;
        };
        let rect = Rect::from_min_size(origin + Vec2::new(0.0, self.tops[index]), size);
        rect.contains(pos).then_some((index, rect))
    }

    /// FR-039: shows the picture of block `index` enlarged.
    fn enlarge(&mut self, ui: &egui::Ui, index: usize) {
        let Some(picture) = self.last_pictures.get(&index).cloned() else {
            return;
        };
        let block = &self.blocks[index];
        let shown = match &block.kind {
            BlockKind::Image { target, .. } => match image_source(self.place.as_ref(), target) {
                Ok(source) => Shown::Image(source),
                Err(_) => return,
            },
            kind => match diagram_language(kind) {
                Some(language) => Shown::Diagram {
                    language,
                    source: block.text(),
                    dark: ui.visuals().dark_mode,
                },
                None => return,
            },
        };
        self.enlarged = Some(Enlarged::new(shown, picture));
    }

    /// Whether a picture is shown enlarged.
    pub fn is_enlarged(&self) -> bool {
        self.enlarged.is_some()
    }

    /// The texture of `picture`, loaded once.
    fn texture(&mut self, ui: &egui::Ui, picture: &Arc<Picture>) -> egui::TextureId {
        let key = Arc::as_ptr(picture) as usize;
        if let Some((_, texture)) = self.textures.get(&key) {
            return texture.id();
        }
        if self.textures.len() > 64 {
            let shown: Vec<usize> = self
                .last_pictures
                .values()
                .map(|p| Arc::as_ptr(p) as usize)
                .collect();
            self.textures.retain(|key, _| shown.contains(key));
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [picture.width as usize, picture.height as usize],
            &picture.pixels,
        );
        let texture = ui
            .ctx()
            .load_texture("diagram", image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        self.textures.insert(key, (Arc::clone(picture), texture));
        id
    }

    /// Where the cursor is: the block holding it and the facade index in it.
    fn cursor_position(&self, source: &SourceView) -> Option<(usize, usize)> {
        let rope = source.document().rope();
        let (cursor, _) = source.cursor_anchor();
        let byte = rope.char_to_byte(cursor);
        let index = self
            .blocks
            .iter()
            .position(|b| byte <= b.span.end.max(b.runs.last().map_or(0, |r| r.end)))
            .unwrap_or(self.blocks.len().checked_sub(1)?);
        let block = &self.blocks[index];
        let at = block
            .facade_index(byte)
            .unwrap_or(if byte < block.span.start {
                0
            } else {
                block.text().chars().count()
            });
        Some((index, at))
    }

    fn cursor_rect(
        &mut self,
        ui: &egui::Ui,
        source: &SourceView,
        theme: &Theme,
        origin: Pos2,
    ) -> Option<Rect> {
        let (index, at) = self.cursor_position(source)?;
        let laid = self.laid(ui, source, theme, index);
        let pos = laid.galley.pos_from_ccursor(CCursor::new(laid.prefix + at));
        let top = origin + Vec2::new(0.0, self.tops[index]) + laid.offset;
        Some(Rect::from_min_size(
            top + pos.min.to_vec2(),
            Vec2::new(2.0, pos.height()),
        ))
    }

    fn char_at(
        &mut self,
        ui: &egui::Ui,
        source: &SourceView,
        theme: &Theme,
        origin: Pos2,
        pos: Pos2,
    ) -> Option<usize> {
        if self.blocks.is_empty() {
            return Some(source.document().len_chars());
        }
        let index = self.block_at(pos.y - origin.y);
        let laid = self.laid(ui, source, theme, index);
        let local = pos - origin - Vec2::new(0.0, self.tops[index]) - laid.offset;
        // A click on the picture of a diagram puts the cursor into its source.
        if local.y < 0.0 && laid.figure.is_some() {
            return Some(self.char_of(source, index, 0));
        }
        let ccursor = laid.galley.cursor_from_pos(local).ccursor.index;
        let at = ccursor.saturating_sub(laid.prefix);
        Some(self.char_of(source, index, at))
    }

    /// The link under the pointer, if any.
    fn link_at_pos(
        &mut self,
        ui: &egui::Ui,
        source: &SourceView,
        theme: &Theme,
        origin: Pos2,
        pos: Pos2,
    ) -> Option<String> {
        if self.blocks.is_empty() {
            return None;
        }
        let index = self.block_at(pos.y - origin.y);
        let laid = self.laid(ui, source, theme, index);
        let local = pos - origin - Vec2::new(0.0, self.tops[index]) - laid.offset;
        if !laid.text_shown
            || local.y < 0.0
            || local.y > laid.galley.size().y
            || local.x > laid.galley.size().x
        {
            return None;
        }
        let at = laid
            .galley
            .cursor_from_pos(local)
            .ccursor
            .index
            .checked_sub(laid.prefix)?;
        let mut rest = at;
        for run in &self.blocks[index].runs {
            let count = run.offsets.len();
            if rest < count {
                return run.style.link.clone();
            }
            rest -= count;
        }
        None
    }

    /// Facade blocks of the text, for following links and headings.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// The char index in the text of facade index `at` of block `index`.
    fn char_of(&self, source: &SourceView, index: usize, at: usize) -> usize {
        let rope = source.document().rope();
        let offset = self.blocks[index].source_offset(at).min(rope.len_bytes());
        rope.byte_to_char(offset)
    }

    fn handle_input(&mut self, ui: &egui::Ui, source: &mut SourceView, theme: &Theme) -> bool {
        let events = ui.input(|i| i.events.clone());
        let mut changed = false;
        for event in events {
            match event {
                Event::Text(text) | Event::Paste(text) => {
                    changed |= self.type_text(source, &text);
                }
                Event::Ime(egui::ImeEvent::Commit(text)) => {
                    changed |= self.type_text(source, &text);
                }
                Event::Copy => {
                    let text = source.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                }
                Event::Cut => {
                    let text = source.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                        changed |= self.delete_selection(source);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    changed |= self.key(ui, source, theme, key, modifiers);
                }
                _ => {}
            }
            if changed {
                self.reparse(source);
            }
        }
        changed
    }

    /// Types `text` at the cursor, escaped where the markup would read it as
    /// formatting (UC-005 step 3).
    fn type_text(&mut self, source: &mut SourceView, text: &str) -> bool {
        self.delete_selection(source);
        self.reparse(source);
        let (cursor, _) = source.cursor_anchor();
        let byte = source.document().rope().char_to_byte(cursor);
        let markdown = source.format() == NoteFormat::Markdown;
        let position = self.cursor_position(source);
        let (raw, fast) = match position {
            Some((index, at)) => match &self.blocks[index].kind {
                BlockKind::Code { .. } if markdown => (text.to_owned(), !text.contains(['`', '~'])),
                BlockKind::Markup | BlockKind::Image { .. } => (text.to_owned(), false),
                _ if markdown => (escape_markdown(text, at == 0), at > 0),
                // AsciiDoc has no escape that always works; plain text away
                // from markup joins its block, anything else parses again.
                _ => {
                    let rope = source.document().rope();
                    let near = |i: usize| (i < rope.len_chars()).then(|| rope.char(i));
                    let before = cursor.checked_sub(1).and_then(near);
                    let plain = asciidoc_plain(before, near(cursor), text);
                    (text.to_owned(), at > 0 && plain)
                }
            },
            None => (text.to_owned(), false),
        };
        // A typed character can change where a bare link ends.
        let fast = fast && link_at(&self.blocks, byte).is_none();
        source.replace(cursor..cursor, &raw);
        // Typing cannot change the structure of the blocks, so the new
        // characters join their block without parsing again (ADR-012).
        if let (true, Some((index, at))) = (fast, position) {
            if insert_typed(&mut self.blocks, index, at, byte, text, &raw) {
                self.parsed = Some(source.document().revision());
                self.tops.clear();
            }
        }
        self.scroll_to_cursor = true;
        true
    }

    fn delete_selection(&mut self, source: &mut SourceView) -> bool {
        let (start, end) = source.selection();
        if start == end {
            return false;
        }
        source.replace(start..end, "");
        true
    }

    fn key(
        &mut self,
        ui: &egui::Ui,
        source: &mut SourceView,
        theme: &Theme,
        key: Key,
        modifiers: egui::Modifiers,
    ) -> bool {
        let Some((index, at)) = self.cursor_position(source) else {
            if key == Key::Enter {
                return self.type_text(source, "\n");
            }
            return false;
        };
        if let Some(tool) = Tool::for_key(key, modifiers) {
            return source.apply_tool(tool);
        }
        let count = self.blocks[index].text().chars().count();
        let mut target = None;
        match key {
            Key::Z if modifiers.command && modifiers.shift => return source.redo(),
            Key::Z if modifiers.command => return source.undo(),
            Key::Y if modifiers.command => return source.redo(),
            Key::A if modifiers.command => {
                source.set_cursor_anchor(source.document().len_chars(), 0);
                return false;
            }
            Key::Enter => return self.enter(source, index, at),
            Key::Backspace => {
                if self.delete_selection(source) {
                    return true;
                }
                return self.delete_back(source, index, at);
            }
            Key::Delete => {
                if self.delete_selection(source) {
                    return true;
                }
                if at < count {
                    return self.delete_back(source, index, at + 1);
                }
                if index + 1 < self.blocks.len() {
                    return self.delete_back(source, index + 1, 0);
                }
                return false;
            }
            Key::ArrowLeft => {
                target = Some(if at > 0 {
                    self.char_of(source, index, at - 1)
                } else if index > 0 {
                    let previous = self.blocks[index - 1].text().chars().count();
                    self.char_of(source, index - 1, previous)
                } else {
                    self.char_of(source, index, 0)
                });
            }
            Key::ArrowRight => {
                target = Some(if at < count {
                    self.char_of(source, index, at + 1)
                } else if index + 1 < self.blocks.len() {
                    self.char_of(source, index + 1, 0)
                } else {
                    self.char_of(source, index, count)
                });
            }
            Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => {
                let laid = self.laid(ui, source, theme, index);
                let galley = &laid.galley;
                let current = galley.from_ccursor(CCursor::new(laid.prefix + at));
                let moved = match key {
                    Key::ArrowUp => galley.cursor_up_one_row(&current),
                    Key::ArrowDown => galley.cursor_down_one_row(&current),
                    Key::Home => galley.cursor_begin_of_row(&current),
                    _ => galley.cursor_end_of_row(&current),
                };
                let same_row = moved.rcursor.row == current.rcursor.row;
                target = Some(match key {
                    Key::ArrowUp if same_row && index > 0 => {
                        let previous = self.blocks[index - 1].text().chars().count();
                        self.char_of(source, index - 1, previous.min(at))
                    }
                    Key::ArrowDown if same_row && index + 1 < self.blocks.len() => {
                        let next = self.blocks[index + 1].text().chars().count();
                        self.char_of(source, index + 1, at.min(next))
                    }
                    _ => {
                        let at = moved.ccursor.index.saturating_sub(laid.prefix).min(count);
                        self.char_of(source, index, at)
                    }
                });
            }
            _ => {}
        }
        if let Some(at) = target {
            let (_, anchor) = source.cursor_anchor();
            source.set_cursor_anchor(at, if modifiers.shift { anchor } else { at });
            self.scroll_to_cursor = true;
        }
        false
    }

    /// Enter: a new paragraph, a new list item with the same marker, or a
    /// new line in code and markup.
    fn enter(&mut self, source: &mut SourceView, index: usize, _at: usize) -> bool {
        self.delete_selection(source);
        self.reparse(source);
        let block = &self.blocks[index];
        let text = source.document().text();
        let insert = match &block.kind {
            BlockKind::Code { .. } | BlockKind::Markup => "\n".to_owned(),
            BlockKind::ListItem { .. } => {
                let line = &text[block.span.clone()];
                format!("\n{}", next_marker(line.lines().next().unwrap_or("")))
            }
            BlockKind::Quote(depth) if source.format() == NoteFormat::Markdown => {
                format!("\n\n{} ", ">".repeat(*depth))
            }
            _ => "\n\n".to_owned(),
        };
        let (cursor, _) = source.cursor_anchor();
        source.replace(cursor..cursor, &insert);
        self.scroll_to_cursor = true;
        true
    }

    /// Deletes the facade character before index `at` of block `index`; at
    /// the start of a block, joins it with the block before.
    fn delete_back(&mut self, source: &mut SourceView, index: usize, at: usize) -> bool {
        let text = source.document().text();
        let block = &self.blocks[index];
        let range = if at > 0 {
            let start = block.source_offset(at - 1);
            let end = start + text[start..].chars().next().map_or(0, char::len_utf8);
            // An escaped character goes with its backslash.
            let start = if start > 0
                && text.as_bytes()[start - 1] == b'\\'
                && text[start..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_punctuation())
            {
                start - 1
            } else {
                start
            };
            start..end
        } else if index > 0 {
            let previous = &self.blocks[index - 1];
            let start = match previous.kind {
                BlockKind::ThematicBreak => previous.span.start,
                _ => previous.source_offset(previous.text().chars().count()),
            };
            start..block.source_offset(0)
        } else {
            return false;
        };
        if range.is_empty() {
            return false;
        }
        let rope = source.document().rope();
        let (start, end) = (rope.byte_to_char(range.start), rope.byte_to_char(range.end));
        source.replace(start..end, "");
        self.scroll_to_cursor = true;
        true
    }
}

/// Colors the facade uses.
struct Theme {
    text: Color32,
    strong: Color32,
    link: Color32,
    code_bg: Color32,
    weak: Color32,
}

/// The layout of one block and the number of marker chars before its text.
fn block_job(block: &Block, theme: &Theme, width: f32, source: &SourceView) -> (LayoutJob, usize) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let heading = matches!(block.kind, BlockKind::Heading(_));
    let size = match block.kind {
        BlockKind::Heading(1) => 26.0,
        BlockKind::Heading(2) => 22.0,
        BlockKind::Heading(3) => 19.0,
        BlockKind::Heading(4) => 17.0,
        BlockKind::Heading(_) => 16.0,
        _ => BODY_SIZE,
    };
    let body = match block.kind {
        BlockKind::Code { .. } | BlockKind::Markup | BlockKind::Image { .. } => {
            FontId::monospace(CODE_SIZE)
        }
        _ => FontId::new(size, fonts::family(heading, false)),
    };
    let prefix = prefix_text(&block.kind);
    let prefix_chars = prefix.chars().count();
    if !prefix.is_empty() {
        job.append(&prefix, 0.0, TextFormat::simple(body.clone(), theme.weak));
    }
    if let BlockKind::Code { language } = &block.kind {
        let code = block.text();
        let lines = highlight_code(&source.highlight_set(), language, &code);
        for (line, spans) in code
            .split_inclusive('\n')
            .zip(lines.iter().chain(std::iter::repeat(&Vec::new())))
        {
            let mut end = 0;
            for span in spans {
                if span.start > end {
                    job.append(
                        &line[end..span.start],
                        0.0,
                        TextFormat::simple(body.clone(), theme.text),
                    );
                }
                let stop = span.end.min(line.len());
                if span.start < stop {
                    job.append(
                        &line[span.start..stop],
                        0.0,
                        TextFormat::simple(body.clone(), rgba(span.color)),
                    );
                    end = stop;
                }
            }
            if end < line.len() {
                job.append(
                    &line[end..],
                    0.0,
                    TextFormat::simple(body.clone(), theme.text),
                );
            }
        }
        return (job, prefix_chars);
    }
    for run in &block.runs {
        let style = &run.style;
        let font = if style.code {
            FontId::monospace(CODE_SIZE)
        } else {
            FontId::new(size, fonts::family(style.bold || heading, style.italic))
        };
        let mut format = TextFormat::simple(
            font,
            if style.link.is_some() {
                theme.link
            } else if style.bold || heading {
                theme.strong
            } else {
                theme.text
            },
        );
        if style.strike {
            format.strikethrough = Stroke::new(1.0_f32, theme.text);
        }
        if style.link.is_some() {
            format.underline = Stroke::new(1.0_f32, theme.link);
        }
        if style.code {
            format.background = theme.code_bg;
        }
        job.append(&run.text, 0.0, format);
    }
    if block.runs.is_empty() {
        job.append("", 0.0, TextFormat::simple(body, theme.text));
    }
    (job, prefix_chars)
}

/// Where the image `target` of a note is: a web address, or a file found as
/// links are found (UC-004 BR-005); otherwise why it is not shown.
fn image_source(place: Option<&(PathBuf, PathBuf)>, target: &str) -> Result<ImageSource, String> {
    let Some((root, note)) = place else {
        return Err(target.to_owned());
    };
    match resolve_link(root, note, target) {
        LinkTarget::Web(address) => Ok(ImageSource::Web(address)),
        LinkTarget::File(path) => Ok(ImageSource::File(path)),
        LinkTarget::Note { id, .. } => Ok(ImageSource::File(root.join(id.as_str()))),
        LinkTarget::MissingNote { .. } | LinkTarget::Heading(_) => {
            let base = note.parent().unwrap_or(root);
            Ok(ImageSource::File(base.join(target)))
        }
        LinkTarget::Outside => Err(format!("{target} lies outside the workspace")),
    }
}

/// The diagram language of a code block, if it is a diagram.
fn diagram_language(kind: &BlockKind) -> Option<DiagramLanguage> {
    match kind {
        BlockKind::Code { language } => DiagramLanguage::from_code_language(language),
        _ => None,
    }
}

/// The marker text the facade shows before a block's text.
fn prefix_text(kind: &BlockKind) -> String {
    let mut prefix = String::new();
    match kind {
        BlockKind::ListItem {
            depth,
            marker,
            checked,
        } => {
            prefix.push_str(&"    ".repeat(*depth));
            match marker {
                Some(ListMarker::Bullet) => prefix.push_str("•  "),
                Some(ListMarker::Number(n)) => prefix.push_str(&format!("{n}.  ")),
                None => prefix.push_str("   "),
            }
            match checked {
                Some(true) => prefix.push_str("☑ "),
                Some(false) => prefix.push_str("☐ "),
                None => {}
            }
        }
        BlockKind::Quote(depth) => prefix.push_str(&"    ".repeat(*depth)),
        BlockKind::Admonition(label) => {
            prefix.push_str("    ");
            prefix.push_str(label);
            prefix.push_str("  ");
        }
        _ => {}
    }
    prefix
}

fn prefix_chars(kind: &BlockKind) -> usize {
    prefix_text(kind).chars().count()
}

fn paint_selection(
    painter: &egui::Painter,
    laid: &Laid,
    top: Pos2,
    from: usize,
    to: usize,
    color: Color32,
) {
    let start = laid
        .galley
        .pos_from_ccursor(CCursor::new(laid.prefix + from));
    let end = laid.galley.pos_from_ccursor(CCursor::new(laid.prefix + to));
    for row in &laid.galley.rows {
        let (y0, y1) = (row.rect.min.y, row.rect.max.y);
        if y1 <= start.min.y || y0 >= end.max.y {
            continue;
        }
        let x0 = if (start.min.y - y0).abs() < 1.0 {
            start.min.x
        } else {
            row.rect.min.x
        };
        let x1 = if (end.min.y - y0).abs() < 1.0 {
            end.min.x
        } else {
            row.rect.max.x
        };
        painter.rect_filled(
            Rect::from_min_max(top + Vec2::new(x0, y0), top + Vec2::new(x1, y1)),
            0.0,
            color,
        );
    }
}

/// Escapes characters that Markdown would read as formatting.
fn escape_markdown(text: &str, at_block_start: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.chars().enumerate() {
        let start = at_block_start && i == 0;
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '~')
            || (start && matches!(c, '#' | '-' | '+' | '|'))
        {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Whether typing `text` between the characters `before` and `after` of an
/// AsciiDoc note keeps the blocks as they are: no character that could
/// start markup, and plain text or spaces on both sides, so no formatting
/// begins or ends there.
fn asciidoc_plain(before: Option<char>, after: Option<char>, text: &str) -> bool {
    let plain = |c: char| c.is_alphanumeric() || c == ' ';
    text.chars().all(|c| plain(c) || ",;!?".contains(c))
        && before.is_some_and(plain)
        && after.is_none_or(|c| plain(c) || c == '\n')
}

/// The marker of the next list item after the item line `line`: same
/// indent, same bullet (`**` in AsciiDoc for the second level), the next
/// number, and an open task box for tasks.
fn next_marker(line: &str) -> String {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &line[indent.len()..];
    let task = |after: &str| {
        let boxes = ["[ ] ", "[x] ", "[X] ", "[*] "];
        if boxes.iter().any(|b| after.starts_with(b)) {
            "[ ] "
        } else {
            ""
        }
    };
    if let Some(bullet) = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '-' | '*' | '+' | '.'))
    {
        let count = rest.chars().take_while(|&c| c == bullet).count();
        let marker = &rest[..count];
        let after = rest.get(count + 1..).unwrap_or("");
        return format!("{indent}{marker} {}", task(after));
    }
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if let Ok(number) = digits.parse::<usize>() {
        let delimiter = rest[digits.len()..].chars().next().unwrap_or('.');
        let after = rest.get(digits.len() + 2..).unwrap_or("");
        return format!("{indent}{}{delimiter} {}", number + 1, task(after));
    }
    format!("{indent}- ")
}

fn rgba([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use underline_facade::highlight::HighlightSet;

    struct Harness {
        ctx: egui::Context,
        source: SourceView,
        facade: FacadeView,
    }

    impl Harness {
        fn new(text: &str) -> Self {
            Self::with_format(text, NoteFormat::Markdown)
        }

        fn with_format(text: &str, format: NoteFormat) -> Self {
            let set = Arc::new(HighlightSet::new(true));
            let ctx = egui::Context::default();
            fonts::install(&ctx);
            let mut harness = Self {
                ctx,
                source: SourceView::new("t", text, format, set),
                facade: FacadeView::new("t"),
            };
            harness.facade.request_focus();
            harness.frame(vec![]);
            harness.frame(vec![]);
            harness
        }

        fn frame(&mut self, events: Vec<Event>) {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                events,
                ..Default::default()
            };
            let (source, facade) = (&mut self.source, &mut self.facade);
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    facade.show(ui, source);
                });
            });
        }

        /// Puts the cursor after the first occurrence of `marker` in the text.
        fn cursor_after(&mut self, marker: &str) {
            let text = self.source.document().text();
            let byte = text.find(marker).unwrap() + marker.len();
            let at = self.source.document().rope().byte_to_char(byte);
            self.source.set_cursor_anchor(at, at);
        }

        fn text(&self) -> String {
            self.source.document().text()
        }

        fn key(&mut self, key: Key) {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
        }
    }

    /// UC-005 step 3, BR-002: typing in the facade changes only the edited
    /// block; formatting characters are escaped.
    #[test]
    fn uc005_typing_changes_only_the_edited_block() {
        let note =
            "# Title\n\nSome *emph*  text.\n\n* star  bullets\n-  dash\n\n| a | b |\n|-|-|\n";
        let mut h = Harness::new(note);
        h.cursor_after("Some *emph");
        h.frame(vec![Event::Text("asis 2*3".into())]);
        assert_eq!(
            h.text(),
            "# Title\n\nSome *emphasis 2\\*3*  text.\n\n* star  bullets\n-  dash\n\n| a | b |\n|-|-|\n"
        );
    }

    /// UC-005: Enter in a list item starts the next item with its marker;
    /// in a paragraph a new paragraph.
    #[test]
    fn uc005_enter_continues_lists_and_paragraphs() {
        let mut h = Harness::new("1. first\n\nPara\n");
        h.cursor_after("first");
        h.key(Key::Enter);
        h.frame(vec![Event::Text("second".into())]);
        assert_eq!(h.text(), "1. first\n2. second\n\nPara\n");
        h.cursor_after("Para");
        h.key(Key::Enter);
        h.frame(vec![Event::Text("Next".into())]);
        assert_eq!(h.text(), "1. first\n2. second\n\nPara\n\nNext\n");
    }

    /// UC-005, ADR-003: in an AsciiDoc note, typed text joins its block
    /// as a new parse would see it, and Enter continues a list with its
    /// marker.
    #[test]
    fn uc005_typing_and_enter_in_asciidoc() {
        let note = "== Plan\n\nSome *bold* text and https://x.org now.\n\n* one\n** inner\n\nNOTE: Mind it.\n";
        let mut h = Harness::with_format(note, NoteFormat::AsciiDoc);
        for (after, typed) in [
            ("Some *bo", "ld"),
            ("* te", "x t"),
            ("https://x.org", "/a"),
            ("Mind", " all of"),
        ] {
            h.cursor_after(after);
            h.frame(vec![Event::Text(typed.into())]);
            let fresh = parse_blocks(&h.text(), NoteFormat::AsciiDoc);
            assert_eq!(h.facade.blocks(), &fresh[..], "after {after:?}");
        }
        assert!(
            h.text()
                .contains("Some *boldld* tex txt and https://x.org/a now."),
            "{}",
            h.text()
        );
        assert!(h.text().contains("NOTE: Mind all of it."));
        h.cursor_after("** inner");
        h.key(Key::Enter);
        h.frame(vec![Event::Text("deeper".into())]);
        assert!(h.text().contains("** inner\n** deeper\n"), "{}", h.text());
    }

    /// FR-036, UC-005 BR-008: a diagram shows its picture; with the cursor
    /// in it, its source below the picture; with an error, its source and
    /// the error (UC-004 A11).
    #[test]
    fn uc005_diagrams_show_pictures_and_their_source_when_edited() {
        let note = "Intro\n\n```plantuml\nAlice -> Bob : hi\n```\n\n```plantuml\nclass A {\nA -> -> B\n```\n\nEnd\n";
        let mut h = Harness::new(note);
        h.facade.diagrams = Some(Arc::new(Diagrams::start(|| {})));
        h.cursor_after("End");
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let (good, bad) = (1, 2);
        let figure = |h: &mut Harness, index: usize| {
            let (ctx, source) = (h.ctx.clone(), &h.source);
            let mut kind = String::new();
            let mut shown = false;
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let theme = Theme {
                        text: Color32::WHITE,
                        strong: Color32::WHITE,
                        link: Color32::WHITE,
                        code_bg: Color32::BLACK,
                        weak: Color32::GRAY,
                    };
                    let laid = h.facade.laid(ui, source, &theme, index);
                    shown = laid.text_shown;
                    kind = match laid.figure {
                        Some(Figure::Picture(..)) => "picture".into(),
                        Some(Figure::Message(m)) if m.text().contains("cannot be shown") => {
                            format!("error: {}", m.text())
                        }
                        Some(Figure::Message(_)) => "pending".into(),
                        None => "none".into(),
                    };
                });
            });
            (kind, shown)
        };
        loop {
            h.frame(vec![]);
            let (a, _) = figure(&mut h, good);
            let (b, _) = figure(&mut h, bad);
            if a == "picture" && b.starts_with("error") {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "{a} / {b}");
            std::thread::sleep(Duration::from_millis(50));
        }
        let (_, shown) = figure(&mut h, good);
        assert!(!shown, "the source is hidden without the cursor");
        let (error, shown) = figure(&mut h, bad);
        assert!(shown && error.contains("Line 1: Syntax Error"), "{error}");
        h.cursor_after("Alice ");
        h.frame(vec![]);
        let (kind, shown) = figure(&mut h, good);
        assert_eq!((kind.as_str(), shown), ("picture", true));
        // Typing in the source keeps the last picture until the new one.
        // (The checks above ran frames without the facade, which drops the
        // focus.)
        h.facade.request_focus();
        h.frame(vec![]);
        h.frame(vec![Event::Text("x".into())]);
        assert!(h.text().contains("Alice x-> Bob"));
        let (kind, _) = figure(&mut h, good);
        assert_eq!(kind, "picture");
    }

    /// UC-004 step 3, TC-004-02, TC-004-06: an image shows its picture
    /// with its caption, its markup while the cursor is in it, and a
    /// missing image says so.
    #[test]
    fn uc004_images_show_pictures_and_captions() {
        let dir = std::env::temp_dir().join(format!("underline-facade-img-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("img")).unwrap();
        std::fs::write(
            dir.join("img/arch.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><rect width="120" height="60" fill="blue"/></svg>"#,
        )
        .unwrap();
        let note = ".The architecture\nimage::img/arch.svg[Architecture]\n\nimage::img/gone.png[Gone]\n\nEnd\n";
        let mut h = Harness::with_format(note, NoteFormat::AsciiDoc);
        let facade = std::mem::replace(&mut h.facade, FacadeView::new("x"));
        h.facade = facade.with_images(
            Arc::new(Images::start(|| {})),
            dir.clone(),
            dir.join("Guide.adoc"),
        );
        h.facade.request_focus();
        h.cursor_after("End");
        let theme = Theme {
            text: Color32::WHITE,
            strong: Color32::WHITE,
            link: Color32::WHITE,
            code_bg: Color32::BLACK,
            weak: Color32::GRAY,
        };
        let look = |h: &mut Harness, index: usize| {
            let ctx = h.ctx.clone();
            let mut out = (String::new(), false, None);
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let laid = h.facade.laid(ui, &h.source, &theme, index);
                    let kind = match &laid.figure {
                        Some(Figure::Picture(_, size)) => format!("picture {}x{}", size.x, size.y),
                        Some(Figure::Message(m)) => m.text().to_owned(),
                        None => "none".into(),
                    };
                    out = (
                        kind,
                        laid.text_shown,
                        laid.caption.map(|(_, g)| g.text().to_owned()),
                    );
                });
            });
            out
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            h.frame(vec![]);
            let (kind, _, _) = look(&mut h, 0);
            if kind.starts_with("picture") {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "{kind}");
            std::thread::sleep(Duration::from_millis(20));
        }
        let (kind, shown, caption) = look(&mut h, 0);
        assert_eq!(kind, "picture 120x60");
        assert!(!shown);
        assert_eq!(caption.as_deref(), Some("Figure 1. The architecture"));
        let (missing, _, _) = look(&mut h, 1);
        assert!(
            missing.contains("Gone") && missing.contains("gone.png"),
            "{missing}"
        );
        h.cursor_after("image::img/arch");
        h.frame(vec![]);
        let (_, shown, _) = look(&mut h, 0);
        assert!(shown, "the markup shows while the cursor is in the image");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// FR-039, UC-004 BR-011: a picture is enlarged over the note; keys do
    /// not reach the note meanwhile, and escape closes the view.
    #[test]
    fn uc004_br011_enlarged_view_opens_and_closes() {
        let dir = std::env::temp_dir().join(format!("underline-enlarge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("shape.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="40" height="30"/></svg>"#,
        )
        .unwrap();
        let note = "![Shape](shape.svg)\n\nEnd\n";
        let mut h = Harness::new(note);
        let facade = std::mem::replace(&mut h.facade, FacadeView::new("x"));
        h.facade = facade.with_images(
            Arc::new(Images::start(|| {})),
            dir.clone(),
            dir.join("N.md"),
        );
        h.facade.request_focus();
        h.cursor_after("End");
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !h.facade.last_pictures.contains_key(&0) {
            h.frame(vec![]);
            assert!(std::time::Instant::now() < deadline, "no picture");
            std::thread::sleep(Duration::from_millis(20));
        }
        let ctx = h.ctx.clone();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| h.facade.enlarge(ui, 0));
        });
        assert!(h.facade.is_enlarged());
        h.facade.request_focus();
        h.frame(vec![]);
        h.frame(vec![Event::Text("x".into())]);
        assert_eq!(h.text(), note, "typing does not reach the note");
        assert!(h.facade.is_enlarged());
        h.key(Key::Escape);
        assert!(!h.facade.is_enlarged());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-005: Backspace deletes an escaped character with its backslash, and
    /// at the start of a block joins it with the block before.
    #[test]
    fn uc005_backspace_deletes_characters_and_joins_blocks() {
        let mut h = Harness::new("# Head\n\nA \\* b\n");
        h.cursor_after("A \\*");
        h.key(Key::Backspace);
        assert_eq!(h.text(), "# Head\n\nA  b\n");
        h.cursor_after("# Head\n\n");
        h.key(Key::Backspace);
        assert_eq!(h.text(), "# HeadA  b\n");
    }

    /// TC-005-09: toolbar keys work in the facade on the selection, and a
    /// link asks first.
    #[test]
    fn uc005_tc09_toolbar_keys_in_the_facade() {
        let mut h = Harness::new("Some word here\n");
        let text = h.text();
        let start = text.find("word").unwrap();
        h.source.set_cursor_anchor(start + 4, start);
        let ctrl = |key, shift| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                ..Default::default()
            },
        };
        h.frame(vec![ctrl(Key::B, false)]);
        assert_eq!(h.text(), "Some **word** here\n");
        h.frame(vec![ctrl(Key::B, false)]);
        assert_eq!(h.text(), "Some word here\n");
        h.frame(vec![ctrl(Key::L, true)]);
        assert_eq!(h.text(), "- Some word here\n");
        h.frame(vec![ctrl(Key::K, false)]);
        assert_eq!(h.source.take_request(), Some(Tool::Link));
    }

    #[test]
    fn next_list_markers() {
        assert_eq!(next_marker("- item"), "- ");
        assert_eq!(next_marker("  * [x] done"), "  * [ ] ");
        assert_eq!(next_marker("9. nine"), "10. ");
        assert_eq!(next_marker("3) three"), "4) ");
    }

    /// ADR-012 / NFR-002: typing in the facade of a 10,000-line note.
    /// Run with `cargo test --release -p underline-editor -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn adr012_facade_typing_in_10000_lines() {
        let mut note = String::new();
        let mut n = 0;
        while note.lines().count() < 10_000 {
            note.push_str(&format!("## Section {n}\n\nSome *prose* with **bold** and a [link](https://x.org/{n}).\n\n- item one\n- item two\n\n```rust\nfn f{n}() -> u32 {{ {n} }}\n```\n\n"));
            n += 1;
        }
        let mut h = Harness::new(&note);
        let target = format!("Section {}", n / 2);
        h.cursor_after(&target);
        h.facade.request_focus();
        h.frame(vec![]);
        let mut times = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            h.frame(vec![Event::Text("x".into())]);
            times.push(start.elapsed());
        }
        times.sort();
        let (median, p99) = (times[50], times[98]);
        println!(
            "facade typing: median {median:?}, p99 {p99:?}, max {:?}",
            times[99]
        );
        assert!(p99 < Duration::from_millis(16), "p99 {p99:?}");
    }
}
