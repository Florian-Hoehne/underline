//! Tests of following links (UC-004 A2 to A4, BR-005 to BR-007, M-07.3),
//! against what `follow_link` of the egui app does. The window sends the
//! place of the click; the session finds the link in its own text.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::Followed;
use crate::editor_view::{LinkAnswer, OpenKindName};
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
    assert_eq!(session.answer_open(true), None, "nothing was asked");
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
            format: crate::FormatName::Markdown,
        })
    );
    assert_eq!(active(&session).as_deref(), Some("Home"));
}

/// A question "Open …" without its id: kind, title, question, warning,
/// and the address as shown.
type Asked = (OpenKindName, String, String, Option<String>, String);

/// The question `followed` asks, without its id; panics when it asks none.
fn asked(followed: &Followed) -> Asked {
    match &followed.answer {
        LinkAnswer::AskOpen {
            about: kind,
            title,
            question,
            warning,
            target,
            ..
        } if followed.open.is_none() => (
            *kind,
            title.clone(),
            question.clone(),
            warning.clone(),
            target.clone(),
        ),
        other => panic!("asked nothing: {other:?}"),
    }
}

/// What the session hands the system to open, with nothing asked.
fn opened(target: &str) -> Followed {
    Followed {
        answer: LinkAnswer::Done,
        open: Some(target.into()),
    }
}

/// UC-004 A4, BR-007, Q-62: `http`, `https` and `mailto` open directly,
/// every time, with nothing asked.
#[test]
fn uc004_br007_q62_http_https_and_mailto_open_directly() {
    let dir = temp_dir("web");
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[go](https://example.org/a?b=1) [mail](mailto:me@example.org) [plain](http://example.org)\n",
        )],
    );

    assert_eq!(go(&mut session), opened("https://example.org/a?b=1"));
    assert_eq!(go(&mut session), opened("https://example.org/a?b=1"));
    assert_eq!(
        click(&mut session, &home(), "mail"),
        opened("mailto:me@example.org")
    );
    assert_eq!(
        click(&mut session, &home(), "plain"),
        opened("http://example.org")
    );
    assert_eq!(session.answer_open(true), None, "nothing was asked");
}

/// UC-004 A4, BR-007, Q-62: any other scheme is asked about every time,
/// naming the scheme and the whole address; "Open" once does not let the
/// next one through, and "Cancel" opens nothing.
#[test]
fn uc004_br007_q62_another_scheme_is_asked_about_every_time() {
    let dir = temp_dir("scheme");
    let address = "ssh://admin@server.example/home";
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[go](ssh://admin@server.example/home) [msdt](MS-MSDT://id%20PCWDiagnostic)\n",
        )],
    );
    let expected = (
        OpenKindName::Link,
        "Open ssh: link".to_owned(),
        "This link uses the ssh: scheme and opens another program on this computer.".to_owned(),
        None,
        address.to_owned(),
    );

    assert_eq!(asked(&go(&mut session)), expected);
    assert_eq!(session.answer_open(true).as_deref(), Some(address));
    assert_eq!(asked(&go(&mut session)), expected, "asked again after Open");
    assert_eq!(session.answer_open(false), None, "Cancel opens nothing");
    assert_eq!(session.answer_open(true), None, "nothing is asked any more");
    let Followed {
        answer: LinkAnswer::AskOpen { title, target, .. },
        open: None,
    } = click(&mut session, &home(), "msdt")
    else {
        panic!("ms-msdt: is asked about");
    };
    assert_eq!(title, "Open ms-msdt: link");
    assert_eq!(target, "MS-MSDT://id%20PCWDiagnostic");
}

