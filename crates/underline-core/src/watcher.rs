//! Watching a workspace for changes made outside Underline (ADR-011,
//! UC-004 A7–A9, NFR-008).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::format::NoteFormat;
use crate::git::Git;
use crate::saver::OwnWrites;
use crate::settings::SETTINGS_FILE_NAME;
use crate::workspace::is_workspace_directory;

/// Changes are collected this long before they are reported (ADR-011).
pub const WATCH_COLLECT: Duration = Duration::from_millis(200);
/// More changes than this in one collection mean a rescan (ADR-011).
pub const BULK_CHANGES: usize = 100;
/// How often open notes are checked when events are missing (ADR-011).
pub const POLL_OPEN_NOTES: Duration = Duration::from_secs(1);
/// How often the whole workspace is checked when events are missing.
pub const POLL_WORKSPACE: Duration = Duration::from_secs(10);

/// A change in the workspace that Underline did not make itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspaceEvent {
    /// A note's file was created or changed; the path is absolute.
    NoteChanged(PathBuf),
    /// A note's file is gone (deleted or moved away).
    NoteRemoved(PathBuf),
    /// The workspace settings file changed.
    SettingsChanged,
    /// Many files changed at once, or a folder changed: look at everything.
    Rescan,
}

/// Watches one workspace.
pub struct WorkspaceWatcher {
    root: PathBuf,
    own: OwnWrites,
    git: Git,
    ignored: HashSet<PathBuf>,
    events: Receiver<DebounceEventResult>,
    _debouncer: Option<Debouncer<notify::RecommendedWatcher, RecommendedCache>>,
    /// Set when the system delivers no events (ADR-011 fallback).
    polling: Option<Poller>,
}

/// The fallback of ADR-011: compares the files of the open notes every
/// second and the whole workspace every 10 seconds.
struct Poller {
    open: Arc<Mutex<Vec<PathBuf>>>,
    seen: HashMap<PathBuf, Option<(SystemTime, u64)>>,
    last_open: std::time::Instant,
    last_all: std::time::Instant,
}

