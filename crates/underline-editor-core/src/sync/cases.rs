//! Tests of the sync (ADR-025, `docs/migration/variante-b.md` sections 3
//! and 6): the transform rules, convergence of random concurrent edits, the
//! reload barrier, and the undo and cursor rules of backend changes.

use ropey::Rope;

use super::*;
use crate::SourceEditor;

fn change(at: usize, remove: usize, insert: &str) -> Change {
    Change {
        at,
        remove,
        insert: insert.to_owned(),
    }
}

fn set(text: &str, at: usize, remove: usize, insert: &str) -> ChangeSet {
    ChangeSet::of(text.chars().count(), &change(at, remove, insert)).expect("fits")
}

fn applied(text: &str, sets: &[&ChangeSet]) -> String {
    let mut rope = Rope::from_str(text);
    for set in sets {
        set.apply(&mut rope).expect("fits");
    }
    rope.to_string()
}

/// Both orders of `backend` and `window`, made on `text`, give `expected`.
fn converges(text: &str, backend: &ChangeSet, window: &ChangeSet, expected: &str) {
    let (backend_moved, window_moved) = ChangeSet::transform(backend, window);
    assert_eq!(
        applied(text, &[backend, &window_moved]),
        expected,
        "backend first"
    );
    assert_eq!(
        applied(text, &[window, &backend_moved]),
        expected,
        "window first"
    );
}

/// ADR-025 section 3.5: the rules of the table, each in both orders.
#[test]
fn adr025_transform_follows_the_table() {
    let text = "0123456789AB";
    // Both insert at the same place: the backend's text comes first.
    converges(
        text,
        &set(text, 5, 0, "b"),
        &set(text, 5, 0, "w"),
        "01234bw56789AB",
    );
    // The window inserts inside what the backend replaced: behind its text.
    converges(
        text,
        &set(text, 3, 7, "B"),
        &set(text, 5, 3, "A"),
        "012BAAB",
    );
    // The window's insert at the end of what the backend removed.
    converges(
        text,
        &set(text, 5, 3, "B"),
        &set(text, 8, 0, "A"),
        "01234BA89AB",
    );
    // The backend's insert at the end of what the window removed.
    converges(
        text,
        &set(text, 8, 0, "B"),
        &set(text, 5, 3, "A"),
        "01234AB89AB",
    );
    // What the backend replaced lies inside what the window replaced: both
    // inserts stay.
    converges(text, &set(text, 4, 2, "B"), &set(text, 2, 8, "A"), "01ABAB");
    // Both remove the same chars: they go once.
    converges(text, &set(text, 2, 4, ""), &set(text, 3, 2, ""), "016789AB");
    // Both replace from the same place.
    converges(
        text,
        &set(text, 5, 3, "B"),
        &set(text, 5, 5, "A"),
        "01234BAAB",
    );
}

/// ADR-025: edits in chars never split an emoji, and line breaks are kept
/// as they are (`\r\n` is two chars here).
#[test]
fn adr025_changes_count_chars_of_the_rope() {
    let text = "a😀\r\nb";
    let set = ChangeSet::of_changes(5, &[change(1, 1, "x"), change(2, 1, "")]).unwrap();
    assert_eq!(applied(text, &[&set]), "ax\nb");
    assert_eq!(set.changes(), [change(1, 2, "x")]);
    assert!(ChangeSet::of(5, &change(4, 2, "")).is_err());
}

/// A small xorshift generator: the runs are the same each time.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }

    fn text(&mut self) -> String {
        const PIECES: [&str; 8] = ["a", "bc", "😀", "\r\n", "\n", " ", "ß", ""];
        (0..self.below(3))
            .map(|_| PIECES[self.below(PIECES.len())])
            .collect()
    }

    /// A random edit of a text of `len` chars.
    fn change(&mut self, len: usize) -> Change {
        let at = self.below(len + 1);
        let remove = self.below(len - at + 1).min(4);
        change(at, remove, &self.text())
    }
}

/// The window: its editor, its side of the sync, the text its undo
/// history starts from, and what happened.
struct Window {
    editor: SourceEditor,
    client: Client,
    origin: String,
    stats: Stats,
}

/// What a run went through, so the runs are known to cover the cases.
#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    moved: usize,
    reloads: usize,
    reverted: usize,
    resyncs: usize,
    failed: usize,
    undone: usize,
}

impl Stats {
    fn add(&mut self, other: Self) {
        self.moved += other.moved;
        self.reloads += other.reloads;
        self.reverted += other.reverted;
        self.resyncs += other.resyncs;
        self.failed += other.failed;
        self.undone += other.undone;
    }
}

