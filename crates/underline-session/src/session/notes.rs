//! UC-003 Create Note, UC-006 Rename Note and UC-007 Delete Note in the
//! session (inventory M-05.4 to M-05.6, M-14): the starting values of the
//! dialogs "New note" and "Rename note", the file name they show while the
//! user types, creating, renaming with the links that lead to the note,
//! deleting with the undo of the status bar, and the deleted notes at the
//! end of the note overview with restoring and removing them for good.
//!
//! Ported from `apps/underline-desktop/src/app.rs`: `start_new_note`,
//! `create_note`, `start_rename`, `rename_note`, `refuse_rename`,
//! `renamed`, `TabEdits`, `start_delete`, `delete_note`, `deleted_note`,
//! `refresh_deleted`, `restore_deleted`, `restored`, `show_restore_dialog`
//! and `show_purge_dialog`. The texts are egui's.
//!
//! ADR-025: the window's editor cores hold the open notes' text; every
//! command here runs after the window sent its edits (`flushed`), so the
//! mirrors are the texts egui's tabs would hold. A rename changes the links
//! of open notes in their mirrors ([`Session::backend_edit`]), which rings
//! the window's cores (`ed-changed`).
//!
//! Like egui in its frame, creating, renaming and deleting read and write
//! the notes while the session is held (the commands run them on the
//! blocking pool, paritaet.md A-22).

use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};

use underline_core::{
    format_local_time, heading_title, now, DeleteProblem, DeletedNote, NewNote, NewNoteProblem,
    NoteId, NoteMove, Rename, HOME_TITLE,
};
use underline_editor_core::utf16::WebOffsets;
use underline_facade::rename::{apply_edits, links_to_note, rename_note, Edit, OpenNotes, Renamed};

use super::Session;
use crate::editor_view::units;
use crate::notes_view::{
    CreateNoteAnswer, DeleteAnswer, DeleteQuestion, DeletedNoteView, NewNoteRequest, NewNoteStart,
    RenameAnswer, RenameStart, RestoreAnswer,
};

/// UC-007 A4: the undo offered right after deleting, while the status shows
/// the report of the deletion (`status`).
#[derive(Debug)]
pub(in crate::session) struct UndoDelete {
    deleted: DeletedNote,
    /// The note's tab was open; the undo opens it again.
    reopen: bool,
    /// The report the offer belongs to.
    pub(in crate::session) status: String,
}

/// UC-007 A6: the deleted note "Restore note" asks another place for.
#[derive(Debug)]
pub(in crate::session) struct Restoring {
    deleted: DeletedNote,
    reopen: bool,
}

/// "1 link", "2 links" (egui's `count`).
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{n} {thing}s")
    }
}

/// A problem as a sentence, as the egui dialogs show it inline: the first
/// letter upper case, a full stop at the end.
fn sentence(problem: &NewNoteProblem) -> String {
    let mut text = problem.to_string();
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    format!("{text}.")
}

/// The keys of `deleted`, in its order: when it was deleted, which of the
/// same time and note it is, and the note.
fn deleted_keys(deleted: &[DeletedNote]) -> Vec<String> {
    let mut seen: HashMap<(u64, &str), usize> = HashMap::new();
    deleted
        .iter()
        .map(|note| {
            let n = seen.entry((note.deleted_at, note.id.as_str())).or_default();
            let key = format!("{}:{n}:{}", note.deleted_at, note.id.as_str());
            *n += 1;
            key
        })
        .collect()
}

/// The open tabs as a rename sees them (UC-006 A7): their texts, and the
/// changes of their links, applied to their mirrors after the rename.
struct OpenTexts {
    texts: HashMap<NoteId, String>,
    edits: Vec<(NoteId, Vec<Edit>)>,
}

impl OpenNotes for OpenTexts {
    fn text(&self, id: &NoteId) -> Option<String> {
        self.texts.get(id).cloned()
    }

    fn edit(&mut self, id: &NoteId, edits: &[Edit]) {
        self.edits.push((id.clone(), edits.to_vec()));
    }
}

impl Session {
    /// UC-015 BR-007: whether the note `id` is in conflict. The conflicts
    /// come with feature F13 (`in_conflict`); until then no note is.
    fn in_conflict(&self, _id: &NoteId) -> bool {
        false
    }

