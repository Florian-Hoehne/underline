//! Tests of UC-025 Open Daily Note in the session (TC-025-01 to TC-025-10)
//! and of the templates of "New note" (UC-003 A7, A8, TC-003-17, TC-003-18),
//! with the clock injected.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use jiff::civil::{date, DateTime};
use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::notes_view::{
    CreateNoteAnswer, DailyNoteAnswer, NewNoteRequest, StatusOffer, TemplateView,
};
use crate::workspace_view::FormatName;
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-daily-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the files `files`.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    for (name, text) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(dir.join("home")).unwrap();
    let mut session = Session::new(
        Some(root.clone()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.take_events();
    (session, root)
}

fn nov11(hour: i8, minute: i8) -> DateTime {
    date(2026, 11, 11).at(hour, minute, 0, 0)
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

fn active(session: &Session) -> Option<String> {
    session.snapshot().tabs.active
}

fn first_in_overview(session: &Session) -> Option<String> {
    session
        .snapshot()
        .overview
        .first()
        .map(|item| item.note.clone())
}

/// The offer of the last status event among `events`.
fn offer(events: &[Event]) -> Option<StatusOffer> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Status(status) => Some(status.offer),
            _ => None,
        })
        .flatten()
}

/// The web offset of the character `at` in `text`.
fn web(text: &str, at: usize) -> u32 {
    let units: usize = text.chars().take(at).map(char::len_utf16).sum();
    u32::try_from(units).unwrap()
}

/// TC-025-01: created from the template, opened, listed first, the cursor
/// where the template asks, the status says it was created.
#[test]
fn uc025_tc01_creates_and_opens_the_daily_note() {
    let dir = temp_dir("tc01");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Home.md", "# Home\n"),
            (
                "templates/Daily.adoc",
                "= {{title}}\n\n{{cursor}}\n== Done\n",
            ),
        ],
    );

    let answer = session.open_daily_note_at(nov11(8, 15));

    let text = fs::read_to_string(root.join("daily/2026-11-11.adoc")).unwrap();
    assert_eq!(text, "= 2026-11-11\n\n== Done\n");
    assert_eq!(
        answer,
        DailyNoteAnswer::Opened {
            note: "daily/2026-11-11".into(),
            rev: session.mirror_rev(&id("daily/2026-11-11")),
            offset: web(&text, "= 2026-11-11\n".len()),
        }
    );
    assert_eq!(session.status(), "Created the daily note 2026-11-11");
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-11"));
    assert_eq!(
        first_in_overview(&session).as_deref(),
        Some("daily/2026-11-11")
    );
}

/// TC-025-02, TC-025-03: without a template only the title, the cursor at
/// the end; asked again it opens unchanged, and while open its tab becomes
/// the active tab.
#[test]
fn uc025_tc02_tc03_without_template_and_existing() {
    let dir = temp_dir("tc03");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    let path = root.join("daily/2026-11-11.md");

    let answer = session.open_daily_note_at(nov11(8, 15));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# 2026-11-11\n");
    assert!(matches!(answer, DailyNoteAnswer::Opened { offset: 13, .. }));

    // The tab is closed, the note was written meanwhile.
    fs::write(&path, "# 2026-11-11\ntoday\n").unwrap();
    session.close_tab(&id("daily/2026-11-11"), true);
    session.open_note(&id("Home"));
    session.status = String::new();
    let answer = session.open_daily_note_at(nov11(9, 0));
    assert_eq!(
        answer,
        DailyNoteAnswer::Opened {
            note: "daily/2026-11-11".into(),
            rev: session.mirror_rev(&id("daily/2026-11-11")),
            offset: 19,
        }
    );
    assert_eq!(session.status(), "", "nothing was created");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# 2026-11-11\ntoday\n");

    session.open_note(&id("Home"));
    let answer = session.open_daily_note_at(nov11(9, 5));
    assert_eq!(
        answer,
        DailyNoteAnswer::Shown {
            note: "daily/2026-11-11".into()
        }
    );
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-11"));
}

