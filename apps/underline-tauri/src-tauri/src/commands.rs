//! The commands the window calls: a thin layer over `underline_session`.
//!
//! Every command returns `Result<_, UiError>`, so the window gets the same
//! texts the egui app shows. A command that changes the session wakes the
//! tick, which sends the changes to the window as events (ADR-022).
//!
//! A new command is registered in three places: `bindings::builder` (handler
//! and TypeScript bindings), `build.rs` (its `allow-…` permission) and
//! `capabilities/main.json` (granted to the window); the tests below check
//! that they agree. Commands that do file, git or search I/O must be `async`
//! (or use `tauri::async_runtime::spawn_blocking`) and hold the session lock
//! only briefly.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_specta::Event as _;
use underline_core::NoteId;
use underline_session::{
    clipboard_view, ClipContentView, CloseQuestion, CreateAnswer, CreateNoteAnswer, DeleteAnswer,
    DeleteQuestion, DragName, EditingViewName, EmbedAnswer, FormatName, Insertion, LinkAnswer,
    MirrorView, NewNoteRequest, NewNoteStart, NewWorkspaceRequest, OpenAnswer, RenameAnswer,
    RenameStart, RestoreAnswer, Scope, Session, SettingName, SettingsAnswer, SettingsWindowView,
    StartInfo, StartOpened, SyncAnswer, SyncBatch, Synced, UiError, UiErrorKind,
};
use underline_session::{
    DialogOutcome, ExportAnswer, ExportDialogPlan, ExportJob, ExportKindName, ExportPlaceAnswer,
    ExportShown, ExportStart, TypedExportPath,
};
use underline_session::{FoundNoteView, FoundPlace, SearchOpened};

use crate::clipboard::SystemClipboard;
use crate::events::CloseQuestionEvent;
use crate::ticker::Waker;

/// The session as the app manages it: shared, so a command can hand it to
/// the blocking pool.
pub type SharedSession = Arc<Mutex<Session>>;

/// M-01: what the window needs to know when it starts: the workspace folder
/// from the command line, the known workspaces, the operating system, the
/// settings with the theme's palette and the status message.
#[tauri::command]
#[specta::specta]
pub fn app_start(session: State<'_, SharedSession>) -> Result<StartInfo, UiError> {
    Ok(session.lock().start_info())
}

/// UC-010 A3, A4, M-20.5: changes a setting from outside the settings
/// window, such as the theme from the Theme menu (M-03.6) or the menu bar by
/// the burger button (M-03.1). It applies at once and is stored at once; a
/// failure is said in the status bar.
///
/// Storing writes a file, so it runs on the blocking pool, not on an async
/// worker (ADR-022). The session stays locked while the one settings file
/// is written, which keeps the setting and its file in step.
#[tauri::command]
#[specta::specta]
pub async fn settings_quick(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    key: SettingName,
    value: String,
) -> Result<(), UiError> {
    let session = Arc::clone(&session);
    let waker = waker.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        session.lock().quick_setting(key, value);
        waker.wake();
    })
    .await
    .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// UC-010 step 2, M-20.2 to M-20.4: what the settings window shows of "For
/// me" and "This workspace". The window asks when it opens; the event
/// `settings-window` keeps it current.
#[tauri::command]
#[specta::specta]
pub async fn settings_window(
    session: State<'_, SharedSession>,
) -> Result<SettingsWindowView, UiError> {
    read_session(&session, Session::settings_window).await
}

/// UC-010 steps 3 and 4, A1, A2, A8, M-20.5: a setting changed in the part
/// `scope` of the settings window; `value` is `null` for "As for me" and
/// "Follow the notes". It applies at once and is stored at once; a failure
/// is said in the status bar and in the answer. Answers with what the
/// window shows now, so its controls never show a value that did not take. Writes a file, so on
/// the blocking pool.
#[tauri::command]
#[specta::specta]
pub async fn settings_set(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    scope: Scope,
    key: SettingName,
    value: Option<String>,
) -> Result<SettingsAnswer, UiError> {
    on_session(&session, &waker, move |session| {
        session.change_setting(scope, key, value)
    })
    .await
}

/// Runs `work` on the session on the blocking pool, as file and git I/O
/// must not run on an async worker (ADR-022), and wakes the tick after it,
/// which sends the changes to the window as events.
async fn on_session<T: Send + 'static>(
    session: &State<'_, SharedSession>,
    waker: &State<'_, Waker>,
    work: impl FnOnce(&mut Session) -> T + Send + 'static,
) -> Result<T, UiError> {
    let session = Arc::clone(session);
    let waker = waker.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = work(&mut session.lock());
        waker.wake();
        result
    })
    .await
    .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// Runs `work`, which only reads the session, on the blocking pool; the
/// tick is not woken, as nothing changed.
async fn read_session<T: Send + 'static>(
    session: &State<'_, SharedSession>,
    work: impl FnOnce(&Session) -> T + Send + 'static,
) -> Result<T, UiError> {
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || work(&session.lock()))
        .await
        .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// M-01.1, UC-002 A1/A2: opens the workspace given on the command line,
/// else the one opened last. The window shows its loading state until the
/// answer comes; `Problem` shows the Open dialog with what stands in the
/// way. Only the first call opens anything.
#[tauri::command]
#[specta::specta]
pub async fn ws_start(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
) -> Result<StartOpened, UiError> {
    let shared = Arc::clone(&session);
    // The directory is checked and read without holding the session (A-22).
    let prepared = blocking(move || {
        let plan = shared.lock().plan_start();
        plan.map(|(plan, dir)| plan.prepare(&dir, false))
    })
    .await?;
    on_session(&session, &waker, move |session| {
        session.finish_start(prepared)
    })
    .await
}

/// Runs `work` on the blocking pool.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, UiError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| UiError::new(UiErrorKind::Internal, error.to_string()))
}

/// M-21.1, UC-002 step 2: the recently opened workspaces, the most recent
/// first, for the Open dialog.
#[tauri::command]
#[specta::specta]
pub async fn ws_recent(session: State<'_, SharedSession>) -> Result<Vec<String>, UiError> {
    read_session(&session, Session::recent_workspaces).await
}

/// M-21.1, UC-002 steps 4–9: opens the directory the user chose in the Open
/// dialog, or says what stands in the way. With `anyway`, a directory inside
/// a workspace is opened all the same (A5 step 2, A6).
#[tauri::command]
#[specta::specta]
pub async fn ws_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    path: String,
    anyway: bool,
) -> Result<OpenAnswer, UiError> {
    let plan = read_session(&session, Session::plan_open).await?;
    // Checking and reading the directory runs without the session (A-22).
    let prepared = blocking(move || plan.prepare(&PathBuf::from(path), anyway)).await?;
    on_session(&session, &waker, move |session| {
        session.finish_open(prepared)
    })
    .await
}

/// M-21.2, UC-001 step 2: checks the directory of the Create dialog's first
/// stage; the answer is the next stage or the first one with the problem.
#[tauri::command]
#[specta::specta]
pub async fn ws_check_new(
    session: State<'_, SharedSession>,
    path: String,
) -> Result<CreateAnswer, UiError> {
    read_session(&session, move |session| session.check_new_directory(&path)).await
}

/// M-21.2, UC-001 step 5, A3: creates the workspace and opens it with its
/// Home note; a failure is said in the status bar (A5, A6).
#[tauri::command]
#[specta::specta]
pub async fn ws_create(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    request: NewWorkspaceRequest,
) -> Result<CreateAnswer, UiError> {
    let plan = on_session(&session, &waker, move |session| {
        session.plan_create(request)
    })
    .await?;
    let plan = match plan {
        Ok(plan) => plan,
        Err(answer) => return Ok(answer),
    };
    // Creating the directory and its git repository runs without the
    // session (A-22).
    let prepared = blocking(move || plan.prepare()).await?;
    on_session(&session, &waker, move |session| {
        session.finish_create(prepared)
    })
    .await
}

/// M-05.2, UC-004 step 2: opens the note `note` of the overview in a tab, or
/// activates its tab, and moves it to the top of the overview.
#[tauri::command]
#[specta::specta]
pub async fn tab_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.open_note(&NoteId::from_relative(&note));
    })
    .await
}

/// M-06.2, UC-004 A5: activates the tab of `note`.
#[tauri::command]
#[specta::specta]
pub async fn tab_activate(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.activate_tab(&NoteId::from_relative(&note));
    })
    .await
}

