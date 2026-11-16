//! UC-022 Replace in Workspace (FR-074, FR-075, NFR-017): the places a
//! replacement changes in the notes of the workspace, the notes and places
//! the user leaves out, the replacement itself and its undo.
//!
//! The places are found on the thread of [`underline_core::Searcher`]
//! (BR-007, NFR-002), never while the session is held, from the texts as
//! the search of the workspace reads them: a note open in a tab with its
//! changes not yet saved (BR-001). They are found again when the text, the
//! replacement, the switches, the notes or a text open in a tab changed
//! (A4); a place left out stays left out while it is still found, known by
//! its line and where in the line it starts ([`PlaceKey`]).
//!
//! Replacing (step 5) changes a note only when the text it holds is the
//! one whose places were shown, or holds the same places (BR-004): a note
//! changed meanwhile by another program is never overwritten. A closed note
//! is written atomically (ADR-005, BR-006); a note open in a tab is written
//! too and changes in its tab through the mirror, as one undo step of its
//! own (ADR-025, BR-005). The status bar offers to undo the whole
//! replacement while it shows the report (A6, OQ-191).
//!
//! BR-005, UC-026 BR-001: before a note changes, a snapshot with the cause
//! "Replace in workspace" keeps its content; a note whose snapshot cannot
//! be taken is not changed and is named in the status. The undo of the
//! status bar (A6) gives back the same content, kept in memory while the
//! report shows, and keeps what it replaces as a snapshot with the cause
//! Restore (OQ-289). The texts and choices the specification leaves open
//! are OQ-228 to OQ-235.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A4: how often the places are found again while the notes change.
const RESEARCH_EVERY: Duration = Duration::from_millis(300);

use std::fmt::Write as _;

use underline_core::{
    note_places, replace_places, NoteEntry, NoteFormat, NoteId, NotePlace, NotePlaces, NoteWriter,
    PlaceKey, PlacesRequest, PlacesResult, Query, Replacement, SearchReading, SnapshotCause,
    SnapshotStore, Switches, Timestamp, SHOWN_REPLACE_PLACES,
};

use super::Session;
use crate::editor_view::{units, SearchSwitches};
use crate::replace_view::{
    ChosenView, ReplaceNoteView, ReplacePlaceView, ReplaceResultView, ReplaceView,
};
use crate::{Event, StatusOffer};

/// "1 place", "2 places", "10,000 places".
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{} {thing}s", thousands(n))
    }
}

