//! Tests of putting files, dropped items and pasted images into a note
//! (UC-005 A9 to A13, A18, A19, M-10.3 to M-10.6), as the egui window's
//! `answer_tool`, `drop_items` and `paste_image` do it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use crate::editor_view::{DragName, EmbedAnswer, Insertion};
use crate::{Session, SettingsState};

/// A fresh, empty folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-embed-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on the workspace `dir/notes` with `Home` and `guide/Setup`,
/// the tab of `guide/Setup` open, and a folder `dir/outside` beside it.
fn session_in(dir: &Path) -> Session {
    let root = dir.join("notes");
    fs::create_dir_all(root.join("guide")).unwrap();
    fs::create_dir_all(dir.join("outside")).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::write(root.join("Home.md"), "# Home\n").unwrap();
    fs::write(root.join("guide/Setup.md"), "# Setup\n").unwrap();
    let mut session = Session::new(
        Some(root),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.open_note(&setup());
    session.take_events();
    session
}

fn setup() -> NoteId {
    NoteId::from_relative("guide/Setup")
}

fn path(dir: &Path, place: &str) -> String {
    dir.join(place).display().to_string()
}

/// UC-005 A9, A10, M-10.4: a file from outside is copied next to the note;
/// another file of the same name gets the next number and says so; a file
/// that is no picture is a link.
#[test]
fn uc005_a10_m10_4_a_file_from_outside_is_copied_next_to_the_note() {
    let dir = temp_dir("copy");
    let mut session = session_in(&dir);
    fs::write(dir.join("outside/photo.png"), b"one").unwrap();
    let plan = session.embed_plan(&setup()).unwrap();

    let answers = session.finish_embed(plan.embed(&path(&dir, "outside/photo.png"), false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "photo.png".into(),
                image: true
            }
        }]
    );
    assert!(dir.join("notes/guide/photo.png").exists());

    fs::create_dir_all(dir.join("outside/other")).unwrap();
    fs::write(dir.join("outside/other/photo.png"), b"two").unwrap();
    let answers = session.finish_embed(plan.embed(&path(&dir, "outside/other/photo.png"), false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "photo-2.png".into(),
                image: true
            }
        }]
    );
    assert_eq!(
        session.status(),
        "Copied as photo-2.png, since the name was taken"
    );

    fs::write(dir.join("outside/report.pdf"), b"pdf").unwrap();
    let answers = session.finish_embed(plan.embed(&path(&dir, "outside/report.pdf"), false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "report.pdf".into(),
                image: false
            }
        }]
    );
}

/// UC-005 A11: a file of the workspace is referred to where it is.
#[test]
fn uc005_a11_a_file_of_the_workspace_is_referred_to() {
    let dir = temp_dir("inside");
    let mut session = session_in(&dir);
    fs::write(dir.join("notes/logo.svg"), b"<svg/>").unwrap();
    let plan = session.embed_plan(&setup()).unwrap();
    let answers = session.finish_embed(plan.embed(&path(&dir, "notes/logo.svg"), false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "../logo.svg".into(),
                image: true
            }
        }]
    );
}

/// UC-005 A19, M-10.4: a file larger than 10 MB is asked about, in egui's
/// words; "Copy" copies it all the same.
#[test]
fn uc005_a19_m10_4_a_large_file_is_asked_about() {
    let dir = temp_dir("large");
    let mut session = session_in(&dir);
    let big = dir.join("outside/film.png");
    fs::File::create(&big)
        .unwrap()
        .set_len(11 * 1024 * 1024)
        .unwrap();
    let plan = session.embed_plan(&setup()).unwrap();
    let answers = session.finish_embed(plan.embed(&big.display().to_string(), false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Large {
            path: big.display().to_string(),
            question: format!(
                "{} has 11 MB. Copy it into the workspace, or only link to where it is (it is then not part of the workspace and not pushed with git)?",
                big.display()
            ),
        }]
    );
    assert!(!dir.join("notes/guide/film.png").exists());
    let answers = session.finish_embed(plan.embed(&big.display().to_string(), true));
    assert!(matches!(answers[0], EmbedAnswer::Insert { .. }));
    assert!(dir.join("notes/guide/film.png").exists());
}

