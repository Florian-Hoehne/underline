//! Tests of exporting a note (UC-009, inventory M-15, M-04.8), against what
//! the egui app does (`start_export`, `ask_export_place`, `show_export`,
//! `run_export`, `take_export` of `apps/underline-desktop/src/app.rs`). The
//! exports go to temporary folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::{ExportJob, FILE_EXISTS, NOT_ONTO_A_NOTE, NO_PATH};
use crate::export_view::ExportOfferView;
use crate::export_view::{
    DialogOutcome, ExportAnswer, ExportKindName, ExportPlaceAnswer, TypedExportPath,
};
use crate::workspace_view::FormatName;
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-export-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files`; the
/// first note open in the active tab.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let root = dir.join("notes");
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

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// Chooses `path` in the dialog for `note` and `kind`, then runs the
/// export as the commands do: prepared under the session, made without it,
/// taken in after.
fn export_to(session: &mut Session, note: &str, kind: ExportKindName, path: &Path) {
    let answer =
        session.export_dialog_answer(&id(note), kind, DialogOutcome::Chosen(path.to_owned()));
    assert!(
        matches!(answer, ExportPlaceAnswer::Chosen { .. }),
        "{answer:?}"
    );
    let job = session.export_prepare(&id(note), kind, None).unwrap();
    assert_eq!(session.status(), format!("Exporting {}…", id(note).title()));
    session.finish_export(job.run());
}

/// The last status the window was told, if any.
fn last_status(session: &mut Session) -> Option<crate::StatusView> {
    session
        .take_events()
        .into_iter()
        .rev()
        .find_map(|event| match event {
            Event::Status(view) => Some(view),
            _ => None,
        })
}

/// The offers of the last status the window was told.
fn last_offers(session: &mut Session) -> Vec<ExportOfferView> {
    last_status(session).map(|s| s.exports).unwrap_or_default()
}

fn offer(title: &str, open: bool, show: bool) -> ExportOfferView {
    ExportOfferView {
        title: title.into(),
        open,
        show,
    }
}

#[test]
fn uc009_m15_1_export_start_names_the_kinds_of_the_notes_format() {
    let dir = temp_dir("start");
    let (mut session, _) = session_with(
        &dir,
        &[("Plan.md", "# Plan\n"), ("Guide.adoc", "= Guide\n")],
    );

    let start = session.export_start(&id("Plan")).unwrap();
    assert_eq!(start.title, "Plan");
    let labels: Vec<_> = start.choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["AsciiDoc", "HTML", "PDF"]);
    assert_eq!(start.choices[0].kind, ExportKindName::OtherFormat);

    let start = session.export_start(&id("Guide")).unwrap();
    let labels: Vec<_> = start.choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Markdown", "HTML", "PDF"]);
}

#[test]
fn uc009_m05_4_the_overview_names_each_notes_format() {
    let dir = temp_dir("overview-format");
    let (session, _) = session_with(
        &dir,
        &[("Plan.md", "# Plan\n"), ("Guide.adoc", "= Guide\n")],
    );

    let overview = session.snapshot().overview;
    let format = |note: &str| overview.iter().find(|i| i.note == note).unwrap().format;
    assert_eq!(format("Plan"), FormatName::Markdown);
    assert_eq!(format("Guide"), FormatName::AsciiDoc);
    let kinds = session.start_info().export_kinds;
    assert_eq!(kinds.ascii_doc[0].label, "Markdown");
    assert_eq!(kinds.markdown[0].label, "AsciiDoc");
}

#[test]
fn uc009_m15_1_without_a_workspace_nothing_is_exported() {
    let dir = temp_dir("no-workspace");
    let mut session = Session::new(
        None,
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(None, Instant::now()),
    );

    assert!(session.export_start(&id("Plan")).is_none());
    assert!(session
        .export_dialog(&id("Plan"), ExportKindName::Html)
        .is_none());
    assert_eq!(
        session
            .export_prepare(
                &id("Plan"),
                ExportKindName::Html,
                Some(TypedExportPath {
                    path: dir.join("x.html").display().to_string(),
                    replace: None,
                }),
            )
            .unwrap_err(),
        ExportAnswer::Done
    );
}

#[test]
fn uc009_m15_2_the_dialog_proposes_the_name_and_the_last_folder() {
    let dir = temp_dir("dialog");
    let (mut session, _) = session_with(&dir, &[("guide/Getting-Started.md", "# Start\n")]);
    let note = id("guide/Getting-Started");

    let plan = session
        .export_dialog(&note, ExportKindName::OtherFormat)
        .unwrap();
    assert_eq!(plan.title, "Export as AsciiDoc");
    assert_eq!(plan.file_name, "Getting-Started.adoc");
    assert_eq!(
        (plan.filter.as_str(), plan.extension.as_str()),
        ("AsciiDoc", "adoc")
    );
    // The first time: the home folder.
    let home = super::home_folder().unwrap_or_else(|| PathBuf::from("."));
    assert_eq!(plan.folder, home);

    let out = dir.join("out");
    fs::create_dir_all(&out).unwrap();
    export_to(
        &mut session,
        note.as_str(),
        ExportKindName::OtherFormat,
        &out.join("Getting-Started.adoc"),
    );

    // TC-009-01 step 4: the folder of the last export.
    let plan = session.export_dialog(&note, ExportKindName::Pdf).unwrap();
    assert_eq!(plan.folder, out);
    assert_eq!(plan.file_name, "Getting-Started.pdf");
    assert_eq!(plan.title, "Export as PDF");
}

#[test]
fn uc009_a1_m15_1_a_cancelled_dialog_writes_nothing() {
    let dir = temp_dir("cancel");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let before = session.status().to_owned();

    let answer =
        session.export_dialog_answer(&id("Plan"), ExportKindName::Pdf, DialogOutcome::Cancelled);

    assert_eq!(answer, ExportPlaceAnswer::Done);
    // Nothing was chosen, so a run does nothing.
    assert_eq!(
        session
            .export_prepare(&id("Plan"), ExportKindName::Pdf, None)
            .unwrap_err(),
        ExportAnswer::Done
    );
    assert_eq!(session.status(), before);
}

#[test]
fn uc009_m15_2_adr016_without_a_dialog_the_path_is_typed() {
    let dir = temp_dir("typed");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);

    let answer = session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::Html,
        DialogOutcome::Unavailable,
    );

    let home = super::home_folder().unwrap_or_else(|| PathBuf::from("."));
    assert_eq!(
        answer,
        ExportPlaceAnswer::Typed {
            heading: "Export Plan as HTML to:".into(),
            path: home.join("Plan.html").display().to_string(),
        }
    );
}

#[test]
fn uc009_d03_m15_2_replacing_a_typed_file_holds_for_that_path_only() {
    let dir = temp_dir("replace");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n\nNew text.\n")]);
    let first = dir.join("Plan.html");
    let second = dir.join("Other.html");
    fs::write(&first, "old").unwrap();
    fs::write(&second, "other").unwrap();
    let typed = |path: &Path, replace: Option<&Path>| TypedExportPath {
        path: path.display().to_string(),
        replace: replace.map(|p| p.display().to_string()),
    };

    let answer = session
        .export_prepare(&id("Plan"), ExportKindName::Html, Some(typed(&first, None)))
        .unwrap_err();
    assert_eq!(
        answer,
        ExportAnswer::Exists {
            problem: FILE_EXISTS.into(),
            path: first.display().to_string(),
        }
    );
    // D-03: the agreement for the first path does not hold for another.
    let answer = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(typed(&second, Some(&first))),
        )
        .unwrap_err();
    assert!(matches!(answer, ExportAnswer::Exists { .. }), "{answer:?}");
    assert_eq!(fs::read_to_string(&second).unwrap(), "other");

    let job = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(typed(&first, Some(&first))),
        )
        .unwrap();
    session.finish_export(job.run());

    assert!(fs::read_to_string(&first).unwrap().contains("New text."));
}

#[test]
fn uc009_a5_m15_3_a_note_is_not_replaced_by_an_export() {
    let dir = temp_dir("onto-note");
    let (mut session, root) = session_with(
        &dir,
        &[("Plan.adoc", "= Plan\n"), ("Ideas.md", "# Ideas\n")],
    );
    session.open_note(&id("Ideas"));

    // TC-009-12: another note open in a tab.
    let answer = session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::OtherFormat,
        DialogOutcome::Chosen(root.join("Ideas.md")),
    );
    assert!(matches!(answer, ExportPlaceAnswer::Chosen { .. }));
    let answer = session
        .export_prepare(&id("Plan"), ExportKindName::OtherFormat, None)
        .unwrap_err();

    assert_eq!(answer, ExportAnswer::AskAgain);
    assert_eq!(session.status(), NOT_ONTO_A_NOTE);
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "# Ideas\n"
    );

    // The note itself, typed and agreed to replace.
    let path = root.join("Plan.adoc").display().to_string();
    let answer = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(TypedExportPath {
                path: path.clone(),
                replace: Some(path),
            }),
        )
        .unwrap_err();
    // UX review F11: the field says why.
    assert_eq!(
        answer,
        ExportAnswer::Problem {
            problem: NOT_ONTO_A_NOTE.into()
        }
    );
    assert_eq!(
        fs::read_to_string(root.join("Plan.adoc")).unwrap(),
        "= Plan\n"
    );
}

#[test]
fn uc009_m15_1_a_chosen_place_counts_for_its_note_and_kind_only() {
    let dir = temp_dir("chosen-other");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::Pdf,
        DialogOutcome::Chosen(dir.join("Plan.pdf")),
    );

    assert_eq!(
        session
            .export_prepare(&id("Plan"), ExportKindName::Html, None)
            .unwrap_err(),
        ExportAnswer::Done
    );
    assert!(!dir.join("Plan.pdf").exists());
}

#[test]
fn uc009_br001_m15_4_the_tab_text_with_unsaved_changes_is_exported() {
    let dir = temp_dir("unsaved");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    session.window_type(&id("Plan"), 7, "new line\n");
    let target = dir.join("Plan.html");

    export_to(&mut session, "Plan", ExportKindName::Html, &target);

    let html = fs::read_to_string(&target).unwrap();
    assert!(html.contains("new line"), "{html}");
    assert!(html.contains("<title>Plan</title>"), "{html}");
    // The note itself is unchanged (not saved by the export).
    assert_eq!(
        fs::read_to_string(root.join("Plan.md")).unwrap(),
        "# Plan\n"
    );
}

#[test]
fn uc009_br001_a_note_without_a_tab_is_exported_as_stored() {
    let dir = temp_dir("stored");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Plan.md", "# Plan\n"),
            ("Trip.md", "# Trip\n\nBy ferry.\n"),
        ],
    );
    let target = dir.join("Trip.adoc");

    export_to(&mut session, "Trip", ExportKindName::OtherFormat, &target);

    let adoc = fs::read_to_string(&target).unwrap();
    assert!(adoc.starts_with("= Trip"), "{adoc}");
    assert!(adoc.contains("By ferry."), "{adoc}");
    assert_eq!(
        session.status(),
        format!("Exported Trip to {}", target.display())
    );
    assert_eq!(last_offers(&mut session), [offer("Trip", true, false)]);
    assert_eq!(session.export_target(0), Some(target.display().to_string()));
}

#[test]
fn uc009_a2_m15_5_m15_6_approximated_blocks_are_reported_with_their_lines() {
    let dir = temp_dir("approximated");
    let (mut session, _) = session_with(
        &dir,
        &[("Guide.adoc", "= Guide\n\nIntro.\n\nNOTE: Mind the gap.\n")],
    );
    let target = dir.join("Guide.md");

    export_to(&mut session, "Guide", ExportKindName::OtherFormat, &target);

    assert_eq!(
        session.status(),
        format!(
            "Exported Guide to {}; 1 block only approximated",
            target.display()
        )
    );
    assert_eq!(last_offers(&mut session), [offer("Guide", true, true)]);
    let shown = session.export_approximated(0).unwrap();
    assert_eq!(
        (shown.title.as_str(), shown.lines.as_slice()),
        ("Guide", &[5][..])
    );
    let markdown = fs::read_to_string(&target).unwrap();
    assert!(markdown.contains("**Note:**"), "{markdown}");
}

#[test]
fn uc009_a3_m15_5_missing_images_are_named() {
    let dir = temp_dir("missing");
    let (mut session, _) = session_with(
        &dir,
        &[("Broken.md", "# Broken\n\n![Plan](img/missing.png)\n")],
    );
    let target = dir.join("Broken.html");

    export_to(&mut session, "Broken", ExportKindName::Html, &target);

    assert!(
        session.status().contains("; missing: "),
        "{}",
        session.status()
    );
    assert!(fs::read_to_string(&target).unwrap().contains("Plan"));
}

#[test]
fn uc009_a4_m15_5_an_export_that_cannot_be_written_leaves_nothing() {
    let dir = temp_dir("unwritable");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let target = dir.join("missing-folder").join("Plan.html");

    export_to(&mut session, "Plan", ExportKindName::Html, &target);

    assert!(
        session.status().starts_with("Plan could not be exported: "),
        "{}",
        session.status()
    );
    assert!(!target.exists());
    let status = last_status(&mut session).unwrap();
    assert!(status.error, "A4 is marked as an error");
    assert_eq!(status.exports, [offer("Plan", false, false)]);
    assert!(!status.busy);
    assert_eq!(session.export_target(0), None);
}

#[test]
fn uc009_a6_an_export_into_the_workspace_is_a_new_note() {
    let dir = temp_dir("into-workspace");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    fs::create_dir_all(root.join("archive")).unwrap();

    export_to(
        &mut session,
        "Plan",
        ExportKindName::OtherFormat,
        &root.join("archive").join("Plan.adoc"),
    );

    let overview = session.snapshot().overview;
    assert!(
        overview.iter().any(|item| item.note == "archive/Plan"),
        "{overview:?}"
    );
    assert_eq!(
        fs::read_to_string(root.join("Plan.md")).unwrap(),
        "# Plan\n"
    );
}

#[test]
fn uc009_br004_m15_4_a_pdf_is_set() {
    let dir = temp_dir("pdf");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n\nGrüße, 日本語.\n")]);
    let target = dir.join("Plan.pdf");

    export_to(&mut session, "Plan", ExportKindName::Pdf, &target);

    let bytes = fs::read(&target).unwrap();
    assert!(bytes.starts_with(b"%PDF"), "{}", session.status());
}

#[test]
fn uc009_m04_8_the_offers_end_with_another_report() {
    let dir = temp_dir("offer-ends");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    export_to(
        &mut session,
        "Plan",
        ExportKindName::Html,
        &dir.join("Plan.html"),
    );
    assert_eq!(last_offers(&mut session), [offer("Plan", true, false)]);

    session.check_wiki("owner/repo");

    assert!(last_offers(&mut session).is_empty());
    assert_eq!(session.export_target(0), None);
    assert!(session.export_approximated(0).is_none());
}

#[test]
fn uc009_d04_saved_does_not_take_the_place_of_an_export_report() {
    let dir = temp_dir("saved-keeps");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    export_to(
        &mut session,
        "Plan",
        ExportKindName::Html,
        &dir.join("Plan.html"),
    );
    let report = session.status().to_owned();

    // The autosave stores a change typed after the export.
    session.window_type(&id("Plan"), 7, "more\n");
    let start = Instant::now();
    while fs::read_to_string(root.join("Plan.md")).unwrap() == "# Plan\n"
        && start.elapsed() < Duration::from_secs(5)
    {
        session.tick(Instant::now() + Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(20));
    }
    session.tick(Instant::now() + Duration::from_secs(3));

    assert_eq!(
        fs::read_to_string(root.join("Plan.md")).unwrap(),
        "# Plan\nmore\n"
    );
    assert_eq!(session.status(), report);
    assert_eq!(
        session.export_target(0),
        Some(dir.join("Plan.html").display().to_string())
    );
}

#[test]
fn uc009_m15_1_a_place_chosen_before_another_workspace_opened_is_not_used() {
    let dir = temp_dir("chosen-workspace");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::Html,
        DialogOutcome::Chosen(dir.join("Plan.html")),
    );
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Plan.md"), "# Other plan\n").unwrap();
    assert!(matches!(
        session.open_chosen(&other, false),
        crate::OpenAnswer::Done
    ));

    assert_eq!(
        session
            .export_prepare(&id("Plan"), ExportKindName::Html, None)
            .unwrap_err(),
        ExportAnswer::Done
    );
    assert!(!dir.join("Plan.html").exists());
}

/// Types `path` into the field, agreeing to replace it.
fn typed(path: &Path) -> TypedExportPath {
    TypedExportPath {
        path: path.display().to_string(),
        replace: Some(path.display().to_string()),
    }
}

fn not_onto_a_note() -> ExportAnswer {
    ExportAnswer::Problem {
        problem: NOT_ONTO_A_NOTE.into(),
    }
}

#[test]
fn uc009_a5_a116_no_file_of_the_workspace_is_replaced_by_an_export() {
    let dir = temp_dir("workspace-files");
    let (mut session, root) = session_with(
        &dir,
        &[
            ("Plan.md", "# Plan\n"),
            ("guide/Closed.md", "# Closed\n"),
            (".todos/1.yaml", "title: a\n"),
            ("underline.yaml", "theme: Nord\n"),
        ],
    );
    let refused = |session: &mut Session, path: &Path| {
        session
            .export_prepare(&id("Plan"), ExportKindName::Html, Some(typed(path)))
            .unwrap_err()
    };

    // A note of the overview without a tab.
    assert_eq!(
        refused(&mut session, &root.join("guide/Closed.md")),
        not_onto_a_note()
    );
    assert_eq!(
        refused(&mut session, &root.join("underline.yaml")),
        not_onto_a_note()
    );
    assert_eq!(
        refused(&mut session, &root.join(".todos/1.yaml")),
        not_onto_a_note()
    );
    // A new file in a dot folder is refused too.
    assert_eq!(
        refused(&mut session, &root.join(".git-new.html")),
        not_onto_a_note()
    );
    assert_eq!(
        fs::read_to_string(root.join("underline.yaml")).unwrap(),
        "theme: Nord\n"
    );
    // Through a symbolic link to the workspace's folder.
    #[cfg(unix)]
    {
        let link = dir.join("link");
        std::os::unix::fs::symlink(root.join("guide"), &link).unwrap();
        assert_eq!(
            refused(&mut session, &link.join("Closed.md")),
            not_onto_a_note()
        );
        let file_link = dir.join("closed-link.md");
        std::os::unix::fs::symlink(root.join("guide/Closed.md"), &file_link).unwrap();
        assert_eq!(refused(&mut session, &file_link), not_onto_a_note());
    }
    assert_eq!(
        fs::read_to_string(root.join("guide/Closed.md")).unwrap(),
        "# Closed\n"
    );

    // Chosen in the dialog: the place is asked again.
    session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::Html,
        DialogOutcome::Chosen(root.join("guide/Closed.md")),
    );
    assert_eq!(
        session
            .export_prepare(&id("Plan"), ExportKindName::Html, None)
            .unwrap_err(),
        ExportAnswer::AskAgain
    );
}

#[test]
fn uc009_a117_an_empty_path_or_a_folder_is_refused_up_front() {
    let dir = temp_dir("typed-checks");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let answer = |session: &mut Session, path: &str| {
        session
            .export_prepare(
                &id("Plan"),
                ExportKindName::Html,
                Some(TypedExportPath {
                    path: path.into(),
                    replace: None,
                }),
            )
            .unwrap_err()
    };

    let empty = ExportAnswer::Problem {
        problem: NO_PATH.into(),
    };
    assert_eq!(answer(&mut session, ""), empty);
    assert_eq!(answer(&mut session, "   "), empty);
    let folder = dir.display().to_string();
    assert_eq!(
        answer(&mut session, &folder),
        ExportAnswer::Problem {
            problem: format!("{folder} is a folder. Enter a file name in it."),
        }
    );
    // A relative path counts from the working directory: `src` of this crate.
    assert_eq!(
        answer(&mut session, "src"),
        ExportAnswer::Problem {
            problem: "src is a folder. Enter a file name in it.".into(),
        }
    );
}

#[test]
fn uc009_d03_spaces_around_the_typed_path_do_not_matter() {
    let dir = temp_dir("trimmed");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let target = dir.join("Plan.html");
    fs::write(&target, "old").unwrap();
    let path = target.display().to_string();

    let answer = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(TypedExportPath {
                path: format!(" {path} "),
                replace: None,
            }),
        )
        .unwrap_err();
    assert_eq!(
        answer,
        ExportAnswer::Exists {
            problem: FILE_EXISTS.into(),
            path: path.clone(),
        }
    );
    let job = session.export_prepare(
        &id("Plan"),
        ExportKindName::Html,
        Some(TypedExportPath {
            path: format!("{path}  "),
            replace: Some(path),
        }),
    );
    assert!(job.is_ok());
}

#[test]
fn uc009_a118_a_panic_in_the_export_is_a_failure() {
    let dir = temp_dir("panic");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.export_dialog_answer(
        &id("Plan"),
        ExportKindName::Pdf,
        DialogOutcome::Chosen(dir.join("Plan.pdf")),
    );
    let job: ExportJob = session
        .export_prepare(&id("Plan"), ExportKindName::Pdf, None)
        .unwrap();
    assert!(last_status(&mut session).unwrap().busy);

    let outcome = job.run_with(|_, _| panic!("Typst fell over"));
    let failure = session.finish_export(outcome);

    assert_eq!(
        session.status(),
        "Plan could not be exported: the export stopped unexpectedly"
    );
    assert_eq!(failure.as_deref(), Some(session.status()));
    let status = last_status(&mut session).unwrap();
    assert!(!status.busy && status.error);
    assert!(!dir.join("Plan.pdf").exists());
}

#[test]
fn uc009_a118_a_job_that_could_not_run_ends_too() {
    let dir = temp_dir("aborted");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let job = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(typed(&dir.join("P.html"))),
        )
        .unwrap();

    session.finish_export(ExportJob::aborted(job.id(), job.title().to_owned(), "gone"));

    assert_eq!(session.status(), "Plan could not be exported: gone");
    assert!(!last_status(&mut session).unwrap().busy);
}

#[test]
fn uc009_a121_one_dialog_at_a_time_and_its_status() {
    let dir = temp_dir("single-dialog");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n"), ("Trip.md", "# Trip\n")]);
    session.check_wiki("owner/repo");
    let before = session.status().to_owned();

    assert!(session
        .export_dialog(&id("Plan"), ExportKindName::Pdf)
        .is_some());
    assert_eq!(session.status(), "Choosing where to save Plan…");
    // A second one while it is open: nothing.
    assert!(session
        .export_dialog(&id("Trip"), ExportKindName::Html)
        .is_none());
    assert_eq!(session.status(), "Choosing where to save Plan…");

    session.export_dialog_answer(&id("Plan"), ExportKindName::Pdf, DialogOutcome::Cancelled);
    assert_eq!(session.status(), before);
    assert!(session
        .export_dialog(&id("Trip"), ExportKindName::Html)
        .is_some());
}

#[test]
fn uc009_a119_open_only_for_the_kinds_extension() {
    let dir = temp_dir("extension");
    let (mut session, _) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    let target = dir.join("Plan.txt");

    // egui writes to the name as typed.
    let job = session
        .export_prepare(&id("Plan"), ExportKindName::Html, Some(typed(&target)))
        .unwrap();
    session.finish_export(job.run());

    assert!(fs::read_to_string(&target).unwrap().contains("<html"));
    assert_eq!(last_offers(&mut session), [offer("Plan", false, false)]);
    assert_eq!(session.export_target(0), None);
}

#[test]
fn uc009_a120_reports_of_exports_ending_together_are_kept() {
    let dir = temp_dir("two-exports");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Plan.md", "# Plan\n"),
            ("Guide.adoc", "= Guide\n\nNOTE: Gap.\n"),
        ],
    );
    let first = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(typed(&dir.join("Plan.html"))),
        )
        .unwrap();
    let second = session
        .export_prepare(
            &id("Guide"),
            ExportKindName::OtherFormat,
            Some(typed(&dir.join("Guide.md"))),
        )
        .unwrap();
    assert_eq!(session.status(), "Exporting Guide…");

    session.finish_export(first.run());
    assert!(last_status(&mut session).unwrap().busy, "Guide still runs");
    session.finish_export(second.run());

    assert_eq!(
        session.status(),
        format!(
            "Exported Plan to {}\nExported Guide to {}; 1 block only approximated",
            dir.join("Plan.html").display(),
            dir.join("Guide.md").display()
        )
    );
    let status = last_status(&mut session).unwrap();
    assert!(!status.busy);
    assert_eq!(
        status.exports,
        [offer("Plan", true, false), offer("Guide", true, true)]
    );
    assert_eq!(
        session.export_target(0),
        Some(dir.join("Plan.html").display().to_string())
    );
    assert_eq!(session.export_approximated(1).unwrap().lines, [3]);
}

#[test]
fn uc009_a113_saved_does_not_take_the_place_of_exporting() {
    let dir = temp_dir("exporting-kept");
    let (mut session, root) = session_with(&dir, &[("Plan.md", "# Plan\n")]);
    session.open_note(&id("Plan"));
    session.window_type(&id("Plan"), 7, "more\n");
    let job = session
        .export_prepare(
            &id("Plan"),
            ExportKindName::Html,
            Some(typed(&dir.join("P.html"))),
        )
        .unwrap();

    let start = Instant::now();
    while fs::read_to_string(root.join("Plan.md")).unwrap() == "# Plan\n"
        && start.elapsed() < Duration::from_secs(5)
    {
        session.tick(Instant::now() + Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(20));
    }
    session.tick(Instant::now() + Duration::from_secs(3));

    assert_eq!(session.status(), "Exporting Plan…");
    session.finish_export(job.run());
    assert!(session.status().starts_with("Exported Plan to "));
}

#[test]
fn uc009_a116_a_typed_note_is_refused_before_replace_is_asked() {
    let dir = temp_dir("refused-first");
    let (mut session, root) = session_with(
        &dir,
        &[("Plan.md", "# Plan\n"), ("Closed.md", "# Closed\n")],
    );
    session.open_note(&id("Plan"));
    for note in ["Closed.md", "Plan.md"] {
        let answer = session
            .export_prepare(
                &id("Plan"),
                ExportKindName::Html,
                Some(TypedExportPath {
                    path: root.join(note).display().to_string(),
                    replace: None,
                }),
            )
            .unwrap_err();
        assert_eq!(answer, not_onto_a_note(), "{note}");
    }
}

#[test]
fn uc009_a121_a_cancelled_dialog_gives_back_the_report_with_its_offers() {
    let dir = temp_dir("restore-offers");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Guide.adoc", "= Guide\n\nNOTE: Gap.\n"),
            ("Plan.md", "# Plan\n"),
        ],
    );
    export_to(
        &mut session,
        "Guide",
        ExportKindName::OtherFormat,
        &dir.join("Guide.md"),
    );
    let report = session.status().to_owned();
    assert_eq!(last_offers(&mut session), [offer("Guide", true, true)]);

    session
        .export_dialog(&id("Plan"), ExportKindName::Pdf)
        .unwrap();
    let status = last_status(&mut session).unwrap();
    assert!(status.exports.is_empty(), "no offers while the dialog asks");
    let _ = session.tick(Instant::now());
    session.export_dialog_answer(&id("Plan"), ExportKindName::Pdf, DialogOutcome::Cancelled);

    let status = last_status(&mut session).unwrap();
    assert_eq!(status.text, report);
    assert_eq!(status.exports, [offer("Guide", true, true)]);
    assert!(session.export_target(0).is_some());
    assert_eq!(session.export_approximated(0).unwrap().lines, [3]);
}

#[test]
fn uc009_a121_a_cancelled_dialog_gives_back_the_undo_of_a_deletion() {
    let dir = temp_dir("restore-undo");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n"), ("Plan.md", "")]);
    session.delete_note(&id("Plan"), false);
    let _ = session.take_events();

    session
        .export_dialog(&id("Home"), ExportKindName::Pdf)
        .unwrap();
    let _ = session.tick(Instant::now());
    session.export_dialog_answer(&id("Home"), ExportKindName::Pdf, DialogOutcome::Cancelled);

    let status = last_status(&mut session).unwrap();
    assert_eq!(status.text, "Deleted Plan");
    assert_eq!(status.offer, Some(crate::StatusOffer::UndoDelete));
}
