//! Tests of the facade in the session (M-09, ADR-024, ADR-025): the
//! conversion of the blocks with their offsets (CRLF, emoji), the changes
//! of the views after an edit, the figures (M-09.5 to M-09.7) and their
//! colours and pictures for the window's editor core (`ed-enrich`), the
//! switch of the editing view (M-07.2) and what the own protocol serves
//! (ADR-023).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use underline_core::{NoteFormat, NoteId, StateStore, SETTINGS_FILE_NAME};

use super::build::{run, Job};
use super::{splices, views_of, Facade, Splice};
use crate::assets::{Asset, AssetError};
use crate::facade_view::{BlockKindView, BlockView, CellAlignName, FacadeState};
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

/// Ticks until the facade's jobs are done and its views are of the text;
/// the `ed-enrich` events meanwhile.
fn settle(session: &mut Session) -> Vec<crate::EnrichView> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut out = Vec::new();
    loop {
        for event in session.tick(Instant::now()) {
            if let Event::Enrich(view) = event {
                out.push(*view);
            }
        }
        let done = session.open.as_ref().is_some_and(|open| {
            open.tabs.iter().all(|(_, tab)| {
                tab.facade.as_ref().is_none_or(|f| {
                    !f.building && tab.edit.facade.parsed == Some(tab.edit.editor.revision())
                })
            })
        });
        if done {
            return out;
        }
        assert!(Instant::now() < deadline, "the facade's job did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The facade of Home as the session made it for the window's colours and
/// figures: shown, its job done (what `fa_open` gave in variant A).
fn facade_of(session: &mut Session) -> FacadeState {
    session
        .set_editing_view(&home(), EditingViewName::Facade)
        .unwrap();
    settle(session);
    let open = session.open.as_ref().unwrap();
    let tab = open.tabs.get(open.tabs.find(&home()).unwrap());
    tab.edit.facade_state("Home")
}

/// M-09.6, M-09.7, ADR-023: the figures say where their pictures are, or
/// why they cannot be shown, in egui's words.
#[test]
fn m09_6_m09_7_figures_of_the_facade() {
    let (mut session, root, _dir) = figures_session("figures");

    let state = facade_of(&mut session);

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

/// M-09.7, ADR-009, ADR-025: the diagram renders in the background; the
/// tick sends its picture (`ed-enrich`), which the protocol serves as SVG.
#[test]
fn m09_7_a_rendered_diagram_is_sent_and_served() {
    let (mut session, _root, _dir) = figures_session("diagram");
    facade_of(&mut session);

    let deadline = Instant::now() + Duration::from_secs(60);
    let picture = loop {
        let found = session
            .tick(Instant::now())
            .into_iter()
            .find_map(|e| match e {
                Event::Enrich(view) => view
                    .diagrams
                    .iter()
                    .find_map(|(_, figure)| figure.picture.clone())
                    .filter(|p| p.starts_with("diagram/")),
                _ => None,
            });
        if let Some(found) = found {
            break found;
        }
        assert!(Instant::now() < deadline, "the diagram did not render");
        std::thread::sleep(Duration::from_millis(20));
    };

    let Ok(Asset::Svg(svg)) = session.asset(&format!("/{picture}")) else {
        panic!("the SVG is not served");
    };
    assert!(svg.contains("<svg"), "{svg}");
}

/// ADR-024, ADR-025: the views follow a window's edit with the next jobs,
/// as one change of the chain of versions that `ed-enrich` follows.
#[test]
fn m09_1_the_views_follow_a_windows_edit() {
    let dir = temp_dir("input");
    let (mut session, _root) = session_with(&dir, "# Home\n\nOne\n\nTwo\n");
    let state = facade_of(&mut session);
    assert_eq!(state.blocks.len(), 3);

    session.window_type(&home(), 11, "!");
    let after = facade_of(&mut session);

    assert_eq!(after.version, state.version + 1);
    assert_eq!(after.blocks[1].runs[0].text, "One!");
    assert_eq!(after.blocks[2].start, state.blocks[2].start + 1);
}

/// RUST 5: a job's result for a text that changed meanwhile is dropped;
/// the next job brings the text as it is.
#[test]
fn adr024_a_stale_result_is_dropped() {
    let dir = temp_dir("stale");
    let (mut session, _root) = session_with(&dir, "# Home\n\nOne\n");
    facade_of(&mut session);
    session.window_type(&home(), 11, "a");
    session.tick(Instant::now());
    // Typed while the job runs: its result is for an older text.
    session.window_type(&home(), 12, "b");

    let after = facade_of(&mut session);

    assert_eq!(after.blocks[1].runs[0].text, "Oneab");
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
    let mut model = underline_editor_view::FacadeModel::default();
    let older = run(job(1, "# Old\n"));
    let newer = run(job(2, "# New\n"));

    assert!(facade.commit(&mut model, newer).is_some());
    assert_eq!(facade.commit(&mut model, older), None);
    assert_eq!(model.views[0].runs[0].text, "New");
    assert_eq!(model.version, 1);
}

/// M-07.2, M-22.2: the switch remembers the view of the tab; the facade is
/// not kept current while the source view is shown, but its blocks stay.
#[test]
fn m07_2_the_editing_view_is_remembered_per_tab() {
    let dir = temp_dir("switch");
    let (mut session, _root) = session_with(&dir, "# Home\n");
    let first = facade_of(&mut session);

    session
        .set_editing_view(&home(), EditingViewName::Source)
        .unwrap();

    let events = session.take_events();
    let view = events.iter().find_map(|e| match e {
        Event::Tabs(tabs) => Some(tabs.tabs[0].view),
        _ => None,
    });
    assert_eq!(view, Some(EditingViewName::Source));
    session
        .set_editing_view(&home(), EditingViewName::Facade)
        .unwrap();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.tabs.tabs[0].view, EditingViewName::Facade);
    // The blocks were kept: shown again, they are not made anew.
    let again = facade_of(&mut session);
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

    let state = facade_of(&mut session);

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

/// The backend's views of `text` in `format`, with the colours of code of
/// `theme` (dark or not), in the workspace `root`.
fn backend_views(
    root: &Path,
    text: &str,
    format: NoteFormat,
    (theme, dark): (underline_core::Theme, bool),
) -> (Vec<underline_facade::blocks::Block>, Vec<BlockView>) {
    let built = run(Job {
        seq: 1,
        rev: 1,
        generation: 0,
        rope: ropey::Rope::from_str(text),
        format,
        blocks: None,
        last_pictures: std::collections::HashMap::new(),
        ctx: super::build::Context {
            looks: (theme, true),
            colors: Some(std::sync::Arc::new(
                underline_facade::highlight::HighlightSet::for_theme(theme),
            )),
            dark,
            diagrams: None,
            code_colors: std::sync::Arc::default(),
            images: std::sync::Arc::default(),
            root: root.to_owned(),
            note: root.join("Home.md"),
        },
    });
    (built.blocks.to_vec(), built.views)
}

/// What the backend sends the webview of `backend`, the views of `blocks`
/// in `theme` (dark or not): the colours of code, the figures of diagrams
/// and images, by their keys; kept in views for the theme named `shown`.
fn cache_of(
    blocks: &[underline_facade::blocks::Block],
    backend: &[BlockView],
    (theme, dark): (underline_core::Theme, bool),
    shown: &str,
) -> underline_editor_view::CachedViews {
    use underline_editor_view::views::{colors_key, diagram_key, diagram_language};
    use underline_facade::blocks::BlockKind;

    let mut cached = underline_editor_view::CachedViews::new(shown, dark);
    for (block, view) in blocks.iter().zip(backend) {
        let code: String = view.runs.iter().map(|r| r.text.as_str()).collect();
        match &block.kind {
            BlockKind::Code { language } => match diagram_language(&block.kind) {
                Some(diagram) => cached.insert_diagram(
                    diagram_key(diagram, &code, dark),
                    view.figure.clone().unwrap(),
                ),
                None => cached.insert_colors(
                    colors_key(theme.label(), language, &code),
                    view.colors.clone(),
                ),
            },
            BlockKind::Image { target, .. } => {
                let mut figure = view.figure.clone().unwrap();
                figure.caption = None;
                cached.insert_image(target, figure);
            }
            _ => {}
        }
    }
    cached
}

const EQUIVALENCE_MD: &str = "# Notes\n\nSome *text* with `code`, emoji 😀\r\nand CRLF.\n\n- one\n- [x] two\n\n> quoted\n\n```rust\nfn main() { let x = 1; }\n```\n\n```mermaid\ngraph TD\nA-->B\n```\n\n![The plan](img/plan.png)\n\n![Gone](img/gone.png)\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n";
const EQUIVALENCE_ADOC: &str = "= Notes\n\nSome text.\n\nNOTE: An admonition.\n\n.The plan\nimage::img/plan.png[Plan]\n\n.Numbers\n|===\n| a | b\n| 1 | 2\n|===\n\n[source,python]\n----\nprint(\"hi\")\n----\n\n.Again\nimage::img/gone.png[Gone]\n";

/// A workspace with one image for the equivalence tests.
fn equivalence_root(name: &str) -> PathBuf {
    let root = temp_dir(name).join("notes");
    fs::create_dir_all(root.join("img")).unwrap();
    fs::write(root.join("img/plan.png"), b"\x89PNG").unwrap();
    root
}

/// ADR-025, M-09.4, M-09.6, M-09.7, TC-004-02: the webview's views
/// (`CachedViews`), given what the backend made of the colours and figures,
/// are the backend's views, in Markdown and in AsciiDoc, on a light and on a
/// dark theme: the kinds, runs, offsets, prefixes and the numbers of the
/// captions are made by the same code (`docs/migration/variante-b.md`,
/// step B3b). Colours sent for another theme are not shown.
#[test]
fn adr025_cached_views_with_the_backends_figures_equal_the_backends_views() {
    use underline_core::Theme;
    use underline_editor_core::facade::MakeViews;

    let root = equivalence_root("equivalence");
    for (text, format, looks) in [
        (EQUIVALENCE_MD, NoteFormat::Markdown, (Theme::Light, false)),
        (
            EQUIVALENCE_ADOC,
            NoteFormat::AsciiDoc,
            (Theme::Light, false),
        ),
        (
            EQUIVALENCE_MD,
            NoteFormat::Markdown,
            (Theme::GitHubDark, true),
        ),
        (
            EQUIVALENCE_ADOC,
            NoteFormat::AsciiDoc,
            (Theme::GitHubDark, true),
        ),
    ] {
        let case = format!("{format:?} {:?}", looks.0);
        let rope = ropey::Rope::from_str(text);
        let (blocks, backend) = backend_views(&root, text, format, looks);
        let mut cached = cache_of(&blocks, &backend, looks, looks.0.label());

        assert_eq!(cached.all(&blocks, text, &rope), backend, "{case}");
        assert!(
            backend.iter().any(|v| v.colors.is_some()),
            "{case}: code is coloured"
        );
        // Markdown has no captions; AsciiDoc numbers figures and tables.
        let captioned = backend.iter().any(|v| {
            v.figure.as_ref().is_some_and(|f| f.caption.is_some())
                || matches!(&v.kind, BlockKindView::Table { table } if table.caption.is_some())
        });
        assert_eq!(captioned, format == NoteFormat::AsciiDoc, "{case}");

        // The colours of the theme sent are not those of another theme.
        let mut other = cache_of(&blocks, &backend, looks, Theme::Nord.label());
        let uncoloured: Vec<BlockView> = backend
            .iter()
            .cloned()
            .map(|mut view| {
                view.colors = None;
                view
            })
            .collect();
        assert_eq!(other.all(&blocks, text, &rope), uncoloured, "{case}");
    }
}

/// ADR-025, M-09.10: after an edit, the views the editor's tab made anew in
/// the webview (only the blocks around it, `MakeViews::some`) are the views
/// of the whole edited note, as the backend makes them.
#[test]
fn adr025_cached_views_after_an_edit_equal_the_backends_views_of_the_new_text() {
    use underline_core::Theme;
    use underline_editor_core::facade::{parse_all, MakeViews};
    use underline_editor_view::{EditorTab, FacadeCall};

    use crate::editor_view::revision;
    use crate::facade_view::FacadeAt;

    let root = equivalence_root("equivalence-edit");
    let looks = (Theme::Light, false);
    for (text, format) in [
        (EQUIVALENCE_MD, NoteFormat::Markdown),
        (EQUIVALENCE_ADOC, NoteFormat::AsciiDoc),
    ] {
        let (blocks, backend) = backend_views(&root, text, format, looks);
        let mut cached = cache_of(&blocks, &backend, looks, looks.0.label());
        let mut tab = EditorTab::new(text, format);
        let (parsed, refs) = parse_all(text, format);
        tab.facade.views = cached.all(&parsed, text, &ropey::Rope::from_str(text));
        tab.facade.blocks = std::sync::Arc::new(parsed);
        tab.facade.refs = refs;
        tab.facade.parsed = Some(tab.editor.revision());
        // Before "Some", which only ASCII precedes.
        let at = u32::try_from(text.find("Some").unwrap()).unwrap();
        let call = FacadeCall {
            note: "Home",
            at: FacadeAt {
                rev: revision(tab.editor.revision()),
                anchor: at,
                head: at,
            },
            mac: false,
            earlier: Vec::new(),
            views: &mut cached,
        };

        let handled = tab.facade_input(
            call,
            crate::FacadeInput::Text { text: "x".into() },
            None,
            None,
        );

        let edited = tab.editor.text();
        assert!(edited.contains("xSome"), "{format:?}: {edited}");
        assert_eq!(handled.patch.blocks.len(), 1, "{format:?}: one change");
        let (_, expected) = backend_views(&root, &edited, format, looks);
        assert_eq!(tab.facade.views, expected, "{format:?}");
    }
}

/// The next `ed-enrich` of the tick with colours, within 10 s.
fn next_enrich(session: &mut Session) -> Option<crate::EnrichView> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        for event in session.tick(Instant::now()) {
            if let Event::Enrich(view) = event {
                if view.colors.iter().any(|(_, c)| c.is_some()) {
                    return Some(*view);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

const CODE: &str = "# Home\n\n```rust\nfn main() {}\n```\n";

/// ADR-025 (review B5b): a new core in the window (`ed_mirror`) gets all
/// colours again, starting anew (`clear`).
#[test]
fn adr025_a_new_core_gets_everything_again() {
    let dir = temp_dir("forget-sent");
    let (mut session, _root) = session_with(&dir, CODE);
    session.editor_mirror(&home()).unwrap();
    session
        .set_editing_view(&home(), EditingViewName::Facade)
        .unwrap();
    let first = next_enrich(&mut session).expect("the first ed-enrich with colours");
    // Nothing new: nothing sent.
    assert!(next_enrich_soon(&mut session).is_none());

    session.editor_mirror(&home()).unwrap();
    session
        .set_editing_view(&home(), EditingViewName::Facade)
        .unwrap();

    let again = next_enrich(&mut session).expect("everything again");
    assert!(again.clear);
    assert_eq!(again.colors, first.colors);
}

/// An `ed-enrich` of the next few ticks.
/// What the backend sent of the facade of Home.
fn sent(session: &mut Session) -> &mut super::Sent {
    let open = session.open.as_mut().unwrap();
    let index = open.tabs.find(&home()).unwrap();
    &mut open.tabs.get_mut(index).facade.as_mut().unwrap().sent
}

fn next_enrich_soon(session: &mut Session) -> Option<crate::EnrichView> {
    for _ in 0..20 {
        for event in session.tick(Instant::now()) {
            if let Event::Enrich(view) = event {
                return Some(*view);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

/// ADR-025 (rust-reviewer B5b): what was sent stays bounded: past
/// `CODE_COLORS_KEPT` the backend starts anew (`clear`) with the colours of
/// the blocks now, which the window then keeps alone.
#[test]
fn adr025_what_was_sent_stays_bounded() {
    let dir = temp_dir("sent-bound");
    let (mut session, _root) = session_with(&dir, CODE);
    session.editor_mirror(&home()).unwrap();
    session
        .set_editing_view(&home(), EditingViewName::Facade)
        .unwrap();
    next_enrich(&mut session).expect("the first ed-enrich");
    for i in 0..underline_editor_view::CODE_COLORS_KEPT {
        sent(&mut session).colors.insert(format!("old {i}"), None);
    }

    // In the code block: its colours are made anew.
    session.window_type(&home(), 30, "x");
    let start = Instant::now();
    let mut cleared = false;
    while !cleared && start.elapsed() < Duration::from_secs(10) {
        cleared = session
            .tick(Instant::now())
            .iter()
            .any(|event| matches!(event, Event::Enrich(view) if view.clear));
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(cleared, "an ed-enrich that starts anew");
    // Only the code block's colours are kept now.
    assert!(sent(&mut session).colors.len() <= 1);
}
