//! FR-103, UC-004 A22, BR-021 (OQ-199): "Link" of an unlinked mention in
//! the tool panel turns the mention into a link to the active note, written
//! as link completion writes a link ([`note_link`], OQ-248), with the
//! mention as the link's text.
//!
//! The mention is looked for again in the text the note holds now: only
//! when an unlinked mention of the active note's title still stands at the
//! place listed, with the same text, is it changed; else the status says
//! the note changed since the list was made, and nothing is changed.
//!
//! - A note open in a tab is changed in its tab through its mirror, as one
//!   undo step of its own, and stored by its autosave (ADR-025).
//! - A closed note is read, checked and written atomically without the
//!   session ([`MentionJob::run`], ADR-005), after a snapshot of its content
//!   (UC-026 BR-001, cause Editing, ADR-031); when the snapshot cannot be
//!   taken, the note is not changed. Jobs on one note run one after the
//!   other ([`NOTE_LOCKS`]), so two quick "Link"s never write over each
//!   other; the file is read again just before it is written, and when
//!   another program changed it meanwhile the mention is looked for once
//!   more, else reported as moved.
//! - A Markdown mention right after `!` gets the `!` escaped (`\!`), so the
//!   link does not become a picture; one right after an odd run of `\`
//!   gets one more, so the link's `[` is not escaped (OQ-305).

use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use underline_core::{NoteFormat, NoteId, NoteWriter, SnapshotCause, SnapshotStore, Timestamp};
use underline_facade::mentions::{odd_backslashes_before, unlinked_mentions, MentionTitle};
use underline_markup::completion::note_link;

use super::note_links::MENTIONS_SHOWN;
use super::Session;
use crate::links_view::LinkPlace;
use crate::{UiError, UiErrorKind};

/// What linking a mention needs of the session: done in the tab, or a
/// closed note to change without the session.
#[derive(Debug)]
pub enum MentionPlan {
    /// The note is open: its tab was changed, or the status says why not.
    Done,
    /// The note is closed: [`MentionJob::run`] changes its file.
    Closed(MentionJob),
}

/// A22 for a closed note: reads, checks and writes it without the session.
#[derive(Debug)]
pub struct MentionJob {
    note: NoteId,
    path: PathBuf,
    format: NoteFormat,
    /// The note's file in the workspace, which the link is relative to.
    file: String,
    /// The active note's file and title, which the link leads to.
    target: (String, String),
    place: LinkPlace,
    writer: NoteWriter,
    snapshots: (SnapshotStore, Timestamp),
}

/// A22: the locks of the notes linked by jobs; a note takes the one its
/// path falls on, so jobs on one note never run at once.
static NOTE_LOCKS: [Mutex<()>; 64] = [const { Mutex::new(()) }; 64];

/// The lock of the note at `path`.
fn note_lock(path: &Path) -> std::sync::MutexGuard<'static, ()> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    let index = usize::try_from(hasher.finish() % 64).unwrap_or_default();
    // A job that panicked left nothing half done the lock guards.
    NOTE_LOCKS[index]
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The text of the file at `path` with its time and size.
fn read_stamped(path: &Path) -> Result<(String, Option<(std::time::SystemTime, u64)>), String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let stamp = std::fs::metadata(path)
        .ok()
        .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
    Ok((text, stamp))
}

/// What [`MentionJob::run`] did, for [`Session::link_mention_finish`].
#[derive(Debug)]
pub struct MentionDone {
    note: NoteId,
    format: NoteFormat,
    file: String,
    /// The file and the title of the note the link leads to.
    target: (String, String),
    place: LinkPlace,
    /// The text read and the text written, when it was.
    outcome: Result<Option<(String, String)>, String>,
}

impl MentionDone {
    /// Whether the mention was linked in the file.
    #[must_use]
    pub fn linked(&self) -> bool {
        matches!(self.outcome, Ok(Some(_)))
    }
}

