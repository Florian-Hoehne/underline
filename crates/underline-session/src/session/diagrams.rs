//! M-09.7, ADR-009: the diagrams of the facade as SVG, rendered offline on
//! a worker thread with a cache, the most recently asked for first, as the
//! facade engine's `Diagrams` does for the egui app. The window shows the
//! SVG as it is (plan section 6), so nothing is rasterized; the own protocol
//! `ul-asset` serves it by its key (ADR-023).
//!
//! The cache is bounded by count and by bytes. A picture's address carries
//! the version it was rendered in, so a diagram rendered again after it left
//! the cache loads again. A renderer that panics, also while it starts, is
//! made anew for the next diagram; should the worker itself end, every
//! diagram not rendered says so instead of waiting for ever.

use std::collections::hash_map::DefaultHasher;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use underline_facade::diagram::{
    DiagramError, DiagramLanguage, DiagramRenderer, Mermaid, PlantUml,
};

use super::Wake;

/// Diagrams kept in the cache; more than the egui app keeps pictures, as an
/// SVG is small and the window may ask for one again while it scrolls.
const CACHE_SIZE: usize = 128;
/// The most bytes of SVG kept in the cache.
const CACHE_BYTES: usize = 32 * 1024 * 1024;
/// A diagram nobody asked for again within this time is not rendered.
const FORGET_AFTER: Duration = Duration::from_secs(3);

/// What is known of a diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SvgState {
    /// Being rendered.
    Pending,
    /// Rendered; the path the protocol serves it by, with its version.
    Ready(String),
    /// UC-004 A11: it cannot be rendered.
    Failed(DiagramError),
}

/// The diagrams of the window, rendered on a worker thread.
pub(crate) struct SvgDiagrams {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for SvgDiagrams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SvgDiagrams")
    }
}

struct Shared {
    state: Mutex<State>,
    work: Condvar,
    wake: Wake,
}

#[derive(Default)]
struct State {
    cache: HashMap<u64, Done>,
    /// The bytes of SVG in the cache.
    bytes: usize,
    wanted: HashMap<u64, Job>,
    generation: u64,
    stop: bool,
    /// The worker ended; nothing is rendered any more.
    dead: bool,
}

struct Done {
    result: Result<Arc<str>, DiagramError>,
    used: Instant,
    /// The generation it was rendered in, part of its address.
    version: u64,
}

struct Job {
    language: DiagramLanguage,
    source: String,
    dark: bool,
    asked: Instant,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while the lock was held leaves a usable cache.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Marks the worker dead when its thread ends in any way but a stop.
struct Alive(Arc<Shared>);

impl Drop for Alive {
    fn drop(&mut self) {
        let stopped = {
            let mut state = self.0.lock();
            state.dead = true;
            state.stop
        };
        if !stopped {
            self.0.wake.wake();
        }
    }
}

/// The key of a diagram: its language, its source and the theme's darkness.
fn key(language: DiagramLanguage, source: &str, dark: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    (language, source, dark).hash(&mut hasher);
    hasher.finish()
}

/// The key of the protocol path `diagram/{key}.svg` (with or without its
/// query), or of the bare key.
fn key_of(name: &str) -> Option<u64> {
    let name = name.split(['?', '#']).next().unwrap_or("");
    let name = name.strip_prefix("diagram/").unwrap_or(name);
    let name = name.strip_suffix(".svg").unwrap_or(name);
    u64::from_str_radix(name, 16).ok()
}

/// What the window is told when the worker cannot render.
fn not_available() -> DiagramError {
    DiagramError::Engine("the renderer is not running".into())
}

impl SvgDiagrams {
    /// Starts the worker thread; `wake` is called when a diagram is done.
    ///
    /// # Errors
    ///
    /// The thread could not be started.
    pub(crate) fn start(wake: Wake) -> std::io::Result<Self> {
        Self::start_with(wake, renderer)
    }

    /// Like [`Self::start`], with the renderers `make` gives.
    fn start_with(
        wake: Wake,
        make: fn(DiagramLanguage) -> Box<dyn DiagramRenderer>,
    ) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            wake,
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("underline-svg-diagrams".into())
            .spawn(move || {
                let alive = Alive(Arc::clone(&worker));
                work(&worker, make);
                drop(alive);
            })?;
        Ok(Self { shared })
    }

