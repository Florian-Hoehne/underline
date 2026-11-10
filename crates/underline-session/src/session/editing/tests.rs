//! Tests of editing in the session (UC-005, ADR-025): the window's edits
//! arrive as batches (`ed_sync`, `Session::window_edit`); around them the
//! autosave and its marks (M-06.1, M-10.8), storing at once (M-10.9),
//! closing after storing (M-06.3, M-21.4), the changes from outside with
//! the open text (M-07.5), the highlighting (M-08.2) and the properties
//! (M-16.2). The rules of each input (the source view's keys, the
//! clipboard, Nano, the toolbar, the Vim keys' changes, the status line,
//! folding) are the editor core's and are tested where it runs
//! (`underline-editor-view`, `tab/source_tests.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::{Event, Session, SettingsState};

/// A fresh, empty folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-editing-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on a workspace `dir/notes` with Markdown notes of these ids,
/// each holding `# {id}\n`, opened at its first note.
fn session_with(dir: &Path, notes: &[&str]) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    for note in notes {
        fs::write(root.join(format!("{note}.md")), format!("# {note}\n")).unwrap();
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

fn label(session: &Session, note: &str) -> String {
    session
        .snapshot()
        .tabs
        .tabs
        .into_iter()
        .find(|tab| tab.note == note)
        .map(|tab| tab.label)
        .unwrap_or_default()
}

/// The window's text of `note`: the backend's mirror after a sync.
fn editor_text(session: &mut Session, note: &str) -> String {
    session.window_text(&id(note))
}

/// The window types `text` at the start of `note`.
fn type_at_start(session: &mut Session, note: &str, text: &str) {
    session.window_type(&id(note), 0, text);
}

/// Ticks until `done` holds, at most five seconds; the watcher collects
/// changes for 200 ms, and the saver stores on its own thread.
fn tick_until(session: &mut Session, mut done: impl FnMut(&Session) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        session.tick(Instant::now());
        if done(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Ticks as if `seconds` had passed since now, until the background save
/// of `note` reported: its tab is no longer marked " •".
fn autosave_after(session: &mut Session, note: &str, seconds: f32) {
    let later = Instant::now() + Duration::from_secs_f32(seconds);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        session.tick(later);
        if !label(session, note).contains('•') {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the autosave of {note} did not report");
}

/// Replaces the note's file by a folder, so storing it fails on every
/// system.
fn block(file: &Path) {
    fs::remove_file(file).unwrap();
    fs::create_dir_all(file.join("blocked")).unwrap();
}

#[test]
fn m06_1_m10_8_a_change_marks_the_tab_until_the_autosave_stored_it() {
    let dir = temp_dir("autosave");
    let (mut session, root) = session_with(&dir, &["Home"]);

    type_at_start(&mut session, "Home", "x");
    let events = session.tick(Instant::now());

    assert!(events.iter().any(|e| matches!(e, Event::Tabs(_))));
    assert_eq!(label(&session, "Home"), "Home •");
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "# Home\n"
    );

    // Not yet after half a second, but after the quiet second.
    session.tick(Instant::now() + Duration::from_millis(500));
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "# Home\n"
    );
    autosave_after(&mut session, "Home", 1.1);

    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "x# Home\n"
    );
    assert_eq!(label(&session, "Home"), "Home");
    assert_eq!(session.status(), "Saved Home");
}

#[test]
fn m10_8_a_failed_save_marks_the_tab_and_says_why() {
    let dir = temp_dir("save-fails");
    let (mut session, root) = session_with(&dir, &["Home"]);
    block(&root.join("Home.md"));
    type_at_start(&mut session, "Home", "x");

    autosave_after(&mut session, "Home", 1.1);

    // The watcher may have seen the file go meanwhile: " (deleted)" follows.
    assert!(label(&session, "Home").starts_with("Home ⚠"));
    let tab = session.snapshot().tabs.tabs.remove(0);
    let error = tab.save_error.expect("the reason is kept for the banner");
    assert_eq!(
        session.status(),
        format!("Home could not be stored: {error}")
    );
}

#[test]
fn m10_9_store_stores_at_once_and_says_so() {
    let dir = temp_dir("store");
    let (mut session, root) = session_with(&dir, &["Home"]);
    type_at_start(&mut session, "Home", "x");

    session.store_tab(&id("Home"));

    assert_eq!(session.status(), "Stored Home");
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "x# Home\n"
    );
    assert_eq!(label(&session, "Home"), "Home");
}

