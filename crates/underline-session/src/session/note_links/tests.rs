//! Tests of the backlinks and the outgoing links (FR-078, FR-079, UC-004
//! step 4, BR-015, A14, NFR-019; TC-004-29 to TC-004-31), and of the
//! headings link completion offers (FR-080, UC-005 BR-018).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::{LINKS_SWEEP, MENTIONS_SHOWN};
use crate::links_view::{
    HeadingView, LinkFormName, LinkKindName, LinkPlace, LinksView, MentionsView, OutgoingLinkView,
};
use crate::{Event, Session, SettingsState};

/// A scratch folder that is removed again.
pub(in crate::session) struct Scratch(PathBuf);

impl Scratch {
    pub(in crate::session) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "underline-session-links-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(in crate::session) fn root(&self) -> PathBuf {
        self.0.join("notes")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(in crate::session) fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub(in crate::session) fn id(note: &str) -> NoteId {
    NoteId::from_relative(note)
}

/// A session on the workspace of `scratch` with the notes `files`, `open`
/// in the active tab.
pub(in crate::session) fn session_with(
    scratch: &Scratch,
    files: &[(&str, &str)],
    open: &str,
) -> Session {
    let root = scratch.root();
    for (name, text) in files {
        write(&root, name, text);
    }
    fs::create_dir_all(scratch.0.join("home")).unwrap();
    let mut session = Session::new(
        Some(root),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(scratch.0.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    assert!(session.open_note(&id(open)));
    session.take_events();
    session
}

/// Ticks until lists come for which `wanted` holds, at most `within`.
pub(in crate::session) fn links_until(
    session: &mut Session,
    within: Duration,
    wanted: impl Fn(&LinksView) -> bool,
) -> LinksView {
    let until = Instant::now() + within;
    let mut last = None;
    loop {
        for event in session.tick(Instant::now()) {
            if let Event::Links(view) = event {
                if wanted(&view) {
                    return view;
                }
                last = Some(view);
            }
        }
        assert!(
            Instant::now() < until,
            "no lists as wanted came; the last: {last:#?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(in crate::session) fn links(session: &mut Session) -> LinksView {
    links_until(session, Duration::from_secs(5), |_| true)
}

/// TC-004-29's notes.
fn tc_004_29_notes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Plan.md", "# Plan\n\n## Goals\n\nSee [top](#goals).\n"),
        (
            "Ideas.md",
            "# Ideas\n\nOne\n[the plan](Plan.md) first\n\nTwo\n\nThree\n[goals](Plan.md#goals)\n",
        ),
        ("travel/Trip.adoc", "= Trip\nlink:../Plan.md[Plan]\n"),
        (
            "Code.md",
            "# Code\n\n`[x](Plan.md)`\n\n```\n[y](Plan.md)\n```\n",
        ),
        ("Log.md", "# Log\n"),
    ]
}

/// TC-004-29 step 1, FR-078, BR-015: three backlinks of "Plan": "Ideas"
/// with lines 4 and 9 and "Trip" with line 2, each with its line and the
/// link marked; not the code of "Code", not "Plan" itself.
#[test]
fn tc_004_29_step_1_backlinks_of_plan() {
    let scratch = Scratch::new("tc29");
    let mut session = session_with(&scratch, &tc_004_29_notes(), "Plan");

    let view = links(&mut session);

    assert_eq!(view.note, "Plan");
    assert_eq!(view.backlink_count, 3);
    let notes: Vec<(&str, Vec<u32>)> = view
        .backlinks
        .iter()
        .map(|note| {
            (
                note.title.as_str(),
                note.links.iter().map(|link| link.line).collect(),
            )
        })
        .collect();
    // In the order of the note overview: by title, none opened before.
    assert_eq!(notes, [("Ideas", vec![4, 9]), ("Trip", vec![2])]);
    let first = &view.backlinks[0].links[0];
    assert_eq!(
        (
            first.before.as_str(),
            first.link.as_str(),
            first.after.as_str()
        ),
        ("", "[the plan](Plan.md)", " first")
    );
    let trip = &view.backlinks[1];
    assert_eq!(trip.note, "travel/Trip");
    assert_eq!(trip.folder.as_deref(), Some("travel"));
    assert_eq!(trip.links[0].link, "link:../Plan.md[Plan]");
}

/// TC-004-29 step 2, UC-004 A14: choosing the backlink of line 9 opens
/// "Ideas" with the cursor at the start of the link.
#[test]
fn tc_004_29_step_2_a14_going_to_a_backlink() {
    let scratch = Scratch::new("tc29-go");
    let mut session = session_with(&scratch, &tc_004_29_notes(), "Plan");
    let view = links(&mut session);
    let place = view.backlinks[0].links[1].place;

    let opened = session
        .links_open(&id("Ideas"), place)
        .unwrap()
        .expect("Ideas opened");

    let text = tc_004_29_notes()[1].1;
    let at = u32::try_from(text.find("[goals]").unwrap()).unwrap();
    assert_eq!(
        (opened.note.as_str(), opened.anchor, opened.head),
        ("Ideas", at, at)
    );
    assert_eq!(
        place,
        LinkPlace {
            from: at,
            to: at + 22,
            check: LinkPlace::check_of("[goals](Plan.md#goals)"),
        }
    );
    let tabs = session.tabs_view();
    assert_eq!(tabs.active.as_deref(), Some("Ideas"));
}

fn outgoing(
    text: &str,
    target: &str,
    kind: LinkKindName,
    broken: bool,
) -> (String, String, LinkKindName, bool) {
    (text.to_owned(), target.to_owned(), kind, broken)
}

/// TC-004-30 step 1, FR-079, BR-015: five outgoing links of "Hub" in the
/// order of the note; "Gone" and "Goals" are broken, the web address is not.
#[test]
fn tc_004_30_step_1_outgoing_and_broken_links() {
    let scratch = Scratch::new("tc30");
    let hub = "# Hub\n\n[Plan](Plan.md)\n[Gone](Gone.md)\n[Goals](Plan.md#nothing)\n\
        [Logo](img/logo.png)\n<https://example.org>\n";
    let mut session = session_with(
        &scratch,
        &[
            ("Hub.md", hub),
            ("Plan.md", "# Plan\n"),
            ("img/logo.png", "png"),
        ],
        "Hub",
    );

    let view = links(&mut session);

    let seen: Vec<_> = view
        .outgoing
        .iter()
        .map(|link| {
            (
                link.text.clone(),
                link.target.clone(),
                link.kind,
                link.broken,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            outgoing("Plan", "Plan", LinkKindName::Note, false),
            outgoing("Gone", "Gone.md", LinkKindName::Note, true),
            outgoing("Goals", "Plan#nothing", LinkKindName::Note, true),
            outgoing("Logo", "img/logo.png", LinkKindName::File, false),
            outgoing(
                "https://example.org",
                "https://example.org",
                LinkKindName::Web,
                false
            ),
        ]
    );
    // A14 step 2: the place of the link to "Plan", in web units of the
    // revision of the lists, and its line.
    let plan: &OutgoingLinkView = &view.outgoing[0];
    assert_eq!((plan.line, plan.offset), (3, 7));
    assert!(view.backlinks.is_empty());
}

/// BR-015: a heading of the note itself, a file that is missing, a link
/// leading outside the workspace, and a heading of another note that
/// exists.
#[test]
fn uc_004_br_015_other_kinds_of_links() {
    let scratch = Scratch::new("kinds");
    let note = "# Top\n\n[up](#top) [down](#bottom) [pdf](missing.pdf) [out](../x.md) \
        [g](Plan.md#goals)\n";
    let mut session = session_with(
        &scratch,
        &[("Note.md", note), ("Plan.md", "# Plan\n\n## Goals\n")],
        "Note",
    );

    let view = links(&mut session);

    let broken: Vec<(&str, bool)> = view
        .outgoing
        .iter()
        .map(|link| (link.target.as_str(), link.broken))
        .collect();
    assert_eq!(
        broken,
        [
            ("#top", false),
            ("#bottom", true),
            ("missing.pdf", true),
            ("../x.md", false),
            ("Plan#goals", false),
        ]
    );
}

/// TC-004-31, NFR-019: the backlinks follow a change made by another
/// program within 2 s, the text of an open tab with changes not stored, and
/// a broken link is no longer marked once its note exists.
#[test]
fn tc_004_31_links_follow_changes() {
    let scratch = Scratch::new("tc31");
    let mut notes = tc_004_29_notes();
    notes.push(("Hub.md", "# Hub\n\n[Gone](Gone.md)\n"));
    let mut session = session_with(&scratch, &notes, "Plan");
    links(&mut session);
    let root = scratch.root();

    // Step 1: another program adds a link to "Log".
    let changed = Instant::now();
    write(&root, "Log.md", "# Log\n\n[plan](Plan.md)\n");
    let view = links_until(&mut session, Duration::from_secs(3), |view| {
        view.backlinks.iter().any(|note| note.title == "Log")
    });
    let took = changed.elapsed();
    eprintln!("NFR-019: a change from outside showed after {took:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
    assert_eq!(view.backlink_count, 4);

    // Step 2: "Ideas" in another tab loses its link of line 4, not stored.
    assert!(session.open_note(&id("Ideas")));
    let ideas = tc_004_29_notes()[1]
        .1
        .replace("[the plan](Plan.md) first", "first");
    session.editor_resync(&id("Ideas"), &ideas).unwrap();
    session.activate_tab(&id("Plan"));
    let view = links_until(&mut session, Duration::from_secs(3), |view| {
        view.note == "Plan" && view.backlink_count == 3
    });
    let lines: Vec<u32> = view.backlinks[0].links.iter().map(|l| l.line).collect();
    assert_eq!(
        (view.backlinks[0].title.as_str(), lines),
        ("Ideas", vec![9])
    );
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        tc_004_29_notes()[1].1,
        "not stored yet"
    );

    // Step 3: once `Gone.md` exists, "Gone" is not broken.
    assert!(session.open_note(&id("Hub")));
    let view = links_until(&mut session, Duration::from_secs(3), |view| {
        view.note == "Hub"
    });
    assert!(view.outgoing[0].broken);
    write(&root, "Gone.md", "# Gone\n");
    links_until(&mut session, Duration::from_secs(3), |view| {
        view.note == "Hub" && !view.outgoing[0].broken
    });
}

/// UC-005 BR-018: the headings of a note with the anchors GitHub gives
/// them, of the open tab or of the file; none of a note that does not
/// exist.
#[test]
fn uc_005_br_018_headings_for_completion() {
    let scratch = Scratch::new("headings");
    let mut session = session_with(
        &scratch,
        &[
            (
                "travel/Packing.md",
                "# Packing\n\n## Clothes\n\n## Papers & Visa\n",
            ),
            ("Guide.adoc", "= Guide\n\n== First Steps\n"),
            ("Ideas.md", "# Ideas\n"),
        ],
        "Ideas",
    );
    let heading = |label: &str, anchor: &str, level: u32| HeadingView {
        label: label.into(),
        anchor: anchor.into(),
        level,
    };

    assert_eq!(
        session.note_headings(&id("travel/Packing")),
        [
            heading("Packing", "packing", 1),
            heading("Clothes", "clothes", 2),
            heading("Papers & Visa", "papers--visa", 2),
        ]
    );
    assert_eq!(
        session.note_headings(&id("Guide")),
        [
            heading("Guide", "_guide", 1),
            heading("First Steps", "_first_steps", 2)
        ]
    );
    session
        .editor_resync(&id("Ideas"), "# Ideas\n\n## New\n")
        .unwrap();
    assert_eq!(
        session.note_headings(&id("Ideas"))[1],
        heading("New", "new", 2)
    );
    assert!(session.note_headings(&id("Nothing")).is_empty());
    session.take_events();
}

/// NFR-019 on 10,000 notes: the first lists come once every note was
/// read; after that a change of one note on disk shows within 2 s, and the
/// links follow every key's revision of the active note at once.
#[test]
fn nfr_019_ten_thousand_notes() {
    let scratch = Scratch::new("nfr019");
    let root = scratch.root();
    for n in 0..10_000 {
        let text = format!(
            "# Note {n}\n\nSome text about note {n}, see [the next](Note-{}.md) and \
             [the hub](../Hub.md#top).\n\n- an item with <https://example.org/{n}>\n\n\
             ```\n[not a link](Hub.md)\n```\n",
            (n + 1) % 10_000
        );
        write(&root, &format!("f{}/Note-{n}.md", n % 50), &text);
    }
    write(&root, "Hub.md", "# Top\n\n[Note 7](f7/Note-7.md)\n");
    let mut session = session_with(&scratch, &[], "Hub");

    let started = Instant::now();
    let view = links_until(&mut session, Duration::from_secs(60), |view| {
        view.note == "Hub"
    });
    eprintln!(
        "NFR-019: the first lists of 10,000 notes after {:?}",
        started.elapsed()
    );
    assert_eq!(view.backlink_count, 10_000);

    let changed = Instant::now();
    write(&root, "f3/Note-3.md", "# Note 3\n\nNo link any more.\n");
    links_until(&mut session, Duration::from_secs(5), |view| {
        view.backlink_count == 9_999
    });
    let took = changed.elapsed();
    eprintln!("NFR-019: one changed note of 10,000 showed after {took:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// UC-005 BR-018, OQ-201: link completion writes the place of a note as the
/// formatting toolbar writes a link to it (`note_link`): relative to the
/// note, with its extension.
#[test]
fn uc_005_br_018_completion_writes_the_place_as_the_toolbar() {
    use underline_editor_view::editor_view::{EditorInput, Insertion, LinkChoice};
    use underline_editor_view::EditorTab;

    let scratch = Scratch::new("as-toolbar");
    let files = [
        ("a/Ideas.md", "# Ideas\n"),
        ("a/b/Deep.md", "# Deep\n"),
        ("travel/Packing.md", "# Packing\n"),
        ("Plan.adoc", "= Plan\n"),
    ];
    let session = session_with(&scratch, &files, "a/Ideas");
    for (target, format) in [
        ("a/b/Deep", "Markdown"),
        ("travel/Packing", "Markdown"),
        ("Plan", "AsciiDoc"),
    ] {
        let Insertion::Link { address, label } =
            session.note_link(&id("a/Ideas"), &id(target)).unwrap()
        else {
            panic!("a link");
        };
        let mut tab = EditorTab::new("", underline_core::NoteFormat::Markdown);
        tab.source_input(
            EditorInput::Text {
                text: "[](x".into(),
            },
            false,
        );
        let rev = tab.link_context("a/Ideas", false, false).unwrap().rev;
        let choice = LinkChoice {
            rev,
            note: Some(target.into()),
            format: Some(format.into()),
            anchor: None,
            label: label.clone().unwrap(),
        };
        tab.source_complete("a/Ideas", &choice).unwrap();
        assert_eq!(
            tab.editor.text(),
            format!("[{}]({address})", label.unwrap()),
            "{target}"
        );
    }
}

/// ADR-022, NFR-019: the files are looked at every second, but the window
/// is told only when the lists changed.
#[test]
fn nfr_019_unchanged_lists_are_not_sent_again() {
    let scratch = Scratch::new("quiet");
    let mut session = session_with(&scratch, &tc_004_29_notes(), "Plan");
    links(&mut session);

    let until = Instant::now() + LINKS_SWEEP * 2 + Duration::from_millis(300);
    while Instant::now() < until {
        let events = session.tick(Instant::now());
        assert!(
            !events.iter().any(|event| matches!(event, Event::Links(_))),
            "{events:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// TC-004-29 step 2, UC-004 A14, OQ-249: a backlink whose note changed
/// since the list was made opens the note without a jump and says so.
#[test]
fn tc_004_29_step_2_a_link_that_moved_is_said() {
    let scratch = Scratch::new("moved");
    let mut session = session_with(&scratch, &tc_004_29_notes(), "Plan");
    let view = links(&mut session);
    let place = view.backlinks[0].links[1].place;
    write(
        &scratch.root(),
        "Ideas.md",
        "# Ideas\n\nMoved down\n\n\n\n\n\n\n\n\n[goals](Plan.md#goals)\n",
    );

    let opened = session.links_open(&id("Ideas"), place).unwrap();

    assert_eq!(opened, None);
    assert_eq!(
        session.status(),
        "Ideas changed since the backlinks were listed; the link is no longer at its place"
    );
    assert_eq!(session.tabs_view().active.as_deref(), Some("Ideas"));
}

/// FR-079: choosing an outgoing link in an older revision follows nothing
/// and says why.
#[test]
fn tc_004_30_step_3_a_stale_choice_is_said() {
    let scratch = Scratch::new("stale");
    let mut session = session_with(
        &scratch,
        &[("Hub.md", "[Plan](Plan.md)\n"), ("Plan.md", "# Plan\n")],
        "Hub",
    );
    let view = links(&mut session);
    session
        .editor_resync(&id("Hub"), "x [Plan](Plan.md)\n")
        .unwrap();

    let followed = session
        .follow_outgoing(&id("Hub"), view.outgoing[0].offset, view.rev)
        .unwrap();

    assert_eq!(followed.open, None);
    assert_eq!(
        session.status(),
        "Hub changed since its links were listed; choose the link again"
    );
    // In the revision of the lists it is followed: "Plan" opens.
    let view = links(&mut session);
    session
        .follow_outgoing(&id("Hub"), view.outgoing[0].offset, view.rev)
        .unwrap();
    assert_eq!(session.tabs_view().active.as_deref(), Some("Plan"));
}

/// UC-004 A14: the check of a backlink's markup is FNV-1a over its bytes,
/// the same number the window's mock computes (`mockLinkCheck`).
#[test]
fn uc_004_a14_the_check_of_a_link() {
    assert_eq!(LinkPlace::check_of("[goals](Plan.md#goals) é"), 436_401_621);
}

/// UC-004 A14 (chaos): a place that cuts a character in two (a hostile or
/// stale request) is no place; it must not panic the session.
#[test]
fn uc_004_a14_chaos_place_inside_a_character() {
    let scratch = Scratch::new("midchar");
    let mut session = session_with(
        &scratch,
        &[("Plan.md", "# Plan\n"), ("Hub.md", "é [Plan](Plan.md)\n")],
        "Plan",
    );
    let place = LinkPlace {
        from: 1,
        to: 2,
        check: LinkPlace::check_of(""),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.links_open(&id("Hub"), place)
    }));
    assert!(result.is_ok(), "links_open panicked on a place inside é");
}

/// UC-004 BR-015, BR-005 (chaos): two headings of the same name get the
/// anchors GitHub gives them: `intro`, `intro-1`; a link to the second is
/// not broken.
#[test]
fn uc_004_br_015_chaos_duplicate_headings_have_numbered_anchors() {
    let scratch = Scratch::new("dupes");
    let mut session = session_with(
        &scratch,
        &[
            ("Hub.md", "[a](Plan.md#intro) [b](Plan.md#intro-1)\n"),
            ("Plan.md", "# Plan\n\n## Intro\n\n## Intro\n"),
        ],
        "Hub",
    );
    let view = links(&mut session);
    let broken: Vec<_> = view.outgoing.iter().map(|l| l.broken).collect();
    assert_eq!(broken, [false, false], "{:#?}", view.outgoing);
    let anchors: Vec<_> = session
        .note_headings(&id("Plan"))
        .into_iter()
        .map(|h| h.anchor)
        .collect();
    assert_eq!(anchors, ["plan", "intro", "intro-1"]);
}

/// UC-004 BR-015 (chaos): text in a formula holds no links.
#[test]
fn uc_004_br_015_chaos_formula_holds_no_links() {
    let scratch = Scratch::new("formula");
    let mut session = session_with(
        &scratch,
        &[
            ("Plan.md", "# Plan\n"),
            ("Math.md", "$$\n[x](Plan.md)\n$$\n"),
        ],
        "Plan",
    );
    let view = links(&mut session);
    assert!(view.backlinks.is_empty(), "{:#?}", view.backlinks);
}

/// UC-004 BR-005 (chaos): a percent-encoded multibyte link finds its note.
#[test]
fn uc_004_br_005_chaos_percent_encoded_multibyte_backlink() {
    let scratch = Scratch::new("cafe");
    let mut session = session_with(
        &scratch,
        &[("Café.md", "# Café\n"), ("Hub.md", "[c](Caf%C3%A9.md)\n")],
        "Café",
    );
    let view = links(&mut session);
    assert_eq!(view.backlinks.len(), 1, "{view:#?}");
}

/// UC-005 BR-018 (chaos): the anchors of headings with inline markup are
/// those of their text.
#[test]
fn uc_005_br_018_chaos_heading_with_markup() {
    let scratch = Scratch::new("markup");
    let session = session_with(
        &scratch,
        &[(
            "Plan.md",
            "## See [Plan](Plan.md) now\n\n## `code` **bold** x\n\n## Goals ##\n",
        )],
        "Plan",
    );
    let anchors: Vec<_> = session
        .note_headings(&id("Plan"))
        .into_iter()
        .map(|h| h.anchor)
        .collect();
    assert_eq!(anchors, ["see-plan-now", "code-bold-x", "goals"]);
}

/// NFR-019 (chaos): reading the links of a note grows linearly with its
/// text, also when all its links are on one very long line (a pasted
/// export): four times the links must not take sixteen times as long. Now
/// every link counts the characters of the line before it again
/// (`context`), so 10,000 links on one line take 16 s in a debug build.
#[test]
fn nfr_019_chaos_one_very_long_line_of_links() {
    let read = |links: usize| {
        let text = "[a](Plan.md)".repeat(links);
        let started = Instant::now();
        let held = super::held_links(
            Path::new("/w"),
            &id("Long"),
            underline_core::NoteFormat::Markdown,
            &text,
        );
        assert_eq!(held.len(), links);
        started.elapsed()
    };
    let small = read(1_500);
    let large = read(6_000);
    eprintln!("1,500 links {small:?}, 6,000 links {large:?}");
    assert!(
        large < small * 8 + Duration::from_millis(100),
        "4x the links took {large:?} against {small:?}: not linear"
    );
}

/// UC-004 BR-005 (chaos): a symlink loop and a link to a symlinked note
/// neither hang nor lose the other links.
#[test]
fn uc_004_br_005_chaos_symlinks() {
    let scratch = Scratch::new("symlinks");
    let root = scratch.root();
    write(&root, "Plan.md", "# Plan\n");
    write(
        &root,
        "Hub.md",
        "[p](Plan.md) [l](loop/Plan.md) [alias](Alias.md)\n",
    );
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    std::os::unix::fs::symlink(root.join("Plan.md"), root.join("Alias.md")).unwrap();
    let mut session = session_with(&scratch, &[], "Plan");
    let view = links(&mut session);
    assert!(
        view.backlinks.iter().any(|note| note.note == "Hub"),
        "{view:#?}"
    );
}

/// UC-004 BR-005, BR-015 (chaos): a link to a heading in other case
/// (`#Goals`) or with the anchor percent-encoded (`#caf%C3%A9`) leads to
/// the heading, as in the browsers that follow it; it is not broken.
#[test]
fn uc_004_br_015_chaos_anchor_case_and_escapes() {
    let scratch = Scratch::new("anchor-case");
    let mut session = session_with(
        &scratch,
        &[(
            "Hub.md",
            "# Goals\n\n## Café\n\n[a](#Goals) [b](#caf%C3%A9) [c](Hub.md#GOALS)\n",
        )],
        "Hub",
    );
    let view = links(&mut session);
    let broken: Vec<_> = view.outgoing.iter().map(|l| l.broken).collect();
    assert_eq!(broken, [false, false, false], "{:#?}", view.outgoing);
}

/// The outgoing links of a view as (text, target, kind, form, broken).
fn forms(view: &LinksView) -> Vec<(String, String, LinkKindName, LinkFormName, bool)> {
    view.outgoing
        .iter()
        .map(|link| {
            (
                link.text.clone(),
                link.target.clone(),
                link.kind,
                link.form,
                link.broken,
            )
        })
        .collect()
}

/// TC-004-41 step 1, OQ-244, BR-015: pictures and included files are
/// outgoing links, marked as such; a missing one is broken; in code they
/// are none. Markdown and AsciiDoc.
#[test]
fn tc_004_41_step_1_pictures_and_includes_are_outgoing_links() {
    let scratch = Scratch::new("tc41");
    let hub = "# Hub\n\n![Logo](img/logo.png) ![gone](img/gone.png)\n\
        ![web](https://example.org/a.png)\n`![x](img/logo.png)`\n";
    let book = "= Book\n\ninclude::Chapter.adoc[]\ninclude::Missing.adoc[]\n\
        image::img/logo.png[Logo]\nimage:img/gone.png[]\n----\ninclude::Code.adoc[]\n----\n";
    let mut session = session_with(
        &scratch,
        &[
            ("Hub.md", hub),
            ("Book.adoc", book),
            ("Chapter.adoc", "= Chapter\n"),
            ("img/logo.png", "png"),
        ],
        "Hub",
    );
    let file = |text: &str, target: &str, broken| {
        (
            text.to_owned(),
            target.to_owned(),
            LinkKindName::File,
            LinkFormName::Picture,
            broken,
        )
    };

    let view = links(&mut session);
    assert_eq!(
        forms(&view),
        [
            file("img/logo.png", "img/logo.png", false),
            file("img/gone.png", "img/gone.png", true),
            (
                "https://example.org/a.png".to_owned(),
                "https://example.org/a.png".to_owned(),
                LinkKindName::Web,
                LinkFormName::Picture,
                false
            ),
        ]
    );

    assert!(session.open_note(&id("Book")));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "Book"
    });
    assert_eq!(
        forms(&view),
        [
            (
                "Chapter.adoc".to_owned(),
                "Chapter".to_owned(),
                LinkKindName::Note,
                LinkFormName::Include,
                false
            ),
            (
                "Missing.adoc".to_owned(),
                "Missing.adoc".to_owned(),
                LinkKindName::Note,
                LinkFormName::Include,
                true
            ),
            file("img/logo.png", "img/logo.png", false),
            file("img/gone.png", "img/gone.png", true),
        ]
    );
}

/// TC-004-41 step 2, OQ-244, FR-078: a note included by another lists it
/// as a backlink, marked as an include; a link and an include of the same
/// note are told apart.
#[test]
fn tc_004_41_step_2_an_include_is_a_backlink() {
    let scratch = Scratch::new("tc41-back");
    let mut session = session_with(
        &scratch,
        &[
            ("Chapter.adoc", "= Chapter\n"),
            (
                "Book.adoc",
                "= Book\n\nxref:Chapter.adoc[The chapter]\ninclude::Chapter.adoc[]\n",
            ),
            ("Pics.md", "# Pics\n\n![chapter](Chapter.adoc)\n"),
        ],
        "Chapter",
    );

    let view = links(&mut session);

    assert_eq!(view.backlink_count, 3);
    let seen: Vec<(&str, u32, LinkFormName, &str)> = view
        .backlinks
        .iter()
        .flat_map(|note| {
            note.links.iter().map(|link| {
                (
                    note.title.as_str(),
                    link.line,
                    link.form,
                    link.link.as_str(),
                )
            })
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                "Book",
                3,
                LinkFormName::Link,
                "xref:Chapter.adoc[The chapter]"
            ),
            ("Book", 4, LinkFormName::Include, "include::Chapter.adoc[]"),
            ("Pics", 3, LinkFormName::Picture, "![chapter](Chapter.adoc)"),
        ]
    );
}

/// TC-004-41 step 3, OQ-244, A14: following an included note from the
/// outgoing links opens it; following a picture opens it with the system's
/// program; a picture of a program asks first (Q-61).
#[test]
fn tc_004_41_step_3_following_a_picture_and_an_include() {
    let scratch = Scratch::new("tc41-follow");
    let book = "= Book\n\ninclude::Chapter.adoc[]\nimage::logo.png[]\nimage::run.desktop[]\n";
    let mut session = session_with(
        &scratch,
        &[
            ("Book.adoc", book),
            ("Chapter.adoc", "= Chapter\n"),
            ("logo.png", "png"),
            ("run.desktop", "[Desktop Entry]\n"),
        ],
        "Book",
    );
    let view = links(&mut session);
    let (rev, include, picture, program) = (
        view.rev,
        view.outgoing[0].offset,
        view.outgoing[1].offset,
        view.outgoing[2].offset,
    );

    let followed = session.follow_outgoing(&id("Book"), picture, rev).unwrap();
    let opened = followed.open.expect("opened with the system");
    assert!(opened.ends_with("logo.png"), "{opened}");

    let followed = session.follow_outgoing(&id("Book"), program, rev).unwrap();
    assert!(followed.open.is_none(), "asked first");
    assert!(
        matches!(
            followed.answer,
            crate::editor_view::LinkAnswer::AskOpen { .. }
        ),
        "{:?}",
        followed.answer
    );

    session.follow_outgoing(&id("Book"), include, rev).unwrap();
    assert_eq!(session.tabs_view().active.as_deref(), Some("Chapter"));
}

/// The mentions of a view as (note, line, before, mention, after).
fn mentioned(view: &MentionsView) -> Vec<(String, u32, String, String, String)> {
    view.notes
        .iter()
        .flat_map(|note| {
            note.mentions.iter().map(|mention| {
                (
                    note.title.clone(),
                    mention.line,
                    mention.before.clone(),
                    mention.mention.clone(),
                    mention.after.clone(),
                )
            })
        })
        .collect()
}

/// TC-004-42 step 1, FR-103, BR-021: the unlinked mentions of "Road map"
/// in the other notes, in the order of the overview, with their lines;
/// not the linked one, not in code, not in the note itself; both formats.
#[test]
fn tc_004_42_step_1_unlinked_mentions() {
    let scratch = Scratch::new("tc42");
    let mut session = session_with(
        &scratch,
        &[
            ("Road-map.md", "# Road map\n\nThe road map itself.\n"),
            (
                "Ideas.md",
                "# Ideas\n\nSee the road map first.\n[Road map](Road-map.md)\n`road map`\n",
            ),
            ("travel/Trip.adoc", "= Trip\n\nThe ROAD MAP, again.\r\n"),
            ("Log.md", "# Log\n\nroadmap and road maps\n"),
        ],
        "Road-map",
    );

    let view = links(&mut session);

    let mut seen = mentioned(&view.mentions);
    seen.sort();
    assert_eq!(
        seen,
        [
            (
                "Ideas".to_owned(),
                3,
                "See the ".to_owned(),
                "road map".to_owned(),
                " first.".to_owned()
            ),
            (
                "Trip".to_owned(),
                3,
                "The ".to_owned(),
                "ROAD MAP".to_owned(),
                ", again.".to_owned()
            ),
        ]
    );
    assert_eq!((view.mentions.count, view.mentions.more), (2, 0));
    assert!(!view.mentions.too_short);
    let trip = view
        .mentions
        .notes
        .iter()
        .find(|note| note.title == "Trip")
        .unwrap();
    assert_eq!(trip.folder.as_deref(), Some("travel"));
    // The backlink of "Ideas" is a backlink, not a mention.
    assert_eq!(view.backlink_count, 1);
}

/// TC-004-42 step 2, BR-021: a note open in a tab counts with its text not
/// stored yet; a title of fewer than three characters has no mentions.
#[test]
fn tc_004_42_step_2_open_tabs_and_short_titles() {
    let scratch = Scratch::new("tc42-open");
    let mut session = session_with(
        &scratch,
        &[
            ("Plan.md", "# Plan\n"),
            ("Ideas.md", "# Ideas\n"),
            ("Go.md", "# Go\n\nGo plan go\n"),
        ],
        "Plan",
    );
    let view = links(&mut session);
    assert_eq!(view.mentions.count, 1, "{:?}", view.mentions);

    assert!(session.open_note(&id("Ideas")));
    session
        .editor_resync(&id("Ideas"), "# Ideas\n\nA plan, a Plan.\n")
        .unwrap();
    session.activate_tab(&id("Plan"));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "Plan" && view.mentions.count == 3
    });
    let ideas = view
        .mentions
        .notes
        .iter()
        .find(|note| note.title == "Ideas")
        .unwrap();
    assert_eq!(ideas.mentions.len(), 2, "not stored yet");

    assert!(session.open_note(&id("Go")));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "Go"
    });
    assert!(view.mentions.too_short);
    assert!(view.mentions.notes.is_empty());
    assert_eq!(view.mentions.count, 0);
}

