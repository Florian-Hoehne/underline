//! Tests of feature F13: branches, syncs and conflicts (UC-013, UC-014,
//! UC-015, inventory M-04.1 to M-04.3, M-19.6 to M-19.10), against what the
//! egui app does (`open_branch_menu`, `switch_branch`, `request_sync`,
//! `run_syncs`, `conflict_bar`, `resolve_active`, `check_resolved`,
//! `after_resolving`, `show_abort_confirm`, `change_action`, `poll_todos`
//! of `apps/underline-desktop/src/app.rs`).
//!
//! Each test makes real repositories in a temporary folder: a bare one as
//! the remote (no network), a clone `a` the session opens, and a clone `b`
//! for the other computer. git reads only a configuration of the test's
//! own (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`, `HOME`), never the
//! user's.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use underline_core::{create_todo, save_todo, NoteId, StateStore, SyncMode, SETTINGS_FILE_NAME};

use crate::remote_view::{
    BranchTarget, ChoiceName, ConflictBarView, GitBarView, SideName, SwitchAnswer, TodoConflictView,
};
use crate::{Event, Session, SettingsState};

/// How long a test waits for git.
const WAIT: Duration = Duration::from_secs(30);

/// The git configuration of a test: its own global file, its own home.
struct Config {
    home: PathBuf,
    file: PathBuf,
}

impl Config {
    fn new(dir: &Path, extra: &str) -> Self {
        let home = dir.join("home");
        fs::create_dir_all(&home).unwrap();
        let file = home.join("gitconfig");
        fs::write(
            &file,
            format!(
                "[user]\n\tname = Test\n\temail = test@example.com\n[init]\n\tdefaultBranch = main\n{extra}"
            ),
        )
        .unwrap();
        Self { home, file }
    }

    fn env(&self) -> Vec<(&'static str, &std::ffi::OsStr)> {
        vec![
            ("GIT_CONFIG_GLOBAL", self.file.as_os_str()),
            ("GIT_CONFIG_NOSYSTEM", std::ffi::OsStr::new("1")),
            ("HOME", self.home.as_os_str()),
        ]
    }

