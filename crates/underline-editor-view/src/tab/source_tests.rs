//! Tests of the source view's inputs on the editor's tab (UC-005, ADR-025):
//! what the window's editor core answers, which it runs in the WASM. They
//! were the session's tests of the commands `ed_input`, `ed_apply`,
//! `ed_select`, `ed_fold`, `ed_clipboard` and `ed_keys_status` (variant A,
//! removed in step B5c): the same rules, now where they run. Feature F4:
//! the Nano keys (M-11.3), the toolbar (M-10.2), the edits of the window's
//! Vim keys (A-01), the status line's keys and place (M-11.4) and folding
//! (M-08.8).

use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_markup::NoteFormat;

use super::*;
use crate::editor_view::{
    ClipContentView, CopyOnSelectView, EditorCommandName, EditorInput, Insertion, KeyName,
    TextChange, ToolName, VimChange, VimModeName, VimState,
};

/// What the clipboard holds: `text` alone.
fn clip(text: &str) -> ClipContentView {
    ClipContentView {
        text: Some(text.to_owned()),
        ..ClipContentView::default()
    }
}

fn text(text: &str) -> EditorInput {
    EditorInput::Text { text: text.into() }
}

fn keyed(key: KeyName, ctrl: bool, alt: bool, shift: bool) -> EditorInput {
    EditorInput::Key {
        key,
        ctrl,
        alt,
        shift,
        meta: false,
        page: 10,
    }
}

fn key(key: KeyName) -> EditorInput {
    keyed(key, false, false, false)
}

/// The command key with `key` (Ctrl; the tests run as on Linux).
fn command(key: KeyName, shift: bool) -> EditorInput {
    keyed(key, true, false, shift)
}

fn ctrl(key: KeyName) -> EditorInput {
    keyed(key, true, false, false)
}

fn alt(key: KeyName) -> EditorInput {
    keyed(key, false, true, false)
}

/// A Markdown tab holding `body`, the cursor at its start, with the keys
/// of `mode` (the window's Vim keys leave the usual ones to the core).
fn tab_with(body: &str, mode: KeysInputMode) -> EditorTab {
    let mut tab = EditorTab::new(body, NoteFormat::Markdown);
    tab.keys.mode = mode;
    tab
}

fn input(tab: &mut EditorTab, input: EditorInput) -> EditorPatch {
    tab.source_input(input, false).patch
}

fn rev(tab: &EditorTab) -> u32 {
    revision(tab.editor.revision())
}

/// ADR-025, UC-005 step 1, M-08.4: an input changes the text and answers
/// with a patch in UTF-16 units.
#[test]
fn uc005_m08_4_an_input_changes_the_text_and_answers_with_a_patch() {
    let mut tab = tab_with("# Home\n", KeysInputMode::Normal);
    let opened = tab.state("Home");
    assert_eq!(
        (opened.text.as_str(), opened.head, opened.anchor),
        ("# Home\n", 0, 0)
    );

    input(&mut tab, key(KeyName::End));
    let patch = input(&mut tab, text("🙂"));

    assert_eq!(patch.base, opened.rev);
    assert_ne!(patch.rev, opened.rev);
    assert_eq!(
        patch.changes,
        [TextChange {
            from: 6,
            to: 6,
            insert: "🙂".into()
        }]
    );
    assert_eq!((patch.head, patch.anchor), (8, 8), "UTF-16 units");
    assert!(patch.scroll);
    assert_eq!(tab.editor.text(), "# Home🙂\n");
}

/// M-08.6: copy and cut give the markup, paste inserts the text.
#[test]
fn m08_6_copy_and_cut_give_the_markup_and_paste_inserts_text() {
    let mut tab = tab_with("# Home\n", KeysInputMode::Normal);

    // Nothing selected: nothing for the clipboard.
    assert_eq!(tab.copy().0, None);

    tab.select(rev(&tab), 0, 6);
    let (copied, patch) = tab.copy();
    assert!(patch.changes.is_empty());
    assert_eq!(copied.as_deref(), Some("# Home"));

    let (cut, handled) = tab.cut();
    assert_eq!(cut.as_deref(), Some("# Home"));
    assert_eq!(handled.patch.changes.len(), 1);
    assert_eq!(tab.editor.text(), "\n");

    let (image, pasted) = tab.paste(Some(&clip("**pasted**")));
    assert!(!image);
    assert_eq!(tab.editor.text(), "**pasted**\n");
    assert!(pasted.outcome.edited(), "the host marks the tab");
    assert!(tab.paste(None).1.patch.changes.is_empty());
}

