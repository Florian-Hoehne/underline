//! Test monkey, package 8b: random notes against the skip rules of
//! UC-005 BR-023 (the seed is printed on failure).

use underline_markup::NoteFormat;
use underline_spell::note_words;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

const PLAIN: &[&str] = &[
    "hello",
    "wrld",
    "Ärger",
    "naïve",
    "日本語",
    "שלום",
    "😀",
    "e\u{301}cole",
    "don't",
    "well-known",
    "ABC",
    "abc123",
    "x2y",
    "Under",
    "lne",
    "\r\n",
    "\n",
    "\n\n",
    " ",
    "  ",
    ".",
    ",",
];

const MD_PROTECTED: &[&str] = &[
    "`qzxcode`",
    "[text](http://qzxurl.example/a)",
    "<http://qzxauto.example>",
    "https://qzxraw.example/path",
    "www.qzxwww.example",
    "qzxmail@qzx.example",
    "mailto:qzxmail",
    "#qzxtag",
    "$qzxformula$",
    "\n\n$$\nqzxdisplay\n$$\n\n",
    "\n\n```rust\nqzxfence\n```\n\n",
    "\n\n```mermaid\nqzxdia\n```\n\n",
    "\n\n![qzxalt](qzximg.png)\n\n",
    "\n\n---\n\n",
];

const ADOC_PROTECTED: &[&str] = &[
    "`qzxcode`",
    "link:qzxurl[text]",
    "https://qzxraw.example[text]",
    "mailto:qzxmail@qzx.example[x]",
    "www.qzxwww.example",
    "latexmath:[qzxlatex]",
    "\n\n[source,rust]\n----\nqzxfence\n----\n\n",
    "\n\n....\nqzxlit\n....\n\n",
    "\n\n[plantuml]\n----\nqzxdia\n----\n\n",
    "\n\nimage::qzximg.png[qzxalt]\n\n",
    "\n\n[stem]\n++++\nqzxdisp\n++++\n\n",
];

fn gen(rng: &mut Rng, protected: &[&str], head: &str) -> String {
    let mut s = String::from(head);
    for _ in 0..(rng.next() % 40) {
        if rng.next().is_multiple_of(3) {
            s.push_str(rng.pick(protected));
        } else {
            s.push_str(rng.pick(PLAIN));
        }
        s.push(' ');
    }
    s
}

fn check(source: &str, format: NoteFormat, seed: u64) {
    for block in note_words(source, format) {
        for w in block.words {
            assert!(
                source.is_char_boundary(w.range.start) && source.is_char_boundary(w.range.end),
                "seed {seed}: bad boundary {:?} in {source:?}",
                w.range
            );
            let raw = &source[w.range.clone()];
            assert!(
                !w.text.contains("qzx") && !raw.contains("qzx"),
                "seed {seed} {format:?}: protected text checked: {w:?} raw {raw:?} in {source:?}"
            );
            assert!(
                !w.text.chars().any(char::is_numeric),
                "seed {seed}: digit word {w:?}"
            );
            let letters: Vec<char> = w.text.chars().filter(|c| c.is_alphabetic()).collect();
            assert!(
                !letters.iter().all(|c| c.is_uppercase()),
                "seed {seed}: all caps {w:?}"
            );
            assert!(w.range.start >= block.span.start && w.range.end <= block.span.end);
        }
    }
}

#[test]
fn br023_chaos_random_markdown_notes_keep_the_skip_rules() {
    for seed in 1..3000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let head = if seed.is_multiple_of(5) {
            "---\nqzxattr: value\ntags: [qzxtag2]\n---\n\n"
        } else {
            ""
        };
        let s = gen(&mut rng, MD_PROTECTED, head);
        check(&s, NoteFormat::Markdown, seed);
    }
}

#[test]
fn br023_chaos_random_asciidoc_notes_keep_the_skip_rules() {
    for seed in 1..3000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let head = if seed.is_multiple_of(5) {
            "= Title\n:qzxattr: value\n:tags: qzxtag2\n\n"
        } else {
            ""
        };
        let s = gen(&mut rng, ADOC_PROTECTED, head);
        check(&s, NoteFormat::AsciiDoc, seed);
    }
}

mod hostile {
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use underline_spell::Dictionaries;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("monkey8b-dict-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("xx")).unwrap();
        dir
    }

