//! The minimap (FR-030, UC-004 BR-013): a narrow strip beside the editor
//! with the whole note scaled down, each line or block as a bar, and the
//! visible part marked. A click puts that place in view; dragging moves the
//! view along. Both editing views give it their bars as [`Mark`]s.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

/// Width of the strip.
pub(crate) const WIDTH: f32 = 80.0;
/// Space between the editor and the strip.
const GAP: f32 = 4.0;
/// The largest scale: a short note is not stretched over the strip.
const MAX_SCALE: f32 = 0.2;
/// How long a blinking search hit is shown, in seconds.
const FOUND_ON: f64 = 0.6;
/// How long it is hidden then, in seconds.
const FOUND_OFF: f64 = 0.4;

/// UC-008 BR-008: whether the search hits are shown in the strip now; they
/// blink when `blink` is set (a setting), and ask for the frame that turns
/// them on or off.
pub(crate) fn found_shown(ui: &egui::Ui, blink: bool) -> bool {
    if !blink {
        return true;
    }
    let phase = ui.input(|i| i.time) % (FOUND_ON + FOUND_OFF);
    let shown = phase < FOUND_ON;
    let turn = if shown {
        FOUND_ON
    } else {
        FOUND_ON + FOUND_OFF
    };
    let next = turn - phase;
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs_f64(next.max(0.01)));
    shown
}

/// One bar: where it is in the view's content, how much of the width it
/// takes (0 to 1), and its color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Mark {
    pub top: f32,
    pub height: f32,
    pub width: f32,
    pub color: Color32,
}

/// UC-008 BR-008: one search hit in the strip: where it is in the view's
/// content, and where in the width (0 to 1), so only the word is marked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hit {
    pub top: f32,
    pub height: f32,
    pub left: f32,
    pub width: f32,
}

/// UC-008 BR-008: the search hits of a view, and whether they are shown in
/// this frame (they blink).
pub(crate) struct Hits<'a> {
    pub hits: &'a [Hit],
    pub shown: bool,
    pub color: Color32,
}

/// Splits the space of `ui` into the editor and, when `shown`, the strip.
pub(crate) fn split(ui: &egui::Ui, shown: bool) -> (Rect, Option<Rect>) {
    let full = ui.available_rect_before_wrap();
    if !shown || full.width() < 3.0 * WIDTH {
        return (full, None);
    }
    let strip = Rect::from_min_max(Pos2::new(full.right() - WIDTH, full.top()), full.max);
    let editor = Rect::from_min_max(full.min, Pos2::new(strip.left() - GAP, full.bottom()));
    (editor, Some(strip))
}

/// The scale from content to strip for `content` points of content.
fn scale(strip: Rect, content: f32) -> f32 {
    (strip.height() / content.max(1.0)).min(MAX_SCALE)
}

/// Draws the strip in `strip` for content of `content` points, of which
/// `visible` (from, to) is in view, with the search hits `found` over it,
/// and handles its pointer. Returns the scroll offset to show, when the
/// user clicked or dragged.
///
/// While there are hits the strip works like a sieve: the note behind it
/// is veiled, and the hits shine through where they are, with a glow.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(
    ui: &egui::Ui,
    id: egui::Id,
    strip: Rect,
    marks: &[Mark],
    found: Hits<'_>,
    content: f32,
    visible: (f32, f32),
    background: Color32,
) -> Option<f32> {
    let response = ui.interact(strip, id.with("minimap"), Sense::click_and_drag());
    let painter = ui.painter_at(strip);
    painter.rect_filled(strip, 0.0, background);
    let scale = scale(strip, content);
    // One bar per pixel row of the strip: the widest mark there, in its
    // color; so a long note costs no more to draw than a short one.
    let rows = strip.height().ceil().max(1.0) as usize;
    let mut bars: Vec<Option<(f32, Color32)>> = vec![None; rows];
    for mark in marks {
        if mark.height <= 0.0 {
            continue;
        }
        let first = (mark.top * scale).floor().max(0.0) as usize;
        let last = ((mark.top + mark.height) * scale).ceil() as usize;
        let end = last.max(first + 1).min(rows);
        for bar in bars.iter_mut().take(end).skip(first) {
            match bar {
                Some((width, color)) if *width >= mark.width => {
                    if mark.color.a() > color.a() {
                        *color = mark.color;
                    }
                }
                bar => *bar = Some((mark.width, mark.color)),
            }
        }
    }
    let inner = strip.shrink2(Vec2::new(6.0, 0.0));
    for (row, bar) in bars.iter().enumerate() {
        if let Some((width, color)) = bar {
            let y = strip.top() + row as f32;
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(inner.left(), y),
                    Vec2::new(inner.width() * width.clamp(0.05, 1.0), 1.0),
                ),
                0.0,
                *color,
            );
        }
    }
    // UC-008 BR-008: the sieve. While there are hits the note is veiled,
    // under the mark of the visible part, and the hits blink through it.
    if !found.hits.is_empty() {
        painter.rect_filled(strip, 0.0, Color32::from_black_alpha(150));
    }
    // The visible part.
    let (from, to) = visible;
    let marked = Rect::from_min_max(
        Pos2::new(strip.left(), strip.top() + from * scale),
        Pos2::new(
            strip.right(),
            strip.top() + (to * scale).max(from * scale + 4.0),
        ),
    );
    let tint = ui.visuals().selection.bg_fill;
    painter.rect_filled(marked, 2.0, tint.gamma_multiply(0.35));
    painter.rect_stroke(marked, 2.0, egui::Stroke::new(1.0_f32, tint));
    // Each hit at its word, at least 3 by 2 pixels so a single one is seen,
    // with a glow; hits on the same pixel are drawn once, so thousands of
    // hits cost no more than the strip has pixels.
    if found.shown {
        let mut drawn = std::collections::HashSet::new();
        for hit in found.hits {
            let core = Rect::from_min_size(
                Pos2::new(
                    inner.left() + hit.left.clamp(0.0, 1.0) * inner.width(),
                    strip.top() + hit.top * scale,
                ),
                Vec2::new(
                    (hit.width * inner.width()).max(3.0),
                    (hit.height * scale).max(2.0),
                ),
            );
            if !drawn.insert((core.left() as i32, core.top() as i32)) {
                continue;
            }
            painter.rect_filled(core.expand(2.5), 3.0, found.color.gamma_multiply(0.3));
            painter.rect_filled(core, 1.0, found.color);
        }
    }

    let pointer = response.interact_pointer_pos()?;
    if !(response.clicked() || response.dragged() || response.is_pointer_button_down_on()) {
        return None;
    }
    // The place under the pointer comes to the middle of the view.
    let at = (pointer.y - strip.top()) / scale;
    let height = to - from;
    Some((at - height / 2.0).clamp(0.0, (content - height).max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_fits_the_note_and_never_stretches() {
        let strip = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, 800.0));
        assert_eq!(scale(strip, 80_000.0), 0.01);
        assert_eq!(scale(strip, 1_000.0), MAX_SCALE);
    }
}
