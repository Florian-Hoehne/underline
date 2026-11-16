//! UC-008 Find Note: the search over the whole workspace, which runs in the
//! background so typing is never held up (BR-005). The same thread finds
//! the places a replacement in the workspace changes (UC-022 step 2,
//! BR-007), with the texts it keeps. What a search text
//! matches (BR-002) and the places found in a note (BR-004) are in
//! `underline-markup` (re-exported here), so the window's editor core marks
//! the words with the same rules.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::note::NoteId;
use crate::workspace::NoteEntry;

pub use underline_markup::search::{
    found_lines, lower, note_places, replace_places, FoundLine, NotePlace, PlaceKey, Query,
    QueryError, Reading, Replacement, ShownPlace, Switches, LINE_CONTEXT, SHOWN_PLACES,
};
use underline_markup::search::{found_lines_at, found_lines_lowered, places_at, shown_place};
use underline_markup::tags::{note_tags, tag_matches, tag_words, TagPlace};

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

/// One search over the workspace.
#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub text: String,
    /// The search switches (FR-075, UC-008 BR-011).
    pub switches: Switches,
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
    /// The switches it was searched with.
    pub switches: Switches,
    pub found: Vec<FoundNote>,
    /// Notes that could not be read (A4).
    pub unreadable: usize,
}

/// The tags of a note and where it carries them (UC-004 BR-016), asked
/// with the note and its text.
pub type NoteTags = Arc<[TagPlace]>;

/// The tags of the note `entry` whose text is `text`, found anew.
#[must_use]
pub fn tags_of(entry: &NoteEntry, text: &str) -> NoteTags {
    Arc::from(note_tags(text, entry.format))
}

/// Searches the notes of the workspace at `root` (UC-008 step 2): the
/// title, folder, and text of every note, title and folder matches first.
/// `read` gives the text of a note and the same in lower case; it returns
/// `None` when the note cannot be read. Stops early, returning `None`, when
/// `cancelled` says so. A word `#tag` finds the notes that carry the tag
/// (A8, BR-012), their tags found anew ([`search_notes_with`] keeps them).
pub fn search_notes(
    request: &SearchRequest,
    read: impl FnMut(&NoteEntry) -> Option<Texts>,
    cancelled: impl Fn() -> bool,
) -> Option<SearchResult> {
    search_notes_with(request, read, |entry, text| tags_of(entry, text), cancelled)
}

/// [`search_notes`] with `tags` giving the tags of a note from its text:
/// the searcher keeps them between searches, so a search by tag parses a
/// note only when its text changed (NFR-003). It is asked only when the
/// search text holds a word `#tag` (UC-008 BR-012).
pub fn search_notes_with(
    request: &SearchRequest,
    mut read: impl FnMut(&NoteEntry) -> Option<Texts>,
    mut tags: impl FnMut(&NoteEntry, &Arc<str>) -> NoteTags,
    cancelled: impl Fn() -> bool,
) -> Option<SearchResult> {
    let mut result = SearchResult {
        text: request.text.clone(),
        switches: request.switches,
        ..SearchResult::default()
    };
    // BR-012: words `#tag` search for a tag, unless the text is a regular
    // expression, where `#` has no meaning of its own.
    let (wanted, rest) = if request.switches.regex {
        (Vec::new(), request.text.clone())
    } else {
        tag_words(&request.text)
    };
    // UC-008 A9: an expression that is not valid finds nothing; the
    // session says why before it asks.
    let Ok(query) = Query::new(&rest, request.switches, Reading::Words) else {
        return Some(result);
    };
    if query.is_empty() && wanted.is_empty() {
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
        if !query.is_empty()
            && !query.matches_texts(
                &[&title, folder, &text],
                &[&lower(&title), &lower(folder), &lowered],
            )
        {
            continue;
        }
        if wanted.is_empty() {
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
            continue;
        }
        let carried = tags(entry, &text);
        let mut places: Vec<std::ops::Range<usize>> = Vec::new();
        let mut every = true;
        for want in &wanted {
            let before = places.len();
            places.extend(
                carried
                    .iter()
                    .filter(|place| tag_matches(&place.key, want))
                    .map(|place| place.range.clone()),
            );
            every &= places.len() > before;
        }
        if !every {
            continue;
        }
        // The places found are the lines that hold the tag, and those
        // holding the other words.
        if !query.is_empty() {
            places.extend(query.occurrences(&text));
        }
        places.sort_by_key(|range| (range.start, std::cmp::Reverse(range.end)));
        places.dedup();
        let (count, lines) = found_lines_at(&text, places);
        by_text.push(FoundNote {
            id: entry.id.clone(),
            by_name: false,
            places: count,
            lines,
        });
    }
    by_name.extend(by_text);
    result.found = by_name;
    Some(result)
}

