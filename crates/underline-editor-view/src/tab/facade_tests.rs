//! Tests of editing in the facade on the editor's tab (UC-005, M-09.10 to
//! M-09.14, M-11.2, M-11.3, ADR-025): the rules of each kind of block, the
//! empty lines after Enter, joining blocks, the defects D-17 and D-18, CRLF
//! and emoji, the one history of both views, the Vim and Nano keys with a
//! block as a line, and that the window, applying the patches, always holds
//! the blocks a full parse gives. They were the session's tests of
//! `fa_input` and `fa_clipboard` (variant A, removed in step B5c): the same
//! rules, now where the window's editor core runs them.

use std::time::Instant;

use ropey::Rope;
use underline_editor_core::facade::{parse_all, MakeViews};
use underline_editor_core::keys::InputMode as KeysInputMode;
use underline_markup::NoteFormat;

use super::*;
use crate::editor_view::{ClipAction, ClipContentView, Insertion, KeyName};
use crate::facade_view::{BlockView, FacadeAt, FacadeInput, FacadePatch, GapView, RowsView};
use crate::CachedViews;

/// What the window holds of the facade, as `FacadeView.svelte` keeps it.
struct Window {
    rev: u32,
    version: u32,
    blocks: Vec<BlockView>,
    anchor: u32,
    head: u32,
    gap: Option<GapView>,
}

/// The facade of a note on the editor's tab, opened in the window.
struct Facade {
    tab: EditorTab,
    views: CachedViews,
    window: Window,
    format: NoteFormat,
}

/// The views of a full parse of `text` (no colours or pictures sent).
fn views_of(text: &str, format: NoteFormat) -> Vec<BlockView> {
    let (blocks, _) = parse_all(text, format);
    CachedViews::new("Light", false).all(&blocks, text, &Rope::from_str(text))
}

impl Facade {
    fn new(_name: &str, text: &str) -> Self {
        Self::with(text, NoteFormat::Markdown)
    }

    fn adoc(_name: &str, text: &str) -> Self {
        Self::with(text, NoteFormat::AsciiDoc)
    }

    fn with(text: &str, format: NoteFormat) -> Self {
        let mut tab = EditorTab::new(text, format);
        let mut views = CachedViews::new("Light", false);
        tab.bring_facade_up_to_date(&mut views);
        let state = tab.facade_state("Home");
        Self {
            tab,
            views,
            window: Window {
                rev: state.rev,
                version: state.version,
                blocks: state.blocks,
                anchor: state.anchor,
                head: state.head,
                gap: state.gap,
            },
            format,
        }
    }

    /// The note's text, its line breaks kept.
    fn text(&self) -> String {
        self.tab.editor.text()
    }

    fn at(&self) -> FacadeAt {
        FacadeAt {
            rev: self.window.rev,
            anchor: self.window.anchor,
            head: self.window.head,
        }
    }

    /// Takes in a patch as the window does, and checks the window's blocks
    /// against a full parse of the text.
    fn take(&mut self, patch: FacadePatch) -> FacadePatch {
        for change in &patch.blocks {
            assert_eq!(change.base, self.window.version, "changes in the chain");
            let from = change.from as usize;
            let removed = change.removed as usize;
            let mut after: Vec<BlockView> = self.window.blocks.split_off(from + removed);
            self.window.blocks.truncate(from);
            self.window.blocks.extend(change.blocks.iter().cloned());
            for block in &mut after {
                block.start =
                    u32::try_from(i64::from(block.start) + i64::from(change.shift)).unwrap();
                block.line =
                    u32::try_from(i64::from(block.line) + i64::from(change.line_shift)).unwrap();
            }
            self.window.blocks.extend(after);
            self.window.version = change.version;
            self.window.rev = change.rev;
        }
        self.window.rev = patch.rev;
        self.window.anchor = patch.anchor;
        self.window.head = patch.head;
        self.window.gap = patch.gap;
        self.assert_in_step();
        patch
    }

    /// The window's blocks are those of a full parse of the text, as far
    /// as the text goes (colours and pictures aside).
    fn assert_in_step(&self) {
        let strip = |views: &[BlockView]| -> Vec<BlockView> {
            views
                .iter()
                .cloned()
                .map(|mut view| {
                    view.colors = None;
                    view.figure = None;
                    view
                })
                .collect()
        };
        let text = self.text();
        assert_eq!(
            strip(&self.window.blocks),
            strip(&views_of(&text, self.format)),
            "the window's blocks of {text:?}"
        );
    }

    /// One input at the selection `at`, as the window's core runs it.
    fn call(&mut self, at: FacadeAt, input: FacadeInput, rows: Option<RowsView>) -> FacadePatch {
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: "Home",
            at,
            mac: false,
            earlier,
            views: &mut self.views,
        };
        self.tab.facade_input(call, input, rows, None).patch
    }

    /// Copy, cut or paste at the selection `at`; the patch is taken in.
    fn clipboard(
        &mut self,
        at: FacadeAt,
        action: ClipAction,
        plain: bool,
        text: Option<&str>,
    ) -> (Option<String>, FacadePatch) {
        let content = text.map(|text| ClipContentView {
            text: Some(text.to_owned()),
            ..ClipContentView::default()
        });
        let clipped = self.clipboard_with(at, action, plain, content.as_ref());
        (clipped.copied, clipped.handled.patch)
    }

    /// Copy, cut or paste `content` at the selection `at`, all of the answer.
    fn clipboard_with(
        &mut self,
        at: FacadeAt,
        action: ClipAction,
        plain: bool,
        content: Option<&ClipContentView>,
    ) -> FacadeClipped {
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: "Home",
            at,
            mac: false,
            earlier,
            views: &mut self.views,
        };
        self.tab.facade_clipboard(call, action, plain, content)
    }

    fn input(&mut self, input: FacadeInput) -> FacadePatch {
        self.input_with(input, None)
    }

    fn input_with(&mut self, input: FacadeInput, rows: Option<RowsView>) -> FacadePatch {
        let at = self.at();
        let patch = self.call(at, input, rows);
        self.take(patch)
    }

    fn type_text(&mut self, text: &str) -> FacadePatch {
        self.input(FacadeInput::Text {
            text: text.to_owned(),
        })
    }

    fn key(&mut self, key: KeyName) -> FacadePatch {
        self.key_with(key, false, false)
    }

    fn key_with(&mut self, key: KeyName, ctrl: bool, shift: bool) -> FacadePatch {
        self.input(FacadeInput::Key {
            key,
            ctrl,
            alt: false,
            shift,
            meta: false,
        })
    }

    /// Puts the cursor at the web offset `at`, as a click does.
    fn click(&mut self, at: u32) {
        let rev = self.window.rev;
        self.tab.select(rev, at, at);
        self.window.anchor = at;
        self.window.head = at;
        self.window.gap = None;
    }

    /// Puts the cursor right after the first `needle` of the text (no
    /// `\r` before it).
    fn click_after(&mut self, needle: &str) {
        let text = self.text().replace("\r\n", "\n");
        let byte = text.find(needle).unwrap() + needle.len();
        let at = text[..byte].encode_utf16().count();
        self.click(u32::try_from(at).unwrap());
    }

    fn click_before(&mut self, needle: &str) {
        let text = self.text().replace("\r\n", "\n");
        let byte = text.find(needle).unwrap();
        let at = text[..byte].encode_utf16().count();
        self.click(u32::try_from(at).unwrap());
    }

    /// The input mode of the settings (the facade has the core's Vim).
    fn mode(&mut self, mode: &str) {
        self.tab.keys.mode = match mode {
            "Vim" => KeysInputMode::Vim,
            "Nano" => KeysInputMode::Nano,
            _ => KeysInputMode::Normal,
        };
    }
}

