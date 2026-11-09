//! The blinking text cursor of the source view and the facade.

use std::time::Duration;

use eframe::egui;

/// When the cursor last moved, and where it was then: it blinks from that
/// time on, following the text cursor style of the visuals.
#[derive(Default)]
pub(crate) struct Blink {
    since: f64,
    /// `None` until the cursor is first drawn, so it then shows at once.
    at: Option<(usize, usize)>,
}

impl Blink {
    /// Whether the cursor at `at` (cursor and anchor) is shown now; it shows
    /// at once after it moved. Asks for the frame that turns it on or off.
    pub(crate) fn shown(&mut self, ui: &egui::Ui, at: (usize, usize)) -> bool {
        let now = ui.input(|i| i.time);
        if self.at != Some(at) {
            *self = Self {
                since: now,
                at: Some(at),
            };
        }
        let style = &ui.visuals().text_cursor;
        let period = style.on_duration + style.off_duration;
        if !style.blink || period <= 0.0 {
            return true;
        }
        let phase = ((now - self.since) as f32) % period;
        let shown = phase < style.on_duration;
        let next = if shown { style.on_duration } else { period } - phase;
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(next.max(0.01)));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-005: the cursor blinks, and shows at once after it moved.
    #[test]
    fn uc005_cursor_blinks_and_shows_after_moving() {
        let ctx = egui::Context::default();
        let mut blink = Blink::default();
        let mut shown_at = |time: f64, at: (usize, usize)| {
            let input = egui::RawInput {
                time: Some(time),
                ..Default::default()
            };
            let mut shown = false;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    shown = blink.shown(ui, at);
                });
            });
            shown
        };
        assert!(shown_at(10.0, (3, 3)));
        assert!(!shown_at(10.7, (3, 3)));
        assert!(shown_at(11.1, (3, 3)));
        assert!(shown_at(11.3, (4, 4)), "shown at once after moving");
    }
}
