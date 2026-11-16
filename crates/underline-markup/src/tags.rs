//! FR-091, FR-092: the tags of a note (UC-004 BR-016, UC-024 BR-005) and
//! the tag words of a search (UC-008 BR-012).
//!
//! - A Markdown note carries a tag in its text as `#` followed directly by
//!   the tag, where the `#` does not follow a letter, a digit or `&`; code,
//!   link addresses, diagrams and formulas hold none, and neither does the
//!   front matter. A `#` followed by a space starts a heading.
//! - Every note carries the tags of its note attribute `tags`: the items of
//!   a list, or the parts of a text between commas (OQ-213: an AsciiDoc note
//!   carries tags only there, as `#` marks highlighted text in AsciiDoc).
//! - A tag holds letters, digits, `_`, `-` and `/`, and at least one
//!   character that is not a digit. Case does not matter; a `/` makes a tag
//!   a part of the tag before it.

use std::ops::Range;

use crate::attributes::{note_head, AttributeKind, NoteHead};
use crate::blocks::{parse_blocks, Block, BlockKind, Run};
use crate::search::split_words_quoted;
use crate::statistics::front_matter_end;
use crate::NoteFormat;

/// Where a note carries a tag (entity NOTE_TAG, origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagOrigin {
    /// `#tag` in the text of a Markdown note.
    Text,
    /// An item of the note attribute `tags`.
    Attribute,
}

/// A tag a note carries, and where.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagPlace {
    /// The tag as written, without its `#`.
    pub tag: String,
    /// The tag in lower case, as tags are compared.
    pub key: String,
    /// The bytes of the note holding it: the `#tag` in the text, the value
    /// of the attribute `tags`.
    pub range: Range<usize>,
    /// Where the note carries it.
    pub origin: TagOrigin,
}

/// A character a tag may hold.
fn tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// UC-004 BR-016: whether `text` (without `#`) is a tag: tag characters
/// only, not starting or ending with `/`, and at least one character that
/// is not a digit (of any script). A `/` at the end is no part of a tag:
/// every reader trims it first ([`trimmed_tag`]).
///
/// ```
/// use underline_markup::tags::is_tag;
///
/// assert!(is_tag("project/alpha"));
/// assert!(!is_tag("2026"));
/// assert!(!is_tag("a b"));
/// ```
#[must_use]
pub fn is_tag(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(tag_char)
        && !text.starts_with('/')
        && !text.ends_with('/')
        && text.chars().any(|c| !c.is_numeric() && c != '/')
}

/// A tag as written, without the `/` at its end (`#project/` is `project`),
/// as the text, the attribute `tags` and a search read it alike.
#[must_use]
pub fn trimmed_tag(text: &str) -> &str {
    text.trim_end_matches('/')
}

/// The key of a tag: in lower case, so `#Project` and `#project` are one.
#[must_use]
pub fn tag_key(tag: &str) -> String {
    tag.to_lowercase()
}

/// UC-008 BR-012: whether a note's tag `key` is the tag `wanted` (a key)
/// or a tag below it.
///
/// ```
/// use underline_markup::tags::tag_matches;
///
/// assert!(tag_matches("project/alpha", "project"));
/// assert!(!tag_matches("projects", "project"));
/// ```
#[must_use]
pub fn tag_matches(key: &str, wanted: &str) -> bool {
    key == wanted
        || key
            .strip_prefix(wanted)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `source` may hold a tag in its text: a `#` followed by a tag
/// character, not after a letter, a digit or `&`. A quick look before the
/// note is parsed.
#[must_use]
pub fn may_hold_text_tags(source: &str) -> bool {
    source.match_indices('#').any(|(at, _)| {
        let after = source[at + 1..].chars().next().is_some_and(tag_char);
        let before = source[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '&'));
        after && before
    })
}

