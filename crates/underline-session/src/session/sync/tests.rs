//! Tests of the session's side of the sync (ADR-025,
//! `docs/migration/variante-b.md` section 6, step B4b): a window's batches
//! against the mirror, the backend's own changes, the reload barrier, the
//! session's own editor (for the facade's colours and figures) following
//! the mirror, storing, and the events.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};
use underline_editor_core::sync::{Change, Client};

use crate::sync_view::{SyncAnswer, SyncBatch};
use crate::workspace_view::EditingViewName;
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-sync-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on `dir/notes`, whose note `A.md` holds `text`, opened in a
/// tab.
fn session_with(dir: &Path, text: &str) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Home.md"), "# Home\n").unwrap();
    fs::write(root.join("A.md"), text).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    let mut session = Session::new(
        Some(root.clone()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.open_note(&a());
    session.take_events();
    (session, root)
}

fn a() -> NoteId {
    NoteId::from_relative("A")
}

/// The mirror's text and the session's own editor's: always the same.
fn texts(session: &Session) -> String {
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&a()).unwrap());
    assert_eq!(
        tab.mirror.text(),
        tab.edit.editor.text(),
        "the copies are alike"
    );
    tab.mirror.text()
}

/// A window holding the text as the backend had it when the tab opened.
fn window(session: &Session) -> Client {
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&a()).unwrap());
    Client::new(&tab.mirror.text(), tab.mirror.rev())
}

/// Sends what `client` has and takes the answer in, writing a copy to
/// keep as the command does; the answer.
fn sync(session: &mut Session, client: &mut Client) -> SyncAnswer {
    let batch = client.take_outbox().expect("something to send");
    let synced = session.editor_sync(&a(), &SyncBatch::from(&batch)).unwrap();
    if let Some(copy) = &synced.copy {
        let written = copy.write();
        session.copy_kept(copy, written);
    }
    client
        .acknowledge(&(&synced.answer).into())
        .expect("the copies stay alike");
    synced.answer
}

fn edit(at: usize, remove: usize, insert: &str) -> Change {
    Change {
        at,
        remove,
        insert: insert.to_owned(),
    }
}

/// ADR-025, UC-019: an agent's change while the user types: the window's
/// late edit moves over it, and both copies end alike; storing writes the
/// mirror's text.
#[test]
fn uc019_adr025_an_agents_change_while_the_user_types() {
    let dir = temp_dir("agent");
    let (mut session, root) = session_with(&dir, "one\ntwo\n");
    let mut client = window(&session);
    // The user typed at the end of "one"; the agent changes "two".
    client.local(&edit(3, 0, "!")).unwrap();
    session
        .backend_edit(&a(), &[(4..7, "TWO".to_owned())], true)
        .unwrap();

    let answer = sync(&mut session, &mut client);

    assert_eq!(answer.remote.len(), 1, "the agent's change");
    assert_eq!(texts(&session), "one!\nTWO\n");
    session.store_tab(&a());
    assert_eq!(
        fs::read_to_string(root.join("A.md")).unwrap(),
        "one!\nTWO\n"
    );
}

/// ADR-025: the session's own editor (which makes the facade's views for
/// the colours and figures) follows the window's edits, and the autosave
/// stores the mirror's text.
#[test]
fn adr025_the_sessions_editor_follows_the_windows_edits() {
    let dir = temp_dir("follow");
    let (mut session, root) = session_with(&dir, "# A\n\nText 😀\r\nend\n");
    let mut client = window(&session);
    client.local(&edit(0, 0, "x")).unwrap();
    client.local(&edit(13, 0, "\nz")).unwrap();
    client.local(&edit(14, 1, "")).unwrap();
    sync(&mut session, &mut client);
    assert_eq!(texts(&session), "x# A\n\nText 😀\r\n\nend\n");

    let later = Instant::now() + Duration::from_secs(10);
    let start = Instant::now();
    while fs::read_to_string(root.join("A.md")).unwrap() != texts(&session) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the autosave stored"
        );
        session.tick(later);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// ADR-025, M-06.3: edits sent before the tab closes are stored with it;
