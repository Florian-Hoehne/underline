//! UC-004 BR-020 (FR-090, OQ-212, OQ-261): the text statistics of a note
//! and of its selection, counted on the text the facade shows.
//!
//! - Counted: the text of headings, paragraphs, list items, quotes and
//!   admonitions, the cells and the caption of a table, the caption of an
//!   image, and the text of code blocks.
//! - Not counted: the markup, the front matter of a Markdown note, link
//!   addresses (only a link's text is shown), blocks the facade shows as
//!   their markup (UC-005 BR-008, such as AsciiDoc attribute entries), and
//!   the sources of diagrams and formulas.
//! - A word is a run of letters and digits; a `'` (or `’`) or a `-` between
//!   two of them belongs to the word, so "It's" and "well-known" are one word
//!   each. Characters are the characters a reader sees (grapheme clusters),
//!   spaces counted, line breaks not (OQ-261).
//! - The reading time is the words divided by [`WORDS_PER_MINUTE`], rounded
//!   up to whole minutes.
//!
//! The selection counts the characters shown whose source lies within it, so
//! selecting `[fact](https://x.org)` in the source view counts "fact" as the
//! facade selection of "fact" does. A character of several code points is
//! selected when its first one is, so the selection never counts more than
//! the note. A caption, which has no source places, is selected only with
//! its whole block (OQ-261).
//!
//! Pure and without I/O, so it runs in the WASM editor core on every change
//! without a call to the backend (ADR-025, ADR-027). The note's counts and a
//! selection's are worked out apart ([`note_counts`], [`selection_counts`]),
//! so a moved selection costs only the blocks it touches.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::blocks::{Block, BlockKind, Run};
use crate::diagram::DiagramLanguage;
use crate::NoteFormat;

/// UC-004 BR-020: the reading speed the reading time is worked out with.
pub const WORDS_PER_MINUTE: usize = 200;

/// How many words and characters a text has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    /// Runs of letters and digits.
    pub words: usize,
    /// Characters a reader sees, spaces counted, line breaks not.
    pub characters: usize,
}

impl Counts {
    /// UC-004 BR-020: the words divided by [`WORDS_PER_MINUTE`], rounded up;
    /// at least one minute for a text with words, none without.
    #[must_use]
    pub fn reading_minutes(self) -> usize {
        self.words.div_ceil(WORDS_PER_MINUTE)
    }
}

/// The text statistics of a note, and of its selection while text is
/// selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Statistics {
    /// The whole note.
    pub note: Counts,
    /// The selection; `None` while nothing is selected.
    pub selection: Option<Counts>,
}

/// UC-004 BR-020: the statistics of `source`, a note in `format` split into
/// `blocks` (`parse_blocks`), with `selection` (source byte offsets; empty
/// or `None` for none) counted on its own.
///
/// # Examples
///
/// ```
/// use underline_markup::blocks::parse_blocks;
/// use underline_markup::statistics::text_statistics;
/// use underline_markup::NoteFormat;
///
/// let note = "# Title\n\nA [link](https://example.org) here.\n";
/// let blocks = parse_blocks(note, NoteFormat::Markdown);
/// let stats = text_statistics(note, NoteFormat::Markdown, &blocks, None);
/// assert_eq!(stats.note.words, 4);
/// assert_eq!(stats.note.reading_minutes(), 1);
/// ```
#[must_use]
pub fn text_statistics(
    source: &str,
    format: NoteFormat,
    blocks: &[Block],
    selection: Option<Range<usize>>,
) -> Statistics {
    Statistics {
        note: note_counts(source, format, blocks),
        selection: selection
            .filter(|range| range.start < range.end)
            .map(|range| selection_counts(source, format, blocks, range)),
    }
}

/// UC-004 BR-020: the words and characters of the whole note.
#[must_use]
pub fn note_counts(source: &str, format: NoteFormat, blocks: &[Block]) -> Counts {
    let skip = skipped_before(source, format);
    let mut counter = Counter::default();
    for block in blocks.iter().filter(|block| block.span.start >= skip) {
        for_each_piece(block, |piece| {
            match piece {
                Piece::Runs(runs) => {
                    for run in runs {
                        // FR-098: the source of a formula is not counted.
                        if run.style.formula.is_some() {
                            counter.end();
                            continue;
                        }
                        counter.text(&run.text);
                    }
                }
                Piece::Caption(caption) => counter.text(caption),
            }
            counter.end();
        });
    }
    counter.counts
}