#[test]
fn m10_9_store_that_fails_says_why() {
    let dir = temp_dir("store-fails");
    let (mut session, root) = session_with(&dir, &["Home"]);
    block(&root.join("Home.md"));

    session.store_tab(&id("Home"));

    assert!(
        session.status().starts_with("Home could not be stored: "),
        "{}",
        session.status()
    );
    assert_eq!(label(&session, "Home"), "Home ⚠");
}

#[test]
fn m06_3_closing_a_tab_stores_it_first() {
    let dir = temp_dir("close-stores");
    let (mut session, root) = session_with(&dir, &["Home", "Ideas"]);
    session.open_note(&id("Ideas"));
    type_at_start(&mut session, "Ideas", "x");

    let question = session.close_tab(&id("Ideas"), false);

    assert_eq!(question, None);
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "x# Ideas\n"
    );
    assert!(
        session.editor_mirror(&id("Ideas")).is_err(),
        "the tab is closed"
    );
}

#[test]
fn m06_3_uc005_a14_a_tab_that_cannot_be_stored_asks_before_closing() {
    let dir = temp_dir("close-asks");
    let (mut session, root) = session_with(&dir, &["Home"]);
    block(&root.join("Home.md"));
    type_at_start(&mut session, "Home", "x");

    let question = session.close_tab(&id("Home"), false).expect("a question");

    assert_eq!(question.tab.as_deref(), Some("Home"));
    assert_eq!(question.what, "the tab Home");
    assert!(question.error.starts_with("Home: "), "{}", question.error);
    assert!(session.editor_mirror(&id("Home")).is_ok(), "the tab stays");

    // "Close anyway".
    assert_eq!(session.close_tab(&id("Home"), true), None);
    assert!(session.editor_mirror(&id("Home")).is_err());
}

#[test]
fn m21_4_closing_underline_stores_everything_or_asks() {
    let dir = temp_dir("close-app");
    let (mut session, root) = session_with(&dir, &["Home", "Ideas"]);
    session.open_note(&id("Ideas"));
    type_at_start(&mut session, "Home", "h");
    type_at_start(&mut session, "Ideas", "i");

    assert_eq!(session.request_close(), None);
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "h# Home\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "i# Ideas\n"
    );

    block(&root.join("Ideas.md"));
    type_at_start(&mut session, "Ideas", "j");
    let question = session.request_close().expect("a question");
    assert_eq!(question.tab, None);
    assert_eq!(question.what, "Underline");
    assert!(question.error.starts_with("Ideas: "), "{}", question.error);
}

#[test]
fn uc005_br001_m21_1_switching_the_workspace_stores_first() {
    let dir = temp_dir("switch-stores");
    let (mut session, root) = session_with(&dir, &["Home"]);
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Start.md"), "# Start\n").unwrap();
    type_at_start(&mut session, "Home", "x");

    session.open_chosen(&other, false);

    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "x# Home\n"
    );
    assert!(
        session.status().starts_with("Opened "),
        "{}",
        session.status()
    );
}

#[test]
fn uc005_br001_m21_1_a_workspace_stays_open_when_its_changes_cannot_be_stored() {
    let dir = temp_dir("switch-blocked");
    let (mut session, root) = session_with(&dir, &["Home"]);
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    block(&root.join("Home.md"));
    type_at_start(&mut session, "Home", "x");

    session.open_chosen(&other, false);

    assert!(
        session
            .status()
            .starts_with("The workspace stays open, because changes could not be stored: Home: "),
        "{}",
        session.status()
    );
    assert_eq!(
        session.workspace_root().map(Path::to_path_buf),
        Some(std::path::absolute(&root).unwrap())
    );
}

#[test]
fn uc004_a7_m07_5_a_tab_without_changes_shows_the_text_from_outside() {
    let dir = temp_dir("reload");
    let (mut session, root) = session_with(&dir, &["Home"]);
    let before = session.editor_mirror(&id("Home")).unwrap().rev;

    fs::write(root.join("Home.md"), "# Home\nfrom outside\n").unwrap();
    assert!(tick_until(&mut session, |s| s.status() == "Home was changed outside"));

    let state = session.editor_mirror(&id("Home")).unwrap();
    assert_eq!(state.text, "# Home\nfrom outside\n");
    assert_ne!(state.rev, before);
    assert_eq!(label(&session, "Home"), "Home");
}

