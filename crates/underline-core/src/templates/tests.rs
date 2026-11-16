//! UC-003 A7, A8, BR-008 (TC-003-17, TC-003-18) and UC-025 (TC-025-01 to
//! TC-025-10), as far as they concern the domain; the clock is injected.

use std::fs;
use std::path::PathBuf;

use jiff::civil::{date, time, DateTime};

use super::*;
use crate::new_note::NewNote;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let root =
            std::env::temp_dir().join(format!("underline-templates-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (path, content) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        Self(root)
    }

    fn open(&self) -> Workspace {
        Workspace::open(&self.0).unwrap()
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap()
    }

    fn exists(&self, path: &str) -> bool {
        self.0.join(path).exists()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn heading(format: NoteFormat, title: &str) -> String {
    match format {
        NoteFormat::Markdown => format!("# {title}\n"),
        NoteFormat::AsciiDoc => format!("= {title}\n"),
    }
}

/// 11 November 2026 at `hour`:`minute`.
fn nov11(hour: i8, minute: i8) -> DateTime {
    date(2026, 11, 11).at(hour, minute, 0, 0)
}

fn daily(now: DateTime) -> DailyRequest<'static> {
    DailyRequest {
        folder: None,
        template_folder: None,
        template: None,
        now,
    }
}

fn created(note: DailyNote) -> CreatedNote {
    match note {
        DailyNote::Created(created) => created,
        DailyNote::Exists(id) => panic!("expected a new daily note, {id:?} exists"),
    }
}

const MEETING: &str = "# {{title}}\nDate: {{date}} {{time}}\n{{cursor}}\nOwner: {{owner}}\n";

/// TC-003-17 step 3: the placeholders are filled in, `{{cursor}}` taken
/// out and its place kept, other braces left as they are written.
#[test]
fn uc003_tc17_fills_the_placeholders() {
    let filled = fill_template(MEETING, "  Standup ", date(2026, 11, 11), time(9, 0, 0, 0));
    assert_eq!(
        filled.text,
        "# Standup\nDate: 2026-11-11 09:00\n\nOwner: {{owner}}\n"
    );
    let cursor = filled.cursor.unwrap();
    assert_eq!(
        filled.text.chars().take(cursor).collect::<String>(),
        "# Standup\nDate: 2026-11-11 09:00\n"
    );
}

/// UC-003 BR-008: only the exact placeholders are filled; the cursor is
/// counted in characters, and only its first place counts.
#[test]
fn uc003_br008_only_exact_placeholders_and_first_cursor() {
    let filled = fill_template(
        "Ü {{{title}}} {{ title }} {{Title}} {{date}}{{cursor}}x{{cursor}}",
        "A  B",
        date(2026, 1, 2),
        time(23, 5, 0, 0),
    );
    assert_eq!(filled.text, "Ü {A B} {{ title }} {{Title}} 2026-01-02x");
    assert_eq!(
        filled.cursor,
        Some("Ü {A B} {{ title }} {{Title}} 2026-01-02".chars().count())
    );
    assert_eq!(
        fill_template(
            "no placeholders {{",
            "T",
            date(2026, 1, 2),
            time(0, 0, 0, 0)
        ),
        Filled {
            text: "no placeholders {{".to_owned(),
            cursor: None
        }
    );
}

/// TC-003-17 steps 1, 3 to 5: the templates of the folder and its folders,
/// the note in the template's format, the template unchanged.
#[test]
fn uc003_tc17_creates_a_note_from_a_template() {
    let sandbox = Sandbox::new(
        "tc17",
        &[
            ("Home.md", "# Home\n"),
            ("templates/Meeting.md", MEETING),
            ("templates/Spec.adoc", "= {{title}}\n\n{{cursor}}\n"),
            ("templates/work/Retro.md", "# {{title}}\n"),
            ("templatesX/Not.md", ""),
        ],
    );
    let mut workspace = sandbox.open();
    let templates = workspace.templates(None);
    let names: Vec<String> = templates.iter().map(Template::file_name).collect();
    assert_eq!(
        names,
        [
            "templates/Meeting.md",
            "templates/Spec.adoc",
            "templates/work/Retro.md"
        ]
    );

    let new = NewNote {
        folder: String::new(),
        title: "Standup".into(),
        format: NoteFormat::Markdown,
    };
    let choice = workspace.template_choice(None, Some("templates/Meeting"));
    let made = workspace
        .create_note_from(&new, &choice, nov11(9, 0), &heading)
        .unwrap();
    assert_eq!(made.id.as_str(), "Standup");
    assert_eq!(
        sandbox.read("Standup.md"),
        "# Standup\nDate: 2026-11-11 09:00\n\nOwner: {{owner}}\n"
    );
    assert!(made.cursor.is_some());
    assert!(matches!(made.template, TemplateUse::Used(_)));
    assert_eq!(sandbox.read("templates/Meeting.md"), MEETING);

    // Step 5: the note gets the format of the template, whatever was asked.
    let api = NewNote {
        title: "API".into(),
        ..new
    };
    let choice = workspace.template_choice(None, Some("templates/Spec"));
    workspace
        .create_note_from(&api, &choice, nov11(9, 0), &heading)
        .unwrap();
    assert_eq!(sandbox.read("API.adoc"), "= API\n\n");
    assert!(!sandbox.exists("API.md"));
}

/// TC-003-18: a template deleted after it was offered gives the note with
/// only its title, and says so.
#[test]
fn uc003_tc18_template_deleted_after_it_was_offered() {
    let sandbox = Sandbox::new("tc18", &[("templates/Meeting.md", MEETING)]);
    let mut workspace = sandbox.open();
    let choice = workspace.template_choice(None, Some("templates/Meeting"));
    fs::remove_file(sandbox.0.join("templates/Meeting.md")).unwrap();
    let new = NewNote {
        folder: String::new(),
        title: "Retro".into(),
        format: NoteFormat::Markdown,
    };
    let made = workspace
        .create_note_from(&new, &choice, nov11(9, 0), &heading)
        .unwrap();
    assert_eq!(sandbox.read("Retro.md"), "# Retro\n");
    assert_eq!(made.cursor, None);
    assert!(matches!(made.template, TemplateUse::Unreadable { .. }));

    // Chosen, then no template of the workspace any more (rescanned).
    workspace.rescan();
    let gone = workspace.template_choice(None, Some("templates/Meeting"));
    assert_eq!(gone, TemplateChoice::Gone("templates/Meeting".into()));
    let made = workspace
        .create_note_from(
            &NewNote {
                title: "Retro 2".into(),
                ..new
            },
            &gone,
            nov11(9, 0),
            &heading,
        )
        .unwrap();
    assert!(matches!(made.template, TemplateUse::Unreadable { .. }));
    assert_eq!(sandbox.read("Retro-2.md"), "# Retro 2\n");
}

/// UC-003 BR-008, OQ-217: a missing template folder, one outside the
/// workspace, a hidden one and the top folder hold no templates; another
/// folder named in the settings does.
#[test]
fn uc003_br008_template_folder_missing_outside_or_hidden() {
    let sandbox = Sandbox::new(
        "folders",
        &[("Home.md", ""), (".tpl/Hidden.md", ""), ("tpl/Day.md", "")],
    );
    let workspace = sandbox.open();
    assert!(workspace.templates(None).is_empty());
    assert!(workspace.templates(Some("../outside")).is_empty());
    assert!(workspace.templates(Some("/tmp")).is_empty());
    assert!(workspace.templates(Some(".tpl")).is_empty());
    assert!(workspace.templates(Some(".")).is_empty());
    assert_eq!(workspace.templates(Some(" tpl/ ")).len(), 1);
    // An empty setting is the built-in default.
    assert!(workspace.templates(Some("")).is_empty());
    assert_eq!(
        workspace.template_choice(Some("tpl"), Some("Home")),
        TemplateChoice::Gone("Home".into())
    );
}

/// TC-003-07, TC-003-09 with a template: a title that is not allowed and a
/// note that exists are refused before anything is written.
#[test]
fn uc003_a1_a2_with_a_template() {
    let sandbox = Sandbox::new(
        "a1a2",
        &[("templates/Meeting.md", MEETING), ("Standup.adoc", "")],
    );
    let mut workspace = sandbox.open();
    let choice = workspace.template_choice(None, Some("templates/Meeting"));
    let new = |title: &str| NewNote {
        folder: String::new(),
        title: title.into(),
        format: NoteFormat::Markdown,
    };
    assert!(matches!(
        workspace.create_note_from(&new("a:b"), &choice, nov11(9, 0), &heading),
        Err(NewNoteProblem::Title(_))
    ));
    assert!(matches!(
        workspace.create_note_from(&new("standup"), &choice, nov11(9, 0), &heading),
        Err(NewNoteProblem::Exists(_))
    ));
    assert!(!sandbox.exists("Standup.md"));
}

/// TC-025-01: the daily note from the template titled Daily, in its
/// format, the folder `daily` created, the cursor where the template asks.
#[test]
fn uc025_tc01_creates_the_daily_note_from_the_template() {
    let sandbox = Sandbox::new(
        "tc01",
        &[
            ("Home.md", "# Home\n"),
            (
                "templates/Daily.adoc",
                "= {{title}}\n\n{{cursor}}\n== Done\n",
            ),
        ],
    );
    let mut workspace = sandbox.open();
    let made = created(
        workspace
            .open_or_create_daily_note(&daily(nov11(8, 15)), &heading)
            .unwrap(),
    );
    assert_eq!(made.id.as_str(), "daily/2026-11-11");
    assert_eq!(
        sandbox.read("daily/2026-11-11.adoc"),
        "= 2026-11-11\n\n== Done\n"
    );
    // The cursor stands on the empty line above "Done" (OQ-275).
    assert_eq!(made.cursor, Some("= 2026-11-11\n".chars().count()));
    assert!(workspace.listing().notes.iter().any(|e| e.id == made.id));
}

/// TC-025-02: without a template, only the title, in the default format.
#[test]
fn uc025_tc02_without_a_template() {
    let sandbox = Sandbox::new("tc02", &[("Home.md", "# Home\n")]);
    let mut workspace = sandbox.open();
    let made = created(
        workspace
            .open_or_create_daily_note(&daily(nov11(8, 15)), &heading)
            .unwrap(),
    );
    assert_eq!(sandbox.read("daily/2026-11-11.md"), "# 2026-11-11\n");
    assert_eq!(made.cursor, None);
    assert_eq!(made.template, TemplateUse::None);
}

/// TC-025-03, BR-006: an existing daily note, in either format and
/// whatever was asked, is opened and never replaced.
#[test]
fn uc025_tc03_existing_daily_note_is_not_replaced() {
    let sandbox = Sandbox::new(
        "tc03",
        &[
            ("daily/2026-11-11.adoc", "= 2026-11-11\n\ntoday\n"),
            ("templates/Daily.md", "# {{title}}\n"),
        ],
    );
    let mut workspace = sandbox.open();
    for _ in 0..2 {
        let note = workspace
            .open_or_create_daily_note(&daily(nov11(8, 15)), &heading)
            .unwrap();
        assert_eq!(
            note,
            DailyNote::Exists(NoteId::from_relative("daily/2026-11-11"))
        );
    }
    assert_eq!(
        sandbox.read("daily/2026-11-11.adoc"),
        "= 2026-11-11\n\ntoday\n"
    );
    assert!(!sandbox.exists("daily/2026-11-11.md"));
}

/// TC-025-04: the nearest earlier and later daily note; none for the days
/// between, none past the ends, and only for a daily note.
#[test]
fn uc025_tc04_daily_note_of_another_day() {
    let sandbox = Sandbox::new(
        "tc04",
        &[
            ("daily/2026-11-05.md", ""),
            ("daily/2026-11-11.md", ""),
            ("daily/notes.md", ""),
            ("daily/2026-13-01.md", ""),
            ("daily/sub/2026-11-08.md", ""),
            ("2026-11-09.md", ""),
        ],
    );
    let workspace = sandbox.open();
    let id = NoteId::from_relative;
    let dates: Vec<String> = workspace
        .daily_notes(None)
        .iter()
        .map(|(date, _)| date_text(*date))
        .collect();
    assert_eq!(dates, ["2026-11-05", "2026-11-11"]);
    assert_eq!(
        workspace.daily_note_beside(None, &id("daily/2026-11-11"), false),
        Ok(Some(id("daily/2026-11-05")))
    );
    assert_eq!(
        workspace.daily_note_beside(None, &id("daily/2026-11-05"), false),
        Ok(None)
    );
    assert_eq!(
        workspace.daily_note_beside(None, &id("daily/2026-11-05"), true),
        Ok(Some(id("daily/2026-11-11")))
    );
    assert_eq!(
        workspace.daily_note_beside(None, &id("daily/2026-11-11"), true),
        Ok(None)
    );
    assert_eq!(
        workspace.daily_note_beside(None, &id("daily/notes"), true),
        Err(NotDaily)
    );
    assert_eq!(
        workspace.daily_note_beside(None, &id("2026-11-09"), true),
        Err(NotDaily)
    );
    assert_eq!(
        workspace.daily_note_beside(Some("."), &id("2026-11-09"), true),
        Ok(None)
    );
    assert!(sandbox.exists("daily/2026-11-05.md"));
    assert_eq!(fs::read_dir(sandbox.0.join("daily")).unwrap().count(), 5);
}

/// TC-025-05: a template the settings name that does not exist gives the
/// title only and says so.
#[test]
fn uc025_tc05_template_not_found() {
    let sandbox = Sandbox::new("tc05", &[("templates/Daily.md", "# {{title}}\nx\n")]);
    let mut workspace = sandbox.open();
    let request = DailyRequest {
        template: Some("templates/Journal-Day.md"),
        ..daily(nov11(8, 15))
    };
    let made = created(
        workspace
            .open_or_create_daily_note(&request, &heading)
            .unwrap(),
    );
    assert_eq!(
        made.template,
        TemplateUse::NotFound("templates/Journal-Day.md".into())
    );
    assert_eq!(sandbox.read("daily/2026-11-11.md"), "# 2026-11-11\n");
}

/// TC-025-06, BR-002: a hidden folder, one outside the workspace, a
/// workspace of its own and a name not allowed create nothing.
#[test]
fn uc025_tc06_daily_note_folder_not_allowed() {
    let sandbox = Sandbox::new("tc06", &[("Home.md", ""), ("nested/underline.yaml", "")]);
    let mut workspace = sandbox.open();
    for (folder, expected) in [
        (".days", "hidden"),
        ("../days", "outside"),
        ("/days", "outside"),
        ("nested", "own"),
        ("a:b", "name"),
    ] {
        let request = DailyRequest {
            folder: Some(folder),
            ..daily(nov11(8, 15))
        };
        let problem = workspace
            .open_or_create_daily_note(&request, &heading)
            .unwrap_err();
        let matched = match expected {
            "hidden" => matches!(problem, NewNoteProblem::Hidden(_)),
            "outside" => matches!(problem, NewNoteProblem::Outside),
            "own" => matches!(problem, NewNoteProblem::OwnWorkspace(_)),
            _ => matches!(problem, NewNoteProblem::FolderName { .. }),
        };
        assert!(matched, "{folder}: {problem:?}");
    }
    assert!(!sandbox.exists(".days"));
    assert!(!sandbox.0.parent().unwrap().join("days").exists() || cfg!(windows));
}

/// TC-025-07, NFR-006: a folder that cannot be created leaves nothing
/// behind.
#[cfg(unix)]
#[test]
fn uc025_tc07_daily_note_cannot_be_created() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("tc07", &[("Home.md", "")]);
    let mut workspace = sandbox.open();
    let locked = sandbox.0.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    let request = DailyRequest {
        folder: Some("locked/days/deep"),
        ..daily(nov11(8, 15))
    };
    let result = workspace.open_or_create_daily_note(&request, &heading);
    let writable = fs::write(locked.join("probe"), "").is_ok();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        // Running as root: permissions do not hold.
        return;
    }
    assert!(
        matches!(result, Err(NewNoteProblem::Io { .. })),
        "{result:?}"
    );
    assert!(!locked.join("days").exists());
}

