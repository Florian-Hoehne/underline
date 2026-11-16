use super::*;

use std::sync::{Arc, Barrier};

use crate::note::NoteId;
use crate::rename::Rename;
use crate::workspace::Workspace;

const MINUTE: Timestamp = 60 * 1000;
const DAY: Timestamp = 24 * 60 * MINUTE;
/// 2025-11-11 09:00 UTC.
const T0: Timestamp = 1_762_851_600_000;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root =
            std::env::temp_dir().join(format!("underline-uc026-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (path, content) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        Self(root)
    }

    fn store(&self) -> SnapshotStore {
        SnapshotStore::new(&self.0)
    }

    fn contents(&self, note_file: &str, now: Timestamp) -> Vec<(SnapshotCause, String)> {
        self.store()
            .list(note_file, now)
            .iter()
            .map(|s| (s.cause, s.content().unwrap()))
            .collect()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// UC-026 step 2, BR-002: snapshots are listed newest first, each with
/// its cause, and kept in the Underline folder, which git ignores.
#[test]
fn uc026_tc01_lists_newest_first_in_the_underline_folder() {
    let sandbox = Sandbox::new("list", &[("Plan.md", "Version B\n")]);
    let store = sandbox.store();

    let first = store
        .take("Plan.md", "Version A\n", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    store
        .take(
            "Plan.md",
            "Version B\n",
            SnapshotCause::Restore,
            T0 + MINUTE,
        )
        .unwrap()
        .unwrap();

    let listed = store.list("Plan.md", T0 + 2 * MINUTE);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].cause, SnapshotCause::Restore);
    assert_eq!(listed[1], first);
    assert_eq!(listed[1].content().unwrap(), "Version A\n");
    assert!(sandbox
        .0
        .join(".underline/snapshots/Plan.md")
        .join(first.key())
        .is_file());
    assert_eq!(
        fs::read_to_string(sandbox.0.join(".underline/.gitignore")).unwrap(),
        "*\n"
    );
}

/// UC-026 BR-001 (TC-026-02, OQ-223): a change in Underline takes a
/// snapshot only when the newest is older than 10 minutes, with the clock
/// injected.
#[test]
fn uc026_tc02_editing_takes_one_snapshot_per_ten_minutes() {
    let sandbox = Sandbox::new("editing", &[("Log.md", "start")]);
    let store = sandbox.store();

    let at_10_00 = store.take_for_editing("Log.md", "start", T0).unwrap();
    let at_10_05 = store
        .take_for_editing("Log.md", "start one", T0 + 5 * MINUTE)
        .unwrap();
    let at_10_10 = store
        .take_for_editing("Log.md", "start one two", T0 + 10 * MINUTE)
        .unwrap();
    let at_10_20 = store
        .take_for_editing("Log.md", "start one two", T0 + 20 * MINUTE)
        .unwrap();

    assert_eq!(at_10_00.unwrap().cause, SnapshotCause::Editing);
    assert!(at_10_05.is_none());
    assert!(at_10_10.is_none(), "exactly 10 minutes is not older");
    assert!(at_10_20.is_some());
    assert_eq!(
        sandbox.contents("Log.md", T0 + 21 * MINUTE),
        [
            (SnapshotCause::Editing, "start one two".to_owned()),
            (SnapshotCause::Editing, "start".to_owned()),
        ]
    );
}

/// UC-026 BR-001 (TC-026-04): no snapshot of the content of the newest,
/// whatever the cause and however long ago.
#[test]
fn uc026_tc04_no_snapshot_of_the_same_content() {
    let sandbox = Sandbox::new("dedupe", &[("Plan.md", "Version A")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "Version A", SnapshotCause::Git, T0)
        .unwrap()
        .unwrap();

    assert!(store
        .take_for_editing("Plan.md", "Version A", T0 + 15 * MINUTE)
        .unwrap()
        .is_none());
    assert!(store
        .take(
            "Plan.md",
            "Version A",
            SnapshotCause::AgentChange,
            T0 + 16 * MINUTE
        )
        .unwrap()
        .is_none());
    // Only the newest counts: an older content comes again.
    store
        .take(
            "Plan.md",
            "Version B",
            SnapshotCause::Restore,
            T0 + 17 * MINUTE,
        )
        .unwrap()
        .unwrap();
    assert!(store
        .take(
            "Plan.md",
            "Version A",
            SnapshotCause::Restore,
            T0 + 18 * MINUTE
        )
        .unwrap()
        .is_some());
    assert_eq!(store.list("Plan.md", T0 + 19 * MINUTE).len(), 3);
}

/// UC-026 BR-002, TC-026-11, OQ-224: a snapshot is kept for exactly 30
/// days; an older one is not listed and is removed by pruning; one taken
/// "later" than now (a clock set back) stays.
#[test]
fn uc026_tc11_kept_thirty_days_with_an_injected_clock() {
    let sandbox = Sandbox::new("retention", &[("Plan.md", "now")]);
    let store = sandbox.store();
    let october = T0 - 41 * DAY;
    let november = T0 - 10 * DAY;
    store
        .take("Plan.md", "October", SnapshotCause::Editing, october)
        .unwrap();
    store
        .take("Plan.md", "November", SnapshotCause::Editing, november)
        .unwrap();
    store
        .take(
            "Ideas.md",
            "boundary",
            SnapshotCause::Editing,
            T0 - 30 * DAY,
        )
        .unwrap();

    assert_eq!(
        sandbox.contents("Plan.md", T0),
        [(SnapshotCause::Editing, "November".to_owned())]
    );
    // Exactly 30 days: kept; one millisecond more: gone.
    assert_eq!(store.list("Ideas.md", T0).len(), 1);
    assert!(store.list("Ideas.md", T0 + 1).is_empty());
    assert_eq!(store.list("Plan.md", T0 - 50 * DAY).len(), 2);

    assert_eq!(store.prune(T0), 1);
    assert_eq!(store.prune(T0), 0);
    assert_eq!(store.list("Plan.md", october).len(), 1);
    assert_eq!(store.prune(T0 + 1), 1);
    assert!(!sandbox.0.join(".underline/snapshots/Ideas.md").exists());
}

/// UC-026 BR-001, BR-002: the dedupe looks only at snapshots still kept:
/// the same content as an expired one is taken again.
#[test]
fn uc026_br002_an_expired_snapshot_does_not_stop_a_new_one() {
    let sandbox = Sandbox::new("expired-dedupe", &[("Plan.md", "same")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "same", SnapshotCause::Editing, T0 - 31 * DAY)
        .unwrap();

    assert!(store
        .take("Plan.md", "same", SnapshotCause::Editing, T0)
        .unwrap()
        .is_some());
    assert_eq!(store.list("Plan.md", T0).len(), 1);
}

/// NFR-006, BR-004: a snapshot holds the note's bytes exactly: a byte
/// order mark, CRLF and lone CR line endings, no line ending at the end.
#[test]
fn uc026_snapshots_are_byte_exact() {
    let text = "\u{feff}# Plan\r\nline\rend";
    let sandbox = Sandbox::new("bytes", &[("Plan.md", text)]);
    let store = sandbox.store();

    let snapshot = store
        .take("Plan.md", text, SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();

    assert_eq!(fs::read(&snapshot.file).unwrap(), text.as_bytes());
    assert_eq!(snapshot.content().unwrap(), text);
    // The same text without its byte order mark or with LF is another one.
    for other in ["# Plan\r\nline\rend", "\u{feff}# Plan\nline\rend"] {
        assert!(store
            .take("Plan.md", other, SnapshotCause::Editing, T0 + 1)
            .unwrap()
            .is_some());
    }
}

/// UC-026 failure postcondition: damaged and missing snapshots: a file of
/// another name is never listed, pruned or taken as the newest; one that
/// holds no text says so; one gone is gone; a key that would leave the
/// note's folder names nothing.
#[test]
fn uc026_damaged_and_missing_snapshot_files() {
    let sandbox = Sandbox::new("damaged", &[("Plan.md", "x")]);
    let store = sandbox.store();
    let folder = sandbox.0.join(".underline/snapshots/Plan.md");
    fs::create_dir_all(&folder).unwrap();
    for name in [
        "notes.txt",
        "abc-editing.snap",
        "1-unknown.snap",
        "1-editing-x.snap",
        "1-editing-2-3.snap",
        "-editing.snap",
    ] {
        fs::write(folder.join(name), "junk").unwrap();
    }
    let damaged = folder.join(format!("{T0}-git.snap"));
    fs::write(&damaged, [0xff, 0xfe, 0x00]).unwrap();

    let listed = store.list("Plan.md", T0);
    assert_eq!(listed.len(), 1);
    assert!(matches!(listed[0].content(), Err(SnapshotError::NotText)));
    // The damaged newest does not hold "x": a snapshot of it is taken.
    let taken = store
        .take("Plan.md", "x", SnapshotCause::Editing, T0 + 1)
        .unwrap()
        .unwrap();
    assert!(matches!(
        store.find("Plan.md", "../../state.yaml"),
        Err(SnapshotError::Gone)
    ));
    assert!(matches!(
        store.find("Plan.md", "notes.txt"),
        Err(SnapshotError::Gone)
    ));
    assert_eq!(store.find("Plan.md", &taken.key()).unwrap(), taken);
    fs::remove_file(&taken.file).unwrap();
    assert!(matches!(
        store.find("Plan.md", &taken.key()),
        Err(SnapshotError::Gone)
    ));
    assert!(matches!(taken.content(), Err(SnapshotError::Gone)));
    assert_eq!(store.prune(T0 + 100 * DAY), 1, "only the damaged snapshot");
    assert!(folder.join("notes.txt").is_file());
}

/// UC-026: a note's file outside the workspace, or a name that would leave
/// the snapshots folder, has no snapshots.
#[test]
fn uc026_only_notes_of_the_workspace() {
    let sandbox = Sandbox::new("outside", &[("a/Plan.md", "x")]);
    let store = sandbox.store();

    assert_eq!(
        store.note_file(&sandbox.0.join("a/Plan.md")).unwrap(),
        "a/Plan.md"
    );
    assert!(store.note_file(Path::new("/etc/passwd")).is_err());
    assert!(store.take("../x.md", "x", SnapshotCause::Git, T0).is_err());
    assert!(store.take("a//x.md", "x", SnapshotCause::Git, T0).is_err());
    assert!(store.list("..", T0).is_empty());
}

/// UC-026, ADR-031 (update 2026-11-16): snapshot files are created 0600
/// and the folders the snapshots create 0700, also those kept with a
/// deleted note; a folder that was there keeps its mode.
#[cfg(unix)]
#[test]
fn uc026_adr031_snapshots_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("private", &[("work/Plan.md", "now"), ("Home.md", "")]);
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    let underline = sandbox.0.join(UNDERLINE_FOLDER);
    fs::create_dir_all(&underline).unwrap();
    fs::set_permissions(&underline, fs::Permissions::from_mode(0o755)).unwrap();
    let store = sandbox.store();

    let taken = store
        .take("work/Plan.md", "one", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();

    assert_eq!(mode(&taken.file), 0o600);
    let folder = taken.file.parent().unwrap();
    assert_eq!(mode(folder), 0o700);
    assert_eq!(mode(&underline.join(SNAPSHOTS_FOLDER)), 0o700);
    assert_eq!(mode(&underline), 0o755, "a folder that was there stays");

    let mut workspace = Workspace::open(&sandbox.0).unwrap();
    let now = T0 + MINUTE;
    let deleted = workspace
        .delete_note(&NoteId::from_relative("work/Plan"), now)
        .unwrap();
    let kept = underline
        .join("deleted")
        .join(now.to_string())
        .join(DELETED_SNAPSHOTS_FOLDER);
    let mut files = Vec::new();
    let mut folders = vec![kept.clone()];
    while let Some(dir) = folders.pop() {
        assert_eq!(mode(&dir), 0o700, "{}", dir.display());
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                folders.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    assert_eq!(files.len(), 1, "the snapshot is kept with the deleted note");
    assert_eq!(mode(&files[0]), 0o600);
    assert!(workspace.deleted_notes().contains(&deleted));
}

/// UC-026 BR-003, TC-026-12: the snapshots follow a renamed note, are kept
/// with a deleted one, and come back when it is restored, also to another
/// place.
#[test]
fn uc026_tc12_renamed_and_deleted_notes() {
    let sandbox = Sandbox::new("follow", &[("Plan.md", "now"), ("Home.md", "")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "one", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Plan.md", "two", SnapshotCause::Editing, T0 + 20 * MINUTE)
        .unwrap();
    let mut workspace = Workspace::open(&sandbox.0).unwrap();
    let now = T0 + 30 * MINUTE;

    let planned = workspace
        .plan_rename(
            &NoteId::from_relative("Plan"),
            &Rename {
                title: "Roadmap".into(),
                folder: "work".into(),
            },
        )
        .unwrap()
        .unwrap();
    workspace.move_note(&planned, "now").unwrap();
    assert!(store.list("Plan.md", now).is_empty());
    assert_eq!(store.list("work/Roadmap.md", now).len(), 2);

    let deleted = workspace
        .delete_note(&NoteId::from_relative("work/Roadmap"), now)
        .unwrap();
    assert!(store.list("work/Roadmap.md", now).is_empty());
    // The deleted notes do not list the snapshots as notes.
    assert_eq!(workspace.deleted_notes(), std::slice::from_ref(&deleted));

    let id = workspace.restore_note(&deleted, None).unwrap();
    assert_eq!(id.as_str(), "work/Roadmap");
    assert_eq!(
        sandbox.contents("work/Roadmap.md", now),
        [
            (SnapshotCause::Editing, "two".to_owned()),
            (SnapshotCause::Editing, "one".to_owned()),
        ]
    );
    assert!(workspace.deleted_notes().is_empty());
    assert!(!sandbox
        .0
        .join(".underline/deleted")
        .join(now.to_string())
        .exists());

    // A6: restored under another title, the snapshots go there.
    let deleted = workspace.delete_note(&id, now + 1).unwrap();
    workspace
        .create_note(
            &crate::new_note::NewNote {
                folder: "work".into(),
                title: "Roadmap".into(),
                format: crate::format::NoteFormat::Markdown,
            },
            "taken",
        )
        .unwrap();
    let instead = Rename {
        title: "Roadmap old".into(),
        folder: String::new(),
    };
    let id = workspace.restore_note(&deleted, Some(&instead)).unwrap();
    assert_eq!(store.list(&id.file_name(deleted.format), now).len(), 2);
    assert!(store.list("work/Roadmap.md", now).is_empty());
}

/// UC-026 BR-003, UC-007 A7, A8: a deleted note removed for good, or a
/// note removed without being kept, takes its snapshots with it.
#[test]
fn uc026_br003_removed_for_good_with_its_snapshots() {
    let sandbox = Sandbox::new("for-good", &[("Plan.md", "x"), ("Old.md", "y")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "a", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Old.md", "b", SnapshotCause::Editing, T0)
        .unwrap();
    let mut workspace = Workspace::open(&sandbox.0).unwrap();

    let deleted = workspace
        .delete_note(&NoteId::from_relative("Plan"), T0)
        .unwrap();
    workspace.forget_deleted(&deleted).unwrap();
    workspace
        .delete_note_for_good(&NoteId::from_relative("Old"))
        .unwrap();

    assert!(!sandbox
        .0
        .join(".underline/deleted")
        .join(T0.to_string())
        .exists());
    assert!(!sandbox.0.join(".underline/snapshots/Old.md").exists());
    assert!(store.list("Plan.md", T0).is_empty());
}

/// UC-026 BR-003: only the snapshot files of a note move with it, never
/// a folder below its folder (a file and a folder never share a name in
/// the workspace, so this holds whatever lies there).
#[test]
fn uc026_br003_only_the_notes_own_snapshots_move() {
    let sandbox = Sandbox::new("folder-like", &[]);
    let store = sandbox.store();
    store
        .take("Plan.md", "a", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Plan.md/Sub.md", "b", SnapshotCause::Editing, T0)
        .unwrap();

    store.moved("Plan.md", "Roadmap.md").unwrap();

    assert_eq!(store.list("Roadmap.md", T0).len(), 1);
    assert_eq!(store.list("Plan.md/Sub.md", T0).len(), 1);
    assert!(store.list("Plan.md", T0).is_empty());
}

/// UC-026 BR-001, NFR-005: snapshots taken at once from several threads
/// (the saver and the session) are all kept, each whole, none twice.
#[test]
fn uc026_concurrent_snapshots_are_all_kept_once() {
    let sandbox = Sandbox::new("concurrent", &[("Plan.md", "x")]);
    let store = sandbox.store();
    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|n| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                // Half of them the same text, all in the same millisecond.
                let text = if n % 2 == 0 {
                    "same".to_owned()
                } else {
                    format!("text {n}")
                };
                store.take("Plan.md", &text, SnapshotCause::Editing, T0)
            })
        })
        .collect();
    let taken: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().unwrap().unwrap())
        .collect();

    let listed = store.list("Plan.md", T0);
    assert_eq!(listed.len(), taken.iter().flatten().count());
    assert!(listed.len() >= 5, "{}", listed.len());
    let mut keys: Vec<String> = listed.iter().map(Snapshot::key).collect();
    keys.dedup();
    assert_eq!(keys.len(), listed.len());
    for (one, next) in listed.iter().zip(listed.iter().skip(1)) {
        assert_ne!(one.content().unwrap(), next.content().unwrap());
    }
}

/// UC-026 step 2 (TC-026-01): "differing in 1 line" for a changed line;
/// lines taken away and added count each.
#[test]
fn uc026_tc01_lines_differing() {
    assert_eq!(lines_differing("Version A\n", "Version B\n"), 1);
    assert_eq!(lines_differing("a\nb\nc\n", "a\nc\nd\ne\n"), 3);
    assert_eq!(lines_differing("same\n", "same\n"), 0);
    assert_eq!(lines_differing("", "one\ntwo\n"), 2);
}

/// UC-026 step 2: the day and time, to the second, in the zone given.
#[test]
fn uc026_time_text() {
    let zone = jiff::tz::TimeZone::get("Europe/Zurich").unwrap();
    assert_eq!(time_text_in(T0, &zone), "2025-11-11 10:00:00");
    assert_eq!(time_text_in(u64::MAX, &zone), "unknown");
}

/// UC-026 BR-001: a job for the saver takes an Editing snapshot only when
/// it is due, others always unless the content is that of the newest.
#[test]
fn uc026_jobs_follow_their_cause() {
    let sandbox = Sandbox::new("jobs", &[("Plan.md", "x")]);
    let job = |content: &str, cause, at| SnapshotJob {
        store: sandbox.store(),
        note_file: "Plan.md".into(),
        content: content.into(),
        cause,
        at,
    };

    assert!(job("a", SnapshotCause::Editing, T0)
        .run()
        .unwrap()
        .is_some());
    assert!(job("b", SnapshotCause::Editing, T0 + MINUTE)
        .run()
        .unwrap()
        .is_none());
    assert!(job("b", SnapshotCause::OutsideChange, T0 + MINUTE)
        .run()
        .unwrap()
        .is_some());
}

// ---- Test monkey, package 8a: attacks on paths, symlinks, races, clocks.

/// UC-026 BR-002 (ADR-008): the snapshots stay in the Underline folder. A
/// folder of the snapshots that is a symlink to a place outside must never
/// be written through: taking a snapshot of that note must not put a file
/// outside the workspace.
#[cfg(unix)]
#[test]
fn uc026_monkey_take_never_writes_through_a_symlinked_folder() {
    let sandbox = Sandbox::new("monkey-symlink-take", &[("Plan.md", "x")]);
    let outside = Sandbox::new("monkey-symlink-take-outside", &[]);
    let base = sandbox.0.join(".underline/snapshots");
    fs::create_dir_all(&base).unwrap();
    std::os::unix::fs::symlink(&outside.0, base.join("Plan.md")).unwrap();

    let _ = sandbox
        .store()
        .take("Plan.md", "secret", SnapshotCause::Editing, T0);

    assert_eq!(
        fs::read_dir(&outside.0).unwrap().count(),
        0,
        "a snapshot was written outside the workspace"
    );
}

/// UC-026 BR-003 (ADR-008): removing a note for good removes its snapshots,
/// but never files outside the workspace that a symlinked snapshot folder
/// points to.
#[cfg(unix)]
#[test]
fn uc026_monkey_removal_never_deletes_through_a_symlinked_folder() {
    let sandbox = Sandbox::new("monkey-symlink-remove", &[("Plan.md", "x")]);
    let outside = Sandbox::new("monkey-symlink-remove-outside", &[]);
    let victim = outside.0.join(format!("{T0}-editing.snap"));
    fs::write(&victim, "precious").unwrap();
    let base = sandbox.0.join(".underline/snapshots");
    fs::create_dir_all(&base).unwrap();
    std::os::unix::fs::symlink(&outside.0, base.join("Plan.md")).unwrap();
    let mut workspace = Workspace::open(&sandbox.0).unwrap();

    workspace
        .delete_note_for_good(&NoteId::from_relative("Plan"))
        .unwrap();

    assert!(victim.exists(), "a file outside the workspace was deleted");
}

/// Same for the move that a rename does: files behind a symlinked folder are
/// never moved into the workspace's snapshots.
#[cfg(unix)]
#[test]
fn uc026_monkey_move_never_follows_a_symlinked_folder() {
    let sandbox = Sandbox::new("monkey-symlink-move", &[("Plan.md", "x")]);
    let outside = Sandbox::new("monkey-symlink-move-outside", &[]);
    let victim = outside.0.join(format!("{T0}-editing.snap"));
    fs::write(&victim, "precious").unwrap();
    let base = sandbox.0.join(".underline/snapshots");
    fs::create_dir_all(&base).unwrap();
    std::os::unix::fs::symlink(&outside.0, base.join("Plan.md")).unwrap();

    sandbox.store().moved("Plan.md", "Roadmap.md").unwrap();

    assert!(victim.exists(), "a file outside was moved away");
}

/// UC-026 BR-002 (ADR-008): a snapshot file that is a symlink to a file
/// outside the Underline folder is not a snapshot: listing skips it, so
/// `find` must not hand it out either (restore would read the outside file).
#[cfg(unix)]
#[test]
fn uc026_monkey_find_never_reads_a_symlinked_snapshot() {
    let sandbox = Sandbox::new("monkey-symlink-find", &[("Plan.md", "x")]);
    let outside = Sandbox::new("monkey-symlink-find-outside", &[]);
    let target = outside.0.join("secret.txt");
    fs::write(&target, "outside content").unwrap();
    let folder = sandbox.0.join(".underline/snapshots/Plan.md");
    fs::create_dir_all(&folder).unwrap();
    let name = format!("{T0}-editing.snap");
    std::os::unix::fs::symlink(&target, folder.join(&name)).unwrap();
    let store = sandbox.store();

    assert!(store.list("Plan.md", T0).is_empty());
    assert!(
        matches!(store.find("Plan.md", &name), Err(SnapshotError::Gone)),
        "find returned a snapshot that list does not show"
    );
}

/// UC-026 BR-001: the editing snapshot is due when the newest is older than
/// 10 minutes. A snapshot dated in the future (a clock set back) must not
/// suppress the editing snapshots for days.
#[test]
fn uc026_monkey_future_dated_snapshot_does_not_silence_editing() {
    let sandbox = Sandbox::new("monkey-future", &[("Plan.md", "x")]);
    let store = sandbox.store();
    store
        .take(
            "Plan.md",
            "from the future",
            SnapshotCause::Editing,
            T0 + 20 * DAY,
        )
        .unwrap();

    // Editing at T0 (clock was set back), then again 11 minutes later.
    let first = store.take_for_editing("Plan.md", "edit one", T0).unwrap();
    assert!(
        first.is_some(),
        "no editing snapshot after the clock went back"
    );
}

/// UC-026 BR-002: pruning racing the taking of a snapshot must not lose
/// the snapshot (prune removes folders it finds empty).
#[test]
fn uc026_monkey_pruning_racing_a_take_loses_nothing() {
    let sandbox = Sandbox::new("monkey-prune-race", &[]);
    let store = sandbox.store();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let pruner = {
        let store = store.clone();
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                store.prune(T0);
            }
        })
    };
    let mut lost = 0;
    for n in 0..3000 {
        let file = format!("n{n}.md");
        if store
            .take(&file, "content", SnapshotCause::Editing, T0)
            .is_err()
        {
            lost += 1;
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    pruner.join().unwrap();
    assert_eq!(lost, 0, "{lost} snapshots failed while the pruning ran");
}

/// Crafted names and keys: nothing outside the note's folder is named,
/// whatever the key or note file says.
#[test]
fn uc026_monkey_hostile_names() {
    let sandbox = Sandbox::new("monkey-names", &[("Plan.md", "x")]);
    let store = sandbox.store();
    for note in [
        "../x.md",
        "a/../../x.md",
        "/abs.md",
        "a\\b.md",
        "",
        ".",
        "a//b.md",
        "a/./b.md",
    ] {
        assert!(
            store.take(note, "x", SnapshotCause::Editing, T0).is_err(),
            "{note:?}"
        );
        assert!(store.list(note, T0).is_empty());
    }
    // NUL and very long names fail with an error, they do not panic.
    assert!(store
        .take("a\0b.md", "x", SnapshotCause::Editing, T0)
        .is_err());
    let long = format!("{}.md", "n".repeat(300));
    assert!(store.take(&long, "x", SnapshotCause::Editing, T0).is_err());
    assert!(!sandbox.0.join("x.md").exists());
    assert!(!sandbox.0.join(".underline/x.md").exists());

    let folder = sandbox.0.join(".underline/snapshots/Plan.md");
    fs::create_dir_all(folder.join("5-editing.snap")).unwrap();
    fs::write(sandbox.0.join(".underline/snapshots/x.snap"), "outside").unwrap();
    for key in [
        "../x.snap",
        "..",
        "999999999999999999999-editing.snap",
        "18446744073709551616-editing.snap",
        "5-editing.snap",
        "abc-weird.snap",
        "1-editing-99999999999.snap",
        "1-editing\0.snap",
        "",
        "-editing.snap",
        "+5-editing.snap",
        "١-editing.snap",
    ] {
        assert!(
            matches!(store.find("Plan.md", key), Err(SnapshotError::Gone)),
            "{key:?}"
        );
    }
    assert!(store.list("Plan.md", T0).is_empty());
    assert_eq!(store.prune(T0 + 100 * DAY), 0);
}

/// UC-026 BR-003 rename chain a -> b -> a keeps every snapshot once.
#[test]
fn uc026_monkey_rename_chain_back_and_forth() {
    let sandbox = Sandbox::new("monkey-chain", &[("A.md", "x")]);
    let store = sandbox.store();
    for n in 0..5u64 {
        store
            .take("A.md", &format!("v{n}"), SnapshotCause::Editing, T0)
            .unwrap();
    }
    store.moved("A.md", "B.md").unwrap();
    store
        .take("A.md", "new in a", SnapshotCause::Git, T0)
        .unwrap();
    store.moved("B.md", "A.md").unwrap();
    let all = store.list("A.md", T0);
    assert_eq!(all.len(), 6);
    let mut contents: Vec<String> = all.iter().map(|s| s.content().unwrap()).collect();
    contents.sort();
    assert_eq!(contents, ["new in a", "v0", "v1", "v2", "v3", "v4"]);
    assert!(store.list("B.md", T0).is_empty());
}

/// NFR-005: a line diff of two entirely different 10k line notes (100k in practice) returns
/// in reasonable time.
#[test]
fn uc026_monkey_line_diff_of_huge_different_texts() {
    // Quadratic: 5k lines 3 s, 10k lines 13 s, 20k lines 54 s (debug build);
    // a 100k line note would take about an hour.
    let lines: usize = 10_000;
    let old: String = (0..lines).map(|n| format!("old line {n}\n")).collect();
    let new: String = (0..lines).map(|n| format!("new line {n}\n")).collect();
    let started = std::time::Instant::now();
    let n = lines_differing(&old, &new);
    let took = started.elapsed();
    assert_eq!(n, lines);
    assert!(took.as_secs() < 5, "{lines} lines took {took:?}");
}

/// UC-026 BR-003, UC-007 A5: restoring a deleted note brings its snapshots
/// back. When they cannot be moved back (the snapshot folder is read-only),
/// they must stay with the deleted note or at least not be destroyed: the
/// restored note's snapshots must not be lost.
#[cfg(unix)]
#[test]
fn uc026_monkey_br003_restore_that_cannot_move_snapshots_back_loses_none() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("monkey-restore-ro", &[("Plan.md", "now")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "one", SnapshotCause::Editing, T0)
        .unwrap();
    let mut workspace = Workspace::open(&sandbox.0).unwrap();
    let deleted = workspace
        .delete_note(&NoteId::from_relative("Plan"), T0)
        .unwrap();
    let base = sandbox.0.join(".underline/snapshots");
    fs::create_dir_all(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o555)).unwrap();

    let restored = workspace.restore_note(&deleted, None);

    fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(restored.is_ok());
    let kept_back = store.list("Plan.md", T0).len();
    let kept_with_deleted = fs::read_dir(
        sandbox
            .0
            .join(".underline/deleted")
            .join(T0.to_string())
            .join(DELETED_SNAPSHOTS_FOLDER),
    )
    .map(|d| d.count())
    .unwrap_or(0);
    assert!(
        kept_back + kept_with_deleted > 0,
        "the snapshots of the restored note were destroyed"
    );
}

/// UC-026 step 2: snapshots of the same millisecond, of any cause, get
/// numbers in the order they were taken and list newest first, always the
/// same way.
#[test]
fn uc026_same_millisecond_orders_by_taking() {
    let sandbox = Sandbox::new("same-ms", &[("Plan.md", "x")]);
    let store = sandbox.store();
    for (text, cause) in [
        ("a", SnapshotCause::Restore),
        ("b", SnapshotCause::Editing),
        ("c", SnapshotCause::Git),
    ] {
        store.take("Plan.md", text, cause, T0).unwrap().unwrap();
    }
    let contents: Vec<String> = store
        .list("Plan.md", T0)
        .iter()
        .map(|s| s.content().unwrap())
        .collect();
    assert_eq!(contents, ["c", "b", "a"]);
}

/// UC-026 BR-003: a move never overwrites a snapshot already at the
/// target, also of the same millisecond and cause.
#[test]
fn uc026_br003_a_move_never_overwrites() {
    let sandbox = Sandbox::new("no-overwrite", &[]);
    let store = sandbox.store();
    store
        .take("A.md", "from a", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("B.md", "in b", SnapshotCause::Editing, T0)
        .unwrap();

    store.moved("A.md", "B.md").unwrap();

    let mut contents: Vec<String> = store
        .list("B.md", T0)
        .iter()
        .map(|s| s.content().unwrap())
        .collect();
    contents.sort();
    assert_eq!(contents, ["from a", "in b"]);
}

/// UC-026 BR-002: pruning removes temporary files of writes interrupted
/// more than an hour ago, so their folders can go, and keeps fresh ones.
#[test]
fn uc026_br002_prune_removes_stale_temporary_files() {
    let sandbox = Sandbox::new("stale-temp", &[]);
    let folder = sandbox.0.join(".underline/snapshots/Plan.md");
    fs::create_dir_all(&folder).unwrap();
    let stale = folder.join(".underline-new-1-1");
    let fresh = sandbox.0.join(".underline/snapshots/.underline-new-1-2");
    fs::write(&stale, "half").unwrap();
    fs::write(&fresh, "half").unwrap();
    let old = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
    fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();

    sandbox.store().prune(T0);

    assert!(!folder.exists(), "the folder went with its stale file");
    assert!(fresh.exists());
}

/// UC-026 BR-003, OQ-291: snapshots that cannot follow a renamed note stay
/// where they were, and the workspace says so once.
#[cfg(unix)]
#[test]
fn uc026_br003_a_rename_says_when_snapshots_cannot_follow() {
    let sandbox = Sandbox::new("rename-problem", &[("Plan.md", "x")]);
    let outside = Sandbox::new("rename-problem-outside", &[]);
    let store = sandbox.store();
    store
        .take("Plan.md", "one", SnapshotCause::Editing, T0)
        .unwrap();
    std::os::unix::fs::symlink(
        &outside.0,
        sandbox.0.join(".underline/snapshots/Roadmap.md"),
    )
    .unwrap();
    let mut workspace = Workspace::open(&sandbox.0).unwrap();
    let planned = workspace
        .plan_rename(
            &NoteId::from_relative("Plan"),
            &Rename {
                title: "Roadmap".into(),
                folder: String::new(),
            },
        )
        .unwrap()
        .unwrap();

    workspace.move_note(&planned, "x").unwrap();

    let problem = workspace.take_snapshot_problem().unwrap();
    assert!(problem.starts_with("Roadmap: "), "{problem}");
    assert!(workspace.take_snapshot_problem().is_none());
    assert_eq!(
        store.list("Plan.md", T0).len(),
        1,
        "kept under the old name"
    );
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
}

/// UC-026 BR-003: a snapshot gone from its place while a move runs counts
/// as moved; the others still move.
#[test]
fn uc026_br003_a_move_goes_on_past_a_snapshot_gone_meanwhile() {
    let sandbox = Sandbox::new("move-gone", &[]);
    let store = sandbox.store();
    let gone = store
        .take("A.md", "one", SnapshotCause::Editing, T0)
        .unwrap()
        .unwrap();
    store
        .take("A.md", "two", SnapshotCause::Editing, T0 + 1)
        .unwrap();
    let source = Snapshot::parse(gone.file.clone()).unwrap();
    fs::remove_file(&gone.file).unwrap();

    assert!(move_one(
        &sandbox.0,
        &source,
        &sandbox.0.join(".underline/snapshots/B.md")
    )
    .is_ok());
    store.moved("A.md", "B.md").unwrap();

    assert_eq!(store.list("B.md", T0 + 1).len(), 1);
    assert!(store.list("A.md", T0 + 1).is_empty());
}

// ---- OQ-294, UC-026 A7: the space the snapshots take, and removing them all.

/// UC-026 A7, OQ-294, TC-026-15: the snapshots of the workspace are counted
/// with the space they take; a workspace without any has none.
#[test]
fn uc026_a7_tc15_usage_counts_the_snapshots_and_their_bytes() {
    let sandbox = Sandbox::new("usage", &[("Plan.md", "x"), ("Projects/Trip.md", "y")]);
    let store = sandbox.store();
    assert_eq!(store.usage().unwrap(), SnapshotUsage::default());

    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Plan.md", "1234567890", SnapshotCause::Restore, T0 + MINUTE)
        .unwrap();
    store
        .take("Projects/Trip.md", "abc", SnapshotCause::Git, T0)
        .unwrap();

    assert_eq!(
        store.usage().unwrap(),
        SnapshotUsage {
            count: 3,
            bytes: 18,
            unreadable: 0
        }
    );
}

/// UC-026 A7, OQ-294, TC-026-15: removing all snapshots empties
/// `.underline/snapshots` and nothing else: the notes, the state file and
/// the snapshots of a deleted note stay (BR-003).
#[test]
fn uc026_a7_tc15_remove_all_removes_only_the_snapshots_folder() {
    let sandbox = Sandbox::new(
        "remove-all",
        &[
            ("Plan.md", "x"),
            (".underline/state.yaml", "tabs: []\n"),
            (
                ".underline/deleted/1/.snapshots/Old.md/1762851600000-editing.snap",
                "old",
            ),
        ],
    );
    let store = sandbox.store();
    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("Projects/Trip.md", "abc", SnapshotCause::Git, T0)
        .unwrap();
    fs::write(sandbox.0.join(".underline/snapshots/notes.txt"), "zz").unwrap();

    let removal = store.remove_all().unwrap();

    assert_eq!(
        removal.removed,
        SnapshotUsage {
            count: 2,
            bytes: 10,
            unreadable: 0
        }
    );
    assert_eq!(removal.failed, 0);
    assert!(!sandbox.0.join(".underline/snapshots").exists());
    assert_eq!(store.usage().unwrap(), SnapshotUsage::default());
    assert!(sandbox.0.join("Plan.md").is_file());
    assert!(sandbox.0.join(".underline/state.yaml").is_file());
    assert!(sandbox
        .0
        .join(".underline/deleted/1/.snapshots/Old.md/1762851600000-editing.snap")
        .is_file());
    // A snapshot is taken again afterwards.
    assert!(store
        .take("Plan.md", "later", SnapshotCause::Editing, T0 + DAY)
        .unwrap()
        .is_some());
}

/// UC-026 A7, OQ-294: removing without any snapshot removes nothing and
/// is no error.
#[test]
fn uc026_a7_remove_all_without_snapshots_is_no_error() {
    let sandbox = Sandbox::new("remove-none", &[("Plan.md", "x")]);
    let removal = sandbox.store().remove_all().unwrap();
    assert_eq!(removal.removed, SnapshotUsage::default());
    assert!(sandbox.0.join("Plan.md").is_file());
}

/// UC-026 A7, OQ-294, ADR-031: a link below the snapshots is neither
/// followed nor removed, and the files it points to are neither counted
/// nor removed.
#[cfg(unix)]
#[test]
fn uc026_a7_remove_all_never_follows_a_link_below() {
    let sandbox = Sandbox::new("remove-all-link", &[("Plan.md", "x")]);
    let outside = Sandbox::new("remove-all-link-outside", &[]);
    let victim = outside.0.join(format!("{T0}-editing.snap"));
    fs::write(&victim, "precious").unwrap();
    let store = sandbox.store();
    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    let base = sandbox.0.join(".underline/snapshots");
    std::os::unix::fs::symlink(&outside.0, base.join("Trip.md")).unwrap();
    std::os::unix::fs::symlink(&victim, base.join("Plan.md").join("linked.snap")).unwrap();

    assert_eq!(store.usage().unwrap().count, 1);
    let removal = store.remove_all().unwrap();

    assert_eq!(removal.removed.count, 1);
    assert_eq!(removal.links, 2);
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert!(fs::symlink_metadata(base.join("Trip.md")).is_ok());
}

/// UC-026 A7, OQ-294, ADR-031: when the folder of the snapshots (or the
/// Underline folder) is a link, nothing is counted or removed.
#[cfg(unix)]
#[test]
fn uc026_a7_remove_all_refuses_a_linked_snapshots_folder() {
    let sandbox = Sandbox::new("remove-all-linked-folder", &[("Plan.md", "x")]);
    let outside = Sandbox::new("remove-all-linked-folder-outside", &[]);
    let victim = outside.0.join("Plan.md").join(format!("{T0}-editing.snap"));
    fs::create_dir_all(victim.parent().unwrap()).unwrap();
    fs::write(&victim, "precious").unwrap();
    fs::create_dir_all(sandbox.0.join(".underline")).unwrap();
    std::os::unix::fs::symlink(&outside.0, sandbox.0.join(".underline/snapshots")).unwrap();

    let store = sandbox.store();
    assert!(matches!(store.usage(), Err(SnapshotError::Link(_))));
    assert!(matches!(store.remove_all(), Err(SnapshotError::Link(_))));
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
}

// ---- test-monkey: OQ-294, UC-026 A7 under hostile folders and threads.

/// UC-026 A7, OQ-294 (test-monkey): a folder of the snapshots that cannot be
/// read keeps its snapshots; removing all must say so (`failed` > 0), not
/// report a clean removal while the files stay.
#[cfg(unix)]
#[test]
fn uc026_a7_remove_all_reports_an_unreadable_folder() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("remove-all-unreadable", &[("Plan.md", "x"), ("B.md", "y")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("B.md", "abc", SnapshotCause::Editing, T0)
        .unwrap();
    let locked = sandbox.0.join(".underline/snapshots/B.md");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return; // running as root: permissions do not apply
    }

    let removal = store.remove_all().unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(removal.removed.count, 1);
    assert!(
        locked.join(format!("{T0}-editing.snap")).exists(),
        "the snapshot stays"
    );
    assert!(
        removal.failed > 0,
        "an unreadable folder holding snapshots is reported as failed, not as removed cleanly"
    );
}

/// UC-026 A7, OQ-294 (test-monkey): `usage` never undercounts silently when a
/// folder cannot be read, and neither it nor `remove_all` panics on a
/// read-only folder.
#[cfg(unix)]
#[test]
fn uc026_a7_remove_all_in_a_read_only_folder_counts_the_failures() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("remove-all-readonly", &[("Plan.md", "x")]);
    let store = sandbox.store();
    for n in 0..3 {
        store
            .take(
                "Plan.md",
                &format!("v{n}"),
                SnapshotCause::Editing,
                T0 + n * MINUTE,
            )
            .unwrap();
    }
    let folder = sandbox.0.join(".underline/snapshots/Plan.md");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let usage = store.usage().unwrap();
    let removal = store.remove_all().unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    if removal.removed.count == 3 {
        return; // running as root
    }
    assert_eq!(usage.count, 3);
    assert_eq!(removal.failed, 3);
    assert!(removal.first_error.is_some());
    assert_eq!(store.usage().unwrap().count, 3, "nothing is lost");
}

/// UC-026 A7, ADR-031 (test-monkey): a linked `.underline` refuses both, and
/// a link as a deeper level of a note's folder is skipped by both.
#[cfg(unix)]
#[test]
fn uc026_a7_links_at_every_level_are_never_followed() {
    let sandbox = Sandbox::new("links-levels", &[("Plan.md", "x")]);
    let outside = Sandbox::new("links-levels-out", &[]);
    let victim = outside.0.join("snapshots/Plan.md");
    fs::create_dir_all(&victim).unwrap();
    fs::write(victim.join(format!("{T0}-editing.snap")), "precious").unwrap();
    std::os::unix::fs::symlink(&outside.0, sandbox.0.join(".underline")).unwrap();
    let store = sandbox.store();
    assert!(matches!(store.usage(), Err(SnapshotError::Link(_))));
    assert!(matches!(store.remove_all(), Err(SnapshotError::Link(_))));
    assert!(victim.join(format!("{T0}-editing.snap")).exists());
    fs::remove_file(sandbox.0.join(".underline")).unwrap();

    // A link deep down, and a dangling and a looping one.
    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    let base = sandbox.0.join(".underline/snapshots");
    fs::create_dir_all(base.join("a/b/c")).unwrap();
    std::os::unix::fs::symlink(&outside.0, base.join("a/b/c/deep")).unwrap();
    std::os::unix::fs::symlink(base.join("a"), base.join("a/b/loop")).unwrap();
    std::os::unix::fs::symlink(base.join("nothing"), base.join("dangling")).unwrap();
    assert_eq!(store.usage().unwrap().count, 1);
    let removal = store.remove_all().unwrap();
    assert_eq!(removal.removed.count, 1);
    assert_eq!(removal.links, 3);
    assert!(victim.join(format!("{T0}-editing.snap")).exists());
    assert!(fs::symlink_metadata(base.join("a/b/loop")).is_ok());
}

/// UC-026 A7, OQ-294 (test-monkey): removing all while other threads take,
/// prune and move: no panic, no hang, no error from a take or a move that
/// the removal races with, and afterwards a take works and the counts agree.
#[test]
fn uc026_a7_remove_all_races_with_take_prune_and_move() {
    let sandbox = Sandbox::new("remove-all-race", &[("A.md", "x"), ("B/C.md", "y")]);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut workers = Vec::new();
    for (n, (note, other)) in [
        ("A.md", "D.md"),
        ("B/C.md", "B/E.md"),
        ("F/G/H.md", "F/G/I.md"),
    ]
    .into_iter()
    .enumerate()
    {
        let store = sandbox.store();
        let stop = Arc::clone(&stop);
        let errors = Arc::clone(&errors);
        workers.push(std::thread::spawn(move || {
            let mut i = 0u64;
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                i += 1;
                let at = T0 + i * MINUTE;
                if let Err(e) = store.take(note, &format!("v{n}-{i}"), SnapshotCause::Git, at) {
                    errors.lock().unwrap().push(format!("take {note}: {e}"));
                }
                if i.is_multiple_of(3) {
                    if let Err(e) = store.moved(note, other) {
                        errors.lock().unwrap().push(format!("moved {note}: {e}"));
                    }
                    if let Err(e) = store.moved(other, note) {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("moved back {note}: {e}"));
                    }
                }
                if i.is_multiple_of(5) {
                    store.prune(T0 + 100 * DAY);
                }
                let _ = store.usage();
            }
        }));
    }
    let store = sandbox.store();
    let started = std::time::Instant::now();
    for _ in 0..2000 {
        let removal = store.remove_all().unwrap();
        assert_eq!(removal.failed, 0, "{:?}", removal.first_error);
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(started.elapsed() < Duration::from_secs(60));
    let errors = errors.lock().unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    store.remove_all().unwrap();
    assert_eq!(store.usage().unwrap(), SnapshotUsage::default());
    assert!(store
        .take("A.md", "after", SnapshotCause::Editing, T0)
        .unwrap()
        .is_some());
}

/// UC-026 A7, OQ-294 (test-monkey): many thousands of files and deep folders
/// are counted and removed, and the snapshots of deleted notes stay.
#[test]
fn uc026_a7_remove_all_many_files_and_deep_folders() {
    let sandbox = Sandbox::new(
        "remove-all-many",
        &[(
            ".underline/deleted/1/.snapshots/Old.md/1762851600000-editing.snap",
            "old",
        )],
    );
    let base = sandbox.0.join(".underline/snapshots");
    for d in 0..50 {
        let folder = base.join(format!("f{d}.md"));
        fs::create_dir_all(&folder).unwrap();
        for n in 0..300 {
            fs::write(folder.join(format!("{}-editing.snap", T0 + n)), "ab").unwrap();
        }
    }
    let mut deep = base.clone();
    for _ in 0..100 {
        deep.push("d");
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join(format!("{T0}-git.snap")), "z").unwrap();
    let store = sandbox.store();
    let started = std::time::Instant::now();
    let usage = store.usage().unwrap();
    assert_eq!(usage.count, 15_001);
    let removal = store.remove_all().unwrap();
    assert_eq!(removal.removed.count, 15_001);
    assert_eq!(removal.failed, 0);
    assert!(started.elapsed() < Duration::from_secs(30));
    assert!(!base.exists());
    assert!(sandbox
        .0
        .join(".underline/deleted/1/.snapshots/Old.md/1762851600000-editing.snap")
        .is_file());
}

/// UC-026 A7, OQ-294: a folder that cannot be read is counted as such, so
/// the space shown is "at least" and never too small without saying so.
#[cfg(unix)]
#[test]
fn uc026_a7_usage_counts_an_unreadable_folder() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("usage-unreadable", &[("Plan.md", "x"), ("B.md", "y")]);
    let store = sandbox.store();
    store
        .take("Plan.md", "12345", SnapshotCause::Editing, T0)
        .unwrap();
    store
        .take("B.md", "abc", SnapshotCause::Editing, T0)
        .unwrap();
    let locked = sandbox.0.join(".underline/snapshots/B.md");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable = fs::read_dir(&locked).is_ok();
    let usage = store.usage().unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if readable {
        return; // running as root
    }
    assert_eq!(
        usage,
        SnapshotUsage {
            count: 1,
            bytes: 5,
            unreadable: 1
        }
    );
}
