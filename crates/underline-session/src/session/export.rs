//! Feature F11 of the migration: UC-009 Export Note in the session
//! (inventory M-15, M-04.8, M-05.4, M-06.4).
//!
//! Ported from `start_export`, `ask_export_place`, `show_export`,
//! `run_export` and `take_export` of `apps/underline-desktop/src/app.rs`;
//! the texts are egui's unless marked new. What is exported is the
//! facade's (`underline_facade::export`, ADR-015), unchanged.
//!
//! The steps:
//! - [`Session::export_start`]: "Export {t} as:" with the kinds (step 1),
//!   refused for a note in conflict (UC-015 BR-007);
//! - [`Session::export_dialog`]: where the system's file dialog starts
//!   (step 2), one dialog at a time; the dialog itself runs in the Tauri
//!   app without the session (ADR-016), and its answer comes back to
//!   [`Session::export_dialog_answer`], which keeps a chosen file;
//! - [`Session::export_prepare`]: after the window sent its edits
//!   (`flushed`, ADR-025), the mirror's text is taken, the place is checked
//!   (A5 and the files of the workspace, A-116), and an [`ExportJob`] is
//!   made;
//! - [`ExportJob::run`]: the export is made and written atomically
//!   (ADR-005) without the session, so a PDF set by Typst holds nothing up;
//!   a panic in it becomes a failure (review F11);
//! - [`Session::finish_export`]: the report of step 5, A2 to A4, with the
//!   offers "Open" and "Show" of the status bar (M-04.8). Reports of
//!   exports that end while an earlier report still shows are kept
//!   together (A-120).

use std::fmt::Write as _;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};

use underline_core::{NoteFormat, NoteId, SETTINGS_FILE_NAME};
use underline_facade::export::{export, ExportError, ExportKind, ExportRequest, Exported, Paper};

use super::Session;
use crate::export_view::{
    DialogOutcome, ExportAnswer, ExportChoice, ExportKindName, ExportOfferView, ExportPlaceAnswer,
    ExportShown, ExportStart, TypedExportPath,
};

/// UC-009 A5: what the status says when the place is a note's file.
pub const NOT_ONTO_A_NOTE: &str = "A note cannot be replaced by an export; choose another place";

/// ADR-016: the text under the path field when the file exists.
pub const FILE_EXISTS: &str = "The file exists. Export again to replace it.";

/// Review F11, new text (A-117): the path field is empty.
pub const NO_PATH: &str = "Enter the file to export to.";

/// Review F11, new text (A-117): the typed path is a folder.
fn is_a_folder(path: &str) -> String {
    format!("{path} is a folder. Enter a file name in it.")
}

/// Review F11, new text (A-118): the export job ended with a panic.
const STOPPED: &str = "the export stopped unexpectedly";

/// At most this many reports of exports show together (A-120).
const REPORTS_AT_MOST: usize = 3;

/// "1 block", "2 blocks" (egui's `count`).
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{n} {thing}s")
    }
}

