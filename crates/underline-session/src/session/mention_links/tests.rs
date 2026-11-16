//! Tests of linking an unlinked mention (FR-103, UC-004 A22, BR-021,
//! TC-004-42 step 3, OQ-199, OQ-248): in a note open in a tab as one undo
//! step, in a closed note after a snapshot, and refused when the mention
//! moved.

use std::fs;
use std::time::Duration;

use super::super::note_links::tests::{id, links, links_until, session_with, write, Scratch};
use super::MentionPlan;
use crate::links_view::{LinkPlace, LinksView, MentionView};
use crate::Session;

/// The first mention listed in the note titled `title`.
fn mention_in(view: &LinksView, title: &str) -> MentionView {
    view.mentions
        .notes
        .iter()
        .find(|note| note.title == title)
        .and_then(|note| note.mentions.first())
        .cloned()
        .unwrap_or_else(|| panic!("no mention in {title}: {:#?}", view.mentions))
}

/// Links the mention at `place` in `note` to `active`, as the command does:
/// a closed note's job runs without the session.
fn link(session: &mut Session, active: &str, note: &str, place: LinkPlace) -> String {
    match session
        .link_mention_plan(&id(active), &id(note), place)
        .unwrap()
    {
        MentionPlan::Done => session.status().to_owned(),
        MentionPlan::Closed(job) => {
            let done = job.run();
            session.link_mention_finish(done)
        }
    }
}

/// TC-004-42 step 3, A22: a mention in a closed note becomes a link to the
/// active note, written as link completion writes it, with the mention as
/// its text; a snapshot keeps the note as it was; the mention leaves the
/// list and a backlink comes.
#[test]
fn tc_004_42_step_3_link_a_mention_in_a_closed_note() {
    let scratch = Scratch::new("a22-closed");
    let mut session = session_with(
        &scratch,
        &[
            ("projects/Road-map.md", "# Road map\n"),
            ("Ideas.md", "# Ideas\n\nSee the road map first.\r\n"),
            ("travel/Trip.adoc", "= Trip\n\nThe ROAD MAP.\n"),
        ],
        "projects/Road-map",
    );
    let view = links(&mut session);
    let place = mention_in(&view, "Ideas").place;

    let status = link(&mut session, "projects/Road-map", "Ideas", place);

    assert_eq!(
        status,
        "Linked the mention of Road map in Ideas. The earlier version is in its snapshots."
    );
    let root = scratch.root();
    assert_eq!(
        fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "# Ideas\n\nSee the [road map](projects/Road-map.md) first.\r\n"
    );
    let snapshots = root.join(".underline/snapshots/Ideas.md");
    let kept: Vec<_> = fs::read_dir(&snapshots).unwrap().flatten().collect();
    assert_eq!(kept.len(), 1, "one snapshot");
    assert_eq!(
        fs::read_to_string(kept[0].path()).unwrap(),
        "# Ideas\n\nSee the road map first.\r\n"
    );
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.backlink_count == 1
    });
    assert_eq!(view.mentions.count, 1);

    // AsciiDoc, from a subfolder.
    let place = mention_in(&view, "Trip").place;
    link(&mut session, "projects/Road-map", "travel/Trip", place);
    assert_eq!(
        fs::read_to_string(root.join("travel/Trip.adoc")).unwrap(),
        "= Trip\n\nThe link:../projects/Road-map.md[ROAD MAP].\n"
    );
}

/// UC-004 A22, ADR-005 (decision of 2026-11-16): a closed note a mention
/// is linked in keeps who may read and write it.
#[cfg(unix)]
#[test]
fn uc_004_a22_adr005_linked_note_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("a22-mode");
    let mut session = session_with(
        &scratch,
        &[
            ("Road-map.md", "# Road map\n"),
            ("Ideas.md", "# Ideas\n\nSee the road map first.\n"),
        ],
        "Road-map",
    );
    let path = scratch.root().join("Ideas.md");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let view = links(&mut session);
    let place = mention_in(&view, "Ideas").place;

    link(&mut session, "Road-map", "Ideas", place);

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# Ideas\n\nSee the [road map](Road-map.md) first.\n"
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

