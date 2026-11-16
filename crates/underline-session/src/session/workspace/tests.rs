//! Tests of the workspace part of the session (UC-001, UC-002, M-05, M-06,
//! M-21, M-22).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, PaneSide, StateStore, SETTINGS_FILE_NAME, STATE_WRITE_DELAY};

use crate::workspace_view::{
    CreateAnswer, EditingViewName, FormatName, NewWorkspaceRequest, OpenAnswer, OpenOffer, PaneName,
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
    // (M-05.6), what the settings window shows (M-20.2) and the agents
    // entry (M-04.4) are no part of the snapshot; they follow as events.
    assert!(
        events.iter().all(|e| matches!(
            e,
            Event::Status(_)
                | Event::Properties(_)
                | Event::Deleted(_)
                | Event::SettingsWindow(_)
                | Event::Agents(_)
        )),
        "{events:?}"
    );
    session.open_note(&id("A"));
    assert!(session
        .take_events()
        .iter()
        .any(|e| matches!(e, Event::Tabs(t) if t.active.as_deref() == Some("A"))));
}

// UC-004 A12, BR-014 (FR-076): go to note matches the title and the folder.

/// Removes the folder of a test when it ends, also when it fails.
struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn uc004_a12_tc_004_25_the_overview_names_title_and_folder_of_each_note() {
    let dir = temp_dir("goto-title-folder");
    let _cleanup = Cleanup(dir.clone());
    let root = workspace_with(&dir, "notes", &["Plan", "travel/Packing", "Über-uns"]);
    let mut session = session_in(&dir, Some(root));
    session.start_workspace();

    let items = session.snapshot().overview;

    let of = |note: &str| items.iter().find(|item| item.note == note).unwrap();
    assert_eq!(of("Plan").title, "Plan");
    assert_eq!(of("Plan").folder, None);
    assert_eq!(of("travel/Packing").title, "Packing");
    assert_eq!(of("travel/Packing").folder.as_deref(), Some("travel"));
    assert_eq!(of("Über-uns").title, "Über uns");
}

// FR-096, FR-097, UC-004 A18 to A20, BR-018 (package 7a)

fn right(session: &Session) -> Vec<String> {
    session
        .snapshot()
        .tabs
        .split
        .map(|split| split.right.into_iter().map(|tab| tab.note).collect())
        .unwrap_or_default()
}

