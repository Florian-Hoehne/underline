//! UC-008 Find Note, the pure part: what a search text matches (BR-002),
//! where its words occur in a text (BR-008, A6), and the places found in a
//! note (BR-004). Nothing here reads files or starts threads, so the window's
//! editor core marks the words of a search with the same rules
//! (`docs/migration/offene-fragen.md` Q-38). `underline-core` re-exports it
//! under its former path and searches the workspace with it.
//!
//! FR-075, UC-008 BR-011: the search switches (match case, whole word,
//! regular expression) change the rules; and replacing (FR-073, FR-074,
//! UC-005 BR-017, UC-022 BR-002, BR-003) reads the search text as one text
//! and writes the replacement, with the groups of a regular expression
//! (ADR-026). The places a replacement in the workspace changes are in
//! [`note_places`].

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

use regex::{Regex, RegexBuilder};

/// How many lines found a note shows before "N more" (UC-008 A5).
pub const SHOWN_PLACES: usize = 5;
/// How many characters of a long line are shown around the first word.
pub const LINE_CONTEXT: usize = 120;
/// ADR-026: the longest regular expression taken, in bytes.
const PATTERN_MOST: usize = 4096;
/// ADR-026: how large a compiled expression and its lazy automaton may grow.
const COMPILED_MOST: usize = 1 << 20;

/// FR-075, UC-008 BR-011: the three search switches, each off at first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Switches {
    /// Upper and lower case must match as typed.
    pub match_case: bool,
    /// A word matches only where it is not part of a longer word; letters
    /// and digits belong to a word.
    pub whole_word: bool,
    /// The search text is one regular expression (OQ-193).
    pub regex: bool,
}

/// How a search text is read without the switch regular expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Words at spaces, in any order, quoted words kept together (UC-008
    /// BR-002): the search of the workspace and the find field.
    Words,
    /// One text, spaces included (UC-022 BR-002, UC-005 BR-017, OQ-192):
    /// replacing.
    OneText,
}

/// Why a search text or a replacement cannot be used (UC-008 A9, UC-022
/// A2, UC-005 A22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    /// The search text is no valid regular expression; why, in a few words.
    Regex(String),
    /// The replacement names a group (`$n`) the expression does not have.
    NoGroup(usize),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Regex(why) => write!(f, "Not a valid regular expression: {why}."),
            Self::NoGroup(n) => write!(
                f,
                "The replacement names ${n}, but the expression has no group {n}."
            ),
        }
    }
}

impl std::error::Error for QueryError {}

/// What is wrong with a regular expression, in the last line of the
/// parser's message (`error: unclosed group`), without its picture of the
/// pattern.
fn regex_problem(error: &regex::Error) -> String {
    match error {
        regex::Error::Syntax(text) => text
            .lines()
            .rev()
            .find_map(|line| line.trim().strip_prefix("error: "))
            .unwrap_or_else(|| text.lines().last().unwrap_or(text))
            .to_owned(),
        regex::Error::CompiledTooBig(_) => "it is too large".to_owned(),
        _ => error.to_string(),
    }
}

/// A regular expression of a search (ADR-026).
#[derive(Debug, Clone)]
struct Pattern {
    /// The search text it was made of.
    source: String,
    regex: Regex,
    /// It names a line end (`\n`), so a place may span lines (UC-022
    /// BR-002); else a place found lies within one line.
    lines: bool,
}

/// Whether `c` belongs to a word (UC-008 BR-011): letters and digits, and
/// the combining marks written on them (test-monkey: `e` + U+0301).
fn in_word(c: char) -> bool {
    c.is_alphanumeric() || combining(c)
}

/// Whether `c` is a combining mark of the common blocks.
fn combining(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05BD}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0900}'..='\u{0903}'
            | '\u{093A}'..='\u{094F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{200C}'..='\u{200D}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{3099}'..='\u{309A}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// UC-008 BR-011: whether `range` of `text` is not part of a longer word:
/// where it starts or ends with a letter or a digit, the character next to
/// it is none.
fn bounded(text: &str, range: &Range<usize>) -> bool {
    let inside = &text[range.clone()];
    let joins_before = inside.chars().next().is_some_and(in_word)
        && text[..range.start].chars().next_back().is_some_and(in_word);
    let joins_after = inside.chars().next_back().is_some_and(in_word)
        && text[range.end..].chars().next().is_some_and(in_word);
    !joins_before && !joins_after
}

/// The byte offset of the end of the line `at` is in (its `\n`, or the end).
fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |n| at + n)
}

/// The byte offset of the character after the one at `at`.
fn next_char(text: &str, at: usize) -> usize {
    text[at..]
        .chars()
        .next()
        .map_or(text.len() + 1, |c| at + c.len_utf8())
}

impl Pattern {
    /// The places of `text` this pattern matches, as byte ranges, sorted
    /// and apart: within one line unless it names a line end, never empty,
    /// and with `whole_word` only those not part of a longer word.
    fn places(&self, text: &str, whole_word: bool, limit: usize) -> Vec<Range<usize>> {
        let mut found = Vec::new();
        let mut at = 0;
        // Once a match ran over a line end, the rest of the text is searched
        // line by line, so no line is searched more than once.
        let mut by_line = false;
        while at <= text.len() && found.len() < limit {
            let end = if by_line {
                line_end(text, at)
            } else {
                text.len()
            };
            let Some(found_at) = self.regex.find_at(&text[..end], at) else {
                if by_line && end < text.len() {
                    at = end + 1;
                    continue;
                }
                break;
            };
            let range = found_at.range();
            if !self.lines && !by_line && text[range.clone()].contains('\n') {
                // UC-022 BR-002: a place lies within its line. No match starts
                // before this one, so the search goes on from its start.
                by_line = true;
                at = range.start;
                continue;
            }
            if range.is_empty() {
                at = next_char(text, range.start);
                continue;
            }
            if whole_word && !bounded(text, &range) {
                // A start inside a run of word characters is part of a longer
                // word too: the search goes on after the run, so a long word
                // costs one pass, not one per character (test-monkey).
                at = next_char(text, range.start);
                if text[range.start..].chars().next().is_some_and(in_word) {
                    while let Some(c) = text.get(at..).and_then(|rest| rest.chars().next()) {
                        if !in_word(c) {
                            break;
                        }
                        at += c.len_utf8();
                    }
                }
                continue;
            }
            at = range.end;
            found.push(range);
        }
        found
    }

    /// The haystack the place starting at `start` was matched in: its line,
    /// or the whole text when the pattern spans lines.
    fn haystack<'t>(&self, text: &'t str, place: &Range<usize>) -> &'t str {
        if self.lines {
            text
        } else {
            &text[..line_end(text, place.start).max(place.end)]
        }
    }
}