/// TC-025-04: the previous and the next daily note; none between, and the
/// status says when there is none or the active note is no daily note.
#[test]
fn uc025_tc04_previous_and_next_daily_note() {
    let dir = temp_dir("tc04");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Home.md", "# Home\n"),
            ("daily/2026-11-05.md", "# 2026-11-05\n"),
            ("daily/2026-11-11.md", "# 2026-11-11\n"),
        ],
    );
    session.open_note(&id("Home"));
    assert_eq!(session.step_daily_note(false), DailyNoteAnswer::NotDaily);

    session.open_note(&id("daily/2026-11-11"));
    assert_eq!(
        session.step_daily_note(false),
        DailyNoteAnswer::Shown {
            note: "daily/2026-11-05".into()
        }
    );
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-05"));
    assert_eq!(session.status(), "", "an earlier report does not stay");
    assert_eq!(session.step_daily_note(false), DailyNoteAnswer::Done);
    assert_eq!(session.status(), "There is no earlier daily note");
    session.step_daily_note(true);
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-11"));
    session.step_daily_note(true);
    assert_eq!(session.status(), "There is no later daily note");
    assert_eq!(fs::read_dir(root.join("daily")).unwrap().count(), 2);
}

/// TC-025-05, TC-025-08: the folder and the template from the workspace
/// settings; a template they name that does not exist is said.
#[test]
fn uc025_tc05_tc08_settings_of_the_workspace() {
    let dir = temp_dir("tc08");
    let (mut session, root) = session_with(
        &dir,
        &[
            (
                SETTINGS_FILE_NAME,
                "daily_note_folder: journal/days\ndaily_note_template: templates/Day.md\n",
            ),
            ("templates/Day.md", "# {{date}}\nWritten at {{time}}\n"),
        ],
    );
    session.open_daily_note_at(nov11(8, 15));
    assert_eq!(
        fs::read_to_string(root.join("journal/days/2026-11-11.md")).unwrap(),
        "# 2026-11-11\nWritten at 08:15\n"
    );

    fs::write(
        root.join(SETTINGS_FILE_NAME),
        "daily_note_template: templates/Journal-Day.md\n",
    )
    .unwrap();
    let (mut session, root) = session_with(&dir, &[]);
    session.open_daily_note_at(nov11(8, 15));
    assert_eq!(
        fs::read_to_string(root.join("daily/2026-11-11.md")).unwrap(),
        "# 2026-11-11\n"
    );
    assert_eq!(
        session.status(),
        "Created the daily note 2026-11-11 with only its title: the daily note template \
         templates/Journal-Day.md was not found"
    );
}

/// TC-025-06: a hidden daily note folder creates nothing; the status says
/// why and offers the settings of the workspace, until another report.
#[test]
fn uc025_tc06_folder_not_allowed_offers_the_settings() {
    let dir = temp_dir("tc06");
    let (mut session, root) = session_with(
        &dir,
        &[
            (SETTINGS_FILE_NAME, "daily_note_folder: .days\n"),
            ("Home.md", "# Home\n"),
        ],
    );

    let answer = session.open_daily_note_at(nov11(8, 15));

    assert_eq!(answer, DailyNoteAnswer::Done);
    assert_eq!(
        session.status(),
        "The daily note cannot be created in the daily note folder: the folder `.days` is hidden"
    );
    assert!(!root.join(".days").exists());
    assert_eq!(
        offer(&session.take_events()),
        Some(StatusOffer::WorkspaceSettings)
    );
    session.status = "Something else".into();
    assert_eq!(offer(&session.take_events()), None);
}

/// TC-025-07: a folder that cannot be created is said, nothing is left.
#[cfg(unix)]
#[test]
fn uc025_tc07_cannot_be_created() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("tc07");
    let (mut session, root) = session_with(&dir, &[("Home.md", "# Home\n")]);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
    let writable = fs::write(root.join("probe"), "").is_ok();

    session.open_daily_note_at(nov11(8, 15));

    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return; // Running as root.
    }
    assert!(
        session
            .status()
            .starts_with("The daily note could not be created: cannot create the folder"),
        "{}",
        session.status()
    );
    assert!(!root.join("daily").exists());
}

/// TC-025-10: the day changes at midnight.
#[test]
fn uc025_tc10_the_day_changes_at_midnight() {
    let dir = temp_dir("tc10");
    let (mut session, root) = session_with(&dir, &[("daily/2026-11-11.md", "# 2026-11-11\n")]);
    session.open_daily_note_at(nov11(23, 59));
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-11"));
    session.open_daily_note_at(date(2026, 11, 12).at(0, 1, 0, 0));
    assert_eq!(active(&session).as_deref(), Some("daily/2026-11-12"));
    assert!(root.join("daily/2026-11-12.md").exists());
}

fn template_request(title: &str, template: Option<&str>) -> NewNoteRequest {
    NewNoteRequest {
        title: title.into(),
        folder: String::new(),
        format: FormatName::Markdown,
        template: template.map(str::to_owned),
        side: false,
    }
}

