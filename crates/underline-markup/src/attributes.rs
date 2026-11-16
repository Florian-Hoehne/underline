//! UC-024 Edit Note Attributes (FR-093, FR-091): the note attributes at
//! the head of a note, read from its markup and changed in it.
//!
//! - BR-001: in a Markdown note they are the front matter, a block at the
//!   very start between two lines `---`, one `name: value` per entry, in the
//!   YAML GitHub reads (C-003); in an AsciiDoc note the document attributes
//!   `:name: value` of the header, below the document title or at the start
//!   of a note without one (C-004).
//! - BR-002: the kinds of their values, told from the way they are written.
//! - BR-003: a change writes only the line or lines of that attribute; the
//!   front matter is edited as lines of text, never written out again from
//!   a parsed YAML value, so comments, blank lines, the order and the way
//!   the other values are written stay byte for byte (NFR-007). A new
//!   attribute follows the last one.
//! - BR-004: the names allowed.
//! - A1, A2: the first attribute makes the head; removing the last one
//!   removes the front matter.
//! - A5: a front matter that is no YAML GitHub reads is never changed: it is
//!   told with its line and the reason ([`HeadProblem`]).
//! - A6: a value with fields of its own, a list of such fields, or a value
//!   spread over lines is [`AttributeKind::Nested`], shown as its markup and
//!   changed only in the source view (OQ-215); keys this module does not
//!   understand are kept as they are.
//!
//! Nothing here reads files: the window's editor core reads the attributes
//! of the text it owns and writes a change as one edit of it (ADR-025,
//! UC-024 BR-006).

use std::ops::Range;

use crate::NoteFormat;

/// BR-004: the longest name of a note attribute, in characters.
pub const NAME_MOST: usize = 100;

/// UC-024 BR-002: what the value of a note attribute holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeKind {
    /// Any text on one line.
    Text,
    /// Items in square brackets or one per line after a dash; in AsciiDoc
    /// separated by commas.
    List,
    /// A whole or decimal number.
    Number,
    /// Year, month and day as `2026-11-11`.
    Date,
    /// `true` or `false`.
    YesNo,
    /// Fields of its own, or anything else spread over lines: shown as its
    /// markup and changed only in the source view (A6).
    Nested,
}

/// A note attribute as the note holds it (entity NOTE_ATTRIBUTE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteAttribute {
    /// Its name as written; in AsciiDoc in lower case.
    pub name: String,
    /// What its value holds (BR-002).
    pub kind: AttributeKind,
    /// The value as shown: the text without its quotes, the number or the
    /// date as written, `true` or `false`; the markup of its lines for
    /// [`AttributeKind::Nested`]; the items joined by `, ` for a list.
    pub value: String,
    /// The items of a list, without their quotes.
    pub items: Vec<String>,
    /// The bytes of its lines in the note, their line breaks included.
    pub span: Range<usize>,
    /// The bytes of its value in the note (empty where it has none).
    pub value_span: Range<usize>,
    /// The line it starts on, counted from 1.
    pub line: usize,
}

/// UC-024 A5: why the head of a note cannot be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadProblem {
    /// The line of the note the problem is in, counted from 1.
    pub line: usize,
    /// What is wrong, in a few words.
    pub reason: String,
    /// The text of the front matter, its two lines `---` included.
    pub markup: String,
}

/// The note attributes of a note (UC-024 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteHead {
    /// The attributes in the order of the note.
    pub attributes: Vec<NoteAttribute>,
    /// Why the head cannot be read; then there are no attributes (A5).
    pub problem: Option<HeadProblem>,
    /// The bytes of the front matter, its two lines `---` included; `None`
    /// without one, and always in an AsciiDoc note.
    pub front_matter: Option<Range<usize>>,
}

impl NoteHead {
    /// The attribute named `name` (in AsciiDoc in any case).
    #[must_use]
    pub fn attribute(&self, name: &str, format: NoteFormat) -> Option<&NoteAttribute> {
        let name = written_name(name, format);
        self.attributes.iter().find(|a| a.name == name)
    }
}

/// UC-024 step 3: a value given for a note attribute, of the kind it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// A text on one line.
    Text(String),
    /// The items of a list.
    List(Vec<String>),
    /// The number as typed.
    Number(String),
    /// The date as typed, `2026-11-11`.
    Date(String),
    /// Yes (`true`) or no (`false`).
    YesNo(bool),
}

impl AttributeValue {
    /// The kind of the value.
    #[must_use]
    pub fn kind(&self) -> AttributeKind {
        match self {
            Self::Text(_) => AttributeKind::Text,
            Self::List(_) => AttributeKind::List,
            Self::Number(_) => AttributeKind::Number,
            Self::Date(_) => AttributeKind::Date,
            Self::YesNo(_) => AttributeKind::YesNo,
        }
    }
}

/// UC-024 step 3: what the user does to the note attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeChange {
    /// Adds an attribute after the last one (A1 for the first).
    Add {
        /// Its name, as typed.
        name: String,
        /// Its value.
        value: AttributeValue,
    },
    /// Gives the attribute `name` the value `value`.
    Set {
        /// Its name.
        name: String,
        /// The new value.
        value: AttributeValue,
    },
    /// Removes the attribute `name` (A2 for the last).
    Remove {
        /// Its name.
        name: String,
    },
}

/// One edit of the note: the bytes `range` become `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadEdit {
    /// The bytes of the note replaced.
    pub range: Range<usize>,
    /// What they become.
    pub text: String,
}

