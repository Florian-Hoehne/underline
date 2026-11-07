//! UC-008 Find Note: what a search text matches (BR-002), the places found
//! in a note (BR-004), and the search over the whole workspace, which runs in
//! the background so typing is never held up (BR-005).

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::note::NoteId;
use crate::workspace::NoteEntry;

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

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Whether every word occurs somewhere in `texts` together.
    pub fn matches_all(&self, texts: &[&str]) -> bool {
        let lowered: Vec<String> = texts.iter().map(|t| lower(t)).collect();
        let lowered: Vec<&str> = lowered.iter().map(String::as_str).collect();
        self.matches_lowered(&lowered)
    }

    /// [`Self::matches_all`] for texts already in lower case.
    fn matches_lowered(&self, lowered: &[&str]) -> bool {
        self.words
            .iter()
            .all(|word| lowered.iter().any(|text| text.contains(word.as_str())))
    }

    /// Every occurrence of a word in `text`, as byte ranges of `text`,
    /// sorted and not overlapping (UC-008 BR-008, A6).
    pub fn occurrences(&self, text: &str) -> Vec<Range<usize>> {
        let (lowered, map) = lower_with_map(text);
        let mut found: Vec<Range<usize>> = Vec::new();
        for word in self.words.iter().filter(|w| !w.is_empty()) {
            let mut from = 0;
            while let Some(at) = lowered[from..].find(word.as_str()) {
                let start = from + at;
                let end = start + word.len();
                found.push(map[start]..map[end]);
                from = end;
            }
        }
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

/// `text` in lower case, the way the search compares (BR-002).
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

/// A note that matches (UC-008 step 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundNote {
    pub id: NoteId,
    /// The title or the folder holds every word (BR-003).
    pub by_name: bool,
    /// How many lines hold a word, and the first of them (A5).
    pub places: usize,
    pub lines: Vec<FoundLine>,
}

/// The lines of `text` that hold a word, the first [`SHOWN_PLACES`] of them
/// shown, and how many there are.
pub fn found_lines(query: &Query, text: &str) -> (usize, Vec<FoundLine>) {
    found_lines_lowered(query, text, &lower(text))
}

/// [`found_lines`] with `text` already in lower case as `lowered`. A line
/// break stays a line break in lower case, so the lines of both match.
fn found_lines_lowered(query: &Query, text: &str, lowered: &str) -> (usize, Vec<FoundLine>) {
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

/// One search over the workspace.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub text: String,
    /// The notes in the order of the note overview (BR-003).
    pub notes: Vec<NoteEntry>,
    /// The texts of the notes open in tabs, with changes not yet saved
    /// (BR-001).
    pub open: HashMap<NoteId, String>,
}

/// The text of a note and the same in lower case.
pub type Texts = (Arc<str>, Arc<str>);

/// The answer to a [`SearchRequest`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub text: String,
    pub found: Vec<FoundNote>,
    /// Notes that could not be read (A4).
    pub unreadable: usize,
}

/// Searches the notes of the workspace at `root` (UC-008 step 2): the
/// title, folder, and text of every note, title and folder matches first.
/// `read` gives the text of a note and the same in lower case; it returns
/// `None` when the note cannot be read. Stops early, returning `None`, when
/// `cancelled` says so.
pub fn search_notes(
    request: &SearchRequest,
    mut read: impl FnMut(&NoteEntry) -> Option<Texts>,
    cancelled: impl Fn() -> bool,
) -> Option<SearchResult> {
    let query = Query::parse(&request.text);
    let mut result = SearchResult {
        text: request.text.clone(),
        ..SearchResult::default()
    };
    if query.is_empty() {
        return Some(result);
    }
    let mut by_name = Vec::new();
    let mut by_text = Vec::new();
    for entry in &request.notes {
        if cancelled() {
            return None;
        }
        let title = entry.id.title();
        let folder = entry.id.folder().unwrap_or_default();
        let (text, lowered) = match request.open.get(&entry.id) {
            Some(text) => (Arc::from(text.as_str()), Arc::from(lower(text))),
            None => match read(entry) {
                Some(texts) => texts,
                None => {
                    result.unreadable += 1;
                    continue;
                }
            },
        };
        if !query.matches_lowered(&[&lower(&title), &lower(folder), &lowered]) {
            continue;
        }
        let (places, lines) = found_lines_lowered(&query, &text, &lowered);
        let name = query.matches_all(&[&title, folder]);
        let found = FoundNote {
            id: entry.id.clone(),
            by_name: name,
            places,
            lines,
        };
        if name {
            by_name.push(found);
        } else {
            by_text.push(found);
        }
    }
    by_name.extend(by_text);
    result.found = by_name;
    Some(result)
}