impl Window {
    fn new(text: &str, rev: u32) -> Self {
        let mut editor = SourceEditor::new(text);
        editor.record_edits();
        Self {
            editor,
            client: Client::new(text, rev),
            origin: text.to_owned(),
            stats: Stats::default(),
        }
    }

    /// The edits the editor made go to the client.
    fn record(&mut self) {
        for edit in self.editor.take_edits() {
            self.client.local(&Change::from(&edit)).expect("fits");
        }
    }

    fn take(&mut self, incoming: Vec<Incoming>) {
        for one in incoming {
            match one {
                Incoming::Edits { set, sealed } => {
                    self.stats.moved += 1;
                    apply_remote(&mut self.editor, &set, sealed).expect("fits");
                }
                Incoming::Reload { text } => {
                    self.stats.reloads += 1;
                    self.editor.reload(&text);
                    self.origin = text;
                }
            }
        }
        // The backend has these.
        self.editor.take_edits();
    }

    /// Takes `answer` in; after a desync the window's text wins.
    fn answer(&mut self, server: &mut Server, answer: &Answer) {
        match self.client.acknowledge(answer) {
            Ok(incoming) => self.take(incoming),
            Err(desync) => {
                assert_eq!(desync, Desync::Asked, "only a refused batch");
                self.stats.resyncs += 1;
                let text = self.editor.text();
                let resynced = server.resync(&text);
                self.client.reset(&text, resynced.answer.rev);
            }
        }
    }
}

/// Random replacements of a text of `len` chars, in order and apart.
fn replacements(random: &mut Random, len: usize) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out = Vec::new();
    let mut at = 0;
    for _ in 0..=random.below(3) {
        if at > len {
            break;
        }
        let start = at + random.below(len - at + 1);
        let end = (start + random.below(3)).min(len);
        out.push((start..end, random.text()));
        at = end + 1;
    }
    out
}

/// One random run: the window types, undoes and redoes; the backend edits
/// (one or more places, its own editor's edits too), reloads, and keeps
/// only `KEPT` revisions; batches get lost and answers come late; at every
/// step the window's text is the backend's as the window knows it with its
/// own edits on top, and at the end both texts are the same, and so are
/// their hashes.
fn random_run(seed: u64) -> Stats {
    let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let start = "Hello 😀\r\nworld\n";
    let mut server = Server::new(start);
    let mut window = Window::new(start, server.rev());
    let mut sent: Option<Batch> = None;
    let mut answer: Option<Answer> = None;
    // Every third run the window types only now and then, so it takes
    // reloads as well as taking them back.
    let quiet = seed.is_multiple_of(3);
    for _ in 0..60 {
        match random.below(14) {
            0..=3 if !quiet || random.below(8) == 0 => {
                let change = random.change(window.editor.document().len_chars());
                window
                    .editor
                    .replace(change.at..change.at + change.remove, &change.insert);
                window.record();
            }
            4 => {
                // Typed and taken back: the edits come to nothing.
                let at = random.below(window.editor.document().len_chars() + 1);
                window.editor.replace(at..at, "x");
                window.editor.replace(at..at + 1, "");
                window.record();
            }
            5 => {
                window.stats.undone += 1;
                if random.below(2) == 0 {
                    window.editor.undo();
                } else {
                    window.editor.redo();
                }
                window.record();
            }
            6 | 7 => {
                let edits = replacements(&mut random, server.rope().len_chars());
                server.backend_edit(&edits, random.below(2) == 0).unwrap();
                window.client.ring();
            }
            8 => {
                let change = random.change(server.rope().len_chars());
                server.local_edit(&[change]).unwrap();
                window.client.ring();
            }
            9 => {
                if random.below(3) == 0 {
                    server.reload(&format!("outside {seed}\n"));
                    window.client.ring();
                } else if seed.is_multiple_of(97) {
                    // More changes than the backend keeps.
                    for _ in 0..=KEPT {
                        server.backend_edit(&[(0..0, "k".into())], false).unwrap();
                    }
                }
            }
            10 => {
                if sent.is_none() && answer.is_none() {
                    sent = window.client.take_outbox();
                }
            }
            11 => {
                if let Some(batch) = sent.take() {
                    if random.below(6) == 0 {
                        // Lost on the way.
                        window.stats.failed += 1;
                        window.client.failed();
                    } else {
                        let received = server.receive(&batch);
                        window.stats.reverted += usize::from(received.reverted.is_some());
                        answer = Some(received.answer);
                    }
                }
            }
            _ => {
                if let Some(answer) = answer.take() {
                    window.answer(&mut server, &answer);
                }
            }
        }
        assert_eq!(
            window.editor.text(),
            window.client.projected(),
            "seed {seed}: the window's text is the backend's with its edits"
        );
    }
    // Everything travels to its end.
    for _ in 0..20 {
        if let Some(batch) = sent.take() {
            answer = Some(server.receive(&batch).answer);
        }
        if let Some(answer) = answer.take() {
            window.answer(&mut server, &answer);
        }
        window.client.ring();
        sent = window.client.take_outbox();
        if sent.is_none() {
            break;
        }
    }
    let text = window.editor.text();
    assert_eq!(text, server.text(), "seed {seed}");
    assert_eq!(hash(&text), server.hash(), "seed {seed}");
    assert_eq!(hash_rope(server.rope()), hash(&server.text()));
    // The undo history stays one line with the text: undoing everything
    // gives the text it started from.
    while window.editor.undo().edited() {}
    assert_eq!(window.editor.text(), window.origin, "seed {seed}: undo");
    window.stats
}

