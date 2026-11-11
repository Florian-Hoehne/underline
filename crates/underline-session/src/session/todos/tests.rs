//! Tests of the todos (UC-020, UC-021, inventory M-04.5, M-04.6, M-06.5,
//! M-16.3, M-18), against what the egui app does
//! (`apps/underline-desktop/src/todos_panel.rs` and `app.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{load_todos, Day, NoteId, StateStore, SETTINGS_FILE_NAME};

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::disk::Disk;
use super::{calendar_view, PollAsk, Poller, TodoJob, TODOS_EVERY};
use crate::session::Wake;
use crate::todos_view::{
    TodoAnswer, TodoChange, TodoFilterName, TodoSortName, TodosView, UrgencyName,
};
use crate::{Event, Session, SettingsState, StatusOffer};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-todos-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files`, the
/// first open in the active tab, and its todos read once.
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
    settle(&mut session);
    session.take_events();
    (session, root)
}

/// Ticks until the look of the second answered, as the app's tick does a
/// second apart.
fn settle(session: &mut Session) {
    session.todos.checked = None;
    session.tick(Instant::now());
    for _ in 0..500 {
        if !session.todos.poller.as_ref().is_some_and(|p| p.waiting) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
        session.tick(Instant::now());
    }
    panic!("the todos were not looked at");
}

/// Runs `job` as the command does: without the session, then taken in.
fn run(session: &mut Session, job: Option<TodoJob>) -> TodoAnswer {
    match job {
        Some(job) => session.todo_finish(job.run()),
        None => TodoAnswer::Done,
    }
}

fn create(session: &mut Session, title: &str) -> TodoAnswer {
    match session.todo_create_start(title) {
        Ok(job) => run(session, Some(job)),
        Err(answer) => answer,
    }
}

fn change(session: &mut Session, number: u32, change: TodoChange) -> TodoAnswer {
    match session.todo_change_start(number, change) {
        Ok(job) => run(session, Some(job)),
        Err(answer) => answer,
    }
}

/// The todos the window was told last, after a tick.
fn view(session: &mut Session) -> TodosView {
    session.tick(Instant::now());
    session.todos_view().expect("the todos were read")
}

fn todos_event(events: &[Event]) -> Option<&TodosView> {
    events.iter().find_map(|event| match event {
        Event::Todos(view) => Some(&**view),
        _ => None,
    })
}

fn status_offer(events: &[Event]) -> Option<StatusOffer> {
    events.iter().find_map(|event| match event {
        Event::Status(status) => status.offer,
        _ => None,
    })
}

fn day(text: &str) -> TodoChange {
    TodoChange::Due { day: text.into() }
}

#[test]
fn uc020_m18_1_a_new_todo_is_created_stored_and_told() {
    let dir = temp_dir("create");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    assert!(session.todos_read());

    assert_eq!(create(&mut session, "  Buy   milk "), TodoAnswer::Done);

    assert_eq!(session.status(), "Created todo #1 Buy milk");
    assert!(root.join(".todos/1.yaml").exists());
    let events = session.tick(Instant::now());
    let todos = todos_event(&events).expect("the todos are told");
    assert_eq!(todos.heading, "Todos (1)");
    assert_eq!(todos.open, [1]);
    assert_eq!(todos.todos[0].name, "#1 Buy milk");
    assert_eq!(todos.todos[0].urgency, UrgencyName::Normal);
    assert!(todos.todos[0].stamp.starts_with("created "));
    assert_eq!(todos.todos[0].stamp.len(), "created YYYY-MM-DD HH:MM".len());
    // Told once.
    assert!(todos_event(&session.tick(Instant::now())).is_none());
}

#[test]
fn uc020_a3_m18_1_an_empty_or_long_title_is_a_problem_for_the_field() {
    let dir = temp_dir("no-title");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    let before = session.status().to_owned();

    assert_eq!(
        create(&mut session, "   "),
        TodoAnswer::Problem {
            problem: "a todo needs a title".into()
        }
    );
    assert_eq!(
        create(&mut session, &"x".repeat(201)),
        TodoAnswer::Problem {
            problem: "the title of a todo has at most 200 characters".into()
        }
    );
    assert!(!root.join(".todos").exists());
    assert_eq!(session.status(), before);
}

#[test]
fn uc020_m18_3_fields_are_stored_and_wrong_ones_said() {
    let dir = temp_dir("fields");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Ship");

    assert_eq!(change(&mut session, 1, day("2030-05-02")), TodoAnswer::Done);
    assert_eq!(
        change(
            &mut session,
            1,
            TodoChange::Scheduled {
                day: "2030-05-09".into()
            }
        ),
        TodoAnswer::Done
    );
    assert_eq!(
        change(
            &mut session,
            1,
            TodoChange::Urgency {
                urgency: UrgencyName::High
            }
        ),
        TodoAnswer::Done
    );
    assert_eq!(
        change(&mut session, 1, day("2030-13-40")),
        TodoAnswer::Problem {
            problem: "`2030-13-40` is not a day; write it as YYYY-MM-DD".into()
        }
    );
    assert_eq!(
        change(&mut session, 1, TodoChange::Title { title: " ".into() }),
        TodoAnswer::Problem {
            problem: "a todo needs a title".into()
        }
    );

    let stored = load_todos(&root);
    let todo = stored.get(1).unwrap();
    assert_eq!(todo.due, Some("2030-05-02".parse::<Day>().unwrap()));
    assert_eq!(todo.title, "Ship");
    let shown = &view(&mut session).todos[0];
    assert_eq!(shown.due.as_deref(), Some("2030-05-02"));
    assert_eq!(shown.urgency_label, "High");
    assert!(shown.scheduled_after_due);
    // Empty takes the day away.
    change(&mut session, 1, day(""));
    assert_eq!(load_todos(&root).get(1).unwrap().due, None);
}

#[test]
fn uc020_m18_2_done_and_open_again_from_the_section() {
    let dir = temp_dir("done");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "A");
    create(&mut session, "B");

    change(&mut session, 1, TodoChange::Done { done: true });

    let todos = view(&mut session);
    assert_eq!(todos.heading, "Todos (1)");
    assert_eq!(todos.open, [2]);
    assert_eq!(todos.done, [1]);
    let done = &todos.todos[0];
    assert!(done.done);
    assert!(done.stamp.starts_with("done "));
    assert!(done.done_at.is_some());
    // No undo from the section, as in egui.
    assert_eq!(session.status(), "Created todo #2 B");
    change(&mut session, 1, TodoChange::Done { done: false });
    assert!(!load_todos(&root).get(1).unwrap().is_done());
}

#[test]
fn uc020_m18_1_open_todos_by_due_day_those_without_last() {
    let dir = temp_dir("order");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    for title in ["none", "late", "early"] {
        create(&mut session, title);
    }
    change(&mut session, 2, day("2031-02-01"));
    change(&mut session, 3, day("2031-01-01"));

    assert_eq!(view(&mut session).open, [3, 2, 1]);
}

#[test]
fn uc020_m18_4_links_the_active_note_and_says_a_missing_one() {
    let dir = temp_dir("links");
    let (mut session, _) = session_with(
        &dir,
        &[("Home.md", "# Home\n"), ("Projects/Plan.md", "# Plan\n")],
    );
    create(&mut session, "Plan it");

    let todos = view(&mut session);
    let active = todos.active_note.expect("Home is active");
    assert_eq!(active.link_label, "Link Home");
    change(
        &mut session,
        1,
        TodoChange::Link {
            note: active.note.clone(),
        },
    );
    // Review F15: only a note of the workspace can be linked.
    assert_eq!(
        change(
            &mut session,
            1,
            TodoChange::Link {
                note: "Projects/Gone".into(),
            },
        ),
        TodoAnswer::Problem {
            problem: "the note Projects/Gone is not in this workspace".into()
        }
    );
    // A note linked before, deleted since (UC-020 A6), as a sync leaves it.
    let root = dir.join("notes");
    let mut file = fs::read_to_string(root.join(".todos/1.yaml")).unwrap();
    file.push_str("- Projects/Gone\n");
    fs::write(root.join(".todos/1.yaml"), file).unwrap();
    settle(&mut session);

    let todo = view(&mut session).todos[0].clone();
    assert!(todo.links_active);
    assert_eq!(todo.links.len(), 2);
    assert!(todo.links[0].exists);
    assert_eq!(todo.links[1].title, "Gone");
    assert!(!todo.links[1].exists);
    session.todo_open_link("Projects/Gone");
    assert_eq!(
        session.status(),
        "The note Projects/Gone no longer exists; remove the link with ×"
    );
    change(
        &mut session,
        1,
        TodoChange::Unlink {
            link: "projects/gone".into(),
        },
    );
    assert_eq!(view(&mut session).todos[0].links.len(), 1);
    // A link that exists opens its note.
    change(
        &mut session,
        1,
        TodoChange::Link {
            note: "Projects/Plan".into(),
        },
    );
    session.todo_open_link("projects/plan");
    assert_eq!(
        session.snapshot().tabs.active.as_deref(),
        Some("Projects/Plan")
    );
}

#[test]
fn uc020_a1_m18_4_link_a_note_offers_up_to_eight() {
    let dir = temp_dir("candidates");
    let mut files: Vec<(String, String)> = (0..10)
        .map(|i| (format!("Road-{i}.md"), format!("# Road {i}\n")))
        .collect();
    files.push(("Home.md".into(), "# Home\n".into()));
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let (mut session, _) = session_with(&dir, &files);
    create(&mut session, "Walk");
    change(
        &mut session,
        1,
        TodoChange::Link {
            note: "Road-0".into(),
        },
    );

    let found = session.todo_candidates(1, " ROAD ");
    assert_eq!(found.notes.len(), 8);
    assert!(found.notes.iter().all(|n| n.note != "Road-0"));
    assert!(found.notes[0].label.starts_with("Road "));
    assert_eq!(found.none, None);
    let none = session.todo_candidates(1, "zebra");
    assert!(none.notes.is_empty());
    assert_eq!(none.none.as_deref(), Some("No note matches"));
    assert_eq!(session.todo_candidates(1, "  ").none, None);
}

#[test]
fn uc020_a5_m18_5_m04_6_delete_and_undo() {
    let dir = temp_dir("delete");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Old");
    change(&mut session, 1, day("2030-01-01"));
    session.tick(Instant::now());

    let job = session.todo_delete_start(1);
    assert_eq!(run(&mut session, job), TodoAnswer::Done);

    assert_eq!(session.status(), "Deleted todo #1 Old");
    assert!(!root.join(".todos/1.yaml").exists());
    let events = session.tick(Instant::now());
    assert_eq!(status_offer(&events), Some(StatusOffer::UndoTodoDelete));
    assert!(todos_event(&events).unwrap().todos.is_empty());

    let job = session.todo_undo_start();
    assert_eq!(run(&mut session, job), TodoAnswer::Done);
    assert_eq!(session.status(), "Todo #1 Old is back");
    let back = load_todos(&root);
    assert_eq!(back.get(1).unwrap().due, "2030-01-01".parse().ok());
    // The offer went with its report.
    assert!(session.todo_undo_start().is_none());
}

#[test]
fn uc020_a5_m04_6_another_report_ends_the_undo() {
    let dir = temp_dir("undo-ends");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Old");
    let job = session.todo_delete_start(1);
    run(&mut session, job);

    create(&mut session, "New");

    let events = session.tick(Instant::now());
    assert_eq!(status_offer(&events), None);
    assert!(session.todo_undo_start().is_none());
}

#[test]
fn uc021_m18_7_marked_in_the_list_with_undo() {
    let dir = temp_dir("mark");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Call");

    let job = session.todo_mark_start(1, true);
    assert_eq!(run(&mut session, job), TodoAnswer::Done);

    assert_eq!(session.status(), "Marked todo #1 Call done");
    assert!(load_todos(&root).get(1).unwrap().is_done());
    assert_eq!(
        status_offer(&session.tick(Instant::now())),
        Some(StatusOffer::UndoTodoMark)
    );
    // D-04 (fixed): the report keeps its undo while a note is saved.
    assert!(session.status_kept());

    let job = session.todo_undo_start();
    assert_eq!(run(&mut session, job), TodoAnswer::Done);
    assert_eq!(session.status(), "Todo #1 Call is as before");
    assert!(!load_todos(&root).get(1).unwrap().is_done());
    // Already open: nothing to mark.
    assert!(session.todo_mark_start(1, false).is_none());
}

#[cfg(not(unix))]
fn writable(_folder: &Path, _writable: bool) {}

/// Without Unix permissions the todo folder cannot be made unwritable.
#[cfg(not(unix))]
fn unwritable(_root: &Path) -> bool {
    eprintln!("skipped: the todo folder cannot be made unwritable on this system");
    false
}

#[cfg(unix)]
fn writable(folder: &Path, writable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if writable { 0o755 } else { 0o555 };
    fs::set_permissions(folder, fs::Permissions::from_mode(mode)).unwrap();
}

/// Makes the todo files of `root` unwritable; `false` when this user can
/// write all the same (root), and the test cannot run.
#[cfg(unix)]
fn unwritable(root: &Path) -> bool {
    let folder = root.join(".todos");
    writable(&folder, false);
    let probe = folder.join("probe");
    if fs::write(&probe, "").is_ok() {
        let _ = fs::remove_file(probe);
        writable(&folder, true);
        eprintln!("skipped: this user (root) can write into a read-only folder");
        return false;
    }
    true
}

fn urgency(urgency: UrgencyName) -> TodoChange {
    TodoChange::Urgency { urgency }
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn uc020_a9_m18_2_a_change_not_stored_is_kept_and_tried_again() {
    let dir = temp_dir("unstored");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Keep");
    if !unwritable(&root) {
        return;
    }

    assert_eq!(
        change(&mut session, 1, urgency(UrgencyName::Critical)),
        TodoAnswer::Done
    );

    assert!(
        session
            .status()
            .starts_with("Todo #1 could not be stored: the todos cannot be stored: "),
        "{}",
        session.status()
    );
    let todos = view(&mut session);
    let todo = &todos.todos[0];
    assert_eq!(todo.urgency, UrgencyName::Critical);
    assert!(todo.unstored.is_some());

    // Writable again: the look of the second stores it.
    writable(&root.join(".todos"), true);
    settle(&mut session);
    settle(&mut session);
    let todos = view(&mut session);
    assert_eq!(todos.todos[0].unstored, None);
    assert_eq!(
        load_todos(&root).get(1).unwrap().urgency,
        underline_core::Urgency::Critical
    );
}

#[test]
fn review_f15_a_change_is_not_written_over_a_todo_changed_elsewhere() {
    let dir = temp_dir("changed-elsewhere");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Mine");
    // An agent changes the todo after the window read it.
    let file = root.join(".todos/1.yaml");
    let theirs = fs::read_to_string(&file)
        .unwrap()
        .replace("title: Mine", "title: Theirs");
    fs::write(&file, &theirs).unwrap();

    assert_eq!(
        change(&mut session, 1, urgency(UrgencyName::High)),
        TodoAnswer::Done
    );

    // Neither version is lost: theirs in the file, mine shown, not stored.
    assert_eq!(fs::read_to_string(&file).unwrap(), theirs);
    let reason = super::disk::changed_elsewhere(1);
    assert_eq!(
        session.status(),
        format!("Todo #1 could not be stored: {reason}")
    );
    let shown = view(&mut session).todos[0].clone();
    assert_eq!(shown.title, "Mine");
    assert_eq!(shown.urgency, UrgencyName::High);
    assert_eq!(shown.unstored.as_deref(), Some(reason.as_str()));
    // The look of the second does not write it either.
    settle(&mut session);
    settle(&mut session);
    assert_eq!(fs::read_to_string(&file).unwrap(), theirs);
    // Changed again by the user, it is stored over theirs, as the reason says.
    change(&mut session, 1, urgency(UrgencyName::Low));
    let stored = load_todos(&root);
    assert_eq!(stored.get(1).unwrap().title, "Mine");
    assert_eq!(view(&mut session).todos[0].unstored, None);
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn review_f15_the_retry_never_writes_over_an_unreadable_or_deleted_file() {
    let dir = temp_dir("retry-safe");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "One");
    create(&mut session, "Two");
    if !unwritable(&root) {
        return;
    }
    change(&mut session, 1, urgency(UrgencyName::High));
    change(&mut session, 2, urgency(UrgencyName::High));
    writable(&root.join(".todos"), true);
    // Meanwhile #1 became unreadable, and an agent deleted #2.
    fs::write(root.join(".todos/1.yaml"), "title: [unclosed").unwrap();
    fs::remove_file(root.join(".todos/2.yaml")).unwrap();

    settle(&mut session);
    settle(&mut session);

    assert_eq!(
        fs::read_to_string(root.join(".todos/1.yaml")).unwrap(),
        "title: [unclosed"
    );
    assert!(!root.join(".todos/2.yaml").exists(), "not brought back");
    let todos = view(&mut session);
    let why = |n: u32| {
        todos
            .todos
            .iter()
            .find(|t| t.number == n)
            .and_then(|t| t.unstored.clone())
            .unwrap_or_default()
    };
    assert!(why(1).starts_with("todo #1 cannot be read: "), "{}", why(1));
    assert_eq!(why(2), super::disk::deleted_elsewhere(2));

    // A todo that is only kept here, without a file, can be deleted.
    let job = session.todo_delete_start(2);
    assert!(job.is_none());
    assert_eq!(session.status(), "Deleted todo #2 Two");
    assert!(view(&mut session).todos.iter().all(|t| t.number != 2));
}

#[test]
fn review_f15_undo_never_writes_over_a_todo_made_or_changed_since() {
    let dir = temp_dir("undo-safe");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Old");
    let job = session.todo_delete_start(1);
    run(&mut session, job);
    // Another computer made a todo #1 meanwhile.
    fs::write(root.join(".todos/1.yaml"), "title: New\n").unwrap();

    let job = session.todo_undo_start();
    assert_eq!(run(&mut session, job), TodoAnswer::Failed);
    assert_eq!(
        session.status(),
        "Todo #1 Old could not be brought back: there is a todo #1 again"
    );
    assert_eq!(
        fs::read_to_string(root.join(".todos/1.yaml")).unwrap(),
        "title: New\n"
    );

    // Marked, then changed by an agent: the undo leaves the agent's change.
    create(&mut session, "Call");
    let job = session.todo_mark_start(2, true);
    run(&mut session, job);
    let file = root.join(".todos/2.yaml");
    let theirs = fs::read_to_string(&file)
        .unwrap()
        .replace("title: Call", "title: Call back");
    fs::write(&file, &theirs).unwrap();
    let job = session.todo_undo_start();
    assert_eq!(run(&mut session, job), TodoAnswer::Failed);
    assert_eq!(
        session.status(),
        format!(
            "Todo #2 could not be stored: {}",
            super::disk::changed_elsewhere(2)
        )
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), theirs);
}

#[test]
fn review_f15_a_refused_change_with_a_newer_one_pending_keeps_the_newer() {
    let dir = temp_dir("two-pending");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Both");
    let first = session.todo_change_start(1, day("2030-03-03")).unwrap();
    let second = session
        .todo_change_start(1, urgency(UrgencyName::Low))
        .unwrap();
    let file = root.join(".todos/1.yaml");
    let theirs = fs::read_to_string(&file)
        .unwrap()
        .replace("title: Both", "title: Theirs");
    fs::write(&file, &theirs).unwrap();

    // The first is refused while the second is still being written: it is
    // not kept on its own, the second (which holds it) is.
    let done = first.run();
    session.todo_finish(done);
    let shown = view(&mut session).todos[0].clone();
    assert_eq!(shown.unstored, None);
    assert_eq!(shown.urgency, UrgencyName::Low);
    let done = second.run();
    session.todo_finish(done);

    let shown = view(&mut session).todos[0].clone();
    assert_eq!(shown.due.as_deref(), Some("2030-03-03"));
    assert_eq!(shown.urgency, UrgencyName::Low);
    assert!(shown.unstored.is_some());
    assert_eq!(fs::read_to_string(&file).unwrap(), theirs);
}

fn close_question(events: &[Event]) -> Option<&crate::CloseQuestion> {
    events.iter().find_map(|event| match event {
        Event::CloseQuestion(question) => Some(question),
        _ => None,
    })
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn review_f15_a_todo_not_stored_is_asked_about_before_another_workspace_opens() {
    let dir = temp_dir("settle");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Last");
    if !unwritable(&root) {
        return;
    }
    change(&mut session, 1, urgency(UrgencyName::High));
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Other.md"), "# Other\n").unwrap();

    session.open_chosen(&other, false);

    // Not opened yet: "Changes not stored" asks.
    assert_eq!(session.workspace_root(), Some(root.as_path()));
    let events = session.take_events();
    let question = close_question(&events).expect("asked");
    assert!(question.switching);
    assert_eq!(question.what, "the workspace notes");
    assert!(question.error.starts_with("Todo #1 could not be stored: "));
    // Closing Underline asks as well.
    let question = session.request_close().expect("asked");
    assert!(question.error.starts_with("Todo #1 could not be stored: "));
    assert!(!question.switching);

    // Writable again: opening stores it and opens the other.
    writable(&root.join(".todos"), true);
    session.open_chosen(&other, false);
    assert_eq!(
        load_todos(&root).get(1).unwrap().urgency,
        underline_core::Urgency::High
    );
    assert!(session.workspace_root().unwrap().ends_with("other"));
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn review_f15_close_anyway_opens_the_other_workspace_without_the_todos() {
    let dir = temp_dir("switch-anyway");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Lost");
    if !unwritable(&root) {
        return;
    }
    change(&mut session, 1, urgency(UrgencyName::High));
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Other.md"), "# Other\n").unwrap();
    session.open_chosen(&other, false);
    assert!(close_question(&session.take_events()).is_some());
    writable(&root.join(".todos"), true);

    session.switch_anyway();

    assert!(session.workspace_root().unwrap().ends_with("other"));
    assert!(
        session.status().starts_with("Opened "),
        "{}",
        session.status()
    );
    // Given up: not written later either.
    assert_eq!(
        load_todos(&root).get(1).unwrap().urgency,
        underline_core::Urgency::Normal
    );
    // Without a workspace waiting, nothing happens.
    session.switch_anyway();
    assert!(session.workspace_root().unwrap().ends_with("other"));
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn review_f15_the_last_attempt_at_the_end_never_writes_over_changed_or_unreadable_files() {
    let dir = temp_dir("end");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "One");
    create(&mut session, "Two");
    if !unwritable(&root) {
        return;
    }
    change(&mut session, 1, urgency(UrgencyName::High));
    change(&mut session, 2, urgency(UrgencyName::High));
    writable(&root.join(".todos"), true);
    let one = root.join(".todos/1.yaml");
    let theirs = fs::read_to_string(&one)
        .unwrap()
        .replace("title: One", "title: Theirs");
    fs::write(&one, &theirs).unwrap();
    fs::write(root.join(".todos/2.yaml"), "title: [unclosed").unwrap();

    let question = session.request_close().expect("asked");
    assert!(
        question.error.contains(&super::disk::changed_elsewhere(1)),
        "{}",
        question.error
    );
    assert!(question.error.contains("todo #2 cannot be read: "));
    let problems = session.end().unwrap_err();
    assert_eq!(problems.len(), 2, "{problems:?}");

    assert_eq!(fs::read_to_string(&one).unwrap(), theirs);
    assert_eq!(
        fs::read_to_string(root.join(".todos/2.yaml")).unwrap(),
        "title: [unclosed"
    );
}

#[test]
fn review_f15_undo_of_a_mark_goes_when_the_todo_is_changed_again() {
    let dir = temp_dir("undo-mark-later");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Call");
    let job = session.todo_mark_start(1, true);
    run(&mut session, job);
    assert_eq!(
        status_offer(&session.tick(Instant::now())),
        Some(StatusOffer::UndoTodoMark)
    );

    change(
        &mut session,
        1,
        TodoChange::Title {
            title: "Call back".into(),
        },
    );

    assert!(session.todo_undo_start().is_none());
    let stored = load_todos(&root);
    assert_eq!(stored.get(1).unwrap().title, "Call back");
    assert!(stored.get(1).unwrap().is_done());
}

#[test]
fn review_f15_a_todo_kept_only_here_and_deleted_is_never_written_after() {
    let dir = temp_dir("forget-local");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Ghost");
    fs::remove_file(root.join(".todos/1.yaml")).unwrap();
    // Refused (deleted elsewhere): kept here only.
    change(&mut session, 1, urgency(UrgencyName::High));
    // A change of it waits to be written when it is deleted.
    let queued = session
        .todo_change_start(1, urgency(UrgencyName::Low))
        .unwrap();

    assert!(session.todo_delete_start(1).is_none());
    assert_eq!(session.status(), "Deleted todo #1 Ghost");
    let done = queued.run();
    session.todo_finish(done);
    settle(&mut session);
    let _ = session.todos_settle();

    assert!(!root.join(".todos/1.yaml").exists());
    assert!(view(&mut session).todos.is_empty());
}

#[test]
fn review_f15_a_deletion_wins_over_a_change_in_either_order() {
    let dir = temp_dir("delete-wins");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Before");
    create(&mut session, "After");

    // The change was made before the deletion, and written after it.
    let change_one = session
        .todo_change_start(1, urgency(UrgencyName::High))
        .unwrap();
    let delete_one = session.todo_delete_start(1).unwrap();
    // The change was made after the deletion started.
    let delete_two = session.todo_delete_start(2).unwrap();
    let change_two = session
        .todo_change_start(2, urgency(UrgencyName::High))
        .unwrap();
    for job in [delete_one, delete_two, change_one, change_two] {
        let done = job.run();
        session.todo_finish(done);
    }

    assert!(!root.join(".todos/1.yaml").exists());
    assert!(!root.join(".todos/2.yaml").exists());
    let todos = view(&mut session);
    assert!(todos.todos.is_empty(), "{:?}", todos.todos);
}

#[test]
fn review_f15_changed_again_after_an_outside_deletion_and_not_brought_back_later() {
    let dir = temp_dir("again-after-deletion");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Mine");
    let file = root.join(".todos/1.yaml");
    fs::remove_file(&file).unwrap();

    change(&mut session, 1, urgency(UrgencyName::High));
    assert!(!file.exists());
    assert_eq!(
        view(&mut session).todos[0].unstored.as_deref(),
        Some(super::disk::deleted_elsewhere(1).as_str())
    );
    // Changed again by the user: stored, as the reason says.
    change(&mut session, 1, urgency(UrgencyName::Critical));
    assert!(file.exists());
    assert_eq!(view(&mut session).todos[0].unstored, None);

    // Deleted elsewhere again: the looks of the second do not bring it back.
    fs::remove_file(&file).unwrap();
    settle(&mut session);
    settle(&mut session);
    assert!(!file.exists());
    assert!(view(&mut session).todos.is_empty());
}

#[test]
#[cfg_attr(
    not(unix),
    ignore = "needs Unix permissions to make the todo folder unwritable"
)]
fn review_f15_a_retry_without_a_base_never_writes_over_a_todo_made_elsewhere() {
    let dir = temp_dir("retry-no-base");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Mine");
    let file = root.join(".todos/1.yaml");
    fs::remove_file(&file).unwrap();
    change(&mut session, 1, urgency(UrgencyName::High));
    if !unwritable(&root) {
        return;
    }
    // Changed again while the folder cannot be written: kept, no base.
    change(&mut session, 1, urgency(UrgencyName::Critical));
    writable(&root.join(".todos"), true);
    // Another computer's todo #1 arrives before the retry.
    fs::write(&file, "title: Theirs\n").unwrap();

    settle(&mut session);
    settle(&mut session);

    assert_eq!(fs::read_to_string(&file).unwrap(), "title: Theirs\n");
}

#[test]
fn review_f15_the_thread_of_a_workspace_left_stops_and_writes_nothing() {
    let dir = temp_dir("stop");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    let stop = Arc::clone(&session.todos.poller.as_ref().unwrap().stop);
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Other.md"), "# Other\n").unwrap();

    session.open_chosen(&other, false);

    assert!(stop.load(Ordering::SeqCst));
    // A stopped thread writes nothing it is still asked to.
    let poller = Poller::start(root.clone(), Disk::default(), Wake::default()).unwrap();
    poller.stop.store(true, Ordering::SeqCst);
    let todo = underline_core::Todo::new(5, "Late").unwrap();
    poller
        .ask
        .send(PollAsk {
            seq: 1,
            unstored: vec![(todo, None)],
            before: None,
        })
        .unwrap();
    assert!(poller.answer.recv_timeout(Duration::from_secs(2)).is_err());
    assert!(!root.join(".todos/5.yaml").exists());
}

#[test]
fn review_f15_numbers_are_given_once_even_when_created_together() {
    let dir = temp_dir("numbers");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    let jobs: Vec<TodoJob> = ["A", "B", "C"]
        .iter()
        .map(|t| session.todo_create_start(t).unwrap())
        .collect();
    let done: Vec<_> = jobs
        .into_iter()
        .map(|job| std::thread::spawn(move || job.run()))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    for d in done {
        session.todo_finish(d);
    }
    settle(&mut session);
    let mut numbers: Vec<u32> = view(&mut session).todos.iter().map(|t| t.number).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [1, 2, 3]);

    // BR-002: a number freed by a deletion is not given again.
    let job = session.todo_delete_start(3);
    run(&mut session, job);
    create(&mut session, "D");
    assert_eq!(session.status(), "Created todo #4 D");
}

#[test]
fn uc020_a7_a8_m18_9_a_change_from_outside_shows_within_a_second() {
    let dir = temp_dir("outside");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Mine");
    session.tick(Instant::now());
    // An agent (or a sync) writes a todo.
    fs::write(
        root.join(".todos/7.yaml"),
        "title: From the agent\nurgency: high\nstatus: open\ncreated: 2026-11-10T09:00:00+01:00\n",
    )
    .unwrap();

    // Not before the second is over.
    session.tick(Instant::now());
    assert_eq!(view(&mut session).todos.len(), 1);
    let mut told = Vec::new();
    let mut at = Instant::now() + TODOS_EVERY;
    for _ in 0..500 {
        told.extend(session.tick(at));
        if todos_event(&told).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
        at += Duration::from_millis(5);
    }

    let todos = todos_event(&told).expect("the change is told");
    assert_eq!(todos.todos.len(), 2);
    assert_eq!(todos.todos[1].name, "#7 From the agent");
    assert_eq!(todos.heading, "Todos (2)");
    assert_eq!(TODOS_EVERY, Duration::from_secs(1));
}

#[test]
fn uc020_br006_a_later_change_is_never_written_over_by_an_earlier_one() {
    let dir = temp_dir("order-of-writes");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Both");

    // Two changes made one after the other; their writes finish the other
    // way round.
    let first = session.todo_change_start(1, day("2030-03-03")).unwrap();
    let second = session
        .todo_change_start(
            1,
            TodoChange::Urgency {
                urgency: UrgencyName::Low,
            },
        )
        .unwrap();
    let second = second.run();
    let first = first.run();
    session.todo_finish(second);
    session.todo_finish(first);

    let stored = load_todos(&root);
    let todo = stored.get(1).unwrap();
    assert_eq!(todo.due, "2030-03-03".parse().ok());
    assert_eq!(todo.urgency, underline_core::Urgency::Low);
    let shown = &view(&mut session).todos[0];
    assert_eq!(shown.due.as_deref(), Some("2030-03-03"));
    assert_eq!(shown.urgency, UrgencyName::Low);
}

#[test]
fn uc021_m06_5_m18_6_the_tab_shows_closes_and_keeps_its_sort_and_filter() {
    let dir = temp_dir("tab");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);

    session.show_todo_list(None);
    let tabs = session.snapshot().tabs;
    assert!(tabs.todo_list.open && tabs.todo_list.active);
    // The note tab stays the active note tab; no note is linked from here.
    assert_eq!(tabs.active.as_deref(), Some("Home"));
    assert_eq!(view(&mut session).active_note, None);

    session.set_todo_list(TodoSortName::Urgency, TodoFilterName::All);
    let list = session.state.todo_list(&root);
    assert_eq!(list.sort, underline_core::TodoSort::Urgency);
    assert_eq!(list.filter, underline_core::TodoFilter::All);
    assert!(list.open && list.active);

    // UC-021 step 6: a note opens; the tab "Todos" stays open.
    session.open_note(&NoteId::from_relative("Home"));
    let tabs = session.snapshot().tabs;
    assert!(tabs.todo_list.open && !tabs.todo_list.active);
    assert!(view(&mut session).active_note.is_some());

    session.close_todo_list();
    let list = session.state.todo_list(&root);
    assert!(!list.open && !list.active);
    assert_eq!(list.filter, underline_core::TodoFilter::All);
}

#[test]
fn uc021_a2_m04_5_overdue_todos_open_the_list_with_the_filter_open() {
    let dir = temp_dir("overdue");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    session.show_todo_list(None);
    session.set_todo_list(TodoSortName::Due, TodoFilterName::Done);
    session.close_todo_list();
    create(&mut session, "Late");
    create(&mut session, "Later");
    let today = Day::today();
    change(&mut session, 1, day("2020-01-01"));
    change(&mut session, 2, day("2020-01-02"));

    let todos = view(&mut session);
    assert_eq!(todos.overdue.as_deref(), Some("2 todos overdue"));
    let late = &todos.todos[0];
    let days = today.days_since("2020-01-01".parse().unwrap());
    assert_eq!(
        late.overdue.as_deref(),
        Some(format!("2020-01-01  {days} days overdue").as_str())
    );
    change(&mut session, 2, TodoChange::Done { done: true });
    assert_eq!(
        view(&mut session).overdue.as_deref(),
        Some("1 todo overdue")
    );

    session.show_todo_list(Some(TodoFilterName::Open));
    let table = view(&mut session).table;
    assert_eq!(table.filter, TodoFilterName::Open);
    assert_eq!(table.rows, [1]);
    assert_eq!(table.done_heading, "");
}

#[test]
fn uc021_br001_br002_a3_m18_8_rows_sorted_filtered_and_empty() {
    let dir = temp_dir("rows");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    session.show_todo_list(None);
    assert_eq!(
        view(&mut session).table.empty.as_deref(),
        Some("No open todos. Type a title above to create one.")
    );
    for title in ["a", "b", "c"] {
        create(&mut session, title);
    }
    change(&mut session, 1, day("2031-01-02"));
    change(&mut session, 2, day("2031-01-01"));
    change(
        &mut session,
        3,
        TodoChange::Urgency {
            urgency: UrgencyName::Critical,
        },
    );

    assert_eq!(view(&mut session).table.rows, [2, 1, 3]);
    session.set_todo_list(TodoSortName::Urgency, TodoFilterName::Open);
    assert_eq!(view(&mut session).table.rows, [3, 2, 1]);
    session.set_todo_list(TodoSortName::Urgency, TodoFilterName::Done);
    let table = view(&mut session).table;
    assert_eq!(table.empty.as_deref(), Some("No done todos."));
    assert_eq!(table.done_heading, "Done");
}

#[test]
fn uc021_br009_m18_8_more_than_two_notes_are_counted() {
    let dir = temp_dir("more-links");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Home.md", "# Home\n"),
            ("Plan.md", "# Plan\n"),
            ("Ideas.md", "# Ideas\n"),
            ("Trip.md", "# Trip\n"),
            ("Log.md", "# Log\n"),
        ],
    );
    create(&mut session, "Many");
    for note in ["Plan", "Ideas", "Trip", "Log"] {
        change(&mut session, 1, TodoChange::Link { note: note.into() });
    }

    let todo = view(&mut session).todos[0].clone();
    assert_eq!(todo.more_links.as_deref(), Some("+2"));
    assert_eq!(todo.links_summary, "Plan, Ideas +2");
}

#[test]
fn uc020_another_workspace_forgets_the_todos_of_the_one_left() {
    let dir = temp_dir("switch");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    create(&mut session, "Here");
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Other.md"), "# Other\n").unwrap();

    session.open_chosen(&other, false);

    assert!(!session.todos_read());
    assert!(session.todo_change_start(1, day("")).is_err());
    settle(&mut session);
    assert!(view(&mut session).todos.is_empty());
}

#[test]
fn uc020_br003_m18_3_the_calendar_starts_on_monday() {
    let november = calendar_view(Some("2026-11-11"), 0);
    assert_eq!(november.title, "November 2026");
    assert_eq!(november.month, "2026-11-01");
    // 1 November 2026 is a Sunday.
    assert_eq!(november.lead, 6);
    assert_eq!(november.days.len(), 30);
    assert_eq!(november.days[10].day, "2026-11-11");
    let december = calendar_view(Some(&november.month), 1);
    assert_eq!(december.title, "December 2026");
    assert_eq!(calendar_view(Some("2026-01-31"), -1).month, "2025-12-01");
    // Without a day, today's month, today marked.
    let now = calendar_view(None, 0);
    assert_eq!(now.days.iter().filter(|d| d.today).count(), 1);
}
