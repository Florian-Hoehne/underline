//! FR-098, UC-004 BR-019, A21, ADR-029: the formulas of the facade typeset
//! as SVG on a worker thread of their own, so a formula never waits behind
//! a PlantUML diagram, with a cache and the most recently asked for first,
//! as the diagrams have (ADR-009, `diagrams.rs`). The window shows the SVG
//! as an `<img>`; the own protocol `ul-asset` serves it by its key
//! (ADR-023).
//!
//! The key is the formula's source, whether it is a display formula, and
//! the theme, whose text colour it is drawn in; another theme is another
//! picture. A picture's address carries the version it was typeset in
//! (`?v=`), so one typeset again after it left the cache loads again. A
//! renderer that panics says so for its formula and is made anew; should
//! the worker itself end, every formula not typeset says so instead of
//! waiting for ever.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use underline_facade::math::{MathError, MathRenderer, Ratex, Typeset};
use underline_facade::themes::Rgb;

use super::Wake;

/// Formulas kept in the cache.
const CACHE_SIZE: usize = 1024;
/// The most bytes of SVG kept in the cache.
const CACHE_BYTES: usize = 32 * 1024 * 1024;
/// A formula nobody asked for again within this time is not typeset.
const FORGET_AFTER: Duration = Duration::from_secs(3);

/// A typeset formula as the window shows it: the path the protocol serves
/// it by, with its version, and its size in thousandths of an em.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) depth: u32,
}

/// What is known of a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormulaState {
    /// Being typeset.
    Pending,
    /// Typeset: the protocol path with its version, and its size.
    Ready(String, Size),
    /// UC-004 A21: it cannot be typeset.
    Failed(MathError),
}

/// One formula to typeset.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted<'a> {
    pub(crate) source: &'a str,
    pub(crate) display: bool,
    /// The theme's name, part of the key.
    pub(crate) theme: &'a str,
    /// The theme's text colour.
    pub(crate) color: Rgb,
}

/// The formulas of the window, typeset on a worker thread.
pub(crate) struct SvgFormulas {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for SvgFormulas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SvgFormulas")
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
    bytes: usize,
    wanted: HashMap<u64, Job>,
    generation: u64,
    /// The formula the worker typesets now: asked for again, it waits for
    /// that run instead of being typeset twice.
    in_flight: Option<u64>,
    stop: bool,
    dead: bool,
}

struct Done {
    result: Result<(Arc<str>, Size), MathError>,
    used: Instant,
    version: u64,
    /// What was typeset: the key is a 64-bit hash, so a formula found by it
    /// is taken only when it is this one.
    what: (String, bool, Rgb),
}

struct Job {
    source: String,
    display: bool,
    color: Rgb,
    asked: Instant,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
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

/// The key of a formula, the same as the webview keeps it by (ADR-025,
/// `underline_editor_view::views::formula_key`), and the name of its
/// picture.
fn key(wanted: &Wanted<'_>) -> u64 {
    underline_editor_view::views::formula_hash(wanted.theme, wanted.display, wanted.source)
}

/// The key of the protocol path `formula/{key}.svg` (with or without its
/// query), or of the bare key.
fn key_of(name: &str) -> Option<u64> {
    let name = name.split(['?', '#']).next().unwrap_or("");
    let name = name.strip_prefix("formula/").unwrap_or(name);
    let name = name.strip_suffix(".svg").unwrap_or(name);
    u64::from_str_radix(name, 16).ok()
}

/// Thousandths of an em.
fn milli(em: f64) -> u32 {
    // A formula is a few em; anything else is clamped.
    let value = (em.max(0.0) * 1000.0).round().min(f64::from(u32::MAX));
    // The value is in range and not negative.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let value = value as u32;
    value
}

/// The renderer the worker makes.
type Make = fn() -> Box<dyn MathRenderer>;

fn ratex() -> Box<dyn MathRenderer> {
    Box::new(Ratex)
}

impl SvgFormulas {
    /// Starts the worker thread; `wake` is called when a formula is done.
    ///
    /// # Errors
    ///
    /// The thread could not be started.
    pub(crate) fn start(wake: Wake) -> std::io::Result<Self> {
        Self::start_with(wake, ratex)
    }

