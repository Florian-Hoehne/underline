//! Tests of the git changes (UC-011, UC-012, inventory M-19.1 to M-19.5,
//! M-05.3), against what the egui app does (`changes_ui`, `track_changes`,
//! `commit_changes`, `show_identity`, `start_discard`, `discard_change` of
//! `apps/underline-desktop/src/app.rs`).
//!
//! Each test makes a real repository with `git init` in a temporary folder.
//! git reads only a configuration of the test's own (`GIT_CONFIG_GLOBAL`,
//! `GIT_CONFIG_NOSYSTEM`, `HOME`), so the user's git settings are neither
//! read nor changed; the name and email of the commits are set in the
//! repository itself, except where git is to not know the user (A3).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::{NEEDS_MESSAGE, NOTHING_CHOSEN};
use crate::git_view::{
    ChangeKindName, ChangesView, CommitAnswer, DiffBody, DiffLineKind, DiffView,
};
use crate::{Event, Session, SettingsState};

/// How long a test waits for git's answer.
const WAIT: Duration = Duration::from_secs(20);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-git-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The git configuration of a test: its own global file in `dir/home`.
struct Config {
    home: PathBuf,
    file: PathBuf,
}

impl Config {
    fn new(dir: &Path, content: &str) -> Self {
        let home = dir.join("home");
        fs::create_dir_all(&home).unwrap();
        let file = home.join("gitconfig");
        fs::write(&file, content).unwrap();
        Self { home, file }
    }

    /// The environment of the session's git, reading only this
    /// configuration.
    fn env(&self) -> Vec<(&'static str, &std::ffi::OsStr)> {
        vec![
            ("GIT_CONFIG_GLOBAL", self.file.as_os_str()),
            ("GIT_CONFIG_NOSYSTEM", std::ffi::OsStr::new("1")),
            ("HOME", self.home.as_os_str()),
        ]
    }

