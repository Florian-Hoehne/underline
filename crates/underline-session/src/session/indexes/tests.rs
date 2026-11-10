//! Tests of the tool panel's outline, code and figures (M-16.5, UC-004
//! step 4, FR-041 to FR-043).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use underline_core::{NoteFormat, NoteId, StateStore, SETTINGS_FILE_NAME};

use super::{make_indexes, INDEXES_EVERY};
use crate::editor_view::{IndexEntryView, IndexesView};
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-indexes-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files`, `Home.md`
/// open in the active tab.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> Session {
    let root = dir.join("notes");
    for (name, text) in files {
        fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
        fs::write(root.join(name), text).unwrap();
    }
    fs::create_dir_all(dir.join("home")).unwrap();
    let mut session = Session::new(
        Some(root),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.open_note(&NoteId::from_relative("Home"));
    session.take_events();
    session
}

/// Ticks from `now` on until the lists come, at most 5 s.
fn lists_from(session: &mut Session, now: Instant) -> IndexesView {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        for event in session.tick(now) {
            if let Event::Indexes(view) = event {
                return view;
            }
        }
        assert!(Instant::now() < until, "no lists came");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn entry(label: &str, depth: u32, offset: u32) -> IndexEntryView {
    IndexEntryView {
        label: label.into(),
        depth,
        offset,
    }
}

const NOTE: &str = "# Top 🙂\r\n\r\n## Inner\r\n\r\n```rust\r\nfn main() {}\r\n```\r\n\r\n```mermaid\r\n%% c\r\ngraph TD; A-->B\r\n```\r\n\r\n![A cat](cat.png)\r\n\r\n```\r\n\r\n```\r\n";

#[test]
fn m16_5_lists_carry_labels_depths_and_web_offsets() {
    let rope = ropey::Rope::from_str(NOTE);

    let view = make_indexes(
        &NoteId::from_relative("Home"),
        3,
        &rope,
        NoteFormat::Markdown,
    );

    // Each block's offset is where it starts, in web units: 🙂 counts two,
    // each `\r\n` one.
    let web =
        |prefix: &str| u32::try_from(prefix.replace("\r\n", "\n").encode_utf16().count()).unwrap();
    let at = |marker: &str| web(&NOTE[..NOTE.find(marker).unwrap()]);
    assert_eq!(view.note, "Home");
    assert_eq!(view.rev, 3);
    assert_eq!(
        view.outline,
        [entry("Top 🙂", 0, 0), entry("Inner", 1, at("## Inner"))]
    );
    assert_eq!(
        view.code,
        [
            entry("rust: fn main() {}", 0, at("```rust")),
            entry("(empty)", 0, at("```\r\n\r\n```\r\n")),
        ]
    );
    assert_eq!(
        view.figures,
        [
            entry("Mermaid diagram: graph TD; A-->B", 0, at("```mermaid")),
            entry("A cat", 0, at("![A cat]")),
        ]
    );
}

#[test]
fn m16_5_an_empty_note_has_empty_lists() {
    let view = make_indexes(
        &NoteId::from_relative("Home"),
        1,
        &ropey::Rope::new(),
        NoteFormat::AsciiDoc,
    );

    assert!(view.outline.is_empty() && view.code.is_empty() && view.figures.is_empty());
}

#[test]
fn m16_5_the_tick_sends_the_active_tabs_lists_once() {
    let dir = temp_dir("once");
    let mut session = session_with(&dir, &[("Home.md", "# Home\n\n## Next\n")]);
    let now = Instant::now();

    let view = lists_from(&mut session, now);

    assert_eq!(view.note, "Home");
    assert_eq!(view.rev, 1);
    assert_eq!(view.outline.len(), 2);
    for _ in 0..5 {
        assert!(
            !session
                .tick(now + Duration::from_secs(1))
                .iter()
                .any(|e| matches!(e, Event::Indexes(_))),
            "the same lists are not sent again"
        );
    }
}

#[test]
fn m16_5_the_lists_follow_the_text_the_window_sent() {
    let dir = temp_dir("follow");
    let mut session = session_with(&dir, &[("Home.md", "# Home\n")]);
    let home = NoteId::from_relative("Home");
    let start = Instant::now();
    lists_from(&mut session, start);

    session.window_type(&home, 7, "\n## Added\n");
    // At most every 300 ms while the text changes.
    assert!(!session
        .tick(start + INDEXES_EVERY / 2)
        .iter()
        .any(|e| matches!(e, Event::Indexes(_))));
    let view = lists_from(&mut session, start + INDEXES_EVERY);

    assert_eq!(view.rev, 2);
    assert_eq!(view.outline, [entry("Home", 0, 0), entry("Added", 1, 8)]);
}

