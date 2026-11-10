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