#[test]
fn uc005_m09_10_typing_escapes_markdown_and_keeps_other_blocks_byte_for_byte() {
    let note = "# Title\n\nSome text.\n\n```rust\nfn a() {}\n```\n\n* odd  list\n";
    let mut f = Facade::new("escape", note);
    f.click_after("Some");
    let patch = f.type_text(" *x_");
    assert_eq!(
        f.text(),
        "# Title\n\nSome \\*x\\_ text.\n\n```rust\nfn a() {}\n```\n\n* odd  list\n"
    );
    // ADR-024: only the edited block went to the window.
    assert_eq!(patch.blocks.len(), 1);
    assert_eq!(patch.blocks[0].removed, 1);
    assert_eq!(patch.blocks[0].blocks.len(), 1);
    // Code takes raw text.
    f.click_after("fn a(");
    f.type_text("*b");
    assert!(f.text().contains("fn a(*b) {}"));
    // At the start of a block, `#` is escaped too.
    f.click_before("Title");
    f.type_text("#");
    assert!(f.text().starts_with("# \\#Title"), "{}", f.text());
}

#[test]
fn uc005_m09_10_asciidoc_is_not_escaped() {
    let mut f = Facade::adoc("adoc", "= Doc\n\nSome text here.\n");
    f.click_after("Some");
    f.type_text(" plain");
    f.type_text(" *b*");
    assert_eq!(f.text(), "= Doc\n\nSome plain *b* text here.\n");
}

#[test]
fn uc005_m09_11_enter_per_kind_of_block() {
    let note = "Para one.\n\n- item\n1. first\n- [x] done\n\n> quoted\n\n```\ncode\n```\n";
    let mut f = Facade::new("enter", note);
    f.click_after("item");
    f.key(KeyName::Enter);
    assert!(f.text().contains("- item\n- \n1. first"), "{:?}", f.text());
    f.click_after("first");
    f.key(KeyName::Enter);
    assert!(f.text().contains("1. first\n2. \n"), "{:?}", f.text());
    f.click_after("done");
    f.key(KeyName::Enter);
    assert!(f.text().contains("- [x] done\n- [ ] \n"), "{:?}", f.text());
    f.click_after("quoted");
    f.key(KeyName::Enter);
    assert!(f.text().contains("> quoted\n\n> \n"), "{:?}", f.text());
    f.click_after("code");
    f.key(KeyName::Enter);
    assert!(f.text().contains("code\n\n```"), "{:?}", f.text());
    f.click_after("Para");
    f.key(KeyName::Enter);
    assert!(f.text().starts_with("Para\n\n one."), "{:?}", f.text());
}

#[test]
fn uc005_m09_11_the_gap_after_enter_grows_and_shrinks() {
    let mut f = Facade::new("gap", "First.\n\nSecond.\n");
    f.click_after("First.");
    let patch = f.key(KeyName::Enter);
    assert_eq!(f.text(), "First.\n\n\n\nSecond.\n");
    assert_eq!(
        patch.gap,
        Some(GapView {
            before: 1,
            lines: 1
        })
    );
    let patch = f.key(KeyName::Enter);
    assert_eq!(f.text(), "First.\n\n\n\n\nSecond.\n");
    assert_eq!(
        patch.gap,
        Some(GapView {
            before: 1,
            lines: 2
        })
    );
    let patch = f.key(KeyName::Backspace);
    assert_eq!(f.text(), "First.\n\n\n\nSecond.\n");
    assert_eq!(
        patch.gap,
        Some(GapView {
            before: 1,
            lines: 1
        })
    );
    // From the first empty line, Backspace takes the paragraph break.
    let patch = f.key(KeyName::Backspace);
    assert_eq!(f.text(), "First.\n\nSecond.\n");
    assert_eq!(patch.gap, None);
    // Typing in the gap makes a paragraph of its own.
    f.key(KeyName::Enter);
    f.type_text("Middle");
    assert_eq!(f.text(), "First.\n\nMiddle\n\nSecond.\n");
}

/// D-17: Enter before a space and Backspace give the text back.
#[test]
fn d17_enter_then_backspace_keeps_the_space() {
    let mut f = Facade::new("d17", "An image block here.\n");
    f.click_after("image");
    f.key(KeyName::Enter);
    assert_eq!(f.text(), "An image\n\n block here.\n");
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "An image block here.\n");
}

#[test]
fn uc005_m09_12_backspace_and_delete_take_a_shown_character_or_join() {
    let mut f = Facade::new("delete", "A \\*b\\* c\n\nNext\n\n---\n\nLast\n");
    f.click_after("\\*b\\*");
    f.key(KeyName::Backspace);
    // The escaped `*` goes with its backslash.
    assert_eq!(f.text(), "A \\*b c\n\nNext\n\n---\n\nLast\n");
    f.click_before("Next");
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "A \\*b cNext\n\n---\n\nLast\n");
    // Before a break, Backspace takes the break whole.
    f.click_before("Last");
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "A \\*b cNext\n\nLast\n");
    // Delete at the end of a block joins the next.
    let mut g = Facade::new("delete-forward", "One\n\nTwo\n");
    g.click_after("One");
    g.key(KeyName::Delete);
    assert_eq!(g.text(), "OneTwo\n");
    g.key(KeyName::Delete);
    assert_eq!(g.text(), "Onewo\n");
}

#[test]
fn uc005_m10_1_undo_and_redo_are_the_notes_one_history() {
    let mut f = Facade::new("undo", "Hello\n\nWorld\n");
    f.click_after("Hello");
    f.type_text(" there");
    f.key(KeyName::Enter);
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), "Hello there\n\nWorld\n");
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), "Hello\n\nWorld\n");
    f.key_with(KeyName::Y, true, false);
    assert_eq!(f.text(), "Hello there\n\nWorld\n");
    // The source view undoes the facade's edits too (same document).
    let patch = f
        .tab
        .source_input(
            crate::editor_view::EditorInput::Key {
                key: KeyName::Z,
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
                page: 1,
            },
            false,
        )
        .patch;
    assert!(!patch.changes.is_empty());
    assert_eq!(f.text(), "Hello\n\nWorld\n");
}

#[test]
fn adr021_crlf_and_emoji_offsets_in_the_facade() {
    let mut f = Facade::new("crlf", "A 🙂 b\r\n\r\nNext 日本\r\n");
    f.click_after("🙂");
    f.type_text("x");
    assert_eq!(f.text(), "A 🙂x b\r\n\r\nNext 日本\r\n");
    f.key(KeyName::Backspace);
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "A  b\r\n\r\nNext 日本\r\n");
    f.click_after("日本");
    f.key(KeyName::Enter);
    f.type_text("z");
    assert_eq!(f.text(), "A  b\r\n\r\nNext 日本\n\nz\r\n");
    // Left from the start of a block goes to the end of the one before.
    f.click_before("z");
    let patch = f.key(KeyName::ArrowLeft);
    let text = f.text().replace("\r\n", "\n");
    let end = text.find("日本").unwrap() + "日本".len();
    assert_eq!(patch.head as usize, text[..end].encode_utf16().count());
}

