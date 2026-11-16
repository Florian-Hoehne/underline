//! FR-099, FR-100, UC-005 BR-023, A28 (ADR-030): the spell check of the
//! open notes, on the mirror.
//!
//! - After an edit was confirmed (`ed_sync`), the note is checked again
//!   once the text rested [`SPELL_DEBOUNCE`], at the latest
//!   [`SPELL_AT_THE_LATEST`] after the first edit not checked yet (notes
//!   over [`LARGE_NOTE`] bytes rest longer, [`LARGE_DEBOUNCE`] and
//!   [`LARGE_AT_THE_LATEST`]); only the changed blocks are checked anew
//!   ([`NoteSpeller`]). A note just opened, another language and changed
//!   user's words are checked at once.
//! - The check runs on a thread of its own, never on the tick or a command:
//!   it reads the dictionary the first time a language is needed, finds the
//!   words of the note, checks the part in view first and sends those marks
//!   before the rest are done. Work of an older language or dictionary is
//!   skipped. A fault while checking a note is caught: the note stays
//!   unmarked until it changes and the status bar says so; a thread that
//!   ended is started again, at most [`RESTARTS`] times.
//! - The marks go to the window as `spell-marks`, tagged with the mirror's
//!   revision (ADR-025) and only for the tab they were checked for (a tab
//!   opened again is another one).
//! - The corrections for a word come from a job run off the session
//!   ([`SuggestJob`]); "Add to dictionary" stores the word in the user's
//!   words file next to the user settings file and checks every open note
//!   again, keeping what the dictionary said of each word. Only the time
//!   and size of that file are looked at on the tick, every
//!   [`USER_WORDS_POLL`] while the spell check is on and a note is open;
//!   when they changed it is read on the check thread, so a change made
//!   outside (another Underline, an editor) applies too (ADR-011's poll, as
//!   for the user settings file). A word that could not be stored counts
//!   until Underline closes, whatever the file holds.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use underline_core::{NoteFormat, NoteId, SpellCheckLanguage};
use underline_editor_core::utf16::WebOffsets;
use underline_spell::{Dictionaries, NoteSpeller, SpellError, UserWords, USER_WORDS_FILE_NAME};

use super::{Session, Wake};
use crate::editor_view::units;
use crate::spell_view::{SpellMarksView, SpellStateView};
use crate::{Event, UiError, UiErrorKind};

/// ADR-030: how long the text rests after an edit before it is checked.
pub const SPELL_DEBOUNCE: Duration = Duration::from_millis(250);

/// UC-005 BR-023: a note being typed in is checked at least this often, so
/// a mark follows a word within 1 second while typing goes on.
pub const SPELL_AT_THE_LATEST: Duration = Duration::from_millis(700);

/// A note of more bytes is parsed whole for each check (a few
/// milliseconds per 100 KB), so it is checked less often while typing.
pub(super) const LARGE_NOTE: usize = 1024 * 1024;

/// [`SPELL_DEBOUNCE`] of a large note.
pub(super) const LARGE_DEBOUNCE: Duration = Duration::from_millis(1000);

/// [`SPELL_AT_THE_LATEST`] of a large note.
pub(super) const LARGE_AT_THE_LATEST: Duration = Duration::from_millis(3000);

/// How often the user's words file is looked at.
pub const USER_WORDS_POLL: Duration = Duration::from_millis(500);

/// How many times the check thread is started again after it ended.
pub(super) const RESTARTS: u32 = 3;

/// The bytes around the cursor checked first when no lines in view are
/// known (the facade), before and after it.
const AROUND_CURSOR: (usize, usize) = (4_000, 12_000);