    /// Runs git in `dir` with this configuration; its output.
    fn run(&self, dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", &self.file)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.home)
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

/// The global configuration of most tests: the first branch is `main`.
const MAIN: &str = "[init]\n\tdefaultBranch = main\n";

/// A repository at `dir/notes` with `committed` in its first commit, the
/// name and email set in the repository, then `changed` applied (`None`
/// deletes the file); a session on it, the first note in a tab.
fn repo_session(
    dir: &Path,
    config: &Config,
    committed: &[(&str, &str)],
    changed: &[(&str, Option<&str>)],
) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    config.run(&root, &["init", "--quiet"]);
    config.run(&root, &["config", "user.name", "Test"]);
    config.run(&root, &["config", "user.email", "test@example.com"]);
    for (name, text) in committed {
        write(&root, name, text);
    }
    config.run(&root, &["add", "--all"]);
    config.run(&root, &["commit", "--quiet", "--message", "First"]);
    for (name, text) in changed {
        match text {
            Some(text) => write(&root, name, text),
            None => fs::remove_file(root.join(name)).unwrap(),
        }
    }
    (open_session(dir, config, &root), root)
}

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A session on the workspace `root`, git reading only `config`.
fn open_session(dir: &Path, config: &Config, root: &Path) -> Session {
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

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// Ticks until the window is told changes that `fits`; those changes.
fn wait_changes(session: &mut Session, fits: impl Fn(&ChangesView) -> bool) -> ChangesView {
    let start = Instant::now();
    let mut last = None;
    while start.elapsed() < WAIT {
        for event in session.tick(Instant::now()) {
            if let Event::Changes(Some(view)) = event {
                if fits(&view) {
                    return view;
                }
                last = Some(view);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no fitting changes came; the last were {last:?}");
}

/// Ticks until the window is told a diff that `fits`; that diff.
fn wait_diff(session: &mut Session, fits: impl Fn(&DiffView) -> bool) -> DiffView {
    let start = Instant::now();
    let mut last = None;
    while start.elapsed() < WAIT {
        for event in session.tick(Instant::now()) {
            if let Event::Diff(Some(view)) = event {
                if fits(&view) {
                    return view;
                }
                last = Some(view);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no fitting diff came; the last was {last:?}");
}

/// The paths, letters and names of `view`.
fn rows(view: &ChangesView) -> Vec<(String, String, String)> {
    view.files
        .iter()
        .map(|f| (f.path.clone(), f.letter.clone(), f.name.clone()))
        .collect()
}

/// Commits as the command does: prepared under the session, run without
/// it, finished under it.
fn commit(session: &mut Session, message: &str, amend: bool) -> CommitAnswer {
    match session.commit_prepare(message, amend) {
        Ok(job) => session.finish_commit(job.run()),
        Err(answer) => answer,
    }
}

/// The texts of `diff`'s lines with their kinds.
fn diff_lines(diff: &DiffView) -> Vec<(DiffLineKind, String)> {
    match &diff.body {
        DiffBody::Hunks { hunks } => hunks
            .iter()
            .flat_map(|h| &h.lines)
            .map(|l| {
                let text: String = l.parts.iter().map(|p| p.text.as_str()).collect();
                (l.kind, text)
            })
            .collect(),
        other => panic!("no hunks: {other:?}"),
    }
}

const PLAN: &str = "# Plan\n\none\ntwo\nthree\nfour\nfive\nsix\nseven\n";

#[test]
fn uc011_m19_2_lists_conflicts_notes_then_other_files_with_their_marks() {
    let dir = temp_dir("list");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(
        &dir,
        &config,
        &[
            ("Plan.md", PLAN),
            ("Gone.md", "# Gone\n"),
            ("guide/Old.md", "# Old\n\na\nb\nc\nd\ne\n"),
            ("notes.txt", "text\n"),
        ],
        &[
            (
                "Plan.md",
                Some("# Plan\n\none\nTWO\nthree\nfour\nfive\nsix\nseven\n"),
            ),
            ("Gone.md", None),
            ("guide/Old.md", None),
            ("guide/New.md", Some("# Old\n\na\nb\nc\nd\ne\n")),
            ("Fresh.md", Some("# Fresh\n")),
            ("notes.txt", Some("text, changed\n")),
        ],
    );
    assert!(session.tracks_changes());

    let view = wait_changes(&mut session, |v| v.files.len() == 5);

    assert_eq!(
        rows(&view),
        [
            ("Fresh.md".into(), "N".into(), "Fresh".into()),
            ("Gone.md".into(), "D".into(), "Gone".into()),
            ("guide/New.md".into(), "R".into(), "New ← Old".into()),
            ("Plan.md".into(), "M".into(), "Plan".into()),
            ("notes.txt".into(), "M".into(), "notes.txt".into()),
        ]
    );
    assert_eq!(view.files[2].kind, ChangeKindName::Renamed);
    assert!(view.files[..4].iter().all(|f| f.note));
    assert!(!view.files[4].note, "the other file comes after the notes");
    assert!(view.files.iter().all(|f| f.chosen), "BR-004: all chosen");
    assert_eq!(view.branch.as_ref().unwrap().label, "main");
    assert_eq!(view.problem, None);
    assert!(!view.merging);
    // UC-012 BR-002.
    assert_eq!(
        view.proposed,
        "Update Plan; Add Fresh; Delete Gone; Rename Old to New and 1 other file"
    );
    assert_eq!(view.amendable.as_deref(), Some("First"));
}

#[test]
fn uc011_m05_3_the_overview_marks_changed_notes_and_unstored_tabs() {
    let dir = temp_dir("marks");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(
        &dir,
        &config,
        &[
            ("Plan.md", PLAN),
            ("Same.md", "# Same\n"),
            ("Typed.md", "# Typed\n"),
        ],
        &[("Fresh.md", Some("# Fresh\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);
    // A tab with a change not stored yet is marked `M` (egui `change_marks`).
    session.open_note(&id("Typed"));
    session.window_type(&id("Typed"), 0, "x");

    let mark = |session: &Session, note: &str| {
        session
            .snapshot()
            .overview
            .into_iter()
            .find(|item| item.note == note)
            .unwrap()
            .mark
    };
    let start = Instant::now();
    while mark(&session, "Fresh").is_none() && start.elapsed() < WAIT {
        session.tick(Instant::now());
    }
    session.tick(Instant::now());

    assert_eq!(mark(&session, "Fresh").as_deref(), Some("N"));
    assert_eq!(mark(&session, "Typed").as_deref(), Some("M"));
    assert_eq!(mark(&session, "Same"), None);
}

#[test]
fn uc011_a1_nothing_changed_lists_nothing_and_a_workspace_without_git_tracks_nothing() {
    let dir = temp_dir("nothing");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(&dir, &config, &[("Plan.md", PLAN)], &[]);

    let view = wait_changes(&mut session, |_| true);

    assert!(view.files.is_empty());
    assert_eq!(view.proposed, "");

    let plain = dir.join("plain");
    write(&plain, "A.md", "# A\n");
    let mut session = open_session(&dir, &config, &plain);
    assert!(!session.tracks_changes());
    let events = session.tick(Instant::now());
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Changes(_) | Event::Diff(_))),
        "{events:?}"
    );
}

#[test]
fn uc011_a7_no_commit_yet_names_the_branch_to_be_and_every_note_is_new() {
    let dir = temp_dir("no-commit");
    let config = Config::new(&dir, MAIN);
    let root = dir.join("notes");
    write(&root, "Plan.md", PLAN);
    config.run(&root, &["init", "--quiet"]);
    let mut session = open_session(&dir, &config, &root);

    let view = wait_changes(&mut session, |v| !v.files.is_empty());

    assert_eq!(view.branch.unwrap().label, "main (no commit yet)");
    assert_eq!(view.files[0].letter, "N");
    assert_eq!(view.amendable, None);
}

#[test]
fn uc011_a9_a_broken_repository_says_why_and_shows_nothing_else() {
    let dir = temp_dir("broken");
    let config = Config::new(&dir, MAIN);
    let root = dir.join("notes");
    write(&root, "Plan.md", PLAN);
    // A `.git` that is no repository.
    fs::create_dir_all(root.join(".git")).unwrap();
    let mut session = open_session(&dir, &config, &root);

    let view = wait_changes(&mut session, |v| v.problem.is_some());

    assert!(view.files.is_empty());
    assert_eq!(view.branch, None);
    assert!(view.problem.unwrap().contains("git"), "in git's words");
}

#[test]
fn uc011_m19_3_the_diff_of_the_active_tab_follows_its_unsaved_text() {
    let dir = temp_dir("diff-tab");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(&dir, &config, &[("Plan.md", PLAN)], &[]);
    session.open_note(&id("Plan"));

    // A2: unchanged.
    let diff = wait_diff(&mut session, |_| true);
    assert_eq!(diff.path, "Plan.md");
    assert_eq!(diff.note.as_deref(), Some("Plan"));
    assert_eq!(diff.body, DiffBody::Unchanged);

    // BR-001: the tab's text, with changes not stored.
    let at = "# Plan\n\none\n".chars().count();
    session.window_edit(&id("Plan"), at, 3, "TWO");
    let rev = session.mirror_rev(&id("Plan"));
    let diff = wait_diff(&mut session, |d| d.rev == Some(rev));

    let lines = diff_lines(&diff);
    assert_eq!(
        lines,
        [
            (DiffLineKind::Same, "# Plan".into()),
            (DiffLineKind::Same, String::new()),
            (DiffLineKind::Same, "one".into()),
            (DiffLineKind::Removed, "two".into()),
            (DiffLineKind::Added, "TWO".into()),
            (DiffLineKind::Same, "three".into()),
            (DiffLineKind::Same, "four".into()),
            (DiffLineKind::Same, "five".into()),
        ],
        "BR-004: three lines around the change"
    );
    let DiffBody::Hunks { hunks } = &diff.body else {
        unreachable!()
    };
    let added = &hunks[0].lines[4];
    assert_eq!(added.numbers, "        4 + ");
    let removed = &hunks[0].lines[3];
    assert_eq!(removed.numbers, "   4      - ");
    // The words changed are marked strongly.
    assert!(added.parts.iter().any(|p| p.strong && p.text == "TWO"));
    // Steps 7–8: a click puts the cursor at the start of line 4.
    assert_eq!(added.offset, Some(u32::try_from(at).unwrap()));
}

#[test]
fn uc011_a4_a5_a_deleted_note_and_another_file_are_shown_without_a_tab() {
    let dir = temp_dir("diff-chosen");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN), ("Gone.md", "# Gone\nline\n")],
        &[("Gone.md", None)],
    );
    let root = session.workspace_root().unwrap().to_owned();
    fs::write(root.join("logo.png"), [0x89, b'P', b'N', b'G', 0xff, 0xfe]).unwrap();
    wait_changes(&mut session, |v| v.files.len() == 2);

    session.choose_change("Gone.md");
    let diff = wait_diff(&mut session, |d| d.path == "Gone.md");
    assert_eq!(diff.note, None);
    assert_eq!(
        diff_lines(&diff),
        [
            (DiffLineKind::Removed, "# Gone".into()),
            (DiffLineKind::Removed, "line".into()),
        ]
    );
    assert!(session.snapshot().tabs.tabs.len() <= 1, "no tab opened");

    session.choose_change("logo.png");
    let diff = wait_diff(&mut session, |d| d.path == "logo.png");
    assert_eq!(
        diff.body,
        DiffBody::NotText {
            text: "This file is not text; it is new.".into()
        }
    );

    // Step 5: a changed note opens in a tab, whose diff is shown.
    write(&root, "Plan.md", "# Plan\n");
    wait_changes(&mut session, |v| v.files.len() == 3);
    session.choose_change("Plan.md");
    assert_eq!(session.snapshot().tabs.active.as_deref(), Some("Plan"));
    wait_diff(&mut session, |d| d.path == "Plan.md");
}

#[test]
fn uc012_a1_a2_m19_4_a_commit_needs_a_message_and_a_choice() {
    let dir = temp_dir("refused");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Plan.md", Some("# P\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);

    assert_eq!(
        commit(&mut session, "  \n ", false),
        CommitAnswer::Problem {
            problem: NEEDS_MESSAGE.into()
        }
    );
    session.choose_for_commit(None, false);
    let view = wait_changes(&mut session, |v| v.files.iter().all(|f| !f.chosen));
    assert_eq!(view.proposed, "");
    assert_eq!(
        commit(&mut session, "Update", false),
        CommitAnswer::Problem {
            problem: NOTHING_CHOSEN.into()
        }
    );
    session.choose_for_commit(None, true);
    wait_changes(&mut session, |v| v.files.iter().all(|f| f.chosen));
}

#[test]
fn uc012_m19_4_commits_the_chosen_changes_with_the_unsaved_text() {
    let dir = temp_dir("commit");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN), ("Other.md", "# Other\n")],
        &[
            ("Other.md", Some("# Other, changed\n")),
            ("Fresh.md", Some("# Fresh\n")),
        ],
    );
    session.open_note(&id("Plan"));
    wait_changes(&mut session, |v| v.files.len() == 2);
    // BR-001: the tab's text not stored yet is committed.
    session.window_type(&id("Plan"), 0, "Typed. ");
    wait_changes(&mut session, |v| v.files.len() == 3);
    // BR-004: one left out.
    session.choose_for_commit(Some("Fresh.md"), false);
    let view = wait_changes(&mut session, |v| v.files.iter().any(|f| !f.chosen));
    assert_eq!(view.proposed, "Update Other, Plan");

    let answer = commit(&mut session, "  Update the plan  ", false);

    assert_eq!(answer, CommitAnswer::Committed);
    let id_now = config.run(&root, &["rev-parse", "--short", "HEAD"]);
    assert_eq!(
        session.status(),
        format!("Committed {} holding 2 notes", id_now.trim())
    );
    assert_eq!(
        config.run(&root, &["log", "-1", "--format=%B"]).trim(),
        "Update the plan"
    );
    assert!(config
        .run(&root, &["show", "HEAD:Plan.md"])
        .starts_with("Typed. # Plan"));
    // The change left out is still changed and chosen again.
    let view = wait_changes(&mut session, |v| v.files.len() == 1);
    assert_eq!(view.files[0].path, "Fresh.md");
    assert!(view.files[0].chosen);
}

#[test]
fn uc012_a6_amends_the_last_commit() {
    let dir = temp_dir("amend");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Fresh.md", Some("# F\n"))],
    );
    let view = wait_changes(&mut session, |v| v.files.len() == 1);
    assert_eq!(view.amendable.as_deref(), Some("First"));

    assert_eq!(
        commit(&mut session, "First, amended", true),
        CommitAnswer::Committed
    );

    let id_now = config.run(&root, &["rev-parse", "--short", "HEAD"]);
    assert_eq!(
        session.status(),
        format!(
            "Amended the last commit, now {} holding 1 note",
            id_now.trim()
        )
    );
    assert_eq!(
        config.run(&root, &["rev-list", "--count", "HEAD"]).trim(),
        "1"
    );
}

#[test]
fn uc012_a4_a_refused_commit_says_why_in_gits_words() {
    let dir = temp_dir("hook");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Plan.md", Some("# P\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);
    let hook = root.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'checks failed' >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }

    assert_eq!(commit(&mut session, "Update", false), CommitAnswer::Done);

    assert!(
        session.status().starts_with("The commit failed: "),
        "{}",
        session.status()
    );
    assert!(session.status().contains("checks failed"));
}

#[test]
fn uc012_a3_m19_4_unknown_user_is_asked_and_kept_in_the_users_git_settings() {
    let dir = temp_dir("identity");
    // No name and email anywhere, and git is not to guess them.
    let config = Config::new(
        &dir,
        "[init]\n\tdefaultBranch = main\n[user]\n\tuseConfigOnly = true\n",
    );
    let root = dir.join("notes");
    write(&root, "Plan.md", PLAN);
    config.run(&root, &["init", "--quiet"]);
    let mut session = open_session(&dir, &config, &root);
    wait_changes(&mut session, |v| v.files.len() == 1);

    assert_eq!(commit(&mut session, "First", false), CommitAnswer::Identity);
    // The button is enabled only with a name and an email with '@'.
    assert!(session.identity_prepare(" ", "a@b").is_none());
    assert!(session.identity_prepare("Ann", "ann").is_none());

    session
        .identity_prepare(" Ann ", " ann@example.com ")
        .unwrap()
        .run()
        .unwrap();
    assert_eq!(
        commit(&mut session, "First", false),
        CommitAnswer::Committed
    );

    let kept = fs::read_to_string(&config.file).unwrap();
    assert!(kept.contains("name = Ann") && kept.contains("email = ann@example.com"));
    assert_eq!(
        config
            .run(&root, &["log", "-1", "--format=%an <%ae>"])
            .trim(),
        "Ann <ann@example.com>"
    );
}

#[test]
fn uc012_a5_a_note_that_cannot_be_stored_stops_the_commit() {
    let dir = temp_dir("unstored");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Fresh.md", Some("# F\n"))],
    );
    session.open_note(&id("Plan"));
    wait_changes(&mut session, |v| v.files.len() == 1);
    // The note's place is taken by a folder, so it cannot be written.
    fs::remove_file(root.join("Plan.md")).unwrap();
    fs::create_dir_all(root.join("Plan.md")).unwrap();
    session.window_type(&id("Plan"), 0, "x");