    /// UC-003 step 2, M-14.1: the dialog "New note" starts with the folder
    /// of the active note and the default format; `None` without a
    /// workspace.
    #[must_use]
    pub fn new_note_start(&self) -> Option<NewNoteStart> {
        let open = self.open.as_ref()?;
        let folder = open
            .tabs
            .active()
            .and_then(|(note, _)| note.folder())
            .unwrap_or_default()
            .to_owned();
        Some(NewNoteStart {
            folder,
            format: open.workspace.default_format().into(),
        })
    }

    /// UC-003 step 2: the file name the note of `request` will get, shown
    /// while the user types ("File: {name}"); `None` while it cannot be
    /// created.
    #[must_use]
    pub fn plan_new_note(&self, request: &NewNoteRequest) -> Option<String> {
        let open = self.open.as_ref()?;
        let new = new_note(request);
        open.workspace
            .plan_note(&new)
            .ok()
            .map(|id| id.file_name(new.format))
    }

    /// UC-003 steps 4–6, M-14.1: creates the note with its title as
    /// heading, opens it in a tab and says "Created the note {t}"; the
    /// answer puts the cursor at the end of its text. A note that exists
    /// is offered to open instead (A2, A3); a title or folder not allowed
    /// keeps the dialog (A1, A4); a failure to write closes it and says why
    /// in the status (A5).
    pub fn create_note(&mut self, request: &NewNoteRequest) -> CreateNoteAnswer {
        let Some(open) = &mut self.open else {
            return CreateNoteAnswer::Done;
        };
        let new = new_note(request);
        let content = self
            .formats
            .new_note(new.format, &heading_title(&new.title));
        match open.workspace.create_note(&new, &content) {
            Ok(id) => {
                self.open_note(&id);
                self.status = format!("Created the note {}", id.title());
                self.end_of(&id)
                    .map_or(CreateNoteAnswer::Done, |(rev, offset)| {
                        CreateNoteAnswer::Created {
                            note: id.as_str().to_owned(),
                            rev,
                            offset,
                        }
                    })
            }
            Err(NewNoteProblem::Exists(id)) => CreateNoteAnswer::Problem {
                problem: format!(
                    "The note {} exists. Change the title or the folder, or open it.",
                    id.title()
                ),
                existing: Some(id.as_str().to_owned()),
            },
            Err(problem @ NewNoteProblem::Io { .. }) => {
                self.status = format!("The note could not be created: {problem}");
                CreateNoteAnswer::Done
            }
            Err(problem) => CreateNoteAnswer::Problem {
                problem: sentence(&problem),
                existing: None,
            },
        }
    }

    /// The end of the backend's text of the open note `id`: its revision
    /// and the place in web units.
    fn end_of(&self, id: &NoteId) -> Option<(u32, u32)> {
        let open = self.open.as_ref()?;
        let tab = open.tabs.get(open.tabs.find(id)?);
        let rope = tab.mirror.rope();
        let end = WebOffsets::of(rope).to_web(rope, rope.len_chars());
        Some((tab.mirror.rev(), units(end)))
    }

    /// UC-006 step 1, M-14.2: the dialog "Rename note" for `note`, its
    /// title and folder, and whether it is the Home note (BR-009). A note
    /// in conflict is not renamed (UC-015 BR-007); the status says so.
    pub fn rename_start(&mut self, note: &NoteId) -> Option<RenameStart> {
        self.open.as_ref()?;
        if self.in_conflict(note) {
            self.status = format!(
                "{} is in conflict; resolve it before renaming it",
                note.title()
            );
            return None;
        }
        Some(RenameStart {
            note: note.as_str().to_owned(),
            title: note.title(),
            folder: note.folder().unwrap_or_default().to_owned(),
            home: note.as_str() == HOME_TITLE,
        })
    }

    /// UC-006 step 2: the file the note `from` will be in with `title` and
    /// `folder`, shown while the user types; `None` when it cannot be
    /// renamed so, or nothing would change.
    #[must_use]
    pub fn plan_rename(&self, from: &NoteId, title: &str, folder: &str) -> Option<String> {
        let open = self.open.as_ref()?;
        let rename = Rename {
            folder: folder.to_owned(),
            title: title.to_owned(),
        };
        open.workspace
            .plan_rename(from, &rename)
            .ok()
            .flatten()
            .map(|planned| planned.to_file())
    }