/// The spell check's state of one tab.
#[derive(Debug, Default)]
pub(super) struct TabSpell {
    /// Which tab this is: a tab opened again is another one, so marks
    /// checked for the tab before never reach it. 0 until first checked.
    instance: u64,
    /// The mirror's revision last seen, and when it changed.
    seen_rev: Option<u32>,
    last_edit: Option<Instant>,
    /// The first edit not checked yet.
    first_edit: Option<Instant>,
    /// A check of the tab runs.
    pub(super) running: bool,
    /// The revision and generation whose marks the window has.
    pub(super) told: Option<(u32, u64)>,
    /// The generation of the last check given to the thread, so a check
    /// whose outcome was dropped is not followed by one at once: the next
    /// waits for the rest like any other.
    tried: Option<u64>,
    /// The marks sent last, for the tests.
    pub(super) marks: Vec<(u32, u32)>,
}

impl TabSpell {
    /// The window holds no marks of the note any more (a new editor core);
    /// they are sent again.
    pub(super) fn forget_told(&mut self) {
        self.told = None;
        self.tried = None;
    }

    /// ADR-030: an edit of the window was confirmed at `now` (`ed_sync`);
    /// the rest before the check counts from here.
    pub(super) fn edited(&mut self, now: Instant) {
        self.last_edit = Some(now);
        self.first_edit.get_or_insert(now);
    }
}

/// A check of one note.
struct Job {
    note: NoteId,
    instance: u64,
    rev: u32,
    /// The generation of everything the marks depend on.
    generation: u64,
    /// The generation of the language and the dictionaries.
    dictionary: u64,
    language: &'static str,
    user: Arc<UserWords>,
    rope: ropey::Rope,
    format: NoteFormat,
    first: Option<Range<usize>>,
}

/// What the check thread is asked to do.
enum Work {
    Check(Box<Job>),
    /// Read the user's words file, last seen with this time and size.
    ReadWords(PathBuf, Stamp),
    /// Only the tabs of these instances are open: the others' spellers go.
    Retain(Vec<u64>),
}

/// The time and size of a file, to tell that it changed.
type Stamp = Option<(SystemTime, u64)>;

/// What the check thread found.
struct Outcome {
    note: NoteId,
    /// The language the note was checked in.
    language: &'static str,
    instance: u64,
    rev: u32,
    generation: u64,
    /// More marks follow for the same revision (the part in view came
    /// first).
    partial: bool,
    marks: Result<Vec<(u32, u32)>, Arc<SpellError>>,
}

/// What the check thread answers.
enum Done {
    Checked(Outcome),
    Words(Result<UserWords, String>, Stamp),
}

/// The spell check of the session.
pub(super) struct Spell {
    dictionaries: Arc<Dictionaries>,
    /// The user's words in effect: those of the file and those of this
    /// session.
    user: Arc<UserWords>,
    /// The words of the file, as read on the check thread.
    file_words: UserWords,
    /// UC-005 A28: words added that could not be stored; they count until
    /// Underline closes, whatever the file holds.
    session_words: BTreeSet<String>,
    /// The words file was read once (or there is none).
    user_ready: bool,
    /// A read of the words file was asked for and not answered yet.
    user_reading: bool,
    user_seen: Stamp,
    user_checked: Option<Instant>,
    /// The language checked in now.
    language: SpellCheckLanguage,
    /// Grows with every change of the language, the dictionaries or the
    /// user's words, so every open note is checked again at once; shared
    /// with the check thread, which skips older work.
    generation: Arc<AtomicU64>,
    /// Grows with every change of the language or the dictionaries only:
    /// what the dictionary said is kept while only the user's words change.
    dictionary: u64,
    next_instance: u64,
    /// The instances of the tabs the thread was told of last.
    live: Vec<u64>,
    /// The check thread, started when first needed.
    work: Option<Sender<Work>>,
    done: Option<Receiver<Done>>,
    /// How many times the thread was started again.
    restarts: u32,
    /// The languages whose missing dictionary was said, in this generation.
    said: HashSet<&'static str>,
    /// What the status bar shows of the spell check, and what it was told.
    state: SpellStateView,
    told_state: Option<SpellStateView>,
}

impl std::fmt::Debug for Spell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spell")
            .field("language", &self.language)
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

