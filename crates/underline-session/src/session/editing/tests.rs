//! Tests of editing in the session (UC-005, ADR-024): the source view's
//! inputs and patches, the autosave and its marks (M-06.1, M-10.8), storing
//! at once (M-10.9), closing after storing (M-06.3, M-21.4), the changes
//! from outside with the open text (M-07.5), the clipboard (M-08.6), the
//! highlighting (M-08.2) and the properties (M-16.2). Feature F4: the Nano
//! keys (M-11.3), the toolbar (M-10.2), the edits of the window's Vim keys
//! (A-01), the status line's keys and place (M-11.4) and folding (M-08.8).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::editor_view::{
    EditorCommandName, EditorInput, KeyName, TextChange, ToolName, VimChange, VimModeName, VimState,
};
use crate::{Event, Session, SettingName, SettingsState};

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

fn text(text: &str) -> EditorInput {
    EditorInput::Text { text: text.into() }
}

fn key(key: KeyName) -> EditorInput {
    EditorInput::Key {
        key,
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
        page: 10,
    }
}

/// The platform's command key with `key`: Cmd on macOS, else Ctrl.
fn command(key: KeyName, shift: bool) -> EditorInput {
    let mac = cfg!(target_os = "macos");
    EditorInput::Key {
        key,
        ctrl: !mac,
        alt: false,
        shift,
        meta: mac,
        page: 10,
    }
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

fn editor_text(session: &mut Session, note: &str) -> String {
    session.editor_state(&id(note)).unwrap().text
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
fn adr024_uc005_m08_4_an_input_changes_the_text_and_answers_with_a_patch() {
    let dir = temp_dir("input");
    let (mut session, _) = session_with(&dir, &["Home"]);
    let opened = session.editor_state(&id("Home")).unwrap();
    assert_eq!(
        (opened.text.as_str(), opened.head, opened.anchor),
        ("# Home\n", 0, 0)
    );

    session
        .editor_input(&id("Home"), key(KeyName::End))
        .unwrap();
    let patch = session.editor_input(&id("Home"), text("🙂")).unwrap();

    assert_eq!(patch.base, opened.rev);
    assert_ne!(patch.rev, opened.rev);
    assert_eq!(
        patch.changes,
        [TextChange {
            from: 6,
            to: 6,
            insert: "🙂".into()
        }]
    );
    assert_eq!((patch.head, patch.anchor), (8, 8), "UTF-16 units");
    assert!(patch.scroll && patch.unstored);
    assert_eq!(editor_text(&mut session, "Home"), "# Home🙂\n");
}

#[test]
fn m06_1_m10_8_a_change_marks_the_tab_until_the_autosave_stored_it() {
    let dir = temp_dir("autosave");
    let (mut session, root) = session_with(&dir, &["Home"]);

    session.editor_input(&id("Home"), text("x")).unwrap();
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
    session.editor_input(&id("Home"), text("x")).unwrap();

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
    session.editor_input(&id("Home"), text("x")).unwrap();

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
    session.editor_input(&id("Ideas"), text("x")).unwrap();

    let question = session.close_tab(&id("Ideas"), false);

    assert_eq!(question, None);
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "x# Ideas\n"
    );
    assert!(
        session.editor_state(&id("Ideas")).is_err(),
        "the tab is closed"
    );
}

#[test]
fn m06_3_uc005_a14_a_tab_that_cannot_be_stored_asks_before_closing() {
    let dir = temp_dir("close-asks");
    let (mut session, root) = session_with(&dir, &["Home"]);
    block(&root.join("Home.md"));
    session.editor_input(&id("Home"), text("x")).unwrap();

    let question = session.close_tab(&id("Home"), false).expect("a question");

    assert_eq!(question.tab.as_deref(), Some("Home"));
    assert_eq!(question.what, "the tab Home");
    assert!(question.error.starts_with("Home: "), "{}", question.error);
    assert!(session.editor_state(&id("Home")).is_ok(), "the tab stays");

    // "Close anyway".
    assert_eq!(session.close_tab(&id("Home"), true), None);
    assert!(session.editor_state(&id("Home")).is_err());
}

#[test]
fn m21_4_closing_underline_stores_everything_or_asks() {
    let dir = temp_dir("close-app");
    let (mut session, root) = session_with(&dir, &["Home", "Ideas"]);
    session.open_note(&id("Ideas"));
    session.editor_input(&id("Home"), text("h")).unwrap();
    session.editor_input(&id("Ideas"), text("i")).unwrap();

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
    session.editor_input(&id("Ideas"), text("j")).unwrap();
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
    session.editor_input(&id("Home"), text("x")).unwrap();

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
    session.editor_input(&id("Home"), text("x")).unwrap();

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
    let before = session.editor_state(&id("Home")).unwrap().rev;

    fs::write(root.join("Home.md"), "# Home\nfrom outside\n").unwrap();
    assert!(tick_until(&mut session, |s| s.status() == "Home was changed outside"));

    let state = session.editor_state(&id("Home")).unwrap();
    assert_eq!(state.text, "# Home\nfrom outside\n");
    assert_ne!(state.rev, before);
    assert_eq!(label(&session, "Home"), "Home");
}

#[test]
fn uc004_a7_m07_5_a_reload_tells_the_window_the_new_revision() {
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
                Event::TextReplaced(replaced) => Some(replaced),
                _ => None,
            });
        std::thread::sleep(Duration::from_millis(20));
    }

    let seen = seen.expect("a TextReplaced event");
    assert_eq!(seen.note, "Home");
    assert_eq!(seen.rev, session.editor_state(&id("Home")).unwrap().rev);
}

