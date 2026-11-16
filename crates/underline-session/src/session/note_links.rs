//! FR-078, FR-079, UC-004 step 4, BR-015, NFR-019: the backlinks and the
//! outgoing links of the active note for the tool panel, next to its
//! outline (`indexes.rs`).
//!
//! The links of every note are found with the scan the rename uses
//! ([`found_links`], UC-006), so the links a rename changes and the links
//! the tool panel lists are the same: none in code, diagrams or comments.
//! Where a link leads is read as following it reads it
//! ([`resolve_link_with`], UC-004 BR-005), with the notes known instead of
//! asking the disk.
//!
//! A thread of its own keeps the links of every note of the workspace and
//! answers the tick's questions: it looks at the files again (their time
//! and size; only a changed file is read and scanned again), takes the text
//! of each note open in a tab instead of its file (BR-015, with changes not
//! stored yet), and makes the lists of the active note. The session is not
//! held meanwhile. The tick asks again when the active note, the text of a
//! tab or the notes of the workspace changed, at most every
//! [`LINKS_EVERY`], and looks at the files every [`LINKS_SWEEP`] while a
//! note is shown, so a change made by another program, an agent or
//! Underline itself shows within 2 seconds in 10,000 notes (NFR-019).
//!
//! One question is answered at a time; newer ones replace those waiting. A
//! question that panics is logged and answered with nothing; the next one
//! is asked as usual. The window gets the lists of the active note as the
//! event `links`, only when they changed.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use ropey::Rope;
use underline_core::{
    asciidoc_anchor, heading_anchor, resolve_link_with, LinkTarget, NoteFormat, NoteId,
};
use underline_editor_core::utf16::WebOffsets;
use underline_facade::blocks::{parse_blocks, BlockKind};
use underline_facade::rename::{found_links, FoundLink, LinkMarkup};

use super::{Session, Wake};
use crate::editor_view::units;
use crate::links_view::{
    BacklinkNoteView, BacklinkView, HeadingView, LinkKindName, LinkPlace, LinksView,
    OutgoingLinkView,
};
use crate::Event;

/// How often the links are asked again while texts change, at most: 300 ms,
/// as the outline (M-16.5).
pub(super) const LINKS_EVERY: Duration = Duration::from_millis(300);

/// How often the files are looked at again while a note is shown and
/// nothing else asked: 1 s, so a change shows within 2 s (NFR-019).
pub(super) const LINKS_SWEEP: Duration = Duration::from_secs(1);

/// How many characters of the line a backlink shows before the link.
const BEFORE_CHARS: usize = 40;
/// How many characters of the link it shows.
const LINK_CHARS: usize = 80;
/// How many characters of the line it shows after the link.
const AFTER_CHARS: usize = 60;

/// The notes of the workspace in the overview's order, with their formats.
type Notes = Arc<Vec<(NoteId, NoteFormat)>>;

/// A file's time and size when it was read.
type FileStamp = (SystemTime, u64);

/// The anchors of the headings of files, by the stamp they were read at.
type Anchors = HashMap<NoteId, (FileStamp, Arc<HashSet<String>>)>;

/// A note open in a tab: its text counts instead of its file (BR-015).
#[derive(Debug, Clone)]
struct Open {
    id: NoteId,
    format: NoteFormat,
    rev: u32,
    rope: Rope,
}

/// A question for the thread.
#[derive(Debug)]
struct Ask {
    /// Its number; only the answer to the newest question is taken in.
    seq: u64,
    root: PathBuf,
    /// The notes of the workspace in the order of the note overview.
    notes: Arc<Vec<(NoteId, NoteFormat)>>,
    /// The notes open in a tab.
    open: Vec<Open>,
    /// The active note, one of `open`, whose lists are made.
    active: Option<NoteId>,
    /// Every file is looked at again; else only those not read yet (the
    /// active note or the text of a tab changed, NFR-018 while typing).
    sweep: bool,
}