/// TC-004-42 step 3, A22, ADR-025: a mention in a note open in a tab is
/// linked in the tab, as one undo step, not stored at once; one undo takes
/// it back.
#[test]
fn tc_004_42_step_3_link_a_mention_in_an_open_note() {
    let scratch = Scratch::new("a22-open");
    let ideas = "# Ideas\n\nÜber den Plan [v2] und plan [V2].\n";
    let mut session = session_with(
        &scratch,
        &[("Plan-[v2].md", "# Plan [v2]\n"), ("Ideas.md", ideas)],
        "Ideas",
    );
    assert!(session.open_note(&id("Plan-[v2]")));
    let view = links_until(&mut session, Duration::from_secs(5), |view| {
        view.note == "Plan-[v2]"
    });
    assert_eq!(view.mentions.count, 2);
    let place = mention_in(&view, "Ideas").place;

    let status = link(&mut session, "Plan-[v2]", "Ideas", place);

    assert!(
        status.starts_with("Linked the mention of Plan [v2] in Ideas. Undo with "),
        "{status}"
    );
    assert!(status.ends_with("+Z in Ideas."), "{status}");
    let linked = "# Ideas\n\nÜber den [Plan \\[v2\\]](Plan-%5Bv2%5D.md) und plan [V2].\n";
    let open = session.open.as_mut().unwrap();
    let tab = open.tabs.get_mut(open.tabs.find(&id("Ideas")).unwrap());
    assert_eq!(tab.mirror.text(), linked);
    assert_eq!(tab.edit.editor.text(), linked);
    assert_eq!(
        fs::read_to_string(scratch.root().join("Ideas.md")).unwrap(),
        ideas,
        "stored by the autosave, not at once"
    );
    tab.edit.editor.undo();
    assert_eq!(tab.edit.editor.text(), ideas, "one undo step");
}

/// A22: a mention that moved since the list was made, or whose text
/// changed, is not linked; the status says so and nothing changes.
#[test]
fn uc_004_a22_a_moved_mention_is_not_linked() {
    let scratch = Scratch::new("a22-moved");
    let mut session = session_with(
        &scratch,
        &[("Plan.md", "# Plan\n"), ("Log.md", "the plan\n")],
        "Plan",
    );
    let view = links(&mut session);
    let place = mention_in(&view, "Log").place;
    let root = scratch.root();
    write(&root, "Log.md", "now the plan\n");

    let status = link(&mut session, "Plan", "Log", place);

    assert_eq!(
        status,
        "Log changed since the mentions were listed; the mention is no longer at its place"
    );
    assert_eq!(
        fs::read_to_string(root.join("Log.md")).unwrap(),
        "now the plan\n"
    );
    assert!(!root.join(".underline/snapshots/Log.md").exists());

    // The same place, now in a link: not linked again.
    write(&root, "Log.md", "the [plan](Plan.md)\n");
    let wrong = LinkPlace {
        from: 4,
        to: 8,
        check: LinkPlace::check_of("plan"),
    };
    link(&mut session, "Plan", "Log", wrong);
    assert_eq!(
        fs::read_to_string(root.join("Log.md")).unwrap(),
        "the [plan](Plan.md)\n"
    );
    // Another text at the place (the check differs).
    write(&root, "Log.md", "the PLAN\n");
    link(&mut session, "Plan", "Log", place);
    assert_eq!(
        fs::read_to_string(root.join("Log.md")).unwrap(),
        "the PLAN\n"
    );
}

/// A22 (chaos): places outside the text or inside a character, and notes
/// that are not in the workspace, change nothing.
#[test]
fn uc_004_a22_chaos_places_and_notes() {
    let scratch = Scratch::new("a22-chaos");
    let mut session = session_with(
        &scratch,
        &[("Plan.md", "# Plan\n"), ("Ünï.md", "ü plan\n")],
        "Plan",
    );
    for (from, to) in [
        (0, 1),
        (1, 5),
        (2, 6),
        (100, 104),
        (7, 3),
        (u32::MAX, u32::MAX),
    ] {
        let place = LinkPlace {
            from,
            to,
            check: LinkPlace::check_of("plan"),
        };
        link(&mut session, "Plan", "Ünï", place);
        assert_eq!(
            fs::read_to_string(scratch.root().join("Ünï.md")).unwrap(),
            "ü plan\n",
            "{from}..{to}"
        );
    }
    let place = LinkPlace {
        from: 3,
        to: 7,
        check: LinkPlace::check_of("plan"),
    };
    assert!(session
        .link_mention_plan(&id("Plan"), &id("Nowhere"), place)
        .is_err());
    assert!(session
        .link_mention_plan(&id("../Plan"), &id("Ünï"), place)
        .is_err());
    link(&mut session, "Plan", "Ünï", place);
    assert_eq!(
        fs::read_to_string(scratch.root().join("Ünï.md")).unwrap(),
        "ü [plan](Plan.md)\n"
    );
}