/// TC-004-42 step 4, BR-021: at most 200 mentions are listed; "Show more"
/// lists 200 more, and another note starts at 200 again.
#[test]
fn tc_004_42_step_4_at_most_200_then_more() {
    let scratch = Scratch::new("tc42-more");
    let many = "plan ".repeat(150);
    let mut session = session_with(
        &scratch,
        &[
            ("Plan.md", "# Plan\n"),
            ("A.md", many.as_str()),
            ("B.md", many.as_str()),
            ("C.md", many.as_str()),
        ],
        "Plan",
    );
    let view = links(&mut session);
    let listed = |view: &LinksView| {
        view.mentions
            .notes
            .iter()
            .map(|note| note.mentions.len())
            .sum::<usize>()
    };
    assert_eq!(listed(&view), MENTIONS_SHOWN);
    assert_eq!((view.mentions.count, view.mentions.more), (450, 250));

    session.links_more_mentions(&id("Plan"));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.mentions.more == 50
    });
    assert_eq!(listed(&view), 400);

    // Another note's "Show more" changes nothing here.
    session.links_more_mentions(&id("A"));
    assert!(session.open_note(&id("A")));
    links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "A"
    });
    session.activate_tab(&id("Plan"));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "Plan"
    });
    assert_eq!(listed(&view), MENTIONS_SHOWN, "back to 200");
}