#[test]
fn uc004_a9_m07_5_a_change_from_outside_meets_an_unstored_change_and_both_stay() {
    let dir = temp_dir("keep-both");
    let (mut session, root) = session_with(&dir, &["Home"]);
    session.editor_input(&id("Home"), text("mine ")).unwrap();

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

    session.editor_input(&id("Home"), text("x")).unwrap();
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
fn m08_6_copy_and_cut_give_the_markup_and_paste_inserts_text() {
    let dir = temp_dir("clipboard");
    let (mut session, _) = session_with(&dir, &["Home"]);

    // Nothing selected: nothing for the clipboard.
    let (copied, _) = session.editor_copy(&id("Home")).unwrap();
    assert_eq!(copied, None);

    let rev = session.editor_state(&id("Home")).unwrap().rev;
    session.editor_select(&id("Home"), rev, 0, 6).unwrap();
    let (copied, patch) = session.editor_copy(&id("Home")).unwrap();
    assert!(patch.changes.is_empty());
    assert_eq!(copied.as_deref(), Some("# Home"));

    let (cut, patch) = session.editor_cut(&id("Home")).unwrap();
    assert_eq!(cut.as_deref(), Some("# Home"));
    assert_eq!(patch.changes.len(), 1);
    assert_eq!(editor_text(&mut session, "Home"), "\n");

    session
        .editor_paste(&id("Home"), Some("**pasted**"))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "**pasted**\n");
    assert_eq!(label(&session, "Home"), "Home •");
    let nothing = session.editor_paste(&id("Home"), None).unwrap();
    assert!(nothing.changes.is_empty());
}

#[test]
fn m08_3_a_selection_from_an_older_revision_is_dropped() {
    let dir = temp_dir("stale-select");
    let (mut session, _) = session_with(&dir, &["Home"]);
    let old = session.editor_state(&id("Home")).unwrap().rev;
    session.editor_input(&id("Home"), text("x")).unwrap();

    session.editor_select(&id("Home"), old, 0, 3).unwrap();

    let state = session.editor_state(&id("Home")).unwrap();
    assert_eq!((state.anchor, state.head), (1, 1));
}