/// What the thread answers: the number of the question and the lists, or
/// `None` without an active note or when the question panicked.
type Answer = (u64, Option<LinksView>);

/// The tick's side of the thread, kept by the open workspace.
pub(super) struct LinkIndex {
    ask: Option<Sender<Ask>>,
    answers: Option<Receiver<Answer>>,
    /// The number of the next question.
    next: u64,
    /// The question asked and not answered yet.
    waiting: Option<u64>,
    /// The notes of the workspace in the overview's order, by the
    /// generation of the notes they were made for.
    notes: Option<(u64, Notes)>,
    /// When the last question was asked.
    asked_at: Option<Instant>,
    /// When the last question that looked at every file was asked.
    swept_at: Option<Instant>,
    /// What the last question was of.
    asked: Option<Stamp>,
    /// The newest lists, with their number ([`NUMBERS`]); lists like the
    /// ones before keep their number, so the window is told only of a
    /// change.
    latest: Option<(u64, LinksView)>,
}

/// The numbers of the lists, never repeated in a session, so lists of
/// another workspace are told too.
static NUMBERS: AtomicU64 = AtomicU64::new(1);

impl Default for LinkIndex {
    fn default() -> Self {
        Self {
            ask: None,
            answers: None,
            next: 1,
            waiting: None,
            notes: None,
            asked_at: None,
            swept_at: None,
            asked: None,
            latest: None,
        }
    }
}

impl std::fmt::Debug for LinkIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkIndex")
            .field("waiting", &self.waiting)
            .finish_non_exhaustive()
    }
}

/// What a question is of: the active note and its revision, the revisions
/// of the open notes, and the generation of the notes of the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    active: Option<(NoteId, u32)>,
    open: Vec<(NoteId, u32)>,
    generation: u64,
}

impl LinkIndex {
    /// Takes in the answers the thread sent: the newest question's.
    fn take_answers(&mut self) {
        let Some(answers) = &self.answers else {
            return;
        };
        loop {
            let (seq, view) = match answers.try_recv() {
                Ok(answer) => answer,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    // The thread ended; the next question starts a new one.
                    eprintln!("FR-078: the thread of the links ended; it is started again");
                    self.ask = None;
                    self.answers = None;
                    self.waiting = None;
                    return;
                }
            };
            if self.waiting == Some(seq) {
                self.waiting = None;
                let Some(view) = view else {
                    continue;
                };
                if self
                    .latest
                    .as_ref()
                    .is_none_or(|(_, latest)| *latest != view)
                {
                    self.latest = Some((NUMBERS.fetch_add(1, Ordering::Relaxed), view));
                }
            }
        }
    }

    /// Sends `ask` to the thread, started the first time; `false` when the
    /// thread could not be started or is gone.
    fn send(&mut self, ask: Ask, wake: &Wake) -> bool {
        if self.ask.is_none() {
            let (ask_send, asks) = mpsc::channel();
            let (answer_send, answers) = mpsc::channel();
            let wake = wake.clone();
            let spawned = std::thread::Builder::new()
                .name("underline-links".into())
                .spawn(move || run(&asks, &answer_send, &wake));
            if let Err(error) = spawned {
                eprintln!("FR-078: the links could not be looked for: {error}");
                return false;
            }
            self.ask = Some(ask_send);
            self.answers = Some(answers);
        }
        let sent = self
            .ask
            .as_ref()
            .is_some_and(|sender| sender.send(ask).is_ok());
        if !sent {
            // The thread ended; the next question starts a new one.
            self.ask = None;
            self.answers = None;
        }
        sent
    }
}