/// a batch after the close finds no tab.
#[test]
fn adr025_m06_3_edits_sent_before_closing_are_stored() {
    let dir = temp_dir("close");
    let (mut session, root) = session_with(&dir, "note\n");
    let mut client = window(&session);
    client.local(&edit(4, 0, " here")).unwrap();
    sync(&mut session, &mut client);

    assert!(session.close_tab(&a(), false).is_none());

    assert_eq!(
        fs::read_to_string(root.join("A.md")).unwrap(),
        "note here\n"
    );
    client.local(&edit(0, 0, "late ")).unwrap();
    let batch = client.take_outbox().unwrap();
    assert!(session.editor_sync(&a(), &SyncBatch::from(&batch)).is_err());
}

/// ADR-025, UC-004 A9: a window's edits made before a reload from outside
/// take it back: the window's text stays and is stored, the text from
/// outside is kept as a copy, and the status says so as egui does.
#[test]
fn uc004_a9_edits_made_before_a_reload_keep_the_users_text() {
    let dir = temp_dir("barrier");
    let (mut session, root) = session_with(&dir, "mine\n");
    let mut client = window(&session);
    // The window synced once, so reloads wait for it.
    client.ring();
    sync(&mut session, &mut client);
    client.local(&edit(4, 0, "!")).unwrap();
    // Changed outside while the edit travelled.
    fs::write(root.join("A.md"), "outside\n").unwrap();
    {
        let open = session.open.as_mut().unwrap();
        let index = open.tabs.find(&a()).unwrap();
        open.tabs.get_mut(index).reload("outside\n".to_owned());
        assert!(open.tabs.get(index).mirror.reload_unconfirmed());
    }

    sync(&mut session, &mut client);

    assert_eq!(texts(&session), "mine!\n");
    assert!(
        session.status().starts_with(
            "A was changed outside while you were editing; your text stays, the other version is kept as "
        ),
        "{}",
        session.status()
    );
    let copies: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| fs::read_to_string(entry.path()).unwrap_or_default())
        .collect();
    assert!(
        copies.iter().any(|text| text == "outside\n"),
        "kept as a copy"
    );
    session.store_tab(&a());
    assert_eq!(fs::read_to_string(root.join("A.md")).unwrap(), "mine!\n");
}

/// ADR-025, UC-004 A7: without a window that synced its edits, a reload
/// needs no confirmation.
#[test]
fn uc004_a7_the_sessions_editor_takes_a_reload_at_once() {
    let dir = temp_dir("reload");
    let (mut session, _) = session_with(&dir, "mine\n");
    let open = session.open.as_mut().unwrap();
    let index = open.tabs.find(&a()).unwrap();
    open.tabs.get_mut(index).reload("outside\n".to_owned());
    assert!(!open.tabs.get(index).mirror.reload_unconfirmed());
    assert_eq!(texts(&session), "outside\n");
}

