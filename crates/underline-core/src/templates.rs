//! UC-003 A7, A8, BR-008 (FR-094): templates, the notes of the template
//! folder that a new note can start from, and their placeholders; UC-025
//! (FR-095): the daily note of a day, found in or created in the daily
//! note folder, and the daily notes before and after it.
//!
//! The clock is a parameter ([`DateTime`] of the local time zone, see
//! [`local_now`]), so the day that changes at midnight (UC-025 BR-001) is
//! tested without waiting for it.

use jiff::civil::{Date, DateTime, Time};

use crate::format::NoteFormat;
use crate::new_note::{folder_levels, heading_title, NewNote, NewNoteProblem};
use crate::note::NoteId;
use crate::workspace::Workspace;

/// UC-003 BR-008, UC-010 BR-001: the template folder unless the workspace
/// settings name another.
pub const TEMPLATE_FOLDER: &str = "templates";
/// UC-025 BR-002, UC-010 BR-001: the daily note folder unless the
/// workspace settings name another.
pub const DAILY_NOTE_FOLDER: &str = "daily";
/// UC-025 BR-003: the title of the daily note template unless the
/// workspace settings name another.
pub const DAILY_TEMPLATE_TITLE: &str = "Daily";

/// UC-003 BR-008: the placeholders of a template.
const TITLE: &str = "{{title}}";
const DATE: &str = "{{date}}";
const TIME: &str = "{{time}}";
const CURSOR: &str = "{{cursor}}";

/// UC-025 BR-001: the date and time of the clock of this computer, in its
/// time zone. When the system's time zone cannot be found out, jiff uses
/// UTC, so the day may then change at another hour than the user's
/// midnight.
#[must_use]
pub fn local_now() -> DateTime {
    jiff::Zoned::now().datetime()
}

/// UC-025 BR-001, UC-003 BR-008: a date as `2026-11-11`.
#[must_use]
pub fn date_text(date: Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

/// UC-003 BR-008: a time as `14:30`.
#[must_use]
pub fn time_text(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// UC-025 BR-001: the date a daily note's title gives, exactly `2026-11-11`;
/// `None` for any other title.
#[must_use]
pub fn daily_date(title: &str) -> Option<Date> {
    let bytes = title.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(at, b)| at == 4 || at == 7 || b.is_ascii_digit());
    if !shape {
        return None;
    }
    let year = title[0..4].parse().ok()?;
    let month = title[5..7].parse().ok()?;
    let day = title[8..10].parse().ok()?;
    Date::new(year, month, day).ok()
}

/// A note of the template folder (entity TEMPLATE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// The note that serves as the template.
    pub id: NoteId,
    /// Its note format, which notes made from it get (UC-003 A7 step 1).
    pub format: NoteFormat,
}

impl Template {
    /// Its title, as the note overview shows it.
    #[must_use]
    pub fn title(&self) -> String {
        self.id.title()
    }

    /// Its file in the workspace, such as `templates/Meeting.md`.
    #[must_use]
    pub fn file_name(&self) -> String {
        self.id.file_name(self.format)
    }
}

/// A template with its placeholders filled in (UC-003 BR-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filled {
    /// The content of the new note.
    pub text: String,
    /// Where `{{cursor}}` stood, in characters of `text`; `None` without.
    pub cursor: Option<usize>,
}

/// The placeholder `tail` starts with, and its length in bytes.
fn placeholder(tail: &str) -> Option<(&'static str, usize)> {
    [TITLE, DATE, TIME, CURSOR]
        .into_iter()
        .find(|name| tail.starts_with(name))
        .map(|name| (name, name.len()))
}

/// OQ-275, TC-025-01: a line that holds only `{{cursor}}` right after an
/// empty line takes the place of that empty line, so the cursor stands on
/// it and no second empty line is left; elsewhere (TC-003-17) it stays a
/// line of its own, which becomes empty.
fn join_cursor_lines(template: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for line in template.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let after_empty = lines
            .last()
            .is_some_and(|before| before.trim_end_matches(['\n', '\r']).is_empty());
        if content == CURSOR && after_empty {
            lines.pop();
        }
        lines.push(line);
    }
    lines.concat()
}