#[test]
fn m10_1_undo_and_redo_keys_reach_the_document() {
    let dir = temp_dir("undo");
    let (mut session, _) = session_with(&dir, &["Home"]);
    for c in ["a", "b", " ", "c"] {
        session.editor_input(&id("Home"), text(c)).unwrap();
    }

    session
        .editor_input(&id("Home"), command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "ab# Home\n");
    let patch = session
        .editor_input(&id("Home"), command(KeyName::Y, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "ab c# Home\n");
    assert_eq!(patch.head, 4, "after the text put back");
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

    session.editor_input(&id("Home"), text("x")).unwrap();
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

    let state = session.editor_state(&id("Home")).unwrap();
    assert_eq!(state.text, "# Home\n🙂 first\nmixed\nlone\nend\n");
    // Moving about and selecting change nothing.
    session
        .editor_input(&id("Home"), key(KeyName::End))
        .unwrap();
    session.editor_select(&id("Home"), state.rev, 0, 9).unwrap();
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
    let state = session.editor_state(&id("Home")).unwrap();
    assert_eq!(state.text, "# Home\nline\n");

    // The window's place after the first break is 7.
    session.editor_select(&id("Home"), state.rev, 7, 7).unwrap();
    let patch = session.editor_input(&id("Home"), text("x")).unwrap();
    assert_eq!(
        patch.changes,
        [TextChange {
            from: 7,
            to: 7,
            insert: "x".into()
        }]
    );
    session
        .editor_input(&id("Home"), key(KeyName::Enter))
        .unwrap();
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
    session.editor_input(&id("Home"), text("old ")).unwrap();
    // The autosave is due: its job starts with the older text …
    session.tick(Instant::now() + Duration::from_millis(1100));
    // … and the newer text is stored at once while it may still run.
    session.editor_input(&id("Home"), text("new ")).unwrap();

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

// ------------------------------------------------------------- F4 ----

/// `key` with Ctrl (and Cmd on macOS for the command key's functions).
fn ctrl(key: KeyName) -> EditorInput {
    EditorInput::Key {
        key,
        ctrl: true,
        alt: false,
        shift: false,
        meta: false,
        page: 10,
    }
}

fn alt(key: KeyName) -> EditorInput {
    EditorInput::Key {
        key,
        ctrl: false,
        alt: true,
        shift: false,
        meta: false,
        page: 10,
    }
}

/// The note `Home` holding `body`, the cursor at its start, in `mode`.
fn home_with(name: &str, body: &str, mode: &str) -> Session {
    let dir = temp_dir(name);
    let (mut session, _) = session_with(&dir, &["Home"]);
    let home = id("Home");
    session
        .editor_input(&home, command(KeyName::A, false))
        .unwrap();
    session.editor_input(&home, text(body)).unwrap();
    session
        .editor_input(&home, command(KeyName::Home, false))
        .unwrap();
    // Nano's Ctrl+A goes to the line start: the mode comes after the text.
    session.quick_setting(SettingName::InputMode, mode.into());
    session
}

/// Ticks until an event of the active tab matches `pick`.
fn event_soon<T>(session: &mut Session, mut pick: impl FnMut(Event) -> Option<T>) -> Option<T> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        for event in session.tick(Instant::now() + Duration::from_secs(1)) {
            if let Some(found) = pick(event) {
                return Some(found);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn uc005_br010_m11_3_nano_cuts_lines_that_add_up_and_pastes_them() {
    let mut session = home_with("nano-cut", "a\nb\nc\n", "Nano");
    let home = id("Home");

    session.editor_input(&home, ctrl(KeyName::K)).unwrap();
    let patch = session.editor_input(&home, ctrl(KeyName::K)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "c\n");
    assert_eq!(patch.keys.as_ref().unwrap().keymap, "NANO");

    session.editor_input(&home, ctrl(KeyName::U)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "a\nb\nc\n");

    // Typing ends the run of cuts: the next cut holds only the new line,
    // which as the last line takes the break before it along.
    session.editor_input(&home, ctrl(KeyName::K)).unwrap();
    session.editor_input(&home, text("x")).unwrap();
    session.editor_input(&home, ctrl(KeyName::K)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "a\nb");
    session.editor_input(&home, ctrl(KeyName::U)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "a\nx\nb");
}

#[test]
fn m11_3_nano_ctrl_a_and_ctrl_e_go_to_the_line_ends_alt_u_and_alt_e_undo_and_redo() {
    let mut session = home_with("nano-keys", "one two\n", "Nano");
    let home = id("Home");

    let end = session.editor_input(&home, ctrl(KeyName::E)).unwrap();
    assert_eq!(end.place, "Ln 1, Col 8");
    let start = session.editor_input(&home, ctrl(KeyName::A)).unwrap();
    assert_eq!(start.place, "Ln 1, Col 1");

    session.editor_input(&home, text("x")).unwrap();
    session.editor_input(&home, alt(KeyName::U)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "one two\n");
    session.editor_input(&home, alt(KeyName::E)).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "xone two\n");
}

#[test]
fn uc008_br008_m11_3_nano_searches_by_prompt_and_marks_the_hits_until_escape() {
    let mut session = home_with("nano-search", "ab\nab\n", "Nano");
    let home = id("Home");

    let prompt = session.editor_input(&home, ctrl(KeyName::W)).unwrap();
    let keys = prompt.keys.unwrap();
    assert!(keys.prompting);
    assert_eq!(keys.pending, "Search: ");
    assert_eq!(keys.hints[0].keys, "Enter");
    // The prompt takes the text; the note keeps it.
    let typed = session.editor_input(&home, text("ab")).unwrap();
    assert!(typed.changes.is_empty());
    assert_eq!(typed.keys.unwrap().pending, "Search: ab");

    let found = session.editor_input(&home, key(KeyName::Enter)).unwrap();
    assert_eq!(found.place, "Ln 2, Col 1", "the next hit, after the cursor");
    let keys = found.keys.unwrap();
    assert!(!keys.prompting);
    assert_eq!(
        (keys.hints[0].keys.as_str(), keys.hints[0].action.as_str()),
        ("Esc", "End search")
    );
    session.editor_viewport(&home, 0, 3).unwrap();
    let marked = event_soon(&mut session, |e| match e {
        Event::Found(found) => Some(found),
        _ => None,
    })
    .unwrap();
    assert_eq!(marked.ranges, [(0, 2), (3, 5)]);

    session.editor_input(&home, key(KeyName::Escape)).unwrap();
    let cleared = event_soon(&mut session, |e| match e {
        Event::Found(found) => Some(found),
        _ => None,
    })
    .unwrap();
    assert!(cleared.ranges.is_empty());
}

#[test]
fn m11_3_nano_ctrl_c_and_escape_cancel_the_prompt() {
    let mut session = home_with("nano-cancel", "ab\n", "Nano");
    let home = id("Home");

    session.editor_input(&home, ctrl(KeyName::W)).unwrap();
    let cancelled = session.editor_input(&home, ctrl(KeyName::C)).unwrap();
    assert!(!cancelled.keys.unwrap().prompting);
    session.editor_input(&home, ctrl(KeyName::W)).unwrap();
    let cancelled = session.editor_input(&home, key(KeyName::Escape)).unwrap();
    assert!(!cancelled.keys.unwrap().prompting);
}

#[test]
fn m10_9_m11_3_nano_ctrl_o_ctrl_s_and_ctrl_x_ask_the_window() {
    let mut session = home_with("nano-commands", "a\n", "Nano");
    let home = id("Home");

    for (input, asked) in [
        (KeyName::O, EditorCommandName::Store),
        (KeyName::S, EditorCommandName::Store),
        (KeyName::X, EditorCommandName::Close),
    ] {
        let patch = session.editor_input(&home, ctrl(input)).unwrap();
        assert_eq!(patch.commands, [asked]);
        assert!(patch.changes.is_empty());
    }
}

#[test]
fn m11_1_without_nano_its_keys_do_nothing_but_the_usual() {
    let mut session = home_with("nano-off", "a\nb\n", "Normal");
    let home = id("Home");

    let patch = session.editor_input(&home, ctrl(KeyName::K)).unwrap();

    assert!(patch.keys.is_none());
    assert!(patch.changes.is_empty(), "Ctrl+K asks for a link instead");
    assert_eq!(patch.request, Some(ToolName::Link));
    assert_eq!(editor_text(&mut session, "Home"), "a\nb\n");
}

#[test]
fn uc005_a2_m10_2_the_toolbar_toggles_inline_formats_as_one_undo_step() {
    let mut session = home_with("toolbar-inline", "word\n", "Normal");
    let home = id("Home");
    let rev = session.editor_state(&home).unwrap().rev;
    session.editor_select(&home, rev, 0, 4).unwrap();

    let bold = session
        .editor_input(
            &home,
            EditorInput::Tool {
                tool: ToolName::Bold,
            },
        )
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "**word**\n");
    assert_eq!(
        bold.place, "Ln 1, Col 7 · 4 chars",
        "the word stays selected"
    );

    session
        .editor_input(
            &home,
            EditorInput::Tool {
                tool: ToolName::Bold,
            },
        )
        .unwrap();
    assert_eq!(
        editor_text(&mut session, "Home"),
        "word\n",
        "a second time removes it"
    );

    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "**word**\n");
    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "word\n");
}

