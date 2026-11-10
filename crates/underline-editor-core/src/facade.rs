//! UC-005, M-09.10 to M-09.14, M-11.2, M-11.3, ADR-024, ADR-025: editing in
//! the facade without a user interface. Every input is an edit of the markup
//! at the source position the facade character stands for, and the blocks
//! of the note follow it at once: typed characters join their block without
//! parsing, everything else parses only the blocks around the edit
//! ([`parse_all`] parses the whole note).
//!
//! What the window shows of the blocks (runs, code colours, figures) is
//! made behind the [`MakeViews`] trait, so highlighting, diagrams and images
//! stay in the backend; [`refresh`] decides which views are made anew after
//! an input and which only move, and [`splices`] tells the window what
//! changed.
//!
//! Moved here from `underline-session` (plan section 17, step B2), so the
//! same rules can run in the webview (ADR-025). Nothing here touches files,
//! threads or clocks: the module builds for `wasm32-unknown-unknown`.

mod edit;
mod refresh;
mod reparse;
mod rules;
mod splice;

pub use edit::{cursor_in_code, gap_at, Dirty, FacadeEdit, FacadeTarget, Gap, Gathered, Rows};
pub use refresh::{
    captions, carry_spans, moved_indices, refresh, Before, MakeViews, Part, Refreshed,
};
pub use reparse::parse_all;
pub use splice::{splices, ShownBlock, Splice};

#[cfg(test)]
mod tests;
