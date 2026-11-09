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
//! The egui frame loop becomes a backend tick: a thread calls
//! `Session::tick(now)` every 100 ms, the tick keeps today's intervals (agent
//! requests 100 ms, settings 500 ms, presence and todos 1 s, change tracker
//! 3 s, sync interval, autosave deadlines) and returns the events the window
//! needs. The wake callbacks of the background services trigger the tick at
//! once instead of repainting a frame.
//!
//! So far the crate holds only the skeleton: the error every command returns
//! ([`UiError`]), what the window learns on start ([`StartInfo`], M-01) and a
//! placeholder [`Session`].
//!
//! The data types crossing to the window derive `specta::Type` behind the
//! `specta` feature, so TypeScript bindings can be generated from them.

mod error;
mod session;
mod start;

pub use error::{UiError, UiErrorKind};
pub use session::Session;
pub use start::{Platform, StartInfo};