/// M-08.3: a selection made on an older revision is dropped.
#[test]
fn m08_3_a_selection_from_an_older_revision_is_dropped() {
    let mut tab = tab_with("# Home\n", KeysInputMode::Normal);
    let old = rev(&tab);
    input(&mut tab, text("x"));

    tab.select(old, 0, 3);

    let state = tab.state("Home");
    assert_eq!((state.anchor, state.head), (1, 1));
}

/// M-10.1: Ctrl+Z and Ctrl+Y reach the document's history; a word is one
/// step, as in egui.
#[test]
fn m10_1_undo_and_redo_keys_reach_the_document() {
    let mut tab = tab_with("# Home\n", KeysInputMode::Normal);
    for c in ["a", "b", " ", "c"] {
        input(&mut tab, text(c));
    }

    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "ab# Home\n");
    let patch = input(&mut tab, command(KeyName::Y, false));
    assert_eq!(tab.editor.text(), "ab c# Home\n");
    assert_eq!(patch.head, 4, "after the text put back");
}

/// NFR-007: in a CRLF note the window's units count a `\r\n` pair once;
/// an edit changes only its place, and Enter inserts `\n` as in egui.
#[test]
fn nfr007_editing_a_crlf_note_changes_only_the_edited_place() {
    let mut tab = tab_with("# Home\r\nline\r\n", KeysInputMode::Normal);
    let state = tab.state("Home");
    assert_eq!(state.text, "# Home\nline\n");

    // The window's place after the first break is 7.
    tab.select(state.rev, 7, 7);
    let patch = input(&mut tab, text("x"));
    assert_eq!(
        patch.changes,
        [TextChange {
            from: 7,
            to: 7,
            insert: "x".into()
        }]
    );
    input(&mut tab, key(KeyName::Enter));

    assert_eq!(tab.editor.text(), "# Home\r\nx\nline\r\n");
}

/// UC-005 BR-010, M-11.3: Nano's Ctrl+K cuts lines that add up, Ctrl+U
/// pastes them; typing ends the run of cuts.
#[test]
fn uc005_br010_m11_3_nano_cuts_lines_that_add_up_and_pastes_them() {
    let mut tab = tab_with("a\nb\nc\n", KeysInputMode::Nano);

    input(&mut tab, ctrl(KeyName::K));
    let patch = input(&mut tab, ctrl(KeyName::K));
    assert_eq!(tab.editor.text(), "c\n");
    assert_eq!(patch.keys.as_ref().unwrap().keymap, "NANO");

    input(&mut tab, ctrl(KeyName::U));
    assert_eq!(tab.editor.text(), "a\nb\nc\n");

    // Typing ends the run of cuts: the next cut holds only the new line,
    // which as the last line takes the break before it along.
    input(&mut tab, ctrl(KeyName::K));
    input(&mut tab, text("x"));
    input(&mut tab, ctrl(KeyName::K));
    assert_eq!(tab.editor.text(), "a\nb");
    input(&mut tab, ctrl(KeyName::U));
    assert_eq!(tab.editor.text(), "a\nx\nb");
}

/// M-11.3: Nano's Ctrl+A and Ctrl+E go to the line ends, Alt+U and Alt+E
/// undo and redo.
#[test]
fn m11_3_nano_ctrl_a_and_ctrl_e_go_to_the_line_ends_alt_u_and_alt_e_undo_and_redo() {
    let mut tab = tab_with("one two\n", KeysInputMode::Nano);

    assert_eq!(input(&mut tab, ctrl(KeyName::E)).place, "Ln 1, Col 8");
    assert_eq!(input(&mut tab, ctrl(KeyName::A)).place, "Ln 1, Col 1");

    input(&mut tab, text("x"));
    input(&mut tab, alt(KeyName::U));
    assert_eq!(tab.editor.text(), "one two\n");
    input(&mut tab, alt(KeyName::E));
    assert_eq!(tab.editor.text(), "xone two\n");
}

