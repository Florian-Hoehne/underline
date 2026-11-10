//! One open tab (UC-004): the note's text in its editor, with its autosave
//! clock and save state (UC-005), its highlighting and its properties.
//!
//! Ported from `OpenTab` of `apps/underline-desktop/src/app.rs`, without
//! the egui widgets: the window shows the text, the session owns it
//! (ADR-024).

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{Autosave, EditingView, NoteFormat, NoteProperties, NoteSaver};
use underline_editor_core::{Keys, SourceEditor};
use underline_facade::highlight::DocumentHighlight;

use super::facade::Facade;
use super::folding::Folds;
use crate::editor_view::{NotePropertiesView, TabPropertiesView};

/// One open tab.
pub(super) struct Tab {
    pub(super) path: PathBuf,
    pub(super) title: String,
    /// The note format, for the highlighting and the properties.
    pub(super) format: NoteFormat,
    /// The editing view the tab shows (UC-004 BR-003).
    pub(super) editing_view: EditingView,
    /// The text with its cursor and history (ADR-024).
    pub(super) editor: SourceEditor,
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
    /// M-11.3: the state of the Nano keys (the Vim keys of the source view
    /// are the window's, A-01).
    pub(super) keys: Keys,
    /// M-08.8: what is folded, shared by both views.
    pub(super) folds: Folds,
    /// The revision and the pattern whose marked Nano hits the window has.
    pub(super) found_told: Option<(u64, Option<String>)>,
    /// When the marked hits were found last.
    pub(super) found_at: Option<Instant>,
    /// M-09: the facade's blocks, while the facade shows the note.
    pub(super) facade: Option<Facade>,
    /// M-09.14: the markup Underline copied last from this tab, which
    /// pasting in the facade brings back as markup (`own_markup` of egui).
    pub(super) own_markup: Option<String>,
}

impl std::fmt::Debug for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tab")
            .field("path", &self.path)
            .field("title", &self.title)
            .field("revision", &self.editor.revision())
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
            format,
            editing_view,
            editor: SourceEditor::new(&body),
            autosave: Autosave::default(),
            saving: false,
            saving_text: None,
            error: None,
            deleted: false,
            kept_outside: None,
            on_disk: body,
            properties: None,
            highlight: None,
            viewport: None,
            highlight_told: None,
            keys: Keys::default(),
            folds: Folds::default(),
            found_told: None,
            found_at: None,
            facade: None,
            own_markup: None,
        }
    }

    /// M-06.1: the title, marked " ⚠" when the last save failed, else " •"
    /// with changes not stored, and " (deleted)" when the note was deleted
    /// outside.
    pub(super) fn label(&self) -> String {
        let mut label = self.title.clone();
        if self.error.is_some() {
            label.push_str(" ⚠");
        } else if self.autosave.is_unstored() {
            label.push_str(" •");
        }
        if self.deleted {
            label.push_str(" (deleted)");
        }
        label
    }

    /// M-16.2: reads the properties of the note again.
    pub(super) fn read_properties(&mut self, root: &Path) {
        self.properties = NoteProperties::read(root, &self.path, self.format).ok();
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
        let text = self.editor.text();
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
    /// held no unstored change before, so its mark changes.
    pub(super) fn changed(&mut self, now: Instant) -> bool {
        let was_unstored = self.autosave.is_unstored();
        self.autosave.changed(now);
        !was_unstored
    }

    /// The highlighting follows the changes of an input (ADR-010).
    pub(super) fn highlight_edited(&mut self, changes: &[underline_editor_core::Applied]) {
        let Some(highlight) = &mut self.highlight else {
            return;
        };
        let lines = self.editor.document().len_lines();
        for change in changes {
            if change.history {
                highlight.invalidate_from(change.line, lines);
            } else {
                highlight.edited(change.line, change.added_lines, lines);
            }
        }
    }

    /// Shows `text` from outside instead (UC-004 A7): the cursor stays at
    /// its line and column, the history and the colours start anew.
    pub(super) fn reload(&mut self, text: String) {
        self.editor.reload(&text);
        self.on_disk = text;
        self.highlight = None;
        self.highlight_told = None;
        // M-08.8: the folded sections stay folded where they are now.
        self.refold();
    }
}