/// ADR-025, UC-006 A7, UC-019 BR-007: the backend's changes are byte
/// ranges; the window is told (`ed-changed`) and gets them as edits, an
/// agent's sealed as an undo step of its own, a rename's rewrite not.
#[test]
fn uc006_uc019_backend_changes_by_bytes_with_their_undo_steps() {
    let dir = temp_dir("backend");
    let (mut session, _) = session_with(&dir, "😀 [a](old.md)\n");
    let mut client = window(&session);
    // "😀 [a](" is nine bytes.
    let rev = session
        .backend_edit(&a(), &[(9..12, "new".to_owned())], false)
        .unwrap();
    session
        .backend_edit(&a(), &[(0..0, "> ".to_owned())], true)
        .unwrap();
    assert_eq!(texts(&session), "> 😀 [a](new.md)\n");
    // Ranges out of order do not fit, and change nothing.
    let backwards = [(9..12, String::new()), (2..4, String::new())];
    assert!(session.backend_edit(&a(), &backwards, false).is_err());
    assert_eq!(texts(&session), "> 😀 [a](new.md)\n");

    let events = session.take_events();
    let changed: Vec<u32> = events
        .iter()
        .filter_map(|e| match e {
            Event::Changed(view) => Some(view.rev),
            _ => None,
        })
        .collect();
    assert_eq!(changed, [rev + 1, rev + 1], "told with the revision now");

    client.ring();
    let answer = sync(&mut session, &mut client);
    let sealed: Vec<bool> = answer
        .remote
        .iter()
        .map(|op| match &op.change {
            crate::sync_view::RemoteChange::Edits { sealed, .. } => *sealed,
            crate::sync_view::RemoteChange::Reload { .. } => panic!("no reload"),
        })
        .collect();
    assert_eq!(sealed, [false, true], "the rename's, then the agent's step");
}

/// Q-41 (decided in F14), UC-019: as egui's `apply_edits`, a byte past
/// the end counts as the end and a byte inside a character as that
/// character's start.
#[test]
fn q41_backend_edits_round_bytes_as_egui() {
    let dir = temp_dir("round");
    let (mut session, _) = session_with(&dir, "😀ab\n");
    // Byte 2 lies inside "😀" (bytes 0 to 3): from its start.
    session
        .backend_edit(&a(), &[(2..4, "x".to_owned())], true)
        .unwrap();
    assert_eq!(texts(&session), "xab\n");
    // Past the end: at the end.
    session
        .backend_edit(&a(), &[(99..120, "!".to_owned())], true)
        .unwrap();
    assert_eq!(texts(&session), "xab\n!");
}

/// ADR-025 section 3.10: the window's text wins after the copies went
/// apart; a batch that does not fit asks for it.
#[test]
fn adr025_a_resync_takes_the_windows_text() {
    let dir = temp_dir("resync");
    let (mut session, _) = session_with(&dir, "abc\n");
    let wrong = SyncBatch {
        seq: 0,
        base: 1,
        edits: vec![(&edit(2, 9, "")).into()],
    };
    assert!(session.editor_sync(&a(), &wrong).unwrap().answer.resync);

    let synced = session.editor_resync(&a(), "the window's\n").unwrap();
    assert!(synced.copy.is_none(), "no reload waited");
    let answer = synced.answer;

    assert!(!answer.resync);
    assert_eq!(texts(&session), "the window's\n");
    assert_eq!(
        answer.hash,
        underline_editor_core::sync::hash("the window's\n")
    );
}

/// ADR-025: `ed-flush` as asked, for a note or all.
#[test]
fn adr025_the_window_is_asked_to_flush() {
    let dir = temp_dir("flush");
    let (mut session, _) = session_with(&dir, "a\n");
    session.request_flush(Some(&a()));
    session.request_flush(None);

    let flushes: Vec<Option<String>> = session
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Flush(view) => Some(view.note),
            _ => None,
        })
        .collect();

    assert_eq!(flushes, [Some("A".to_owned()), None]);
}