/// UC-008 BR-008, M-11.3: Nano searches by its prompt and marks the hits
/// until Escape; the marks follow the input, not within it.
#[test]
fn uc008_br008_m11_3_nano_searches_by_prompt_and_marks_the_hits_until_escape() {
    let mut tab = tab_with("ab\nab\n", KeysInputMode::Nano);

    let keys = input(&mut tab, ctrl(KeyName::W)).keys.unwrap();
    assert!(keys.prompting);
    assert_eq!(keys.pending, "Search: ");
    assert_eq!(keys.hints[0].keys, "Enter");
    // The prompt takes the text; the note keeps it.
    let typed = input(&mut tab, text("ab"));
    assert!(typed.changes.is_empty());
    assert_eq!(typed.keys.unwrap().pending, "Search: ab");

    let found = input(&mut tab, key(KeyName::Enter));
    assert_eq!(found.place, "Ln 2, Col 1", "the next hit, after the cursor");
    let keys = found.keys.unwrap();
    assert!(!keys.prompting);
    assert_eq!(
        (keys.hints[0].keys.as_str(), keys.hints[0].action.as_str()),
        ("Esc", "End search")
    );
    let marked = tab.found_view("Home", 1000.0).unwrap();
    assert_eq!(marked.ranges, [(0, 2), (3, 5)]);

    input(&mut tab, key(KeyName::Escape));
    let cleared = tab.found_view("Home", 2000.0).unwrap();
    assert!(cleared.ranges.is_empty());
}

/// M-11.3: Ctrl+C and Escape cancel Nano's prompt.
#[test]
fn m11_3_nano_ctrl_c_and_escape_cancel_the_prompt() {
    let mut tab = tab_with("ab\n", KeysInputMode::Nano);

    input(&mut tab, ctrl(KeyName::W));
    assert!(!input(&mut tab, ctrl(KeyName::C)).keys.unwrap().prompting);
    input(&mut tab, ctrl(KeyName::W));
    assert!(
        !input(&mut tab, key(KeyName::Escape))
            .keys
            .unwrap()
            .prompting
    );
}

/// M-10.9, M-11.3: Nano's Ctrl+O, Ctrl+S and Ctrl+X ask the window.
#[test]
fn m10_9_m11_3_nano_ctrl_o_ctrl_s_and_ctrl_x_ask_the_window() {
    let mut tab = tab_with("a\n", KeysInputMode::Nano);

    for (key, asked) in [
        (KeyName::O, EditorCommandName::Store),
        (KeyName::S, EditorCommandName::Store),
        (KeyName::X, EditorCommandName::Close),
    ] {
        let patch = input(&mut tab, ctrl(key));
        assert_eq!(patch.commands, [asked]);
        assert!(patch.changes.is_empty());
    }
}

/// M-11.1: without Nano its keys do what they usually do.
#[test]
fn m11_1_without_nano_its_keys_do_nothing_but_the_usual() {
    let mut tab = tab_with("a\nb\n", KeysInputMode::Normal);

    let patch = input(&mut tab, ctrl(KeyName::K));

    assert!(patch.keys.is_none());
    assert!(patch.changes.is_empty(), "Ctrl+K asks for a link instead");
    assert_eq!(patch.request, Some(ToolName::Link));
    assert_eq!(tab.editor.text(), "a\nb\n");
}

/// UC-005 A2, M-10.2: the toolbar toggles an inline format; each is one
/// undo step.
#[test]
fn uc005_a2_m10_2_the_toolbar_toggles_inline_formats_as_one_undo_step() {
    let mut tab = tab_with("word\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 0, 4);

    let bold = input(
        &mut tab,
        EditorInput::Tool {
            tool: ToolName::Bold,
        },
    );
    assert_eq!(tab.editor.text(), "**word**\n");
    assert_eq!(
        bold.place, "Ln 1, Col 7 · 4 chars",
        "the word stays selected"
    );

    input(
        &mut tab,
        EditorInput::Tool {
            tool: ToolName::Bold,
        },
    );
    assert_eq!(tab.editor.text(), "word\n", "a second time removes it");

    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "**word**\n");
    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "word\n");
}