#[test]
fn uc004_a7_m07_5_a_reload_rings_the_window_with_the_new_revision() {
    let dir = temp_dir("reload-event");
    let (mut session, root) = session_with(&dir, &["Home"]);

    fs::write(root.join("Home.md"), "# Home\nnew\n").unwrap();
    let mut seen = None;
    let start = Instant::now();
    while seen.is_none() && start.elapsed() < Duration::from_secs(5) {
        seen = session
            .tick(Instant::now())
            .into_iter()
            .find_map(|e| match e {
                Event::Changed(changed) => Some(changed),
                _ => None,
            });
        std::thread::sleep(Duration::from_millis(20));
    }

    // ADR-025: the window's core fetches the change (`ed-changed`).
    let seen = seen.expect("an ed-changed event");
    assert_eq!(seen.note, "Home");
    assert_eq!(seen.rev, session.editor_mirror(&id("Home")).unwrap().rev);
}

#[test]
fn uc004_a9_m07_5_a_change_from_outside_meets_an_unstored_change_and_both_stay() {
    let dir = temp_dir("keep-both");
    let (mut session, root) = session_with(&dir, &["Home"]);
    type_at_start(&mut session, "Home", "mine ");

    fs::write(root.join("Home.md"), "# Home\ntheirs\n").unwrap();
    assert!(tick_until(&mut session, |s| s.status().contains("kept as")));

    assert!(
        session.status().starts_with(
            "Home was changed outside while you were editing; your text stays, the other version is kept as Home"
        ),
        "{}",
        session.status()
    );
    assert_eq!(editor_text(&mut session, "Home"), "mine # Home\n");
    let copies: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("Home") && name != "Home.md")
        .collect();
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(
        fs::read_to_string(root.join(&copies[0])).unwrap(),
        "# Home\ntheirs\n"
    );
}

#[test]
fn uc005_a15_m07_5_a_note_deleted_outside_is_stored_again_at_its_place() {
    let dir = temp_dir("deleted-stored");
    let (mut session, root) = session_with(&dir, &["Home", "Ideas"]);
    fs::remove_file(root.join("Home.md")).unwrap();
    assert!(tick_until(&mut session, |s| s.status() == "Home was deleted outside"));
    assert_eq!(label(&session, "Home"), "Home (deleted)");

    type_at_start(&mut session, "Home", "x");
    autosave_after(&mut session, "Home", 1.1);

    assert_eq!(session.status(), "Stored Home again at its place");
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "x# Home\n"
    );
    assert_eq!(label(&session, "Home"), "Home");
}

#[test]
fn m16_2_the_properties_of_the_active_note_and_not_saved_yet() {
    let dir = temp_dir("properties");
    let (mut session, root) = session_with(&dir, &["Home", "Ideas"]);
    session.open_note(&id("Ideas"));

    let events = session.take_events();
    let properties = events
        .iter()
        .find_map(|e| match e {
            Event::Properties(p) => p.clone(),
            _ => None,
        })
        .expect("the properties of the active tab");
    assert_eq!(properties.note, "Ideas");
    let shown = properties.properties.expect("a stored note");
    assert_eq!(shown.place, "Ideas.md");
    assert_eq!(shown.format, "Markdown");
    assert_eq!(shown.size, "8 bytes");
    assert_eq!(shown.last_change.len(), "2026-11-01 09:30".len());

    fs::remove_file(root.join("Ideas.md")).unwrap();
    let mut gone = false;
    let start = Instant::now();
    while !gone && start.elapsed() < Duration::from_secs(5) {
        gone = session.tick(Instant::now()).into_iter().any(|e| {
            matches!(e, Event::Properties(Some(p)) if p.note == "Ideas" && p.properties.is_none())
        });
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(gone, "\"Not saved yet\" after the file is gone");
}

#[test]
fn m08_2_adr024_colours_follow_the_viewport_and_every_edit() {
    let dir = temp_dir("highlight");
    let (mut session, _) = session_with(&dir, &["Home"]);
    session.editor_viewport(&id("Home"), 0, 40).unwrap();

    // The colours are made on a thread of their own first.
    let first = highlight_soon(&mut session).expect("colours for the shown lines");
    assert_eq!(first.note, "Home");
    assert_eq!(first.from, 0);
    assert_eq!(first.lines.len(), 2, "the heading and the empty last line");
    assert!(!first.lines[0].is_empty(), "the heading has a colour");
    assert!(first.lines[0].iter().all(|(start, end, style)| start < end
        && *end <= 6
        && (*style as usize) < first.styles.len()));
    assert!(
        session
            .tick(Instant::now())
            .iter()
            .all(|e| !matches!(e, Event::Highlight(_))),
        "unchanged colours are not sent again"
    );

    type_at_start(&mut session, "Home", "x");
    let again = highlight_soon(&mut session).expect("new colours after the edit");
    // ADR-025: the colours carry the mirror's revision.
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&id("Home")).unwrap());
    assert_eq!(again.rev, tab.mirror.rev());
    assert_ne!(again.rev, first.rev);
}