/// UC-003 BR-008: `template` with `{{title}}`, `{{date}}`, `{{time}}`
/// filled in and `{{cursor}}` taken out, its first place kept as the
/// cursor. Any other text in double curly braces stays as it is written;
/// the placeholders are matched exactly, in lower case and without spaces.
/// A line of only `{{cursor}}` after an empty line replaces it (OQ-275).
///
/// The cursor counts characters (`char`s) of the filled text, a `\r` of a
/// CRLF line ending included, as the editor's text holds the file as it is.
#[must_use]
pub fn fill_template(template: &str, title: &str, date: Date, time: Time) -> Filled {
    let title = heading_title(title);
    let date = date_text(date);
    let time = time_text(time);
    let template = join_cursor_lines(template);
    let mut text = String::with_capacity(template.len());
    let mut cursor = None;
    let mut rest = template.as_str();
    while let Some(at) = rest.find("{{") {
        text.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some((name, length)) = placeholder(tail) else {
            // Not a placeholder: the first brace stays, and the search goes
            // on after it, so `{{{title}}}` still fills in the title.
            text.push('{');
            rest = &tail[1..];
            continue;
        };
        match name {
            TITLE => text.push_str(&title),
            DATE => text.push_str(&date),
            TIME => text.push_str(&time),
            _ => {
                cursor.get_or_insert_with(|| text.chars().count());
            }
        }
        rest = &tail[length..];
    }
    text.push_str(rest);
    Filled { text, cursor }
}

/// The folder a setting names, as levels joined by `/` (`""` for the top
/// folder); an empty or missing setting gives `default` (entity SETTINGS:
/// an empty value means the setting is not given).
pub(crate) fn configured_folder(
    setting: Option<&str>,
    default: &str,
) -> Result<String, NewNoteProblem> {
    let value = setting.map(str::trim).filter(|value| !value.is_empty());
    Ok(folder_levels(value.unwrap_or(default))?.join("/"))
}

/// UC-003 BR-008: the template folder a setting names (`templates` when
/// not given), as levels joined by `/`; `None` when it cannot hold
/// templates: outside the workspace, hidden, a name not allowed, or the
/// top folder, which would make every note a template (OQ-274).
#[must_use]
pub fn template_folder(setting: Option<&str>) -> Option<String> {
    configured_folder(setting, TEMPLATE_FOLDER)
        .ok()
        .filter(|folder| !folder.is_empty())
}

/// UC-010 BR-001, OQ-274 (b): why `value` cannot be the template folder
/// (`top_allowed` false) or the daily note folder: it lies outside the
/// workspace, is hidden, holds a name not allowed, or is the top folder,
/// which cannot hold the templates. `None` when it can; an empty value is
/// no value of the workspace's own and always can.
#[must_use]
pub fn folder_setting_problem(value: &str, top_allowed: bool) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    match folder_levels(value) {
        Err(problem) => Some(problem.to_string()),
        Ok(levels) if levels.is_empty() && !top_allowed => {
            Some("the top folder cannot hold the templates".to_owned())
        }
        Ok(_) => None,
    }
}

/// Whether the folder `a` is the folder `b`, upper and lower case ignored
/// as for the names of notes (UC-003 BR-003), so `Templates` holds the
/// notes of `templates` on a system that does not tell them apart.
fn same_folder(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Whether `id` lies in `folder` (levels joined by `/`) or below it.
fn lies_in(id: &NoteId, folder: &str) -> bool {
    id.as_str()
        .get(..folder.len())
        .is_some_and(|start| same_folder(start, folder))
        && id.as_str()[folder.len()..].starts_with('/')
}

/// The page name of `id`, the last part of its place.
fn page_of(id: &NoteId) -> &str {
    id.as_str().split('/').next_back().unwrap_or_default()
}

/// UC-025 BR-003: the daily note template, as the settings and the
/// template folder give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DailyTemplate {
    /// No template is named, and the template folder holds none titled
    /// Daily: the daily note holds only its title (BR-004).
    None,
    /// The template a new daily note starts from.
    Found(Template),
    /// A3: the workspace settings name a template that does not exist;
    /// the setting as written.
    NotFound(String),
}

