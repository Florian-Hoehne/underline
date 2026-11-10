//! Tests of the workspace part of the session (UC-001, UC-002, M-05, M-06,
//! M-21, M-22).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME, STATE_WRITE_DELAY};

use crate::workspace_view::{
    CreateAnswer, EditingViewName, FormatName, NewWorkspaceRequest, OpenAnswer, OpenOffer,
};
use crate::{Event, Session, SettingsState};

/// A fresh, empty folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-workspace-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session whose app state is `dir/data/state.yaml` and whose user
/// settings file is in `dir/home`, the home directory of the test.
fn session_in(dir: &Path, given: Option<PathBuf>) -> Session {
    fs::create_dir_all(dir.join("home")).unwrap();
    let (state, notice) = StateStore::load(dir.join("data").join("state.yaml"));
    Session::new(
        given,
        state,
        notice,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    )
}

/// A workspace folder `dir/name` with Markdown notes of these ids.
fn workspace_with(dir: &Path, name: &str, notes: &[&str]) -> PathBuf {
    let root = dir.join(name);
    for note in notes {
        let file = root.join(format!("{note}.md"));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, format!("# {note}\n")).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    root
}

/// Waits until the clock of "last opened" (milliseconds) moved on.
fn later() {
    std::thread::sleep(Duration::from_millis(5));
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap()
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

fn overview(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .overview
        .into_iter()
        .map(|item| item.note)
        .collect()
}

fn tabs(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .tabs
        .tabs
        .into_iter()
        .map(|tab| tab.note)
        .collect()
}

fn active(session: &Session) -> Option<String> {
    session.snapshot().tabs.active
}

/// Ticks until `done` holds, at most five seconds; the watcher collects
/// changes for 200 ms before it reports them (ADR-011).
fn tick_until(session: &mut Session, done: impl Fn(&Session) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        session.tick(Instant::now());
        if done(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

// M-01.1, UC-002 A1, A2

#[test]
fn uc002_a1_m01_1_start_opens_the_given_directory() {
    let dir = temp_dir("start-given");
    let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
    let mut session = session_in(&dir, Some(root.clone()));

    let started = session.start_workspace();

    assert_eq!(started.answer, OpenAnswer::Done);
    let workspace = started.snapshot.workspace.unwrap();
    assert_eq!(workspace.root, absolute(&root).display().to_string());
    assert_eq!(workspace.name, "notes");
    assert_eq!(started.snapshot.tabs.active.as_deref(), Some("Home"));
    assert_eq!(
        session.status(),
        format!("Opened {}", absolute(&root).display())
    );
}

#[test]
fn uc002_a2_m01_1_start_opens_the_workspace_opened_last() {
    let dir = temp_dir("start-last");
    let older = workspace_with(&dir, "older", &["Home"]);
    let newer = workspace_with(&dir, "newer", &["Home"]);
    let mut first = session_in(&dir, Some(older));
    first.start_workspace();
    first.open_chosen(&newer, false);
    first.write_state_now().unwrap();

    let mut session = session_in(&dir, None);
    let started = session.start_workspace();

    assert_eq!(started.answer, OpenAnswer::Done);
    assert_eq!(started.snapshot.workspace.unwrap().name, "newer");
}

#[test]
fn uc002_a2_m02_1_without_a_workspace_to_start_with_nothing_opens() {
    let dir = temp_dir("start-none");
    let mut session = session_in(&dir, None);

    let started = session.start_workspace();

    assert_eq!(started.answer, OpenAnswer::Done);
    assert_eq!(started.snapshot.workspace, None);
    assert!(started.snapshot.overview.is_empty());
    assert_eq!(session.status(), "");
}

#[test]
fn m01_1_only_the_first_start_opens_anything() {
    let dir = temp_dir("start-once");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let other = workspace_with(&dir, "other", &["Home"]);
    let mut session = session_in(&dir, Some(root));
    session.start_workspace();
    session.open_chosen(&other, false);

    let again = session.start_workspace();

    assert_eq!(again.answer, OpenAnswer::Done);
    assert_eq!(again.snapshot.workspace.unwrap().name, "other");
}

#[test]
fn uc002_a3_m01_1_a_given_directory_that_is_gone_shows_the_open_dialog() {
    let dir = temp_dir("start-missing");
    let missing = dir.join("gone");
    let mut session = session_in(&dir, Some(missing.clone()));

    let started = session.start_workspace();

    let OpenAnswer::Problem {
        path,
        problem,
        offer,
        ..
    } = started.answer
    else {
        panic!("the dialog shows the problem");
    };
    assert_eq!(path, missing.display().to_string());
    assert_eq!(
        problem,
        format!(
            "{} does not exist. Choose another directory or create a new workspace there.",
            missing.display()
        )
    );
    assert_eq!(
        offer,
        Some(OpenOffer::Create {
            directory: missing.display().to_string()
        })
    );
    assert_eq!(started.snapshot.workspace, None);
}

// M-21.1, UC-002 step 4

#[test]
fn uc002_a3_m21_1_a_remembered_workspace_that_is_gone_is_forgotten() {
    let dir = temp_dir("forget");
    let gone = workspace_with(&dir, "gone", &["Home"]);
    let kept = workspace_with(&dir, "kept", &["Home"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&gone, false);
    session.open_chosen(&kept, false);
    fs::remove_dir_all(&gone).unwrap();

    let answer = session.open_chosen(&gone, false);

    let OpenAnswer::Problem { recent, .. } = answer else {
        panic!("the dialog shows the problem");
    };
    assert_eq!(recent, [absolute(&kept).display().to_string()]);
    assert_eq!(session.recent_workspaces(), recent);
}

#[test]
fn uc002_a5_a6_m21_1_a_directory_inside_a_workspace_offers_both() {
    let dir = temp_dir("inside");
    let outer = workspace_with(&dir, "outer", &["Home", "inner/Page"]);
    fs::write(outer.join(SETTINGS_FILE_NAME), "").unwrap();
    let inner = outer.join("inner");
    let mut session = session_in(&dir, None);

    let answer = session.open_chosen(&inner, false);

    let OpenAnswer::Problem { problem, offer, .. } = answer else {
        panic!("the dialog shows the problem");
    };
    let (outer, inner) = (absolute(&outer), absolute(&inner));
    assert_eq!(
        problem,
        format!(
            "{} lies inside the workspace {}.",
            inner.display(),
            outer.display()
        )
    );
    assert_eq!(
        offer,
        Some(OpenOffer::Outer {
            outer: outer.display().to_string(),
            inner: inner.display().to_string(),
        })
    );

    assert_eq!(session.open_chosen(&outer, true), OpenAnswer::Done);
    assert_eq!(session.workspace_root(), Some(outer.as_path()));
    // A5 step 2: the inner directory anyway, although the outer is open.
    assert_eq!(session.open_chosen(&inner, true), OpenAnswer::Done);
    assert_eq!(session.workspace_root(), Some(inner.as_path()));
    assert_eq!(overview(&session), ["Page"]);
}

#[test]
fn uc002_a11_m21_1_the_home_directory_is_refused() {
    let dir = temp_dir("home");
    let mut session = session_in(&dir, None);
    let home = absolute(&dir.join("home"));

    let answer = session.open_chosen(&home, false);

    let OpenAnswer::Problem { problem, offer, .. } = answer else {
        panic!("the dialog shows the problem");
    };
    assert_eq!(
        problem,
        format!(
            "the home directory {} cannot be a workspace, because it holds the user settings file and all of your files. Choose another directory.",
            home.display()
        )
    );
    assert_eq!(offer, None);
}

#[test]
fn uc002_m21_1_a_file_is_no_directory() {
    let dir = temp_dir("file");
    let file = dir.join("note.md");
    fs::write(&file, "").unwrap();
    let mut session = session_in(&dir, None);

    let answer = session.open_chosen(&file, false);

    assert!(matches!(
        answer,
        OpenAnswer::Problem { problem, offer: None, .. }
            if problem == format!("{} is a file, not a directory. Choose another directory.", file.display())
    ));
}

#[test]
fn uc002_a9_m21_1_the_open_workspace_stays_as_it_is() {
    let dir = temp_dir("same");
    let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Ideas"));
    session.check_wiki("not a url");
    let status = session.status().to_owned();

    let answer = session.open_chosen(&root.join("."), false);

    assert_eq!(answer, OpenAnswer::Done);
    assert_eq!(session.status(), status);
    assert_eq!(tabs(&session), ["Home", "Ideas"]);
}

#[test]
fn uc002_a8_m21_1_a_broken_workspace_settings_file_is_named() {
    let dir = temp_dir("broken-settings");
    let root = workspace_with(&dir, "notes", &["Home"]);
    fs::write(root.join(SETTINGS_FILE_NAME), "theme: [unclosed").unwrap();
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);

    // The settings' own message comes last, as in the egui app.
    assert!(
        session
            .status()
            .starts_with("The settings keep their last values: "),
        "{}",
        session.status()
    );
    assert_eq!(session.workspace_root(), Some(absolute(&root).as_path()));
}

// M-21.2, UC-001

#[test]
fn uc001_m21_2_a_new_directory_goes_on_to_the_options() {
    let dir = temp_dir("check-new");
    let session = session_in(&dir, None);
    let path = dir.join("fresh").display().to_string();

    let answer = session.check_new_directory(&format!("  {path} "));

    assert_eq!(answer, CreateAnswer::Options { directory: path });
}

#[test]
fn uc001_a1_m21_2_a_directory_with_content_offers_to_open_it() {
    let dir = temp_dir("check-full");
    let full = workspace_with(&dir, "full", &["Page"]);
    let session = session_in(&dir, None);
    let typed = full.display().to_string();

    let answer = session.check_new_directory(&typed);

    assert_eq!(
        answer,
        CreateAnswer::Directory {
            path: typed,
            problem: format!(
                "{} already holds files or folders. Choose another directory or open it as a workspace.",
                absolute(&full).display()
            ),
            offer: Some(absolute(&full).display().to_string()),
        }
    );
}

#[test]
fn uc001_a7_m21_2_a_directory_inside_a_workspace_is_refused() {
    let dir = temp_dir("check-inside");
    let outer = workspace_with(&dir, "outer", &["Home"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&outer, false);
    let inner = outer.join("sub");

    let answer = session.check_new_directory(&inner.display().to_string());

    assert!(matches!(
        answer,
        CreateAnswer::Directory { problem, offer: None, .. }
            if problem.ends_with(" Choose another directory.") && problem.contains("lies inside the workspace")
    ));
}

#[test]
fn uc001_m21_2_creating_opens_the_home_note_and_says_so() {
    let dir = temp_dir("create");
    let before = workspace_with(&dir, "before", &["Home", "Old"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&before, false);
    let root = dir.join("created");

    let answer = session.create_workspace(NewWorkspaceRequest {
        directory: root.display().to_string(),
        format: FormatName::AsciiDoc,
        git: false,
        remote: "ignored without git".into(),
    });

    assert_eq!(answer, CreateAnswer::Done);
    assert!(root.join("Home.adoc").is_file());
    assert!(!root.join(".git").exists());
    assert_eq!(
        session.status(),
        format!("Workspace created in {}", absolute(&root).display())
    );
    assert_eq!(tabs(&session), ["Home"]);
    assert_eq!(active(&session).as_deref(), Some("Home"));
    assert_eq!(overview(&session), ["Home"]);
    // UC-001 step 6: the workspace before keeps its tabs.
    session.open_chosen(&before, false);
    assert_eq!(tabs(&session), ["Home"]);
}

#[test]
fn uc001_a1_m21_2_creating_in_a_directory_that_filled_up_goes_back() {
    let dir = temp_dir("create-full");
    let root = workspace_with(&dir, "full", &["Page"]);
    let mut session = session_in(&dir, None);

    let answer = session.create_workspace(NewWorkspaceRequest {
        directory: root.display().to_string(),
        format: FormatName::Markdown,
        git: false,
        remote: String::new(),
    });

    assert!(matches!(
        answer,
        CreateAnswer::Directory { offer: Some(_), .. }
    ));
    assert_eq!(session.workspace_root(), None);
}

// M-05.2, UC-002 step 7, BR-008

#[test]
fn uc002_br008_m05_2_recently_opened_first_then_a_to_z() {
    let dir = temp_dir("overview");
    let root = workspace_with(&dir, "notes", &["Home", "beta", "Alpha", "guide/Gamma"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);

    assert_eq!(overview(&session), ["Home", "Alpha", "beta", "guide/Gamma"]);

    later();
    session.open_note(&id("guide/Gamma"));

    assert_eq!(overview(&session), ["guide/Gamma", "Home", "Alpha", "beta"]);
    assert_eq!(active(&session).as_deref(), Some("guide/Gamma"));
}

#[test]
fn uc004_a10_m05_2_a_note_that_cannot_be_read_gets_no_tab() {
    let dir = temp_dir("unreadable-note");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);

    session.open_note(&id("Missing"));

    assert!(
        session.status().starts_with("Missing cannot be read: "),
        "{}",
        session.status()
    );
    assert_eq!(tabs(&session), ["Home"]);
}

#[test]
fn m05_2_a_note_outside_the_workspace_is_refused() {
    let dir = temp_dir("escape");
    let root = workspace_with(&dir, "notes", &["Home"]);
    workspace_with(&dir, "secret", &["Plan"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);

    session.open_note(&id("../secret/Plan"));

    assert_eq!(tabs(&session), ["Home"]);
}

// M-06, UC-004

#[test]
fn m06_1_a_tab_shows_its_title_and_its_file() {
    let dir = temp_dir("tab-label");
    let root = workspace_with(&dir, "notes", &["Getting-Started"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);

    session.open_note(&id("Getting-Started"));

    let tab = &session.snapshot().tabs.tabs[0];
    assert_eq!(tab.label, "Getting Started");
    assert_eq!(
        tab.path,
        absolute(&root)
            .join("Getting-Started.md")
            .display()
            .to_string()
    );
    assert_eq!(tab.view, EditingViewName::Facade);
}

#[test]
fn m06_2_activating_a_tab_moves_its_note_up() {
    let dir = temp_dir("activate");
    let root = workspace_with(&dir, "notes", &["Home", "A", "B"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));
    later();
    session.open_note(&id("B"));
    later();
    assert_eq!(overview(&session), ["B", "A", "Home"]);

    session.activate_tab(&id("Home"));

    assert_eq!(active(&session).as_deref(), Some("Home"));
    assert_eq!(overview(&session), ["Home", "B", "A"]);
    // New tabs open right of the active one.
    assert_eq!(tabs(&session), ["Home", "A", "B"]);
}

#[test]
fn m06_3_closing_a_tab_activates_its_right_neighbour() {
    let dir = temp_dir("close");
    let root = workspace_with(&dir, "notes", &["Home", "A", "B"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));
    session.open_note(&id("B"));
    session.activate_tab(&id("A"));

    session.close_tab(&id("A"), false);

    assert_eq!(tabs(&session), ["Home", "B"]);
    assert_eq!(active(&session).as_deref(), Some("B"));
    session.close_tab(&id("Unknown"), false);
    assert_eq!(tabs(&session), ["Home", "B"]);
}

#[test]
fn m06_7_m22_2_tabs_views_and_the_active_tab_come_back_from_the_state_file() {
    let dir = temp_dir("restore");
    let root = workspace_with(&dir, "notes", &["Home", "A", "B"]);
    fs::create_dir_all(root.join(".underline")).unwrap();
    fs::write(
        root.join(".underline").join("state.yaml"),
        "tabs:\n- note: A\n  position: 1\n  editing_view: facade\n  active: false\n- note: B\n  position: 2\n  editing_view: source\n  active: true\n- note: Gone\n  position: 3\n  editing_view: facade\n  active: false\n",
    )
    .unwrap();
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);

    let snapshot = session.snapshot();
    assert_eq!(tabs(&session), ["A", "B"]);
    assert_eq!(snapshot.tabs.active.as_deref(), Some("B"));
    assert_eq!(snapshot.tabs.tabs[1].view, EditingViewName::Source);
    // Restoring tabs does not change the order of the overview.
    assert_eq!(overview(&session), ["A", "B", "Home"]);
}

#[test]
fn uc002_br005_m06_7_without_remembered_tabs_the_readme_opens() {
    let dir = temp_dir("readme");
    let root = workspace_with(&dir, "notes", &["README", "Zeta"]);
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);

    assert_eq!(tabs(&session), ["README"]);
    assert_eq!(overview(&session), ["README", "Zeta"]);
}

#[test]
fn uc002_br005_m06_7_without_home_or_readme_no_tab_opens() {
    let dir = temp_dir("no-first");
    let root = workspace_with(&dir, "notes", &["Zeta"]);
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);

    assert!(tabs(&session).is_empty());
    assert_eq!(active(&session), None);
}

#[test]
fn m22_2_the_tabs_are_written_to_the_workspace_state_file() {
    let dir = temp_dir("persist");
    let root = workspace_with(&dir, "notes", &["Home", "A"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));

    session.tick(Instant::now() + STATE_WRITE_DELAY + Duration::from_millis(10));

    let written = fs::read_to_string(root.join(".underline").join("state.yaml")).unwrap();
    assert!(written.contains("note: A"), "{written}");
    assert!(written.contains("note: Home"), "{written}");
    let app = fs::read_to_string(dir.join("data").join("state.yaml")).unwrap();
    assert!(
        app.contains(&absolute(&root).display().to_string()),
        "{app}"
    );
}

#[test]
fn m06_5_m22_2_opening_a_note_leaves_the_todos_tab_open_but_not_active() {
    let dir = temp_dir("todos-tab");
    let root = workspace_with(&dir, "notes", &["Home", "A"]);
    fs::create_dir_all(root.join(".underline")).unwrap();
    fs::write(
        root.join(".underline").join("state.yaml"),
        "tabs:\n- note: Home\n  position: 1\n  editing_view: facade\n  active: true\ntodo_list:\n  open: true\n  active: true\n",
    )
    .unwrap();
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    let restored = session.snapshot().tabs;
    assert!(restored.todo_list.open && restored.todo_list.active);
    // The tab bar marks the tab "Todos"; the overview still marks Home.
    assert_eq!(restored.active.as_deref(), Some("Home"));

    session.open_note(&id("A"));

    let tabs = session.snapshot().tabs;
    assert!(tabs.todo_list.open);
    assert!(!tabs.todo_list.active);
    assert_eq!(tabs.active.as_deref(), Some("A"));
}

// M-21.3

#[test]
fn uc007_br005_m21_3_notes_deleted_long_ago_go_when_the_workspace_opens() {
    let dir = temp_dir("purge");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let old = root.join(".underline").join("deleted").join("1000");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("Old.md"), "# Old\n").unwrap();
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);

    assert!(!old.exists());
}

#[test]
fn adr011_m21_3_a_note_created_outside_shows_in_the_overview() {
    let dir = temp_dir("watch-create");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.take_events();

    fs::write(root.join("Fresh.md"), "# Fresh\n").unwrap();

    assert!(
        tick_until(&mut session, |s| overview(s).contains(&"Fresh".to_owned())),
        "{:?}",
        overview(&session)
    );
}

#[test]
fn uc004_a8_m06_1_a_tab_whose_note_is_deleted_outside_stays_marked() {
    let dir = temp_dir("watch-delete");
    let root = workspace_with(&dir, "notes", &["Home", "A"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));

    fs::remove_file(root.join("A.md")).unwrap();

    assert!(
        tick_until(&mut session, |s| s.snapshot().tabs.tabs[1].label
            == "A (deleted)"),
        "{:?}",
        session.snapshot().tabs
    );
    assert_eq!(session.status(), "A was deleted outside");
    assert_eq!(overview(&session), ["Home"]);
}

// ADR-022: events

#[test]
fn adr022_opening_a_workspace_sends_it_its_overview_and_tabs_once() {
    let dir = temp_dir("events");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);
    let events = session.take_events();

    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Workspace(Some(w)) if w.name == "notes")));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Overview(items) if items.len() == 1)));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Tabs(tabs) if tabs.active.as_deref() == Some("Home"))));
    assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
    assert!(session.take_events().is_empty());
}