/// UC-022 BR-002: whether the expression `pattern` names a line end itself,
/// in any notation (`\n`, `\x0A`, `\x{A}`, a line end typed, `[\n]`), so a
/// place may span lines; `\s`, `.` and `[^x]` name none.
fn names_line_end(pattern: &str) -> bool {
    use regex_syntax::ast::{Ast, ClassSet, ClassSetItem};

    fn in_item(item: &ClassSetItem) -> bool {
        match item {
            ClassSetItem::Literal(literal) => literal.c == '\n',
            ClassSetItem::Range(range) => (range.start.c..=range.end.c).contains(&'\n'),
            ClassSetItem::Bracketed(inner) => !inner.negated && in_set(&inner.kind),
            ClassSetItem::Union(union) => union.items.iter().any(in_item),
            _ => false,
        }
    }
    fn in_set(set: &ClassSet) -> bool {
        match set {
            ClassSet::Item(item) => in_item(item),
            ClassSet::BinaryOp(op) => in_set(&op.lhs) || in_set(&op.rhs),
        }
    }
    fn in_ast(ast: &Ast) -> bool {
        match ast {
            Ast::Literal(literal) => literal.c == '\n',
            Ast::ClassBracketed(class) => !class.negated && in_set(&class.kind),
            Ast::Repetition(repetition) => in_ast(&repetition.ast),
            Ast::Group(group) => in_ast(&group.ast),
            Ast::Alternation(alternation) => alternation.asts.iter().any(in_ast),
            Ast::Concat(concat) => concat.asts.iter().any(in_ast),
            _ => false,
        }
    }
    regex_syntax::ast::parse::Parser::new()
        .parse(pattern)
        .is_ok_and(|ast| in_ast(&ast))
}

/// A search text: words in lower case, since case does not matter (UC-008
/// BR-002), or with the search switches (BR-011) words as typed, one text,
/// or a regular expression.
#[derive(Debug, Clone, Default)]
pub struct Query {
    words: Vec<String>,
    switches: Switches,
    pattern: Option<Pattern>,
}

impl PartialEq for Query {
    fn eq(&self, other: &Self) -> bool {
        self.words == other.words
            && self.switches == other.switches
            && self.pattern.as_ref().map(|p| (&p.source, p.lines))
                == other.pattern.as_ref().map(|p| (&p.source, p.lines))
    }
}

impl Eq for Query {}

/// The words of `text` at spaces, words in double quotes kept together
/// (UC-008 BR-002).
pub(crate) fn split_words(text: &str) -> Vec<String> {
    split_words_quoted(text)
        .into_iter()
        .map(|(word, _)| word)
        .collect()
}

/// [`split_words`], each word with whether it was in double quotes.
pub(crate) fn split_words_quoted(text: &str) -> Vec<(String, bool)> {
    let mut words = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| !c.is_whitespace()) {
        rest = &rest[start..];
        let (word, after, quoted) = match rest.strip_prefix('"') {
            Some(quoted) => match quoted.find('"') {
                Some(end) => (&quoted[..end], &quoted[end + 1..], true),
                None => (quoted, "", true),
            },
            None => {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                (&rest[..end], &rest[end..], false)
            }
        };
        if !word.is_empty() {
            words.push((word.to_owned(), quoted));
        }
        rest = after;
    }
    words
}

impl Query {
    /// Splits `text` into its words (BR-002), without a search switch.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut words: Vec<String> = split_words(text).iter().map(|w| lower(w)).collect();
        words.dedup();
        Self {
            words,
            ..Self::default()
        }
    }

    /// `text` read as `reading`, with the search switches `switches` (FR-075,
    /// UC-008 BR-011): with regular expression, one expression in the usual
    /// notation of programming editors (OQ-193, ADR-026), matching within a
    /// line unless it names a line end; with match case, words as typed.
    ///
    /// # Errors
    ///
    /// The switch regular expression is on and `text` is no valid regular
    /// expression (UC-008 A9).
    pub fn new(text: &str, switches: Switches, reading: Reading) -> Result<Self, QueryError> {
        if switches.regex {
            if text.is_empty() {
                return Ok(Self {
                    switches,
                    ..Self::default()
                });
            }
            if text.len() > PATTERN_MOST {
                return Err(QueryError::Regex("it is too large".to_owned()));
            }
            let regex = RegexBuilder::new(text)
                .case_insensitive(!switches.match_case)
                .multi_line(true)
                .crlf(true)
                .size_limit(COMPILED_MOST)
                .dfa_size_limit(COMPILED_MOST)
                .build()
                .map_err(|error| QueryError::Regex(regex_problem(&error)))?;
            return Ok(Self {
                words: Vec::new(),
                switches,
                pattern: Some(Pattern {
                    source: text.to_owned(),
                    regex,
                    lines: names_line_end(text),
                }),
            });
        }
        let typed = match reading {
            Reading::Words => split_words(text),
            Reading::OneText => vec![text.to_owned()],
        };
        let mut words: Vec<String> = typed
            .into_iter()
            .filter(|w| !w.is_empty())
            .map(|w| if switches.match_case { w } else { lower(&w) })
            .collect();
        words.dedup();
        Ok(Self {
            words,
            switches,
            pattern: None,
        })
    }

    /// Whether there is no word.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.pattern.is_none()
    }

    /// The search switches it was made with.
    #[must_use]
    pub fn switches(&self) -> Switches {
        self.switches
    }

    /// Without a search switch: words found in lower case, anywhere.
    fn plain(&self) -> bool {
        self.pattern.is_none() && !self.switches.match_case && !self.switches.whole_word
    }

    /// Whether every word occurs somewhere in `texts` together; with the
    /// switch regular expression, whether one of them holds a place.
    #[must_use]
    pub fn matches_all(&self, texts: &[&str]) -> bool {
        if let Some(pattern) = &self.pattern {
            return texts
                .iter()
                .any(|text| !pattern.places(text, self.switches.whole_word, 1).is_empty());
        }
        if self.plain() {
            let lowered: Vec<String> = texts.iter().map(|t| lower(t)).collect();
            let lowered: Vec<&str> = lowered.iter().map(String::as_str).collect();
            return self.matches_lowered(&lowered);
        }
        self.words.iter().all(|word| {
            texts.iter().any(|text| {
                let one = Self {
                    words: vec![word.clone()],
                    switches: self.switches,
                    pattern: None,
                };
                !one.occurrences(text).is_empty()
            })
        })
    }

    /// [`Self::matches_all`] for texts already in lower case, without a
    /// search switch.
    #[must_use]
    pub fn matches_lowered(&self, lowered: &[&str]) -> bool {
        self.words
            .iter()
            .all(|word| lowered.iter().any(|text| text.contains(word.as_str())))
    }

    /// [`Self::matches_all`] of `texts`, whose lower case is `lowered`:
    /// without a search switch the quick way over `lowered`.
    #[must_use]
    pub fn matches_texts(&self, texts: &[&str], lowered: &[&str]) -> bool {
        if self.plain() {
            self.matches_lowered(lowered)
        } else {
            self.matches_all(texts)
        }
    }

    /// Every occurrence of a word in `text`, as byte ranges of `text`,
    /// sorted and not overlapping (UC-008 BR-008, A6), by the search
    /// switches (BR-011).
    #[must_use]
    pub fn occurrences(&self, text: &str) -> Vec<Range<usize>> {
        if let Some(pattern) = &self.pattern {
            return pattern.places(text, self.switches.whole_word, usize::MAX);
        }
        // Text in ASCII lowers byte for byte, so its offsets stay; the window
        // marks long notes with it after every input (ADR-025, Q-38).
        let whole = self.switches.whole_word;
        let fits = |range: &Range<usize>| !whole || bounded(text, range);
        let mut found = if self.switches.match_case {
            self.find_words(text, |at| at, |_, range| fits(range))
        } else if text.is_ascii() {
            self.find_words(&text.to_ascii_lowercase(), |at| at, |_, range| fits(range))
        } else {
            // A word found in the lower case of part of a character (`i` in
            // the `i̇` of `İ`) is no place: a place is whole characters
            // whose lower case is the word (test-monkey).
            let (lowered, map) = lower_with_map(text);
            self.find_words(
                &lowered,
                |at| map[at],
                |word, range| {
                    !range.is_empty() && lower(&text[range.clone()]) == word && fits(range)
                },
            )
        };
        found.retain(|range| range.start < range.end);
        found.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
        // Places of different words that overlap are one; places that only
        // touch stay apart, so each is counted and replaced (test-monkey).
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in found {
            match merged.last_mut() {
                Some(last) if range.start < last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }

    /// UC-022 BR-003, UC-005 BR-017: the replacement `typed`, written as
    /// typed; with the switch regular expression, `$0` to `$9` stand for the
    /// place found and its groups, and `$$` for `$` (OQ-193).
    ///
    /// # Errors
    ///
    /// The replacement names a group the expression does not have (UC-022
    /// A2).
    pub fn replacement(&self, typed: &str) -> Result<Replacement, QueryError> {
        let Some(pattern) = &self.pattern else {
            return Ok(Replacement {
                parts: vec![Part::Text(typed.to_owned())],
            });
        };
        let groups = pattern.regex.captures_len();
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut chars = typed.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '$' {
                text.push(c);
                continue;
            }
            match chars.peek().copied() {
                Some('$') => {
                    chars.next();
                    text.push('$');
                }
                Some(digit @ '0'..='9') => {
                    chars.next();
                    let group = digit as usize - '0' as usize;
                    if group >= groups {
                        return Err(QueryError::NoGroup(group));
                    }
                    parts.push(Part::Text(std::mem::take(&mut text)));
                    parts.push(Part::Group(group));
                }
                _ => text.push('$'),
            }
        }
        parts.push(Part::Text(text));
        parts.retain(|part| !matches!(part, Part::Text(t) if t.is_empty()));
        Ok(Replacement { parts })
    }

    /// The text that takes the place of `place` of `text` (found by
    /// [`Self::occurrences`]) with the replacement `with`.
    #[must_use]
    pub fn replaced(&self, text: &str, place: &Range<usize>, with: &Replacement) -> String {
        let groups = with.parts.iter().any(|p| matches!(p, Part::Group(_)));
        let captures = match (&self.pattern, groups) {
            (Some(pattern), true) => {
                let captures = pattern
                    .regex
                    .captures_at(pattern.haystack(text, place), place.start)
                    .filter(|c| c.get(0).map(|m| m.range()) == Some(place.clone()));
                // The place was found with the same expression and haystack.
                debug_assert!(captures.is_some(), "no captures at {place:?}");
                captures
            }
            _ => None,
        };
        with.parts
            .iter()
            .map(|part| match part {
                Part::Text(t) => t.as_str(),
                Part::Group(n) => captures
                    .as_ref()
                    .and_then(|c| c.get(*n))
                    .map_or("", |m| m.as_str()),
            })
            .collect()
    }
}

