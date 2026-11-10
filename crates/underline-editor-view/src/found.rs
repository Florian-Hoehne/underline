//! UC-008 BR-008, A6, M-11.3, M-13.2, M-13.4: the words marked in a tab:
//! those of the workspace search (or of the find field, for the active tab)
//! and those of the last search of the Nano keys, as egui's source view
//! marks them (`set_search`, `found_ranges`), and the find field's steps
//! from one to the next (`find_in_note` of the egui app).
//!
//! The window's editor core owns the text (ADR-025), so the marks are found
//! here, in the WASM, with the same rules as the workspace search
//! ([`underline_markup::search::Query`]); `docs/migration/offene-fragen.md`
//! Q-38. They are found after the inputs (`views`), not in the keystroke,
//! and while typing goes on with the same search at most every
//! [`FOUND_EVERY_MS`]; a step of the find field finds them at once.

use std::ops::Range;

use underline_markup::search::Query;

use crate::editor_view::{revision, units, FindAnswer, FindStatus, FindStep, FoundView};
use crate::EditorTab;

/// How often the marked hits are found again while the text changes with
/// the same search: 300 ms.
pub const FOUND_EVERY_MS: f64 = 300.0;

/// What the hits were found for: the revision of the text, the pattern of
/// the Nano search, and the words of the search of the window.
type FoundKey = (u64, Option<String>, Option<Query>);

/// The marked words of one tab.
#[derive(Debug, Default)]
pub(crate) struct Found {
    /// UC-008 BR-008: the words of the search of the window, or of the find
    /// field for the active tab (A6).
    query: Option<Query>,
    /// The hits as char ranges, sorted, and what they were found for.
    cache: Option<(FoundKey, Vec<Range<usize>>)>,
    /// What the window was told of last.
    pub(crate) told: Option<FoundKey>,
    /// When the hits were found last, in milliseconds of the host's clock.
    at: Option<f64>,
}

impl EditorTab {
    /// UC-008 BR-008, M-13.2: marks every occurrence of the words of
    /// `text` (BR-002), or none without a word. The Nano search keeps its
    /// own marks beside them.
    pub fn set_search(&mut self, text: Option<&str>) {
        let query = text.map(Query::parse).filter(|query| !query.is_empty());
        self.found.query = query;
    }

    /// The key of the hits as the text and the searches are now.
    fn found_key(&self) -> FoundKey {
        (
            self.editor.revision(),
            self.keys.search().map(str::to_owned),
            self.found.query.clone(),
        )
    }

    /// The occurrences of the marked words as char ranges, sorted: those of
    /// the window's search and those of the Nano search; ranges of the two
    /// may overlap. Worked out again when the text or a search changed.
    pub fn found_ranges(&mut self) -> &[Range<usize>] {
        let key = self.found_key();
        let fresh = self
            .found
            .cache
            .as_ref()
            .is_some_and(|(cached, _)| *cached == key);
        if !fresh {
            let ranges = self.find_ranges(&key);
            self.found.cache = Some((key, ranges));
        }
        self.found.cache.as_ref().map_or(&[], |(_, ranges)| ranges)
    }

    /// As egui's `found_ranges`: the words' byte ranges and the pattern's,
    /// sorted and without doubles, as chars.
    fn find_ranges(&self, (_, pattern, query): &FoundKey) -> Vec<Range<usize>> {
        if query.is_none() && pattern.is_none() {
            return Vec::new();
        }
        let mut ranges: Vec<Range<usize>> = match query {
            Some(query) => {
                let text = self.editor.text();
                chars_of(&text, query.occurrences(&text))
            }
            None => Vec::new(),
        };
        if let Some(pattern) = pattern.as_deref() {
            ranges.extend(self.editor.find_all(pattern));
        }
        ranges.sort_by_key(|r| (r.start, r.end));
        ranges.dedup();
        // A word may lower into part of a character (`İ` lowers to `i̇`):
        // its range is empty in the text and marks nothing.
        ranges.retain(|r| r.start < r.end);
        ranges
    }

    /// M-12.4, M-13.2, UC-008 BR-008: the marked hits at `now_ms`, once
    /// they changed; while typing goes on with the same searches, at most
    /// every [`FOUND_EVERY_MS`] (ADR-024). `None` while the window has them.
    pub fn found_view(&mut self, note: &str, now_ms: f64) -> Option<FoundView> {
        self.set_now(now_ms);
        let now_ms = self.now_ms;
        let key = self.found_key();
        let told = self.found.told.clone();
        if told.as_ref() == Some(&key) {
            return None;
        }
        let searching = key.1.is_some() || key.2.is_some();
        let same_search = told
            .as_ref()
            .is_some_and(|(_, pattern, query)| *pattern == key.1 && *query == key.2);
        let recent = self
            .found
            .at
            .is_some_and(|at| (now_ms - at).max(0.0) < FOUND_EVERY_MS);
        if same_search && recent && searching {
            return None;
        }
        let all = self.found_ranges().to_vec();
        let count = units(all.len());
        // A text of ASCII without `\r` has the same offsets in chars and on
        // the web: the hits of a long note need no conversion each.
        let plain = self.editor.document().rope().len_bytes() == self.editor.document().len_chars()
            && !self.editor.document().rope().bytes().any(|b| b == b'\r');
        let ranges = all
            .into_iter()
            .take(FoundView::MAX)
            .map(|range| {
                let web = if plain {
                    range
                } else {
                    self.editor.to_web_range(range)
                };
                (units(web.start), units(web.end))
            })
            .collect();
        // Nothing to tell when no hits were marked and none are.
        let was_marked = told
            .as_ref()
            .is_some_and(|(_, pattern, query)| pattern.is_some() || query.is_some());
        let view = (searching || was_marked).then(|| FoundView {
            note: note.to_owned(),
            rev: revision(key.0),
            ranges,
            count,
        });
        self.found.told = Some(key);
        self.found.at = Some(now_ms);
        view
    }