/// TC-025-08: folder and template from the settings, `{{date}}` and
/// `{{time}}` filled in.
#[test]
fn uc025_tc08_folder_and_template_from_the_settings() {
    let sandbox = Sandbox::new(
        "tc08",
        &[("templates/Day.md", "# {{date}}\nWritten at {{time}}\n")],
    );
    let mut workspace = sandbox.open();
    let request = DailyRequest {
        folder: Some("journal/days"),
        template: Some("templates/Day.md"),
        ..daily(nov11(8, 15))
    };
    let made = created(
        workspace
            .open_or_create_daily_note(&request, &heading)
            .unwrap(),
    );
    assert_eq!(made.id.as_str(), "journal/days/2026-11-11");
    assert_eq!(
        sandbox.read("journal/days/2026-11-11.md"),
        "# 2026-11-11\nWritten at 08:15\n"
    );
    // The setting may leave the extension out, and names any listed note.
    assert!(matches!(
        workspace.daily_template(None, Some("templates/Day")),
        DailyTemplate::Found(_)
    ));
    assert!(matches!(
        workspace.daily_template(None, Some("templates/Day.adoc")),
        DailyTemplate::NotFound(_)
    ));
    assert!(matches!(
        workspace.daily_template(None, Some("../Day.md")),
        DailyTemplate::NotFound(_)
    ));
}