#[test]
fn adr022_activating_the_active_tab_again_sends_nothing() {
    let dir = temp_dir("events-quiet");
    let root = workspace_with(&dir, "notes", &["Home"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.take_events();

    session.activate_tab(&id("Home"));

    assert!(session.take_events().is_empty());
}

#[test]
fn q03_reading_the_user_file_again_sends_the_settings_with_a_new_generation() {
    let dir = temp_dir("events-width");
    let mut session = session_in(&dir, None);
    let start = Instant::now();
    fs::write(
        dir.join("home").join(SETTINGS_FILE_NAME),
        "overview_width: 200\n",
    )
    .unwrap();

    let events = session.tick(start + Duration::from_secs(1));

    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Settings(view) if view.width_generation == 1)),
        "{events:?}"
    );
}

// Review additions (F2)

#[test]
fn adr011_without_change_events_the_status_says_so() {
    let dir = temp_dir("polling");
    let mut session = session_in(&dir, None);

    session.note_polling(false);
    assert_eq!(session.status(), "");
    session.note_polling(true);

    assert_eq!(
        session.status(),
        "Changes from outside may show up later in the note overview; this system delivers no change events"
    );
}

#[cfg(unix)]
#[test]
fn uc002_a13_m21_1_a_folder_that_cannot_be_read_is_named() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("unreadable-folder");
    let root = workspace_with(&dir, "notes", &["Home", "locked/Secret"]);
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        // Running as root: every folder can be read.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let mut session = session_in(&dir, None);

    session.open_chosen(&root, false);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    let status = session.status();
    assert!(
        status.starts_with(&format!(
            "Opened {}; could not read ",
            absolute(&root).display()
        )),
        "{status}"
    );
    assert!(status.contains("locked"), "{status}");
    assert_eq!(overview(&session), ["Home"]);
}

