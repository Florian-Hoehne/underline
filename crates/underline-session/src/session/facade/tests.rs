//! Tests of the facade in the session (M-09, ADR-024): the conversion of
//! the blocks for the window with their offsets (CRLF, emoji), the
//! changes sent after an edit (`ed-blocks`), the figures (M-09.5 to
//! M-09.7), the switch of the editing view (M-07.2) and what the own
//! protocol serves (ADR-023).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteFormat, NoteId, StateStore, SETTINGS_FILE_NAME};

use super::build::{run, Job};
use super::{splices, views_of, Facade, Splice};
use crate::assets::{Asset, AssetError};
use crate::editor_view::EditorInput;
use crate::facade_view::{BlockKindView, BlockView, CellAlignName};
use crate::workspace_view::EditingViewName;
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-facade-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on a workspace `dir/notes` whose only note `Home.md` holds
/// `home`, opened in a tab.
fn session_with(dir: &Path, home: &str) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Home.md"), home).unwrap();
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
    session.take_events();
    (session, root)
}

fn home() -> NoteId {
    NoteId::from_relative("Home")
}

/// The UTF-16 units of `text` with its line breaks as the window has them.
fn web_units(text: &str) -> Vec<u16> {
    underline_editor_core::utf16::normalize_breaks(text)
        .encode_utf16()
        .collect()
}

/// The web offset of the markup that unit `unit` of `run` in `block` shows.
fn source_of(block: &BlockView, run: &crate::RunView, unit: u32) -> usize {
    let (from, at) = run
        .offsets
        .iter()
        .rev()
        .find(|(u, _)| *u <= unit)
        .copied()
        .unwrap();
    (block.start + at + unit - from) as usize
}

/// Every shown character of `views` stands at its markup character in the
/// window's text (a space may stand for a line break).
fn assert_offsets_fit(text: &str, views: &[BlockView]) {
    let web = web_units(text);
    for block in views {
        for run in &block.runs {
            let shown: Vec<u16> = run.text.encode_utf16().collect();
            for (unit, &c) in shown.iter().enumerate() {
                let source = source_of(block, run, u32::try_from(unit).unwrap());
                let fits = web[source] == c || (c == u16::from(b' ') && web[source] == 0x0a);
                assert!(
                    fits,
                    "{:?} {:?}: unit {unit} {c:#x} maps to {:#x}",
                    block.kind, run.text, web[source]
                );
            }
            assert!((block.start + run.end) as usize <= web.len());
        }
        assert!((block.start + block.length) as usize <= web.len());
    }
}

const NOTE: &str = "# Hi 🙂 *there*\n\nSome **bold \\* x** and [a link](http://x) ~~gone~~ 🙂.\nnext `code` line\n\n- one\n- [x] two\n  1. inner\n\n> quoted\n\n```rust\nfn a() {} // 🙂\n```\n\n| a | b |\n|:--|--:|\n| **1** | 2 |\n\n---\n\nEnd 日本\n";

