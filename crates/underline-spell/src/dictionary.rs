//! ADR-030: the dictionaries that come with Underline, as resource files:
//! one folder per language, named by its code (`en-US`), holding
//! `<code>.aff` and `<code>.dic` and their license texts. They are read when
//! a language is first needed, on the thread that needs it (the spell
//! check's own), and kept for the rest of the session. A dictionary whose
//! affix file says `SET ISO8859-1` or `SET ISO8859-15` is decoded when it is
//! read; nothing is converted when Underline is built.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use crate::checker::{Hunspell, SpellChecker};
use crate::SpellError;

/// The folder of the resources that holds the dictionaries.
pub const DICTIONARY_DIR: &str = "dictionaries";

/// A language's checker, or why there is none.
type Loaded = Result<Arc<dyn SpellChecker>, Arc<SpellError>>;

/// The dictionaries of the languages, each read the first time it is
/// needed.
pub struct Dictionaries {
    /// The folder holding one folder per language; `None` when Underline
    /// found no resources, so every language lacks its dictionary.
    dir: Option<PathBuf>,
    loaded: Mutex<HashMap<String, Arc<OnceLock<Loaded>>>>,
}

impl std::fmt::Debug for Dictionaries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dictionaries")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl Dictionaries {
    /// The dictionaries in the folder `dir`, one folder per language.
    #[must_use]
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// No dictionaries but those given with [`Dictionaries::with`].
    #[must_use]
    pub fn none() -> Self {
        Self::new(None)
    }

    /// For tests and the window's mock: `checker` is the dictionary of the
    /// language `code`.
    #[must_use]
    pub fn with(self, code: &str, checker: Arc<dyn SpellChecker>) -> Self {
        let slot = OnceLock::new();
        let _ = slot.set(Ok(checker));
        self.slots().insert(code.to_owned(), Arc::new(slot));
        self
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<OnceLock<Loaded>>>> {
        // A panic while the map was held leaves it whole: it only inserts.
        self.loaded.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn slot(&self, code: &str) -> Arc<OnceLock<Loaded>> {
        Arc::clone(
            self.slots()
                .entry(code.to_owned())
                .or_insert_with(|| Arc::new(OnceLock::new())),
        )
    }

    /// The checker of the language `code`, read now if it was not read yet;
    /// this blocks while the dictionary is read, so it is called off the
    /// session. A dictionary that could not be read is not read again.
    ///
    /// # Errors
    ///
    /// There is no dictionary for the language, or it cannot be read.
    pub fn get(&self, code: &str) -> Loaded {
        // A code names a folder of `dir`, never a place elsewhere: letters,
        // digits, `-` and `_` only.
        let allowed = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        if code.is_empty() || !code.chars().all(allowed) {
            return Err(Arc::new(SpellError::Missing {
                language: code.to_owned(),
            }));
        }
        let slot = self.slot(code);
        slot.get_or_init(|| match &self.dir {
            Some(dir) => load(dir, code)
                .map(|checker| Arc::new(checker) as Arc<dyn SpellChecker>)
                .map_err(Arc::new),
            None => Err(Arc::new(SpellError::Missing {
                language: code.to_owned(),
            })),
        })
        .clone()
    }

    /// ADR-030 (update 2026-11-16): lets go of the dictionaries of every
    /// language but those of `keep`, and of every dictionary that could not
    /// be read (it is tried again when it is needed again), so a language
    /// no open note and no setting uses any more frees its memory.
    pub fn retain(&self, keep: &[&str]) {
        self.slots().retain(|code, slot| {
            keep.contains(&code.as_str()) && slot.get().is_none_or(Result::is_ok)
        });
    }

    /// The checker of `code` when it was read already; `None` while it was
    /// not, so a caller that must not wait does not.
    #[must_use]
    pub fn ready(&self, code: &str) -> Option<Loaded> {
        self.slots().get(code).and_then(|slot| slot.get().cloned())
    }
}

/// Reads the dictionary of the language `code` in the folder `dir`:
/// `dir/<code>/<code>.aff` and `.dic`.
///
/// # Errors
///
/// The folder or a file is missing ([`SpellError::Missing`]), cannot be
/// read, is in a character set that is not read, or is no dictionary.
pub fn load(dir: &Path, code: &str) -> Result<Hunspell, SpellError> {
    let folder = dir.join(code);
    let aff_path = folder.join(format!("{code}.aff"));
    let dic_path = folder.join(format!("{code}.dic"));
    if !aff_path.is_file() || !dic_path.is_file() {
        return Err(SpellError::Missing {
            language: code.to_owned(),
        });
    }
    let read = |path: &Path| {
        std::fs::read(path).map_err(|source| SpellError::Read {
            path: path.to_owned(),
            source,
        })
    };
    let aff_bytes = read(&aff_path)?;
    let dic_bytes = read(&dic_path)?;
    let encoding = encoding_of(&aff_bytes);
    let aff = decode(&aff_bytes, &encoding, &aff_path)?;
    let dic = decode(&dic_bytes, &encoding, &dic_path)?;
    Hunspell::new(&aff, &dic).map_err(|message| SpellError::Invalid {
        path: folder,
        message,
    })
}

/// The character set the affix file `aff` names with `SET`; Hunspell reads
/// ISO 8859-1 without one.
fn encoding_of(aff: &[u8]) -> String {
    aff.split(|&b| b == b'\n')
        .map(|line| line.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(line))
        .find_map(|line| line.strip_prefix(b"SET "))
        .map_or_else(
            || "ISO8859-1".to_owned(),
            |rest| String::from_utf8_lossy(rest).trim().to_ascii_uppercase(),
        )
}

/// The characters of ISO 8859-15 that differ from ISO 8859-1.
const LATIN9: [(u8, char); 8] = [
    (0xA4, '€'),
    (0xA6, 'Š'),
    (0xA8, 'š'),
    (0xB4, 'Ž'),
    (0xB8, 'ž'),
    (0xBC, 'Œ'),
    (0xBD, 'œ'),
    (0xBE, 'Ÿ'),
];

/// ADR-030: the text of a dictionary's file `bytes` (at `path`) in the
/// character set `encoding` (as `SET` names it: `UTF-8`, `ISO8859-1`,
/// `ISO8859-15`), without a byte order mark.
///
/// # Errors
///
/// Another character set, or bytes that are no UTF-8 in a UTF-8 file.
pub fn decode(bytes: &[u8], encoding: &str, path: &Path) -> Result<String, SpellError> {
    let simple: String = encoding
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    match simple.as_str() {
        "UTF8" => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            String::from_utf8(bytes.to_vec()).map_err(|error| SpellError::Invalid {
                path: path.to_owned(),
                message: format!("not UTF-8 as its affix file says: {error}"),
            })
        }
        "ISO88591" | "LATIN1" => Ok(bytes.iter().map(|&b| char::from(b)).collect()),
        "ISO885915" | "LATIN9" => Ok(bytes
            .iter()
            .map(|&b| {
                LATIN9
                    .iter()
                    .find(|(byte, _)| *byte == b)
                    .map_or(char::from(b), |(_, c)| *c)
            })
            .collect()),
        _ => Err(SpellError::Encoding {
            path: path.to_owned(),
            encoding: encoding.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
    }

    #[test]
    fn adr030_a_small_dictionary_is_read_from_its_folder() {
        let checker = load(&data(), "xx-TEST").expect("the test dictionary reads");
        assert!(checker.check("ferry"));
        assert!(checker.check("ferries"));
        assert!(checker.check("Ferry"));
        assert!(!checker.check("ferri"));
        let mut out = Vec::new();
        checker.suggest("ferri", &mut out);
        assert!(out.iter().any(|s| s == "ferry"), "{out:?}");
    }

    #[test]
    fn adr030_a_latin1_dictionary_is_decoded_when_read() {
        let checker = load(&data(), "xx-LATIN").expect("the Latin-1 dictionary reads");
        assert!(checker.check("café"));
        assert!(checker.check("Straße"));
    }

    #[test]
    fn br023_a_missing_dictionary_says_so() {
        let error = load(&data(), "xx-NONE").expect_err("there is none");
        assert!(matches!(error, SpellError::Missing { .. }));
        assert_eq!(error.to_string(), "No dictionary for xx-NONE");
        let dictionaries = Dictionaries::new(Some(data()));
        assert!(dictionaries.ready("xx-NONE").is_none());
        assert!(dictionaries.get("xx-NONE").is_err());
        assert!(dictionaries.ready("xx-NONE").is_some_and(|r| r.is_err()));
        assert!(Dictionaries::none().get("en-US").is_err());
        for bad in ["../xx-TEST", "xx/..", "..", "a\\b", "", "a.b", "en US", "ä"] {
            assert!(dictionaries.get(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn br023_a_corrupt_dictionary_is_an_error_not_a_panic() {
        let error = load(&data(), "xx-BROKEN").expect_err("it is broken");
        assert!(
            matches!(
                error,
                SpellError::Invalid { .. } | SpellError::Encoding { .. }
            ),
            "{error:?}"
        );
        let bad_utf8 = decode(b"\xFF\xFE", "UTF-8", Path::new("x.dic"));
        assert!(matches!(bad_utf8, Err(SpellError::Invalid { .. })));
        let unknown = decode(b"abc", "KOI8-R", Path::new("x.aff"));
        assert!(matches!(unknown, Err(SpellError::Encoding { .. })));
    }

    #[test]
    fn adr030_the_character_set_comes_from_the_affix_file() {
        assert_eq!(encoding_of(b"# x\nSET UTF-8\nTRY abc\n"), "UTF-8");
        assert_eq!(encoding_of(b"\xEF\xBB\xBFSET ISO8859-15\n"), "ISO8859-15");
        assert_eq!(encoding_of(b"TRY abc\n"), "ISO8859-1");
        assert_eq!(
            decode(b"\xEF\xBB\xBFabc", "UTF-8", Path::new("x")).unwrap(),
            "abc"
        );
        assert_eq!(decode(&[0xA4], "ISO8859-15", Path::new("x")).unwrap(), "€");
        assert_eq!(decode(&[0xE9], "ISO8859-1", Path::new("x")).unwrap(), "é");
    }

    #[test]
    fn adr030_languages_not_used_and_failed_ones_are_let_go() {
        let dictionaries = Dictionaries::new(Some(data()));
        assert!(dictionaries.get("xx-TEST").is_ok());
        assert!(dictionaries.get("xx-NONE").is_err());
        dictionaries.retain(&["xx-TEST", "xx-NONE"]);
        assert!(dictionaries.ready("xx-TEST").is_some());
        assert!(
            dictionaries.ready("xx-NONE").is_none(),
            "a failed load is tried again"
        );
        dictionaries.retain(&[]);
        assert!(dictionaries.ready("xx-TEST").is_none());
        assert!(
            dictionaries.get("xx-TEST").is_ok(),
            "read again when needed"
        );
    }

    #[test]
    fn adr030_a_language_is_read_once_and_kept() {
        let dictionaries = Dictionaries::new(Some(data()));
        let first = dictionaries.get("xx-TEST").expect("reads");
        let again = dictionaries.ready("xx-TEST").expect("kept").expect("ok");
        assert!(Arc::ptr_eq(&first, &again));
    }
}
