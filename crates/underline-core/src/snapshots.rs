//! UC-026 Restore Snapshot (FR-101, FR-102): earlier versions of the notes,
//! kept in the Underline folder of the workspace on this computer only
//! (BR-002, ADR-008), whether the workspace is a git repository or not
//! (BR-005).
//!
//! Each note has a folder of its own below `.underline/snapshots/`, named
//! by the note's file as it lies in the workspace (`Projects/Plan.md`).
//! Each snapshot is one file in it holding the note's bytes exactly as they
//! were (a byte order mark and line endings included), named by when it
//! was taken and why: `1762851600000-editing.snap`. No list is kept beside
//! them that could get out of step; a file whose name is not of this form
//! is left alone and never listed.
//!
//! Every snapshot is written whole or not at all (ADR-005), and never
//! replaces another one, also when it is moved. Taking a snapshot of a note
//! holds a lock of that note's folder (one of a fixed set of locks, by the
//! folder), so two threads of this process (the saver and the session)
//! never take the same content twice one after the other (BR-001: no
//! snapshot of the content of the newest). The lock is held while the
//! snapshot is flushed to disk, as the check and the write belong together;
//! other notes are not held up by it.
//!
//! Known limit (OQ-291, ADR-031): two processes on one workspace (the window
//! and an MCP server without a window) do not share the lock. They may each
//! take the same content once, a duplicate; they never overwrite each other,
//! as every snapshot is a new file.
//!
//! No link is ever followed below the workspace: a folder or file of the
//! snapshots that is a symbolic link is not read, written, moved or removed
//! ([`SnapshotError::Link`]).
//!
//! The snapshots of a deleted note lie in the folder of its deletion, in
//! `.snapshots/` beside the note (BR-003); a hidden folder never holds a
//! note, so they are never taken for a deleted note.

use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use similar::{Algorithm, DiffTag, TextDiff};

use crate::atomic::write_new_atomic_with;
use crate::state::{underline_folder, Timestamp, UNDERLINE_FOLDER};

/// The folder of the snapshots inside the Underline folder.
pub const SNAPSHOTS_FOLDER: &str = "snapshots";
/// The folder of a deleted note's snapshots inside the folder of its
/// deletion (UC-026 BR-003).
pub const DELETED_SNAPSHOTS_FOLDER: &str = ".snapshots";
/// How long a snapshot is kept, in milliseconds (UC-026 BR-002, OQ-224).
pub const KEEP_SNAPSHOTS_MS: Timestamp = 30 * 24 * 60 * 60 * 1000;
/// UC-026 BR-001, OQ-223: a change made in Underline takes a snapshot when
/// the newest snapshot of the note is older than this, in milliseconds.
pub const EDITING_SNAPSHOT_AFTER_MS: Timestamp = 10 * 60 * 1000;
/// NFR-005: how long one line diff of a snapshot with its note may take;
/// after it the diff is coarser (whole changed parts), never wrong.
pub const DIFF_DEADLINE: Duration = Duration::from_millis(200);
/// The extension of a snapshot file.
const EXTENSION: &str = "snap";
/// How often a write is tried when its folder vanished meanwhile (pruning
/// removes folders it finds empty).
const WRITE_ATTEMPTS: usize = 5;
/// How many snapshots of one note one millisecond can hold.
const MAX_NUMBER: u32 = 1000;
/// A temporary file of an interrupted write older than this is removed by
/// pruning.
const STALE_TEMPORARY: Duration = Duration::from_secs(60 * 60);
/// How many locks the notes' folders share.
const LOCKS: usize = 64;

/// The locks of the notes' folders: the check against the newest snapshot
/// and the write belong together.
static TAKING: [Mutex<()>; LOCKS] = [const { Mutex::new(()) }; LOCKS];

/// The lock of the note's snapshot folder `folder`.
fn lock_of(folder: &Path) -> MutexGuard<'static, ()> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    folder.hash(&mut hasher);
    let index = usize::try_from(hasher.finish() % LOCKS as u64).unwrap_or(0);
    TAKING[index]
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Why a snapshot was taken (UC-026 BR-001; entity SNAPSHOT.cause).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotCause {
    /// A change of the note in Underline, after a while without a snapshot.
    Editing,
    /// Another program changed a note open in a tab (UC-004 A7).
    OutsideChange,
    /// An agent changes the note (UC-019).
    AgentChange,
    /// A replacement in the workspace changes the note (UC-022).
    ReplaceInWorkspace,
    /// A pull, a sync, a switch of branch or the resolution of a conflict
    /// changes the note (UC-013 to UC-015).
    Git,
    /// A snapshot is restored (UC-026 step 6).
    Restore,
}

