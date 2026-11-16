//! The spell check of Underline (FR-099, FR-100, UC-005 BR-023, A28,
//! ADR-030).
//!
//! - [`SpellChecker`] is what checks a word and suggests corrections;
//!   [`Hunspell`] reads a Hunspell dictionary with `spellbook`, and
//!   [`WordList`] is a plain list of words for tests.
//! - [`Dictionaries`] loads the dictionaries that come with Underline, one
//!   folder per language, the first time a language is needed, and keeps
//!   them.
//! - [`note_words`] finds the words of a note the spell check looks at: the
//!   text the facade shows, without code, link and web addresses, diagrams,
//!   formulas, names of note attributes, tags, words holding digits and
//!   words written only in capitals.
//! - [`NoteSpeller`] marks the words a dictionary does not know, block by
//!   block, keeping the marks of blocks it checked before.
//! - [`UserWords`] are the words the user added, never marked.
//!
//! Nothing here holds a window or a session: the session runs the check on
//! a thread of its own and sends the marks to the window.

mod checker;
mod dictionary;
mod marks;
mod user_words;
mod words;

pub use checker::{Hunspell, SpellChecker, WordList};
pub use dictionary::{decode, Dictionaries, DICTIONARY_DIR};
pub use marks::{NoteSpeller, MAX_SUGGESTED_LENGTH, MAX_SUGGESTIONS};
pub use user_words::{UserWords, MAX_WORDS_FILE_SIZE, MAX_WORD_LENGTH, USER_WORDS_FILE_NAME};
pub use words::{is_checked_word, note_words, BlockWords, Word};

use std::path::PathBuf;

/// What can go wrong with a dictionary or the user's words.
#[derive(Debug, thiserror::Error)]
pub enum SpellError {
    /// UC-005 BR-023: Underline holds no dictionary for the language.
    #[error("No dictionary for {language}")]
    Missing {
        /// The code of the language, such as `en-GB`.
        language: String,
    },
    /// A file could not be read.
    #[error("cannot read {}: {source}", .path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// A file could not be written.
    #[error("cannot write {}: {source}", .path.display())]
    Write {
        /// The file.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// The affix file names a character set that is not read.
    #[error("{} is in the character set {encoding}, which Underline does not read", .path.display())]
    Encoding {
        /// The file.
        path: PathBuf,
        /// The `SET` of the affix file.
        encoding: String,
    },
    /// The dictionary's files are not a Hunspell dictionary.
    #[error("{} is no dictionary Underline can read: {message}", .path.display())]
    Invalid {
        /// The folder of the dictionary.
        path: PathBuf,
        /// What the reader said.
        message: String,
    },
    /// UC-005 A28: the word cannot be added to the user's words.
    #[error("{0}")]
    Word(String),
    /// The check of a note failed (a fault of the checker or the parser);
    /// the note is not marked until it changes.
    #[error("{0}")]
    Failed(String),
}
