//! Tests of the spell check in the session (FR-099, FR-100, UC-005 BR-023,
//! A28, TC-005-39, TC-005-40, TC-010-21, ADR-030): the marks follow the
//! mirror's revisions after a rest, in web units; the user's words are
//! stored next to the user settings file; a missing dictionary is said;
//! the corrections are found without the session.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};
use underline_spell::{
    Dictionaries, SpellChecker, WordList, USER_WORDS_FILE_NAME, WORKSPACE_WORDS_FILE_NAME,
};

use super::{WordsOf, SPELL_AT_THE_LATEST, SPELL_DEBOUNCE};
use crate::settings::{Scope, SettingName};
use crate::{Event, Session, SettingsState, SpellMarksView};

/// A fresh, empty folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-spell-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The words of the test language `en-US`.
fn english() -> WordList {
    WordList::new([
        "the", "ferry", "leaves", "and", "a", "second", "word", "is", "here", "very",
    ])
}

/// A session on `dir/notes` holding `notes` (id, text), its user settings
/// in `dir/home` saying `settings`, with the dictionary `en-US` of
/// [`english`] only; nothing is open yet.
fn session_with(dir: &Path, notes: &[(&str, &str)], settings: &str) -> Session {
    session_with_checker(dir, notes, settings, Arc::new(english()))
}

fn session_with_checker(
    dir: &Path,
    notes: &[(&str, &str)],
    settings: &str,
    checker: Arc<dyn SpellChecker>,
) -> Session {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    for (note, text) in notes {
        fs::write(root.join(format!("{note}.md")), text).unwrap();
    }
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(SETTINGS_FILE_NAME), settings).unwrap();
    let mut session = Session::new(
        Some(root),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(home.join(SETTINGS_FILE_NAME)), Instant::now()),
    );
    session.set_dictionaries(Dictionaries::none().with("en-US", checker));
    session.start_workspace();
    session.take_events();
    session
}

fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// The marks of `note` in the events.
fn marks_of(events: &[Event], note: &str) -> Option<SpellMarksView> {
    events.iter().rev().find_map(|event| match event {
        Event::SpellMarks(view) if view.note == note => Some(view.clone()),
        _ => None,
    })
}

/// Ticks until the spell check of every open note is done, at most five
/// seconds; the last marks of each note that got some.
fn settle(session: &mut Session) -> HashMap<String, SpellMarksView> {
    let start = Instant::now();
    let mut last = HashMap::new();
    let mut quiet = 0;
    while start.elapsed() < Duration::from_secs(5) && quiet < 3 {
        let events = session.tick(Instant::now());
        let mut any = false;
        for event in events {
            if let Event::SpellMarks(view) = event {
                any = true;
                last.insert(view.note.clone(), view);
            }
        }
        quiet = if any || session.spell_busy() {
            0
        } else {
            quiet + 1
        };
        std::thread::sleep(Duration::from_millis(10));
    }
    last
}

/// Ticks until the spell check is done; the last marks of `note`.
fn tick_for_marks(session: &mut Session, note: &str) -> Option<SpellMarksView> {
    settle(session).remove(note)
}

/// The marked words of `text` (ASCII only), as the window shows them.
fn marked<'a>(text: &'a str, view: &SpellMarksView) -> Vec<&'a str> {
    view.marks
        .iter()
        .map(|&(from, to)| &text[from as usize..to as usize])
        .collect()
}

impl Session {
    /// A check of a tab runs, or one is due.
    fn spell_busy(&self) -> bool {
        let open = self.open.as_ref().expect("open");
        open.tabs.iter().any(|(_, tab)| {
            tab.spell.running || tab.spell.told.map(|(rev, _)| rev) != Some(tab.mirror.rev())
        })
    }

    fn spell_running(&self, note: &str) -> bool {
        let open = self.open.as_ref().expect("open");
        let index = open.tabs.find(&id(note)).expect("tab");
        open.tabs.get(index).spell.running
    }
}