/// FR-091: the tags of `source`, a note in `format`: those of its note
/// attribute `tags` first, then those of its text, in the order of the
/// note.
#[must_use]
pub fn note_tags(source: &str, format: NoteFormat) -> Vec<TagPlace> {
    let head = note_head(source, format);
    let mut places = attribute_tags(&head, format);
    if format == NoteFormat::Markdown && may_hold_text_tags(source) {
        places.extend(text_tags(source, &parse_blocks(source, format)));
    }
    places
}

/// [`note_tags`] with the blocks of the note at hand.
#[must_use]
pub fn note_tags_with(
    source: &str,
    format: NoteFormat,
    head: &NoteHead,
    blocks: &[Block],
) -> Vec<TagPlace> {
    let mut places = attribute_tags(head, format);
    if format == NoteFormat::Markdown {
        places.extend(text_tags(source, blocks));
    }
    places
}

/// UC-024 BR-005: the tags of the note attribute `tags`.
#[must_use]
pub fn attribute_tags(head: &NoteHead, format: NoteFormat) -> Vec<TagPlace> {
    let Some(attribute) = head.attribute("tags", format) else {
        return Vec::new();
    };
    let items: Vec<&str> = match attribute.kind {
        AttributeKind::List => attribute.items.iter().map(String::as_str).collect(),
        AttributeKind::Text => attribute.value.split(',').collect(),
        _ => Vec::new(),
    };
    let range = if attribute.value_span.is_empty() {
        attribute.span.clone()
    } else {
        attribute.value_span.clone()
    };
    items
        .into_iter()
        .map(|item| trimmed_tag(item.trim().trim_start_matches('#')))
        .filter(|item| is_tag(item))
        .map(|tag| TagPlace {
            tag: tag.to_owned(),
            key: tag_key(tag),
            range: range.clone(),
            origin: TagOrigin::Attribute,
        })
        .collect()
}

/// UC-004 BR-016: the tags in the text of a Markdown note whose blocks are
/// `blocks`: in its text blocks and table cells, not in code, link
/// addresses, diagrams, formulas or the front matter.
#[must_use]
pub fn text_tags(source: &str, blocks: &[Block]) -> Vec<TagPlace> {
    let skip = front_matter_end(source);
    let mut places = Vec::new();
    for block in blocks.iter().filter(|block| block.span.start >= skip) {
        match &block.kind {
            BlockKind::Heading(_)
            | BlockKind::Paragraph
            | BlockKind::ListItem { .. }
            | BlockKind::Quote(_)
            | BlockKind::Admonition(_) => {
                for run in &block.runs {
                    run_tags(source, run, &mut places);
                }
            }
            BlockKind::Table(table) => {
                for run in table.rows.iter().flatten().flatten() {
                    run_tags(source, run, &mut places);
                }
            }
            BlockKind::Code { .. }
            | BlockKind::ThematicBreak
            | BlockKind::Markup
            | BlockKind::Image { .. }
            | BlockKind::Formula { .. } => {}
        }
    }
    places
}

/// The tags of one run of text, by its source places.
fn run_tags(source: &str, run: &Run, places: &mut Vec<TagPlace>) {
    if run.style.code
        || run.style.formula.is_some()
        || run.style.link.as_deref() == Some(run.text.as_str())
    {
        // Code, a formula (FR-098), and an address shown as its own link,
        // hold no tags.
        return;
    }
    let chars: Vec<char> = run.text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let Some(&at) = run.offsets.get(i) else {
            return;
        };
        if chars[i] != '#' || source.as_bytes().get(at) != Some(&b'#') {
            i += 1;
            continue;
        }
        let before = source[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '&'));
        // The tag's characters follow one another in the source too.
        let mut end = i + 1;
        while end < chars.len()
            && tag_char(chars[end])
            && run.offsets.get(end).copied()
                == run
                    .offsets
                    .get(end - 1)
                    .map(|o| o + chars[end - 1].len_utf8())
        {
            end += 1;
        }
        let written: String = chars[i + 1..end].iter().collect();
        let tag = trimmed_tag(&written).to_owned();
        if before && is_tag(&tag) {
            let start = at;
            places.push(TagPlace {
                key: tag_key(&tag),
                range: start..start + 1 + tag.len(),
                tag,
                origin: TagOrigin::Text,
            });
        }
        i = end.max(i + 1);
    }
}