/// The home folder, where the first export of a session starts (BR-007).
pub(super) fn home_folder() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The locale of the system, for the paper of a PDF (UC-009 BR-004,
/// M-15.4): `LC_ALL`, `LC_PAPER`, `LANG`, the first one set.
fn system_locale() -> String {
    ["LC_ALL", "LC_PAPER", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default()
}

/// Where `path` really is: canonical when it exists, else its canonical
/// folder with its name; `None` when its folder does not exist either.
fn real_place(path: &Path) -> Option<PathBuf> {
    if let Ok(real) = path.canonicalize() {
        return Some(real);
    }
    let parent = path.parent()?.canonicalize().ok()?;
    Some(parent.join(path.file_name()?))
}

/// UC-009 step 2: what the system's file dialog shows (egui's
/// `ask_export_place`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDialogPlan {
    /// "Export as {label}".
    pub title: String,
    /// The folder of the last export in this session, else the home
    /// folder.
    pub folder: PathBuf,
    /// The note's name with the extension of the export (BR-007).
    pub file_name: String,
    /// The one filter: its name ("AsciiDoc", "HTML", "PDF") and extension.
    pub filter: String,
    /// The extension of the filter, without the dot.
    pub extension: String,
}

/// The file the user chose in the system's dialog, until the export runs.
#[derive(Debug)]
struct ChosenPlace {
    /// The workspace it was chosen in; another one opened meanwhile ends it.
    root: PathBuf,
    note: NoteId,
    kind: ExportKind,
    path: PathBuf,
}

/// The system's dialog is open (one at a time, review F11): the status
/// says "Choosing where to save {t}…" and says what it said before once
/// the dialog is cancelled.
#[derive(Debug)]
struct Asking {
    status: String,
    before: String,
}

/// UC-009 step 5, A2, A4: the report of one export, with what the status
/// bar offers for it while it shows.
#[derive(Debug)]
struct ExportReport {
    title: String,
    /// The file written; `None` when the export failed.
    target: Option<PathBuf>,
    /// The export has the extension of its kind, so "Open" opens it with
    /// the program for that kind (review F11, A-119).
    openable: bool,
    /// The lines of the approximated blocks (A2).
    approximated: Vec<usize>,
}

/// What the session knows of exports: the last folder (BR-007, only for
/// this session, M-22.4), the chosen file, the open dialog, the running
/// jobs and the reports the status shows.
#[derive(Debug, Default)]
pub(super) struct Exports {
    last_folder: Option<PathBuf>,
    chosen: Option<ChosenPlace>,
    asking: Option<Asking>,
    /// The running jobs by number, with their "Exporting {t}…".
    running: Vec<(u64, String)>,
    next_job: u64,
    reports: Vec<ExportReport>,
    /// The status the reports belong to; another status ends them.
    reports_status: String,
    /// One of the reports is a failure (UC-009 A4).
    failed: bool,
}

/// One export, made under the session and run without it.
#[derive(Debug)]
pub struct ExportJob {
    id: u64,
    request: ExportRequest,
    kind: ExportKind,
    title: String,
}

/// What a finished export says (egui's `ExportOutcome`).
#[derive(Debug)]
pub struct ExportOutcome {
    id: u64,
    kind: ExportKind,
    title: String,
    target: PathBuf,
    result: Result<Exported, String>,
}

impl ExportJob {
    /// UC-009 step 4, A4: makes the export and writes it whole or not at
    /// all (BR-007, ADR-005). Long for a PDF; runs without the session. A
    /// panic while making it is a failure, so the status never stays at
    /// "Exporting {t}…" (review F11).
    #[must_use]
    pub fn run(self) -> ExportOutcome {
        self.run_with(export)
    }

    fn run_with(
        self,
        make: impl FnOnce(&ExportRequest, ExportKind) -> Result<Exported, ExportError>,
    ) -> ExportOutcome {
        let request = &self.request;
        let kind = self.kind;
        let made = catch_unwind(AssertUnwindSafe(|| make(request, kind)));
        let result = match made {
            Ok(Ok(exported)) => underline_core::write_atomic(&request.target, &exported.bytes)
                .map(|()| exported)
                .map_err(|e| e.to_string()),
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err(STOPPED.to_owned()),
        };
        self.outcome(result)
    }

    fn outcome(self, result: Result<Exported, String>) -> ExportOutcome {
        ExportOutcome {
            id: self.id,
            kind: self.kind,
            title: self.title,
            target: self.request.target,
            result,
        }
    }

    /// The outcome of a job that could not run at all (its thread ended),
    /// as a failure with `why`.
    #[must_use]
    pub fn aborted(id: u64, title: String, why: &str) -> ExportOutcome {
        ExportOutcome {
            id,
            kind: ExportKind::Html,
            title,
            target: PathBuf::new(),
            result: Err(why.to_owned()),
        }
    }

    /// The number of the job, for [`ExportJob::aborted`].
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The title of the note exported.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Where the export goes.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.request.target
    }
}

impl Session {
    /// The note format of `note`, Markdown when its file is gone (egui).
    fn export_format(&self, note: &NoteId) -> Option<NoteFormat> {
        let open = self.open.as_ref()?;
        Some(
            open.workspace
                .format_of(note)
                .unwrap_or(NoteFormat::Markdown),
        )
    }