/// UC-022 A7: how many places the replace of the workspace shows; the
/// others are counted and replaced all the same.
pub const SHOWN_REPLACE_PLACES: usize = 10_000;

/// UC-022 step 2: the places a replacement in the workspace changes.
#[derive(Debug, Clone)]
pub struct PlacesRequest {
    /// The number the session gave this request; the result carries it.
    pub asked: u32,
    /// The search text, read as one text (BR-002).
    pub text: String,
    /// The replacement as typed (BR-003).
    pub replacement: String,
    pub switches: Switches,
    /// The notes in the order of the note overview.
    pub notes: Vec<NoteEntry>,
    /// The texts of the notes open in tabs, with changes not yet saved
    /// (BR-001).
    pub open: HashMap<NoteId, String>,
}

/// The places of one note (UC-022 step 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePlaces {
    pub id: NoteId,
    /// The text searched, to tell whether the note changed since.
    pub text: Arc<str>,
    /// How many places it holds.
    pub count: usize,
    /// The places shown, in the order of the text (A7: no more than
    /// [`SHOWN_REPLACE_PLACES`] over all notes), and how the list shows
    /// each; the others are only counted.
    pub places: Vec<NotePlace>,
    pub shown: Vec<ShownPlace>,
}

/// The answer to a [`PlacesRequest`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacesResult {
    /// The number of the request.
    pub asked: u32,
    /// The notes with places, in the order of the overview.
    pub notes: Vec<NotePlaces>,
    /// How many places there are in all.
    pub places: usize,
    /// How many of them are shown (A7).
    pub shown: usize,
    /// Notes that could not be read.
    pub unreadable: usize,
}

/// UC-022 step 2, BR-001, BR-002, A7: the places of `request` in the text
/// of every note (`read` as in [`search_notes`]); only the text, never the
/// title or the folder. `None` when `cancelled` says so.
pub fn find_places(
    request: &PlacesRequest,
    mut read: impl FnMut(&NoteEntry) -> Option<Texts>,
    cancelled: impl Fn() -> bool,
) -> Option<PlacesResult> {
    let mut result = PlacesResult {
        asked: request.asked,
        ..PlacesResult::default()
    };
    let Ok(query) = Query::new(&request.text, request.switches, Reading::OneText) else {
        return Some(result);
    };
    let Ok(with) = query.replacement(&request.replacement) else {
        return Some(result);
    };
    if query.is_empty() {
        return Some(result);
    }
    for entry in &request.notes {
        if cancelled() {
            return None;
        }
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
        if !query.matches_texts(&[&text], &[&lowered]) {
            continue;
        }
        let ranges = query.occurrences(&text);
        if ranges.is_empty() {
            continue;
        }
        // Only the places shown are worked out whole (BR-007).
        let room = SHOWN_REPLACE_PLACES.saturating_sub(result.shown);
        let places = places_at(&query, &with, &text, &ranges[..room.min(ranges.len())]);
        let shown: Vec<ShownPlace> = places
            .iter()
            .map(|place| shown_place(&text, place))
            .collect();
        result.shown += shown.len();
        result.places += ranges.len();
        result.notes.push(NotePlaces {
            id: entry.id.clone(),
            text,
            count: ranges.len(),
            places,
            shown,
        });
    }
    Some(result)
}

/// A job of the search thread.
enum Job {
    Search(SearchRequest),
    Places(PlacesRequest),
    /// Reads the texts and the tags of these notes ahead (NFR-003).
    Warm(Vec<NoteEntry>),
}