/// The thread: answers the newest question waiting, one after the other,
/// until the open workspace lets go of it.
fn run(asks: &Receiver<Ask>, answers: &Sender<Answer>, wake: &Wake) {
    let mut index = Index::default();
    while let Ok(mut ask) = asks.recv() {
        while let Ok(newer) = asks.try_recv() {
            ask = newer;
        }
        let made = catch_unwind(AssertUnwindSafe(|| index.answer(&ask)));
        // A panic of one note's links costs only that note (`held_safely`);
        // one here costs only this answer: the index stays.
        let view = made.unwrap_or_else(|_| {
            eprintln!("FR-078: making the lists of the links panicked");
            None
        });
        if answers.send((ask.seq, view)).is_err() {
            return;
        }
        wake.wake();
    }
}

/// A link to a note, as the thread keeps it for the backlinks.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    /// The note the link leads to.
    leads_to: NoteId,
    /// The line, counted from 1.
    line: u32,
    /// The bytes of the link, and the check of its markup.
    from: usize,
    to: usize,
    check: u32,
    /// The line around it.
    before: String,
    link: String,
    after: String,
}

/// The links to notes of a file, as last read.
#[derive(Debug)]
struct OnDisk {
    format: NoteFormat,
    /// Its time and size when read; `None` when it could not be read.
    stamp: Option<(SystemTime, u64)>,
    links: Arc<Vec<Held>>,
}

/// What the thread keeps between questions.
#[derive(Debug, Default)]
struct Index {
    root: PathBuf,
    disk: HashMap<NoteId, OnDisk>,
    /// The links of the notes open in a tab, by revision.
    open: HashMap<NoteId, (u32, NoteFormat, Arc<Vec<Held>>)>,
    /// The anchors of the headings of files, by their time and size, for
    /// the broken links (BR-015): a file not changed is not read again.
    anchors: Anchors,
}

impl Index {
    /// The lists of the active note of `ask`, after the files and the open
    /// notes were looked at.
    fn answer(&mut self, ask: &Ask) -> Option<LinksView> {
        if self.root != ask.root {
            *self = Self {
                root: ask.root.clone(),
                ..Self::default()
            };
        }
        self.take_open(&ask.open);
        self.look_at_files(ask, ask.sweep);
        let active = ask.active.as_ref()?;
        let open = ask.open.iter().find(|open| &open.id == active)?;
        let known: HashSet<&NoteId> = ask.notes.iter().map(|(id, _)| id).collect();
        let formats: HashMap<&NoteId, NoteFormat> =
            ask.notes.iter().map(|(id, format)| (id, *format)).collect();
        let backlinks = self.backlinks(ask, active);
        let anchors = std::cell::RefCell::new(std::mem::take(&mut self.anchors));
        let outgoing = outgoing_links(
            &ask.root,
            open,
            |id| known.contains(id),
            |id| {
                headings_of(
                    &ask.root,
                    &ask.open,
                    id,
                    formats.get(id).copied(),
                    &mut anchors.borrow_mut(),
                )
            },
        );
        self.anchors = anchors.into_inner();
        self.anchors.retain(|id, _| known.contains(id));
        let backlink_count = backlinks.iter().map(|note| note.links.len()).sum::<usize>();
        Some(LinksView {
            note: active.as_str().to_owned(),
            rev: open.rev,
            backlinks,
            backlink_count: units(backlink_count),
            outgoing,
        })
    }

    /// The links of the open notes, scanned again for a new revision.
    fn take_open(&mut self, open: &[Open]) {
        self.open
            .retain(|id, _| open.iter().any(|tab| &tab.id == id));
        for tab in open {
            let current = self
                .open
                .get(&tab.id)
                .is_some_and(|(rev, format, _)| *rev == tab.rev && *format == tab.format);
            if !current {
                let links = held_safely(&self.root, &tab.id, tab.format, &tab.rope.to_string());
                self.open
                    .insert(tab.id.clone(), (tab.rev, tab.format, Arc::new(links)));
            }
        }
    }

