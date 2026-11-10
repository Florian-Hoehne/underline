//! ADR-025, `docs/migration/variante-b.md` sections 3.3 to 3.10 (step B4b):
//! the session's side of the sync. The mirror (`Tab::mirror`) takes the
//! window's edits (`ed_sync`), its whole text after the copies went apart
//! (`ed_resync`), and the backend's own changes ([`Session::backend_edit`]);
//! the window is told to fetch them (`ed-changed`) or to send its edits now
//! (`ed-flush`), and gets what the backend made of the facade's blocks
//! (`ed-enrich`).
//!
//! The session's own editor follows the mirror: both always have the same
//! text, from which the session makes the facade's colours and figures.

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use underline_core::{outside_copy_path, NoteId, NoteWriter};
use underline_editor_core::sync::{Batch, ChangeSet};

use super::editing::millis;
use super::tab::Tab;
use super::Session;
use crate::editor_view::units;
use crate::sync_view::{ChangedView, FlushView, MirrorView, SyncAnswer, SyncBatch};
use crate::{Event, UiError, UiErrorKind};

fn not_open(note: &NoteId) -> UiError {
    UiError::new(
        UiErrorKind::Invalid,
        format!("The tab {} is not open", note.title()),
    )
}

/// UC-004 A9: a version of a note to keep as a copy beside it, written
/// after the session is let go (`KeepCopy::write`, then
/// [`Session::copy_kept`]).
#[derive(Debug)]
pub struct KeepCopy {
    title: String,
    path: PathBuf,
    text: String,
    writer: NoteWriter,
}

impl KeepCopy {
    /// Writes the copy, atomically.
    ///
    /// # Errors
    ///
    /// Why it could not be written.
    pub fn write(&self) -> Result<(), String> {
        self.writer.save_now(&self.path, &self.text)
    }
}

/// What taking in the window's edits gave: the answer, and maybe a copy to
/// keep.
#[derive(Debug)]
pub struct Synced {
    /// The answer for the window.
    pub answer: SyncAnswer,
    /// UC-004 A9: the version the window's text took the place of.
    pub copy: Option<KeepCopy>,
}

impl Session {
    fn sync_tab(&mut self, note: &NoteId) -> Result<&mut Tab, UiError> {
        let open = self.open.as_mut().ok_or_else(|| not_open(note))?;
        let index = open.tabs.find(note).ok_or_else(|| not_open(note))?;
        Ok(open.tabs.get_mut(index))
    }

    /// The copy of `text`, the version of the tab of `note` the window's
    /// text took the place of; the session's editor shows the mirror's text
    /// again.
    fn keep_outside(&mut self, note: &NoteId, text: String) -> Result<KeepCopy, UiError> {
        let writer = self.saver.writer();
        let tab = self.sync_tab(note)?;
        let mirror = tab.mirror.text();
        tab.edit.set_now(millis(Instant::now()));
        tab.edit.reload(&mirror);
        tab.edit.editor.take_edits();
        tab.highlight = None;
        tab.highlight_told = None;
        tab.kept_outside = Some(text.clone());
        let copy = KeepCopy {
            title: tab.title.clone(),
            path: outside_copy_path(&tab.path, SystemTime::now()),
            text,
            writer,
        };
        Ok(copy)
    }

    /// UC-004 A9: the copy `copy` was written (or not, `written`): the
    /// status says so, as when the note was changed outside while the user
    /// edited it (`apps/underline-desktop/src/app.rs:1789-1810`).
    pub fn copy_kept(&mut self, copy: &KeepCopy, written: Result<(), String>) {
        let title = &copy.title;
        self.status = match written {
            Ok(()) => {
                let name = copy.path.file_name().unwrap_or_default().to_string_lossy();
                format!(
                    "{title} was changed outside while you were editing; your text stays, the other version is kept as {name}"
                )
            }
            Err(err) => {
                format!("{title} was changed outside; the other version could not be kept: {err}")
            }
        };
    }

