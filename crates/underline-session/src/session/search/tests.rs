//! Tests of the search of the workspace (UC-008, M-13.1, M-13.3), against
//! what `run_search`, `found_list` and `open_found` of the egui app do. The
//! rules of what matches are tested in `underline-markup` and
//! `underline-core`; the marks in the tabs in `underline-editor-view`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::search_view::{FoundPlace, SearchResultView, SearchView};
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-search-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files` (relative
/// path, content).
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    for (name, text) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(dir.join("home")).unwrap();
    let mut session = Session::new(
        Some(root.clone()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.take_events();
    (session, root)
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// Ticks until the window is told a result searched for `text`.
fn result_for(session: &mut Session, text: &str) -> SearchResultView {
    let start = Instant::now();
    loop {
        for event in session.tick(Instant::now()) {
            if let Event::Search(SearchView {
                result: Some(result),
                ..
            }) = event
            {
                if result.text == text {
                    return result;
                }
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no result for {text}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Searches `text` and waits for its result.
fn search(session: &mut Session, text: &str) -> SearchResultView {
    session.search_set(text);
    result_for(session, text)
}

fn titles(result: &SearchResultView) -> Vec<&str> {
    result.notes.iter().map(|n| n.title.as_str()).collect()
}

/// The marked text of the first line of the first note.
fn marked(result: &SearchResultView) -> Vec<&str> {
    result.notes[0].lines[0]
        .parts
        .iter()
        .filter(|p| p.hit)
        .map(|p| p.text.as_str())
        .collect()
}

/// TC-008-01, M-13.1, M-13.3: "ferry" finds Trip with its line, and the
/// found line opens Trip with the word selected, first in the overview;
/// the search keeps its text and result.
#[test]
fn uc008_tc01_m13_1_m13_3_find_and_open_at_the_place() {
    let dir = temp_dir("tc01");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Trip.md", "# Trip\n\n😀 Book the ferry to Hydra\n"),
            ("Ideas.md", "nothing here\n"),
        ],
    );

    let result = search(&mut session, "ferry");

    assert_eq!(titles(&result), ["Trip"]);
    let trip = &result.notes[0];
    assert_eq!(trip.places, "1 place");
    assert_eq!(trip.folder, None);
    assert_eq!(trip.more, 0);
    let parts: Vec<(&str, bool)> = trip.lines[0]
        .parts
        .iter()
        .map(|p| (p.text.as_str(), p.hit))
        .collect();
    assert_eq!(
        parts,
        [
            ("😀 Book the ", false),
            ("ferry", true),
            (" to Hydra", false)
        ]
    );
    assert_eq!(trip.place, Some(trip.lines[0].place));

    let opened = session
        .search_open(&id("Trip"), Some(trip.lines[0].place))
        .unwrap()
        .unwrap();

    assert_eq!(opened.note, "Trip");
    let text = session.window_text(&id("Trip"));
    let web: Vec<u16> = text.encode_utf16().collect();
    let word = String::from_utf16(&web[opened.anchor as usize..opened.head as usize]).unwrap();
    assert_eq!(word, "ferry");
    assert_eq!(opened.rev, session.mirror_rev(&id("Trip")));
    let events = session.take_events();
    assert!(events.iter().any(|e| matches!(e,
        Event::Overview(items) if items.first().is_some_and(|i| i.note == "Trip"))));
    assert!(events.iter().any(|e| matches!(e,
        Event::Tabs(tabs) if tabs.active.as_deref() == Some("Trip"))));
    // Step 5, A3: opening refreshed the overview; the search runs again and
    // finds the same.
    assert_eq!(titles(&result_for(&mut session, "ferry")), ["Trip"]);
}

/// TC-008-05, BR-003: title and folder matches first, then the others,
/// each in the order of the overview; a note found by name only says so.
#[test]
fn uc008_tc05_m13_1_titles_and_folders_first() {
    let dir = temp_dir("tc05");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Budget.md", "travel costs\n"),
            ("travel/Packing.md", "socks\n"),
            ("Travel-Log.md", "travel diary\n"),
        ],
    );

    let result = search(&mut session, "travel");

    assert_eq!(titles(&result), ["Packing", "Travel Log", "Budget"]);
    assert_eq!(result.notes[0].places, "in the title or folder");
    assert_eq!(result.notes[0].folder.as_deref(), Some("travel"));
    assert_eq!(result.notes[0].place, None, "no line: opens as it is");
    assert_eq!(result.notes[1].places, "1 place");
}

/// TC-008-07 (A1), TC-008-08 (A2): nothing found is a result without
/// notes; only spaces end the search, and the window is told so.
#[test]
fn uc008_tc07_tc08_m13_1_nothing_found_and_cleared() {
    let dir = temp_dir("tc07");
    let (mut session, _) = session_with(&dir, &[("Trip.md", "ferry\n")]);

    session.search_set("  zebra ");
    let result = result_for(&mut session, "zebra");
    assert_eq!(result.total, 0);
    assert!(result.notes.is_empty());

    session.search_set("   ");
    let events = session.tick(Instant::now());
    assert!(events.contains(&Event::Search(SearchView {
        text: String::new(),
        result: None,
    })));
    assert!(session
        .tick(Instant::now())
        .iter()
        .all(|e| !matches!(e, Event::Search(_))));
}

/// TC-008-06, TC-008-09, BR-001, A3: the text of an open tab is searched
/// with its changes not stored yet, and typing there searches again.
#[test]
fn uc008_tc06_tc09_m13_1_open_texts_and_changes_search_again() {
    let dir = temp_dir("tc09");
    let (mut session, _) = session_with(&dir, &[("Draft.md", "old text\n"), ("Trip.md", "x\n")]);
    assert!(session.open_note(&id("Draft")));
    session.window_type(&id("Draft"), 0, "mango ");

    let result = search(&mut session, "mango");
    assert_eq!(titles(&result), ["Draft"]);

    // A3: the word is deleted in the tab: Draft is no longer found.
    session.window_edit(&id("Draft"), 0, 6, "");
    let start = Instant::now();
    loop {
        let events = session.tick(Instant::now());
        let found = events.iter().find_map(|e| match e {
            Event::Search(SearchView {
                result: Some(r), ..
            }) => Some(r.total),
            _ => None,
        });
        if found == Some(0) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// TC-008-10, A4: a note that cannot be read is counted below the found
/// notes, with egui's text.
#[test]
fn uc008_tc10_m13_1_a_note_that_cannot_be_read_is_counted() {
    let dir = temp_dir("tc10");
    let (mut session, root) = session_with(&dir, &[("Trip.md", "ferry\n"), ("Secret.md", "x")]);
    // Not UTF-8: it cannot be read as a note.
    fs::write(root.join("Secret.md"), [0xff, 0xfe, 0x00, 0xc3]).unwrap();

    let result = search(&mut session, "ferry");

    assert_eq!(titles(&result), ["Trip"]);
    assert_eq!(
        result.unreadable.as_deref(),
        Some("1 note could not be searched.")
    );
}

/// TC-008-11, A5: five lines shown, "and 3 more".
#[test]
fn uc008_tc11_m13_1_many_places() {
    let dir = temp_dir("tc11");
    let log: String = (0..8).map(|n| format!("{n}: deploy\n")).collect();
    let (mut session, _) = session_with(&dir, &[("Log.md", log.as_str())]);

    let result = search(&mut session, "deploy");

    let note = &result.notes[0];
    assert_eq!(note.places, "8 places");
    assert_eq!(note.lines.len(), 5);
    assert_eq!(note.more, 3);
    assert_eq!(marked(&result), ["deploy"]);
}

/// M-13.1, A-89: the found notes come in pages; a page of an older result
/// is not given.
#[test]
fn m13_1_the_found_notes_come_in_pages() {
    let dir = temp_dir("pages");
    let names: Vec<String> = (0..150).map(|n| format!("N{n:03}.md")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "kiwi\n")).collect();
    let (mut session, _) = session_with(&dir, &files);

    let result = search(&mut session, "kiwi");

    assert_eq!(result.total, 150);
    assert_eq!(result.notes.len(), SearchResultView::PAGE);
    let rest = session.search_page(result.serial, 100).unwrap();
    assert_eq!(rest.len(), 50);
    assert_eq!(rest[0].note, "N100");
    assert!(session
        .search_page(result.serial.wrapping_add(1), 0)
        .is_none());
    session.search_set("");
    session.tick(Instant::now());
    assert!(session.search_page(result.serial, 0).is_none());
}

/// UC-008 step 4: a place past the end of the tab's text (it changed since
/// the search) is clamped, as egui clamps it; a note without a place opens
/// as it is.
#[test]
fn uc008_m13_3_a_place_is_clamped_and_a_note_without_one_opens() {
    let dir = temp_dir("clamp");
    let (mut session, _) = session_with(&dir, &[("Trip.md", "ferry\n"), ("Ideas.md", "x\n")]);

    let opened = session
        .search_open(&id("Trip"), Some(FoundPlace { from: 90, to: 95 }))
        .unwrap()
        .unwrap();
    assert_eq!((opened.anchor, opened.head), (6, 6));

    assert!(session.search_open(&id("Ideas"), None).unwrap().is_none());
    assert!(session.take_events().iter().any(|e| matches!(e,
        Event::Tabs(tabs) if tabs.active.as_deref() == Some("Ideas"))));

    // UC-004 A10: a note that is gone opens no tab.
    assert!(session
        .search_open(&id("Gone"), Some(FoundPlace { from: 0, to: 1 }))
        .unwrap()
        .is_none());
    assert!(
        session.status().starts_with("Gone cannot be read"),
        "{}",
        session.status()
    );
}

/// M-21.3: another workspace keeps the text of the overlay and searches it
/// there.
#[test]
fn m21_3_m13_1_another_workspace_searches_the_same_text() {
    let dir = temp_dir("switch");
    let (mut session, _) = session_with(&dir, &[("Trip.md", "ferry\n")]);
    assert_eq!(titles(&search(&mut session, "ferry")), ["Trip"]);
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Boat.md"), "a ferry\n").unwrap();

    session.open_chosen(&other, false);

    assert_eq!(titles(&result_for(&mut session, "ferry")), ["Boat"]);
}

/// TC-008-15, NFR-003, BR-005: 20 searches over 10,000 notes of 1 to 50 KB,
/// from the window's text to the event with the found notes (the session's
/// part, without IPC and drawing); at least 19 within 1 second. The first
/// search reads every file (the searcher keeps the texts afterwards, as in
/// egui). Prints the times; run with
/// `cargo test -p underline-session --release -- --ignored --nocapture uc008_tc15`.
#[test]
#[ignore = "writes 250 MB and takes a while"]
fn uc008_tc15_nfr003_speed_in_a_large_workspace() {
    let dir = temp_dir("tc15");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let words = [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
        "juliett", "kilo", "lima", "mike", "november", "oscar", "papa",
    ];
    let mut k: usize = 7;
    for n in 0usize..10_000 {
        let size = 1_000 + (n * 7_919) % 49_000;
        let mut text = format!("# Note {n}\n\n");
        while text.len() < size {
            k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            text.push_str(words[(k >> 8) % words.len()]);
            text.push_str(if (k >> 4).is_multiple_of(13) {
                "\n"
            } else {
                " "
            });
        }
        text.push_str(&format!("unique{n}\n"));
        let folder = root.join(format!("f{}", n % 20));
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(format!("Note-{n}.md")), text).unwrap();
    }
    let (mut session, _) = session_with(&dir, &[]);
    let queries = [
        "unique9999",
        "zulu",
        "alpha bravo",
        "\"golf hotel\"",
        "unique1 unique2",
        "kilo",
        "note 5000",
        "f3",
        "papa oscar november",
        "\"mike lima\"",
        "unique42",
        "xyz",
        "delta",
        "echo foxtrot golf",
        "india",
        "\"alpha alpha\"",
        "juliett",
        "Note-77",
        "charlie unique7",
        "hotel",
    ];
    let mut times = Vec::new();
    for query in queries {
        let start = Instant::now();
        let result = search(&mut session, query);
        let took = start.elapsed();
        let json = serde_json::to_string(&result).unwrap();
        eprintln!(
            "{query:>22}: {:>7.1} ms, {:>5} notes, first page {:>6} bytes",
            took.as_secs_f64() * 1000.0,
            result.total,
            json.len()
        );
        times.push(took);
    }
    let _ = fs::remove_dir_all(&dir);
    let slow = times.iter().filter(|t| t.as_secs_f64() > 1.0).count();
    assert!(slow <= 1, "{slow} of 20 searches took more than 1 s");
}
