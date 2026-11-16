//! UC-026 Restore Snapshot in the session (FR-101, FR-102): the list of a
//! note's snapshots, the differences of one with the note as it is now,
//! the whole snapshot, restoring it and taking the restore back.
//!
//! The snapshots themselves are `underline_core::SnapshotStore`'s, in the
//! Underline folder of the workspace (BR-002, ADR-008, ADR-031). The session
//! takes them where a note changes (BR-001): before the autosave or storing
//! at once of a change made in Underline (cause Editing, at most one each
//! 10 minutes, OQ-223), when a note open in a tab is changed outside (UC-004
//! A7), before an agent's change, a replacement in the workspace, a
//! resolved conflict, and a restore. Those of the autosave, of changes from
//! outside, of agents and of conflicts go to the saver's thread, before the
//! note's next save, so they never hold up typing or the autosave
//! (NFR-005); those of a replacement and a restore are taken at once, as
//! the note is then written at once and is not changed without them.
//!
//! Listing, comparing and showing a snapshot read files and parse; the
//! window's commands do that without the session held ([`SnapshotsJob`]),
//! as for the replace of the workspace. Only the newest snapshots are
//! compared with the note for the list, in a bounded time, and every diff
//! has a deadline (NFR-005).
//!
//! Restoring a note open in a tab changes the tab's mirror as one undo
//! step of its own (BR-004, ADR-025): the window is told to fetch the
//! change (`ed-changed`), and edits it had not sent yet are moved over it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use underline_core::{
    diff, lines_differing, snapshot_time_text, NoteFormat, NoteId, Snapshot, SnapshotCause,
    SnapshotError, SnapshotJob, SnapshotStore, Timestamp,
};
use underline_editor_view::{CachedViews, EditorTab};

use super::git::hunks_view;
use super::Session;
use crate::snapshots_view::{
    SnapshotDiffBody, SnapshotDiffView, SnapshotItemView, SnapshotWholeView, SnapshotsNoneView,
    SnapshotsView,
};
use crate::{Event, StatusOffer, UiError, UiErrorKind};

/// The clock of the snapshots, in milliseconds since 1970; injected in
/// tests (BR-001, BR-002).
pub(crate) type Clock = Arc<dyn Fn() -> Timestamp + Send + Sync>;

/// A4: how often the note shown is looked at for a change while its
/// snapshots are shown.
const WATCH_EVERY: Duration = Duration::from_millis(500);

/// A1: what the dialog says first when the note has no snapshot.
pub const NO_SNAPSHOT: &str = "This note has no snapshot yet. Underline takes one:";

/// A1, BR-001: when snapshots are taken, one line each.
pub const SNAPSHOTS_TAKEN: [&str; 6] = [
    "before you change the note, at most once every 10 minutes",
    "before another program changes it while it is open",
    "before an agent changes it",
    "before a replace in the workspace changes it",
    "before git changes it: a pull, a switch of branch, a resolved conflict",
    "before a snapshot of it is restored",
];

/// BR-002: how long snapshots are kept, below the list.
pub const SNAPSHOTS_KEPT: &str = "Kept on this computer for 30 days";

/// NFR-005: how many snapshots of the list are compared with the note.
const COMPARED_AT_MOST: usize = 30;
/// NFR-005: how long comparing the list's snapshots may take in all; the
/// others show no count of lines.
const COMPARING_AT_MOST: Duration = Duration::from_secs(1);

/// How many lines of context the differences show around each change.
const CONTEXT: usize = 3;

/// The session's part of the snapshots.
pub(super) struct Snapshots {
    clock: Clock,
    /// A3: the undo offered while the report of a restore shows.
    undo: Option<UndoRestore>,
    /// A4: the note whose snapshots are shown, and what it was like.
    watched: Option<Watched>,
    /// A4: grows whenever the session or the saver took a snapshot, so the
    /// snapshots' folder is not read to learn of new ones.
    generation: u64,
}

impl Default for Snapshots {
    fn default() -> Self {
        Self {
            clock: Arc::new(underline_core::now),
            undo: None,
            watched: None,
            generation: 0,
        }
    }
}

