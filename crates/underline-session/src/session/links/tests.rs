//! Tests of following links (UC-004 A2 to A4, BR-005 to BR-007, M-07.3),
//! against what `follow_link` of the egui app does. The window sends the
//! place of the click; the session finds the link in its own text.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::Followed;
use crate::editor_view::LinkAnswer;
use crate::{Session, SettingsState, UiErrorKind};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-links-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with the notes `files` (relative
/// path, content), `Home.md` open in the active tab.
fn session_with(dir: &Path, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let root = dir.join("notes");
    for (name, text) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
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
    assert!(session.open_note(&home()));
    session.take_events();
    (session, root)
}

/// A session whose `Home.md` holds only `[go](link)`, beside `files`.
fn linking(dir: &Path, link: &str, files: &[(&str, &str)]) -> (Session, PathBuf) {
    let home_text = format!("[go]({link})\n");
    let mut all = vec![("Home.md", home_text.as_str())];
    all.extend_from_slice(files);
    session_with(dir, &all)
}

fn home() -> NoteId {
    NoteId::from_relative("Home")
}

/// The web offset of the first `label` in the backend's text of `note`,
/// plus one: a click inside the label.
fn place_of(session: &Session, note: &NoteId, label: &str) -> u32 {
    let text = session.window_text(note).replace("\r\n", "\n");
    let byte = text.find(label).expect("the label is in the text");
    u32::try_from(text[..byte].encode_utf16().count() + 1).unwrap()
}

/// A Ctrl+click on `label` in `note`, at the backend's revision.
fn click(session: &mut Session, note: &NoteId, label: &str) -> Followed {
    let at = place_of(session, note, label);
    let rev = session.mirror_rev(note);
    session.follow_link(note, at, rev).unwrap()
}

/// A Ctrl+click on the one link `[go](…)` of Home.
fn go(session: &mut Session) -> Followed {
    click(session, &home(), "go")
}

fn active(session: &Session) -> Option<String> {
    session
        .open
        .as_ref()
        .and_then(|open| open.tabs.active())
        .map(|(note, _)| note.as_str().to_owned())
}

fn answer(answer: LinkAnswer) -> Followed {
    Followed { answer, open: None }
}

fn done() -> Followed {
    answer(LinkAnswer::Done)
}

#[test]
fn uc004_a2_m07_3_a_heading_of_the_same_note_is_a_jump_in_web_units() {
    let dir = temp_dir("heading");
    // 🙂 is two web units, the `\r\n` pairs one each.
    let home_text = "# Home 🙂\r\n\r\nSee [goals](#the-goals).\r\n\r\n## The goals\r\n";
    let (mut session, _) = session_with(&dir, &[("Home.md", home_text)]);

    let followed = click(&mut session, &home(), "goals]");

    let at = "# Home 🙂\n\nSee [goals](#the-goals).\n\n## "
        .encode_utf16()
        .count();
    assert_eq!(
        followed,
        answer(LinkAnswer::Jump {
            note: "Home".into(),
            rev: 1,
            offset: u32::try_from(at).unwrap(),
        })
    );
    assert!(
        !session.status().contains("not found"),
        "{}",
        session.status()
    );
}

#[test]
fn uc004_a2_m07_3_a_heading_that_is_not_there_is_said() {
    let dir = temp_dir("no-heading");
    let (mut session, _) = linking(&dir, "#nowhere", &[]);

    assert_eq!(go(&mut session), done());
    assert_eq!(session.status(), "The heading nowhere was not found");
}

#[test]
fn uc004_a2_m07_3_the_link_is_found_in_the_text_the_window_sent() {
    let dir = temp_dir("edited");
    let (mut session, _) = session_with(&dir, &[("Home.md", "# Home\n")]);
    session.window_type(&home(), 7, "\n[down](#later)\n\n## Later\n");

    let followed = click(&mut session, &home(), "down");

    assert_eq!(
        followed,
        answer(LinkAnswer::Jump {
            note: "Home".into(),
            rev: 2,
            offset: 27,
        })
    );
}

#[test]
fn m07_3_a_place_beside_a_link_follows_nothing() {
    let dir = temp_dir("beside");
    let (mut session, _) = session_with(&dir, &[("Home.md", "Plain [go](#x) text\n")]);

    let followed = click(&mut session, &home(), "Plain");

    assert_eq!(followed, done());
    assert_eq!(session.follow_link(&home(), u32::MAX, 1).unwrap(), done());
}

#[test]
fn adr025_m07_3_a_place_of_another_revision_is_not_followed() {
    let dir = temp_dir("other-rev");
    let (mut session, _) = linking(&dir, "https://example.org", &[]);
    let at = place_of(&session, &home(), "go");
    session.window_type(&home(), 0, "x");

    let followed = session.follow_link(&home(), at, 1).unwrap();

    assert_eq!(followed, done());
    assert_eq!(session.answer_web(true), None, "nothing was asked");
}

#[test]
fn uc004_a2_m07_3_an_asciidoc_heading_has_its_asciidoctor_id() {
    let dir = temp_dir("adoc");
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.adoc",
            "= Home\n\nlink:#_next_step[go]\n\n== Next step\n",
        )],
    );

    let followed = go(&mut session);

    assert_eq!(
        followed,
        answer(LinkAnswer::Jump {
            note: "Home".into(),
            rev: 1,
            offset: 33,
        })
    );
}