/// UC-004 BR-020: the words and characters shown whose source lies within
/// `range` (byte offsets). Only the blocks the range touches are walked.
#[must_use]
pub fn selection_counts(
    source: &str,
    format: NoteFormat,
    blocks: &[Block],
    range: Range<usize>,
) -> Counts {
    let skip = skipped_before(source, format);
    let first = blocks.partition_point(|block| block.span.end < range.start);
    let mut counter = Counter::default();
    for block in blocks[first..]
        .iter()
        .take_while(|block| block.span.start <= range.end)
        .filter(|block| block.span.start >= skip)
    {
        for_each_piece(block, |piece| {
            match piece {
                Piece::Runs(runs) => {
                    for run in runs {
                        if run.style.formula.is_some() {
                            counter.end();
                            continue;
                        }
                        for (offset, grapheme) in placed_graphemes(run) {
                            if range.contains(&offset) {
                                counter.grapheme(grapheme);
                            } else {
                                counter.end();
                            }
                        }
                    }
                }
                // A caption has no source places of its own: it is selected
                // with the whole block.
                Piece::Caption(caption) => {
                    if range.start <= block.span.start && range.end >= content_end(source, block) {
                        counter.text(caption);
                    }
                }
            }
            counter.end();
        });
    }
    counter.counts
}

/// The graphemes of `run` with the source place of the first character of
/// each.
fn placed_graphemes(run: &Run) -> impl Iterator<Item = (usize, &str)> {
    let mut chars = 0;
    run.text.graphemes(true).map(move |grapheme| {
        let offset = run.offsets.get(chars).copied().unwrap_or(run.end);
        chars += grapheme.chars().count();
        (offset, grapheme)
    })
}

/// Where the counted text starts: after the front matter of a Markdown
/// note.
fn skipped_before(source: &str, format: NoteFormat) -> usize {
    match format {
        NoteFormat::Markdown => front_matter_end(source),
        NoteFormat::AsciiDoc => 0,
    }
}