#[cfg(unix)]
#[test]
fn uc001_a5_m21_2_a_workspace_that_cannot_be_created_says_why() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("create-fails");
    let parent = dir.join("read-only");
    fs::create_dir_all(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(parent.join("probe"), "").is_ok() {
        // Running as root: every folder can be written.
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let mut session = session_in(&dir, None);

    let answer = session.create_workspace(NewWorkspaceRequest {
        directory: parent.join("new").display().to_string(),
        format: FormatName::Markdown,
        git: false,
        remote: String::new(),
    });
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(answer, CreateAnswer::Done);
    assert!(
        session
            .status()
            .starts_with("The workspace could not be created: "),
        "{}",
        session.status()
    );
    assert!(!session.status().contains("could not remove"));
    assert_eq!(session.workspace_root(), None);
}

#[test]
fn uc001_a6_the_leftovers_are_named() {
    let err = underline_core::CreateError {
        problem: underline_core::CreateProblem::Io {
            action: "write the Home note",
            source: std::io::Error::other("the disk is full"),
        },
        leftovers: vec![PathBuf::from("/w/Home.md"), PathBuf::from("/w")],
    };

    assert_eq!(
        super::create_failure_status(&err),
        "The workspace could not be created: cannot write the Home note: the disk is full; could not remove: /w/Home.md, /w"
    );
}

#[test]
fn uc004_a7_a_note_changed_outside_is_said() {
    let dir = temp_dir("watch-change");
    let root = workspace_with(&dir, "notes", &["Home", "A"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));

    fs::write(root.join("A.md"), "# A\n\nNew text.\n").unwrap();

    assert!(
        tick_until(&mut session, |s| s.status() == "A was changed outside"),
        "{}",
        session.status()
    );
}

#[test]
fn m22_2_uc002_step5_each_workspace_gets_its_own_tabs_back() {
    let dir = temp_dir("switch-back");
    let first = workspace_with(&dir, "first", &["Home", "A"]);
    let second = workspace_with(&dir, "second", &["Home", "B"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&first, false);
    session.open_note(&id("A"));
    session.open_chosen(&second, false);
    session.open_note(&id("B"));
    session.activate_tab(&id("Home"));

    session.open_chosen(&first, false);
    assert_eq!(tabs(&session), ["Home", "A"]);
    assert_eq!(active(&session).as_deref(), Some("A"));

    session.open_chosen(&second, false);
    assert_eq!(tabs(&session), ["Home", "B"]);
    assert_eq!(active(&session).as_deref(), Some("Home"));
}

#[test]
fn adr022_m01_1_the_start_snapshot_is_not_sent_again_as_events() {
    let dir = temp_dir("start-told");
    let root = workspace_with(&dir, "notes", &["Home", "A"]);
    let mut session = session_in(&dir, Some(root));
    session.start_workspace();

    let events = session.take_events();

    // The properties of the tool panel (M-16.2), the deleted notes
    // (M-05.6) and what the settings window shows (M-20.2) are no part of
    // the snapshot; they follow as events.
    assert!(
        events.iter().all(|e| matches!(
            e,
            Event::Status(_) | Event::Properties(_) | Event::Deleted(_) | Event::SettingsWindow(_)
        )),
        "{events:?}"
    );
    session.open_note(&id("A"));
    assert!(session
        .take_events()
        .iter()
        .any(|e| matches!(e, Event::Tabs(t) if t.active.as_deref() == Some("A"))));
}