#[test]
fn uc004_a2_m07_3_another_note_opens_at_its_heading() {
    let dir = temp_dir("note");
    let (mut session, _) = session_with(
        &dir,
        &[
            (
                "Home.md",
                "[plan](guide/Plan.md#steps) [plain](guide/Plan)\n",
            ),
            ("guide/Plan.md", "# Plan\n\n## Steps\n"),
        ],
    );

    let followed = click(&mut session, &home(), "plan]");

    assert_eq!(active(&session).as_deref(), Some("guide/Plan"));
    assert_eq!(
        followed,
        answer(LinkAnswer::Jump {
            note: "guide/Plan".into(),
            rev: 1,
            offset: 11,
        })
    );
    // Without the extension and without a heading: it opens, nothing jumps.
    session.open_note(&home());
    assert_eq!(click(&mut session, &home(), "plain"), done());
    assert_eq!(active(&session).as_deref(), Some("guide/Plan"));
}

#[test]
fn uc004_a2_m07_3_a_missing_heading_of_another_note_leaves_it_open_at_its_place() {
    let dir = temp_dir("note-no-heading");
    let (mut session, _) = linking(&dir, "Plan.md#gone", &[("Plan.md", "# Plan\n")]);

    assert_eq!(go(&mut session), done());
    assert_eq!(active(&session).as_deref(), Some("Plan"));
    assert_eq!(session.status(), "The heading gone was not found");
}

#[cfg(unix)]
#[test]
fn uc004_a10_m07_3_a_note_that_cannot_be_read_jumps_nowhere() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("unreadable");
    let (mut session, root) = linking(&dir, "Plan.md#steps", &[("Plan.md", "## Steps\n")]);
    let plan = root.join("Plan.md");
    fs::set_permissions(&plan, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&plan).is_ok() {
        // Run with the rights to read anything (root): nothing to try.
        return;
    }

    let followed = go(&mut session);

    assert_eq!(followed, done());
    assert_eq!(active(&session).as_deref(), Some("Home"));
    assert!(
        session.status().starts_with("Plan cannot be read: "),
        "{}",
        session.status()
    );
}

#[test]
fn uc004_a3_m07_3_a_missing_note_is_offered_for_creation() {
    let dir = temp_dir("missing");
    let (mut session, _) = linking(&dir, "ideas/Big%20Idea.md", &[]);

    assert_eq!(
        go(&mut session),
        answer(LinkAnswer::NewNote {
            folder: "ideas".into(),
            title: "Big Idea".into(),
            problem: "Big Idea does not exist yet. Create it?".into(),
        })
    );
    assert_eq!(active(&session).as_deref(), Some("Home"));
}

#[test]
fn uc004_a4_m07_3_a_web_address_is_asked_about_once_per_session() {
    let dir = temp_dir("web");
    let address = "https://example.org/a?b=1";
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[go](https://example.org/a?b=1) [mail](mailto:me@example.org)\n",
        )],
    );

    assert_eq!(
        go(&mut session),
        answer(LinkAnswer::AskWeb {
            address: address.into()
        })
    );
    // "Cancel": nothing opens, and the next one is asked about again.
    assert_eq!(session.answer_web(false), None);
    assert_eq!(session.answer_web(true), None, "nothing is asked any more");
    assert!(matches!(go(&mut session).answer, LinkAnswer::AskWeb { .. }));
    assert_eq!(session.answer_web(true).as_deref(), Some(address));
}

#[test]
fn uc004_a4_m07_3_after_open_web_addresses_open_without_asking() {
    let dir = temp_dir("web-allowed");
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[go](https://example.org) [mail](mailto:me@example.org)\n",
        )],
    );
    go(&mut session);
    session.answer_web(true);

    assert_eq!(
        click(&mut session, &home(), "mail"),
        Followed {
            answer: LinkAnswer::Done,
            open: Some("mailto:me@example.org".into()),
        }
    );
    assert_eq!(
        go(&mut session).open.as_deref(),
        Some("https://example.org")
    );
    assert_eq!(session.answer_web(true), None, "nothing was asked");
}