#[test]
fn uc004_a4_m07_3_a_later_link_replaces_the_one_asked_about() {
    let dir = temp_dir("web-twice");
    let (mut session, _) = session_with(
        &dir,
        &[(
            "Home.md",
            "[one](ftp://one.example) [two](ftp://two.example)\n",
        )],
    );

    click(&mut session, &home(), "one");
    click(&mut session, &home(), "two");

    assert_eq!(
        session.answer_open(true).as_deref(),
        Some("ftp://two.example")
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

/// UC-004 A4, BR-005, Q-61: a `file://` target is asked about every time,
/// naming the file and its kind; "Open" opens it.
#[test]
fn uc004_a4_q61_a_file_url_is_asked_about() {
    let dir = temp_dir("full-place");
    let outside = dir.join("large.iso");
    fs::write(&outside, "x").unwrap();
    let (mut session, _) = linking(&dir, &format!("file://{}", outside.display()), &[]);
    let place = outside.display().to_string();
    let expected = (
        OpenKindName::File,
        "Open file".to_owned(),
        "This link points to a file on this computer: large.iso, ISO file.".to_owned(),
        None,
        place.clone(),
    );

    assert_eq!(asked(&go(&mut session)), expected);
    assert_eq!(session.answer_open(true), Some(place));
    assert_eq!(asked(&go(&mut session)), expected, "asked again");
}

/// UC-004 A4, BR-005: a file by its full place without `file://` (as a
/// large file is linked, UC-005 A19) opens like a file of the workspace.
#[test]
fn uc004_a4_m07_3_a_file_by_its_full_place_opens_like_a_file() {
    let dir = temp_dir("full-path");
    let outside = dir.join("large.iso");
    fs::write(&outside, "x").unwrap();
    let (mut session, _) = linking(&dir, &outside.display().to_string(), &[]);

    assert_eq!(go(&mut session), opened(&outside.display().to_string()));
}

/// UC-004 BR-005, Q-63: a `file://` place is percent-decoded (`%20` is a
/// space), and a relative one is relative to the note that holds it, not
/// to the program's working folder; it leads outside the workspace when it
/// climbs above it.
#[test]
fn uc004_br005_q63_a_file_link_is_decoded_and_relative_to_the_note() {
    let dir = temp_dir("file-url");
    let spaced = dir.join("a b.txt");
    fs::write(&spaced, "x").unwrap();
    let encoded = format!("file://{}", dir.join("a%20b.txt").display());
    let (mut session, root) = session_with(
        &dir,
        &[
            (
                "Home.md",
                &format!(
                    "[enc]({encoded}) [rel](file://docs/r%201.pdf) [out](file://../../x.pdf)\n"
                ),
            ),
            ("docs/r 1.pdf", "%PDF"),
        ],
    );

    let target_of = |followed: Followed| match followed.answer {
        LinkAnswer::AskOpen { target, .. } => target,
        other => panic!("asked about the file, not {other:?}"),
    };
    assert_eq!(
        target_of(click(&mut session, &home(), "enc")),
        spaced.display().to_string()
    );
    assert_eq!(
        target_of(click(&mut session, &home(), "rel")),
        root.join("docs/r 1.pdf").display().to_string()
    );
    assert_eq!(click(&mut session, &home(), "out"), done());
    assert_eq!(
        session.status(),
        "file://../../x.pdf leads outside the workspace and is not followed"
    );
}

/// UC-004 A4, BR-007, Q-61: a file of the workspace that may run as a
/// program is asked about every time, naming it and its kind; a plain file
/// opens directly.
#[test]
fn uc004_br007_q61_an_executable_file_is_asked_about() {
    let dir = temp_dir("exec");
    let (mut session, root) = session_with(
        &dir,
        &[
            (
                "Home.md",
                "[sh](tools/deploy.sh) [desk](app.desktop) [bat](run.BAT) [pdf](r.pdf)\n",
            ),
            ("tools/deploy.sh", "#!/bin/sh\n"),
            ("app.desktop", "[Desktop Entry]\n"),
            ("run.BAT", "@echo off\n"),
            ("r.pdf", "%PDF"),
        ],
    );

    assert_eq!(
        asked(&click(&mut session, &home(), "sh")),
        (
            OpenKindName::Program,
            "Open program".to_owned(),
            "deploy.sh is a shell script.".to_owned(),
            Some("Opening it may run it on this computer.".to_owned()),
            root.join("tools/deploy.sh").display().to_string(),
        )
    );
    assert_eq!(session.answer_open(false), None);
    for (label, kind) in [("desk", "a desktop launcher"), ("bat", "a batch file")] {
        let LinkAnswer::AskOpen { question, .. } = click(&mut session, &home(), label).answer
        else {
            panic!("{label} is asked about");
        };
        assert!(question.contains(kind), "{question}");
    }
    assert_eq!(
        click(&mut session, &home(), "pdf"),
        opened(&root.join("r.pdf").display().to_string())
    );
}

/// UC-004 BR-007, Q-61: on Unix a file with an execute permission is a
/// program, whatever its name.
#[cfg(unix)]
#[test]
fn uc004_br007_q61_a_file_with_execute_permission_is_asked_about() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("exec-bit");
    let (mut session, root) = linking(&dir, "bin/tool", &[("bin/tool", "#!/bin/sh\n")]);
    let tool = root.join("bin/tool");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();

    let Followed {
        answer:
            LinkAnswer::AskOpen {
                title,
                question,
                target,
                ..
            },
        open: None,
    } = go(&mut session)
    else {
        panic!("a program is asked about");
    };
    assert_eq!(title, "Open program");
    assert!(question.starts_with("tool is a program."), "{question}");
    assert_eq!(target, tool.display().to_string());
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

/// UC-004 A4, Q-61 (monkey finding): a question that was not answered
/// (the window reloaded, the dialog was lost) must not wait for a later
/// "Open": following another link drops it, so a stray answer opens nothing.
#[test]
fn uc004_a4_q61_monkey_a_question_does_not_outlive_the_next_link() {
    let dir = temp_dir("stale-ask");
    let (mut session, _) = session_with(
        &dir,
        &[
            ("Home.md", "[go](tool.exe) and [other](gone.pdf)\n"),
            ("tool.exe", "x"),
        ],
    );
    assert!(matches!(
        click(&mut session, &home(), "go").answer,
        LinkAnswer::AskOpen { .. }
    ));
    // The question is never answered; the user follows another link.
    click(&mut session, &home(), "other");
    assert_eq!(session.answer_open(true), None, "the old question is gone");
}

/// UC-004 A4, Q-62 (monkey finding): an address whose scheme cannot be
/// read (a zero-width character or a byte order mark before `http`, a
/// Cyrillic `һttp`) is asked about, and the question names what was
/// found rather than "Open : link" and "the scheme :".
#[test]
fn uc004_a4_q62_monkey_an_unreadable_scheme_is_still_named() {
    for link in [
        "\u{feff}http://example.org",
        "\u{200b}http://example.org",
        "һttp://example.org",
    ] {
        let dir = temp_dir("odd-scheme");
        let (mut session, _) = linking(&dir, link, &[]);
        let Followed {
            answer: LinkAnswer::AskOpen {
                title, question, ..
            },
            ..
        } = go(&mut session)
        else {
            panic!("{link:?} was not asked about");
        };
        assert_ne!(title, "Open : link", "{link:?}");
        assert!(!question.contains("scheme :"), "{link:?}: {question}");
    }
}

/// UC-004 A4, Q-61: an answer names its question; one to a question no
/// longer asked opens nothing and leaves the asked one alone.
#[test]
fn uc004_a4_q61_an_answer_names_its_question() {
    let dir = temp_dir("answer-id");
    let (mut session, _) = linking(&dir, "ftp://x.example", &[]);
    let LinkAnswer::AskOpen { id: first, .. } = go(&mut session).answer else {
        panic!("asked");
    };
    let LinkAnswer::AskOpen { id: second, .. } = go(&mut session).answer else {
        panic!("asked");
    };
    assert_ne!(first, second);

    assert_eq!(session.answer_question(first, true), None, "stale answer");
    assert_eq!(
        session.answer_question(second, true).as_deref(),
        Some("ftp://x.example")
    );
    assert_eq!(session.answer_question(second, true), None, "answered once");
}

/// UC-004 BR-007, Q-61: a place on another computer is asked about from
/// its text alone, without looking at it, and a name hiding its ending
/// behind a change of direction is shown with the character written out.
#[test]
fn uc004_br007_q61_remote_places_and_hidden_characters() {
    let dir = temp_dir("remote");
    let (mut session, _) = session_with(
        &dir,
        &[
            (
                "Home.md",
                "[unc](//server/share/setup.exe) [doc](//server/share/a.pdf) [bidi](evil%E2%80%AEfdp.exe)\n",
            ),
            ("evil\u{202e}fdp.exe", "MZ"),
        ],
    );

    let (kind, title, question, warning, target) = asked(&click(&mut session, &home(), "unc"));
    assert_eq!(
        (kind, title.as_str(), question.as_str()),
        (
            OpenKindName::Program,
            "Open program",
            "setup.exe is a Windows program."
        )
    );
    assert!(warning.is_some());
    assert_eq!(target, "//server/share/setup.exe");
    let (kind, _, question, _, _) = asked(&click(&mut session, &home(), "doc"));
    assert_eq!(kind, OpenKindName::File);
    assert_eq!(
        question,
        "This link points to a file on another computer or a device: a.pdf."
    );
    let (_, _, question, _, target) = asked(&click(&mut session, &home(), "bidi"));
    assert_eq!(question, "evil\\u{202e}fdp.exe is a Windows program.");
    assert!(target.ends_with("evil\\u{202e}fdp.exe"), "{target}");
}
