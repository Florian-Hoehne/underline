//! The enlarged view of an image or a diagram (FR-039, UC-004 BR-011): the
//! picture over the whole window, zoomed with the wheel or the keys, moved by
//! dragging, fitted again with `0`, and closed with escape, a click beside it,
//! or its close button. Zoomed in, the picture is rendered again at a higher
//! scale, so its details stay sharp (ADR-009).

use std::sync::Arc;

use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2};
use underline_facade::diagram::{DiagramLanguage, DiagramState, Diagrams, Picture};
use underline_facade::images::{ImageSource, ImageState, Images};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 16.0;

/// What the enlarged picture shows, to render it again at a higher scale.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Shown {
    Diagram {
        language: DiagramLanguage,
        source: String,
        dark: bool,
    },
    Image(ImageSource),
}

impl Shown {
    /// Whether the picture is drawn from lines and shapes, so it gets
    /// sharper at a higher scale; a bitmap does not.
    fn is_vector(&self) -> bool {
        match self {
            Self::Diagram { .. } => true,
            Self::Image(ImageSource::File(path)) => path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("svg")),
            Self::Image(ImageSource::Web(address)) => address
                .split(['?', '#'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .ends_with(".svg"),
        }
    }
}

/// An open enlarged view.
pub(crate) struct Enlarged {
    shown: Shown,
    /// The picture as the facade shows it, until a sharper one is ready.
    picture: Arc<Picture>,
    texture: Option<(usize, egui::TextureHandle)>,
    /// Points of the screen per point of the picture; `None` fits it.
    zoom: Option<f32>,
    /// How far the picture is moved from the middle of the window.
    offset: Vec2,
}

impl Enlarged {
    pub(crate) fn new(shown: Shown, picture: Arc<Picture>) -> Self {
        Self {
            shown,
            picture,
            texture: None,
            zoom: None,
            offset: Vec2::ZERO,
        }
    }

    /// The zoom that fits the picture into `area`, at most its own size.
    fn fit(&self, area: Rect) -> f32 {
        let [w, h] = self.picture_size();
        ((area.width() - 64.0) / w)
            .min((area.height() - 64.0) / h)
            .clamp(MIN_ZOOM, 1.0_f32.max(MIN_ZOOM))
    }

    /// The size of the picture in points, the same at every scale.
    fn picture_size(&self) -> [f32; 2] {
        self.picture.size()
    }

