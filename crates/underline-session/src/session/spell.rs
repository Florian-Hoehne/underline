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
//! - OQ-208: "Add to workspace dictionary" stores the word in
//!   `underline-words.txt` in the root of the workspace, committed with it;
//!   that file is followed the same way, and a word that could not be
//!   stored there counts until the workspace is left. The words in effect
//!   are the dictionary's, the user's and the workspace's.
//! - OQ-207: a note's attribute `lang` overrides the language of the
//!   settings for that note; it is read on the check thread from the text
//!   checked, so the marks follow a change of it. A language Underline does
//!   not offer marks nothing, and the status bar says "No dictionary for …"
//!   while that note is the active one.

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
use underline_spell::{
    note_language, Dictionaries, NoteLanguage, NoteSpeller, SpellError, UserWords, LANG_HEAD_BYTES,
    USER_WORDS_FILE_NAME, WORKSPACE_WORDS_FILE_NAME,
};

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

/// ADR-030 (update 2026-11-16): how often the dictionaries no open note and
/// no setting uses are let go.
pub(super) const DICTIONARY_SWEEP: Duration = Duration::from_secs(60);

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
    /// OQ-207: the note was last checked in the language its attribute
    /// `lang` names, not in that of the settings.
    pub(super) from_note: bool,
    /// OQ-207: what the status bar shows while the note is the active one
    /// and checked in its own language (a language without a dictionary).
    pub(super) problem: Option<SpellStateView>,
    /// The code of the language the note was last checked in, so its
    /// dictionary is kept while the tab is open.
    pub(super) language: Option<String>,
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

/// The language a note is checked in.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckLanguage {
    /// The language of the settings, by the code of its dictionary.
    Settings(&'static str),
    /// OQ-207: an offered language the note's attribute `lang` names.
    Note(&'static str),
    /// OQ-207: a language the note names that Underline does not offer, as
    /// written; it has no dictionary.
    Unknown(String),
}

impl CheckLanguage {
    /// OQ-207: the language of `text`, a note in `format`, checked while the
    /// settings say `setting`.
    fn of(setting: &'static str, text: &str, format: NoteFormat) -> Self {
        match note_language(text, format) {
            NoteLanguage::Settings => Self::Settings(setting),
            NoteLanguage::Offered(language) => match language.dictionary() {
                Some(code) if code != setting => Self::Note(code),
                _ => Self::Settings(setting),
            },
            NoteLanguage::Unknown(tag) => Self::Unknown(tag),
        }
    }

    /// The code of its dictionary, or the language as the note writes it.
    fn code(&self) -> &str {
        match self {
            Self::Settings(code) | Self::Note(code) => code,
            Self::Unknown(tag) => tag,
        }
    }

    /// The note names it, not the settings.
    fn is_the_notes(&self) -> bool {
        !matches!(self, Self::Settings(_))
    }

    /// The name the status bar uses for it: the name of an offered
    /// language, else the language as written.
    fn name(&self) -> String {
        SpellCheckLanguage::ALL
            .iter()
            .find(|language| language.dictionary() == Some(self.code()))
            .map_or_else(
                || self.code().to_owned(),
                |language| language.label().to_owned(),
            )
    }
}

/// Whose words a words file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordsOf {
    /// The user's, next to the user settings file (UC-005 A28).
    User,
    /// The workspace's, in its root (OQ-208).
    Workspace,
}

impl WordsOf {
    /// How long a word that could not be stored counts.
    fn until(self) -> &'static str {
        match self {
            Self::User => "until Underline closes",
            Self::Workspace => "until the workspace is closed",
        }
    }

    /// Whose words, for the status bar.
    fn whose(self) -> &'static str {
        match self {
            Self::User => "the words of your dictionary",
            Self::Workspace => "the words of the workspace dictionary",
        }
    }

    /// UC-005 A28, OQ-208: the status bar once `word` was added.
    fn added(self, word: &str) -> String {
        match self {
            Self::User => format!("Added \"{word}\" to your dictionary"),
            Self::Workspace => format!("Added \"{word}\" to the workspace dictionary"),
        }
    }

    /// The status bar when the file could not be read.
    fn unreadable(self, error: &str) -> String {
        match self {
            Self::User => format!("The words of your dictionary could not be read: {error}"),
            Self::Workspace => {
                format!("The words of the workspace dictionary could not be read: {error}")
            }
        }
    }
}

