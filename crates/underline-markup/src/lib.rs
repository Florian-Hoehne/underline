//! Note formats and the facade parser of Underline (ADR-003, ADR-025).
//!
//! The pure part of the facade engine: the note formats, the parse of a
//! note into blocks with source spans (comrak for Markdown, an own parser for
//! AsciiDoc), its sections, and the formatting rules. Nothing here touches
//! files, threads, or clocks, so the crate builds for
//! `wasm32-unknown-unknown` too. `underline-core` and `underline-facade`
//! re-export it under their former paths.

mod asciidoc;
pub mod blocks;
pub mod diagram;
mod format;
pub mod formatting;
pub mod sections;

pub use format::NoteFormat;