impl SnapshotCause {
    /// All causes, in the order of BR-001.
    pub const ALL: [Self; 6] = [
        Self::Editing,
        Self::OutsideChange,
        Self::AgentChange,
        Self::ReplaceInWorkspace,
        Self::Git,
        Self::Restore,
    ];

    /// The cause as the list of snapshots names it (UC-026 step 2): what
    /// the snapshot was taken before.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Editing => "Before editing",
            Self::OutsideChange => "Before an outside change",
            Self::AgentChange => "Before an agent's change",
            Self::ReplaceInWorkspace => "Before a replace in the workspace",
            Self::Git => "Before a git change",
            Self::Restore => "Before a restore",
        }
    }

    /// The order of the causes in BR-001, which orders snapshots of the
    /// same millisecond and number.
    fn rank(self) -> usize {
        Self::ALL
            .iter()
            .position(|cause| *cause == self)
            .unwrap_or_default()
    }

    /// The cause in a snapshot's file name.
    fn slug(self) -> &'static str {
        match self {
            Self::Editing => "editing",
            Self::OutsideChange => "outside",
            Self::AgentChange => "agent",
            Self::ReplaceInWorkspace => "replace",
            Self::Git => "git",
            Self::Restore => "restore",
        }
    }

    fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cause| cause.slug() == slug)
    }
}

/// A snapshot of a note (entity SNAPSHOT): when and why it was taken; its
/// content is read from its file when needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// When it was taken.
    pub taken_at: Timestamp,
    /// Why it was taken.
    pub cause: SnapshotCause,
    /// Told apart from another snapshot of the same millisecond.
    number: u32,
    /// Its file.
    file: PathBuf,
}

impl Snapshot {
    /// The name that tells this snapshot from the note's others, which the
    /// window sends back ([`SnapshotStore::find`]).
    #[must_use]
    pub fn key(&self) -> String {
        self.file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The content of the note when the snapshot was taken, byte for byte.
    ///
    /// # Errors
    ///
    /// The snapshot is gone, cannot be read, or holds no text.
    pub fn content(&self) -> Result<String, SnapshotError> {
        // A link is never followed (ADR-031).
        match fs::symlink_metadata(&self.file) {
            Ok(meta) if !meta.is_file() => return Err(SnapshotError::Gone),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(SnapshotError::Gone)
            }
            _ => {}
        }
        match fs::read(&self.file) {
            Ok(bytes) => String::from_utf8(bytes).map_err(|_| SnapshotError::NotText),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Err(SnapshotError::Gone),
            Err(error) => Err(SnapshotError::Read(error)),
        }
    }

    /// Reads a snapshot's file name: `{taken_at}-{cause}[-{number}].snap`.
    fn parse(file: PathBuf) -> Option<Self> {
        let name = file.file_name()?.to_str()?;
        let stem = name.strip_suffix(EXTENSION)?.strip_suffix('.')?;
        let mut parts = stem.split('-');
        let taken_at = parts.next()?;
        if taken_at.is_empty() || !taken_at.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let taken_at = taken_at.parse().ok()?;
        let cause = SnapshotCause::from_slug(parts.next()?)?;
        let number = match parts.next() {
            Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => n.parse().ok()?,
            Some(_) => return None,
            None => 0,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            taken_at,
            cause,
            number,
            file,
        })
    }

    /// Newest first: by time, then by the number of the same millisecond
    /// (given in the order they were taken), then by cause, so the order
    /// never depends on the order of the folder.
    fn newer_first(a: &Self, b: &Self) -> std::cmp::Ordering {
        b.taken_at
            .cmp(&a.taken_at)
            .then(b.number.cmp(&a.number))
            .then(b.cause.rank().cmp(&a.cause.rank()))
    }

    /// The file name of a snapshot taken at `taken_at` for `cause`, the
    /// `number`th of that millisecond.
    fn name(taken_at: Timestamp, cause: SnapshotCause, number: u32) -> String {
        match number {
            0 => format!("{taken_at}-{}.{EXTENSION}", cause.slug()),
            n => format!("{taken_at}-{}-{n}.{EXTENSION}", cause.slug()),
        }
    }
}