    /// UC-008 A6, M-13.4: the find field of the tab holds `text`; its words
    /// are marked, and the selection moves to one of them (`find_in_note`
    /// of the egui app):
    ///
    /// - [`FindStep::First`] (the text changed): the first starting at the
    ///   start of the selection or after it, else the first of the note;
    /// - [`FindStep::Next`]: the first starting after it, else the first;
    /// - [`FindStep::Previous`]: the last starting before it, else the last.
    ///
    /// `None` when nothing is found; the selection stays then. A selection
    /// in a folded section expands it, as a click does (M-08.8).
    pub fn find(&mut self, text: &str, step: FindStep) -> Option<FindAnswer> {
        self.set_search(Some(text));
        let at = self.editor.selection().start;
        let found = self.found_ranges();
        let chosen = match step {
            FindStep::Previous => found.iter().rev().find(|r| r.start < at).or(found.last()),
            FindStep::Next => found.iter().find(|r| r.start > at).or(found.first()),
            FindStep::First => found.iter().find(|r| r.start >= at).or(found.first()),
        }
        .cloned()?;
        self.editor.set_selection(chosen.end, chosen.start);
        self.unfold_at_cursor();
        let (head, anchor) = self.editor.selection_web();
        Some(FindAnswer {
            rev: revision(self.editor.revision()),
            anchor: units(anchor),
            head: units(head),
            place: self.editor.cursor_place().label(),
            status: self.find_status(),
        })
    }

    /// M-13.4: what the find field says: which hit is selected (exactly)
    /// and how many there are ("{i} of {n}", "{n} found").
    pub fn find_status(&mut self) -> FindStatus {
        let selection = self.editor.selection();
        let found = self.found_ranges();
        FindStatus {
            at: found
                .iter()
                .position(|r| *r == selection)
                .map(|at| units(at + 1)),
            count: units(found.len()),
        }
    }
}