/// UC-005 A13: a folder or a missing file is refused in the status bar.
#[test]
fn uc005_a13_a_folder_is_refused() {
    let dir = temp_dir("folder");
    let mut session = session_in(&dir);
    let plan = session.embed_plan(&setup()).unwrap();
    let folder = dir.join("outside");
    let answers = session.finish_embed(plan.embed(&folder.display().to_string(), false));
    assert_eq!(answers, vec![EmbedAnswer::Refused]);
    assert_eq!(
        session.status(),
        format!(
            "{} is a folder; folders cannot be put into a note",
            folder.display()
        )
    );
}

/// UC-005 A9, A12, M-10.5: dropped items go in one after the other with a
/// space between them, a web address as a link, a file as dropped (the
/// editor core puts only its place into a code block); nothing to drop is
/// said.
#[test]
fn uc005_a12_m10_5_dropped_items_go_in_with_spaces() {
    let dir = temp_dir("drop");
    let mut session = session_in(&dir);
    fs::write(dir.join("outside/a.png"), b"a").unwrap();
    let plan = session.embed_plan(&setup()).unwrap();
    let answers = session.finish_embed(
        plan.drop_items(&["https://x.org/page".to_owned(), path(&dir, "outside/a.png")]),
    );
    assert_eq!(
        answers,
        vec![
            EmbedAnswer::Insert {
                insertion: Insertion::Link {
                    address: "https://x.org/page".into(),
                    label: None
                }
            },
            EmbedAnswer::Insert {
                insertion: Insertion::Text { text: " ".into() }
            },
            EmbedAnswer::Insert {
                insertion: Insertion::Dropped {
                    place: "a.png".into(),
                    image: true
                }
            },
        ]
    );

    assert!(session.finish_embed(plan.drop_items(&[])).is_empty());
    assert_eq!(session.status(), "This cannot be dropped into a note");

    session.drag(DragName::Over);
    assert_eq!(
        session.status(),
        "Drop to insert at the place under the pointer"
    );
    session.drag(DragName::Left);
    assert_eq!(session.status(), "");
    session.drag(DragName::NoNote);
    assert_eq!(session.status(), "Open a note to drop files into it");
    session.drag(DragName::Left);
    assert_eq!(session.status(), "Open a note to drop files into it");
}

/// M-10.3, UC-005 A2: a note of the workspace is linked by its place
/// relative to the note's folder, with its title.
#[test]
fn m10_3_a_note_is_linked_relative_with_its_title() {
    let dir = temp_dir("link");
    let session = session_in(&dir);
    assert_eq!(
        session
            .note_link(&setup(), &NoteId::from_relative("Home"))
            .unwrap(),
        Insertion::Link {
            address: "../Home.md".into(),
            label: Some("Home".into())
        }
    );
    assert!(session
        .note_link(&NoteId::from_relative("Missing"), &setup())
        .is_err());
}

/// UC-005 A18, BR-014, M-10.6: a pasted image is stored as a PNG next to the
/// note, named after it and the time, and said so.
#[test]
fn uc005_a18_m10_6_a_pasted_image_is_stored_next_to_the_note() {
    let dir = temp_dir("image");
    let mut session = session_in(&dir);
    let plan = session.embed_plan(&setup()).unwrap();
    let stored = plan.store_image(1, 1, &[255, 0, 0, 255], SystemTime::now());
    let name = session.finish_pasted_image(stored).expect("stored");
    assert!(
        name.starts_with("Setup-") && name.ends_with(".png"),
        "{name}"
    );
    assert!(dir.join("notes/guide").join(&name).exists());
    assert_eq!(session.status(), format!("Pasted image stored as {name}"));

    let broken = plan.store_image(2, 2, &[0], SystemTime::now());
    assert_eq!(session.finish_pasted_image(broken), None);
    assert!(
        session
            .status()
            .starts_with("The pasted image could not be stored: "),
        "{}",
        session.status()
    );
}