/// `n` with a comma between thousands ("10,000"), as the texts write it.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The titles of `notes` joined: "A", "A and B", "A, B and C".
fn titles(notes: &[String]) -> String {
    match notes {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The replace of the workspace: what the window asked, the newest places,
/// what is left out, and the undo of the last replacement.
#[derive(Debug, Default)]
pub(super) struct Replace {
    /// The replace is open: its places are found again when notes change.
    open: bool,
    text: String,
    replacement: String,
    switches: SearchSwitches,
    /// A2: what is wrong with the text or the replacement.
    problem: Option<String>,
    /// The number of the newest `replace_set`.
    asked: u32,
    /// The newest places for `asked`, with their number.
    result: Option<(u32, Arc<PlacesResult>)>,
    /// The number of the last places taken in; grows for the whole session.
    serial: u32,
    /// The request and the state of the notes the searcher was asked for
    /// last, and when.
    stamp: Option<(u32, u64)>,
    asked_at: Option<Instant>,
    /// The texts of the open tabs sent last, by the revision of each.
    texts: HashMap<NoteId, (u32, String)>,
    /// Step 3: the notes left out.
    left_notes: HashSet<NoteId>,
    /// Step 3: the places left out.
    left_places: HashSet<(NoteId, PlaceKey)>,
    /// What the window was told of last: the request, the problem and the
    /// places.
    told: Option<(u32, Option<String>, Option<u32>)>,
    /// A6: the undo offered while the report of the replacement shows.
    undo: Option<UndoReplace>,
}

/// A6: what the undo of a replacement gives back.
#[derive(Debug)]
struct UndoReplace {
    /// The report it belongs to.
    status: String,
    notes: Vec<ReplacedNote>,
}

/// A note a replacement changed.
#[derive(Debug)]
struct ReplacedNote {
    id: NoteId,
    path: PathBuf,
    before: String,
    after: String,
}

/// The smallest edit that makes `new` of `old`: the bytes of `old` that
/// differ, and what takes their place, so the tab's history holds one step
/// that changes only what changed.
pub(super) fn span_edit(old: &str, new: &str) -> (Range<usize>, String) {
    let mut start = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let most = (old.len() - start).min(new.len() - start);
    let mut same = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(most)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - same) || !new.is_char_boundary(new.len() - same) {
        same -= 1;
    }
    (
        start..old.len() - same,
        new[start..new.len() - same].to_owned(),
    )
}

/// The query and the replacement of a replace, or what is wrong (A2).
fn query_of(
    text: &str,
    switches: SearchSwitches,
    replacement: &str,
) -> Result<(Query, Replacement), String> {
    let query = Query::new(text, Switches::from(switches), SearchReading::OneText)
        .map_err(|problem| problem.to_string())?;
    let with = query
        .replacement(replacement)
        .map_err(|problem| problem.to_string())?;
    Ok((query, with))
}

/// A6: notes changed since, notes that could not be changed, notes done.
#[derive(Debug, Default)]
struct Report {
    done: Vec<ReplacedNote>,
    places: usize,
    changed: Vec<String>,
    failed: Vec<String>,
}

/// What one note's text becomes (step 5, BR-004).
enum Plan {
    /// The note does not hold the places shown any more.
    Changed,
    /// Every place of it is left out.
    Nothing,
    /// It becomes `after`, with `places` places replaced.
    Write { after: String, places: usize },
}

/// Step 5, BR-004: what the note holding `now` becomes, when its places
/// were shown from `seen` (`count` in all, `shown` of them shown, `left`
/// left out).
fn plan_text(
    (query, with): (&Query, &Replacement),
    seen: &str,
    (count, shown): (usize, &[NotePlace]),
    left: &HashSet<PlaceKey>,
    now: &str,
) -> Plan {
    let places = note_places(query, with, now);
    let same = seen == now
        || (count == shown.len()
            && places.len() == shown.len()
            && places
                .iter()
                .zip(shown)
                .all(|(now, seen)| now.key == seen.key && now.replaced == seen.replaced));
    if !same {
        return Plan::Changed;
    }
    let chosen: Vec<(Range<usize>, String)> = places
        .into_iter()
        .enumerate()
        .filter(|(i, place)| *i >= shown.len() || !left.contains(&place.key))
        .map(|(_, place)| (place.range, place.replaced))
        .collect();
    if chosen.is_empty() {
        return Plan::Nothing;
    }
    Plan::Write {
        after: replace_places(now, &chosen),
        places: chosen.len(),
    }
}

/// What is done with a note that is not open in a tab, without the session.
#[derive(Debug)]
enum Work {
    /// Step 5: its chosen places replaced.
    Replace {
        seen: Arc<str>,
        count: usize,
        shown: Vec<NotePlace>,
        left: HashSet<PlaceKey>,
    },
    /// A6: its text from before given back.
    GiveBack { before: String, after: String },
}

/// A note not open in a tab, to be changed by [`ReplaceJob::run`].
#[derive(Debug)]
struct Closed {
    id: NoteId,
    path: PathBuf,
    work: Work,
}

/// UC-022 steps 5–6, A5, A6, ADR-005: the part of a replacement or of its
/// undo that reads and writes the notes not open in a tab. It runs without
/// the session ([`ReplaceJob::run`]); the notes open in tabs were changed
/// already, through their mirrors, while the session was held.
#[derive(Debug)]
pub struct ReplaceJob {
    /// The undo of a replacement (A6), not a replacement.
    undoing: bool,
    query: Option<(Query, Replacement)>,
    writer: NoteWriter,
    /// UC-026: the snapshots of the workspace and the time they are taken.
    snapshots: Option<(SnapshotStore, Timestamp)>,
    closed: Vec<Closed>,
    report: Report,
}

/// A [`ReplaceJob`] that ran, for [`Session::replace_finish`].
#[derive(Debug)]
pub struct ReplaceDone(ReplaceJob);

/// Writes `new` to `path` atomically when the file still holds `old`,
/// after `keep` kept `old` (UC-026 BR-001); whether it did.
fn write_if(
    writer: &NoteWriter,
    path: &Path,
    (old, new): (&str, &str),
    keep: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    match std::fs::read_to_string(path) {
        Ok(now) if now == old => {}
        Ok(_) => return Ok(false),
        Err(error) => return Err(error.to_string()),
    }
    keep()?;
    writer.save_now(path, new)?;
    Ok(true)
}

/// UC-026 BR-001: a snapshot of `content`, the content of the note at
/// `path`, for `cause`; none without snapshots.
fn keep_snapshot(
    snapshots: Option<&(SnapshotStore, Timestamp)>,
    path: &Path,
    content: &str,
    cause: SnapshotCause,
) -> Result<(), String> {
    let Some((store, at)) = snapshots else {
        return Ok(());
    };
    let note_file = store.note_file(path).map_err(|e| e.to_string())?;
    store
        .take(&note_file, content, cause, *at)
        .map(drop)
        .map_err(|e| e.to_string())
}

impl ReplaceJob {
    /// Reads, compares and writes each note not open in a tab: a note that
    /// changed since is left as it is, one that cannot be written stays
    /// whole (BR-006).
    #[must_use]
    pub fn run(mut self) -> ReplaceDone {
        for note in std::mem::take(&mut self.closed) {
            let title = note.id.title();
            let outcome = match &note.work {
                Work::Replace {
                    seen,
                    count,
                    shown,
                    left,
                } => self.replace_closed(&note, seen, (*count, shown), left),
                Work::GiveBack { before, after } => {
                    let keep = || {
                        keep_snapshot(
                            self.snapshots.as_ref(),
                            &note.path,
                            after,
                            SnapshotCause::Restore,
                        )
                    };
                    write_if(&self.writer, &note.path, (after, before), keep).map(|done| {
                        if done {
                            ClosedOutcome::Done(
                                ReplacedNote {
                                    id: note.id.clone(),
                                    path: note.path.clone(),
                                    before: after.clone(),
                                    after: before.clone(),
                                },
                                0,
                            )
                        } else {
                            ClosedOutcome::Changed
                        }
                    })
                }
            };
            match outcome {
                Ok(ClosedOutcome::Done(done, places)) => {
                    self.report.places += places;
                    self.report.done.push(done);
                }
                Ok(ClosedOutcome::Changed) => self.report.changed.push(title),
                Ok(ClosedOutcome::Nothing) => {}
                Err(why) => self
                    .report
                    .failed
                    .push(format!("{title} could not be changed: {why}")),
            }
        }
        ReplaceDone(self)
    }

    /// Step 5 for a closed note: replaced, changed since, or every place of
    /// it left out (test-monkey: that is no change).
    fn replace_closed(
        &self,
        note: &Closed,
        seen: &str,
        shown: (usize, &[NotePlace]),
        left: &HashSet<PlaceKey>,
    ) -> Result<ClosedOutcome, String> {
        let Some((query, with)) = &self.query else {
            return Ok(ClosedOutcome::Nothing);
        };
        let now = std::fs::read_to_string(&note.path).map_err(|error| error.to_string())?;
        let (after, places) = match plan_text((query, with), seen, shown, left, &now) {
            Plan::Write { after, places } => (after, places),
            Plan::Changed => return Ok(ClosedOutcome::Changed),
            Plan::Nothing => return Ok(ClosedOutcome::Nothing),
        };
        let keep = || {
            keep_snapshot(
                self.snapshots.as_ref(),
                &note.path,
                &now,
                SnapshotCause::ReplaceInWorkspace,
            )
        };
        if !write_if(&self.writer, &note.path, (&now, &after), keep)? {
            return Ok(ClosedOutcome::Changed);
        }
        Ok(ClosedOutcome::Done(
            ReplacedNote {
                id: note.id.clone(),
                path: note.path.clone(),
                before: now,
                after,
            },
            places,
        ))
    }
}

/// What became of a closed note in [`ReplaceJob::run`].
enum ClosedOutcome {
    /// Written, with how many places.
    Done(ReplacedNote, usize),
    /// It changed since its places were shown.
    Changed,
    /// Every place of it was left out.
    Nothing,
}

impl Session {
    /// UC-022 step 1, 2, A1, A2: the replace holds `text` (as one text),
    /// `replacement` and the search switches `switches`; its places are
    /// found again on the search thread. The number of this request, which
    /// the result names. A change of the text or the switches forgets what
    /// was left out.
    pub fn replace_set(&mut self, text: &str, replacement: &str, switches: SearchSwitches) -> u32 {
        let replace = &mut self.replace;
        if replace.text != text || replace.switches != switches {
            replace.left_notes.clear();
            replace.left_places.clear();
        }
        replace.open = true;
        text.clone_into(&mut replace.text);
        replacement.clone_into(&mut replace.replacement);
        replace.switches = switches;
        replace.asked = replace.asked.wrapping_add(1);
        replace.problem = if text.is_empty() {
            None
        } else {
            query_of(text, switches, replacement).err()
        };
        if text.is_empty() || replace.problem.is_some() {
            replace.result = None;
        }
        self.run_replace();
        self.replace.asked
    }

    /// UC-022 A8: the replace closed; its places are no longer found.
    pub fn replace_close(&mut self) {
        let undo = self.replace.undo.take();
        self.replace = super::replace::Replace {
            serial: self.replace.serial,
            asked: self.replace.asked,
            undo,
            ..super::replace::Replace::default()
        };
    }

    /// UC-022 step 2: the notes of the places `serial` from the `from`th
    /// on, a page of about [`ReplaceResultView::PAGE`] places; `None` when
    /// they are no longer the newest.
    #[must_use]
    pub fn replace_page(&self, serial: u32, from: u32) -> Option<Vec<ReplaceNoteView>> {
        let (taken, result) = self.replace.result.as_ref()?;
        if *taken != serial {
            return None;
        }
        Some(self.replace_page_of(result, from as usize))
    }

    /// UC-022 step 3, BR-008: leaves the note `note` out (`out`), or takes
    /// it in again; with `at`, only its place starting at that byte. How
    /// many places in how many notes are chosen now, or `None` when the
    /// places `serial` are no longer the newest and nothing was taken.
    pub fn replace_leave_out(
        &mut self,
        serial: u32,
        note: &NoteId,
        at: Option<u32>,
        out: bool,
    ) -> Option<ChosenView> {
        let result = match &self.replace.result {
            Some((taken, result)) if *taken == serial && result.asked == self.replace.asked => {
                Arc::clone(result)
            }
            _ => return None,
        };
        let places = result.notes.iter().find(|n| n.id == *note)?;
        let replace = &mut self.replace;
        match at {
            None => {
                if out {
                    replace.left_notes.insert(note.clone());
                } else {
                    replace.left_notes.remove(note);
                }
            }
            Some(at) => {
                let place = places
                    .places
                    .iter()
                    .find(|p| p.range.start == at as usize)?;
                let key = (note.clone(), place.key);
                if out {
                    replace.left_places.insert(key);
                } else {
                    replace.left_places.remove(&key);
                }
            }
        }
        Some(self.chosen_view(&result))
    }

    /// Step 3: how many places in how many notes are chosen, as the list
    /// and the button "Replace" say it.
    fn chosen_view(&self, result: &PlacesResult) -> ChosenView {
        let replace = &self.replace;
        let (mut places, mut notes) = (0, 0);
        for note in &result.notes {
            if replace.left_notes.contains(&note.id) {
                continue;
            }
            let left = note
                .places
                .iter()
                .filter(|p| replace.left_places.contains(&(note.id.clone(), p.key)))
                .count();
            let chosen = note.count - left;
            if chosen > 0 {
                places += chosen;
                notes += 1;
            }
        }
        ChosenView {
            places: units(places),
            notes: units(notes),
            summary: format!(
                "{} of {} chosen",
                thousands(places),
                count(result.places, "place")
            ),
            action: if places == 0 {
                "Replace".to_owned()
            } else {
                format!(
                    "Replace {} in {}",
                    count(places, "place"),
                    count(notes, "note")
                )
            },
        }
    }

    /// The places `serial`, while they answer the request held now and the
    /// replace is open; else the status says to look again.
    fn current_places(&mut self, serial: u32) -> Option<Arc<PlacesResult>> {
        let replace = &self.replace;
        match &replace.result {
            Some((taken, result))
                if *taken == serial
                    && result.asked == replace.asked
                    && replace.open
                    && replace.problem.is_none() =>
            {
                Some(Arc::clone(result))
            }
            _ => {
                "The places changed while you looked at them; look at them again before you replace"
                    .clone_into(&mut self.status);
                None
            }
        }
    }

    /// UC-022 steps 4–6, A3, A5, BR-004 to BR-006: replaces the chosen
    /// places of the places `serial`, in every note or only in `only`
    /// (A3), and says what was done in the status (also the answer).
    ///
    /// BR-004 is kept by comparing texts: a note is changed only while it
    /// holds the text its places were shown from, or the same places (a
    /// note open in a tab also only while its file holds what the tab knows
    /// of it); any other note is left as it is and named. The places must
    /// answer the request held now (`replace_set`), else nothing changes.
    ///
    /// This runs the three parts in one go; the window's command runs
    /// [`Session::replace_plan`] and [`Session::replace_finish`] with the
    /// session held and [`ReplaceJob::run`] without it (no file I/O of
    /// closed notes under the session's lock).
    pub fn replace_confirm(&mut self, serial: u32, only: Option<&NoteId>) -> String {
        match self.replace_plan(serial, only) {
            Some(job) => self.replace_finish(job.run()),
            None => self.status.clone(),
        }
    }

    /// UC-022 A6: [`Session::replace_undo_plan`], run and finished in one go.
    pub fn replace_undo(&mut self) -> String {
        match self.replace_undo_plan() {
            Some(job) => self.replace_finish(job.run()),
            None => self.status.clone(),
        }
    }

    /// Steps 4–5, first part, with the session held: checks that the places
    /// `serial` are the newest, changes the notes open in tabs (through
    /// their mirrors, ADR-025), and gives the closed notes to a job. `None`
    /// when nothing is to be done; the status says why.
    pub fn replace_plan(&mut self, serial: u32, only: Option<&NoteId>) -> Option<ReplaceJob> {
        let result = self.current_places(serial)?;
        self.settle_saves();
        let Ok((query, with)) = query_of(
            &self.replace.text,
            self.replace.switches,
            &self.replace.replacement,
        ) else {
            return None;
        };
        let mut report = Report::default();
        let mut closed = Vec::new();
        for note in &result.notes {
            if only.is_some_and(|only| *only != note.id)
                || self.replace.left_notes.contains(&note.id)
            {
                continue;
            }
            let left: HashSet<PlaceKey> = self
                .replace
                .left_places
                .iter()
                .filter(|(id, _)| *id == note.id)
                .map(|(_, key)| *key)
                .collect();
            if self.has_tab(&note.id) {
                self.replace_in_tab(note, (&query, &with), &left, &mut report);
                continue;
            }
            let path = match self
                .open
                .as_ref()
                .map(|open| open.workspace.note_path(&note.id))
            {
                Some(Ok(path)) => path,
                Some(Err(error)) => {
                    report
                        .failed
                        .push(format!("{} could not be changed: {error}", note.id.title()));
                    continue;
                }
                None => return None,
            };
            closed.push(Closed {
                id: note.id.clone(),
                path,
                work: Work::Replace {
                    seen: Arc::clone(&note.text),
                    count: note.count,
                    shown: note.places.clone(),
                    left,
                },
            });
        }
        Some(ReplaceJob {
            undoing: false,
            query: Some((query, with)),
            writer: self.saver.writer(),
            snapshots: self.replace_snapshots(),
            closed,
            report,
        })
    }

    /// A6, first part, with the session held: the notes open in tabs get
    /// their text from before back (unless changed since); the closed ones
    /// go to a job. `None` without an undo.
    pub fn replace_undo_plan(&mut self) -> Option<ReplaceJob> {
        self.settle_saves();
        let undo = self.replace.undo.take()?;
        let mut report = Report::default();
        let mut closed = Vec::new();
        for note in undo.notes {
            if self.has_tab(&note.id) {
                match self.store_in_tab(
                    &note.id,
                    (&note.after, &note.before),
                    SnapshotCause::Restore,
                ) {
                    Ok(true) => report.done.push(note),
                    Ok(false) => report.changed.push(note.id.title()),
                    Err(why) => report
                        .failed
                        .push(format!("{} could not be changed: {why}", note.id.title())),
                }
                continue;
            }
            let path = self
                .open
                .as_ref()
                .and_then(|open| open.workspace.note_path(&note.id).ok())
                .unwrap_or_else(|| note.path.clone());
            closed.push(Closed {
                id: note.id,
                path,
                work: Work::GiveBack {
                    before: note.before,
                    after: note.after,
                },
            });
        }
        Some(ReplaceJob {
            undoing: true,
            query: None,
            writer: self.saver.writer(),
            snapshots: self.replace_snapshots(),
            closed,
            report,
        })
    }

    /// Steps 5–6 and A6, last part, with the session held: the status says
    /// what was done (also the answer), the undo is offered after a
    /// replacement, and the places are found again.
    pub fn replace_finish(&mut self, done: ReplaceDone) -> String {
        let ReplaceJob {
            undoing, report, ..
        } = done.0;
        let mut status = if undoing {
            format!(
                "Took back the replacement in {}",
                count(report.done.len(), "note")
            )
        } else if report.done.is_empty() && report.changed.is_empty() && report.failed.is_empty() {
            "Nothing was replaced: every place is left out".to_owned()
        } else if report.done.is_empty() {
            // OQ-228: no "Replaced 0 places in 0 notes" before the reasons.
            "Nothing was replaced".to_owned()
        } else {
            format!(
                "Replaced {} in {}",
                count(report.places, "place"),
                count(report.done.len(), "note")
            )
        };
        if !report.changed.is_empty() {
            let (one, many) = if undoing {
                (
                    "was changed since and is left as it is",
                    "were changed since and are left as they are",
                )
            } else {
                (
                    "was changed meanwhile and is left as it is",
                    "were changed meanwhile and are left as they are",
                )
            };
            let verb = if report.changed.len() == 1 { one } else { many };
            let _ = write!(status, "; {} {verb}", titles(&report.changed));
        }
        for failure in &report.failed {
            let _ = write!(status, "; {failure}");
        }
        self.status.clone_from(&status);
        // UC-026 A4: the replacement took snapshots.
        self.snapshot_taken();
        self.reload_tabs_opened_meanwhile(&report.done);
        self.replace.undo = (!undoing && !report.done.is_empty()).then(|| UndoReplace {
            status: status.clone(),
            notes: report.done,
        });
        // Step 6: the replaced places are no longer listed.
        self.replace.stamp = None;
        self.replace.asked_at = None;
        self.refresh_overview();
        self.run_replace();
        status
    }

    /// UC-026: the snapshots of the open workspace, taken now.
    fn replace_snapshots(&self) -> Option<(SnapshotStore, Timestamp)> {
        let open = self.open.as_ref()?;
        Some((
            SnapshotStore::new(open.workspace.root()),
            (self.snapshot_clock())(),
        ))
    }

    /// A6: the undo is offered while the report of the replacement shows.
    pub(super) fn replace_offer(&mut self) -> Option<StatusOffer> {
        let status = &self.status;
        self.replace.undo.take_if(|undo| undo.status != *status);
        self.replace.undo.as_ref().map(|_| StatusOffer::UndoReplace)
    }

    /// M-21.3: another workspace opened: the replace starts anew there.
    pub(super) fn replace_switched(&mut self) {
        self.replace.texts.clear();
        self.replace.result = None;
        self.replace.stamp = None;
        self.replace.left_notes.clear();
        self.replace.left_places.clear();
        self.replace.undo = None;
    }

    /// The tick's part of the replace: the places taken in, found again
    /// when needed, and the window told of a changed problem or result.
    pub(super) fn replace_events(&mut self) -> Vec<Event> {
        self.run_replace();
        let told = Some((
            self.replace.asked,
            self.replace.problem.clone(),
            self.replace.result.as_ref().map(|(serial, _)| *serial),
        ));
        if self.replace.told == told || (!self.replace.open && self.replace.told.is_none()) {
            return Vec::new();
        }
        self.replace.told = told;
        vec![Event::Replace(self.replace_view())]
    }

    /// UC-022 step 2, A4: takes in the newest places when they answer the
    /// request held now, and asks again when the request, the notes or a
    /// text open in a tab changed.
    fn run_replace(&mut self) {
        let Some(open) = &self.open else {
            return;
        };
        if let Some(result) = open.searcher.poll_places() {
            if result.asked == self.replace.asked {
                self.replace.serial = self.replace.serial.wrapping_add(1);
                self.replace.result = Some((self.replace.serial, Arc::new(result)));
            }
        }
        let replace = &mut self.replace;
        if !replace.open || replace.text.is_empty() || replace.problem.is_some() {
            replace.stamp = None;
            replace.texts.clear();
            return;
        }
        let revisions = open
            .tabs
            .iter()
            .fold(open.search_generation, |sum, (_, tab)| {
                sum.wrapping_mul(31)
                    .wrapping_add(u64::from(tab.mirror.rev()))
            });
        let stamp = (replace.asked, revisions);
        if replace.stamp == Some(stamp) {
            return;
        }
        // A4: while the notes change under the same request (typing in a
        // tab), the places are found again at most every 300 ms.
        let same_request = replace
            .stamp
            .is_some_and(|(asked, _)| asked == replace.asked);
        if same_request
            && replace
                .asked_at
                .is_some_and(|at| at.elapsed() < RESEARCH_EVERY)
        {
            return;
        }
        let formats: HashMap<&NoteId, NoteFormat> = open
            .workspace
            .listing()
            .notes
            .iter()
            .map(|n| (&n.id, n.format))
            .collect();
        let notes = open
            .overview
            .iter()
            .filter_map(|entry| {
                formats.get(&entry.id).map(|format| NoteEntry {
                    id: entry.id.clone(),
                    format: *format,
                })
            })
            .collect();
        let cached = &mut replace.texts;
        cached.retain(|id, _| open.tabs.find(id).is_some());
        let texts = open
            .tabs
            .iter()
            .map(|(id, tab)| {
                let rev = tab.mirror.rev();
                let entry = cached
                    .entry(id.clone())
                    .or_insert_with(|| (rev, tab.mirror.text()));
                if entry.0 != rev {
                    *entry = (rev, tab.mirror.text());
                }
                (id.clone(), entry.1.clone())
            })
            .collect();
        open.searcher.find_places(PlacesRequest {
            asked: replace.asked,
            text: replace.text.clone(),
            replacement: replace.replacement.clone(),
            switches: replace.switches.into(),
            notes,
            open: texts,
        });
        replace.stamp = Some(stamp);
        replace.asked_at = Some(Instant::now());
    }

    /// UC-022 step 2: the replace as the window shows it.
    fn replace_view(&self) -> ReplaceView {
        ReplaceView {
            asked: self.replace.asked,
            problem: self.replace.problem.clone(),
            result: self
                .replace
                .result
                .as_ref()
                .map(|(serial, result)| ReplaceResultView {
                    serial: *serial,
                    asked: result.asked,
                    places: units(result.places),
                    notes: units(result.notes.len()),
                    summary: format!(
                        "{} in {}",
                        count(result.places, "place"),
                        count(result.notes.len(), "note")
                    ),
                    more: (result.places > result.shown).then(|| {
                        let more = result.places - result.shown;
                        format!(
                            "The first {} places are shown; {} more are not shown and are replaced too.",
                            thousands(SHOWN_REPLACE_PLACES),
                            thousands(more)
                        )
                    }),
                    list: self.replace_page_of(result, 0),
                    chosen: self.chosen_view(result),
                    listed: units(result.notes.len()),
                    unreadable: (result.unreadable > 0).then(|| {
                        format!("{} could not be searched.", count(result.unreadable, "note"))
                    }),
                }),
        }
    }

    /// A page of the notes of `result` from the `from`th on.
    fn replace_page_of(&self, result: &PlacesResult, from: usize) -> Vec<ReplaceNoteView> {
        let mut page = Vec::new();
        let mut places = 0;
        for note in result.notes.iter().skip(from) {
            if places >= ReplaceResultView::PAGE {
                break;
            }
            places += note.shown.len().max(1);
            page.push(self.note_view(note));
        }
        page
    }

    /// UC-022 step 2, 3: a note with its places shown.
    fn note_view(&self, note: &NotePlaces) -> ReplaceNoteView {
        let replace = &self.replace;
        ReplaceNoteView {
            note: note.id.as_str().to_owned(),
            title: note.id.title(),
            folder: note.id.folder().map(str::to_owned),
            count: count(note.count, "place"),
            left_out: replace.left_notes.contains(&note.id),
            places: note
                .places
                .iter()
                .zip(&note.shown)
                .map(|(place, shown)| ReplacePlaceView {
                    at: units(place.range.start),
                    line: units(place.line),
                    before: shown.before.clone(),
                    old: shown.old.clone(),
                    new: shown.new.clone(),
                    after: shown.after.clone(),
                    left_out: replace.left_places.contains(&(note.id.clone(), place.key)),
                })
                .collect(),
        }
    }

    /// Step 5 for a note open in a tab: its chosen places replaced in its
    /// text, when it still holds the places shown, and stored at once.
    fn replace_in_tab(
        &mut self,
        note: &NotePlaces,
        query: (&Query, &Replacement),
        left: &HashSet<PlaceKey>,
        report: &mut Report,
    ) {
        let title = note.id.title();
        let Some(now) = self.tab_text(&note.id) else {
            report.changed.push(title);
            return;
        };
        match plan_text(query, &note.text, (note.count, &note.places), left, &now) {
            Plan::Changed => report.changed.push(title),
            Plan::Nothing => {}
            Plan::Write { after, places } => {
                match self.store_in_tab(&note.id, (&now, &after), SnapshotCause::ReplaceInWorkspace)
                {
                    Ok(true) => {
                        report.places += places;
                        report.done.push(ReplacedNote {
                            id: note.id.clone(),
                            path: self.tab_path(&note.id).unwrap_or_default(),
                            before: now,
                            after,
                        });
                    }
                    Ok(false) => report.changed.push(title),
                    Err(why) => report
                        .failed
                        .push(format!("{title} could not be changed: {why}")),
                }
            }
        }
    }

    /// BR-004, ADR-025 (test-monkey): a note the job wrote may have been
    /// opened in a tab while the job ran, with the text from before; the
    /// watcher leaves Underline's own writes out, so the tab is given the
    /// written text here, unless the user changed it already (then the
    /// change from outside is handled as any other, UC-004 A9).
    fn reload_tabs_opened_meanwhile(&mut self, done: &[ReplacedNote]) {
        for note in done {
            let Some(open) = self.open.as_mut() else {
                return;
            };
            let Some(index) = open.tabs.find(&note.id) else {
                continue;
            };
            let tab = open.tabs.get_mut(index);
            if tab.on_disk == note.after || tab.mirror.text() != tab.on_disk {
                continue;
            }
            tab.reload(note.after.clone());
            self.changed.push(note.id.clone());
            self.views_dirty = true;
        }
    }

    /// The mirror's text of the tab of `note`; `None` while it waits for a
    /// text from outside to be taken (ADR-025 section 3.6).
    fn tab_text(&self, note: &NoteId) -> Option<String> {
        let open = self.open.as_ref()?;
        let tab = open.tabs.get(open.tabs.find(note)?);
        (!tab.mirror.reload_unconfirmed()).then(|| tab.mirror.text())
    }

    /// The file of the tab of `note`.
    fn tab_path(&self, note: &NoteId) -> Option<PathBuf> {
        let open = self.open.as_ref()?;
        Some(open.tabs.get(open.tabs.find(note)?).path.clone())
    }

    /// BR-004 to BR-006, ADR-025: the tab of `note`, holding `old`, gets
    /// `new` as one undo step of its own through its mirror, and is stored
    /// at once; when it cannot be stored, the change is taken back. Whether
    /// it was done: not when the tab no longer holds `old`, its note was
    /// deleted, or its file was changed elsewhere since the tab read it.
    /// UC-026 BR-001: `old` is kept as a snapshot for `cause` first; when it
    /// cannot be, the note is not changed.
    fn store_in_tab(
        &mut self,
        note: &NoteId,
        (old, new): (&str, &str),
        cause: SnapshotCause,
    ) -> Result<bool, String> {
        let started = Instant::now();
        let Some(open) = self.open.as_ref() else {
            return Ok(false);
        };
        let Some(index) = open.tabs.find(note) else {
            return Ok(false);
        };
        let tab = open.tabs.get(index);
        if tab.deleted || tab.mirror.reload_unconfirmed() || tab.mirror.text() != old {
            return Ok(false);
        }
        let path = tab.path.clone();
        match std::fs::read_to_string(&path) {
            Ok(disk) if disk == tab.on_disk => {}
            Ok(_) => return Ok(false),
            Err(error) => return Err(error.to_string()),
        }
        keep_snapshot(self.replace_snapshots().as_ref(), &path, old, cause)?;
        let (range, text) = span_edit(old, new);
        self.change_mirror(note, &[(range, text)], true, false)
            .map_err(|error| error.message)?;
        if let Err(why) = self.saver.save_now(&path, new) {
            // A5: the tab stays as it was.
            let (range, text) = span_edit(new, old);
            if let Err(error) = self.change_mirror(note, &[(range, text)], true, false) {
                eprintln!(
                    "UC-022 A5: {} could not be given its text back: {}",
                    note.as_str(),
                    error.message
                );
            }
            return Err(why);
        }
        if let Some(open) = self.open.as_mut() {
            if let Some(index) = open.tabs.find(note) {
                let tab = open.tabs.get_mut(index);
                new.clone_into(&mut tab.on_disk);
                tab.saving_text = None;
                tab.autosave.stored(started);
                tab.error = None;
            }
        }
        self.views_dirty = true;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