#[test]
fn m09_1_block_views_carry_kind_prefix_and_runs() {
    let views = views_of(NOTE, NoteFormat::Markdown);

    let kinds: Vec<String> = views
        .iter()
        .map(|b| match &b.kind {
            BlockKindView::Table { .. } => "Table".to_owned(),
            kind => format!("{kind:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Heading { level: 1 }",
            "Paragraph",
            "ListItem { depth: 0 }",
            "ListItem { depth: 0 }",
            "ListItem { depth: 1 }",
            "Quote { depth: 1 }",
            "Code { language: \"rust\" }",
            "Table",
            "ThematicBreak",
            "Paragraph",
        ]
    );
    let prefixes: Vec<&str> = views.iter().map(|b| b.prefix.as_str()).collect();
    assert_eq!(
        prefixes,
        ["", "", "•  ", "•  ☑ ", "    1.  ", "    ", "", "", "", ""]
    );
    let heading = &views[0];
    assert_eq!(heading.runs[1].text, "there");
    assert!(heading.runs[1].italic);
    let paragraph = &views[1];
    let link = paragraph.runs.iter().find(|r| r.link.is_some()).unwrap();
    assert_eq!(
        (link.text.as_str(), link.link.as_deref()),
        ("a link", Some("http://x"))
    );
    assert!(paragraph.runs.iter().any(|r| r.strike && r.text == "gone"));
    assert!(paragraph.runs.iter().any(|r| r.code && r.text == "code"));
    assert!(paragraph
        .runs
        .iter()
        .any(|r| r.bold && r.text.contains('*')));
    let BlockKindView::Table { table } = &views[7].kind else {
        panic!("{:?}", views[7].kind);
    };
    assert!(table.header);
    assert_eq!(table.align, [CellAlignName::Left, CellAlignName::Right]);
    assert!(table.rows[1][0][0].bold);
    assert!(views[7].runs[0].text.starts_with("| a | b |"));
    // Lines for folding and for the height before drawing.
    assert_eq!((views[0].line, views[0].lines), (0, 1));
    assert_eq!((views[1].line, views[1].lines), (2, 2));
    assert_eq!(views[6].lines, 3);
}

/// ADR-021, plan section 11: every shown character maps to the markup
/// character it stands for, in the window's units: an emoji is two units, a
/// `\r\n` pair one.
#[test]
fn m09_1_offsets_are_web_units_with_emoji_and_crlf() {
    let lf = views_of(NOTE, NoteFormat::Markdown);
    assert_offsets_fit(NOTE, &lf);
    // The escaped star stands at the star, not at its backslash.
    let web = web_units(NOTE);
    let bold = lf[1].runs.iter().find(|r| r.bold).unwrap();
    let star = bold
        .text
        .encode_utf16()
        .position(|c| c == u16::from(b'*'))
        .unwrap();
    let star_at = source_of(&lf[1], bold, u32::try_from(star).unwrap());
    assert_eq!(web[star_at], u16::from(b'*'));
    assert_eq!(web[star_at - 1], u16::from(b'\\'));

    let crlf_note = NOTE.replace('\n', "\r\n");
    let crlf = views_of(&crlf_note, NoteFormat::Markdown);
    assert_offsets_fit(&crlf_note, &crlf);
    // With its line breaks normalised the window sees the same blocks.
    assert_eq!(crlf.len(), lf.len());
    for (a, b) in crlf.iter().zip(&lf) {
        assert_eq!(a.start, b.start, "{:?}", a.kind);
        assert_eq!(a.line, b.line, "{:?}", a.kind);
        assert_eq!(a.prefix, b.prefix);
        let text = |v: &BlockView| v.runs.iter().map(|r| r.text.clone()).collect::<String>();
        assert_eq!(text(a), text(b), "{:?}", a.kind);
        assert!(!text(a).contains('\r'));
    }
}

/// M-09.2: AsciiDoc blocks get their prefixes too: an admonition its label.
#[test]
fn m09_2_asciidoc_admonitions_and_lists() {
    let note = "= Title\n\nNOTE: Mind this.\n\n* one\n** two\n\n. first\n";
    let views = views_of(note, NoteFormat::AsciiDoc);

    assert_offsets_fit(note, &views);
    let admonition = views
        .iter()
        .find(|v| matches!(v.kind, BlockKindView::Admonition { .. }))
        .unwrap();
    assert_eq!(admonition.prefix, "    NOTE  ");
    assert!(views.iter().any(|v| v.prefix == "    •  "));
    assert!(views.iter().any(|v| v.prefix == "1.  "));
}

fn edited(text: &str, at: &str, insert: &str) -> String {
    let at = text.find(at).unwrap();
    let mut out = text.to_owned();
    out.insert_str(at, insert);
    out
}

/// ADR-024: an edit sends the changed blocks only; the blocks after them
/// move by one shift.
#[test]
fn m09_1_an_edit_changes_only_its_block_and_moves_the_rest() {
    let old = views_of(NOTE, NoteFormat::Markdown);
    let new = views_of(&edited(NOTE, "quoted", "🙂"), NoteFormat::Markdown);

    let changes = splices(&old, &new);

    assert_eq!(changes.len(), 1);
    let Splice {
        from,
        removed,
        blocks,
        shift,
        line_shift,
    } = &changes[0];
    assert_eq!((*from, *removed, blocks.len()), (5, 1, 1));
    assert_eq!((*shift, *line_shift), (2, 0));
    // Applied as the window applies it, the result is the new blocks.
    let mut applied = old.clone();
    applied.splice(from..&(from + removed), blocks.iter().cloned());
    for block in &mut applied[from + blocks.len()..] {
        block.start = u32::try_from(i64::from(block.start) + shift).unwrap();
    }
    assert_eq!(applied, new);

    // A new line in a paragraph moves the lines too.
    let lines = views_of(&edited(NOTE, "next ", "x\n"), NoteFormat::Markdown);
    let changes = splices(&old, &lines);
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].shift, changes[0].line_shift), (2, 1));
    assert!(splices(&old, &old).is_empty());
}