/// The next colours the tick sends, within five seconds.
fn highlight_soon(session: &mut Session) -> Option<crate::HighlightView> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        let found = session
            .tick(Instant::now())
            .into_iter()
            .find_map(|e| match e {
                Event::Highlight(view) => Some(view),
                _ => None,
            });
        if found.is_some() {
            return found;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn nfr007_opening_a_crlf_note_and_closing_it_unchanged_keeps_its_bytes() {
    let dir = temp_dir("crlf-fidelity");
    let bytes = "# Home\r\n🙂 first\r\nmixed\nlone\rend\r\n";
    fs::create_dir_all(dir.join("notes")).unwrap();
    fs::write(dir.join("notes").join("Home.md"), bytes).unwrap();
    let (mut session, root) = session_with(&dir, &[]);

    // The window's core starts from the text as it is, line breaks and all.
    let state = session.editor_mirror(&id("Home")).unwrap();
    assert_eq!(state.text, bytes);
    session.tick(Instant::now() + Duration::from_secs(3));
    assert_eq!(session.request_close(), None);
    assert!(session.end().is_ok());

    assert_eq!(fs::read(root.join("Home.md")).unwrap(), bytes.as_bytes());
}

#[test]
fn nfr007_editing_a_crlf_note_changes_only_the_edited_bytes() {
    let dir = temp_dir("crlf-edit");
    fs::create_dir_all(dir.join("notes")).unwrap();
    fs::write(dir.join("notes").join("Home.md"), "# Home\r\nline\r\n").unwrap();
    let (mut session, root) = session_with(&dir, &[]);
    // The window's core sends chars of the text: after the first break
    // (`\r\n`, two chars) is 8. Its Enter inserts `\n`, as in egui.
    session.window_type(&id("Home"), 8, "x");
    session.window_type(&id("Home"), 9, "\n");
    session.store_tab(&id("Home"));

    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "# Home\r\nx\nline\r\n",
        "Enter inserts \\n as in egui; the other breaks stay"
    );
}

#[test]
fn uc005_br001_storing_at_once_waits_for_a_background_save() {
    let dir = temp_dir("flush-in-flight");
    let (mut session, root) = session_with(&dir, &["Home"]);
    type_at_start(&mut session, "Home", "old ");
    // The autosave is due: its job starts with the older text …
    session.tick(Instant::now() + Duration::from_millis(1100));
    // … and the newer text is stored at once while it may still run.
    session.window_type(&id("Home"), 4, "new ");

    session.store_tab(&id("Home"));

    assert_eq!(session.status(), "Stored Home");
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..5 {
        session.tick(Instant::now());
    }
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "old new # Home\n",
        "the older text never lands after the newer one"
    );
    assert_eq!(label(&session, "Home"), "Home");
}

#[test]
fn m10_9_storing_at_once_leaves_no_later_saved_status() {
    let dir = temp_dir("store-once");
    let (mut session, _) = session_with(&dir, &["Home"]);
    let home = id("Home");
    type_at_start(&mut session, "Home", "x");

    session.store_tab(&home);
    assert_eq!(session.status(), "Stored Home");
    for seconds in [1.5, 3.0, 12.0] {
        session.tick(Instant::now() + Duration::from_secs_f32(seconds));
        std::thread::sleep(Duration::from_millis(20));
        session.tick(Instant::now() + Duration::from_secs_f32(seconds));
    }

    assert_eq!(
        session.status(),
        "Stored Home",
        "no \"Saved Home\" after it"
    );
}
