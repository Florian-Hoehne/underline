//! UC-005 A28, entity DICTIONARY_WORD, ADR-030: the words the user added to
//! the spell check. They are a plain UTF-8 file next to the user settings
//! file, one word per line, sorted, written atomically (ADR-005); a word in
//! it is never marked, in any workspace and any language.
//!
//! OQ-208: the workspace dictionary is the same kind of file in the root of
//! the workspace, committed with it; its words are not marked in that
//! workspace.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{link_below, write_atomic};

use crate::checker::decapitalized;
use crate::SpellError;

/// The name of the file of the user's words, next to `underline.yaml`.
pub const USER_WORDS_FILE_NAME: &str = "underline-words.txt";

/// OQ-208: the name of the workspace dictionary, in the root of the
/// workspace.
pub const WORKSPACE_WORDS_FILE_NAME: &str = "underline-words.txt";

/// Entity DICTIONARY_WORD: the longest word, in characters.
pub const MAX_WORD_LENGTH: usize = 100;

/// The largest words file read, in bytes; a larger one is no list of words.
pub const MAX_WORDS_FILE_SIZE: u64 = 2 * 1024 * 1024;

/// Whether `word` may stand in the user's words: not empty, no white space,
/// no control character or byte order mark, at most [`MAX_WORD_LENGTH`].
fn allowed(word: &str) -> bool {
    !word.is_empty()
        && word.chars().count() <= MAX_WORD_LENGTH
        && !word
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '\u{feff}')
}

/// The words the user added (entity DICTIONARY_WORD).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserWords {
    words: BTreeSet<String>,
    /// Lines of the file that are no word (written by hand: with a space,
    /// a control character, too long), kept as they are when it is written
    /// again, after the words.
    kept: Vec<String>,
}

impl UserWords {
    /// The words of `text`, one per line; empty lines and spaces around a
    /// word are left out, and so is a byte order mark; a line that is no
    /// word (too long, holding control characters) is left out too.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut parsed = Self::default();
        for line in text.lines() {
            let word = line.trim();
            if allowed(word) {
                parsed.words.insert(word.to_owned());
            } else if !word.is_empty() {
                parsed.kept.push(line.trim_end_matches('\r').to_owned());
            }
        }
        parsed
    }

    /// The text of the file: the words sorted, one per line.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for line in self.words.iter().chain(&self.kept) {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    /// OQ-208: adds the words of `other` (not its lines that are no word),
    /// so the words of several files count together.
    pub fn extend(&mut self, other: &UserWords) {
        self.words.extend(other.words.iter().cloned());
    }

    /// The words, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.words.iter().map(String::as_str)
    }

    /// How many words there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether there is none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// UC-005 BR-023: whether `word` is one of the user's words, as added,
    /// or with a capital at the start where it was added with a small
    /// letter (as at the start of a sentence).
    #[must_use]
    pub fn contains(&self, word: &str) -> bool {
        self.words.contains(word)
            || decapitalized(word).is_some_and(|lower| self.words.contains(&lower))
    }

    /// UC-005 A28 step 2: adds `word`; whether it was new.
    ///
    /// # Errors
    ///
    /// The word is empty, holds a space, or is longer than
    /// [`MAX_WORD_LENGTH`] characters (entity DICTIONARY_WORD).
    pub fn add(&mut self, word: &str) -> Result<bool, SpellError> {
        let word = word.trim();
        if word.is_empty() {
            return Err(SpellError::Word("There is no word to add".to_owned()));
        }
        if word.chars().any(char::is_whitespace) {
            return Err(SpellError::Word(format!(
                "\"{word}\" holds a space and cannot be added to the dictionary"
            )));
        }
        if word.chars().any(|c| c.is_control() || c == '\u{feff}') {
            return Err(SpellError::Word(
                "A word in the dictionary holds no control characters".to_owned(),
            ));
        }
        if word.chars().count() > MAX_WORD_LENGTH {
            return Err(SpellError::Word(format!(
                "A word in the dictionary has at most {MAX_WORD_LENGTH} characters"
            )));
        }
        Ok(self.words.insert(word.to_owned()))
    }

    /// Reads the file at `path`; a missing file holds no words. A link to
    /// the file is followed (the user's own file), but only a regular file
    /// is read, and never more than [`MAX_WORDS_FILE_SIZE`] bytes, so a
    /// pipe or a device never blocks or fills the reader.
    ///
    /// # Errors
    ///
    /// The file exists but is no regular file, cannot be read, is larger
    /// than [`MAX_WORDS_FILE_SIZE`], or is not UTF-8.
    pub fn load(path: &Path) -> Result<Self, SpellError> {
        Self::load_below(None, path)
    }

    /// OQ-208: reads the words file `path` of the workspace at `root`, a
    /// folder the user may have cloned: as [`UserWords::load`], and no part
    /// of `path` below `root` may be a symbolic link.
    ///
    /// # Errors
    ///
    /// As [`UserWords::load`], or a part of `path` is a link.
    pub fn load_in(root: &Path, path: &Path) -> Result<Self, SpellError> {
        Self::load_below(Some(root), path)
    }

    fn load_below(root: Option<&Path>, path: &Path) -> Result<Self, SpellError> {
        let read_error = |source| SpellError::Read {
            path: path.to_owned(),
            source,
        };
        if let Some(root) = root {
            refuse_links(root, path).map_err(read_error)?;
        }
        Ok(read_bounded(path)
            .map_err(read_error)?
            .map(|text| Self::parse(&text))
            .unwrap_or_default())
    }

    /// Adds the words of this list to the file at `path` at once, never
    /// half (ADR-005), and answers what the file holds then. The file is
    /// read again under a lock file next to it right before it is written,
    /// so words another Underline stored meanwhile stay; its lines that are
    /// no word stay too. A file that cannot be read is never written over.
    ///
    /// # Errors
    ///
    /// The file cannot be read again, the lock cannot be taken, or the file
    /// cannot be written.
    pub fn store(&self, path: &Path) -> Result<UserWords, SpellError> {
        self.store_below(None, path)
    }

    /// OQ-208: [`UserWords::store`] for the words file `path` of the
    /// workspace at `root`: no part of `path` below `root` may be a
    /// symbolic link, so a link is never replaced or written through.
    ///
    /// # Errors
    ///
    /// As [`UserWords::store`], or a part of `path` is a link.
    pub fn store_in(&self, root: &Path, path: &Path) -> Result<UserWords, SpellError> {
        self.store_below(Some(root), path)
    }

    fn store_below(&self, root: Option<&Path>, path: &Path) -> Result<UserWords, SpellError> {
        let write_error = |source| SpellError::Write {
            path: path.to_owned(),
            source,
        };
        if let Some(root) = root {
            refuse_links(root, path).map_err(write_error)?;
        }
        let _lock = FileLock::take(path).map_err(write_error)?;
        let mut merged = Self::load_below(root, path)?;
        merged.merge(self);
        write_atomic(path, merged.to_text().as_bytes()).map_err(write_error)?;
        Ok(merged)
    }

    /// The words and the kept lines of `other` added to these.
    fn merge(&mut self, other: &UserWords) {
        self.extend(other);
        for line in &other.kept {
            if !self.kept.contains(line) {
                self.kept.push(line.clone());
            }
        }
    }
}

