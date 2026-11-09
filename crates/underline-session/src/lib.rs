//! Underline's workspace session, without a user interface.
//!
//! The egui app keeps its orchestration in its window code: the open
//! workspace, the tabs with their documents, the save pipeline, changes from
//! outside, agent requests (ADR-018), the git flows, the sync rules, the todos
//! poll, the settings state machine, exporting, following links and the status
//! texts. This crate is where that logic is ported to, feature by feature, so
//! that the Tauri app (`apps/underline-tauri`) is only a thin layer of commands
//! and events over it and the logic is unit-tested without a window.
//!
//! The egui frame loop becomes a backend tick (ADR-022): a thread calls
//! [`Session::tick`] every 100 ms, the tick keeps today's intervals (agent
//! requests 100 ms, settings 500 ms, presence and todos 1 s, change tracker
//! 3 s, sync interval, autosave deadlines) and returns the [`Event`]s the
//! window needs. The wake callbacks of the background services trigger the
//! tick at once instead of repainting a frame.
//!
//! Ported so far (feature F1, the shell): the error every command returns
//! ([`UiError`]), what the window learns on start ([`StartInfo`], M-01), the
//! settings state machine ([`SettingsState`], UC-010), the theme palettes
//! ([`PaletteView`], M-01.6), the app state (M-01.3, M-22.1) and the status
//! message (M-04.7). Feature F2 adds the workspace: the Open and Create
//! dialogs (UC-001, UC-002, M-21), the note overview (M-05), the tabs
//! (M-06) and what is remembered of them (M-22.2).
//!
//! The data types crossing to the window derive `specta::Type` behind the
//! `specta` feature, so TypeScript bindings can be generated from them.

mod error;
mod event;
mod palette;
mod session;
mod settings;
mod start;
mod workspace_view;

pub use error::{UiError, UiErrorKind};
pub use event::{Event, StatusView};
pub use palette::PaletteView;
pub use session::Session;
pub use settings::{
    agent_access_refused_message, theme_names, Scope, SettingName, SettingsState, SettingsView,
    USER_SETTINGS_POLL,
};
pub use start::{Platform, StartInfo};
pub use workspace_view::{
    CreateAnswer, EditingViewName, FormatName, NewWorkspaceRequest, OpenAnswer, OpenOffer,
    OverviewItem, StartOpened, TabView, TabsView, TodoTabView, WorkspaceSnapshot, WorkspaceView,
};
