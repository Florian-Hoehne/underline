//! Spike F0: the owner of a note's text, as variant A runs it in the Tauri
//! backend and variant B runs it in the webview as WASM.
//!
//! The owner keeps the rope (`underline_editor_core::Document`) and the
//! facade's blocks, takes one input at a time from either view, and answers
//! with a patch for that view. Offsets crossing the boundary are JavaScript
//! UTF-16 code units; they are converted here, in one place (ADR-021).

#[cfg(feature = "real-facade")]
pub use underline_facade::blocks;

#[cfg(not(feature = "real-facade"))]
#[path = "blocks_copy.rs"]
pub mod blocks;

pub mod mirror;
pub mod note;
pub mod owner;
pub mod render;

pub use mirror::{Mirror, MirrorEdit, MirrorError};
pub use owner::{BenchPosition, FacadeInput, FacadePatch, Input, Owner, SourceInput, SourcePatch};

/// The blocks of `source`, read as Markdown, by whichever facade is built in.
pub fn parse(source: &str) -> Vec<blocks::Block> {
    #[cfg(feature = "real-facade")]
    {
        blocks::parse_blocks(source, underline_core::NoteFormat::Markdown)
    }
    #[cfg(not(feature = "real-facade"))]
    {
        blocks::parse_blocks(source, blocks::NoteFormat::Markdown)
    }
}

/// FNV-1a over the UTF-8 bytes of `text`, as hex: both copies of the text in
/// variant B must hash the same.
pub fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}
