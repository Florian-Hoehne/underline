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
    EditorCommandName, EditorInput, KeyName, TextChange, ToolName, VimChange, VimModeName, VimState,
};

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

    let pasted = tab.paste(Some("**pasted**"));
    assert_eq!(tab.editor.text(), "**pasted**\n");
    assert!(pasted.outcome.edited(), "the host marks the tab");
    assert!(tab.paste(None).patch.changes.is_empty());
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

    let handled = tab.paste(Some("pasted"));

    assert!(handled.patch.changes.is_empty());
    assert_eq!(tab.editor.text(), "ab\n");
    input(&mut tab, key(KeyName::Escape));
    tab.paste(Some("x"));
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