/// A file of words the session follows (the user's or the workspace's).
#[derive(Debug, Default)]
struct WordsFile {
    /// Where it is; `None` without a home directory, or without a workspace.
    path: Option<PathBuf>,
    /// OQ-208: the root of the workspace it is in, below which no link is
    /// followed (a cloned workspace is not trusted); `None` for the user's
    /// file, which may be a link the user made.
    root: Option<PathBuf>,
    /// OQ-208: the workspace's file is the user's file (a workspace in the
    /// folder of the user settings file): it is read and followed once, as
    /// the user's; this one holds no words of its own.
    same_as_user: bool,
    /// Its words, as read on the check thread.
    words: UserWords,
    /// UC-005 A28: words added that could not be stored; they count until
    /// Underline closes (the user's) or the workspace is left (the
    /// workspace's), whatever the file holds.
    unstored: BTreeSet<String>,
    /// It was read once (or there is none).
    ready: bool,
    /// A read was asked for and not answered yet.
    reading: bool,
    /// Its time and size when last read.
    seen: Stamp,
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
    /// The language of the settings; the note may name another (OQ-207).
    setting: &'static str,
    user: Arc<UserWords>,
    rope: ropey::Rope,
    format: NoteFormat,
    first: Option<Range<usize>>,
}

/// What the check thread is asked to do.
enum Work {
    Check(Box<Job>),
    /// Read a words file, last seen with this time and size.
    ReadWords(WordsOf, Option<PathBuf>, PathBuf, Stamp),
    /// Only the tabs of these instances are open: the others' spellers go.
    Retain(Vec<u64>),
}

/// The time and size of a file, to tell that it changed.
type Stamp = Option<(SystemTime, u64)>;

/// What the check thread found.
struct Outcome {
    note: NoteId,
    /// The language the note was checked in.
    language: CheckLanguage,
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
    Words(WordsOf, PathBuf, Result<UserWords, String>, Stamp),
}

/// The spell check of the session.
pub(super) struct Spell {
    dictionaries: Arc<Dictionaries>,
    /// The words in effect besides the dictionary's: those of the user's
    /// file and of the workspace's, and those not stored.
    user: Arc<UserWords>,
    /// UC-005 A28: the user's words file.
    user_file: WordsFile,
    /// OQ-208: the workspace's words file.
    workspace_file: WordsFile,
    /// When the words files were last looked at.
    user_checked: Option<Instant>,
    /// When the dictionaries not in use were last let go.
    swept: Option<Instant>,
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
    said: HashSet<String>,
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
            user_file: WordsFile::default(),
            workspace_file: WordsFile::default(),
            user_checked: None,
            swept: None,
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

    /// OQ-208: the words in effect are those of the user's file and of the
    /// workspace's, and those not stored; a change checks every note again.
    fn rebuild_user(&mut self) {
        let mut words = self.user_file.words.clone();
        words.extend(&self.workspace_file.words);
        for word in self
            .user_file
            .unstored
            .iter()
            .chain(&self.workspace_file.unstored)
        {
            let _ = words.add(word);
        }
        if *self.user != words {
            self.user = Arc::new(words);
            self.bump();
        }
    }

    fn file_mut(&mut self, which: WordsOf) -> &mut WordsFile {
        match which {
            WordsOf::User => &mut self.user_file,
            WordsOf::Workspace => &mut self.workspace_file,
        }
    }

    /// Both words files were read once (or there are none).
    fn words_ready(&self) -> bool {
        self.user_file.ready && self.workspace_file.ready
    }

    /// No read of a words file is waited for (the thread was replaced or
    /// ended).
    fn forget_reads(&mut self) {
        self.user_file.reading = false;
        self.workspace_file.reading = false;
    }
}

