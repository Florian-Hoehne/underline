//! The system's save dialog for an export (UC-009 step 2, ADR-016), through
//! `rfd` as in the egui app; no Tauri plugin.
//!
//! `rfd`'s blocking dialog runs on a thread of the blocking pool, never on
//! the main thread, which runs Tauri's event loop: on Linux it asks the XDG
//! desktop portal over D-Bus (libdbus, no GTK), falling back to `zenity`;
//! on Windows it is the COM dialog on the calling thread; on macOS `rfd`
//! hands the panel to the main thread itself (`run_on_main`).
//!
//! Inventory D-09 (fixed, paritaet.md A-02): egui took any empty answer
//! within 300 ms for "no dialog could be shown", so a quick cancel showed
//! the path field. `rfd`'s blocking API answers `None` for both; what
//! tells them apart is what `rfd` logs: when neither the portal nor
//! `zenity` could show a dialog, it logs an error ("Failed to save file
//! with zenity") on the calling thread. A small logger takes `rfd`'s
//! records only and remembers an error of the current thread; a warning
//! that it falls back to `zenity` forgets the portal's errors before it,
//! since `zenity` may still show the dialog.

use std::cell::Cell;
use std::path::PathBuf;

use underline_session::{DialogOutcome, ExportDialogPlan};

thread_local! {
    /// `rfd` could not show a dialog during the current call on this thread.
    static FAILED: Cell<bool> = const { Cell::new(false) };
}

/// Takes the records of `rfd` and passes them to stderr, as there is no
/// other logger in the app.
struct RfdLog;

impl log::Log for RfdLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target().starts_with("rfd") && metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        match record.level() {
            log::Level::Error => FAILED.with(|failed| failed.set(true)),
            // The portal failed; zenity is tried next and may work. Coupled
            // to rfd 0.17.2's text `warn!("Using zenity fallback")` in
            // `backend/xdg_desktop_portal.rs`; rfd is pinned exactly for it.
            _ if message.contains("zenity fallback") => FAILED.with(|failed| failed.set(false)),
            _ => {}
        }
        eprintln!("{}: {message}", record.target());
    }

    fn flush(&self) {}
}

static LOGGER: RfdLog = RfdLog;

/// Installs the logger that tells a failed dialog from a cancelled one. A
/// second call, or another logger set before, changes nothing; the dialog
/// then counts every empty answer as cancelled.
pub fn install_logger() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    } else {
        eprintln!(
            "Another logger was set first; a save dialog that cannot show counts as cancelled"
        );
    }
}

/// What the dialog's answer means, given whether `rfd` logged that it could
/// not show one.
fn outcome(chosen: Option<PathBuf>, failed: bool) -> DialogOutcome {
    match chosen {
        Some(path) => DialogOutcome::Chosen(path),
        None if failed => DialogOutcome::Unavailable,
        None => DialogOutcome::Cancelled,
    }
}

/// UC-009 step 2, M-15.2: shows the system's save dialog "Export as
/// {label}" in the planned folder with the planned name and one filter,
/// and waits for the user. Blocks: call it on the blocking pool.
pub fn ask_save(plan: &ExportDialogPlan) -> DialogOutcome {
    FAILED.with(|failed| failed.set(false));
    let chosen = rfd::FileDialog::new()
        .set_title(&plan.title)
        .set_directory(&plan.folder)
        .set_file_name(&plan.file_name)
        .add_filter(&plan.filter, &[plan.extension.as_str()])
        .save_file();
    outcome(chosen, FAILED.with(Cell::get))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc009_m15_2_a_chosen_file_is_taken() {
        assert_eq!(
            outcome(Some(PathBuf::from("/out/Plan.pdf")), false),
            DialogOutcome::Chosen(PathBuf::from("/out/Plan.pdf"))
        );
    }

    /// D-09 (fixed): an empty answer is a cancel, however quick, unless
    /// `rfd` said it could show no dialog.
    #[test]
    fn uc009_d09_an_empty_answer_is_a_cancel_unless_no_dialog_showed() {
        assert_eq!(outcome(None, false), DialogOutcome::Cancelled);
        assert_eq!(outcome(None, true), DialogOutcome::Unavailable);
    }

    #[test]
    fn uc009_d09_the_logger_remembers_rfds_failure_on_this_thread() {
        install_logger();
        FAILED.with(|failed| failed.set(false));

        log::error!(target: "rfd::backend::xdg_desktop_portal", "OpenFile failed: no portal");
        assert!(FAILED.with(Cell::get));
        // The portal's failure is forgotten when zenity is tried.
        log::warn!(target: "rfd::backend::xdg_desktop_portal", "Using zenity fallback");
        assert!(!FAILED.with(Cell::get));
        log::error!(target: "rfd::backend::xdg_desktop_portal", "Failed to save file with zenity: not found");
        assert!(FAILED.with(Cell::get));

        // Other crates' records do not count; nor does another thread.
        FAILED.with(|failed| failed.set(false));
        log::error!(target: "tauri::app", "something else");
        std::thread::spawn(|| log::error!(target: "rfd::backend", "elsewhere"))
            .join()
            .unwrap();
        assert!(!FAILED.with(Cell::get));
    }
}
