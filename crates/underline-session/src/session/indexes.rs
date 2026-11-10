//! M-16.5, UC-004 step 4, FR-041 to FR-043: the outline, the code index and
//! the figure index of the active note for the tool panel, made from the
//! blocks of the backend's text ([`underline_facade::index::indexes`], as
//! egui makes them) and kept for one revision.
//!
//! As the facade's views (ADR-024), the blocks are not parsed while the
//! session is held: the tick hands a snapshot of the text (a rope, copied
//! in no time) to a thread of its own and takes the lists in with a later
//! tick. One job runs per tab at a time, and a new one starts at most every
//! [`INDEXES_EVERY`] while the text changes (egui makes them in the frame
//! of each revision). The window gets the lists of the active tab as the
//! event `indexes`, with their revision; they stay shown until newer ones
//! come.
//!
//! A job is not cancelled: it runs to its end even when its tab closed or
//! the workspace changed meanwhile. Its result is then dropped: a tab takes
//! in only the lists of the job it started (by its number), so a tab of the
//! same note opened anew, or in another workspace, waits for its own. A job
//! that panics is logged; its tab takes no lists of that revision and
//! starts the next job only when the text changed.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ropey::Rope;
use underline_core::{NoteFormat, NoteId};
use underline_editor_core::utf16::WebOffsets;
use underline_facade::blocks::parse_blocks;
use underline_facade::index::{indexes, Entry};

use super::Session;
use crate::editor_view::{units, IndexEntryView, IndexesView};
use crate::Event;

/// How often the lists of a changing text are made again, at most: 300 ms,
/// as the sections of the source view (A-30).
pub(super) const INDEXES_EVERY: Duration = Duration::from_millis(300);

/// The lists of one tab: the newest made, the job that runs.
#[derive(Debug, Default)]
pub(super) struct TabIndexes {
    /// The newest lists, with the number they were made under.
    latest: Option<(u64, IndexesView)>,
    /// The number of the job that runs for this tab, and the revision it
    /// makes the lists of; only its lists are taken in (a tab of the same
    /// note opened anew, or in another workspace, waits for its own).
    waiting: Option<(u64, u32)>,
    /// When the last job started.
    started: Option<Instant>,
    /// The revision whose job panicked; not tried again.
    failed: Option<u32>,
}

/// What makes the lists of a text (`make_indexes`; tests put in others).
type MakeIndexes = fn(&NoteId, u32, &Rope, NoteFormat) -> IndexesView;

/// What a job sends back: its tab, its number, and the lists, or `None`
/// when it panicked.
type Made = (NoteId, u64, Option<IndexesView>);

/// The way back for the jobs of all tabs, and their numbers.
pub(super) struct IndexJobs {
    send: Sender<Made>,
    made: Receiver<Made>,
    /// What makes the lists.
    make: MakeIndexes,
    /// The number of the next job; numbers never repeat in a session, so
    /// the window is told of lists of another workspace too.
    next: u64,
}

impl Default for IndexJobs {
    fn default() -> Self {
        let (send, made) = mpsc::channel();
        Self {
            send,
            made,
            make: make_indexes,
            next: 1,
        }
    }
}

impl std::fmt::Debug for IndexJobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IndexJobs")
    }
}

/// M-16.5: the lists of the text in `rope` (revision `rev`) of `note`, with
/// their offsets in web units.
pub(super) fn make_indexes(
    note: &NoteId,
    rev: u32,
    rope: &Rope,
    format: NoteFormat,
) -> IndexesView {
    let text = rope.to_string();
    let lists = indexes(&parse_blocks(&text, format));
    let web = WebOffsets::of(rope);
    let entries = |entries: Vec<Entry>| -> Vec<IndexEntryView> {
        entries
            .into_iter()
            .map(|entry| {
                let at = rope.byte_to_char(entry.offset.min(rope.len_bytes()));
                IndexEntryView {
                    label: entry.label,
                    depth: units(entry.depth),
                    offset: units(web.to_web(rope, at)),
                }
            })
            .collect()
    };
    IndexesView {
        note: note.as_str().to_owned(),
        rev,
        outline: entries(lists.outline),
        code: entries(lists.code),
        figures: entries(lists.figures),
    }
}