    fn start_with(wake: Wake, make: Make) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            wake,
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("underline-svg-formulas".into())
            // A deeply nested formula recurses in the parser and the layout.
            .stack_size(8 << 20)
            .spawn(move || {
                let alive = Alive(Arc::clone(&worker));
                work(&worker, make);
                drop(alive);
            })?;
        Ok(Self { shared })
    }

    /// The formula `wanted`; of the formulas asked for at about the same
    /// time, the one `ahead` the most is typeset first. Asking never waits
    /// for the typesetting (NFR-002).
    pub(crate) fn get(&self, wanted: &Wanted<'_>, ahead: Duration) -> FormulaState {
        let key = key(wanted);
        let mut state = self.shared.lock();
        let what = (wanted.source.to_owned(), wanted.display, wanted.color);
        if let Some(done) = state.cache.get_mut(&key).filter(|done| done.what == what) {
            done.used = Instant::now();
            return match &done.result {
                Ok((_, size)) => {
                    FormulaState::Ready(format!("formula/{key:016x}.svg?v={}", done.version), *size)
                }
                Err(error) => FormulaState::Failed(error.clone()),
            };
        }
        if state.dead {
            return FormulaState::Failed(not_available());
        }
        if state.in_flight == Some(key) {
            return FormulaState::Pending;
        }
        let now = Instant::now() + ahead;
        state
            .wanted
            .entry(key)
            .and_modify(|job| job.asked = now)
            .or_insert_with(|| Job {
                source: wanted.source.to_owned(),
                display: wanted.display,
                color: wanted.color,
                asked: now,
            });
        self.shared.work.notify_one();
        FormulaState::Pending
    }

    /// The SVG served by `name` (the key, or the protocol path), if it is
    /// typeset and still cached.
    pub(crate) fn svg(&self, name: &str) -> Option<Arc<str>> {
        let key = key_of(name)?;
        let mut state = self.shared.lock();
        let done = state.cache.get_mut(&key)?;
        done.used = Instant::now();
        done.result.as_ref().ok().map(|(svg, _)| Arc::clone(svg))
    }

    /// Counts the formulas typeset so far; it changes when one is done.
    pub(crate) fn generation(&self) -> u64 {
        self.shared.lock().generation
    }
}

/// Stops the worker without waiting for it (it is not joined): a formula it
/// typesets now is finished and dropped, and its thread ends then.
impl Drop for SvgFormulas {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.work.notify_all();
    }
}

/// What the window is told when the worker cannot typeset.
fn not_available() -> MathError {
    MathError::Engine("the typesetting is not running".into())
}

/// Typesets `job`; a renderer that panics is made anew for the next one.
fn render(
    renderer: &mut Option<Box<dyn MathRenderer>>,
    make: Make,
    job: &Job,
) -> Result<Typeset, MathError> {
    let current = match renderer {
        Some(current) => current,
        None => renderer.insert(
            panic::catch_unwind(make)
                .map_err(|_| MathError::Engine("the typesetting could not be started".into()))?,
        ),
    };
    let done = panic::catch_unwind(AssertUnwindSafe(|| {
        current.render(&job.source, job.display, job.color)
    }));
    done.unwrap_or_else(|_| {
        *renderer = None;
        Err(MathError::Engine("the typesetting failed".into()))
    })
}

/// The worker: typesets the most recently asked for formula until stopped.
fn work(shared: &Shared, make: Make) {
    let mut renderer = None;
    while let Some((key, job)) = next_job(shared) {
        let result = render(&mut renderer, make, &job).map(|typeset| {
            let size = Size {
                width: milli(typeset.width),
                height: milli(typeset.height),
                depth: milli(typeset.depth),
            };
            (Arc::<str>::from(typeset.svg), size)
        });
        // A picture the cache cannot hold is not kept (it would push out
        // every other one and still go beyond the bound).
        let result = result.and_then(|(svg, size)| {
            if svg.len() > CACHE_BYTES {
                Err(MathError::TooLarge(
                    "its picture is larger than the cache".into(),
                ))
            } else {
                Ok((svg, size))
            }
        });
        {
            let mut state = shared.lock();
            state.in_flight = None;
            state.generation += 1;
            let size = result.as_ref().map_or(0, |(svg, _)| svg.len());
            let version = state.generation;
            if let Some(old) = state.cache.insert(
                key,
                Done {
                    result,
                    used: Instant::now(),
                    version,
                    what: (job.source, job.display, job.color),
                },
            ) {
                state.bytes -= old.result.map_or(0, |(svg, _)| svg.len());
            }
            state.bytes += size;
            evict(&mut state, key);
        }
        shared.wake.wake();
    }
}

