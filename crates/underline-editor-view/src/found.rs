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

use underline_markup::blocks::{parse_blocks, Block};
use underline_markup::search::{replace_places, Query, Reading};

use underline_editor_core::facade::MakeViews;
use underline_editor_core::Outcome;

use crate::editor_view::{
    revision, units, EditorPatch, FindAnswer, FindSpec, FindStatus, FindStep, FoundView, Replaced,
};
use crate::facade_view::{BlockView, FacadePatch};
use crate::EditorTab;

/// How often the marked hits are found again while the text changes with
/// the same search: 300 ms.
pub const FOUND_EVERY_MS: f64 = 300.0;

/// What the hits were found for: the revision of the text, the pattern of
/// the Nano search, the search of the window, and whether only the text the
/// facade shows counts.
pub(crate) type FoundKey = (u64, Option<String>, Option<Query>, bool);

/// The marked words of one tab.
#[derive(Debug, Default)]
pub(crate) struct Found {
    /// UC-008 BR-008: the words of the search of the window, or of the find
    /// field for the active tab (A6), with their switches (BR-011).
    query: Option<Query>,
    /// UC-005 BR-017, OQ-190: only the text the facade shows counts.
    shown_only: bool,
    /// UC-008 A9, UC-005 A22: why the search text finds nothing.
    problem: Option<String>,
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
        let spec = text.map(|text| FindSpec {
            text: text.to_owned(),
            ..FindSpec::default()
        });
        self.set_find(spec.as_ref());
    }

    /// UC-008 BR-008, BR-011, UC-005 BR-017: marks every place `spec`
    /// finds, with its search switches, read as one text while replacing,
    /// and in the facade's shown text only when it says so; none without
    /// `spec` or a word. An expression that is not valid marks nothing and
    /// is the find field's problem (A9).
    pub fn set_find(&mut self, spec: Option<&FindSpec>) {
        self.found.problem = None;
        self.found.shown_only = spec.is_some_and(|spec| spec.shown_only);
        self.found.query = spec.and_then(|spec| {
            let reading = if spec.one_text {
                Reading::OneText
            } else {
                Reading::Words
            };
            match Query::new(&spec.text, spec.switches.into(), reading) {
                Ok(query) => Some(query).filter(|query| !query.is_empty()),
                Err(problem) => {
                    self.found.problem = Some(problem.to_string());
                    None
                }
            }
        });
    }

    /// The key of the hits as the text and the searches are now.
    fn found_key(&self) -> FoundKey {
        (
            self.editor.revision(),
            self.keys.search().map(str::to_owned),
            self.found.query.clone(),
            self.found.shown_only,
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

    /// The places of the window's search in `text` (the tab's), as byte
    /// ranges, sorted and apart; in the facade's shown text only when the
    /// search says so (UC-005 BR-017).
    fn hit_bytes(&self, text: &str) -> Vec<Range<usize>> {
        let Some(query) = &self.found.query else {
            return Vec::new();
        };
        let mut hits = query.occurrences(text);
        if self.found.shown_only {
            let spans = if self.facade.parsed == Some(self.editor.revision()) {
                shown_spans(&self.facade.blocks, text)
            } else {
                shown_spans(&parse_blocks(text, self.format), text)
            };
            hits.retain(|hit| within(&spans, hit));
        }
        hits
    }

    /// As egui's `found_ranges`: the words' byte ranges and the pattern's,
    /// sorted and without doubles, as chars.
    fn find_ranges(&self, (_, pattern, query, _): &FoundKey) -> Vec<Range<usize>> {
        if query.is_none() && pattern.is_none() {
            return Vec::new();
        }
        let mut ranges: Vec<Range<usize>> = if query.is_some() {
            let text = self.editor.text();
            chars_of(&text, self.hit_bytes(&text))
        } else {
            Vec::new()
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
        let same_search = told.as_ref().is_some_and(|(_, pattern, query, shown)| {
            *pattern == key.1 && *query == key.2 && *shown == key.3
        });
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
            .is_some_and(|(_, pattern, query, _)| pattern.is_some() || query.is_some());
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

    /// UC-008 A6, M-13.4: the find field holds `text`; its words are
    /// marked, and the selection moves to one of them (as [`Self::find_by`]).
    pub fn find(&mut self, text: &str, step: FindStep) -> Option<FindAnswer> {
        let spec = FindSpec {
            text: text.to_owned(),
            ..FindSpec::default()
        };
        self.find_by(&spec, step)
    }

    /// UC-008 A6, A7, M-13.4: the find field holds `spec` (its text and
    /// search switches); its places are marked, and the selection moves to
    /// one of them (`find_in_note` of the egui app):
    ///
    /// - [`FindStep::First`] (the text changed): the first starting at the
    ///   start of the selection or after it, else the first of the note;
    /// - [`FindStep::Next`]: the first starting after it, else the first;
    /// - [`FindStep::Previous`]: the last starting before it, else the last.
    ///
    /// `None` when nothing is found; the selection stays then. A selection
    /// in a folded section expands it, as a click does (M-08.8).
    pub fn find_by(&mut self, spec: &FindSpec, step: FindStep) -> Option<FindAnswer> {
        self.set_find(Some(spec));
        self.select_hit(step)?;
        let (head, anchor) = self.editor.selection_web();
        Some(FindAnswer {
            rev: revision(self.editor.revision()),
            anchor: units(anchor),
            head: units(head),
            place: self.editor.cursor_place().label(),
            status: self.find_status(),
        })
    }

    /// Selects the hit `step` leads to from the selection; whether there
    /// was one.
    fn select_hit(&mut self, step: FindStep) -> Option<()> {
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
        Some(())
    }

    /// M-13.4: what the find field says: which hit is selected (exactly)
    /// and how many there are ("{i} of {n}", "{n} found"), or what is wrong
    /// with the search text (UC-008 A9).
    pub fn find_status(&mut self) -> FindStatus {
        let selection = self.editor.selection();
        let problem = self.found.problem.clone();
        let found = self.found_ranges();
        FindStatus {
            at: found
                .iter()
                .position(|r| *r == selection)
                .map(|at| units(at + 1)),
            count: units(found.len()),
            problem,
        }
    }

    /// UC-005 A22: what the find field says while it replaces with
    /// `replacement`: as [`Self::find_status`], and what is wrong with the
    /// replacement (a group the expression does not have).
    pub fn replace_status(&mut self, replacement: &str) -> FindStatus {
        let mut status = self.find_status();
        if status.problem.is_none() {
            if let Some(query) = &self.found.query {
                if let Err(problem) = query.replacement(replacement) {
                    status.problem = Some(problem.to_string());
                }
            }
        }
        status
    }

    /// UC-005 A20, A21, BR-017, FR-073: replaces in the text by `spec`
    /// (read as one text) with `replacement`:
    ///
    /// - one (`all` false): the selected occurrence, when the selection is
    ///   one exactly, as one change; then the next one is selected. When the
    ///   selection is none, the next one is selected and nothing replaced.
    /// - all: every occurrence at once, as one change that one undo takes
    ///   back; the cursor goes after the last.
    ///
    /// The outcome for the view, how many were replaced, and what the find
    /// field says; nothing is replaced while it says what is wrong (A22).
    pub(crate) fn replace_found(
        &mut self,
        spec: &FindSpec,
        replacement: &str,
        all: bool,
    ) -> (Outcome, u32) {
        let mut spec = spec.clone();
        spec.one_text = true;
        self.set_find(Some(&spec));
        let Some(query) = self.found.query.clone() else {
            return (Outcome::default(), 0);
        };
        let Ok(with) = query.replacement(replacement) else {
            return (Outcome::default(), 0);
        };
        let text = self.editor.text();
        let hits = self.hit_bytes(&text);
        let mut outcome = Outcome::default();
        let replaced = if all {
            let (Some(first), Some(last)) = (hits.first(), hits.last()) else {
                return (outcome, 0);
            };
            let span = first.start..last.end;
            let places: Vec<(Range<usize>, String)> = hits
                .iter()
                .map(|hit| {
                    let new = query.replaced(&text, hit, &with);
                    (hit.start - span.start..hit.end - span.start, new)
                })
                .collect();
            let new = replace_places(&text[span.clone()], &places);
            let end = span.start + new.len();
            self.editor.seal();
            outcome = self.editor.replace_bytes(span, &new, end..end);
            self.editor.seal();
            hits.len()
        } else {
            let selection = self.editor.selection_bytes();
            match hits.iter().find(|hit| **hit == selection) {
                Some(hit) => {
                    let new = query.replaced(&text, hit, &with);
                    let end = hit.start + new.len();
                    self.editor.seal();
                    outcome = self.editor.replace_bytes(hit.clone(), &new, end..end);
                    self.editor.seal();
                    if self.select_hit(FindStep::First).is_some() {
                        outcome.moved = true;
                    }
                    1
                }
                None => {
                    outcome.moved = self.select_hit(FindStep::First).is_some();
                    0
                }
            }
        };
        // The marks of the replaced text go at once, not after the pause
        // of typing (app-tester: a replaced place stayed marked).
        if replaced > 0 {
            self.found.at = None;
        }
        (outcome, units(replaced))
    }
}

impl EditorTab {
    /// UC-005 A20, A21, FR-073: replacing in the find field while the
    /// source view shows the note (`replace_found`): the patch of
    /// the source view, how many were replaced, and what the field says.
    pub fn source_replace(
        &mut self,
        spec: &FindSpec,
        replacement: &str,
        all: bool,
    ) -> Replaced<EditorPatch> {
        let base = self.editor.revision();
        let (outcome, replaced) = self.replace_found(spec, replacement, all);
        let patch = self.after_input(base, &outcome);
        Replaced {
            patch,
            replaced,
            status: self.replace_status(replacement),
        }
    }

    /// UC-005 A20, A21, BR-017: replacing in the find field while the
    /// facade shows the note: only in the text it shows (OQ-190); its
    /// blocks made anew with `views`.
    pub fn facade_replace<M: MakeViews<View = BlockView>>(
        &mut self,
        note: &str,
        views: &mut M,
        spec: &FindSpec,
        replacement: &str,
        all: bool,
    ) -> Replaced<FacadePatch> {
        let mut spec = spec.clone();
        spec.shown_only = true;
        let (outcome, replaced) = self.replace_found(&spec, replacement, all);
        self.unfold_at_cursor();
        let mut patch = self.facade_refresh(note, views);
        patch.scroll = outcome.moved || outcome.edited();
        Replaced {
            patch,
            replaced,
            status: self.replace_status(replacement),
        }
    }
}

/// UC-005 BR-017, OQ-190: the bytes of `text` the facade shows as they
/// stand in the markup (`blocks` are its blocks), as sorted spans apart: a
/// link address, markup and an escaped character are in none.
fn shown_spans(blocks: &[Block], text: &str) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = Vec::new();
    for block in blocks {
        // Every shown character with its offset and its run.
        let chars: Vec<(char, usize, usize)> = block
            .runs
            .iter()
            .enumerate()
            .flat_map(|(r, run)| {
                run.text
                    .chars()
                    .zip(run.offsets.iter().copied())
                    .map(move |(c, at)| (c, at, r))
            })
            .collect();
        let mut verbatim: Vec<bool> = chars
            .iter()
            .map(|&(c, at, _)| text.get(at..).is_some_and(|rest| rest.starts_with(c)))
            .collect();
        // Review (test-monkey): markup between two characters of the same
        // run or style is an escape or an entity (`\*`, `&amp;`), which
        // writes the character next to it: neither of the two is shown as
        // it stands. Markup that changes the style (`**`, a link) is not.
        for k in 1..chars.len() {
            let (before, at, run) = chars[k - 1];
            let (_, next, next_run) = chars[k];
            let same = run == next_run || block.runs[run].style == block.runs[next_run].style;
            if same && at + before.len_utf8() != next {
                verbatim[k - 1] = false;
                verbatim[k] = false;
            }
        }
        for (&(c, at, _), verbatim) in chars.iter().zip(&verbatim) {
            if !verbatim {
                continue;
            }
            let end = at + c.len_utf8();
            match spans.last_mut() {
                Some(last) if last.end == at => last.end = end,
                _ => spans.push(at..end),
            }
        }
    }
    spans.sort_by_key(|span| span.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

/// Whether `range` lies within one of the sorted `spans`.
fn within(spans: &[Range<usize>], range: &Range<usize>) -> bool {
    let at = spans.partition_point(|span| span.start <= range.start);
    at > 0 && range.end <= spans[at - 1].end
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
                count: 3,
                problem: None,
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
            FindStatus {
                at: None,
                count: 3,
                problem: None
            },
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
        assert_eq!(
            tab.find_status(),
            FindStatus {
                at: None,
                count: 0,
                problem: None
            }
        );
    }

    fn spec(text: &str) -> FindSpec {
        FindSpec {
            text: text.to_owned(),
            one_text: true,
            ..FindSpec::default()
        }
    }

    /// TC-005-29, UC-005 A20, FR-073: Enter replaces the selected one and
    /// selects the next; the selection elsewhere only moves to the next.
    #[test]
    fn uc005_tc29_replace_one_by_one() {
        let mut tab = tab("ferry one ferry two ferry");
        let first = tab.find_by(&spec("ferry"), FindStep::First).unwrap();
        assert_eq!((first.anchor, first.head, first.status.count), (0, 5, 3));

        let done = tab.source_replace(&spec("ferry"), "boat", false);
        assert_eq!(done.replaced, 1);
        assert_eq!(tab.editor.text(), "boat one ferry two ferry");
        assert_eq!(tab.editor.selection(), 9..14, "the second is selected");
        assert_eq!((done.status.at, done.status.count), (Some(1), 2));
        assert_eq!(done.patch.changes.len(), 1);

        tab.editor.set_selection(0, 0);
        let moved = tab.source_replace(&spec("ferry"), "boat", false);
        assert_eq!(moved.replaced, 0, "the selection was none: it only moves");
        assert_eq!(tab.editor.selection(), 9..14);
    }

    /// TC-005-30, UC-005 A21, BR-004: replace all is one change that one
    /// undo takes back.
    #[test]
    fn uc005_tc30_replace_all_undone_at_once() {
        let mut tab = tab("ferry one ferry two ferry");
        let done = tab.source_replace(&spec("FERRY"), "boat", true);
        assert_eq!(done.replaced, 3);
        assert_eq!(tab.editor.text(), "boat one boat two boat");
        assert_eq!(done.status.count, 0);
        tab.editor.undo();
        assert_eq!(tab.editor.text(), "ferry one ferry two ferry");
    }

    /// TC-005-31, UC-005 A22: nothing to replace, an expression or a
    /// replacement that is not valid.
    #[test]
    fn uc005_tc31_nothing_to_replace() {
        let mut tab = tab("ferry");
        let none = tab.source_replace(&spec("zebra"), "x", true);
        assert_eq!((none.replaced, none.status.count), (0, 0));
        let mut regex = spec("[a-");
        regex.switches.regex = true;
        let wrong = tab.source_replace(&regex, "x", true);
        assert_eq!(wrong.replaced, 0);
        assert!(wrong
            .status
            .problem
            .unwrap()
            .starts_with("Not a valid regular expression"));
        let mut groups = spec("(f)");
        groups.switches.regex = true;
        let no_group = tab.source_replace(&groups, "$2", true);
        assert_eq!(no_group.replaced, 0);
        assert_eq!(
            no_group.status.problem.as_deref(),
            Some("The replacement names $2, but the expression has no group 2.")
        );
        assert_eq!(tab.editor.text(), "ferry");
    }

    /// TC-005-32, UC-005 BR-017, OQ-190: the facade replaces only the text
    /// it shows; the source view the markup; groups in both.
    #[test]
    fn uc005_tc32_replace_in_the_facade_and_in_the_source_view() {
        let text = "See [the plan](plan.md) and the Plan.";
        let mut facade = tab(text);
        let mut views = crate::CachedViews::default();
        let done = facade.facade_replace("Links", &mut views, &spec("plan"), "roadmap", true);
        assert_eq!(done.replaced, 2);
        assert_eq!(
            facade.editor.text(),
            "See [the roadmap](plan.md) and the roadmap."
        );

        let mut source = tab(text);
        source.source_replace(&spec("plan"), "roadmap", true);
        assert_eq!(
            source.editor.text(),
            "See [the roadmap](roadmap.md) and the roadmap."
        );

        let mut regex = tab(text);
        let mut groups = spec(r"(\w+)\.md");
        groups.switches.regex = true;
        regex.source_replace(&groups, "$1.adoc", true);
        assert_eq!(
            regex.editor.text(),
            "See [the plan](plan.adoc) and the Plan."
        );
    }

    /// TC-008-19, UC-008 A7, BR-011: the find field's switches count its
    /// hits; an expression that is not valid is its problem (A9).
    #[test]
    fn uc008_tc19_switches_of_the_find_field() {
        let mut tab = tab("Plan plan planning");
        let mut find = FindSpec {
            text: "plan".to_owned(),
            ..FindSpec::default()
        };
        let count = |tab: &mut EditorTab, find: &FindSpec| {
            tab.find_by(find, FindStep::First)
                .map_or(0, |answer| answer.status.count)
        };
        assert_eq!(count(&mut tab, &find), 3);
        find.switches.match_case = true;
        assert_eq!(count(&mut tab, &find), 2);
        find.switches.whole_word = true;
        assert_eq!(count(&mut tab, &find), 1);
        find.switches.regex = true;
        find.text = "pl.n".to_owned();
        assert_eq!(count(&mut tab, &find), 1);
        find.text = "(pl".to_owned();
        assert!(tab.find_by(&find, FindStep::First).is_none());
        assert!(tab.find_status().problem.is_some());
        tab.set_find(None);
        assert_eq!(tab.find_status().problem, None);
    }

    // ---- Test-monkey: package 1 (FR-073, FR-074, UC-005 A20 to A22) ----

    /// UC-005 A21, BR-017: places that touch are all replaced.
    #[test]
    fn uc005_a21_touching_places_are_all_replaced() {
        let mut all = tab("xxxx and abab");
        assert_eq!(all.source_replace(&spec("x"), "y", true).replaced, 4);
        assert_eq!(all.editor.text(), "yyyy and abab");
        let mut pairs = tab("xxxx and abab");
        pairs.source_replace(&spec("ab"), "-", true);
        assert_eq!(pairs.editor.text(), "xxxx and --");
        let mut facade = tab("xxxx");
        let mut views = crate::CachedViews::default();
        let done = facade.facade_replace("A", &mut views, &spec("x"), "y", true);
        assert_eq!((done.replaced, facade.editor.text().as_str()), (4, "yyyy"));
    }

    /// UC-005 A20: one by one replaces a place that touches the next, and
    /// the count says how many are left.
    #[test]
    fn uc005_a20_replace_one_of_touching_places() {
        let mut tab = tab("aaa");
        let first = tab.find_by(&spec("a"), FindStep::First).unwrap();
        assert_eq!((first.anchor, first.head, first.status.count), (0, 1, 3));
        let done = tab.source_replace(&spec("a"), "b", false);
        assert_eq!(done.replaced, 1);
        assert_eq!(tab.editor.text(), "baa");
        assert_eq!(done.status.count, 2);
    }

    /// UC-005 BR-017: `i` inside `İ` is no place; nothing is inserted.
    #[test]
    fn uc005_br017_no_replacing_inside_a_character() {
        let mut tab = tab("İstanbul");
        let done = tab.source_replace(&spec("i"), "X", true);
        assert_eq!(tab.editor.text(), "İstanbul");
        assert_eq!(done.replaced, 0);
        let one = tab.find_by(&spec("i"), FindStep::First);
        assert!(one.is_none_or(|a| a.anchor != a.head));
    }

    /// UC-005 A21, FR-073: CRLF, a lone CR, emoji and combining marks: the
    /// bytes outside the places stay, one undo takes it all back.
    #[test]
    fn uc005_a21_odd_text_keeps_what_is_not_replaced() {
        for text in [
            "ferry\r\nferry\r\n",
            "ferry\rferry\r",
            "😀 ferry e\u{301} ferry 😀",
            "\u{feff}ferry",
            "ferry",
        ] {
            let mut tab = tab(text);
            let done = tab.source_replace(&spec("ferry"), "boat", true);
            assert!(done.replaced > 0, "{text:?}");
            assert_eq!(tab.editor.text(), text.replace("ferry", "boat"), "{text:?}");
            tab.editor.undo();
            assert_eq!(tab.editor.text(), text, "{text:?}");
        }
    }

    /// UC-005 A21, FR-074: a regular expression with groups, over a line
    /// end, with an anchor and a zero-width alternative.
    #[test]
    fn uc005_a21_regular_expressions_in_the_note() {
        let regex = |text: &str| {
            let mut spec = spec(text);
            spec.switches.regex = true;
            spec
        };
        let mut tab1 = tab("one\ntwo\nthree");
        tab1.source_replace(&regex(r"one\ntwo"), "1-2", true);
        assert_eq!(tab1.editor.text(), "1-2\nthree");
        let mut tab2 = tab("ab\nab\n");
        tab2.source_replace(&regex("^ab$"), "X", true);
        assert_eq!(tab2.editor.text(), "X\nX\n");
        let mut tab3 = tab("ab ab");
        let done = tab3.source_replace(&regex("(|b)"), "-", true);
        assert_eq!((done.replaced, tab3.editor.text().as_str()), (0, "ab ab"));
        let mut tab4 = tab("(f)(e)rry");
        tab4.source_replace(&regex(r"\((\w)\)\((\w)\)"), "$2$1$$", true);
        assert_eq!(tab4.editor.text(), "ef$rry");
    }

    /// UC-005 BR-017: in the facade, what the user sees is replaced, not a
    /// piece of the markup that wrote it: an escape and an entity.
    #[test]
    fn uc005_br017_the_facade_replaces_shown_text_with_its_escape() {
        let mut views = crate::CachedViews::default();
        let mut escape = tab("a\\*b and a\\*b");
        let done = escape.facade_replace("A", &mut views, &spec("*b"), "X", true);
        assert!(
            !escape.editor.text().contains('\\') || done.replaced == 0,
            "a backslash is left in front of the replacement: {:?}",
            escape.editor.text()
        );
    }

    /// UC-005 BR-017: in the facade, an entity is replaced whole.
    #[test]
    fn uc005_br017_the_facade_replaces_shown_text_with_its_entity() {
        let mut views = crate::CachedViews::default();
        let mut entity = tab("fish &amp; chips");
        let done = entity.facade_replace("A", &mut views, &spec("&"), "and", true);
        assert!(
            !entity.editor.text().contains("andamp;") || done.replaced == 0,
            "half an entity is left: {:?}",
            entity.editor.text()
        );
    }

    /// UC-005 A22: a note with nothing, an empty note, an empty search.
    #[test]
    fn uc005_a22_empty_notes_and_texts() {
        let mut empty = tab("");
        let done = empty.source_replace(&spec("x"), "y", true);
        assert_eq!((done.replaced, empty.editor.text().as_str()), (0, ""));
        let mut note = tab("abc");
        assert_eq!(note.source_replace(&spec(""), "y", true).replaced, 0);
        let mut regex = spec("");
        regex.switches.regex = true;
        assert_eq!(note.source_replace(&regex, "y", true).replaced, 0);
        assert_eq!(note.editor.text(), "abc");
        let huge = "z".repeat(1_000_000);
        let done = note.source_replace(&spec("b"), &huge, true);
        assert_eq!(done.replaced, 1);
        assert_eq!(note.editor.text().len(), 1_000_002);
        note.editor.undo();
        assert_eq!(note.editor.text(), "abc");
    }

    /// UC-005 A21, seeded: random notes and search texts; replacing all by
    /// the text itself changes nothing, replacing by a marker leaves none
    /// of the search text and one undo gives everything back.
    #[test]
    fn uc005_a21_random_notes_replace_and_undo() {
        const PARTS: &[&str] = &[
            "a", "aa", "ab", "İ", "i", "ß", "é", "\n", "\r\n", " ", "😀", "e\u{301}", "x",
        ];
        let mut seed: u64 = 0x0A21_5EED;
        eprintln!("uc005 replace fuzz seed {seed:#x}");
        let next = |seed: &mut u64, most: usize| {
            *seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (*seed >> 33) as usize % most
        };
        for round in 0..400 {
            let start = seed;
            let mut text = String::new();
            for _ in 0..1 + round % 30 {
                text.push_str(PARTS[next(&mut seed, PARTS.len())]);
            }
            let word = PARTS[next(&mut seed, PARTS.len())];
            if word.trim().is_empty() {
                continue;
            }
            let mut note = tab(&text);
            let context = format!("seed {start:#x} text {text:?} word {word:?}");
            note.source_replace(&spec(word), word, true);
            assert_eq!(note.editor.text(), text, "identity: {context}");
            let mut note = tab(&text);
            note.source_replace(&spec(word), "\u{1}", true);
            let left = note.editor.text();
            assert_eq!(
                left.replace('\u{1}', word).to_lowercase(),
                text.to_lowercase(),
                "{context}"
            );
            note.editor.undo();
            assert_eq!(note.editor.text(), text, "undo: {context}");
        }
    }
}
