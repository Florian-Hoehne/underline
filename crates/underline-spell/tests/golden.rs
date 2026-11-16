//! ADR-030: golden checks of the dictionaries that come with Underline, so
//! a new version of `spellbook` or of a dictionary that reads them
//! differently fails here (FR-099, UC-005 BR-023, TC-005-39).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use underline_spell::{Dictionaries, NoteSpeller, SpellChecker};

fn bundled() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/underline-tauri/src-tauri/resources/dictionaries")
}

fn checker(code: &str) -> std::sync::Arc<dyn SpellChecker> {
    Dictionaries::new(Some(bundled()))
        .get(code)
        .unwrap_or_else(|error| panic!("the dictionary {code} reads: {error}"))
}

fn assert_known(code: &str, known: &[&str], unknown: &[&str]) {
    let checker = checker(code);
    for word in known {
        assert!(checker.check(word), "{code} knows {word}");
    }
    for word in unknown {
        assert!(!checker.check(word), "{code} does not know {word}");
    }
}

#[test]
fn tc005_39_english_us() {
    assert_known(
        "en-US",
        &[
            "The",
            "ferry",
            "leaves",
            "Underline",
            "color",
            "don't",
            "Don’t",
            "optimize",
        ],
        &["ferri", "colour", "optimise", "Kubernetes"],
    );
    let suggestions = NoteSpeller::suggest(checker("en-US").as_ref(), "ferri");
    assert!(suggestions.iter().any(|s| s == "ferry"), "{suggestions:?}");
    assert!(suggestions.len() <= 5);
}

#[test]
fn adr030_english_uk() {
    assert_known(
        "en-GB",
        &["colour", "organise", "ferry", "centre"],
        &["ferri", "color", "center"],
    );
}

#[test]
fn adr030_french() {
    assert_known(
        "fr-FR",
        &[
            "maison",
            "éléphant",
            "l'homme",
            "l’homme",
            "Château",
            "était",
        ],
        &["maisonn", "elephant"],
    );
}

#[test]
fn adr030_spanish() {
    assert_known(
        "es-ES",
        &["mañana", "canción", "árbol", "Niño"],
        &["cancion", "arbool"],
    );
}

/// UC-005 A28: suggestions come within a bound even unoptimised; the
/// window asks for them off the session, so a slow one only delays the
/// menu's list, never the text.
#[test]
fn uc005_a28_suggestions_come_in_time() {
    let checker = checker("en-US");
    let start = Instant::now();
    for word in ["ferri", "recieve", "teh", "accomodate", "definately"] {
        let _ = NoteSpeller::suggest(checker.as_ref(), word);
    }
    let limit = if cfg!(debug_assertions) { 20 } else { 2 };
    assert!(
        start.elapsed() < Duration::from_secs(limit),
        "five suggestions took {:?}",
        start.elapsed()
    );
}