/// A22: the text of `text` (a note at `file` in `format`) with the mention
/// of `title` at `place` turned into a link to the note file `target`;
/// `None` when no unlinked mention of it stands there with the same text.
fn linked(
    text: &str,
    format: NoteFormat,
    file: &str,
    (target, title): (&str, &str),
    place: LinkPlace,
) -> Option<(Range<usize>, String)> {
    let wanted = MentionTitle::new(title)?;
    let (from, to) = (place.from as usize, place.to as usize);
    let span = unlinked_mentions(text, format, &wanted)
        .into_iter()
        .find(|span| span.start == from && span.end == to)?;
    let mention = &text[span.clone()];
    if LinkPlace::check_of(mention) != place.check {
        return None;
    }
    let link = note_link(file, target, None, mention, format);
    if format == NoteFormat::Markdown {
        // OQ-305: `!` before `[` makes a picture, `\` before it plain text.
        if text[..span.start].ends_with('!')
            && odd_backslashes_before(text, span.start - 1).is_none()
        {
            return Some((span.start - 1..span.end, format!("\\!{link}")));
        }
        if odd_backslashes_before(text, span.start).is_some() {
            return Some((span, format!("\\{link}")));
        }
    }
    Some((span, link))
}

/// `text` with `edit`.
fn apply(text: &str, (range, with): &(Range<usize>, String)) -> String {
    let mut changed = String::with_capacity(text.len() + with.len());
    changed.push_str(&text[..range.start]);
    changed.push_str(with);
    changed.push_str(&text[range.end..]);
    changed
}

/// A22: the status after a mention in `note` was linked to `target`, with
/// how to take it back: undo in the tab of an open note, the snapshots of a
/// closed one.
fn linked_status(note: &str, target: &str, in_tab: bool) -> String {
    if in_tab {
        let key = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        format!("Linked the mention of {target} in {note}. Undo with {key}+Z in {note}.")
    } else {
        format!(
            "Linked the mention of {target} in {note}. The earlier version is in its snapshots."
        )
    }
}

impl MentionJob {
    /// A22 for a closed note: the mention is looked for again in its file;
    /// a snapshot keeps its content; then it is written with the link.
    #[must_use]
    pub fn run(self) -> MentionDone {
        let outcome = self.change();
        MentionDone {
            note: self.note,
            format: self.format,
            file: self.file,
            target: self.target,
            place: self.place,
            outcome,
        }
    }

    fn change(&self) -> Result<Option<(String, String)>, String> {
        let _held = note_lock(&self.path);
        let target = (self.target.0.as_str(), self.target.1.as_str());
        for _ in 0..2 {
            let (text, stamp) = read_stamped(&self.path)?;
            let Some(edit) = linked(&text, self.format, &self.file, target, self.place) else {
                return Ok(None);
            };
            let (store, at) = &self.snapshots;
            let note_file = store.note_file(&self.path).map_err(|e| e.to_string())?;
            store
                .take(&note_file, &text, SnapshotCause::Editing, *at)
                .map_err(|e| e.to_string())?;
            let changed = apply(&text, &edit);
            // Another program may have written meanwhile: look again.
            if read_stamped(&self.path)? != (text.clone(), stamp) {
                continue;
            }
            self.writer.save_now(&self.path, &changed)?;
            return Ok(Some((text, changed)));
        }
        Ok(None)
    }
}