impl WorkspaceWatcher {
    /// Starts watching `root`. Falls back to polling when the system cannot
    /// deliver events; [`Self::is_polling`] says so. `wake` is called from
    /// the watcher thread whenever changes arrive, so a window can ask for a
    /// new frame and call [`Self::poll`].
    pub fn start(root: &Path, own: OwnWrites, git: Git, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, events) = channel();
        let handler = move |result: DebounceEventResult| {
            let _ = tx.send(result);
            wake();
        };
        let debouncer =
            new_debouncer(WATCH_COLLECT, None, handler)
                .ok()
                .and_then(|mut debouncer| {
                    debouncer
                        .watch(root, RecursiveMode::Recursive)
                        .ok()
                        .map(|()| debouncer)
                });
        let polling = debouncer.is_none().then(|| Poller {
            open: Arc::default(),
            seen: HashMap::new(),
            last_open: std::time::Instant::now(),
            last_all: std::time::Instant::now(),
        });
        let mut watcher = Self {
            root: root.to_owned(),
            own,
            ignored: HashSet::new(),
            git,
            events,
            _debouncer: debouncer,
            polling,
        };
        watcher.refresh_ignored();
        watcher
    }

    /// Whether events are missing and the files are polled instead.
    pub fn is_polling(&self) -> bool {
        self.polling.is_some()
    }

    /// Tells the poller which files the open tabs show.
    pub fn set_open_notes(&mut self, paths: Vec<PathBuf>) {
        if let Some(poller) = &mut self.polling {
            *poller.open.lock().unwrap_or_else(|e| e.into_inner()) = paths;
        }
    }

    /// The changes since the last call, without Underline's own writes.
    pub fn poll(&mut self) -> Vec<WorkspaceEvent> {
        let mut paths = BTreeSet::new();
        let mut rescan = false;
        for result in self.events.try_iter() {
            match result {
                Ok(events) => {
                    for event in events {
                        paths.extend(event.event.paths.iter().cloned());
                    }
                }
                Err(_) => rescan = true,
            }
        }
        if let Some(poller) = &mut self.polling {
            rescan |= poller.poll(&mut paths);
        }
        if paths.len() > BULK_CHANGES {
            rescan = true;
        }
        let mut out = BTreeSet::new();
        if rescan {
            out.insert(WorkspaceEvent::Rescan);
        }
        if paths
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == ".gitignore"))
        {
            self.refresh_ignored();
            out.insert(WorkspaceEvent::Rescan);
        }
        if !rescan {
            for path in paths {
                if let Some(event) = self.classify(&path) {
                    out.insert(event);
                }
            }
        }
        out.into_iter().collect()
    }

    /// What a changed path means, or `None` when it does not concern notes.
    fn classify(&self, path: &Path) -> Option<WorkspaceEvent> {
        let relative = path.strip_prefix(&self.root).ok()?;
        if relative.as_os_str().is_empty() || self.is_hidden_or_ignored(relative) {
            return None;
        }
        if relative == Path::new(SETTINGS_FILE_NAME) {
            return Some(WorkspaceEvent::SettingsChanged);
        }
        // A folder that is a workspace of its own is not part of this one.
        if relative
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .filter(|a| !a.as_os_str().is_empty())
            .any(|a| is_workspace_directory(&self.root.join(a)))
        {
            return None;
        }
        let is_note = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(NoteFormat::from_extension)
            .is_some();
        if path.is_dir() {
            return Some(WorkspaceEvent::Rescan);
        }
        if !is_note {
            // A folder that is gone shows only as its old path.
            return (!path.exists() && path.extension().is_none())
                .then_some(WorkspaceEvent::Rescan);
        }
        match std::fs::read(path) {
            Ok(content) if self.own.is_own(path, &content) => None,
            Ok(_) => Some(WorkspaceEvent::NoteChanged(path.to_owned())),
            Err(_) if !path.exists() => Some(WorkspaceEvent::NoteRemoved(path.to_owned())),
            Err(_) => Some(WorkspaceEvent::NoteChanged(path.to_owned())),
        }
    }

    fn is_hidden_or_ignored(&self, relative: &Path) -> bool {
        relative.components().any(|c| match c {
            Component::Normal(name) => name.to_string_lossy().starts_with('.'),
            _ => true,
        }) || relative.ancestors().any(|a| self.ignored.contains(a))
    }

    fn refresh_ignored(&mut self) {
        self.ignored = self
            .git
            .ignored_paths(&self.root)
            .unwrap_or_default()
            .into_iter()
            .collect();
    }
}

impl Poller {
    /// Adds the paths whose size or modification time changed; returns
    /// whether the whole workspace should be rescanned.
    fn poll(&mut self, paths: &mut BTreeSet<PathBuf>) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_open) >= POLL_OPEN_NOTES {
            self.last_open = now;
            let open = self.open.lock().unwrap_or_else(|e| e.into_inner()).clone();
            for path in open {
                let stamp = std::fs::metadata(&path)
                    .ok()
                    .map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()));
                let before = self.seen.insert(path.clone(), stamp);
                if before.is_some_and(|b| b != stamp) {
                    paths.insert(path);
                }
            }
        }
        if now.duration_since(self.last_all) >= POLL_WORKSPACE {
            self.last_all = now;
            return true;
        }
        false
    }
}

/// UC-004 A9: where the version from outside is kept when it meets a change
/// of the user that is not stored yet: next to the note, named after the note
/// and the time of the change (UTC), with the next free number when taken.
pub fn outside_copy_path(note: &Path, at: SystemTime) -> PathBuf {
    let seconds = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    let minutes = (seconds % 86_400) / 60;
    let stamp = format!(
        "{year:04}-{month:02}-{day:02}-{:02}{:02}",
        minutes / 60,
        minutes % 60
    );
    let stem = note.file_stem().unwrap_or_default().to_string_lossy();
    let extension = note.extension().unwrap_or_default().to_string_lossy();
    let dir = note.parent().unwrap_or_else(|| Path::new("."));
    let mut number = 1;
    loop {
        let suffix = if number == 1 {
            String::new()
        } else {
            format!("-{number}")
        };
        let candidate = dir.join(format!(
            "{stem}-changed-outside-{stamp}{suffix}.{extension}"
        ));
        if !candidate.exists() {
            return candidate;
        }
        number += 1;
    }
}