    /// Loads, checks and suggests in a thread; the time, or None on a hang.
    fn run(name: &str, aff: &[u8], dic: &[u8], word: &str) -> Option<(Duration, String)> {
        let dir = scratch(name);
        std::fs::write(dir.join("xx/xx.aff"), aff).unwrap();
        std::fs::write(dir.join("xx/xx.dic"), dic).unwrap();
        let word = word.to_owned();
        let (tx, rx) = mpsc::channel();
        let d2 = dir.clone();
        std::thread::spawn(move || {
            let start = Instant::now();
            let dictionaries = Dictionaries::new(Some(d2));
            let note = match dictionaries.get("xx") {
                Ok(c) => {
                    let known = c.check(&word);
                    let mut out = Vec::new();
                    c.suggest(&word, &mut out);
                    format!("loaded known={known} suggestions={}", out.len())
                }
                Err(e) => format!("error: {e}"),
            };
            let _ = tx.send((start.elapsed(), note));
        });
        let r = rx.recv_timeout(Duration::from_secs(20)).ok();
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    /// Test monkey 8b, ADR-030: truncated, recursive and huge dictionaries.
    #[test]
    fn adr030_hostile_dictionaries_fail_cleanly_and_never_hang() {
        let cases: Vec<(&str, Vec<u8>, Vec<u8>)> = vec![
            ("empty", b"".to_vec(), b"".to_vec()),
            ("empty-dic-count", b"SET UTF-8\n".to_vec(), b"3\nfoo\nbar\n".to_vec()),
            ("huge-count", b"SET UTF-8\n".to_vec(), b"99999999999999999999\nfoo\n".to_vec()),
            ("neg-count", b"SET UTF-8\n".to_vec(), b"-5\nfoo\n".to_vec()),
            ("huge-flag-num", b"SET UTF-8\nFLAG num\nSFX 65535 Y 1\nSFX 65535 0 s .\n".to_vec(), b"1\nfoo/65535\n".to_vec()),
            ("flag-overflow", b"SET UTF-8\nFLAG num\nSFX 99999999 Y 1\nSFX 99999999 0 s .\n".to_vec(), b"1\nfoo/99999999\n".to_vec()),
            ("trunc-aff", b"SET UTF-8\nSFX A Y 5\nSFX A 0 s".to_vec(), b"1\nfoo/A\n".to_vec()),
            ("sfx-count-mismatch", b"SET UTF-8\nSFX A Y 100000\nSFX A 0 s .\n".to_vec(), b"1\nfoo/A\n".to_vec()),
            ("recursive-affix", b"SET UTF-8\nSFX A Y 1\nSFX A 0 s/A .\nPFX B Y 1\nPFX B 0 un/B .\n".to_vec(), b"1\nfoo/AB\n".to_vec()),
            ("recursive-cross", b"SET UTF-8\nSFX A Y 1\nSFX A 0 s/B .\nSFX B Y 1\nSFX B 0 ed/A .\n".to_vec(), b"1\nfoo/A\n".to_vec()),
            ("wrong-set", b"SET KLINGON\n".to_vec(), b"1\nfoo\n".to_vec()),
            ("set-garbage", b"SET \xFF\xFE\x00\n".to_vec(), b"1\nfoo\n".to_vec()),
            ("nul-bytes", vec![0u8; 4096], vec![0u8; 4096]),
            ("compound-huge", b"SET UTF-8\nCOMPOUNDRULE 1\nCOMPOUNDRULE (a)*(b)*(c)*(a)*(b)*(c)*(a)*(b)*(c)*\nCOMPOUNDMIN 1\n".to_vec(), b"3\nx/a\ny/b\nz/c\n".to_vec()),
            ("compound-flag", b"SET UTF-8\nCOMPOUNDFLAG A\nCOMPOUNDMIN 1\nMAXCPDSUGS 99999\nMAXNGRAMSUGS 99999\n".to_vec(), b"3\nx/A\ny/A\nz/A\n".to_vec()),
            ("rep-many", {let mut v=b"SET UTF-8\nREP 20000\n".to_vec(); for i in 0..20000 { v.extend(format!("REP a{i} b\n").bytes()); } v}, b"1\nfoo\n".to_vec()),
            ("big-dic", b"SET UTF-8\n".to_vec(), {let mut v=b"300000\n".to_vec(); for i in 0..300000 { v.extend(format!("word{}x\n", i).bytes()); } v}),
            ("ngram-try", b"SET UTF-8\nTRY abcdefghijklmnopqrstuvwxyz\nMAP 100\n".to_vec(), b"2\nfoo\nbar\n".to_vec()),
            ("crlf", b"SET UTF-8\r\nTRY abc\r\n".to_vec(), b"1\r\nfoo\r\n".to_vec()),
            ("bom-latin", b"\xEF\xBB\xBFSET ISO8859-1\n".to_vec(), b"1\nfoo\n".to_vec()),
        ];
        let mut hung = Vec::new();
        for (name, aff, dic) in cases {
            for word in [
                "fo",
                &"x".repeat(10_000),
                "\u{1F600}\u{301}\u{200F}\u{202E}",
            ] {
                let r = run(name, &aff, &dic, word);
                match r {
                    Some((t, _)) if t < Duration::from_secs(10) => {}
                    other => hung.push(format!("{name} wordlen={}: {other:?}", word.len())),
                }
            }
        }
        assert!(hung.is_empty(), "slow or hanging dictionaries: {hung:?}");
    }
}

/// Test monkey 8b, UC-005 BR-023: an address in brackets or quotes is no
/// prose, as is one at the start of a piece of text (`www.` after `(`).
#[test]
fn br023_a_web_address_in_parentheses_is_not_checked() {
    for source in [
        "see (www.qzxa.example) now",
        "see \"www.qzxb.example\" now",
        "see [www.qzxc.example] now",
        "see (mailto:qzxd) now",
    ] {
        let words: Vec<String> = note_words(source, NoteFormat::Markdown)
            .into_iter()
            .flat_map(|block| block.words)
            .map(|word| word.text)
            .collect();
        assert!(
            words.iter().all(|word| !word.contains("qzx")),
            "{source:?} gives the words {words:?}"
        );
    }
}

/// Test monkey 8b, UC-005 A28: the corrections for a word are bounded in
/// time: a word longer than a checked word (100 characters) is never marked,
/// so none is looked for; `spellbook` takes over a second for 260 letters.
#[test]
fn uc005_a28_suggestions_for_a_long_word_are_bounded_in_time() {
    use std::time::{Duration, Instant};
    use underline_spell::{Dictionaries, NoteSpeller};
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/underline-tauri/src-tauri/resources/dictionaries");
    let checker = Dictionaries::new(Some(dir))
        .get("en-US")
        .expect("en-US reads");
    let word = "abcdefghijklmnopqrstuvwxyz".repeat(6);
    let start = Instant::now();
    let _ = NoteSpeller::suggest(checker.as_ref(), &word);
    assert!(
        start.elapsed() < Duration::from_millis(300),
        "{} characters took {:?}",
        word.len(),
        start.elapsed()
    );
}

/// Test monkey 8b, UC-005 BR-023: a lone carriage return (a note of another
/// system's line breaks) keeps every word in its block and at its place; the
/// block's marks are taken relative to its start, which panicked
/// (`attempt to subtract with overflow`, `marks.rs`) when a word started
/// before it.
#[test]
fn br023_a_lone_carriage_return_keeps_words_in_their_blocks() {
    use std::sync::Arc;
    use underline_spell::{NoteSpeller, UserWords, WordList};
    for source in [
        "wrnga\rwrngb wrngc",
        "a\r- wrnga wrngb\r",
        "\rwrnga",
        "x\r\rwrnga",
        "- a\r- wrnga",
    ] {
        for block in note_words(source, NoteFormat::Markdown) {
            for word in block.words {
                assert!(
                    block.span.start <= word.range.start && word.range.end <= block.span.end,
                    "{source:?}: {word:?} is outside its block {:?}",
                    block.span
                );
                assert_eq!(
                    source[word.range.clone()].replace("**", ""),
                    word.text,
                    "{source:?}: the range is not the word"
                );
            }
        }
        // Changed in package 8b's fixes: the checker knows "a" too, as
        // BR-023 marks every unknown word, one-letter words included.
        let mut speller = NoteSpeller::new(
            Arc::new(WordList::new(["x", "a"])),
            Arc::new(UserWords::default()),
        );
        let marks = speller.check_note(source, NoteFormat::Markdown, Some(0..2), &mut |_| {});
        for mark in marks {
            assert!(
                source[mark.clone()].starts_with("wrng"),
                "{source:?}: the mark {mark:?} is {:?}",
                &source[mark.clone()]
            );
        }
    }
}