    /// UC-006 steps 4–7, M-14.2: renames the note `from` to `title` in
    /// `folder`. Its tab is stored first (step 5); the links to it in other
    /// notes follow, in open tabs too, in their text, saved with their next
    /// autosave without "Saved {t}" (A7). The status says "Renamed A to B;
    /// updated N link(s) in M note(s)", and which notes could not be
    /// updated (A6).
    pub fn rename_note(&mut self, from: &NoteId, title: &str, folder: &str) -> RenameAnswer {
        let rename = Rename {
            folder: folder.to_owned(),
            title: title.to_owned(),
        };
        // Step 5: unsaved changes of the note are saved first.
        if let Err(err) = self.store_first(from) {
            self.status = format!("The note could not be renamed, because {err}");
            return RenameAnswer::Done;
        }
        let Some(open) = &mut self.open else {
            return RenameAnswer::Done;
        };
        let planned = match open.workspace.plan_rename(from, &rename) {
            Ok(Some(planned)) => planned,
            // A4: nothing changed.
            Ok(None) => return RenameAnswer::Done,
            Err(problem) => return self.refuse_rename(problem),
        };
        let mut texts = OpenTexts {
            texts: open
                .tabs
                .iter()
                .filter(|(id, _)| *id != from)
                .map(|(id, tab)| (id.clone(), tab.mirror.text()))
                .collect(),
            edits: Vec::new(),
        };
        match rename_note(&mut open.workspace, &planned, &mut texts) {
            Ok(renamed) => {
                self.renamed(&planned, renamed, texts.edits);
                RenameAnswer::Renamed {
                    from: planned.from.as_str().to_owned(),
                    to: planned.to.as_str().to_owned(),
                }
            }
            Err(problem) => self.refuse_rename(problem),
        }
    }

    /// UC-005 BR-001: stores the tab of `note`, if one is open, after any
    /// background save of it; the failure names the tab.
    fn store_first(&mut self, note: &NoteId) -> Result<(), String> {
        self.settle_saves();
        let Some(open) = &mut self.open else {
            return Ok(());
        };
        let Some(index) = open.tabs.find(note) else {
            return Ok(());
        };
        self.views_dirty = true;
        open.tabs.get_mut(index).flush(&self.saver)
    }

    /// UC-006 A1–A3 keep the dialog with what the user entered; A5 reports
    /// in the status bar.
    fn refuse_rename(&mut self, problem: NewNoteProblem) -> RenameAnswer {
        match problem {
            NewNoteProblem::Exists(id) => RenameAnswer::Problem {
                problem: format!(
                    "A note named {} exists in that folder. Change the title or the folder.",
                    id.title()
                ),
            },
            problem @ NewNoteProblem::Io { .. } => {
                self.status = format!("The note could not be renamed: {problem}");
                RenameAnswer::Done
            }
            problem => RenameAnswer::Problem {
                problem: sentence(&problem),
            },
        }
    }

    /// UC-006 step 7: the links in open tabs, the renamed note's tab, the
    /// remembered state, the note overview and the report (BR-007, A6, A7).
    fn renamed(
        &mut self,
        planned: &NoteMove,
        mut renamed: Renamed,
        edits: Vec<(NoteId, Vec<Edit>)>,
    ) {
        // A7: the links of open tabs change in their text; the tab saves
        // them with its next autosave and keeps this report.
        for (id, edits) in edits {
            let quiet = self.open.as_ref().and_then(|open| {
                let tab = open.tabs.get(open.tabs.find(&id)?);
                Some(!tab.autosave.is_unstored())
            });
            match self.backend_edit(&id, &edits, false) {
                Ok(_) => {
                    if let Some(tab) = self.tab_of(&id) {
                        tab.quiet_save = quiet.unwrap_or(false);
                    }
                }
                Err(error) => {
                    // Never expected: the edits were made on the mirror's
                    // text. The note is named among those not updated; the
                    // counts of the report stay as `rename_note` made them
                    // (it cannot say how many of its links were these).
                    eprintln!(
                        "UC-006 A7: the links of {} were not changed: {error}",
                        id.as_str()
                    );
                    renamed.not_updated.push(id);
                }
            }
        }
        self.move_tab(planned, &renamed.own_edits);
        let Some(open) = &self.open else {
            return;
        };
        let root = open.root().to_owned();
        self.state.note_renamed(&root, &planned.from, &planned.to);
        self.persist_tabs();
        self.refresh_overview();
        let mut status = format!(
            "Renamed {} to {}; updated {} in {}",
            planned.from.title(),
            planned.to.title(),
            count(renamed.links, "link"),
            count(renamed.notes, "note"),
        );
        if !renamed.not_updated.is_empty() {
            let names: Vec<String> = renamed.not_updated.iter().map(NoteId::title).collect();
            status.push_str(&format!(
                "; these notes could not be updated and still link to the old title: {}",
                names.join(", ")
            ));
        }
        self.status = status;
    }