    fn run(&self, dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", &self.file)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.home)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-remote-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn read(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).unwrap()
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// A bare remote, clone `a` with `files` committed and pushed, and clone
/// `b` of it.
struct Remote {
    dir: PathBuf,
    config: Config,
}

impl Remote {
    fn new(name: &str, extra: &str, files: &[(&str, &str)]) -> Self {
        let dir = temp_dir(name);
        let config = Config::new(&dir, extra);
        config.run(&dir, &["init", "--quiet", "--bare", "remote.git"]);
        config.run(&dir, &["clone", "--quiet", "remote.git", "a"]);
        let a = dir.join("a");
        for (file, text) in files {
            write(&a, file, text);
        }
        config.run(&a, &["add", "--all"]);
        config.run(&a, &["commit", "--quiet", "--message", "First"]);
        config.run(&a, &["push", "--quiet", "--set-upstream", "origin", "main"]);
        config.run(&dir, &["clone", "--quiet", "remote.git", "b"]);
        Self { dir, config }
    }

    fn a(&self) -> PathBuf {
        self.dir.join("a")
    }

    fn b(&self) -> PathBuf {
        self.dir.join("b")
    }

    /// The other computer commits everything and pushes.
    fn push_from_b(&self, message: &str) {
        let b = self.b();
        self.config.run(&b, &["add", "--all"]);
        self.config
            .run(&b, &["commit", "--quiet", "--message", message]);
        self.config.run(&b, &["push", "--quiet"]);
    }

    /// The remote's commit messages, newest first.
    fn remote_log(&self) -> String {
        self.config.run(
            &self.dir.join("remote.git"),
            &["log", "--format=%s", "main"],
        )
    }

    /// A session on clone `a`.
    fn session(&self) -> Session {
        session_on(&self.dir, &self.config, &self.a())
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn session_on(dir: &Path, config: &Config, root: &Path) -> Session {
    let mut session = Session::new(
        Some(root.to_owned()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.use_git_env(&config.env());
    session.start_workspace();
    session
}

/// What the window was told last.
#[derive(Default)]
struct Told {
    bar: Option<GitBarView>,
    conflict: Option<ConflictBarView>,
    todos: Vec<TodoConflictView>,
}

impl Told {
    fn take(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::GitBar(bar) => self.bar = Some(bar),
                Event::Conflict(bar) => self.conflict = bar,
                Event::TodoConflicts(todos) => self.todos = todos,
                _ => {}
            }
        }
    }

    fn sync_text(&self) -> String {
        self.bar
            .as_ref()
            .and_then(|b| b.sync.as_ref())
            .map(|s| s.text.clone())
            .unwrap_or_default()
    }
}

/// Ticks until `done` holds; panics with `what` after [`WAIT`].
fn until(
    session: &mut Session,
    told: &mut Told,
    what: &str,
    mut done: impl FnMut(&Session, &Told) -> bool,
) {
    let start = Instant::now();
    while start.elapsed() < WAIT {
        let events = session.tick(Instant::now());
        told.take(events);
        if done(session, told) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "{what} did not come; the status is {:?}, the bar {:?}",
        session.status(),
        told.bar
    );
}

/// The revision of the backend's text of the open note.
fn rev(session: &mut Session, note: &str) -> u32 {
    session.editor_mirror(&id(note)).unwrap().rev
}

/// The backend's text of the open note.
fn tab_text(session: &mut Session, note: &str) -> String {
    session.editor_mirror(&id(note)).unwrap().text
}

const PLAN: &str = "one\ntwo\nthree\n";

/// TC-014-01 / TC-014-02 / TC-014-05 / TC-014-18, M-19.8, M-04.1, M-04.3:
/// the workspace syncs when it opens; the other side's change shows in the
/// open tab; "Sync now" commits the changes as `Auto:` and pushes them.
#[test]
fn uc014_tc01_tc02_tc05_m19_8_sync_on_open_and_sync_now() {
    let remote = Remote::new("sync", "", &[("Plan.md", PLAN), ("Ideas.md", "ideas\n")]);
    write(&remote.b(), "Ideas.md", "ideas from b\n");
    remote.push_from_b("From b");
    let mut session = remote.session();
    session.open_note(&id("Ideas"));
    let mut told = Told::default();

    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    assert_eq!(read(&remote.a(), "Ideas.md"), "ideas from b\n");
    assert_eq!(tab_text(&mut session, "Ideas"), "ideas from b\n");
    let bar = told.bar.clone().unwrap();
    assert!(bar.branch.as_deref().unwrap().starts_with("Branch main"));
    assert_eq!(bar.conflicts, None);
    let sync = bar.sync.unwrap();
    assert!(sync.can_sync && !sync.running && !sync.warning);
    assert_eq!((sync.to_push, sync.to_pull), (None, None));

    // Sync now: the change goes out as an automatic commit.
    session.open_note(&id("Plan"));
    session
        .backend_edit(&id("Plan"), &[(0..3, "ONE".to_owned())], false)
        .unwrap();
    session.request_sync(SyncMode::Full, true);
    until(&mut session, &mut told, "Syncing…", |_, t| {
        t.sync_text() == "Syncing…"
    });
    assert!(!told.bar.as_ref().unwrap().sync.as_ref().unwrap().can_sync);
    until(&mut session, &mut told, "the sync", |_, t| {
        t.sync_text() == "Synced just now"
    });
    assert!(remote.remote_log().starts_with("Auto: Update Plan"));
    assert_eq!(read(&remote.a(), "Plan.md"), "ONE\ntwo\nthree\n");
}

/// TC-014-16 / M-19.8: without a remote no sync runs, and the refusal is
/// said only when the user asked.
#[test]
fn uc014_tc16_m19_8_no_remote_says_so_only_when_asked() {
    let dir = temp_dir("no-remote");
    let config = Config::new(&dir, "");
    let root = dir.join("ws");
    fs::create_dir_all(&root).unwrap();
    config.run(&root, &["init", "--quiet"]);
    write(&root, "Plan.md", PLAN);
    config.run(&root, &["add", "--all"]);
    config.run(&root, &["commit", "--quiet", "--message", "First"]);
    let mut session = session_on(&dir, &config, &root);
    let mut told = Told::default();
    until(&mut session, &mut told, "the branch", |_, t| {
        t.bar.as_ref().is_some_and(|b| b.branch.is_some())
    });
    assert_eq!(
        told.bar.as_ref().unwrap().branch.as_deref(),
        Some("Branch main")
    );
    assert!(told.bar.as_ref().unwrap().sync.is_none());
    let said = session.status().to_owned();

    session.request_sync(SyncMode::Full, false);
    assert_eq!(session.status(), said);
    session.request_sync(SyncMode::Full, true);
    assert_eq!(
        session.status(),
        "This workspace has no remote to sync with"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// TC-011 A9 / M-04.1 (Q-93): when git cannot answer, the status bar says
/// "Changes cannot be shown: {e}" instead of the branch.
#[test]
fn uc011_a9_m04_1_changes_cannot_be_shown() {
    let dir = temp_dir("broken");
    let config = Config::new(&dir, "");
    let root = dir.join("ws");
    fs::create_dir_all(root.join(".git")).unwrap();
    write(&root, "Plan.md", PLAN);
    let mut session = session_on(&dir, &config, &root);
    let mut told = Told::default();

    until(&mut session, &mut told, "the problem", |_, t| {
        t.bar.as_ref().is_some_and(|b| b.problem.is_some())
    });
    let bar = told.bar.unwrap();
    assert!(
        bar.problem
            .as_deref()
            .unwrap()
            .starts_with("Changes cannot be shown: "),
        "{bar:?}"
    );
    assert_eq!(bar.branch, None);
    let _ = fs::remove_dir_all(&dir);
}

/// Makes both sides change line 2 of Plan, and opens the session on `a`
/// with Plan in a tab; ticks until the sync stopped with the conflict and
/// the bar of the note shows.
fn conflict_session(remote: &Remote, told: &mut Told) -> Session {
    write(&remote.b(), "Plan.md", "one\nTWO from b\nthree\n");
    remote.push_from_b("From b");
    write(&remote.a(), "Plan.md", "one\ntwo from a\nthree\n");
    let mut session = remote.session();
    session.open_note(&id("Plan"));
    until(&mut session, told, "the conflict bar", |_, t| {
        t.conflict.is_some()
    });
    session
}

/// TC-014-09 / TC-015-01 / TC-015-02 / TC-015-03 / TC-015-11 / M-04.2,
/// M-19.9, M-19.10: a sync stops with a conflict; the bar shows the place,
/// rename, delete and export wait, "Here" resolves it, the note is stored
/// and marked resolved, the joining completes and the sync goes on.
#[test]
fn uc015_tc01_tc02_tc03_tc11_m19_9_m19_10_resolve_a_place() {
    let remote = Remote::new("resolve", "", &[("Plan.md", PLAN), ("Ideas.md", "i\n")]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);

    // The watcher's report of git's places may come after the sync's (as
    // in egui).
    assert!(
        [
            "The sync stopped: Plan.md changed here and on the remote in the same lines",
            "Plan was changed outside"
        ]
        .contains(&session.status()),
        "{}",
        session.status()
    );
    let bar = told.bar.clone().unwrap();
    assert_eq!(bar.conflicts.as_deref(), Some("1 conflict"));
    let sync = bar.sync.unwrap();
    assert_eq!(sync.text, "Sync stopped: conflict");
    assert!(sync.warning && !sync.can_sync);
    assert_eq!(sync.hover.as_deref(), Some("Plan.md"));
    let conflict = told.conflict.clone().unwrap();
    assert_eq!(conflict.note, "Plan");
    assert_eq!(conflict.places.len(), 1);
    let place = &conflict.places[0];
    assert_eq!((place.line, place.offset), (2, 4));
    assert_eq!(
        (place.here.as_str(), place.remote.as_str()),
        ("two from a", "TWO from b")
    );
    // TC-015-11, UC-015 BR-007: other use cases wait.
    assert!(session.rename_start(&id("Plan")).is_none());
    assert_eq!(
        session.status(),
        "Plan is in conflict; resolve it before renaming it"
    );
    assert!(session.delete_start(&id("Plan")).is_none());
    assert!(session.export_start(&id("Plan")).is_none());
    assert_eq!(
        session.status(),
        "Plan is in conflict; resolve it before exporting it"
    );
    // A7: no sync while a merge waits.
    session.request_sync(SyncMode::Full, true);
    assert_eq!(
        session.status(),
        "No sync, because notes are in conflict; resolve them first"
    );

    let r = rev(&mut session, "Plan");
    session.conflict_choose(&id("Plan"), r, Some(0), ChoiceName::Here);
    assert_eq!(tab_text(&mut session, "Plan"), "one\ntwo from a\nthree\n");
    until(&mut session, &mut told, "the conflicts resolved", |s, _| {
        s.status() == "The conflicts are resolved; syncing"
    });
    assert!(told.conflict.is_none());
    until(
        &mut session,
        &mut told,
        "the sync after resolving",
        |_, t| t.sync_text() == "Synced just now",
    );
    assert!(remote
        .remote_log()
        .contains("Merge remote changes into main"));
    assert_eq!(read(&remote.a(), "Plan.md"), "one\ntwo from a\nthree\n");
    assert!(!session.in_conflict(&id("Plan")));
}

/// TC-015-02 / TC-015-04 / M-19.9: "Both" keeps here first, then remote;
/// "All: Remote" takes every open place; editing the places away by hand
/// resolves too (BR-003).
#[test]
fn uc015_tc02_tc04_m19_9_both_all_and_by_hand() {
    let remote = Remote::new("both", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);
    let marked = tab_text(&mut session, "Plan");

    let r = rev(&mut session, "Plan");
    session.conflict_choose(&id("Plan"), r, Some(0), ChoiceName::Both);
    assert_eq!(
        tab_text(&mut session, "Plan"),
        "one\ntwo from a\nTWO from b\nthree\n"
    );
    // Undone by hand: the markers come back, then go by hand.
    let len = tab_text(&mut session, "Plan").len();
    session
        .backend_edit(&id("Plan"), &[(0..len, marked.clone())], false)
        .unwrap();
    let r = rev(&mut session, "Plan");
    session.conflict_choose(&id("Plan"), r, None, ChoiceName::Remote);
    assert_eq!(tab_text(&mut session, "Plan"), "one\nTWO from b\nthree\n");
    until(&mut session, &mut told, "the conflicts resolved", |s, _| {
        s.status() == "The conflicts are resolved; syncing"
    });
    assert_eq!(read(&remote.a(), "Plan.md"), "one\nTWO from b\nthree\n");
}

/// TC-015-08 / M-04.2, M-19.10: "Abort the sync" puts the workspace back
/// as before the sync; the tab shows the note again, the local commit stays.
#[test]
fn uc015_tc08_m19_10_abort() {
    let remote = Remote::new("abort", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);

    let job = session.abort_prepare().unwrap();
    session.finish_abort(job.run());

    assert_eq!(
        session.status(),
        "The sync was aborted; the workspace is as before it"
    );
    assert_eq!(read(&remote.a(), "Plan.md"), "one\ntwo from a\nthree\n");
    assert_eq!(tab_text(&mut session, "Plan"), "one\ntwo from a\nthree\n");
    let log = remote
        .config
        .run(&remote.a(), &["log", "-1", "--format=%s"]);
    assert!(log.starts_with("Auto:"), "{log}");
    until(&mut session, &mut told, "the conflict gone", |_, t| {
        t.conflict.is_none() && t.bar.as_ref().is_some_and(|b| b.conflicts.is_none())
    });
    // Nothing waits any more: no second abort.
    assert!(session.abort_prepare().is_none());
}

/// TC-015-05 / TC-015-06 / M-19.2: a file that is not a note keeps one
/// side and the other as a copy; a note deleted on the remote and changed
/// here is deleted with "Delete" and goes to the deleted notes; then the
/// joining completes.
#[test]
fn uc015_tc05_tc06_m19_2_keep_here_and_delete() {
    let remote = Remote::new(
        "sides",
        "",
        &[
            ("shot.png", "png 1"),
            ("Ideas.md", "ideas\n"),
            ("Plan.md", PLAN),
        ],
    );
    write(&remote.b(), "shot.png", "png b");
    fs::remove_file(remote.b().join("Ideas.md")).unwrap();
    remote.push_from_b("From b");
    write(&remote.a(), "shot.png", "png a");
    write(&remote.a(), "Ideas.md", "ideas changed here\n");
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "two conflicts", |_, t| {
        t.bar
            .as_ref()
            .is_some_and(|b| b.conflicts.as_deref() == Some("2 conflicts"))
    });

    session.conflict_take_side("shot.png", SideName::Here);
    until(&mut session, &mut told, "the side taken", |s, _| {
        s.status() == "Kept one version of shot.png; the other is shot (remote).png"
    });
    assert_eq!(read(&remote.a(), "shot.png"), "png a");
    assert_eq!(read(&remote.a(), "shot (remote).png"), "png b");

    session.conflict_keep("Ideas.md", false);
    until(&mut session, &mut told, "the conflicts resolved", |s, _| {
        s.status() == "The conflicts are resolved; syncing"
    });
    assert!(!remote.a().join("Ideas.md").exists());
    until(&mut session, &mut told, "the steps done", |s, _| {
        !s.resolving()
    });
}

/// TC-020 A10 / M-19.10: a todo changed on both sides shows field by field;
/// "Keep mine" keeps this computer's version and completes the joining.
#[test]
fn uc020_a10_m19_10_todo_in_conflict() {
    let remote = Remote::new("todo", "", &[("Plan.md", PLAN)]);
    let todo = create_todo(&remote.a(), "Buy milk").unwrap();
    remote.config.run(&remote.a(), &["add", "--all"]);
    remote
        .config
        .run(&remote.a(), &["commit", "--quiet", "--message", "Todo"]);
    remote.config.run(&remote.a(), &["push", "--quiet"]);
    remote
        .config
        .run(&remote.b(), &["pull", "--quiet", "--no-rebase"]);
    let mut theirs = todo.clone();
    theirs.title = "Buy oat milk".into();
    save_todo(&remote.b(), &theirs).unwrap();
    remote.push_from_b("From b");
    let mut mine = todo;
    mine.title = "Buy goat milk".into();
    save_todo(&remote.a(), &mine).unwrap();
    let mut session = remote.session();
    let mut told = Told::default();

    until(&mut session, &mut told, "the todo in conflict", |_, t| {
        !t.todos.is_empty()
    });
    let view = &told.todos[0];
    assert_eq!(view.heading, "Todo #1 was changed here and on the remote");
    assert_eq!(view.rows[0].name, "Title");
    assert_eq!(
        (view.rows[0].here.as_str(), view.rows[0].remote.as_str()),
        ("Buy goat milk", "Buy oat milk")
    );
    assert!(view.rows[0].differ && !view.rows[3].differ);

    session.todo_conflict_resolve(1, true);
    until(&mut session, &mut told, "the conflicts resolved", |s, _| {
        s.status() == "The conflicts are resolved; syncing"
    });
    assert!(read(&remote.a(), ".todos/1.yaml").contains("Buy goat milk"));
    until(&mut session, &mut told, "the todos gone", |_, t| {
        t.todos.is_empty()
    });
}

/// A repository without a remote with branches `main` and `drafts`:
/// `drafts` changes Plan and adds Sketch.
fn branch_repo(name: &str) -> (PathBuf, Config, PathBuf) {
    let dir = temp_dir(name);
    let config = Config::new(&dir, "");
    let root = dir.join("ws");
    fs::create_dir_all(&root).unwrap();
    config.run(&root, &["init", "--quiet"]);
    write(&root, "Plan.md", PLAN);
    write(&root, "Ideas.md", "ideas\n");
    config.run(&root, &["add", "--all"]);
    config.run(&root, &["commit", "--quiet", "--message", "First"]);
    config.run(&root, &["switch", "--quiet", "--create", "drafts"]);
    write(&root, "Plan.md", "draft plan\n");
    write(&root, "Sketch.md", "sketch\n");
    config.run(&root, &["add", "--all"]);
    config.run(&root, &["commit", "--quiet", "--message", "Drafts"]);
    (dir, config, root)
}

/// Lists the branches as the command does; the menu.
fn menu(session: &mut Session) -> crate::BranchMenuView {
    let job = session.branches_prepare().unwrap();
    session.branches_listed(job.run()).unwrap()
}

/// Switches as the command does.
fn switch(session: &mut Session, target: BranchTarget) -> SwitchAnswer {
    match session.switch_prepare(&target) {
        Ok(job) => session.finish_switch(job.run()),
        Err(answer) => answer,
    }
}

/// Ticks until git answered about the changes.
fn tracked(session: &mut Session, told: &mut Told) {
    until(session, told, "the branch", |_, t| {
        t.bar.as_ref().is_some_and(|b| b.branch.is_some())
    });
}

/// TC-013-01 / TC-013-02 / TC-013-03 / TC-013-04 / M-19.6, M-19.7: the
/// menu lists the branches; a switch carries the changes along, closes the
/// tab of a note the branch does not hold and shows the others as it
/// holds them.
#[test]
fn uc013_tc01_tc02_tc03_tc04_m19_6_m19_7_switch() {
    let (dir, config, root) = branch_repo("switch");
    let mut session = session_on(&dir, &config, &root);
    session.open_note(&id("Sketch"));
    session.open_note(&id("Plan"));
    write(&root, "Ideas.md", "ideas changed\n");
    let mut told = Told::default();
    // The tracker saw the change of Ideas.
    until(&mut session, &mut told, "one change", |_, t| {
        t.bar
            .as_ref()
            .is_some_and(|b| b.branch.as_deref() == Some("Branch drafts · 1 change"))
    });

    let listed = menu(&mut session);
    let labels: Vec<&str> = listed.branches.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["drafts  (current)", "main"]);
    assert!(!listed.merging && listed.problem.is_none());

    let answer = switch(
        &mut session,
        BranchTarget::Local {
            name: "main".into(),
        },
    );
    assert_eq!(answer, SwitchAnswer::Done);
    assert_eq!(
        session.status(),
        "Switched to main; carried along 1 change; closed Sketch, not on this branch"
    );
    assert_eq!(tab_text(&mut session, "Plan"), PLAN);
    assert!(session.editor_mirror(&id("Sketch")).is_err());
    assert_eq!(read(&root, "Ideas.md"), "ideas changed\n");
    let _ = fs::remove_dir_all(&dir);
}

/// TC-013-05 / TC-013-06 / TC-013-07 / M-19.6, M-19.7: a new branch's
/// name is shown cleaned or refused; a taken name offers the existing
/// branch; changes in the way keep the menu open.
#[test]
fn uc013_tc05_tc06_tc07_m19_6_new_branch_and_in_the_way() {
    assert_eq!(
        super::super::branch_name_view(" trip  ideas ").text,
        "Branch: trip-ideas"
    );
    let bad = super::super::branch_name_view("a..b");
    assert!(!bad.ok && bad.text.ends_with('.'), "{bad:?}");

    let (dir, config, root) = branch_repo("new");
    let mut session = session_on(&dir, &config, &root);
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    menu(&mut session);
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::New {
                name: "main".into()
            }
        ),
        SwitchAnswer::Exists {
            problem: "The branch main exists; choose it in the list to switch to it.".into(),
            name: "main".into(),
        }
    );
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::New {
                name: "trip ideas".into()
            }
        ),
        SwitchAnswer::Done
    );
    assert_eq!(session.status(), "Switched to trip-ideas");