/// UC-005 BR-005, M-10.2: line formats and the shortcuts follow the note's
/// format; Image asks first.
#[test]
fn uc005_br005_m10_2_line_formats_and_shortcuts_follow_the_note_format() {
    let mut tab = EditorTab::new("line\n", NoteFormat::AsciiDoc);

    // Ctrl+Shift+H: an AsciiDoc heading; Ctrl+B: AsciiDoc bold.
    input(&mut tab, command(KeyName::H, true));
    assert_eq!(tab.editor.text(), "== line\n");
    input(&mut tab, command(KeyName::End, false));
    input(&mut tab, command(KeyName::B, false));
    assert_eq!(tab.editor.text(), "== line\n**");
    let image = input(
        &mut tab,
        EditorInput::Tool {
            tool: ToolName::Image,
        },
    );
    assert_eq!(image.request, Some(ToolName::Image));
    assert!(image.changes.is_empty(), "Image asks first (feature F6)");
}

fn change(from: u32, to: u32, insert: &str, new_step: bool) -> VimChange {
    VimChange {
        from,
        to,
        insert: insert.into(),
        new_step,
    }
}

/// A-01: the window's Vim keys' changes are taken for their revision only,
/// in the one history (M-10.1).
#[test]
fn a01_the_vim_keys_changes_are_taken_for_their_revision_only() {
    let mut tab = tab_with("one two\n", KeysInputMode::Normal);
    let base = rev(&tab);

    let handled = tab
        .source_apply(base, &[change(0, 4, "", true)], (0, 0), false)
        .unwrap();
    assert_eq!(tab.editor.text(), "two\n");
    assert!(
        handled.patch.changes.is_empty(),
        "the window has the change"
    );
    assert_eq!(handled.patch.base, base);
    assert_ne!(handled.patch.rev, base);
    assert!(handled.outcome.edited(), "the host marks the tab");

    assert!(
        tab.source_apply(base, &[], (0, 0), false).is_none(),
        "an old revision is refused"
    );

    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "one two\n");
}

/// A-01: changes that do not fit the text are refused whole.
#[test]
fn a01_changes_that_do_not_fit_are_refused_whole() {
    let mut tab = tab_with("a🙂b\r\ncd", KeysInputMode::Normal);
    // In the window: "a🙂b\ncd", 7 units.
    let base = rev(&tab);
    let mut refused =
        |changes: &[VimChange]| tab.source_apply(base, changes, (0, 0), false).is_none();

    assert!(refused(&[change(0, 8, "", false)]), "beyond the end");
    assert!(refused(&[change(3, 2, "", false)]), "backwards");
    assert!(
        refused(&[change(2, 2, "x", false)]),
        "inside the surrogate pair"
    );
    assert!(
        refused(&[change(0, 1, "", false), change(6, 7, "", false)]),
        "the second is beyond the end after the first"
    );
    assert_eq!(tab.editor.text(), "a🙂b\r\ncd", "nothing applied");

    // Several changes, each on the text after the one before; the `\r\n`
    // pair counts once, and stays bytes for bytes elsewhere.
    assert!(tab
        .source_apply(
            base,
            &[change(0, 1, "", true), change(4, 6, "CD", true)],
            (0, 0),
            false,
        )
        .is_some());
    assert_eq!(tab.state("Home").text, "🙂b\nCD");
}

/// A-01, M-10.1: the window's Vim keys mark their undo steps.
#[test]
fn a01_m10_1_the_vim_keys_mark_their_undo_steps() {
    let mut tab = tab_with("", KeysInputMode::Normal);

    // An insert session typed through the usual inputs, after its start
    // was marked; then a command of its own.
    let start = rev(&tab);
    tab.source_apply(start, &[], (0, 0), true).unwrap();
    input(&mut tab, text("ab"));
    let now = rev(&tab);
    tab.source_apply(now, &[change(2, 2, "c", true)], (3, 3), false)
        .unwrap();
    // The same insertion without a mark merges with the typing.
    let now = rev(&tab);
    tab.source_apply(now, &[change(3, 3, "d", false)], (4, 4), false)
        .unwrap();

    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "ab");
    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "");
}

/// M-11.3: a paste into Nano's open prompt stays out of the note.
#[test]
fn m11_3_a_paste_into_the_open_nano_prompt_stays_out_of_the_note() {
    let mut tab = tab_with("ab\n", KeysInputMode::Nano);
    input(&mut tab, ctrl(KeyName::W));

    let (_, handled) = tab.paste(Some(&clip("pasted")));

    assert!(handled.patch.changes.is_empty());
    assert_eq!(tab.editor.text(), "ab\n");
    input(&mut tab, key(KeyName::Escape));
    tab.paste(Some(&clip("x")));
    assert_eq!(tab.editor.text(), "xab\n");
}