/// What became of the template of a new note (UC-003 A7, A8; UC-025 A3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateUse {
    /// The note holds only its title as heading, as asked.
    None,
    /// The note holds the template with its placeholders filled in.
    Used(Template),
    /// UC-025 A3: the template the settings name does not exist; the note
    /// holds only its title.
    NotFound(String),
    /// UC-003 A8: the template cannot be read, or was deleted after it was
    /// offered; the note holds only its title. The template's file and why.
    Unreadable { name: String, reason: String },
}

/// A note created from a template or with only its title (UC-003 step 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedNote {
    /// The new note.
    pub id: NoteId,
    /// Where `{{cursor}}` of the template was, in characters of its text;
    /// `None` for the end of the note (UC-003 A7 step 2).
    pub cursor: Option<usize>,
    /// What became of the template.
    pub template: TemplateUse,
}

/// The template a new note was asked to start from (UC-003 A7, A8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateChoice {
    /// "No template".
    None,
    /// A template of the workspace.
    Template(Template),
    /// The template chosen, which is no template of the workspace now, by
    /// its place (A8: deleted after it was offered).
    Gone(String),
    /// UC-025 A3: the settings name a template that does not exist.
    NotFound(String),
}

/// The daily note asked for (UC-025 steps 2 to 4, A1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DailyNote {
    /// A1: the daily note folder holds the note of the day, in either note
    /// format; nothing was created.
    Exists(NoteId),
    /// Step 3: the note was created.
    Created(CreatedNote),
}

/// UC-025: what the settings of the workspace give for the daily note, and
/// the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyRequest<'a> {
    /// The setting "Daily note folder", if given (BR-002).
    pub folder: Option<&'a str>,
    /// The setting "Template folder", if given (UC-003 BR-008).
    pub template_folder: Option<&'a str>,
    /// The setting "Daily note template", if given (BR-003).
    pub template: Option<&'a str>,
    /// The date and time of the clock of this computer (BR-001).
    pub now: DateTime,
}

/// A2, BR-005: the note asked from is no daily note: it is not titled with
/// a date, or it lies outside the daily note folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the note is no daily note")]
pub struct NotDaily;

/// What a new note is written with (UC-003 step 5, A7, A8): the request
/// with the note format it gets, its content, and the template's outcome.
struct Plan {
    new: NewNote,
    content: String,
    /// Where `{{cursor}}` was, in characters of `content`.
    cursor: Option<usize>,
    used: TemplateUse,
}

impl Workspace {
    /// UC-003 BR-008, entity TEMPLATE: the notes of the template folder
    /// (`setting`, `templates` when not given) and of its folders, by
    /// their place. A folder that is missing, lies outside the workspace,
    /// or is hidden holds none: only the listed notes of the workspace can
    /// be templates.
    #[must_use]
    pub fn templates(&self, setting: Option<&str>) -> Vec<Template> {
        let Some(folder) = template_folder(setting) else {
            return Vec::new();
        };
        let mut templates: Vec<Template> = self
            .listing()
            .notes
            .iter()
            .filter(|entry| lies_in(&entry.id, &folder))
            .map(|entry| Template {
                id: entry.id.clone(),
                format: entry.format,
            })
            .collect();
        templates.sort_by(|a, b| a.id.cmp(&b.id));
        templates
    }

    /// UC-003 A8: the template chosen in the dialog by its note, if it is a
    /// template of the workspace now.
    #[must_use]
    pub fn template_choice(&self, setting: Option<&str>, chosen: Option<&str>) -> TemplateChoice {
        let Some(chosen) = chosen.map(str::trim).filter(|chosen| !chosen.is_empty()) else {
            return TemplateChoice::None;
        };
        self.templates(setting)
            .into_iter()
            .find(|template| template.id.as_str() == chosen)
            .map_or_else(
                || TemplateChoice::Gone(chosen.to_owned()),
                TemplateChoice::Template,
            )
    }