impl Query {
    /// Every occurrence of a word in `lowered`, as ranges of the text it was
    /// lowered from (`original` gives the offset there of an offset in it).
    /// Only a range `valid` takes for its word counts; after one it does not
    /// take, the search goes on from the next character.
    fn find_words(
        &self,
        lowered: &str,
        original: impl Fn(usize) -> usize,
        valid: impl Fn(&str, &Range<usize>) -> bool,
    ) -> Vec<Range<usize>> {
        let mut found: Vec<Range<usize>> = Vec::new();
        for word in self.words.iter().filter(|w| !w.is_empty()) {
            let mut from = 0;
            while let Some(at) = lowered[from..].find(word.as_str()) {
                let start = from + at;
                let end = start + word.len();
                let range = original(start)..original(end);
                if valid(word, &range) {
                    found.push(range);
                    from = end;
                } else {
                    from = next_char(lowered, start);
                }
            }
        }
        found
    }
}

/// A part of a replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    /// `$n`: group `n` of the place found, `$0` the place itself.
    Group(usize),
}

/// A replacement as the user typed it (UC-022 BR-003), ready to be written
/// at each place ([`Query::replaced`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Replacement {
    parts: Vec<Part>,
}

/// `text` with each range of `places` (sorted, apart) replaced by its text.
#[must_use]
pub fn replace_places(text: &str, places: &[(Range<usize>, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, with) in places {
        out.push_str(&text[at..range.start]);
        out.push_str(with);
        at = range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// What a place is known by while the text around it changes (UC-022 A4):
/// its line (by a hash and the length, so a long line is not kept twice),
/// where in the line it starts, and how many lines before it are alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlaceKey {
    /// FNV-1a of its line, without the line end.
    pub line: u64,
    /// The length of its line in bytes.
    pub length: usize,
    /// Where in the line it starts, in bytes.
    pub column: usize,
    /// How many lines before it have the same text.
    pub repeat: usize,
}

/// The hash and the length of `line` without its `\r`, for [`PlaceKey`].
fn line_key(line: &str) -> (u64, usize) {
    let line = line.trim_end_matches('\r');
    let hash = line.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    (hash, line.len())
}

/// UC-022 step 2: a place a replacement in the workspace changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePlace {
    /// The place, as bytes of the note's text.
    pub range: Range<usize>,
    /// The number of its line, from 1.
    pub line: usize,
    /// What it is known by (A4).
    pub key: PlaceKey,
    /// The text that takes its place (BR-003).
    pub replaced: String,
}

/// UC-022 step 2, BR-002: every place of `text` the query matches, with
/// what takes its place.
#[must_use]
pub fn note_places(query: &Query, with: &Replacement, text: &str) -> Vec<NotePlace> {
    places_at(query, with, text, &query.occurrences(text))
}

/// [`note_places`] of the places `ranges` (found by the query, sorted).
#[must_use]
pub fn places_at(
    query: &Query,
    with: &Replacement,
    text: &str,
    ranges: &[Range<usize>],
) -> Vec<NotePlace> {
    let mut places = Vec::with_capacity(ranges.len());
    // How many lines before the one walked to have each text.
    let mut lines_before: HashMap<(u64, usize), usize> = HashMap::new();
    let (mut start, mut number) = (0, 1);
    let mut end = line_end(text, 0);
    let mut current: Option<((u64, usize), usize)> = None;
    for range in ranges {
        // A place starting at a line's end belongs to that line.
        while range.start > end {
            *lines_before.entry(line_key(&text[start..end])).or_default() += 1;
            start = end + 1;
            number += 1;
            end = line_end(text, start);
            current = None;
        }
        let ((line, length), repeat) = *current.get_or_insert_with(|| {
            let key = line_key(&text[start..end]);
            (key, lines_before.get(&key).copied().unwrap_or(0))
        });
        places.push(NotePlace {
            replaced: query.replaced(text, range, with),
            key: PlaceKey {
                line,
                length,
                column: range.start - start,
                repeat,
            },
            range: range.clone(),
            line: number,
        });
    }
    places
}

/// UC-022 step 2: a place with its line as it is and as it will be: the
/// text before it in its line, the place, what takes its place, and the
/// text after it, each shortened when long. A line end in a place that
/// spans lines shows as `↵`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownPlace {
    pub before: String,
    pub old: String,
    pub new: String,
    pub after: String,
}

/// `text` on one line: a line end shows as `↵`.
fn flat(text: &str) -> String {
    text.replace("\r\n", "↵").replace('\n', "↵")
}

/// The last `n` chars of `text`, and whether some were cut.
fn tail(text: &str, n: usize) -> (&str, bool) {
    match text.char_indices().rev().nth(n) {
        Some((at, c)) => (&text[at + c.len_utf8()..], true),
        None => (text, false),
    }
}