    /// Draws the view over everything and handles its input. Returns
    /// whether it stays open.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        diagrams: Option<&Diagrams>,
        images: Option<&Images>,
    ) -> bool {
        let screen = ctx.screen_rect();
        let mut open = true;
        egui::Area::new(egui::Id::new("underline-enlarged"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, Color32::from_black_alpha(235));

                let fit = self.fit(rect);
                let mut zoom = self.zoom.unwrap_or(fit);
                // Zooming keeps the point under the pointer in its place.
                let (scroll, pinch, keys) = ui.input(|i| {
                    let mut keys = 1.0;
                    if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                        keys *= 1.25;
                    }
                    if i.key_pressed(Key::Minus) {
                        keys /= 1.25;
                    }
                    (i.raw_scroll_delta.y, i.zoom_delta(), keys)
                });
                let factor = (scroll / 300.0).exp() * pinch * keys;
                if (factor - 1.0).abs() > f32::EPSILON {
                    let pivot =
                        response.hover_pos().unwrap_or(rect.center()) - rect.center() - self.offset;
                    let new_zoom = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
                    self.offset -= pivot * (new_zoom / zoom - 1.0);
                    zoom = new_zoom;
                    self.zoom = Some(zoom);
                }
                if response.dragged() {
                    self.offset += response.drag_delta();
                }
                if ui.input(|i| i.key_pressed(Key::Num0)) {
                    self.zoom = None;
                    self.offset = Vec2::ZERO;
                    zoom = fit;
                }

                self.sharpen(ctx, zoom, diagrams, images);
                let [w, h] = self.picture_size();
                let size = Vec2::new(w, h) * zoom;
                let picture_rect = Rect::from_center_size(rect.center() + self.offset, size);
                let texture = self.texture(ctx);
                // A transparent picture is shown on the background of the
                // facade it was made for, not on the note behind.
                painter.rect_filled(picture_rect, 0.0, ui.visuals().extreme_bg_color);
                painter.image(
                    texture,
                    picture_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                // The close button, and the zoom in percent.
                let close = Rect::from_center_size(
                    rect.right_top() + Vec2::new(-28.0, 28.0),
                    Vec2::splat(32.0),
                );
                let close_response = ui.interact(close, ui.id().with("close"), Sense::click());
                painter.circle_filled(close.center(), 18.0, Color32::from_black_alpha(170));
                let color = if close_response.hovered() {
                    Color32::WHITE
                } else {
                    Color32::from_gray(190)
                };
                let cross = 9.0;
                for (a, b) in [
                    (Vec2::new(-cross, -cross), Vec2::new(cross, cross)),
                    (Vec2::new(-cross, cross), Vec2::new(cross, -cross)),
                ] {
                    painter.line_segment(
                        [close.center() + a, close.center() + b],
                        Stroke::new(2.0_f32, color),
                    );
                }
                let hint = rect.center_bottom() - Vec2::new(0.0, 16.0);
                painter.rect_filled(
                    Rect::from_center_size(hint - Vec2::new(0.0, 8.0), Vec2::new(500.0, 24.0)),
                    6.0,
                    Color32::from_black_alpha(170),
                );
                painter.text(
                    hint,
                    egui::Align2::CENTER_BOTTOM,
                    format!(
                        "{:.0} %   ·   wheel or +/− to zoom, drag to move, 0 to fit, Esc to close",
                        zoom * 100.0
                    ),
                    egui::FontId::proportional(12.0),
                    Color32::from_gray(190),
                );

                let beside = response.clicked()
                    && response
                        .interact_pointer_pos()
                        .is_some_and(|p| !picture_rect.contains(p));
                if close_response.clicked() || beside || ui.input(|i| i.key_pressed(Key::Escape)) {
                    open = false;
                }
            });
        open
    }

    /// Asks for the picture at the scale the zoom needs; a vector picture
    /// (diagram or SVG image) comes back sharper, a bitmap as it is.
    fn sharpen(
        &mut self,
        ctx: &egui::Context,
        zoom: f32,
        diagrams: Option<&Diagrams>,
        images: Option<&Images>,
    ) {
        // Steps of two, so zooming asks for few scales.
        let wanted = ctx.pixels_per_point() * zoom.max(1.0);
        let scale = 2.0_f32.powf(wanted.log2().ceil()).min(8.0);
        if !self.shown.is_vector() || self.picture.scale >= scale {
            return;
        }
        let picture = match &self.shown {
            Shown::Diagram {
                language,
                source,
                dark,
            } => match diagrams.map(|d| d.get(*language, source, *dark, scale)) {
                Some(DiagramState::Ready(picture)) => picture,
                _ => return,
            },
            Shown::Image(source) => match images.map(|i| i.get(source, scale)) {
                Some(ImageState::Ready(picture)) => picture,
                _ => return,
            },
        };
        if picture.scale > self.picture.scale {
            self.picture = picture;
        }
    }

    fn texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        let key = Arc::as_ptr(&self.picture) as usize;
        if let Some((shown, texture)) = &self.texture {
            if *shown == key {
                return texture.id();
            }
        }
        let image = egui::ColorImage::from_rgba_premultiplied(
            [self.picture.width as usize, self.picture.height as usize],
            &self.picture.pixels,
        );
        let texture = ctx.load_texture("enlarged", image, egui::TextureOptions::LINEAR);
        let id = texture.id();
        self.texture = Some((key, texture));
        id
    }
}