/// Searches in the background (UC-008 BR-005): every new request replaces
/// the one running, and the texts of the notes are kept between searches
/// and read again only when their files changed.
pub struct Searcher {
    requests: Sender<SearchRequest>,
    results: Receiver<SearchResult>,
    latest: Arc<AtomicU64>,
}

type Cache = HashMap<PathBuf, (Option<SystemTime>, u64, Texts)>;

impl Searcher {
    /// Starts the searcher for the workspace at `root`; `wake` is called when
    /// a result is ready.
    pub fn start(root: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, inbox) = channel::<SearchRequest>();
        let (outbox, results) = channel();
        let latest = Arc::new(AtomicU64::new(0));
        let seen = Arc::clone(&latest);
        let cache = Arc::new(Mutex::new(Cache::new()));
        std::thread::Builder::new()
            .name("underline-search".into())
            .spawn(move || {
                let mut generation = 0;
                while let Ok(mut request) = inbox.recv() {
                    // Only the newest waiting request counts.
                    generation += 1;
                    while let Ok(newer) = inbox.try_recv() {
                        request = newer;
                        generation += 1;
                    }
                    let mine = generation;
                    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
                    let result = search_notes(
                        &request,
                        |entry| read_cached(&mut cache, &root, entry),
                        || seen.load(Ordering::Relaxed) > mine,
                    );
                    if let Some(result) = result {
                        if outbox.send(result).is_err() {
                            return;
                        }
                        wake();
                    }
                }
            })
            .expect("the search thread starts");
        Self {
            requests,
            results,
            latest,
        }
    }

    /// Starts a search; a running one is given up.
    pub fn search(&self, request: SearchRequest) {
        self.latest.fetch_add(1, Ordering::Relaxed);
        let _ = self.requests.send(request);
    }

    /// The newest result that arrived, if any.
    pub fn poll(&self) -> Option<SearchResult> {
        self.results.try_iter().last()
    }
}