    assert_eq!(commit(&mut session, "Update", false), CommitAnswer::Done);

    assert!(
        session
            .status()
            .starts_with("Nothing was committed, because Plan: "),
        "{}",
        session.status()
    );
}

#[test]
fn uc011_a6_m19_5_discards_a_changed_note_also_in_its_tab() {
    let dir = temp_dir("discard-changed");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(&dir, &config, &[("Plan.md", PLAN)], &[]);
    session.open_note(&id("Plan"));
    let at = "# Plan\n\none\n".chars().count();
    session.window_edit(&id("Plan"), at, 3, "TWO");
    wait_changes(&mut session, |v| v.files.len() == 1);

    let question = session
        .discard_start("Plan.md")
        .unwrap()
        .question()
        .unwrap();
    assert_eq!(
        question.question,
        "Discard the changes of Plan since the last commit?"
    );
    assert_eq!(
        question.consequence,
        "2 changed lines will be lost and cannot be restored."
    );

    let job = session.discard_prepare("Plan.md").unwrap();
    session.finish_discard(job.run());

    assert_eq!(session.status(), "Discarded the changes of Plan");
    assert_eq!(fs::read_to_string(root.join("Plan.md")).unwrap(), PLAN);
    assert_eq!(session.window_text(&id("Plan")), PLAN);
    let events = session.tick(Instant::now());
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Changed(c) if c.note == "Plan")),
        "the window's core fetches the text: {events:?}"
    );
    wait_changes(&mut session, |v| v.files.is_empty());
    // The tab holds nothing to store any more.
    assert!(!session.snapshot().tabs.tabs[0].label.ends_with('•'));
}

