//! The prototype measurement of ADR-012 (NFR-002, UC-005 BR-003): typing in
//! a note of 10,000 lines with highlighting, and scrolling through it.
//!
//! Run with `cargo test --release -p underline-editor -- --ignored --nocapture`.
//! The frame time measured here is the CPU work of one frame (input, layout,
//! painting, tessellation); drawing on the GPU comes on top of it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Pos2, RawInput, Rect, Vec2};
use underline_core::NoteFormat;
use underline_facade::highlight::HighlightSet;

use crate::SourceView;

const LINES: usize = 10_000;
const FRAME_BUDGET: Duration = Duration::from_millis(16);

/// A note of `LINES` lines: headings, prose, lists, and Rust and Python code.
fn sample_note() -> String {
    let mut text = String::new();
    while text.lines().count() < LINES {
        let n = text.len() / 500;
        text.push_str(&format!("## Section {n}\n\nSome *prose* with **bold** text, `code`, and a [link](https://example.org/{n}).\n\n- first item\n- second item\n\n```rust\nfn section_{n}(x: u32) -> u32 {{\n    let y = x * 2; // double\n    if y > 10 {{ y - 1 }} else {{ y + 1 }}\n}}\n```\n\n```python\ndef section_{n}(x):\n    return [i * 2 for i in range(x) if i % 3]\n```\n\n"));
    }
    let lines: Vec<&str> = text.lines().take(LINES).collect();
    lines.join("\n") + "\n"
}

struct Harness {
    ctx: egui::Context,
    view: SourceView,
}

impl Harness {
    fn new() -> Self {
        let set = Arc::new(HighlightSet::new(true));
        Self {
            ctx: egui::Context::default(),
            view: SourceView::new("bench", &sample_note(), NoteFormat::Markdown, set),
        }
    }

    /// Runs one frame with `events` and returns the time it took.
    fn frame(&mut self, events: Vec<Event>) -> Duration {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..Default::default()
        };
        let start = Instant::now();
        let view = &mut self.view;
        let output = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.show(ui);
            });
        });
        let _ = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        start.elapsed()
    }

    fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
    }
}

fn report(name: &str, mut times: Vec<Duration>) -> Duration {
    times.sort();
    let pick = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    let (median, p99, max) = (pick(0.5), pick(0.99), *times.last().unwrap());
    println!(
        "{name}: {} frames, median {median:?}, p99 {p99:?}, max {max:?}",
        times.len()
    );
    p99
}

/// ADR-012 prototype: typing at line 5,000 of a 10,000-line note.
#[test]
#[ignore]
fn adr012_typing_in_10000_lines_within_16_ms() {
    let mut harness = Harness::new();
    harness.frame(vec![]);
    harness.click(Pos2::new(200.0, 100.0));

    // Jump to line 5,000: the first frame there highlights the lines before
    // it, which ADR-010 moves to a worker thread; it is reported, not judged.
    let at = harness.view.document().rope().line_to_char(5_000);
    harness.view.set_cursor(at);
    let first = harness.frame(vec![]);
    println!("first frame at line 5,000: {first:?}");
    assert!(first < FRAME_BUDGET * 2, "the first frame does not wait for the colors");
    let mut frames = 1;
    while !harness.view.is_colored() {
        harness.frame(vec![]);
        frames += 1;
    }
    println!("colors of line 5,000 ready after {frames} frames");

    let lines_before = harness.view.document().len_lines();
    let mut times = Vec::new();
    for i in 0..300 {
        let text = if i % 40 == 39 { "\n" } else { "x" };
        let time = harness.frame(vec![Event::Text(text.to_owned())]);
        if time > FRAME_BUDGET {
            let line = harness
                .view
                .document()
                .rope()
                .char_to_line(harness.view.cursor());
            println!("slow frame {i} ({text:?}) at line {line}: {time:?}");
        }
        times.push(time);
    }
    let p99 = report("typing", times);
    assert_eq!(harness.view.document().len_lines(), lines_before + 300 / 40);
    assert!(
        p99 < FRAME_BUDGET,
        "typing p99 {p99:?} exceeds {FRAME_BUDGET:?}"
    );
}

/// ADR-012 prototype: scrolling through the note keeps 60 frames per second
/// once it is highlighted.
#[test]
#[ignore]
fn adr012_scrolling_keeps_60_frames_per_second() {
    let mut harness = Harness::new();
    harness.frame(vec![]);
    let last = harness.view.document().len_chars();
    harness.view.set_cursor(last);
    let start = std::time::Instant::now();
    let mut frames = 0;
    loop {
        let time = harness.frame(vec![]);
        frames += 1;
        assert!(time < FRAME_BUDGET * 2, "coloring frame took {time:?}");
        if harness.view.is_colored() {
            break;
        }
    }
    println!("coloring all 10,000 lines: {frames} frames, {:?}", start.elapsed());

    let mut times = Vec::new();
    for step in 0..200 {
        let line = (step * 50) % LINES;
        let at = harness.view.document().rope().line_to_char(line);
        harness.view.set_cursor(at);
        times.push(harness.frame(vec![]));
    }
    let p99 = report("scrolling 50 lines per frame", times);
    assert!(
        p99 < FRAME_BUDGET,
        "scrolling p99 {p99:?} exceeds {FRAME_BUDGET:?}"
    );
}