/// Year, month, and day of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saver::NoteSaver;
    use std::time::Instant;

    fn wait_for(watcher: &mut WorkspaceWatcher, expected: &WorkspaceEvent) -> Vec<WorkspaceEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            seen.extend(watcher.poll());
            if seen.contains(expected) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        seen
    }

    fn quiet(watcher: &mut WorkspaceWatcher) -> Vec<WorkspaceEvent> {
        std::thread::sleep(WATCH_COLLECT * 3);
        watcher.poll()
    }

    /// TC-004-19, TC-004-20, ADR-011: changes from outside are reported
    /// within 1 second, Underline's own writes and hidden files are not.
    #[test]
    fn uc004_reports_outside_changes_but_not_own_writes() {
        let root = std::env::temp_dir().join(format!("underline-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".underline")).unwrap();
        let plan = root.join("Plan.md");
        std::fs::write(&plan, "# Plan\n").unwrap();
        let own = OwnWrites::default();
        let saver = NoteSaver::new(own.clone());
        let mut watcher = WorkspaceWatcher::start(&root, own, Git::new(), || {});
        if watcher.is_polling() {
            return; // No events on this system; the poller is tested by hand.
        }
        quiet(&mut watcher);

        let start = Instant::now();
        std::fs::write(&plan, "# Plan\n\nfrom outside\n").unwrap();
        let seen = wait_for(&mut watcher, &WorkspaceEvent::NoteChanged(plan.clone()));
        assert!(
            seen.contains(&WorkspaceEvent::NoteChanged(plan.clone())),
            "{seen:?}"
        );
        assert!(start.elapsed() < Duration::from_secs(1));

        saver.save_now(&plan, "# Plan\n\nmine\n").unwrap();
        std::fs::write(root.join(".underline/state.yaml"), "tabs: []\n").unwrap();
        assert_eq!(quiet(&mut watcher), Vec::<WorkspaceEvent>::new());

        std::fs::remove_file(&plan).unwrap();
        let seen = wait_for(&mut watcher, &WorkspaceEvent::NoteRemoved(plan.clone()));
        assert!(
            seen.contains(&WorkspaceEvent::NoteRemoved(plan.clone())),
            "{seen:?}"
        );

        std::fs::write(root.join(SETTINGS_FILE_NAME), "default_format: asciidoc\n").unwrap();
        let seen = wait_for(&mut watcher, &WorkspaceEvent::SettingsChanged);
        assert!(seen.contains(&WorkspaceEvent::SettingsChanged), "{seen:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// TC-004-21: the copy of an outside version is named after the note and
    /// the time, with a number when the name is taken.
    #[test]
    fn uc004_tc21_outside_copy_is_named_after_note_and_time() {
        let dir = std::env::temp_dir().join(format!("underline-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 2026-11-06 14:30 UTC.
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_975_400);
        let first = outside_copy_path(&dir.join("Plan.md"), at);
        assert_eq!(first, dir.join("Plan-changed-outside-2026-11-06-1430.md"));
        std::fs::write(&first, "x").unwrap();
        let second = outside_copy_path(&dir.join("Plan.md"), at);
        assert_eq!(
            second,
            dir.join("Plan-changed-outside-2026-11-06-1430-2.md")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// ADR-011: many changes at once mean one rescan.
    #[test]
    fn many_changes_mean_a_rescan() {
        let root =
            std::env::temp_dir().join(format!("underline-watch-bulk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut watcher = WorkspaceWatcher::start(&root, OwnWrites::default(), Git::new(), || {});
        if watcher.is_polling() {
            return;
        }
        quiet(&mut watcher);
        for i in 0..150 {
            std::fs::write(root.join(format!("N{i}.md")), "x").unwrap();
        }
        // Let every event arrive first: one poll then sees them all, as the
        // window does when it was busy while git changed the files.
        std::thread::sleep(Duration::from_secs(1));
        let seen = watcher.poll();
        assert!(seen.contains(&WorkspaceEvent::Rescan), "{seen:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