    /// UC-025 BR-003, UC-010 BR-001: the daily note template: the note the
    /// setting `named` gives, by its place in the workspace with or without
    /// extension, or else the template titled Daily in the template folder,
    /// the one nearest the top of it first.
    #[must_use]
    pub fn daily_template(
        &self,
        template_folder: Option<&str>,
        named: Option<&str>,
    ) -> DailyTemplate {
        if let Some(named) = named.map(str::trim).filter(|named| !named.is_empty()) {
            return self.named_note(named).map_or_else(
                || DailyTemplate::NotFound(named.to_owned()),
                DailyTemplate::Found,
            );
        }
        let mut daily: Vec<Template> = self
            .templates(template_folder)
            .into_iter()
            .filter(|template| page_of(&template.id).eq_ignore_ascii_case(DAILY_TEMPLATE_TITLE))
            .collect();
        daily.sort_by_key(|template| template.id.as_str().matches('/').count());
        daily
            .into_iter()
            .next()
            .map_or(DailyTemplate::None, DailyTemplate::Found)
    }

    /// The listed note `named` gives: `templates/Day.md` or `templates/Day`.
    fn named_note(&self, named: &str) -> Option<Template> {
        let named = named.trim_start_matches("./");
        let (path, format) = match named.rsplit_once('.') {
            Some((stem, extension)) if !stem.ends_with('/') => {
                match NoteFormat::from_extension(extension) {
                    Some(format) => (stem, Some(format)),
                    None => (named, None),
                }
            }
            _ => (named, None),
        };
        let id = NoteId::from_relative(path);
        if !id.is_valid() {
            return None;
        }
        self.listing()
            .notes
            .iter()
            .find(|entry| entry.id == id && format.is_none_or(|format| entry.format == format))
            .map(|entry| Template {
                id: entry.id.clone(),
                format: entry.format,
            })
    }

    /// UC-003 steps 4 and 5, A7, A8: creates the note `new`, from
    /// `template` with its placeholders filled in for `now`, or holding only
    /// its title as `heading` writes it in its note format. A note made from
    /// a template gets the note format of the template; a template that
    /// cannot be read gives the note with only its title.
    ///
    /// # Errors
    ///
    /// As [`Workspace::create_note`]: the title or folder is not allowed,
    /// the note exists, or it cannot be written.
    pub fn create_note_from(
        &mut self,
        new: &NewNote,
        template: &TemplateChoice,
        now: DateTime,
        heading: &dyn Fn(NoteFormat, &str) -> String,
    ) -> Result<CreatedNote, NewNoteProblem> {
        self.create_dated(new, template, now.date(), now.time(), heading)
    }

    /// [`Workspace::create_note_from`] with the date of the note apart from
    /// the clock (UC-025 BR-003: the date of the daily note).
    fn create_dated(
        &mut self,
        new: &NewNote,
        template: &TemplateChoice,
        date: Date,
        time: Time,
        heading: &dyn Fn(NoteFormat, &str) -> String,
    ) -> Result<CreatedNote, NewNoteProblem> {
        let title_only = |used: TemplateUse| Plan {
            new: new.clone(),
            content: heading(new.format, &heading_title(&new.title)),
            cursor: None,
            used,
        };
        let plan = match template {
            TemplateChoice::None => title_only(TemplateUse::None),
            TemplateChoice::NotFound(name) => title_only(TemplateUse::NotFound(name.clone())),
            TemplateChoice::Gone(name) => title_only(TemplateUse::Unreadable {
                name: name.clone(),
                reason: "it is no template of the workspace now".to_owned(),
            }),
            TemplateChoice::Template(template) => match self.read(&template.id) {
                Ok(note) => {
                    let filled = fill_template(&note.body, &new.title, date, time);
                    Plan {
                        new: NewNote {
                            format: template.format,
                            ..new.clone()
                        },
                        content: filled.text,
                        cursor: filled.cursor,
                        used: TemplateUse::Used(template.clone()),
                    }
                }
                Err(err) => title_only(TemplateUse::Unreadable {
                    name: template.file_name(),
                    reason: err.to_string(),
                }),
            },
        };
        let id = self.create_note(&plan.new, &plan.content)?;
        Ok(CreatedNote {
            id,
            cursor: plan.cursor,
            template: plan.used,
        })
    }