/// Why a change of the note attributes is not written (A3, A4, A5, A6).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttributeError {
    /// A3: the name is empty.
    #[error("A name is needed.")]
    NoName,
    /// A3, BR-004: the name holds a space.
    #[error("A name cannot hold a space.")]
    Space,
    /// A3, BR-004: the name does not start with a letter.
    #[error("A name starts with a letter.")]
    Start,
    /// A3, BR-004: the name holds a character not allowed.
    #[error("A name holds only letters, digits, _ and -, not “{0}”.")]
    Character(char),
    /// A3, BR-004: the name is too long.
    #[error("A name has at most 100 characters.")]
    TooLong,
    /// A3: the note has an attribute of that name.
    #[error("The note has the attribute “{0}” already.")]
    Taken(String),
    /// A4: the value is no number.
    #[error("A number is expected, such as 2 or 2.5.")]
    NotANumber,
    /// A4: the value is no date.
    #[error("A date is expected, as year-month-day, such as 2026-11-11.")]
    NotADate,
    /// BR-002: a text or an item holds a line break.
    #[error("A value stays on one line.")]
    LineBreak,
    /// BR-002: an AsciiDoc note holds only texts and lists.
    #[error("An AsciiDoc note holds only text and list attributes.")]
    Kind,
    /// BR-002: an item of an AsciiDoc list holds a comma, which separates
    /// the items.
    #[error("An item of an AsciiDoc list cannot hold a comma.")]
    Comma,
    /// OQ-264: an AsciiDoc value ending in `\` goes on in the next line.
    #[error("An AsciiDoc value cannot end in \\.")]
    Continuation,
    /// OQ-264: the attribute before the new one ends in `\`, so it would
    /// take the new one in.
    #[error("The attribute before ends in \\ and goes on in the next line; change it in the source view first.")]
    ContinuedBefore,
    /// BR-004: the name in lower case, as AsciiDoc writes it, holds a
    /// character a name cannot hold.
    #[error(
        "In lower case, as AsciiDoc writes names, “{0}” holds a character a name cannot hold."
    )]
    LowerCase(String),
    /// BR-005, OQ-270: an item of the attribute `tags` is no tag.
    #[error("“{0}” is no tag: a tag holds letters, digits, _, - and /, and not only digits.")]
    NotATag(String),
    /// A6: the attribute holds a nested value.
    #[error("“{0}” can be changed only in the source view.")]
    Nested(String),
    /// The note has no attribute of that name (changed meanwhile, A7).
    #[error("The note has no attribute “{0}”.")]
    Missing(String),
    /// A5: the head of the note cannot be read.
    #[error("The front matter cannot be read, so it is not changed.")]
    Unreadable,
}

/// What a refused change is about, so the field in question is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeField {
    /// The name of a new attribute.
    Name,
    /// The value.
    Value,
    /// The note as a whole (its head, an attribute gone).
    Note,
}

impl AttributeError {
    /// The field the refusal is about (UC-024 A3, A4).
    #[must_use]
    pub fn about(&self) -> AttributeField {
        match self {
            Self::NoName
            | Self::Space
            | Self::Start
            | Self::Character(_)
            | Self::TooLong
            | Self::Taken(_)
            | Self::LowerCase(_) => AttributeField::Name,
            Self::NotANumber
            | Self::NotADate
            | Self::LineBreak
            | Self::Kind
            | Self::Comma
            | Self::Continuation
            | Self::NotATag(_) => AttributeField::Value,
            Self::ContinuedBefore | Self::Nested(_) | Self::Missing(_) | Self::Unreadable => {
                AttributeField::Note
            }
        }
    }
}

/// UC-024 BR-005, OQ-270: every item of a value for the attribute `tags`
/// is a tag (a `#` before it and a `/` after it allowed).
fn check_tags(
    name: &str,
    format: NoteFormat,
    value: &AttributeValue,
) -> Result<(), AttributeError> {
    if written_name(name.trim(), format) != "tags" {
        return Ok(());
    }
    let items: Vec<&str> = match value {
        AttributeValue::List(items) => items.iter().map(String::as_str).collect(),
        AttributeValue::Text(text) => text.split(',').collect(),
        _ => Vec::new(),
    };
    for item in items {
        let tag = crate::tags::trimmed_tag(item.trim().trim_start_matches('#'));
        if !tag.is_empty() && !crate::tags::is_tag(tag) {
            return Err(AttributeError::NotATag(item.trim().to_owned()));
        }
    }
    Ok(())
}

/// The note attributes of `source`, a note in `format` (UC-024 step 2).
///
/// ```
/// use underline_markup::attributes::{note_head, AttributeKind};
/// use underline_markup::NoteFormat;
///
/// let head = note_head("---\nstatus: draft\ndue: 2026-12-01\n---\n# Plan\n", NoteFormat::Markdown);
/// assert_eq!(head.attributes[0].value, "draft");
/// assert_eq!(head.attributes[1].kind, AttributeKind::Date);
/// ```
#[must_use]
pub fn note_head(source: &str, format: NoteFormat) -> NoteHead {
    match format {
        NoteFormat::Markdown => markdown_head(source),
        NoteFormat::AsciiDoc => NoteHead {
            attributes: asciidoc_header(source).attributes,
            ..NoteHead::default()
        },
    }
}

/// UC-024 BR-004: the name `typed` as the note writes it (in AsciiDoc in
/// lower case), when it is allowed.
///
/// # Errors
///
/// A3: the name is empty, holds a character not allowed, does not start
/// with a letter, or is too long.
pub fn checked_name(typed: &str, format: NoteFormat) -> Result<String, AttributeError> {
    let name = typed.trim();
    let Some(first) = name.chars().next() else {
        return Err(AttributeError::NoName);
    };
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_alphanumeric() || *c == '_' || *c == '-'))
    {
        return Err(if bad.is_whitespace() {
            AttributeError::Space
        } else {
            AttributeError::Character(bad)
        });
    }
    if !first.is_alphabetic() {
        return Err(AttributeError::Start);
    }
    if name.chars().count() > NAME_MOST {
        return Err(AttributeError::TooLong);
    }
    let written = written_name(name, format);
    if written
        .chars()
        .any(|c| !(c.is_alphanumeric() || c == '_' || c == '-'))
    {
        return Err(AttributeError::LowerCase(name.to_owned()));
    }
    Ok(written)
}