    // A3: Plan differs on main and is changed here.
    write(&root, "Plan.md", "changed here\n");
    until(&mut session, &mut told, "the change", |_, t| {
        t.bar
            .as_ref()
            .is_some_and(|b| b.branch.as_deref() == Some("Branch trip-ideas · 1 change"))
    });
    menu(&mut session);
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::Local {
                name: "main".into()
            }
        ),
        SwitchAnswer::InTheWay {
            text:
                "Changes not committed would be overwritten: Plan.md. Commit them first, or stay."
                    .into()
        }
    );
    assert_eq!(read(&root, "Plan.md"), "changed here\n");
    let _ = fs::remove_dir_all(&dir);
}

/// TC-013-08 / M-19.6: a branch of the remote is listed after the local
/// ones and becomes a local branch following it.
#[test]
fn uc013_tc08_m19_6_remote_branch() {
    let remote = Remote::new("track", "", &[("Plan.md", PLAN)]);
    remote
        .config
        .run(&remote.b(), &["switch", "--quiet", "--create", "review"]);
    write(&remote.b(), "Review.md", "review\n");
    remote.config.run(&remote.b(), &["add", "--all"]);
    remote
        .config
        .run(&remote.b(), &["commit", "--quiet", "--message", "Review"]);
    remote.config.run(
        &remote.b(),
        &["push", "--quiet", "--set-upstream", "origin", "review"],
    );
    remote.config.run(&remote.a(), &["fetch", "--quiet"]);
    let mut session = remote.session();
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    // S1: no switch while the first sync runs.
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });

    let listed = menu(&mut session);
    let labels: Vec<&str> = listed.branches.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(
        labels,
        ["main  (current)", "origin/review  (on the remote)"]
    );
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::Remote {
                name: "origin/review".into()
            }
        ),
        SwitchAnswer::Done
    );
    assert_eq!(session.status(), "Switched to review");
    assert_eq!(read(&remote.a(), "Review.md"), "review\n");
}