    /// The diagram in `language` with `source`, in dark or light colours
    /// (inventory D-10: not the theme's palette). Of the diagrams asked
    /// for at about the same time, the one `ahead` the most renders first.
    pub(crate) fn get(
        &self,
        language: DiagramLanguage,
        source: &str,
        dark: bool,
        ahead: Duration,
    ) -> SvgState {
        let key = key(language, source, dark);
        let mut state = self.shared.lock();
        if let Some(done) = state.cache.get_mut(&key) {
            done.used = Instant::now();
            return match &done.result {
                Ok(_) => SvgState::Ready(format!("diagram/{key:016x}.svg?v={}", done.version)),
                Err(error) => SvgState::Failed(error.clone()),
            };
        }
        if state.dead {
            return SvgState::Failed(not_available());
        }
        let now = Instant::now() + ahead;
        state
            .wanted
            .entry(key)
            .and_modify(|job| job.asked = now)
            .or_insert_with(|| Job {
                language,
                source: source.to_owned(),
                dark,
                asked: now,
            });
        self.shared.work.notify_one();
        SvgState::Pending
    }

    /// The SVG served by `name` (the key, or the protocol path), if it is
    /// rendered and still cached.
    pub(crate) fn svg(&self, name: &str) -> Option<Arc<str>> {
        let key = key_of(name)?;
        let mut state = self.shared.lock();
        let done = state.cache.get_mut(&key)?;
        done.used = Instant::now();
        done.result.as_ref().ok().map(Arc::clone)
    }

    /// Whether the picture at the protocol path `path` is still served.
    pub(crate) fn has(&self, path: &str) -> bool {
        key_of(path).is_some_and(|key| {
            self.shared
                .lock()
                .cache
                .get(&key)
                .is_some_and(|done| done.result.is_ok())
        })
    }

    /// Counts the diagrams rendered so far; it changes when one is done.
    pub(crate) fn generation(&self) -> u64 {
        self.shared.lock().generation
    }
}

impl Drop for SvgDiagrams {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.work.notify_all();
    }
}

/// The renderer of `language` (ADR-009).
fn renderer(language: DiagramLanguage) -> Box<dyn DiagramRenderer> {
    match language {
        DiagramLanguage::PlantUml => Box::new(PlantUml::new()),
        DiagramLanguage::Mermaid => Box::new(Mermaid::new()),
    }
}

/// Renders `job`; a renderer that panics, also while it is made, is
/// dropped and made anew for the next diagram.
fn render(
    renderers: &mut HashMap<DiagramLanguage, Box<dyn DiagramRenderer>>,
    make: fn(DiagramLanguage) -> Box<dyn DiagramRenderer>,
    job: &Job,
) -> Result<Arc<str>, DiagramError> {
    let renderer = match renderers.entry(job.language) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let made = panic::catch_unwind(|| make(job.language))
                .map_err(|_| DiagramError::Engine("the renderer could not be started".into()))?;
            entry.insert(made)
        }
    };
    let rendered = panic::catch_unwind(AssertUnwindSafe(|| renderer.render(&job.source, job.dark)));
    match rendered {
        Ok(result) => result.map(Arc::<str>::from),
        Err(_) => {
            renderers.remove(&job.language);
            Err(DiagramError::Engine("the renderer failed".into()))
        }
    }
}

/// The worker: renders the most recently asked for diagram until stopped.
fn work(shared: &Shared, make: fn(DiagramLanguage) -> Box<dyn DiagramRenderer>) {
    let mut renderers: HashMap<DiagramLanguage, Box<dyn DiagramRenderer>> = HashMap::new();
    while let Some((key, job)) = next_job(shared) {
        let result = render(&mut renderers, make, &job);
        {
            let mut state = shared.lock();
            state.generation += 1;
            let size = result.as_ref().map_or(0, |svg| svg.len());
            let version = state.generation;
            if let Some(old) = state.cache.insert(
                key,
                Done {
                    result,
                    used: Instant::now(),
                    version,
                },
            ) {
                state.bytes -= old.result.map_or(0, |svg| svg.len());
            }
            state.bytes += size;
            evict(&mut state, key);
        }
        shared.wake.wake();
    }
}

/// Drops the diagrams used longest ago while the cache holds too many or
/// too many bytes; `keep` stays.
fn evict(state: &mut State, keep: u64) {
    while state.cache.len() > CACHE_SIZE || state.bytes > CACHE_BYTES {
        let oldest = state
            .cache
            .iter()
            .filter(|(key, _)| **key != keep)
            .min_by_key(|(_, done)| done.used)
            .map(|(key, _)| *key);
        let Some(oldest) = oldest else {
            break;
        };
        if let Some(done) = state.cache.remove(&oldest) {
            state.bytes -= done.result.map_or(0, |svg| svg.len());
        }
    }
}