/// M-06.3, UC-004 A5: closes the tab of `note`, storing its changes first.
/// When they cannot be stored, the tab stays and the answer is the question
/// "Changes not stored" (UC-005 A14 step 3); with `force` ("Close anyway")
/// the tab closes all the same.
#[tauri::command]
#[specta::specta]
pub async fn tab_close(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    clipboard: State<'_, SharedClipboard>,
    note: String,
    force: bool,
) -> Result<Option<CloseQuestion>, UiError> {
    let id = NoteId::from_relative(&note);
    let question = on_session(&session, &waker, move |session| {
        session.close_tab(&id, force)
    })
    .await?;
    if question.is_none() {
        // BR-011: what copy on select kept for the tab goes with it.
        clipboard.lock().forget(&note);
    }
    Ok(question)
}

/// M-10.9, UC-005 BR-009: stores the note of the tab at once; the status
/// bar says "Stored {t}" or why it could not be stored.
#[tauri::command]
#[specta::specta]
pub async fn tab_store(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.store_tab(&NoteId::from_relative(&note));
    })
    .await
}

/// M-07.2, UC-004 BR-003: the tab of `note` shows `view` from now on; the
/// tabs are remembered with it (M-22.2).
#[tauri::command]
#[specta::specta]
pub async fn tab_set_view(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    view: EditingViewName,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.set_editing_view(&NoteId::from_relative(&note), view)
    })
    .await?
}

/// UC-004 A9: writes the copy a sync asked for, after the session was let
/// go, and puts the outcome in the status line.
fn keep_copy(session: &SharedSession, synced: Synced) -> SyncAnswer {
    if let Some(copy) = &synced.copy {
        let written = copy.write();
        session.lock().copy_kept(copy, written);
    }
    synced.answer
}

/// ADR-025: the window's edits made on revision `batch.base` (none to
/// fetch the backend's changes); the answer carries the backend's changes
/// since, the new revision and its hash.
///
/// Synchronous: it runs on the main thread in the order the
/// window sent it. The window has at most one batch of a note on its way
/// (`docs/migration/variante-b.md` section 3.3), so order would hold as an
/// async command too; synchronous keeps it simple, and the work under the
/// session is small (the edits, the transform over the backend's changes
/// since, the hash). A copy to keep (UC-004 A9, rare) is written after the
/// session is let go.
#[tauri::command]
#[specta::specta]
pub fn ed_sync(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    batch: SyncBatch,
) -> Result<SyncAnswer, UiError> {
    let synced = session
        .lock()
        .editor_sync(&NoteId::from_relative(&note), &batch);
    waker.wake();
    Ok(keep_copy(&session, synced?))
}

/// ADR-025: the window's copy of the text went apart from the backend's;
/// the window's `text` is taken.
#[tauri::command]
#[specta::specta]
pub fn ed_resync(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    text: String,
) -> Result<SyncAnswer, UiError> {
    let synced = session
        .lock()
        .editor_resync(&NoteId::from_relative(&note), &text);
    waker.wake();
    Ok(keep_copy(&session, synced?))
}

/// M-11.2, A-01: the registers `"+` and `"*` of the window's Vim keys read
/// the system clipboard (ADR-014) as plain text; `null` without text.
#[tauri::command]
#[specta::specta]
pub async fn clip_read_text(
    clipboard: State<'_, SharedClipboard>,
) -> Result<Option<String>, UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || clipboard.lock().read_text()).await
}

/// M-11.2, A-01: a yank into `"+` or `"*` puts the text on the system
/// clipboard.
#[tauri::command]
#[specta::specta]
pub async fn clip_write_text(
    clipboard: State<'_, SharedClipboard>,
    text: String,
) -> Result<(), UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || clipboard.lock().write_text(&text)).await
}

/// UC-005 A17, A18, M-08.6, M-09.14, M-10.7: what a paste in the tab of
/// `note` takes from the system clipboard (ADR-014): its text, its HTML as
/// markup of the format `rich` (the facade; `null` in the source view and
/// with Ctrl+Shift+V), and whether it holds an image alone. Over the
/// selection copy on select copied (`over_copied`) it is what the clipboard
/// held before (BR-011).
#[tauri::command]
#[specta::specta]
pub async fn clip_read(
    clipboard: State<'_, SharedClipboard>,
    note: String,
    rich: Option<FormatName>,
    over_copied: bool,
) -> Result<ClipContentView, UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || {
        let read = clipboard.lock().read_for_paste(&note, over_copied);
        clipboard_view(read.text, read.html.as_deref(), read.image, rich)
    })
    .await
}

/// BR-011, M-10.7: the selection copy on select copied in the tab of `note`
/// is gone; what the clipboard held before it is let go.
#[tauri::command]
#[specta::specta]
pub async fn clip_forget(
    clipboard: State<'_, SharedClipboard>,
    note: String,
) -> Result<(), UiError> {
    clipboard.lock().forget(&note);
    Ok(())
}

/// M-08.6, M-09.14, M-10.7, BR-011: puts copied markup on the system
/// clipboard, with the HTML of the facade's selection for other programs.
/// With `keep_before` (the first copy of a selection by copy on select),
/// what the clipboard held is kept first for pasting over the selection.
#[tauri::command]
#[specta::specta]
pub async fn clip_write(
    clipboard: State<'_, SharedClipboard>,
    note: String,
    text: String,
    html: Option<String>,
    keep_before: bool,
) -> Result<(), UiError> {
    let clipboard = Arc::clone(&clipboard);
    blocking(move || {
        let mut clipboard = clipboard.lock();
        if keep_before {
            clipboard.keep_before(&note);
        }
        clipboard.write(&text, html.as_deref());
    })
    .await
}

/// UC-005 A18, BR-014, M-10.6: stores the image the clipboard holds (over
/// the copied selection: the one it held before, as `clip_read`) as a PNG
/// next to the note of `note`; the status says "Pasted image stored as
/// {name}" or why not. The name to insert, `null` when nothing was stored.
/// Encoding and writing run without the session.
#[tauri::command]
#[specta::specta]
pub async fn clip_store_image(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    clipboard: State<'_, SharedClipboard>,
    note: String,
    over_copied: bool,
) -> Result<Option<String>, UiError> {
    let id = NoteId::from_relative(&note);
    let plan = read_session(&session, move |session| session.embed_plan(&id)).await??;
    let clipboard = Arc::clone(&clipboard);
    let stored = blocking(move || {
        let image = clipboard.lock().take_pasted_image(&note, over_copied)?;
        Some(plan.store_image(image.width, image.height, &image.rgba, SystemTime::now()))
    })
    .await?;
    let Some(stored) = stored else {
        // The clipboard changed meanwhile and holds no image any more.
        return Ok(None);
    };
    on_session(&session, &waker, move |session| {
        session.finish_pasted_image(stored)
    })
    .await
}

/// UC-005 A2, A9 to A11, A19, M-10.4: the answer of the image dialog: puts
/// the file `path` into the note of `note`, copied next to it from outside
/// the workspace; `allow_large` after "Copy" for a file over 10 MB. The
/// file is copied without the session.
#[tauri::command]
#[specta::specta]
pub async fn ed_embed(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    path: String,
    allow_large: bool,
) -> Result<EmbedAnswer, UiError> {
    let id = NoteId::from_relative(&note);
    let plan = read_session(&session, move |session| session.embed_plan(&id)).await??;
    let done = blocking(move || plan.embed(&path, allow_large)).await?;
    let answers = on_session(&session, &waker, move |session| session.finish_embed(done)).await?;
    Ok(answers.into_iter().next().unwrap_or(EmbedAnswer::Refused))
}

/// UC-005 A9 to A13, M-10.5: what was dropped on the window (`paths`), for
/// the tab of `note`, one answer per item with a space between them; the
/// files are copied without the session.
#[tauri::command]
#[specta::specta]
pub async fn ed_drop(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    paths: Vec<String>,
) -> Result<Vec<EmbedAnswer>, UiError> {
    let id = NoteId::from_relative(&note);
    let plan = read_session(&session, move |session| session.embed_plan(&id)).await??;
    let done = blocking(move || plan.drop_items(&paths)).await?;
    on_session(&session, &waker, move |session| session.finish_embed(done)).await
}

/// UC-005 A9, M-10.5: files are dragged over the window, left it, or were
/// dropped with no note shown; the status bar says so.
#[tauri::command]
#[specta::specta]
pub async fn ed_drag(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    state: DragName,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| session.drag(state)).await
}