/// TC-013-11 / M-19.6: while a merge waits, the menu says so and no switch
/// is made.
#[test]
fn uc013_tc11_m19_6_unfinished_merge() {
    let remote = Remote::new("merging", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);

    assert!(menu(&mut session).merging);
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::New {
                name: "elsewhere".into()
            }
        ),
        SwitchAnswer::Done
    );
    assert_eq!(
        remote
            .config
            .run(&remote.a(), &["branch", "--show-current"]),
        "main\n"
    );
}

/// Q-29 / UC-014 BR-008 / UC-004 A9: a key typed into a note while the
/// branch switches keeps the window's text; the branch's version is kept
/// as a copy, and the status says so.
#[test]
fn q29_uc013_m19_7_typing_during_a_switch_keeps_both() {
    let (dir, config, root) = branch_repo("typing");
    let mut session = session_on(&dir, &config, &root);
    session.open_note(&id("Plan"));
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    menu(&mut session);

    let job = session
        .switch_prepare(&BranchTarget::Local {
            name: "main".into(),
        })
        .unwrap();
    session
        .backend_edit(&id("Plan"), &[(0..0, "typed ".to_owned())], false)
        .unwrap();
    let outcome = job.run();
    session.finish_switch(outcome);

    assert_eq!(tab_text(&mut session, "Plan"), "typed draft plan\n");
    assert!(
        session
            .status()
            .starts_with("Plan was changed outside while you were editing; your text stays"),
        "{}",
        session.status()
    );
    let copies = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("Plan-changed-outside-")
        })
        .count();
    assert_eq!(copies, 1);
    let _ = fs::remove_dir_all(&dir);
}