/// Why a snapshot could not be read, taken or moved.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// The window named a snapshot the note does not have (any more).
    #[error("the snapshot is gone")]
    Gone,
    /// The file of the snapshot holds no text.
    #[error("the snapshot is damaged: it holds no text")]
    NotText,
    /// The file of the snapshot is there but cannot be read, with the
    /// system's reason.
    #[error("the snapshot cannot be read: {0}")]
    Read(io::Error),
    /// The Underline folder cannot take it.
    #[error("the snapshot cannot be kept: {0}")]
    Write(io::Error),
    /// The note's file lies outside the workspace or is no note's file.
    #[error("{0} is not a note of the workspace")]
    NotANote(String),
    /// Snapshots could not be moved to their note's new place (a rename, a
    /// deletion, a restored note), with the system's reason; they stay
    /// where they were.
    #[error("the snapshots cannot be moved: {0}")]
    Move(io::Error),
    /// A folder or file of the snapshots is a symbolic link, which is never
    /// followed.
    #[error("the snapshots of {0} are behind a link, which is not followed")]
    Link(String),
}

/// The snapshots of the notes of one workspace (UC-026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotStore {
    root: PathBuf,
}

impl SnapshotStore {
    /// The snapshots of the workspace at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_owned(),
        }
    }

    /// The workspace's root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder of all snapshots.
    fn all_folder(&self) -> PathBuf {
        self.root.join(UNDERLINE_FOLDER).join(SNAPSHOTS_FOLDER)
    }

    /// The note's file `path` (inside the workspace) as the snapshots name
    /// it: relative, with `/` between folders.
    ///
    /// # Errors
    ///
    /// `path` is not inside the workspace or names no file of a note.
    pub fn note_file(&self, path: &Path) -> Result<String, SnapshotError> {
        let not_a_note = || SnapshotError::NotANote(path.display().to_string());
        let relative = path.strip_prefix(&self.root).map_err(|_| not_a_note())?;
        let parts: Vec<String> = relative
            .components()
            .map(|part| match part {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Option<_>>()
            .ok_or_else(not_a_note)?;
        let file = parts.join("/");
        folder_in(&self.all_folder(), &file).ok_or_else(not_a_note)?;
        Ok(file)
    }

    /// The folder of the snapshots of the note's file `note_file`, which
    /// is no link and lies below none.
    fn folder(&self, note_file: &str) -> Result<PathBuf, SnapshotError> {
        let folder = folder_in(&self.all_folder(), note_file)
            .ok_or_else(|| SnapshotError::NotANote(note_file.to_owned()))?;
        no_link_below(&self.root, &folder)?;
        Ok(folder)
    }

    /// The folder of the snapshots kept with the deleted note's file
    /// `note_file` in the folder of its deletion, `deletion`; no link.
    fn deleted_folder(&self, deletion: &Path, note_file: &str) -> Result<PathBuf, SnapshotError> {
        let folder = folder_in(&deletion.join(DELETED_SNAPSHOTS_FOLDER), note_file)
            .ok_or_else(|| SnapshotError::NotANote(note_file.to_owned()))?;
        no_link_below(&self.root, &folder)?;
        Ok(folder)
    }

    /// UC-026 step 2: the snapshots of the note's file `note_file` kept at
    /// `now`, the newest first; those older than 30 days are left out
    /// (BR-002), as are files that are no snapshot.
    #[must_use]
    pub fn list(&self, note_file: &str, now: Timestamp) -> Vec<Snapshot> {
        let Ok(folder) = self.folder(note_file) else {
            return Vec::new();
        };
        let mut snapshots: Vec<Snapshot> = snapshots_in(&folder)
            .into_iter()
            .filter(|snapshot| kept_at(snapshot.taken_at, now))
            .collect();
        snapshots.sort_by(Snapshot::newer_first);
        snapshots
    }

    /// The newest snapshot of the note's file still kept at `now`.
    fn newest(folder: &Path, now: Timestamp) -> Option<Snapshot> {
        snapshots_in(folder)
            .into_iter()
            .filter(|snapshot| kept_at(snapshot.taken_at, now))
            .min_by(Snapshot::newer_first)
    }

    /// UC-026 A2, step 5: the snapshot `key` of the note's file, as the
    /// window names it.
    ///
    /// # Errors
    ///
    /// [`SnapshotError::Gone`] when the note has no such snapshot any more,
    /// also when `key` names no snapshot file at all or a link.
    pub fn find(&self, note_file: &str, key: &str) -> Result<Snapshot, SnapshotError> {
        let folder = self.folder(note_file)?;
        if key.contains(['/', '\\', '\0']) || key.starts_with('.') {
            return Err(SnapshotError::Gone);
        }
        let snapshot = Snapshot::parse(folder.join(key)).ok_or(SnapshotError::Gone)?;
        if fs::symlink_metadata(&snapshot.file).is_ok_and(|meta| meta.is_file()) {
            Ok(snapshot)
        } else {
            Err(SnapshotError::Gone)
        }
    }

    /// UC-026 BR-001: takes a snapshot of `content`, the note's content in
    /// the file `note_file`, at `at` for `cause`. `None` when the newest
    /// snapshot of the note holds the same content, so nothing was taken.
    ///
    /// # Errors
    ///
    /// The snapshot could not be written, or its folder is a link; nothing
    /// is left half written.
    pub fn take(
        &self,
        note_file: &str,
        content: &str,
        cause: SnapshotCause,
        at: Timestamp,
    ) -> Result<Option<Snapshot>, SnapshotError> {
        self.take_if(note_file, content, cause, at, |_| true)
    }

    /// UC-026 BR-001, OQ-223, cause Editing: takes a snapshot of `content`
    /// before a change made in Underline, only when the newest snapshot of
    /// the note is older than 10 minutes at `at`, or there is none. A newest
    /// snapshot dated after `at` (the clock was set back) counts as old.
    ///
    /// # Errors
    ///
    /// The snapshot could not be written.
    pub fn take_for_editing(
        &self,
        note_file: &str,
        content: &str,
        at: Timestamp,
    ) -> Result<Option<Snapshot>, SnapshotError> {
        self.take_if(note_file, content, SnapshotCause::Editing, at, |newest| {
            newest.is_none_or(|newest| {
                newest.taken_at > at || at - newest.taken_at > EDITING_SNAPSHOT_AFTER_MS
            })
        })
    }

    fn take_if(
        &self,
        note_file: &str,
        content: &str,
        cause: SnapshotCause,
        at: Timestamp,
        due: impl FnOnce(Option<&Snapshot>) -> bool,
    ) -> Result<Option<Snapshot>, SnapshotError> {
        let folder = self.folder(note_file)?;
        let _taking = lock_of(&folder);
        let newest = Self::newest(&folder, at);
        if !due(newest.as_ref()) {
            return Ok(None);
        }
        // BR-001: never the content of the newest snapshot again. A newest
        // snapshot that cannot be read does not count.
        if let Some(newest) = &newest {
            if fs::read(&newest.file).is_ok_and(|bytes| bytes == content.as_bytes()) {
                return Ok(None);
            }
        }
        let mut last = io::Error::from(io::ErrorKind::NotFound);
        // Pruning may remove the folder between making it and writing.
        for _ in 0..WRITE_ATTEMPTS {
            underline_folder(&self.root).map_err(SnapshotError::Write)?;
            match create_private_folders(&folder) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    last = error;
                    continue;
                }
                Err(error) => return Err(SnapshotError::Write(error)),
            }
            no_link_below(&self.root, &folder)?;
            let mut number = next_number(&folder, at);
            loop {
                if number >= MAX_NUMBER {
                    return Err(SnapshotError::Write(io::Error::from(
                        io::ErrorKind::AlreadyExists,
                    )));
                }
                let file = folder.join(Snapshot::name(at, cause, number));
                match write_new_atomic_with(&file, content.as_bytes(), private_file().as_ref()) {
                    Ok(()) => {
                        return Ok(Some(Snapshot {
                            taken_at: at,
                            cause,
                            number,
                            file,
                        }))
                    }
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => number += 1,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        last = error;
                        break;
                    }
                    Err(error) => return Err(SnapshotError::Write(error)),
                }
            }
        }
        Err(SnapshotError::Write(last))
    }

    /// UC-026 BR-003, UC-006: the snapshots of the note's file `from` follow
    /// it to `to`. Files of other notes in the folder (a folder of notes
    /// named like a note's file) stay. A folder of `from` that is a link
    /// holds nothing of the workspace and is left alone.
    ///
    /// # Errors
    ///
    /// A snapshot could not be moved, or the folder of `to` is a link; those
    /// moved before stay moved, none is lost or overwritten.
    pub fn moved(&self, from: &str, to: &str) -> Result<(), SnapshotError> {
        let Ok(from) = self.folder(from) else {
            return Ok(());
        };
        let to = self.folder(to)?;
        move_snapshots(&self.root, &from, &to)
    }

    /// UC-026 BR-003, UC-007 step 4: the snapshots of the note's file
    /// `note_file` go to the folder of its deletion, `deletion`.
    ///
    /// # Errors
    ///
    /// They could not be moved, or a folder is a link; those not moved stay
    /// where they were.
    pub(crate) fn to_deleted(&self, note_file: &str, deletion: &Path) -> Result<(), SnapshotError> {
        let from = self.folder(note_file)?;
        let to = self.deleted_folder(deletion, note_file)?;
        move_snapshots(&self.root, &from, &to)
    }

    /// UC-026 BR-003, UC-007 A5: the snapshots kept with the deleted note
    /// `deleted_file` in the folder of its deletion come back, to the
    /// note's file `note_file`.
    ///
    /// # Errors
    ///
    /// They could not be moved, or a folder is a link; those not moved stay
    /// with the deleted note.
    pub(crate) fn back_from_deleted(
        &self,
        deletion: &Path,
        deleted_file: &str,
        note_file: &str,
    ) -> Result<(), SnapshotError> {
        let from = self.deleted_folder(deletion, deleted_file)?;
        let to = self.folder(note_file)?;
        move_snapshots(&self.root, &from, &to)
    }

    /// UC-026 BR-003, UC-007 A8: a note removed for good takes its
    /// snapshots with it; a folder that is a link is left alone.
    pub(crate) fn remove(&self, note_file: &str) {
        if let Ok(folder) = self.folder(note_file) {
            remove_snapshots(&folder);
        }
    }

    /// UC-026 BR-003: removes the snapshots kept with the deleted note
    /// `deleted_file` in the folder of its deletion, and the folders that
    /// leaves empty up to `.snapshots`; a link is left alone.
    pub(crate) fn forget_deleted(&self, deletion: &Path, deleted_file: &str) {
        let base = deletion.join(DELETED_SNAPSHOTS_FOLDER);
        let Ok(folder) = self.deleted_folder(deletion, deleted_file) else {
            return;
        };
        remove_snapshots(&folder);
        let mut dir = folder.parent();
        while let Some(parent) = dir {
            if !parent.starts_with(&base) || fs::remove_dir(parent).is_err() {
                break;
            }
            dir = parent.parent();
        }
    }

    /// UC-026 BR-002: removes every snapshot of the workspace older than 30
    /// days at `now`, temporary files of writes interrupted more than an
    /// hour ago, and the folders that leaves empty; how many snapshots were
    /// removed. Other files and links stay.
    pub fn prune(&self, now: Timestamp) -> usize {
        let all = self.all_folder();
        if no_link_below(&self.root, &all).is_err() {
            return 0;
        }
        prune_folder(&all, now)
    }

    /// UC-026 A7, OQ-294: how many snapshots the workspace keeps and the
    /// space everything below `.underline/snapshots` takes on the disk
    /// (temporary files of writes included, as removing them frees it too).
    /// The snapshots of deleted notes are not counted: they belong to the
    /// deleted notes (BR-003). A link below is neither followed nor counted.
    ///
    /// # Errors
    ///
    /// [`SnapshotError::Link`] when the folder of the snapshots, or one
    /// above it in the workspace, is a symbolic link.
    pub fn usage(&self) -> Result<SnapshotUsage, SnapshotError> {
        let all = self.all_folder();
        no_link_below(&self.root, &all)?;
        let mut usage = SnapshotUsage::default();
        usage_of(&all, &mut usage);
        Ok(usage)
    }

    /// UC-026 A7, OQ-294: removes everything below `.underline/snapshots`
    /// and that folder itself, and nothing else: the snapshots of deleted
    /// notes in their `.snapshots` stay (BR-003). A symbolic link below is
    /// never followed nor removed (ADR-031); it stays with the folders
    /// above it. A file that cannot be removed does not stop the others.
    ///
    /// # Errors
    ///
    /// [`SnapshotError::Link`] when the folder of the snapshots, or one
    /// above it in the workspace, is a symbolic link; nothing is removed
    /// then.
    pub fn remove_all(&self) -> Result<SnapshotRemoval, SnapshotError> {
        let all = self.all_folder();
        no_link_below(&self.root, &all)?;
        let mut removal = SnapshotRemoval::default();
        remove_all_in(&all, &mut removal);
        Ok(removal)
    }
}