/// A name as `format` writes it: AsciiDoc in lower case (BR-004).
fn written_name(name: &str, format: NoteFormat) -> String {
    match format {
        NoteFormat::Markdown => name.to_owned(),
        NoteFormat::AsciiDoc => name.to_lowercase(),
    }
}

/// UC-024 A4, BR-002: whether `value` fits its kind.
///
/// # Errors
///
/// A number or a date that is none, or a text or item with a line break.
pub fn check_value(value: &AttributeValue) -> Result<(), AttributeError> {
    let one_line = |text: &str| {
        if text.contains(['\n', '\r']) {
            Err(AttributeError::LineBreak)
        } else {
            Ok(())
        }
    };
    match value {
        AttributeValue::Text(text) => one_line(text),
        AttributeValue::List(items) => items.iter().try_for_each(|item| one_line(item)),
        AttributeValue::Number(number) if is_number(number.trim()) => Ok(()),
        AttributeValue::Number(_) => Err(AttributeError::NotANumber),
        AttributeValue::Date(date) if is_date(date.trim()) => Ok(()),
        AttributeValue::Date(_) => Err(AttributeError::NotADate),
        AttributeValue::YesNo(_) => Ok(()),
    }
}

/// BR-002: a whole or decimal number as both YAML 1.1 (GitHub) and YAML
/// 1.2 read one: digits with at most one point, a sign before. An exponent
/// is not taken: YAML 1.1 reads `1e5` as a text, 1.2 as a number, so such a
/// value is neither offered as a number nor written plainly as a text.
fn is_number(text: &str) -> bool {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (digits, None),
    };
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    all_digits(whole)
        && fraction.is_none_or(all_digits)
        && (!whole.is_empty() || fraction.is_some_and(|f| !f.is_empty()))
}

