//! UC-005 A28, entity DICTIONARY_WORD, ADR-030: the words the user added to
//! the spell check. They are a plain UTF-8 file next to the user settings
//! file, one word per line, sorted, written atomically (ADR-005); a word in
//! it is never marked, in any workspace and any language.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use underline_core::write_atomic;

use crate::checker::decapitalized;
use crate::SpellError;

/// The name of the file of the user's words, next to `underline.yaml`.
pub const USER_WORDS_FILE_NAME: &str = "underline-words.txt";

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

    /// Reads the file at `path`; a missing file holds no words.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read, is larger than
    /// [`MAX_WORDS_FILE_SIZE`], or is not UTF-8.
    pub fn load(path: &Path) -> Result<Self, SpellError> {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > MAX_WORDS_FILE_SIZE {
                return Err(SpellError::Read {
                    path: path.to_owned(),
                    source: io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("it is larger than {} MB", MAX_WORDS_FILE_SIZE / 1024 / 1024),
                    ),
                });
            }
        }
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(SpellError::Read {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Writes the words to `path` at once, never half (ADR-005).
    ///
    /// # Errors
    ///
    /// The file cannot be written.
    pub fn store(&self, path: &Path) -> Result<(), SpellError> {
        write_atomic(path, self.to_text().as_bytes()).map_err(|source| SpellError::Write {
            path: path.to_owned(),
            source,
        })
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
}