/// UC-026 A7, OQ-294: the space the snapshots of a workspace take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotUsage {
    /// How many snapshots there are.
    pub count: u64,
    /// How many bytes the files below the folder of the snapshots take.
    pub bytes: u64,
    /// How many folders below could not be read; with any, `count` and
    /// `bytes` are "at least".
    pub unreadable: u64,
}

/// UC-026 A7, OQ-294: what [`SnapshotStore::remove_all`] removed and what
/// it left.
#[derive(Debug, Default)]
pub struct SnapshotRemoval {
    /// The snapshots removed and the space that freed.
    pub removed: SnapshotUsage,
    /// How many files could not be removed, and folders that could not
    /// be read (their snapshots stay).
    pub failed: u64,
    /// The system's reason for the first of them.
    pub first_error: Option<io::Error>,
    /// How many symbolic links were left as they are.
    pub links: u64,
}

/// Adds the snapshots and files below `folder` to `usage`; links are
/// skipped.
fn usage_of(folder: &Path, usage: &mut SnapshotUsage) {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(_) => {
            usage.unreadable += 1;
            return;
        }
    };
    for entry in entries.filter_map(Result::ok) {
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.is_dir() {
            usage_of(&entry.path(), usage);
        } else if meta.is_file() {
            usage.bytes = usage.bytes.saturating_add(meta.len());
            if Snapshot::parse(entry.path()).is_some() {
                usage.count += 1;
            }
        }
    }
}