/// NFR-019 for the unlinked mentions: the mentions of a note's title in
/// 10,000 notes are found within 2 s of switching to it, and a change of
/// one note shows within 2 s.
#[test]
fn nfr_019_mentions_in_ten_thousand_notes() {
    let scratch = Scratch::new("nfr019-mentions");
    let root = scratch.root();
    for n in 0..10_000 {
        let text = format!(
            "# Note {n}\n\nSome text about the hub and note {n}, see [the hub](../Hub.md) \
             and `hub`.\n\n- an item with <https://example.org/{n}>\n"
        );
        write(&root, &format!("f{}/Note-{n}.md", n % 50), &text);
    }
    write(&root, "Hub.md", "# Hub\n");
    let mut session = session_with(&scratch, &[], "f7/Note-7");
    links_until(&mut session, Duration::from_secs(60), |view| {
        view.note == "f7/Note-7"
    });

    assert!(session.open_note(&id("Hub")));
    let started = Instant::now();
    let view = links_until(&mut session, Duration::from_secs(60), |view| {
        view.note == "Hub"
    });
    let took = started.elapsed();
    // Every note mentions "Hub", so every one is read and searched; the
    // lists of a note shown first are no stored change, so NFR-019 does
    // not bound this time (as in `nfr_019_ten_thousand_notes`).
    eprintln!("NFR-019: the mentions of Hub in 10,000 notes after {took:?}");
    assert_eq!(view.mentions.count, 10_000);
    assert_eq!(view.mentions.more, 10_000 - 200);

    // A note no other note mentions is shown at once: no file is read.
    assert!(session.open_note(&id("f9/Note-9")));
    let started = Instant::now();
    let view = links_until(&mut session, Duration::from_secs(60), |view| {
        view.note == "f9/Note-9"
    });
    let took = started.elapsed();
    eprintln!("NFR-019: the mentions of Note 9 in 10,000 notes after {took:?}");
    assert_eq!(view.mentions.count, 0);
    assert!(took < Duration::from_secs(2), "{took:?}");
    session.activate_tab(&id("Hub"));
    links_until(&mut session, Duration::from_secs(60), |view| {
        view.note == "Hub"
    });

    let changed = Instant::now();
    write(&root, "f3/Note-3.md", "# Note 3\n\nNo mention any more.\n");
    links_until(&mut session, Duration::from_secs(5), |view| {
        view.mentions.count == 9_999
    });
    let took = changed.elapsed();
    eprintln!("NFR-019: one changed mention of 10,000 showed after {took:?}");
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// TC-004-41 step 1, OQ-244, BR-015 (monkey): a picture that holds its
/// data in its address (`data:image/png;base64,…`) is no link to a note
/// that is missing: the outgoing links do not mark it as broken.
#[test]
fn tc_004_41_monkey_data_picture_is_not_a_broken_note() {
    let scratch = Scratch::new("tc41-data");
    let mut session = session_with(
        &scratch,
        &[("Hub.md", "# Hub\n\n![x](data:image/png;base64,AAAA)\n")],
        "Hub",
    );
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        !view.outgoing.is_empty()
    });
    let link = &view.outgoing[0];
    assert!(
        !link.broken,
        "marked broken, as a {:?} to {:?}",
        link.kind, link.target
    );
}

