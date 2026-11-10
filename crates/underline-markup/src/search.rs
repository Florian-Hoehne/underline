//! UC-008 Find Note, the pure part: what a search text matches (BR-002),
//! where its words occur in a text (BR-008, A6), and the places found in a
//! note (BR-004). Nothing here reads files or starts threads, so the window's
//! editor core marks the words of a search with the same rules
//! (`docs/migration/offene-fragen.md` Q-38). `underline-core` re-exports it
//! under its former path and searches the workspace with it.

use std::ops::Range;

/// How many lines found a note shows before "N more" (UC-008 A5).
pub const SHOWN_PLACES: usize = 5;
/// How many characters of a long line are shown around the first word.
pub const LINE_CONTEXT: usize = 120;

/// A search text split into words (UC-008 BR-002): at spaces, with words in
/// double quotes kept together; in lower case, since case does not matter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    words: Vec<String>,
}

impl Query {
    /// Splits `text` into its words (BR-002).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut words = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find(|c: char| !c.is_whitespace()) {
            rest = &rest[start..];
            let (word, after) = match rest.strip_prefix('"') {
                Some(quoted) => match quoted.find('"') {
                    Some(end) => (&quoted[..end], &quoted[end + 1..]),
                    None => (quoted, ""),
                },
                None => {
                    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                    (&rest[..end], &rest[end..])
                }
            };
            if !word.is_empty() {
                words.push(lower(word));
            }
            rest = after;
        }
        words.dedup();
        Self { words }
    }

    /// Whether there is no word.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Whether every word occurs somewhere in `texts` together.
    #[must_use]
    pub fn matches_all(&self, texts: &[&str]) -> bool {
        let lowered: Vec<String> = texts.iter().map(|t| lower(t)).collect();
        let lowered: Vec<&str> = lowered.iter().map(String::as_str).collect();
        self.matches_lowered(&lowered)
    }

    /// [`Self::matches_all`] for texts already in lower case.
    #[must_use]
    pub fn matches_lowered(&self, lowered: &[&str]) -> bool {
        self.words
            .iter()
            .all(|word| lowered.iter().any(|text| text.contains(word.as_str())))
    }

    /// Every occurrence of a word in `text`, as byte ranges of `text`,
    /// sorted and not overlapping (UC-008 BR-008, A6).
    #[must_use]
    pub fn occurrences(&self, text: &str) -> Vec<Range<usize>> {
        // Text in ASCII lowers byte for byte, so its offsets stay; the window
        // marks long notes with it after every input (ADR-025, Q-38).
        let mut found = if text.is_ascii() {
            self.find_words(&text.to_ascii_lowercase(), |at| at)
        } else {
            let (lowered, map) = lower_with_map(text);
            self.find_words(&lowered, |at| map[at])
        };
        found.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in found {
            match merged.last_mut() {
                Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
                _ => merged.push(range),
            }
        }
        merged
    }
}

impl Query {
    /// Every occurrence of a word in `lowered`, as ranges of the text it was
    /// lowered from (`original` gives the offset there of an offset in it).
    fn find_words(&self, lowered: &str, original: impl Fn(usize) -> usize) -> Vec<Range<usize>> {
        let mut found: Vec<Range<usize>> = Vec::new();
        for word in self.words.iter().filter(|w| !w.is_empty()) {
            let mut from = 0;
            while let Some(at) = lowered[from..].find(word.as_str()) {
                let start = from + at;
                let end = start + word.len();
                found.push(original(start)..original(end));
                from = end;
            }
        }
        found
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

/// The line `line` of `text`, shortened to about [`LINE_CONTEXT`]
/// characters around its first word, with the words marked.
fn shown_line(text: &str, line: Range<usize>, words: &[Range<usize>]) -> FoundLine {
    let raw = text[line.clone()].trim_end_matches('\r');
    let first = words[0].clone();
    let (mut from, mut to) = (0, raw.len());
    if raw.chars().count() > LINE_CONTEXT {
        let at = first.start - line.start;
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
    let marks = words
        .iter()
        .filter_map(|w| {
            let start = (w.start - line.start).max(from);
            let end = (w.end - line.start).min(to);
            (start < end).then(|| start - from + offset..end - from + offset)
        })
        .collect();
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
}