    /// Looks at the file of every note not open in a tab: one changed since
    /// it was read (time, size or format) is read and scanned again.
    /// Without `sweep`, only the files not read yet are.
    fn look_at_files(&mut self, ask: &Ask, sweep: bool) {
        let listed: HashSet<&NoteId> = ask.notes.iter().map(|(id, _)| id).collect();
        self.disk.retain(|id, _| listed.contains(id));
        for (id, format) in ask.notes.iter() {
            if self.open.contains_key(id) || (!sweep && self.disk.contains_key(id)) {
                continue;
            }
            let path = self.root.join(id.file_name(*format));
            let stamp = fs::metadata(&path).ok().map(|meta| {
                (
                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    meta.len(),
                )
            });
            let current = self.disk.get(id).is_some_and(|known| {
                known.stamp.is_some() && known.stamp == stamp && known.format == *format
            });
            if current {
                continue;
            }
            let links = fs::read_to_string(&path)
                .map(|text| held_safely(&self.root, id, *format, &text))
                .unwrap_or_default();
            self.disk.insert(
                id.clone(),
                OnDisk {
                    format: *format,
                    stamp,
                    links: Arc::new(links),
                },
            );
        }
    }

    /// FR-078, BR-015: the notes that link to `active`, in the order of the
    /// note overview, each with its links to it; not `active` itself.
    fn backlinks(&self, ask: &Ask, active: &NoteId) -> Vec<BacklinkNoteView> {
        ask.notes
            .iter()
            .filter(|(id, _)| id != active)
            .filter_map(|(id, _)| {
                let links = self
                    .open
                    .get(id)
                    .map(|(_, _, links)| links)
                    .or_else(|| self.disk.get(id).map(|known| &known.links))?;
                let links: Vec<BacklinkView> = links
                    .iter()
                    .filter(|held| &held.leads_to == active)
                    .map(|held| BacklinkView {
                        line: held.line,
                        before: held.before.clone(),
                        link: held.link.clone(),
                        after: held.after.clone(),
                        place: LinkPlace {
                            from: units(held.from),
                            to: units(held.to),
                            check: held.check,
                        },
                    })
                    .collect();
                (!links.is_empty()).then(|| BacklinkNoteView {
                    note: id.as_str().to_owned(),
                    title: id.title(),
                    folder: id.folder().map(str::to_owned),
                    links,
                })
            })
            .collect()
    }
}

/// [`held_links`]; a panic on the text of one note is logged and leaves that
/// note without links until its text changes, never the whole index.
fn held_safely(root: &Path, note: &NoteId, format: NoteFormat, text: &str) -> Vec<Held> {
    catch_unwind(AssertUnwindSafe(|| held_links(root, note, format, text))).unwrap_or_else(|_| {
        eprintln!("FR-078: the links of {} could not be read", note.as_str());
        Vec::new()
    })
}

/// The links of the text of `note` (in `format`, a note of the workspace at
/// `root`) that lead to a note, whether it exists or not, with their lines.
fn held_links(root: &Path, note: &NoteId, format: NoteFormat, text: &str) -> Vec<Held> {
    let path = root.join(note.file_name(format));
    let starts = line_starts(text);
    links_of(text, format)
        .into_iter()
        .filter_map(|found| {
            let target = text.get(found.target.clone())?;
            // Every note counts as there: a backlink leads to the active
            // note, which is.
            let LinkTarget::Note { id, .. } =
                resolve_link_with(root, &path, target, |_| true, |_| false)
            else {
                return None;
            };
            let line = starts.partition_point(|start| *start <= found.span.start);
            let (before, link, after) = context(text, &starts, line, &found);
            Some(Held {
                leads_to: id,
                line: units(line),
                from: found.span.start,
                to: found.span.end,
                check: LinkPlace::check_of(text.get(found.span.clone()).unwrap_or_default()),
                before,
                link,
                after,
            })
        })
        .collect()
}

/// BR-015: the links of a text that count: links and web addresses, not
/// pictures and included files.
fn links_of(text: &str, format: NoteFormat) -> Vec<FoundLink> {
    found_links(text, format)
        .into_iter()
        .filter(|found| matches!(found.markup, LinkMarkup::Link | LinkMarkup::Address))
        .collect()
}