#[test]
fn m09_13_arrows_move_by_shown_characters_and_rows() {
    let mut f = Facade::new("arrows", "# Head\n\nA **b** c\n\nLast line\n");
    f.click_after("A ");
    // Right over the markup of `**b**`: one shown character.
    let at = f.window.head;
    let patch = f.key(KeyName::ArrowRight);
    assert_eq!(patch.head, at + 5);
    // Down without rows measured: into the next block at the same index.
    let patch = f.key(KeyName::ArrowDown);
    let text = f.text();
    let last = text.find("Last").unwrap();
    assert_eq!(patch.head as usize, last + 3);
    // Up with the rows the window measured: a row within the block.
    let head = patch.head;
    let rows = RowsView {
        at: head,
        up: Some(u32::try_from(last).unwrap()),
        down: None,
        start: u32::try_from(last).unwrap(),
        end: u32::try_from(last + 9).unwrap(),
    };
    let patch = f.input_with(
        FacadeInput::Key {
            key: KeyName::ArrowUp,
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        },
        Some(rows),
    );
    assert_eq!(patch.head as usize, last);
    let patch = f.input_with(
        FacadeInput::Key {
            key: KeyName::End,
            ctrl: false,
            alt: false,
            shift: true,
            meta: false,
        },
        Some(RowsView {
            at: patch.head,
            ..rows
        }),
    );
    assert_eq!(
        (patch.anchor as usize, patch.head as usize),
        (last, last + 9)
    );
}