#[test]
fn m16_5_another_active_tab_gets_its_own_lists() {
    let dir = temp_dir("switch");
    let mut session = session_with(
        &dir,
        &[
            ("Home.md", "# Home\n"),
            ("Plan.md", "# Plan\n\n```sh\nls\n```\n"),
        ],
    );
    let now = Instant::now();
    lists_from(&mut session, now);

    session.open_note(&NoteId::from_relative("Plan"));
    let plan = lists_from(&mut session, now);
    assert_eq!(plan.note, "Plan");
    assert_eq!(plan.code, [entry("sh: ls", 0, 8)]);

    // Back to Home: its lists, kept, are sent again.
    session.activate_tab(&NoteId::from_relative("Home"));
    let home = lists_from(&mut session, now);
    assert_eq!(home.note, "Home");
}

/// Makes the lists after 100 ms, so a job outlives what the test does.
fn slow(note: &NoteId, rev: u32, rope: &ropey::Rope, format: NoteFormat) -> IndexesView {
    std::thread::sleep(Duration::from_millis(100));
    make_indexes(note, rev, rope, format)
}

#[test]
fn m16_5_the_lists_of_a_closed_tab_are_dropped() {
    let dir = temp_dir("closed");
    let mut session = session_with(&dir, &[("Home.md", "# Old\n")]);
    session.make_indexes_with(slow);
    let home = NoteId::from_relative("Home");
    let now = Instant::now();
    session.tick(now);

    // The job runs on while the tab closes and opens anew on another text.
    assert!(session.close_tab(&home, false).is_none());
    fs::write(dir.join("notes").join("Home.md"), "# New\n").unwrap();
    session.open_note(&home);

    let view = lists_from(&mut session, now + Duration::from_secs(1));
    assert_eq!(view.outline, [entry("New", 0, 0)]);
}

#[test]
fn m16_5_the_lists_of_a_note_of_the_same_name_in_another_workspace_are_dropped() {
    let dir = temp_dir("other-workspace");
    let mut session = session_with(&dir, &[("Home.md", "# A\n")]);
    session.make_indexes_with(slow);
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Home.md"), "# B\n").unwrap();
    let now = Instant::now();
    session.tick(now);

    session.open_chosen(&other, false);
    session.open_note(&NoteId::from_relative("Home"));

    // Same note, same revision: still the other text's lists.
    let view = lists_from(&mut session, now + Duration::from_secs(1));
    assert_eq!(view.rev, 1);
    assert_eq!(view.outline, [entry("B", 0, 0)]);
}

static PANICKED: AtomicUsize = AtomicUsize::new(0);

fn panicking(_: &NoteId, _: u32, _: &ropey::Rope, _: NoteFormat) -> IndexesView {
    PANICKED.fetch_add(1, Ordering::SeqCst);
    panic!("a broken parser");
}

#[test]
fn m16_5_a_job_that_panics_frees_its_tab_and_is_not_tried_again_for_the_same_text() {
    let dir = temp_dir("panic");
    let mut session = session_with(&dir, &[("Home.md", "# Home\n")]);
    session.make_indexes_with(panicking);
    let home = NoteId::from_relative("Home");
    let start = Instant::now();
    session.tick(start);
    let until = Instant::now() + Duration::from_secs(5);
    while session.open_tab_indexes_waiting(&home) {
        assert!(Instant::now() < until, "the job never ended");
        session.tick(start);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(PANICKED.load(Ordering::SeqCst), 1);

    // The same text is not tried again.
    for step in 1..5 {
        session.tick(start + INDEXES_EVERY * step);
    }
    assert!(!session.open_tab_indexes_waiting(&home));
    assert_eq!(PANICKED.load(Ordering::SeqCst), 1);

    // A changed text is, and with a working parser its lists come.
    session.make_indexes_with(make_indexes);
    session.window_type(&home, 7, "## Next\n");
    let view = lists_from(&mut session, start + INDEXES_EVERY * 10);
    assert_eq!(view.outline.len(), 2);
}

static COUNTED: AtomicUsize = AtomicUsize::new(0);

fn counted(note: &NoteId, rev: u32, rope: &ropey::Rope, format: NoteFormat) -> IndexesView {
    COUNTED.fetch_add(1, Ordering::SeqCst);
    make_indexes(note, rev, rope, format)
}

#[test]
fn m16_5_while_the_text_changes_a_job_starts_at_most_every_300_ms() {
    let dir = temp_dir("throttle");
    let mut session = session_with(&dir, &[("Home.md", "# Home\n")]);
    session.make_indexes_with(counted);
    let home = NoteId::from_relative("Home");
    let start = Instant::now();
    lists_from(&mut session, start);
    assert_eq!(COUNTED.load(Ordering::SeqCst), 1);

    // Typing on: within 300 ms of the last start no job starts.
    for (step, ms) in [10_u64, 100, 200, 290].into_iter().enumerate() {
        session.window_type(&home, 7 + step, "x");
        session.tick(start + Duration::from_millis(ms));
    }
    assert_eq!(COUNTED.load(Ordering::SeqCst), 1);

    // Then the newest text's lists come in one job.
    let view = lists_from(&mut session, start + INDEXES_EVERY);
    assert_eq!(view.rev, 5);
    assert_eq!(COUNTED.load(Ordering::SeqCst), 2);
}