/// TC-025-09, BR-005: a daily note renamed out of its date is no daily
/// note; asking again creates a new one.
#[test]
fn uc025_tc09_renamed_daily_note_is_no_daily_note() {
    let sandbox = Sandbox::new("tc09", &[("daily/Kickoff.md", "# 2026-11-11\n")]);
    let mut workspace = sandbox.open();
    created(
        workspace
            .open_or_create_daily_note(&daily(nov11(8, 15)), &heading)
            .unwrap(),
    );
    assert_eq!(sandbox.read("daily/Kickoff.md"), "# 2026-11-11\n");
    assert_eq!(sandbox.read("daily/2026-11-11.md"), "# 2026-11-11\n");
}

/// TC-025-10, BR-001: the day changes at midnight of the local clock.
#[test]
fn uc025_tc10_the_day_changes_at_midnight() {
    let sandbox = Sandbox::new("tc10", &[("daily/2026-11-11.md", "# 2026-11-11\n")]);
    let mut workspace = sandbox.open();
    let before = workspace
        .open_or_create_daily_note(&daily(nov11(23, 59)), &heading)
        .unwrap();
    assert_eq!(
        before,
        DailyNote::Exists(NoteId::from_relative("daily/2026-11-11"))
    );
    let after = created(
        workspace
            .open_or_create_daily_note(&daily(date(2026, 11, 12).at(0, 1, 0, 0)), &heading)
            .unwrap(),
    );
    assert_eq!(after.id.as_str(), "daily/2026-11-12");
    assert_eq!(sandbox.read("daily/2026-11-12.md"), "# 2026-11-12\n");
}