/// D-18: typing before a table keeps its cell offsets right, so the window
/// gets the blocks a full parse gives (checked by `take`).
#[test]
fn d18_typing_before_a_table_keeps_the_window_in_step() {
    let mut f = Facade::new("d18", "Para\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
    f.click_after("Pa");
    for c in ["x", "y", "z"] {
        f.type_text(c);
    }
    assert!(f.text().starts_with("Paxyzra"));
}

#[test]
fn m09_8_the_markup_of_a_figure_is_edited_as_raw_text() {
    let mut f = Facade::new(
        "figure",
        "Text\n\n![Alt *x*](a.png)\n\n| a |\n|---|\n| 1 |\n",
    );
    f.click_after("Alt");
    f.type_text(" *y*");
    assert!(f.text().contains("![Alt *y* *x*](a.png)"), "{}", f.text());
    f.click_after("| 1");
    f.type_text("0");
    assert!(f.text().contains("| 10 |"));
}

#[test]
fn m09_14_own_markup_pastes_as_markup_other_text_as_typed() {
    let mut f = Facade::new("paste", "One **two**\n\nThree\n");
    // Copying gives the markup.
    let at = FacadeAt {
        rev: f.window.rev,
        anchor: 0,
        head: 11,
    };
    let (copied, patch) = f.clipboard(at, ClipAction::Copy, false, None);
    f.take(patch);
    assert_eq!(copied.as_deref(), Some("One **two**"));
    f.click_after("Three");
    let multi = "- a\n- b";
    // Plain text is typed: escaped.
    let (_, patch) = f.clipboard(f.at(), ClipAction::Paste, false, Some(" *x*"));
    f.take(patch);
    assert!(f.text().contains("Three \\*x\\*"), "{}", f.text());
    // The own markup comes as markup, on lines of its own when it has several.
    f.tab.own_markup = Some(multi.to_owned());
    let (_, patch) = f.clipboard(f.at(), ClipAction::Paste, false, Some(multi));
    f.take(patch);
    // A blank line before and after it, as in egui.
    assert!(
        f.text().ends_with("Three \\*x\\*\n\n- a\n- b\n\n"),
        "{:?}",
        f.text()
    );
    // Ctrl+Shift+V types even the own markup.
    f.tab.own_markup = Some("*z*".to_owned());
    f.click_after("One");
    let (_, patch) = f.clipboard(f.at(), ClipAction::Paste, true, Some("*z*"));
    f.take(patch);
    assert!(f.text().starts_with("One\\*z\\* **two**"), "{:?}", f.text());
    // Cutting takes the selection away.
    let at = FacadeAt {
        rev: f.window.rev,
        anchor: 0,
        head: 3,
    };
    let (cut, patch) = f.clipboard(at, ClipAction::Cut, false, None);
    f.take(patch);
    assert_eq!(cut.as_deref(), Some("One"));
    assert!(f.text().starts_with("\\*z\\* **two**"), "{:?}", f.text());
}

#[test]
fn m11_2_vim_in_the_facade_a_line_is_a_block() {
    let mut f = Facade::new("vim", "First block.\n\nSecond block.\n\nThird.\n");
    f.mode("Vim");
    f.click_after("Second");
    // `dd` deletes the block with what separates it from the next.
    f.type_text("d");
    let patch = f.type_text("d");
    assert_eq!(f.text(), "First block.\n\nThird.\n");
    assert_eq!(
        patch.keys.as_ref().and_then(|k| k.mode.clone()).as_deref(),
        Some("NORMAL")
    );
    // `A` appends at the end of the block; typing goes into the note.
    f.type_text("A");
    f.type_text("!");
    assert_eq!(f.text(), "First block.\n\nThird.!\n");
    f.key(KeyName::Escape);
    // `k` goes one block up; `x` deletes a shown character.
    f.type_text("k");
    f.type_text("0");
    f.type_text("x");
    assert_eq!(f.text(), "irst block.\n\nThird.!\n");
    // `u` undoes it.
    f.type_text("u");
    assert_eq!(f.text(), "First block.\n\nThird.!\n");
}

#[test]
fn m11_3_nano_in_the_facade_cuts_and_pastes_blocks() {
    let mut f = Facade::new("nano", "One.\n\nTwo.\n\nThree.\n");
    f.mode("Nano");
    f.click_after("Tw");
    let patch = f.key_with(KeyName::K, true, false);
    assert_eq!(f.text(), "One.\n\nThree.\n");
    assert_eq!(patch.keys.as_ref().map(|k| k.keymap.as_str()), Some("NANO"));
    f.key_with(KeyName::U, true, false);
    assert_eq!(f.text(), "One.\n\nTwo.\n\nThree.\n");
    // Typing goes through as in Normal.
    f.click_after("One");
    f.type_text("_");
    assert_eq!(f.text(), "One\\_.\n\nTwo.\n\nThree.\n");
}

/// ADR-024, ADR-025: an edit of a heading or indented code right before a
/// line that starts with a non-ASCII char keeps the window in step: the
/// part of the note made anew ends on a char boundary (no panic in ropey).
#[test]
fn adr025_editing_right_before_a_non_ascii_line() {
    for (name, text, after, edited) in [
        ("wide-head", "# H\néx\n", "# H", "# Hy\néx\n"),
        ("wide-crlf", "# H\r\néx\r\n", "# H", "# Hy\r\néx\r\n"),
        ("wide-code", "    code\né\n", "    co", "    coyde\né\n"),
        ("wide-emoji", "# H\n🙂x\n", "# H", "# Hy\n🙂x\n"),
    ] {
        let mut f = Facade::new(name, text);
        f.click_after(after);
        f.type_text("y");
        assert_eq!(f.text(), edited);
    }
}

/// ADR-024: an input whose selection is of an older revision (the window
/// had not taken in the patch of the input before) drops that selection:
/// it applies at the backend's cursor, where the input before left it.
#[test]
fn adr024_a_stale_selection_is_dropped() {
    let mut f = Facade::new("stale-at", "Hello world.\n");
    f.click_after("Hello");
    let stale = f.at();
    f.type_text("X");
    assert_eq!(f.text(), "HelloX world.\n");
    // The window still sends the old revision, with the cursor at the start.
    let at = FacadeAt {
        rev: stale.rev,
        anchor: 0,
        head: 0,
    };
    assert_ne!(at.rev, f.window.rev);
    let patch = f.call(
        at,
        FacadeInput::Text {
            text: "Y".to_owned(),
        },
        None,
    );
    f.take(patch);
    assert_eq!(f.text(), "HelloXY world.\n");
    // With the revision the window has, its selection is taken in.
    f.click(0);
    f.type_text("Z");
    assert_eq!(f.text(), "ZHelloXY world.\n");
}

/// ADR-024: many edits in a row keep the window in step (checked in `take`).
#[test]
fn adr024_a_run_of_edits_keeps_the_window_in_step() {
    let note = "# A\n\npara *one* two\n\n- x\n- y\n\n1. a\n2. b\n\n> q\n\n```\nc\n```\n\n| a |\n|---|\n| 1 |\n\nend\n";
    let mut f = Facade::new("run", note);
    for needle in ["one", "- y", "2. b", "> q", "c\n", "| 1", "end", "# A"] {
        if !f.text().contains(needle) {
            continue;
        }
        f.click_after(needle);
        f.type_text("Z");
        f.key(KeyName::Enter);
        f.type_text("w");
        f.key(KeyName::Backspace);
        f.key(KeyName::Backspace);
        f.key(KeyName::Delete);
        f.key(KeyName::ArrowLeft);
        f.key(KeyName::Backspace);
    }
}

#[test]
fn m11_2_vim_j_and_k_move_by_the_rows_the_window_measured() {
    let mut f = Facade::new("vim-rows", "A long paragraph.\n\nNext.\n");
    f.mode("Vim");
    f.click_after("A long");
    let text = f.text();
    let next = u32::try_from(text.find("Next").unwrap()).unwrap();
    let head = f.window.head;
    // On the paragraph's last row: `j` goes into the next block, at the same index.
    let rows = RowsView {
        at: head,
        up: Some(1),
        down: None,
        start: 0,
        end: 17,
    };
    let patch = f.input_with(
        FacadeInput::Text {
            text: "j".to_owned(),
        },
        Some(rows),
    );
    assert_eq!(patch.head, next + 5);
    // Back up with a row measured above in the same block.
    let back = RowsView {
        at: patch.head,
        up: None,
        down: None,
        start: next,
        end: next + 5,
    };
    let patch = f.input_with(
        FacadeInput::Text {
            text: "k".to_owned(),
        },
        Some(back),
    );
    assert_eq!(patch.head, 5);
}

#[test]
fn m09_12_asciidoc_backspace_and_enter_keep_the_window_in_step() {
    let mut f = Facade::adoc("adoc-edit", "= Doc\n\n* one\n* two\n\nText *b* here.\n");
    f.click_after("one");
    f.key(KeyName::Enter);
    assert!(f.text().contains("* one\n* \n* two"), "{:?}", f.text());
    f.type_text("new");
    f.click_after("Text");
    f.key(KeyName::Backspace);
    f.key(KeyName::Enter);
    assert!(f.text().contains("Tex\n\n *b* here."), "{:?}", f.text());
}

/// ADR-024 measuring point, the editor core's part (ADR-025: what the
/// window's WASM runs, natively here): the time of `facade_input` alone for typing, Enter and Backspace at the ends of paragraphs at 2 %,
/// 50 % and 98 % of a long note. Run with the spike's 10,000-line note:
/// `UL_NOTE=/path/Home.md cargo test -p underline-editor-view --release
/// facade_input_timing -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, run by hand"]
fn adr024_facade_input_timing() {
    let Ok(path) = std::env::var("UL_NOTE") else {
        return;
    };
    let note = std::fs::read_to_string(path).unwrap();
    let mut f = Facade::new("timing", &note);
    let paragraphs: Vec<usize> = f
        .window
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| matches!(b.kind, crate::facade_view::BlockKindView::Paragraph))
        .map(|(i, _)| i)
        .collect();
    let n = f.window.blocks.len();
    for (name, fraction) in [("end", 0.98), ("middle", 0.5), ("start", 0.02)] {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let target = (n as f64 * fraction) as usize;
        let index = *paragraphs.iter().find(|&&i| i >= target).unwrap();
        let block = &f.window.blocks[index];
        let end = block.start + block.runs.last().map_or(0, |r| r.end);
        f.click(end);
        let times = |f: &mut Facade, input: FacadeInput| {
            let at = f.at();
            let start = Instant::now();
            let patch = f.call(at, input, None);
            let took = start.elapsed().as_secs_f64() * 1000.0;
            let json = serde_json::to_string(&patch).unwrap().len();
            // The window's side, without the check against a full parse.
            for change in &patch.blocks {
                let from = change.from as usize;
                let mut after = f.window.blocks.split_off(from + change.removed as usize);
                f.window.blocks.truncate(from);
                f.window.blocks.extend(change.blocks.iter().cloned());
                for b in &mut after {
                    b.start = u32::try_from(i64::from(b.start) + i64::from(change.shift)).unwrap();
                    b.line =
                        u32::try_from(i64::from(b.line) + i64::from(change.line_shift)).unwrap();
                }
                f.window.blocks.extend(after);
                f.window.version = change.version;
            }
            f.window.rev = patch.rev;
            f.window.anchor = patch.anchor;
            f.window.head = patch.head;
            (took, json)
        };
        let mut typed = Vec::new();
        let mut size = 0;
        for i in 0..300 {
            let c = "the quick brown fox *jumps* over the lazy_dog, "
                .chars()
                .nth(i % 47)
                .unwrap();
            let (t, s) = times(
                &mut f,
                FacadeInput::Text {
                    text: c.to_string(),
                },
            );
            typed.push(t);
            size = s;
        }
        let key = |key| FacadeInput::Key {
            key,
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
        let mut enter = Vec::new();
        for _ in 0..30 {
            enter.push(times(&mut f, key(KeyName::Enter)).0);
            enter.push(times(&mut f, key(KeyName::Backspace)).0);
        }
        let mut back = Vec::new();
        for _ in 0..100 {
            back.push(times(&mut f, key(KeyName::Backspace)).0);
        }
        let q = |v: &mut Vec<f64>, p: f64| {
            v.sort_by(f64::total_cmp);
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let at = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len()) - 1;
            v[at]
        };
        println!(
            "{name:6} block {index:5}: type median {:.2} p99 {:.2} ms (patch {size} bytes); enter/backspace median {:.2} p99 {:.2}; backspace median {:.2} p99 {:.2}",
            q(&mut typed, 0.5),
            q(&mut typed, 0.99),
            q(&mut enter, 0.5),
            q(&mut enter, 0.99),
            q(&mut back, 0.5),
            q(&mut back, 0.99),
        );
    }
}

