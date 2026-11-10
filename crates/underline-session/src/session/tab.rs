//! One open tab (UC-004): the note's text in its editor, with its autosave
//! clock and save state (UC-005), its highlighting and its properties.
//!
//! Ported from `OpenTab` of `apps/underline-desktop/src/app.rs`, without
//! the egui widgets: the window shows the text, the session owns it
//! (ADR-024). The text with what answers the window's inputs is the
//! editor's tab ([`EditorTab`], ADR-025), the same code the window runs
//! with variant B.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{Autosave, EditingView, NoteFormat, NoteProperties, NoteSaver};
use underline_editor_core::sync::{apply_remote, ChangeSet, Server, Touched};
use underline_editor_view::EditorTab;
use underline_facade::highlight::DocumentHighlight;

use super::facade::Facade;
use super::indexes::TabIndexes;
use crate::editor_view::{NotePropertiesView, TabPropertiesView};

/// One open tab.
pub(super) struct Tab {
    pub(super) path: PathBuf,
    pub(super) title: String,
    /// The editing view the tab shows (UC-004 BR-003).
    pub(super) editing_view: EditingView,
    /// The text with its cursor and history, its keys, folds and facade
    /// blocks, as the window will hold it (ADR-025; the note's format is
    /// its `format`).
    pub(super) edit: EditorTab,
    /// ADR-025: the backend's copy of the text, with its revisions: what is
    /// stored and highlighted, what agents and renames change. Until the
    /// window holds the text (step B5) it is always `edit`'s text.
    pub(super) mirror: Server,
    /// A window synced its edits with `ed_sync`; a reload then waits for it
    /// to confirm (`docs/migration/variante-b.md` section 3.6).
    pub(super) window_synced: bool,
    /// The mirror's revision the window was last told of or answered with;
    /// a later one rings it (`ed-changed`).
    pub(super) told_rev: u32,
    /// When the text is stored next (UC-005 BR-001).
    pub(super) autosave: Autosave,
    /// A background save of this tab is running.
    pub(super) saving: bool,
    /// The text a running background save writes.
    pub(super) saving_text: Option<String>,
    /// Why the last save failed (UC-005 A14).
    pub(super) error: Option<String>,
    /// The note was deleted outside while open (UC-004 A8).
    pub(super) deleted: bool,
    /// UC-006 A7: a rename changed the links of this tab, which held no
    /// unstored change; its next save keeps the rename's report instead of
    /// saying "Saved {t}". Any later change of its own ends that.
    pub(super) quiet_save: bool,
    /// The version from outside last kept as a copy (UC-004 A9), so one
    /// change is kept once, however many events report it.
    pub(super) kept_outside: Option<String>,
    /// What the file held when Underline last read or stored it; a file
    /// that holds anything else was changed outside (ADR-011).
    pub(super) on_disk: String,
    /// M-16.2: the properties as read last; read again when the note was
    /// stored, changed outside or shown, not for every look at them.
    pub(super) properties: Option<NoteProperties>,
    /// M-08.2: the highlighting, made when the window first asks for it.
    pub(super) highlight: Option<DocumentHighlight>,
    /// The lines the source view shows, the end not included.
    pub(super) viewport: Option<Range<usize>>,
    /// The revision and the lines whose colours the window has.
    pub(super) highlight_told: Option<(u64, Range<usize>)>,
    /// M-09: how the backend makes the facade's views (its jobs, figures
    /// and colours); the blocks are the editor's (`edit.facade`).
    pub(super) facade: Option<Facade>,
    /// M-16.5: the outline, code and figures of the tool panel.
    pub(super) indexes: TabIndexes,
}

impl std::fmt::Debug for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tab")
            .field("path", &self.path)
            .field("title", &self.title)
            .field("revision", &self.edit.editor.revision())
            .field("error", &self.error)
            .field("deleted", &self.deleted)
            .finish_non_exhaustive()
    }
}

impl Tab {
    /// A tab showing `body`, the text of the note in `path`.
    pub(super) fn new(
        path: PathBuf,
        title: String,
        format: NoteFormat,
        editing_view: EditingView,
        body: String,
    ) -> Self {
        Self {
            path,
            title,
            editing_view,
            edit: {
                let mut edit = EditorTab::new(&body, format);
                edit.editor.record_edits();
                edit
            },
            mirror: Server::new(&body),
            window_synced: false,
            told_rev: 1,
            autosave: Autosave::default(),
            saving: false,
            saving_text: None,
            error: None,
            deleted: false,
            quiet_save: false,
            kept_outside: None,
            on_disk: body,
            properties: None,
            highlight: None,
            viewport: None,
            highlight_told: None,
            facade: None,
            indexes: TabIndexes::default(),
        }
    }