/// Searches in the background (UC-008 BR-005): every new request replaces
/// the one of its kind running, and the texts of the notes are kept between
/// searches and read again only when their files changed. The searches of
/// the overlay and the places of a replacement (UC-022) are two kinds.
pub struct Searcher {
    requests: Sender<(u64, Job)>,
    results: Receiver<SearchResult>,
    places: Receiver<PlacesResult>,
    /// The number of the newest request of each kind: search, places,
    /// warm-up.
    latest: Arc<[AtomicU64; 3]>,
}

type Cache = HashMap<PathBuf, (Option<SystemTime>, u64, Texts)>;

/// What tags were found for: a file as it was on disk (its time and size),
/// or the text of an open tab by its hash. Neither keeps the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKey {
    File(Option<SystemTime>, u64),
    Text(u64, usize),
}

/// The tags of the notes, by what they were found for (NFR-003): a note
/// whose file or text did not change is not parsed again.
type TagCache = HashMap<NoteId, (TagKey, NoteTags)>;

/// The key of the text of an open tab.
fn text_key(text: &str) -> TagKey {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    TagKey::Text(hasher.finish(), text.len())
}

/// The tags of `entry` with the text `text`, found for `key`: from `cache`
/// while the key is the same, else found anew and kept.
fn tags_cached(cache: &mut TagCache, entry: &NoteEntry, key: TagKey, text: &str) -> NoteTags {
    if let Some((known, tags)) = cache.get(&entry.id) {
        if *known == key {
            return Arc::clone(tags);
        }
    }
    let tags = tags_of(entry, text);
    cache.insert(entry.id.clone(), (key, Arc::clone(&tags)));
    tags
}

/// The key of a note's file as `read_cached` last read it.
fn file_key(cache: &Cache, root: &Path, entry: &NoteEntry) -> Option<TagKey> {
    cache
        .get(&root.join(entry.id.file_name(entry.format)))
        .map(|(modified, len, _)| TagKey::File(*modified, *len))
}

/// FR-092, NFR-003: reads the tags of `notes` ahead, keeping only the tags;
/// stops as soon as `go_on` says no.
fn warm_tags(cache: &mut TagCache, root: &Path, notes: &[NoteEntry], go_on: impl Fn() -> bool) {
    let listed: std::collections::HashSet<&NoteId> = notes.iter().map(|entry| &entry.id).collect();
    cache.retain(|id, _| listed.contains(id));
    for entry in notes {
        if !go_on() {
            return;
        }
        let path = root.join(entry.id.file_name(entry.format));
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        let key = TagKey::File(meta.modified().ok(), meta.len());
        if cache.get(&entry.id).is_some_and(|(known, _)| *known == key) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            tags_cached(cache, entry, key, &text);
        }
    }
}