    /// UC-006 step 7: the tab of the renamed note shows it under its new
    /// name and place, with its own links changed (`own_edits`, in the text
    /// it was stored with). The file holds them already, so the tab holds
    /// no unstored change; should its text differ from the file's after
    /// all, it shows the file's.
    fn move_tab(&mut self, planned: &NoteMove, own_edits: &[Edit]) {
        let Some(open) = &mut self.open else {
            return;
        };
        if open.tabs.find(&planned.from).is_none() {
            return;
        }
        open.tabs.rename(&planned.from, planned.to.clone());
        let root = open.root().to_owned();
        let path = root.join(planned.to_file());
        let written = std::fs::read_to_string(&path).ok();
        let Some(tab) = self.tab_of(&planned.to) else {
            return;
        };
        tab.path = path;
        tab.title = planned.to.title();
        tab.deleted = false;
        let fits = written
            .as_deref()
            .is_some_and(|file| apply_edits(&tab.mirror.text(), own_edits) == file);
        if fits && !own_edits.is_empty() {
            if let Err(error) = self.change_mirror(&planned.to, own_edits, false, false) {
                eprintln!(
                    "UC-006: the links of {} were not changed in its tab: {error}",
                    planned.to.as_str()
                );
            }
        } else if let (false, Some(file)) = (fits, written) {
            if let Some(tab) = self.tab_of(&planned.to) {
                tab.reload(file);
            }
            self.changed.push(planned.to.clone());
        }
        if let Some(tab) = self.tab_of(&planned.to) {
            tab.on_disk = tab.mirror.text();
            tab.read_properties(&root);
        }
    }

