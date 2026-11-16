//! ADR-030: golden checks of the dictionaries that come with Underline, so
//! a new version of `spellbook` or of a dictionary that reads them
//! differently fails here (FR-099, UC-005 BR-023, TC-005-39).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use std::sync::Arc;

use underline_markup::NoteFormat;
use underline_spell::{Dictionaries, NoteSpeller, SpellChecker, UserWords};

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

/// OQ-299: German as written in Germany knows `ß` where it is written.
#[test]
fn oq299_german_germany() {
    assert_known(
        "de-DE",
        &[
            "Straße",
            "Haus",
            "Häuser",
            "Fahrrad",
            "Wörterbuch",
            "Grüße",
            "Donaudampfschifffahrt",
        ],
        &["Strase", "Hauss", "Wörtebuch"],
    );
    let suggestions = NoteSpeller::suggest(checker("de-DE").as_ref(), "Wörtebuch");
    assert!(
        suggestions.iter().any(|s| s == "Wörterbuch"),
        "{suggestions:?}"
    );
}

/// OQ-299, TC-005-40: German as written in Switzerland marks "Straße" and
/// knows "Strasse".
#[test]
fn tc005_40_german_switzerland() {
    assert_known(
        "de-CH",
        &["Strasse", "Grüsse", "Haus", "Fahrrad", "Wörterbuch"],
        &["Straße", "Grüße", "Hauss"],
    );
}

/// TC-005-40: the note `Mixed.md` checked in German (Switzerland): of the
/// shown words only "Straße" and the plain "fehlr" are marked; the code,
/// the address, the tag, "HTTPX", "abc123", the formula and the name
/// "statsu" are not.
#[test]
fn tc005_40_mixed_note_in_german_switzerland() {
    let text = "---\nstatsu: offen\n---\n# Gemischt\n\nStrasse und Straße und fehlr, \
                `fehlr`, [Link](https://exampel.org), #taggg, HTTPX, abc123, $fehlr$.\n\n\
                ```\nfehlr im Code\n```\n";
    let mut speller = NoteSpeller::new(checker("de-CH"), Arc::new(UserWords::default()));
    let marks = speller.check_note(text, NoteFormat::Markdown, None, &mut |_| {});
    let words: Vec<&str> = marks.iter().map(|m| &text[m.clone()]).collect();
    assert_eq!(words, ["Straße", "fehlr"]);
}

/// OQ-299: Italian, with its elisions.
#[test]
fn oq299_italian() {
    assert_known(
        "it-IT",
        &[
            "casa",
            "perché",
            "città",
            "ragazzo",
            "l'acqua",
            "dell'anno",
            "Grazie",
        ],
        &["casaa", "perche", "ragazo", "grazzie"],
    );
    let suggestions = NoteSpeller::suggest(checker("it-IT").as_ref(), "ragazo");
    assert!(
        suggestions.iter().any(|s| s == "ragazzo"),
        "{suggestions:?}"
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

/// OQ-299 (chaos): Italian written with the typographic apostrophe, as
/// keyboards and word processors produce it, is as right as with the
/// straight one: `l’acqua` and `dell’anno` are not marked (English accepts
/// `Don’t`).
#[test]
fn oq299_chaos_italian_typographic_apostrophe_is_not_marked() {
    let text = "Ecco l'acqua dell'anno e l’acqua dell’anno, un’amica.\n";
    let mut speller = NoteSpeller::new(checker("it-IT"), Arc::new(UserWords::default()));
    let marks = speller.check_note(text, NoteFormat::Markdown, None, &mut |_| {});
    let words: Vec<&str> = marks.iter().map(|m| &text[m.clone()]).collect();
    assert!(words.is_empty(), "marked: {words:?}");
}

/// OQ-207 (chaos): the value of `lang` is never marked, whatever it is, in
/// front matter and in AsciiDoc.
#[test]
fn oq207_chaos_the_lang_value_is_never_marked() {
    for value in ["xyzzyq", "de-CHH", "Schwiizertüütsch", "pt-BR", "off"] {
        let md = format!("---\nlang: {value}\n---\nStrasse\n");
        let adoc = format!("= Titel\n:lang: {value}\n\nStrasse\n");
        for (text, format) in [(md, NoteFormat::Markdown), (adoc, NoteFormat::AsciiDoc)] {
            let mut speller = NoteSpeller::new(checker("de-CH"), Arc::new(UserWords::default()));
            let marks = speller.check_note(&text, format, None, &mut |_| {});
            let words: Vec<&str> = marks.iter().map(|m| &text[m.clone()]).collect();
            assert!(
                words.iter().all(|w| !value.contains(w) || w.is_empty()),
                "{value}: {words:?}"
            );
        }
    }
}