impl Default for Spell {
    fn default() -> Self {
        Self {
            dictionaries: Arc::new(Dictionaries::none()),
            user: Arc::new(UserWords::default()),
            file_words: UserWords::default(),
            session_words: BTreeSet::new(),
            user_ready: false,
            user_reading: false,
            user_seen: None,
            user_checked: None,
            language: SpellCheckLanguage::Off,
            generation: Arc::new(AtomicU64::new(0)),
            dictionary: 0,
            next_instance: 0,
            live: Vec::new(),
            work: None,
            done: None,
            restarts: 0,
            said: HashSet::new(),
            state: SpellStateView {
                problem: None,
                detail: None,
            },
            // The window starts without a problem of the spell check.
            told_state: Some(SpellStateView {
                problem: None,
                detail: None,
            }),
        }
    }
}

impl Spell {
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Everything is checked again.
    fn bump(&mut self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// The language or the dictionaries changed: checked anew, from the
    /// dictionary on. A stopped spell check stays stopped (and said).
    fn bump_dictionary(&mut self) {
        self.dictionary += 1;
        self.bump();
        self.said.clear();
        if !self.stopped() {
            self.state = SpellStateView {
                problem: None,
                detail: None,
            };
        }
    }

    /// The thread ended more than [`RESTARTS`] times without a check done
    /// in between.
    fn stopped(&self) -> bool {
        self.restarts > RESTARTS
    }

    /// The words in effect are those of the file and of this session; a
    /// change checks every note again.
    fn rebuild_user(&mut self) {
        let mut words = self.file_words.clone();
        for word in &self.session_words {
            let _ = words.add(word);
        }
        if *self.user != words {
            self.user = Arc::new(words);
            self.bump();
        }
    }
}

/// UC-005 A28: the corrections for a word, worked out without the session.
pub struct SuggestJob {
    dictionaries: Arc<Dictionaries>,
    language: Option<&'static str>,
    word: String,
}

impl SuggestJob {
    /// Up to five corrections, the likeliest first; none when the spell
    /// check is off or has no dictionary. Reads the dictionary when it was
    /// not read yet, so it runs off the session.
    ///
    /// # Errors
    ///
    /// The language's dictionary cannot be read ("Corrections unavailable").
    pub fn run(self) -> Result<Vec<String>, UiError> {
        let Some(language) = self.language else {
            return Ok(Vec::new());
        };
        match self.dictionaries.get(language) {
            Ok(checker) => Ok(NoteSpeller::suggest(checker.as_ref(), &self.word)),
            Err(error) => Err(UiError::new(UiErrorKind::Internal, error.to_string())),
        }
    }
}

impl Session {
    /// ADR-030: the dictionaries the spell check reads, the folder holding
    /// one folder per language (Underline's resources); every open note is
    /// checked again on a new thread.
    pub fn set_dictionaries(&mut self, dictionaries: Dictionaries) {
        self.spell.dictionaries = Arc::new(dictionaries);
        // A new start for a spell check that stopped.
        self.spell.restarts = 0;
        self.spell.bump_dictionary();
        // The thread keeps the dictionaries it was started with.
        self.spell.work = None;
        self.spell.done = None;
        self.spell.user_reading = false;
        self.reset_spell_tabs();
    }

    /// No check runs any more and every tab is checked again (the thread
    /// was replaced or ended).
    fn reset_spell_tabs(&mut self) {
        if let Some(open) = self.open.as_mut() {
            for (_, tab) in open.tabs.iter_mut() {
                tab.spell.running = false;
                tab.spell.told = None;
                tab.spell.tried = None;
            }
        }
    }

    /// The user's words file: next to the user settings file; `None`
    /// without a home directory.
    fn user_words_path(&self) -> Option<PathBuf> {
        self.settings
            .user_settings_dir()
            .map(|dir| dir.join(USER_WORDS_FILE_NAME))
    }

