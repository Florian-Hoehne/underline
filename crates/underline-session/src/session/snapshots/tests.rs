//! Tests of the snapshots in the session (UC-026, TC-026-*): taken while
//! editing and for changes from outside, listed, compared, shown whole,
//! restored and taken back, with the snapshots' clock injected. How they
//! are stored, kept and moved is tested in `underline-core`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use underline_core::{
    NoteId, SnapshotCause, SnapshotStore, StateStore, Timestamp, SETTINGS_FILE_NAME,
};

use crate::snapshots_view::{SnapshotDiffBody, SnapshotsView};
use crate::sync_view::{SyncBatch, SyncEdit};
use crate::{BlockKindView, DiffLineKind, Event, Session, SettingsState, StatusOffer};

const MINUTE: Timestamp = 60 * 1000;
/// 2025-11-11 09:00 UTC.
const T0: Timestamp = 1_762_851_600_000;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-snapshots-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on `dir/notes` with `files`, its snapshots' clock at the
/// returned time (milliseconds), which the test moves on.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf, Arc<AtomicU64>) {
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
    let clock = Arc::new(AtomicU64::new(T0));
    let read = Arc::clone(&clock);
    session.set_snapshot_clock(move || read.load(Ordering::SeqCst));
    session.start_workspace();
    session.take_events();
    (session, root, clock)
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

fn store(root: &Path) -> SnapshotStore {
    SnapshotStore::new(root)
}

/// The snapshots of `file` with their causes and contents, newest first.
fn kept(root: &Path, file: &str) -> Vec<(SnapshotCause, String)> {
    store(root)
        .list(file, T0 + 1000 * MINUTE)
        .iter()
        .map(|s| (s.cause, s.content().unwrap()))
        .collect()
}