/// Review F6, A-68: a path through `..` or a symbolic link counts by where
/// it really is: one leaving the workspace is copied, not linked with `..`.
#[cfg(unix)]
#[test]
fn a68_a_path_out_of_the_workspace_through_dots_or_a_link_is_copied() {
    let dir = temp_dir("canonical");
    let mut session = session_in(&dir);
    fs::write(dir.join("outside/photo.png"), b"png").unwrap();
    std::os::unix::fs::symlink(dir.join("outside"), dir.join("notes/linked")).unwrap();
    let plan = session.embed_plan(&setup()).unwrap();

    let dots = path(&dir, "notes/../outside/photo.png");
    let answers = session.finish_embed(plan.embed(&dots, false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "photo.png".into(),
                image: true
            }
        }]
    );
    let linked = path(&dir, "notes/linked/photo.png");
    let answers = session.finish_embed(plan.embed(&linked, false));
    assert_eq!(
        answers,
        vec![EmbedAnswer::Insert {
            insertion: Insertion::File {
                place: "photo.png".into(),
                image: true
            }
        }],
        "the same content: the copy made before is used"
    );
    assert!(dir.join("notes/guide/photo.png").exists());
}

/// Review F6: a pipe is refused before anything reads it; a browser's link
/// file is read only when small, a large one is put in as a file.
#[cfg(unix)]
#[test]
fn a_pipe_is_refused_and_a_large_link_file_is_a_file() {
    let dir = temp_dir("special");
    let mut session = session_in(&dir);
    let pipe = dir.join("outside/pipe.png");
    let made = std::process::Command::new("mkfifo").arg(&pipe).status();
    let plan = session.embed_plan(&setup()).unwrap();
    if made.is_ok_and(|status| status.success()) {
        let answers = session.finish_embed(plan.drop_items(&[pipe.display().to_string()]));
        assert_eq!(answers, vec![EmbedAnswer::Refused]);
        assert_eq!(
            session.status(),
            format!(
                "{} is not a regular file; only files can be put into a note",
                pipe.display()
            )
        );
    }
    let small = dir.join("outside/Site.url");
    fs::write(&small, "[InternetShortcut]\nURL=https://x.org\n").unwrap();
    let large = dir.join("outside/Big.url");
    fs::write(&large, format!("URL=https://y.org\n{}", "x".repeat(70_000))).unwrap();
    let answers = session
        .finish_embed(plan.drop_items(&[small.display().to_string(), large.display().to_string()]));
    assert_eq!(
        answers[0],
        EmbedAnswer::Insert {
            insertion: Insertion::Link {
                address: "https://x.org".into(),
                label: Some("Site".into())
            }
        }
    );
    assert_eq!(
        answers[2],
        EmbedAnswer::Insert {
            insertion: Insertion::Dropped {
                place: "Big.url".into(),
                image: false
            }
        }
    );
}

/// Review F6: copying a note in lists it in the overview; a picture does
/// not make the session list the notes anew.
#[test]
fn a_copied_note_is_listed() {
    let dir = temp_dir("rescan");
    let mut session = session_in(&dir);
    fs::write(dir.join("outside/Extra.md"), "# Extra\n").unwrap();
    let plan = session.embed_plan(&setup()).unwrap();
    let done = plan.embed(&path(&dir, "outside/Extra.md"), false);
    assert!(done.rescanned.is_some());
    session.finish_embed(done);
    assert!(session
        .snapshot()
        .overview
        .iter()
        .any(|item| item.note == "guide/Extra"));
    fs::write(dir.join("outside/a.png"), b"a").unwrap();
    assert!(plan
        .embed(&path(&dir, "outside/a.png"), false)
        .rescanned
        .is_none());
}

/// Kann (review F6): links to notes whose names hold spaces or brackets,
/// in subfolders, are their plain relative places with the title (as
/// egui); the markup made of them is the formatting rules' (Q-52).
#[test]
fn m10_3_note_links_with_spaces_brackets_and_folders() {
    let dir = temp_dir("names");
    let mut session = session_in(&dir);
    fs::create_dir_all(dir.join("notes/a b")).unwrap();
    fs::write(dir.join("notes/a b/Plan [v2].md"), "# Plan\n").unwrap();
    session.open_note(&NoteId::from_relative("Home"));
    let link = session
        .note_link(&setup(), &NoteId::from_relative("a b/Plan [v2]"))
        .unwrap();
    assert_eq!(
        link,
        Insertion::Link {
            address: "../a b/Plan [v2].md".into(),
            label: Some("Plan [v2]".into())
        }
    );
}
