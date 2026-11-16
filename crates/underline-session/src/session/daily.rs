//! UC-025 Open Daily Note in the session (FR-095): the daily note of today,
//! opened or created from the daily note template in the daily note folder
//! the workspace settings name, and the daily notes before and after the
//! one in the active tab (A2).
//!
//! The domain is `underline_core::templates`; here the settings of the
//! workspace are read, the note is opened in a tab and listed first, and
//! the status bar says what happened. The clock is a parameter
//! ([`Session::open_daily_note_at`]), so the tests choose the day.

use jiff::civil::DateTime;
use underline_core::{
    date_text, local_now, DailyNote, DailyRequest, NewNoteProblem, NotDaily, NoteId, SettingKey,
    TemplateUse,
};

use super::Session;
use crate::notes_view::DailyNoteAnswer;

impl Session {
    /// UC-025 steps 1 to 4, A1, A3 to A5: opens the daily note of today by
    /// the clock of this computer, creating it first when the daily note
    /// folder holds none (BR-001, BR-006).
    pub fn open_daily_note(&mut self) -> DailyNoteAnswer {
        self.open_daily_note_at(local_now())
    }

    /// [`Session::open_daily_note`] at the time `now` of the local clock.
    ///
    /// Step 4: a new daily note opens in a tab, listed first, with the
    /// cursor where its template marks it or else at its end, and the
    /// status says it was created (A3: that the template was not found).
    /// A1: an existing one opens with the cursor at its end, or its open
    /// tab becomes the active tab. A4: a daily note folder that is not
    /// allowed creates nothing; the status says why and offers the settings
    /// of the workspace. A5: a failure to write says why.
    pub fn open_daily_note_at(&mut self, now: DateTime) -> DailyNoteAnswer {
        let folder = self.settings.workspace_text(SettingKey::DailyNoteFolder);
        let template_folder = self.settings.workspace_text(SettingKey::TemplateFolder);
        let template = self.settings.workspace_text(SettingKey::DailyNoteTemplate);
        let formats = &self.formats;
        let Some(open) = &mut self.open else {
            return DailyNoteAnswer::Done;
        };
        let request = DailyRequest {
            folder: folder.as_deref(),
            template_folder: template_folder.as_deref(),
            template: template.as_deref(),
            now,
        };
        let heading = |format, title: &str| formats.new_note(format, title);
        let title = date_text(now.date());
        match open.workspace.open_or_create_daily_note(&request, &heading) {
            Ok(DailyNote::Exists(id)) => {
                let was_open = open.tabs.find(&id).is_some();
                // UX review: an earlier report of a daily note does not stay.
                self.status.clear();
                if !self.open_daily(&id) {
                    return DailyNoteAnswer::Done;
                }
                if was_open {
                    return DailyNoteAnswer::Shown {
                        note: id.as_str().to_owned(),
                    };
                }
                self.jump(&id, None)
            }
            Ok(DailyNote::Created(created)) => {
                self.status.clear();
                let opened = self.open_daily(&created.id);
                let why = std::mem::take(&mut self.status);
                self.status = match created.template {
                    TemplateUse::NotFound(name) => format!(
                        "Created the daily note {title} with only its title: the daily note template {name} was not found"
                    ),
                    TemplateUse::Unreadable { name, reason } => format!(
                        "Created the daily note {title} with only its title: the daily note template {name} could not be read ({reason})"
                    ),
                    TemplateUse::None | TemplateUse::Used(_) => {
                        format!("Created the daily note {title}")
                    }
                };
                if !opened {
                    self.status = format!("{}; it could not be opened: {why}", self.status);
                    return DailyNoteAnswer::Done;
                }
                self.jump(&created.id, created.cursor)
            }
            Err(problem @ NewNoteProblem::Io { .. }) => {
                self.status = format!("The daily note could not be created: {problem}");
                DailyNoteAnswer::Done
            }
            Err(problem) => {
                self.status =
                    format!("The daily note cannot be created in the daily note folder: {problem}");
                self.settings_offer = Some(self.status.clone());
                DailyNoteAnswer::Done
            }
        }
    }

    /// UC-025 A2: opens the daily note of the nearest earlier (`later`
    /// false) or later date than the daily note in the active tab; none is
    /// created for the days between. The status says when there is none;
    /// when the active note is no daily note (BR-005, OQ-271) the answer
    /// says so, and the window names the key of today's daily note.
    pub fn step_daily_note(&mut self, later: bool) -> DailyNoteAnswer {
        let folder = self.settings.workspace_text(SettingKey::DailyNoteFolder);
        let Some(open) = &self.open else {
            return DailyNoteAnswer::Done;
        };
        let active = if open.todos_shown() {
            None
        } else {
            open.tabs.active().map(|(note, _)| note.clone())
        };
        let Some(active) = active else {
            return DailyNoteAnswer::NotDaily;
        };
        match open
            .workspace
            .daily_note_beside(folder.as_deref(), &active, later)
        {
            Ok(Some(id)) => {
                self.status.clear();
                if self.open_daily(&id) {
                    return DailyNoteAnswer::Shown {
                        note: id.as_str().to_owned(),
                    };
                }
            }
            Ok(None) => {
                self.status = if later {
                    "There is no later daily note".to_owned()
                } else {
                    "There is no earlier daily note".to_owned()
                };
            }
            Err(NotDaily) => return DailyNoteAnswer::NotDaily,
        }
        DailyNoteAnswer::Done
    }

    /// Opens the daily note `id`; when it cannot be, the status says why
    /// (UC-004 A10 or the limit of tabs), never nothing.
    fn open_daily(&mut self, id: &NoteId) -> bool {
        let opened = self.open_note(id);
        if !opened && self.status.is_empty() {
            self.status = format!("The daily note {} could not be opened", id.title());
        }
        opened
    }

    /// The daily note `id` is open: its cursor goes to `cursor` (in
    /// characters), or to its end.
    fn jump(&self, id: &NoteId, cursor: Option<usize>) -> DailyNoteAnswer {
        self.place_in(id, cursor)
            .map_or(DailyNoteAnswer::Done, |(rev, offset)| {
                DailyNoteAnswer::Opened {
                    note: id.as_str().to_owned(),
                    rev,
                    offset,
                }
            })
    }
}

#[cfg(test)]
mod tests;