impl Session {
    /// M-16.5: the tick's part of the lists: what the jobs made is taken in,
    /// a job starts when the active tab's lists are not of its text (and
    /// the last one started at least [`INDEXES_EVERY`] before `now`), and
    /// the window is told of the active tab's newest lists once.
    pub(super) fn indexes_events(&mut self, now: Instant) -> Vec<Event> {
        self.take_indexes();
        let send = self.index_jobs.send.clone();
        let make = self.index_jobs.make;
        let wake = self.wake.clone();
        let Some(open) = self.open.as_mut() else {
            return Vec::new();
        };
        let Some((note, tab)) = open.tabs.active_mut() else {
            return Vec::new();
        };
        let rev = tab.mirror.rev();
        let lists = &mut tab.indexes;
        let current = lists
            .latest
            .as_ref()
            .is_some_and(|(_, view)| view.rev == rev);
        let due = lists
            .started
            .is_none_or(|at| now.saturating_duration_since(at) >= INDEXES_EVERY);
        let failed = lists.failed == Some(rev);
        if !current && !failed && lists.waiting.is_none() && due {
            let seq = self.index_jobs.next;
            self.index_jobs.next += 1;
            let rope = tab.mirror.rope().clone();
            let format = tab.edit.format;
            let note = note.clone();
            let spawned = std::thread::Builder::new()
                .name("underline-indexes".into())
                .spawn(move || {
                    let made = catch_unwind(AssertUnwindSafe(|| make(&note, rev, &rope, format)));
                    if made.is_err() {
                        eprintln!("M-16.5: making the outline of {} panicked", note.as_str());
                    }
                    let _ = send.send((note, seq, made.ok()));
                    wake.wake();
                });
            lists.started = Some(now);
            match spawned {
                Ok(_) => lists.waiting = Some((seq, rev)),
                // Tried again after INDEXES_EVERY; egui has no thread here
                // and so no text for it.
                Err(error) => eprintln!("M-16.5: the outline could not be made: {error}"),
            }
        }
        match &tab.indexes.latest {
            Some((seq, view)) if self.told.indexes != Some(*seq) => {
                self.told.indexes = Some(*seq);
                vec![Event::Indexes(view.clone())]
            }
            _ => Vec::new(),
        }
    }

    /// Takes in the lists the jobs made, for the tabs still open.
    fn take_indexes(&mut self) {
        while let Ok((note, seq, made)) = self.index_jobs.made.try_recv() {
            let Some(open) = self.open.as_mut() else {
                continue;
            };
            let Some(index) = open.tabs.find(&note) else {
                continue;
            };
            let lists = &mut open.tabs.get_mut(index).indexes;
            let Some((waited, rev)) = lists.waiting.filter(|(waited, _)| *waited == seq) else {
                // The job of a tab closed meanwhile, or of another workspace.
                continue;
            };
            lists.waiting = None;
            match made {
                // One job runs at a time per tab: it made the newest lists.
                Some(view) => lists.latest = Some((waited, view)),
                None => lists.failed = Some(rev),
            }
        }
    }
}

#[cfg(test)]
impl Session {
    /// For the tests: the lists are made by `make` from now on.
    pub(super) fn make_indexes_with(&mut self, make: MakeIndexes) {
        self.index_jobs.make = make;
    }

    /// For the tests: whether a job runs for the tab of `note`.
    pub(super) fn open_tab_indexes_waiting(&self, note: &NoteId) -> bool {
        let open = self.open.as_ref().expect("a workspace is open");
        let tab = open
            .tabs
            .get(open.tabs.find(note).expect("the tab is open"));
        tab.indexes.waiting.is_some()
    }
}

#[cfg(test)]
mod tests;
