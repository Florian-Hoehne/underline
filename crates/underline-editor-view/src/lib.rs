//! The editor of Underline as the window holds it (ADR-025): the data the
//! window and the editor exchange about the text of an open note
//! ([`editor_view`] for the source view, [`facade_view`] for the facade),
//! and the tab that answers the window's inputs in both views
//! ([`EditorTab`]): the adapter between the window and the editor core
//! (`underline-editor-core`), the folds of the sections ([`folds`]), the
//! hits of the Nano search, and the views of the facade's blocks
//! ([`views`]), made in the webview from what the backend sent
//! ([`CachedViews`]); and what the two exchange to keep their copies of the
//! text alike ([`sync_view`]).
//!
//! Offsets are web units (`underline_editor_core::utf16`); revisions and
//! offsets are `u32`, as TypeScript numbers carry them exactly. The data
//! types derive `specta::Type` behind the `specta` feature, so TypeScript
//! bindings can be generated from them. Nothing here touches files,
//! threads, or clocks (the host passes its time in milliseconds): the crate
//! builds for `wasm32-unknown-unknown`, so the same code runs in the backend
//! and, compiled to WASM, in the webview.
//!
//! Moved here from `underline-session` (`docs/migration/variante-b.md`,
//! steps B3a and B3b), which re-exports the data under the former paths.

#![warn(missing_docs)]

mod cached;
pub mod editor_view;
pub mod facade_view;
pub mod folds;
mod found;
pub mod sync_view;
mod tab;
pub mod views;

pub use cached::{CachedViews, CODE_COLORS_KEPT};
pub use found::FOUND_EVERY_MS;
pub use tab::{
    blocks_views, fold_key, gap_view, indicator, keys_status, link_at_web, EditorTab, FacadeCall,
    FacadeClipped, FacadeModel, Handled, Versioned,
};
