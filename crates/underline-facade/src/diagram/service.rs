//! Rendering diagrams on a worker thread with a cache (ADR-009, NFR-004):
//! the facade asks for a diagram every frame it shows it and gets its
//! picture at once when it is known; otherwise the diagram is rendered in
//! the background, the most recently asked for first, and the facade is
//! woken when it is done.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::{
    rasterize, DiagramError, DiagramLanguage, DiagramRenderer, Mermaid, Picture, PlantUml,
};

/// Diagrams kept in the cache.
const CACHE_SIZE: usize = 64;
/// A diagram nobody asked for again within this time is not rendered.
const FORGET_AFTER: Duration = Duration::from_secs(3);

/// What is known of a diagram.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagramState {
    /// Being rendered.
    Pending,
    Ready(Arc<Picture>),
    Failed(DiagramError),
}

/// The diagrams of the app, rendered on a worker thread.
pub struct Diagrams {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,
    work: Condvar,
    wake: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct State {
    cache: HashMap<u64, Done>,
    wanted: HashMap<u64, Job>,
    generation: u64,
    stop: bool,
}

struct Done {
    result: Result<Arc<Picture>, DiagramError>,
    used: Instant,
}

struct Job {
    language: DiagramLanguage,
    source: String,
    dark: bool,
    scale: f32,
    asked: Instant,
}

impl Diagrams {
    /// Starts the worker thread; `wake` is called when a diagram is done.
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            wake: Box::new(wake),
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("underline-diagrams".into())
            .spawn(move || work(&worker))
            .expect("start the diagram thread");
        Self { shared }
    }

    /// The diagram in `language` with `source`, in dark or light colors, as
    /// pixels at `scale` pixels per point.
    pub fn get(
        &self,
        language: DiagramLanguage,
        source: &str,
        dark: bool,
        scale: f32,
    ) -> DiagramState {
        let key = key(language, source, dark, scale);
        let mut state = self.shared.state.lock().expect("diagram state");
        if let Some(done) = state.cache.get_mut(&key) {
            done.used = Instant::now();
            return match &done.result {
                Ok(picture) => DiagramState::Ready(Arc::clone(picture)),
                Err(error) => DiagramState::Failed(error.clone()),
            };
        }
        let now = Instant::now();
        state
            .wanted
            .entry(key)
            .and_modify(|job| job.asked = now)
            .or_insert_with(|| Job {
                language,
                source: source.to_owned(),
                dark,
                scale,
                asked: now,
            });
        self.shared.work.notify_one();
        DiagramState::Pending
    }

    /// Counts the diagrams rendered so far; it changes when one is done.
    pub fn generation(&self) -> u64 {
        self.shared.state.lock().expect("diagram state").generation
    }
}

impl Drop for Diagrams {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.stop = true;
        }
        self.shared.work.notify_all();
    }
}

fn key(language: DiagramLanguage, source: &str, dark: bool, scale: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    (language, source, dark, scale.to_bits()).hash(&mut hasher);
    hasher.finish()
}

/// The worker: renders the most recently asked for diagram until stopped.
fn work(shared: &Shared) {
    let mut renderers: HashMap<DiagramLanguage, Box<dyn DiagramRenderer>> = HashMap::new();
    loop {
        let (key, job) = {
            let mut state = shared.state.lock().expect("diagram state");
            loop {
                if state.stop {
                    return;
                }
                state
                    .wanted
                    .retain(|_, job| job.asked.elapsed() < FORGET_AFTER);
                let latest = state
                    .wanted
                    .iter()
                    .max_by_key(|(_, job)| job.asked)
                    .map(|(key, _)| *key);
                if let Some(key) = latest {
                    let job = state.wanted.remove(&key).expect("wanted");
                    break (key, job);
                }
                state = shared.work.wait(state).expect("diagram state");
            }
        };
        let renderer = renderers
            .entry(job.language)
            .or_insert_with(|| renderer(job.language));
        // A failing engine must not stop the worker: it starts afresh.
        let rendered = panic::catch_unwind(AssertUnwindSafe(|| {
            renderer
                .render(&job.source, job.dark)
                .and_then(|svg| rasterize(&svg, job.scale))
        }));
        let result = match rendered {
            Ok(result) => result.map(Arc::new),
            Err(_) => {
                renderers.remove(&job.language);
                Err(DiagramError::Engine("the renderer failed".into()))
            }
        };
        {
            let mut state = shared.state.lock().expect("diagram state");
            if state.cache.len() >= CACHE_SIZE {
                let oldest = state
                    .cache
                    .iter()
                    .min_by_key(|(_, done)| done.used)
                    .map(|(key, _)| *key);
                if let Some(oldest) = oldest {
                    state.cache.remove(&oldest);
                }
            }
            state.cache.insert(
                key,
                Done {
                    result,
                    used: Instant::now(),
                },
            );
            state.generation += 1;
        }
        (shared.wake)();
    }
}

fn renderer(language: DiagramLanguage) -> Box<dyn DiagramRenderer> {
    match language {
        DiagramLanguage::PlantUml => Box::new(PlantUml::new()),
        DiagramLanguage::Mermaid => Box::new(Mermaid::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// NFR-004: asking does not wait; the picture comes from the worker and
    /// is then known at once, and the facade is woken.
    #[test]
    fn nfr004_diagrams_render_in_the_background() {
        let woken = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&woken);
        let diagrams = Diagrams::start(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let source = "@startuml\nAlice -> Bob : hi\n@enduml";
        let asked = Instant::now();
        assert_eq!(
            diagrams.get(DiagramLanguage::PlantUml, source, false, 1.0),
            DiagramState::Pending
        );
        assert!(asked.elapsed() < Duration::from_millis(50));
        let deadline = Instant::now() + Duration::from_secs(30);
        let picture = loop {
            match diagrams.get(DiagramLanguage::PlantUml, source, false, 1.0) {
                DiagramState::Ready(picture) => break picture,
                DiagramState::Failed(error) => panic!("{error}"),
                DiagramState::Pending => {
                    assert!(Instant::now() < deadline, "no picture");
                    thread::sleep(Duration::from_millis(20));
                }
            }
        };
        assert!(picture.width > 50 && picture.height > 50);
        assert!(woken.load(Ordering::SeqCst) >= 1);
        assert!(diagrams.generation() >= 1);

        let mermaid = "graph TD\nA-->B";
        diagrams.get(DiagramLanguage::Mermaid, mermaid, false, 1.0);
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match diagrams.get(DiagramLanguage::Mermaid, mermaid, false, 1.0) {
                DiagramState::Ready(_) => break,
                DiagramState::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10))
                }
                other => panic!("{other:?}"),
            }
        }
    }
}