    /// UC-015 BR-007: a note in conflict is not exported; the status says
    /// so. `false` also without a workspace.
    fn may_export(&mut self, note: &NoteId) -> bool {
        if self.open.is_none() {
            return false;
        }
        if self.in_conflict(note) {
            self.status = format!(
                "{} is in conflict; resolve it before exporting it",
                note.title()
            );
            return false;
        }
        true
    }

    /// UC-009 step 1, BR-008, M-15.1: Ctrl+Shift+E or "Export…" of the tab
    /// asks "Export {t} as:" with the kinds of export of the note's format.
    /// `None` without a workspace, or for a note in conflict (the status
    /// says so).
    pub fn export_start(&mut self, note: &NoteId) -> Option<ExportStart> {
        if !self.may_export(note) {
            return None;
        }
        let format = self.export_format(note)?;
        Some(ExportStart {
            note: note.as_str().to_owned(),
            title: note.title(),
            choices: ExportChoice::all(format),
        })
    }

    /// UC-009 step 2, M-15.2: what the system's file dialog for exporting
    /// `note` as `kind` shows: "Export as {label}", the folder of the last
    /// export of this session or the home folder, the note's name with the
    /// extension, and one filter. The status says "Choosing where to save
    /// {t}…" while it is open (new text, A-121). `None` without a
    /// workspace, for a note in conflict (the menu of the note overview
    /// gives the kind at once, M-05.4, so the refusal is here too), or
    /// while a dialog is open already (one at a time, review F11).
    pub fn export_dialog(
        &mut self,
        note: &NoteId,
        kind: ExportKindName,
    ) -> Option<ExportDialogPlan> {
        if self.exports.asking.is_some() || !self.may_export(note) {
            return None;
        }
        let format = self.export_format(note)?;
        let kind = ExportKind::from(kind);
        let extension = kind.extension(format);
        let label = kind.label(format);
        let status = format!("Choosing where to save {}…", note.title());
        self.exports.asking = Some(Asking {
            status: status.clone(),
            before: std::mem::replace(&mut self.status, status),
        });
        Some(ExportDialogPlan {
            title: format!("Export as {label}"),
            folder: self.export_folder(),
            file_name: export_file_name(note, extension),
            filter: label.to_owned(),
            extension: extension.to_owned(),
        })
    }

