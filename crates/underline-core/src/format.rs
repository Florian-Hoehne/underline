//! The note formats live in `underline-markup` (ADR-025); re-exported here so
//! `crate::format::NoteFormat` and `underline_core::NoteFormat` keep working.

pub use underline_markup::NoteFormat;