/// BR-002: a date `YYYY-MM-DD` that exists.
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let number = |range: Range<usize>| -> Option<u32> {
        let part = text.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (Some(year), Some(month), Some(day)) = (number(0..4), number(5..7), number(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// The edit of `source`, a note in `format`, that makes `change` (UC-024
/// step 4, BR-003).
///
/// # Errors
///
/// A3 to A6: the name or the value is not allowed, the attribute holds a
/// nested value, or the front matter cannot be read.
///
/// ```
/// use underline_markup::attributes::{head_edit, AttributeChange, AttributeValue};
/// use underline_markup::NoteFormat;
///
/// let note = "---\nstatus: draft # for now\n---\n";
/// let change = AttributeChange::Set {
///     name: "status".into(),
///     value: AttributeValue::Text("final".into()),
/// };
/// let edit = head_edit(note, NoteFormat::Markdown, &change).unwrap();
/// let mut changed = note.to_owned();
/// changed.replace_range(edit.range, &edit.text);
/// assert_eq!(changed, "---\nstatus: final # for now\n---\n");
/// ```
pub fn head_edit(
    source: &str,
    format: NoteFormat,
    change: &AttributeChange,
) -> Result<HeadEdit, AttributeError> {
    if let AttributeChange::Add { name, value } | AttributeChange::Set { name, value } = change {
        check_tags(name, format, value)?;
    }
    match format {
        NoteFormat::Markdown => markdown_edit(source, change),
        NoteFormat::AsciiDoc => asciidoc_edit(source, change),
    }
}

/// The line break the note uses: `\r\n` when its first line ends so.
fn line_break(source: &str) -> &'static str {
    match source.find('\n') {
        Some(at) if source[..at].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

/// The lines of `text` from byte `from` on, each with its start, its bytes
/// without the line break, the end after its line break, and its number
/// (lines end at `\n` only), counted once for all of them.
fn lines_from(text: &str, from: usize) -> impl Iterator<Item = Line<'_>> {
    let mut at = from;
    let mut number = line_number(text, from);
    text[from..].split_inclusive('\n').map(move |raw| {
        let start = at;
        at += raw.len();
        let content = raw.trim_end_matches('\n').trim_end_matches('\r');
        let line = Line {
            start,
            content,
            end: at,
            number,
        };
        number += 1;
        line
    })
}

/// A line of the note.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    start: usize,
    /// Its text without the line break.
    content: &'a str,
    /// The byte after its line break.
    end: usize,
    /// Its number in the note, from 1.
    number: usize,
}

impl Line<'_> {
    /// The byte after its text, before the line break.
    fn content_end(&self) -> usize {
        self.start + self.content.len()
    }
}

/// The line of the note byte `at` lies on, counted from 1; lines end at
/// `\n` only, as the source view counts them.
fn line_number(source: &str, at: usize) -> usize {
    source.as_bytes()[..at.min(source.len())]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// The length of the byte order mark `source` starts with, 0 without one.
#[must_use]
pub(crate) fn bom_len(source: &str) -> usize {
    if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Markdown: the front matter
// ---------------------------------------------------------------------------

/// The parts of a front matter.
#[derive(Debug, Clone, Copy)]
struct FrontMatter {
    /// The start of its first line `---` (after a byte order mark).
    start: usize,
    /// The start of the matter, after the first line.
    body: usize,
    /// The start of the closing line.
    close: usize,
    /// The byte after the closing line and its line break.
    end: usize,
}

/// The front matter `source` starts with, as
/// [`crate::statistics::front_matter_end`] finds it.
fn front_matter(source: &str) -> Option<FrontMatter> {
    let start = bom_len(source);
    let mut lines = lines_from(source, start);
    let first = lines.next()?;
    if first.content.trim_end() != "---" {
        return None;
    }
    let close = lines.find(|line| matches!(line.content.trim_end(), "---" | "..."))?;
    Some(FrontMatter {
        start,
        body: first.end,
        close: close.start,
        end: close.end,
    })
}

/// UC-024 A5: why the matter of a front matter is no YAML mapping GitHub
/// reads: the line of the note and the reason; `None` when it is one.
fn yaml_problem(source: &str, front: FrontMatter) -> Option<(usize, String)> {
    let body = &source[front.body..front.close];
    let first_line = line_number(source, front.body);
    match serde_norway::from_str::<serde_norway::Value>(body) {
        Ok(serde_norway::Value::Mapping(_) | serde_norway::Value::Null) => None,
        Ok(_) => Some((
            first_line,
            "the front matter holds no names with values".to_owned(),
        )),
        Err(error) => {
            let message = error.to_string();
            // "problem at line 4 column 1, while scanning a simple key at
            // line 3 column 1": the place where the part in error starts is
            // the line to look at; the lines count from the matter's first.
            let named = message
                .match_indices("at line ")
                .filter_map(|(at, found)| {
                    let digits: String = message[at + found.len()..]
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect();
                    digits.parse::<usize>().ok()
                })
                .last()
                .or_else(|| error.location().map(|at| at.line()));
            let line = named.map_or(first_line, |n| first_line + n.saturating_sub(1));
            let reason = message
                .split(" at line ")
                .next()
                .unwrap_or(&message)
                .trim()
                .to_owned();
            Some((line, reason))
        }
    }
}

/// Whether the front matter `source` starts with is one the facade and the
/// note properties read: it exists and is YAML GitHub reads (A5). The
/// bytes it spans, delimiters included, when it is.
#[must_use]
pub fn readable_front_matter(source: &str) -> Option<Range<usize>> {
    let front = front_matter(source)?;
    yaml_problem(source, front)
        .is_none()
        .then_some(front.start..front.end)
}

/// The entries of a front matter, as lines.
#[derive(Debug, Clone)]
struct Entry<'a> {
    /// The name, unquoted.
    name: String,
    /// The line holding the name.
    key: Line<'a>,
    /// The byte after the `:` of the name.
    colon_end: usize,
    /// The lines after the key line that belong to it, without blank and
    /// comment lines at their end.
    more: Vec<Line<'a>>,
}

impl Entry<'_> {
    /// The bytes of all its lines, their line breaks included.
    fn span(&self) -> Range<usize> {
        self.key.start..self.more.last().map_or(self.key.end, |line| line.end)
    }
}

/// Whether `content` is blank or a comment.
fn blank_or_comment(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// The name of a key line at the start of `content` and the byte after its
/// `:`; `None` when the line is no key line of the mapping (indented, a
/// comment, a list item, or anything else). A quoted name may hold escaped
/// quotes (`"a\"b"`, `'it''s'`).
fn key_of(content: &str) -> Option<(String, usize)> {
    let first = content.chars().next()?;
    let item = first == '-' && (content == "-" || content[1..].starts_with([' ', '\t']));
    if first.is_whitespace() || item || matches!(first, '#' | '?' | '[' | '{' | ':') {
        return None;
    }
    if matches!(first, '"' | '\'') {
        let (name, len) = if first == '"' {
            double_quoted(content)?
        } else {
            single_quoted(content)?
        };
        let rest = &content[len..];
        let after = rest.trim_start_matches([' ', '\t']);
        let colon = len + (rest.len() - after.len());
        let follows = after.strip_prefix(':')?;
        if !(follows.is_empty() || follows.starts_with([' ', '\t'])) {
            return None;
        }
        return Some((name, colon + 1));
    }
    let bytes = content.as_bytes();
    let mut at = 0;
    while let Some(offset) = content[at..].find(':') {
        let colon = at + offset;
        let next = bytes.get(colon + 1);
        if next.is_none_or(|b| *b == b' ' || *b == b'\t') {
            let name = content[..colon].trim_end();
            return (!name.is_empty()).then(|| (name.to_owned(), colon + 1));
        }
        at = colon + 1;
    }
    None
}

/// The indentation of the mapping of a front matter: that of its first line
/// holding more than a comment (YAML lets a whole mapping be indented).
fn mapping_indent(source: &str, front: FrontMatter) -> usize {
    lines_from(&source[..front.close], front.body)
        .find(|line| !blank_or_comment(line.content))
        .map_or(0, |line| {
            line.content.len() - line.content.trim_start_matches(' ').len()
        })
}

/// The entries of the matter `body` of the front matter `front`.
fn entries(source: &str, front: FrontMatter) -> Vec<Entry<'_>> {
    let indent = mapping_indent(source, front);
    let mut found: Vec<Entry<'_>> = Vec::new();
    for line in lines_from(&source[..front.close], front.body) {
        let at_indent = line.content.len() >= indent
            && line.content.as_bytes()[..indent].iter().all(|b| *b == b' ');
        let key = if at_indent {
            key_of(&line.content[indent..])
        } else {
            None
        };
        if let Some((name, colon)) = key {
            found.push(Entry {
                name,
                key: line,
                colon_end: line.start + indent + colon,
                more: Vec::new(),
            });
        } else if let Some(entry) = found.last_mut() {
            entry.more.push(line);
        }
    }
    // Blank lines and comments at the mapping's own indentation after an
    // entry belong to no entry; indented lines (a block scalar's text, a
    // comment inside a nested value) are the entry's.
    for entry in &mut found {
        while entry.more.last().is_some_and(|line| {
            let own = line.content.len() - line.content.trim_start_matches(' ').len() <= indent;
            line.content.trim().is_empty() || (own && blank_or_comment(line.content))
        }) {
            entry.more.pop();
        }
    }
    found
}

/// A YAML scalar written on one line, as text: its value and the bytes of
/// it in `content` (comment and spaces left out); `None` when it is no
/// plain or quoted scalar (a list, a mapping, an anchor, a block scalar).
fn scalar(content: &str) -> Option<(String, Range<usize>, bool)> {
    let start = content.len() - content.trim_start_matches([' ', '\t']).len();
    let rest = &content[start..];
    match rest.chars().next() {
        None | Some('#') => Some((String::new(), start..start, false)),
        Some('"') => {
            let (value, len) = double_quoted(rest)?;
            Some((value, start..start + len, true))
        }
        Some('\'') => {
            let (value, len) = single_quoted(rest)?;
            Some((value, start..start + len, true))
        }
        Some('[' | '{' | '&' | '*' | '!' | '|' | '>' | '%' | '@' | '`') => None,
        Some(_) => {
            let end = rest
                .find(" #")
                .or_else(|| rest.find("\t#"))
                .unwrap_or(rest.len());
            let value = rest[..end].trim_end();
            Some((value.to_owned(), start..start + value.len(), false))
        }
    }
}

/// A double-quoted scalar at the start of `text`: its value and length.
fn double_quoted(text: &str) -> Option<(String, usize)> {
    let mut value = String::new();
    let mut chars = text.char_indices().skip(1);
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Some((value, at + 1)),
            '\\' => {
                let (_, escaped) = chars.next()?;
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '0' => value.push('\0'),
                    'u' | 'x' | 'U' => {
                        let len = match escaped {
                            'x' => 2,
                            'u' => 4,
                            _ => 8,
                        };
                        let hex: String = (0..len)
                            .filter_map(|_| chars.next())
                            .map(|(_, c)| c)
                            .collect();
                        value.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    other => value.push(other),
                }
            }
            c => value.push(c),
        }
    }
    None
}