    /// Looks at the time and size of the user's words file at most every
    /// [`USER_WORDS_POLL`], while the spell check is on and a note is open;
    /// when they changed, the check thread reads it.
    fn poll_user_words(&mut self, now: Instant) {
        let in_use = self.spell.language != SpellCheckLanguage::Off
            && self
                .open
                .as_ref()
                .is_some_and(|open| open.tabs.iter().next().is_some());
        if !in_use {
            return;
        }
        let due = self
            .spell
            .user_checked
            .is_none_or(|at| now.saturating_duration_since(at) >= USER_WORDS_POLL);
        if !due || self.spell.user_reading {
            return;
        }
        self.spell.user_checked = Some(now);
        let Some(path) = self.user_words_path() else {
            self.spell.user_ready = true;
            return;
        };
        let stamp = stamp(&path);
        if self.spell.user_ready && stamp == self.spell.user_seen {
            return;
        }
        if stamp.is_none() {
            // No file: no words of the file; those of this session stay.
            self.spell.user_seen = None;
            self.spell.file_words = UserWords::default();
            self.spell.rebuild_user();
            self.spell.user_ready = true;
            return;
        }
        self.spell.user_reading = true;
        self.send_spell_work(Work::ReadWords(path, stamp));
    }

    /// The user's words as the check thread read them.
    fn take_user_words(&mut self, read: Result<UserWords, String>, seen: Stamp) {
        self.spell.user_reading = false;
        self.spell.user_ready = true;
        self.spell.user_seen = seen;
        match read {
            Ok(words) => {
                self.spell.file_words = words;
                self.spell.rebuild_user();
            }
            Err(error) => {
                self.status = format!("The words of your dictionary could not be read: {error}");
            }
        }
    }

    /// UC-005 A28: the job that finds the corrections for `word`, run off
    /// the session.
    #[must_use]
    pub fn spell_suggest_job(&self, word: &str) -> SuggestJob {
        SuggestJob {
            dictionaries: Arc::clone(&self.spell.dictionaries),
            language: self.settings.effective().spell_check_language.dictionary(),
            word: word.to_owned(),
        }
    }

    /// UC-005 A28 step 2: adds `word` to the user's words, stores them and
    /// checks every open note again, so the word loses its mark in every
    /// note. The file is read first and written with the word added: words
    /// another Underline added stay, words the user removed by hand stay
    /// removed, and lines that are no word stay as they are. A file that
    /// cannot be read (not UTF-8, no permission) is never written over: the
    /// word then counts until Underline closes, as one that cannot be
    /// stored.
    ///
    /// # Errors
    ///
    /// The word cannot be added (it holds a space, it is too long), or the
    /// file cannot be read or written; the status bar says why.
    pub fn spell_add_word(&mut self, word: &str) -> Result<(), UiError> {
        let invalid = |error: SpellError| UiError::new(UiErrorKind::Invalid, error.to_string());
        let mut check = UserWords::default();
        check.add(word).map_err(invalid)?;
        let word = word.trim().to_owned();
        let path = self.user_words_path();
        self.spell.user_ready = true;
        self.wake.wake();
        let Some(path) = path else {
            self.add_word_for_now(&word);
            self.status = format!(
                "\"{word}\" is not marked until Underline closes: there is no home directory to store it in"
            );
            return Ok(());
        };
        let mut words = match UserWords::load(&path) {
            Ok(words) => words,
            Err(error) => {
                self.add_word_for_now(&word);
                let reason = format!(
                    "\"{word}\" is not marked until Underline closes: the words of your dictionary could not be read, so they are not written over ({error})"
                );
                self.status.clone_from(&reason);
                return Err(UiError::new(UiErrorKind::Internal, reason));
            }
        };
        words.add(&word).map_err(invalid)?;
        match words.store(&path) {
            Ok(()) => {
                self.spell.user_seen = stamp(&path);
                self.spell.file_words = words;
                self.spell.rebuild_user();
                self.status = format!("Added \"{word}\" to the dictionary");
                Ok(())
            }
            Err(error) => {
                self.add_word_for_now(&word);
                let reason = format!(
                    "\"{word}\" is not marked until Underline closes, as it could not be stored: {error}"
                );
                self.status.clone_from(&reason);
                Err(UiError::new(UiErrorKind::Internal, reason))
            }
        }
    }