/// A part of a block's shown text that a word cannot cross.
enum Piece<'a> {
    /// Text with its source places.
    Runs(&'a [Run]),
    /// Text without source places: the caption of a table or an image.
    Caption(&'a str),
}

/// Gives the shown text of `block` that is counted to `each`, in order.
fn for_each_piece<'a>(block: &'a Block, mut each: impl FnMut(Piece<'a>)) {
    match &block.kind {
        BlockKind::Heading(_)
        | BlockKind::Paragraph
        | BlockKind::ListItem { .. }
        | BlockKind::Quote(_)
        | BlockKind::Admonition(_) => each(Piece::Runs(&block.runs)),
        BlockKind::Code { language } if is_figure_source(language) => {}
        BlockKind::Code { .. } => each(Piece::Runs(&block.runs)),
        BlockKind::Table(table) => {
            if let Some(caption) = &table.caption {
                each(Piece::Caption(caption));
            }
            for cell in table.rows.iter().flatten() {
                each(Piece::Runs(cell));
            }
        }
        BlockKind::Image {
            caption: Some(caption),
            ..
        } => each(Piece::Caption(caption)),
        BlockKind::Image { caption: None, .. }
        | BlockKind::Markup
        | BlockKind::ThematicBreak
        | BlockKind::Formula { .. } => {}
    }
}

/// The languages of code blocks that hold formulas, as GitHub and AsciiDoc
/// name them (FR-098).
const FORMULA_LANGUAGES: [&str; 4] = ["math", "stem", "latexmath", "asciimath"];

/// Whether a code block in `language` is the source of a diagram or a
/// formula, which is not counted.
fn is_figure_source(language: &str) -> bool {
    DiagramLanguage::from_code_language(language).is_some()
        || FORMULA_LANGUAGES
            .iter()
            .any(|formula| formula.eq_ignore_ascii_case(language))
}

/// Where the text of `block` ends in `source`, without the line breaks after
/// it.
fn content_end(source: &str, block: &Block) -> usize {
    let span = block.span.start.min(source.len())..block.span.end.min(source.len());
    source
        .get(span.clone())
        .map_or(span.end, |text| span.start + text.trim_end().len())
}

/// The end of the front matter a Markdown note starts with: a line `---`,
/// the matter, and a line `---` or `...`; 0 without one. A byte order mark
/// before it is part of it.
///
/// ```
/// use underline_markup::statistics::front_matter_end;
///
/// assert_eq!(front_matter_end("---\na: b\n---\nText\n"), 13);
/// assert_eq!(front_matter_end("\u{feff}---\na: b\n---\n"), 16);
/// assert_eq!(front_matter_end("Text\n"), 0);
/// ```
#[must_use]
pub fn front_matter_end(source: &str) -> usize {
    let bom = if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let mut lines = source[bom..].split_inclusive('\n');
    let Some(first) = lines.next() else {
        return 0;
    };
    if first.trim_end() != "---" {
        return 0;
    }
    let mut at = bom + first.len();
    for line in lines {
        at += line.len();
        if matches!(line.trim_end(), "---" | "...") {
            return at;
        }
    }
    0
}

/// Counts words and characters of text given piece by piece.
#[derive(Debug, Default)]
struct Counter {
    counts: Counts,
    /// The last character seen belongs to a word.
    in_word: bool,
    /// A `'` or `-` follows a word: the word goes on if a letter or digit
    /// comes next.
    joined: bool,
}

impl Counter {
    /// Counts `text`, which goes on from the text before it.
    fn text(&mut self, text: &str) {
        for grapheme in text.graphemes(true) {
            self.grapheme(grapheme);
        }
    }

    /// Counts one character as a reader sees it.
    fn grapheme(&mut self, grapheme: &str) {
        if matches!(grapheme, "\n" | "\r\n" | "\r") {
            self.end();
            return;
        }
        self.counts.characters += 1;
        // A grapheme is never empty; a space stands in for none.
        let first = grapheme.chars().next().unwrap_or(' ');
        if first.is_alphanumeric() {
            if !self.in_word {
                self.counts.words += 1;
            }
            self.in_word = true;
            self.joined = false;
        } else if self.in_word && !self.joined && matches!(first, '\'' | '’' | '-') {
            self.joined = true;
        } else {
            self.end();
        }
    }

    /// No word goes on past here.
    fn end(&mut self) {
        self.in_word = false;
        self.joined = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::parse_blocks;

    fn stats(source: &str, format: NoteFormat, selection: Option<Range<usize>>) -> Statistics {
        let blocks = parse_blocks(source, format);
        text_statistics(source, format, &blocks, selection)
    }

    fn note(source: &str, format: NoteFormat) -> Counts {
        stats(source, format, None).note
    }

    fn counts(words: usize, characters: usize) -> Counts {
        Counts { words, characters }
    }

    const COUNT_MD: &str = "# Title\n\nIt's a well-known [fact](https://example.org/a-b-c).\n\n```rust\nlet x = 1;\n```\n";

    /// TC-004-39 step 1, UC-004 BR-020: eight words (Title, It's, a,
    /// well-known, fact, let, x, 1), the characters of the text shown
    /// without markup and link address, and a reading time of one minute.
    #[test]
    fn tc004_39_count_md_has_eight_words() {
        let counts = note(COUNT_MD, NoteFormat::Markdown);
        assert_eq!(counts.words, 8);
        // "Title" 5, "It's a well-known fact." 23, "let x = 1;" 10.
        assert_eq!(counts.characters, 38);
        assert_eq!(counts.reading_minutes(), 1);
    }

    /// TC-004-39 step 2: "well-known fact" selected is 2 words and 15
    /// characters, whether the source selection holds the link's markup
    /// or not.
    #[test]
    fn tc004_39_selection_counts_the_shown_text_within_it() {
        let from = COUNT_MD.find("well-known").unwrap();
        let fact = COUNT_MD.find("fact").unwrap();
        // As the facade selects it: up to the end of "fact".
        let shown = stats(COUNT_MD, NoteFormat::Markdown, Some(from..fact + 4));
        assert_eq!(shown.selection, Some(counts(2, 15)));
        // As the source view selects it: with the link's address.
        let to = COUNT_MD.find(").").unwrap() + 1;
        let source = stats(COUNT_MD, NoteFormat::Markdown, Some(from..to));
        assert_eq!(source.selection, Some(counts(2, 15)));
        assert_eq!(source.note, shown.note);
    }

    /// UC-004 BR-020: without a selection, or with an empty one, there are
    /// no statistics of the selection.
    #[test]
    fn uc004_br020_no_selection_without_selected_text() {
        assert_eq!(stats(COUNT_MD, NoteFormat::Markdown, None).selection, None);
        assert_eq!(
            stats(COUNT_MD, NoteFormat::Markdown, Some(5..5)).selection,
            None
        );
        let nothing_shown = stats(COUNT_MD, NoteFormat::Markdown, Some(0..2));
        assert_eq!(
            nothing_shown.selection,
            Some(counts(0, 0)),
            "only `# ` is selected"
        );
    }

    /// TC-004-39 step 3: 400 more words make a reading time of 3 minutes.
    #[test]
    fn tc004_39_reading_time_rounds_up() {
        let mut text = COUNT_MD.to_owned();
        text.push('\n');
        text.push_str(&"word ".repeat(400));
        assert_eq!(note(&text, NoteFormat::Markdown).reading_minutes(), 3);
        assert_eq!(counts(0, 0).reading_minutes(), 0);
        assert_eq!(counts(1, 1).reading_minutes(), 1);
        assert_eq!(counts(200, 1).reading_minutes(), 1);
        assert_eq!(counts(201, 1).reading_minutes(), 2);
    }

    /// UC-004 BR-020: the same note in AsciiDoc counts the same.
    #[test]
    fn uc004_br020_asciidoc_counts_the_shown_text() {
        let text = "= Title\n:toc:\n\nIt's a well-known https://example.org/a-b-c[fact].\n\n[source,rust]\n----\nlet x = 1;\n----\n\n// a comment\n";
        let counts = note(text, NoteFormat::AsciiDoc);
        assert_eq!(
            counts.words, 8,
            "the attribute entry and the comment are markup"
        );
        assert_eq!(counts.characters, 38);
    }

    /// OQ-212: front matter, diagrams and formulas are not counted; code is.
    #[test]
    fn oq212_front_matter_diagrams_and_formulas_are_left_out() {
        let text = "---\nstatus: draft\ntags: [a, b]\n---\n\nOne two.\n\n```mermaid\ngraph TD; A-->B\n```\n\n```math\nE = mc^2\n```\n\n```plantuml\n@startuml\nA -> B\n@enduml\n```\n\n    indented code\n";
        // "One two." 8; the indented code as the facade shows it, with its
        // four spaces, 17.
        assert_eq!(note(text, NoteFormat::Markdown), counts(4, 25));
        let adoc = "One two.\n\n[plantuml]\n----\nA -> B\n----\n";
        assert_eq!(note(adoc, NoteFormat::AsciiDoc).words, 2);
    }

    /// UC-004 BR-020, FR-098 (OQ-261): an inline formula and a block
    /// formula are typeset, so their sources are not counted, and a word
    /// does not run across an inline formula.
    #[test]
    fn uc004_br020_formulas_are_not_counted() {
        assert_eq!(
            note("Energy $E=mc^2$ is all.\n", NoteFormat::Markdown).words,
            3
        );
        assert_eq!(
            note("ab$x$cd and $$y$$\n\n$$z$$\n", NoteFormat::Markdown).words,
            3
        );
        assert_eq!(
            note("See latexmath:[b^2] here.\n", NoteFormat::AsciiDoc).words,
            2
        );
    }

    /// A Markdown note starting with a line `---` that never ends is no
    /// front matter.
    #[test]
    fn uc004_br020_front_matter_needs_its_end() {
        assert_eq!(front_matter_end("---\na: b\n---\nText\n"), 13);
        assert_eq!(front_matter_end("---\na: b\n...\n"), 13);
        assert_eq!(front_matter_end("---\na: b\n"), 0);
        assert_eq!(front_matter_end("Text\n---\n"), 0);
        assert_eq!(front_matter_end(""), 0);
    }

    /// UC-004 BR-020: tables count their cells and caption, not their
    /// markup; images their caption only.
    #[test]
    fn uc004_br020_tables_and_images() {
        let table = "| Name | Age |\n|---|---|\n| Ada | 36 |\n";
        assert_eq!(note(table, NoteFormat::Markdown), counts(4, 12));
        let image = ".The shot\nimage::shot.png[Alt text]\n";
        assert_eq!(note(image, NoteFormat::AsciiDoc), counts(2, 8));
        let md_image = "![Alt text](shot.png)\n";
        assert_eq!(note(md_image, NoteFormat::Markdown), counts(0, 0));
    }

    /// UC-004 BR-020: `'` and `-` join only between letters or digits;
    /// other signs and line breaks end a word.
    #[test]
    fn uc004_br020_word_rule() {
        let words = |text: &str| note(text, NoteFormat::Markdown).words;
        assert_eq!(words("rock'n'roll it’s re-do"), 3);
        assert_eq!(words("'quoted' and -dash- and a--b"), 6);
        assert_eq!(words("snake_case end. 3.14 v2"), 6);
        assert_eq!(words("first\nsecond"), 2);
        assert_eq!(words("**bold**text and *it*alic"), 3);
    }

    /// UC-004 BR-020, OQ-261: characters are what a reader sees: an accent
    /// written as its own code point, an emoji with its skin tone and a
    /// family of emoji are one character each; line breaks are not counted.
    #[test]
    fn oq261_multibyte_characters() {
        let text = "Cafe\u{301} 👍🏽 👨‍👩‍👧\n\nÜber straße";
        let shown = note(text, NoteFormat::Markdown);
        assert_eq!(shown.words, 3);
        assert_eq!(shown.characters, 8 + 11);
        let selection = stats(text, NoteFormat::Markdown, Some(0..text.find(' ').unwrap()));
        assert_eq!(selection.selection, Some(counts(1, 4)));
    }

    /// OQ-261: a run of CJK letters is one word, as the rule reads; every
    /// character counts once.
    #[test]
    fn oq261_cjk_text() {
        let counts = note("日本語のテキスト。中文 text", NoteFormat::Markdown);
        assert_eq!(counts.words, 3);
        assert_eq!(counts.characters, 16);
    }

    /// No offsets or selections past the end make it panic.
    #[test]
    fn uc004_br020_hostile_selections() {
        #[allow(clippy::reversed_empty_ranges)]
        let ranges = [0..usize::MAX, 1000..2000, 3..1];
        for range in ranges {
            let _ = stats(COUNT_MD, NoteFormat::Markdown, Some(range));
        }
        assert_eq!(note("", NoteFormat::AsciiDoc), counts(0, 0));
    }

    /// UC-004 BR-020, OQ-261 (test monkey): a selection is part of the
    /// note, so it never counts more characters than the note, even when it
    /// starts inside an emoji family (the ZWJ then stands alone).
    #[test]
    fn oq261_selection_inside_a_cluster_is_not_more_than_the_note() {
        let text = "👨\u{200d}👩\u{200d}👧";
        let whole = stats(text, NoteFormat::Markdown, Some(0..text.len()));
        assert_eq!(whole.note.characters, 1);
        let from = "👨".len();
        let part = stats(text, NoteFormat::Markdown, Some(from..text.len()));
        let selected = part.selection.unwrap();
        assert!(
            selected.characters <= part.note.characters,
            "selection {selected:?} of note {:?}",
            part.note
        );
    }

    /// UC-004 BR-020, TC-004-39 step 2 (test monkey): selecting the whole
    /// source counts the whole note, also for a table whose header row
    /// starts with a space (the body cells' source places are off).
    #[test]
    fn tc004_39_selecting_everything_counts_the_whole_table() {
        let text = " a|b\n-|-\nb|c\n";
        let all = stats(text, NoteFormat::Markdown, Some(0..text.len()));
        assert_eq!(all.note.words, 4);
        assert_eq!(all.selection, Some(all.note));
        // "b|c" in the source is the body row.
        let row = stats(text, NoteFormat::Markdown, Some(9..12));
        assert_eq!(row.selection.map(|c| c.words), Some(2));
    }

    /// UC-004 BR-020, OQ-261 (test monkey): the line break style does not
    /// change what is shown, so it does not change the counts.
    #[test]
    fn oq261_crlf_counts_like_lf() {
        let lf = ")```\n) ```";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(
            note(&crlf, NoteFormat::Markdown),
            note(lf, NoteFormat::Markdown)
        );
    }

    /// OQ-261, review: a caption is selected with its whole block only; a
    /// part of a table counts the cells selected, not the caption.
    #[test]
    fn oq261_captions_in_a_selection() {
        let adoc = ".Ages\n|===\n| Ada | 36\n|===\n";
        let all = stats(adoc, NoteFormat::AsciiDoc, Some(0..adoc.len()));
        assert_eq!(all.note, counts(3, 9));
        assert_eq!(all.selection, Some(all.note));
        let row = adoc.find("| Ada").unwrap();
        let part = stats(adoc, NoteFormat::AsciiDoc, Some(row..row + 10));
        assert_eq!(part.selection, Some(counts(2, 5)));

        let image = ".The shot\nimage::shot.png[Alt text]\n";
        let whole = stats(image, NoteFormat::AsciiDoc, Some(0..image.len()));
        assert_eq!(whole.selection, Some(counts(2, 8)));
        let line = image.find("image::").unwrap();
        let only_macro = stats(image, NoteFormat::AsciiDoc, Some(line..image.len()));
        assert_eq!(only_macro.selection, Some(counts(0, 0)));

        let md = "| Name | Age |\n|---|---|\n| Ada | 36 |\n";
        let body = md.find("| Ada").unwrap();
        let cells = stats(md, NoteFormat::Markdown, Some(body..md.len()));
        assert_eq!(cells.selection, Some(counts(2, 5)));
    }

    /// OQ-261: a byte order mark before the front matter keeps it front
    /// matter.
    #[test]
    fn oq261_front_matter_after_a_byte_order_mark() {
        let text = "\u{feff}---\nstatus: draft\n---\n\nOne two.\n";
        assert_eq!(note(text, NoteFormat::Markdown), counts(2, 8));
    }
}