/// Removes the files and folders below `folder`, then `folder` when that
/// leaves it empty; links stay. The folders below are done first, so the
/// lock of a note's folder (taken while its files go) is never held twice.
fn remove_all_in(folder: &Path, removal: &mut SnapshotRemoval) {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        // Removed meanwhile (pruning, a move): nothing left to remove.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        // A folder that cannot be read keeps what it holds: said, not hidden.
        Err(error) => {
            removal.failed += 1;
            removal.first_error.get_or_insert(error);
            return;
        }
    };
    let mut files = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            removal.links += 1;
        } else if meta.is_dir() {
            remove_all_in(&path, removal);
        } else {
            files.push((path, meta.len()));
        }
    }
    // Not while a snapshot is being taken into it; one taken after this
    // creates the folder again.
    let _taking = lock_of(folder);
    for (file, len) in files {
        let snapshot = Snapshot::parse(file.clone()).is_some();
        match fs::remove_file(&file) {
            Ok(()) => {
                removal.removed.bytes = removal.removed.bytes.saturating_add(len);
                removal.removed.count += u64::from(snapshot);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                removal.failed += 1;
                removal.first_error.get_or_insert(error);
            }
        }
    }
    // Not empty (a link, a file that stayed): the folder stays too.
    let _ = fs::remove_dir(folder);
}