/// TC-013-10 / TC-014 D-08 (A-141): a note that cannot be stored stops the
/// switch, and no sync starts.
#[cfg(unix)]
#[test]
fn uc013_tc10_d08_a_note_that_cannot_be_stored_stops_both() {
    use std::os::unix::fs::PermissionsExt;

    let remote = Remote::new("unstored", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    session.open_note(&id("Plan"));
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    menu(&mut session);
    session
        .backend_edit(&id("Plan"), &[(0..0, "x".to_owned())], false)
        .unwrap();
    let a = remote.a();
    fs::set_permissions(&a, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(a.join("probe"), "x").is_ok() {
        // Running as root: every folder can be written.
        fs::set_permissions(&a, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let answer = switch(
        &mut session,
        BranchTarget::New {
            name: "other".into(),
        },
    );
    let switch_status = session.status().to_owned();
    session.request_sync(SyncMode::Full, true);
    let sync_status = session.status().to_owned();
    let events = session.tick(Instant::now());
    told.take(events);
    fs::set_permissions(&a, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(answer, SwitchAnswer::Done);
    assert!(
        switch_status.starts_with("Did not switch, because Plan: "),
        "{switch_status}"
    );
    assert!(
        sync_status.starts_with("Not everything could be saved before the sync: Plan: "),
        "{sync_status}"
    );
    assert_ne!(told.sync_text(), "Syncing…");
}

/// TC-014-15 / M-19.8: closing syncs once more and closes when the sync
/// ended; a second close goes through at once.
#[test]
fn uc014_tc15_m19_8_closing_syncs_last() {
    let remote = Remote::new("closing", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    write(&remote.a(), "Plan.md", "closing\n");

    assert!(session.closing_sync(Instant::now()));
    assert_eq!(session.status(), "Syncing before closing…");
    assert!(!session.closing_sync_over(Instant::now()));
    assert!(!session.closing_sync(Instant::now()));
    until(&mut session, &mut told, "the last sync", |s, _| {
        s.closing_sync_over(Instant::now())
    });
    assert!(remote.remote_log().starts_with("Auto: Update Plan"));
    // Ten seconds at most.
    assert!(session.closing_sync_over(Instant::now() + super::CLOSING_SYNC_AT_MOST));
}

/// TC-014-07 / M-19.6, M-19.8: "Push" is refused by the remote when it
/// holds commits this branch lacks; "Pull" joins them.
#[test]
fn uc014_tc07_m19_8_pull_and_push() {
    let remote = Remote::new("pullpush", "", &[("Plan.md", PLAN), ("Ideas.md", "i\n")]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    write(&remote.b(), "Ideas.md", "from b\n");
    remote.push_from_b("From b");
    write(&remote.a(), "Plan.md", "local\n");

    session.request_sync(SyncMode::Push, true);
    until(&mut session, &mut told, "the refusal", |_, t| {
        t.sync_text() == "Push refused"
    });
    assert_eq!(
        session.status(),
        "The remote has commits this branch lacks; sync to join them first"
    );
    session.request_sync(SyncMode::Pull, true);
    until(&mut session, &mut told, "the pull", |_, t| {
        t.sync_text() == "Synced just now"
    });
    assert_eq!(read(&remote.a(), "Ideas.md"), "from b\n");
    let sync = told.bar.as_ref().unwrap().sync.clone().unwrap();
    assert_eq!(sync.to_push.as_deref(), Some("2 to push"));
}

/// A-145, UC-015 BR-001: a note whose file still holds places is never
/// marked resolved, even when its tab shows none (egui marked it after
/// storing the tab).
#[test]
fn a145_uc015_br001_a_file_with_places_is_not_marked() {
    let remote = Remote::new("guard", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let session = conflict_session(&remote, &mut told);
    let git = session.git_setup().git();

    let resolved = super::super::conflicts::resolve(
        &git,
        remote.a(),
        super::super::conflicts::Step::Mark(vec!["Plan.md".into()]),
    );

    assert!(resolved.result.is_err());
    let open = remote
        .config
        .run(&remote.a(), &["diff", "--name-only", "--diff-filter=U"]);
    assert_eq!(open, "Plan.md\n");
}

/// TC-014-06 / TC-015-09 / M-19.9: with `pull.rebase`, "Here" is still this
/// computer's version, and completing goes on with the rebase.
#[test]
fn uc015_tc09_m19_9_rebase_keeps_here_as_this_computer() {
    let remote = Remote::new("rebase", "[pull]\n\trebase = true\n", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);

    let place = &told.conflict.clone().unwrap().places[0];
    assert_eq!(
        (place.here.as_str(), place.remote.as_str()),
        ("two from a", "TWO from b")
    );
    let r = rev(&mut session, "Plan");
    session.conflict_choose(&id("Plan"), r, None, ChoiceName::Here);
    until(&mut session, &mut told, "the conflicts resolved", |s, _| {
        s.status() == "The conflicts are resolved; syncing"
    });
    until(
        &mut session,
        &mut told,
        "the sync after resolving",
        |_, t| t.sync_text() == "Synced just now",
    );
    let log = remote.remote_log();
    assert!(!log.contains("Merge"), "{log}");
    assert_eq!(read(&remote.a(), "Plan.md"), "one\ntwo from a\nthree\n");
}

/// M1 (review F13) / UC-013 BR-003: a note only the old branch holds, typed
/// into while git switches, keeps its tab and text and is stored again.
#[test]
fn m1_uc013_m19_7_a_note_typed_into_during_a_switch_is_never_dropped() {
    let (dir, config, root) = branch_repo("m1");
    let mut session = session_on(&dir, &config, &root);
    session.open_note(&id("Sketch"));
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    menu(&mut session);

    let job = session
        .switch_prepare(&BranchTarget::Local {
            name: "main".into(),
        })
        .unwrap();
    session
        .backend_edit(&id("Sketch"), &[(0..0, "typed ".to_owned())], false)
        .unwrap();
    session.finish_switch(job.run());

    assert_eq!(tab_text(&mut session, "Sketch"), "typed sketch\n");
    assert!(
        session
            .status()
            .contains("kept Sketch open with changes not stored, not on this branch"),
        "{}",
        session.status()
    );
    until(&mut session, &mut told, "Sketch stored again", |_, _| {
        root.join("Sketch.md").exists()
    });
    assert_eq!(read(&root, "Sketch.md"), "typed sketch\n");
    let _ = fs::remove_dir_all(&dir);
}

/// S6 (review F13): only a listed branch is switched to; `--detach` never
/// reaches git.
#[test]
fn s6_uc013_m19_7_only_listed_branches() {
    let (dir, config, root) = branch_repo("listed");
    let mut session = session_on(&dir, &config, &root);
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    menu(&mut session);

    for target in [
        BranchTarget::Local {
            name: "--detach".into(),
        },
        BranchTarget::Remote {
            name: "origin/--detach".into(),
        },
    ] {
        assert_eq!(switch(&mut session, target), SwitchAnswer::Done);
    }
    assert_eq!(config.run(&root, &["branch", "--show-current"]), "drafts\n");
    let _ = fs::remove_dir_all(&dir);
}

/// S1 / S4 (review F13): no switch while a sync runs; a sync asked for
/// while one runs waits and runs once after it, and closing waits for both.
#[test]
fn s1_s4_uc014_a8_one_git_job_at_a_time() {
    let remote = Remote::new("exclusive", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    menu(&mut session);

    session.request_sync(SyncMode::Full, true);
    session.request_sync(SyncMode::Pull, true);
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::New {
                name: "other".into()
            }
        ),
        SwitchAnswer::Done
    );
    assert_eq!(
        session.status(),
        "Did not switch, because a sync is running; try again when it ended"
    );
    assert!(session.closing_sync(Instant::now()));
    // The second sync is still to run when the first ends.
    let mut syncs = 0;
    let mut was_running = true;
    until(&mut session, &mut told, "both syncs", |s, t| {
        let running = t.sync_text() == "Syncing…";
        if was_running && !running {
            syncs += 1;
        }
        was_running = running;
        s.closing_sync_over(Instant::now())
    });
    assert!(syncs >= 1);
    assert!(!session.sync_running());
}

/// S3 (review F13): a choice made on an older text, or made twice, changes
/// nothing.
#[test]
fn s3_uc015_m19_9_a_choice_of_an_older_text_does_nothing() {
    let remote = Remote::new("stale", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);
    let before = tab_text(&mut session, "Plan");
    let r = rev(&mut session, "Plan");

    session.conflict_choose(&id("Plan"), r + 1, Some(0), ChoiceName::Here);
    assert_eq!(tab_text(&mut session, "Plan"), before);
    session.conflict_choose(&id("Plan"), r, Some(0), ChoiceName::Both);
    let once = tab_text(&mut session, "Plan");
    session.conflict_choose(&id("Plan"), r, Some(0), ChoiceName::Both);
    assert_eq!(tab_text(&mut session, "Plan"), once);
    assert_eq!(once, "one\ntwo from a\nTWO from b\nthree\n");
}

/// S5 (review F13): a tab showing an older text without places, while its
/// file has them, takes the file's text and is never marked resolved.
#[test]
fn s5_uc015_br001_a_stale_tab_takes_the_file_first() {
    let remote = Remote::new("staletab", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);
    {
        let open = session.open.as_mut().unwrap();
        let index = open.tabs.find(&id("Plan")).unwrap();
        open.tabs.get_mut(index).reload("one\ntwo\nthree\n".into());
    }

    until(&mut session, &mut told, "the file's places back", |s, _| {
        s.open.as_ref().is_some_and(|open| {
            let tab = open.tabs.get(open.tabs.find(&id("Plan")).unwrap());
            tab.mirror.text().contains("<<<<<<<")
        })
    });
    assert!(session.in_conflict(&id("Plan")));
    assert!(read(&remote.a(), "Plan.md").contains("<<<<<<<"));
}

/// S7 (review F13): a sync that does not end is given up and reported.
#[test]
fn s7_uc014_a7_a_sync_that_hangs_is_given_up() {
    let remote = Remote::new("hang", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    session.request_sync(SyncMode::Full, true);
    session.sync_started_before(super::SYNC_GIVEN_UP);

    let events = session.tick(Instant::now());
    told.take(events);

    let sync = told.bar.unwrap().sync.unwrap();
    assert_eq!(sync.text, "Sync failed");
    assert_eq!(
        sync.hover.as_deref(),
        Some("it took longer than 10 minutes and was given up")
    );
    assert!(!sync.running && sync.can_sync);
}

/// S8 (review F13): a token in a remote's address never reaches a text.
#[test]
fn s8_a_token_in_an_address_is_redacted() {
    assert_eq!(
        super::redact("fatal: unable to access 'https://me:ghp_secret@example.com/x.git/': 403"),
        "fatal: unable to access 'https://example.com/x.git/': 403"
    );
    assert_eq!(
        super::redact("ssh://git@host:22/repo and http://a@b c"),
        "ssh://host:22/repo and http://b c"
    );
    assert_eq!(super::redact("no address"), "no address");
}

/// K5 (review F13): closing says "Syncing before closing…" only when a
/// sync runs.
#[test]
fn k5_uc014_a9_no_closing_sync_while_conflicts_wait() {
    let remote = Remote::new("noclose", "", &[("Plan.md", PLAN)]);
    let mut told = Told::default();
    let mut session = conflict_session(&remote, &mut told);
    let said = session.status().to_owned();

    assert!(!session.closing_sync(Instant::now()));
    assert_eq!(session.status(), said);
}

/// Round 2 of review F13, S7: a sync given up still runs in git (a remote
/// that hangs for 3 s): saves keep waiting, no switch starts, a new sync
/// waits for the old outcome and then runs.
#[test]
fn r2_s7_a_sync_given_up_still_holds_git() {
    let remote = Remote::new("givenup", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    menu(&mut session);
    let a = remote.a();
    let bare = remote.dir.join("remote.git");
    remote
        .config
        .run(&a, &["config", "protocol.ext.allow", "always"]);
    remote
        .config
        .run(&a, &["remote", "set-url", "origin", "ext::sh -c sleep% 3"]);

    session.request_sync(SyncMode::Full, true);
    session.sync_started_before(super::SYNC_GIVEN_UP);
    told.take(session.tick(Instant::now()));

    assert_eq!(told.sync_text(), "Sync failed");
    assert_eq!(
        told.bar
            .as_ref()
            .unwrap()
            .sync
            .as_ref()
            .unwrap()
            .hover
            .as_deref(),
        Some("it took longer than 10 minutes and was given up")
    );
    assert!(session.sync_running());
    assert!(session
        .saves_paused
        .contains(&super::super::branches::Pause::Sync));
    assert_eq!(
        switch(
            &mut session,
            BranchTarget::New {
                name: "other".into()
            }
        ),
        SwitchAnswer::Done
    );
    session.request_sync(SyncMode::Full, true);
    remote
        .config
        .run(&a, &["remote", "set-url", "origin", bare.to_str().unwrap()]);
    until(&mut session, &mut told, "the waiting sync", |s, t| {
        !s.sync_running() && t.sync_text() == "Synced just now"
    });
    assert!(session.saves_paused.is_empty());
}

/// Round 2 of review F13: a sync waiting behind steps of resolving runs
/// after them; the steps never wait for it.
#[test]
fn r2_s1_a_waiting_sync_runs_after_the_steps() {
    let remote = Remote::new("steps", "", &[("Plan.md", PLAN)]);
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync", |s, t| {
        s.syncs() && t.sync_text() == "Synced just now"
    });
    session.queue_step(super::super::conflicts::Step::Complete);
    session.queue_step(super::super::conflicts::Step::Complete);
    session.request_sync(SyncMode::Full, true);
    assert!(session.sync_running() && !session.sync_in_flight());

    let mut saw_syncing = false;
    until(
        &mut session,
        &mut told,
        "the steps, then the sync",
        |s, t| {
            saw_syncing |= t.sync_text() == "Syncing…";
            !s.resolving() && !s.sync_running() && saw_syncing
        },
    );
}

/// Round 2 of review F13: a sync asked for while one meets a conflict does
/// not replace "Sync stopped: conflict" with "No sync".
#[test]
fn r2_a_waiting_sync_is_dropped_after_a_conflict() {
    let remote = Remote::new("dropped", "", &[("Plan.md", PLAN)]);
    write(&remote.b(), "Plan.md", "one\nTWO from b\nthree\n");
    remote.push_from_b("From b");
    write(&remote.a(), "Plan.md", "one\ntwo from a\nthree\n");
    let mut session = remote.session();
    let mut told = Told::default();
    until(&mut session, &mut told, "the first sync running", |s, _| {
        s.sync_in_flight()
    });
    session.request_sync(SyncMode::Full, false);

    until(&mut session, &mut told, "the conflict", |_, t| {
        t.sync_text() == "Sync stopped: conflict"
    });
    for _ in 0..20 {
        told.take(session.tick(Instant::now()));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(told.sync_text(), "Sync stopped: conflict");
    assert!(!session.sync_running());
}

/// Round 2 of review F13, M1: a note in a folder the new branch does not
/// have is stored again with its folder.
#[test]
fn r2_m1_a_note_in_a_gone_folder_is_stored_again() {
    let (dir, config, root) = branch_repo("m1deep");
    write(&root, "deep/Inner.md", "inner\n");
    config.run(&root, &["add", "--all"]);
    config.run(&root, &["commit", "--quiet", "--message", "Deep"]);
    let mut session = session_on(&dir, &config, &root);
    session.open_note(&id("deep/Inner"));
    let mut told = Told::default();
    tracked(&mut session, &mut told);
    menu(&mut session);

    let job = session
        .switch_prepare(&BranchTarget::Local {
            name: "main".into(),
        })
        .unwrap();
    session
        .backend_edit(&id("deep/Inner"), &[(0..0, "typed ".to_owned())], false)
        .unwrap();
    session.finish_switch(job.run());
    assert!(!root.join("deep").exists());

    until(&mut session, &mut told, "Inner stored again", |_, _| {
        root.join("deep/Inner.md").exists()
    });
    assert_eq!(read(&root, "deep/Inner.md"), "typed inner\n");
    let _ = fs::remove_dir_all(&dir);
}