    /// ADR-025: the window's edits made on revision `batch.base`. They are
    /// moved over the backend's changes since and applied to the mirror;
    /// the answer carries those changes, the new revision and its hash.
    /// Edits made before a reload take it back (UC-004 A9): the window's
    /// text stays, and the version it took the place of is to be kept as a
    /// copy ([`Synced::copy`]), written after the session is let go.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_sync(&mut self, note: &NoteId, batch: &SyncBatch) -> Result<Synced, UiError> {
        let tab = self.sync_tab(note)?;
        tab.window_synced = true;
        let received = tab.mirror.receive(&Batch::from(batch));
        // The window has the backend's text up to here.
        tab.told_rev = received.answer.rev;
        let answer = SyncAnswer::from(&received.answer);
        if answer.resync {
            return Ok(Synced { answer, copy: None });
        }
        let edited = !received.applied.is_identity() || received.reverted.is_some();
        let copy = if let Some(outside) = received.reverted {
            Some(self.keep_outside(note, outside)?)
        } else {
            tab.highlight_touched(&received.touched);
            if !received.applied.is_identity() {
                tab.follow_mirror(&received.applied, false);
            }
            None
        };
        if edited {
            let tab = self.sync_tab(note)?;
            if tab.changed(Instant::now()) {
                self.views_dirty = true;
            }
        }
        Ok(Synced { answer, copy })
    }

    /// ADR-025 section 3.10: the copies went apart; the window's text wins.
    /// The mirror takes `text` at a new revision. A reload the window never
    /// took is replaced too; its version is to be kept as a copy (UC-004
    /// A9).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_resync(&mut self, note: &NoteId, text: &str) -> Result<Synced, UiError> {
        let tab = self.sync_tab(note)?;
        tab.window_synced = true;
        eprintln!(
            "ADR-025: the window's text of {} went apart from the backend's; the window's is taken",
            note.as_str()
        );
        let resynced = tab.mirror.resync(text);
        tab.told_rev = resynced.answer.rev;
        tab.highlight_touched(&resynced.touched);
        let copy = match resynced.outside {
            Some(outside) => Some(self.keep_outside(note, outside)?),
            None => {
                if tab.edit.editor.text() != text {
                    tab.edit.set_now(millis(Instant::now()));
                    tab.edit.reload(text);
                    tab.edit.editor.take_edits();
                }
                None
            }
        };
        let tab = self.sync_tab(note)?;
        if !resynced.touched.is_empty() && tab.changed(Instant::now()) {
            self.views_dirty = true;
        }
        Ok(Synced {
            answer: SyncAnswer::from(&resynced.answer),
            copy,
        })
    }

    /// ADR-025 section 3.6: a change of the backend's own to the open note
    /// `note`: the bytes of each range (in order, apart, in the text as it
    /// is) replaced by its text, as an agent (`sealed`: an undo step of its
    /// own, UC-019 BR-007), a rename (UC-006 A7) or a resolved conflict
    /// changes an open note. The tab holds a change not stored yet; the
    /// window is told to fetch it. The new revision.
    ///
    /// # Errors
    ///
    /// The note has no open tab, or a range does not fit the text: past its
    /// end, backwards, out of order, or not on a char boundary.
    pub fn backend_edit(
        &mut self,
        note: &NoteId,
        edits: &[(Range<usize>, String)],
        sealed: bool,
    ) -> Result<u32, UiError> {
        self.change_mirror(note, edits, sealed, true)
    }

    /// [`Session::backend_edit`]; without `unstored` the tab's text counts
    /// as stored afterwards, as for the own links of a renamed note, whose
    /// file was written with them (UC-006 step 7).
    pub(super) fn change_mirror(
        &mut self,
        note: &NoteId,
        edits: &[(Range<usize>, String)],
        sealed: bool,
        unstored: bool,
    ) -> Result<u32, UiError> {
        let tab = self.sync_tab(note)?;
        let rope = tab.mirror.rope();
        let invalid = |why: String| {
            UiError::new(
                UiErrorKind::Invalid,
                format!("The change does not fit the note: {why}"),
            )
        };
        let char_at = |byte: usize| -> Result<usize, UiError> {
            let at = rope
                .try_byte_to_char(byte)
                .map_err(|_| invalid(format!("byte {byte} lies past its end")))?;
            if rope.char_to_byte(at) == byte {
                Ok(at)
            } else {
                Err(invalid(format!("byte {byte} lies inside a character")))
            }
        };
        let replacements = edits
            .iter()
            .map(|(range, text)| Ok((char_at(range.start)?..char_at(range.end)?, text.clone())))
            .collect::<Result<Vec<_>, UiError>>()?;
        let set = ChangeSet::of_replacements(rope.len_chars(), &replacements)
            .map_err(|error| invalid(error.to_string()))?;
        let (rev, touched) = tab
            .mirror
            .backend_edit(&replacements, sealed)
            .map_err(|error| invalid(error.to_string()))?;
        tab.highlight_touched(&touched);
        tab.follow_mirror(&set, sealed);
        if unstored && tab.changed(Instant::now()) {
            self.views_dirty = true;
        }
        self.changed.push(note.clone());
        Ok(rev)
    }

    /// ADR-025: asks the window to send its edits now, for `note` or for
    /// every open note (`ed-flush`): before an agent's change of an open
    /// note, before closing (`docs/migration/variante-b.md` section 3.9).
    pub fn request_flush(&mut self, note: Option<&NoteId>) {
        self.flushes.push((note.cloned(), false));
    }

    /// ADR-025 section 3.9: Underline is to close: the window sends the
    /// edits of every note and then asks to close (`app_request_close`).
    pub fn request_close_flush(&mut self) {
        self.flushes.push((None, true));
    }

    /// ADR-025: the backend's text of the tab of `note` as the window's
    /// editor core starts from it, with the tab's cursor.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn editor_mirror(&mut self, note: &NoteId) -> Result<MirrorView, UiError> {
        // The window's core counts as holding the text with its first
        // `ed_sync` (so a core that failed to load blocks nothing).
        let tab = self.sync_tab(note)?;
        // A new core in the window has none of the facade's colours yet.
        if let Some(facade) = tab.facade.as_mut() {
            facade.forget_sent();
        }
        let (head, anchor) = tab.edit.editor.selection_web();
        Ok(MirrorView {
            note: note.as_str().to_owned(),
            rev: tab.mirror.rev(),
            text: tab.mirror.text(),
            format: underline_editor_view::indicator(tab.edit.format).to_owned(),
            anchor: units(anchor),
            head: units(head),
        })
    }

    /// ADR-025: `ed-changed` for the notes whose mirror the backend changed,
    /// and `ed-flush` as asked.
    pub(super) fn sync_events(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        for note in std::mem::take(&mut self.changed) {
            let rev = self
                .open
                .as_ref()
                .and_then(|open| open.tabs.find(&note).map(|i| open.tabs.get(i)))
                .map(|tab| tab.mirror.rev());
            if let Some(rev) = rev {
                if let Some(tab) = self
                    .open
                    .as_mut()
                    .and_then(|open| open.tabs.find(&note).map(|i| open.tabs.get_mut(i)))
                {
                    tab.told_rev = rev;
                }
                events.push(Event::Changed(ChangedView {
                    note: note.as_str().to_owned(),
                    rev,
                }));
            }
        }
        // ADR-025, safety net: every change of a mirror the backend makes
        // itself goes through `changed` above, which also moves `told_rev`.
        // Should one ever bypass it, a window that synced still hears of it
        // here rather than never (the notes rung above are in step already).
        if let Some(open) = &mut self.open {
            for (note, tab) in open.tabs.iter_mut() {
                if tab.window_synced && tab.mirror.rev() != tab.told_rev {
                    tab.told_rev = tab.mirror.rev();
                    events.push(Event::Changed(ChangedView {
                        note: note.as_str().to_owned(),
                        rev: tab.mirror.rev(),
                    }));
                }
            }
        }
        for (note, closing) in std::mem::take(&mut self.flushes) {
            events.push(Event::Flush(FlushView {
                note: note.map(|note| note.as_str().to_owned()),
                closing,
            }));
        }
        events
    }
}

#[cfg(test)]
mod tests;