/// Errors when a part of `path` below `root` that exists is a symbolic
/// link (ADR-031: no link is followed).
fn no_link_below(root: &Path, path: &Path) -> Result<(), SnapshotError> {
    if path.strip_prefix(root).is_err() {
        return Err(SnapshotError::NotANote(path.display().to_string()));
    }
    match link_below(root, path) {
        Some(link) => Err(SnapshotError::Link(link.display().to_string())),
        None => Ok(()),
    }
}

/// ADR-031, OQ-208: the first part of `path` below `root` that exists and
/// is a symbolic link; `None` when there is none, or when `path` is not
/// below `root`. A file of a workspace the user cloned is read and written
/// only where no link leads elsewhere.
#[must_use]
pub fn link_below(root: &Path, path: &Path) -> Option<std::path::PathBuf> {
    let below = path.strip_prefix(root).ok()?;
    let mut at = root.to_owned();
    for part in below.components() {
        at.push(part);
        match fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return Some(at),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    None
}

/// The number the next snapshot taken at `at` in `folder` gets: after
/// every one of that millisecond, whatever its cause.
fn next_number(folder: &Path, at: Timestamp) -> u32 {
    snapshots_in(folder)
        .iter()
        .filter(|snapshot| snapshot.taken_at == at)
        .map(|snapshot| snapshot.number.saturating_add(1))
        .max()
        .unwrap_or(0)
}

/// Whether a snapshot taken at `taken_at` is still kept at `now` (BR-002):
/// for 30 days; one taken "later" than now (a clock set back) is kept.
fn kept_at(taken_at: Timestamp, now: Timestamp) -> bool {
    now.saturating_sub(taken_at) <= KEEP_SNAPSHOTS_MS
}

/// The folder of the note's file `note_file` below `base`; `None` for a
/// name that would leave it or is empty.
fn folder_in(base: &Path, note_file: &str) -> Option<PathBuf> {
    let mut folder = base.to_owned();
    let mut any = false;
    for part in note_file.split('/') {
        // Windows: no drive (`C:`), stream (`a:b`) or NUL in a part either.
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':', '\0']) {
            return None;
        }
        folder.push(part);
        any = true;
    }
    any.then_some(folder)
}

/// The snapshot files directly in `folder`.
fn snapshots_in(folder: &Path) -> Vec<Snapshot> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| Snapshot::parse(entry.path()))
        .collect()
}