/// Waits until `file` has `count` snapshots (some are taken on the saver's
/// thread).
fn wait_for(root: &Path, file: &str, count: usize) -> Vec<(SnapshotCause, String)> {
    let start = Instant::now();
    loop {
        let kept = kept(root, file);
        if kept.len() >= count || start.elapsed() > Duration::from_secs(5) {
            return kept;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Ticks as if `seconds` had passed, until no tab holds a change not
/// stored.
fn autosave(session: &mut Session) {
    let later = Instant::now() + Duration::from_secs(3);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        session.tick(later);
        let open = session.open.as_ref().unwrap();
        if open
            .tabs
            .iter()
            .all(|(_, tab)| !tab.autosave.is_unstored() && !tab.saving)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the autosave did not report");
}

fn list(session: &mut Session, note: &str) -> SnapshotsView {
    session.snapshots_plan(&id(note)).unwrap().run()
}

fn tab<'s>(session: &'s mut Session, note: &str) -> &'s mut crate::session::tab::Tab {
    let open = session.open.as_mut().unwrap();
    let index = open.tabs.find(&id(note)).unwrap();
    open.tabs.get_mut(index)
}

fn read(root: &Path, file: &str) -> String {
    fs::read_to_string(root.join(file)).unwrap()
}

/// The whole text of `note` replaced in its tab, as an edit of the user.
fn rewrite(session: &mut Session, note: &str, text: &str) {
    let len = session.window_text(&id(note)).chars().count();
    session.window_edit(&id(note), 0, len, text);
}

/// TC-026-01, TC-026-07, UC-026 main success scenario, A3, BR-004: the
/// snapshot of an edit is listed and compared, restored in the tab as one
/// undo step and stored, with a snapshot of the text before; the undo of
/// the status bar gives that back.
#[test]
fn uc026_tc01_tc07_compare_restore_and_undo() {
    let dir = temp_dir("tc01");
    let (mut session, root, clock) = session_with(&dir, &[("Plan.md", "Version A\n")]);
    assert!(session.open_note(&id("Plan")));
    rewrite(&mut session, "Plan", "Version B\n");
    autosave(&mut session);
    clock.store(T0 + 30 * MINUTE, Ordering::SeqCst);

    let listed = list(&mut session, "Plan");
    assert_eq!(listed.items.len(), 1, "{listed:?}");
    let item = &listed.items[0];
    assert_eq!(
        (item.cause.as_str(), item.differs),
        ("Before editing", Some(1))
    );
    assert_eq!(listed.none, None);

    let diff = session
        .snapshot_plan(&id("Plan"), &item.key)
        .unwrap()
        .diff();
    let SnapshotDiffBody::Hunks { hunks } = diff.body else {
        panic!("{diff:?}");
    };
    let lines: Vec<(DiffLineKind, String)> = hunks[0]
        .lines
        .iter()
        .map(|l| (l.kind, l.parts.iter().map(|p| p.text.as_str()).collect()))
        .collect();
    assert_eq!(
        lines,
        [
            (DiffLineKind::Removed, "Version A".to_owned()),
            (DiffLineKind::Added, "Version B".to_owned()),
        ]
    );
    assert_eq!(
        read(&root, "Plan.md"),
        "Version B\n",
        "comparing changes nothing"
    );

    let status = session.snapshot_restore(&id("Plan"), &item.key).unwrap();

    assert!(
        status.starts_with("Restored Plan to the snapshot of "),
        "{status}"
    );
    assert_eq!(session.restore_offer(), Some(StatusOffer::UndoRestore));
    assert_eq!(read(&root, "Plan.md"), "Version A\n");
    assert_eq!(session.window_text(&id("Plan")), "Version A\n");
    assert!(!tab(&mut session, "Plan").autosave.is_unstored(), "stored");
    assert_eq!(
        kept(&root, "Plan.md"),
        [
            (SnapshotCause::Restore, "Version B\n".to_owned()),
            (SnapshotCause::Editing, "Version A\n".to_owned()),
        ]
    );
    assert!(session.take_events().iter().any(|e| matches!(
        e,
        Event::Status(view) if view.offer == Some(StatusOffer::UndoRestore)
    )));
    // TC-026-01 step 5: one undo step in the tab.
    let editor = &mut tab(&mut session, "Plan").edit.editor;
    editor.undo();
    assert_eq!(editor.text(), "Version B\n");
    editor.redo();

    // TC-026-07: "Undo" of the status bar.
    let status = session.snapshot_undo_restore();

    assert_eq!(status, "Took back the restore of Plan");
    assert_eq!(read(&root, "Plan.md"), "Version B\n");
    assert_eq!(session.window_text(&id("Plan")), "Version B\n");
    assert_eq!(session.restore_offer(), None);
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-02, BR-001, OQ-223: while editing, a snapshot of the content
/// before the change once the newest is older than 10 minutes, with the
/// clock injected.
#[test]
fn uc026_tc02_snapshots_while_editing() {
    let dir = temp_dir("tc02");
    let (mut session, root, clock) = session_with(&dir, &[("Log.md", "start")]);
    assert!(session.open_note(&id("Log")));

    session.window_type(&id("Log"), 5, " one");
    autosave(&mut session);
    clock.store(T0 + 5 * MINUTE, Ordering::SeqCst);
    session.window_type(&id("Log"), 9, " two");
    autosave(&mut session);
    clock.store(T0 + 20 * MINUTE, Ordering::SeqCst);
    session.window_type(&id("Log"), 13, " three");
    autosave(&mut session);

    assert_eq!(read(&root, "Log.md"), "start one two three");
    let listed = store(&root).list("Log.md", T0 + 20 * MINUTE);
    assert_eq!(
        listed
            .iter()
            .map(|s| (s.taken_at, s.content().unwrap()))
            .collect::<Vec<_>>(),
        [
            (T0 + 20 * MINUTE, "start one two".to_owned()),
            (T0, "start".to_owned()),
        ]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-02, UC-005 BR-001: storing at once (closing the tab) takes the
/// Editing snapshot too.
#[test]
fn uc026_br001_storing_at_once_takes_the_editing_snapshot() {
    let dir = temp_dir("store-now");
    let (mut session, root, _) = session_with(&dir, &[("Log.md", "start")]);
    assert!(session.open_note(&id("Log")));
    session.window_type(&id("Log"), 0, "x");

    assert_eq!(session.close_tab(&id("Log"), false), None);

    assert_eq!(read(&root, "Log.md"), "xstart");
    assert_eq!(
        wait_for(&root, "Log.md", 1),
        [(SnapshotCause::Editing, "start".to_owned())]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-03 step 1, BR-001: a note open in a tab changed by another
/// program keeps the text the tab showed, with the cause Outside change.
#[test]
fn uc026_tc03_a_change_from_outside() {
    let dir = temp_dir("tc03");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "mine\n")]);
    assert!(session.open_note(&id("Plan")));

    fs::write(root.join("Plan.md"), "theirs\n").unwrap();
    let start = Instant::now();
    while session.window_text(&id("Plan")) != "theirs\n" {
        assert!(start.elapsed() < Duration::from_secs(5), "not taken in");
        session.tick(Instant::now());
        std::thread::sleep(Duration::from_millis(20));
    }

    assert_eq!(
        wait_for(&root, "Plan.md", 1),
        [(SnapshotCause::OutsideChange, "mine\n".to_owned())]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-05, A1: a note never changed in Underline has no snapshot; the
/// dialog says when they are taken.
#[test]
fn uc026_tc05_no_snapshots() {
    let dir = temp_dir("tc05");
    let (mut session, _, _) = session_with(&dir, &[("New.md", "")]);

    let listed = list(&mut session, "New");

    assert!(listed.items.is_empty());
    let none = listed.none.unwrap();
    assert_eq!(none.lead, super::NO_SNAPSHOT);
    assert_eq!(none.when.len(), 6);
    assert_eq!(listed.kept, "Kept on this computer for 30 days");
    assert_eq!(listed.title, "New");
    assert!(session.snapshots_plan(&id("Gone")).is_err());
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-06, A2: the whole snapshot as the facade's blocks, with its
/// heading and table; nothing changes.
#[test]
fn uc026_tc06_the_whole_snapshot() {
    let dir = temp_dir("tc06");
    let text = "# Plan\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "now\n")]);
    let taken = store(&root)
        .take("Plan.md", text, SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();

    let whole = session
        .snapshot_plan(&id("Plan"), &taken.key())
        .unwrap()
        .whole();

    assert_eq!(whole.problem, None);
    assert!(
        matches!(whole.blocks[0].kind, BlockKindView::Heading { .. }),
        "{:?}",
        whole.blocks[0].kind
    );
    assert!(whole
        .blocks
        .iter()
        .any(|b| matches!(b.kind, BlockKindView::Table { .. })));
    assert_eq!(read(&root, "Plan.md"), "now\n");
    let gone = session
        .snapshot_plan(&id("Plan"), "1-editing.snap")
        .unwrap()
        .whole();
    assert_eq!(gone.problem.as_deref(), Some("The snapshot is gone"));
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-08, A4: while the snapshots are shown, a change of the note from
/// outside tells the window to ask again; after closing, nothing.
#[test]
fn uc026_tc08_the_note_changes_while_shown() {
    let dir = temp_dir("tc08");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "Version B\n")]);
    list(&mut session, "Plan");
    let changed = |events: &[Event]| {
        events
            .iter()
            .any(|e| matches!(e, Event::SnapshotsChanged(note) if note == "Plan"))
    };
    assert!(!changed(&session.tick(Instant::now())), "the first look");

    std::thread::sleep(Duration::from_millis(20));
    fs::write(root.join("Plan.md"), "Version C\n").unwrap();
    let start = Instant::now();
    let mut later = Instant::now();
    loop {
        later += Duration::from_secs(1);
        if changed(&session.tick(later)) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "not told");
        std::thread::sleep(Duration::from_millis(20));
    }

    session.snapshots_close();
    fs::write(root.join("Plan.md"), "Version D, longer\n").unwrap();
    assert!(!changed(&session.tick(later + Duration::from_secs(1))));
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-09, A5: the restored note cannot be stored: the tab keeps the
/// restored text, marked as not stored; the status says why; the file is
/// as it was.
#[test]
fn uc026_tc09_the_note_cannot_be_stored() {
    let dir = temp_dir("tc09");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "Version B\n")]);
    let taken = store(&root)
        .take("Plan.md", "Version A\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    assert!(session.open_note(&id("Plan")));
    let file = root.join("Plan.md");
    fs::remove_file(&file).unwrap();
    fs::create_dir_all(file.join("blocked")).unwrap();

    let status = session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

    assert!(status.starts_with("Plan could not be stored: "), "{status}");
    assert_eq!(session.status(), status);
    assert_eq!(session.restore_offer(), None);
    let tab = tab(&mut session, "Plan");
    assert_eq!(tab.mirror.text(), "Version A\n");
    assert!(tab.autosave.is_unstored() && tab.error.is_some());
    assert!(tab.label().contains('⚠'), "{}", tab.label());
    assert!(file.is_dir(), "nothing written");
    fs::remove_dir_all(&dir).unwrap();
}

/// TC-026-10, A6, failure postconditions: looking at snapshots and
/// closing them changes nothing; a snapshot gone or damaged is not
/// restored and the note stays.
#[test]
fn uc026_tc10_cancel_and_snapshots_that_cannot_be_restored() {
    let dir = temp_dir("tc10");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "now\n")]);
    let taken = store(&root)
        .take("Plan.md", "then\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    let listed = list(&mut session, "Plan");
    let _ = session
        .snapshot_plan(&id("Plan"), &listed.items[0].key)
        .unwrap()
        .diff();
    session.snapshots_close();
    assert_eq!(read(&root, "Plan.md"), "now\n");
    assert_eq!(kept(&root, "Plan.md").len(), 1);

    let damaged = root.join(".underline/snapshots/Plan.md").join(taken.key());
    fs::write(&damaged, [0xff, 0xfe]).unwrap();
    let listed = list(&mut session, "Plan");
    assert_eq!(listed.items[0].differs, None);
    assert_eq!(
        listed.items[0].problem.as_deref(),
        Some("The snapshot is damaged: it holds no text")
    );
    let error = session
        .snapshot_restore(&id("Plan"), &taken.key())
        .unwrap_err();
    assert_eq!(
        error.message,
        "Plan could not be restored: the snapshot is damaged: it holds no text"
    );
    fs::remove_file(&damaged).unwrap();
    let error = session
        .snapshot_restore(&id("Plan"), &taken.key())
        .unwrap_err();
    assert_eq!(
        error.message,
        "Plan could not be restored: the snapshot is gone"
    );
    assert_eq!(read(&root, "Plan.md"), "now\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 steps 5–7, A3 for a note not open in a tab: the file is written
/// at once, after a snapshot with the cause Restore.
#[test]
fn uc026_restore_a_closed_note_and_take_it_back() {
    let dir = temp_dir("closed");
    let (mut session, root, clock) = session_with(&dir, &[("Plan.md", "Version B\n")]);
    let taken = store(&root)
        .take("Plan.md", "Version A\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    clock.store(T0 + MINUTE, Ordering::SeqCst);

    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

    assert_eq!(read(&root, "Plan.md"), "Version A\n");
    assert_eq!(
        kept(&root, "Plan.md")[0],
        (SnapshotCause::Restore, "Version B\n".to_owned())
    );
    // Restoring it again finds the note holding it already.
    let status = session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();
    assert!(
        status.starts_with("Plan already holds the snapshot of "),
        "{status}"
    );
    assert_eq!(session.snapshot_undo_restore(), status, "no undo offered");

    fs::write(root.join("Plan.md"), "Version B\n").unwrap();
    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();
    assert_eq!(
        session.snapshot_undo_restore(),
        "Took back the restore of Plan"
    );
    assert_eq!(read(&root, "Plan.md"), "Version B\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026, ADR-005 (decision of 2026-11-16): a restored note keeps who
/// may read and write it.
#[cfg(unix)]
#[test]
fn uc026_adr005_restored_note_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("mode");
    let (mut session, root, clock) = session_with(&dir, &[("Plan.md", "Version B\n")]);
    let path = root.join("Plan.md");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let taken = store(&root)
        .take("Plan.md", "Version A\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    clock.store(T0 + MINUTE, Ordering::SeqCst);

    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

    assert_eq!(read(&root, "Plan.md"), "Version A\n");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// NFR-006, BR-004: a restore writes the snapshot's bytes exactly, a byte
/// order mark and CRLF line endings included, in a tab and from a file.
#[test]
fn uc026_restore_is_byte_exact() {
    let dir = temp_dir("bytes");
    let old = "\u{feff}# Plan\r\nline one\r\nlast";
    let (mut session, root, _) = session_with(
        &dir,
        &[
            ("Plan.md", "\u{feff}# Plan\r\nchanged\r\n"),
            ("Log.md", "x\n"),
        ],
    );
    for file in ["Plan.md", "Log.md"] {
        let taken = store(&root)
            .take(file, old, SnapshotCause::Editing, T0)
            .unwrap()
            .unwrap();
        if file == "Plan.md" {
            assert!(session.open_note(&id("Plan")));
        }
        let note = file.trim_end_matches(".md");
        session.snapshot_restore(&id(note), &taken.key()).unwrap();
        assert_eq!(fs::read(root.join(file)).unwrap(), old.as_bytes(), "{file}");
    }
    assert_eq!(session.window_text(&id("Plan")), old);
    fs::remove_dir_all(&dir).unwrap();
}

/// ADR-025, BR-004: a restore of an open note while the window holds edits
/// it has not sent: they move over the restore, both copies end alike, and
/// the snapshot of the text before holds what the backend had.
#[test]
fn uc026_adr025_restore_with_edits_the_window_has_not_sent() {
    let dir = temp_dir("unsent");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "one\n")]);
    let taken = store(&root)
        .take("Plan.md", "zero\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    assert!(session.open_note(&id("Plan")));
    let base = session.mirror_rev(&id("Plan"));

    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();
    // The window typed "!" after "one" before it learnt of the restore.
    let synced = session
        .editor_sync(
            &id("Plan"),
            &SyncBatch {
                base,
                edits: vec![SyncEdit {
                    at: 3,
                    remove: 0,
                    insert: "!".into(),
                }],
                seq: 0,
            },
        )
        .unwrap();

    assert!(!synced.answer.resync);
    let text = session.window_text(&id("Plan"));
    assert_eq!(text, "zero!\n");
    assert_eq!(tab(&mut session, "Plan").edit.editor.text(), text);
    assert_eq!(
        kept(&root, "Plan.md")[0],
        (SnapshotCause::Restore, "one\n".to_owned())
    );
    autosave(&mut session);
    assert_eq!(read(&root, "Plan.md"), "zero!\n");
    fs::remove_dir_all(&dir).unwrap();
}

/// NFR-005, UC-005 BR-001: a restore while a background save of the note
/// runs waits for it, so the older text never lands after the restored
/// one; the Editing snapshot of the save is kept too.
#[test]
fn uc026_restore_while_the_autosave_runs() {
    let dir = temp_dir("concurrent");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "Version A\n")]);
    let taken = store(&root)
        .take(
            "Plan.md",
            "Version 0\n",
            SnapshotCause::Git,
            T0 - 20 * MINUTE,
        )
        .unwrap()
        .unwrap();
    assert!(session.open_note(&id("Plan")));
    rewrite(&mut session, "Plan", "Version B\n");
    // The autosave starts on the saver's thread and is not waited for.
    session.tick(Instant::now() + Duration::from_secs(3));

    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

    for _ in 0..20 {
        session.tick(Instant::now() + Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(read(&root, "Plan.md"), "Version 0\n");
    assert_eq!(
        kept(&root, "Plan.md"),
        [
            (SnapshotCause::Restore, "Version B\n".to_owned()),
            (SnapshotCause::Editing, "Version A\n".to_owned()),
            (SnapshotCause::Git, "Version 0\n".to_owned()),
        ]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 restore of a note whose file another program changed and the
/// tab has not taken in yet: that version is kept too.
#[test]
fn uc026_restore_keeps_a_change_from_outside_not_taken_in() {
    let dir = temp_dir("outside-first");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "mine\n")]);
    let taken = store(&root)
        .take("Plan.md", "old\n", SnapshotCause::Editing, T0 - MINUTE)
        .unwrap()
        .unwrap();
    assert!(session.open_note(&id("Plan")));
    fs::write(root.join("Plan.md"), "theirs\n").unwrap();

    session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

    let kept = kept(&root, "Plan.md");
    assert!(
        kept.contains(&(SnapshotCause::OutsideChange, "theirs\n".to_owned())),
        "{kept:?}"
    );
    assert!(
        kept.contains(&(SnapshotCause::Restore, "mine\n".to_owned())),
        "{kept:?}"
    );
    assert_eq!(read(&root, "Plan.md"), "old\n");
    fs::remove_dir_all(&dir).unwrap();
}

// ---- Test monkey, package 8a.

/// A small seeded generator (xorshift), so a failing sequence can be
/// replayed from the seed in the failure message.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap()
    }
    fn text(&mut self) -> String {
        let alphabet = ['a', 'b', ' ', '\n', 'ä', '😀', 'é', '\r'];
        (0..self.below(12))
            .map(|_| alphabet[self.below(alphabet.len())])
            .collect()
    }
}

/// UC-026 BR-004, ADR-025 section 3.6: a restore, then edits the window
/// sent against the revision before the restore. Whatever the texts, the
/// edit is kept once, the backend's text and its editor agree, the window
/// is not told to start over, and the stored file is the text of the tab.
#[test]
fn uc026_monkey_restore_rebases_unsent_edits() {
    let seed: u64 = 0x8a8a_2026_1113;
    eprintln!("seed {seed:#x}");
    let mut rng = Seeded(seed);
    for round in 0..60 {
        let dir = temp_dir(&format!("monkey-rebase-{round}"));
        let current = format!("{}\n", rng.text());
        let wanted = format!("{}\n", rng.text());
        let (mut session, root, _) = session_with(&dir, &[("Plan.md", &current)]);
        let taken = store(&root)
            .take("Plan.md", &wanted, SnapshotCause::Editing, T0 - MINUTE)
            .unwrap()
            .unwrap();
        assert!(session.open_note(&id("Plan")));
        let base = session.mirror_rev(&id("Plan"));
        let units = current.chars().count();
        let at = rng.below(units + 1);
        let insert = ["X", "ü", "😀"][rng.below(3)];

        session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();
        let synced = session
            .editor_sync(
                &id("Plan"),
                &SyncBatch {
                    base,
                    edits: vec![SyncEdit {
                        at: u32::try_from(at).unwrap(),
                        remove: 0,
                        insert: insert.into(),
                    }],
                    seq: 0,
                },
            )
            .unwrap();

        let ctx =
            format!("seed {seed:#x} round {round}: {current:?} -> {wanted:?}, {insert:?} at {at}");
        assert!(!synced.answer.resync, "{ctx}");
        let text = session.window_text(&id("Plan"));
        assert_eq!(
            text.chars().count(),
            wanted.chars().count() + 1,
            "{ctx}: {text:?}"
        );
        assert_eq!(tab(&mut session, "Plan").edit.editor.text(), text, "{ctx}");
        autosave(&mut session);
        assert_eq!(read(&root, "Plan.md"), text, "{ctx}");
        fs::remove_dir_all(&dir).unwrap();
    }
}

/// UC-026 BR-004, NFR-006: byte-exact restore of an empty snapshot, one
/// with a BOM and CRLF, in a note open in a tab and in a closed note.
#[test]
fn uc026_monkey_restore_empty_and_bom_crlf_in_tab_and_closed() {
    for (open, wanted) in [
        (true, ""),
        (false, ""),
        (true, "\u{feff}a\r\nb"),
        (false, "\u{feff}a\r\nb"),
        (true, "\n"),
    ] {
        let dir = temp_dir("monkey-bytes");
        let (mut session, root, _) = session_with(&dir, &[("Plan.md", "now\r\nthere\n")]);
        let taken = store(&root)
            .take("Plan.md", wanted, SnapshotCause::Editing, T0 - MINUTE)
            .unwrap()
            .unwrap();
        if open {
            assert!(session.open_note(&id("Plan")));
        }

        let status = session.snapshot_restore(&id("Plan"), &taken.key()).unwrap();

        assert!(
            status.starts_with("Restored Plan"),
            "{open} {wanted:?}: {status}"
        );
        assert_eq!(fs::read(root.join("Plan.md")).unwrap(), wanted.as_bytes());
        if open {
            assert_eq!(session.window_text(&id("Plan")), wanted);
        }
        // The undo gives the bytes from before back.
        session.snapshot_undo_restore();
        autosave(&mut session);
        assert_eq!(
            fs::read(root.join("Plan.md")).unwrap(),
            b"now\r\nthere\n",
            "{open} {wanted:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}

/// UC-026 failure postconditions: a snapshot deleted between the list and
/// the restore, a damaged one, and a note deleted in between: an error, the
/// note and the other snapshots are unchanged, no snapshot is added.
#[test]
fn uc026_monkey_restore_of_vanished_damaged_and_deleted() {
    let dir = temp_dir("monkey-vanished");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "now\n"), ("Gone.md", "g\n")]);
    let first = store(&root)
        .take(
            "Plan.md",
            "first\n",
            SnapshotCause::Editing,
            T0 - 3 * MINUTE,
        )
        .unwrap()
        .unwrap();
    let damaged = store(&root)
        .take(
            "Plan.md",
            "second\n",
            SnapshotCause::Editing,
            T0 - 2 * MINUTE,
        )
        .unwrap()
        .unwrap();
    let listed = list(&mut session, "Plan");
    assert_eq!(listed.items.len(), 2);
    let folder = root.join(".underline/snapshots/Plan.md");
    fs::write(folder.join(damaged.key()), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    fs::remove_file(folder.join(first.key())).unwrap();
    for open in [false, true] {
        if open {
            assert!(session.open_note(&id("Plan")));
        }
        for key in [first.key(), damaged.key()] {
            assert!(session.snapshot_restore(&id("Plan"), &key).is_err());
            assert_eq!(read(&root, "Plan.md"), "now\n");
        }
    }
    assert_eq!(
        fs::read_dir(&folder).unwrap().count(),
        1,
        "no snapshot was added"
    );

    // A note deleted after the list was made.
    let taken = store(&root)
        .take("Gone.md", "old\n", SnapshotCause::Editing, T0 - MINUTE)
        .unwrap()
        .unwrap();
    fs::remove_file(root.join("Gone.md")).unwrap();
    assert!(session.snapshot_restore(&id("Gone"), &taken.key()).is_err());
    assert!(
        !root.join("Gone.md").exists(),
        "a deleted note must not come back"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// NFR-005: only the newest 30 snapshots are read for the list; a damaged
/// one further down shows its problem only when it is chosen.
#[test]
fn uc026_nfr005_the_list_reads_only_the_newest() {
    let dir = temp_dir("lazy-list");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "now\n")]);
    let store = store(&root);
    let oldest = store
        .take("Plan.md", "v0\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    for n in 1..=31u64 {
        store
            .take(
                "Plan.md",
                &format!("v{n}\n"),
                SnapshotCause::Editing,
                T0 + n,
            )
            .unwrap();
    }
    fs::write(
        root.join(".underline/snapshots/Plan.md").join(oldest.key()),
        [0xff, 0xfe],
    )
    .unwrap();

    let listed = list(&mut session, "Plan");

    assert_eq!(listed.items.len(), 32);
    assert!(listed.items[..30]
        .iter()
        .all(|item| item.differs == Some(1)));
    let last = listed.items.last().unwrap();
    assert_eq!((last.differs, last.problem.as_deref()), (None, None));
    let diff = session
        .snapshot_plan(&id("Plan"), &last.key)
        .unwrap()
        .diff();
    assert!(
        matches!(diff.body, SnapshotDiffBody::Problem { .. }),
        "{diff:?}"
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 A7, OQ-294, TC-026-15 steps 1–2: the settings show the space the
/// snapshots of the workspace take, and say "None" without any.
#[test]
fn uc026_a7_tc15_the_space_of_the_snapshots() {
    let dir = temp_dir("a7-space");
    let (session, root, _) = session_with(&dir, &[("Plan.md", "x\n")]);
    let empty = session.snapshots_space_plan().unwrap().run();
    assert_eq!(
        (empty.summary.as_str(), empty.any, empty.problem.as_deref()),
        ("None", false, None)
    );
    assert_eq!(empty.kept, "Kept on this computer for 30 days");

    let store = store(&root);
    store
        .take("Plan.md", &"a".repeat(2048), SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Plan.md", "b", SnapshotCause::Restore, T0 + MINUTE)
        .unwrap();
    let space = session.snapshots_space_plan().unwrap().run();
    assert_eq!(space.summary, "2 KB in 2 snapshots");
    assert!(space.any);
    assert!(
        space
            .question
            .starts_with("Remove all snapshots of this workspace (2 KB in 2 snapshots)?"),
        "{}",
        space.question
    );
    assert!(space.question.ends_with("This cannot be undone."));
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 A7, OQ-294, TC-026-15 steps 3–5: "Remove all snapshots" removes
/// them all, the status says how many and the space freed, the undo of a
/// restore is no longer offered, and the next change takes a snapshot again.
#[test]
fn uc026_a7_tc15_remove_all_snapshots_says_what_it_removed() {
    let dir = temp_dir("a7-remove");
    let (mut session, root, clock) = session_with(&dir, &[("Plan.md", "Version A\n")]);
    assert!(session.open_note(&id("Plan")));
    rewrite(&mut session, "Plan", "Version B\n");
    autosave(&mut session);
    let key = list(&mut session, "Plan").items[0].key.clone();
    session.snapshot_restore(&id("Plan"), &key).unwrap();
    assert_eq!(session.restore_offer(), Some(StatusOffer::UndoRestore));

    let job = session.snapshots_removal_plan().unwrap();
    let removed = job.run();
    let status = session.snapshots_removed(&removed);

    assert_eq!(
        status,
        "Removed all snapshots of this workspace: 20 bytes in 2 snapshots"
    );
    assert_eq!(session.status(), status);
    assert_eq!(session.restore_offer(), None);
    assert!(kept(&root, "Plan.md").is_empty());
    assert!(!root.join(".underline/snapshots").exists());
    assert_eq!(read(&root, "Plan.md"), "Version A\n", "the note stays");
    assert_eq!(
        session.snapshots_space_plan().unwrap().run().summary,
        "None"
    );

    let again = session.snapshots_removal_plan().unwrap().run();
    assert_eq!(
        session.snapshots_removed(&again),
        "There were no snapshots to remove"
    );

    clock.store(T0 + 30 * MINUTE, Ordering::SeqCst);
    rewrite(&mut session, "Plan", "Version C\n");
    autosave(&mut session);
    assert_eq!(
        wait_for(&root, "Plan.md", 1),
        [(SnapshotCause::Editing, "Version A\n".to_owned())]
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 A7, OQ-294, ADR-031: with the folder of the snapshots behind a
/// link, nothing is removed and the status says why; the space cannot be
/// measured either.
#[cfg(unix)]
#[test]
fn uc026_a7_a_linked_snapshots_folder_is_reported_not_followed() {
    let dir = temp_dir("a7-link");
    let (mut session, root, _) = session_with(&dir, &[("Plan.md", "x\n")]);
    let outside = dir.join("outside");
    fs::create_dir_all(outside.join("Plan.md")).unwrap();
    let victim = outside.join("Plan.md").join(format!("{T0}-editing.snap"));
    fs::write(&victim, "precious").unwrap();
    fs::create_dir_all(root.join(".underline")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".underline/snapshots")).unwrap();

    let space = session.snapshots_space_plan().unwrap().run();
    assert!(!space.any);
    assert!(space
        .problem
        .as_deref()
        .is_some_and(|p| p.starts_with("The space of the snapshots cannot be measured:")));

    let removed = session.snapshots_removal_plan().unwrap().run();
    let status = session.snapshots_removed(&removed);
    assert!(
        status.starts_with("No snapshot was removed: the snapshots of "),
        "{status}"
    );
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 A7: without a workspace there is nothing to measure or remove.
#[test]
fn uc026_a7_without_a_workspace() {
    let dir = temp_dir("a7-none");
    let mut session = Session::new(
        None,
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
    );
    assert!(session.snapshots_space_plan().is_err());
    assert!(session.snapshots_removal_plan().is_err());
    fs::remove_dir_all(&dir).unwrap();
}

/// UC-026 A7, OQ-294: the report says what went and what stayed, with
/// thousands separated, never a clean removal when something stayed; the
/// space of snapshots partly unreadable is "at least".
#[test]
fn uc026_a7_reports_of_partial_removals_and_unreadable_space() {
    use underline_core::{SnapshotRemoval, SnapshotUsage};
    let usage = SnapshotUsage {
        count: 1_203,
        bytes: 13_002_342,
        unreadable: 0,
    };
    let report = |removal: SnapshotRemoval| super::removal_report(&Ok(removal));
    assert_eq!(
        report(SnapshotRemoval {
            removed: usage,
            ..SnapshotRemoval::default()
        }),
        "Removed all snapshots of this workspace: 12.4 MB in 1,203 snapshots"
    );
    assert_eq!(
        report(SnapshotRemoval {
            removed: usage,
            failed: 1_500,
            first_error: Some(std::io::Error::other("denied")),
            links: 2_000,
        }),
        "Removed 12.4 MB in 1,203 snapshots; 1,500 files or folders could not be removed: \
         denied; 2,000 links were left as they are and not followed"
    );
    assert_eq!(
        report(SnapshotRemoval {
            failed: 1,
            first_error: Some(std::io::Error::other("denied")),
            ..SnapshotRemoval::default()
        }),
        "No snapshot was removed; 1 file or folder could not be removed: denied"
    );
    assert_eq!(
        report(SnapshotRemoval {
            links: 1,
            ..SnapshotRemoval::default()
        }),
        "No snapshot was removed; 1 link was left as it is and not followed"
    );
    let view = super::space_view(SnapshotUsage {
        unreadable: 1,
        ..usage
    });
    assert_eq!(
        view.summary,
        "at least 12.4 MB in 1,203 snapshots; 1 folder could not be read"
    );
    assert!(view.any);
    // Package 12 review: stray bytes without a snapshot remove nothing.
    let stray = super::space_view(SnapshotUsage {
        count: 0,
        bytes: 100,
        ..SnapshotUsage::default()
    });
    assert!(!stray.any);
}
