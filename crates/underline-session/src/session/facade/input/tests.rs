//! Tests of editing in the facade (UC-005, M-09.10 to M-09.14, M-11.2,
//! M-11.3, ADR-024): the rules of each kind of block, the empty lines after
//! Enter, joining blocks, the defects D-17 and D-18, CRLF and emoji, the
//! one history of both views, the Vim and Nano keys with a block as a line,
//! and that the window, applying the patches, always holds the blocks a
//! full parse gives.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};

use super::super::build::views_of;
use crate::editor_view::{ClipAction, KeyName};
use crate::facade_view::{BlockView, FacadeAt, FacadeInput, FacadePatch, GapView, RowsView};
use crate::{Session, SettingName, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-facade-input-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on a workspace whose only note `Home.md` (or `Home.adoc`)
/// holds `text`, opened in a tab.
fn session_with(dir: &Path, file: &str, text: &str) -> Session {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(file), text).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
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
    session.take_events();
    session
}

fn home() -> NoteId {
    NoteId::from_relative("Home")
}

/// What the window holds of the facade, as `FacadeView.svelte` keeps it.
struct Window {
    rev: u32,
    version: u32,
    blocks: Vec<BlockView>,
    anchor: u32,
    head: u32,
    gap: Option<GapView>,
}

/// The facade of a note, opened in the window.
struct Facade {
    session: Session,
    window: Window,
    adoc: bool,
}

impl Facade {
    fn new(name: &str, text: &str) -> Self {
        Self::with_file(name, "Home.md", text)
    }

    fn adoc(name: &str, text: &str) -> Self {
        Self::with_file(name, "Home.adoc", text)
    }

    fn with_file(name: &str, file: &str, text: &str) -> Self {
        let mut session = session_with(&temp_dir(name), file, text);
        let state = session.facade_state(&home()).unwrap();
        Self {
            session,
            window: Window {
                rev: state.rev,
                version: state.version,
                blocks: state.blocks,
                anchor: state.anchor,
                head: state.head,
                gap: state.gap,
            },
            adoc: file.ends_with(".adoc"),
        }
    }

    /// The note's text as it is in the backend, its line breaks kept.
    fn text(&self) -> String {
        let open = self.session.open.as_ref().unwrap();
        let index = open.tabs.find(&home()).unwrap();
        open.tabs.get(index).editor.text()
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
        let format = if self.adoc {
            underline_core::NoteFormat::AsciiDoc
        } else {
            underline_core::NoteFormat::Markdown
        };
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
            strip(&views_of(&text, format)),
            "the window's blocks of {text:?}"
        );
    }

    fn input(&mut self, input: FacadeInput) -> FacadePatch {
        self.input_with(input, None)
    }

    fn input_with(&mut self, input: FacadeInput, rows: Option<RowsView>) -> FacadePatch {
        let at = self.at();
        let patch = self
            .session
            .facade_input(&home(), at, input, rows, None)
            .unwrap();
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
        self.session.editor_select(&home(), rev, at, at).unwrap();
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

    fn mode(&mut self, mode: &str) {
        self.session
            .quick_setting(SettingName::InputMode, mode.to_owned());
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
        .session
        .editor_input(
            &home(),
            crate::EditorInput::Key {
                key: KeyName::Z,
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
                page: 1,
            },
        )
        .unwrap();
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
    let (copied, patch) = f
        .session
        .facade_clipboard(&home(), at, ClipAction::Copy, false, None)
        .unwrap();
    f.take(patch);
    assert_eq!(copied.as_deref(), Some("One **two**"));
    f.click_after("Three");
    let multi = "- a\n- b";
    // Plain text is typed: escaped.
    let (_, patch) = f
        .session
        .facade_clipboard(&home(), f.at(), ClipAction::Paste, false, Some(" *x*"))
        .unwrap();
    f.take(patch);
    assert!(f.text().contains("Three \\*x\\*"), "{}", f.text());
    // The own markup comes as markup, on lines of its own when it has several.
    f.session.open.as_mut().unwrap().tabs.get_mut(0).own_markup = Some(multi.to_owned());
    let (_, patch) = f
        .session
        .facade_clipboard(&home(), f.at(), ClipAction::Paste, false, Some(multi))
        .unwrap();
    f.take(patch);
    // A blank line before and after it, as in egui.
    assert!(
        f.text().ends_with("Three \\*x\\*\n\n- a\n- b\n\n"),
        "{:?}",
        f.text()
    );
    // Ctrl+Shift+V types even the own markup.
    f.session.open.as_mut().unwrap().tabs.get_mut(0).own_markup = Some("*z*".to_owned());
    f.click_after("One");
    let (_, patch) = f
        .session
        .facade_clipboard(&home(), f.at(), ClipAction::Paste, true, Some("*z*"))
        .unwrap();
    f.take(patch);
    assert!(f.text().starts_with("One\\*z\\* **two**"), "{:?}", f.text());
    // Cutting takes the selection away.
    let at = FacadeAt {
        rev: f.window.rev,
        anchor: 0,
        head: 3,
    };
    let (cut, patch) = f
        .session
        .facade_clipboard(&home(), at, ClipAction::Cut, false, None)
        .unwrap();
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

/// ADR-024 measuring point, the backend's part: the time of `facade_input`
/// alone for typing, Enter and Backspace at the ends of paragraphs at 2 %,
/// 50 % and 98 % of a long note. Run with the spike's 10,000-line note:
/// `UL_NOTE=/path/Home.md cargo test -p underline-session --release
/// facade_input_timing -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, run by hand"]
fn adr024_facade_input_timing() {
    let Ok(path) = std::env::var("UL_NOTE") else {
        return;
    };
    let note = fs::read_to_string(path).unwrap();
    let mut f = Facade::new("timing", &note);
    let paragraphs: Vec<usize> = f
        .window
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| matches!(b.kind, crate::BlockKindView::Paragraph))
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
            let patch = f
                .session
                .facade_input(&home(), at, input, None, None)
                .unwrap();
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
