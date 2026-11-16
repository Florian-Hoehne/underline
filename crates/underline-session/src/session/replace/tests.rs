//! Tests of the replace of the workspace (UC-022, TC-022-*). What matches
//! and what a replacement writes is tested in `underline-markup`, the
//! places of the notes in `underline-core`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::editor_view::SearchSwitches;
use crate::replace_view::{ReplaceResultView, ReplaceView};
use crate::{Event, Session, SettingsState, StatusOffer};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-replace-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the files `files`.
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

/// TC-022-01's workspace.
const FERRY: &[(&str, &str)] = &[
    ("Trip.md", "Book the ferry\nferry times\n"),
    ("Ideas.md", "a ferry to Hydra\n"),
    ("Log.md", "ferry\n"),
];

/// Ticks until the window is told of places that answer `asked`.
fn places_for(session: &mut Session, asked: u32) -> ReplaceResultView {
    let start = Instant::now();
    loop {
        for event in session.tick(Instant::now()) {
            if let Event::Replace(ReplaceView {
                result: Some(result),
                ..
            }) = event
            {
                if result.asked == asked {
                    return result;
                }
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "no places for {asked}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Ticks until newer places than `serial` come.
fn newer_than(session: &mut Session, serial: u32) -> ReplaceResultView {
    let start = Instant::now();
    loop {
        for event in session.tick(Instant::now()) {
            if let Event::Replace(ReplaceView {
                result: Some(result),
                ..
            }) = event
            {
                if result.serial != serial {
                    return result;
                }
            }
        }
        assert!(start.elapsed() < Duration::from_secs(20), "no newer places");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn replace(session: &mut Session, text: &str, with: &str) -> ReplaceResultView {
    let asked = session.replace_set(text, with, SearchSwitches::default());
    places_for(session, asked)
}

fn read(root: &Path, file: &str) -> String {
    fs::read_to_string(root.join(file)).unwrap()
}

/// What the status bar offers beside the report now.
fn offer(session: &mut Session) -> Option<StatusOffer> {
    session.replace_offer()
}

/// The tab of `note`.
fn tab<'s>(session: &'s mut Session, note: &str) -> &'s mut crate::session::tab::Tab {
    let open = session.open.as_mut().unwrap();
    let index = open.tabs.find(&id(note)).unwrap();
    open.tabs.get_mut(index)
}

/// TC-022-01, UC-022 main success scenario, BR-004: 4 places in 3 notes,
/// each with its line before and after; "Log" left out; the report with
/// its undo; the replaced places no longer listed.
#[test]
fn uc022_tc01_see_leave_out_and_replace() {
    let dir = temp_dir("tc01");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.summary, "4 places in 3 notes");
    let trip = result.list.iter().find(|n| n.title == "Trip").unwrap();
    let first = &trip.places[0];
    assert_eq!(
        (
            first.before.as_str(),
            first.old.as_str(),
            first.new.as_str()
        ),
        ("Book the ", "ferry", "boat")
    );
    assert!(session
        .replace_leave_out(result.serial, &id("Log"), None, true)
        .is_some());

    session.replace_confirm(result.serial, None);

    assert_eq!(session.status(), "Replaced 3 places in 2 notes");
    assert_eq!(offer(&mut session), Some(StatusOffer::UndoReplace));
    assert_eq!(read(&root, "Trip.md"), "Book the boat\nboat times\n");
    assert_eq!(read(&root, "Ideas.md"), "a boat to Hydra\n");
    assert_eq!(read(&root, "Log.md"), "ferry\n");
    let again = newer_than(&mut session, result.serial);
    let titles: Vec<&str> = again.list.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["Log"]);
    assert!(again.list[0].left_out, "Log stays left out");
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-05, TC-022-06, A1, A2: nothing found; an expression or a
/// replacement that is not valid finds nothing and says why.
#[test]
fn uc022_tc05_tc06_nothing_found_and_not_valid() {
    let dir = temp_dir("tc05");
    let (mut session, _) = session_with(&dir, &[("Dates.md", "Due 11.11.2026\n")]);
    let none = replace(&mut session, "zebra", "x");
    assert_eq!((none.places, none.list.len()), (0, 0));

    let regex = SearchSwitches {
        regex: true,
        ..SearchSwitches::default()
    };
    session.replace_set(r"(\d\d", "", regex);
    let view = session.replace_view();
    assert_eq!(
        view.problem.as_deref(),
        Some("Not a valid regular expression: unclosed group.")
    );
    assert!(view.result.is_none());
    session.replace_set(r"(\d\d)", "$2", regex);
    assert_eq!(
        session.replace_view().problem.as_deref(),
        Some("The replacement names $2, but the expression has no group 2.")
    );
    let asked = session.replace_set(r"(\d\d)\.(\d\d)\.(\d{4})", "$3-$2-$1", regex);
    let dates = places_for(&mut session, asked);
    assert_eq!(dates.list[0].places[0].new, "2026-11-11");
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-07, A3: replacing in one note leaves the others listed.
#[test]
fn uc022_tc07_replace_in_one_note() {
    let dir = temp_dir("tc07");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_confirm(result.serial, Some(&id("Ideas")));
    assert_eq!(read(&root, "Ideas.md"), "a boat to Hydra\n");
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert_eq!(session.status(), "Replaced 1 place in 1 note");
    let again = newer_than(&mut session, result.serial);
    let mut titles: Vec<&str> = again.list.iter().map(|n| n.title.as_str()).collect();
    titles.sort_unstable();
    assert_eq!(titles, ["Log", "Trip"]);
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-08, A4: a note changes while its places are shown: they are
/// found again; the place left out stays left out, the new one is chosen.
#[test]
fn uc022_tc08_a_note_changes_while_shown() {
    let dir = temp_dir("tc08");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Log")));
    let result = replace(&mut session, "ferry", "boat");
    let log = result.list.iter().find(|n| n.title == "Log").unwrap();
    assert!(session
        .replace_leave_out(result.serial, &id("Log"), Some(log.places[0].at), true)
        .is_some());

    session
        .backend_edit(&id("Log"), &[(6..6, "ferry port\n".to_owned())], false)
        .unwrap();
    let again = newer_than(&mut session, result.serial);
    let log = again.list.iter().find(|n| n.title == "Log").unwrap();
    assert_eq!(log.count, "2 places");
    assert!(log.places[0].left_out);
    assert!(!log.places[1].left_out);

    session.replace_confirm(again.serial, Some(&id("Log")));
    assert_eq!(read(&root, "Log.md"), "ferry\nboat port\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004: a note changed by another program after its places were shown
/// is not overwritten, and is named.
#[test]
fn uc022_br004_a_note_changed_meanwhile_is_not_overwritten() {
    let dir = temp_dir("meanwhile");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    fs::write(root.join("Ideas.md"), "a ferry and a ferry\n").unwrap();
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Ideas.md"), "a ferry and a ferry\n");
    assert_eq!(
        session.status(),
        "Replaced 3 places in 2 notes; Ideas was changed meanwhile and is left as it is"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-09, A5, BR-006: a note that cannot be stored stays whole and is
/// named; the others are replaced.
#[cfg(unix)]
#[test]
fn uc022_tc09_a_note_cannot_be_stored() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("tc09");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Trip.md", "ferry\n"),
            ("locked/Ideas.md", "a ferry to Hydra\n"),
            ("Log.md", "ferry\n"),
        ],
    );
    let result = replace(&mut session, "ferry", "boat");
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    session.replace_confirm(result.serial, None);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(read(&root, "locked/Ideas.md"), "a ferry to Hydra\n");
    assert_eq!(read(&root, "Trip.md"), "boat\n");
    assert_eq!(read(&root, "Log.md"), "boat\n");
    assert!(
        session
            .status()
            .starts_with("Replaced 2 places in 2 notes; Ideas could not be changed: "),
        "{}",
        session.status()
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-10, A6: the undo gives the notes back, except one changed
/// since, which is named.
#[test]
fn uc022_tc10_undo_the_replacement() {
    let dir = temp_dir("tc10");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_leave_out(result.serial, &id("Log"), None, true);
    session.replace_confirm(result.serial, None);
    fs::write(root.join("Ideas.md"), "a boat to Hydra and back\n").unwrap();

    session.replace_undo();

    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert_eq!(read(&root, "Ideas.md"), "a boat to Hydra and back\n");
    assert_eq!(
        session.status(),
        "Took back the replacement in 1 note; Ideas was changed since and is left as it is"
    );
    assert_eq!(offer(&mut session), None, "offered once");
    fs::remove_dir_all(&dir).unwrap();
}

/// A6, OQ-191: another report ends the offer to undo.
#[test]
fn uc022_a6_another_report_ends_the_undo() {
    let dir = temp_dir("a6");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_confirm(result.serial, None);
    "Something else".clone_into(&mut session.status);
    assert_eq!(offer(&mut session), None);
    session.replace_undo();
    assert_eq!(read(&root, "Log.md"), "boat\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-11, A7: 10,000 places shown, 2,000 more said, all replaced.
#[test]
fn uc022_tc11_many_places() {
    let dir = temp_dir("tc11");
    let note = "x ".repeat(600);
    let names: Vec<String> = (0..20).map(|n| format!("N{n}.md")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), note.as_str())).collect();
    let (mut session, root) = session_with(&dir, &files);
    let result = replace(&mut session, "x", "y");
    assert_eq!(result.places, 12_000);
    assert_eq!(
        result.more.as_deref(),
        Some("The first 10,000 places are shown; 2,000 more are not shown and are replaced too.")
    );
    assert!(result.list.len() < 20, "a page");
    let mut listed = result.list.len();
    while let Some(page) = session.replace_page(result.serial, units(listed)) {
        if page.is_empty() {
            break;
        }
        listed += page.len();
    }
    assert_eq!(listed, 20);
    session.replace_confirm(result.serial, None);
    assert_eq!(session.status(), "Replaced 12,000 places in 20 notes");
    for name in &names {
        assert!(!read(&root, name).contains('x'));
    }
    fs::remove_dir_all(&dir).unwrap();
}

fn units(n: usize) -> u32 {
    u32::try_from(n).unwrap()
}

/// TC-022-12, A8: closing changes nothing.
#[test]
fn uc022_tc12_cancel() {
    let dir = temp_dir("tc12");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_close();
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-13, BR-001, BR-005: an open note with changes not yet saved is
/// replaced in its tab and stored; one undo in the tab takes it back.
#[test]
fn uc022_tc13_open_note_with_unsaved_changes() {
    let dir = temp_dir("tc13");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Trip")));
    session
        .backend_edit(&id("Trip"), &[(0..0, "ferry pier\n".to_owned())], false)
        .unwrap();
    let result = replace(&mut session, "ferry", "boat");
    let trip = result.list.iter().find(|n| n.title == "Trip").unwrap();
    assert_eq!(trip.places[0].after, " pier");

    session.replace_confirm(result.serial, None);

    let text = "boat pier\nBook the boat\nboat times\n";
    assert_eq!(read(&root, "Trip.md"), text);
    let tab = tab(&mut session, "Trip");
    assert_eq!(tab.mirror.text(), text);
    assert!(!tab.autosave.is_unstored(), "stored");
    tab.edit.editor.undo();
    assert_eq!(
        tab.edit.editor.text(),
        "ferry pier\nBook the ferry\nferry times\n",
        "one undo takes back every place of the note"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-14, TC-022-15, BR-001, BR-006: titles, folders, hidden folders
/// and other files stay; a link is replaced as text.
#[test]
fn uc022_tc14_tc15_what_is_not_replaced() {
    let dir = temp_dir("tc14");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("ferry/Ferry-Log.md", "nothing\n"),
            (".hidden/F.md", "ferry\n"),
            ("img/ferry.txt", "ferry\n"),
            ("Index.md", "[Plan](Plan.md) by ferry\n"),
            ("Plan.md", "plan\n"),
        ],
    );
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.summary, "1 place in 1 note");
    session.replace_confirm(result.serial, None);
    assert!(root.join("ferry/Ferry-Log.md").exists());
    assert_eq!(read(&root, ".hidden/F.md"), "ferry\n");
    assert_eq!(read(&root, "img/ferry.txt"), "ferry\n");

    let links = replace(&mut session, "Plan.md", "Plan-2026.md");
    session.replace_confirm(links.serial, None);
    assert_eq!(read(&root, "Index.md"), "[Plan](Plan-2026.md) by boat\n");
    assert!(root.join("Plan.md").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn span_edit_is_the_smallest_change() {
    assert_eq!(
        super::span_edit("a ferry b ferry c", "a boat b boat c"),
        (2..15, "boat b boat".to_owned())
    );
    assert_eq!(super::span_edit("ü x", "ü y"), (3..4, "y".to_owned()));
    assert_eq!(super::span_edit("aé", "aè"), (1..3, "è".to_owned()));
    assert_eq!(super::span_edit("same", "same"), (4..4, String::new()));
}

/// TC-022-16, NFR-017, BR-007: replacing in 1,000 of 10,000 notes. Prints
/// the time; run with `cargo test -p underline-session --release --
/// --ignored --nocapture uc022_tc16`.
#[test]
#[ignore = "a measurement"]
fn uc022_tc16_nfr017_replacing_in_1000_notes() {
    let dir = temp_dir("tc16");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let mut k: usize = 7;
    for n in 0usize..10_000 {
        k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let size = 1_000 + (k >> 8) % 49_000;
        let mut text = String::with_capacity(size + 64);
        while text.len() < size {
            text.push_str("lorem ipsum dolor sit amet\n");
        }
        if n % 10 == 0 {
            text.push_str("Acme ships the order.\n");
        }
        fs::write(root.join(format!("Note-{n}.md")), text).unwrap();
    }
    let (mut session, root) = session_with(&dir, &[]);
    let start = Instant::now();
    let result = replace(&mut session, "Acme", "Acme Corp");
    eprintln!("places found (reading the files): {:?}", start.elapsed());
    assert_eq!(result.places, 1_000);
    let start = Instant::now();
    session.replace_confirm(result.serial, None);
    let took = start.elapsed();
    eprintln!("replaced in 1,000 notes: {took:?} ({})", session.status());
    assert_eq!(session.status(), "Replaced 1,000 places in 1,000 notes");
    assert!(read(&root, "Note-0.md").contains("Acme Corp ships"));
    fs::remove_dir_all(&dir).unwrap();
    assert!(took < Duration::from_secs(10), "NFR-017");
}

/// Review: places of an older request are never replaced; the status says
/// to look again.
#[test]
fn uc022_br004_places_of_an_older_request_replace_nothing() {
    let dir = temp_dir("older");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_set("Hydra", "Paros", SearchSwitches::default());

    let status = session.replace_confirm(result.serial, None);

    assert_eq!(
        status,
        "The places changed while you looked at them; look at them again before you replace"
    );
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert!(session
        .replace_leave_out(result.serial, &id("Log"), None, true)
        .is_none());
    fs::remove_dir_all(&dir).unwrap();
}

/// Step 3: what is chosen, for the list and the button.
#[test]
fn uc022_step3_what_is_chosen() {
    let dir = temp_dir("chosen");
    let (mut session, _) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.chosen.action, "Replace 4 places in 3 notes");
    let trip = result.list.iter().find(|n| n.title == "Trip").unwrap();
    let chosen = session
        .replace_leave_out(result.serial, &id("Trip"), Some(trip.places[0].at), true)
        .unwrap();
    assert_eq!(chosen.summary, "3 of 4 places chosen");
    assert_eq!(chosen.action, "Replace 3 places in 3 notes");
    let chosen = session
        .replace_leave_out(result.serial, &id("Log"), None, true)
        .unwrap();
    assert_eq!(chosen.action, "Replace 2 places in 2 notes");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004, review: an open note whose file was changed elsewhere since its
/// tab read it is not overwritten, and is named.
#[test]
fn uc022_br004_an_open_note_changed_on_disk_is_left() {
    let dir = temp_dir("disk");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Log")));
    let result = replace(&mut session, "ferry", "boat");
    fs::write(root.join("Log.md"), "ferry from elsewhere\n").unwrap();

    let status = session.replace_confirm(result.serial, Some(&id("Log")));

    assert_eq!(read(&root, "Log.md"), "ferry from elsewhere\n");
    assert_eq!(tab(&mut session, "Log").mirror.text(), "ferry\n");
    assert!(status.contains("Log was changed meanwhile"), "{status}");
    fs::remove_dir_all(&dir).unwrap();
}

/// A5, review: an open note that cannot be stored keeps its text in its
/// tab, and is named.
#[cfg(unix)]
#[test]
fn uc022_a5_an_open_note_that_cannot_be_stored_keeps_its_text() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("a5tab");
    let (mut session, root) = session_with(&dir, &[("locked/Log.md", "ferry\n")]);
    assert!(session.open_note(&id("locked/Log")));
    let result = replace(&mut session, "ferry", "boat");
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let status = session.replace_confirm(result.serial, None);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(read(&root, "locked/Log.md"), "ferry\n");
    assert_eq!(tab(&mut session, "locked/Log").mirror.text(), "ferry\n");
    assert!(status.contains("Log could not be changed: "), "{status}");
    fs::remove_dir_all(&dir).unwrap();
}

/// Review: the job reads and writes the closed notes without the session;
/// the three parts give what the one call gives.
#[test]
fn uc022_the_job_runs_without_the_session() {
    let dir = temp_dir("job");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    let job = session.replace_plan(result.serial, None).unwrap();
    let done = std::thread::spawn(move || job.run()).join().unwrap();
    assert_eq!(session.replace_finish(done), "Replaced 4 places in 3 notes");
    assert_eq!(read(&root, "Log.md"), "boat\n");
    fs::remove_dir_all(&dir).unwrap();
}

// ---- Test-monkey: package 1 (UC-022) ----

/// BR-003: places that touch are all replaced (`xxxx` is four places).
#[test]
fn uc022_br003_touching_places_are_all_replaced() {
    let dir = temp_dir("touching");
    let (mut session, root) = session_with(&dir, &[("A.md", "xxxx\nabab\n")]);
    let result = replace(&mut session, "x", "y");
    assert_eq!(result.places, 4);
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "A.md"), "yyyy\nabab\n");
    let result = replace(&mut session, "ab", "-");
    assert_eq!(result.places, 2);
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "A.md"), "yyyy\n--\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-003, BR-006: a text that lowers to part of a character is no place;
/// nothing is inserted into the note.
#[test]
fn uc022_br003_nothing_is_inserted_inside_a_character() {
    let dir = temp_dir("dotted");
    let (mut session, root) = session_with(&dir, &[("A.md", "İstanbul\n")]);
    let asked = session.replace_set("i", "X", SearchSwitches::default());
    let result = places_for(&mut session, asked);
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "A.md"), "İstanbul\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-006: a replacement keeps what it does not replace: BOM, CRLF, a lone
/// CR, no line end at the end; closed and open.
#[test]
fn uc022_br006_bom_and_line_ends_are_kept() {
    let dir = temp_dir("bytes");
    let files: &[(&str, &str)] = &[
        ("Bom.md", "\u{feff}ferry\r\nferry\r\n"),
        ("Cr.md", "ferry\rferry\r"),
        ("Bare.md", "ferry"),
        ("Open.md", "\u{feff}ferry\r\nferry\r\n"),
        ("OpenCr.md", "ferry\rferry"),
    ];
    let (mut session, root) = session_with(&dir, files);
    assert!(session.open_note(&id("Open")));
    assert!(session.open_note(&id("OpenCr")));
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.places, 9);
    session.replace_confirm(result.serial, None);
    for (name, text) in files {
        assert_eq!(
            fs::read(root.join(name)).unwrap(),
            text.replace("ferry", "boat").into_bytes(),
            "{name}"
        );
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-001, A5: a file that is no UTF-8 is never touched and does not stop
/// the others; the list says that it could not be searched.
#[test]
fn uc022_br001_a_binary_note_is_left_alone() {
    let dir = temp_dir("binary");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let bad: Vec<u8> = [
        &b"ferry "[..],
        &[0xff, 0xfe, 0x00, 0xc3, 0x28],
        &b" ferry"[..],
    ]
    .concat();
    fs::write(root.join("Bad.md"), &bad).unwrap();
    fs::write(root.join("Empty.md"), "").unwrap();
    let (mut session, root) = session_with(&dir, &[("Good.md", "ferry\n")]);
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.places, 1);
    assert!(result.unreadable.is_some(), "the list says so");
    session.replace_confirm(result.serial, None);
    assert_eq!(fs::read(root.join("Bad.md")).unwrap(), bad);
    assert_eq!(read(&root, "Good.md"), "boat\n");
    assert_eq!(read(&root, "Empty.md"), "");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-001, BR-006: a link in the workspace to a file or folder outside of
/// it is never written through.
#[cfg(unix)]
#[test]
fn uc022_br001_symlinks_never_lead_outside_the_workspace() {
    let dir = temp_dir("symlink");
    let root = dir.join("notes");
    let outside = dir.join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("Secret.md"), "ferry secret\n").unwrap();
    fs::write(outside.join("Other.md"), "ferry other\n").unwrap();
    std::os::unix::fs::symlink(outside.join("Secret.md"), root.join("Link.md")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("Folder")).unwrap();
    let (mut session, root) = session_with(&dir, &[("Inside.md", "ferry\n")]);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&dir.join("outside"), "Secret.md"), "ferry secret\n");
    assert_eq!(read(&dir.join("outside"), "Other.md"), "ferry other\n");
    assert_eq!(read(&root, "Inside.md"), "boat\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-001: what lives in the workspace's own folders is never replaced.
#[test]
fn uc022_br001_the_workspaces_own_folders_stay() {
    let dir = temp_dir("own");
    let files: &[(&str, &str)] = &[
        (".underline/Notes.md", "ferry\n"),
        (".git/Note.md", "ferry\n"),
        (".todos/Todo.md", "ferry\n"),
        (".todos/a.yaml", "title: ferry\n"),
        ("Visible.md", "ferry\n"),
    ];
    let (mut session, root) = session_with(&dir, files);
    let result = replace(&mut session, "ferry", "boat");
    assert_eq!(result.summary, "1 place in 1 note");
    session.replace_confirm(result.serial, None);
    for (name, text) in &files[..4] {
        assert_eq!(read(&root, name), *text, "{name}");
    }
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004: after the request changed, the old places replace nothing.
#[test]
fn uc022_br004_a_confirm_after_the_request_changed_writes_nothing() {
    let dir = temp_dir("stale");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    session.replace_set("Hydra", "Crete", SearchSwitches::default());
    let status = session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Ideas.md"), "a ferry to Hydra\n");
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert!(status.starts_with("The places changed"), "{status}");
    // The same after the replace was closed.
    let result = replace(&mut session, "ferry", "boat");
    session.replace_close();
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Log.md"), "ferry\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004, step 5: two confirms in a row replace once, also when the
/// replacement holds the search text.
#[test]
fn uc022_br004_two_confirms_in_a_row_replace_once() {
    for (find, with, regex) in [
        ("ferry", "ferry boat", false),
        ("ferry", "boat", false),
        ("(ferry)", "$1$1", true),
        ("ferry", "ferry", false),
    ] {
        let dir = temp_dir("twice");
        let (mut session, root) = session_with(&dir, FERRY);
        let switches = SearchSwitches {
            regex,
            ..SearchSwitches::default()
        };
        let asked = session.replace_set(find, with, switches);
        let result = places_for(&mut session, asked);
        session.replace_confirm(result.serial, None);
        let once: Vec<String> = ["Trip.md", "Ideas.md", "Log.md"]
            .iter()
            .map(|f| read(&root, f))
            .collect();
        session.replace_confirm(result.serial, None);
        let twice: Vec<String> = ["Trip.md", "Ideas.md", "Log.md"]
            .iter()
            .map(|f| read(&root, f))
            .collect();
        assert_eq!(once, twice, "{find} -> {with}");
        fs::remove_dir_all(&dir).unwrap();
    }
}

/// BR-004, A5: a note deleted or renamed between the plan and the job is
/// not made again and not written; it is named, the others are replaced.
#[test]
fn uc022_a5_a_note_deleted_or_renamed_meanwhile_is_not_brought_back() {
    let dir = temp_dir("gone");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    let job = session.replace_plan(result.serial, None).unwrap();
    fs::remove_file(root.join("Ideas.md")).unwrap();
    fs::rename(root.join("Log.md"), root.join("Log2.md")).unwrap();
    let status = session.replace_finish(job.run());
    assert!(!root.join("Ideas.md").exists(), "not brought back");
    assert!(!root.join("Log.md").exists(), "not brought back");
    assert_eq!(read(&root, "Log2.md"), "ferry\n");
    assert_eq!(read(&root, "Trip.md"), "Book the boat\nboat times\n");
    assert!(
        status.contains("Ideas") && status.contains("Log"),
        "{status}"
    );
    assert!(
        status.starts_with("Replaced 2 places in 1 note"),
        "{status}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004: a note changed on disk between the plan and the job is left,
/// also when it changed to the same length, and also in the job's own undo.
#[test]
fn uc022_br004_a_note_changed_between_plan_and_job_is_left() {
    let dir = temp_dir("planjob");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    let job = session.replace_plan(result.serial, None).unwrap();
    fs::write(root.join("Log.md"), "ferrz\n").unwrap();
    fs::write(root.join("Ideas.md"), "a ferry to Crete\n").unwrap();
    let status = session.replace_finish(job.run());
    assert_eq!(read(&root, "Log.md"), "ferrz\n");
    assert!(status.contains("Log"), "{status}");
    // Ideas holds the place shown, a line of its own before and after
    // differ: it is left.
    assert_eq!(read(&root, "Ideas.md"), "a ferry to Crete\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-005, A6: the undo after the note was changed in its tab since leaves
/// the tab and the file as they are, and names the note.
#[test]
fn uc022_a6_undo_after_further_edits_in_the_tab() {
    let dir = temp_dir("undo-tab");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Log")));
    let result = replace(&mut session, "ferry", "boat");
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Log.md"), "boat\n");
    session
        .backend_edit(&id("Log"), &[(0..0, "my own words ".to_owned())], false)
        .unwrap();
    let status = session.replace_undo();
    assert_eq!(
        tab(&mut session, "Log").mirror.text(),
        "my own words boat\n"
    );
    assert!(status.contains("Log was changed since"), "{status}");
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1, "the others are back");
    fs::remove_dir_all(&dir).unwrap();
}

/// A6: the undo gives back a note whose tab was closed since, and twice
/// is no second undo; an undo with no replacement does nothing.
#[test]
fn uc022_a6_undo_with_the_tab_closed_and_undo_twice() {
    let dir = temp_dir("undo-twice");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Log")));
    let result = replace(&mut session, "ferry", "boat");
    session.replace_confirm(result.serial, None);
    session.close_tab(&id("Log"), true);
    session.replace_undo();
    assert_eq!(read(&root, "Log.md"), "ferry\n");
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    fs::write(root.join("Log.md"), "mine\n").unwrap();
    session.replace_undo();
    assert_eq!(read(&root, "Log.md"), "mine\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-004, ADR-025: a tab opened between the plan and the job reads the
/// new text and does not store the old one over it.
#[test]
fn uc022_br004_a_tab_opened_meanwhile_does_not_undo_the_replacement() {
    let dir = temp_dir("opened");
    let (mut session, root) = session_with(&dir, FERRY);
    let result = replace(&mut session, "ferry", "boat");
    let job = session.replace_plan(result.serial, None).unwrap();
    assert!(session.open_note(&id("Log")));
    let done = job.run();
    session.replace_finish(done);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(2500) {
        session.tick(Instant::now());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(read(&root, "Log.md"), "boat\n", "the file");
    assert_eq!(tab(&mut session, "Log").mirror.text(), "boat\n", "the tab");
    fs::remove_dir_all(&dir).unwrap();
}

/// Step 3: every place left out one by one: nothing is replaced and the
/// status says so; identical lines are told apart.
#[test]
fn uc022_step3_places_left_out_one_by_one() {
    let dir = temp_dir("oneby");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Same.md", "ferry\nferry\nferry ferry\n"),
            ("Two.md", "ferry\r\nferry\n"),
        ],
    );
    let result = replace(&mut session, "ferry", "boat");
    let same = result.list.iter().find(|n| n.title == "Same").unwrap();
    assert_eq!(same.places.len(), 4);
    // Leave out the second line and the second place of the third line.
    for at in [same.places[1].at, same.places[3].at] {
        assert!(session
            .replace_leave_out(result.serial, &id("Same"), Some(at), true)
            .is_some());
    }
    let two = result.list.iter().find(|n| n.title == "Two").unwrap();
    session.replace_leave_out(result.serial, &id("Two"), Some(two.places[1].at), true);
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "Same.md"), "boat\nferry\nboat ferry\n");
    assert_eq!(read(&root, "Two.md"), "boat\r\nferry\n");

    let waited = Instant::now();
    while waited.elapsed() < Duration::from_millis(600) {
        session.tick(Instant::now());
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = replace(&mut session, "ferry", "x");
    for note in &result.list {
        for place in &note.places {
            session.replace_leave_out(result.serial, &id(&note.note), Some(place.at), true);
        }
    }
    let status = session.replace_confirm(result.serial, None);
    assert_eq!(status, "Nothing was replaced: every place is left out");
    assert_eq!(read(&root, "Two.md"), "boat\r\nferry\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-007: a note of one very long line is listed and replaced at once.
#[test]
fn uc022_br007_a_single_line_of_megabytes() {
    let dir = temp_dir("longline");
    let line = format!(
        "{}ferry{}",
        "word ".repeat(400_000),
        "ferry ".repeat(20_000)
    );
    let (mut session, root) = session_with(&dir, &[("Long.md", &line)]);
    let started = Instant::now();
    let regex = SearchSwitches {
        regex: true,
        ..SearchSwitches::default()
    };
    let asked = session.replace_set("(fer)(ry)", "$2$1", regex);
    let result = places_for(&mut session, asked);
    assert_eq!(result.places, 20_001);
    session.replace_confirm(result.serial, None);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(read(&root, "Long.md"), line.replace("ferry", "ryfer"));
    fs::remove_dir_all(&dir).unwrap();
}

/// FR-075: a regular expression that names a line end replaces across
/// lines in a note on disk and in a tab, with CRLF.
#[test]
fn uc022_fr075_a_place_over_a_line_end() {
    let dir = temp_dir("lines");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("A.md", "one\ntwo\nthree\n"),
            ("B.md", "one\r\ntwo\r\nthree\r\n"),
        ],
    );
    assert!(session.open_note(&id("B")));
    let regex = SearchSwitches {
        regex: true,
        ..SearchSwitches::default()
    };
    let asked = session.replace_set(r"one\s*\ntwo", "1+2", regex);
    let result = places_for(&mut session, asked);
    assert_eq!(result.places, 2);
    session.replace_confirm(result.serial, None);
    assert_eq!(read(&root, "A.md"), "1+2\nthree\n");
    assert_eq!(read(&root, "B.md"), "1+2\r\nthree\r\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// The snapshots of `file` in the workspace at `root`, newest first, with
/// their causes and contents (UC-026).
fn snapshots(root: &Path, file: &str) -> Vec<(underline_core::SnapshotCause, String)> {
    underline_core::SnapshotStore::new(root)
        .list(file, underline_core::now())
        .iter()
        .map(|s| (s.cause, s.content().unwrap()))
        .collect()
}

/// TC-022-01 step 5, TC-022-07, TC-022-10, BR-005, UC-026 BR-001: every
/// note a replacement changes, closed or open, keeps its content from
/// before as a snapshot with the cause Replace in workspace; a note left
/// out gets none; the undo keeps the replaced text with the cause Restore.
#[test]
fn uc022_tc01_tc07_br005_snapshots_of_the_replaced_notes() {
    use underline_core::SnapshotCause::{ReplaceInWorkspace, Restore};
    let dir = temp_dir("snapshots");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Ideas")));
    let result = replace(&mut session, "ferry", "boat");
    session.replace_leave_out(result.serial, &id("Log"), None, true);

    session.replace_confirm(result.serial, None);

    assert_eq!(
        snapshots(&root, "Trip.md"),
        [(ReplaceInWorkspace, FERRY[0].1.to_owned())]
    );
    assert_eq!(
        snapshots(&root, "Ideas.md"),
        [(ReplaceInWorkspace, FERRY[1].1.to_owned())]
    );
    assert!(snapshots(&root, "Log.md").is_empty());

    session.replace_undo();

    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert_eq!(
        snapshots(&root, "Trip.md")[0],
        (Restore, "Book the boat\nboat times\n".to_owned())
    );
    assert_eq!(
        snapshots(&root, "Ideas.md")[0],
        (Restore, "a boat to Hydra\n".to_owned())
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-022-13, BR-001, BR-005: the snapshot of an open note holds its text
/// with the changes not yet saved.
#[test]
fn uc022_tc13_the_snapshot_holds_the_unsaved_text() {
    let dir = temp_dir("snapshot-unsaved");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Trip")));
    session
        .backend_edit(&id("Trip"), &[(0..0, "ferry pier\n".to_owned())], false)
        .unwrap();
    let result = replace(&mut session, "ferry", "boat");

    session.replace_confirm(result.serial, None);

    assert_eq!(
        snapshots(&root, "Trip.md")[0],
        (
            underline_core::SnapshotCause::ReplaceInWorkspace,
            "ferry pier\nBook the ferry\nferry times\n".to_owned()
        )
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// BR-005, UC-026 BR-001: a note whose snapshot cannot be kept is not
/// changed, open or closed, and is named with the reason; the others are.
#[test]
fn uc022_br005_no_change_without_a_snapshot() {
    let dir = temp_dir("snapshot-refused");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Ideas")));
    let snapshots = root.join(".underline/snapshots");
    fs::create_dir_all(&snapshots).unwrap();
    // A file where the folders of "Trip" and "Ideas" would go.
    fs::write(snapshots.join("Trip.md"), "").unwrap();
    fs::write(snapshots.join("Ideas.md"), "").unwrap();
    let result = replace(&mut session, "ferry", "boat");

    session.replace_confirm(result.serial, None);

    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert_eq!(read(&root, "Ideas.md"), FERRY[1].1);
    assert_eq!(read(&root, "Log.md"), "boat\n");
    assert_eq!(tab(&mut session, "Ideas").mirror.text(), FERRY[1].1);
    let status = session.status();
    assert!(
        status.starts_with("Replaced 1 place in 1 note; "),
        "{status}"
    );
    assert!(
        status.contains("Trip could not be changed: the snapshot cannot be kept"),
        "{status}"
    );
    assert!(
        status.contains("Ideas could not be changed: the snapshot cannot be kept"),
        "{status}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// Test monkey, BR-005, UC-026 BR-001: a replacement whose snapshot cannot
/// be taken because `.underline/snapshots` is read-only does not change the
/// note, open or closed (the failure is reported, nothing is half done).
#[cfg(unix)]
#[test]
fn uc022_monkey_br005_read_only_snapshot_folder_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("monkey-readonly");
    let (mut session, root) = session_with(&dir, FERRY);
    assert!(session.open_note(&id("Ideas")));
    let base = root.join(".underline/snapshots");
    fs::create_dir_all(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o555)).unwrap();
    let result = replace(&mut session, "ferry", "boat");

    session.replace_confirm(result.serial, None);

    fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(read(&root, "Trip.md"), FERRY[0].1);
    assert_eq!(read(&root, "Ideas.md"), FERRY[1].1);
    assert_eq!(tab(&mut session, "Ideas").mirror.text(), FERRY[1].1);
    assert!(snapshots(&root, "Trip.md").is_empty());
    fs::remove_dir_all(&dir).unwrap();
}