impl Searcher {
    /// Starts the searcher for the workspace at `root`; `wake` is called when
    /// a result is ready.
    pub fn start(root: PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, inbox) = channel::<(u64, Job)>();
        let (outbox, results) = channel();
        let (places_out, places) = channel();
        let latest = Arc::new([AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)]);
        let seen = Arc::clone(&latest);
        let cache = Arc::new(Mutex::new(Cache::new()));
        let mut tag_cache = TagCache::new();
        std::thread::Builder::new()
            .name("underline-search".into())
            .spawn(move || {
                while let Ok(first) = inbox.recv() {
                    // Only the newest waiting request of each kind counts.
                    let (mut search, mut find, mut warm) = (None, None, None);
                    for (n, job) in std::iter::once(first).chain(inbox.try_iter()) {
                        match job {
                            Job::Search(request) => search = Some((n, request)),
                            Job::Places(request) => find = Some((n, request)),
                            Job::Warm(notes) => warm = Some((n, notes)),
                        }
                    }
                    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some((mine, request)) = search {
                        let listed: std::collections::HashSet<&NoteId> =
                            request.notes.iter().map(|entry| &entry.id).collect();
                        tag_cache.retain(|id, _| listed.contains(id));
                        let texts = std::cell::RefCell::new(&mut *cache);
                        let result = search_notes_with(
                            &request,
                            |entry| read_cached(&mut texts.borrow_mut(), &root, entry),
                            |entry, text| {
                                let key = if request.open.contains_key(&entry.id) {
                                    None
                                } else {
                                    file_key(&texts.borrow(), &root, entry)
                                };
                                let key = key.unwrap_or_else(|| text_key(text));
                                tags_cached(&mut tag_cache, entry, key, text)
                            },
                            || seen[0].load(Ordering::Relaxed) > mine,
                        );
                        if let Some(result) = result {
                            if outbox.send(result).is_err() {
                                return;
                            }
                            wake();
                        }
                    }
                    if let Some((mine, request)) = find {
                        let result = find_places(
                            &request,
                            |entry| read_cached(&mut cache, &root, entry),
                            || seen[1].load(Ordering::Relaxed) > mine,
                        );
                        if let Some(result) = result {
                            if places_out.send(result).is_err() {
                                return;
                            }
                            wake();
                        }
                    }
                    if let Some((mine, notes)) = warm {
                        // FR-092, NFR-003: the tags of the notes are read
                        // ahead, so the first search by tag needs not parse
                        // every note. Any request, a newer warm-up, or the
                        // searcher let go of stops it.
                        let asked = [
                            seen[0].load(Ordering::Relaxed),
                            seen[1].load(Ordering::Relaxed),
                        ];
                        drop(cache);
                        warm_tags(&mut tag_cache, &root, &notes, || {
                            Arc::strong_count(&seen) > 1
                                && seen[0].load(Ordering::Relaxed) == asked[0]
                                && seen[1].load(Ordering::Relaxed) == asked[1]
                                && seen[2].load(Ordering::Relaxed) == mine
                        });
                    }
                }
            })
            .expect("the search thread starts");
        Self {
            requests,
            results,
            places,
            latest,
        }
    }

    /// Starts a search; a running one is given up.
    pub fn search(&self, request: SearchRequest) {
        let n = self.latest[0].fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.requests.send((n, Job::Search(request)));
    }

    /// FR-092, NFR-003: reads the texts and the tags of `notes` ahead, in
    /// the background; a search or a replacement asked meanwhile stops it.
    pub fn warm(&self, notes: Vec<NoteEntry>) {
        let n = self.latest[2].fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.requests.send((n, Job::Warm(notes)));
    }

    /// The newest result that arrived, if any.
    pub fn poll(&self) -> Option<SearchResult> {
        self.results.try_iter().last()
    }

    /// UC-022 step 2: starts finding the places of a replacement; a running
    /// one is given up.
    pub fn find_places(&self, request: PlacesRequest) {
        let n = self.latest[1].fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.requests.send((n, Job::Places(request)));
    }

    /// The newest places that arrived, if any.
    pub fn poll_places(&self) -> Option<PlacesResult> {
        self.places.try_iter().last()
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
            switches: Switches::default(),
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

    /// TC-008-21, UC-008 A8, BR-012: a word `#tag` finds the notes that
    /// carry the tag or one below it, in the text or the attribute `tags`,
    /// with the lines that hold it; `#42` is no tag; with the switch regular
    /// expression `#` means nothing of its own.
    #[test]
    fn tc008_21_search_by_tag() {
        let texts: HashMap<&str, (&str, NoteFormat)> = [
            ("Trip", ("Ferry\n#travel ferry\n", NoteFormat::Markdown)),
            (
                "Hydra",
                (
                    "---\ntags: [travel/greece]\n---\nIsland\n",
                    NoteFormat::Markdown,
                ),
            ),
            (
                "Guide",
                ("= Guide\n:tags: Travel\n\nText\n", NoteFormat::AsciiDoc),
            ),
            ("Price", ("costs #42\ntravel\n", NoteFormat::Markdown)),
            ("Travelling", ("#travelling\n", NoteFormat::Markdown)),
        ]
        .into_iter()
        .collect();
        let notes: Vec<NoteEntry> = ["Trip", "Hydra", "Guide", "Price", "Travelling"]
            .into_iter()
            .map(|id| NoteEntry {
                id: NoteId::from_relative(id),
                format: texts[id].1,
            })
            .collect();
        let search = |text: &str, switches: Switches| {
            let request = SearchRequest {
                text: text.into(),
                switches,
                notes: notes.clone(),
                open: HashMap::new(),
            };
            search_notes(
                &request,
                |e| {
                    texts
                        .get(e.id.as_str())
                        .map(|(t, _)| (Arc::from(*t), Arc::from(lower(t))))
                },
                || false,
            )
            .unwrap()
        };
        let ids = |result: &SearchResult| -> Vec<String> {
            result
                .found
                .iter()
                .map(|f| f.id.as_str().to_owned())
                .collect()
        };
        let found = search("#travel", Switches::default());
        assert_eq!(ids(&found), ["Trip", "Hydra", "Guide"]);
        assert_eq!(found.found[0].lines[0].shown, "#travel ferry");
        assert_eq!(found.found[0].lines[0].marks, vec![0..7_usize]);
        assert_eq!(found.found[1].lines[0].shown, "tags: [travel/greece]");
        assert_eq!(found.found[2].lines[0].shown, ":tags: Travel");
        assert_eq!(ids(&search("#TRAVEL ferry", Switches::default())), ["Trip"]);
        assert_eq!(
            ids(&search("#travel/greece", Switches::default())),
            ["Hydra"]
        );
        // `#42` is no tag: it is searched as text.
        assert_eq!(ids(&search("#42", Switches::default())), ["Price"]);
        let regex = Switches {
            regex: true,
            ..Switches::default()
        };
        assert_eq!(ids(&search("#travel", regex)), ["Trip", "Travelling"]);
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
            switches: Switches::default(),
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
        // FR-092: tags read ahead, then searched; a changed file is read again.
        fs::write(root.join("Tagged.md"), "#travel\n").unwrap();
        searcher.warm(vec![entry("Trip"), entry("Tagged")]);
        let tagged = |searcher: &Searcher| {
            searcher.search(SearchRequest {
                text: "#travel".into(),
                switches: Switches::default(),
                notes: vec![entry("Trip"), entry("Tagged")],
                open: HashMap::new(),
            });
            let start = std::time::Instant::now();
            loop {
                if let Some(result) = searcher.poll() {
                    break result;
                }
                assert!(start.elapsed().as_secs() < 5);
                std::thread::yield_now();
            }
        };
        assert_eq!(tagged(&searcher).found.len(), 1);
        fs::write(root.join("Trip.md"), "Book the ferry, #travel too\n").unwrap();
        assert_eq!(tagged(&searcher).found.len(), 2);
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
            switches: Switches::default(),
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

    fn places_request(text: &str, replacement: &str, notes: &[&str]) -> PlacesRequest {
        PlacesRequest {
            asked: 7,
            text: text.into(),
            replacement: replacement.into(),
            switches: Switches::default(),
            notes: notes.iter().map(|n| entry(n)).collect(),
            open: HashMap::new(),
        }
    }

    fn read_from<'a>(
        texts: &'a HashMap<&str, &str>,
    ) -> impl FnMut(&NoteEntry) -> Option<Texts> + 'a {
        |e| {
            texts
                .get(e.id.as_str())
                .map(|t| (Arc::from(*t), Arc::from(lower(t))))
        }
    }

    /// TC-022-01, TC-022-13, UC-022 step 2, BR-001: every place in every
    /// note, the open text with changes not yet saved, only the text.
    #[test]
    fn uc022_tc01_tc13_places_in_the_notes() {
        let texts: HashMap<&str, &str> = [
            ("Trip", "Book the ferry\nferry times"),
            ("Ideas", "a ferry to Hydra"),
            ("Log", "ferry"),
            ("ferry/Ferry-Log", "nothing here"),
        ]
        .into_iter()
        .collect();
        let mut request = places_request(
            "ferry",
            "boat",
            &["Trip", "Ideas", "Log", "ferry/Ferry-Log"],
        );
        let result = find_places(&request, read_from(&texts), || false).unwrap();
        assert_eq!((result.asked, result.places, result.notes.len()), (7, 4, 3));
        assert_eq!(result.notes[0].shown[0].old, "ferry");
        assert_eq!(result.notes[0].shown[0].new, "boat");
        request.open.insert(
            NoteId::from_relative("Trip"),
            "ferry pier\nBook the ferry".into(),
        );
        let open = find_places(&request, read_from(&texts), || false).unwrap();
        assert_eq!(open.notes[0].shown[0].after, " pier");
        assert!(find_places(&request, read_from(&texts), || true).is_none());
    }

    /// TC-022-11, UC-022 A7: 10,000 places shown, all counted.
    #[test]
    fn uc022_tc11_many_places() {
        let note = "x ".repeat(600);
        let names: Vec<String> = (0..20).map(|n| format!("N{n}")).collect();
        let texts: HashMap<&str, &str> =
            names.iter().map(|n| (n.as_str(), note.as_str())).collect();
        let notes: Vec<&str> = names.iter().map(String::as_str).collect();
        let result = find_places(&places_request("x", "y", &notes), read_from(&texts), || {
            false
        })
        .unwrap();
        assert_eq!(
            (result.places, result.shown),
            (12_000, SHOWN_REPLACE_PLACES)
        );
        assert_eq!(result.notes[19].shown.len(), 0);
        assert_eq!(result.notes[19].count, 600);
    }

    /// TC-022-06, A2: a request that is not valid finds nothing.
    #[test]
    fn uc022_tc06_nothing_for_an_invalid_request() {
        let texts: HashMap<&str, &str> = [("A", "(12")].into_iter().collect();
        let mut request = places_request("(\\d\\d", "", &["A"]);
        request.switches.regex = true;
        assert_eq!(
            find_places(&request, read_from(&texts), || false)
                .unwrap()
                .places,
            0
        );
    }

    /// TC-022-16, NFR-017, UC-022 BR-007: the places of 20 search texts in
    /// 10,000 notes of 1 to 50 KB, 1,000 of which hold "Acme", with the
    /// texts kept as the searcher keeps them. Prints the times; run with
    /// `cargo test -p underline-core --release -- --ignored --nocapture
    /// uc022_tc16`.
    #[test]
    #[ignore = "a measurement"]
    fn uc022_tc16_nfr017_places_in_a_large_workspace() {
        let root =
            std::env::temp_dir().join(format!("underline-places-big-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let words = [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
            "juliet",
        ];
        let mut notes = Vec::new();
        let mut k: usize = 1;
        for n in 0usize..10_000 {
            k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let size = 1_000 + (k >> 8) % 49_000;
            let mut text = String::with_capacity(size + 64);
            while text.len() < size {
                k = k.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                text.push_str(words[(k >> 16) % words.len()]);
                text.push_str(if (k >> 12).is_multiple_of(11) {
                    "\n"
                } else {
                    " "
                });
            }
            if n % 10 == 0 {
                text.push_str("\nAcme ships the order.\n");
            }
            text.push_str(&format!("unique{n}\n"));
            fs::write(root.join(format!("Note-{n}.md")), text).unwrap();
            notes.push(entry(&format!("Note-{n}")));
        }
        let mut cache = Cache::new();
        let request = |text: &str, regex: bool| PlacesRequest {
            asked: 1,
            text: text.into(),
            replacement: "Acme Corp".into(),
            switches: Switches {
                regex,
                ..Switches::default()
            },
            notes: notes.clone(),
            open: HashMap::new(),
        };
        let start = std::time::Instant::now();
        find_places(
            &request("Acme", false),
            |e| read_cached(&mut cache, &root, e),
            || false,
        )
        .unwrap();
        eprintln!("first, reading the files: {:?}", start.elapsed());
        let texts: [(&str, bool); 20] = [
            ("Acme", false),
            ("acme ships", false),
            ("unique9999", false),
            ("zulu", false),
            ("golf hotel", false),
            ("alpha", false),
            ("india juliet", false),
            ("order.", false),
            ("unique1", false),
            ("bravo charlie delta", false),
            (r"Acme\s+\w+", true),
            (r"unique\d{4}", true),
            ("^alpha", true),
            ("hotel$", true),
            (r"\bgolf\b", true),
            ("(echo|delta) golf", true),
            ("x", false),
            ("e", false),
            ("ships the", false),
            ("Acme ships the order", false),
        ];
        let mut times = Vec::new();
        for (text, regex) in texts {
            let start = std::time::Instant::now();
            let result = find_places(
                &request(text, regex),
                |e| read_cached(&mut cache, &root, e),
                || false,
            )
            .unwrap();
            let took = start.elapsed();
            eprintln!(
                "{text:>24}: {took:>12.2?}, {} places in {} notes",
                result.places,
                result.notes.len()
            );
            times.push(took);
        }
        fs::remove_dir_all(&root).unwrap();
        let slow = times.iter().filter(|t| t.as_secs_f64() > 2.0).count();
        eprintln!("{} of 20 within 2 s", 20 - slow);
        assert!(slow <= 1, "NFR-017: 95% within 2 seconds");
    }
}