/// ADR-025: random concurrent edits of the window and the backend, with
/// reloads, lost batches, late answers and forgotten revisions, end in the
/// same text on both sides.
#[test]
fn adr025_random_concurrent_edits_converge() {
    let mut all = Stats::default();
    for seed in 1..=2000 {
        all.add(random_run(seed));
    }
    assert!(
        all.moved > 1000
            && all.reloads > 50
            && all.reverted > 50
            && all.resyncs > 5
            && all.failed > 100
            && all.undone > 100,
        "the runs cover the cases: {all:?}"
    );
}

/// ADR-025, UC-004 A7: a window without edits of its own takes a reload,
/// and confirms it with its next batch; agents wait meanwhile.
#[test]
fn uc004_a7_a_reload_is_taken_and_confirmed() {
    let mut server = Server::new("mine\n");
    let mut client = Client::new("mine\n", server.rev());
    server.reload("outside\n");
    assert!(server.reload_unconfirmed());
    client.ring();

    let batch = client.take_outbox().unwrap();
    let answer = server.receive(&batch).answer;
    let incoming = client.acknowledge(&answer).unwrap();

    assert_eq!(
        incoming,
        [Incoming::Reload {
            text: "outside\n".into()
        }]
    );
    assert!(server.reload_unconfirmed(), "not yet confirmed");
    let confirm = client.take_outbox().expect("confirms");
    let received = server.receive(&confirm);
    assert!(received.reverted.is_none());
    assert!(!server.reload_unconfirmed());
    assert_eq!(server.text(), "outside\n");
}

/// ADR-025, UC-004 A9: edits the window made before a reload take it back:
/// the window's text stays, the text from outside is handed back to keep as
/// a copy, and backend edits made before the reload stay.
#[test]
fn uc004_a9_edits_before_a_reload_take_it_back() {
    let mut server = Server::new("mine\n");
    let mut client = Client::new("mine\n", server.rev());
    server.backend_edit(&[(0..0, "> ".into())], true).unwrap();
    client.local(&change(4, 0, "!")).unwrap();
    server.reload("outside\n");

    let batch = client.take_outbox().unwrap();
    let received = server.receive(&batch);

    assert_eq!(received.reverted.as_deref(), Some("outside\n"));
    assert_eq!(server.text(), "> mine!\n");
    assert!(!server.reload_unconfirmed());
    let incoming = client.acknowledge(&received.answer).unwrap();
    assert!(matches!(
        &incoming[..],
        [Incoming::Edits { sealed: true, .. }]
    ));
}

/// ADR-025, UC-004 A9: a reload that arrives while the window has edits
/// not sent yet is skipped; the window's next batch takes it back.
#[test]
fn uc004_a9_a_reload_meeting_unsent_edits_is_skipped() {
    let mut server = Server::new("mine\n");
    let mut client = Client::new("mine\n", server.rev());
    server.reload("outside\n");
    client.ring();
    let pull = client.take_outbox().unwrap();
    let answer = server.receive(&pull).answer;
    // Typed while the pull travelled.
    client.local(&change(0, 0, "x")).unwrap();

    assert!(client.acknowledge(&answer).unwrap().is_empty());
    let batch = client.take_outbox().unwrap();
    let received = server.receive(&batch);

    assert_eq!(received.reverted.as_deref(), Some("outside\n"));
    assert_eq!(server.text(), "xmine\n");
    assert!(client.acknowledge(&received.answer).unwrap().is_empty());
}