/// Moves the snapshot files of `from` into `to` (both checked to be no
/// links below `root`), under the next free number of their millisecond
/// there; a snapshot is never overwritten. `from` goes when it is left
/// empty. The same folder under another case (a rename that changes only
/// the case, on a system that ignores it) moves nothing.
///
/// Every snapshot is tried; when one cannot be moved (or its old name not
/// removed after it was linked to the new one) the others still are, and
/// the first error is returned at the end.
fn move_snapshots(root: &Path, from: &Path, to: &Path) -> Result<(), SnapshotError> {
    if from == to || same_folder(from, to) {
        return Ok(());
    }
    let snapshots = snapshots_in(from);
    if snapshots.is_empty() {
        return Ok(());
    }
    let _taking = lock_of(to);
    let mut first_error = None;
    for snapshot in snapshots {
        if let Err(error) = move_one(root, &snapshot, to) {
            first_error.get_or_insert(error);
        }
    }
    // Gone already (pruned meanwhile) or not empty: either is fine.
    let _ = fs::remove_dir(from);
    first_error.map_or(Ok(()), Err)
}

/// Moves one snapshot into `to` by linking it there under a free name and
/// removing it from its place. A snapshot gone from its place meanwhile
/// (moved or pruned by someone else) counts as moved.
fn move_one(root: &Path, snapshot: &Snapshot, to: &Path) -> Result<(), SnapshotError> {
    let gone = || {
        fs::symlink_metadata(&snapshot.file)
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    };
    let mut last = io::Error::from(io::ErrorKind::NotFound);
    for _ in 0..WRITE_ATTEMPTS {
        if gone() {
            return Ok(());
        }
        match create_private_folders(to) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                last = error;
                continue;
            }
            Err(error) => return Err(SnapshotError::Move(error)),
        }
        no_link_below(root, to)?;
        let mut number = next_number(to, snapshot.taken_at);
        loop {
            if number >= MAX_NUMBER {
                return Err(SnapshotError::Move(io::Error::from(
                    io::ErrorKind::AlreadyExists,
                )));
            }
            let target = to.join(Snapshot::name(snapshot.taken_at, snapshot.cause, number));
            match fs::hard_link(&snapshot.file, &target) {
                // The snapshot is at its new place; an old name that stays
                // is a second copy, never a loss. An old name gone meanwhile
                // (all snapshots removed, UC-026 A7) is gone as wanted.
                Ok(()) => {
                    return match fs::remove_file(&snapshot.file) {
                        Err(error) if error.kind() != io::ErrorKind::NotFound => {
                            Err(SnapshotError::Move(error))
                        }
                        _ => Ok(()),
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => number += 1,
                Err(_) if gone() => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    last = error;
                    break;
                }
                // Without hard links: a rename to a name that is free.
                Err(_) if fs::symlink_metadata(&target).is_ok() => number += 1,
                Err(_) => {
                    return fs::rename(&snapshot.file, &target).map_err(SnapshotError::Move);
                }
            }
        }
    }
    Err(SnapshotError::Move(last))
}

/// Creates the missing folders up to `folder` so that only their owner
/// may enter them (`0700` on Unix; ADR-031, update 2026-11-16): a snapshot
/// holds a note's earlier text, which stays as protected as the note.
/// Folders that are there are left as they are.
fn create_private_folders(folder: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(folder)
}

/// The permissions of a new snapshot file: only its owner may read and
/// write it (`0600` on Unix; ADR-031, update 2026-11-16); elsewhere the
/// system's default.
fn private_file() -> Option<fs::Permissions> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Some(fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Whether `a` and `b` are one folder under two names (differing in case).
fn same_folder(a: &Path, b: &Path) -> bool {
    if a.to_string_lossy().to_lowercase() != b.to_string_lossy().to_lowercase() {
        return false;
    }
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a_meta), Ok(b_meta)) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                a_meta.dev() == b_meta.dev() && a_meta.ino() == b_meta.ino()
            }
            #[cfg(not(unix))]
            {
                a_meta.is_dir() && b_meta.is_dir()
            }
        }
        _ => false,
    }
}

/// Removes the snapshot files of `folder`, and the folder when left empty.
fn remove_snapshots(folder: &Path) {
    for snapshot in snapshots_in(folder) {
        let _ = fs::remove_file(&snapshot.file);
    }
    let _ = fs::remove_dir(folder);
}

fn prune_folder(folder: &Path, now: Timestamp) -> usize {
    let Ok(entries) = fs::read_dir(folder) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // A link is never followed nor removed (ADR-031).
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            removed += prune_folder(&path, now);
            // Not while a snapshot is being taken into it (the lock of a
            // note's folder); a folder above notes' folders may still go
            // meanwhile, which taking tries again for.
            let _taking = lock_of(&path);
            let _ = fs::remove_dir(&path);
        } else if kind.is_file() && is_stale_temporary(&entry) {
            let _ = fs::remove_file(&path);
        } else if let Some(snapshot) = Snapshot::parse(path) {
            if !kept_at(snapshot.taken_at, now) && fs::remove_file(&snapshot.file).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// A temporary file of a write (`.underline-new-…`, `.underline-tmp-…`)
/// last changed more than an hour ago: its write was interrupted.
fn is_stale_temporary(entry: &fs::DirEntry) -> bool {
    entry
        .file_name()
        .to_string_lossy()
        .starts_with(".underline-")
        && entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > STALE_TEMPORARY)
}