#[test]
fn uc011_a6_m19_5_discards_a_new_a_deleted_and_a_renamed_note() {
    let dir = temp_dir("discard-kinds");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[
            ("Plan.md", PLAN),
            ("Gone.md", "# Gone\n"),
            ("Old.md", "# Old\n\na\nb\nc\n"),
        ],
        &[
            ("Fresh.md", Some("# Fresh\n")),
            ("Gone.md", None),
            ("Old.md", None),
            ("New.md", Some("# Old\n\na\nb\nc\n")),
        ],
    );
    session.open_note(&id("Fresh"));
    session.open_note(&id("New"));
    wait_changes(&mut session, |v| v.files.len() == 3);
    let ask = |session: &mut Session, path: &str| {
        session.discard_start(path).unwrap().question().unwrap()
    };

    let new = ask(&mut session, "Fresh.md");
    assert_eq!(
        new.consequence,
        "The note is new; it goes to the deleted notes."
    );
    let job = session.discard_prepare("Fresh.md").unwrap();
    session.finish_discard(job.run());
    assert_eq!(session.status(), "Discarded the changes of Fresh");
    assert!(!root.join("Fresh.md").exists());
    assert!(session.deleted_view().iter().any(|d| d.title == "Fresh"));

    let gone = ask(&mut session, "Gone.md");
    assert_eq!(gone.consequence, "The note comes back as it was committed.");
    let job = session.discard_prepare("Gone.md").unwrap();
    session.finish_discard(job.run());
    assert_eq!(
        fs::read_to_string(root.join("Gone.md")).unwrap(),
        "# Gone\n"
    );

    ask(&mut session, "New.md");
    let job = session.discard_prepare("New.md").unwrap();
    session.finish_discard(job.run());
    assert_eq!(session.status(), "Discarded the changes of New");
    assert!(!root.join("New.md").exists());
    assert!(root.join("Old.md").exists());
    let tabs: Vec<_> = session
        .snapshot()
        .tabs
        .tabs
        .into_iter()
        .map(|t| t.note)
        .collect();
    assert!(
        !tabs.contains(&"New".to_owned()) && !tabs.contains(&"Fresh".to_owned()),
        "{tabs:?}"
    );
    wait_changes(&mut session, |v| v.files.is_empty());
}