/// UC-005 BR-004, M-10.2: Bold without a selection merges with the typing,
/// as egui's `apply_format` replaces through the document like typing.
#[test]
fn uc005_br004_m10_2_bold_without_selection_merges_with_the_typing_as_in_egui() {
    let mut tab = tab_with("", KeysInputMode::Normal);
    input(&mut tab, text("w"));
    input(&mut tab, text("o"));

    input(
        &mut tab,
        EditorInput::Tool {
            tool: ToolName::Bold,
        },
    );
    assert_eq!(tab.editor.text(), "wo****");

    input(&mut tab, command(KeyName::Z, false));
    assert_eq!(tab.editor.text(), "");
}

/// M-11.4: the keys of the status line follow the input mode of the
/// settings.
#[test]
fn m11_4_the_keys_of_the_status_line_follow_the_input_mode() {
    let tab = tab_with("a\n", KeysInputMode::Normal);
    assert!(tab
        .source_keys_status(KeysInputMode::Normal, None)
        .is_none());

    let vim = tab
        .source_keys_status(
            KeysInputMode::Vim,
            Some(VimState {
                mode: VimModeName::VisualLine,
                pending: "2d".into(),
                marked: true,
            }),
        )
        .unwrap();
    assert_eq!(
        (vim.keymap.as_str(), vim.mode.as_deref()),
        ("VIM", Some("VISUAL LINE"))
    );
    assert_eq!(vim.color, 2);
    assert_eq!(vim.pending, "", "visual mode shows nothing typed");
    assert_eq!(vim.hints[0].keys, "d");

    let normal = tab
        .source_keys_status(
            KeysInputMode::Vim,
            Some(VimState {
                mode: VimModeName::Normal,
                pending: "2d".into(),
                marked: true,
            }),
        )
        .unwrap();
    assert_eq!(normal.pending, "2d");
    assert_eq!(normal.hints[0].action, "end search");

    let nano = tab_with("a\n", KeysInputMode::Nano)
        .source_keys_status(KeysInputMode::Nano, None)
        .unwrap();
    assert_eq!((nano.keymap.as_str(), nano.mode), ("NANO", None));
    assert_eq!(nano.hints[0].keys, "^O");
}

/// UC-005, M-11.4: the place says line, column and what is selected.
#[test]
fn uc005_m11_4_the_place_says_line_column_and_what_is_selected() {
    let mut tab = tab_with("ab\ncd\n", KeysInputMode::Normal);
    let base = rev(&tab);

    assert_eq!(tab.select(base, 1, 2), "Ln 1, Col 3 · 1 char");
    assert_eq!(tab.select(base, 0, 5), "Ln 2, Col 3 · 2 lines");
    assert_eq!(tab.state("Home").place, "Ln 2, Col 3 · 2 lines");
}

/// UC-004 BR-012, M-08.8: folding by key and by the arrow; the sections
/// follow the input, not within it.
#[test]
fn uc004_br012_m08_8_folding_by_key_is_told_after_the_input() {
    let mut tab = tab_with("# A\none\n# B\ntwo\n", KeysInputMode::Normal);
    let first = tab.sections_view("Home", 1000.0).unwrap();
    assert_eq!(first.sections.len(), 2);
    assert!(first.sections.iter().all(|s| !s.folded));

    // The cursor on "one": folding moves it to the heading.
    input(&mut tab, key(KeyName::ArrowDown));
    let folded = input(&mut tab, command(KeyName::BracketLeft, true));
    assert_eq!(folded.place, "Ln 1, Col 4");
    let told = tab.sections_view("Home", 2000.0).unwrap();
    assert!(told.sections[0].folded);
    assert_eq!((told.sections[0].line, told.sections[0].last), (0, 1));

    // Down steps over the folded line; the arrow expands again.
    let down = input(&mut tab, key(KeyName::ArrowDown));
    assert_eq!(down.place, "Ln 3, Col 4");
    tab.fold_line(0);
    let open = tab.sections_view("Home", 3000.0).unwrap();
    assert!(!open.sections[0].folded);
}