/// A22, ADR-025: a note opened in a tab while its job wrote it gets the
/// written text in its tab, so the tab does not hold the text from before.
#[test]
fn uc_004_a22_a_tab_opened_meanwhile_gets_the_link() {
    let scratch = Scratch::new("a22-meanwhile");
    let mut session = session_with(
        &scratch,
        &[("Plan.md", "# Plan\n"), ("Log.md", "the plan\n")],
        "Plan",
    );
    let view = links(&mut session);
    let place = mention_in(&view, "Log").place;
    let MentionPlan::Closed(job) = session
        .link_mention_plan(&id("Plan"), &id("Log"), place)
        .unwrap()
    else {
        panic!("a closed note");
    };
    let done = job.run();
    assert!(session.open_note(&id("Log")));
    let open = session.open.as_mut().unwrap();
    let tab = open.tabs.get_mut(open.tabs.find(&id("Log")).unwrap());
    // Opened from the file before it was written.
    tab.reload("the plan\n".to_owned());
    tab.on_disk = "the plan\n".to_owned();

    session.link_mention_finish(done);

    let open = session.open.as_mut().unwrap();
    let tab = open.tabs.get(open.tabs.find(&id("Log")).unwrap());
    assert_eq!(tab.mirror.text(), "the [plan](Plan.md)\n");
}

/// Links the first mention of `active`'s title in `Log.md` (`log`), and
/// returns the note as it is then and the lists once they follow.
fn link_first_and_look(
    name: &str,
    active: &str,
    log: &str,
) -> (String, crate::links_view::LinksView) {
    let scratch = Scratch::new(name);
    let mut session = session_with(
        &scratch,
        &[(&format!("{active}.md"), "# Active\n"), ("Log.md", log)],
        active,
    );
    let view = links(&mut session);
    let place = mention_in(&view, "Log").place;
    link(&mut session, active, "Log", place);
    let after = fs::read_to_string(scratch.root().join("Log.md")).unwrap();
    let mut view = links(&mut session);
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(10));
        for event in session.tick(std::time::Instant::now()) {
            if let crate::Event::Links(latest) = event {
                view = latest;
            }
        }
    }
    (after, view)
}

/// UC-004 A22, BR-021 (monkey): two "Link" actions on different mentions
/// of one closed note, started together (two quick clicks, each command
/// runs on its own thread), must not lose one: whenever both say they
/// linked, both links stand in the file. Today both jobs read the file
/// before either wrote it, and the later write drops the earlier link
/// while the status still says "Linked".
#[test]
fn uc_004_a22_monkey_two_mentions_of_a_closed_note_linked_at_once_both_stay() {
    for round in 0..20 {
        let scratch = Scratch::new(&format!("monkey12-par{round}"));
        let mut session = session_with(
            &scratch,
            &[
                ("Plan.md", "# Plan\n"),
                ("Log.md", "the plan and also the plan\n"),
            ],
            "Plan",
        );
        let view = links_until(&mut session, Duration::from_secs(5), |view| {
            view.mentions.count == 2
        });
        let note = view
            .mentions
            .notes
            .iter()
            .find(|n| n.title == "Log")
            .unwrap();
        let places = [note.mentions[0].place, note.mentions[1].place];
        let jobs: Vec<_> = places
            .into_iter()
            .map(|place| {
                match session
                    .link_mention_plan(&id("Plan"), &id("Log"), place)
                    .unwrap()
                {
                    MentionPlan::Closed(job) => job,
                    MentionPlan::Done => panic!("a closed note"),
                }
            })
            .collect();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = jobs
            .into_iter()
            .map(|job| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    job.run()
                })
            })
            .collect();
        let linked = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|done| matches!(done.outcome, Ok(Some(_))))
            .count();
        let text = fs::read_to_string(scratch.root().join("Log.md")).unwrap();
        assert_eq!(
            text.matches("](Plan.md)").count(),
            linked,
            "round {round}: {linked} jobs said linked, the file holds: {text:?}"
        );
    }
}