    /// `word` counts as the user's until Underline closes, whatever the
    /// file holds; nothing is stored.
    fn add_word_for_now(&mut self, word: &str) {
        self.spell.session_words.insert(word.to_owned());
        self.spell.rebuild_user();
        self.wake.wake();
    }

    /// UC-005 BR-023: the tick's part of the spell check: the outcomes of
    /// the check thread become `spell-marks` events, and the open notes
    /// that changed are given to it.
    pub(super) fn spell_events(&mut self, now: Instant) -> Vec<Event> {
        let language = self.settings.effective().spell_check_language;
        if language != self.spell.language {
            self.spell.language = language;
            self.spell.bump_dictionary();
        }
        let mut events = self.take_spell_done();
        self.poll_user_words(now);
        // The user's words are needed only to mark words.
        if self.spell.user_ready || self.spell.language == SpellCheckLanguage::Off {
            events.extend(self.start_spell_checks(now));
        }
        if self.spell.told_state.as_ref() != Some(&self.spell.state) {
            self.spell.told_state = Some(self.spell.state.clone());
            events.push(Event::SpellState(self.spell.state.clone()));
        }
        events
    }

    /// What the check thread answered; a thread that ended is noticed.
    fn take_spell_done(&mut self) -> Vec<Event> {
        let mut answers = Vec::new();
        let mut ended = false;
        if let Some(done) = &self.spell.done {
            loop {
                match done.try_recv() {
                    Ok(answer) => answers.push(answer),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
        }
        let mut events = Vec::new();
        for answer in answers {
            match answer {
                Done::Words(read, seen) => self.take_user_words(read, seen),
                Done::Checked(outcome) => events.extend(self.take_spell_outcome(outcome)),
            }
        }
        if ended {
            self.spell_thread_ended();
        }
        events
    }

    /// The check thread ended: it is started again with the next check, at
    /// most [`RESTARTS`] times; after that the status bar says so.
    fn spell_thread_ended(&mut self) {
        self.spell.work = None;
        self.spell.done = None;
        self.spell.user_reading = false;
        self.spell.live.clear();
        self.spell.restarts += 1;
        self.reset_spell_tabs();
        if self.spell.stopped() {
            self.spell.state = SpellStateView {
                problem: Some("Spell check stopped".to_owned()),
                detail: Some(
                    "The spell check stopped after repeated faults; no word is marked until Underline starts again."
                        .to_owned(),
                ),
            };
            self.status = "The spell check stopped after repeated faults".to_owned();
        }
    }

    /// One outcome of the check thread, still of its tab's text.
    fn take_spell_outcome(&mut self, outcome: Outcome) -> Option<Event> {
        let generation = self.spell.generation();
        let open = self.open.as_mut()?;
        let index = open.tabs.find(&outcome.note)?;
        let tab = open.tabs.get_mut(index);
        // Marks of a tab closed meanwhile never reach the one opened again.
        if tab.spell.instance != outcome.instance {
            return None;
        }
        if !outcome.partial {
            tab.spell.running = false;
        }
        // An outcome of an older text or language is dropped, its error too;
        // the tick checks the note again.
        if outcome.generation != generation || tab.mirror.rev() != outcome.rev {
            return None;
        }
        if !outcome.partial {
            tab.spell.told = Some((outcome.rev, generation));
        }
        let marks = match outcome.marks {
            Ok(marks) => {
                // A check done: the thread works again.
                self.spell.restarts = 0;
                marks
            }
            Err(error) => {
                self.spell_failed(&error, &outcome.note, outcome.language);
                Vec::new()
            }
        };
        let open = self.open.as_mut()?;
        let tab = open.tabs.get_mut(index);
        tab.spell.marks.clone_from(&marks);
        Some(Event::SpellMarks(SpellMarksView {
            note: outcome.note.as_str().to_owned(),
            rev: outcome.rev,
            marks,
        }))
    }

    /// UC-005 BR-023: says once per language that it has no dictionary, or
    /// why its dictionary cannot be read, and keeps it in the status bar;
    /// a check that failed is said for its note.
    fn spell_failed(&mut self, error: &SpellError, note: &NoteId, code: &'static str) {
        if let SpellError::Failed(why) = error {
            self.status = format!("{} could not be spell checked: {why}", note.title());
            return;
        }
        // The language the note was checked in, which is the one in effect
        // (an outcome of another was dropped before).
        let Some(language) = SpellCheckLanguage::ALL
            .iter()
            .copied()
            .find(|language| language.dictionary() == Some(code))
        else {
            return;
        };
        if self.spell.said.insert(code) {
            let (problem, detail) = match error {
                SpellError::Missing { .. } => (
                    format!("Spell check: {}, no dictionary", language.label()),
                    format!(
                        "No dictionary for {}. Choose another language in the settings.",
                        language.label()
                    ),
                ),
                other => (
                    format!("Spell check: {}, dictionary unreadable", language.label()),
                    format!(
                        "The dictionary for {} cannot be read: {other}. Choose another language in the settings.",
                        language.label()
                    ),
                ),
            };
            self.status.clone_from(&detail);
            self.spell.state = SpellStateView {
                problem: Some(problem),
                detail: Some(detail),
            };
        }
    }

    /// Gives the check thread the open notes whose marks are not of their
    /// text, the shown ones first; with the spell check off, their marks go.
    fn start_spell_checks(&mut self, now: Instant) -> Vec<Event> {
        let mut events = Vec::new();
        let generation = self.spell.generation();
        let dictionary = self.spell.dictionary;
        let language = self.spell.language.dictionary();
        let user = Arc::clone(&self.spell.user);
        let stopped = self.spell.stopped();
        let mut next_instance = self.spell.next_instance;
        let Some(open) = self.open.as_mut() else {
            return events;
        };
        let mut order: Vec<NoteId> = open.panes.shown_notes();
        for (note, _) in open.tabs.iter() {
            if !order.contains(note) {
                order.push(note.clone());
            }
        }
        let mut jobs = Vec::new();
        for note in order {
            let Some(index) = open.tabs.find(&note) else {
                continue;
            };
            let tab = open.tabs.get_mut(index);
            let rev = tab.mirror.rev();
            let large = tab.mirror.rope().len_bytes() > LARGE_NOTE;
            let spell = &mut tab.spell;
            if spell.instance == 0 {
                next_instance += 1;
                spell.instance = next_instance;
            }
            if spell.seen_rev != Some(rev) {
                // A change not confirmed through `ed_sync` (an agent's, a
                // rename's) counts from when the tick sees it.
                if spell.seen_rev.is_some() && spell.last_edit.is_none() {
                    spell.edited(now);
                }
                spell.seen_rev = Some(rev);
            }
            if spell.running || spell.told == Some((rev, generation)) {
                continue;
            }
            let Some(language) = language.filter(|_| !stopped) else {
                // Off: no word is marked; the window is told once.
                let first_time = spell.told.is_none_or(|(_, told)| told != generation);
                spell.told = Some((rev, generation));
                spell.first_edit = None;
                if first_time || !spell.marks.is_empty() {
                    spell.marks.clear();
                    events.push(Event::SpellMarks(SpellMarksView {
                        note: note.as_str().to_owned(),
                        rev,
                        marks: Vec::new(),
                    }));
                }
                continue;
            };
            let (rest, latest) = if large {
                (LARGE_DEBOUNCE, LARGE_AT_THE_LATEST)
            } else {
                (SPELL_DEBOUNCE, SPELL_AT_THE_LATEST)
            };
            // Not tried in this generation (a note just opened, another
            // language, other words): checked at once; else after the rest.
            let fresh = spell.tried != Some(generation);
            let rested = spell
                .last_edit
                .is_none_or(|at| now.saturating_duration_since(at) >= rest);
            let overdue = spell
                .first_edit
                .is_some_and(|at| now.saturating_duration_since(at) >= latest);
            if !(fresh || rested || overdue) {
                continue;
            }
            spell.running = true;
            spell.tried = Some(generation);
            spell.first_edit = None;
            spell.last_edit = None;
            let instance = spell.instance;
            let rope = tab.mirror.rope().clone();
            let first = first_part(&rope, tab.viewport.clone(), tab.edit.editor.cursor());
            jobs.push(Job {
                note,
                instance,
                rev,
                generation,
                dictionary,
                language,
                user: Arc::clone(&user),
                rope,
                format: tab.edit.format,
                first: Some(first),
            });
        }
        self.spell.next_instance = next_instance;
        // The spellers of tabs closed since go.
        let mut live: Vec<u64> = open
            .tabs
            .iter()
            .map(|(_, tab)| tab.spell.instance)
            .collect();
        live.sort_unstable();
        if live != self.spell.live && self.spell.work.is_some() {
            self.spell.live.clone_from(&live);
            self.send_spell_work(Work::Retain(live));
        }
        for job in jobs {
            self.send_spell_work(Work::Check(Box::new(job)));
        }
        events
    }

    /// Gives `work` to the check thread, starting it when it is not running.
    fn send_spell_work(&mut self, work: Work) {
        if self.spell.work.is_none() {
            let (sender, done) = start_thread(
                Arc::clone(&self.spell.dictionaries),
                Arc::clone(&self.spell.generation),
                self.wake.clone(),
            );
            self.spell.work = sender;
            self.spell.done = done;
        }
        let sent = self
            .spell
            .work
            .as_ref()
            .is_some_and(|sender| sender.send(work).is_ok());
        if !sent {
            // The thread could not start or ended: started again later.
            self.spell_thread_ended();
        }
    }
}

/// The bytes of `rope` checked first: the lines the source view shows, else
/// the text around the cursor (char index `cursor`).
fn first_part(rope: &ropey::Rope, lines: Option<Range<usize>>, cursor: usize) -> Range<usize> {
    let len = rope.len_bytes();
    match lines {
        Some(lines) => {
            let count = rope.len_lines();
            let start = rope.line_to_byte(lines.start.min(count));
            let end = rope.line_to_byte(lines.end.min(count));
            start..end.max(start)
        }
        None => {
            let at = rope.char_to_byte(cursor.min(rope.len_chars()));
            at.saturating_sub(AROUND_CURSOR.0)..(at + AROUND_CURSOR.1).min(len)
        }
    }
}

/// The time and size of the file at `path`; `None` when there is none.
fn stamp(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok()?;
    // A system without modification times: the size alone tells.
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    Some((modified, meta.len()))
}

/// Starts the check thread; `None`s when it cannot start.
fn start_thread(
    dictionaries: Arc<Dictionaries>,
    generation: Arc<AtomicU64>,
    wake: Wake,
) -> (Option<Sender<Work>>, Option<Receiver<Done>>) {
    let (work, take) = mpsc::channel::<Work>();
    let (give, done) = mpsc::channel::<Done>();
    let started = std::thread::Builder::new()
        .name("underline-spell".into())
        .spawn(move || run_thread(&dictionaries, &generation, &take, &give, &wake));
    match started {
        Ok(_) => (Some(work), Some(done)),
        Err(_) => (None, None),
    }
}

/// One speller per tab (its instance) and language, with the dictionary
/// generation it was made for.
type Spellers = HashMap<(u64, &'static str), (u64, NoteSpeller)>;

/// The check thread: does the work one after the other and sends what it
/// found; ends when the session lets go of it. A tab has at most one check
/// at a time (`TabSpell::running`). A fault while checking is caught and
/// answered as a failed check.
fn run_thread(
    dictionaries: &Dictionaries,
    generation: &AtomicU64,
    work: &Receiver<Work>,
    done: &Sender<Done>,
    wake: &Wake,
) {
    let mut spellers: Spellers = HashMap::new();
    while let Ok(next) = work.recv() {
        let answer = match next {
            Work::ReadWords(path, seen) => {
                Done::Words(UserWords::load(&path).map_err(|e| e.to_string()), seen)
            }
            Work::Retain(live) => {
                spellers.retain(|(instance, _), _| live.binary_search(instance).is_ok());
                continue;
            }
            Work::Check(job) => Done::Checked(check_caught(
                dictionaries,
                generation,
                &mut spellers,
                *job,
                done,
                wake,
            )),
        };
        if done.send(answer).is_err() {
            return;
        }
        wake.wake();
    }
}

/// [`check`], a fault caught: the note's speller is dropped and the check
/// answered as failed.
fn check_caught(
    dictionaries: &Dictionaries,
    generation: &AtomicU64,
    spellers: &mut Spellers,
    job: Job,
    done: &Sender<Done>,
    wake: &Wake,
) -> Outcome {
    let failed = Outcome {
        note: job.note.clone(),
        language: job.language,
        instance: job.instance,
        rev: job.rev,
        generation: job.generation,
        partial: false,
        marks: Err(Arc::new(SpellError::Failed(
            "the spell check met a fault in this text".to_owned(),
        ))),
    };
    let key = (job.instance, job.language);
    let caught = panic::catch_unwind(AssertUnwindSafe(|| {
        check(dictionaries, generation, spellers, job, done, wake)
    }));
    caught.unwrap_or_else(|_| {
        spellers.remove(&key);
        failed
    })
}

/// Checks the note of `job`; the part in view goes out first. Work of an
/// older generation is answered as dropped.
fn check(
    dictionaries: &Dictionaries,
    generation: &AtomicU64,
    spellers: &mut Spellers,
    job: Job,
    done: &Sender<Done>,
    wake: &Wake,
) -> Outcome {
    let answer = |marks| Outcome {
        note: job.note.clone(),
        language: job.language,
        instance: job.instance,
        rev: job.rev,
        generation: job.generation,
        partial: false,
        marks,
    };
    if job.generation < generation.load(Ordering::SeqCst) {
        // The session checks the note again for what changed.
        return answer(Ok(Vec::new()));
    }
    let checker = match dictionaries.get(job.language) {
        Ok(checker) => checker,
        Err(error) => return answer(Err(error)),
    };
    let key = (job.instance, job.language);
    let renew = spellers
        .get(&key)
        .is_none_or(|(made_for, _)| *made_for != job.dictionary);
    if renew {
        spellers.retain(|_, (made_for, _)| *made_for == job.dictionary);
        if spellers.len() > 64 {
            spellers.clear();
        }
        spellers.insert(
            key,
            (
                job.dictionary,
                NoteSpeller::new(checker, Arc::clone(&job.user)),
            ),
        );
    }
    let Some((_, speller)) = spellers.get_mut(&key) else {
        return answer(Ok(Vec::new()));
    };
    // The user's words change without what the dictionary said.
    speller.set_user_words(Arc::clone(&job.user));
    let text = job.rope.to_string();
    let offsets = WebOffsets::of(&job.rope);
    let rope = &job.rope;
    let web = |marks: &[Range<usize>]| -> Vec<(u32, u32)> {
        marks
            .iter()
            .take(SpellMarksView::MAX)
            .map(|m| {
                let from = offsets.to_web(rope, rope.byte_to_char(m.start));
                let to = offsets.to_web(rope, rope.byte_to_char(m.end));
                (units(from), units(to))
            })
            .collect()
    };
    let mut early = |marks: &[Range<usize>]| {
        let _ = done.send(Done::Checked(Outcome {
            note: job.note.clone(),
            language: job.language,
            instance: job.instance,
            rev: job.rev,
            generation: job.generation,
            partial: true,
            marks: Ok(web(marks)),
        }));
        wake.wake();
    };
    let marks = speller.check_note(&text, job.format, job.first.clone(), &mut early);
    answer(Ok(web(&marks)))
}

#[cfg(test)]
mod tests;