#[test]
fn uc004_a4_m07_3_a_later_web_address_replaces_the_one_asked_about() {
    let dir = temp_dir("web-twice");
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[one](https://one.example) [two](https://two.example)\n",
        )],
    );

    click(&mut session, &home(), "one");
    click(&mut session, &home(), "two");

    assert_eq!(
        session.answer_web(true).as_deref(),
        Some("https://two.example")
    );
}

#[test]
fn uc004_a4_m07_3_a_file_opens_when_it_exists() {
    let dir = temp_dir("file");
    let (mut session, root) = linking(&dir, "docs/report.pdf", &[("docs/report.pdf", "%PDF")]);

    assert_eq!(
        go(&mut session),
        Followed {
            answer: LinkAnswer::Done,
            open: Some(root.join("docs/report.pdf").display().to_string()),
        }
    );
}

#[test]
fn uc004_a4_m07_3_a_file_that_does_not_exist_is_said() {
    let dir = temp_dir("no-file");
    let (mut session, root) = linking(&dir, "docs/gone.pdf", &[]);

    assert_eq!(go(&mut session), done());
    assert_eq!(
        session.status(),
        format!("{} does not exist", root.join("docs/gone.pdf").display())
    );
}

#[test]
fn uc004_a4_m07_3_a_file_by_its_full_place_opens_like_a_file() {
    let dir = temp_dir("full-place");
    let outside = dir.join("large.iso");
    fs::write(&outside, "x").unwrap();
    let (mut session, _) = linking(&dir, &format!("file://{}", outside.display()), &[]);

    assert_eq!(go(&mut session).open, Some(outside.display().to_string()));
}

/// As egui (`resolve_link`, Q-63): a `file://` place is taken as written,
/// neither decoded nor resolved against the note; such a link names no
/// existing file and is said not to exist.
#[test]
fn uc004_a4_m07_3_a_file_link_is_neither_decoded_nor_relative() {
    let dir = temp_dir("file-raw");
    let spaced = dir.join("a b.txt");
    fs::write(&spaced, "x").unwrap();
    let encoded = format!("file://{}", dir.join("a%20b.txt").display());
    let (mut session, _) = session_with(
        &dir,
        &[
            (
                "Home.md",
                &format!("[enc]({encoded}) [rel](file://docs/r.pdf)\n"),
            ),
            ("docs/r.pdf", "%PDF"),
        ],
    );

    assert_eq!(click(&mut session, &home(), "enc"), done());
    assert!(
        session.status().ends_with("a%20b.txt does not exist"),
        "{}",
        session.status()
    );
    assert_eq!(click(&mut session, &home(), "rel"), done());
    assert_eq!(session.status(), "docs/r.pdf does not exist");
}

#[test]
fn uc004_br005_m07_3_a_link_leading_outside_is_not_followed() {
    let dir = temp_dir("outside");
    let (mut session, _) = linking(&dir, "../../etc/passwd", &[]);

    assert_eq!(go(&mut session), done());
    assert_eq!(
        session.status(),
        "../../etc/passwd leads outside the workspace and is not followed"
    );
}

/// As egui: the place is checked by its path, not by where a symbolic link
/// on it leads (Q-60).
#[cfg(unix)]
#[test]
fn uc004_br005_m07_3_a_symbolic_link_out_of_the_workspace_is_followed_like_egui() {
    let dir = temp_dir("symlink");
    let outside = dir.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), "x").unwrap();
    let (mut session, root) = linking(&dir, "out/secret.txt", &[]);
    std::os::unix::fs::symlink(&outside, root.join("out")).unwrap();

    assert_eq!(
        go(&mut session).open,
        Some(root.join("out/secret.txt").display().to_string())
    );
}

#[test]
fn uc004_br007_m07_3_a_failure_to_open_is_said() {
    let dir = temp_dir("open-failed");
    let (mut session, _) = session_with(&dir, &[("Home.md", "")]);

    session.open_failed("https://example.org", "no browser");

    assert_eq!(
        session.status(),
        "https://example.org could not be opened: no browser"
    );
}

#[test]
fn m07_3_a_link_of_a_note_without_a_tab_is_refused() {
    let dir = temp_dir("no-tab");
    let (mut session, _) = session_with(&dir, &[("Home.md", "")]);

    let refused = session
        .follow_link(&NoteId::from_relative("Other"), 0, 1)
        .unwrap_err();

    assert_eq!(refused.kind, UiErrorKind::Invalid);
}