/// M-09.7: blocks that changed in place (pictures ready) are sent one run
/// at a time, not with the blocks in between.
#[test]
fn m09_7_changes_in_place_are_sent_separately() {
    let old = views_of(NOTE, NoteFormat::Markdown);
    let mut new = old.clone();
    new[1].prefix = "x".into();
    new[8].prefix = "y".into();

    let changes = splices(&old, &new);

    let ranges: Vec<(usize, usize)> = changes.iter().map(|s| (s.from, s.removed)).collect();
    assert_eq!(ranges, [(1, 1), (8, 1)]);
}

const FIGURES: &str = "# Figures\n\n![The plan](img/plan.png)\n\n![Gone](img/gone.png)\n\n![Out](../out.png)\n\n![Web](https://example.com/a.png)\n\n```mermaid\ngraph TD\nA-->B\n```\n\n| a |\n|---|\n| 1 |\n";

fn figures_session(name: &str) -> (Session, PathBuf, PathBuf) {
    let dir = temp_dir(name);
    let (session, root) = session_with(&dir, FIGURES);
    fs::create_dir_all(root.join("img")).unwrap();
    fs::write(root.join("img/plan.png"), b"\x89PNG").unwrap();
    fs::write(dir.join("out.png"), b"\x89PNG").unwrap();
    (session, root, dir)
}

/// Ticks until the facade's jobs are done; the blocks events meanwhile.
fn settle(session: &mut Session) -> Vec<crate::BlocksView> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut out = Vec::new();
    loop {
        let events = session.tick(Instant::now());
        out.extend(blocks_events(&events).into_iter().cloned());
        let building = session.open.as_ref().is_some_and(|open| {
            open.tabs
                .iter()
                .any(|(_, tab)| tab.facade.as_ref().is_some_and(|f| f.building))
        });
        if !building {
            return out;
        }
        assert!(Instant::now() < deadline, "the facade's job did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn blocks_events(events: &[Event]) -> Vec<&crate::BlocksView> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Blocks(view) => Some(view),
            _ => None,
        })
        .collect()
}