#[test]
fn uc011_a6_only_the_change_asked_about_is_discarded() {
    let dir = temp_dir("discard-asked");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Plan.md", Some("# P\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);

    // Not asked: nothing happens.
    assert!(session.discard_prepare("Plan.md").is_none());
    assert_eq!(fs::read_to_string(root.join("Plan.md")).unwrap(), "# P\n");
    // Another file than the one asked about: nothing happens either.
    session.discard_start("Plan.md").unwrap();
    assert!(session.discard_prepare("Other.md").is_none());
    assert_eq!(fs::read_to_string(root.join("Plan.md")).unwrap(), "# P\n");
}

#[test]
fn uc011_m16_4_opening_a_workspace_without_git_ends_the_changes_shown() {
    let dir = temp_dir("switch");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Fresh.md", Some("# F\n"))],
    );
    session.open_note(&id("Plan"));
    wait_changes(&mut session, |v| v.files.len() == 1);
    let start = Instant::now();
    while session.changes.told_diff.is_none() && start.elapsed() < WAIT {
        session.tick(Instant::now());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(session.changes.told_diff.is_some());
    let plain = dir.join("plain");
    write(&plain, "A.md", "# A\n");

    session.open_chosen(&plain, false);
    let events = session.tick(Instant::now());

    assert!(events.contains(&Event::Changes(None)), "{events:?}");
    assert!(events.contains(&Event::Diff(None)), "{events:?}");
    assert!(session
        .snapshot()
        .overview
        .iter()
        .all(|item| item.mark.is_none()));
}