/// UC-025 BR-001: only `YYYY-MM-DD` of a real date is a daily note title.
#[test]
fn uc025_br001_daily_titles() {
    assert_eq!(daily_date("2026-11-11"), Some(date(2026, 11, 11)));
    assert_eq!(daily_date("2028-02-29"), Some(date(2028, 2, 29)));
    for title in [
        "2026-02-30",
        "2026-1-11",
        "2026-11-11x",
        "2026 11 11",
        "x026-11-11",
        "",
    ] {
        assert_eq!(daily_date(title), None, "{title}");
    }
    assert_eq!(date_text(date(987, 3, 4)), "0987-03-04");
    assert_eq!(time_text(time(7, 5, 59, 0)), "07:05");
}

/// UC-025 BR-003: the template titled Daily nearest the top of the template
/// folder, in any case; a folder setting moves it.
#[test]
fn uc025_br003_the_template_titled_daily() {
    let sandbox = Sandbox::new(
        "br003",
        &[
            ("templates/a/Daily.md", ""),
            ("templates/daily.adoc", ""),
            ("tpl/Daily.md", ""),
        ],
    );
    let workspace = sandbox.open();
    match workspace.daily_template(None, None) {
        DailyTemplate::Found(template) => {
            assert_eq!(template.file_name(), "templates/daily.adoc");
        }
        other => panic!("{other:?}"),
    }
    match workspace.daily_template(Some("tpl"), Some("  ")) {
        DailyTemplate::Found(template) => assert_eq!(template.file_name(), "tpl/Daily.md"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        workspace.daily_template(Some("missing"), None),
        DailyTemplate::None
    );
}

/// UC-025 A4, BR-002 (monkey): a daily note folder that is a link to a place
/// outside the workspace, directly or as a level of the path, must not make
/// the note be written outside the workspace.
#[cfg(unix)]
#[test]
fn uc025_a4_daily_folder_link_outside_the_workspace_is_refused() {
    for (name, setting) in [("top", None), ("nested", Some("link/sub"))] {
        let outside = std::env::temp_dir().join(format!(
            "underline-monkey-outside-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&outside);
        fs::create_dir_all(&outside).unwrap();
        let sandbox = Sandbox::new(&format!("link-{name}"), &[("Home.md", "# Home\n")]);
        let link = if setting.is_none() { "daily" } else { "link" };
        std::os::unix::fs::symlink(&outside, sandbox.0.join(link)).unwrap();
        let mut workspace = sandbox.open();

        let result = workspace.open_or_create_daily_note(
            &DailyRequest {
                folder: setting,
                template_folder: None,
                template: None,
                now: nov11(10, 0),
            },
            &heading,
        );

        let wrote: Vec<_> = walk_files(&outside);
        let _ = fs::remove_dir_all(&outside);
        assert!(
            wrote.is_empty(),
            "{name}: wrote {wrote:?}, result {result:?}"
        );
        assert!(result.is_err(), "{name}: {result:?}");
    }
}

#[cfg(unix)]
fn walk_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk_files(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// UC-025 A5 (monkey): a daily note folder that is a file cannot take the
/// note; nothing is written and the file stays.
#[test]
fn uc025_a5_daily_folder_that_is_a_file() {
    let sandbox = Sandbox::new("file-folder", &[("daily", "not a folder")]);
    let mut workspace = sandbox.open();
    let result = workspace.open_or_create_daily_note(&daily(nov11(8, 15)), &heading);
    assert!(
        matches!(result, Err(NewNoteProblem::Io { .. })),
        "{result:?}"
    );
    assert_eq!(sandbox.read("daily"), "not a folder");
}

/// UC-003 BR-008 (monkey): for any text, a template without placeholders
/// stays as it is, and the cursor of a filled one lies within the text.
#[test]
fn uc003_br008_fill_keeps_plain_text_and_the_cursor_within() {
    let pieces = [
        "",
        "a",
        "{",
        "}",
        "{{",
        "}}",
        "{{x}}",
        "\n",
        "\r\n",
        "Ü",
        "😀",
        "{{cursor}}",
        "{{title}}",
        "{{date}}",
        "{{ time }}",
    ];
    for a in pieces {
        for b in pieces {
            for c in pieces {
                let template = format!("{a}{b}{c}");
                let filled = fill_template(&template, "T", date(2026, 1, 2), time(3, 4, 0, 0));
                if !template.contains("{{title}}")
                    && !template.contains("{{date}}")
                    && !template.contains("{{cursor}}")
                {
                    assert_eq!(filled.text, template, "{template:?}");
                }
                if let Some(cursor) = filled.cursor {
                    assert!(cursor <= filled.text.chars().count(), "{template:?}");
                }
                assert!(!filled.text.contains("{{cursor}}"), "{template:?}");
            }
        }
    }
}

/// OQ-275: a line of only `{{cursor}}` after an empty line takes its
/// place; after a line with text, and with CRLF, it stays a line.
#[test]
fn uc003_oq275_cursor_line_after_an_empty_line() {
    let at = |text: &str| fill_template(text, "T", date(2026, 1, 2), time(3, 4, 0, 0));
    let joined = at("# T\r\n\r\n{{cursor}}\r\nEnd\r\n");
    assert_eq!(joined.text, "# T\r\n\r\nEnd\r\n");
    assert_eq!(joined.cursor, Some(5));
    let kept = at("# T\nText\n{{cursor}}\nEnd\n");
    assert_eq!(kept.text, "# T\nText\n\nEnd\n");
    assert_eq!(kept.cursor, Some(9));
    let first = at("{{cursor}}\n");
    assert_eq!((first.text.as_str(), first.cursor), ("\n", Some(0)));
}