/// A single-quoted scalar at the start of `text`: its value and length.
fn single_quoted(text: &str) -> Option<(String, usize)> {
    let mut value = String::new();
    let mut chars = text.char_indices().skip(1).peekable();
    while let Some((at, c)) = chars.next() {
        if c == '\'' {
            if chars.next_if(|(_, c)| *c == '\'').is_some() {
                value.push('\'');
                continue;
            }
            return Some((value, at + 1));
        }
        value.push(c);
    }
    None
}

/// The items of a flow list `[a, "b", c]` on one line, and its length;
/// `None` when it holds more than plain or quoted items.
fn flow_items(text: &str) -> Option<(Vec<String>, usize)> {
    let mut items = Vec::new();
    let mut at = 1;
    loop {
        let rest = &text[at..];
        let skipped = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        at += skipped;
        let rest = &text[at..];
        match rest.chars().next()? {
            ']' if items.is_empty() => return Some((items, at + 1)),
            '"' => {
                let (value, len) = double_quoted(rest)?;
                items.push(value);
                at += len;
            }
            '\'' => {
                let (value, len) = single_quoted(rest)?;
                items.push(value);
                at += len;
            }
            '[' | '{' | '&' | '*' | '!' | '#' | ',' | ']' => return None,
            _ => {
                let len = rest.find([',', ']'])?;
                let value = rest[..len].trim_end();
                if value.contains(": ") || value.contains(" #") {
                    return None;
                }
                items.push(value.to_owned());
                at += len;
            }
        }
        let rest = &text[at..];
        let skipped = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        at += skipped;
        match text[at..].chars().next()? {
            ',' => at += 1,
            ']' => return Some((items, at + 1)),
            _ => return None,
        }
    }
}

/// BR-002: the kind of a plain scalar as GitHub's YAML reads it.
fn plain_kind(value: &str) -> AttributeKind {
    if matches!(
        value,
        "true" | "false" | "True" | "False" | "TRUE" | "FALSE"
    ) {
        AttributeKind::YesNo
    } else if is_number(value) {
        AttributeKind::Number
    } else if is_date(value) {
        AttributeKind::Date
    } else {
        AttributeKind::Text
    }
}

/// The attribute of a front matter entry (BR-002).
fn attribute_of(source: &str, entry: &Entry<'_>) -> NoteAttribute {
    let span = entry.span();
    let line = entry.key.number;
    let rest_start = entry.colon_end;
    let rest = &source[rest_start..entry.key.content_end()];
    let nested = |entry: &Entry<'_>| {
        let markup = source[entry.key.start..entry.span().end]
            .trim_end_matches(['\n', '\r'])
            .to_owned();
        NoteAttribute {
            name: entry.name.clone(),
            kind: AttributeKind::Nested,
            value: markup,
            items: Vec::new(),
            span: entry.span(),
            value_span: rest_start..entry.span().end,
            line,
        }
    };
    let trimmed = rest.trim_start_matches([' ', '\t']);
    let value_start = rest_start + (rest.len() - trimmed.len());
    if entry.more.is_empty() {
        if trimmed.starts_with('[') {
            if let Some((items, len)) = flow_items(trimmed) {
                if blank_or_comment(&trimmed[len..]) {
                    return NoteAttribute {
                        name: entry.name.clone(),
                        kind: AttributeKind::List,
                        value: items.join(", "),
                        items,
                        span,
                        value_span: value_start..value_start + len,
                        line,
                    };
                }
            }
            return nested(entry);
        }
        let Some((value, at, quoted)) = scalar(rest) else {
            return nested(entry);
        };
        let kind = if quoted {
            AttributeKind::Text
        } else {
            plain_kind(&value)
        };
        return NoteAttribute {
            name: entry.name.clone(),
            kind,
            value,
            items: Vec::new(),
            span,
            value_span: rest_start + at.start..rest_start + at.end,
            line,
        };
    }
    // Lines below the name: a list of plain or quoted items, one per line
    // after a dash at one indentation; anything else is nested.
    if !blank_or_comment(rest) {
        return nested(entry);
    }
    let mut items = Vec::new();
    let mut indent = None;
    for more in &entry.more {
        if blank_or_comment(more.content) {
            continue;
        }
        let text = more.content.trim_start_matches(' ');
        let this = more.content.len() - text.len();
        if *indent.get_or_insert(this) != this {
            return nested(entry);
        }
        let Some(item) = text.strip_prefix('-') else {
            return nested(entry);
        };
        if !(item.is_empty() || item.starts_with([' ', '\t'])) {
            return nested(entry);
        }
        if key_of(item.trim_start()).is_some() || item.trim_start().starts_with('-') {
            return nested(entry);
        }
        match scalar(item) {
            Some((value, _, _)) => items.push(value),
            None => return nested(entry),
        }
    }
    let first = entry.more.first().map_or(span.end, |line| line.start);
    let last = entry.more.last().map_or(span.end, Line::content_end);
    NoteAttribute {
        name: entry.name.clone(),
        kind: AttributeKind::List,
        value: items.join(", "),
        items,
        span,
        value_span: first..last,
        line,
    }
}

