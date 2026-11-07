//! Facade engine of Underline (ADR-003).
//!
//! Every write of markup goes through the format module of the note's format,
//! looked up in a [`Registry`]. A new note format is one new module. Parsing
//! into blocks with source spans (comrak for Markdown, an own parser for
//! AsciiDoc) is added with the use cases that edit notes in the facade.

mod asciidoc;
pub mod blocks;
pub mod diagram;
pub mod formatting;
pub mod highlight;
pub mod images;
pub mod index;
pub mod sections;

use underline_core::NoteFormat;

/// What the facade engine needs from one note format.
pub trait FormatModule: Send + Sync {
    /// The note format this module handles.
    fn format(&self) -> NoteFormat;

    /// Markup of a new note that holds only `title` as its heading
    /// (UC-001 BR-003).
    fn new_note(&self, title: &str) -> String;
}

/// Markdown as GitHub shows it.
#[derive(Debug, Default)]
pub struct Markdown;

impl FormatModule for Markdown {
    fn format(&self) -> NoteFormat {
        NoteFormat::Markdown
    }

    fn new_note(&self, title: &str) -> String {
        format!("# {}\n", title.trim())
    }
}

/// AsciiDoc, as Asciidoctor reads it.
#[derive(Debug, Default)]
pub struct AsciiDoc;

impl FormatModule for AsciiDoc {
    fn format(&self) -> NoteFormat {
        NoteFormat::AsciiDoc
    }

    fn new_note(&self, title: &str) -> String {
        format!("= {}\n", title.trim())
    }
}

/// The format modules Underline knows, one per note format.
pub struct Registry {
    modules: Vec<Box<dyn FormatModule>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            modules: vec![Box::new(Markdown), Box::new(AsciiDoc)],
        }
    }
}

impl Registry {
    /// The module for `format`.
    pub fn module(&self, format: NoteFormat) -> &dyn FormatModule {
        self.modules
            .iter()
            .find(|module| module.format() == format)
            .map(Box::as_ref)
            .unwrap_or_else(|| panic!("no format module for {format}"))
    }

    /// Markup of a new note titled `title` in `format`.
    pub fn new_note(&self, format: NoteFormat, title: &str) -> String {
        self.module(format).new_note(title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_note_format_has_a_module() {
        let registry = Registry::default();
        for format in NoteFormat::ALL {
            assert_eq!(registry.module(format).format(), format);
        }
    }

    /// UC-001 BR-003: the Home note holds only its title as heading.
    #[test]
    fn uc001_home_note_holds_only_its_title_as_heading() {
        let registry = Registry::default();
        assert_eq!(registry.new_note(NoteFormat::Markdown, "Home"), "# Home\n");
        assert_eq!(registry.new_note(NoteFormat::AsciiDoc, "Home"), "= Home\n");
    }
}