    /// M-06.1: the title, marked " ⚠" when the last save failed, else " •"
    /// with changes not stored, and " (deleted)" when the note was deleted
    /// outside. Links a rename changed in a tab that held no change of its
    /// own are no change of the user's: they get no " •" (M-14.2, A-82).
    pub(super) fn label(&self) -> String {
        let mut label = self.title.clone();
        if self.error.is_some() {
            label.push_str(" ⚠");
        } else if self.autosave.is_unstored() && !self.quiet_save {
            label.push_str(" •");
        }
        if self.deleted {
            label.push_str(" (deleted)");
        }
        label
    }

    /// M-16.2: reads the properties of the note again.
    pub(super) fn read_properties(&mut self, root: &Path) {
        self.properties = NoteProperties::read(root, &self.path, self.edit.format).ok();
    }

    /// M-16.2: the properties section for this tab, showing `note`.
    pub(super) fn properties_view(&self, note: &str) -> TabPropertiesView {
        TabPropertiesView {
            note: note.to_owned(),
            properties: self.properties.as_ref().map(|p| NotePropertiesView {
                place: p.place.clone(),
                format: p.format.name().to_owned(),
                size: p.size_text(),
                last_change: p.changed_text(),
            }),
        }
    }

    /// UC-005 BR-001: stores the tab's changes now, as before closing it or
    /// switching the workspace. The failure names the tab. The session
    /// waits for a background save of the tab first (`settle_saves`).
    pub(super) fn flush(&mut self, saver: &NoteSaver) -> Result<(), String> {
        if !self.autosave.is_unstored() {
            return Ok(());
        }
        let started = Instant::now();
        let text = self.mirror.text();
        match saver.save_now(&self.path, &text) {
            Ok(()) => {
                self.on_disk = text;
                self.saving_text = None;
                self.autosave.stored(started);
                self.error = None;
                self.deleted = false;
                Ok(())
            }
            Err(err) => {
                self.autosave.failed(started);
                self.error = Some(err.clone());
                Err(format!("{}: {err}", self.title))
            }
        }
    }

    /// The text changed at `now` (UC-005 step 4); returns whether the tab
    /// showed no unstored change before, so its mark changes.
    pub(super) fn changed(&mut self, now: Instant) -> bool {
        // A tab whose only change were a rename's links had no mark.
        let was_marked = self.autosave.is_unstored() && !self.quiet_save;
        self.quiet_save = false;
        self.autosave.changed(now);
        !was_marked
    }

    /// The highlighting follows the changes of the mirror (ADR-010): each
    /// at its line of the text just before it.
    pub(super) fn highlight_touched(&mut self, touched: &[Touched]) {
        let Some(highlight) = &mut self.highlight else {
            return;
        };
        let lines = self.mirror.rope().len_lines();
        for change in touched {
            highlight.edited(change.line, change.added_lines, lines);
        }
    }

    /// ADR-025: a change of the mirror (`set`, in chars of the text both
    /// had) goes to the session's own editor too, as egui applies a change
    /// of the backend; `sealed` makes it an undo step of its own. Should the
    /// set not fit the editor's text (never expected: they are alike), the
    /// editor takes the mirror's text whole.
    pub(super) fn follow_mirror(&mut self, set: &ChangeSet, sealed: bool) {
        match apply_remote(&mut self.edit.editor, set, sealed) {
            Ok(outcome) => self.edit.refold_after(&outcome),
            Err(error) => {
                eprintln!("ADR-025: the editor of {} went apart ({error})", self.title);
                let text = self.mirror.text();
                self.edit.reload(&text);
            }
        }
        // The mirror has it already.
        self.edit.editor.take_edits();
    }

    /// Shows `text` from outside instead (UC-004 A7): the cursor stays at
    /// its line and column, the history and the colours start anew.
    pub(super) fn reload(&mut self, text: String) {
        // M-08.8: the folded sections stay folded where they are now.
        self.edit.set_now(super::editing::millis(Instant::now()));
        self.edit.reload(&text);
        self.mirror.reload(&text);
        if !self.window_synced {
            // No window holds the text: nobody is to confirm it.
            self.mirror.confirm_reload();
        }
        self.on_disk = text;
        self.highlight = None;
        self.highlight_told = None;
    }
}
