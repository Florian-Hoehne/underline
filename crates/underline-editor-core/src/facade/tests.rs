//! Tests of the facade rules without a backend (UC-005, M-09.10 to M-09.13,
//! M-11.2, ADR-024, ADR-025): editing through [`FacadeEdit`] and the Vim
//! keys, which views [`refresh`] makes anew with a stand-in [`MakeViews`],
//! the splices for the window, and the carried colour spans. The same
//! rules are tested through the session with its real views in
//! `underline-session` (`session/facade/input/tests.rs`).

use std::collections::HashMap;
use std::ops::Range;

use ropey::Rope;
use underline_markup::blocks::Block;
use underline_markup::formatting::Format;
use underline_markup::NoteFormat;

use super::*;
use crate::keys::InputMode;
use crate::{Key, KeyInput, Keys, Modifiers, SourceEditor, Tool};

/// A stand-in for the backend's view of a block: where it starts, its
/// shown text, and colour spans to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct View {
    start: u32,
    line: u32,
    text: String,
    colors: Option<Vec<(u32, u32, u8)>>,
}

impl ShownBlock for View {
    fn start(&self) -> u32 {
        self.start
    }

    fn line(&self) -> u32 {
        self.line
    }

    fn is_moved(&self, old: &Self, shift: i64, line_shift: i64) -> bool {
        i64::from(old.start) + shift == i64::from(self.start)
            && i64::from(old.line) + line_shift == i64::from(self.line)
            && old.text == self.text
            && old.colors == self.colors
    }

    fn shift(&mut self, shift: i64, line_shift: i64) {
        self.start = u32::try_from(i64::from(self.start) + shift).unwrap();
        self.line = u32::try_from(i64::from(self.line) + line_shift).unwrap();
    }

    /// A shown `!` stands for a caption here.
    fn captioned(&self) -> bool {
        self.text.contains('!')
    }
}

/// Makes [`View`]s (the notes here are ASCII without `\r`, so a web unit is
/// a char) and records what it was asked.
#[derive(Debug, Default)]
struct Views {
    all: usize,
    some: Vec<Range<usize>>,
    moved: Vec<(Range<usize>, isize)>,
    made: Vec<(usize, bool)>,
    /// Kept per block, as the session keeps the last pictures.
    kept: HashMap<usize, &'static str>,
}

fn view(block: &Block, rope: &Rope) -> View {
    let start = block.span.start.min(rope.len_bytes());
    View {
        start: u32::try_from(rope.byte_to_char(start)).unwrap(),
        line: u32::try_from(rope.byte_to_line(start)).unwrap(),
        text: block.runs.iter().map(|run| run.text.as_str()).collect(),
        colors: None,
    }
}

impl MakeViews for Views {
    type View = View;

    fn all(&mut self, blocks: &[Block], _text: &str, rope: &Rope) -> Vec<View> {
        self.all += 1;
        blocks.iter().map(|block| view(block, rope)).collect()
    }

    fn some(
        &mut self,
        blocks: &[Block],
        range: Range<usize>,
        part: Part<'_>,
        rope: &Rope,
    ) -> Vec<View> {
        // The part holds the blocks, and starts where the first does.
        assert_eq!(part.from, blocks[range.start].span.start);
        assert_eq!(part.origin, rope.byte_to_char(part.from));
        assert_eq!(
            part.text,
            rope.byte_slice(part.from..part.from + part.text.len())
                .to_string()
        );
        self.some.push(range.clone());
        blocks[range]
            .iter()
            .map(|block| view(block, rope))
            .collect()
    }

    fn blocks_moved(&mut self, replaced: Range<usize>, grown: isize) {
        self.moved.push((replaced.clone(), grown));
        self.kept = moved_indices(std::mem::take(&mut self.kept), replaced, grown);
    }

    fn made(&mut self, index: usize, view: &mut View, old: Option<&View>) {
        self.made.push((index, old.is_some()));
        if let Some(colors) = old.and_then(|old| old.colors.as_ref()) {
            let units = |v: &View| v.text.encode_utf16().collect::<Vec<u16>>();
            view.colors = Some(carry_spans(&units(old.unwrap()), &units(view), colors));
        }
    }
}