#[test]
fn uc005_br005_m10_2_line_formats_and_shortcuts_follow_the_note_format() {
    let dir = temp_dir("toolbar-format");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Doc.adoc"), "line\n").unwrap();
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
    let doc = id("Doc");
    session.open_note(&doc);

    // Ctrl+Shift+H: an AsciiDoc heading; Ctrl+B: AsciiDoc bold.
    session
        .editor_input(&doc, command(KeyName::H, true))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Doc"), "== line\n");
    session
        .editor_input(&doc, command(KeyName::End, false))
        .unwrap();
    session
        .editor_input(&doc, command(KeyName::B, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Doc"), "== line\n**");
    let image = session
        .editor_input(
            &doc,
            EditorInput::Tool {
                tool: ToolName::Image,
            },
        )
        .unwrap();
    assert_eq!(image.request, Some(ToolName::Image));
    assert!(image.changes.is_empty(), "Image asks first (feature F6)");
}

fn change(from: u32, to: u32, insert: &str, new_step: bool) -> VimChange {
    VimChange {
        from,
        to,
        insert: insert.into(),
        new_step,
    }
}

#[test]
fn a01_the_vim_keys_changes_are_taken_for_their_revision_only() {
    let mut session = home_with("vim-apply", "one two\n", "Vim");
    let home = id("Home");
    let rev = session.editor_state(&home).unwrap().rev;

    let patch = session
        .editor_apply(&home, rev, &[change(0, 4, "", true)], (0, 0), false)
        .unwrap()
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "two\n");
    assert!(patch.changes.is_empty(), "the window has the change");
    assert_eq!(patch.base, rev);
    assert_ne!(patch.rev, rev);
    assert!(label(&session, "Home").ends_with('•'));

    let refused = session
        .editor_apply(&home, rev, &[], (0, 0), false)
        .unwrap();
    assert!(refused.is_none(), "an old revision is refused");

    // One history with the other inputs (M-10.1).
    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "one two\n");
}