/// FR-091, OQ-265: the tags of a note as the tool panel lists them: in
/// lower case, each once, in the order of `places`.
#[must_use]
pub fn tag_names(places: &[TagPlace]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    places
        .iter()
        .filter(|place| seen.insert(place.key.as_str()))
        .map(|place| place.key.clone())
        .collect()
}

/// UC-008 BR-012: the words of a search text that search for a tag (`#`
/// followed by a tag, not in quotes), as keys, and the search text without
/// them. With the switch regular expression on, `#` has no meaning of its
/// own: the caller does not ask then.
///
/// ```
/// use underline_markup::tags::tag_words;
///
/// let (tags, rest) = tag_words("#Travel ferry \"#not a tag\" #42");
/// assert_eq!(tags, ["travel"]);
/// assert_eq!(rest, "ferry \"#not a tag\" #42");
/// ```
#[must_use]
pub fn tag_words(text: &str) -> (Vec<String>, String) {
    let mut tags = Vec::new();
    let mut rest = Vec::new();
    for (word, quoted) in split_words_quoted(text) {
        let tag = word.strip_prefix('#').map(trimmed_tag);
        match tag {
            Some(tag) if !quoted && is_tag(tag) => {
                let key = tag_key(tag);
                if !tags.contains(&key) {
                    tags.push(key);
                }
            }
            _ if quoted => rest.push(format!("\"{word}\"")),
            _ => rest.push(word),
        }
    }
    (tags, rest.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(source: &str, format: NoteFormat) -> Vec<String> {
        tag_names(&note_tags(source, format))
    }

    /// TC-004-32 step 1: tags of the text and of the attribute; not issue
    /// numbers, `C#`, code or link addresses.
    #[test]
    fn tc004_32_tags_of_a_markdown_note() {
        let note = "---\ntags: [Travel]\n---\nFerry to #hydra and #Project/alpha, issue #12, C#, `#code`, [link](#ferry)\n";
        assert_eq!(
            keys(note, NoteFormat::Markdown),
            ["travel", "hydra", "project/alpha"]
        );
    }

    /// TC-004-32 step 3: an AsciiDoc note carries tags only in `:tags:`
    /// (OQ-213); `#highlight#` is highlighted text.
    #[test]
    fn tc004_32_tags_of_an_asciidoc_note() {
        let note = "= Guide\n:tags: travel, docs\n\na #highlight# word\n";
        assert_eq!(keys(note, NoteFormat::AsciiDoc), ["travel", "docs"]);
    }

    /// UC-004 BR-016, FR-098 (OQ-267): a formula holds no tags.
    #[test]
    fn uc004_br016_formulas_hold_no_tags() {
        let note = "See $\\#x$ and #real, $$#y$$\n\n$$#z$$\n";
        assert_eq!(keys(note, NoteFormat::Markdown), ["real"]);
    }

    /// UC-004 BR-016: a heading is no tag; a tag in a heading is; code
    /// blocks, diagrams and the front matter's comments hold none.
    #[test]
    fn uc004_br016_where_tags_are() {
        let note = "# Title #topic\n\n#start of a line\n\n```\n#nope\n```\n\n```mermaid\n#no\n```\n\n- item #listed\n> quoted #quote\n";
        assert_eq!(
            keys(note, NoteFormat::Markdown),
            ["topic", "start", "listed", "quote"]
        );
        let front = "---\n# comment\ntitle: x #y\n---\ntext\n";
        assert!(keys(front, NoteFormat::Markdown).is_empty());
    }

    /// UC-004 BR-016: `&#35;` and `x#y` are no tags; a web address shown as
    /// its own link holds none.
    #[test]
    fn uc004_br016_not_after_a_letter_or_ampersand() {
        let note = "x#y and &#35; and https://example.org/#part and (#ok)\n";
        assert_eq!(keys(note, NoteFormat::Markdown), ["ok"]);
    }

    /// UC-004 BR-016: tags in other scripts, with multibyte characters, and
    /// a trailing `/` left out; their places are bytes of the note.
    #[test]
    fn uc004_br016_multibyte_tags_and_places() {
        let note = "Über #straße und #日本/東京/ und #ü-1\r\n";
        let places = note_tags(note, NoteFormat::Markdown);
        let tags: Vec<&str> = places.iter().map(|p| p.tag.as_str()).collect();
        assert_eq!(tags, ["straße", "日本/東京", "ü-1"]);
        for place in &places {
            assert_eq!(&note[place.range.clone()], format!("#{}", place.tag));
        }
    }

    /// UC-024 BR-005: the attribute `tags` as a list one per line, or as a
    /// text with commas; `#` before an item and items that are no tags.
    #[test]
    fn uc024_br005_attribute_tags() {
        let block = "---\ntags:\n  - one\n  - \"#two\"\n  - 42\n---\n";
        assert_eq!(keys(block, NoteFormat::Markdown), ["one", "two"]);
        let text = "---\ntags: a, B\n---\n";
        assert_eq!(keys(text, NoteFormat::Markdown), ["a", "b"]);
    }

    /// OQ-214: one tag in different cases is listed once, in lower case.
    #[test]
    fn oq214_one_tag_in_different_cases() {
        let note = "---\ntags: [Travel]\n---\n#travel #TRAVEL\n";
        assert_eq!(keys(note, NoteFormat::Markdown), ["travel"]);
    }

    /// TC-008-21: tag words of a search, the tags below a tag.
    #[test]
    fn tc008_21_tag_words() {
        let (tags, rest) = tag_words("#travel ferry");
        assert_eq!((tags, rest.as_str()), (vec!["travel".to_owned()], "ferry"));
        let (tags, rest) = tag_words("#42");
        assert!(tags.is_empty());
        assert_eq!(rest, "#42");
        assert!(tag_matches("travel/greece", "travel"));
        assert!(!tag_matches("travelling", "travel"));
    }

    /// The quick look finds what the parse could find.
    #[test]
    fn the_quick_look() {
        assert!(may_hold_text_tags("a #b"));
        assert!(!may_hold_text_tags("# Heading\nC# and a#b"));
    }

    /// UC-004 BR-016 (test monkey, package 6a): a tag after an entity such as
    /// `&copy;` or `&lt;` at the start of a block is a tag; the entity's text
    /// has no places of its own, which must not hide the text after it.
    #[test]
    fn uc004_br016_tag_after_an_entity_at_the_start_of_a_block() {
        for note in ["&copy; #a\n", "&lt;#a\n", "- &nbsp;#a\n", "&gt; #a\n"] {
            assert_eq!(keys(note, NoteFormat::Markdown), ["a"], "{note:?}");
        }
        // The same entity later in the block works.
        assert_eq!(keys("x &amp; #a\n", NoteFormat::Markdown), ["a"]);
    }

    /// UC-004 BR-016 (test monkey, package 6a): the place of a tag is its
    /// `#tag` in the note, also in the line after a table's rows when it
    /// holds a character such as U+2028.
    #[test]
    fn uc004_br016_places_after_a_table_with_a_line_separator() {
        let note = "| a |\n|---|\n| c |\n\u{2028}a#b #a\n";
        let places = note_tags(note, NoteFormat::Markdown);
        let found: Vec<(&str, &str)> = places
            .iter()
            .map(|p| (p.tag.as_str(), &note[p.range.clone()]))
            .collect();
        assert_eq!(found, [("a", "#a")]);
    }
}