/// UC-004 A22, UC-005 BR-018 (monkey): a mention right after `!` is linked
/// as a link, not turned into a picture (`![Plan](Plan.md)`).
#[test]
fn uc_004_a22_monkey_mention_after_a_bang_becomes_a_link_not_a_picture() {
    let (after, view) = link_first_and_look("monkey12-bang", "Plan", "Wow!Plan next\n");
    let forms: Vec<_> = view
        .backlinks
        .iter()
        .flat_map(|note| note.links.iter().map(|link| link.form))
        .collect();
    assert_eq!(forms, [crate::links_view::LinkFormName::Link], "{after:?}");
}

/// UC-004 A22, UC-005 BR-018 (monkey): a mention right after a backslash
/// (`C:\Plan`) is linked as a link; `\[` would make the bracket plain text.
#[test]
fn uc_004_a22_monkey_mention_after_a_backslash_becomes_a_link() {
    let (after, view) = link_first_and_look("monkey12-slash", "Plan", "open C:\\Plan next\n");
    assert_eq!(view.backlink_count, 1, "{after:?}");
}

/// UC-004 A22, UC-005 BR-018, OQ-248 (monkey): the link to a note whose
/// title starts with a scheme-like word and a colon (`Tel: Alice`) leads
/// back to the note; `Tel:-Alice.md` is read as an address of another
/// program.
#[test]
fn uc_004_a22_monkey_link_to_a_note_titled_like_a_scheme_is_a_backlink() {
    let (after, view) = link_first_and_look("monkey12-tel", "Tel:-Alice", "call Tel: Alice now\n");
    assert_eq!(view.backlink_count, 1, "{after:?}");
}

/// UC-004 A22, OQ-248 (monkey): a single backtick in the title would pair
/// with the one in the place and swallow the link; the written link is a
/// backlink.
#[test]
fn uc_004_a22_monkey_link_to_a_note_with_a_backtick_in_its_title_is_a_backlink() {
    let (after, view) = link_first_and_look("monkey12-tick", "Plan`s", "the Plan`s now\n");
    assert_eq!(view.backlink_count, 1, "{after:?}");
}

/// Opens `Log` in a tab holding `typed`, as if it had been opened from the
/// file before the job wrote it and the user typed meanwhile; then the job
/// finishes. The status and the tab's text.
fn typed_meanwhile(name: &str, typed: &str) -> (String, String) {
    let scratch = Scratch::new(name);
    let mut session = session_with(
        &scratch,
        &[("Plan.md", "# Plan\n"), ("Log.md", "the plan\n")],
        "Plan",
    );
    let view = links(&mut session);
    let place = mention_in(&view, "Log").place;
    let MentionPlan::Closed(job) = session
        .link_mention_plan(&id("Plan"), &id("Log"), place)
        .unwrap()
    else {
        panic!("a closed note");
    };
    let done = job.run();
    assert!(session.open_note(&id("Log")));
    {
        let open = session.open.as_mut().unwrap();
        let tab = open.tabs.get_mut(open.tabs.find(&id("Log")).unwrap());
        tab.reload("the plan\n".to_owned());
    }
    session.editor_resync(&id("Log"), typed).unwrap();
    let status = session.link_mention_finish(done);
    let open = session.open.as_mut().unwrap();
    let tab = open.tabs.get(open.tabs.find(&id("Log")).unwrap());
    (status, tab.mirror.text())
}

/// A22, ADR-025 (review): typing in a tab opened while the job wrote
/// keeps the link: it is made in the tab too, after the typed text.
#[test]
fn uc_004_a22_typing_in_a_tab_opened_meanwhile_keeps_the_link() {
    let (_, text) = typed_meanwhile("a22-typed", "the plan, typed\n");
    assert_eq!(text, "the [plan](Plan.md), typed\n");
}

/// A22 (review): when the typing moved the mention, the status says the
/// link could not be kept in the tab; nothing is lost silently.
#[test]
fn uc_004_a22_a_moved_mention_in_a_tab_opened_meanwhile_is_said() {
    let (status, text) = typed_meanwhile("a22-typed-moved", "now the plan\n");
    assert_eq!(text, "now the plan\n");
    assert!(status.contains("could not be kept in its tab"), "{status}");
}