/// A snapshot to take later, on the saver's thread, in the order of the
/// notes' saves (UC-026 BR-001, NFR-005: taking it never holds up typing).
#[derive(Debug, Clone)]
pub struct SnapshotJob {
    /// The snapshots of the note's workspace.
    pub store: SnapshotStore,
    /// The note's file, relative to the workspace.
    pub note_file: String,
    /// The content to keep.
    pub content: String,
    /// Why; for [`SnapshotCause::Editing`] only when it is due
    /// ([`SnapshotStore::take_for_editing`]).
    pub cause: SnapshotCause,
    /// When it is taken.
    pub at: Timestamp,
}

impl SnapshotJob {
    /// Takes the snapshot now; `None` when none was due or the content is
    /// that of the newest.
    ///
    /// # Errors
    ///
    /// The snapshot could not be written.
    pub fn run(&self) -> Result<Option<Snapshot>, SnapshotError> {
        match self.cause {
            SnapshotCause::Editing => {
                self.store
                    .take_for_editing(&self.note_file, &self.content, self.at)
            }
            cause => self
                .store
                .take(&self.note_file, &self.content, cause, self.at),
        }
    }
}

impl crate::git::Git {
    /// UC-026 BR-001, cause Git: before git changes the notes of the
    /// workspace at `dir`, a snapshot of each note that the files differing
    /// in `range` (arguments of `git diff`, such as `HEAD...@{upstream}`)
    /// name, as it is now. A note not there, or not readable as text, has
    /// nothing to keep; a snapshot that cannot be taken does not stop git
    /// (the content from before is in the repository's history).
    pub(crate) fn snapshot_before(&self, dir: &Path, range: &[&str]) {
        let mut args = vec!["diff", "--name-only", "-z", "--no-renames"];
        args.extend_from_slice(range);
        args.push("--");
        let Ok(files) = self.run(dir, &args) else {
            return;
        };
        let store = SnapshotStore::new(dir);
        let at = crate::state::now();
        for file in files.split('\0').filter(|file| is_note_file(file)) {
            let Ok(content) = fs::read_to_string(dir.join(file)) else {
                continue;
            };
            let _ = store.take(file, &content, SnapshotCause::Git, at);
        }
    }
}

/// Whether `file` (relative, with `/`) is the file of a note: in a known
/// note format and in no hidden folder.
fn is_note_file(file: &str) -> bool {
    !file.is_empty()
        && file
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.'))
        && file.rsplit_once('.').is_some_and(|(_, extension)| {
            crate::format::NoteFormat::from_extension(extension).is_some()
        })
}

/// UC-026 step 2: how many lines `old` and `new` differ in: a changed line
/// counts once, as do a line taken away and a line added.
///
/// NFR-005: the diff gives up its search after [`DIFF_DEADLINE`] and counts
/// what is left as changed parts; the count is then at least as high as the
/// fewest lines that differ, never lower than what differs.
#[must_use]
pub fn lines_differing(old: &str, new: &str) -> usize {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Myers)
        .deadline(Instant::now() + DIFF_DEADLINE)
        .diff_lines(old, new);
    diff.ops()
        .iter()
        .map(|op| match op.tag() {
            DiffTag::Equal => 0,
            DiffTag::Delete => op.old_range().len(),
            DiffTag::Insert => op.new_range().len(),
            DiffTag::Replace => op.old_range().len().max(op.new_range().len()),
        })
        .sum()
}

/// UC-026 step 2: the day and time a snapshot was taken, in the local time
/// zone, to the second: `2026-11-11 09:30:05`.
#[must_use]
pub fn snapshot_time_text(taken_at: Timestamp) -> String {
    let zone = jiff::tz::TimeZone::system();
    time_text_in(taken_at, &zone)
}

fn time_text_in(taken_at: Timestamp, zone: &jiff::tz::TimeZone) -> String {
    i64::try_from(taken_at)
        .ok()
        .and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
        .map_or_else(
            || String::from("unknown"),
            |timestamp| {
                timestamp
                    .to_zoned(zone.clone())
                    .strftime("%Y-%m-%d %H:%M:%S")
                    .to_string()
            },
        )
}

#[cfg(test)]
mod tests;
