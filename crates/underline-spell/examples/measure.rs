//! ADR-030: measures the bundled dictionaries: the time to read each, the
//! time to check a word, and the time of suggestions. Run optimised:
//!
//! ```sh
//! cargo run --release -p underline-spell --example measure
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use underline_markup::NoteFormat;
use underline_spell::{Dictionaries, NoteSpeller, UserWords};

fn main() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/underline-tauri/src-tauri/resources/dictionaries");
    let samples: [(&str, &[&str]); 4] = [
        (
            "en-US",
            &[
                "ferri",
                "recieve",
                "teh",
                "accomodate",
                "definately",
                "wierd",
            ],
        ),
        (
            "en-GB",
            &["ferri", "colur", "organiz", "centr", "neighbor", "wierd"],
        ),
        (
            "fr-FR",
            &[
                "maisonn",
                "elephant",
                "chateu",
                "bonjur",
                "aujourdui",
                "merçi",
            ],
        ),
        (
            "es-ES",
            &["cancion", "arbool", "manana", "nino", "graciass", "holaa"],
        ),
    ];
    for (code, misspelled) in samples {
        let start = Instant::now();
        let checker = match Dictionaries::new(Some(dir.clone())).get(code) {
            Ok(checker) => checker,
            Err(error) => {
                println!("{code}: {error}");
                continue;
            }
        };
        let load = start.elapsed();
        let words: Vec<String> = (0..20_000).map(|i| format!("word{}", i % 500)).collect();
        let start = Instant::now();
        for word in &words {
            let _ = checker.check(word);
        }
        let check = start.elapsed() / u32::try_from(words.len()).unwrap_or(1);
        let mut total = Duration::ZERO;
        let mut most = Duration::ZERO;
        for word in misspelled {
            let start = Instant::now();
            let _ = NoteSpeller::suggest(checker.as_ref(), word);
            let took = start.elapsed();
            total += took;
            most = most.max(took);
        }
        let mean = total / u32::try_from(misspelled.len()).unwrap_or(1);
        // A note of 10,000 words, checked whole the first time and again
        // after one word changed (the changed block only).
        let paragraph = "The quick brown fox jumps over the lazy dog and ferri leaves. ";
        let note: String = (0..200)
            .map(|i| format!("{}\n\n", paragraph.repeat(4) + &i.to_string()))
            .collect();
        let mut speller = NoteSpeller::new(checker, std::sync::Arc::new(UserWords::default()));
        let start = Instant::now();
        let marks = speller.check_note(&note, NoteFormat::Markdown, None, &mut |_| {});
        let first = start.elapsed();
        let changed = note.replacen("lazy", "lazzy", 1);
        let start = Instant::now();
        let _ = speller.check_note(&changed, NoteFormat::Markdown, None, &mut |_| {});
        let again = start.elapsed();
        println!(
            "{code}: load {load:?}, check {check:?}/word, suggest mean {mean:?} max {most:?}, \
             note of {} bytes: first {first:?} ({} marks), after an edit {again:?}",
            note.len(),
            marks.len()
        );
    }
}