fn markdown_head(source: &str) -> NoteHead {
    let Some(front) = front_matter(source) else {
        return NoteHead::default();
    };
    let range = Some(front.start..front.end);
    if let Some((line, reason)) = yaml_problem(source, front) {
        return NoteHead {
            attributes: Vec::new(),
            problem: Some(HeadProblem {
                line,
                reason,
                markup: source[front.start..front.end]
                    .trim_end_matches(['\n', '\r'])
                    .to_owned(),
            }),
            front_matter: range,
        };
    }
    NoteHead {
        attributes: entries(source, front)
            .iter()
            .map(|entry| attribute_of(source, entry))
            .collect(),
        problem: None,
        front_matter: range,
    }
}

/// Characters YAML does not take as they are in a scalar: line breaks of
/// YAML 1.1 (U+0085, U+2028, U+2029), the byte order mark and the
/// non-characters U+FFFE and U+FFFF; they are escaped.
fn yaml_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{85}' | '\u{2028}' | '\u{2029}' | '\u{feff}' | '\u{fffe}' | '\u{ffff}'
        )
}

/// Whether a reader of YAML 1.1 (GitHub) or 1.2 takes the plain scalar
/// `text` for something other than a text: a number in any notation
/// (`1e5`, `1_000`, `1:30`, `0b101`, `0x1F`), a date with a time, a merge
/// key or a value key.
fn yaml_reads_otherwise(text: &str) -> bool {
    let lower = text.to_lowercase();
    let reserved = matches!(
        lower.as_str(),
        "null"
            | "~"
            | "true"
            | "false"
            | "yes"
            | "no"
            | "y"
            | "n"
            | "on"
            | "off"
            | ".inf"
            | "-.inf"
            | "+.inf"
            | ".nan"
            | "="
            | "<<"
    );
    let unsigned = lower.trim_start_matches(['-', '+']);
    let numeric = unsigned.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        && unsigned.chars().all(|c| {
            c.is_ascii_hexdigit()
                || matches!(c, '_' | ':' | '.' | 'x' | 'o' | 'b' | 'e' | '+' | '-')
        });
    let timestamp = text.len() > 10
        && text.is_char_boundary(10)
        && is_date(&text[..10])
        && text[10..].starts_with([' ', 't', 'T', '\t']);
    reserved || numeric || timestamp
}

/// Whether `text` is written as a plain YAML scalar and read back as the
/// same text: not empty, no indicator first, no `: ` or ` #`, not read as
/// a number, date, yes or no, or nothing.
fn plain_text_ok(text: &str, in_flow: bool) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    !yaml_reads_otherwise(text)
        && plain_kind(text) == AttributeKind::Text
        && !first.is_whitespace()
        && !text.ends_with([' ', '\t', ':'])
        && !"-?:,[]{}#&*!|>'\"%@`".contains(first)
        && !text.contains(": ")
        && !text.contains(" #")
        && !text.contains('\t')
        && !(in_flow && text.contains([',', '[', ']', '{', '}']))
        && !text.chars().any(yaml_unsafe)
}

