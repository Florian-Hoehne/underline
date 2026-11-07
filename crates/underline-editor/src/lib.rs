//! Editor widget of Underline (ADR-012): one rope per note, a source view
//! that lays out only the visible lines, and later the facade on the same
//! text.

mod document;
mod enlarged;
mod facade_view;
pub mod fonts;
mod minimap;
mod source_view;
mod toolbar;

pub use document::{Document, Edit};
pub use facade_view::FacadeView;
pub use source_view::SourceView;
pub use toolbar::Tool;

#[cfg(test)]
mod prototype_benchmark;