/// The first `n` chars of `text`, and whether some were cut.
fn head(text: &str, n: usize) -> (&str, bool) {
    match text.char_indices().nth(n) {
        Some((at, _)) => (&text[..at], true),
        None => (text, false),
    }
}

/// The byte offset at most `bytes` before `at` that starts a character.
fn back_by(text: &str, at: usize, bytes: usize) -> usize {
    let mut from = at.saturating_sub(bytes);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    from
}

/// The byte offset at most `bytes` after `at` that starts a character.
fn on_by(text: &str, at: usize, bytes: usize) -> usize {
    let mut to = (at + bytes).min(text.len());
    while !text.is_char_boundary(to) {
        to += 1;
    }
    to
}

/// UC-022 step 2: `place` of `text` as the list shows it, about
/// [`LINE_CONTEXT`] characters of its line. Only the characters shown are
/// looked at, so a line of a megabyte costs no more than a short one.
#[must_use]
pub fn shown_place(text: &str, place: &NotePlace) -> ShownPlace {
    let range = &place.range;
    let near = LINE_CONTEXT / 3;
    // The line before the place, as far as it is shown (4 bytes a char).
    let from = back_by(text, range.start, 4 * (near + 1));
    let window = &text[from..range.start];
    let (window, more_before) = match window.rfind('\n') {
        Some(n) => (&window[n + 1..], false),
        None => (window, from > 0 && !text[..from].ends_with('\n')),
    };
    let (before, cut_before) = tail(window, near);
    let cut_before = cut_before || more_before;
    let (old, cut_old) = head(&text[range.clone()], LINE_CONTEXT / 2);
    let (new, cut_new) = head(&place.replaced, LINE_CONTEXT / 2);
    let room = LINE_CONTEXT
        .saturating_sub(before.chars().count() + old.chars().count())
        .max(LINE_CONTEXT / 6);
    let to = on_by(text, range.end, 4 * (room + 1));
    let window = &text[range.end..to];
    let (window, more_after) = match window.find('\n') {
        Some(n) => (&window[..n], false),
        None => (window, to < text.len() && !text[to..].starts_with('\n')),
    };
    let (after, cut_after) = head(window.trim_end_matches('\r'), room);
    let cut_after = cut_after || more_after;
    let mark = |text: &str, cut: bool, front: bool| {
        let text = flat(text);
        match (cut, front) {
            (false, _) => text,
            (true, true) => format!("…{text}"),
            (true, false) => format!("{text}…"),
        }
    };
    ShownPlace {
        before: mark(before, cut_before, true),
        old: mark(old, cut_old, false),
        new: mark(new, cut_new, false),
        after: mark(after, cut_after, false),
    }
}

/// `text` in lower case, the way the search compares (BR-002).
#[must_use]
pub fn lower(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// `text` in lower case, with the byte offset in `text` of every byte
/// offset of the lowered text (and of its end).
fn lower_with_map(text: &str) -> (String, Vec<usize>) {
    let mut lowered = String::with_capacity(text.len());
    let mut map = Vec::with_capacity(text.len() + 1);
    for (at, c) in text.char_indices() {
        for l in c.to_lowercase() {
            for _ in 0..l.len_utf8() {
                map.push(at);
            }
            lowered.push(l);
        }
    }
    map.push(text.len());
    (lowered, map)
}

/// A line of a note that holds a word (UC-008 BR-004).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundLine {
    /// The byte range of the whole line in the note.
    pub line: Range<usize>,
    /// The part of the line shown, shortened around the words when long,
    /// and the words in it, as byte ranges of `shown`.
    pub shown: String,
    pub marks: Vec<Range<usize>>,
    /// The first word found, as a byte range of the note.
    pub first: Range<usize>,
}

/// The lines of `text` that hold a word, the first [`SHOWN_PLACES`] of them
/// shown, and how many there are.
#[must_use]
pub fn found_lines(query: &Query, text: &str) -> (usize, Vec<FoundLine>) {
    found_lines_lowered(query, text, &lower(text))
}

/// [`found_lines`] with `text` already in lower case as `lowered`. A line
/// break stays a line break in lower case, so the lines of both match.
#[must_use]
pub fn found_lines_lowered(query: &Query, text: &str, lowered: &str) -> (usize, Vec<FoundLine>) {
    if !query.plain() {
        return found_lines_switched(query, text);
    }
    let mut lines = Vec::new();
    let mut places = 0;
    let mut start = 0;
    for (line, low) in text.split('\n').zip(lowered.split('\n')) {
        if query.words.iter().any(|word| low.contains(word.as_str())) {
            places += 1;
            if lines.len() < SHOWN_PLACES {
                let words: Vec<Range<usize>> = query
                    .occurrences(line)
                    .into_iter()
                    .map(|r| start + r.start..start + r.end)
                    .collect();
                if !words.is_empty() {
                    lines.push(shown_line(text, start..start + line.len(), &words));
                }
            }
        }
        start += line.len() + 1;
    }
    (places, lines)
}

/// [`found_lines`] with a search switch on (UC-008 BR-011): the places
/// found in the whole text, by the line each starts in.
fn found_lines_switched(query: &Query, text: &str) -> (usize, Vec<FoundLine>) {
    found_lines_at(text, query.occurrences(text))
}

/// The lines of `text` holding `places` (byte ranges, sorted by start),
/// the first [`SHOWN_PLACES`] of them shown with the places marked, and how
/// many lines there are: UC-008 BR-012, where the tags found are the
/// places.
#[must_use]
pub fn found_lines_at(text: &str, places: Vec<Range<usize>>) -> (usize, Vec<FoundLine>) {
    let mut lines = Vec::new();
    let mut count = 0;
    let mut found = places
        .into_iter()
        .filter(|range| range.end <= text.len() && range.start < range.end)
        .peekable();
    let mut start = 0;
    for line in text.split('\n') {
        let end = start + line.len();
        let mut words = Vec::new();
        while let Some(range) = found.next_if(|r| r.start <= end) {
            words.push(range);
        }
        if !words.is_empty() {
            count += 1;
            if lines.len() < SHOWN_PLACES {
                lines.push(shown_line(text, start..end, &words));
            }
        }
        if found.peek().is_none() {
            break;
        }
        start = end + 1;
    }
    (count, lines)
}