/// Drops the formulas used longest ago while the cache holds too many or
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
            state.bytes -= done.result.map_or(0, |(svg, _)| svg.len());
        }
    }
}

/// Waits for the formula asked for last; `None` once stopped.
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
            state.in_flight = Some(job.0);
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

    fn wanted(source: &str) -> Wanted<'_> {
        Wanted {
            source,
            display: false,
            theme: "Dark",
            color: [0xee, 0xee, 0xee],
        }
    }

    fn wait(formulas: &SvgFormulas, source: &str) -> FormulaState {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match formulas.get(&wanted(source), Duration::ZERO) {
                FormulaState::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(2));
                }
                state => return state,
            }
        }
    }

    /// TC-004-38, FR-098, NFR-002: asking does not wait; the SVG comes from
    /// the worker in the theme's text colour, is cached and served by its
    /// key with its size, and the tick is woken.
    #[test]
    fn tc004_38_formulas_typeset_in_the_background_and_are_cached() {
        let woken = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&woken);
        let wake = Wake(Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        let formulas = SvgFormulas::start(wake).unwrap();

        assert_eq!(
            formulas.get(&wanted("E=mc^2"), Duration::ZERO),
            FormulaState::Pending
        );
        let FormulaState::Ready(path, size) = wait(&formulas, "E=mc^2") else {
            panic!("not typeset");
        };

        assert!(
            path.starts_with("formula/") && path.contains(".svg?v="),
            "{path}"
        );
        let svg = formulas.svg(&path).unwrap();
        assert!(svg.starts_with("<svg ") && svg.contains("238,238,238"));
        assert!(size.width > size.height && size.depth > 0 && size.depth < size.height);
        assert!(woken.load(Ordering::SeqCst) >= 1);
        assert!(formulas.generation() >= 1);
        // Another theme is another picture.
        let light = Wanted {
            theme: "Light",
            ..wanted("E=mc^2")
        };
        assert_eq!(formulas.get(&light, Duration::ZERO), FormulaState::Pending);
        assert_eq!(formulas.svg("not a key"), None);
    }

    /// UC-004 A21: a formula with an error says why, and is not served.
    #[test]
    fn uc004_a21_a_formula_with_an_error_says_why() {
        let formulas = SvgFormulas::start(Wake::default()).unwrap();

        let state = wait(&formulas, "\\frac{1}{");

        let FormulaState::Failed(error) = state else {
            panic!("{state:?}");
        };
        assert!(matches!(error, MathError::Syntax { .. }), "{error:?}");
    }

    struct Panicking;

    impl MathRenderer for Panicking {
        fn render(&mut self, _: &str, _: bool, _: Rgb) -> Result<Typeset, MathError> {
            panic!("broken engine")
        }
    }

    /// FR-098, UC-004 A21: a renderer that panics says so for its formula; once the
    /// worker is gone, a formula fails instead of waiting.
    #[test]
    fn fr098_a_failing_engine_says_so() {
        let formulas = SvgFormulas::start_with(Wake::default(), || Box::new(Panicking)).unwrap();
        assert_eq!(
            wait(&formulas, "x"),
            FormulaState::Failed(MathError::Engine("the typesetting failed".into()))
        );
        formulas.shared.lock().dead = true;
        assert_eq!(
            formulas.get(&wanted("y"), Duration::ZERO),
            FormulaState::Failed(not_available())
        );
    }

    /// A renderer of a fixed SVG size that panics on a source with `panic_on`.
    struct Sized {
        bytes: usize,
        panic_on: &'static str,
    }

    impl MathRenderer for Sized {
        fn render(&mut self, source: &str, _: bool, _: Rgb) -> Result<Typeset, MathError> {
            if !self.panic_on.is_empty() && source.contains(self.panic_on) {
                panic!("broken engine on {source}");
            }
            Ok(Typeset {
                svg: format!("<svg {source}>{}", "x".repeat(self.bytes)),
                width: 1.0,
                height: 1.0,
                depth: 0.1,
            })
        }
    }

    /// The bytes the cache really holds.
    fn held(formulas: &SvgFormulas) -> (usize, usize, usize) {
        let state = formulas.shared.lock();
        let real = state
            .cache
            .values()
            .map(|done| done.result.as_ref().map_or(0, |(svg, _)| svg.len()))
            .sum();
        (state.cache.len(), state.bytes, real)
    }

    /// FR-098 (test monkey, passes): the cache holds at most `CACHE_SIZE`
    /// formulas, and its count of bytes is what it holds.
    #[test]
    fn fr098_the_cache_is_bounded_by_count_and_its_bytes_are_true() {
        let formulas = SvgFormulas::start_with(Wake::default(), || {
            Box::new(Sized {
                bytes: 10,
                panic_on: "",
            })
        })
        .unwrap();
        let sources: Vec<String> = (0..CACHE_SIZE + 300).map(|i| format!("f{i}")).collect();
        for source in &sources {
            formulas.get(&wanted(source), Duration::ZERO);
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        while formulas.generation() < sources.len() as u64 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let (count, bytes, real) = held(&formulas);
        assert!(count <= CACHE_SIZE, "{count} formulas");
        assert_eq!(bytes, real);
    }

    /// FR-098 (found by the test monkey): `CACHE_BYTES` is the most bytes of
    /// SVG the cache keeps. One formula of 40 MiB is kept as it is
    /// (`evict` never drops the newest), so the cache holds more than its
    /// limit, and two such formulas in turn hold 40 MiB each in the window's
    /// memory and in the protocol's reply.
    #[test]
    fn fr098_a_formula_larger_than_the_cache_is_not_kept_beyond_its_limit() {
        let formulas = SvgFormulas::start_with(Wake::default(), || {
            Box::new(Sized {
                bytes: CACHE_BYTES + 8 * 1024 * 1024,
                panic_on: "",
            })
        })
        .unwrap();
        wait(&formulas, "huge");
        let (_, bytes, _) = held(&formulas);
        assert!(
            bytes <= CACHE_BYTES,
            "{bytes} bytes held, limit {CACHE_BYTES}"
        );
    }

    /// FR-098 (test monkey, passes): a renderer that panics for one formula
    /// fails that one, is made anew, and the next ones are typeset.
    #[test]
    fn fr098_the_worker_goes_on_after_a_panic_in_the_renderer() {
        let formulas = SvgFormulas::start_with(Wake::default(), || {
            Box::new(Sized {
                bytes: 10,
                panic_on: "boom",
            })
        })
        .unwrap();
        assert!(matches!(wait(&formulas, "a"), FormulaState::Ready(..)));
        assert!(matches!(wait(&formulas, "boom"), FormulaState::Failed(_)));
        assert!(matches!(wait(&formulas, "b"), FormulaState::Ready(..)));
        assert!(matches!(wait(&formulas, "xboomx"), FormulaState::Failed(_)));
        assert!(matches!(wait(&formulas, "c"), FormulaState::Ready(..)));
        assert!(!formulas.shared.lock().dead);
    }

    static SLOW_RENDERS: AtomicUsize = AtomicUsize::new(0);

    /// Takes its time over each formula, and counts them.
    struct Slow;

    impl MathRenderer for Slow {
        fn render(&mut self, source: &str, _: bool, _: Rgb) -> Result<Typeset, MathError> {
            SLOW_RENDERS.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(150));
            Ok(Typeset {
                svg: format!("<svg {source}/>"),
                width: 1.0,
                height: 1.0,
                depth: 0.1,
            })
        }
    }

    /// NFR-002, FR-098 (review): a formula asked for again while it is
    /// typeset is not queued again, so it is typeset once and keeps the
    /// version it was typeset in.
    #[test]
    fn fr098_a_formula_in_flight_is_typeset_once() {
        let formulas = SvgFormulas::start_with(Wake::default(), || Box::new(Slow)).unwrap();
        assert_eq!(
            formulas.get(&wanted("z"), Duration::ZERO),
            FormulaState::Pending
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while formulas.shared.lock().in_flight.is_none() {
            assert!(Instant::now() < deadline, "never typeset");
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            formulas.get(&wanted("z"), Duration::ZERO),
            FormulaState::Pending
        );
        assert!(formulas.shared.lock().wanted.is_empty(), "not queued again");
        let FormulaState::Ready(path, _) = wait(&formulas, "z") else {
            panic!("not typeset");
        };
        assert!(path.ends_with("?v=1"), "{path}");
        thread::sleep(Duration::from_millis(200));
        assert_eq!(SLOW_RENDERS.load(Ordering::SeqCst), 1);
        assert_eq!(formulas.generation(), 1);
    }
}