/// The byte ranges `ranges` of `text`, sorted and not overlapping (as
/// [`Query::occurrences`] gives them), as char ranges, in one walk over the
/// text.
fn chars_of(text: &str, ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if text.is_ascii() {
        return ranges;
    }
    let (mut byte, mut char) = (0, 0);
    let mut to_char = |at: usize| {
        let at = at.clamp(byte, text.len());
        char += text.get(byte..at).map_or(0, |part| part.chars().count());
        byte = at;
        char
    };
    ranges
        .into_iter()
        .map(|range| {
            let start = to_char(range.start);
            start..to_char(range.end)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use underline_markup::NoteFormat;

    use super::*;

    fn tab(text: &str) -> EditorTab {
        EditorTab::new(text, NoteFormat::Markdown)
    }

    /// TC-008-01, M-13.2: the words of the search are marked as egui marks
    /// them, case aside, and follow the text.
    #[test]
    fn uc008_br008_m13_2_the_words_of_the_search_are_marked() {
        let mut tab = tab("Book the ferry. FERRY again, Ferryman.");
        assert!(tab.found_ranges().is_empty());

        tab.set_search(Some("ferry"));

        assert_eq!(tab.found_ranges(), [9..14, 16..21, 29..34]);
        let view = tab.found_view("Trip", 0.0).unwrap();
        assert_eq!(view.count, 3);
        assert_eq!(view.ranges[0], (9, 14));
        assert!(tab.found_view("Trip", 1.0).is_none(), "told once");

        tab.set_search(None);
        let cleared = tab.found_view("Trip", 2.0).unwrap();
        assert!(cleared.ranges.is_empty(), "the marks go (A2)");
        assert!(tab.found_view("Trip", 3.0).is_none());
    }

    /// UC-008 BR-002: words in any order, quoted words with their space,
    /// accents count; the offsets are web units.
    #[test]
    fn uc008_br002_m13_2_words_quotes_and_web_units() {
        let mut tab = tab("😀 release plan, Über uns");
        tab.set_search(Some("\"release plan\" über"));
        let view = tab.found_view("A", 0.0).unwrap();
        assert_eq!(view.ranges, [(3, 15), (17, 21)]);
        tab.set_search(Some("uber"));
        assert!(tab.found_ranges().is_empty());
        tab.set_search(Some("   "));
        assert!(tab.found_view("A", 1.0).unwrap().ranges.is_empty());
    }

    #[test]
    fn the_byte_ranges_become_char_ranges_in_one_walk() {
        let text = "😀 ferry Über ferry";
        let ranges = Query::parse("ferry über").occurrences(text);
        let chars: Vec<String> = chars_of(text, ranges)
            .into_iter()
            .map(|r| text.chars().skip(r.start).take(r.end - r.start).collect())
            .collect();
        assert_eq!(chars, ["ferry", "Über", "ferry"]);
    }

    /// A word found inside one character (`i` in `İ`) gives no empty mark.
    #[test]
    fn uc008_br002_no_empty_marks_inside_a_character() {
        let mut tab = tab("İstanbul and Istanbul");
        tab.set_search(Some("i"));
        assert!(tab.found_ranges().iter().all(|r| r.start < r.end));
        assert_eq!(tab.found_ranges().to_vec(), vec![13..14]);
        assert_eq!(tab.find_status().count, 1);
    }

    /// M-12.4: while typing goes on with the same search, the hits are
    /// found again at most every 300 ms; another search at once.
    #[test]
    fn m12_4_typing_finds_the_hits_again_at_most_every_300_ms() {
        let mut tab = tab("alpha beta alpha");
        tab.set_search(Some("alpha"));
        assert!(tab.found_view("A", 1000.0).is_some());
        tab.editor.type_text("alpha ");
        assert!(tab.found_view("A", 1100.0).is_none(), "too soon");
        let later = tab.found_view("A", 1000.0 + FOUND_EVERY_MS).unwrap();
        assert_eq!(later.count, 3);
        tab.set_search(Some("beta"));
        assert_eq!(tab.found_view("A", 1301.0).unwrap().count, 1);
    }

    /// TC-008-12, M-13.4: the first from the cursor, then Next and
    /// Previous with wrapping, "{i} of {n}".
    #[test]
    fn uc008_tc12_m13_4_find_steps_through_the_note() {
        let text = "ferry one ferry two ferry";
        let mut tab = tab(text);
        // The cursor between the first and the second.
        tab.editor.set_selection(5, 5);

        let first = tab.find("FERRY", FindStep::First).unwrap();
        assert_eq!((first.anchor, first.head), (10, 15), "the second");
        assert_eq!(
            first.status,
            FindStatus {
                at: Some(2),
                count: 3
            }
        );

        let next = tab.find("FERRY", FindStep::Next).unwrap();
        assert_eq!((next.anchor, next.head), (20, 25));
        let wrapped = tab.find("FERRY", FindStep::Next).unwrap();
        assert_eq!((wrapped.anchor, wrapped.head), (0, 5), "wraps to the first");
        let back = tab.find("FERRY", FindStep::Previous).unwrap();
        assert_eq!((back.anchor, back.head), (20, 25), "wraps to the last");
        assert_eq!(back.status.at, Some(3));

        tab.editor.set_selection(2, 2);
        assert_eq!(
            tab.find_status(),
            FindStatus { at: None, count: 3 },
            "{{n}} found"
        );
    }

    /// Q-38, ADR-025: what marking costs in a long note (450,000 chars,
    /// 10,000 lines): after the input, at most every 300 ms, never in the
    /// keystroke. Prints the times; run with `cargo test -p
    /// underline-editor-view --release -- --ignored --nocapture q38`.
    #[test]
    #[ignore = "a measurement"]
    fn q38_m13_2_marking_a_long_note() {
        let line = "Book the ferry to Hydra, then walk along the harbour of the island.\n";
        let text = line.repeat(450_000 / line.len());
        let mut tab = tab(&text);
        tab.set_search(Some("ferry harbour"));
        let mut times = Vec::new();
        for n in 0..20 {
            tab.editor.type_text("x");
            let start = std::time::Instant::now();
            let view = tab.found_view("A", f64::from(n) * 1000.0).unwrap();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(view.ranges.len(), FoundView::MAX);
        }
        let query = Query::parse("ferry harbour");
        let start = std::time::Instant::now();
        let bytes = query.occurrences(&text);
        eprintln!(
            "occurrences alone: {:.1} ms for {} hits",
            start.elapsed().as_secs_f64() * 1000.0,
            bytes.len()
        );
        times.sort_by(f64::total_cmp);
        eprintln!(
            "marking {} chars: median {:.1} ms, max {:.1} ms",
            text.len(),
            times[10],
            times[19]
        );
    }

    /// M-13.4: nothing found leaves the selection; an empty text marks
    /// nothing.
    #[test]
    fn m13_4_nothing_found_leaves_the_selection() {
        let mut tab = tab("ferry");
        tab.editor.set_selection(1, 3);
        assert!(tab.find("zebra", FindStep::First).is_none());
        assert_eq!(tab.editor.selection(), 1..3);
        assert!(tab.find("", FindStep::Next).is_none());
        assert_eq!(tab.find_status(), FindStatus { at: None, count: 0 });
    }
}