/// M-10.7, FR-031, BR-011 (TC-005-17): copy on select copies a finished
/// selection once, asks the backend to keep the former content with its
/// first copy, and pasting over exactly that selection is known; without a
/// selection it is forgotten, and switched off nothing is copied.
#[test]
fn m10_7_copy_on_select_copies_a_selection_once() {
    let mut tab = tab_with("One two three\n", KeysInputMode::Normal);
    assert_eq!(
        tab.copy_on_select(true, false),
        CopyOnSelectView::default(),
        "nothing selected"
    );

    tab.select(rev(&tab), 0, 3);
    let copy = tab.copy_on_select(true, false).copy.expect("a copy");
    assert_eq!(copy.text, "One");
    assert_eq!(copy.html, None, "the source view copies the markup only");
    assert!(copy.first);
    assert_eq!(tab.own_markup.as_deref(), Some("One"));
    assert_eq!(
        tab.copy_on_select(true, false).copy,
        None,
        "once per selection"
    );
    assert!(tab.over_copied());

    // A longer selection is copied again, the former content stays kept.
    tab.select(rev(&tab), 0, 7);
    let copy = tab.copy_on_select(true, false).copy.expect("a copy");
    assert_eq!(copy.text, "One two");
    assert!(!copy.first);

    // Without a selection it is forgotten, and the backend lets go once.
    tab.select(rev(&tab), 2, 2);
    let done = tab.copy_on_select(true, false);
    assert_eq!((done.copy, done.forget), (None, true));
    assert!(!tab.copy_on_select(true, false).forget);
    assert!(!tab.over_copied());
    tab.select(rev(&tab), 0, 7);
    assert!(!tab.over_copied());

    // Switched off, nothing is copied.
    let mut off = tab_with("One two\n", KeysInputMode::Normal);
    off.select(rev(&off), 0, 3);
    assert_eq!(off.copy_on_select(false, false).copy, None);
    assert!(!off.over_copied());
}

