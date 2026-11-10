//! UC-008 Find Note: the search over the whole workspace, which runs in the
//! background so typing is never held up (BR-005). What a search text
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

use underline_markup::search::found_lines_lowered;
pub use underline_markup::search::{
    found_lines, lower, FoundLine, Query, LINE_CONTEXT, SHOWN_PLACES,
};

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