    /// Whether a tab shows the note `note`.
    #[must_use]
    pub fn has_tab(&self, note: &NoteId) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.tabs.find(note).is_some())
    }

    /// The tab of `note`, if it is open.
    fn tab_of(&mut self, note: &NoteId) -> Option<&mut super::tab::Tab> {
        let open = self.open.as_mut()?;
        let index = open.tabs.find(note)?;
        Some(open.tabs.get_mut(index))
    }

    /// UC-007 steps 1–2, M-14.3: the question before deleting `note`,
    /// naming the links in other notes that lead to it (BR-001) and
    /// whether it is the Home note (BR-010). A note in conflict is not
    /// deleted (UC-015 BR-007); the status says so.
    pub fn delete_start(&mut self, note: &NoteId) -> Option<DeleteQuestion> {
        if self.in_conflict(note) {
            self.status = format!(
                "{} is in conflict; resolve it before deleting it",
                note.title()
            );
            return None;
        }
        let open = self.open.as_ref()?;
        let (links, notes) = links_to_note(&open.workspace, note);
        let links = if links == 0 {
            "No links in other notes lead to it.".to_owned()
        } else {
            format!(
                "{} in {} lead to it and will lead nowhere.",
                count(links, "link"),
                count(notes, "note")
            )
        };
        Some(DeleteQuestion {
            note: note.as_str().to_owned(),
            title: note.title(),
            links,
            home: note.as_str() == HOME_TITLE,
        })
    }

    /// UC-007 steps 4–5, A3, A8, M-14.3: deletes `note` after storing its
    /// tab; it is kept among the deleted notes, or, with `for_good` (A8
    /// step 2), removed. Its tab closes, the report "Deleted {t}" offers
    /// the undo when it is kept. When it cannot be kept, the answer asks to
    /// delete it for good; other failures are in the status (A2, A3).
    pub fn delete_note(&mut self, note: &NoteId, for_good: bool) -> DeleteAnswer {
        // Step 4: unsaved changes are saved first, so they are kept too.
        if let Err(err) = self.store_first(note) {
            self.status = format!("The note could not be deleted, because {err}");
            return DeleteAnswer::Done;
        }
        let Some(open) = &mut self.open else {
            return DeleteAnswer::Done;
        };
        let result = if for_good {
            open.workspace.delete_note_for_good(note).map(|()| None)
        } else {
            open.workspace.delete_note(note, now()).map(Some)
        };
        match result {
            Ok(deleted) => {
                self.deleted_note(note, deleted);
                DeleteAnswer::Done
            }
            Err(DeleteProblem::CannotKeep(err)) => DeleteAnswer::CannotKeep {
                question: format!(
                    "The note cannot be kept among the deleted notes ({err}), so it cannot be restored. Delete it for good?"
                ),
            },
            Err(problem) => {
                // A3: the note and its tab stay.
                self.status = format!("The note could not be deleted: {problem}");
                DeleteAnswer::Done
            }
        }
    }

    /// UC-007 step 5: the tab closes, the note is forgotten, and the report
    /// offers to undo when the note is kept.
    fn deleted_note(&mut self, id: &NoteId, deleted: Option<DeletedNote>) {
        let Some(open) = &mut self.open else {
            return;
        };
        let root = open.root().to_owned();
        let reopen = match open.tabs.find(id) {
            Some(index) => {
                open.tabs.close(index);
                true
            }
            None => false,
        };
        self.state.note_deleted(&root, id);
        self.persist_tabs();
        self.refresh_overview();
        self.refresh_deleted();
        self.status = format!("Deleted {}", id.title());
        self.undo_delete = deleted.map(|deleted| UndoDelete {
            deleted,
            reopen,
            status: self.status.clone(),
        });
    }

    /// UC-007 A4, M-14.3: "Undo" of the report "Deleted {t}": the note
    /// comes back, in its tab if it had one. Nothing happens once another
    /// report took the place of the deletion's.
    pub fn undo_delete(&mut self) -> RestoreAnswer {
        match self.undo_delete.take() {
            Some(offer) if offer.status == self.status => self.restore(offer.deleted, offer.reopen),
            _ => RestoreAnswer::Done,
        }
    }

    /// UC-007 A5, M-05.6: "Restore" of the deleted note `key`; when its
    /// place is taken, the answer asks for another title or folder (A6).
    pub fn restore_deleted(&mut self, key: &str) -> RestoreAnswer {
        match self.deleted_by_key(key) {
            Some(deleted) => self.restore(deleted, false),
            None => RestoreAnswer::Done,
        }
    }

    /// UC-007 A6, M-14.4: "Restore" in "Restore note": the note asked about
    /// is restored as `title` in `folder`, or the dialog says why not.
    pub fn restore_deleted_as(&mut self, title: &str, folder: &str) -> RestoreAnswer {
        let (Some(restoring), Some(open)) = (&self.restoring, &mut self.open) else {
            return RestoreAnswer::Done;
        };
        let instead = Rename {
            folder: folder.to_owned(),
            title: title.to_owned(),
        };
        match open
            .workspace
            .restore_note(&restoring.deleted, Some(&instead))
        {
            Ok(id) => {
                let reopen = restoring.reopen;
                self.restored(&id, reopen)
            }
            Err(NewNoteProblem::Exists(existing)) => RestoreAnswer::Ask {
                title: title.to_owned(),
                folder: folder.to_owned(),
                problem: taken(&existing),
            },
            Err(problem) => RestoreAnswer::Ask {
                title: title.to_owned(),
                folder: folder.to_owned(),
                problem: sentence(&problem),
            },
        }
    }

    /// UC-007 A4, A5: puts a deleted note back at its place; asks for
    /// another title or folder when it is taken (A6).
    fn restore(&mut self, deleted: DeletedNote, reopen: bool) -> RestoreAnswer {
        let Some(open) = &mut self.open else {
            return RestoreAnswer::Done;
        };
        match open.workspace.restore_note(&deleted, None) {
            Ok(id) => self.restored(&id, reopen),
            Err(NewNoteProblem::Exists(existing)) => {
                let answer = RestoreAnswer::Ask {
                    title: deleted.id.title(),
                    folder: deleted.id.folder().unwrap_or_default().to_owned(),
                    problem: taken(&existing),
                };
                self.restoring = Some(Restoring { deleted, reopen });
                answer
            }
            Err(problem) => {
                self.status = format!("The note could not be restored: {problem}");
                RestoreAnswer::Done
            }
        }
    }

    fn restored(&mut self, id: &NoteId, reopen: bool) -> RestoreAnswer {
        self.restoring = None;
        self.undo_delete = None;
        if reopen {
            self.open_note(id);
        }
        self.refresh_overview();
        self.refresh_deleted();
        self.status = format!("Restored {}", id.title());
        RestoreAnswer::Restored {
            note: id.as_str().to_owned(),
        }
    }

    /// UC-007 A7, M-14.4: removes the deleted note `key` for good, or all
    /// of them without one; the status says how many, or how many could
    /// not be removed.
    pub fn purge_deleted(&mut self, key: Option<&str>) {
        let targets: Vec<DeletedNote> = match key {
            Some(key) => match self.deleted_by_key(key) {
                Some(deleted) => vec![deleted],
                None => return,
            },
            None => self
                .open
                .as_ref()
                .map(|open| open.deleted.clone())
                .unwrap_or_default(),
        };
        let Some(open) = &self.open else {
            return;
        };
        let failed = targets
            .iter()
            .filter(|deleted| open.workspace.forget_deleted(deleted).is_err())
            .count();
        if self
            .undo_delete
            .as_ref()
            .is_some_and(|offer| targets.contains(&offer.deleted))
        {
            self.undo_delete = None;
        }
        if self
            .restoring
            .as_ref()
            .is_some_and(|restoring| targets.contains(&restoring.deleted))
        {
            self.restoring = None;
        }
        self.refresh_deleted();
        self.status = if failed == 0 {
            format!("Removed {} for good", count(targets.len(), "deleted note"))
        } else {
            format!("{} could not be removed", count(failed, "deleted note"))
        };
    }

    /// The deleted note of `key`, as the window last got the list.
    fn deleted_by_key(&self, key: &str) -> Option<DeletedNote> {
        let open = self.open.as_ref()?;
        let at = deleted_keys(&open.deleted).iter().position(|k| k == key)?;
        open.deleted.get(at).cloned()
    }

    /// M-05.6, UC-007 A5 step 1: reads the deleted notes again.
    pub(super) fn refresh_deleted(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        let deleted = open.workspace.deleted_notes();
        if deleted != open.deleted {
            open.deleted = deleted;
            open.deleted_generation += 1;
        }
    }

    /// M-05.6: the deleted notes as the overview shows them.
    pub(super) fn deleted_view(&self) -> Vec<DeletedNoteView> {
        let Some(open) = &self.open else {
            return Vec::new();
        };
        open.deleted
            .iter()
            .zip(deleted_keys(&open.deleted))
            .map(|(deleted, key)| {
                let time =
                    format_local_time(UNIX_EPOCH + Duration::from_millis(deleted.deleted_at));
                let folder = deleted.id.folder().unwrap_or("top folder").to_owned();
                DeletedNoteView {
                    key,
                    title: deleted.id.title(),
                    hover: format!("{folder} · deleted {time}"),
                    folder,
                    deleted: time,
                }
            })
            .collect()
    }
}

/// UC-007 A6: "A note named {t} exists in that folder. Choose another
/// title or folder."
fn taken(existing: &NoteId) -> String {
    format!(
        "A note named {} exists in that folder. Choose another title or folder.",
        existing.title()
    )
}

/// The core's request of `request`.
fn new_note(request: &NewNoteRequest) -> NewNote {
    NewNote {
        folder: request.folder.clone(),
        title: request.title.clone(),
        format: request.format.into(),
    }
}

#[cfg(test)]
mod tests;