/// OQ-208: an error when a part of `path` below `root` is a symbolic link.
fn refuse_links(root: &Path, path: &Path) -> io::Result<()> {
    match link_below(root, path) {
        Some(link) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} is a symbolic link, which Underline does not follow in a workspace",
                link.display()
            ),
        )),
        None => Ok(()),
    }
}

/// The text of the regular file at `path`, read at most
/// [`MAX_WORDS_FILE_SIZE`] bytes; `None` when there is none.
fn read_bounded(path: &Path) -> io::Result<Option<String>> {
    let not_a_file = || io::Error::new(io::ErrorKind::InvalidData, "it is no regular file");
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err(not_a_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(not_a_file());
    }
    let mut bytes = Vec::new();
    file.take(MAX_WORDS_FILE_SIZE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_WORDS_FILE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("it is larger than {} MB", MAX_WORDS_FILE_SIZE / 1024 / 1024),
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// How long a writer waits for the lock of a words file.
const LOCK_WAIT: Duration = Duration::from_secs(3);

/// A lock file older than this was left by a writer that ended; it is
/// taken over.
const LOCK_STALE: Duration = Duration::from_secs(10);

/// The lock of a words file while it is read and written again: the file
/// `<name>.lock` next to it, created only when there is none; removed when
/// dropped.
struct FileLock(PathBuf);

impl FileLock {
    fn take(path: &Path) -> io::Result<Self> {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let lock = path.with_file_name(name);
        let start = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&lock) {
                Ok(_) => return Ok(Self(lock)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::symlink_metadata(&lock)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|at| at.elapsed().ok())
                        .is_some_and(|age| age > LOCK_STALE);
                    if stale {
                        let _ = std::fs::remove_file(&lock);
                        continue;
                    }
                    if start.elapsed() > LOCK_WAIT {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "another Underline is writing it",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-spell-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn uc005_a28_the_users_words_file_round_trips_sorted() {
        let dir = scratch("round-trip");
        let path = dir.join(USER_WORDS_FILE_NAME);
        assert!(UserWords::load(&path).expect("missing is empty").is_empty());
        let mut words = UserWords::default();
        assert!(words.add("Kubernetes").unwrap());
        assert!(words.add("zettel").unwrap());
        assert!(words.add("Ärger").unwrap());
        assert!(words.add("abc").unwrap());
        assert!(!words.add("abc").unwrap(), "added once");
        words.store(&path).expect("stored");
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "Kubernetes\nabc\nzettel\nÄrger\n");
        assert_eq!(UserWords::load(&path).unwrap(), words);
        // No temporary file stays behind.
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(left.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oq208_the_words_of_two_files_count_together() {
        let mut words = UserWords::parse("Kubernetes\n");
        words.extend(&UserWords::parse("Zettelkasten\ntwo words\n"));
        assert_eq!(
            words.iter().collect::<Vec<_>>(),
            ["Kubernetes", "Zettelkasten"]
        );
        assert_eq!(words.to_text(), "Kubernetes\nZettelkasten\n");
    }

    #[test]
    fn uc005_a28_a_hand_written_file_is_read_leniently() {
        let words = UserWords::parse("\u{feff}  Kubernetes \r\n\r\nzettel\n");
        assert_eq!(words.iter().collect::<Vec<_>>(), ["Kubernetes", "zettel"]);
    }

    #[test]
    fn br023_a_word_added_small_is_known_at_the_start_of_a_sentence() {
        let words = UserWords::parse("zettel\nKubernetes\n");
        assert!(words.contains("zettel"));
        assert!(words.contains("Zettel"));
        assert!(words.contains("Kubernetes"));
        assert!(!words.contains("kubernetes"));
    }

    #[test]
    fn entity_dictionary_word_holds_no_space_and_at_most_100_characters() {
        let mut words = UserWords::default();
        assert!(words.add("two words").is_err());
        assert!(words.add("  ").is_err());
        assert!(words.add(&"a".repeat(101)).is_err());
        assert!(words.add(&"ä".repeat(100)).is_ok());
        assert!(words.add("a\u{0}b").is_err());
        assert!(words.add("\u{feff}word").is_err());
        assert!(words.add("a\u{7}").is_err());
        let long = "x".repeat(101);
        let read = UserWords::parse(&format!("ok\nbad\u{1}\n{long}\ntwo words\n"));
        assert_eq!(read.iter().collect::<Vec<_>>(), ["ok"]);
        // Lines that are no word stay in the file when it is written again.
        assert_eq!(read.to_text(), format!("ok\nbad\u{1}\n{long}\ntwo words\n"));
    }

    #[test]
    fn uc005_a28_a_words_file_too_large_or_no_utf8_is_not_read() {
        let dir = scratch("too-large");
        let path = dir.join(USER_WORDS_FILE_NAME);
        std::fs::write(&path, b"alpha\nbet\xE0\n").unwrap();
        assert!(UserWords::load(&path).is_err());
        let big = vec![b'a'; usize::try_from(MAX_WORDS_FILE_SIZE).unwrap() + 1];
        std::fs::write(&path, big).unwrap();
        assert!(UserWords::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn oq208_a_link_in_a_workspace_is_neither_read_nor_written() {
        let dir = scratch("link");
        let root = dir.join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.join("outside.txt");
        std::fs::write(&outside, "secret\n").unwrap();
        let path = root.join(WORKSPACE_WORDS_FILE_NAME);
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        assert!(UserWords::load_in(&root, &path).is_err());
        let mut words = UserWords::default();
        words.add("word").unwrap();
        assert!(words.store_in(&root, &path).is_err());
        assert!(std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "secret\n");
        // The user's own file may be a link (a dotfiles folder).
        assert_eq!(
            UserWords::load(&path).unwrap().iter().collect::<Vec<_>>(),
            ["secret"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn oq208_a_pipe_or_a_device_is_no_words_file_and_never_blocks() {
        let dir = scratch("fifo");
        let fifo = dir.join(WORKSPACE_WORDS_FILE_NAME);
        let made = std::process::Command::new("mkfifo").arg(&fifo).status();
        if made.is_ok_and(|status| status.success()) {
            assert!(UserWords::load_in(&dir, &fifo).is_err());
        }
        assert!(UserWords::load(Path::new("/dev/zero")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oq208_storing_adds_to_what_the_file_holds_now() {
        let dir = scratch("merge");
        let path = dir.join(WORKSPACE_WORDS_FILE_NAME);
        std::fs::write(&path, "alpha\nnot a word\n").unwrap();
        let mut mine = UserWords::default();
        mine.add("beta").unwrap();
        let stored = mine.store(&path).unwrap();
        assert_eq!(stored.to_text(), "alpha\nbeta\nnot a word\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stored.to_text());
        assert!(!dir.join("underline-words.txt.lock").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