/// A pre-commit check that refuses every commit.
fn refuse_commits(root: &Path) {
    let hook = root.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'checks failed' >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn review_f12_a_panicked_diff_job_says_so_and_a_silent_one_is_given_up() {
    let dir = temp_dir("diff-panic");
    let config = Config::new(&dir, MAIN);
    let (mut session, _) = repo_session(&dir, &config, &[("Plan.md", PLAN)], &[]);
    let started = Instant::now();
    session.changes.diffs.running = Some(super::Running {
        seq: 77,
        made_for: ("Plan.md".into(), Some(1), 0),
        started,
    });

    session.changes.diffs.send.send((77, None, None)).unwrap();
    session.take_diffs(started);

    assert!(session.changes.diffs.running.is_none());
    let shown = &session.changes.diffs.shown.as_ref().unwrap().view;
    assert_eq!(
        shown.body,
        DiffBody::Problem {
            text: "The diff cannot be shown: it stopped unexpectedly".into()
        }
    );

    // A job that never answers is given up, so the next one can start.
    session.changes.diffs.running = Some(super::Running {
        seq: 78,
        made_for: ("Plan.md".into(), Some(1), 0),
        started,
    });
    session.take_diffs(started + super::DIFF_GIVEN_UP);
    assert!(session.changes.diffs.running.is_none());
}

#[test]
fn review_f12_a_failure_of_git_is_not_taken_for_a_note_never_committed() {
    let dir = temp_dir("git-fails");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Plan.md", Some("# P\n"))],
    );
    session.open_note(&id("Plan"));
    wait_changes(&mut session, |v| v.files.len() == 1);
    let plan = session.discard_start("Plan.md").unwrap();
    // The objects of the repository are gone: git cannot answer.
    for entry in fs::read_dir(root.join(".git/objects")).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap().len() == 2 {
            fs::remove_dir_all(path).unwrap();
        }
    }

    let refused = plan.question().unwrap_err();
    session.discard_refused(&refused);
    assert!(
        session
            .status()
            .starts_with("The changes of Plan could not be discarded: `git ls-tree` failed"),
        "{}",
        session.status()
    );
    // The diff read before is kept for the same commit; without it, git
    // is asked and fails.
    session.changes.diffs.cache = None;
    session.window_type(&id("Plan"), 0, "x");
    let diff = wait_diff(&mut session, |d| matches!(d.body, DiffBody::Problem { .. }));
    assert_eq!(diff.path, "Plan.md");
}