/// M-09.6, M-09.7, ADR-023: the figures say where their pictures are, or
/// why they cannot be shown, in egui's words.
#[test]
fn m09_6_m09_7_figures_of_the_facade() {
    let (mut session, root, _dir) = figures_session("figures");

    let state = session.facade_state(&home()).unwrap();

    assert_eq!(state.format, "Markdown");
    let figure = |i: usize| state.blocks[i].figure.clone().unwrap();
    let plan = figure(1);
    let picture = plan.picture.unwrap();
    assert!(picture.starts_with("file/img/plan.png?v="), "{picture}");
    assert_eq!(plan.loading.as_deref(), Some("Loading img/plan.png…"));
    assert_eq!(
        figure(2).message.as_deref(),
        Some("The image Gone cannot be shown: img/gone.png was not found")
    );
    assert_eq!(
        figure(3).message.as_deref(),
        Some("../out.png lies outside the workspace")
    );
    assert_eq!(
        figure(4).message.as_deref(),
        Some("The image Web cannot be shown: https://example.com/a.png could not be loaded")
    );
    assert_eq!(
        figure(5).message.as_deref(),
        Some("Rendering the Mermaid diagram…")
    );
    assert!(matches!(state.blocks[6].kind, BlockKindView::Table { .. }));

    // The protocol serves the image below the root, and nothing else.
    let path = format!("/{picture}");
    match session.asset(&path) {
        Ok(Asset::File { path, mime }) => {
            assert_eq!(path, root.canonicalize().unwrap().join("img/plan.png"));
            assert_eq!(mime, "image/png");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        session.asset("/file/..%2Fout.png"),
        Err(AssetError::Refused)
    );
    assert_eq!(session.asset("/file/Home.md"), Err(AssetError::Refused));
    assert_eq!(session.asset("/elsewhere/x.png"), Err(AssetError::Refused));
    assert_eq!(
        session.asset("/diagram/0000000000000001.svg"),
        Err(AssetError::NotFound)
    );
}

/// M-09.7, ADR-009: the diagram renders in the background; the tick sends
/// its block with the picture, which the protocol serves as SVG.
#[test]
fn m09_7_a_rendered_diagram_is_sent_and_served() {
    let (mut session, _root, _dir) = figures_session("diagram");
    session.facade_state(&home()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(60);
    let picture = loop {
        let events = session.tick(Instant::now());
        let found = blocks_events(&events).into_iter().find_map(|b| {
            b.blocks
                .iter()
                .find_map(|block| block.figure.as_ref()?.picture.clone())
                .filter(|p| p.starts_with("diagram/"))
                .map(|p| (b.from, b.removed, p))
        });
        if let Some(found) = found {
            break found;
        }
        assert!(Instant::now() < deadline, "the diagram did not render");
        std::thread::sleep(Duration::from_millis(20));
    };

    assert_eq!((picture.0, picture.1), (5, 1));
    let Ok(Asset::Svg(svg)) = session.asset(&format!("/{}", picture.2)) else {
        panic!("the SVG is not served");
    };
    assert!(svg.contains("<svg"), "{svg}");
}

/// ADR-024: an input sends the changed blocks with the next tick, in a
/// chain of versions.
#[test]
fn m09_1_the_tick_sends_the_blocks_an_input_changed() {
    let dir = temp_dir("input");
    let (mut session, _root) = session_with(&dir, "# Home\n\nOne\n\nTwo\n");
    let state = session.facade_state(&home()).unwrap();
    assert_eq!(state.blocks.len(), 3);
    session.editor_select(&home(), state.rev, 11, 11).unwrap();

    session
        .editor_input(&home(), EditorInput::Text { text: "!".into() })
        .unwrap();
    // ADR-024: the tick only starts the job; a later tick takes it in.
    let first = session.tick(Instant::now());
    assert!(blocks_events(&first).is_empty());
    let blocks = settle(&mut session);

    assert_eq!(blocks.len(), 1);
    let change = &blocks[0];
    assert_eq!(change.base, state.version);
    assert_eq!(change.version, state.version + 1);
    assert_eq!((change.from, change.removed, change.shift), (1, 1, 1));
    assert_eq!(change.blocks[0].runs[0].text, "One!");
    assert!(settle(&mut session).is_empty());
}

/// RUST 5: a job's result for a text that changed meanwhile is dropped;
/// the next job brings the text as it is.
#[test]
fn adr024_a_stale_result_is_dropped() {
    let dir = temp_dir("stale");
    let (mut session, _root) = session_with(&dir, "# Home\n\nOne\n");
    let state = session.facade_state(&home()).unwrap();
    session.editor_select(&home(), state.rev, 11, 11).unwrap();
    session
        .editor_input(&home(), EditorInput::Text { text: "a".into() })
        .unwrap();
    session.tick(Instant::now());
    // Typed while the job runs: its result is for an older text.
    session
        .editor_input(&home(), EditorInput::Text { text: "b".into() })
        .unwrap();

    let blocks = settle(&mut session);
    let blocks = [blocks, settle(&mut session)].concat();

    let last = blocks.last().unwrap();
    assert_eq!(last.blocks[0].runs[0].text, "Oneab");
    // Every change applies to the one before it.
    for pair in blocks.windows(2) {
        assert_eq!(pair[1].base, pair[0].version);
    }
}

/// RUST 9: `fa_open` racing a tick: whichever result comes last, the
/// later job's wins, and the window's chain of versions holds.
#[test]
fn adr024_fa_open_racing_a_tick() {
    let dir = temp_dir("race");
    let (mut session, _root) = session_with(&dir, "# Home\n\nOne\n");
    let state = session.facade_state(&home()).unwrap();
    session.editor_select(&home(), state.rev, 11, 11).unwrap();
    session
        .editor_input(&home(), EditorInput::Text { text: "!".into() })
        .unwrap();
    // The tick's job runs while the window opens the facade again.
    session.tick(Instant::now());
    let opened = session.facade_state(&home()).unwrap();
    assert_eq!(opened.blocks[1].runs[0].text, "One!");

    let after = settle(&mut session);

    // A result older than the one `fa_open` took in brings nothing.
    assert!(after.iter().all(|b| b.base >= opened.version), "{after:?}");
    let again = session.facade_state(&home()).unwrap();
    assert_eq!(again.blocks, opened.blocks);
}

/// RUST 9: results are taken in in the order their jobs started.
#[test]
fn adr024_an_older_job_never_overwrites_a_newer_one() {
    let job = |seq: u64, text: &str| Job {
        seq,
        rev: seq,
        generation: 0,
        rope: ropey::Rope::from_str(text),
        format: NoteFormat::Markdown,
        blocks: None,
        last_pictures: std::collections::HashMap::new(),
        ctx: super::build::Context {
            looks: (underline_core::Theme::Light, false),
            colors: None,
            dark: false,
            diagrams: None,
            code_colors: std::sync::Arc::default(),
            images: std::sync::Arc::default(),
            root: PathBuf::from("/nowhere"),
            note: PathBuf::from("/nowhere/Home.md"),
        },
    };
    let mut facade = Facade::default();
    let older = run(job(1, "# Old\n"));
    let newer = run(job(2, "# New\n"));

    assert!(facade.commit(newer).is_some());
    assert_eq!(facade.commit(older), None);
    assert_eq!(facade.views[0].runs[0].text, "New");
    assert_eq!(facade.version, 1);
}

/// M-07.2, M-22.2: the switch remembers the view of the tab; the facade is
/// not kept current while the source view is shown, but its blocks stay.
#[test]
fn m07_2_the_editing_view_is_remembered_per_tab() {
    let dir = temp_dir("switch");
    let (mut session, _root) = session_with(&dir, "# Home\n");
    let first = session.facade_state(&home()).unwrap();

    session.set_editing_view(&home(), EditingViewName::Source);

    let events = session.take_events();
    let view = events.iter().find_map(|e| match e {
        Event::Tabs(tabs) => Some(tabs.tabs[0].view),
        _ => None,
    });
    assert_eq!(view, Some(EditingViewName::Source));
    assert!(blocks_events(&session.tick(Instant::now())).is_empty());
    session.set_editing_view(&home(), EditingViewName::Facade);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.tabs.tabs[0].view, EditingViewName::Facade);
    // The blocks were kept: shown again, they are not made anew.
    let again = session.facade_state(&home()).unwrap();
    assert_eq!(again.version, first.version);
    assert_eq!(again.blocks, first.blocks);
}

/// RUST 9: the edges of the changes: from and to no blocks, the last block
/// deleted, and a figure whose number changes with an earlier one.
#[test]
fn m09_1_changes_at_the_edges() {
    let some = views_of("# A\n\nB\n", NoteFormat::Markdown);
    let none: Vec<BlockView> = Vec::new();

    let from_none = splices(&none, &some);
    assert_eq!(from_none.len(), 1);
    assert_eq!(
        (
            from_none[0].from,
            from_none[0].removed,
            from_none[0].blocks.len()
        ),
        (0, 0, 2)
    );
    let to_none = splices(&some, &none);
    assert_eq!(
        (to_none[0].from, to_none[0].removed, to_none[0].blocks.len()),
        (0, 2, 0)
    );
    assert!(splices(&none, &none).is_empty());

    let shorter = views_of("# A\n", NoteFormat::Markdown);
    let cut = splices(&some, &shorter);
    assert_eq!(cut.len(), 1);
    assert_eq!(
        (cut[0].from, cut[0].removed, cut[0].blocks.len()),
        (1, 1, 0)
    );

    // AsciiDoc numbers figures with a title; one added before moves the number.
    let one = "= T\n\n.Second\nimage::b.png[B]\n";
    let two = "= T\n\n.First\nimage::a.png[A]\n\n.Second\nimage::b.png[B]\n";
    let old = views_of(one, NoteFormat::AsciiDoc);
    let new = views_of(two, NoteFormat::AsciiDoc);
    let caption = |v: &BlockView| v.figure.as_ref().and_then(|f| f.caption.clone());
    assert_eq!(
        caption(old.last().unwrap()).as_deref(),
        Some("Figure 1. Second")
    );
    assert_eq!(
        caption(new.last().unwrap()).as_deref(),
        Some("Figure 2. Second")
    );
    let changes = splices(&old, &new);
    let mut applied = old.clone();
    for change in &changes {
        applied.splice(
            change.from..change.from + change.removed,
            change.blocks.iter().cloned(),
        );
        for block in &mut applied[change.from + change.blocks.len()..] {
            block.start = u32::try_from(i64::from(block.start) + change.shift).unwrap();
            block.line = u32::try_from(i64::from(block.line) + change.line_shift).unwrap();
        }
    }
    assert_eq!(applied, new);
}

/// RUST 9, ADR-023: without a workspace nothing is served; after a switch,
/// only the files of the new one.
#[test]
fn adr023_assets_follow_the_open_workspace() {
    let dir = temp_dir("asset-switch");
    fs::create_dir_all(dir.join("home")).unwrap();
    let empty = Session::new(
        None,
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    assert_eq!(empty.asset("/file/a.png"), Err(AssetError::Refused));
    assert_eq!(empty.asset("/diagram/00.svg"), Err(AssetError::NotFound));

    let (mut session, root) = session_with(&dir, "# Home\n");
    fs::write(root.join("a.png"), b"png").unwrap();
    assert!(session.asset("/file/a.png").is_ok());
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Home.md"), "# Home\n").unwrap();
    fs::write(other.join("b.png"), b"png").unwrap();

    session.open_chosen(&other, false);

    assert_eq!(session.asset("/file/a.png"), Err(AssetError::NotFound));
    assert!(session.asset("/file/b.png").is_ok());
    assert_eq!(
        session.asset("/file/..%2Fnotes%2Fa.png"),
        Err(AssetError::Refused)
    );
}

/// RUST 8: a target that is no image, or lies in a hidden folder, says so.
#[test]
fn m09_6_targets_that_are_no_images() {
    let dir = temp_dir("not-image");
    let note = "![Doc](notes.txt)\n\n![Cache](.underline/c.png)\n";
    let (mut session, root) = session_with(&dir, note);
    fs::write(root.join("notes.txt"), "text").unwrap();
    fs::create_dir_all(root.join(".underline")).unwrap();
    fs::write(root.join(".underline/c.png"), b"png").unwrap();

    let state = session.facade_state(&home()).unwrap();

    let message = |i: usize| state.blocks[i].figure.as_ref().unwrap().message.clone();
    assert_eq!(
        message(0).as_deref(),
        Some("The image Doc cannot be shown: the image cannot be read: The image format could not be determined")
    );
    assert_eq!(
        message(1).as_deref(),
        Some(".underline/c.png lies outside the workspace")
    );
}
