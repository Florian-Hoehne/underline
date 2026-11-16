//! UC-005 BR-023: the words of a note the spell check looks at.
//!
//! Only the text the facade shows is checked, taken from the blocks and
//! inline runs of `underline-markup`, each character with the place in the
//! markup it stands for. Left out are:
//!
//! - code blocks, diagrams (code blocks of their language), formulas,
//!   pictures, and blocks the facade shows as their markup;
//! - inline code and inline formulas;
//! - link addresses (never shown) and web addresses shown as text (an
//!   address shown as its own link, or any piece of text holding `://`,
//!   starting with `www.` or `mailto:`, or an e-mail address);
//! - tags (`#tag` in a Markdown note's text, the value of the attribute
//!   `tags`);
//! - the names of note attributes;
//! - words holding digits, and words written only in capitals.
//!
//! The text is cut into words by the word boundaries of Unicode (UAX #29),
//! as the word count does (ADR-027), so "don't" and "l'homme" are one word
//! and "well-known" two.

use std::ops::Range;

use underline_markup::attributes::{note_head, AttributeKind};
use underline_markup::blocks::{parse_blocks, Block, BlockKind, Run};
use underline_markup::tags::text_tags;
use underline_markup::NoteFormat;
use unicode_segmentation::UnicodeSegmentation;

use crate::user_words::MAX_WORD_LENGTH;

/// A word the spell check looks at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// The bytes of the markup it stands in; markup inside the word, such
    /// as `**` in `**Under**line`, is part of it.
    pub range: Range<usize>,
    /// The word as the facade shows it.
    pub text: String,
}

/// The words of one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockWords {
    /// The bytes of the block's markup.
    pub span: Range<usize>,
    /// Its words, in order.
    pub words: Vec<Word>,
}

/// UC-005 BR-023: whether the spell check looks at `word`: it holds a
/// letter and no digit, is not written only in capitals, and is not longer
/// than a word of the dictionary can be.
#[must_use]
pub fn is_checked_word(word: &str) -> bool {
    let mut letters = 0usize;
    let mut capitals = 0usize;
    for c in word.chars() {
        if c.is_numeric() {
            return false;
        }
        if c.is_alphabetic() {
            letters += 1;
            if c.is_uppercase() {
                capitals += 1;
            }
        }
    }
    letters > 0 && capitals < letters && word.chars().count() <= MAX_WORD_LENGTH
}

/// UC-005 BR-023: the words of `source`, a note in `format`, the spell
/// check looks at, block by block.
#[must_use]
pub fn note_words(source: &str, format: NoteFormat) -> Vec<BlockWords> {
    let blocks = parse_blocks(source, format);
    let context = Context::of(source, format, &blocks);
    blocks
        .iter()
        .map(|block| BlockWords {
            span: block.span.clone(),
            words: block_words(source, block, &context),
        })
        .collect()
}

/// What the words of a block depend on beyond the block.
#[derive(Debug, Clone, Default)]
pub(crate) struct Context {
    /// The bytes no word is taken from although the facade shows them: the
    /// names of note attributes, the values of `tags` and `lang`, the tags
    /// in the text;
    /// sorted by their start.
    pub(crate) skips: Vec<Range<usize>>,
}

impl Context {
    /// The context of the blocks `blocks` of `source`, a note in `format`.
    pub(crate) fn of(source: &str, format: NoteFormat, blocks: &[Block]) -> Self {
        Self {
            skips: skipped(source, format, blocks),
        }
    }
}