#[test]
fn review_f12_paths_are_literal_for_git() {
    let dir = temp_dir("literal");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[
            ("[a].md", Some("# bracket\n")),
            ("a.md", Some("# a\n")),
            (":(top)b.md", Some("# top\n")),
            ("b.md", Some("# b\n")),
        ],
    );
    wait_changes(&mut session, |v| v.files.len() == 4);
    session.choose_for_commit(None, false);
    session.choose_for_commit(Some("[a].md"), true);
    session.choose_for_commit(Some(":(top)b.md"), true);

    assert_eq!(
        commit(&mut session, "Two odd names", false),
        CommitAnswer::Committed
    );

    let committed = config.run(&root, &["show", "--name-only", "--format=", "HEAD"]);
    let mut names: Vec<&str> = committed.lines().collect();
    names.sort_unstable();
    assert_eq!(names, [":(top)b.md", "[a].md"]);
    let view = wait_changes(&mut session, |v| v.files.len() == 2);
    let paths: Vec<_> = view.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.md", "b.md"]);
}

#[test]
fn review_f12_discarding_after_a_refused_commit_leaves_nothing_staged() {
    let dir = temp_dir("staged");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Plan.md", Some("# P\n")), ("Fresh.md", Some("# Fresh\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 2);
    refuse_commits(&root);
    // The refused commit staged both changes (`git add`).
    assert_eq!(commit(&mut session, "Refused", false), CommitAnswer::Done);

    for path in ["Fresh.md", "Plan.md"] {
        session.discard_start(path).unwrap().question().unwrap();
        let job = session.discard_prepare(path).unwrap();
        session.finish_discard(job.run());
        assert!(
            session.status().starts_with("Discarded"),
            "{}",
            session.status()
        );
    }

    assert_eq!(config.run(&root, &["status", "--porcelain"]), "");
    wait_changes(&mut session, |v| v.files.is_empty());
}