/// Waits for the diagram asked for last; `None` once stopped.
fn next_job(shared: &Shared) -> Option<(u64, Job)> {
    let mut state = shared.lock();
    loop {
        if state.stop {
            return None;
        }
        state
            .wanted
            .retain(|_, job| job.asked.elapsed() < FORGET_AFTER);
        let latest = state
            .wanted
            .iter()
            .max_by_key(|(_, job)| job.asked)
            .map(|(key, _)| *key);
        if let Some(job) = latest.and_then(|key| state.wanted.remove(&key).map(|job| (key, job))) {
            return Some(job);
        }
        state = shared
            .work
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// Renders `<svg>{source}</svg>`, or fails for "boom".
    struct Fake;

    impl DiagramRenderer for Fake {
        fn render(&mut self, source: &str, dark: bool) -> Result<String, DiagramError> {
            if source == "boom" {
                return Err(DiagramError::Syntax {
                    line: Some(1),
                    message: "no diagram".into(),
                });
            }
            Ok(format!("<svg data-dark=\"{dark}\">{source}</svg>"))
        }
    }

    fn fake(_: DiagramLanguage) -> Box<dyn DiagramRenderer> {
        Box::new(Fake)
    }

    fn wait(diagrams: &SvgDiagrams, source: &str) -> SvgState {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match diagrams.get(DiagramLanguage::Mermaid, source, false, Duration::ZERO) {
                SvgState::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                state => return state,
            }
        }
    }

    /// M-09.7, NFR-004: asking does not wait; the SVG comes from the worker,
    /// is cached and served by its key, and the tick is woken.
    #[test]
    fn m09_7_diagrams_render_to_svg_in_the_background_and_are_cached() {
        let woken = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&woken);
        let wake = Wake(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        let diagrams = SvgDiagrams::start_with(wake, fake).unwrap();

        assert_eq!(
            diagrams.get(DiagramLanguage::Mermaid, "graph TD", false, Duration::ZERO),
            SvgState::Pending
        );
        let SvgState::Ready(path) = wait(&diagrams, "graph TD") else {
            panic!("no picture");
        };

        assert!(
            path.starts_with("diagram/") && path.contains(".svg?v="),
            "{path}"
        );
        assert_eq!(
            diagrams.svg(&path).as_deref(),
            Some("<svg data-dark=\"false\">graph TD</svg>")
        );
        assert!(diagrams.has(&path));
        assert!(woken.load(Ordering::SeqCst) >= 1);
        assert!(diagrams.generation() >= 1);
        // The same source in the dark is another picture.
        assert_eq!(
            diagrams.get(DiagramLanguage::Mermaid, "graph TD", true, Duration::ZERO),
            SvgState::Pending
        );
        assert_eq!(diagrams.svg("not a key"), None);
        assert_eq!(diagrams.svg("0000000000000001"), None);
    }

    /// UC-004 A11: a diagram that cannot be rendered says why, and is not
    /// served.
    #[test]
    fn m09_7_a_failing_diagram_says_why() {
        let diagrams = SvgDiagrams::start_with(Wake::default(), fake).unwrap();

        let state = wait(&diagrams, "boom");

        let SvgState::Failed(error) = state else {
            panic!("{state:?}");
        };
        assert_eq!(error.to_string(), "Line 1: no diagram");
    }

    fn panicking(_: DiagramLanguage) -> Box<dyn DiagramRenderer> {
        panic!("no engine")
    }

    /// RUST 7: a renderer that cannot be made says so for its diagram.
    #[test]
    fn m09_7_a_renderer_that_cannot_start_says_so() {
        let diagrams = SvgDiagrams::start_with(Wake::default(), panicking).unwrap();

        let state = wait(&diagrams, "graph TD");

        assert_eq!(
            state,
            SvgState::Failed(DiagramError::Engine(
                "the renderer could not be started".into()
            ))
        );
    }

    /// RUST 7: once the worker is gone, a diagram fails instead of waiting.
    #[test]
    fn m09_7_without_a_worker_diagrams_fail() {
        let diagrams = SvgDiagrams::start_with(Wake::default(), fake).unwrap();
        diagrams.shared.lock().dead = true;

        assert_eq!(
            diagrams.get(DiagramLanguage::Mermaid, "x", false, Duration::ZERO),
            SvgState::Failed(not_available())
        );
    }

    /// RUST 6: the cache keeps to its bytes, the newest picture stays.
    #[test]
    fn m09_7_the_cache_is_bounded_by_bytes() {
        let mut state = State::default();
        for key in 0..3_u64 {
            state.cache.insert(
                key,
                Done {
                    result: Ok(Arc::from("x".repeat(CACHE_BYTES / 2).as_str())),
                    used: Instant::now(),
                    version: key,
                },
            );
            state.bytes += CACHE_BYTES / 2;
        }

        evict(&mut state, 2);

        assert!(state.bytes <= CACHE_BYTES);
        assert!(state.cache.contains_key(&2));
        assert_eq!(state.cache.len(), 2);
    }
}