/// UC-005 A28: the corrections for a word, worked out without the session.
pub struct SuggestJob {
    dictionaries: Arc<Dictionaries>,
    /// `None` while the spell check is off.
    language: Option<CheckLanguage>,
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
        if let CheckLanguage::Unknown(tag) = &language {
            let missing = SpellError::Missing {
                language: tag.clone(),
            };
            return Err(UiError::new(UiErrorKind::Internal, missing.to_string()));
        }
        match self.dictionaries.get(language.code()) {
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
        self.spell.forget_reads();
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

    /// OQ-208: the workspace's words file: in the root of the open
    /// workspace; `None` without one.
    fn workspace_words_path(&self) -> Option<PathBuf> {
        self.open
            .as_ref()
            .map(|open| open.root().join(WORKSPACE_WORDS_FILE_NAME))
    }

    /// OQ-208: the workspace's words file follows the open workspace, as
    /// one opens or closes (called on every tick); the words of one left,
    /// the stored and the unstored, count no more.
    fn follow_workspace_words(&mut self) {
        let path = self.workspace_words_path();
        if path != self.spell.workspace_file.path {
            let root = self.open.as_ref().map(|open| open.root().to_owned());
            let user = self.user_words_path();
            let same_as_user = same_file(path.as_deref(), user.as_deref());
            self.spell.workspace_file = WordsFile {
                path,
                root,
                same_as_user,
                ..WordsFile::default()
            };
            self.spell.rebuild_user();
        }
    }

    /// Looks at the time and size of the words files at most every
    /// [`USER_WORDS_POLL`], while the spell check is on and a note is open;
    /// when they changed, the check thread reads them.
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
        if !due {
            return;
        }
        self.spell.user_checked = Some(now);
        self.spell.user_file.path = self.user_words_path();
        self.poll_words_file(WordsOf::User);
        self.poll_words_file(WordsOf::Workspace);
    }

    /// Asks the check thread to read the words file `which` when its time
    /// or size changed.
    fn poll_words_file(&mut self, which: WordsOf) {
        let file = self.spell.file_mut(which);
        if file.reading {
            return;
        }
        let Some(path) = file.path.clone().filter(|_| !file.same_as_user) else {
            file.ready = true;
            return;
        };
        let stamp = stamp(&path);
        if file.ready && stamp == file.seen {
            return;
        }
        if stamp.is_none() {
            // No file: no words of the file; those not stored stay.
            file.seen = None;
            file.words = UserWords::default();
            file.ready = true;
            self.spell.rebuild_user();
            return;
        }
        file.reading = true;
        let root = file.root.clone();
        self.send_spell_work(Work::ReadWords(which, root, path, stamp));
    }

    /// The words of the file `which` at `path` as the check thread read
    /// them; a read of a workspace left meanwhile is dropped.
    fn take_user_words(
        &mut self,
        which: WordsOf,
        path: &Path,
        read: Result<UserWords, String>,
        seen: Stamp,
    ) {
        let file = self.spell.file_mut(which);
        if file.path.as_deref() != Some(path) {
            return;
        }
        file.reading = false;
        file.ready = true;
        file.seen = seen;
        match read {
            Ok(words) => {
                file.words = words;
                self.spell.rebuild_user();
            }
            Err(error) => self.status = which.unreadable(&error),
        }
    }