/// TC-005-42, FR-098, UC-005 BR-008: a block formula and an inline one are
/// edited as their source; the window's blocks stay those of a full parse,
/// and Backspace and the arrows walk through the markup.
#[test]
fn tc005_42_editing_a_formula_in_the_facade() {
    let mut f = Facade::new("tc005-42", "$$a^2$$\n\nThe inline formula $b$ here.\n");
    f.click_after("a^2");
    f.type_text("+b^2");
    assert!(f.text().starts_with("$$a^2+b^2$$\n"), "{:?}", f.text());
    let crate::facade_view::BlockKindView::Formula { formula } = &f.window.blocks[0].kind else {
        panic!("{:?}", f.window.blocks[0].kind);
    };
    assert_eq!(formula.source, "a^2+b^2", "the formula follows the change");
    f.click_after("$b");
    f.type_text("\\frac{");
    assert!(f.text().contains("$b\\frac{$ here."), "{:?}", f.text());
    f.key(KeyName::Backspace);
    f.key(KeyName::ArrowLeft);
    f.type_text("_x");
    assert!(f.text().contains("$b\\fra_xc$ here."), "{:?}", f.text());
    // Right past the closing `$` is outside the formula: escaped as text.
    f.key(KeyName::ArrowRight);
    f.key(KeyName::ArrowRight);
    f.type_text("_");
    assert!(f.text().contains("$b\\fra_xc$\\_ here."), "{:?}", f.text());
}

/// NFR-002, FR-098, ADR-029 measuring point: the editor core's time per key
/// (`facade_input` with its views, natively; the window's WASM runs the
/// same) typing next to a formula, inside an inline formula, inside a block
/// formula, and a `$`, in a note of 300 sections with formulas. Run with
/// `cargo test -p underline-editor-view --release formula_typing_timing --
/// --ignored --nocapture`.
#[test]
#[ignore = "a measurement, run by hand"]
fn nfr002_formula_typing_timing() {
    let section = "## Section\n\nText with $E=mc^2$ and $$x^2$$ and a [link](x.md).\n\n$$\n\\sum_{i=1}^n i\n$$\n\n- an item $a_i$\n\n";
    let note = format!("# Title\n\n{}", section.repeat(300));
    let mut f = Facade::new("timing", &note);
    let quantile = |v: &mut Vec<f64>, p: f64| {
        v.sort_by(f64::total_cmp);
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let at = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len()) - 1;
        v[at]
    };
    let middle = note.len() / 2;
    let around = |needle: &str| {
        let byte = middle + note[middle..].find(needle).unwrap() + needle.len();
        u32::try_from(note[..byte].encode_utf16().count()).unwrap()
    };
    // The same note without formulas, as before FR-098.
    let plain = note.replace('$', "S");
    for (name, text, at, typed) in [
        ("without formulas", &plain, around("Text with "), "w"),
        ("next to a formula", &note, around("Text with "), "w"),
        ("in an inline formula", &note, around("$E=mc"), "c"),
        ("in a block formula", &note, around("\\sum_{i=1}^n"), "i"),
        ("a dollar", &note, around("and a "), "$"),
    ] {
        let mut f2 = Facade::new("timing", text);
        std::mem::swap(&mut f, &mut f2);
        f.click(at);
        let mut times = Vec::new();
        for _ in 0..200 {
            let at = f.at();
            let start = Instant::now();
            let patch = f.call(
                at,
                FacadeInput::Text {
                    text: typed.to_owned(),
                },
                None,
            );
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            f.window.rev = patch.rev;
            f.window.anchor = patch.anchor;
            f.window.head = patch.head;
            for change in &patch.blocks {
                f.window.version = change.version;
            }
        }
        println!(
            "NFR-002 {name}: median {:.3} ms, p99 {:.3} ms per key",
            quantile(&mut times, 0.5),
            quantile(&mut times, 0.99)
        );
    }
}

/// Q-46, M-11.3: Nano's Ctrl+K on the last block cuts it to the end of the
/// text and leaves the cursor at the start of the block before; Ctrl+U
/// pastes there. The egui facade does the same: its Nano keys are the
/// editor core's (`underline_editor_core::keys`) and its `whole_line` is
/// this one, so this is parity, kept until Q-46 is decided.
#[test]
fn q46_nano_ctrl_k_on_the_last_block_as_in_egui() {
    let mut f = Facade::new("q46", "One.\n\nTwo.\n\nThree.\n");
    f.mode("Nano");
    f.click_after("Thr");

    let cut = f.key_with(KeyName::K, true, false);
    assert_eq!(f.text(), "One.\n\nTwo.\n\n");
    assert_eq!(cut.place, "Ln 3, Col 1");
    f.key_with(KeyName::U, true, false);
    assert_eq!(f.text(), "One.\n\nThree.\nTwo.\n\n");
}

impl Facade {
    /// Puts `insertion` in at the window's selection; the patch is taken in.
    fn insert(&mut self, insertion: Insertion) -> FacadePatch {
        let at = self.at();
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: "Home",
            at,
            mac: false,
            earlier,
            views: &mut self.views,
        };
        let patch = self.tab.facade_insert(call, insertion).patch;
        self.take(patch)
    }
}

/// UC-005 A17, M-09.14 (TC-005-26): HTML of other programs comes as the
/// markup the backend made of it; Ctrl+Shift+V types the plain text; HTML
/// that kept nothing pastes nothing.
#[test]
fn uc005_a17_m09_14_html_pastes_as_markup() {
    let mut f = Facade::new("html", "One\n\nTwo\n");
    f.click_after("Two");
    let content = ClipContentView {
        text: Some("Install\nUse bold".into()),
        markup: Some("## Install\n\nUse **bold**".into()),
        image: false,
    };
    let clipped = f.clipboard_with(f.at(), ClipAction::Paste, false, Some(&content));
    assert!(!clipped.image);
    f.take(clipped.handled.patch);
    // On lines of its own, as egui pastes markup of several lines.
    assert_eq!(f.text(), "One\n\nTwo\n\n## Install\n\nUse **bold**\n\n");

    let mut plain = Facade::new("plain", "One\n");
    plain.click_after("One");
    let clipped = plain.clipboard_with(plain.at(), ClipAction::Paste, true, Some(&content));
    plain.take(clipped.handled.patch);
    assert!(plain.text().starts_with("OneInstall"), "{:?}", plain.text());

    let mut empty = Facade::new("empty", "One\n");
    empty.click_after("One");
    let nothing = ClipContentView {
        text: Some("x".into()),
        markup: Some(String::new()),
        image: false,
    };
    let clipped = empty.clipboard_with(empty.at(), ClipAction::Paste, false, Some(&nothing));
    empty.take(clipped.handled.patch);
    assert_eq!(empty.text(), "One\n");
}

/// UC-005 BR-011, M-09.14: copying in the facade gives the markup and the
/// HTML of the selection; a paste of an image alone is left to the backend.
#[test]
fn uc005_br011_m10_6_facade_copy_gives_html_and_an_image_is_left() {
    let mut f = Facade::new("copy", "Use **bold** here\n");
    let at = FacadeAt {
        rev: f.window.rev,
        anchor: 0,
        head: 12,
    };
    let clipped = f.clipboard_with(at, ClipAction::Copy, false, None);
    assert_eq!(clipped.copied.as_deref(), Some("Use **bold**"));
    assert_eq!(
        clipped.html.as_deref(),
        Some("<p>Use <strong>bold</strong></p>")
    );
    f.take(clipped.handled.patch);

    f.click_after("here");
    let picture = ClipContentView {
        text: None,
        markup: None,
        image: true,
    };
    let clipped = f.clipboard_with(f.at(), ClipAction::Paste, false, Some(&picture));
    assert!(clipped.image);
    f.take(clipped.handled.patch);
    assert_eq!(f.text(), "Use **bold** here\n");
}