    /// UC-025 steps 2 and 3, A1, A3 to A5, BR-001 to BR-006: the daily note
    /// of the day of `request.now`: the note titled with that date in the
    /// daily note folder, in either note format, or else a new one there,
    /// from the daily note template in its note format, or holding only its
    /// title in the default format of the workspace.
    ///
    /// # Errors
    ///
    /// A4: the daily note folder is not allowed ([`NewNoteProblem::Outside`],
    /// `Hidden`, `Ignored`, `OwnWorkspace`, `FolderName`, `PathTooLong`);
    /// A5: the folder or the note cannot be created
    /// ([`NewNoteProblem::Io`]); the folders created for it are removed.
    pub fn open_or_create_daily_note(
        &mut self,
        request: &DailyRequest<'_>,
        heading: &dyn Fn(NoteFormat, &str) -> String,
    ) -> Result<DailyNote, NewNoteProblem> {
        let folder = configured_folder(request.folder, DAILY_NOTE_FOLDER)?;
        let date = request.now.date();
        let choice = match self.daily_template(request.template_folder, request.template) {
            DailyTemplate::None => TemplateChoice::None,
            DailyTemplate::Found(template) => TemplateChoice::Template(template),
            DailyTemplate::NotFound(name) => TemplateChoice::NotFound(name),
        };
        let format = match &choice {
            TemplateChoice::Template(template) => template.format,
            _ => self.default_format(),
        };
        let new = NewNote {
            folder,
            title: date_text(date),
            format,
        };
        // Step 2, BR-006: never a second daily note of the day.
        match self.plan_note(&new) {
            Ok(_) => {}
            Err(NewNoteProblem::Exists(id)) => return Ok(DailyNote::Exists(id)),
            Err(problem) => return Err(problem),
        }
        match self.create_dated(&new, &choice, date, request.now.time(), heading) {
            Ok(created) => Ok(DailyNote::Created(created)),
            // It appeared before it was written (BR-006).
            Err(NewNoteProblem::Exists(id)) => Ok(DailyNote::Exists(id)),
            Err(problem) => Err(problem),
        }
    }

    /// UC-025 A2: the daily notes of the daily note folder (`setting`,
    /// `daily` when not given), by their date; notes of its folders are
    /// none (BR-002).
    #[must_use]
    pub fn daily_notes(&self, setting: Option<&str>) -> Vec<(Date, NoteId)> {
        let Ok(folder) = configured_folder(setting, DAILY_NOTE_FOLDER) else {
            return Vec::new();
        };
        let mut notes: Vec<(Date, NoteId)> = self
            .listing()
            .notes
            .iter()
            .filter(|entry| same_folder(entry.id.folder().unwrap_or_default(), &folder))
            .filter_map(|entry| Some((daily_date(page_of(&entry.id))?, entry.id.clone())))
            .collect();
        notes.sort();
        notes
    }

    /// UC-025 A2: the daily note of the nearest earlier (`later` false) or
    /// later date than the daily note `from`; `None` when there is none.
    ///
    /// # Errors
    ///
    /// [`NotDaily`] when `from` is not a daily note of the daily note
    /// folder (BR-005).
    pub fn daily_note_beside(
        &self,
        setting: Option<&str>,
        from: &NoteId,
        later: bool,
    ) -> Result<Option<NoteId>, NotDaily> {
        let notes = self.daily_notes(setting);
        let Some((date, _)) = notes.iter().find(|(_, id)| id == from) else {
            return Err(NotDaily);
        };
        let date = *date;
        let beside = if later {
            notes.iter().find(|(other, _)| *other > date)
        } else {
            notes.iter().rev().find(|(other, _)| *other < date)
        };
        Ok(beside.map(|(_, id)| id.clone()))
    }
}

#[cfg(test)]
mod tests;