    /// UC-005 A28, OQ-207: the job that finds the corrections for `word`
    /// of the open note `note` (in the language it names, else in that of
    /// the settings), run off the session.
    #[must_use]
    pub fn spell_suggest_job(&self, word: &str, note: Option<&NoteId>) -> SuggestJob {
        let setting = self.settings.effective().spell_check_language.dictionary();
        let language = setting.map(|setting| {
            let tab = note.and_then(|note| {
                let open = self.open.as_ref()?;
                let index = open.tabs.find(note)?;
                Some(open.tabs.get(index))
            });
            match tab {
                Some(tab) => {
                    let rope = tab.mirror.rope();
                    let end = rope.byte_to_char(rope.len_bytes().min(LANG_HEAD_BYTES));
                    let head = rope.slice(..end).to_string();
                    CheckLanguage::of(setting, &head, tab.edit.format)
                }
                None => CheckLanguage::Settings(setting),
            }
        });
        SuggestJob {
            dictionaries: Arc::clone(&self.spell.dictionaries),
            language,
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
        self.add_spell_word(WordsOf::User, word)
    }

    /// OQ-208, UC-005 A28 step 2: adds `word` to the workspace dictionary,
    /// `underline-words.txt` in the root of the workspace, as
    /// [`Session::spell_add_word`] adds it to the user's: read first, the
    /// hand edits kept, never written over when it cannot be read; a word
    /// that cannot be stored counts until the workspace is left.
    ///
    /// # Errors
    ///
    /// No workspace is open, the word cannot be added, or the file cannot
    /// be read or written; the status bar says why.
    pub fn spell_add_workspace_word(&mut self, word: &str) -> Result<(), UiError> {
        self.add_spell_word(WordsOf::Workspace, word)
    }

    /// Adds `word` to the words file `which`: read again and merged under
    /// a lock right before it is written ([`UserWords::store`]); the
    /// workspace's never through a link.
    fn add_spell_word(&mut self, which: WordsOf, word: &str) -> Result<(), UiError> {
        let invalid = |error: SpellError| UiError::new(UiErrorKind::Invalid, error.to_string());
        let mut words = UserWords::default();
        words.add(word).map_err(invalid)?;
        let word = word.trim().to_owned();
        let (path, root) = match which {
            WordsOf::User => (self.user_words_path(), None),
            WordsOf::Workspace => {
                self.follow_workspace_words();
                let file = &self.spell.workspace_file;
                (file.path.clone(), file.root.clone())
            }
        };
        let Some(path) = path else {
            if which == WordsOf::Workspace {
                return Err(UiError::new(
                    UiErrorKind::Invalid,
                    "No workspace is open".to_owned(),
                ));
            }
            self.spell.user_file.ready = true;
            self.add_word_for_now(which, &word);
            self.status = format!(
                "\"{word}\" is not marked until Underline closes: there is no home directory to store it in"
            );
            return Ok(());
        };
        self.spell.file_mut(which).ready = true;
        self.wake.wake();
        let stored = match &root {
            Some(root) => words.store_in(root, &path),
            None => words.store(&path),
        };
        match stored {
            Ok(stored) => {
                let seen = stamp(&path);
                // One file for both: the user's holds its words.
                let into = if self.spell.workspace_file.same_as_user {
                    WordsOf::User
                } else {
                    which
                };
                let file = self.spell.file_mut(into);
                file.seen = seen;
                file.words = stored;
                self.spell.rebuild_user();
                self.status = which.added(&word);
                Ok(())
            }
            Err(error) => {
                self.add_word_for_now(which, &word);
                let until = which.until();
                let reason = if matches!(error, SpellError::Read { .. }) {
                    format!(
                        "\"{word}\" is not marked {until}: {} could not be read, so they are not written over ({error})",
                        which.whose()
                    )
                } else {
                    format!("\"{word}\" is not marked {until}, as it could not be stored: {error}")
                };
                self.status.clone_from(&reason);
                Err(UiError::new(UiErrorKind::Internal, reason))
            }
        }
    }

    /// `word` counts as one of `which` until Underline closes (the user's)
    /// or the workspace is left (the workspace's), whatever the file holds;
    /// nothing is stored.
    fn add_word_for_now(&mut self, which: WordsOf, word: &str) {
        self.spell.file_mut(which).unstored.insert(word.to_owned());
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
        self.sweep_dictionaries(now);
        self.follow_workspace_words();
        self.poll_user_words(now);
        // The user's words are needed only to mark words.
        if self.spell.words_ready() || self.spell.language == SpellCheckLanguage::Off {
            events.extend(self.start_spell_checks(now));
        }
        let shown = self.shown_spell_state();
        if self.spell.told_state.as_ref() != Some(&shown) {
            self.spell.told_state = Some(shown.clone());
            events.push(Event::SpellState(shown));
        }
        events
    }

    /// ADR-030 (update 2026-11-16): every [`DICTIONARY_SWEEP`], lets go of
    /// the dictionaries neither the setting nor an open note uses, and of
    /// those that could not be read.
    fn sweep_dictionaries(&mut self, now: Instant) {
        // The first sweep comes a while after the start, once the open
        // notes were checked and know their language.
        let swept = *self.spell.swept.get_or_insert(now);
        if now.saturating_duration_since(swept) < DICTIONARY_SWEEP {
            return;
        }
        self.spell.swept = Some(now);
        let mut keep: Vec<&str> = self.spell.language.dictionary().into_iter().collect();
        if let Some(open) = self.open.as_ref() {
            keep.extend(
                open.tabs
                    .iter()
                    .filter_map(|(_, tab)| tab.spell.language.as_deref()),
            );
        }
        self.spell.dictionaries.retain(&keep);
    }

    /// FR-100, OQ-207: what the status bar shows of the spell check: the
    /// problem of the active note while it is checked in the language it
    /// names, else that of the language of the settings.
    fn shown_spell_state(&self) -> SpellStateView {
        if self.spell.stopped() || self.spell.language == SpellCheckLanguage::Off {
            return self.spell.state.clone();
        }
        let active = self.open.as_ref().and_then(|open| {
            open.tabs
                .active_index()
                .map(|index| &open.tabs.get(index).spell)
        });
        match active {
            Some(spell) if spell.from_note => spell.problem.clone().unwrap_or(SpellStateView {
                problem: None,
                detail: None,
            }),
            _ => self.spell.state.clone(),
        }
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
                Done::Words(which, path, read, seen) => {
                    self.take_user_words(which, &path, read, seen);
                }
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
        self.spell.forget_reads();
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
        let from_note = outcome.language.is_the_notes();
        let code = outcome.language.code().to_owned();
        let (marks, problem) = match outcome.marks {
            Ok(marks) => {
                // A check done: the thread works again.
                self.spell.restarts = 0;
                (marks, None)
            }
            Err(error) => {
                let problem = self.spell_failed(&error, &outcome.note, &outcome.language);
                (Vec::new(), problem)
            }
        };
        let open = self.open.as_mut()?;
        let tab = open.tabs.get_mut(index);
        tab.spell.marks.clone_from(&marks);
        tab.spell.from_note = from_note;
        tab.spell.problem = problem;
        tab.spell.language = Some(code);
        Some(Event::SpellMarks(SpellMarksView {
            note: outcome.note.as_str().to_owned(),
            rev: outcome.rev,
            marks,
        }))
    }

    /// UC-005 BR-023: says once per language that it has no dictionary, or
    /// why its dictionary cannot be read, and keeps it in the status bar;
    /// a check that failed is said for its note. OQ-207: of a language the
    /// note names, what the status bar shows while the note is the active
    /// one is answered, not kept for every note.
    fn spell_failed(
        &mut self,
        error: &SpellError,
        note: &NoteId,
        language: &CheckLanguage,
    ) -> Option<SpellStateView> {
        if let SpellError::Failed(why) = error {
            self.status = format!("{} could not be spell checked: {why}", note.title());
            return None;
        }
        let name = language.name();
        if language.is_the_notes() {
            // OQ-207: the chip names the note; the status bar shows it only
            // while the note is the active one, so nothing else is said.
            let title = note.title();
            let (problem, detail) = match error {
                SpellError::Missing { .. } => (
                    format!("{title}: no dictionary for {name}"),
                    format!(
                        "No dictionary for {name}, the language this note names in its attribute `lang`. Change or remove the attribute."
                    ),
                ),
                other => (
                    format!("{title}: dictionary for {name} unreadable"),
                    format!(
                        "The dictionary for {name} cannot be read: {other}. Change or remove the note's attribute `lang`."
                    ),
                ),
            };
            return Some(SpellStateView {
                problem: Some(problem),
                detail: Some(detail),
            });
        }
        if self.spell.said.insert(language.code().to_owned()) {
            let (problem, detail) = match error {
                SpellError::Missing { .. } => (
                    format!("Spell check: {name}, no dictionary"),
                    format!("No dictionary for {name}. Choose another language in the settings."),
                ),
                other => (
                    format!("Spell check: {name}, dictionary unreadable"),
                    format!(
                        "The dictionary for {name} cannot be read: {other}. Choose another language in the settings."
                    ),
                ),
            };
            self.status.clone_from(&detail);
            self.spell.state = SpellStateView {
                problem: Some(problem),
                detail: Some(detail),
            };
        }
        None
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
                spell.from_note = false;
                spell.problem = None;
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
                setting: language,
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

/// OQ-208: whether `a` and `b` are the same file (also through another
/// spelling of its folder); `false` when either is `None`.
fn same_file(a: Option<&Path>, b: Option<&Path>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    if a == b {
        return true;
    }
    // The file need not exist yet: its folders decide.
    let real = |path: &Path| {
        let dir = path.parent()?.canonicalize().ok()?;
        Some(dir.join(path.file_name()?))
    };
    real(a).is_some_and(|a| Some(a) == real(b))
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

/// One speller per tab (its instance) and language (its code), with the
/// dictionary generation it was made for.
type Spellers = HashMap<(u64, String), (u64, NoteSpeller)>;

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
            Work::ReadWords(which, root, path, seen) => {
                let read = match &root {
                    Some(root) => UserWords::load_in(root, &path),
                    None => UserWords::load(&path),
                }
                .map_err(|e| e.to_string());
                Done::Words(which, path, read, seen)
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
        language: CheckLanguage::Settings(job.setting),
        instance: job.instance,
        rev: job.rev,
        generation: job.generation,
        partial: false,
        marks: Err(Arc::new(SpellError::Failed(
            "the spell check met a fault in this text".to_owned(),
        ))),
    };
    let instance = job.instance;
    let caught = panic::catch_unwind(AssertUnwindSafe(|| {
        check(dictionaries, generation, spellers, job, done, wake)
    }));
    caught.unwrap_or_else(|_| {
        spellers.retain(|(of, _), _| *of != instance);
        failed
    })
}

/// Checks the note of `job` in the language it names, else in that of the
/// settings (OQ-207); the part in view goes out first. Work of an older
/// generation is answered as dropped.
fn check(
    dictionaries: &Dictionaries,
    generation: &AtomicU64,
    spellers: &mut Spellers,
    job: Job,
    done: &Sender<Done>,
    wake: &Wake,
) -> Outcome {
    let answer = |language: CheckLanguage, marks| Outcome {
        note: job.note.clone(),
        language,
        instance: job.instance,
        rev: job.rev,
        generation: job.generation,
        partial: false,
        marks,
    };
    if job.generation < generation.load(Ordering::SeqCst) {
        // The session checks the note again for what changed.
        return answer(CheckLanguage::Settings(job.setting), Ok(Vec::new()));
    }
    let text = job.rope.to_string();
    let language = CheckLanguage::of(job.setting, &text, job.format);
    if let CheckLanguage::Unknown(tag) = &language {
        let missing = SpellError::Missing {
            language: tag.clone(),
        };
        return answer(language, Err(Arc::new(missing)));
    }
    let checker = match dictionaries.get(language.code()) {
        Ok(checker) => checker,
        Err(error) => return answer(language, Err(error)),
    };
    let key = (job.instance, language.code().to_owned());
    let renew = spellers
        .get(&key)
        .is_none_or(|(made_for, _)| *made_for != job.dictionary);
    if renew {
        // A tab keeps the speller of its language now only (OQ-207: its
        // `lang` changed), so the dictionary it held can go.
        spellers.retain(|(instance, code), (made_for, _)| {
            *made_for == job.dictionary && (*instance != job.instance || *code == key.1)
        });
        if spellers.len() > 64 {
            spellers.clear();
        }
        spellers.insert(
            key.clone(),
            (
                job.dictionary,
                NoteSpeller::new(checker, Arc::clone(&job.user)),
            ),
        );
    }
    let Some((_, speller)) = spellers.get_mut(&key) else {
        return answer(language, Ok(Vec::new()));
    };
    // The user's words change without what the dictionary said.
    speller.set_user_words(Arc::clone(&job.user));
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
            language: language.clone(),
            instance: job.instance,
            rev: job.rev,
            generation: job.generation,
            partial: true,
            marks: Ok(web(marks)),
        }));
        wake.wake();
    };
    let marks = speller.check_note(&text, job.format, job.first.clone(), &mut early);
    answer(language, Ok(web(&marks)))
}

#[cfg(test)]
mod tests;