/// UC-005 A2, A9 step 2, M-10.3, M-10.5: the dialogs' answers and dropped
/// files go in at the facade's cursor; the blocks follow.
#[test]
fn uc005_a2_a9_m10_3_m10_5_insertions_in_the_facade() {
    let mut f = Facade::new("insert", "Read this\n\n```\ncode\n```\n");
    f.click_after("Read");
    f.insert(Insertion::Link {
        address: "https://x.org".into(),
        label: None,
    });
    assert!(
        f.text()
            .starts_with("Read[https://x.org](https://x.org) this"),
        "{:?}",
        f.text()
    );

    f.click_after("code");
    f.insert(Insertion::Dropped {
        place: "a b.png".into(),
        image: true,
    });
    f.insert(Insertion::Text { text: " ".into() });
    assert!(f.text().contains("codea b.png \n```"), "{:?}", f.text());

    let mut g = Facade::new("image", "Text\n");
    g.click_after("Text");
    g.insert(Insertion::Dropped {
        place: "photo.png".into(),
        image: true,
    });
    assert_eq!(g.text(), "Text\n\n![](photo.png)\n");
}

impl Facade {
    /// The window's cursor at the end of the text.
    fn cursor_at_end(&mut self) {
        let end = u32::try_from(self.text().encode_utf16().count()).unwrap();
        self.window.anchor = end;
        self.window.head = end;
    }

    /// FR-080: the place of a link typed at the window's selection.
    fn link_context(&mut self, manual: bool) -> Option<crate::editor_view::LinkContextView> {
        let at = self.at();
        self.tab.facade_link_context("Home", at, manual)
    }

    /// FR-080: the window chose `choice`; the patch is taken in.
    fn complete(&mut self, choice: &crate::editor_view::LinkChoice) -> Option<FacadePatch> {
        let at = self.at();
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: "Home",
            at,
            mac: false,
            earlier,
            views: &mut self.views,
        };
        let patch = self.tab.facade_complete(call, choice)?.patch;
        Some(self.take(patch))
    }
}

fn link_choice(rev: u32, note: &str, format: &str, label: &str) -> crate::editor_view::LinkChoice {
    crate::editor_view::LinkChoice {
        rev,
        note: Some(note.to_owned()),
        format: Some(format.to_owned()),
        anchor: None,
        label: label.to_owned(),
    }
}

/// TC-005-33 step 4, BR-018: `[[plan` in the facade, Enter: a link to
/// "Plan" with the text "Plan" replaces it; `[[` is not in the note, and the
/// window's blocks follow.
#[test]
fn tc_005_33_step_4_facade_brackets_become_a_normal_link() {
    let mut facade = Facade::new("complete", "# Intro\n\nGo ");
    facade.cursor_at_end();
    facade.type_text("[[plan");
    assert!(
        facade.text().ends_with("Go \\[\\[plan"),
        "{}",
        facade.text()
    );

    let context = facade.link_context(false).unwrap();
    assert_eq!(context.typed, "plan");
    assert_eq!(context.named.as_deref(), Some("plan"));

    let choice = link_choice(context.rev, "Plan", "Markdown", "Plan");
    let patch = facade.complete(&choice).unwrap();

    assert_eq!(facade.text(), "# Intro\n\nGo [Plan](Plan.md)");
    assert!(!facade.text().contains("[["));
    assert_eq!(patch.head as usize, "# Intro\n\nGo [Plan](Plan.md)".len());
    assert!(facade.link_context(false).is_none());
}

/// TC-005-33 step 4 in an AsciiDoc note: the toolbar's `link:` form.
#[test]
fn tc_005_33_step_4_facade_asciidoc_link() {
    let mut facade = Facade::adoc("complete-adoc", "== Intro\n\nGo ");
    facade.cursor_at_end();
    facade.type_text("[[pla");
    let context = facade.link_context(false).unwrap();
    assert_eq!(context.typed, "pla");

    facade
        .complete(&link_choice(context.rev, "travel/Plan", "AsciiDoc", "Plan"))
        .unwrap();

    assert_eq!(facade.text(), "== Intro\n\nGo link:travel/Plan.adoc[Plan]");
}

/// TC-005-34 step 1, UC-005 A24: without a choice `[[` stays as typed;
/// Ctrl+Space completes at the cursor without it.
#[test]
fn tc_005_34_step_1_facade_brackets_stay_without_a_choice() {
    let mut facade = Facade::new("dismiss", "Text");
    facade.key_with(KeyName::End, true, false);
    facade.type_text(" [[");
    assert!(facade.link_context(false).is_some());
    assert_eq!(facade.text(), "Text \\[\\[");

    let mut facade = Facade::new("manual", "Text");
    facade.key_with(KeyName::End, true, false);
    assert!(facade.link_context(false).is_none());
    let context = facade.link_context(true).unwrap();
    assert_eq!(context.typed, "");
    facade
        .complete(&link_choice(context.rev, "Plan", "Markdown", "Plan"))
        .unwrap();
    assert_eq!(facade.text(), "Text[Plan](Plan.md)");
}

impl Facade {
    fn alt_key(&mut self, key: KeyName) -> FacadePatch {
        self.input(FacadeInput::Key {
            key,
            ctrl: false,
            alt: true,
            shift: false,
            meta: false,
        })
    }

    fn go_to_line(&mut self, line: u32) -> FacadePatch {
        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
        let call = FacadeCall {
            note: "Home",
            at: self.at(),
            mac: false,
            earlier,
            views: &mut self.views,
        };
        let patch = self
            .tab
            .facade_go_to_line(
                call,
                crate::editor_view::LineTargetView {
                    line,
                    column: Some(9),
                },
            )
            .patch;
        self.take(patch)
    }
}

/// TC-005-36 steps 4 to 6, UC-005 A26, BR-020: in the facade a block moves
/// as a whole, a list item with its items; a paragraph of two lines is
/// copied whole; the first block does not move up. Each is one undo step.
#[test]
fn tc_005_36_blocks_move_and_are_copied_in_the_facade() {
    let note = "one\ntwo\nthree\n\nfour\nfive\n\n- a\n  - a1\n- b\n";
    let mut f = Facade::new("Lines", note);
    f.click_after("- a");
    f.alt_key(KeyName::ArrowDown);
    assert_eq!(
        f.text(),
        "one\ntwo\nthree\n\nfour\nfive\n\n- b\n- a\n  - a1\n"
    );
    f.click_after("fou");
    f.key_with(KeyName::D, true, true);
    assert_eq!(
        f.text(),
        "one\ntwo\nthree\n\nfour\nfive\n\nfour\nfive\n\n- b\n- a\n  - a1\n"
    );
    f.click_after("on");
    let still = f.alt_key(KeyName::ArrowUp);
    assert!(still.blocks.is_empty());
    assert_eq!(
        f.text(),
        "one\ntwo\nthree\n\nfour\nfive\n\nfour\nfive\n\n- b\n- a\n  - a1\n"
    );
    f.key_with(KeyName::Z, true, false);
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), note);
}