#[test]
fn a01_changes_that_do_not_fit_are_refused_whole() {
    let mut session = home_with("vim-refuse", "a🙂b\r\ncd", "Vim");
    let home = id("Home");
    // In the window: "a🙂b\ncd", 7 units.
    let rev = session.editor_state(&home).unwrap().rev;
    let mut refused = |changes: &[VimChange]| {
        session
            .editor_apply(&home, rev, changes, (0, 0), false)
            .unwrap()
            .is_none()
    };

    assert!(refused(&[change(0, 8, "", false)]), "beyond the end");
    assert!(refused(&[change(3, 2, "", false)]), "backwards");
    assert!(
        refused(&[change(2, 2, "x", false)]),
        "inside the surrogate pair"
    );
    assert!(
        refused(&[change(0, 1, "", false), change(6, 7, "", false)]),
        "the second is beyond the end after the first"
    );
    assert_eq!(
        editor_text(&mut session, "Home"),
        "a🙂b\ncd",
        "nothing applied"
    );

    // Several changes, each on the text after the one before; the `\r\n`
    // pair counts once, and stays bytes for bytes elsewhere.
    let patch = session
        .editor_apply(
            &home,
            rev,
            &[change(0, 1, "", true), change(4, 6, "CD", true)],
            (0, 0),
            false,
        )
        .unwrap();
    assert!(patch.is_some());
    assert_eq!(session.editor_state(&home).unwrap().text, "🙂b\nCD");
}

#[test]
fn a01_m10_1_the_vim_keys_mark_their_undo_steps() {
    let mut session = home_with("vim-steps", "", "Vim");
    let home = id("Home");
    let rev = |session: &mut Session| session.editor_state(&home).unwrap().rev;

    // An insert session typed through the usual inputs, after its start
    // was marked; then a command of its own.
    let start = rev(&mut session);
    session
        .editor_apply(&home, start, &[], (0, 0), true)
        .unwrap();
    session.editor_input(&home, text("ab")).unwrap();
    let now = rev(&mut session);
    session
        .editor_apply(&home, now, &[change(2, 2, "c", true)], (3, 3), false)
        .unwrap();
    // The same insertion without a mark merges with the typing.
    let now = rev(&mut session);
    session
        .editor_apply(&home, now, &[change(3, 3, "d", false)], (4, 4), false)
        .unwrap();

    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "ab");
    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "");
}

#[test]
fn m11_3_a_paste_into_the_open_nano_prompt_stays_out_of_the_note() {
    let mut session = home_with("nano-paste", "ab\n", "Nano");
    let home = id("Home");
    session.editor_input(&home, ctrl(KeyName::W)).unwrap();

    let patch = session.editor_paste(&home, Some("pasted")).unwrap();

    assert!(patch.changes.is_empty());
    assert_eq!(editor_text(&mut session, "Home"), "ab\n");
    session.editor_input(&home, key(KeyName::Escape)).unwrap();
    session.editor_paste(&home, Some("x")).unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "xab\n");
}