#[test]
fn review_f12_discard_puts_back_the_committed_bytes() {
    let dir = temp_dir("bytes");
    let config = Config::new(&dir, MAIN);
    let crlf = "# Plan\r\n\r\none\r\n";
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", crlf)],
        &[("Plan.md", Some("# P\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);

    session
        .discard_start("Plan.md")
        .unwrap()
        .question()
        .unwrap();
    let job = session.discard_prepare("Plan.md").unwrap();
    session.finish_discard(job.run());

    assert_eq!(fs::read(root.join("Plan.md")).unwrap(), crlf.as_bytes());
}

#[test]
fn review_f12_a_change_that_is_no_longer_what_was_asked_about_is_not_discarded() {
    let dir = temp_dir("changed-since");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN), ("Keep.md", "# K\n")],
        &[("Plan.md", Some("# P\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);
    session
        .discard_start("Plan.md")
        .unwrap()
        .question()
        .unwrap();
    fs::remove_file(root.join("Plan.md")).unwrap();
    wait_changes(&mut session, |v| {
        v.files.first().is_some_and(|f| f.letter == "D")
    });

    assert!(session.discard_prepare("Plan.md").is_none());

    assert_eq!(
        session.status(),
        "The changes of Plan could not be discarded: it changed since the question; choose Discard… again"
    );
    assert!(!root.join("Plan.md").exists());
}

#[test]
fn review_f12_the_choice_does_not_follow_into_another_workspace() {
    let dir = temp_dir("left-out");
    let config = Config::new(&dir, MAIN);
    let (mut session, root) = repo_session(
        &dir,
        &config,
        &[("Plan.md", PLAN)],
        &[("Fresh.md", Some("# F\n"))],
    );
    wait_changes(&mut session, |v| v.files.len() == 1);
    session.choose_for_commit(Some("Fresh.md"), false);
    wait_changes(&mut session, |v| !v.files[0].chosen);
    let plain = dir.join("plain");
    write(&plain, "A.md", "# A\n");

    session.open_chosen(&plain, false);
    session.open_chosen(&root, false);

    let view = wait_changes(&mut session, |v| v.files.len() == 1);
    assert!(view.files[0].chosen);
}

#[test]
fn review_f12_a_name_or_email_with_a_line_break_is_refused() {
    let dir = temp_dir("identity-broken");
    let config = Config::new(&dir, MAIN);
    let (session, _) = repo_session(&dir, &config, &[("Plan.md", PLAN)], &[]);

    assert!(session
        .identity_prepare("Ann\nB", "ann@example.com")
        .is_none());
    assert!(session
        .identity_prepare("Ann", "ann@ex\rample.com")
        .is_none());
    assert!(session
        .identity_prepare("Ann\0", "ann@example.com")
        .is_none());
    assert!(session.identity_prepare("Ann", "ann@example.com").is_some());
}