/// BR-020: moving up past a list item takes the item with its items; the
/// palette's action is the key's.
#[test]
fn br020_moving_up_past_an_item_with_its_items() {
    let mut f = Facade::new("Lines", "- a\n  - a1\n- b\n\nAfter\n");
    f.click_after("- b");
    f.alt_key(KeyName::ArrowUp);
    assert_eq!(f.text(), "- b\n- a\n  - a1\n\nAfter\n");
    f.click_after("Aft");
    f.input(FacadeInput::Edit {
        action: crate::editor_view::EditActionName::MoveLineUp,
    });
    // OQ-255: a blank line keeps "After" from joining the item "b".
    assert_eq!(f.text(), "- b\n\nAfter\n\n- a\n  - a1\n");
}

/// TC-005-37 steps 1 to 4, UC-005 A27, BR-021: Tab and Shift+Tab in the
/// facade change the markup of the list as in the source view.
#[test]
fn tc_005_37_list_items_are_indented_in_the_facade() {
    let mut f = Facade::new("List", "- a\n- b\n- c\n");
    f.click_after("b");
    f.key(KeyName::Tab);
    assert_eq!(f.text(), "- a\n  - b\n- c\n");
    let b = u32::try_from("- a\n  - ".len()).unwrap();
    let c = u32::try_from("- a\n  - b\n- c".len()).unwrap();
    let rev = f.window.rev;
    f.tab.select(rev, b, c);
    f.window.anchor = b;
    f.window.head = c;
    f.key(KeyName::Tab);
    assert_eq!(f.text(), "- a\n  - b\n  - c\n");
    f.key_with(KeyName::Tab, false, true);
    f.key_with(KeyName::Tab, false, true);
    assert_eq!(f.text(), "- a\n- b\n- c\n");
    f.click_after("- a");
    f.key(KeyName::Tab);
    assert_eq!(f.text(), "- a\n- b\n- c\n", "nothing before a");
    let mut adoc = Facade::adoc("List", "* a\n* b\n");
    adoc.click_after("* b");
    adoc.key(KeyName::Tab);
    assert_eq!(adoc.text(), "* a\n** b\n");
    let mut plain = Facade::new("Plain", "Text\n");
    plain.click_after("Te");
    plain.key(KeyName::Tab);
    assert_eq!(plain.text(), "Text\n", "Tab does nothing outside a list");
}

/// UC-005 A25, BR-019: brackets and quotes close in the facade, written as
/// the facade writes them; the closing one is typed over, and Backspace
/// takes both; a selection within a block is enclosed.
#[test]
fn br019_close_pairs_in_the_facade() {
    let mut f = Facade::new("Pairs", "Text\n");
    f.click_after("Text");
    f.type_text("[");
    assert_eq!(f.text(), "Text\\[\\]\n", "escaped as the facade types");
    f.type_text("x");
    f.type_text("]");
    f.type_text("(");
    assert_eq!(f.text(), "Text\\[x\\]()\n");
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "Text\\[x\\]\n");
    f.type_text("é");
    assert_eq!(f.text(), "Text\\[x\\]é\n", "after the closing bracket");

    let mut selected = Facade::new("Pairs", "a wörd b\n");
    let rev = selected.window.rev;
    selected.tab.select(rev, 2, 6);
    selected.window.anchor = 2;
    selected.window.head = 6;
    let patch = selected.type_text("\"");
    assert_eq!(selected.text(), "a \"wörd\" b\n");
    assert_eq!((patch.anchor, patch.head), (3, 7));

    let mut off = Facade::new("Pairs", "Text\n");
    off.tab.set_close_pairs(false);
    off.click_after("Text");
    off.type_text("(");
    assert_eq!(off.text(), "Text(\n");
}

/// TC-004-33 step 3, UC-004 BR-017: in the facade the cursor goes to the
/// block that holds the line, at its column (here 9) as the facade shows
/// it, at the end of a shorter line.
#[test]
fn tc_004_33_go_to_line_in_the_facade() {
    let mut f = Facade::new("Long", "# Results\n\nfirst line\nsecond line\n\n- item\n");
    let patch = f.go_to_line(4);
    assert_eq!(patch.place, "Ln 4, Col 9", "the column, 9");
    assert!(patch.scroll);
    let item = f.go_to_line(6);
    assert_eq!(item.place, "Ln 6, Col 7", "beyond the line: its end");
    let past = f.go_to_line(99);
    assert_eq!(past.place, "Ln 6, Col 7", "the end of the last block");
}

/// The chars of `text`, sorted: what a move of blocks must keep.
fn chars_sorted(text: &str) -> Vec<char> {
    let mut chars: Vec<char> = text.chars().collect();
    chars.sort_unstable();
    chars
}

/// BR-020, UC-005 A26 (test-monkey): an AsciiDoc list continuation (`+`) that
/// stands right before the next list item gives the facade a block of no
/// text; moving the item with Alt+Down, or the item after it with Alt+Up,
/// panicked in `lines::swap` (the two ranges of lines overlapped) instead of
/// moving the blocks.
#[test]
fn br020_asciidoc_continuation_before_an_item_does_not_panic_when_moving() {
    let note = "* a\n+\n* b\n";
    for (click, key) in [(0, KeyName::ArrowDown), (7, KeyName::ArrowUp)] {
        let mut f = Facade::adoc("Lines", note);
        f.click(click);
        f.alt_key(key);
        assert_eq!(chars_sorted(&f.text()), chars_sorted(note), "{key:?}");
    }
}

/// BR-020 (test-monkey): the paragraph of a list item that follows a `+`
/// belongs to it; Alt+Up on it must not tear the `+` line off its paragraph
/// (`* a\n+\ncont` became `cont\n+\n* a`, the `+` now joins nothing).
#[test]
fn br020_asciidoc_continuation_paragraph_keeps_its_plus_line() {
    let mut f = Facade::adoc("Lines", "* a\n+\ncont\n");
    f.click(10);
    f.alt_key(KeyName::ArrowUp);
    assert!(
        f.text().contains("+\ncont"),
        "the plus line stays right before its paragraph: {:?}",
        f.text()
    );
}

/// BR-020, NFR-007 (test-monkey): a block that moves stays the block it
/// was. `para` next to a list item without a blank line between them moved
/// below `- a` becomes the lazy continuation of that item (`- a\npara`), and
/// can no longer move back.
#[test]
fn br020_a_moved_paragraph_does_not_become_part_of_the_list_item() {
    let mut f = Facade::new("Lines", "para\n- a\n");
    f.click(0);
    f.alt_key(KeyName::ArrowDown);
    let (blocks, _) = parse_all(&f.text(), NoteFormat::Markdown);
    assert!(
        blocks
            .iter()
            .any(|block| format!("{:?}", block.kind).starts_with("Paragraph")),
        "the paragraph is still a block of its own in {:?}",
        f.text()
    );
}

/// BR-019 (test-monkey): `(` typed on the empty line between two blocks
/// writes `()` as one paragraph; the window must show the closing bracket
/// too (it kept `(` while the text held `()`: the patch of the second write
/// was lost).
#[test]
fn br019_a_pair_typed_between_blocks_keeps_the_window_in_step() {
    let mut f = Facade::new("Pairs", "# H\n\npara\n");
    f.click(4);
    f.type_text("(");
    assert_eq!(f.text(), "# H\n()\npara\n");
}