/// TC-004-34, UC-004 A18, BR-018: opening to the side splits the middle;
/// the right pane is active, a chosen note opens there, and the tool panel's
/// note (the active note) follows the pane worked in.
#[test]
fn uc004_tc34_open_to_the_side_and_notes_follow_the_active_pane() {
    let dir = temp_dir("split-open");
    let root = workspace_with(&dir, "notes", &["Plan", "Ideas", "Trip", "Packing", "Log"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Plan"));
    session.open_note(&id("Ideas"));
    session.activate_tab(&id("Plan"));

    assert!(session.open_note_to_side(&id("Trip")));
    let split = session.snapshot().tabs.split.expect("the split view shows");
    assert_eq!(split.pane, PaneName::Right);
    assert_eq!(right(&session), ["Trip"]);
    assert_eq!(active(&session).as_deref(), Some("Trip"));
    assert!(tabs(&session).contains(&"Plan".to_owned()));

    session.open_note(&id("Packing"));
    assert_eq!(right(&session), ["Trip", "Packing"]);

    session.focus_pane(PaneSide::Left);
    assert_eq!(active(&session).as_deref(), Some("Plan"));
    session.open_note(&id("Log"));
    assert!(tabs(&session).contains(&"Log".to_owned()));
    assert_eq!(right(&session), ["Trip", "Packing"]);
}

/// TC-004-35, TC-005-41, UC-004 A19, BR-001, UC-005 BR-016: the active
/// note opened to the side shows the other view; both tabs share the note,
/// closing one keeps the other's text open.
#[test]
fn uc004_tc35_uc005_tc41_the_same_note_on_both_sides() {
    let dir = temp_dir("split-same");
    let root = workspace_with(&dir, "notes", &["Plan"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Plan"));

    assert!(session.open_note_to_side(&id("Plan")));
    let snapshot = session.snapshot();
    assert_eq!(snapshot.tabs.tabs[0].view, EditingViewName::Facade);
    let split = snapshot.tabs.split.expect("split");
    assert_eq!(split.right[0].view, EditingViewName::Source);
    // Choosing it again in the right pane opens no third tab.
    session.open_note(&id("Plan"));
    assert_eq!(right(&session), ["Plan"]);
    // Each tab keeps its own editing view.
    session
        .set_editing_view_in(PaneSide::Right, &id("Plan"), EditingViewName::Facade)
        .unwrap();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.tabs.tabs[0].view, EditingViewName::Facade);
    assert!(session
        .close_tab_in(PaneSide::Right, &id("Plan"), false)
        .is_none());
    assert!(session.snapshot().tabs.split.is_none());
    assert_eq!(tabs(&session), ["Plan"]);
    assert!(session.editor_mirror(&id("Plan")).is_ok());
}

/// TC-004-36, UC-004 A20: ending the split view joins the right tabs after
/// the left ones; closing the last tab of a pane ends it too.
#[test]
fn uc004_tc36_end_the_split_view() {
    let dir = temp_dir("split-end");
    let root = workspace_with(&dir, "notes", &["Plan", "Ideas", "Trip"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Plan"));
    session.open_note(&id("Ideas"));
    session.activate_tab(&id("Plan"));
    session.open_note_to_side(&id("Plan"));
    session.open_note(&id("Trip"));

    session.end_split();
    assert!(session.snapshot().tabs.split.is_none());
    assert_eq!(tabs(&session), ["Plan", "Ideas", "Trip"]);

    session.activate_tab(&id("Trip"));
    session.open_note_to_side(&id("Trip"));
    assert_eq!(right(&session), ["Trip"]);
    session.close_tab_in(PaneSide::Right, &id("Trip"), false);
    assert!(session.snapshot().tabs.split.is_none());
    assert_eq!(tabs(&session), ["Plan", "Ideas", "Trip"]);
}

/// M-22.2, UC-004 step 6, BR-018, OQ-220: the panes and their tabs come
/// back after a restart, the left pane active.
#[test]
fn uc004_br018_the_panes_come_back_with_the_left_one_active() {
    let dir = temp_dir("split-restore");
    let root = workspace_with(&dir, "notes", &["Plan", "Trip"]);
    {
        let mut session = session_in(&dir, None);
        session.open_chosen(&root, false);
        session.open_note(&id("Plan"));
        session.open_note_to_side(&id("Trip"));
        session.write_state_now().unwrap();
    }
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    let snapshot = session.snapshot();
    let split = snapshot.tabs.split.expect("the split view comes back");
    assert_eq!(split.pane, PaneName::Left);
    assert_eq!(right(&session), ["Trip"]);
    assert_eq!(snapshot.tabs.active, split.left_active);
}

// Monkey tests of the panes (package 7a)

/// A small deterministic generator; every failure prints its seed.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

/// What every state of the panes must hold (UC-004 BR-001, BR-008, BR-018,
/// A20): no note twice in a pane, the tabs of the panes are the tabs of the
/// session, within the limit, a split view has two panes with tabs, and the
/// active note is a note of the active pane.
fn check_panes(session: &Session, context: &str) {
    let open = session.open.as_ref().unwrap();
    let mut shown: Vec<NoteId> = Vec::new();
    for (side, pane) in open.panes.sides() {
        let mut notes: Vec<&NoteId> = pane.tabs().iter().map(|tab| &tab.note).collect();
        notes.sort();
        let before = notes.len();
        notes.dedup();
        assert_eq!(before, notes.len(), "{context}: a note twice in {side:?}");
        for tab in pane.tabs() {
            if !shown.contains(&tab.note) {
                shown.push(tab.note.clone());
            }
        }
        if open.panes.is_split() {
            assert!(
                !pane.is_empty(),
                "{context}: an empty pane {side:?} is shown"
            );
        }
        assert_eq!(
            pane.active().is_some(),
            !pane.is_empty(),
            "{context}: {side:?} has tabs and no active one, or the reverse"
        );
    }
    assert!(
        open.panes.len() <= underline_core::MAX_TABS,
        "{context}: {} tabs",
        open.panes.len()
    );
    let mut held: Vec<NoteId> = open.tabs.iter().map(|(note, _)| note.clone()).collect();
    held.sort();
    shown.sort();
    assert_eq!(
        held, shown,
        "{context}: the session's tabs are not the panes' notes"
    );
    if open.panes.active_side() == PaneSide::Right {
        assert!(
            open.panes.is_split(),
            "{context}: right pane active, not shown"
        );
    }
    if let Some(note) = open.panes.active_note() {
        assert_eq!(
            open.tabs
                .find(note)
                .and_then(|i| (open.tabs.active_index() == Some(i)).then_some(i)),
            open.tabs.active_index(),
            "{context}: the active tab is not the active pane's"
        );
    }
}

/// UC-004 A18 to A20, BR-001, BR-008, BR-018 (property, seed printed): random
/// opens (also to the side), closes in either pane, focus changes, ending
/// the split view and view switches keep the panes and the session's tabs
/// in step; what is remembered comes back as it was.
#[test]
fn uc004_br018_random_pane_operations_keep_the_panes_consistent() {
    let names: Vec<String> = (0..12).map(|i| format!("N{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    for seed in 0..40u64 {
        let dir = temp_dir(&format!("panes-fuzz-{seed}"));
        let root = workspace_with(&dir, "notes", &refs);
        let mut session = session_in(&dir, None);
        session.open_chosen(&root, false);
        let mut rng = Lcg(seed * 31 + 7);
        let mut log: Vec<String> = Vec::new();
        for step in 0..60 {
            let note = id(&names[rng.below(names.len())]);
            let side = if rng.below(2) == 0 {
                PaneSide::Left
            } else {
                PaneSide::Right
            };
            let what = match rng.below(9) {
                0 | 1 => {
                    session.open_note(&note);
                    format!("open {note}")
                }
                2 | 3 => {
                    // With no tab open there is no left pane to keep (see
                    // `uc004_a18_opening_to_the_side_without_a_tab_...`).
                    if session.open.as_ref().unwrap().panes.is_empty() {
                        continue;
                    }
                    session.open_note_to_side(&note);
                    format!("side {note}")
                }
                4 => {
                    session.close_tab_in(side, &note, false);
                    format!("close {side:?} {note}")
                }
                5 => {
                    session.close_tab(&note, false);
                    format!("close_tab {note}")
                }
                6 => {
                    session.focus_pane(side);
                    format!("focus {side:?}")
                }
                7 => {
                    session.end_split();
                    "end split".to_owned()
                }
                _ => {
                    let view = if rng.below(2) == 0 {
                        EditingViewName::Source
                    } else {
                        EditingViewName::Facade
                    };
                    let _ = session.set_editing_view_in(side, &note, view);
                    format!("view {side:?} {note} {view:?}")
                }
            };
            log.push(what);
            check_panes(&session, &format!("seed {seed} step {step}: {log:#?}"));
        }
        // What is remembered comes back as it was: the same tabs in the same
        // panes, the left pane active.
        let before: Vec<(PaneSide, String, EditingViewName)> = {
            let open = session.open.as_ref().unwrap();
            open.panes
                .tabs()
                .map(|(side, tab)| (side, tab.note.to_string(), tab.view.into()))
                .collect()
        };
        session.write_state_now().unwrap();
        drop(session);
        let mut again = session_in(&dir, None);
        again.open_chosen(&root, false);
        check_panes(&again, &format!("seed {seed} restored: {log:#?}"));
        let after: Vec<(PaneSide, String, EditingViewName)> = {
            let open = again.open.as_ref().unwrap();
            open.panes
                .tabs()
                .map(|(side, tab)| (side, tab.note.to_string(), tab.view.into()))
                .collect()
        };
        assert_eq!(before, after, "seed {seed}: {log:#?}");
        let _ = fs::remove_dir_all(&dir);
    }
}

/// UC-004 BR-008, A6, BR-018 (property, seed printed): the limit of 100 tabs
/// counts both panes together; opening past it, in either pane and to the
/// side, closes the tab used longest ago and keeps the panes consistent.
#[test]
fn uc004_br008_br018_the_limit_of_tabs_holds_across_both_panes() {
    let names: Vec<String> = (0..130).map(|i| format!("N{i:03}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    for seed in 0..6u64 {
        let dir = temp_dir(&format!("panes-limit-{seed}"));
        let root = workspace_with(&dir, "notes", &refs);
        let mut session = session_in(&dir, None);
        session.open_chosen(&root, false);
        session.open_note(&id(&names[0]));
        let mut rng = Lcg(seed * 17 + 3);
        let mut log: Vec<String> = Vec::new();
        let mut at_limit = 0;
        for step in 0..450 {
            let note = id(&names[rng.below(names.len())]);
            let what = match rng.below(10) {
                0..=4 => {
                    session.open_note(&note);
                    format!("open {note}")
                }
                5..=7 => {
                    session.open_note_to_side(&note);
                    format!("side {note}")
                }
                8 => {
                    let side = if rng.below(2) == 0 {
                        PaneSide::Left
                    } else {
                        PaneSide::Right
                    };
                    session.focus_pane(side);
                    format!("focus {side:?}")
                }
                _ => {
                    session.close_tab(&note, false);
                    format!("close {note}")
                }
            };
            log.push(what);
            if session.open.as_ref().unwrap().panes.len() >= underline_core::MAX_TABS {
                at_limit += 1;
            }
            let last = log.last().cloned().unwrap_or_default();
            let recent = &log[log.len().saturating_sub(8)..];
            check_panes(
                &session,
                &format!("seed {seed} step {step} ({last}), last steps {recent:#?}"),
            );
        }
        assert!(
            at_limit > 20,
            "seed {seed}: the run reached the limit {at_limit} times"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

/// UC-004 A18, A20, BR-018: the panes of the split view never show a pane
/// without tabs. Opening to the side with no tab open (all were closed)
/// divides the middle into an empty left pane and the new tab on the right,
/// which no way of the use case leads to otherwise: A20 ends the split view
/// when a pane has no tab.
#[test]
fn uc004_a18_a20_opening_to_the_side_without_a_tab_shows_no_empty_pane() {
    let dir = temp_dir("split-from-nothing");
    let root = workspace_with(&dir, "notes", &["Plan", "Ideas"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Plan"));
    session.close_tab(&id("Plan"), false);
    assert!(tabs(&session).is_empty(), "{:?}", tabs(&session));
    assert!(session.open_note_to_side(&id("Ideas")));
    let snapshot = session.snapshot();
    let split_with_empty_left = snapshot
        .tabs
        .split
        .as_ref()
        .is_some_and(|_| snapshot.tabs.tabs.is_empty());
    assert!(
        !split_with_empty_left,
        "a split view whose left pane has no tab"
    );
}

/// Writes `yaml` as the workspace state of `root` and opens the workspace in a
/// new session.
fn open_with_state(dir: &Path, root: &Path, yaml: &str) -> Session {
    let folder = root.join(".underline");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(underline_core::STATE_FILE_NAME), yaml).unwrap();
    let mut session = session_in(dir, None);
    session.open_chosen(root, false);
    session
}

/// UC-004 step 6, BR-018, BR-001, BR-008 (hostile state files): remembered
/// tabs with a note twice in a pane, in both panes, a note that is gone, only
/// a right pane, and far more than 100 tabs, give panes that hold their
/// invariants and the tabs of the session.
#[test]
fn uc004_br018_state_files_with_odd_panes_open_as_consistent_panes() {
    let dir = temp_dir("panes-state-odd");
    let root = workspace_with(&dir, "notes", &["A", "B", "C"]);
    let tab = |note: &str, pane: &str, position: u32, active: bool| {
        format!(
            "- note: {note}\n  pane: {pane}\n  position: {position}\n  editing_view: source\n  active: {active}\n"
        )
    };
    let cases: Vec<(&str, String)> = vec![
        (
            "a note twice in a pane",
            format!("tabs:\n{}{}{}", tab("A", "left", 1, true), tab("A", "left", 2, false), tab("B", "left", 3, false)),
        ),
        (
            "a note in both panes",
            format!("tabs:\n{}{}", tab("A", "left", 1, true), tab("A", "right", 1, true)),
        ),
        (
            "a missing note",
            format!("tabs:\n{}{}{}", tab("A", "left", 1, true), tab("Gone", "right", 1, true), tab("B", "right", 2, false)),
        ),
        (
            "only a right pane",
            format!("tabs:\n{}{}", tab("A", "right", 1, true), tab("B", "right", 2, false)),
        ),
        (
            "only a missing note on the right",
            format!("tabs:\n{}{}", tab("A", "left", 1, true), tab("Gone", "right", 1, true)),
        ),
        (
            "every tab active",
            format!("tabs:\n{}{}{}", tab("A", "left", 1, true), tab("B", "left", 2, true), tab("C", "right", 1, true)),
        ),
        (
            "no tab active",
            format!("tabs:\n{}{}", tab("A", "left", 1, false), tab("B", "right", 1, false)),
        ),
        (
            "same positions",
            format!("tabs:\n{}{}{}", tab("A", "left", 1, true), tab("B", "left", 1, false), tab("C", "left", 1, false)),
        ),
        ("a state file without the pane key", "tabs:\n- note: A\n  position: 1\n  editing_view: facade\n  active: true\n- note: B\n  position: 2\n  editing_view: source\n  active: false\n".to_owned()),
    ];
    for (name, yaml) in cases {
        let session = open_with_state(&dir, &root, &yaml);
        check_panes(&session, name);
        // What the session writes next reads back as the same panes.
        let mut session = session;
        session.write_state_now().unwrap();
        let shown: Vec<(PaneSide, String)> = session
            .open
            .as_ref()
            .unwrap()
            .panes
            .tabs()
            .map(|(side, tab)| (side, tab.note.to_string()))
            .collect();
        drop(session);
        let again = open_with_state(
            &dir,
            &root,
            &fs::read_to_string(
                root.join(".underline")
                    .join(underline_core::STATE_FILE_NAME),
            )
            .unwrap(),
        );
        let reread: Vec<(PaneSide, String)> = again
            .open
            .as_ref()
            .unwrap()
            .panes
            .tabs()
            .map(|(side, tab)| (side, tab.note.to_string()))
            .collect();
        assert_eq!(shown, reread, "{name}");
    }
    // More than 100 tabs, half of them in the right pane.
    let many: Vec<String> = (0..160).map(|i| format!("M{i:03}")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    let big = workspace_with(&dir, "big", &refs);
    let yaml: String = std::iter::once("tabs:\n".to_owned())
        .chain(many.iter().enumerate().map(|(i, note)| {
            tab(
                note,
                if i % 2 == 0 { "left" } else { "right" },
                u32::try_from(i / 2 + 1).unwrap(),
                i < 2,
            )
        }))
        .collect();
    let session = open_with_state(&dir, &big, &yaml);
    check_panes(&session, "160 remembered tabs");
}

/// UC-004 step 6, BR-018: a state file written without the split view has
/// no `pane` key, and with it only the right pane's tabs name their pane,
/// so a workspace used without the split view keeps its state file as it
/// was.
#[test]
fn uc004_br018_only_right_tabs_name_their_pane_in_the_state_file() {
    let dir = temp_dir("panes-state-write");
    let root = workspace_with(&dir, "notes", &["A", "B", "C"]);
    let state_file = root
        .join(".underline")
        .join(underline_core::STATE_FILE_NAME);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("A"));
    session.open_note(&id("B"));
    session.write_state_now().unwrap();
    let plain = fs::read_to_string(&state_file).unwrap();
    assert!(!plain.contains("pane"), "{plain}");
    session.open_note_to_side(&id("C"));
    session.write_state_now().unwrap();
    let split = fs::read_to_string(&state_file).unwrap();
    assert_eq!(split.matches("pane: right").count(), 1, "{split}");
    assert_eq!(split.matches("pane:").count(), 1, "{split}");
    session.end_split();
    session.write_state_now().unwrap();
    let ended = fs::read_to_string(&state_file).unwrap();
    assert!(!ended.contains("pane"), "{ended}");
    // The tabs of the left pane read as they did.
    let tabs_of = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| line.trim_start().starts_with("- note:"))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(tabs_of(&ended)[..2], tabs_of(&plain)[..2]);
}

/// UC-004 step 6, BR-018: a pane value that is not known (a hand-written
/// or a newer file) does not take the whole state of the workspace with it
/// (its other tabs, its notes' places).
#[test]
fn uc004_br018_an_unknown_pane_value_does_not_drop_the_other_tabs() {
    let dir = temp_dir("panes-state-unknown");
    let root = workspace_with(&dir, "notes", &["A", "B"]);
    let yaml = "tabs:\n- note: A\n  position: 1\n  editing_view: source\n  active: true\n- note: B\n  pane: top\n  position: 1\n  editing_view: source\n  active: false\n";
    let session = open_with_state(&dir, &root, yaml);
    let notes = tabs(&session);
    assert!(notes.contains(&"A".to_owned()), "{notes:?}");
}

/// UC-006 step 7, UC-004 BR-018 (review): a note open on both sides, renamed,
/// keeps its tabs at their places in both panes.
#[test]
fn uc006_a_renamed_note_keeps_its_places_in_both_panes() {
    let dir = temp_dir("split-rename");
    let root = workspace_with(&dir, "notes", &["Plan", "Ideas", "Trip"]);
    let mut session = session_in(&dir, None);
    session.open_chosen(&root, false);
    session.open_note(&id("Ideas"));
    session.open_note(&id("Plan"));
    session.open_note_to_side(&id("Trip"));
    session.focus_pane(PaneSide::Left);
    session.open_note_to_side(&id("Plan"));
    let before = (tabs(&session), right(&session));
    assert_eq!(before.1, ["Trip", "Plan"]);

    session.rename_note(&id("Plan"), "Goals", "");

    let after = (tabs(&session), right(&session));
    let renamed = |list: Vec<String>| -> Vec<String> {
        list.into_iter()
            .map(|n| if n == "Plan" { "Goals".to_owned() } else { n })
            .collect()
    };
    assert_eq!(after.0, renamed(before.0));
    assert_eq!(after.1, renamed(before.1));
}