impl std::fmt::Debug for Snapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshots")
            .field("undo", &self.undo)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// A3: what the undo of a restore gives back.
#[derive(Debug)]
struct UndoRestore {
    /// The report it belongs to.
    status: String,
    note: NoteId,
    /// The snapshot that holds the note's content from before the restore.
    key: String,
}

/// A4: the note whose snapshots are shown.
#[derive(Debug)]
struct Watched {
    note: NoteId,
    looked: Option<Instant>,
    seen: Option<Seen>,
}

/// What the note and its snapshots were like when last looked at.
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    /// The revision of the tab's text, for a note open in a tab.
    rev: Option<u32>,
    /// When the note's file changed last, and its length.
    file: Option<(SystemTime, u64)>,
    /// [`Snapshots::generation`]: snapshots were taken since.
    generation: u64,
}

/// The snapshots of a tab's note: the tab takes them itself (cause
/// Editing) and the session through it.
#[derive(Clone)]
pub(super) struct TabSnapshots {
    pub(super) store: SnapshotStore,
    pub(super) clock: Clock,
}

impl std::fmt::Debug for TabSnapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabSnapshots")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl TabSnapshots {
    /// A snapshot of `content`, the content of the note at `path`, to take
    /// for `cause` now; `None` for a file that is no note of the workspace.
    pub(super) fn job(
        &self,
        path: &Path,
        content: String,
        cause: SnapshotCause,
    ) -> Option<SnapshotJob> {
        let note_file = self.store.note_file(path).ok()?;
        Some(SnapshotJob {
            store: self.store.clone(),
            note_file,
            content,
            cause,
            at: self.now(),
        })
    }

    /// The snapshots' clock now.
    pub(super) fn now(&self) -> Timestamp {
        (self.clock)()
    }
}

/// UC-026 step 2, A1: the list of a note's snapshots, made without the
/// session held ([`SnapshotsJob::run`]).
#[derive(Debug)]
pub struct SnapshotsJob {
    note: NoteId,
    store: SnapshotStore,
    note_file: String,
    path: PathBuf,
    /// The text of the note's tab, else read from its file.
    current: Option<String>,
    now: Timestamp,
}

impl SnapshotsJob {
    /// Lists the snapshots, the newest first, the newest each with how many
    /// lines it differs from the note as it is now.
    #[must_use]
    pub fn run(self) -> SnapshotsView {
        let current = match self.current {
            Some(text) => Ok(text),
            None => std::fs::read_to_string(&self.path).map_err(|e| e.to_string()),
        };
        let started = Instant::now();
        let items: Vec<SnapshotItemView> = self
            .store
            .list(&self.note_file, self.now)
            .iter()
            .enumerate()
            .map(|(index, snapshot)| {
                // NFR-005: the newest are compared, within a time.
                let compare = index < COMPARED_AT_MOST && started.elapsed() < COMPARING_AT_MOST;
                item_view(snapshot, current.as_deref().ok().filter(|_| compare))
            })
            .collect();
        SnapshotsView {
            note: self.note.as_str().to_owned(),
            title: self.note.title(),
            none: items.is_empty().then(|| SnapshotsNoneView {
                lead: NO_SNAPSHOT.to_owned(),
                when: SNAPSHOTS_TAKEN
                    .iter()
                    .map(|&line| line.to_owned())
                    .collect(),
            }),
            kept: SNAPSHOTS_KEPT.to_owned(),
            items,
            problem: current
                .err()
                .map(|why| format!("The note cannot be read: {why}")),
        }
    }
}

/// One snapshot of the list. Only one compared with `current` is read
/// (NFR-005); of the others nothing is read (the listing only names regular
/// files, never links), so whether they hold text shows when one is chosen.
fn item_view(snapshot: &Snapshot, current: Option<&str>) -> SnapshotItemView {
    let (differs, problem) = match current {
        Some(current) => match snapshot.content() {
            Ok(content) => (
                Some(u32::try_from(lines_differing(&content, current)).unwrap_or(u32::MAX)),
                None,
            ),
            Err(error) => (None, Some(capitalized(&error))),
        },
        None => (None, None),
    };
    SnapshotItemView {
        key: snapshot.key(),
        taken: snapshot_time_text(snapshot.taken_at),
        cause: snapshot.cause.label().to_owned(),
        differs,
        problem,
    }
}