/// The skipped bytes of [`Context`].
fn skipped(source: &str, format: NoteFormat, blocks: &[Block]) -> Vec<Range<usize>> {
    let head = note_head(source, format);
    let mut skips = Vec::new();
    if let (Some(_), Some(front)) = (&head.problem, &head.front_matter) {
        // A front matter that cannot be read shows as its markup.
        skips.push(front.clone());
    }
    for attribute in &head.attributes {
        let value = &attribute.value_span;
        // The value of `tags`, and of `lang`, a language code (OQ-207).
        if attribute.name.eq_ignore_ascii_case("tags")
            || attribute.name.eq_ignore_ascii_case(crate::LANG_ATTRIBUTE)
            || value.is_empty()
            || attribute.kind == AttributeKind::Nested
        {
            skips.push(attribute.span.clone());
        } else {
            // Its name, up to its value; and what follows its value on its
            // lines, such as a closing quote.
            skips.push(attribute.span.start..value.start);
            if value.end < attribute.span.end {
                skips.push(value.end..attribute.span.end);
            }
        }
    }
    if format == NoteFormat::Markdown {
        skips.extend(text_tags(source, blocks).into_iter().map(|tag| tag.range));
    }
    skips.retain(|range| !range.is_empty());
    skips.sort_by_key(|range| range.start);
    skips
}

/// The words of `block`, leaving out those overlapping the context's
/// skipped bytes.
pub(crate) fn block_words(source: &str, block: &Block, context: &Context) -> Vec<Word> {
    let skips = context.skips.as_slice();
    let mut words = Vec::new();
    match &block.kind {
        BlockKind::Heading(_)
        | BlockKind::Paragraph
        | BlockKind::ListItem { .. }
        | BlockKind::Quote(_)
        | BlockKind::Admonition(_) => runs_words(source, &block.runs, skips, &mut words),
        BlockKind::Table(table) if table.front_matter => {
            // UC-024 BR-001: the names are its header and never checked;
            // a value is checked when the facade shows it as written, each
            // character at its place, so both views mark the same words.
            for cell in table.rows.iter().skip(1).flatten() {
                if cell.iter().all(|run| in_place(source, run)) {
                    runs_words(source, cell, skips, &mut words);
                }
            }
        }
        BlockKind::Table(table) => {
            for cell in table.rows.iter().flatten() {
                runs_words(source, cell, skips, &mut words);
            }
        }
        BlockKind::Code { .. }
        | BlockKind::ThematicBreak
        | BlockKind::Markup
        | BlockKind::Image { .. }
        | BlockKind::Formula { .. } => {}
    }
    words
}

/// Whether each character of `run` stands at its own place in `source`,
/// the one after the other.
fn in_place(source: &str, run: &Run) -> bool {
    run.text.chars().count() == run.offsets.len()
        && run
            .text
            .chars()
            .zip(&run.offsets)
            .all(|(c, &at)| source.get(at..).and_then(|rest| rest.chars().next()) == Some(c))
}

/// Whether the run shows text that is no prose: code, a formula, or a web
/// address shown as its own link.
fn left_out(run: &Run) -> bool {
    run.style.code
        || run.style.formula.is_some()
        || run.style.link.as_deref() == Some(run.text.as_str())
}

/// Whether a piece of text between spaces is a web or e-mail address; the
/// brackets and quotes around it, and the punctuation after it, are not
/// part of it.
fn is_address(piece: &str) -> bool {
    let piece = piece.trim_matches(|c: char| {
        matches!(
            c,
            '(' | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '"'
                | '\''
                | '“'
                | '”'
                | '‘'
                | '’'
                | '«'
                | '»'
                | ','
                | ';'
                | ':'
                | '!'
                | '?'
                | '.'
        )
    });
    let starts = |prefix: &str| {
        piece
            .get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    };
    if piece.contains("://") || starts("www.") || starts("mailto:") {
        return true;
    }
    piece.split_once('@').is_some_and(|(name, domain)| {
        name.chars().any(char::is_alphanumeric)
            && domain.contains('.')
            && domain.chars().next().is_some_and(char::is_alphanumeric)
    })
}