/// The byte offsets of the starts of the lines of `text`.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(at, _)| at + 1))
        .collect()
}

/// The line `line` (from 1) around `found`: the text before the link, the
/// link, the text after it, each shortened.
fn context(
    text: &str,
    starts: &[usize],
    line: usize,
    found: &FoundLink,
) -> (String, String, String) {
    // Only as much of the line as is shown is looked at, so a line of many
    // links costs as much per link as a short one (NFR-019).
    let start = starts.get(line.saturating_sub(1)).copied().unwrap_or(0);
    let link = &text[found.span.clone()];
    let after_start = found.span.end.max(start);
    let before = keep_end(&text[start..found.span.start.max(start)], BEFORE_CHARS);
    let after = keep_start_of_line(&text[after_start..], AFTER_CHARS);
    (before, keep_start(link, LINK_CHARS), after)
}

/// The first `chars` characters of `text`, with `…` when there are more.
fn keep_start(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// The first `chars` characters of the line `text` starts, without the
/// spaces at its end, with `…` when the line goes on.
fn keep_start_of_line(text: &str, chars: usize) -> String {
    let mut end = text.len();
    let mut more = false;
    for (count, (at, c)) in text.char_indices().enumerate() {
        if c == '\n' || c == '\r' {
            end = at;
            break;
        }
        if count == chars {
            end = at;
            more = true;
            break;
        }
    }
    let kept = text[..end].trim_end();
    if more {
        format!("{kept}…")
    } else {
        kept.to_owned()
    }
}

/// The last `chars` characters of `text`, without the spaces at its start,
/// with `…` when there are more; only those characters are looked at.
fn keep_end(text: &str, chars: usize) -> String {
    match text.char_indices().rev().nth(chars) {
        Some((at, c)) => format!("…{}", &text[at + c.len_utf8()..]),
        None => text.trim_start().to_owned(),
    }
}

/// FR-079, BR-015: the links of the open note `open` in the order of the
/// note, each with where it leads and whether it is broken. `is_note` says
/// whether a note exists, `anchors` gives the anchors of the headings of a
/// note (`None` when it cannot be read).
fn outgoing_links(
    root: &Path,
    open: &Open,
    is_note: impl Fn(&NoteId) -> bool,
    anchors: impl Fn(&NoteId) -> Option<Arc<HashSet<String>>>,
) -> Vec<OutgoingLinkView> {
    let text = open.rope.to_string();
    let path = root.join(open.id.file_name(open.format));
    let web = WebOffsets::of(&open.rope);
    let starts = line_starts(&text);
    let mut known: HashMap<NoteId, Option<Arc<HashSet<String>>>> = HashMap::new();
    let mut has_heading = |id: &NoteId, anchor: &str| {
        known
            .entry(id.clone())
            .or_insert_with(|| {
                if id == &open.id {
                    Some(Arc::new(anchors_in(&text, open.format)))
                } else {
                    anchors(id)
                }
            })
            .as_ref()
            .is_some_and(|anchors| anchors.contains(&anchor_key(anchor)))
    };
    links_of(&text, open.format)
        .into_iter()
        .filter_map(|found| {
            let written = text.get(found.target.clone())?;
            let target = resolve_link_with(root, &path, written, &is_note, |path| path.is_file());
            let (kind, leads, broken) = match target {
                LinkTarget::Heading(anchor) => {
                    let broken = !has_heading(&open.id, &anchor);
                    (LinkKindName::Note, format!("#{anchor}"), broken)
                }
                LinkTarget::Note { id, heading } => match heading {
                    Some(anchor) => {
                        let broken = !has_heading(&id, &anchor);
                        (
                            LinkKindName::Note,
                            format!("{}#{anchor}", id.as_str()),
                            broken,
                        )
                    }
                    None => (LinkKindName::Note, id.as_str().to_owned(), false),
                },
                LinkTarget::MissingNote { .. } => (LinkKindName::Note, written.to_owned(), true),
                LinkTarget::Web(address) => (LinkKindName::Web, address, false),
                LinkTarget::File(file) => {
                    let broken = !file.exists();
                    let shown = file.strip_prefix(root).map_or_else(
                        |_| file.display().to_string(),
                        |inside| inside.to_string_lossy().replace('\\', "/"),
                    );
                    (LinkKindName::File, shown, broken)
                }
                // BR-005: no local link, so not broken; it is not followed.
                LinkTarget::Outside => (LinkKindName::File, written.to_owned(), false),
            };
            let shown = found
                .text
                .and_then(|range| text.get(range))
                .map(str::trim)
                .filter(|shown| !shown.is_empty())
                .unwrap_or(written);
            let line = starts.partition_point(|start| *start <= found.span.start);
            let at = open.rope.byte_to_char(found.span.start.min(text.len()));
            Some(OutgoingLinkView {
                text: shown.to_owned(),
                target: leads,
                kind,
                broken,
                line: units(line),
                offset: units(web.to_web(&open.rope, at)),
            })
        })
        .collect()
}

/// UC-004 A14 step 2: the place, as written, of the link (a link or a web
/// address the outgoing links list) that starts at web offset `at` of the
/// text in `rope`, a note in `format`.
pub(super) fn outgoing_at(rope: &Rope, format: NoteFormat, at: u32) -> Option<String> {
    let text = rope.to_string();
    let web = WebOffsets::of(rope);
    links_of(&text, format)
        .into_iter()
        .find(|found| {
            let start = rope.byte_to_char(found.span.start.min(text.len()));
            units(web.to_web(rope, start)) == at
        })
        .and_then(|found| text.get(found.target).map(str::to_owned))
}

/// The anchors of the headings of `text`, as GitHub and GitLab name them
/// in the note format.
fn anchors_in(text: &str, format: NoteFormat) -> HashSet<String> {
    headings_in(text, format)
        .into_iter()
        .map(|heading| anchor_key(&heading.anchor))
        .collect()
}

/// UC-005 BR-018, UC-004 BR-005: the headings of `text` with their anchors,
/// as GitHub and GitLab name the place of a heading in the note format.
pub(super) fn headings_in(text: &str, format: NoteFormat) -> Vec<HeadingView> {
    let slug = match format {
        NoteFormat::Markdown => heading_anchor,
        NoteFormat::AsciiDoc => asciidoc_anchor,
    };
    // A name that comes again gets a number, as GitHub (`intro-1`) and
    // Asciidoctor (`_intro_2`) number it.
    let mut seen: HashMap<String, usize> = HashMap::new();
    parse_blocks(text, format)
        .iter()
        .filter_map(|block| match block.kind {
            BlockKind::Heading(level) => {
                let label = block.text();
                let base = slug(&label);
                let count = seen.entry(base.clone()).or_insert(0);
                let anchor = match (*count, format) {
                    (0, _) => base,
                    (n, NoteFormat::Markdown) => format!("{base}-{n}"),
                    (n, NoteFormat::AsciiDoc) => format!("{base}_{}", n + 1),
                };
                *count += 1;
                Some(HeadingView {
                    anchor,
                    label: label.trim().to_owned(),
                    level: u32::from(level),
                })
            }
            _ => None,
        })
        .collect()
}

/// UC-004 BR-005: an anchor as a link names it, compared as the browsers
/// find it: percent escapes decoded, upper and lower case alike.
fn anchor_key(anchor: &str) -> String {
    underline_core::percent_decode(anchor).to_lowercase()
}

/// The anchors of the headings of the note `id` (in `format`): of its tab
/// when it is open, else of its file, read again only when it changed.
fn headings_of(
    root: &Path,
    open: &[Open],
    id: &NoteId,
    format: Option<NoteFormat>,
    known: &mut Anchors,
) -> Option<Arc<HashSet<String>>> {
    if let Some(tab) = open.iter().find(|tab| &tab.id == id) {
        return Some(Arc::new(anchors_in(&tab.rope.to_string(), tab.format)));
    }
    let path = root.join(id.file_name(format?));
    let meta = fs::metadata(&path).ok()?;
    let stamp = (
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        meta.len(),
    );
    if let Some((known_stamp, anchors)) = known.get(id) {
        if *known_stamp == stamp {
            return Some(Arc::clone(anchors));
        }
    }
    let text = fs::read_to_string(&path).ok()?;
    let anchors = Arc::new(anchors_in(&text, format?));
    known.insert(id.clone(), (stamp, Arc::clone(&anchors)));
    Some(anchors)
}

impl Session {
    /// FR-078, FR-079, NFR-019: the tick's part of the links: the answer
    /// of the thread is taken in, a question is asked when what the lists
    /// are of changed (at most every [`LINKS_EVERY`]) or, while a note is
    /// shown, every [`LINKS_SWEEP`] to look at the files, and the window is
    /// told of the active note's lists when they changed.
    pub(super) fn links_events(&mut self, now: Instant) -> Vec<Event> {
        let wake = self.wake.clone();
        let Some(open) = self.open.as_mut() else {
            return Vec::new();
        };
        open.links.take_answers();
        let active = open
            .tabs
            .active()
            .map(|(id, tab)| (id.clone(), tab.mirror.rev()));
        let stamp = Stamp {
            active: active.clone(),
            open: open
                .tabs
                .iter()
                .map(|(id, tab)| (id.clone(), tab.mirror.rev()))
                .collect(),
            generation: open.search_generation,
        };
        let links = &mut open.links;
        let since = links.asked_at.map(|at| now.saturating_duration_since(at));
        let changed = links.asked.as_ref() != Some(&stamp);
        let due = if changed {
            since.is_none_or(|since| since >= LINKS_EVERY)
        } else {
            active.is_some() && since.is_none_or(|since| since >= LINKS_SWEEP)
        };
        // The files are looked at when the notes of the workspace changed or
        // the sweep is due; while only texts of tabs change, not.
        let swept = links.swept_at.map(|at| now.saturating_duration_since(at));
        let sweep = links
            .asked
            .as_ref()
            .is_none_or(|asked| asked.generation != stamp.generation)
            || swept.is_none_or(|swept| swept >= LINKS_SWEEP);
        if due && links.waiting.is_none() {
            // The notes in the overview's order, made once per generation of
            // the notes, not for every second's look at the files.
            let notes = match &links.notes {
                Some((generation, notes)) if *generation == stamp.generation => Arc::clone(notes),
                _ => {
                    let formats: HashMap<&NoteId, NoteFormat> = open
                        .workspace
                        .listing()
                        .notes
                        .iter()
                        .map(|entry| (&entry.id, entry.format))
                        .collect();
                    let notes: Arc<Vec<(NoteId, NoteFormat)>> = Arc::new(
                        open.overview
                            .iter()
                            .filter_map(|entry| Some((entry.id.clone(), *formats.get(&entry.id)?)))
                            .collect(),
                    );
                    links.notes = Some((stamp.generation, Arc::clone(&notes)));
                    notes
                }
            };
            let seq = links.next;
            links.next += 1;
            let ask = Ask {
                seq,
                root: open.workspace.root().to_owned(),
                notes,
                open: open
                    .tabs
                    .iter()
                    .map(|(id, tab)| Open {
                        id: id.clone(),
                        format: tab.edit.format,
                        rev: tab.mirror.rev(),
                        rope: tab.mirror.rope().clone(),
                    })
                    .collect(),
                active: active.as_ref().map(|(id, _)| id.clone()),
                sweep,
            };
            links.asked_at = Some(now);
            if sweep {
                links.swept_at = Some(now);
            }
            links.asked = Some(stamp);
            if links.send(ask, &wake) {
                links.waiting = Some(seq);
            }
        }
        let shown = active.and_then(|(id, _)| {
            links
                .latest
                .as_ref()
                .filter(|(_, view)| view.note == id.as_str())
        });
        match shown {
            Some((number, view)) if self.told.links != Some(*number) => {
                self.told.links = Some(*number);
                vec![Event::Links(view.clone())]
            }
            _ => Vec::new(),
        }
    }

    /// UC-004 A14: opens the note `note` that holds a backlink as the note
    /// overview does, with the cursor at the start of the link at `place`
    /// (bytes of the text the backlinks were found in); `None` when the
    /// note did not open (the status says why, A10).
    ///
    /// # Errors
    ///
    /// The note opened, but its tab is not there.
    pub fn links_open(
        &mut self,
        note: &NoteId,
        place: LinkPlace,
    ) -> Result<Option<crate::SearchOpened>, crate::UiError> {
        if !self.open_note(note) {
            return Ok(None);
        }
        let moved = self.open.as_ref().and_then(|open| {
            let tab = open.tabs.get(open.tabs.find(note)?);
            let rope = tab.mirror.rope();
            let (from, to) = (place.from as usize, place.to as usize);
            let boundary = |at: usize| rope.char_to_byte(rope.byte_to_char(at)) == at;
            let fits = to <= rope.len_bytes()
                && from <= to
                && boundary(from)
                && boundary(to)
                && LinkPlace::check_of(&rope.byte_slice(from..to).to_string()) == place.check;
            (!fits).then(|| tab.title.clone())
        });
        if let Some(title) = moved {
            // UC-004 A14: the note changed since the backlinks were listed.
            self.status = format!(
                "{title} changed since the backlinks were listed; the link is no longer at its place"
            );
            return Ok(None);
        }
        let at = crate::FoundPlace {
            from: place.from,
            to: place.from,
        };
        self.search_open(note, Some(at))
    }

    /// UC-005 A23, BR-018: the headings of the note `note` for link
    /// completion: of its tab when it is open, with changes not stored yet,
    /// else of its file; none when it cannot be read. Reads the file while
    /// the session is held; the command uses [`Session::headings_plan`].
    #[must_use]
    pub fn note_headings(&self, note: &NoteId) -> Vec<HeadingView> {
        self.headings_plan(note).read()
    }

    /// UC-005 A23, BR-018: what the headings of `note` are made from: the
    /// text of its tab, or its file, which [`HeadingsPlan::read`] reads
    /// once the session is let go.
    #[must_use]
    pub fn headings_plan(&self, note: &NoteId) -> HeadingsPlan {
        let Some(open) = self.open.as_ref() else {
            return HeadingsPlan(None);
        };
        if let Some(index) = open.tabs.find(note) {
            let tab = open.tabs.get(index);
            return HeadingsPlan(Some(HeadingsFrom::Text(
                tab.mirror.rope().clone(),
                tab.edit.format,
            )));
        }
        HeadingsPlan(
            open.workspace
                .format_of(note)
                .map(|format| HeadingsFrom::File(open.root().join(note.file_name(format)), format)),
        )
    }
}

/// UC-005 A23: where the headings of a note come from, taken from the
/// session ([`Session::headings_plan`]) and read without it.
#[derive(Debug)]
pub struct HeadingsPlan(Option<HeadingsFrom>);

#[derive(Debug)]
enum HeadingsFrom {
    /// The text of its tab.
    Text(Rope, NoteFormat),
    /// Its file.
    File(PathBuf, NoteFormat),
}

impl HeadingsPlan {
    /// The headings with their anchors; none when the file cannot be read.
    #[must_use]
    pub fn read(self) -> Vec<HeadingView> {
        match self.0 {
            Some(HeadingsFrom::Text(rope, format)) => headings_in(&rope.to_string(), format),
            Some(HeadingsFrom::File(path, format)) => fs::read_to_string(path)
                .map(|text| headings_in(&text, format))
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