/// `text` as a YAML string: plain when it reads back the same, else in
/// double quotes.
fn yaml_text(text: &str, in_flow: bool) -> String {
    use std::fmt::Write as _;
    if plain_text_ok(text, in_flow) {
        return text.to_owned();
    }
    let mut quoted = String::from('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\t' => quoted.push_str("\\t"),
            c if yaml_unsafe(c) => {
                let _ = write!(quoted, "\\u{:04X}", u32::from(c));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// `value` written in YAML on one line.
fn yaml_value(value: &AttributeValue) -> String {
    match value {
        AttributeValue::Text(text) => yaml_text(text, false),
        AttributeValue::List(items) => {
            let items: Vec<String> = items.iter().map(|item| yaml_text(item, true)).collect();
            format!("[{}]", items.join(", "))
        }
        AttributeValue::Number(number) | AttributeValue::Date(number) => number.trim().to_owned(),
        AttributeValue::YesNo(yes) => yes.to_string(),
    }
}

/// The items of `value` without empty ones and spaces around them.
fn tidy(value: &AttributeValue) -> AttributeValue {
    match value {
        AttributeValue::List(items) => AttributeValue::List(
            items
                .iter()
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect(),
        ),
        other => other.clone(),
    }
}

fn markdown_edit(source: &str, change: &AttributeChange) -> Result<HeadEdit, AttributeError> {
    let eol = line_break(source);
    let Some(front) = front_matter(source) else {
        // A1: the first attribute makes the front matter.
        let AttributeChange::Add { name, value } = change else {
            return Err(AttributeError::Missing(change_name(change).to_owned()));
        };
        let name = checked_name(name, NoteFormat::Markdown)?;
        let value = tidy(value);
        check_value(&value)?;
        let start = bom_len(source);
        return Ok(HeadEdit {
            range: start..start,
            text: format!(
                "---{eol}{}: {}{eol}---{eol}",
                yaml_key(&name),
                yaml_value(&value)
            ),
        });
    };
    if yaml_problem(source, front).is_some() {
        return Err(AttributeError::Unreadable);
    }
    let found = entries(source, front);
    let attributes: Vec<NoteAttribute> = found.iter().map(|e| attribute_of(source, e)).collect();
    match change {
        AttributeChange::Add { name, value } => {
            let name = checked_name(name, NoteFormat::Markdown)?;
            if attributes.iter().any(|a| a.name == name) {
                return Err(AttributeError::Taken(name));
            }
            let value = tidy(value);
            check_value(&value)?;
            let at = found.last().map_or(front.close, |entry| entry.span().end);
            // BR-003: a mapping indented as a whole takes the new name at its
            // indentation. The last line of a front matter ends at its
            // closing line, so it has its line break.
            let indent = " ".repeat(mapping_indent(source, front));
            Ok(HeadEdit {
                range: at..at,
                text: format!("{indent}{}: {}{eol}", yaml_key(&name), yaml_value(&value)),
            })
        }
        AttributeChange::Set { name, value } => {
            let index = attributes
                .iter()
                .position(|a| &a.name == name)
                .ok_or_else(|| AttributeError::Missing(name.clone()))?;
            let (entry, attribute) = (&found[index], &attributes[index]);
            if attribute.kind == AttributeKind::Nested {
                return Err(AttributeError::Nested(name.clone()));
            }
            let value = tidy(value);
            check_value(&value)?;
            if !entry.more.is_empty() {
                // A list one item per line keeps that way of writing.
                if let AttributeValue::List(items) = &value {
                    if !items.is_empty() {
                        let indent = entry
                            .more
                            .iter()
                            .find(|line| !blank_or_comment(line.content))
                            .map_or("", |line| {
                                let text = line.content.trim_start_matches(' ');
                                &line.content[..line.content.len() - text.len()]
                            });
                        let lines: String = items
                            .iter()
                            .map(|item| format!("{indent}- {}{eol}", yaml_text(item, false)))
                            .collect();
                        let first = entry.more[0].start;
                        let last = entry.more.last().map_or(first, |line| line.end);
                        return Ok(HeadEdit {
                            range: first..last,
                            text: lines,
                        });
                    }
                }
                let last = entry.more.last().map_or(entry.key.end, Line::content_end);
                return Ok(HeadEdit {
                    range: entry.colon_end..last,
                    text: format!(" {}", yaml_value(&value)),
                });
            }
            let written = yaml_value(&value);
            if attribute.value_span.is_empty() {
                // `name:` without a value, perhaps with a comment after it.
                let rest = &source[entry.colon_end..entry.key.content_end()];
                let comment = rest.trim_start_matches([' ', '\t']).starts_with('#');
                let range = if comment {
                    entry.colon_end..entry.colon_end + (rest.len() - rest.trim_start().len())
                } else {
                    entry.colon_end..entry.key.content_end()
                };
                let space = if comment { " " } else { "" };
                return Ok(HeadEdit {
                    range,
                    text: format!(" {written}{space}"),
                });
            }
            Ok(HeadEdit {
                range: attribute.value_span.clone(),
                text: written,
            })
        }
        AttributeChange::Remove { name } => {
            let index = attributes
                .iter()
                .position(|a| &a.name == name)
                .ok_or_else(|| AttributeError::Missing(name.clone()))?;
            let span = found[index].span();
            // A2: without another attribute and without a comment the whole
            // front matter goes.
            let left = source[front.body..span.start].to_owned() + &source[span.end..front.close];
            if found.len() == 1 && left.trim().is_empty() {
                return Ok(HeadEdit {
                    range: front.start..front.end,
                    text: String::new(),
                });
            }
            Ok(HeadEdit {
                range: span,
                text: String::new(),
            })
        }
    }
}

/// A name as a YAML key: plain, as BR-004 allows only plain names.
fn yaml_key(name: &str) -> String {
    yaml_text(name, false)
}

/// The name a change is of.
fn change_name(change: &AttributeChange) -> &str {
    match change {
        AttributeChange::Add { name, .. }
        | AttributeChange::Set { name, .. }
        | AttributeChange::Remove { name } => name,
    }
}

// ---------------------------------------------------------------------------
// AsciiDoc: the attribute entries of the header
// ---------------------------------------------------------------------------

/// The names whose values an AsciiDoc note holds as lists, items separated
/// by commas (BR-002, OQ-213, OQ-264).
const ASCIIDOC_LISTS: [&str; 2] = ["tags", "keywords"];

/// The header of an AsciiDoc note.
#[derive(Debug, Default)]
struct Header {
    attributes: Vec<NoteAttribute>,
    /// Where a new attribute goes, as the end of the text of a line and
    /// the end of that line: after the last attribute entry, else after the
    /// title and the lines of the header below it; `None` at the start.
    after: Option<(usize, usize)>,
}

/// An attribute entry `:name: value` of `content`: the name, and the
/// value's bytes in `content`; `None` for another line and for an entry
/// that unsets an attribute (`:name!:`).
fn attribute_entry(content: &str) -> Option<(String, Range<usize>)> {
    let rest = content.strip_prefix(':')?;
    let close = rest.find(':')?;
    let name = &rest[..close];
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_alphanumeric() || first == '_')
        || !chars.all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let after = &rest[close + 1..];
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let value = after.trim_start_matches([' ', '\t']);
    let start = 1 + close + 1 + (after.len() - value.len());
    let value = value.trim_end();
    Some((name.to_lowercase(), start..start + value.len()))
}

/// Whether `content` is a comment line of AsciiDoc (not a comment block).
fn asciidoc_comment(content: &str) -> bool {
    content.starts_with("//") && !content.starts_with("////")
}

fn asciidoc_header(source: &str) -> Header {
    let mut header = Header::default();
    let mut lines = lines_from(source, bom_len(source)).peekable();
    // Comment lines may stand above the title.
    while lines
        .next_if(|line| asciidoc_comment(line.content))
        .is_some()
    {}
    let mut after: Option<(usize, usize)> = None;
    let mut titled = false;
    if let Some(title) = lines.next_if(|line| line.content.starts_with("= ")) {
        after = Some((title.content_end(), title.end));
        titled = true;
    }
    let mut below_title = 0;
    let mut attributes_after: Option<(usize, usize)> = None;
    while let Some(line) = lines.next() {
        if line.content.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = attribute_entry(line.content) {
            let mut end = line;
            let mut text = line.content[value.clone()].to_owned();
            let mut spread = false;
            // A value ending in ` \` goes on in the next line.
            while text.ends_with(" \\") {
                let Some(next) = lines.next() else { break };
                spread = true;
                text.push('\n');
                text.push_str(next.content);
                end = next;
            }
            let list = ASCIIDOC_LISTS.contains(&name.as_str());
            let items: Vec<String> = if list && !spread {
                text.split(',')
                    .map(|item| item.trim().to_owned())
                    .filter(|item| !item.is_empty())
                    .collect()
            } else {
                Vec::new()
            };
            let kind = if spread {
                AttributeKind::Nested
            } else if list {
                AttributeKind::List
            } else {
                AttributeKind::Text
            };
            let value_span = if spread {
                line.start + value.start..end.content_end()
            } else {
                line.start + value.start..line.start + value.end
            };
            header.attributes.push(NoteAttribute {
                name,
                kind,
                value: if spread {
                    source[line.start..end.content_end()].to_owned()
                } else if list {
                    items.join(", ")
                } else {
                    text
                },
                items,
                span: line.start..end.end,
                value_span,
                line: line.number,
            });
            attributes_after = Some((end.content_end(), end.end));
            continue;
        }
        if asciidoc_comment(line.content) || line.content.starts_with(':') {
            // A comment or an entry that unsets an attribute: kept as it is.
            if titled || !header.attributes.is_empty() {
                after = Some((line.content_end(), line.end));
            }
            continue;
        }
        // The author and the revision line below a title.
        if titled && header.attributes.is_empty() && below_title < 2 {
            below_title += 1;
            after = Some((line.content_end(), line.end));
            continue;
        }
        break;
    }
    header.after = attributes_after.or(after);
    header
}

fn asciidoc_edit(source: &str, change: &AttributeChange) -> Result<HeadEdit, AttributeError> {
    let eol = line_break(source);
    let header = asciidoc_header(source);
    let written = |value: &AttributeValue| -> Result<String, AttributeError> {
        let text = match value {
            AttributeValue::Text(text) => text.trim().to_owned(),
            AttributeValue::List(items) => {
                // BR-002: the items are separated by commas.
                if items.iter().any(|item| item.contains(',')) {
                    return Err(AttributeError::Comma);
                }
                items.join(", ")
            }
            _ => return Err(AttributeError::Kind),
        };
        // A value ending in `\` would go on in the next line (OQ-264).
        if text.ends_with('\\') {
            return Err(AttributeError::Continuation);
        }
        Ok(text)
    };
    let entry = |name: &str, value: &str| {
        if value.is_empty() {
            format!(":{name}:")
        } else {
            format!(":{name}: {value}")
        }
    };
    match change {
        AttributeChange::Add { name, value } => {
            let name = checked_name(name, NoteFormat::AsciiDoc)?;
            if header.attributes.iter().any(|a| a.name == name) {
                return Err(AttributeError::Taken(name));
            }
            let value = tidy(value);
            check_value(&value)?;
            let line = entry(&name, &written(&value)?);
            if let Some((content_end, _)) = header.after {
                // An entry before that goes on in the next line would take
                // the new one in.
                if source[..content_end].ends_with('\\') {
                    return Err(AttributeError::ContinuedBefore);
                }
            }
            Ok(match header.after {
                // The line before has no line break: the note ends there.
                Some((content_end, end)) if content_end == end => HeadEdit {
                    range: end..end,
                    text: format!("{eol}{line}"),
                },
                Some((_, end)) => HeadEdit {
                    range: end..end,
                    text: format!("{line}{eol}"),
                },
                None => {
                    let start = bom_len(source);
                    HeadEdit {
                        range: start..start,
                        text: format!("{line}{eol}"),
                    }
                }
            })
        }
        AttributeChange::Set { name, value } => {
            let name = written_name(name, NoteFormat::AsciiDoc);
            let attribute = header
                .attributes
                .iter()
                .find(|a| a.name == name)
                .ok_or_else(|| AttributeError::Missing(name.clone()))?;
            if attribute.kind == AttributeKind::Nested {
                return Err(AttributeError::Nested(name));
            }
            let value = tidy(value);
            check_value(&value)?;
            let text = written(&value)?;
            let line = &source[attribute.span.clone()];
            let content_end = attribute.span.start + line.trim_end_matches(['\n', '\r']).len();
            if attribute.value_span.is_empty() {
                // `:name:` without a value.
                return Ok(HeadEdit {
                    range: attribute.value_span.start.min(content_end)..content_end,
                    text: if text.is_empty() {
                        String::new()
                    } else {
                        format!(" {text}")
                    },
                });
            }
            if text.is_empty() {
                // No value: the spaces before it go too.
                // The name as the note writes it, which may differ in length
                // from its lower case.
                let line_text = &source[attribute.span.start + 1..content_end];
                let name_end =
                    attribute.span.start + 1 + line_text.find(':').map_or(0, |at| at + 1);
                return Ok(HeadEdit {
                    range: name_end..content_end,
                    text: String::new(),
                });
            }
            Ok(HeadEdit {
                range: attribute.value_span.clone(),
                text,
            })
        }
        AttributeChange::Remove { name } => {
            let name = written_name(name, NoteFormat::AsciiDoc);
            let attribute = header
                .attributes
                .iter()
                .find(|a| a.name == name)
                .ok_or_else(|| AttributeError::Missing(name.clone()))?;
            let span = attribute.span.clone();
            let ends_with_break = source[..span.end].ends_with('\n');
            if !ends_with_break && span.start > 0 {
                // The last line of the note: the line break before it goes.
                let before = &source[..span.start];
                let cut = if before.ends_with("\r\n") {
                    2
                } else {
                    usize::from(before.ends_with('\n'))
                };
                return Ok(HeadEdit {
                    range: span.start - cut..span.end,
                    text: String::new(),
                });
            }
            Ok(HeadEdit {
                range: span,
                text: String::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests;