#[test]
fn tc005_39_a_word_is_marked_corrected_and_added_to_the_dictionary() {
    let dir = temp_dir("tc005-39");
    let spell = "The ferri leaves\n";
    let other = "Kubernetes and Kubernetes\n";
    let mut session = session_with(
        &dir,
        &[("Spell", spell), ("Other", other)],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Other")));
    assert!(session.open_note(&id("Spell")));
    // Step 1: "ferri" is marked.
    let mut all = settle(&mut session);
    let marks = all.remove("Spell").expect("marks of Spell");
    assert_eq!(marked(spell, &marks), ["ferri"]);
    assert_eq!(marks.rev, session.mirror_rev(&id("Spell")));
    let marks = all.remove("Other").expect("marks of Other");
    assert_eq!(marked(other, &marks), ["Kubernetes", "Kubernetes"]);
    // Step 2: corrections, off the session.
    let suggestions = session
        .spell_suggest_job("ferri", Some(&id("Spell")))
        .run()
        .expect("found");
    assert!(suggestions.contains(&"ferry".to_owned()), "{suggestions:?}");
    assert!(suggestions.len() <= 5);
    // Step 4: added to the dictionary, both lose their mark.
    session.spell_add_word("Kubernetes").expect("added");
    let marks = tick_for_marks(&mut session, "Other").expect("marks again");
    assert!(marks.marks.is_empty(), "{marks:?}");
    let stored = fs::read_to_string(dir.join("home").join(USER_WORDS_FILE_NAME)).unwrap();
    assert_eq!(stored, "Kubernetes\n");
    assert!(session
        .status()
        .contains("Added \"Kubernetes\" to your dictionary"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tc005_39_a_word_added_counts_in_another_workspace_and_language() {
    let dir = temp_dir("words-everywhere");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(USER_WORDS_FILE_NAME), "Kubernetes\n").unwrap();
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    let marks = tick_for_marks(&mut session, "Other").expect("marks");
    assert!(marks.marks.is_empty(), "{marks:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adr030_marks_wait_for_a_rest_and_carry_the_revision() {
    let dir = temp_dir("rest");
    let text = "The ferry leaves\n";
    let mut session = session_with(&dir, &[("Spell", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Spell")));
    let marks = tick_for_marks(&mut session, "Spell").expect("first marks");
    assert!(marks.marks.is_empty());
    // An edit is not checked at once ...
    session.window_type(&id("Spell"), 0, "Teh ");
    let typed = Instant::now();
    let events = session.tick(typed);
    assert!(marks_of(&events, "Spell").is_none());
    assert!(!session.spell_running("Spell"));
    let events = session.tick(typed + SPELL_DEBOUNCE / 2);
    assert!(marks_of(&events, "Spell").is_none());
    assert!(!session.spell_running("Spell"));
    // ... but once the text rested.
    session.tick(typed + SPELL_DEBOUNCE);
    assert!(session.spell_running("Spell"));
    let marks = tick_for_marks(&mut session, "Spell").expect("marks after the rest");
    assert_eq!(marked("Teh The ferry leaves\n", &marks), ["Teh"]);
    assert_eq!(marks.rev, session.mirror_rev(&id("Spell")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn br023_typing_without_a_rest_is_checked_at_the_latest_in_time() {
    let dir = temp_dir("latest");
    let mut session = session_with(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Spell")));
    tick_for_marks(&mut session, "Spell").expect("first marks");
    let start = Instant::now();
    let mut at = start;
    let mut started = None;
    // A key every 100 ms: the text never rests 250 ms.
    for step in 0..12u32 {
        session.window_type(&id("Spell"), 0, "x");
        at = start + Duration::from_millis(100) * step;
        session.tick(at);
        if session.spell_running("Spell") && started.is_none() {
            started = Some(at);
        }
    }
    let started = started.expect("checked while typing");
    assert!(
        started.saturating_duration_since(start)
            <= SPELL_AT_THE_LATEST + Duration::from_millis(100)
    );
    let _ = at;
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adr030_an_outcome_of_an_older_text_is_dropped() {
    struct Slow(WordList, Arc<AtomicBool>);
    impl SpellChecker for Slow {
        fn check(&self, word: &str) -> bool {
            if self.1.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(50));
            }
            self.0.check(word)
        }
        fn suggest(&self, word: &str, out: &mut Vec<String>) {
            self.0.suggest(word, out);
        }
    }
    let dir = temp_dir("older");
    let slow = Arc::new(AtomicBool::new(false));
    let mut session = session_with_checker(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: en-US\n",
        Arc::new(Slow(english(), Arc::clone(&slow))),
    );
    assert!(session.open_note(&id("Spell")));
    tick_for_marks(&mut session, "Spell").expect("first marks");
    slow.store(true, Ordering::SeqCst);
    session.window_type(&id("Spell"), 0, "wun twu thre ");
    let typed = Instant::now();
    session.tick(typed + SPELL_DEBOUNCE);
    assert!(session.spell_running("Spell"));
    // The text changes while the check runs: its marks are not sent.
    session.window_type(&id("Spell"), 0, "four ");
    slow.store(false, Ordering::SeqCst);
    let start = Instant::now();
    let mut stale = false;
    while session.spell_running("Spell") && start.elapsed() < Duration::from_secs(5) {
        let events = session.tick(Instant::now());
        stale |= marks_of(&events, "Spell").is_some_and(|m| m.marks.len() == 3);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!stale, "marks of the older text were sent");
    let marks = tick_for_marks(&mut session, "Spell").expect("marks of the newer text");
    assert_eq!(marks.rev, session.mirror_rev(&id("Spell")));
    assert_eq!(
        marked("four wun twu thre here\n", &marks),
        ["four", "wun", "twu", "thre"]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn br023_marks_are_web_units_with_crlf_and_multibyte_text() {
    let dir = temp_dir("web-units");
    let text = "Ärger fehlr\r\n\r\nthe ferry 😀 fehlr\r\n";
    let mut session = session_with(&dir, &[("Spell", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Spell")));
    let marks = tick_for_marks(&mut session, "Spell").expect("marks");
    // "Ärger " is 6 units; the line breaks count once; the emoji twice.
    assert_eq!(marks.marks, [(0, 5), (6, 11), (26, 31)]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tc005_40_switching_the_spell_check_off_removes_every_mark() {
    let dir = temp_dir("off");
    let text = "The ferri leaves\n";
    let mut session = session_with(&dir, &[("Spell", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Spell")));
    let marks = tick_for_marks(&mut session, "Spell").expect("marks");
    assert_eq!(marked(text, &marks), ["ferri"]);
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("off".into()),
    );
    let marks = tick_for_marks(&mut session, "Spell").expect("marks when off");
    assert!(marks.marks.is_empty());
    let written = fs::read_to_string(dir.join("home").join(SETTINGS_FILE_NAME)).unwrap();
    assert!(written.contains("spell_check_language: off"), "{written}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn br023_a_language_without_a_dictionary_says_so_and_marks_nothing() {
    let dir = temp_dir("missing");
    let text = "The ferri leaves\n";
    let mut session = session_with(&dir, &[("Spell", text)], "spell_check_language: en-GB\n");
    assert!(session.open_note(&id("Spell")));
    let marks = tick_for_marks(&mut session, "Spell").expect("marks");
    assert!(marks.marks.is_empty());
    assert_eq!(
        session.status(),
        "No dictionary for English (UK). Choose another language in the settings."
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn tc010_21_the_workspace_language_wins_over_the_users() {
    let dir = temp_dir("workspace-language");
    let text = "The ferri leaves\n";
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(SETTINGS_FILE_NAME), "spell_check_language: off\n").unwrap();
    let mut session = session_with(&dir, &[("Spell", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Spell")));
    let marks = tick_for_marks(&mut session, "Spell").expect("marks");
    assert!(marks.marks.is_empty(), "the workspace switched it off");
    // "As for me" in "This workspace": the user's English applies.
    session.change_setting(Scope::Workspace, SettingName::SpellCheckLanguage, None);
    let marks = tick_for_marks(&mut session, "Spell").expect("marks in English");
    assert_eq!(marked(text, &marks), ["ferri"]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn adr030_the_users_words_file_changed_outside_is_followed() {
    let dir = temp_dir("outside");
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    let marks = tick_for_marks(&mut session, "Other").expect("marks");
    assert_eq!(marked(text, &marks), ["Kubernetes"]);
    // Another program writes the file; its time must differ to be seen.
    std::thread::sleep(Duration::from_millis(20));
    fs::write(dir.join("home").join(USER_WORDS_FILE_NAME), "Kubernetes\n").unwrap();
    let start = Instant::now();
    let mut cleared = false;
    while start.elapsed() < Duration::from_secs(5) && !cleared {
        let events = session.tick(Instant::now());
        cleared = marks_of(&events, "Other").is_some_and(|m| m.marks.is_empty());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(cleared, "the word added outside is no longer marked");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn uc005_a28_a_word_with_a_space_is_not_added() {
    let dir = temp_dir("space");
    let mut session = session_with(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: en-US\n",
    );
    assert!(session.spell_add_word("two words").is_err());
    assert!(!dir.join("home").join(USER_WORDS_FILE_NAME).exists());
    let _ = fs::remove_dir_all(&dir);
}

/// UC-005 A28, ADR-030: the corrections are worked out off the session, so
/// a slow dictionary only delays the menu's list, never the session (the
/// tick, the edits); the menu shows "Add to dictionary" at once.
#[test]
fn uc005_a28_slow_suggestions_do_not_hold_the_session() {
    struct SlowSuggest(WordList);
    impl SpellChecker for SlowSuggest {
        fn check(&self, word: &str) -> bool {
            self.0.check(word)
        }
        fn suggest(&self, word: &str, out: &mut Vec<String>) {
            std::thread::sleep(Duration::from_millis(400));
            self.0.suggest(word, out);
        }
    }
    let dir = temp_dir("slow-suggest");
    let mut session = session_with_checker(
        &dir,
        &[("Spell", "The ferri leaves\n")],
        "spell_check_language: en-US\n",
        Arc::new(SlowSuggest(english())),
    );
    assert!(session.open_note(&id("Spell")));
    let job = session.spell_suggest_job("ferri", None);
    let asked = Instant::now();
    let running = std::thread::spawn(move || job.run());
    // The session goes on meanwhile.
    session.window_type(&id("Spell"), 0, "a ");
    session.tick(Instant::now());
    assert!(asked.elapsed() < Duration::from_millis(200));
    let suggestions = running.join().expect("the job ends").expect("found");
    assert!(suggestions.contains(&"ferry".to_owned()));
    let _ = fs::remove_dir_all(&dir);
}

/// Test monkey 8b: a check of a tab that was closed must not hand its marks
/// to the tab opened again at once (a mirror starts at revision 1 each time).
#[test]
fn adr030_marks_of_a_closed_tab_are_not_shown_in_the_reopened_one() {
    struct Slow(WordList, Arc<AtomicBool>);
    impl SpellChecker for Slow {
        fn check(&self, word: &str) -> bool {
            if self.1.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
            }
            self.0.check(word)
        }
        fn suggest(&self, word: &str, out: &mut Vec<String>) {
            self.0.suggest(word, out);
        }
    }
    let dir = temp_dir("reopen");
    let slow = Arc::new(AtomicBool::new(true));
    let old = "wun twu thre fower\n";
    let mut session = session_with_checker(
        &dir,
        &[("Spell", old)],
        "spell_check_language: en-US\n",
        Arc::new(Slow(english(), Arc::clone(&slow))),
    );
    assert!(session.open_note(&id("Spell")));
    session.tick(Instant::now());
    assert!(session.spell_running("Spell"));
    assert!(session.close_tab(&id("Spell"), true).is_none());
    let new = "The ferry leaves here\n";
    fs::write(dir.join("notes").join("Spell.md"), new).unwrap();
    assert!(session.open_note(&id("Spell")));
    slow.store(false, Ordering::SeqCst);
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < Duration::from_secs(5) {
        for event in session.tick(Instant::now()) {
            if let Event::SpellMarks(view) = event {
                if view.note == "Spell" {
                    seen.push(view);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let rev = session.mirror_rev(&id("Spell"));
    for view in &seen {
        assert!(
            view.rev != rev || view.marks.is_empty(),
            "marks {:?} of the old text were sent for the new text (rev {})",
            view.marks,
            view.rev
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// Test monkey 8b: adding a word must not lose the words of a words file
/// that is not valid UTF-8 (written by another editor in Latin-1).
#[test]
fn uc005_a28_adding_a_word_keeps_the_words_of_a_file_that_is_no_utf8() {
    let dir = temp_dir("latin1-words");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    let original: &[u8] = b"alpha\nbet\xE0\ngamma\n";
    fs::write(home.join(USER_WORDS_FILE_NAME), original).unwrap();
    let mut session = session_with(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: en-US\n",
    );
    let _ = session.spell_add_word("delta");
    let after = fs::read(home.join(USER_WORDS_FILE_NAME)).unwrap();
    let after = String::from_utf8_lossy(&after);
    assert!(
        after.contains("alpha") && after.contains("gamma"),
        "the other words are lost: {after:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// The text as the window counts it: CRLF is one unit, in UTF-16.
fn web_units(text: &str) -> Vec<u16> {
    text.replace("\r\n", "\n").encode_utf16().collect()
}

/// Knows every word but those starting with "wrng".
struct Wrng;
impl SpellChecker for Wrng {
    fn check(&self, word: &str) -> bool {
        !word.to_lowercase().starts_with("wrng")
    }
    fn suggest(&self, _: &str, out: &mut Vec<String>) {
        out.clear();
    }
}

/// Test monkey 8b, UC-005 BR-023: random notes with CRLF, emoji, combining
/// marks, RTL and CJK: every mark is a valid UTF-16 range of the window's
/// text and holds exactly a word that is not known.
#[test]
fn br023_chaos_marks_are_valid_web_units_in_random_notes() {
    let pieces = [
        "the",
        "wrnga",
        "wrngb",
        "Wrngc",
        "\r\n",
        "\n",
        "\r\n\r\n",
        "😀",
        "e\u{301}",
        "שלום",
        "日本語",
        " ",
        "  ",
        "👨\u{200D}👩\u{200D}👧",
        "\u{feff}",
        "**",
        "_",
        "`code`",
        "- ",
        "# ",
        "> ",
        "| ",
        "\t",
        "wrngd\u{301}",
        "wrnge😀wrngf",
        "\u{202E}",
        "wrng**x**y",
        "<b>",
        "[wrngl](http://a.example)",
        "wrngz-wrngy",
    ];
    let dir = temp_dir("units");
    let mut texts = Vec::new();
    for seed in 1..60u64 {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let mut text = String::new();
        for _ in 0..(1 + next() % 30) {
            text.push_str(pieces[(next() % pieces.len() as u64) as usize]);
            if next() % 2 == 0 {
                text.push(' ');
            }
        }
        texts.push((format!("N{seed}"), text));
    }
    let notes: Vec<(&str, &str)> = texts
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    let mut session = session_with_checker(
        &dir,
        &notes,
        "spell_check_language: en-US\n",
        Arc::new(Wrng),
    );
    for (note, _) in &notes {
        assert!(session.open_note(&id(note)));
    }
    let all = settle(&mut session);
    let mut failures = Vec::new();
    for (note, text) in &texts {
        let Some(marks) = all.get(note) else {
            failures.push(format!("{note}: no marks for {text:?}"));
            continue;
        };
        let units = web_units(text);
        for &(from, to) in &marks.marks {
            let word = units
                .get(from as usize..to as usize)
                .and_then(|u| String::from_utf16(u).ok());
            match word {
                Some(w)
                    if w.to_lowercase().starts_with("wrng")
                        && !w.chars().any(char::is_whitespace) => {}
                other => failures.push(format!(
                    "{note}: mark {from}..{to} is {other:?} in {text:?}"
                )),
            }
        }
    }
    let _ = fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Test monkey 8b, UC-005 BR-023: a note with a lone carriage return must not
/// end the check thread (a panic there leaves every note unmarked and its
/// check "running" for the rest of the session); another note is still
/// marked.
#[test]
fn br023_a_note_with_a_lone_carriage_return_does_not_end_the_spell_check() {
    let dir = temp_dir("lone-cr");
    let mut session = session_with(
        &dir,
        &[
            ("Mac", "The\r- ferri leaves\r"),
            ("Fine", "The ferri leaves\n"),
        ],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Mac")));
    assert!(session.open_note(&id("Fine")));
    let all = settle(&mut session);
    let fine = all.get("Fine").expect("a second note is still marked");
    assert_eq!(marked("The ferri leaves\n", fine), ["ferri"]);
    let mac = all
        .get("Mac")
        .expect("the note with the carriage returns is marked");
    let text = "The\r- ferri leaves\r";
    assert_eq!(marked(text, mac), ["ferri"]);
    let _ = fs::remove_dir_all(&dir);
}

/// ADR-030: a fault while checking one note is caught; the note stays
/// unmarked and the status bar says so, the other notes are still marked.
#[test]
fn adr030_a_fault_while_checking_is_caught_and_said() {
    struct Faulty(WordList);
    impl SpellChecker for Faulty {
        fn check(&self, word: &str) -> bool {
            assert!(word != "boom", "a fault of the checker");
            self.0.check(word)
        }
        fn suggest(&self, word: &str, out: &mut Vec<String>) {
            self.0.suggest(word, out);
        }
    }
    let dir = temp_dir("fault");
    let mut session = session_with_checker(
        &dir,
        &[("Bad", "the boom\n"), ("Good", "The ferri leaves\n")],
        "spell_check_language: en-US\n",
        Arc::new(Faulty(english())),
    );
    assert!(session.open_note(&id("Bad")));
    assert!(session.open_note(&id("Good")));
    let all = settle(&mut session);
    assert_eq!(marked("The ferri leaves\n", &all["Good"]), ["ferri"]);
    assert!(
        session
            .status()
            .starts_with("Bad could not be spell checked"),
        "{}",
        session.status()
    );
    assert!(!session.spell_running("Bad"));
    let _ = fs::remove_dir_all(&dir);
}

/// UC-005 BR-023, FR-100: a language without a dictionary stays in the
/// status bar until another language is chosen.
#[test]
fn br023_a_missing_dictionary_stays_in_the_status_bar() {
    let dir = temp_dir("state");
    let mut session = session_with(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: fr-FR\n",
    );
    assert!(session.open_note(&id("Spell")));
    let start = Instant::now();
    let mut state = None;
    while start.elapsed() < Duration::from_secs(5) && state.is_none() {
        for event in session.tick(Instant::now()) {
            if let Event::SpellState(view) = event {
                if view.problem.is_some() {
                    state = Some(view);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let state = state.expect("the status bar is told");
    assert_eq!(
        state.problem.as_deref(),
        Some("Spell check: French (France), no dictionary")
    );
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("en-US".into()),
    );
    let cleared = session
        .tick(Instant::now())
        .into_iter()
        .any(|event| matches!(event, Event::SpellState(view) if view.problem.is_none()));
    assert!(cleared);
    let _ = fs::remove_dir_all(&dir);
}

/// Re-review 8b: an outcome of a language left meanwhile sets no status:
/// neither "No dictionary for …" nor a failed check is said for the
/// language chosen since.
#[test]
fn br023_an_outcome_of_a_language_left_says_nothing() {
    let dir = temp_dir("left-language");
    let mut session = session_with(
        &dir,
        &[("Spell", "the ferri\n")],
        "spell_check_language: en-GB\n",
    );
    assert!(session.open_note(&id("Spell")));
    session.tick(Instant::now());
    assert!(
        session.spell_running("Spell"),
        "the check of en-GB was given"
    );
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("en-US".into()),
    );
    let all = settle(&mut session);
    assert_eq!(marked("the ferri\n", &all["Spell"]), ["ferri"]);
    assert!(
        !session.status().contains("English (UK)"),
        "{}",
        session.status()
    );
    assert!(session.spell.state.problem.is_none());

    struct Faulty;
    impl SpellChecker for Faulty {
        fn check(&self, _: &str) -> bool {
            panic!("a fault of the checker")
        }
        fn suggest(&self, _: &str, out: &mut Vec<String>) {
            out.clear();
        }
    }
    let other = temp_dir("left-failed");
    let mut session = session_with_checker(
        &other,
        &[("Spell", "the ferri\n")],
        "spell_check_language: en-US\n",
        Arc::new(Faulty),
    );
    assert!(session.open_note(&id("Spell")));
    session.tick(Instant::now());
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("off".into()),
    );
    settle(&mut session);
    assert!(
        !session.status().contains("could not be spell checked"),
        "{}",
        session.status()
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&other);
}

/// Re-review 8b, UC-005 A28: a word that could not be stored (no words
/// file, a folder that takes none) counts until Underline closes; the
/// look at the missing file does not take it away.
#[cfg(unix)]
#[test]
fn uc005_a28_a_word_not_stored_stays_for_the_session() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("not-stored");
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    assert_eq!(marked(text, &settle(&mut session)["Other"]), ["Kubernetes"]);
    let home = dir.join("home");
    fs::set_permissions(&home, fs::Permissions::from_mode(0o555)).unwrap();
    let stored = session.spell_add_word("Kubernetes");
    fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
    if fs::metadata(&home).is_ok() && home.join(USER_WORDS_FILE_NAME).exists() {
        // Running with rights that ignore the folder's: nothing to test.
        let _ = fs::remove_dir_all(&dir);
        return;
    }
    assert!(stored.is_err());
    assert!(
        session.status().contains("could not be stored"),
        "{}",
        session.status()
    );
    // Ticks over several looks at the file.
    let start = Instant::now();
    let mut last = None;
    for step in 0..30u32 {
        let events = session.tick(start + Duration::from_millis(100) * step);
        if let Some(marks) = marks_of(&events, "Other") {
            last = Some(marks);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(session.spell.user.contains("Kubernetes"));
    if let Some(marks) = last {
        assert!(marks.marks.is_empty(), "{marks:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}

/// Re-review 8b, UC-005 A28: adding a word writes the file as it is now
/// with the word: words removed by hand stay removed, lines that are no
/// word stay.
#[test]
fn uc005_a28_adding_a_word_keeps_the_file_as_the_user_left_it() {
    let dir = temp_dir("hand-edited");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    let file = home.join(USER_WORDS_FILE_NAME);
    fs::write(&file, "alpha\ntwo words\n").unwrap();
    let mut session = session_with(
        &dir,
        &[("Spell", "here\n")],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Spell")));
    settle(&mut session);
    assert!(session.spell.user.contains("alpha"));
    // The user removes "alpha" by hand; the session has not looked yet.
    fs::write(&file, "two words\n").unwrap();
    session.spell_add_word("beta").expect("stored");
    assert_eq!(fs::read_to_string(&file).unwrap(), "beta\ntwo words\n");
    assert!(!session.spell.user.contains("alpha"));
    let _ = fs::remove_dir_all(&dir);
}

/// Re-review 8b, ADR-030: a large note waits for its longer rest, also
/// after a check whose outcome was dropped because the text changed.
#[test]
fn adr030_a_large_note_keeps_its_pace_after_a_dropped_check() {
    let dir = temp_dir("large");
    let text = "the ferry leaves here\n\n".repeat(super::LARGE_NOTE / 20);
    let mut session = session_with(&dir, &[("Big", &text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Big")));
    let start = Instant::now();
    session.tick(start);
    assert!(session.spell_running("Big"));
    // The text changes while the first check runs: its outcome is dropped.
    session.window_type(&id("Big"), 0, "a ");
    let typed = Instant::now();
    while session.spell_running("Big") && typed.elapsed() < Duration::from_secs(20) {
        session.tick(typed);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!session.spell_running("Big"));
    session.tick(typed + SPELL_DEBOUNCE + Duration::from_millis(50));
    assert!(!session.spell_running("Big"), "the large note rests longer");
    session.tick(typed + super::LARGE_DEBOUNCE);
    assert!(session.spell_running("Big"));
    let _ = fs::remove_dir_all(&dir);
}

/// Re-review 8b, ADR-030: a thread that ended is started again, at most
/// three times without a check done in between; then the spell check says
/// it stopped and marks nothing. A thread that cannot be given work counts
/// the same. New dictionaries start it again.
#[test]
fn adr030_the_check_thread_is_started_again_then_stops() {
    let dir = temp_dir("restarts");
    let mut session = session_with(
        &dir,
        &[("Spell", "the ferri\n")],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Spell")));
    settle(&mut session);
    for round in 1..=super::RESTARTS + 1 {
        // The thread's end, as the session sees it: its channel is gone.
        let (_, dead) = std::sync::mpsc::channel();
        session.spell.done = Some(dead);
        session.tick(Instant::now());
        assert_eq!(session.spell.restarts, round);
    }
    assert!(session.spell.stopped());
    assert_eq!(
        session.spell.state.problem.as_deref(),
        Some("Spell check stopped")
    );
    // Another language does not hide it while it holds.
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("en-GB".into()),
    );
    session.tick(Instant::now());
    assert_eq!(
        session.spell.state.problem.as_deref(),
        Some("Spell check stopped")
    );
    assert!(!session.spell_running("Spell"));

    // New dictionaries start it again; a thread that takes no work counts.
    session.set_dictionaries(Dictionaries::none().with("en-US", Arc::new(english())));
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("en-US".into()),
    );
    assert_eq!(session.spell.restarts, 0);
    let (gone, receiver) = std::sync::mpsc::channel();
    drop(receiver);
    session.spell.work = Some(gone);
    session.tick(Instant::now());
    assert_eq!(session.spell.restarts, 1, "the failed start counts");
    let all = settle(&mut session);
    assert_eq!(marked("the ferri\n", &all["Spell"]), ["ferri"]);
    assert_eq!(session.spell.restarts, 0, "a check done resets the count");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208, UC-005 A28, TC-005-39 step 5: "Add to workspace dictionary"
/// stores the word in `underline-words.txt` in the workspace, not in the
/// user's file; it loses its mark in every note.
#[test]
fn oq208_a_word_added_to_the_workspace_dictionary_is_stored_in_the_workspace() {
    let dir = temp_dir("workspace-words");
    let other = "Kubernetes and Kubernetes\n";
    let mut session = session_with(&dir, &[("Other", other)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    let marks = tick_for_marks(&mut session, "Other").expect("marks");
    assert_eq!(marked(other, &marks), ["Kubernetes", "Kubernetes"]);
    session
        .spell_add_workspace_word("Kubernetes")
        .expect("added");
    let marks = tick_for_marks(&mut session, "Other").expect("marks again");
    assert!(marks.marks.is_empty(), "{marks:?}");
    let file = dir.join("notes").join(WORKSPACE_WORDS_FILE_NAME);
    assert_eq!(fs::read_to_string(file).unwrap(), "Kubernetes\n");
    assert!(!dir.join("home").join(USER_WORDS_FILE_NAME).exists());
    assert_eq!(
        session.status(),
        "Added \"Kubernetes\" to the workspace dictionary"
    );
    assert!(session.spell_add_workspace_word("two words").is_err());
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208: the words in effect are the dictionary's, the user's and the
/// workspace's; a workspace dictionary written outside (a `git pull`, an
/// editor) is followed, and is no note of the overview.
#[test]
fn oq208_the_workspace_dictionary_is_followed_and_counts_with_the_users() {
    let dir = temp_dir("workspace-words-outside");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join(USER_WORDS_FILE_NAME), "Kubernetes\n").unwrap();
    let text = "Kubernetes and Zettelkasten here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    let marks = tick_for_marks(&mut session, "Other").expect("marks");
    assert_eq!(marked(text, &marks), ["Zettelkasten"]);
    std::thread::sleep(Duration::from_millis(20));
    let file = dir.join("notes").join(WORKSPACE_WORDS_FILE_NAME);
    fs::write(&file, "Zettelkasten\n").unwrap();
    let start = Instant::now();
    let mut cleared = false;
    while start.elapsed() < Duration::from_secs(5) && !cleared {
        let events = session.tick(Instant::now());
        cleared = marks_of(&events, "Other").is_some_and(|m| m.marks.is_empty());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(cleared, "the workspace's word is no longer marked");
    let open = session.open.as_ref().expect("open");
    assert!(
        open.overview.iter().all(|entry| entry.id == id("Other")),
        "the words file is no note: {:?}",
        open.overview
    );
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208, UC-005 A28: adding a word reads the workspace dictionary first:
/// hand edits stay (lines that are no word too), and a file that cannot be
/// read is never written over; the word then counts until the workspace
/// is closed.
#[test]
fn oq208_the_workspace_dictionary_keeps_hand_edits_and_is_never_written_over() {
    let dir = temp_dir("workspace-words-hand");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let file = root.join(WORKSPACE_WORDS_FILE_NAME);
    fs::write(&file, "zeta\ntwo words\n").unwrap();
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    assert!(session.spell.user.contains("zeta"));
    session.spell_add_workspace_word("alpha").expect("stored");
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "alpha\nzeta\ntwo words\n"
    );

    let original: &[u8] = b"alpha\nbet\xE0\n";
    fs::write(&file, original).unwrap();
    let refused = session
        .spell_add_workspace_word("Kubernetes")
        .expect_err("not written over");
    assert!(
        refused.message.contains("until the workspace is closed"),
        "{}",
        refused.message
    );
    assert_eq!(fs::read(&file).unwrap(), original);
    assert!(session.spell.user.contains("Kubernetes"));
    let marks = tick_for_marks(&mut session, "Other").expect("marks");
    assert!(marks.marks.is_empty(), "{marks:?}");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208: the words of a workspace count only in it: in another workspace
/// they are marked again, its unstored ones too.
#[test]
fn oq208_the_workspace_dictionary_counts_only_in_its_workspace() {
    let dir = temp_dir("workspace-words-switch");
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    session
        .spell_add_workspace_word("Kubernetes")
        .expect("added");
    session.add_word_for_now(WordsOf::Workspace, "unstored");
    assert!(session.spell.user.contains("Kubernetes"));
    let second = dir.join("second");
    fs::create_dir_all(&second).unwrap();
    fs::write(second.join("Second.md"), text).unwrap();
    session.open_chosen(&second, true);
    assert!(session.open_note(&id("Second")));
    let marks = tick_for_marks(&mut session, "Second").expect("marks");
    assert_eq!(marked(text, &marks), ["Kubernetes"]);
    assert!(!session.spell.user.contains("unstored"));
    let _ = fs::remove_dir_all(&dir);
}

/// A session of [`session_with`] that also has `de-CH`, knowing `words`.
fn with_swiss(session: &mut Session, words: &[&str]) {
    session.set_dictionaries(
        Dictionaries::none()
            .with("en-US", Arc::new(english()))
            .with("de-CH", Arc::new(WordList::new(words.iter().copied()))),
    );
}

/// The last spell state the window was told, after ticking until the
/// spell check is done.
fn told_state(session: &mut Session) -> Option<crate::SpellStateView> {
    let start = Instant::now();
    let mut state = None;
    let mut quiet = 0;
    while start.elapsed() < Duration::from_secs(5) && quiet < 3 {
        let mut any = false;
        for event in session.tick(Instant::now()) {
            match event {
                Event::SpellState(view) => state = Some(view),
                Event::SpellMarks(_) => any = true,
                _ => {}
            }
        }
        quiet = if any || session.spell_busy() {
            0
        } else {
            quiet + 1
        };
        std::thread::sleep(Duration::from_millis(10));
    }
    state
}

/// OQ-207, UC-005 BR-023: a note's attribute `lang` overrides the language
/// of the settings for that note, in front matter and in AsciiDoc; other
/// notes keep the settings' language.
#[test]
fn oq207_a_notes_lang_overrides_the_spell_check_language() {
    let dir = temp_dir("note-lang");
    let swiss = "---\nlang: de_CH\n---\nStrasse und ferry\n";
    let english_note = "Strasse and the ferry\n";
    fs::create_dir_all(dir.join("notes")).unwrap();
    fs::write(
        dir.join("notes").join("Doc.adoc"),
        "= Doc\n:lang: de-ch\n\nStrasse und ferry\n",
    )
    .unwrap();
    let mut session = session_with(
        &dir,
        &[("Swiss", swiss), ("English", english_note)],
        "spell_check_language: en-US\n",
    );
    with_swiss(&mut session, &["strasse", "und", "Doc"]);
    assert!(session.open_note(&id("Swiss")));
    assert!(session.open_note(&id("English")));
    assert!(session.open_note(&id("Doc")));
    let all = settle(&mut session);
    assert_eq!(marked(swiss, &all["Swiss"]), ["ferry"]);
    assert_eq!(marked(english_note, &all["English"]), ["Strasse"]);
    let doc = "= Doc\n:lang: de-ch\n\nStrasse und ferry\n";
    assert_eq!(marked(doc, &all["Doc"]), ["ferry"]);
    // The corrections come from the note's language.
    let suggestions = session
        .spell_suggest_job("Strase", Some(&id("Swiss")))
        .run()
        .expect("found");
    assert_eq!(suggestions, ["strasse"]);
    let english_suggestions = session
        .spell_suggest_job("Strase", Some(&id("English")))
        .run()
        .expect("found");
    assert!(english_suggestions.is_empty(), "{english_suggestions:?}");
    // The spell check off: the note's language does not switch it on.
    session.change_setting(
        Scope::Me,
        SettingName::SpellCheckLanguage,
        Some("off".into()),
    );
    let marks = tick_for_marks(&mut session, "Swiss").expect("marks when off");
    assert!(marks.marks.is_empty());
    assert!(session
        .spell_suggest_job("Strase", Some(&id("Swiss")))
        .run()
        .expect("none")
        .is_empty());
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-207: the marks are recomputed when the attribute changes; a language
/// not offered marks nothing and says "No dictionary for …" while the note
/// is the active one, and nothing once the attribute names one again.
#[test]
fn oq207_a_changed_lang_is_followed_and_an_unknown_one_is_said() {
    let dir = temp_dir("note-lang-change");
    let text = "---\nlang: de-CH\n---\nStrasse und ferry\n";
    let mut session = session_with(&dir, &[("Swiss", text)], "spell_check_language: en-US\n");
    with_swiss(&mut session, &["Strasse", "und"]);
    assert!(session.open_note(&id("Swiss")));
    assert_eq!(marked(text, &settle(&mut session)["Swiss"]), ["ferry"]);
    // "lang: de-CH" becomes "lang: pt-BR".
    let at = text.find("de-CH").unwrap();
    session.window_edit(&id("Swiss"), at, 5, "pt-BR");
    let state = told_state(&mut session).expect("the status bar is told");
    assert_eq!(
        state.problem.as_deref(),
        Some("Swiss: no dictionary for pt-BR")
    );
    assert_eq!(
        state.detail.as_deref(),
        Some("No dictionary for pt-BR, the language this note names in its attribute `lang`. Change or remove the attribute.")
    );
    assert!(
        !session.status().contains("pt-BR"),
        "the chip says it, not the status bar: {}",
        session.status()
    );
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&id("Swiss")).unwrap());
    assert!(tab.spell.marks.is_empty());
    // Removed again: English, and nothing to say.
    let changed = text.replace("de-CH", "pt-BR");
    let at = changed.find("lang: pt-BR\n").unwrap();
    session.window_edit(&id("Swiss"), at, "lang: pt-BR\n".len(), "");
    let state = told_state(&mut session).expect("told again");
    assert!(state.problem.is_none(), "{state:?}");
    let now = session.window_text(&id("Swiss"));
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&id("Swiss")).unwrap());
    let view = SpellMarksView {
        note: "Swiss".to_owned(),
        rev: 0,
        marks: tab.spell.marks.clone(),
    };
    assert_eq!(marked(&now, &view), ["Strasse", "und"]);
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): a workspace dictionary that is a symbolic link to a file
/// outside the workspace is never written through: the outside file stays
/// as it is.
#[cfg(unix)]
#[test]
fn oq208_chaos_a_linked_workspace_dictionary_never_writes_through() {
    let dir = temp_dir("chaos-ws-symlink");
    let outside = dir.join("outside.txt");
    fs::write(&outside, "secret\n").unwrap();
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(WORKSPACE_WORDS_FILE_NAME)).unwrap();
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    let _ = session.spell_add_workspace_word("Kubernetes");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "secret\n");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): a directory named `underline-words.txt` is never
/// replaced; the word counts for now.
#[test]
fn oq208_chaos_a_directory_named_like_the_workspace_dictionary_stays() {
    let dir = temp_dir("chaos-ws-dir");
    let root = dir.join("notes");
    fs::create_dir_all(root.join(WORKSPACE_WORDS_FILE_NAME)).unwrap();
    fs::write(root.join(WORKSPACE_WORDS_FILE_NAME).join("keep"), "x").unwrap();
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    assert!(session.spell_add_workspace_word("Kubernetes").is_err());
    assert!(root.join(WORKSPACE_WORDS_FILE_NAME).is_dir());
    assert!(session.spell.user.contains("Kubernetes"));
    let marks = tick_for_marks(&mut session, "Other");
    assert!(marks.is_none_or(|m| m.marks.is_empty()));
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): the user's words never land in the workspace file, and
/// the workspace's never in the user's.
#[test]
fn oq208_chaos_the_two_files_never_mix() {
    let dir = temp_dir("chaos-mix");
    let mut session = session_with(&dir, &[("Other", "a b\n")], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    session.spell_add_word("Mine").unwrap();
    session.spell_add_workspace_word("Ours").unwrap();
    session.spell_add_word("Mine2").unwrap();
    // The other file changes outside; the sessions rebuilds.
    let ws = fs::read_to_string(dir.join("notes").join(WORKSPACE_WORDS_FILE_NAME)).unwrap();
    let me = fs::read_to_string(dir.join("home").join(USER_WORDS_FILE_NAME)).unwrap();
    assert_eq!(ws, "Ours\n");
    assert_eq!(me, "Mine\nMine2\n");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): the workspace file deleted while open: its words count
/// no more.
#[test]
fn oq208_chaos_a_deleted_workspace_dictionary_stops_counting() {
    let dir = temp_dir("chaos-ws-deleted");
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    session.spell_add_workspace_word("Kubernetes").unwrap();
    let marks = tick_for_marks(&mut session, "Other").expect("cleared");
    assert!(marks.marks.is_empty());
    fs::remove_file(dir.join("notes").join(WORKSPACE_WORDS_FILE_NAME)).unwrap();
    let start = Instant::now();
    let mut marked_again = false;
    while start.elapsed() < Duration::from_secs(5) && !marked_again {
        let events = session.tick(Instant::now());
        marked_again = marks_of(&events, "Other").is_some_and(|m| !m.marks.is_empty());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(marked_again);
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): a read-only workspace folder: the word counts for now,
/// nothing is written, the error is told.
#[cfg(unix)]
#[test]
fn oq208_chaos_a_read_only_workspace_keeps_the_word_for_now() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("chaos-ws-readonly");
    let text = "Kubernetes is here\n";
    let mut session = session_with(&dir, &[("Other", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    let root = dir.join("notes");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();
    let writable = fs::write(root.join("probe"), "x").is_ok();
    let result = session.spell_add_workspace_word("Kubernetes");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    if !writable {
        assert!(result.is_err());
        assert!(session.spell.user.contains("Kubernetes"));
        assert!(!root.join(WORKSPACE_WORDS_FILE_NAME).exists());
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".underline-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): a hand-written workspace file with CRLF and a byte order
/// mark keeps its words; an added word does not duplicate or lose them.
#[test]
fn oq208_chaos_crlf_and_bom_workspace_file_is_kept() {
    let dir = temp_dir("chaos-ws-crlf");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let file = root.join(WORKSPACE_WORDS_FILE_NAME);
    fs::write(&file, "\u{feff}zeta\r\nbeta\r\n").unwrap();
    let mut session = session_with(&dir, &[("Other", "a\n")], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    session.spell_add_workspace_word("alpha").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "alpha\nbeta\nzeta\n");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (chaos): two sessions adding words to the same workspace
/// dictionary at once lose none of them.
#[test]
fn oq208_chaos_two_sessions_adding_at_once_lose_no_word() {
    let dir = temp_dir("chaos-two-sessions");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let file = root.join(WORKSPACE_WORDS_FILE_NAME);
    let n = 40;
    let handles: Vec<_> = (0..2)
        .map(|who| {
            let file = file.clone();
            std::thread::spawn(move || {
                for i in 0..n {
                    let mut words = underline_spell::UserWords::load(&file).unwrap_or_default();
                    words.add(&format!("w{who}x{i}")).unwrap();
                    let _ = words.store(&file);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let words = underline_spell::UserWords::load(&file).unwrap();
    assert_eq!(words.len(), 2 * n, "words were lost");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-207 (chaos): a `lang` value of 60 KB is not copied into the status
/// bar as it is.
#[test]
fn oq207_chaos_a_huge_unknown_lang_value_is_not_put_in_the_status_bar() {
    let dir = temp_dir("chaos-lang-huge");
    let tag = "x".repeat(60_000);
    let text = format!("---\nlang: {tag}\n---\nbody\n");
    let mut session = session_with(&dir, &[("Big", &text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("Big")));
    let state = told_state(&mut session).expect("told");
    let detail = state.detail.unwrap_or_default();
    let problem = state.problem.unwrap_or_default();
    assert!(detail.len() < 1000, "detail is {} bytes", detail.len());
    assert!(problem.len() < 200, "problem is {} bytes", problem.len());
    assert!(
        session.status().len() < 1000,
        "status is {}",
        session.status().len()
    );
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-207 (chaos): `lang: null` or an empty list names no language: the
/// settings' language applies, nothing says "No dictionary for null".
#[test]
fn oq207_chaos_a_null_lang_leaves_the_settings() {
    let dir = temp_dir("chaos-lang-null");
    let text = "---\nlang: null\n---\nthe ferri\n";
    let mut session = session_with(&dir, &[("N", text)], "spell_check_language: en-US\n");
    assert!(session.open_note(&id("N")));
    let all = settle(&mut session);
    assert_eq!(marked(text, &all["N"]), ["ferri"]);
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-207 (chaos): a note whose `lang` changes while it is checked never
/// shows marks of the old language, edit after edit (seeded).
#[test]
fn oq207_chaos_marks_never_stay_in_the_old_language() {
    let seed: u64 = 20_261_116;
    println!("seed {seed}");
    let dir = temp_dir("chaos-lang-flip");
    let text = "---\nlang: de-CH\n---\nStrasse und ferry\n";
    let mut session = session_with(&dir, &[("Swiss", text)], "spell_check_language: en-US\n");
    with_swiss(&mut session, &["Strasse", "und"]);
    assert!(session.open_note(&id("Swiss")));
    settle(&mut session);
    let mut state = seed;
    let mut current = "de-CH";
    for _ in 0..30 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let next = if (state >> 33).is_multiple_of(2) {
            "de-CH"
        } else {
            "en-US"
        };
        let now = session.window_text(&id("Swiss"));
        let at = now.find(current).unwrap();
        session.window_edit(&id("Swiss"), at, current.len(), next);
        current = next;
        // Read the marks after the settle.
        let _ = settle(&mut session);
        let now = session.window_text(&id("Swiss"));
        let open = session.open.as_ref().unwrap();
        let tab = open.tabs.get(open.tabs.find(&id("Swiss")).unwrap());
        let view = SpellMarksView {
            note: "Swiss".to_owned(),
            rev: 0,
            marks: tab.spell.marks.clone(),
        };
        let got = marked(&now, &view);
        let want: &[&str] = if next == "de-CH" {
            &["ferry"]
        } else {
            &["Strasse", "und"]
        };
        // en-US knows "ferry" but not "Strasse"/"und"; "lang" values are
        // never marked.
        let want: Vec<&str> = if next == "de-CH" {
            want.to_vec()
        } else {
            vec!["Strasse", "und"]
        };
        assert_eq!(got, want, "after switching to {next} (seed {seed})");
    }
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (review): "Add to workspace dictionary" on a workspace dictionary
/// that is a symbolic link refuses and says why; the link stays a link.
#[cfg(unix)]
#[test]
fn oq208_a_linked_workspace_dictionary_refuses_and_says_so() {
    let dir = temp_dir("ws-symlink-refused");
    let outside = dir.join("outside.txt");
    fs::write(&outside, "secret\n").unwrap();
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    let link = root.join(WORKSPACE_WORDS_FILE_NAME);
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let mut session = session_with(
        &dir,
        &[("Other", "Kubernetes is here\n")],
        "spell_check_language: en-US\n",
    );
    assert!(session.open_note(&id("Other")));
    settle(&mut session);
    assert!(
        !session.spell.user.contains("secret"),
        "the link is not read"
    );
    let refused = session
        .spell_add_workspace_word("Kubernetes")
        .expect_err("refused");
    assert!(
        refused.message.contains("symbolic link"),
        "{}",
        refused.message
    );
    assert!(
        session.status().contains("symbolic link"),
        "{}",
        session.status()
    );
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "secret\n");
    let _ = fs::remove_dir_all(&dir);
}

/// OQ-208 (review): the user's file and the workspace's are told apart
/// as files, not as spellings of their paths, so one file reached two ways
/// is followed once (as the user's). The user settings folder itself is
/// never a workspace (UC-001), but another way to it can be.
#[cfg(unix)]
#[test]
fn oq208_one_file_reached_two_ways_is_the_same_file() {
    let dir = temp_dir("same-file");
    let home = dir.join("home");
    fs::create_dir_all(&home).unwrap();
    let alias = dir.join("alias");
    std::os::unix::fs::symlink(&home, &alias).unwrap();
    let user = home.join(USER_WORDS_FILE_NAME);
    let other_way = alias.join(USER_WORDS_FILE_NAME);
    assert!(
        super::same_file(Some(&user), Some(&other_way)),
        "missing yet"
    );
    fs::write(&user, "x\n").unwrap();
    assert!(super::same_file(Some(&user), Some(&other_way)));
    assert!(super::same_file(Some(&user), Some(&user)));
    assert!(!super::same_file(
        Some(&user),
        Some(&dir.join(USER_WORDS_FILE_NAME))
    ));
    assert!(!super::same_file(None, Some(&user)));
    let _ = fs::remove_dir_all(&dir);
}

/// ADR-030 (update 2026-11-16), OQ-207: a dictionary no open note and no
/// setting uses any more is let go after a while; one in use stays.
#[test]
fn adr030_a_dictionary_no_note_uses_is_let_go() {
    let dir = temp_dir("sweep");
    let text = "---\nlang: de-CH\n---\nStrasse und ferry\n";
    let mut session = session_with(&dir, &[("Swiss", text)], "spell_check_language: en-US\n");
    with_swiss(&mut session, &["Strasse", "und"]);
    assert!(session.open_note(&id("Swiss")));
    settle(&mut session);
    let later = Instant::now() + super::DICTIONARY_SWEEP * 2;
    session.tick(later);
    assert!(
        session.spell.dictionaries.ready("de-CH").is_some(),
        "in use"
    );
    assert!(
        session.spell.dictionaries.ready("en-US").is_some(),
        "the setting's"
    );
    let at = session
        .window_text(&id("Swiss"))
        .find("lang: de-CH\n")
        .unwrap();
    session.window_edit(&id("Swiss"), at, "lang: de-CH\n".len(), "");
    settle(&mut session);
    session.tick(later + super::DICTIONARY_SWEEP * 2);
    assert!(
        session.spell.dictionaries.ready("de-CH").is_none(),
        "let go"
    );
    assert!(session.spell.dictionaries.ready("en-US").is_some());
    let _ = fs::remove_dir_all(&dir);
}