/// OQ-244 (review): a picture held in the note is listed as such, with its
/// place shortened, and following it opens nothing.
#[test]
fn tc_004_41_data_picture_is_shortened_and_not_followed() {
    let scratch = Scratch::new("tc41-data-follow");
    let data = format!("data:image/png;base64,{}", "A".repeat(5000));
    let hub = format!("# Hub\n\n![x]({data})\n");
    let mut session = session_with(&scratch, &[("Hub.md", hub.as_str())], "Hub");
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        !view.outgoing.is_empty()
    });
    let link = &view.outgoing[0];
    assert_eq!(
        (link.kind, link.form, link.broken),
        (LinkKindName::Inline, LinkFormName::Picture, false)
    );
    assert!(link.target.chars().count() <= 81, "{}", link.target.len());
    assert!(link.target.ends_with('…'));
    let followed = session
        .follow_outgoing(&id("Hub"), link.offset, view.rev)
        .unwrap();
    assert!(followed.open.is_none());
    assert_eq!(
        session.status(),
        "This picture is held in the note itself; there is nothing to open"
    );
}

/// BR-015 (review): links and pictures of files that exist, which are no
/// notes, are not broken: a script, a picture, a relative `file://` place.
#[test]
fn uc_004_br_015_existing_files_are_not_broken() {
    let scratch = Scratch::new("files-not-broken");
    let hub = "# Hub\n\n[deploy](tools/deploy.sh) ![plan](img/plan.svg) \
        [f](file://tools/deploy.sh) [g](tools/gone.sh)\n";
    let mut session = session_with(
        &scratch,
        &[
            ("Hub.md", hub),
            ("tools/deploy.sh", "echo\n"),
            ("img/plan.svg", "<svg/>"),
        ],
        "Hub",
    );
    let view = links(&mut session);
    let broken: Vec<(&str, bool)> = view
        .outgoing
        .iter()
        .map(|link| (link.text.as_str(), link.broken))
        .collect();
    assert_eq!(
        broken,
        [
            ("deploy", false),
            ("img/plan.svg", false),
            ("f", false),
            ("g", true)
        ]
    );
}