    /// The folder the dialog and the path field start in.
    fn export_folder(&self) -> PathBuf {
        self.exports
            .last_folder
            .clone()
            .or_else(home_folder)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// UC-009 step 3, A1, ADR-016: the answer of the system's file dialog.
    /// A chosen file is kept for [`Session::export_prepare`]; cancelled,
    /// nothing happens and the status says what it said before; without a
    /// dialog, the window asks for the path in a field, proposing the
    /// dialog's folder and name.
    pub fn export_dialog_answer(
        &mut self,
        note: &NoteId,
        kind: ExportKindName,
        outcome: DialogOutcome,
    ) -> ExportPlaceAnswer {
        if let Some(asking) = self.exports.asking.take() {
            if self.status == asking.status {
                self.status = asking.before;
            }
        }
        match outcome {
            DialogOutcome::Chosen(path) => {
                let Some(root) = self.workspace_root().map(Path::to_owned) else {
                    return ExportPlaceAnswer::Done;
                };
                let answer = ExportPlaceAnswer::Chosen {
                    path: path.display().to_string(),
                };
                self.exports.chosen = Some(ChosenPlace {
                    root,
                    note: note.clone(),
                    kind: kind.into(),
                    path,
                });
                answer
            }
            DialogOutcome::Cancelled => ExportPlaceAnswer::Done,
            DialogOutcome::Unavailable => {
                let Some(format) = self.export_format(note) else {
                    return ExportPlaceAnswer::Done;
                };
                let kind = ExportKind::from(kind);
                let name = export_file_name(note, kind.extension(format));
                ExportPlaceAnswer::Typed {
                    heading: format!("Export {} as {} to:", note.title(), kind.label(format)),
                    path: self.export_folder().join(name).display().to_string(),
                }
            }
        }
    }

    /// The typed path, checked (ADR-016, review F11): not empty, not a
    /// folder. Whether it replaces a file is asked after A5 (`exists_unasked`).
    fn typed_place(typed: &TypedExportPath) -> Result<PathBuf, ExportAnswer> {
        let trimmed = typed.path.trim();
        if trimmed.is_empty() {
            return Err(ExportAnswer::Problem {
                problem: NO_PATH.to_owned(),
            });
        }
        let path = PathBuf::from(trimmed);
        if path.is_dir() {
            return Err(ExportAnswer::Problem {
                problem: is_a_folder(trimmed),
            });
        }
        Ok(path)
    }

    /// D-03 (fixed): the typed file exists and the user has not agreed to
    /// replace it; agreeing holds for that path only.
    fn exists_unasked(typed: &TypedExportPath) -> Option<ExportAnswer> {
        let trimmed = typed.path.trim();
        (Path::new(trimmed).exists() && typed.replace.as_deref().map(str::trim) != Some(trimmed))
            .then(|| ExportAnswer::Exists {
                problem: FILE_EXISTS.to_owned(),
                path: trimmed.to_owned(),
            })
    }

    /// UC-009 A5, A-116: whether an export to `path` would replace a file
    /// of the workspace that is no export: a note of the overview (also one
    /// without a tab) or of a tab, the workspace's `underline.yaml`, or
    /// anything in a folder whose name starts with a dot (`.git`, `.todos`,
    /// `.underline`). Symbolic links count by where they lead.
    fn replaces_workspace_file(&self, path: &Path) -> bool {
        let Some(open) = &self.open else {
            return false;
        };
        let Some(real) = real_place(path) else {
            return false;
        };
        let same = |file: &Path| real_place(file).is_some_and(|f| f == real);
        if open.tabs.iter().any(|(_, tab)| same(&tab.path)) {
            return true;
        }
        let Some(root) = real_place(open.root()) else {
            return false;
        };
        let Ok(inside) = real.strip_prefix(&root) else {
            return false;
        };
        let dotted = inside.components().any(
            |part| matches!(part, Component::Normal(name) if name.to_string_lossy().starts_with('.')),
        );
        dotted
            || inside == Path::new(SETTINGS_FILE_NAME)
            || open
                .workspace
                .listing()
                .notes
                .iter()
                .any(|entry| same(&open.root().join(entry.id.file_name(entry.format))))
    }

    /// UC-009 steps 3–4, A5, M-15.3, M-15.4: the export of `note` as
    /// `kind` to the file chosen in the dialog (`typed` is `None`) or typed
    /// in the field. Runs after the window sent its edits (ADR-025), so the
    /// mirror is the text the tab shows, with changes not yet saved
    /// (BR-001); a note without a tab is read from its file. The status
    /// says "Exporting {t}…" until the job is finished, and background
    /// messages do not take its place.
    ///
    /// # Errors
    ///
    /// What the window does instead of the export: ask again for a place
    /// (A5 after the dialog), say why in the path field (A5, an empty path,
    /// a folder, a file that exists), or nothing more (the status says
    /// why).
    pub fn export_prepare(
        &mut self,
        note: &NoteId,
        kind: ExportKindName,
        typed: Option<TypedExportPath>,
    ) -> Result<ExportJob, ExportAnswer> {
        let kind = ExportKind::from(kind);
        let is_typed = typed.is_some();
        let path = match &typed {
            Some(typed) => Self::typed_place(typed)?,
            None => match self.exports.chosen.take() {
                Some(chosen)
                    if &chosen.note == note
                        && chosen.kind == kind
                        && Some(chosen.root.as_path()) == self.workspace_root() =>
                {
                    chosen.path
                }
                _ => return Err(ExportAnswer::Done),
            },
        };
        let Some(open) = &self.open else {
            return Err(ExportAnswer::Done);
        };
        let root = open.root().to_owned();
        let format = open
            .workspace
            .format_of(note)
            .unwrap_or(NoteFormat::Markdown);
        let file = root.join(note.file_name(format));
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        // A5, A-116: a note, nor any file of the workspace's own, is ever
        // replaced by an export.
        if self.replaces_workspace_file(&path) {
            self.status = NOT_ONTO_A_NOTE.to_owned();
            return Err(if is_typed {
                ExportAnswer::Problem {
                    problem: NOT_ONTO_A_NOTE.to_owned(),
                }
            } else {
                ExportAnswer::AskAgain
            });
        }
        // Only a file that may be replaced is asked about (app-tester F11:
        // a note got "Replace" first and was refused only then).
        if let Some(exists) = typed.as_ref().and_then(Self::exists_unasked) {
            return Err(exists);
        }
        let Some(open) = &self.open else {
            return Err(ExportAnswer::Done);
        };
        let text = match open.tabs.find(note) {
            Some(index) => open.tabs.get(index).mirror.text(),
            None => match open.workspace.read(note) {
                Ok(read) => read.body,
                Err(err) => {
                    self.status = format!("{} could not be exported: {err}", note.title());
                    return Err(ExportAnswer::Done);
                }
            },
        };
        self.exports.last_folder = path.parent().map(Path::to_owned);
        self.status = format!("Exporting {}…", note.title());
        self.exports.next_job += 1;
        let id = self.exports.next_job;
        self.exports.running.push((id, self.status.clone()));
        Ok(ExportJob {
            id,
            request: ExportRequest {
                text,
                format,
                title: note.title(),
                root,
                note: file,
                target: path,
                paper: Paper::of_region(&system_locale()),
            },
            kind,
            title: note.title(),
        })
    }

    /// UC-009 step 5, A2 to A4, M-15.5: the report of a finished export:
    /// "Exported {t} to {p}", with "; N block(s) only approximated" and
    /// "; missing: …", offering "Open" and, with approximated blocks,
    /// "Show"; or "{t} could not be exported: {err}". An export into the
    /// workspace is a new note of the overview (A6). While the reports of
    /// earlier exports still show, this one joins them on a line of its
    /// own (A-120). Returns why the export failed, for the path field.
    pub fn finish_export(&mut self, outcome: ExportOutcome) -> Option<String> {
        self.exports.running.retain(|(id, _)| *id != outcome.id);
        let (text, report, failure) = match outcome.result {
            Ok(exported) => {
                let mut status =
                    format!("Exported {} to {}", outcome.title, outcome.target.display());
                if !exported.approximated.is_empty() {
                    let _ = write!(
                        status,
                        "; {} only approximated",
                        count(exported.approximated.len(), "block")
                    );
                }
                if !exported.missing.is_empty() {
                    let _ = write!(status, "; missing: {}", exported.missing.join(", "));
                }
                let openable = outcome.target.extension().is_some_and(|ext| {
                    kind_extensions(outcome.kind)
                        .iter()
                        .any(|known| ext.eq_ignore_ascii_case(known))
                });
                if let Some(open) = &mut self.open {
                    open.workspace.rescan();
                }
                self.refresh_overview();
                let report = ExportReport {
                    title: outcome.title,
                    target: Some(outcome.target),
                    openable,
                    approximated: exported.approximated,
                };
                (status, report, None)
            }
            // A4: nothing is left at the place.
            Err(err) => {
                let status = format!("{} could not be exported: {err}", outcome.title);
                let report = ExportReport {
                    title: outcome.title,
                    target: None,
                    openable: false,
                    approximated: Vec::new(),
                };
                (status.clone(), report, Some(status))
            }
        };
        let shown = !self.exports.reports.is_empty() && self.status == self.exports.reports_status;
        if !shown {
            self.exports.reports.clear();
            self.exports.failed = false;
        }
        self.exports.failed |= failure.is_some();
        self.exports.reports.push(report);
        let joined = if shown {
            format!("{}\n{text}", self.exports.reports_status)
        } else {
            text
        };
        // At most three reports: the oldest goes, with its line.
        if self.exports.reports.len() > REPORTS_AT_MOST {
            self.exports.reports.remove(0);
            let rest = joined
                .split_once('\n')
                .map_or(joined.as_str(), |(_, rest)| rest);
            self.exports.reports_status = rest.to_owned();
        } else {
            self.exports.reports_status = joined;
        }
        self.status.clone_from(&self.exports.reports_status);
        failure
    }

    /// The report shown in the status, number `index` (from 0).
    fn shown_export_report(&self, index: u32) -> Option<&ExportReport> {
        if self.exports.reports.is_empty() || self.status != self.exports.reports_status {
            return None;
        }
        self.exports.reports.get(usize::try_from(index).ok()?)
    }

    /// M-04.8: "Open": the file of the export `index` the status reports,
    /// to open with the system's program; `None` once another report took
    /// its place, or for a failure or a file without the kind's extension.
    #[must_use]
    pub fn export_target(&self, index: u32) -> Option<String> {
        self.shown_export_report(index)
            .filter(|report| report.openable)
            .and_then(|report| report.target.as_ref())
            .map(|target| target.display().to_string())
    }

    /// M-04.8, M-15.6, UC-009 A2 step 3: "Show": the note and the lines of
    /// the blocks the export `index` shown in the status only
    /// approximated.
    #[must_use]
    pub fn export_approximated(&self, index: u32) -> Option<ExportShown> {
        let report = self.shown_export_report(index)?;
        Some(ExportShown {
            title: report.title.clone(),
            lines: report
                .approximated
                .iter()
                .map(|line| u32::try_from(*line).unwrap_or(u32::MAX))
                .collect(),
        })
    }

    /// M-04.8: what the status bar offers for the exports it reports, and
    /// whether one of them failed; the reports end once another status
    /// takes their place (egui).
    pub(super) fn export_offers(&mut self) -> (Vec<ExportOfferView>, bool) {
        if !self.exports.reports.is_empty() && self.status != self.exports.reports_status {
            // App-tester F11: while the system's dialog is open, the reports
            // it replaced come back with their offers when it is cancelled.
            if self.replaced_by_dialog(&self.exports.reports_status) {
                return (Vec::new(), false);
            }
            self.exports.reports.clear();
            self.exports.failed = false;
        }
        let offers = self
            .exports
            .reports
            .iter()
            .map(|report| ExportOfferView {
                title: report.title.clone(),
                open: report.openable && report.target.is_some(),
                show: !report.approximated.is_empty(),
            })
            .collect();
        (offers, self.exports.failed)
    }

    /// The open system dialog's status took the place of `status`, which
    /// comes back with its offers when the dialog is cancelled.
    pub(super) fn replaced_by_dialog(&self, status: &str) -> bool {
        self.exports
            .asking
            .as_ref()
            .is_some_and(|asking| asking.before == status && asking.status == self.status)
    }

    /// Review F11: an export is running; the status bar is busy.
    pub(super) fn exporting(&self) -> bool {
        !self.exports.running.is_empty()
    }

    /// D-04 (fixed, A-113, review F11): the export's own status ("Choosing
    /// where to save {t}…", "Exporting {t}…", its report or failure), which
    /// background messages such as "Saved {t}" do not replace.
    pub(super) fn export_status_kept(&self) -> bool {
        let status = self.status.as_str();
        self.exports
            .asking
            .as_ref()
            .is_some_and(|asking| asking.status == status)
            || self
                .exports
                .running
                .iter()
                .any(|(_, running)| running == status)
            || (!self.exports.reports.is_empty() && self.exports.reports_status == status)
    }
}

/// The extensions a file of the export `kind` has (either note format for
/// the other format, review F11: "Open" only for those).
fn kind_extensions(kind: ExportKind) -> &'static [&'static str] {
    match kind {
        ExportKind::OtherFormat => &["md", "markdown", "adoc", "asciidoc", "asc"],
        ExportKind::Html => &["html", "htm"],
        ExportKind::Pdf => &["pdf"],
    }
}

/// The file name the dialog proposes: the note's name and `extension`
/// (BR-007, `Getting-Started.adoc`).
fn export_file_name(note: &NoteId, extension: &str) -> String {
    format!(
        "{}.{extension}",
        note.as_str().rsplit('/').next().unwrap_or("note")
    )
}

#[cfg(test)]
mod tests;