/// ADR-025 section 3.10: a batch that does not fit asks for the whole text,
/// which wins.
#[test]
fn adr025_a_batch_that_does_not_fit_asks_for_the_text() {
    let mut server = Server::new("abc");
    let wrong = Batch {
        base: server.rev(),
        changes: vec![change(2, 5, "")],
    };
    assert!(server.receive(&wrong).answer.resync);
    let future = Batch {
        base: server.rev() + 1,
        changes: Vec::new(),
    };
    assert!(server.receive(&future).answer.resync);
    let mut client = Client::new("abc", 1);
    assert_eq!(
        client.acknowledge(&server.receive(&wrong).answer),
        Err(Desync::Asked)
    );

    let resynced = server.resync("abXc");
    assert_eq!(server.text(), "abXc");
    assert_eq!(resynced.touched.len(), 1);
    assert_eq!(resynced.outside, None, "no reload waited");
    client.reset("abXc", resynced.answer.rev);
    assert!(client.in_step(resynced.answer.rev));
}

/// ADR-025: edits on a revision the backend no longer keeps ask for the
/// whole text.
#[test]
fn adr025_a_batch_on_a_forgotten_revision_asks_for_the_text() {
    let mut server = Server::new("a");
    for _ in 0..=KEPT {
        server.backend_edit(&[(0..0, "x".into())], false).unwrap();
    }
    let late = Batch {
        base: 1,
        changes: vec![change(0, 0, "y")],
    };
    assert!(server.receive(&late).answer.resync);
}

/// ADR-025: a wrong hash is a desync.
#[test]
fn adr025_another_hash_is_a_desync() {
    let mut client = Client::new("abc", 1);
    client.ring();
    let _ = client.take_outbox();
    let answer = Answer {
        rev: 1,
        hash: hash("abd"),
        remote: Vec::new(),
        resync: false,
    };
    assert_eq!(client.acknowledge(&answer), Err(Desync::Hash));
}

fn typed(editor: &mut SourceEditor, text: &str) {
    for c in text.chars() {
        editor.type_text(&c.to_string());
    }
}

/// UC-019 BR-007: an agent's change is an undo step of its own: undo takes
/// it back first, then the user's word.
#[test]
fn uc019_br007_an_agents_change_is_its_own_undo_step() {
    let mut editor = SourceEditor::new("note\n");
    editor.set_selection(4, 4);
    typed(&mut editor, " here");
    let agent = set(&editor.text(), 0, 0, "# ");
    apply_remote(&mut editor, &agent, true).unwrap();
    typed(&mut editor, "s");
    assert_eq!(editor.text(), "# note heres\n");

    editor.undo();
    assert_eq!(editor.text(), "# note here\n", "the typing after it");
    editor.undo();
    assert_eq!(editor.text(), "note here\n", "the agent's change");
}

/// UC-006 A7: links a rename rewrote are edits without a seal, as egui's
/// `apply_edits`: undo takes them back by the usual rules.
#[test]
fn uc006_a7_a_rename_rewrite_is_no_step_of_its_own() {
    let mut editor = SourceEditor::new("see [a](old.md)\n");
    let rewrite = set(&editor.text(), 8, 3, "new");
    apply_remote(&mut editor, &rewrite, false).unwrap();
    assert_eq!(editor.text(), "see [a](new.md)\n");
    editor.undo();
    assert_eq!(editor.text(), "see [a](old.md)\n");
}

/// UC-004 A7: after a reload the undo history starts anew.
#[test]
fn uc004_a7_a_reload_starts_the_history_anew() {
    let mut editor = SourceEditor::new("a\n");
    typed(&mut editor, "x");
    editor.reload("outside\n");
    editor.undo();
    assert_eq!(editor.text(), "outside\n");
}

/// ADR-025 section 3.8: the cursor and the selection stay at the same place
/// of the content (egui `source_view.rs:428-437`); an insert at the cursor
/// puts it behind the insert.
#[test]
fn adr025_the_cursor_stays_at_its_place_of_the_content() {
    let mut editor = SourceEditor::new("one two three\n");
    editor.set_selection(9, 4);
    let backend = ChangeSet::of_changes(
        14,
        &[change(0, 0, ">> "), change(7, 3, "2"), change(9, 0, "!")],
    )
    .unwrap();
    let outcome = apply_remote(&mut editor, &backend, false).unwrap();
    assert!(!outcome.moved);
    assert_eq!(editor.text(), ">> one 2 !three\n");
    // The anchor before "two", which became "2"; the cursor in "three".
    assert_eq!((editor.anchor(), editor.cursor()), (7, 11));

    assert_eq!(shifted(5, (5, 5), 2), 7, "an insert at the cursor");
    assert_eq!(shifted(6, (5, 9), 1), 6, "inside, as far as it reaches");
    assert_eq!(shifted(8, (5, 9), 1), 6);
    assert_eq!(shifted(4, (5, 9), 1), 4, "before stays");
}