/// The line `line` of `text`, shortened to about [`LINE_CONTEXT`]
/// characters around its first word, with the words marked.
fn shown_line(text: &str, line: Range<usize>, words: &[Range<usize>]) -> FoundLine {
    let raw = text[line.clone()].trim_end_matches('\r');
    let first = words[0].clone();
    let (mut from, mut to) = (0, raw.len());
    if raw.chars().count() > LINE_CONTEXT {
        let at = (first.start - line.start).min(raw.len());
        let before: Vec<usize> = raw[..at].char_indices().map(|(i, _)| i).collect();
        from = before
            .len()
            .checked_sub(LINE_CONTEXT / 3)
            .map_or(0, |n| before[n]);
        to = raw[from..]
            .char_indices()
            .nth(LINE_CONTEXT)
            .map_or(raw.len(), |(i, _)| from + i);
    }
    let mut shown = String::new();
    if from > 0 {
        shown.push('…');
    }
    let offset = shown.len();
    shown.push_str(&raw[from..to]);
    // Places that touch are marked as one, as the search always showed them.
    let mut marks: Vec<Range<usize>> = Vec::new();
    for w in words {
        let start = (w.start - line.start).max(from);
        let end = (w.end - line.start).min(to);
        if start >= end {
            continue;
        }
        let mark = start - from + offset..end - from + offset;
        match marks.last_mut() {
            Some(last) if mark.start <= last.end => last.end = last.end.max(mark.end),
            _ => marks.push(mark),
        }
    }
    if to < raw.len() {
        shown.push('…');
    }
    FoundLine {
        line,
        shown,
        marks,
        first,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> Query {
        Query::parse(text)
    }

    /// TC-008-02 / TC-008-03 / TC-008-04: words, quotes, case, accents.
    #[test]
    fn uc008_tc02_to_tc04_what_matches() {
        assert_eq!(
            q(r#"  Plan "release  plan" x"#).words,
            ["plan", "release  plan", "x"]
        );
        assert!(q("Plan release").matches_all(&["release plan for Q1"]));
        assert!(q("Plan release").matches_all(&["plan the RELEASES"]));
        assert!(!q("Plan release").matches_all(&["release notes"]));
        assert!(q("leas").matches_all(&["release notes"]));
        assert!(q(r#""release plan""#).matches_all(&["release plan for Q1"]));
        assert!(!q(r#""release plan""#).matches_all(&["plan the RELEASES"]));
        assert!(!q("uber").matches_all(&["Über uns"]));
        assert!(q("ÜBER").matches_all(&["Über uns"]));
        assert!(q("travel costs").matches_all(&["Travel", "costs here"]));
        assert!(q("   ").is_empty());
    }

    /// The quick way for ASCII finds what the general one finds.
    #[test]
    fn occurrences_of_ascii_text_as_of_any_text() {
        let ascii = "Ferry FERRY ferryman, a ferr-y; \"release plan\" Plan";
        let any = format!("{ascii}é");
        for query in ["ferry", "plan release", "\"release plan\"", "y", "é"] {
            assert_eq!(
                q(query).occurrences(ascii),
                q(query).occurrences(&any)[..]
                    .iter()
                    .filter(|r| r.end <= ascii.len())
                    .cloned()
                    .collect::<Vec<_>>(),
                "{query}"
            );
        }
    }

    #[test]
    fn occurrences_are_byte_ranges_of_the_text() {
        let text = "Ünïcode FERRY and ferry, Ferryman";
        let found = q("ferry").occurrences(text);
        let words: Vec<&str> = found.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(words, ["FERRY", "ferry", "Ferry"]);
        let overlapping = q("ab bc").occurrences("abc");
        assert_eq!((overlapping.len(), overlapping[0].clone()), (1, 0..3));
    }

    /// TC-008-11: five lines shown, the long one shortened around the word.
    #[test]
    fn uc008_tc11_many_places_and_long_lines() {
        let long = format!("{} deploy {}", "x".repeat(300), "y".repeat(100));
        let mut text = String::new();
        for n in 0..7 {
            text.push_str(&format!("line {n}: deploy now\n"));
        }
        text.push_str(&long);
        let (places, lines) = found_lines(&q("deploy"), &text);
        assert_eq!(places, 8);
        assert_eq!(lines.len(), SHOWN_PLACES);
        assert_eq!(&lines[0].shown[lines[0].marks[0].clone()], "deploy");
        assert_eq!(&text[lines[1].first.clone()], "deploy");
        let (_, only) = found_lines(&q("deploy"), &long);
        let line = &only[0];
        assert!(line.shown.starts_with('…') && line.shown.ends_with('…'));
        assert!(line.shown.chars().count() <= LINE_CONTEXT + 2);
        assert_eq!(&line.shown[line.marks[0].clone()], "deploy");
    }

    fn with(text: &str, case: bool, whole: bool, regex: bool, reading: Reading) -> Query {
        Query::new(
            text,
            Switches {
                match_case: case,
                whole_word: whole,
                regex,
            },
            reading,
        )
        .unwrap()
    }

    fn texts<'t>(text: &'t str, ranges: &[Range<usize>]) -> Vec<&'t str> {
        ranges.iter().map(|r| &text[r.clone()]).collect()
    }

    /// TC-008-18, FR-075, UC-008 BR-011: the search switches in the search
    /// of the workspace.
    #[test]
    fn uc008_tc18_fr075_search_switches() {
        let words = Reading::Words;
        let plan = |case, whole, regex, text: &str| with(text, case, whole, regex, words);
        let notes = ["Plan for Q1", "planning day", "the plan", "Plän"];
        let found = |query: &Query| -> Vec<usize> {
            (0..notes.len())
                .filter(|&i| query.matches_all(&[notes[i]]))
                .collect()
        };
        assert_eq!(found(&plan(false, false, false, "plan")), [0, 1, 2]);
        assert_eq!(found(&plan(true, false, false, "plan")), [1, 2]);
        assert_eq!(found(&plan(false, true, false, "plan")), [0, 2]);
        assert_eq!(found(&plan(false, false, true, "^the")), [2]);
        // Accents count with every switch.
        assert_eq!(found(&plan(false, false, true, "pl.n")), [0, 1, 2, 3]);
        assert!(!plan(false, true, false, "plan").matches_all(&["Plän"]));
    }

    /// TC-008-19: the switches in the find field: 3, 2, 1, and `pl.n` with
    /// all three on.
    #[test]
    fn uc008_tc19_switches_in_the_find_field() {
        let text = "Plan plan planning";
        let count = |q: Query| q.occurrences(text).len();
        assert_eq!(count(with("plan", false, false, false, Reading::Words)), 3);
        assert_eq!(count(with("plan", true, false, false, Reading::Words)), 2);
        assert_eq!(count(with("plan", true, true, false, Reading::Words)), 1);
        assert_eq!(count(with("pl.n", true, true, true, Reading::Words)), 1);
    }

    /// TC-008-20, TC-005-31, TC-022-06: what is wrong with an expression
    /// and with a replacement.
    #[test]
    fn uc008_tc20_uc022_tc06_not_valid() {
        let switches = Switches {
            regex: true,
            ..Switches::default()
        };
        let error = Query::new("(plan", switches, Reading::Words).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Not a valid regular expression: unclosed group."
        );
        assert!(Query::new("[a-", switches, Reading::OneText).is_err());
        let groups = Query::new(r"(\d\d)", switches, Reading::OneText).unwrap();
        assert_eq!(
            groups.replacement("$2").unwrap_err().to_string(),
            "The replacement names $2, but the expression has no group 2."
        );
        assert!(groups.replacement("$1 and $0, $$9").is_ok());
        // OQ-193: no look-behind.
        assert!(Query::new("(?<=a)b", switches, Reading::OneText).is_err());
    }

    /// TC-022-02, OQ-192: replacing reads the search text as one text.
    #[test]
    fn uc022_tc02_one_text() {
        let query = with("release plan", false, false, false, Reading::OneText);
        assert!(query.matches_all(&["the release plan"]));
        assert!(!query.matches_all(&["plan the release"]));
        let spaced = with(" x ", false, false, false, Reading::OneText);
        assert_eq!(spaced.occurrences("a x b"), vec![1..4]);
    }

    /// TC-022-03: match case and whole word: 3, 2, 1 places.
    #[test]
    fn uc022_tc03_match_case_and_whole_word() {
        let text = "Plan plan planning";
        let found = |case, whole| {
            let query = with("plan", case, whole, false, Reading::OneText);
            texts(text, &query.occurrences(text)).len()
        };
        assert_eq!(
            (found(false, false), found(true, false), found(true, true)),
            (3, 2, 1)
        );
    }

    /// TC-022-04, BR-003: groups in the replacement.
    #[test]
    fn uc022_tc04_regular_expression_with_groups() {
        let text = "Due 11.11.2026\nDue 01.12.2026";
        let query = with(
            r"(\d\d)\.(\d\d)\.(\d{4})",
            false,
            false,
            true,
            Reading::OneText,
        );
        let replacement = query.replacement("$3-$2-$1").unwrap();
        let places = note_places(&query, &replacement, text);
        let new: Vec<(String, usize)> = places
            .iter()
            .map(|p| (p.replaced.clone(), p.line))
            .collect();
        assert_eq!(
            new,
            [("2026-11-11".to_owned(), 1), ("2026-12-01".to_owned(), 2)]
        );
        let replaced: Vec<_> = places
            .iter()
            .map(|p| (p.range.clone(), p.replaced.clone()))
            .collect();
        assert_eq!(
            replace_places(text, &replaced),
            "Due 2026-11-11\nDue 2026-12-01"
        );
        // `$0`, `$$` and an empty replacement (BR-003).
        let whole = query.replacement("[$0] $$").unwrap();
        assert_eq!(
            query.replaced(text, &places[0].range, &whole),
            "[11.11.2026] $"
        );
        let none = query.replacement("").unwrap();
        assert_eq!(query.replaced(text, &places[0].range, &none), "");
        // Without the switch, `$1` is written as typed.
        let plain = with("Due", false, false, false, Reading::OneText);
        let typed = plain.replacement("$1").unwrap();
        assert_eq!(plain.replaced(text, &(0..3), &typed), "$1");
    }

    /// UC-022 BR-002: a place never spans two lines unless the expression
    /// names a line end; regular expressions match within a line.
    #[test]
    fn uc022_br002_places_within_a_line() {
        let text = "ferry  \n  port\r\nend";
        let spaces = with(r"y\s+", false, false, true, Reading::OneText);
        assert_eq!(texts(text, &spaces.occurrences(text)), ["y  "]);
        let across = with(r"y\s*\n\s*p", false, false, true, Reading::OneText);
        assert_eq!(texts(text, &across.occurrences(text)), ["y  \n  p"]);
        let ends = with("t$", false, false, true, Reading::OneText);
        assert_eq!(texts(text, &ends.occurrences(text)), ["t"]);
        // A match of nothing is no place.
        let empty = with("x*", false, false, true, Reading::OneText);
        assert!(empty.occurrences(text).is_empty());
        let (places, lines) = found_lines(&spaces, text);
        assert_eq!((places, lines[0].shown.as_str()), (1, "ferry  "));
    }

    /// UC-022 A4: a place keeps its key while lines are added around it;
    /// lines alike are told apart.
    #[test]
    fn uc022_a4_place_keys() {
        let query = with("ferry", false, false, false, Reading::OneText);
        let with_ = query.replacement("boat").unwrap();
        let before = note_places(&query, &with_, "ferry\nferry");
        let after = note_places(&query, &with_, "ferry port\nferry\nferry");
        assert_eq!(before[0].key, after[1].key);
        assert_eq!(before[1].key, after[2].key);
        assert_ne!(before[0].key, before[1].key);
        assert_eq!(after[2].line, 3);
    }

    /// TC-022-01: a place as the list shows it, before and after.
    #[test]
    fn uc022_tc01_shown_place() {
        let text = "x\nBook the ferry today";
        let query = with("ferry", false, false, false, Reading::OneText);
        let place = &note_places(&query, &query.replacement("boat").unwrap(), text)[0];
        assert_eq!(
            shown_place(text, place),
            ShownPlace {
                before: "Book the ".into(),
                old: "ferry".into(),
                new: "boat".into(),
                after: " today".into(),
            }
        );
        let long = format!("{} ferry {}", "a".repeat(300), "b".repeat(300));
        let shown = shown_place(
            &long,
            &note_places(&query, &query.replacement("").unwrap(), &long)[0],
        );
        assert!(shown.before.starts_with('…') && shown.after.ends_with('…'));
        assert!(shown.before.chars().count() + shown.after.chars().count() < LINE_CONTEXT + 4);
    }

    /// UC-022 BR-007: a line of a megabyte with many places costs no more
    /// than the places: keys hash the line once, shown parts stay short.
    #[test]
    fn uc022_br007_a_long_line_with_many_places() {
        let line = "ferry ".repeat(175_000);
        let text = format!("{line}\nferry");
        let query = with("ferry", false, false, false, Reading::OneText);
        let start = std::time::Instant::now();
        let places = note_places(&query, &query.replacement("boat").unwrap(), &text);
        let shown: Vec<ShownPlace> = places
            .iter()
            .take(10_000)
            .map(|p| shown_place(&text, p))
            .collect();
        assert!(start.elapsed().as_secs() < 10, "{:?}", start.elapsed());
        assert_eq!(places.len(), 175_001);
        assert_eq!(places[1].key.column, 6);
        assert_eq!(places[175_000].line, 2);
        assert!(shown
            .iter()
            .all(|s| s.before.chars().count() <= LINE_CONTEXT / 3 + 1));
        assert!(shown[5_000].after.ends_with('…'));
        assert!(shown[5_000].before.starts_with('…'));
        assert_eq!(std::mem::size_of::<PlaceKey>(), 32, "no copy of the line");
    }

    /// ADR-026: an expression that is too large is refused.
    #[test]
    fn adr026_too_large() {
        let regex = Switches {
            regex: true,
            ..Switches::default()
        };
        let long = "a".repeat(5_000);
        assert_eq!(
            Query::new(&long, regex, Reading::OneText)
                .unwrap_err()
                .to_string(),
            "Not a valid regular expression: it is too large."
        );
        assert!(Query::new(r"(\w{100}){100}", regex, Reading::OneText).is_err());
    }

    /// UC-022 BR-002: after a match over a line end the search goes on line
    /// by line and still finds every place.
    #[test]
    fn uc022_br002_line_by_line_after_a_match_over_a_line_end() {
        let text = "a1\nxx\na2 a3\nb";
        let query = with(r"a\d[^z]*?\s", false, false, true, Reading::OneText);
        let found: Vec<&str> = query
            .occurrences(text)
            .iter()
            .map(|r| &text[r.clone()])
            .collect();
        assert_eq!(found, ["a2 "]);
    }

    // ---- Test-monkey: package 1 (FR-073 to FR-075, UC-022) ----

    /// The text after replacing every place of `search` with `typed`.
    fn replace_all(query: &Query, text: &str, typed: &str) -> String {
        let with = query.replacement(typed).unwrap();
        let places: Vec<(Range<usize>, String)> = query
            .occurrences(text)
            .iter()
            .map(|r| (r.clone(), query.replaced(text, r, &with)))
            .collect();
        replace_places(text, &places)
    }

    /// UC-022 BR-002, BR-003: `a` in `aaaa` is four places; each one is
    /// replaced. Places that touch must not be merged into one.
    #[test]
    fn uc022_br002_touching_places_are_not_merged() {
        for case in [false, true] {
            let query = with("a", case, false, false, Reading::OneText);
            assert_eq!(
                query.occurrences("aaaa"),
                [0..1, 1..2, 2..3, 3..4],
                "match case {case}"
            );
            assert_eq!(replace_all(&query, "aaaa", "b"), "bbbb");
            let non_ascii = with("é", case, false, false, Reading::OneText);
            assert_eq!(replace_all(&non_ascii, "ééé", "e"), "eee");
        }
        let text = "abab";
        let query = with("ab", false, false, false, Reading::OneText);
        assert_eq!(replace_all(&query, text, "X"), "XX");
    }

    /// UC-022 BR-002, BR-003: a text that lowers to part of a character
    /// (`i` in `İ`) finds no place there; no empty place, nothing inserted.
    #[test]
    fn uc022_br002_no_empty_place_inside_a_character() {
        let query = with("i", false, false, false, Reading::OneText);
        let text = "İstanbul";
        assert!(
            query.occurrences(text).iter().all(|r| r.start < r.end),
            "{:?}",
            query.occurrences(text)
        );
        assert_eq!(replace_all(&query, text, "X"), text);
        let places = note_places(&query, &query.replacement("X").unwrap(), text);
        assert!(places.is_empty(), "{places:?}");
    }

    /// UC-022 BR-003, OQ-230: the edge cases of a replacement.
    #[test]
    fn uc022_br003_oq230_replacement_texts() {
        let query = with(r"(a)(b)?", false, false, true, Reading::OneText);
        let write = |typed: &str| replace_all(&query, "a", typed);
        assert_eq!(write("$$"), "$");
        assert_eq!(write("$$1"), "$1");
        assert_eq!(write("$$$1"), "$a");
        assert_eq!(write("$"), "$");
        assert_eq!(write("${1}"), "${1}");
        assert_eq!(write("$name"), "$name");
        assert_eq!(write("$10"), "a0");
        assert_eq!(write("$2"), "");
        assert_eq!(write("$0$0"), "aa");
        assert_eq!(write("é$1ß"), "éaß");
        assert_eq!(query.replacement("$3"), Err(QueryError::NoGroup(3)));
        assert_eq!(query.replacement("$9"), Err(QueryError::NoGroup(9)));
        // Without the switch the text is written as typed.
        let plain = with("a", false, false, false, Reading::OneText);
        assert_eq!(replace_all(&plain, "a", "$1$$ ${x}"), "$1$$ ${x}");
        // The switch is on but the text is empty: no pattern, typed text.
        let empty = with("", false, false, true, Reading::OneText);
        assert!(empty.is_empty());
        assert!(empty.replacement("$1").is_ok());
    }

    /// UC-022 BR-003: the replacement is not read again: a replacement that
    /// holds the search text replaces once, and replacing a text by itself
    /// changes nothing.
    #[test]
    fn uc022_br003_a_replacement_that_recreates_the_search_text() {
        let query = with("a", false, false, false, Reading::OneText);
        assert_eq!(replace_all(&query, "a-A-a", "aa"), "aa-aa-aa");
        let regex = with("a+", false, false, true, Reading::OneText);
        assert_eq!(replace_all(&regex, "aaa b a", "$0$0"), "aaaaaa b aa");
        let same = with("Straße", false, false, false, Reading::OneText);
        let text = "STRAßE straße x";
        assert_eq!(replace_all(&same, text, "Straße"), "Straße Straße x");
    }

    /// UC-022 BR-002: zero-width expressions find no place, with and
    /// without whole word (OQ-233); anchors; overlapping candidates.
    #[test]
    fn uc022_br002_oq233_zero_width_and_anchors() {
        let text = "ab\n\nb\n";
        let text = text.replace("\\n", "\n");
        for whole in [false, true] {
            for pattern in ["^", "$", "x*", "(|b)", r"\b", r"\B", "^$"] {
                let query = with(pattern, false, whole, true, Reading::OneText);
                for range in query.occurrences(&text) {
                    assert!(range.start < range.end, "{pattern:?} {range:?}");
                }
            }
        }
        let anchored = with("^b$", false, false, true, Reading::OneText);
        assert_eq!(texts(&text, &anchored.occurrences(&text)), ["b"]);
        assert_eq!(replace_all(&anchored, &text, "X"), "ab\n\nX\n");
        // Overlapping candidates: leftmost, then on after it.
        let overlap = with("aa", false, false, true, Reading::OneText);
        assert_eq!(replace_all(&overlap, "aaaaa", "b"), "bba");
        let plain = with("aa", false, false, false, Reading::OneText);
        assert_eq!(replace_all(&plain, "aaaaa", "b"), "bba");
    }

    /// UC-022 BR-002: CRLF and a lone CR: `$` and `^` work, a place never
    /// holds the `\r`, a place does not span the lines.
    #[test]
    fn uc022_br002_crlf_and_lone_cr() {
        let query = with(r"^foo$", false, false, true, Reading::OneText);
        for text in ["foo\r\nfoo\r\n", "foo\rfoo\r", "foo\nfoo\n"] {
            let found = query.occurrences(text);
            assert!(
                texts(text, &found).iter().all(|t| *t == "foo"),
                "{text:?}: {found:?}"
            );
            assert!(!found.is_empty(), "{text:?}");
        }
        assert_eq!(replace_all(&query, "foo\r\nfoo\r\n", "X"), "X\r\nX\r\n");
        let dot = with(r"a.*", false, false, true, Reading::OneText);
        let text = "ab\r\ncd\ra x\n";
        assert_eq!(texts(text, &dot.occurrences(text)), ["ab", "a x"]);
        let spans = with(r"b\r\nc", false, false, true, Reading::OneText);
        assert_eq!(replace_all(&spans, text, "-"), "a-d\ra x\n");
    }

    /// UC-022 BR-002: a place never spans two lines unless the expression
    /// matches a line end, in whatever notation.
    #[test]
    fn uc022_br002_a_place_never_spans_lines_by_accident() {
        let text = "a\nb a\nb";
        for pattern in [r"a\s", r"a[^x]", r"(?s)a.", r"a\W"] {
            let query = with(pattern, false, false, true, Reading::OneText);
            for range in query.occurrences(text) {
                assert!(
                    !text[range.clone()].contains('\n'),
                    "{pattern}: {:?}",
                    &text[range]
                );
            }
        }
        // An expression that names the line end spans lines.
        for pattern in [r"a\nb", "a\nb", r"a[\n]b", r"a\x0Ab", r"a\x{A}b"] {
            let query = with(pattern, false, false, true, Reading::OneText);
            assert_eq!(
                texts(text, &query.occurrences(text)),
                ["a\nb", "a\nb"],
                "{pattern}"
            );
        }
    }

    /// UC-022 BR-002: `\z` and `\A` mean the end and the start of the note,
    /// also after a place that spans lines.
    #[test]
    fn uc022_br002_text_anchors_stay_text_anchors() {
        let text = "x\ny a\nb a\nlast a";
        let query = with(r"a\nb|a\z", false, false, true, Reading::OneText);
        assert_eq!(texts(text, &query.occurrences(text)), ["a\nb", "a"]);
        let start = with(r"x\ny|\Aa", false, false, true, Reading::OneText);
        let text = "x\ny\na";
        assert_eq!(texts(text, &start.occurrences(text)), ["x\ny"]);
        let end = with(r"y\n|a\z", false, false, true, Reading::OneText);
        let text = "y\na\nb a\nc";
        assert_eq!(texts(text, &end.occurrences(text)), ["y\n"]);
    }

    /// UC-022 BR-002: whole word does not cut a letter from its combining
    /// mark.
    #[test]
    fn uc022_br002_whole_word_and_combining_marks() {
        let text = "cafe\u{301} cafe";
        let query = with("cafe", false, true, false, Reading::OneText);
        let found = texts(text, &query.occurrences(text));
        assert_eq!(found, ["cafe"], "{found:?}");
        // Corrected (the test asked for 2 here, which contradicts the line
        // above): `cafe` + U+0301 is one longer word, so one place.
        assert_eq!(query.occurrences(text).len(), 1);
    }

    /// UC-022 BR-007: a long line of one word and a regular expression
    /// that whole word rejects at every start is no quadratic wait.
    #[test]
    fn uc022_br007_whole_word_rejections_are_not_quadratic() {
        let text = format!("{}b c", "a".repeat(3_000));
        let query = with("a+", false, true, true, Reading::OneText);
        let start = std::time::Instant::now();
        let found = query.occurrences(&text);
        assert!(found.is_empty());
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            start.elapsed()
        );
    }

    /// Seeded text, from the characters a note could hold.
    fn monkey_text(seed: &mut u64, length: usize) -> String {
        const ALPHABET: &[&str] = &[
            "a", "A", "b", "ab", "aa", "İ", "i", "ß", "SS", "ss", "é", "e\u{301}", "\n", "\r\n",
            "\r", " ", "$", "1", "_", "😀", "\u{feff}", "ǅ", "Σ", "ς", "\u{212a}", "k",
        ];
        let mut out = String::new();
        for _ in 0..length {
            *seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            out.push_str(ALPHABET[(*seed >> 33) as usize % ALPHABET.len()]);
        }
        out
    }

    /// UC-022 BR-002, BR-003: for any text and any plain search text the
    /// places are sorted, apart, not empty, on characters, each one matches
    /// the search text in lower case, and replacing a text by itself
    /// changes nothing. The seed is printed.
    #[test]
    fn uc022_br002_places_of_random_texts_hold_their_invariants() {
        let mut seed: u64 = 0x5EED_0022;
        eprintln!("uc022 plain fuzz seed {seed:#x}");
        for round in 0..3000 {
            let start = seed;
            let text = monkey_text(&mut seed, 1 + round % 24);
            let word = monkey_text(&mut seed, 1 + round % 3);
            for (case, whole) in [(false, false), (true, false), (false, true), (true, true)] {
                let query = with(&word, case, whole, false, Reading::OneText);
                let found = query.occurrences(&text);
                let context = format!("seed {start:#x} text {text:?} word {word:?} {case} {whole}");
                let mut last = 0;
                for range in &found {
                    assert!(range.start < range.end, "empty: {context}: {found:?}");
                    assert!(range.start >= last, "overlap: {context}: {found:?}");
                    assert!(
                        text.is_char_boundary(range.start) && text.is_char_boundary(range.end),
                        "{context}"
                    );
                    let place = &text[range.clone()];
                    let same = if case {
                        place == word
                    } else {
                        lower(place) == lower(&word)
                    };
                    assert!(same, "{place:?} is no {word:?}: {context}");
                    last = range.end;
                }
                let places = note_places(&query, &query.replacement(&word).unwrap(), &text);
                assert_eq!(places.len(), found.len(), "{context}");
                if case {
                    assert_eq!(replace_all(&query, &text, &word), text, "{context}");
                }
            }
        }
    }

    /// UC-022 BR-002, BR-003: random regular expressions on random texts
    /// never panic, find no empty place, none over a line end unless named,
    /// and every place can be written. The seed is printed.
    #[test]
    fn uc022_br002_regular_expressions_on_random_texts() {
        const PIECES: &[&str] = &[
            "a", "b", "ab", ".", ".*", ".+?", "\\s", "\\s*", "\\w+", "\\b", "^", "$", "(a)",
            "(a|b)", "(|b)", "x*", "[^a]", "[a-z]+", "\\d", "é", "(?i)", "(?s).", "\\r", "\\z",
            "\\A", "(a)|(b)", "a{2}", "\\B", "(?-m:^)", "(?-m:$)",
        ];
        let mut seed: u64 = 0xC0FF_EE22;
        eprintln!("uc022 regex fuzz seed {seed:#x}");
        for round in 0..4000 {
            let start = seed;
            let mut pattern = String::new();
            for _ in 0..1 + round % 4 {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                pattern.push_str(PIECES[(seed >> 33) as usize % PIECES.len()]);
            }
            let text = monkey_text(&mut seed, 2 + round % 20);
            for (case, whole) in [(false, false), (true, true), (false, true)] {
                let Ok(query) = Query::new(
                    &pattern,
                    Switches {
                        match_case: case,
                        whole_word: whole,
                        regex: true,
                    },
                    Reading::OneText,
                ) else {
                    continue;
                };
                let context =
                    format!("seed {start:#x} pattern {pattern:?} text {text:?} {case} {whole}");
                let found = query.occurrences(&text);
                let mut last = 0;
                for range in &found {
                    assert!(range.start < range.end, "empty: {context}");
                    assert!(range.start >= last, "overlap: {context}");
                    last = range.end;
                }
                let names_line_end = pattern.contains("\\n");
                if !names_line_end {
                    for range in &found {
                        assert!(
                            !text[range.clone()].contains('\n'),
                            "spans a line: {context}: {:?}",
                            &text[range.clone()]
                        );
                    }
                }
                for groups in ["$0", "$1", "x$0y"] {
                    if let Ok(with) = query.replacement(groups) {
                        let _ = replace_all(&query, &text, groups);
                        let _ = with;
                    }
                }
            }
        }
    }

    /// UC-022 BR-006, A7: a hostile expression is refused or bounded, and a
    /// huge replacement does not break the places shown.
    #[test]
    fn uc022_br007_hostile_patterns_and_replacements() {
        for pattern in [
            "(((a{1000}){1000}){1000})",
            r"\p{L}{100}{100}",
            "(a*)*b",
            &"(".repeat(5000),
            &"a".repeat(5000),
            r"[\p{L}--a]",
            "\0",
            "(?<n>a)\\k<n>",
        ] {
            let started = std::time::Instant::now();
            let _ = Query::new(
                pattern,
                Switches {
                    regex: true,
                    ..Switches::default()
                },
                Reading::OneText,
            )
            .map(|q| q.occurrences(&"a".repeat(2000)));
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "{pattern:.30}"
            );
        }
        let query = with("a", false, false, false, Reading::OneText);
        let huge = "x".repeat(2_000_000);
        let place = &note_places(&query, &query.replacement(&huge).unwrap(), "a b")[0];
        let shown = shown_place("a b", place);
        assert!(
            shown.new.chars().count() <= LINE_CONTEXT,
            "{}",
            shown.new.len()
        );
    }

    /// UC-022 step 2: the list shows any place, on any text, without a
    /// panic, with every part shortened.
    #[test]
    fn uc022_step2_shown_place_of_random_texts() {
        let mut seed: u64 = 0x5110_0022;
        eprintln!("uc022 shown_place fuzz seed {seed:#x}");
        for round in 0..1500 {
            let start = seed;
            let text = monkey_text(&mut seed, 1 + (round * 7) % 400);
            let word = monkey_text(&mut seed, 1 + round % 2);
            let query = with(&word, false, false, false, Reading::OneText);
            let with_text = monkey_text(&mut seed, 1 + round % 90);
            let with_r = query.replacement(&with_text).unwrap();
            for place in note_places(&query, &with_r, &text) {
                let shown = shown_place(&text, &place);
                let all = shown.before.chars().count()
                    + shown.old.chars().count()
                    + shown.new.chars().count()
                    + shown.after.chars().count();
                assert!(
                    all <= 3 * LINE_CONTEXT,
                    "seed {start:#x}: {all} chars shown in {text:?}"
                );
                assert!(
                    !shown.before.contains('\n') && !shown.after.contains('\n'),
                    "seed {start:#x}"
                );
            }
        }
    }
}