/// An error as a sentence of its own.
fn capitalized(error: &SnapshotError) -> String {
    let text = error.to_string();
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// UC-026 step 4, A2: one snapshot compared with the note, or shown whole,
/// without the session held.
#[derive(Debug)]
pub struct SnapshotShowJob {
    note: NoteId,
    key: String,
    snapshot: Result<Snapshot, SnapshotError>,
    path: PathBuf,
    current: Option<String>,
    format: NoteFormat,
    theme: (String, bool),
}

impl SnapshotShowJob {
    fn current(&self) -> Result<String, String> {
        match &self.current {
            Some(text) => Ok(text.clone()),
            None => std::fs::read_to_string(&self.path)
                .map_err(|e| format!("The note cannot be read: {e}")),
        }
    }

    fn content(&self) -> Result<String, String> {
        self.snapshot
            .as_ref()
            .map_err(capitalized)
            .and_then(|snapshot| snapshot.content().map_err(|e| capitalized(&e)))
    }

    /// Step 4: the lines of the snapshot taken away and the lines of the
    /// note added since (with the diff's deadline, NFR-005).
    #[must_use]
    pub fn diff(self) -> SnapshotDiffView {
        let body = match self.content().and_then(|old| Ok((old, self.current()?))) {
            Err(text) => SnapshotDiffBody::Problem { text },
            Ok((old, new)) if old == new => SnapshotDiffBody::Same,
            Ok((old, new)) => SnapshotDiffBody::Hunks {
                hunks: hunks_view(&diff(&old, &new, CONTEXT), None),
            },
        };
        SnapshotDiffView {
            note: self.note.as_str().to_owned(),
            key: self.key,
            body,
        }
    }

    /// A2: the whole snapshot as the facade's blocks, read only. Code is
    /// shown without its colours and figures as their markup's
    /// placeholders: the snapshot is not the note, so nothing is rendered
    /// for it in the background (OQ-292).
    #[must_use]
    pub fn whole(self) -> SnapshotWholeView {
        let note = self.note.as_str().to_owned();
        match self.content() {
            Ok(text) => {
                let mut tab = EditorTab::new(&text, self.format);
                let mut views = CachedViews::new(&self.theme.0, self.theme.1);
                let _ = tab.bring_facade_up_to_date(&mut views);
                SnapshotWholeView {
                    note,
                    key: self.key,
                    blocks: tab.facade_state(self.note.as_str()).blocks,
                    problem: None,
                }
            }
            Err(problem) => SnapshotWholeView {
                note,
                key: self.key,
                blocks: Vec::new(),
                problem: Some(problem),
            },
        }
    }
}

fn invalid(message: String) -> UiError {
    UiError::new(UiErrorKind::Invalid, message)
}

/// What [`Session::snapshot_note`] finds of a note.
struct NoteForSnapshots {
    store: SnapshotStore,
    note_file: String,
    path: PathBuf,
    /// The text of its tab, `None` without one.
    current: Option<String>,
    format: NoteFormat,
}

impl Session {
    /// The clock of the snapshots (BR-001, BR-002).
    pub(super) fn snapshot_clock(&self) -> Clock {
        Arc::clone(&self.snapshots.clock)
    }

    /// Tests: the snapshots' clock is `clock`.
    #[cfg(test)]
    pub(crate) fn set_snapshot_clock(
        &mut self,
        clock: impl Fn() -> Timestamp + Send + Sync + 'static,
    ) {
        self.snapshots.clock = Arc::new(clock);
        let snapshots = self.tab_snapshots();
        if let Some(open) = &mut self.open {
            for (_, tab) in open.tabs.iter_mut() {
                tab.snapshots.clone_from(&snapshots);
            }
        }
    }

    /// The snapshots for a tab of the open workspace.
    #[cfg(test)]
    fn tab_snapshots(&self) -> Option<TabSnapshots> {
        let open = self.open.as_ref()?;
        Some(TabSnapshots {
            store: SnapshotStore::new(open.workspace.root()),
            clock: self.snapshot_clock(),
        })
    }

    /// The snapshots of the open workspace.
    fn snapshot_store(&self) -> Option<SnapshotStore> {
        self.open
            .as_ref()
            .map(|open| SnapshotStore::new(open.workspace.root()))
    }

    /// UC-026 BR-002: another workspace opened: the snapshots older than 30
    /// days go, on a thread of their own, and what was shown or offered of
    /// the workspace before is forgotten.
    pub(super) fn snapshots_opened(&mut self) {
        self.snapshots.undo = None;
        self.snapshots.watched = None;
        let Some(store) = self.snapshot_store() else {
            return;
        };
        let now = (self.snapshots.clock)();
        let pruning = std::thread::Builder::new()
            .name("underline-snapshots".into())
            .spawn(move || {
                store.prune(now);
            });
        if let Err(error) = pruning {
            eprintln!("UC-026: the old snapshots could not be removed: {error}");
        }
    }

    /// UC-026 BR-003, OQ-291: the snapshots of the note renamed, deleted or
    /// restored last could not follow it: the status says so after its own
    /// report. They stay where they were.
    pub(super) fn report_snapshot_problem(&mut self) {
        let Some(problem) = self
            .open
            .as_mut()
            .and_then(|open| open.workspace.take_snapshot_problem())
        else {
            return;
        };
        let report = format!("the snapshots of {problem}; they stay where they were");
        if self.status.is_empty() {
            self.status = format!("Note: {report}");
        } else {
            self.status = format!("{}; {report}", self.status);
        }
    }

    /// The note's file, the text of its tab and its format.
    fn snapshot_note(&self, note: &NoteId) -> Result<NoteForSnapshots, UiError> {
        let open = self
            .open
            .as_ref()
            .ok_or_else(|| invalid("No workspace is open".to_owned()))?;
        let tab = open.tabs.find(note).map(|index| open.tabs.get(index));
        let path = match tab {
            Some(tab) => tab.path.clone(),
            None => open
                .workspace
                .note_path(note)
                .map_err(|error| invalid(error.to_string()))?,
        };
        if tab.is_none() && !path.is_file() {
            return Err(invalid(format!("The note {} is gone", note.title())));
        }
        let store = SnapshotStore::new(open.workspace.root());
        let note_file = store
            .note_file(&path)
            .map_err(|error| invalid(capitalized(&error)))?;
        let format = tab.map_or_else(
            || {
                path.extension()
                    .and_then(|e| NoteFormat::from_extension(&e.to_string_lossy()))
                    .unwrap_or(NoteFormat::Markdown)
            },
            |tab| tab.edit.format,
        );
        Ok(NoteForSnapshots {
            store,
            note_file,
            current: tab.map(|tab| tab.mirror.text()),
            path,
            format,
        })
    }

    /// UC-026 steps 1–2, A1: the snapshots of `note` are shown; the list is
    /// made by the job, and the window is told when the note or its
    /// snapshots change (A4).
    ///
    /// # Errors
    ///
    /// No workspace is open, or `note` is no note of it.
    pub fn snapshots_plan(&mut self, note: &NoteId) -> Result<SnapshotsJob, UiError> {
        let found = self.snapshot_note(note)?;
        // A4: what the list is made of now; a change before the next tick
        // is told too.
        let seen = self.seen(note);
        self.snapshots.watched = Some(Watched {
            note: note.clone(),
            looked: Some(Instant::now()),
            seen,
        });
        Ok(SnapshotsJob {
            note: note.clone(),
            store: found.store,
            note_file: found.note_file,
            path: found.path,
            current: found.current,
            now: (self.snapshots.clock)(),
        })
    }

    /// UC-026 step 4, A2: the snapshot `key` of `note`, to compare with the
    /// note or show whole.
    ///
    /// # Errors
    ///
    /// No workspace is open, or `note` is no note of it.
    pub fn snapshot_plan(&mut self, note: &NoteId, key: &str) -> Result<SnapshotShowJob, UiError> {
        let found = self.snapshot_note(note)?;
        let theme = self.settings.effective().theme;
        Ok(SnapshotShowJob {
            note: note.clone(),
            key: key.to_owned(),
            snapshot: found.store.find(&found.note_file, key),
            path: found.path,
            current: found.current,
            format: found.format,
            theme: (
                theme.label().to_owned(),
                underline_facade::themes::palette(theme).dark,
            ),
        })
    }

    /// UC-026 A6: the snapshots are no longer shown.
    pub fn snapshots_close(&mut self) {
        self.snapshots.watched = None;
    }

    /// UC-026 steps 5–7, A5, BR-004: `note` gets the content of its
    /// snapshot `key`, after a snapshot of what it holds now with the cause
    /// Restore; in its tab as one undo step, and stored at once. The status
    /// says what was done and offers to undo (A3); it is also the answer.
    ///
    /// # Errors
    ///
    /// No workspace is open, `note` is no note of it, or the snapshot is
    /// gone or cannot be read; nothing changed then.
    pub fn snapshot_restore(&mut self, note: &NoteId, key: &str) -> Result<String, UiError> {
        let restored = self.restore_from(note, key)?;
        let status = match restored {
            Restored::Stored { time, undo_key } => {
                let status = format!("Restored {} to the snapshot of {time}", note.title());
                self.snapshots.undo = Some(UndoRestore {
                    status: status.clone(),
                    note: note.clone(),
                    key: undo_key,
                });
                status
            }
            Restored::Same(time) => {
                self.snapshots.undo = None;
                format!("{} already holds the snapshot of {time}", note.title())
            }
            Restored::NotStored(why) => {
                self.snapshots.undo = None;
                why
            }
        };
        self.status.clone_from(&status);
        Ok(status)
    }

    /// UC-026 A3: "Undo" of the status bar: the note gets the content of
    /// the snapshot taken before the restore. The status says so.
    pub fn snapshot_undo_restore(&mut self) -> String {
        let status = &self.status;
        let Some(undo) = self
            .snapshots
            .undo
            .take()
            .filter(|undo| undo.status == *status)
        else {
            return self.status.clone();
        };
        let status = match self.restore_from(&undo.note, &undo.key) {
            Ok(Restored::Stored { .. } | Restored::Same(_)) => {
                format!("Took back the restore of {}", undo.note.title())
            }
            Ok(Restored::NotStored(why)) => why,
            Err(error) => format!(
                "The restore of {} could not be taken back: {}",
                undo.note.title(),
                error.message
            ),
        };
        self.status.clone_from(&status);
        status
    }

    /// Steps 5–6 for the snapshot `key` of `note`.
    ///
    /// Known limit: a background save of the note that is still running is
    /// waited for at most `SETTLE_AT_MOST` (5 s, `settle_saves`); should one
    /// take longer (a hanging disk), the restore goes on and that save may
    /// land after it: the file then holds the older text while the tab
    /// shows the restored one, until the tab's next change is stored. The
    /// snapshot of the text before the restore is kept either way.
    fn restore_from(&mut self, note: &NoteId, key: &str) -> Result<Restored, UiError> {
        // A save of an older text must not land after the restore.
        self.settle_saves();
        let found = self.snapshot_note(note)?;
        let refused =
            |why: String| invalid(format!("{} could not be restored: {why}", note.title()));
        let snapshot = found
            .store
            .find(&found.note_file, key)
            .map_err(|error| refused(error.to_string()))?;
        let content = snapshot
            .content()
            .map_err(|error| refused(error.to_string()))?;
        let time = snapshot_time_text(snapshot.taken_at);
        let at = (self.snapshots.clock)();
        let restored = match found.current {
            Some(_) => self.restore_in_tab(note, (&found.store, &found.note_file), &content, at)?,
            None => {
                let now =
                    std::fs::read_to_string(&found.path).map_err(|e| refused(e.to_string()))?;
                if now == content {
                    return Ok(Restored::Same(time));
                }
                let before = found
                    .store
                    .take(&found.note_file, &now, SnapshotCause::Restore, at)
                    .map_err(|error| refused(error.to_string()))?;
                let undo_key = undo_key(&found.store, &found.note_file, before, at);
                match self.saver.save_now(&found.path, &content) {
                    Ok(()) => Restoring::Done(undo_key),
                    // A5: the file stays as it was.
                    Err(why) => {
                        Restoring::NotStored(format!("{} could not be stored: {why}", note.title()))
                    }
                }
            }
        };
        // A4: a snapshot was taken; the properties of the tab follow the
        // restored note (M-16.2).
        self.snapshot_taken();
        if let Some(open) = self.open.as_mut() {
            let root = open.workspace.root().to_owned();
            if let Some(index) = open.tabs.find(note) {
                open.tabs.get_mut(index).read_properties(&root);
            }
        }
        self.views_dirty = true;
        self.refresh_changes();
        Ok(match restored {
            Restoring::Done(undo_key) => Restored::Stored { time, undo_key },
            Restoring::Same => Restored::Same(time),
            Restoring::NotStored(why) => Restored::NotStored(why),
        })
    }

    /// Steps 5–6, A5 for a note open in a tab: its text from before is kept
    /// as a snapshot (and the file's, when another program changed it and
    /// the tab has not taken it in yet), the tab's mirror gets `content` as
    /// one undo step, and the note is stored at once; when it cannot be,
    /// the tab keeps the restored text, marked as not stored.
    ///
    /// ADR-025: the window sends its edits first (`flushed`). Edits it made
    /// after that and had not sent yet are moved over the restored text when
    /// they come (`ed_sync`), as over any change of the backend: they are
    /// not in the snapshot of the text before, and they stay in the note
    /// after the restore (`uc026_adr025_restore_with_edits_the_window_has_not_sent`).
    fn restore_in_tab(
        &mut self,
        note: &NoteId,
        (store, note_file): (&SnapshotStore, &str),
        content: &str,
        at: Timestamp,
    ) -> Result<Restoring, UiError> {
        let refused =
            |why: String| invalid(format!("{} could not be restored: {why}", note.title()));
        let (old, path, on_disk) = {
            let open = self
                .open
                .as_ref()
                .ok_or_else(|| refused("no workspace is open".into()))?;
            let index = open
                .tabs
                .find(note)
                .ok_or_else(|| refused("its tab is closed".into()))?;
            let tab = open.tabs.get(index);
            // ADR-025 section 3.6: a text from outside the window has not
            // taken yet would be lost.
            if tab.mirror.reload_unconfirmed() {
                return Err(refused(
                    "it is being shown again after a change from outside; try again in a moment"
                        .into(),
                ));
            }
            (tab.mirror.text(), tab.path.clone(), tab.on_disk.clone())
        };
        if old == content {
            return Ok(Restoring::Same);
        }
        // A change from outside not yet taken in is kept too.
        if let Ok(disk) = std::fs::read_to_string(&path) {
            if disk != on_disk && disk != old {
                store
                    .take(note_file, &disk, SnapshotCause::OutsideChange, at)
                    .map_err(|error| refused(error.to_string()))?;
            }
        }
        let before = store
            .take(note_file, &old, SnapshotCause::Restore, at)
            .map_err(|error| refused(error.to_string()))?;
        let undo_key = undo_key(store, note_file, before, at);
        let started = Instant::now();
        let (range, text) = super::replace::span_edit(&old, content);
        self.change_mirror(note, &[(range, text)], true, false)?;
        let stored = self.saver.save_now(&path, content);
        let Some(tab) = self.open.as_mut().and_then(|open| {
            let index = open.tabs.find(note)?;
            Some(open.tabs.get_mut(index))
        }) else {
            return Ok(Restoring::Done(undo_key));
        };
        match stored {
            Ok(()) => {
                content.clone_into(&mut tab.on_disk);
                tab.saving_text = None;
                tab.autosave.stored(started);
                tab.error = None;
                tab.deleted = false;
                Ok(Restoring::Done(undo_key))
            }
            Err(why) => {
                // A5, UC-005 A14: kept in the tab, marked as not stored.
                tab.changed(started);
                tab.autosave.failed(Instant::now());
                tab.error = Some(why.clone());
                Ok(Restoring::NotStored(format!(
                    "{} could not be stored: {why}",
                    tab.title
                )))
            }
        }
    }

    /// A3: the undo is offered while the report of the restore shows.
    pub(super) fn restore_offer(&mut self) -> Option<StatusOffer> {
        let status = &self.status;
        self.snapshots.undo.take_if(|undo| undo.status != *status);
        self.snapshots
            .undo
            .as_ref()
            .map(|_| StatusOffer::UndoRestore)
    }

    /// A3: the report of a restore stays while it offers its undo.
    pub(super) fn restore_report_kept(&self) -> bool {
        self.snapshots
            .undo
            .as_ref()
            .is_some_and(|undo| undo.status == self.status)
    }

    /// A4: while snapshots are shown, the window is told when the note or
    /// its snapshots changed (`snapshots-changed`), at most every 500 ms.
    pub(super) fn snapshots_event(&mut self, now: Instant) -> Option<Event> {
        let watched = self.snapshots.watched.as_ref()?;
        if watched
            .looked
            .is_some_and(|looked| now.saturating_duration_since(looked) < WATCH_EVERY)
        {
            return None;
        }
        let note = watched.note.clone();
        let seen = self.seen(&note);
        let watched = self.snapshots.watched.as_mut()?;
        watched.looked = Some(now);
        if watched.seen == seen {
            return None;
        }
        watched.seen = seen;
        Some(Event::SnapshotsChanged(note.as_str().to_owned()))
    }

    /// What `note` and its snapshots are like now: one look at the file's
    /// time and length; the snapshots are known by the generation, without
    /// reading their folder.
    fn seen(&self, note: &NoteId) -> Option<Seen> {
        let open = self.open.as_ref()?;
        let tab = open.tabs.find(note).map(|index| open.tabs.get(index));
        let path = match tab {
            Some(tab) => tab.path.clone(),
            None => open.workspace.note_path(note).ok()?,
        };
        let file = std::fs::metadata(&path)
            .ok()
            .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
        Some(Seen {
            rev: tab.map(|tab| tab.mirror.rev()),
            file,
            generation: self.snapshots.generation,
        })
    }

    /// A4: a snapshot was taken (by the session or the saver).
    pub(super) fn snapshot_taken(&mut self) {
        self.snapshots.generation += 1;
    }

    /// UC-026 BR-001, OQ-291: what became of the snapshots the saver took.
    /// Those it could not take are said in the status bar, how many and the
    /// first one's reason; the notes were stored all the same. A report
    /// with an offer, or of a save that failed, is not replaced.
    pub(super) fn take_snapshot_outcomes(&mut self) {
        let outcomes = self.saver.poll_snapshots();
        if outcomes.iter().any(|outcome| outcome.result == Ok(true)) {
            self.snapshot_taken();
        }
        let failures: Vec<(&String, &String)> = outcomes
            .iter()
            .filter_map(|outcome| Some((&outcome.note_file, outcome.result.as_ref().err()?)))
            .collect();
        let Some((note_file, why)) = failures.first() else {
            return;
        };
        // A note that could not be stored (UC-005 A14) matters more.
        let failed_save = self
            .open
            .as_ref()
            .is_some_and(|open| open.tabs.iter().any(|(_, tab)| tab.error.is_some()));
        if self.status_kept() || failed_save {
            eprintln!("UC-026: no snapshot of {note_file} could be taken: {why}");
            return;
        }
        self.status = match failures.len() {
            1 => format!("No snapshot of {note_file} could be taken: {why}"),
            n => format!("{n} snapshots could not be taken; the first, of {note_file}: {why}"),
        };
    }
}

/// The snapshot that holds the note's content from before a restore: the
/// one just taken, else the newest, which holds the same content.
fn undo_key(
    store: &SnapshotStore,
    note_file: &str,
    taken: Option<Snapshot>,
    at: Timestamp,
) -> String {
    taken
        .or_else(|| store.list(note_file, at).into_iter().next())
        .map(|snapshot| snapshot.key())
        .unwrap_or_default()
}

/// How a restore in a tab or a file ended.
enum Restoring {
    Done(String),
    Same,
    NotStored(String),
}

/// How a restore ended, for the status.
enum Restored {
    /// Stored; the snapshot's time and the snapshot of the content before.
    Stored { time: String, undo_key: String },
    /// The note holds the snapshot's content already.
    Same(String),
    /// A5: the status, why it could not be stored.
    NotStored(String),
}

#[cfg(test)]
mod tests;
