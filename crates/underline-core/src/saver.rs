//! Storing notes on a worker thread (UC-005 BR-001, ADR-005, ADR-012), and
//! remembering Underline's own writes so the watcher can tell them from
//! changes made outside (ADR-011).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::atomic::write_atomic;

/// How long an own write is remembered (ADR-011).
const OWN_WRITE_MEMORY: Duration = Duration::from_secs(60);

/// The files Underline wrote itself, with a hash of what it wrote.
#[derive(Debug, Clone, Default)]
pub struct OwnWrites(Arc<Mutex<HashMap<PathBuf, (u64, Instant)>>>);

impl OwnWrites {
    /// Remembers that Underline wrote `content` to `path`.
    pub fn record(&self, path: &Path, content: &[u8]) {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        map.retain(|_, (_, at)| now.duration_since(*at) < OWN_WRITE_MEMORY);
        map.insert(path.to_owned(), (hash(content), now));
    }

    /// Whether `path` holds exactly what Underline wrote to it last.
    pub fn is_own(&self, path: &Path, content: &[u8]) -> bool {
        let map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        map.get(path).is_some_and(|(h, _)| *h == hash(content))
    }
}

fn hash(content: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// A note to store.
#[derive(Debug, Clone)]
pub struct SaveJob {
    pub path: PathBuf,
    pub text: String,
    /// When the text was taken from the editor.
    pub started: Instant,
}

/// The outcome of a [`SaveJob`].
#[derive(Debug)]
pub struct SaveResult {
    pub path: PathBuf,
    pub started: Instant,
    pub result: Result<(), String>,
}

/// Stores notes atomically on a worker thread.
pub struct NoteSaver {
    jobs: Sender<SaveJob>,
    results: Receiver<SaveResult>,
    own: OwnWrites,
}

impl NoteSaver {
    pub fn new(own: OwnWrites) -> Self {
        let (jobs, job_rx) = channel::<SaveJob>();
        let (result_tx, results) = channel();
        let worker_own = own.clone();
        std::thread::Builder::new()
            .name("underline-saver".into())
            .spawn(move || {
                for job in job_rx {
                    let result = store(&worker_own, &job.path, &job.text);
                    let _ = result_tx.send(SaveResult {
                        path: job.path,
                        started: job.started,
                        result,
                    });
                }
            })
            .expect("the saver thread starts");
        Self { jobs, results, own }
    }

    /// Stores a note in the background; the outcome comes from [`Self::poll`].
    pub fn save(&self, job: SaveJob) {
        let _ = self.jobs.send(job);
    }

    /// Stores a note now, for closing a tab or Underline (UC-005 BR-001).
    pub fn save_now(&self, path: &Path, text: &str) -> Result<(), String> {
        store(&self.own, path, text)
    }

    /// The outcomes of background saves that finished since the last call.
    pub fn poll(&self) -> Vec<SaveResult> {
        self.results.try_iter().collect()
    }

    pub fn own_writes(&self) -> &OwnWrites {
        &self.own
    }

    /// What stores a note at once, like [`NoteSaver::save_now`], apart from
    /// the saver: it can be used while the session is let go.
    pub fn writer(&self) -> NoteWriter {
        NoteWriter(self.own.clone())
    }
}

/// Stores notes atomically at once, as [`NoteSaver::save_now`] does,
/// recording them as Underline's own writes.
#[derive(Debug, Clone)]
pub struct NoteWriter(OwnWrites);

impl NoteWriter {
    /// Stores `text` at `path` now, atomically.
    pub fn save_now(&self, path: &Path, text: &str) -> Result<(), String> {
        store(&self.0, path, text)
    }
}

fn store(own: &OwnWrites, path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    own.record(path, text.as_bytes());
    write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-005 step 4: a note is stored in the background and recognized as
    /// Underline's own write.
    #[test]
    fn uc005_stores_in_the_background_and_remembers_own_writes() {
        let dir = std::env::temp_dir().join(format!("underline-saver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Plan.md");
        let saver = NoteSaver::new(OwnWrites::default());
        saver.save(SaveJob {
            path: path.clone(),
            text: "# Plan\n".into(),
            started: Instant::now(),
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let results = loop {
            let results = saver.poll();
            if !results.is_empty() || Instant::now() > deadline {
                break results;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(results[0].result.is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Plan\n");
        assert!(saver.own_writes().is_own(&path, b"# Plan\n"));
        assert!(!saver.own_writes().is_own(&path, b"# Changed\n"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// TC-005-23: a note that cannot be stored reports why.
    #[test]
    fn uc005_tc23_reports_a_failed_store() {
        let blocker =
            std::env::temp_dir().join(format!("underline-saver-block-{}", std::process::id()));
        std::fs::write(&blocker, "a file").unwrap();
        let saver = NoteSaver::new(OwnWrites::default());
        assert!(saver.save_now(&blocker.join("Plan.md"), "x").is_err());
        std::fs::remove_file(&blocker).unwrap();
    }
}