/// M-10.3, UC-005 A2: the link dialog chose the note `target`: the link to
/// it from the note of `note`, relative, with its title.
#[tauri::command]
#[specta::specta]
pub async fn ed_note_link(
    session: State<'_, SharedSession>,
    note: String,
    target: String,
) -> Result<Insertion, UiError> {
    read_session(&session, move |session| {
        session.note_link(
            &NoteId::from_relative(&note),
            &NoteId::from_relative(&target),
        )
    })
    .await?
}

/// UC-004 A2 to A4, M-07.3: follows the link at `at` (web units) of the
/// text of the tab `note`, clicked while the window was in step with the
/// backend's revision `rev` (its edits sent first). The session finds the
/// link in its own text, so the window never names what is opened; a
/// place of another revision follows nothing. A web address (after "Open
/// web address" agreed) or a file is opened with the system's program once
/// the session is let go (plan section 8: the `open` crate, no plugin); a
/// failure is said in the status bar.
#[tauri::command]
#[specta::specta]
pub async fn ed_follow_link(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    at: u32,
    rev: u32,
) -> Result<LinkAnswer, UiError> {
    let id = NoteId::from_relative(&note);
    let followed = on_session(&session, &waker, move |session| {
        session.follow_link(&id, at, rev)
    })
    .await??;
    if let Some(target) = followed.open {
        open_with_system(&session, &waker, target).await?;
    }
    Ok(followed.answer)
}

/// UC-004 A4: the answer to "Open web address": with `open`, the address
/// asked about opens in the browser, and web addresses are not asked about
/// again until Underline closes.
#[tauri::command]
#[specta::specta]
pub async fn link_open_web(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    open: bool,
) -> Result<(), UiError> {
    let address = on_session(&session, &waker, move |session| session.answer_web(open)).await?;
    if let Some(address) = address {
        open_with_system(&session, &waker, address).await?;
    }
    Ok(())
}

/// UC-004 BR-007: opens `target` with the program the system uses for it,
/// without the session (`open::that_detached`, as egui); a failure becomes
/// the status.
async fn open_with_system(
    session: &State<'_, SharedSession>,
    waker: &State<'_, Waker>,
    target: String,
) -> Result<(), UiError> {
    let opened = blocking({
        let target = target.clone();
        move || open::that_detached(&target).map_err(|error| error.to_string())
    })
    .await?;
    if let Err(error) = opened {
        on_session(session, waker, move |session| {
            session.open_failed(&target, &error);
        })
        .await?;
    }
    Ok(())
}

/// M-08.1, M-08.2: the source view shows the lines `from..to` (from 0, the
/// end not included); their colours come as the event `ed-highlight`.
#[tauri::command]
#[specta::specta]
pub fn ed_viewport(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    from: u32,
    to: u32,
) -> Result<(), UiError> {
    let set = session
        .lock()
        .editor_viewport(&NoteId::from_relative(&note), from, to);
    waker.wake();
    set
}

/// M-21.4, UC-005 A14 step 3: "Close anyway": Underline ends without the
/// changes that could not be stored. What can be stored is stored once
/// more, and the app state is written, when the app exits.
#[tauri::command]
#[specta::specta]
pub fn app_close_anyway(closer: State<'_, Closer>) -> Result<(), UiError> {
    closer.close_anyway();
    Ok(())
}

/// Ends the app (M-21.4); `main.rs` makes it from the app handle, so the
/// command stays free of the runtime type and testable.
pub struct Closer {
    exit: Box<dyn Fn() + Send + Sync>,
    anyway: AtomicBool,
    /// Everything was stored; the end the app asks for next goes through.
    confirmed: AtomicBool,
    /// ADR-025 section 3.9: the close handshake waiting for the window, by
    /// its number (0: none).
    handshake: AtomicU64,
    next: AtomicU64,
    /// What closing does after the window sent its edits
    /// ([`close_after_flush`] with the app's handle).
    after_flush: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Closer {
    /// A closer that ends the app with `exit`.
    pub fn new(exit: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            exit: Box::new(exit),
            anyway: AtomicBool::new(false),
            confirmed: AtomicBool::new(false),
            handshake: AtomicU64::new(0),
            next: AtomicU64::new(0),
            after_flush: OnceLock::new(),
        }
    }

    /// Sets what closing does after the window sent its edits.
    pub fn set_after_flush(&self, after: impl Fn() + Send + Sync + 'static) {
        let _ = self.after_flush.set(Box::new(after));
    }

    /// The window answered the close handshake: with `sent`, closing goes
    /// on; without, the window asks the user ("Changes not stored") and the
    /// handshake ends. An answer is not tied to the handshake's number: one
    /// that comes after the fallback went on (or without a handshake)
    /// changes nothing, so a close never runs twice.
    pub fn flushed(&self, sent: bool) {
        if self.end_handshake(None) && sent {
            if let Some(after) = self.after_flush.get() {
                after();
            }
        }
    }

    /// "Close anyway": ends the app without asking again.
    pub fn close_anyway(&self) {
        self.anyway.store(true, Ordering::SeqCst);
        (self.exit)();
    }

    /// Everything was stored: ends the app without asking again.
    pub fn close_now(&self) {
        self.confirmed.store(true, Ordering::SeqCst);
        (self.exit)();
    }

    /// Whether the user chose "Close anyway".
    pub fn closing_anyway(&self) -> bool {
        self.anyway.load(Ordering::SeqCst)
    }

    /// Whether the app ends without asking (again).
    pub fn closing(&self) -> bool {
        self.closing_anyway() || self.confirmed.load(Ordering::SeqCst)
    }

    /// ADR-025: a close waits for the window to send its edits; the number
    /// of this wait.
    pub fn begin_handshake(&self) -> u64 {
        let number = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.handshake.store(number, Ordering::SeqCst);
        number
    }