const MEETING: &str = "# {{title}}\nDate: {{date}} {{time}}\n{{cursor}}\nOwner: {{owner}}\n";

/// TC-003-17: the dialog offers the templates; the note is made from one
/// with the cursor where it marks it.
#[test]
fn uc003_tc17_new_note_from_a_template() {
    let dir = temp_dir("tc17");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("templates/Meeting.md", MEETING),
            ("templates/Spec.adoc", "= {{title}}\n"),
            ("templates/work/Retro-Notes.md", "# {{title}}\n"),
        ],
    );
    let start = session.new_note_start().unwrap();
    assert_eq!(
        start.templates,
        [
            TemplateView {
                note: "templates/Meeting".into(),
                label: "Meeting".into(),
                format: FormatName::Markdown,
            },
            TemplateView {
                note: "templates/Spec".into(),
                label: "Spec".into(),
                format: FormatName::AsciiDoc,
            },
            TemplateView {
                note: "templates/work/Retro-Notes".into(),
                label: "Retro Notes (work)".into(),
                format: FormatName::Markdown,
            },
        ]
    );

    let answer = session.create_note_at(
        &template_request("Standup", Some("templates/Meeting")),
        date(2026, 11, 11).at(9, 0, 0, 0),
    );

    let text = fs::read_to_string(root.join("Standup.md")).unwrap();
    assert_eq!(
        text,
        "# Standup\nDate: 2026-11-11 09:00\n\nOwner: {{owner}}\n"
    );
    assert_eq!(
        answer,
        CreateNoteAnswer::Created {
            note: "Standup".into(),
            rev: session.mirror_rev(&id("Standup")),
            offset: web(&text, "# Standup\nDate: 2026-11-11 09:00\n".len()),
        }
    );
    assert_eq!(session.status(), "Created the note Standup");
    assert_eq!(
        fs::read_to_string(root.join("templates/Meeting.md")).unwrap(),
        MEETING
    );

    session.create_note_at(
        &template_request("API", Some("templates/Spec")),
        date(2026, 11, 11).at(9, 0, 0, 0),
    );
    assert_eq!(
        fs::read_to_string(root.join("API.adoc")).unwrap(),
        "= API\n"
    );
}

/// TC-003-18: a template deleted after it was offered gives the title only;
/// the status says the template could not be read.
#[test]
fn uc003_tc18_template_deleted_after_it_was_offered() {
    let dir = temp_dir("tc18");
    let (mut session, root) = session_with(&dir, &[("templates/Meeting.md", MEETING)]);
    assert_eq!(session.new_note_start().unwrap().templates.len(), 1);
    fs::remove_file(root.join("templates/Meeting.md")).unwrap();

    session.create_note_at(
        &template_request("Retro", Some("templates/Meeting")),
        date(2026, 11, 11).at(9, 0, 0, 0),
    );

    assert_eq!(
        fs::read_to_string(root.join("Retro.md")).unwrap(),
        "# Retro\n"
    );
    assert!(
        session
            .status()
            .starts_with("Created the note Retro with only its title: the template templates/Meeting.md could not be read"),
        "{}",
        session.status()
    );
}

/// TC-010-22 step 3: another template folder from the workspace settings.
#[test]
fn uc010_tc22_template_folder_from_the_settings() {
    let dir = temp_dir("tc22");
    let (session, _) = session_with(
        &dir,
        &[
            (SETTINGS_FILE_NAME, "template_folder: tpl\n"),
            ("tpl/Day.md", ""),
            ("templates/Meeting.md", ""),
        ],
    );
    let labels: Vec<String> = session
        .new_note_start()
        .unwrap()
        .templates
        .into_iter()
        .map(|template| template.label)
        .collect();
    assert_eq!(labels, ["Day"]);
}

/// UC-003 BR-008 (monkey): templates of the same title in folders that
/// differ only by `-` or a space are offered under different labels; the
/// dialog cannot tell two identical labels apart.
#[test]
fn uc003_br008_same_title_in_similar_folders_has_distinct_labels() {
    let dir = temp_dir("labels");
    let (session, _) = session_with(
        &dir,
        &[("templates/a-b/M.md", "x"), ("templates/a b/M.md", "y")],
    );
    let templates = session.new_note_start().unwrap().templates;
    assert_eq!(templates.len(), 2);
    assert_ne!(templates[0].label, templates[1].label, "{templates:?}");
}