/// The words of `runs`, read as one text: a word may span runs, as in
/// `**Under**line`; text that is left out separates words.
fn runs_words(source: &str, runs: &[Run], skips: &[Range<usize>], out: &mut Vec<Word>) {
    // The text, with the place in the markup of each character taken from
    // it; left out characters become spaces with no place.
    let mut text = String::new();
    let mut places: Vec<(usize, Option<usize>)> = Vec::new();
    for run in runs {
        let skip_run = left_out(run);
        for (index, c) in run.text.chars().enumerate() {
            let place = if skip_run {
                None
            } else {
                run.offsets.get(index).copied()
            };
            places.push((text.len(), place));
            text.push(if place.is_some() { c } else { ' ' });
        }
    }
    words_of(source, &text, &places, skips, out);
}

/// The words of `text`, read from the markup of `source`, each character
/// at the place `places` gives it (`None` for none).
fn words_of(
    source: &str,
    text: &str,
    places: &[(usize, Option<usize>)],
    skips: &[Range<usize>],
    out: &mut Vec<Word>,
) {
    if text.trim().is_empty() {
        return;
    }
    let addresses: Vec<Range<usize>> = pieces(text)
        .into_iter()
        .filter(|(_, piece)| is_address(piece))
        .map(|(at, piece)| at..at + piece.len())
        .collect();
    for (at, word) in text.split_word_bound_indices() {
        if !is_checked_word(word) {
            continue;
        }
        let end = at + word.len();
        if addresses.iter().any(|a| a.start < end && at < a.end) {
            continue;
        }
        let Some(range) = source_range(source, places, at..end) else {
            continue;
        };
        if skips
            .iter()
            .any(|s| s.start < range.end && range.start < s.end)
        {
            continue;
        }
        out.push(Word {
            range,
            text: word.to_owned(),
        });
    }
}

/// The pieces of `text` between white space, each with its byte offset.
fn pieces(text: &str) -> Vec<(usize, &str)> {
    let mut pieces = Vec::new();
    let mut start: Option<usize> = None;
    for (at, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(from)) => {
                pieces.push((from, &text[from..at]));
                start = None;
            }
            (false, None) => start = Some(at),
            _ => {}
        }
    }
    if let Some(from) = start {
        pieces.push((from, &text[from..]));
    }
    pieces
}