#[test]
fn uc005_br004_m10_2_bold_without_selection_merges_with_the_typing_as_in_egui() {
    // egui's `apply_format` replaces through the document like typing, so
    // "****" right after a typed word is undone with it.
    let mut session = home_with("toolbar-merge", "", "Normal");
    let home = id("Home");
    session.editor_input(&home, text("w")).unwrap();
    session.editor_input(&home, text("o")).unwrap();

    session
        .editor_input(
            &home,
            EditorInput::Tool {
                tool: ToolName::Bold,
            },
        )
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "wo****");

    session
        .editor_input(&home, command(KeyName::Z, false))
        .unwrap();
    assert_eq!(editor_text(&mut session, "Home"), "");
}

#[test]
fn m10_9_storing_at_once_leaves_no_later_saved_status() {
    let mut session = home_with("store-once", "x\n", "Nano");
    let home = id("Home");

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

#[test]
fn m11_4_the_keys_of_the_status_line_follow_the_input_mode() {
    let mut session = home_with("keys-status", "a\n", "Normal");
    let home = id("Home");
    assert!(session.editor_keys_status(&home, None).unwrap().is_none());

    session.quick_setting(SettingName::InputMode, "Vim".into());
    let vim = session
        .editor_keys_status(
            &home,
            Some(VimState {
                mode: VimModeName::VisualLine,
                pending: "2d".into(),
                marked: true,
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        (vim.keymap.as_str(), vim.mode.as_deref()),
        ("VIM", Some("VISUAL LINE"))
    );
    assert_eq!(vim.color, 2);
    assert_eq!(vim.pending, "", "visual mode shows nothing typed");
    assert_eq!(vim.hints[0].keys, "d");

    let normal = session
        .editor_keys_status(
            &home,
            Some(VimState {
                mode: VimModeName::Normal,
                pending: "2d".into(),
                marked: true,
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(normal.pending, "2d");
    assert_eq!(normal.hints[0].action, "end search");

    session.quick_setting(SettingName::InputMode, "Nano".into());
    let nano = session.editor_keys_status(&home, None).unwrap().unwrap();
    assert_eq!((nano.keymap.as_str(), nano.mode), ("NANO", None));
    assert_eq!(nano.hints[0].keys, "^O");
}

#[test]
fn uc005_m11_4_the_place_says_line_column_and_what_is_selected() {
    let mut session = home_with("place", "ab\ncd\n", "Normal");
    let home = id("Home");
    let rev = session.editor_state(&home).unwrap().rev;

    assert_eq!(
        session.editor_select(&home, rev, 1, 2).unwrap(),
        "Ln 1, Col 3 · 1 char"
    );
    assert_eq!(
        session.editor_select(&home, rev, 0, 5).unwrap(),
        "Ln 2, Col 3 · 2 lines"
    );
    assert_eq!(
        session.editor_state(&home).unwrap().place,
        "Ln 2, Col 3 · 2 lines"
    );
}

#[test]
fn uc004_br012_m08_8_folding_by_key_is_told_after_the_input() {
    let mut session = home_with("folding", "# A\none\n# B\ntwo\n", "Normal");
    let home = id("Home");
    session.editor_viewport(&home, 0, 5).unwrap();
    let first = event_soon(&mut session, |e| match e {
        Event::Sections(sections) => Some(sections),
        _ => None,
    })
    .unwrap();
    assert_eq!(first.sections.len(), 2);
    assert!(first.sections.iter().all(|s| !s.folded));

    // The cursor on "one": folding moves it to the heading.
    session
        .editor_input(&home, key(KeyName::ArrowDown))
        .unwrap();
    let folded = session
        .editor_input(&home, command(KeyName::BracketLeft, true))
        .unwrap();
    assert_eq!(folded.place, "Ln 1, Col 4");
    let told = event_soon(&mut session, |e| match e {
        Event::Sections(sections) => Some(sections),
        _ => None,
    })
    .unwrap();
    assert!(told.sections[0].folded);
    assert_eq!((told.sections[0].line, told.sections[0].last), (0, 1));

    // Down steps over the folded line; the arrow expands again.
    let down = session
        .editor_input(&home, key(KeyName::ArrowDown))
        .unwrap();
    assert_eq!(down.place, "Ln 3, Col 4");
    session.editor_fold(&home, 0).unwrap();
    let open = event_soon(&mut session, |e| match e {
        Event::Sections(sections) => Some(sections),
        _ => None,
    })
    .unwrap();
    assert!(!open.sections[0].folded);
}