/// M-10.7, BR-011: in the facade copy on select gives the HTML too.
#[test]
fn m10_7_copy_on_select_in_the_facade_gives_html() {
    let mut tab = tab_with("Use **bold** here\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 0, 12);
    let copy = tab.copy_on_select(true, true).copy.expect("a copy");
    assert_eq!(copy.text, "Use **bold**");
    assert_eq!(
        copy.html.as_deref(),
        Some("<p>Use <strong>bold</strong></p>")
    );
}

/// M-08.6: copying and cutting in the source view remember the markup, so
/// the facade pastes it back as markup (A-53, `copy` of egui).
#[test]
fn m08_6_copying_in_the_source_view_remembers_the_markup() {
    let mut tab = tab_with("**a**\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 0, 5);
    tab.copy();
    assert_eq!(tab.own_markup.as_deref(), Some("**a**"));
}

/// UC-005 A17, A18, M-10.6: the source view pastes plain text, never the
/// HTML's markup; an image alone is left to the backend, the text unchanged.
#[test]
fn uc005_a18_m10_6_an_image_alone_is_left_to_the_backend() {
    let mut tab = tab_with("ab\n", KeysInputMode::Normal);
    let html = ClipContentView {
        text: Some("plain".into()),
        markup: Some("**rich**".into()),
        image: false,
    };
    let (image, _) = tab.paste(Some(&html));
    assert!(!image);
    assert_eq!(tab.editor.text(), "plainab\n");

    let picture = ClipContentView {
        text: None,
        markup: None,
        image: true,
    };
    let (image, handled) = tab.paste(Some(&picture));
    assert!(image);
    assert!(handled.patch.changes.is_empty());
    assert_eq!(tab.editor.text(), "plainab\n");
}

/// UC-005 A2, M-10.3, M-10.4, M-10.6: the answers of the link and image
/// dialogs and a pasted image go in as egui's `apply_format` puts them.
#[test]
fn uc005_a2_m10_3_m10_4_insertions_in_the_source_view() {
    let mut tab = tab_with("See here\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 4, 8);
    let handled = tab.source_insert(Insertion::Link {
        address: "https://x.org".into(),
        label: None,
    });
    assert!(handled.outcome.edited());
    assert_eq!(tab.editor.text(), "See [here](https://x.org)\n");

    let mut tab = tab_with("a\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 1, 1);
    tab.source_insert(Insertion::Link {
        address: "../Plan.md".into(),
        label: Some("Plan".into()),
    });
    assert_eq!(tab.editor.text(), "a[Plan](../Plan.md)\n");

    // A file that is no picture is a link with its file name.
    let mut tab = tab_with("", KeysInputMode::Normal);
    tab.source_insert(Insertion::File {
        place: "docs/report.pdf".into(),
        image: false,
    });
    assert_eq!(tab.editor.text(), "[report.pdf](docs/report.pdf)");

    // A picture on a paragraph of its own.
    let mut tab = tab_with("a\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 1, 1);
    tab.source_insert(Insertion::Image {
        place: "Plan-2026-11-10-0930.png".into(),
    });
    assert_eq!(tab.editor.text(), "a\n\n![](Plan-2026-11-10-0930.png)\n");
}

/// UC-005 A9 step 2, M-10.5: a dropped file goes in by its place only in a
/// code block, else as a picture or a link; the space between dropped items
/// is not escaped.
#[test]
fn uc005_a9_m10_5_a_dropped_file_in_a_code_block_is_its_place() {
    let mut tab = tab_with("```\ncode\n```\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 8, 8);
    tab.source_insert(Insertion::Dropped {
        place: "photo.png".into(),
        image: true,
    });
    tab.source_insert(Insertion::Text { text: " *".into() });
    assert_eq!(tab.editor.text(), "```\ncodephoto.png *\n```\n");

    let mut tab = tab_with("Text\n", KeysInputMode::Normal);
    tab.select(rev(&tab), 4, 4);
    tab.source_insert(Insertion::Dropped {
        place: "photo.png".into(),
        image: true,
    });
    assert_eq!(tab.editor.text(), "Text\n\n![](photo.png)\n");
}

/// Kann (review F6), Q-52: a link to a note with spaces and brackets in
/// its place goes in as egui's `apply_format` writes it, in both formats.
#[test]
fn m10_3_a_link_with_spaces_and_brackets_as_egui_writes_it() {
    let link = || Insertion::Link {
        address: "../a b/Plan [v2].md".into(),
        label: Some("Plan [v2]".into()),
    };
    let mut md = tab_with("", KeysInputMode::Normal);
    md.source_insert(link());
    assert_eq!(md.editor.text(), "[Plan [v2]](../a b/Plan [v2].md)");
    let mut adoc = EditorTab::new("", NoteFormat::AsciiDoc);
    adoc.source_insert(link());
    assert_eq!(adoc.editor.text(), "link:../a b/Plan [v2].md[Plan [v2]]");
}

/// The link at web offset `at` of `text`.
fn link(text: &str, format: NoteFormat, at: u32) -> Option<String> {
    link_at_web(&ropey::Rope::from_str(text), format, at)
}

/// UC-004 BR-006, M-07.3: a Ctrl+click follows the link at the place
/// clicked, as egui's `link_at` finds it; elsewhere there is none.
#[test]
fn uc004_m07_3_the_link_at_a_place_of_the_text() {
    let md = "Read [the plan](Plan.md#goals) now.\n";
    let at = |at| link(md, NoteFormat::Markdown, at);
    // On the label, from its first character to just after its last, as
    // egui; not on the brackets or the address.
    assert_eq!(at(6).as_deref(), Some("Plan.md#goals"));
    assert_eq!(at(14).as_deref(), Some("Plan.md#goals"));
    assert_eq!(at(5), None);
    assert_eq!(at(22), None);
    assert_eq!(at(0), None);
    assert_eq!(at(34), None);
    // Past the end: nothing, no panic.
    assert_eq!(at(1000), None);
    assert_eq!(at(u32::MAX), None);

    let adoc = "See link:Plan.adoc[the plan].\n";
    assert_eq!(
        link(adoc, NoteFormat::AsciiDoc, 20).as_deref(),
        Some("Plan.adoc")
    );
}

/// M-07.3, A-26: offsets are web units: an emoji counts two, a `\r\n`
/// pair one.
#[test]
fn m07_3_the_link_is_found_at_web_offsets() {
    let text = "🙂\r\n[x](y.md)\r\n";
    // 🙂 is two units, the pair one: the bracket is at 3, the label at 4.
    assert_eq!(link(text, NoteFormat::Markdown, 4).as_deref(), Some("y.md"));
    assert_eq!(link(text, NoteFormat::Markdown, 3), None);
}
