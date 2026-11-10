//! Tests of creating, renaming, deleting and restoring notes (UC-003,
//! UC-006, UC-007, inventory M-05.4 to M-05.6 and M-14), against what the
//! egui app does (`apps/underline-desktop/src/app.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::notes_view::{
    CreateNoteAnswer, DeleteAnswer, NewNoteRequest, RenameAnswer, RestoreAnswer, StatusOffer,
};
use crate::workspace_view::FormatName;
use crate::{Event, Session, SettingsState, StatusView};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-notes-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files` (relative
/// path, content); the first note open in the active tab.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let root = dir.join("notes");
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

fn tabs(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .tabs
        .tabs
        .iter()
        .map(|tab| tab.label.clone())
        .collect()
}

fn overview(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .overview
        .iter()
        .map(|item| item.note.clone())
        .collect()
}

fn request(title: &str, folder: &str) -> NewNoteRequest {
    NewNoteRequest {
        title: title.into(),
        folder: folder.into(),
        format: FormatName::Markdown,
    }
}

/// The status events among `events`.
fn statuses(events: &[Event]) -> Vec<StatusView> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Status(status) => Some(status.clone()),
            _ => None,
        })
        .collect()
}

/// Ticks later and later until `done` holds, at most five seconds, so the
/// autosave is due and its background save is taken in.
fn tick_until(session: &mut Session, done: impl Fn(&Session) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        session.tick(Instant::now() + Duration::from_secs(3));
        if done(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

// UC-003, M-14.1

#[test]
fn uc003_m14_1_new_note_starts_in_the_active_folder_with_the_default_format() {
    let dir = temp_dir("new-start");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n"), ("guide/A.md", "# A\n")]);
    session.open_note(&id("guide/A"));

    let start = session.new_note_start().unwrap();

    assert_eq!(start.folder, "guide");
    assert_eq!(start.format, FormatName::Markdown);
}

#[test]
fn uc003_m14_1_the_dialog_shows_the_file_name_while_the_user_types() {
    let dir = temp_dir("new-plan");
    let (session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);

    assert_eq!(
        session.plan_new_note(&request("  Big   Idea ", "ideas")),
        Some("ideas/Big-Idea.md".to_owned())
    );
    assert_eq!(session.plan_new_note(&request("", "")), None);
    assert_eq!(session.plan_new_note(&request("Home", "")), None, "exists");
}

#[test]
fn uc003_m14_1_create_opens_the_note_with_the_cursor_at_its_end() {
    let dir = temp_dir("new-create");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);

    let answer = session.create_note(&request("Big Idea", "ideas"));

    let text = fs::read_to_string(root.join("ideas/Big-Idea.md")).unwrap();
    assert_eq!(text, "# Big Idea\n");
    let rev = session.mirror_rev(&id("ideas/Big-Idea"));
    assert_eq!(
        answer,
        CreateNoteAnswer::Created {
            note: "ideas/Big-Idea".into(),
            rev,
            offset: u32::try_from(text.encode_utf16().count()).unwrap(),
        }
    );
    assert_eq!(session.status(), "Created the note Big Idea");
    assert_eq!(
        session.snapshot().tabs.active.as_deref(),
        Some("ideas/Big-Idea")
    );
    assert!(overview(&session).contains(&"ideas/Big-Idea".to_owned()));
}

#[test]
fn uc003_a2_a3_m14_1_an_existing_note_is_offered_to_open() {
    let dir = temp_dir("new-exists");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);

    let answer = session.create_note(&request("home", ""));

    assert_eq!(
        answer,
        CreateNoteAnswer::Problem {
            problem: "The note Home exists. Change the title or the folder, or open it.".into(),
            existing: Some("Home".into()),
        }
    );
}

#[test]
fn uc003_a1_m14_1_a_title_not_allowed_keeps_the_dialog() {
    let dir = temp_dir("new-title");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);

    let answer = session.create_note(&request("a:b", ""));

    assert_eq!(
        answer,
        CreateNoteAnswer::Problem {
            problem: "The title cannot hold the character `:`.".into(),
            existing: None,
        }
    );
    assert!(!root.join("a:b.md").exists());
}

#[test]
fn uc003_a5_m14_1_a_note_that_cannot_be_written_closes_the_dialog() {
    let dir = temp_dir("new-io");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    // A file where the folder should be.
    fs::write(root.join("ideas"), "").unwrap();

    let answer = session.create_note(&request("Big", "ideas"));

    assert_eq!(answer, CreateNoteAnswer::Done);
    assert!(
        session
            .status()
            .starts_with("The note could not be created: "),
        "{}",
        session.status()
    );
}

// UC-006, M-14.2

#[test]
fn uc006_br009_m14_2_rename_starts_with_title_and_folder_and_warns_for_home() {
    let dir = temp_dir("rename-start");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n"), ("g/My-Plan.md", "")]);

    let home = session.rename_start(&id("Home")).unwrap();
    let plan = session.rename_start(&id("g/My-Plan")).unwrap();

    assert!(home.home);
    assert_eq!(
        (plan.title.as_str(), plan.folder.as_str()),
        ("My Plan", "g")
    );
    assert!(!plan.home);
    assert_eq!(
        session.plan_rename(&id("g/My-Plan"), "Plan B", ""),
        Some("Plan-B.md".to_owned())
    );
    assert_eq!(session.plan_rename(&id("g/My-Plan"), "My Plan", "g"), None);
}

#[test]
fn uc006_m14_2_rename_moves_the_note_and_its_links_follow() {
    let dir = temp_dir("rename");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Home.md", "# Home\n"),
            ("Plan.md", "# Plan\n"),
            ("Other.md", "See [Plan](Plan.md).\n"),
        ],
    );
    session.open_note(&id("Plan"));

    let answer = session.rename_note(&id("Plan"), "Roadmap", "");

    assert_eq!(
        answer,
        RenameAnswer::Renamed {
            from: "Plan".into(),
            to: "Roadmap".into()
        }
    );
    assert!(!root.join("Plan.md").exists());
    assert!(root.join("Roadmap.md").exists());
    assert_eq!(
        fs::read_to_string(root.join("Other.md")).unwrap(),
        "See [Roadmap](Roadmap.md).\n"
    );
    assert_eq!(
        session.status(),
        "Renamed Plan to Roadmap; updated 1 link in 1 note"
    );
    // Step 7: the tab shows the note under its new name, stored.
    assert_eq!(tabs(&session), ["Home", "Roadmap"]);
    assert_eq!(session.snapshot().tabs.active.as_deref(), Some("Roadmap"));
    assert!(session.snapshot().tabs.tabs[1].path.ends_with("Roadmap.md"));
    assert!(overview(&session).contains(&"Roadmap".to_owned()));
}

#[test]
fn uc006_a7_m14_2_links_of_an_open_tab_change_in_its_text_and_save_quietly() {
    let dir = temp_dir("rename-open");
    let (mut session, root) = session_with(
        &dir,
        &[("Home.md", "[Plan](Plan.md)\n"), ("Plan.md", "# Plan\n")],
    );
    session.open_note(&id("Home"));
    session.take_events();

    session.rename_note(&id("Plan"), "Roadmap", "");

    // The file is unchanged until the autosave; the tab holds the change.
    assert_eq!(
        fs::read_to_string(root.join("Home.md")).unwrap(),
        "[Plan](Plan.md)\n"
    );
    assert_eq!(session.window_text(&id("Home")), "[Roadmap](Roadmap.md)\n");
    assert!(session
        .take_events()
        .iter()
        .any(|e| matches!(e, Event::Changed(changed) if changed.note == "Home")));
    // M-14.2, A-82: no mark for the rename's links.
    assert_eq!(tabs(&session), ["Home"]);
    // UC-006 A7: the save keeps the rename's report.
    let report = session.status().to_owned();
    assert!(tick_until(&mut session, |_| fs::read_to_string(
        root.join("Home.md")
    )
    .unwrap()
    .contains("Roadmap")));
    assert_eq!(tabs(&session), ["Home"]);
    assert_eq!(session.status(), report);
}

#[test]
fn uc006_m14_2_a_change_of_the_user_after_the_rename_says_saved_again() {
    let dir = temp_dir("rename-typed");
    let (mut session, _) = session_with(
        &dir,
        &[("Home.md", "[Plan](Plan.md)\n"), ("Plan.md", "# Plan\n")],
    );
    session.open_note(&id("Home"));
    session.rename_note(&id("Plan"), "Roadmap", "");

    session.window_type(&id("Home"), 0, "x");

    assert!(tick_until(&mut session, |s| tabs(s) == ["Home"]));
    assert_eq!(session.status(), "Saved Home");
}

#[test]
fn uc006_br005_m14_2_a_moved_note_changes_its_own_links_in_its_tab() {
    let dir = temp_dir("rename-move");
    let (mut session, root) = session_with(
        &dir,
        &[("Home.md", "# Home\n"), ("Plan.md", "[Home](Home.md)\n")],
    );
    session.open_note(&id("Plan"));
    session.take_events();

    session.rename_note(&id("Plan"), "Plan", "work");

    assert_eq!(
        fs::read_to_string(root.join("work/Plan.md")).unwrap(),
        "[Home](../Home.md)\n"
    );
    assert_eq!(
        session.window_text(&id("work/Plan")),
        "[Home](../Home.md)\n"
    );
    // Stored already: no mark, and the window's core of it is rung.
    assert_eq!(tabs(&session), ["Home", "Plan"]);
    assert!(session
        .take_events()
        .iter()
        .any(|e| matches!(e, Event::Changed(changed) if changed.note == "work/Plan")));
}

#[test]
fn uc006_m14_2_the_tab_is_stored_before_the_rename() {
    let dir = temp_dir("rename-store");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    session.window_type(&id("Plan"), 6, " B");

    session.rename_note(&id("Plan"), "Roadmap", "");

    assert_eq!(
        fs::read_to_string(root.join("Roadmap.md")).unwrap(),
        "# Plan B\n"
    );
    assert_eq!(tabs(&session), ["Roadmap"]);
}

#[test]
fn uc006_a2_a4_m14_2_a_taken_name_is_refused_and_no_change_closes() {
    let dir = temp_dir("rename-taken");
    let (mut session, _) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "")]);

    assert_eq!(
        session.rename_note(&id("Plan"), "home", ""),
        RenameAnswer::Problem {
            problem: "A note named Home exists in that folder. Change the title or the folder."
                .into()
        }
    );
    assert_eq!(
        session.rename_note(&id("Plan"), "Plan", ""),
        RenameAnswer::Done
    );
    assert_eq!(
        session.rename_note(&id("Plan"), "", ""),
        RenameAnswer::Problem {
            problem: "The title is empty.".into()
        }
    );
}

// UC-007, M-14.3, M-05.6

#[test]
fn uc007_br001_br010_m14_3_the_question_names_the_links_and_the_home_note() {
    let dir = temp_dir("delete-question");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Home.md", "[a](Plan.md) [b](Plan.md)\n"),
            ("Plan.md", "# Plan\n"),
            ("Other.md", "[c](Plan.md)\n"),
        ],
    );

    let plan = session.delete_start(&id("Plan")).unwrap();
    let home = session.delete_start(&id("Home")).unwrap();

    assert_eq!(plan.title, "Plan");
    assert_eq!(
        plan.links,
        "3 links in 2 notes lead to it and will lead nowhere."
    );
    assert!(!plan.home);
    assert_eq!(home.links, "No links in other notes lead to it.");
    assert!(home.home);
}

#[test]
fn uc007_m14_3_deleting_closes_the_tab_and_offers_the_undo() {
    let dir = temp_dir("delete");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    session.window_type(&id("Plan"), 6, "!");
    session.take_events();

    let answer = session.delete_note(&id("Plan"), false);

    assert_eq!(answer, DeleteAnswer::Done);
    assert!(!root.join("Plan.md").exists());
    assert_eq!(tabs(&session), ["Home"]);
    assert_eq!(overview(&session), ["Home"]);
    let events = session.take_events();
    assert_eq!(
        statuses(&events),
        [StatusView {
            text: "Deleted Plan".into(),
            offer: Some(StatusOffer::UndoDelete),
        }]
    );
    let deleted = events
        .iter()
        .find_map(|e| match e {
            Event::Deleted(deleted) => Some(deleted.clone()),
            _ => None,
        })
        .expect("the deleted notes are sent");
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].title, "Plan");
    assert!(
        deleted[0].hover.starts_with("top folder · deleted "),
        "{}",
        deleted[0].hover
    );

    // UC-007 A4: the undo puts it back, with its unsaved change, in its tab.
    assert_eq!(
        session.undo_delete(),
        RestoreAnswer::Restored {
            note: "Plan".into()
        }
    );
    assert_eq!(
        fs::read_to_string(root.join("Plan.md")).unwrap(),
        "# Plan!\n"
    );
    assert_eq!(tabs(&session), ["Home", "Plan"]);
    assert_eq!(session.status(), "Restored Plan");
    assert!(statuses(&session.take_events())
        .iter()
        .all(|status| status.offer.is_none()));
}

#[test]
fn uc007_a4_m14_3_another_report_ends_the_undo_but_a_save_does_not() {
    let dir = temp_dir("delete-offer");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n"), ("Plan.md", "")]);
    session.open_note(&id("Home"));
    session.delete_note(&id("Plan"), false);
    session.take_events();

    // D-04 (fixed): "Saved Home" does not end the offer.
    session.window_type(&id("Home"), 0, "x");
    assert!(tick_until(&mut session, |s| tabs(s) == ["Home"]));
    assert_eq!(session.status(), "Deleted Plan");

    session.check_wiki("owner/repo");
    let events = session.take_events();
    assert!(statuses(&events).iter().all(|s| s.offer.is_none()));
    assert_eq!(session.undo_delete(), RestoreAnswer::Done);
    assert!(!root.join("Plan.md").exists(), "no undo any more");
}

#[test]
fn uc007_a8_m14_3_a_note_that_cannot_be_kept_is_deleted_for_good() {
    let dir = temp_dir("delete-cannot-keep");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "")]);
    // The Underline folder cannot be made: a file stands in its place.
    let _ = fs::remove_dir_all(root.join(".underline"));
    fs::write(root.join(".underline"), "").unwrap();

    let answer = session.delete_note(&id("Plan"), false);

    let DeleteAnswer::CannotKeep { question } = answer else {
        panic!("{answer:?}");
    };
    assert!(
        question.starts_with("The note cannot be kept among the deleted notes ("),
        "{question}"
    );
    assert!(question.ends_with("), so it cannot be restored. Delete it for good?"));
    assert!(root.join("Plan.md").exists());

    assert_eq!(session.delete_note(&id("Plan"), true), DeleteAnswer::Done);
    assert!(!root.join("Plan.md").exists());
    assert_eq!(session.status(), "Deleted Plan");
    assert!(statuses(&session.take_events())
        .iter()
        .all(|status| status.offer.is_none()));
}

#[test]
fn uc007_a5_a6_m14_4_a_taken_place_asks_for_another() {
    let dir = temp_dir("restore-taken");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "# Old\n")]);
    session.delete_note(&id("Plan"), false);
    fs::write(root.join("Plan.md"), "# New\n").unwrap();
    let key = session.deleted_view()[0].key.clone();

    let answer = session.restore_deleted(&key);

    assert_eq!(
        answer,
        RestoreAnswer::Ask {
            title: "Plan".into(),
            folder: String::new(),
            problem: "A note named Plan exists in that folder. Choose another title or folder."
                .into(),
        }
    );
    assert_eq!(
        session.restore_deleted_as("Plan", ""),
        RestoreAnswer::Ask {
            title: "Plan".into(),
            folder: String::new(),
            problem: "A note named Plan exists in that folder. Choose another title or folder."
                .into(),
        }
    );
    assert_eq!(
        session.restore_deleted_as("Old Plan", "archive"),
        RestoreAnswer::Restored {
            note: "archive/Old-Plan".into()
        }
    );
    assert_eq!(
        fs::read_to_string(root.join("archive/Old-Plan.md")).unwrap(),
        "# Old\n"
    );
    assert_eq!(session.status(), "Restored Old Plan");
    assert!(session.deleted_view().is_empty());
    // BR-006: no tab comes back from the list.
    assert!(!tabs(&session).contains(&"Old Plan".to_owned()));
}

#[test]
fn uc007_a7_m14_4_removing_one_or_all_for_good() {
    let dir = temp_dir("purge");
    let (mut session, _) = session_with(&dir, &[("Home.md", ""), ("A.md", ""), ("B.md", "")]);
    session.delete_note(&id("A"), false);
    session.delete_note(&id("B"), false);
    assert_eq!(session.deleted_view().len(), 2);
    let first = session.deleted_view()[0].key.clone();

    session.purge_deleted(Some(&first));

    assert_eq!(session.status(), "Removed 1 deleted note for good");
    assert_eq!(session.deleted_view().len(), 1);
    session.purge_deleted(None);
    assert_eq!(session.status(), "Removed 1 deleted note for good");
    assert!(session.deleted_view().is_empty());
    // The undo of B went with it.
    assert_eq!(session.undo_delete(), RestoreAnswer::Done);
}

#[test]
fn m05_6_the_deleted_notes_are_sent_when_the_workspace_opens() {
    let dir = temp_dir("deleted-open");
    let root = dir.join("notes");
    let kept = root.join(".underline/deleted/4102444800000/ideas");
    fs::create_dir_all(&kept).unwrap();
    fs::write(kept.join("Old.md"), "# Old\n").unwrap();
    fs::write(root.join("Home.md"), "").unwrap();
    let mut session = Session::new(
        Some(root),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
    );
    session.start_workspace();

    let events = session.take_events();

    let deleted = events
        .iter()
        .find_map(|e| match e {
            Event::Deleted(deleted) => Some(deleted.clone()),
            _ => None,
        })
        .expect("the deleted notes are sent");
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].title, "Old");
    assert!(deleted[0].hover.starts_with("ideas · deleted "));
}

#[test]
fn uc006_m14_2_a_change_typed_just_before_the_rename_is_stored_under_the_new_name() {
    let dir = temp_dir("rename-quick");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    session.window_type(&id("Plan"), 6, "!");

    session.rename_note(&id("Plan"), "Roadmap", "");
    let report = session.status().to_owned();
    for _ in 0..10 {
        session.tick(Instant::now() + Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(tabs(&session), ["Roadmap"]);
    assert_eq!(session.status(), report);
    assert_eq!(
        fs::read_to_string(root.join("Roadmap.md")).unwrap(),
        "# Plan!\n"
    );
}

#[test]
fn uc006_a7_m14_2_links_after_non_ascii_text_change_in_an_open_tab() {
    let dir = temp_dir("rename-utf8");
    let text = "Grüße 😀 [Plan](Plan.md) und [Plan](Plan.md)\n";
    let (mut session, _) = session_with(&dir, &[("Home.md", text), ("Plan.md", "# Plan\n")]);
    session.open_note(&id("Home"));

    session.rename_note(&id("Plan"), "Größer", "");

    assert_eq!(
        session.window_text(&id("Home")),
        "Grüße 😀 [Größer](Größer.md) und [Größer](Größer.md)\n"
    );
    assert_eq!(
        session.status(),
        "Renamed Plan to Größer; updated 2 links in 1 note"
    );
}

#[test]
fn uc006_m14_2_a_rename_that_only_changes_the_case_is_done() {
    let dir = temp_dir("rename-case");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("plan.md", "# plan\n")]);
    session.open_note(&id("plan"));

    let answer = session.rename_note(&id("plan"), "Plan", "");

    assert_eq!(
        answer,
        RenameAnswer::Renamed {
            from: "plan".into(),
            to: "Plan".into()
        }
    );
    assert!(fs::read_dir(&root)
        .unwrap()
        .any(|e| e.unwrap().file_name() == "Plan.md"));
    assert_eq!(tabs(&session), ["Home", "Plan"]);
}

#[test]
fn uc003_uc006_names_not_allowed_are_refused() {
    let dir = temp_dir("names");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "")]);

    for (title, folder) in [
        ("x", ".."),
        ("x", "/x"),
        ("CON", ""),
        ("x", "a/../.."),
        ("x", ".hidden"),
    ] {
        let created = session.create_note(&request(title, folder));
        assert!(
            matches!(created, CreateNoteAnswer::Problem { .. }),
            "{title} in {folder}: {created:?}"
        );
        let renamed = session.rename_note(&id("Plan"), title, folder);
        assert!(
            matches!(renamed, RenameAnswer::Problem { .. }),
            "{title} in {folder}: {renamed:?}"
        );
    }
    assert!(root.join("Plan.md").exists());
    assert!(!dir.join("x.md").exists());
}

#[test]
fn uc007_a6_a_place_taken_by_another_case_or_format_asks() {
    let dir = temp_dir("restore-case");
    let (mut session, root) = session_with(&dir, &[("Home.md", ""), ("Plan.md", "# Old\n")]);
    session.delete_note(&id("Plan"), false);
    fs::write(root.join("PLAN.adoc"), "= New\n").unwrap();
    session.take_events();
    let key = session.deleted_view()[0].key.clone();

    let answer = session.restore_deleted(&key);

    assert!(
        matches!(&answer, RestoreAnswer::Ask { problem, .. }
            if problem == "A note named PLAN exists in that folder. Choose another title or folder."),
        "{answer:?}"
    );
    assert!(!root.join("Plan.md").exists());
}

#[test]
fn uc007_a7_a_stale_key_removes_nothing() {
    let dir = temp_dir("purge-stale");
    let (mut session, _) = session_with(&dir, &[("Home.md", ""), ("A.md", "")]);
    session.delete_note(&id("A"), false);
    let status = session.status().to_owned();

    session.purge_deleted(Some("1:0:Nothing"));
    assert_eq!(session.restore_deleted("1:0:Nothing"), RestoreAnswer::Done);

    assert_eq!(session.deleted_view().len(), 1);
    assert_eq!(session.status(), status);
}
