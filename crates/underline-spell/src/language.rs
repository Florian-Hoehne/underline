//! OQ-207, UC-005 BR-023: the spell check language a note names for itself
//! with its attribute `lang` (front matter `lang: de-CH`, AsciiDoc
//! `:lang: de-CH`), which overrides the language of the settings for that
//! note.

use underline_core::SpellCheckLanguage;
use underline_markup::attributes::note_head;
use underline_markup::NoteFormat;

/// The name of the note attribute that names a note's language.
pub const LANG_ATTRIBUTE: &str = "lang";

/// The most bytes of a note read for its attribute `lang`: the head of a
/// note is at its start, so the rest of a long note is never parsed for it.
pub const LANG_HEAD_BYTES: usize = 64 * 1024;

/// OQ-207: what a note says of its spell check language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NoteLanguage {
    /// The note names none: the language of the settings applies.
    Settings,
    /// The note names an offered language.
    Offered(SpellCheckLanguage),
    /// The note names a language Underline does not offer, as written (cut
    /// to [`MAX_SHOWN_TAG`] characters on one line); it has no dictionary,
    /// so no word of the note is marked.
    Unknown(String),
}

/// OQ-207: the language the attribute `lang` of `source`, a note in
/// `format`, names. A missing or empty attribute, `null`, `~`, `off`, or a
/// head that cannot be read, names none; of an AsciiDoc attribute set
/// twice the last counts. Only the first [`LANG_HEAD_BYTES`] of `source` are
/// looked at.
///
/// ```
/// use underline_core::SpellCheckLanguage;
/// use underline_markup::NoteFormat;
/// use underline_spell::{note_language, NoteLanguage};
///
/// let note = "---\nlang: de_CH\n---\n# Notiz\n";
/// assert_eq!(
///     note_language(note, NoteFormat::Markdown),
///     NoteLanguage::Offered(SpellCheckLanguage::GermanCh)
/// );
/// ```
#[must_use]
pub fn note_language(source: &str, format: NoteFormat) -> NoteLanguage {
    let mut end = source.len().min(LANG_HEAD_BYTES);
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let head = note_head(&source[..end], format);
    // AsciiDoc names attributes in any case, and a later entry of the same
    // attribute wins; front matter names are as written.
    let named = |name: &str| match format {
        NoteFormat::AsciiDoc => name.eq_ignore_ascii_case(LANG_ATTRIBUTE),
        NoteFormat::Markdown => name == LANG_ATTRIBUTE,
    };
    let Some(attribute) = head.attributes.iter().rev().find(|a| named(&a.name)) else {
        return NoteLanguage::Settings;
    };
    let tag = attribute.value.trim();
    // OQ-207: no value, YAML's null and `off` leave the settings' language.
    if tag.is_empty()
        || tag == "~"
        || tag.eq_ignore_ascii_case("null")
        || tag.eq_ignore_ascii_case("off")
    {
        return NoteLanguage::Settings;
    }
    SpellCheckLanguage::of_tag(tag).map_or_else(
        || NoteLanguage::Unknown(shown_tag(tag)),
        NoteLanguage::Offered,
    )
}

/// The most characters of a language not offered that are shown.
pub const MAX_SHOWN_TAG: usize = 35;

/// OQ-207: `tag` as the status bar shows it: on one line, at most
/// [`MAX_SHOWN_TAG`] characters, cut with `…`.
fn shown_tag(tag: &str) -> String {
    let one_line: String = tag
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if one_line.chars().count() <= MAX_SHOWN_TAG {
        return one_line;
    }
    let mut cut: String = one_line.chars().take(MAX_SHOWN_TAG - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oq207_front_matter_lang_names_the_language() {
        for tag in ["de-CH", "de_CH", "de-ch", "'de-CH'", "\"de-CH\""] {
            let note = format!("---\ntitle: Notiz\nlang: {tag}\n---\n# Notiz\n");
            assert_eq!(
                note_language(&note, NoteFormat::Markdown),
                NoteLanguage::Offered(SpellCheckLanguage::GermanCh),
                "{tag}"
            );
        }
    }

    #[test]
    fn oq207_asciidoc_lang_names_the_language() {
        let note = "= Nota\n:lang: it-IT\n\nCiao.\n";
        assert_eq!(
            note_language(note, NoteFormat::AsciiDoc),
            NoteLanguage::Offered(SpellCheckLanguage::Italian)
        );
        let upper = "= Nota\n:LANG: it_it\n\nCiao.\n";
        assert_eq!(
            note_language(upper, NoteFormat::AsciiDoc),
            NoteLanguage::Offered(SpellCheckLanguage::Italian)
        );
    }

    #[test]
    fn oq207_no_lang_or_an_empty_one_leaves_the_settings() {
        assert_eq!(
            note_language("# Plan\n\nlang: de-CH\n", NoteFormat::Markdown),
            NoteLanguage::Settings,
            "only the head counts"
        );
        assert_eq!(
            note_language("---\nlang: \n---\n", NoteFormat::Markdown),
            NoteLanguage::Settings
        );
        assert_eq!(
            note_language("---\nlang: [\n---\n", NoteFormat::Markdown),
            NoteLanguage::Settings,
            "a head that cannot be read names none"
        );
        assert_eq!(
            note_language("= Plan\n\n:lang: de-CH\n", NoteFormat::AsciiDoc),
            NoteLanguage::Settings
        );
    }

    #[test]
    fn oq207_a_language_not_offered_is_unknown_and_shown_short() {
        assert_eq!(
            note_language("---\nlang: pt-BR\n---\n", NoteFormat::Markdown),
            NoteLanguage::Unknown("pt-BR".to_owned())
        );
        let long = format!("---\nlang: {}\n---\n", "x".repeat(500));
        let NoteLanguage::Unknown(shown) = note_language(&long, NoteFormat::Markdown) else {
            panic!("unknown");
        };
        assert_eq!(shown.chars().count(), MAX_SHOWN_TAG);
        assert!(shown.ends_with('…'));
    }

    #[test]
    fn oq207_null_off_and_empty_leave_the_settings() {
        for value in ["null", "~", "off", "Off", "\"\"", "[]"] {
            let note = format!("---\nlang: {value}\n---\n");
            assert_eq!(
                note_language(&note, NoteFormat::Markdown),
                NoteLanguage::Settings,
                "{value}"
            );
        }
    }

    #[test]
    fn oq207_the_last_asciidoc_lang_wins() {
        let note = "= Nota\n:lang: de-CH\n:lang: it\n\nCiao.\n";
        assert_eq!(
            note_language(note, NoteFormat::AsciiDoc),
            NoteLanguage::Offered(SpellCheckLanguage::Italian)
        );
    }

    #[test]
    fn oq207_only_the_head_of_a_long_note_is_read() {
        let mut note = String::from("---\nlang: de-CH\n---\n");
        while note.len() < LANG_HEAD_BYTES + 10 {
            note.push_str("Grüße aus Zürich. ");
        }
        assert_eq!(
            note_language(&note, NoteFormat::Markdown),
            NoteLanguage::Offered(SpellCheckLanguage::GermanCh)
        );
    }
}