/// ADR-025 section 2.3: while the facade shows the note, the tick sends
/// what the backend made of its blocks by their content (`ed-enrich`),
/// once; a changed block sends its new entry.
#[test]
fn adr025_the_tick_sends_the_facades_colours_and_figures() {
    let dir = temp_dir("enrich");
    let (mut session, root) = session_with(
        &dir,
        "# A\n\n```rust\nfn main() {}\n```\n\n![Plan](plan.png)\n",
    );
    fs::write(root.join("plan.png"), b"\x89PNG").unwrap();
    session
        .set_editing_view(&a(), EditingViewName::Facade)
        .unwrap();

    let mut enriched = Vec::new();
    let start = Instant::now();
    while enriched
        .iter()
        .all(|e: &crate::EnrichView| e.colors.is_empty())
        && start.elapsed() < Duration::from_secs(10)
    {
        for event in session.tick(Instant::now()) {
            if let Event::Enrich(view) = event {
                enriched.push(*view);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let first = enriched.first().expect("an ed-enrich");
    assert!(first.clear, "the first look");
    assert!(
        enriched.iter().any(|e| !e.colors.is_empty()),
        "code colours"
    );
    // A new look (the colours became ready) sends everything anew.
    let last_clear = enriched.iter().rposition(|e| e.clear).unwrap();
    let images: Vec<&String> = enriched[last_clear..]
        .iter()
        .flat_map(|e| e.images.iter().map(|(target, _)| target))
        .collect();
    assert_eq!(images, ["plan.png"], "once per look");
    let again: Vec<Event> = session
        .tick(Instant::now())
        .into_iter()
        .filter(|e| matches!(e, Event::Enrich(_)))
        .collect();
    assert!(again.is_empty(), "sent once");
}

/// UC-004 A9, section 3.10: a resync over a reload the window never took
/// keeps the version from outside as a copy.
#[test]
fn uc004_a9_a_resync_over_an_unconfirmed_reload_keeps_a_copy() {
    let dir = temp_dir("resync-copy");
    let (mut session, root) = session_with(&dir, "mine\n");
    let mut client = window(&session);
    client.ring();
    sync(&mut session, &mut client);
    {
        let open = session.open.as_mut().unwrap();
        let index = open.tabs.find(&a()).unwrap();
        open.tabs.get_mut(index).reload("outside\n".to_owned());
    }

    let synced = session.editor_resync(&a(), "mine, edited\n").unwrap();

    let copy = synced.copy.expect("a copy to keep");
    let written = copy.write();
    assert!(written.is_ok());
    session.copy_kept(&copy, written);
    assert_eq!(texts(&session), "mine, edited\n");
    assert!(session.status().contains("the other version is kept as"));
    let kept = fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| fs::read_to_string(entry.path()).unwrap_or_default() == "outside\n");
    assert!(kept);
}

/// ADR-025, M-09.4: colours of code the facade's views were made without
/// (a window's edit in a code block, coloured by a later tick) still reach
/// the window through `ed-enrich`.
#[test]
fn adr025_m09_4_colours_made_later_reach_the_window() {
    let dir = temp_dir("recolor");
    let (mut session, _) = session_with(&dir, "# A\n\n```rust\nfn main() {}\n```\n");
    session
        .set_editing_view(&a(), EditingViewName::Facade)
        .unwrap();
    let start = Instant::now();
    // Until the colours are ready and sent.
    let mut sent = false;
    while !sent && start.elapsed() < Duration::from_secs(10) {
        sent = session.tick(Instant::now()).iter().any(
            |e| matches!(e, Event::Enrich(view) if view.colors.iter().any(|(_, c)| c.is_some())),
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(sent, "the first colours");

    // Before the first char of the code, after its fence.
    session.window_type(&a(), 13, "let x = 1; ");

    let mut keys = Vec::new();
    let start = Instant::now();
    while keys.is_empty() && start.elapsed() < Duration::from_secs(10) {
        for event in session.tick(Instant::now()) {
            if let Event::Enrich(view) = event {
                keys.extend(view.colors.into_iter().filter(|(_, c)| c.is_some()));
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(keys.len(), 1, "the new colours of the edited block");
    assert_eq!(
        texts(&session),
        "# A\n\n```rust\nlet x = 1; fn main() {}\n```\n"
    );
}

/// ADR-025: the window's core starts from the backend's text as the file
/// has it (line breaks kept), with its revision; once it synced (its first
/// `ed_sync`), a change of the backend (an agent's) rings it, once.
#[test]
fn adr025_the_window_starts_from_the_mirror_and_hears_of_changes() {
    let dir = temp_dir("mirror");
    let (mut session, _) = session_with(&dir, "one\r\ntwo\n");

    let mirror = session.editor_mirror(&a()).unwrap();

    assert_eq!(mirror.text, "one\r\ntwo\n");
    assert_eq!(mirror.format, "Markdown");
    let mut client = window(&session);
    client.ring();
    sync(&mut session, &mut client);
    session.take_events();
    session
        .backend_edit(&a(), &[(0..0, "x".to_owned())], true)
        .unwrap();
    let rang: Vec<u32> = session
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Changed(view) => Some(view.rev),
            _ => None,
        })
        .collect();
    assert_eq!(rang, [mirror.rev + 1]);
    assert!(
        !session
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Changed(_))),
        "told once"
    );
}

/// ADR-025 section 3.9: before closing, the window is asked for its edits
/// of every note, with `closing`.
#[test]
fn adr025_closing_asks_the_window_for_its_edits() {
    let dir = temp_dir("closing");
    let (mut session, _) = session_with(&dir, "a\n");
    session.request_close_flush();

    let flushes: Vec<(Option<String>, bool)> = session
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Flush(view) => Some((view.note, view.closing)),
            _ => None,
        })
        .collect();

    assert_eq!(flushes, [(None, true)]);
}

/// ADR-025: a batch whose answer was lost, sent again, is answered again
/// and not applied twice.
#[test]
fn adr025_a_batch_sent_again_is_applied_once() {
    let dir = temp_dir("again");
    let (mut session, _) = session_with(&dir, "ab\n");
    let mut client = window(&session);
    client.local(&edit(2, 0, "c")).unwrap();
    let batch = SyncBatch::from(&client.take_outbox().unwrap());

    let first = session.editor_sync(&a(), &batch).unwrap().answer;
    let again = session.editor_sync(&a(), &batch).unwrap().answer;

    assert_eq!(first, again);
    assert_eq!(texts(&session), "abc\n");
}

/// ADR-025: the backend (an agent) changes the start of a text whose
/// window core holds edits at its end not sent yet: the late batch moves
/// over the change, and both texts end alike.
#[test]
fn adr025_a_backend_edit_meets_unsent_window_edits() {
    let dir = temp_dir("meet");
    let (mut session, _) = session_with(&dir, "one\ntwo\n");
    let mut client = window(&session);
    client.ring();
    sync(&mut session, &mut client);
    client.local(&edit(8, 0, "!")).unwrap();
    session
        .backend_edit(&a(), &[(0..0, "> ".to_owned())], true)
        .unwrap();

    let answer = sync(&mut session, &mut client);

    assert_eq!(answer.remote.len(), 1, "the backend's edit");
    assert_eq!(texts(&session), "> one\ntwo\n!");
    assert_eq!(
        answer.hash,
        underline_editor_core::sync::hash("> one\ntwo\n!")
    );
}

/// ADR-025: the window's facade (its core) asks for the facade's colours
/// and figures by showing it; the tick sends them with the theme they are
/// for, also when the tab showed the facade already.
#[test]
fn adr025_the_windows_facade_gets_the_enrichment() {
    let dir = temp_dir("enrich-window");
    let (mut session, _) = session_with(&dir, "# A\n\n```rust\nfn main() {}\n```\n");
    session
        .set_editing_view(&a(), EditingViewName::Facade)
        .unwrap();
    session
        .set_editing_view(&a(), EditingViewName::Facade)
        .unwrap();

    let mut sent = None;
    let start = Instant::now();
    while sent.is_none() && start.elapsed() < Duration::from_secs(10) {
        sent = session
            .tick(Instant::now())
            .into_iter()
            .find_map(|e| match e {
                Event::Enrich(view) if view.colors.iter().any(|(_, c)| c.is_some()) => Some(view),
                _ => None,
            });
        std::thread::sleep(Duration::from_millis(10));
    }

    let sent = sent.expect("an ed-enrich with colours");
    assert_eq!(sent.theme, "Light");
    assert!(!sent.dark);
}