/// A note, its blocks and views as the window has them, and the editor.
struct Note {
    editor: SourceEditor,
    format: NoteFormat,
    blocks: Vec<Block>,
    refs: bool,
    views: Vec<View>,
    /// The views as a window holds them, applying the splices.
    window: Vec<View>,
    maker: Views,
}

impl Note {
    fn new(text: &str, format: NoteFormat) -> Self {
        let editor = SourceEditor::new(text);
        let (blocks, refs) = parse_all(text, format);
        let mut maker = Views::default();
        let views = maker.all(&blocks, text, editor.document().rope());
        maker.all = 0;
        Self {
            editor,
            format,
            blocks,
            refs,
            window: views.clone(),
            views,
            maker,
        }
    }

    fn markdown(text: &str) -> Self {
        Self::new(text, NoteFormat::Markdown)
    }

    fn text(&self) -> String {
        self.editor.text()
    }

    /// Puts the cursor after the first `needle`.
    fn click_after(&mut self, needle: &str) {
        let text = self.text();
        let byte = text.find(needle).unwrap() + needle.len();
        let at = text[..byte].chars().count();
        self.editor.set_selection(at, at);
    }

    /// Applies `work` as one input, refreshes the views and checks that
    /// they and the window's are those of a full parse.
    fn input(&mut self, work: impl FnOnce(&mut FacadeEdit<'_>)) -> (Gathered, Refreshed<View>) {
        let before = Before::of(&self.editor, self.blocks.len());
        let mut gathered = Gathered::default();
        work(&mut FacadeEdit::new(
            &mut self.editor,
            &mut self.blocks,
            self.format,
            &mut self.refs,
            &mut gathered,
        ));
        let refreshed = match gathered.dirty.range() {
            Some(dirty) => refresh(
                &mut self.views,
                &self.blocks,
                &self.editor,
                &mut self.maker,
                dirty,
                before,
            ),
            None => Refreshed {
                splices: Vec::new(),
                recaptioned: false,
            },
        };
        for splice in &refreshed.splices {
            let mut after = self.window.split_off(splice.from + splice.removed);
            self.window.truncate(splice.from);
            self.window.extend(splice.blocks.iter().cloned());
            for view in &mut after {
                view.shift(splice.shift, splice.line_shift);
            }
            self.window.extend(after);
        }
        let text = self.text();
        let (blocks, _) = parse_all(&text, self.format);
        assert_eq!(self.blocks, blocks, "the blocks of a full parse");
        let full: Vec<View> = blocks
            .iter()
            .map(|block| view(block, self.editor.document().rope()))
            .collect();
        let plain = |views: &[View]| -> Vec<View> {
            views
                .iter()
                .map(|v| View {
                    colors: None,
                    ..v.clone()
                })
                .collect()
        };
        assert_eq!(plain(&self.views), full, "the views of a full parse");
        assert_eq!(self.window, self.views, "the window in step");
        (gathered, refreshed)
    }
}

#[test]
fn adr025_typing_makes_only_its_block_anew_and_moves_the_rest() {
    let mut note = Note::markdown("# Title\n\nSome text.\n\nLast.\n");
    note.click_after("Some");
    let (gathered, refreshed) = note.input(|edit| edit.type_text("x"));
    assert_eq!(note.text(), "# Title\n\nSomex text.\n\nLast.\n");
    assert_eq!(gathered.dirty.range(), Some(1..2));
    assert_eq!(note.maker.some, vec![1..2]);
    assert_eq!(note.maker.all, 0);
    assert!(note.maker.moved.is_empty(), "no block came or went");
    assert_eq!(note.maker.made, vec![(1, true)]);
    let [splice] = refreshed.splices.as_slice() else {
        panic!("one splice: {:?}", refreshed.splices);
    };
    assert_eq!(
        (splice.from, splice.removed, splice.blocks.len()),
        (1, 1, 1)
    );
    assert_eq!((splice.shift, splice.line_shift), (1, 0));
    assert!(!refreshed.recaptioned);
}

#[test]
fn adr025_enter_splits_a_block_and_the_blocks_after_it_follow() {
    let mut note = Note::markdown("One.\n\nSome text.\n\nLast.\n");
    note.maker.kept = HashMap::from([(0, "first"), (1, "split"), (2, "last")]);
    note.click_after("Some");
    let (_, refreshed) = note.input(|edit| edit.key(Key::Enter, Modifiers::default()));
    assert_eq!(note.text(), "One.\n\nSome\n\n text.\n\nLast.\n");
    assert_eq!(note.views.len(), 4);
    // One block became two: what is kept per block after it moves along,
    // that of the split block goes.
    assert_eq!(note.maker.moved, vec![(1..2, 1)]);
    assert_eq!(note.maker.kept, HashMap::from([(0, "first"), (3, "last")]));
    let [splice] = refreshed.splices.as_slice() else {
        panic!("one splice: {:?}", refreshed.splices);
    };
    assert_eq!(
        (splice.from, splice.removed, splice.blocks.len()),
        (1, 1, 2)
    );
    assert_eq!((splice.shift, splice.line_shift), (2, 2));
    // Backspace joins them again.
    note.input(|edit| edit.key(Key::Backspace, Modifiers::default()));
    assert_eq!(note.text(), "One.\n\nSome text.\n\nLast.\n");
}

/// The part of the note a refresh makes views of ends one byte past the
/// blocks; that byte may fall inside a multibyte char right after them.
fn edit_before_a_wide_char(text: &str, after: &str, format: NoteFormat) {
    let mut note = Note::new(text, format);
    note.click_after(after);
    let (gathered, _) = note.input(|edit| edit.type_text("y"));
    assert!(gathered.dirty.range().is_some(), "{text:?}");
    assert_eq!(note.maker.some.len(), 1, "{text:?}");
    let mut expected = text.to_owned();
    let at = text.find(after).unwrap() + after.len();
    expected.insert(at, 'y');
    assert_eq!(note.text(), expected);
}

#[test]
fn adr025_an_edit_right_before_a_non_ascii_line_does_not_split_a_char() {
    edit_before_a_wide_char("# H\néx\n", "# H", NoteFormat::Markdown);
    edit_before_a_wide_char("# Hé\néx\n", "# H", NoteFormat::Markdown);
    edit_before_a_wide_char("# H\r\néx\r\n", "# H", NoteFormat::Markdown);
    edit_before_a_wide_char("    code\né\n", "    co", NoteFormat::Markdown);
    edit_before_a_wide_char("# H\n🙂x\n", "# H", NoteFormat::Markdown);
    edit_before_a_wide_char("== H\néx\n", "== H", NoteFormat::AsciiDoc);
}

#[test]
fn adr025_slices_of_the_rope_keep_to_char_boundaries() {
    use super::rules::{byte_text, ceil_boundary, floor_boundary};
    let rope = Rope::from_str("aé🙂b");
    // é is bytes 1..3, 🙂 bytes 3..7.
    assert_eq!(floor_boundary(&rope, 2), 1);
    assert_eq!(ceil_boundary(&rope, 2), 3);
    assert_eq!(ceil_boundary(&rope, 4), 7);
    assert_eq!(ceil_boundary(&rope, 3), 3);
    assert_eq!(ceil_boundary(&rope, 99), 8);
    assert_eq!(byte_text(&rope, 2..4), "é🙂");
    assert_eq!(byte_text(&rope, 0..1), "a");
}

#[test]
fn adr025_views_out_of_step_are_made_anew_whole() {
    let mut note = Note::markdown("One.\n\nTwo.\n");
    // The window lost a block: all views are made anew.
    note.views.pop();
    note.window.pop();
    note.click_after("Two");
    let (_, refreshed) = note.input(|edit| edit.type_text("s"));
    assert_eq!(note.maker.all, 1);
    assert!(note.maker.some.is_empty());
    assert!(!refreshed.splices.is_empty());
}

#[test]
fn adr025_a_caption_typed_numbers_the_figures_after_it_anew() {
    let mut note = Note::markdown("A table.\n\nAfter.\n");
    note.click_after("A table");
    let (_, refreshed) = note.input(|edit| edit.type_text("!"));
    assert!(refreshed.recaptioned);
    let (_, refreshed) = note.input(|edit| edit.type_text("x"));
    assert!(!refreshed.recaptioned);
}

#[test]
fn m09_4_colours_are_carried_into_the_view_made_anew() {
    let mut note = Note::markdown("```rust\nlet a = 1;\n```\n\nAfter.\n");
    // `let` and `1` coloured.
    note.views[0].colors = Some(vec![(0, 3, 1), (8, 9, 2)]);
    note.window[0].colors = note.views[0].colors.clone();
    note.click_after("let ");
    note.input(|edit| edit.type_text("bb"));
    assert_eq!(note.text(), "```rust\nlet bba = 1;\n```\n\nAfter.\n");
    assert_eq!(note.views[0].colors, Some(vec![(0, 3, 1), (10, 11, 2)]));
}

#[test]
fn m09_4_carried_spans_stay_before_and_move_after_the_change() {
    let units = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let spans = [(0, 3, 'a'), (4, 7, 'b'), (8, 11, 'c')];
    // "bbb" became "XY": the span over it covers the change, the one after
    // it moves.
    assert_eq!(
        carry_spans(&units("aaa bbb ccc"), &units("aaa XY ccc"), &spans),
        vec![(0, 3, 'a'), (4, 6, 'b'), (7, 10, 'c')]
    );
    // A span inside the change shrinks to nothing and goes.
    assert_eq!(
        carry_spans(&units("aaa bbb ccc"), &units("aaa XY ccc"), &[(5, 6, 'z')]),
        vec![]
    );
    // Typed at the end of the first span: it stays, the rest move.
    assert_eq!(
        carry_spans(&units("aaa bbb ccc"), &units("aaaz bbb ccc"), &spans),
        vec![(0, 3, 'a'), (5, 8, 'b'), (9, 12, 'c')]
    );
    assert_eq!(carry_spans::<u8>(&units("a"), &units("b"), &[]), vec![]);
}

#[test]
fn m09_7_moved_indices_drop_the_replaced_and_move_the_rest() {
    let map = HashMap::from([(0, 'a'), (2, 'b'), (3, 'c'), (5, 'd')]);
    assert_eq!(
        moved_indices(map.clone(), 2..4, -1),
        HashMap::from([(0, 'a'), (4, 'd')])
    );
    assert_eq!(
        moved_indices(map, 1..2, 2),
        HashMap::from([(0, 'a'), (4, 'b'), (5, 'c'), (7, 'd')])
    );
    // An index that would fall below zero goes rather than wrapping.
    assert_eq!(
        moved_indices(HashMap::from([(1, 'x')]), 0..1, -5),
        HashMap::new()
    );
}

fn shown(start: u32, line: u32, text: &str) -> View {
    View {
        start,
        line,
        text: text.to_owned(),
        colors: None,
    }
}

#[test]
fn adr024_splices_keep_the_blocks_around_a_change() {
    let old = vec![shown(0, 0, "a"), shown(3, 2, "b"), shown(6, 4, "c")];
    assert!(splices(&old, &old).is_empty());
    // "b" grew: the block after it moved.
    let new = vec![shown(0, 0, "a"), shown(3, 2, "bb"), shown(7, 4, "c")];
    assert_eq!(
        splices(&old, &new),
        vec![Splice {
            from: 1,
            removed: 1,
            blocks: vec![shown(3, 2, "bb")],
            shift: 1,
            line_shift: 0,
        }]
    );
    // Changes in place are sent each on their own (M-09.7).
    let new = vec![shown(0, 0, "A"), shown(3, 2, "b"), shown(6, 4, "C")];
    let changes = splices(&old, &new);
    assert_eq!(changes.len(), 2);
    assert_eq!((changes[0].from, changes[1].from), (0, 2));
    // A block removed at the end.
    let changes = splices(&old, &old[..2]);
    assert_eq!(
        (changes[0].from, changes[0].removed, changes[0].blocks.len()),
        (2, 1, 0)
    );
    assert_eq!(captions(&[shown(0, 0, "a!"), shown(1, 1, "b")]), 1);
}

#[test]
fn adr025_before_is_the_text_before_the_input() {
    let editor = SourceEditor::new("A\r\n🙂\n");
    let before = Before::of(&editor, 2);
    assert_eq!(before.rev, editor.revision());
    assert_eq!(before.blocks, 2);
    // `\r\n` is one web unit, the emoji two.
    assert_eq!(before.web, 5);
    assert_eq!(before.lines, 3);
}

#[test]
fn uc005_m09_10_typing_escapes_markdown_and_keeps_the_code_raw() {
    let mut note = Note::markdown("Some text.\n\n```\nfn a() {}\n```\n");
    note.click_after("Some");
    note.input(|edit| edit.type_text("*x_"));
    assert_eq!(note.text(), "Some\\*x\\_ text.\n\n```\nfn a() {}\n```\n");
    note.click_after("fn a(");
    note.input(|edit| edit.type_text("*b"));
    assert!(note.text().contains("fn a(*b) {}"));
}

#[test]
fn uc005_m09_11_enter_in_a_list_starts_the_next_item() {
    let mut note = Note::markdown("1. one\n2. two\n");
    note.click_after("two");
    note.input(|edit| edit.key(Key::Enter, Modifiers::default()));
    assert_eq!(note.text(), "1. one\n2. two\n3. \n");
}

#[test]
fn d17_enter_then_backspace_keeps_the_space() {
    let mut note = Note::markdown("An image block here.\n");
    note.click_after("image");
    note.input(|edit| edit.key(Key::Enter, Modifiers::default()));
    assert_eq!(note.text(), "An image\n\n block here.\n");
    note.input(|edit| edit.key(Key::Backspace, Modifiers::default()));
    assert_eq!(note.text(), "An image block here.\n");
}

#[test]
fn uc005_a2_link_asks_first_and_bold_applies_at_once() {
    let mut note = Note::markdown("Some text.\n");
    note.editor.set_selection(4, 0);
    let (gathered, _) = note.input(|edit| edit.apply_tool(Tool::Link));
    assert_eq!(gathered.request, Some(Tool::Link));
    assert_eq!(note.text(), "Some text.\n");
    let (gathered, _) = note.input(|edit| edit.apply_tool(Tool::Bold));
    assert_eq!(gathered.request, None);
    assert_eq!(note.text(), "**Some** text.\n");
}

#[test]
fn m10_2_tools_map_to_their_formatting() {
    assert!(matches!(Tool::Bold.format(), Some(Format::Bold)));
    assert!(matches!(Tool::Table.format(), Some(Format::Table)));
    assert!(Tool::Link.format().is_none());
    assert!(Tool::Image.format().is_none());
}

#[test]
fn m11_2_vim_dd_deletes_a_block_and_x_a_shown_character() {
    let mut note = Note::markdown("First block.\n\nSecond block.\n\nThird.\n");
    note.click_after("Second");
    let mut keys = Keys::default();
    keys.mode = InputMode::Vim;
    let mut press = |note: &mut Note, typed: &'static str| {
        note.input(|edit| {
            let through = keys
                .handle(
                    vec![typed],
                    |text| Some(KeyInput::Text(text)),
                    &mut FacadeTarget { edit },
                )
                .0;
            assert!(through.is_empty(), "Vim's normal mode takes {typed}");
        })
    };
    press(&mut note, "d");
    let (gathered, _) = press(&mut note, "d");
    assert_eq!(note.text(), "First block.\n\nThird.\n");
    assert!(!gathered.outcome.changes.is_empty());
    press(&mut note, "0");
    press(&mut note, "x");
    assert_eq!(note.text(), "First block.\n\nhird.\n");
}