impl Session {
    /// FR-103, BR-021: "Show more" of the unlinked mentions of the active
    /// note `active`: [`MENTIONS_SHOWN`] more are listed, until it is
    /// another note.
    pub fn links_more_mentions(&mut self, active: &NoteId) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let limit = &mut open.links.mention_limit;
        if limit.0.as_ref() == Some(active) {
            limit.1 = limit.1.saturating_add(MENTIONS_SHOWN);
        }
    }

    /// UC-004 A22, first part, with the session held: turns the unlinked
    /// mention of the title of `active` at `place` in the note `note` into
    /// a link to `active`. A note open in a tab is changed there, as one
    /// undo step; a closed one goes to a job. When the mention is no
    /// longer at its place, the status says so and nothing changes.
    ///
    /// # Errors
    ///
    /// No workspace is open, or `note` or `active` is no note of it.
    pub fn link_mention_plan(
        &mut self,
        active: &NoteId,
        note: &NoteId,
        place: LinkPlace,
    ) -> Result<MentionPlan, UiError> {
        let open = self
            .open
            .as_ref()
            .ok_or_else(|| UiError::new(UiErrorKind::Invalid, "No workspace is open".to_owned()))?;
        let format_of = |id: &NoteId| {
            open.workspace
                .listing()
                .notes
                .iter()
                .find(|entry| &entry.id == id)
                .map(|entry| entry.format)
                .ok_or_else(|| {
                    UiError::new(
                        UiErrorKind::Invalid,
                        format!("{} is no note of the workspace", id.as_str()),
                    )
                })
        };
        let target = (active.file_name(format_of(active)?), active.title());
        if active == note {
            return Ok(MentionPlan::Done);
        }
        if let Some(index) = open.tabs.find(note) {
            let tab = open.tabs.get(index);
            let format = tab.edit.format;
            let file = note.file_name(format);
            let edit = (!tab.deleted && !tab.mirror.reload_unconfirmed())
                .then(|| tab.mirror.text())
                .and_then(|text| linked(&text, format, &file, (&target.0, &target.1), place));
            match edit {
                Some(edit) => {
                    self.backend_edit(note, &[edit], true)?;
                    self.status = linked_status(&note.title(), &target.1, true);
                    self.links_ask_now();
                }
                None => self.mention_moved(note),
            }
            return Ok(MentionPlan::Done);
        }
        let format = format_of(note)?;
        let path = open
            .workspace
            .note_path(note)
            .map_err(|error| UiError::new(UiErrorKind::Invalid, error.to_string()))?;
        let snapshots = (
            SnapshotStore::new(open.workspace.root()),
            (self.snapshot_clock())(),
        );
        Ok(MentionPlan::Closed(MentionJob {
            note: note.clone(),
            path,
            format,
            file: note.file_name(format),
            target,
            place,
            writer: self.saver.writer(),
            snapshots,
        }))
    }

    /// UC-004 A22, last part: the status says what became of the mention
    /// of a closed note (also the answer), and the lists are made again.
    pub fn link_mention_finish(&mut self, done: MentionDone) -> String {
        let title = done.note.title();
        match done.outcome.clone() {
            Ok(Some((before, written))) => {
                self.status = linked_status(&title, &done.target.1, false);
                // UC-026 A4: a snapshot was taken.
                self.snapshot_taken();
                self.take_into_tab(&done, &before, written);
                self.links_ask_now();
            }
            Ok(None) => self.mention_moved(&done.note),
            Err(why) => self.status = format!("{title} could not be changed: {why}"),
        }
        self.status.clone()
    }

    /// ADR-025 (as for a replacement, UC-022 BR-004): the note may have
    /// been opened in a tab while the job wrote it, with the text from
    /// before (`before`); the watcher leaves Underline's own writes out, so
    /// the tab is given the written text here. When the user typed in it
    /// meanwhile, the link is made in the tab as a change of its own (one
    /// undo step); when the mention moved there, the status says the link
    /// could not be kept, since the tab's next store writes its text.
    fn take_into_tab(&mut self, done: &MentionDone, before: &str, written: String) {
        let note = &done.note;
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let Some(index) = open.tabs.find(note) else {
            return;
        };
        let tab = open.tabs.get_mut(index);
        if tab.on_disk == written || tab.on_disk != before {
            return;
        }
        if tab.mirror.text() == tab.on_disk {
            tab.reload(written);
            self.changed.push(note.clone());
            self.views_dirty = true;
            return;
        }
        let typed = tab.mirror.text();
        let target = (done.target.0.as_str(), done.target.1.as_str());
        let edit = (!tab.mirror.reload_unconfirmed())
            .then(|| linked(&typed, done.format, &done.file, target, done.place))
            .flatten();
        tab.on_disk = written;
        let kept = edit.is_some_and(|edit| self.backend_edit(note, &[edit], true).is_ok());
        if !kept {
            self.status = format!(
                "{} was opened and changed while the mention was linked; the link could not be kept in its tab, link the mention again",
                note.title()
            );
        }
    }

    /// A22: the note changed since the mentions were listed.
    fn mention_moved(&mut self, note: &NoteId) {
        self.status = format!(
            "{} changed since the mentions were listed; the mention is no longer at its place",
            note.title()
        );
        self.links_ask_now();
    }
}

#[cfg(test)]
mod tests;