/// The text of a note, from the cache while its file is unchanged.
fn read_cached(cache: &mut Cache, root: &Path, entry: &NoteEntry) -> Option<Texts> {
    let path = root.join(entry.id.file_name(entry.format));
    let meta = fs::metadata(&path).ok()?;
    let stamp = (meta.modified().ok(), meta.len());
    if let Some((modified, len, texts)) = cache.get(&path) {
        if (*modified, *len) == stamp {
            return Some(texts.clone());
        }
    }
    let text = fs::read_to_string(&path).ok()?;
    let lowered = lower(&text);
    let texts: Texts = (Arc::from(text), Arc::from(lowered));
    cache.insert(path, (stamp.0, stamp.1, texts.clone()));
    Some(texts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::NoteFormat;

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

    #[test]
    fn occurrences_are_byte_ranges_of_the_text() {
        let text = "Ünïcode FERRY and ferry, Ferryman";
        let found = q("ferry").occurrences(text);
        let words: Vec<&str> = found.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(words, ["FERRY", "ferry", "Ferry"]);
        let overlapping = q("ab bc").occurrences("abc");
        assert_eq!(overlapping, [0..3]);
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

    fn entry(id: &str) -> NoteEntry {
        NoteEntry {
            id: NoteId::from_relative(id),
            format: NoteFormat::Markdown,
        }
    }

    /// TC-008-05 / TC-008-06 / TC-008-10: names first, open texts, unreadable
    /// notes counted.
    #[test]
    fn uc008_tc05_tc06_tc10_search_over_notes() {
        let texts: HashMap<&str, &str> = [
            ("Budget", "travel costs"),
            ("travel/Packing", "socks"),
            ("Travel-Log", "travel diary"),
            ("Draft", "old text"),
        ]
        .into_iter()
        .collect();
        let request = SearchRequest {
            text: "travel".into(),
            notes: ["Budget", "travel/Packing", "Secret", "Travel-Log", "Draft"]
                .into_iter()
                .map(entry)
                .collect(),
            open: [(NoteId::from_relative("Draft"), "mango travel".to_owned())]
                .into_iter()
                .collect(),
        };
        let result = search_notes(
            &request,
            |e| {
                texts
                    .get(e.id.as_str())
                    .map(|t| (Arc::from(*t), Arc::from(lower(t))))
            },
            || false,
        )
        .unwrap();
        let ids: Vec<&str> = result.found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["travel/Packing", "Travel-Log", "Budget", "Draft"]);
        assert_eq!(result.unreadable, 1);
        assert_eq!(result.found[0].places, 0);
        assert!(search_notes(&request, |_| None, || true).is_none());
    }

    #[test]
    fn the_searcher_answers_in_the_background() {
        let root = std::env::temp_dir().join(format!("underline-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Trip.md"), "Book the ferry to Hydra\n").unwrap();
        let searcher = Searcher::start(root.clone(), || {});
        searcher.search(SearchRequest {
            text: "ferry".into(),
            notes: vec![entry("Trip")],
            open: HashMap::new(),
        });
        let start = std::time::Instant::now();
        let result = loop {
            if let Some(result) = searcher.poll() {
                break result;
            }
            assert!(start.elapsed().as_secs() < 5);
            std::thread::yield_now();
        };
        assert_eq!(result.found.len(), 1);
        assert_eq!(result.found[0].lines[0].shown, "Book the ferry to Hydra");
        fs::remove_dir_all(&root).unwrap();
    }

    /// TC-008-15: 10,000 notes of about 25 KB, searched with a warm cache.
    /// Run with `cargo test -p underline-core --release -- --ignored`.
    #[test]
    #[ignore]
    fn uc008_tc15_speed_in_a_large_workspace() {
        let root =
            std::env::temp_dir().join(format!("underline-search-big-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let words = [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ];
        let mut notes = Vec::new();
        for n in 0usize..10_000 {
            let mut text = String::new();
            let mut k = n;
            while text.len() < 25_000 {
                k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                text.push_str(words[k % words.len()]);
                text.push_str(if k % 11 == 0 { "\n" } else { " " });
            }
            text.push_str(&format!("unique{n}\n"));
            fs::write(root.join(format!("Note-{n}.md")), text).unwrap();
            notes.push(entry(&format!("Note-{n}")));
        }
        let mut cache = Cache::new();
        let request = |text: &str| SearchRequest {
            text: text.into(),
            notes: notes.clone(),
            open: HashMap::new(),
        };
        // The first search reads the files.
        search_notes(
            &request("x"),
            |e| read_cached(&mut cache, &root, e),
            || false,
        );
        let mut slow = 0;
        for text in [
            "unique9999",
            "zulu",
            "alpha bravo",
            "\"golf hotel\"",
            "unique1 unique2",
        ] {
            let start = std::time::Instant::now();
            search_notes(
                &request(text),
                |e| read_cached(&mut cache, &root, e),
                || false,
            )
            .unwrap();
            let took = start.elapsed();
            eprintln!("{text}: {took:?}");
            slow += usize::from(took.as_secs_f32() > 1.0);
        }
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(slow, 0);
    }
}
