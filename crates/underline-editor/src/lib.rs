//! Editor widget of Underline (ADR-012): one rope per note, a source view
//! that lays out only the visible lines, and later the facade on the same
//! text.

mod document;
mod source_view;

pub use document::{Document, Edit};
pub use source_view::SourceView;

#[cfg(test)]
mod prototype_benchmark;