/// UC-004 A9: edits that came to nothing (typed and deleted) after a
/// skipped reload still fetch it.
#[test]
fn uc004_a9_edits_that_come_to_nothing_fetch_the_skipped_reload() {
    let mut server = Server::new("mine\n");
    let mut client = Client::new("mine\n", server.rev());
    server.reload("outside\n");
    client.ring();
    let pull = client.take_outbox().unwrap();
    let answer = server.receive(&pull).answer;
    client.local(&change(0, 0, "x")).unwrap();
    assert!(client.acknowledge(&answer).unwrap().is_empty(), "skipped");
    client.local(&change(0, 1, "")).unwrap();

    let again = client.take_outbox().expect("fetches the reload again");
    let received = server.receive(&again);

    assert!(received.reverted.is_none(), "nothing to keep");
    let incoming = client.acknowledge(&received.answer).unwrap();
    assert_eq!(
        incoming,
        [Incoming::Reload {
            text: "outside\n".into()
        }]
    );
}

/// UC-004 A9: with several reloads and changes of the backend after them,
/// a late batch keeps the newest text of the mirror as the copy.
#[test]
fn uc004_a9_the_newest_text_is_kept_as_the_copy() {
    let mut server = Server::new("mine\n");
    let mut client = Client::new("mine\n", server.rev());
    client.local(&change(4, 0, "!")).unwrap();
    server.reload("outside 1\n");
    server.backend_edit(&[(0..0, "> ".into())], true).unwrap();
    server.reload("outside 2\n");
    server.backend_edit(&[(9..9, "+".into())], false).unwrap();

    let received = server.receive(&client.take_outbox().unwrap());

    assert_eq!(received.reverted.as_deref(), Some("outside 2+\n"));
    assert_eq!(server.text(), "mine!\n");
    assert!(client.acknowledge(&received.answer).is_ok());
}

/// UC-004 A9, section 3.10: a resync over a reload the window never took
/// keeps the mirror's text as a copy.
#[test]
fn uc004_a9_a_resync_over_an_unconfirmed_reload_keeps_a_copy() {
    let mut server = Server::new("mine\n");
    server.reload("outside\n");

    let resynced = server.resync("mine, edited\n");

    assert_eq!(resynced.outside.as_deref(), Some("outside\n"));
    assert!(!server.reload_unconfirmed());
    assert_eq!(server.resync("again\n").outside, None);
}

/// ADR-025: a batch lost on its way is sent again, on the same revision,
/// with what was typed since.
#[test]
fn adr025_a_lost_batch_is_sent_again() {
    let mut server = Server::new("ab");
    let mut client = Client::new("ab", server.rev());
    client.local(&change(2, 0, "c")).unwrap();
    let lost = client.take_outbox().unwrap();
    client.local(&change(3, 0, "d")).unwrap();

    client.failed();
    let again = client.take_outbox().unwrap();

    assert_eq!(again.base, lost.base);
    let received = server.receive(&again);
    assert!(client.acknowledge(&received.answer).is_ok());
    assert_eq!(server.text(), "abcd");
}

/// ADR-025: a batch on the oldest revision the backend keeps is taken.
#[test]
fn adr025_a_batch_on_the_floor_is_taken() {
    let mut server = Server::new("a");
    for _ in 0..=KEPT {
        server.backend_edit(&[(0..0, "x".into())], false).unwrap();
    }
    let floor = server.rev() - u32::try_from(KEPT).unwrap();
    let on_floor = Batch {
        base: floor,
        changes: vec![change(0, 0, "y")],
    };
    let below = Batch {
        base: floor - 1,
        changes: vec![change(0, 0, "y")],
    };
    assert!(server.clone().receive(&below).answer.resync);
    assert!(!server.receive(&on_floor).answer.resync);
}

/// ADR-025: a set made on a text of another length is not applied.
#[test]
fn adr025_a_set_that_does_not_fit_the_editor_is_refused() {
    let mut editor = SourceEditor::new("abc");
    let wrong = set("abcd", 0, 1, "x");
    assert!(apply_remote(&mut editor, &wrong, false).is_err());
    assert_eq!(editor.text(), "abc");
}