/// The bytes of `source` the characters at bytes `within` of the read text
/// stand for, from the first character's place to the end of the last one.
fn source_range(
    source: &str,
    places: &[(usize, Option<usize>)],
    within: Range<usize>,
) -> Option<Range<usize>> {
    let first = places.partition_point(|(at, _)| *at < within.start);
    let last = places.partition_point(|(at, _)| *at < within.end);
    let chars = places.get(first..last)?;
    let start = chars.first()?.1?;
    let end_char = chars.last()?.1?;
    let end = end_char
        + source
            .get(end_char..)
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8);
    (start < end && end <= source.len()).then_some(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words checked in `source`, as written there.
    fn checked(source: &str, format: NoteFormat) -> Vec<String> {
        note_words(source, format)
            .into_iter()
            .flat_map(|block| block.words)
            .map(|word| {
                assert_eq!(
                    source[word.range.clone()]
                        .replace("**", "")
                        .replace('*', ""),
                    word.text,
                    "the range is the word's markup"
                );
                word.text
            })
            .collect()
    }

    #[test]
    fn br023_words_holding_digits_or_only_capitals_are_not_checked() {
        assert!(is_checked_word("ferri"));
        assert!(is_checked_word("Ferri"));
        assert!(is_checked_word("don't"));
        assert!(!is_checked_word("HTTPX"));
        assert!(!is_checked_word("I"));
        assert!(!is_checked_word("abc123"));
        assert!(!is_checked_word("2026"));
        assert!(!is_checked_word("--"));
        assert!(!is_checked_word(&"a".repeat(101)));
    }

    #[test]
    fn tc005_40_markdown_leaves_out_code_addresses_tags_formulas_and_names() {
        let source = "---\nstatsu: offen\ntags: [taggg]\n---\n\
            Strasse fehlr `fehlr` and [Link](https://exampel.org) and https://exampel.org/x \
            and www.exampel.org and someone@exampel.org #taggg HTTPX abc123 $fehlr$ wordd\n\n\
            ```\nfehlr in code\n```\n\n```mermaid\ngraph TD; fehlr-->x\n```\n\n$$\nfehlr\n$$\n";
        let words = checked(source, NoteFormat::Markdown);
        assert_eq!(
            words,
            ["offen", "Strasse", "fehlr", "and", "Link", "and", "and", "and", "wordd"]
        );
    }

    #[test]
    fn oq207_the_value_of_lang_is_a_code_not_checked() {
        let markdown = "---\nlang: de-CH\nstatsu: offen\n---\nStrasse\n";
        assert_eq!(
            checked(markdown, NoteFormat::Markdown),
            ["offen", "Strasse"]
        );
        let asciidoc = "= Title\n:lang: de-CH\n\nStrasse\n";
        let words = checked(asciidoc, NoteFormat::AsciiDoc);
        assert!(!words.contains(&"de".to_owned()), "{words:?}");
        assert!(words.contains(&"Strasse".to_owned()), "{words:?}");
    }

    #[test]
    fn tc005_40_asciidoc_leaves_out_code_addresses_formulas_and_names() {
        let source = "= Title\n:statsu: offen\n:stem: latexmath\n\n\
            Strasse `fehlr` and https://exampel.org[Link] and stem:[fehlr] HTTPX abc123 wordd\n\n\
            ----\nfehlr in code\n----\n\n[latexmath]\n++++\nfehlr\n++++\n";
        let words = checked(source, NoteFormat::AsciiDoc);
        assert!(words.contains(&"Strasse".to_owned()), "{words:?}");
        assert!(words.contains(&"wordd".to_owned()), "{words:?}");
        assert!(words.contains(&"Link".to_owned()), "{words:?}");
        for left in ["fehlr", "statsu", "HTTPX", "abc123", "exampel", "latexmath"] {
            assert!(!words.contains(&left.to_owned()), "{left} in {words:?}");
        }
    }

    #[test]
    fn br023_a_word_may_span_runs_and_keeps_its_markup() {
        let source = "**Under**lne and *it*alic\n";
        let blocks = note_words(source, NoteFormat::Markdown);
        let words = &blocks[0].words;
        assert_eq!(words[0].text, "Underlne");
        assert_eq!(&source[words[0].range.clone()], "Under**lne");
        assert_eq!(words[2].text, "italic");
    }

    #[test]
    fn br023_multibyte_words_have_byte_ranges_of_the_markup() {
        let source = "Ärger über Straße 😀 naïve café\n";
        let words: Vec<Word> = note_words(source, NoteFormat::Markdown)
            .into_iter()
            .flat_map(|b| b.words)
            .collect();
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Ärger", "über", "Straße", "naïve", "café"]);
        for word in &words {
            assert_eq!(&source[word.range.clone()], word.text);
        }
    }

    #[test]
    fn br023_crlf_line_breaks_keep_the_ranges_right() {
        let source = "first linne\r\nsecond linne\r\n\r\n- itemm one\r\n";
        let words: Vec<Word> = note_words(source, NoteFormat::Markdown)
            .into_iter()
            .flat_map(|b| b.words)
            .collect();
        for word in &words {
            assert_eq!(&source[word.range.clone()], word.text);
        }
        let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["first", "linne", "second", "linne", "itemm", "one"]);
    }

    #[test]
    fn br023_apostrophes_join_and_hyphens_part() {
        let source = "don't l’homme well-known\n";
        let texts: Vec<String> = checked(source, NoteFormat::Markdown);
        assert_eq!(texts, ["don't", "l’homme", "well", "known"]);
    }

    #[test]
    fn br023_table_cells_are_checked_and_the_markup_is_not() {
        let source = "| Namme | Value |\n|---|---|\n| cel | `x` |\n";
        let texts = checked(source, NoteFormat::Markdown);
        assert_eq!(texts, ["Namme", "Value", "cel"]);
    }
}