    /// Ends the wait `number` (or any with `None`); whether it was still
    /// waiting, so only one of the window and the fallback goes on.
    pub fn end_handshake(&self, number: Option<u64>) -> bool {
        match number {
            Some(number) => self
                .handshake
                .compare_exchange(number, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            None => self.handshake.swap(0, Ordering::SeqCst) != 0,
        }
    }
}

/// M-21.4, UC-005 BR-001, ADR-025 section 3.9: after the window sent its
/// edits (or did not answer in time), every change is stored; when that
/// fails, the window asks "Changes not stored" (A14 step 3), else the app
/// ends.
pub fn close_after_flush<R: Runtime>(app: &AppHandle<R>) {
    let question = app.state::<SharedSession>().lock().request_close();
    app.state::<Waker>().wake();
    match question {
        None => app.state::<Closer>().close_now(),
        Some(question) => {
            if let Err(error) = CloseQuestionEvent(question).emit(app) {
                eprintln!("The question before closing could not be sent: {error}");
            }
        }
    }
}

/// ADR-025 section 3.9: the window's answer to `ed-flush` with `closing`:
/// with `sent`, it sent the edits of every note, and Underline closes now
/// after storing everything (or asks why it cannot); without, it could not
/// send them and asks the user itself ("Changes not stored"). Synchronous:
/// it runs on the main thread, which also ends the app.
#[tauri::command]
#[specta::specta]
pub fn app_request_close(closer: State<'_, Closer>, sent: bool) -> Result<(), UiError> {
    closer.flushed(sent);
    Ok(())
}

/// ADR-025: the backend's text of the tab of `note` as the window's editor
/// core starts from it, with its revision and the tab's cursor.
#[tauri::command]
#[specta::specta]
pub fn ed_mirror(session: State<'_, SharedSession>, note: String) -> Result<MirrorView, UiError> {
    session.lock().editor_mirror(&NoteId::from_relative(&note))
}

/// The system clipboard, shared by the commands (ADR-014).
pub type SharedClipboard = Arc<Mutex<SystemClipboard>>;

/// M-03.5: checks the wiki project URL of the Workspace menu; the status
/// bar says the repository or why the URL does not fit. Nothing is stored.
#[tauri::command]
#[specta::specta]
pub fn wiki_check(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    url: String,
) -> Result<(), UiError> {
    session.lock().check_wiki(&url);
    waker.wake();
    Ok(())
}

/// M-14.1, UC-003 step 2: what "New note" starts with: the folder of the
/// active note and the default format; `null` without a workspace.
#[tauri::command]
#[specta::specta]
pub async fn note_new_start(
    session: State<'_, SharedSession>,
) -> Result<Option<NewNoteStart>, UiError> {
    read_session(&session, Session::new_note_start).await
}

/// M-14.1, UC-003 step 2: the file name the note will get ("File: …"),
/// while the user types; `null` while it cannot be created so. Checking a
/// folder may run git, so it runs on the blocking pool.
#[tauri::command]
#[specta::specta]
pub async fn note_plan(
    session: State<'_, SharedSession>,
    request: NewNoteRequest,
) -> Result<Option<String>, UiError> {
    read_session(&session, move |session| session.plan_new_note(&request)).await
}

/// M-14.1, UC-003 steps 4–6: creates the note and opens it; the answer puts
/// the cursor at its end, keeps the dialog with a problem, or closes it
/// (a failure is in the status bar).
#[tauri::command]
#[specta::specta]
pub async fn note_create(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    request: NewNoteRequest,
) -> Result<CreateNoteAnswer, UiError> {
    on_session(&session, &waker, move |session| {
        session.create_note(&request)
    })
    .await
}

/// M-14.2, UC-006 step 1: what "Rename note" starts with for `note`;
/// `null` when it is refused (the status bar says why).
#[tauri::command]
#[specta::specta]
pub async fn note_rename_start(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<Option<RenameStart>, UiError> {
    on_session(&session, &waker, move |session| {
        session.rename_start(&NoteId::from_relative(&note))
    })
    .await
}

/// M-14.2, UC-006 step 2: the file the note will be in ("File: …"), while
/// the user types; `null` while it cannot be renamed so.
#[tauri::command]
#[specta::specta]
pub async fn note_plan_rename(
    session: State<'_, SharedSession>,
    note: String,
    title: String,
    folder: String,
) -> Result<Option<String>, UiError> {
    read_session(&session, move |session| {
        session.plan_rename(&NoteId::from_relative(&note), &title, &folder)
    })
    .await
}

/// M-14.2, UC-006 steps 4–7: renames the note, after the window sent its
/// edits (`flushed`); the links follow, in open tabs too. What copy on
/// select kept for the tab goes with it.
#[tauri::command]
#[specta::specta]
pub async fn note_rename(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    clipboard: State<'_, SharedClipboard>,
    note: String,
    title: String,
    folder: String,
) -> Result<RenameAnswer, UiError> {
    let answer = on_session(&session, &waker, move |session| {
        session.rename_note(&NoteId::from_relative(&note), &title, &folder)
    })
    .await?;
    if let RenameAnswer::Renamed { from, to } = &answer {
        clipboard.lock().rename(from, to);
    }
    Ok(answer)
}

/// M-14.3, UC-007 steps 1–2: the question before deleting `note`, with the
/// links that lead to it; `null` when it is refused (the status bar says
/// why). Counting the links reads the notes.
#[tauri::command]
#[specta::specta]
pub async fn note_delete_start(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<Option<DeleteQuestion>, UiError> {
    on_session(&session, &waker, move |session| {
        session.delete_start(&NoteId::from_relative(&note))
    })
    .await
}

/// M-14.3, UC-007 steps 4–5, A8: deletes `note`, after the window sent its
/// edits (`flushed`); with `for_good` it is not kept. Its tab closes, and
/// what copy on select kept for it goes.
#[tauri::command]
#[specta::specta]
pub async fn note_delete(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    clipboard: State<'_, SharedClipboard>,
    note: String,
    for_good: bool,
) -> Result<DeleteAnswer, UiError> {
    let id = NoteId::from_relative(&note);
    let (answer, still_open) = on_session(&session, &waker, move |session| {
        let answer = session.delete_note(&id, for_good);
        (answer, session.has_tab(&id))
    })
    .await?;
    if !still_open {
        clipboard.lock().forget(&note);
    }
    Ok(answer)
}

/// M-14.3, UC-007 A4: "Undo" of the report "Deleted {t}"; the answer asks
/// for another place when the note's is taken (A6).
#[tauri::command]
#[specta::specta]
pub async fn note_undo_delete(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
) -> Result<RestoreAnswer, UiError> {
    on_session(&session, &waker, Session::undo_delete).await
}

/// M-05.6, UC-007 A5: "Restore" of the deleted note `key`; the answer asks
/// for another place when its own is taken (A6).
#[tauri::command]
#[specta::specta]
pub async fn ws_restore(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    key: String,
) -> Result<RestoreAnswer, UiError> {
    on_session(&session, &waker, move |session| {
        session.restore_deleted(&key)
    })
    .await
}

/// M-14.4, UC-007 A6: "Restore" in "Restore note", as `title` in `folder`.
#[tauri::command]
#[specta::specta]
pub async fn ws_restore_as(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    title: String,
    folder: String,
) -> Result<RestoreAnswer, UiError> {
    on_session(&session, &waker, move |session| {
        session.restore_deleted_as(&title, &folder)
    })
    .await
}

/// M-14.4, UC-007 A7: removes the deleted note `key` for good, or all of
/// them with `null`.
#[tauri::command]
#[specta::specta]
pub async fn ws_purge(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    key: Option<String>,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| {
        session.purge_deleted(key.as_deref());
    })
    .await
}

/// M-13.1, UC-008 steps 1–2, A2: the search overlay holds `text`; the
/// search of the workspace starts again on its own thread, and its result
/// comes as the event `search`. Only spaces end the search.
#[tauri::command]
#[specta::specta]
pub async fn search_set(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    text: String,
) -> Result<(), UiError> {
    on_session(&session, &waker, move |session| session.search_set(&text)).await
}

/// M-13.1, UC-008 step 2: more found notes of the result `serial`, from the
/// `from`th on; `null` when a newer result replaced it.
#[tauri::command]
#[specta::specta]
pub async fn search_page(
    session: State<'_, SharedSession>,
    serial: u32,
    from: u32,
) -> Result<Option<Vec<FoundNoteView>>, UiError> {
    read_session(&session, move |session| session.search_page(serial, from)).await
}

/// M-13.3, UC-008 step 4: opens the found note `note` as the overview does;
/// with `place` (bytes of the text searched), the word to select in the
/// backend's text, which the window's edits reached first (`flushed`,
/// ADR-025). `null` when there is none or the note did not open (the
/// status says why).
#[tauri::command]
#[specta::specta]
pub async fn search_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    place: Option<FoundPlace>,
) -> Result<Option<SearchOpened>, UiError> {
    let id = NoteId::from_relative(&note);
    on_session(&session, &waker, move |session| {
        session.search_open(&id, place)
    })
    .await?
}

/// UC-009 step 2, ADR-016: the system's save dialog for an export, managed
/// by the app so tests can answer it themselves.
#[derive(Clone)]
pub struct SaveDialog(Arc<dyn Fn(&ExportDialogPlan) -> DialogOutcome + Send + Sync>);

impl SaveDialog {
    /// A dialog that answers with `ask`.
    pub fn new(ask: impl Fn(&ExportDialogPlan) -> DialogOutcome + Send + Sync + 'static) -> Self {
        Self(Arc::new(ask))
    }
}

/// UC-009 step 1, BR-008, M-15.1: Ctrl+Shift+E or "Export…" of the tab
/// menu: "Export {t} as:" with the kinds of export of the note. `null`
/// without a workspace or for a note in conflict (the status says so).
#[tauri::command]
#[specta::specta]
pub async fn export_start(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
) -> Result<Option<ExportStart>, UiError> {
    let id = NoteId::from_relative(&note);
    on_session(&session, &waker, move |session| session.export_start(&id)).await
}

/// UC-009 steps 2–3, A1, M-15.2, ADR-016: the system's save dialog "Export
/// as {label}" for `note` as `kind`, starting in the folder of the last
/// export (else the home folder) with the note's name. It runs on the
/// blocking pool without the session, so the window and the tick go on
/// while it is open. A chosen file is kept by the backend; the window then
/// runs the export with `export_run` once its edits are sent. Without a
/// dialog, the answer asks for the path in a field.
#[tauri::command]
#[specta::specta]
pub async fn export_choose(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    dialog: State<'_, SaveDialog>,
    note: String,
    kind: ExportKindName,
) -> Result<ExportPlaceAnswer, UiError> {
    let id = NoteId::from_relative(&note);
    let plan = {
        let id = id.clone();
        on_session(&session, &waker, move |session| {
            session.export_dialog(&id, kind)
        })
        .await?
    };
    let Some(plan) = plan else {
        return Ok(ExportPlaceAnswer::Done);
    };
    let ask = Arc::clone(&dialog.inner().0);
    // A dialog that ended without an answer counts as cancelled, so the
    // next one may open (one at a time).
    let outcome = blocking(move || ask(&plan))
        .await
        .unwrap_or(DialogOutcome::Cancelled);
    on_session(&session, &waker, move |session| {
        session.export_dialog_answer(&id, kind, outcome)
    })
    .await
}

/// UC-009 steps 4–5, A2 to A6, M-15.3 to M-15.5: exports `note` as `kind`
/// to the file chosen in the dialog (`typed` is `null`) or typed in the
/// field. The window's edits reached the backend first (`flushed`,
/// ADR-025), so the tab's text with changes not yet saved is exported. The
/// status says "Exporting {t}…" at once; the export is made and written
/// on the blocking pool without the session (a PDF may take a while), then
/// the status reports it.
#[tauri::command]
#[specta::specta]
pub async fn export_run(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    note: String,
    kind: ExportKindName,
    typed: Option<TypedExportPath>,
) -> Result<ExportAnswer, UiError> {
    let id = NoteId::from_relative(&note);
    let typed_run = typed.is_some();
    let job = on_session(&session, &waker, move |session| {
        session.export_prepare(&id, kind, typed)
    })
    .await?;
    let job = match job {
        Ok(job) => job,
        Err(answer) => return Ok(answer),
    };
    let (id, title) = (job.id(), job.title().to_owned());
    // Review F11: a job whose thread ended still ends "Exporting {t}…".
    let outcome = blocking(move || job.run())
        .await
        .unwrap_or_else(|error| ExportJob::aborted(id, title, &error.message));
    let failure = on_session(&session, &waker, move |session| {
        session.finish_export(outcome)
    })
    .await?;
    // Review F11: a typed path comes back with why it failed.
    Ok(match failure {
        Some(problem) if typed_run => ExportAnswer::Problem { problem },
        _ => ExportAnswer::Done,
    })
}

/// M-04.8, UC-009 step 5: "Open" of the status bar opens the export
/// `index` it reports (from 0) with the system's program; nothing once
/// another report took its place.
#[tauri::command]
#[specta::specta]
pub async fn export_open(
    session: State<'_, SharedSession>,
    waker: State<'_, Waker>,
    index: u32,
) -> Result<(), UiError> {
    let target = read_session(&session, move |session| session.export_target(index)).await?;
    if let Some(target) = target {
        open_with_system(&session, &waker, target).await?;
    }
    Ok(())
}

/// M-04.8, M-15.6, UC-009 A2 step 3: "Show": the note and the lines of the
/// blocks the export `index` the status reports only approximated, for the
/// window "Approximated blocks"; `null` once another report took its place.
#[tauri::command]
#[specta::specta]
pub async fn export_approximated(
    session: State<'_, SharedSession>,
    index: u32,
) -> Result<Option<ExportShown>, UiError> {
    read_session(&session, move |session| session.export_approximated(index)).await
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::Receiver;
    use std::sync::Arc;
    use std::time::Instant;

    use parking_lot::Mutex;
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
    };
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};
    use underline_core::{StateStore, SETTINGS_FILE_NAME};
    use underline_session::{Session, SettingsState};

    use super::{Closer, SaveDialog, SharedClipboard, SharedSession};
    use crate::bindings;
    use crate::clipboard::{ClipContent, ClipImage, SystemClipboard};
    use crate::ticker::Waker;
    use underline_session::{DialogOutcome, ExportKindName};

    /// Every command, by its Rust name; keep in step with `bindings::builder`
    /// and `build.rs`.
    const COMMANDS: &[&str] = &[
        "app_start",
        "settings_quick",
        "settings_window",
        "settings_set",
        "wiki_check",
        "ws_start",
        "ws_recent",
        "ws_open",
        "ws_check_new",
        "ws_create",
        "tab_open",
        "tab_activate",
        "tab_close",
        "tab_store",
        "tab_set_view",
        "ed_viewport",
        "clip_read_text",
        "clip_write_text",
        "clip_read",
        "clip_write",
        "clip_store_image",
        "clip_forget",
        "ed_embed",
        "ed_drop",
        "ed_drag",
        "ed_note_link",
        "app_close_anyway",
        "ed_sync",
        "ed_resync",
        "ed_mirror",
        "app_request_close",
        "ed_follow_link",
        "link_open_web",
        "note_new_start",
        "note_plan",
        "note_create",
        "note_rename_start",
        "note_plan_rename",
        "note_rename",
        "note_delete_start",
        "note_delete",
        "note_undo_delete",
        "ws_restore",
        "ws_restore_as",
        "ws_purge",
        "search_set",
        "search_page",
        "search_open",
        "export_start",
        "export_choose",
        "export_run",
        "export_open",
        "export_approximated",
    ];

    /// A mock app whose session was started with `given` and keeps its user
    /// settings in `dir`, with its window and the receiving end of the wakes.
    struct Mock {
        app: App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
        wakes: Receiver<()>,
    }

    impl Mock {
        fn new(given: Option<PathBuf>, dir: &Path) -> Self {
            Self::with_dialog(given, dir, SaveDialog::new(|_| DialogOutcome::Cancelled))
        }

        /// A mock whose save dialog answers with `dialog` (UC-009 step 2).
        fn with_dialog(given: Option<PathBuf>, dir: &Path, dialog: SaveDialog) -> Self {
            let session = Session::new(
                given,
                StateStore::in_memory(),
                None,
                SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
            );
            let (waker, wakes) = Waker::new();
            let app = mock_builder()
                .manage(Arc::new(Mutex::new(session)))
                .manage(waker)
                .manage(Closer::new(|| {}))
                .manage(dialog)
                .manage(SharedClipboard::new(Mutex::new(
                    SystemClipboard::in_memory(),
                )))
                .invoke_handler(bindings::builder().invoke_handler())
                .build(mock_context(noop_assets()))
                .expect("the mock app builds");
            let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .expect("the mock window opens");
            Self {
                app,
                webview,
                wakes,
            }
        }

        fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().expect("a valid URL"),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            };
            get_ipc_response(&self.webview, request)
                .map(|body| body.deserialize::<Value>().expect("a JSON response"))
        }

        fn status(&self) -> String {
            self.app.state::<SharedSession>().lock().status().to_owned()
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-tauri-commands-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn m01_1_app_start_passes_the_given_dir_along() {
        let dir = temp_dir("start-given");
        let mock = Mock::new(Some(PathBuf::from("notes")), &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["givenDir"], json!("notes"));
        assert!(["macos", "windows", "linux"].contains(&response["platform"].as_str().unwrap()));
    }

    #[test]
    fn m01_1_app_start_without_a_dir() {
        let dir = temp_dir("start-none");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["givenDir"], Value::Null);
        assert_eq!(response["status"], json!(""));
    }

    #[test]
    fn m01_6_app_start_gives_the_settings_with_the_palette() {
        let dir = temp_dir("start-palette");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("app_start", json!({})).unwrap();

        assert_eq!(response["settings"]["theme"], json!("Light"));
        assert_eq!(response["settings"]["showMenuBar"], json!(false));
        assert_eq!(
            response["settings"]["palette"]["background"],
            json!("#ffffff")
        );
        assert_eq!(response["themes"].as_array().unwrap().len(), 11);
    }

    #[test]
    fn uc010_m03_6_settings_quick_stores_the_theme_and_wakes_the_tick() {
        let dir = temp_dir("quick");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("settings_quick", json!({ "key": "theme", "value": "Nord" }));

        assert_eq!(response, Ok(Value::Null));
        assert_eq!(
            std::fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "theme: Nord\n"
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
        let start = mock.invoke("app_start", json!({})).unwrap();
        assert_eq!(start["settings"]["palette"]["theme"], json!("Nord"));
    }

    #[test]
    fn uc010_settings_quick_refuses_an_unknown_key() {
        let dir = temp_dir("quick-unknown");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("settings_quick", json!({ "key": "colour", "value": "red" }));

        assert!(response.is_err());
        assert!(!dir.join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn uc010_m20_2_settings_window_shows_for_me_without_a_workspace() {
        let dir = temp_dir("window");
        let mock = Mock::new(None, &dir);

        let view = mock.invoke("settings_window", json!({})).unwrap();

        assert_eq!(view["workspace"], json!(false));
        assert_eq!(view["thisWorkspace"], json!([]));
        assert_eq!(view["me"][0]["key"], json!("theme"));
        assert_eq!(view["me"][0]["label"], json!("Theme"));
        assert_eq!(view["me"][0]["control"]["kind"], json!("choice"));
        assert_eq!(view["me"][0]["control"]["shown"], json!("Light"));
        assert_eq!(view["me"][0]["note"], Value::Null);
        assert!(!dir.join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn uc010_m20_5_settings_set_stores_the_change_and_wakes_the_tick() {
        let dir = temp_dir("set");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke(
            "settings_set",
            json!({ "scope": "me", "key": "git_sync_interval", "value": "15" }),
        );

        let answer = response.unwrap();
        assert_eq!(answer["view"]["me"][6]["control"]["value"], json!(15));
        assert_eq!(answer["problem"], Value::Null);
        assert_eq!(
            std::fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "git_sync_interval: 15\n"
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
        let view = mock.invoke("settings_window", json!({})).unwrap();
        assert_eq!(view["me"][6]["control"]["value"], json!(15));
    }

    #[test]
    fn uc010_a2_settings_set_takes_null_as_no_value() {
        let dir = temp_dir("set-null");
        std::fs::write(dir.join(SETTINGS_FILE_NAME), "theme: Nord\n").unwrap();
        let mock = Mock::new(None, &dir);

        let response = mock.invoke(
            "settings_set",
            json!({ "scope": "me", "key": "theme", "value": null }),
        );

        assert_eq!(
            response.unwrap()["view"]["me"][0]["control"]["shown"],
            json!("Light")
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            ""
        );
    }

    #[test]
    fn uc010_settings_set_refuses_an_unknown_scope() {
        let dir = temp_dir("set-scope");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke(
            "settings_set",
            json!({ "scope": "everyone", "key": "theme", "value": "Nord" }),
        );

        assert!(response.is_err());
        assert!(!dir.join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn m03_5_wiki_check_sets_the_status() {
        let dir = temp_dir("wiki");
        let mock = Mock::new(None, &dir);

        let response = mock.invoke("wiki_check", json!({ "url": "https://gitlab.com/a/b" }));

        assert_eq!(response, Ok(Value::Null));
        assert_eq!(
            mock.status(),
            "Wiki repository: https://gitlab.com/a/b.wiki.git"
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
    }

    /// A workspace folder `dir/name` with the Markdown notes `notes`.
    fn workspace_with(dir: &Path, name: &str, notes: &[&str]) -> PathBuf {
        let root = dir.join(name);
        std::fs::create_dir_all(&root).unwrap();
        for note in notes {
            std::fs::write(root.join(format!("{note}.md")), format!("# {note}\n")).unwrap();
        }
        root
    }

    #[test]
    fn uc002_a1_m01_1_ws_start_opens_the_given_workspace() {
        let dir = temp_dir("ws-start");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        let mock = Mock::new(Some(root), &dir);

        let response = mock.invoke("ws_start", json!({})).unwrap();

        assert_eq!(response["answer"], json!({ "kind": "done" }));
        assert_eq!(response["snapshot"]["workspace"]["name"], json!("notes"));
        assert_eq!(
            response["snapshot"]["overview"],
            json!([
                { "note": "Home", "label": "Home", "format": "Markdown" },
                { "note": "Ideas", "label": "Ideas", "format": "Markdown" }
            ])
        );
        assert_eq!(response["snapshot"]["tabs"]["active"], json!("Home"));
        assert_eq!(
            response["snapshot"]["tabs"]["todoList"],
            json!({ "open": false, "active": false })
        );
        assert_eq!(mock.wakes.try_recv(), Ok(()));
    }

    #[test]
    fn uc002_a3_m21_1_ws_open_offers_to_create_a_workspace_there() {
        let dir = temp_dir("ws-open-missing");
        let mock = Mock::new(None, &dir);
        let missing = dir.join("gone").display().to_string();

        let response = mock
            .invoke("ws_open", json!({ "path": missing, "anyway": false }))
            .unwrap();

        assert_eq!(response["kind"], json!("problem"));
        assert_eq!(
            response["offer"],
            json!({ "kind": "create", "directory": missing })
        );
        assert_eq!(response["recent"], json!([]));
    }

    #[test]
    fn uc001_m21_2_ws_check_new_and_ws_create_make_a_workspace() {
        let dir = temp_dir("ws-create");
        let mock = Mock::new(None, &dir);
        let path = dir.join("fresh").display().to_string();

        let checked = mock
            .invoke("ws_check_new", json!({ "path": path }))
            .unwrap();
        assert_eq!(checked, json!({ "stage": "options", "directory": path }));

        let created = mock
            .invoke(
                "ws_create",
                json!({ "request": {
                    "directory": path, "format": "Markdown", "git": false, "remote": ""
                } }),
            )
            .unwrap();

        assert_eq!(created, json!({ "stage": "done" }));
        assert!(dir.join("fresh").join("Home.md").is_file());
        assert!(mock.status().starts_with("Workspace created in "));
    }

    #[test]
    fn m05_2_m06_tab_commands_open_activate_and_close() {
        let dir = temp_dir("tabs");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("tab_open", json!({ "note": "Ideas" })).unwrap();
        mock.invoke("tab_activate", json!({ "note": "Home" }))
            .unwrap();
        let closed = mock
            .invoke("tab_close", json!({ "note": "Ideas", "force": false }))
            .unwrap();
        assert_eq!(closed, Value::Null);

        let snapshot = mock.app.state::<SharedSession>().lock().snapshot();
        let tabs: Vec<_> = snapshot.tabs.tabs.iter().map(|t| t.note.as_str()).collect();
        assert_eq!(tabs, ["Home"]);
        assert_eq!(snapshot.tabs.active.as_deref(), Some("Home"));
    }

    /// UC-003, UC-006, UC-007, M-14: a note is created, renamed, deleted,
    /// put back by the undo, deleted again and removed for good, with the
    /// commands' arguments in camelCase as the window sends them.
    #[test]
    fn uc003_uc006_uc007_m14_note_commands_create_rename_delete_and_restore() {
        let dir = temp_dir("notes");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();
        let request = json!({ "request": { "title": "Plan", "folder": "", "format": "Markdown" } });

        let start = mock.invoke("note_new_start", json!({})).unwrap();
        assert_eq!(start, json!({ "folder": "", "format": "Markdown" }));
        assert_eq!(
            mock.invoke("note_plan", request.clone()).unwrap(),
            json!("Plan.md")
        );
        let created = mock.invoke("note_create", request).unwrap();
        assert_eq!(created["kind"], json!("created"));
        assert_eq!(created["note"], json!("Plan"));

        let renamed = mock
            .invoke(
                "note_rename",
                json!({ "note": "Plan", "title": "Roadmap", "folder": "" }),
            )
            .unwrap();
        assert_eq!(
            renamed,
            json!({ "kind": "renamed", "from": "Plan", "to": "Roadmap" })
        );

        let question = mock
            .invoke("note_delete_start", json!({ "note": "Roadmap" }))
            .unwrap();
        assert_eq!(
            question["links"],
            json!("No links in other notes lead to it.")
        );
        let deleted = mock
            .invoke(
                "note_delete",
                json!({ "note": "Roadmap", "forGood": false }),
            )
            .unwrap();
        assert_eq!(deleted, json!({ "kind": "done" }));
        assert_eq!(mock.status(), "Deleted Roadmap");
        assert_eq!(
            mock.invoke("note_undo_delete", json!({})).unwrap(),
            json!({ "kind": "restored", "note": "Roadmap" })
        );
        assert!(root.join("Roadmap.md").is_file());

        mock.invoke(
            "note_delete",
            json!({ "note": "Roadmap", "forGood": false }),
        )
        .unwrap();
        mock.invoke("ws_purge", json!({ "key": null })).unwrap();
        assert_eq!(mock.status(), "Removed 1 deleted note for good");
    }

    /// ADR-025, UC-005, M-10.9: the window's core starts from the mirror
    /// (`ed_mirror`), its edits arrive with `ed_sync`, and the note is stored
    /// and closed; a closed tab has no mirror.
    #[test]
    fn adr025_uc005_m10_9_editor_commands_sync_store_and_close() {
        let dir = temp_dir("editor");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        let mirror = mock.invoke("ed_mirror", json!({ "note": "Home" })).unwrap();
        assert_eq!(mirror["text"], json!("# Home\n"));
        let answer = mock
            .invoke(
                "ed_sync",
                json!({ "note": "Home", "batch": {
                    "base": mirror["rev"], "seq": 1,
                    "edits": [{ "at": 0, "remove": 0, "insert": "🙂" }]
                } }),
            )
            .unwrap();
        assert_eq!(answer["resync"], json!(false));
        assert_eq!(answer["remote"], json!([]));
        mock.invoke("ed_viewport", json!({ "note": "Home", "from": 0, "to": 5 }))
            .unwrap();

        mock.invoke("tab_store", json!({ "note": "Home" })).unwrap();

        assert_eq!(mock.status(), "Stored Home");
        assert_eq!(
            std::fs::read_to_string(root.join("Home.md")).unwrap(),
            "🙂# Home\n"
        );
        let closed = mock
            .invoke("tab_close", json!({ "note": "Home", "force": false }))
            .unwrap();
        assert_eq!(closed, Value::Null);
        assert!(mock.invoke("ed_mirror", json!({ "note": "Home" })).is_err());
    }

    /// M-11.2, M-08.6, ADR-014: the window's Vim registers and its clipboard
    /// keys read and write the system clipboard through the backend.
    #[test]
    fn m11_2_m08_6_the_clipboard_commands_read_and_write_it() {
        let dir = temp_dir("clipboard");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("clip_write_text", json!({ "text": "yanked" }))
            .unwrap();

        assert_eq!(
            mock.invoke("clip_read_text", json!({})).unwrap(),
            json!("yanked")
        );
        assert_eq!(
            mock.app
                .state::<SharedClipboard>()
                .lock()
                .read_text()
                .as_deref(),
            Some("yanked")
        );
    }

    /// UC-005 A17, A18, BR-011, M-09.14, M-10.6, M-10.7: the clipboard
    /// commands of feature F6: markup with HTML is written and read back,
    /// the HTML as markup for the facade; copy on select keeps the former
    /// content; an image alone is stored next to the note.
    #[test]
    fn uc005_a18_m10_6_m10_7_the_clipboard_with_html_images_and_copy_on_select() {
        let dir = temp_dir("clipboard-f6");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        let write = |text: &str, html: Value, keep: bool| {
            mock.invoke(
                "clip_write",
                json!({ "note": "Home", "text": text, "html": html, "keepBefore": keep }),
            )
            .unwrap();
        };
        let read = |rich: Value, over: bool| {
            mock.invoke(
                "clip_read",
                json!({ "note": "Home", "rich": rich, "overCopied": over }),
            )
            .unwrap()
        };
        write("**a**", json!("<p><b>a</b></p>"), false);
        assert_eq!(
            read(json!("Markdown"), false),
            json!({ "text": "**a**", "markup": "**a**", "image": false })
        );
        assert_eq!(
            read(Value::Null, false),
            json!({ "text": "**a**", "markup": null, "image": false })
        );
        write("selected", Value::Null, true);
        assert_eq!(read(Value::Null, true)["text"], json!("**a**"));
        assert_eq!(read(Value::Null, false)["text"], json!("selected"));

        mock.app
            .state::<SharedClipboard>()
            .lock()
            .hold(ClipContent {
                text: None,
                html: None,
                image: Some(ClipImage {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 255, 255],
                }),
            });
        assert_eq!(
            read(json!("Markdown"), false),
            json!({ "text": null, "markup": null, "image": true })
        );
        let name = mock
            .invoke(
                "clip_store_image",
                json!({ "note": "Home", "overCopied": false }),
            )
            .unwrap();
        let name = name.as_str().expect("a name").to_owned();
        assert!(
            name.starts_with("Home-") && name.ends_with(".png"),
            "{name}"
        );
        assert!(root.join(&name).exists());
        assert_eq!(mock.status(), format!("Pasted image stored as {name}"));
    }

    /// UC-005 A9 to A12, M-10.3 to M-10.5: the image dialog's file, the
    /// dropped items and the link dialog's note.
    #[test]
    fn uc005_a9_m10_3_m10_4_m10_5_embed_drop_and_link() {
        let dir = temp_dir("embed");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        std::fs::write(dir.join("photo.png"), b"png").unwrap();
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();
        let photo = dir.join("photo.png").display().to_string();

        let embedded = mock
            .invoke(
                "ed_embed",
                json!({ "note": "Home", "path": photo, "allowLarge": false }),
            )
            .unwrap();
        assert_eq!(
            embedded,
            json!({ "kind": "insert", "insertion": { "kind": "file", "place": "photo.png", "image": true } })
        );
        assert!(root.join("photo.png").exists());

        mock.invoke("ed_drag", json!({ "state": "over" })).unwrap();
        assert_eq!(
            mock.status(),
            "Drop to insert at the place under the pointer"
        );
        let dropped = mock
            .invoke(
                "ed_drop",
                json!({ "note": "Home", "paths": ["https://x.org", photo] }),
            )
            .unwrap();
        assert_eq!(dropped.as_array().map(Vec::len), Some(3));
        assert_eq!(dropped[2]["insertion"]["kind"], json!("dropped"));
        let none = mock
            .invoke("ed_drop", json!({ "note": "Home", "paths": [] }))
            .unwrap();
        assert_eq!(none, json!([]));
        assert_eq!(mock.status(), "This cannot be dropped into a note");

        let link = mock
            .invoke("ed_note_link", json!({ "note": "Home", "target": "Ideas" }))
            .unwrap();
        assert_eq!(
            link,
            json!({ "kind": "link", "address": "Ideas.md", "label": "Ideas" })
        );
        assert!(mock
            .invoke(
                "ed_embed",
                json!({ "note": "Nowhere", "path": photo, "allowLarge": false })
            )
            .is_err());
    }

    /// UC-005 A19 (review F6): `ed_embed` asks about a file over 10 MB and
    /// copies it with `allowLarge`.
    #[test]
    fn uc005_a19_ed_embed_asks_about_a_large_file_and_copies_it_when_allowed() {
        let dir = temp_dir("embed-large");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let big = dir.join("film.png");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(11 * 1024 * 1024)
            .unwrap();
        let mock = Mock::new(Some(root.clone()), &dir);
        mock.invoke("ws_start", json!({})).unwrap();
        let path = big.display().to_string();

        let asked = mock
            .invoke(
                "ed_embed",
                json!({ "note": "Home", "path": path, "allowLarge": false }),
            )
            .unwrap();
        assert_eq!(asked["kind"], json!("large"));
        assert!(asked["question"].as_str().unwrap().contains("has 11 MB"));
        assert!(!root.join("film.png").exists());
        let copied = mock
            .invoke(
                "ed_embed",
                json!({ "note": "Home", "path": path, "allowLarge": true }),
            )
            .unwrap();
        assert_eq!(copied["insertion"]["place"], json!("film.png"));
        assert!(root.join("film.png").exists());
    }

    /// BR-011 (review F6): the content kept for a copied selection goes when
    /// the window says the selection is gone, and when the tab closes.
    #[test]
    fn m10_7_the_kept_content_goes_with_the_selection_or_the_tab() {
        let dir = temp_dir("clip-forget");
        let root = workspace_with(&dir, "notes", &["Home", "Ideas"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();
        mock.invoke("tab_open", json!({ "note": "Ideas" })).unwrap();
        let clipboard = || mock.app.state::<SharedClipboard>();
        let keep = |note: &str| {
            clipboard().lock().write("former", None);
            mock.invoke(
                "clip_write",
                json!({ "note": note, "text": "selected", "html": null, "keepBefore": true }),
            )
            .unwrap();
        };
        let before = |note: &str| clipboard().lock().content(note, true).text;

        keep("Home");
        assert_eq!(before("Home").as_deref(), Some("former"));
        mock.invoke("clip_forget", json!({ "note": "Home" }))
            .unwrap();
        assert_eq!(before("Home").as_deref(), Some("selected"));

        keep("Ideas");
        mock.invoke("tab_close", json!({ "note": "Ideas", "force": false }))
            .unwrap();
        assert_eq!(before("Ideas").as_deref(), Some("selected"));
    }

    /// M-07.2: `tab_set_view` switches the view of a tab and remembers it
    /// (the snapshot); a note without an open tab is an error.
    #[test]
    fn m07_2_tab_set_view_switches_the_view() {
        let dir = temp_dir("facade");
        let root = workspace_with(&dir, "notes", &["Home"]);
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();

        mock.invoke("tab_set_view", json!({ "note": "Home", "view": "source" }))
            .unwrap();

        let snapshot = mock.app.state::<SharedSession>().lock().snapshot();
        assert_eq!(
            snapshot.tabs.tabs[0].view,
            underline_session::EditingViewName::Source
        );
        let unknown = mock
            .invoke("tab_set_view", json!({ "note": "Gone", "view": "facade" }))
            .unwrap_err();
        assert!(unknown.to_string().contains("not open"), "{unknown}");
    }

    #[test]
    fn the_window_may_call_every_command() {
        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();

        for command in COMMANDS {
            let permission = format!("allow-{}", command.replace('_', "-"));
            assert!(
                permissions.contains(&json!(permission)),
                "capabilities/main.json lacks {permission}"
            );
        }
    }

    #[test]
    fn every_command_has_a_permission_from_the_build_script() {
        let build = include_str!("../build.rs");

        for command in COMMANDS {
            assert!(
                build.contains(&format!("\"{command}\"")),
                "build.rs lacks {command}"
            );
        }
    }

    #[test]
    fn every_command_is_in_the_bindings() {
        let bindings = std::fs::read_to_string(bindings::BINDINGS_PATH).unwrap();

        for command in COMMANDS {
            assert!(
                bindings.contains(&format!("__TAURI_INVOKE(\"{command}\"")),
                "the bindings lack {command}"
            );
        }
    }

    /// ADR-025 section 3.9: of the window's answer and the fallback, only
    /// one goes on with a close; a later close waits anew.
    #[test]
    fn adr025_one_end_of_the_close_handshake_goes_on() {
        let closer = Closer::new(|| {});
        let first = closer.begin_handshake();

        assert!(closer.end_handshake(None), "the window answered");
        assert!(
            !closer.end_handshake(Some(first)),
            "the fallback stays still"
        );

        let second = closer.begin_handshake();
        assert!(
            !closer.end_handshake(Some(first)),
            "an old wait ends nothing"
        );
        assert!(closer.end_handshake(Some(second)), "the fallback goes on");
        assert!(!closer.end_handshake(None));
        assert!(!closer.closing());
        closer.close_now();
        assert!(closer.closing());
    }

    /// ADR-025 section 3.9: a window's answer after the fallback went on
    /// does not close a second time; without its edits sent, it closes not.
    #[test]
    fn adr025_a_late_or_failed_answer_does_not_close() {
        use std::sync::atomic::Ordering;

        let closes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let closer = Closer::new(|| {});
        let counted = std::sync::Arc::clone(&closes);
        closer.set_after_flush(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });

        let number = closer.begin_handshake();
        assert!(closer.end_handshake(Some(number)), "the fallback goes on");
        closer.flushed(true);
        assert_eq!(
            closes.load(Ordering::SeqCst),
            0,
            "the late answer changes nothing"
        );

        closer.begin_handshake();
        closer.flushed(false);
        assert_eq!(
            closes.load(Ordering::SeqCst),
            0,
            "edits not sent: the window asks"
        );

        closer.begin_handshake();
        closer.flushed(true);
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }

    /// UC-009 steps 1–5, M-15.1, M-15.2, M-15.5: the kinds, the dialog
    /// (answered by the test) with the planned name, and the export made
    /// without the session and reported.
    #[test]
    fn uc009_m15_export_start_choose_and_run() {
        let dir = temp_dir("export");
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Plan.md"), "# Plan\n\nText.\n").unwrap();
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let target = out.join("Plan.html");
        let asked = Arc::new(Mutex::new(Vec::new()));
        let dialog = {
            let asked = Arc::clone(&asked);
            let target = target.clone();
            SaveDialog::new(move |plan| {
                asked
                    .lock()
                    .push((plan.title.clone(), plan.file_name.clone()));
                DialogOutcome::Chosen(target.clone())
            })
        };
        let mock = Mock::with_dialog(Some(root), &dir, dialog);
        mock.invoke("ws_start", json!({})).unwrap();

        let start = mock
            .invoke("export_start", json!({ "note": "Plan" }))
            .unwrap();
        assert_eq!(start["title"], json!("Plan"));
        assert_eq!(
            start["choices"][0],
            json!({ "kind": "otherFormat", "label": "AsciiDoc" })
        );

        let chosen = mock
            .invoke("export_choose", json!({ "note": "Plan", "kind": "html" }))
            .unwrap();
        assert_eq!(chosen["kind"], json!("chosen"));
        assert_eq!(
            *asked.lock(),
            [("Export as HTML".to_owned(), "Plan.html".to_owned())]
        );

        let ran = mock
            .invoke(
                "export_run",
                json!({ "note": "Plan", "kind": "html", "typed": null }),
            )
            .unwrap();
        assert_eq!(ran, json!({ "kind": "done" }));
        assert!(std::fs::read_to_string(&target).unwrap().contains("Text."));
        assert_eq!(
            mock.status(),
            format!("Exported Plan to {}", target.display())
        );
        assert_eq!(
            mock.invoke("export_approximated", json!({ "index": 0 }))
                .unwrap(),
            json!({ "title": "Plan", "lines": [] })
        );
    }

    /// ADR-016, M-15.2: without a dialog the path is typed; an existing
    /// file asks first (D-03: for that path only).
    #[test]
    fn uc009_m15_2_without_a_dialog_the_path_is_typed() {
        let dir = temp_dir("export-typed");
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Plan.md"), "# Plan\n").unwrap();
        let mock = Mock::with_dialog(
            Some(root),
            &dir,
            SaveDialog::new(|_| DialogOutcome::Unavailable),
        );
        mock.invoke("ws_start", json!({})).unwrap();

        let answer = mock
            .invoke("export_choose", json!({ "note": "Plan", "kind": "pdf" }))
            .unwrap();
        assert_eq!(answer["kind"], json!("typed"));
        assert_eq!(answer["heading"], json!("Export Plan as PDF to:"));
        assert!(answer["path"].as_str().unwrap().ends_with("Plan.pdf"));

        let target = dir.join("Plan.md.html");
        std::fs::write(&target, "old").unwrap();
        let path = target.display().to_string();
        let answer = mock
            .invoke(
                "export_run",
                json!({ "note": "Plan", "kind": "html", "typed": { "path": path, "replace": null } }),
            )
            .unwrap();
        assert_eq!(
            answer,
            json!({ "kind": "exists", "problem": "The file exists. Export again to replace it.", "path": path })
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    }

    /// UC-009 A1: a cancelled dialog writes nothing and says nothing.
    #[test]
    fn uc009_a1_a_cancelled_dialog_does_nothing() {
        let dir = temp_dir("export-cancel");
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Plan.md"), "# Plan\n").unwrap();
        let mock = Mock::new(Some(root), &dir);
        mock.invoke("ws_start", json!({})).unwrap();
        let before = mock.status();

        let answer = mock
            .invoke("export_choose", json!({ "note": "Plan", "kind": "pdf" }))
            .unwrap();
        let ran = mock
            .invoke(
                "export_run",
                json!({ "note": "Plan", "kind": "pdf", "typed": null }),
            )
            .unwrap();

        assert_eq!(answer, json!({ "kind": "done" }));
        assert_eq!(ran, json!({ "kind": "done" }));
        assert_eq!(mock.status(), before);
        // Nothing to open.
        assert_eq!(
            mock.invoke("export_open", json!({ "index": 0 })),
            Ok(Value::Null)
        );
    }

    /// Review F11: one save dialog at a time; a second request while one is
    /// open does not show another.
    #[test]
    fn uc009_a121_a_second_dialog_does_not_open() {
        let dir = temp_dir("export-single");
        let root = dir.join("notes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Plan.md"), "# Plan\n").unwrap();
        let shown = Arc::new(Mutex::new(0));
        let dialog = {
            let shown = Arc::clone(&shown);
            SaveDialog::new(move |_| {
                *shown.lock() += 1;
                DialogOutcome::Cancelled
            })
        };
        let mock = Mock::with_dialog(Some(root), &dir, dialog);
        mock.invoke("ws_start", json!({})).unwrap();
        // A dialog is open (as the first `export_choose` leaves it).
        let open = mock.app.state::<SharedSession>().lock().export_dialog(
            &underline_core::NoteId::from_relative("Plan"),
            ExportKindName::Pdf,
        );
        assert!(open.is_some());

        let answer = mock
            .invoke("export_choose", json!({ "note": "Plan", "kind": "html" }))
            .unwrap();

        assert_eq!(answer, json!({ "kind": "done" }));
        assert_eq!(*shown.lock(), 0);
        assert_eq!(mock.status(), "Choosing where to save Plan…");
    }
}