/// NFR-007, BR-020 (review): a copy in a CRLF note keeps its line breaks.
#[test]
fn br020_a_copy_in_a_crlf_note_keeps_its_line_breaks() {
    let mut f = Facade::new("Crlf", "one\r\n\r\n- a\r\n- b\r\n");
    f.click_after("on");
    f.key_with(KeyName::D, true, true);
    assert_eq!(f.text(), "one\r\n\r\none\r\n\r\n- a\r\n- b\r\n");
    f.click_after("- a");
    f.key_with(KeyName::D, true, true);
    assert_eq!(f.text(), "one\r\n\r\none\r\n\r\n- a\r\n- a\r\n- b\r\n");
    assert!(!f.text().replace("\r\n", "").contains('\n'));
}

/// BR-019 (review): `*`, `_` and `~` enclose a selection in the facade as
/// markup, as in the source view, one step for undo; in code they replace
/// it as typed text does.
#[test]
fn br019_markup_encloses_a_selection_in_the_facade() {
    let mut f = Facade::new("Pairs", "a word b\n");
    let rev = f.window.rev;
    f.tab.select(rev, 2, 6);
    f.window.anchor = 2;
    f.window.head = 6;
    let patch = f.type_text("*");
    assert_eq!(f.text(), "a *word* b\n");
    assert_eq!((patch.anchor, patch.head), (3, 7));
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), "a word b\n");
}

/// BR-019 (review): Backspace right after a pair typed between two blocks
/// takes both and keeps the window in step.
#[test]
fn br019_backspace_on_a_pair_between_blocks_keeps_the_window_in_step() {
    let mut f = Facade::new("Pairs", "# H\n\npara\n");
    f.click(4);
    f.type_text("(");
    f.key(KeyName::Backspace);
    assert_eq!(f.text(), "# H\n\npara\n");
}

/// TC-024-01 steps 2 and 3, TC-024-12, OQ-216: a change of a note attribute
/// in the facade shows in the front matter's table at once, keeps the
/// cursor where it was, and is undone in one step.
#[test]
fn tc024_01_attribute_changed_in_the_facade() {
    use crate::editor_view::{AttributeChangeView, AttributeValueView};
    let plan = "---\nstatus: draft\ndue: 2026-12-01\n---\n# Plan\n\nText\n";
    let mut f = Facade::new("plan", plan);
    f.click_after("Text");
    let head = f.window.head;
    let at = f.at();
    let earlier = f.tab.bring_facade_up_to_date(&mut f.views);
    let call = FacadeCall {
        note: "Home",
        at,
        mac: false,
        earlier,
        views: &mut f.views,
    };
    let edited = f.tab.facade_attribute(
        call,
        AttributeChangeView::Set {
            name: "status".into(),
            value: AttributeValueView::Text {
                text: "final".into(),
            },
        },
    );
    assert_eq!(edited.problem, None);
    f.take(edited.patch.unwrap());
    assert_eq!(f.text(), plan.replace("draft", "final"));
    assert_eq!(f.window.head, head, "the cursor stays after Text");
    let table = format!("{:?}", f.window.blocks[0].kind);
    assert!(table.contains("final"), "{table}");
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), plan);
}

/// ADR-024 (found by the monkey of package 7a, exists at the commit before
/// it): typing at the end of the first line of a paragraph of two lines
/// leaves the window's blocks with other runs than a full parse gives
/// (`"ab "`, the typed text with the line break, not `"talab"` and `" "`).
#[test]
fn adr024_typing_at_the_end_of_a_line_of_a_paragraph_gives_the_runs_of_a_full_parse() {
    let mut f = Facade::new("Home", "tal\nmore\n");
    f.tab.editor.set_selection(3, 3);
    let (head, anchor) = f.tab.editor.selection_web();
    f.window.anchor = u32::try_from(anchor).unwrap();
    f.window.head = u32::try_from(head).unwrap();
    // `take` checks the window's blocks against a full parse.
    f.type_text("ab");
    assert_eq!(f.text(), "talab\nmore\n");
}

/// UC-005 A28 step 2, TC-005-39 step 3: in the facade the correction takes
/// the place of the marked word (the window selects its markup) as one
/// change; Ctrl+Z brings the word back.
#[test]
fn tc005_39_a_correction_is_one_undo_step_in_the_facade() {
    let mut f = Facade::new("spell", "The **ferri** leaves\n");
    f.click_after("The **");
    f.type_text("x");
    f.key(KeyName::Backspace);
    let text = f.text();
    let from = u32::try_from(text.find("ferri").unwrap()).unwrap();
    let rev = f.window.rev;
    f.tab.select(rev, from, from + 5);
    f.window.anchor = from;
    f.window.head = from + 5;
    f.insert(Insertion::Correction {
        text: "ferry".into(),
    });
    assert_eq!(f.text(), "The **ferry** leaves\n");
    f.key_with(KeyName::Z, true, false);
    assert_eq!(f.text(), "The **ferri** leaves\n");
}

/// Test monkey 8b, UC-005 A28 step 2, TC-005-39: random single-line
/// selections with markup in the facade: the markup text is the old one with
/// the selection replaced by the correction, Ctrl+Z gives back the old text.
#[test]
fn tc005_39_chaos_a_correction_in_the_facade_replaces_the_selection_and_undoes() {
    let pieces = [
        "ferri",
        " ",
        "**",
        "*",
        "_",
        "`x`",
        "[a](http://b.example)",
        "😀",
        "e\u{301}",
        "שלום",
        "日本語",
        "~~",
        "\\*",
        "lne",
    ];
    let corrections = ["ferry", "don't", "well-known", "x", "", "é", "Underline"];
    for seed in 1..300u64 {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let mut body = String::new();
        for _ in 0..(2 + next() % 10) {
            body.push_str(pieces[(next() % pieces.len() as u64) as usize]);
        }
        body.push('\n');
        let mut f = Facade::new("spell", &body);
        let text = f.text();
        let units: Vec<u16> = text.encode_utf16().collect();
        let body_len = units.len() - 1;
        let a = (next() % (body_len as u64 + 1)) as usize;
        let b = (next() % (body_len as u64 + 1)) as usize;
        let (lo, hi) = (a.min(b), a.max(b));
        if hi == lo || String::from_utf16(&units[lo..hi]).is_err() {
            continue;
        }
        if String::from_utf16(&units[..lo]).is_err() || String::from_utf16(&units[hi..]).is_err() {
            continue;
        }
        let correction = corrections[(next() % corrections.len() as u64) as usize];
        let rev = f.window.rev;
        let (from, to) = (u32::try_from(lo).unwrap(), u32::try_from(hi).unwrap());
        f.tab.select(rev, from, to);
        f.window.anchor = from;
        f.window.head = to;
        let expected = String::from_utf16(&units[..lo]).unwrap()
            + correction
            + &String::from_utf16(&units[hi..]).unwrap();
        f.insert(Insertion::Correction {
            text: correction.into(),
        });
        assert_eq!(
            f.text(),
            expected,
            "seed {seed}: {text:?} {lo}..{hi} -> {correction:?}"
        );
        f.key_with(KeyName::Z, true, false);
        assert_eq!(
            f.text(),
            text,
            "seed {seed}: undo of {correction:?} in {text:?} {lo}..{hi}"
        );
    }
}
