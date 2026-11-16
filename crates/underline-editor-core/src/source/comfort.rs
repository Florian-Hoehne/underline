//! FR-081 to FR-086: the editing comfort on the text of the source view
//! (`crate::comfort`): close pairs (UC-005 A25), moving and duplicating
//! lines (A26), indenting list items (A27), and going to a line (UC-004
//! A16). The facade uses the same text and the same closing chars written
//! (`facade::FacadeEdit`).

use std::ops::Range;

use underline_markup::NoteFormat;

use super::{Outcome, SourceEditor};
use crate::comfort::goto::LineTarget;
use crate::comfort::lines;
use crate::comfort::lists::{self, ListIndent};
use crate::comfort::pairs::{self, PairContext, Paired};
use crate::comfort::TextEdit;

impl SourceEditor {
    /// BR-019: the closing chars written move with the edit that replaced
    /// the chars `removed` by `inserted` chars; those it replaced are gone.
    pub(super) fn move_closers(&mut self, removed: Range<usize>, inserted: usize) {
        if self.closers.is_empty() {
            return;
        }
        let length = removed.end - removed.start;
        self.closers.retain_mut(|at| {
            if *at < removed.start {
                true
            } else if *at >= removed.end {
                *at = *at - length + inserted;
                true
            } else {
                false
            }
        });
    }

    /// UC-005 BR-019: the places of the closing chars close pairs wrote,
    /// in order.
    #[must_use]
    pub fn closers(&self) -> &[usize] {
        &self.closers
    }

    /// BR-019: close pairs wrote the closing char at `at`.
    pub fn add_closer(&mut self, at: usize) {
        if let Err(index) = self.closers.binary_search(&at) {
            self.closers.insert(index, at);
        }
    }

    /// BR-019: the closing char at `at` was moved over; whether close
    /// pairs had written it.
    pub fn take_closer(&mut self, at: usize) -> bool {
        match self.closers.binary_search(&at) {
            Ok(index) => {
                self.closers.remove(index);
                true
            }
            Err(_) => false,
        }
    }

    /// Puts the cursor and the anchor at these chars without forgetting the
    /// closing chars written: close pairs moves within its pairs. Returns
    /// whether they moved.
    pub fn place(&mut self, cursor: usize, anchor: usize) -> bool {
        let len = self.doc.len_chars();
        let before = (self.cursor, self.anchor);
        self.cursor = cursor.min(len);
        self.anchor = anchor.min(len);
        before != (self.cursor, self.anchor)
    }

    /// Applies `edit` as one change, the selection where it says.
    pub fn apply_edit(&mut self, edit: &TextEdit) -> Outcome {
        let mut outcome = self.replace(edit.range.clone(), &edit.text);
        self.place(edit.cursor, edit.anchor);
        outcome.moved = true;
        outcome
    }

    /// BR-020, BR-021: applies `edit` as a step of its own for undo, never
    /// merged with the typing before or after it.
    pub fn apply_step(&mut self, edit: &TextEdit) -> Outcome {
        self.seal();
        let outcome = self.apply_edit(edit);
        self.seal();
        outcome
    }

    /// UC-005 A25, BR-019: `text` typed with close pairs on, in a note in
    /// `format`; `None` when it is typed as it is.
    pub fn type_paired(&mut self, text: &str, format: NoteFormat) -> Option<Outcome> {
        let paired = PairContext {
            rope: self.doc.rope(),
            cursor: self.cursor,
            anchor: self.anchor,
            closers: &self.closers,
            format,
        }
        .typed(text)?;
        Some(match paired {
            Paired::Over(at) => {
                self.take_closer(at - 1);
                self.place(at, at);
                Outcome {
                    changes: Vec::new(),
                    moved: true,
                }
            }
            Paired::Typed(edit, written) => {
                let outcome = if edit.range.is_empty() {
                    self.apply_edit(&edit)
                } else {
                    self.apply_step(&edit)
                };
                for at in written {
                    self.add_closer(at);
                }
                outcome
            }
        })
    }

    /// UC-005 A25, BR-019: Backspace with close pairs on; `None` when it
    /// deletes as usual.
    pub fn backspace_paired(&mut self) -> Option<Outcome> {
        let edit = pairs::backspace(self.doc.rope(), self.cursor, self.anchor, &self.closers)?;
        Some(self.apply_edit(&edit))
    }

    /// UC-005 A26, BR-020: the lines the selection touches go up or down
    /// past the line next to them, as one step; nothing at the first or
    /// the last line.
    pub fn move_lines(&mut self, up: bool) -> Outcome {
        match lines::move_lines(self.doc.rope(), self.cursor, self.anchor, up) {
            Some(edit) => self.apply_step(&edit),
            None => Outcome::default(),
        }
    }

    /// UC-005 A26, BR-020: a copy of the lines the selection touches goes
    /// below them, the cursor in it, as one step.
    pub fn duplicate_lines(&mut self) -> Outcome {
        let edit = lines::duplicate_lines(self.doc.rope(), self.cursor, self.anchor);
        self.apply_step(&edit)
    }

    /// UC-005 A27, BR-021: Tab (`outdent` false) or Shift+Tab on the list
    /// items the selection touches, in a note in `format`, as one step;
    /// `None` outside a list item, where the key keeps its meaning.
    pub fn indent_list(&mut self, format: NoteFormat, outdent: bool) -> Option<Outcome> {
        match lists::indent(self.doc.rope(), self.cursor, self.anchor, format, outdent) {
            ListIndent::NotList => None,
            ListIndent::Unchanged => Some(Outcome::default()),
            ListIndent::Edit(edit) => Some(self.apply_step(&edit)),
        }
    }

    /// OQ-260, UC-005 BR-021: Vim's `>>` and `<<` (`>` and `<` on a
    /// selection) on the lines from the one holding char `from` to the one
    /// holding char `to`, `times` times over, in a note in `format`: on a
    /// list item by the list rule, as Tab and Shift+Tab; elsewhere Vim's
    /// shift by two columns. One step for undo, the cursor on the first
    /// char of the first line that is not space.
    pub fn vim_shift(
        &mut self,
        (from, to): (usize, usize),
        format: NoteFormat,
        outdent: bool,
        times: usize,
    ) -> Outcome {
        let edit = lists::vim_shift_lines(self.doc.rope(), from, to, format, outdent, times);
        match edit {
            Some(edit) => self.apply_step(&edit),
            None => Outcome::default(),
        }
    }

    /// OQ-204, OQ-256: Shift+Tab outside a list item: the lines the
    /// selection touches lose up to one tab stop of their leading space, as
    /// one step; nothing when they have none.
    pub fn outdent_lines(&mut self) -> Outcome {
        match lists::outdent_lines(self.doc.rope(), self.cursor, self.anchor) {
            Some(edit) => self.apply_step(&edit),
            None => Outcome::default(),
        }
    }

    /// UC-004 A16, BR-017: the cursor to the line and column of `target`;
    /// it is shown even when it was there.
    pub fn go_to(&mut self, target: LineTarget) -> Outcome {
        let at = target.char_in(self.doc.rope());
        self.set_selection(at, at);
        Outcome {
            changes: Vec::new(),
            moved: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, Modifiers};

    const MD: NoteFormat = NoteFormat::Markdown;

    fn editor(text: &str, at: usize) -> SourceEditor {
        let mut editor = SourceEditor::new(text);
        editor.set_selection(at, at);
        editor
    }

    fn typed(editor: &mut SourceEditor, text: &str) {
        for c in text.chars() {
            let c = c.to_string();
            if editor.type_paired(&c, MD).is_none() {
                editor.type_text(&c);
            }
        }
    }

    fn undo(editor: &mut SourceEditor) {
        let command = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::default()
        };
        editor.key(Key::Z, command, 1);
    }

    /// TC-005-35 steps 1 to 3: `(` closes, `a)` types over the written `)`,
    /// and Backspace after a new `(` takes both.
    #[test]
    fn tc_005_35_typing_through_a_pair_and_back() {
        let mut editor = editor("f", 1);
        typed(&mut editor, "(");
        assert_eq!((editor.text().as_str(), editor.cursor()), ("f()", 2));
        typed(&mut editor, "a)");
        assert_eq!((editor.text().as_str(), editor.cursor()), ("f(a)", 4));
        typed(&mut editor, " (");
        assert_eq!(editor.text(), "f(a) ()");
        assert!(editor.backspace_paired().is_some());
        assert_eq!((editor.text().as_str(), editor.cursor()), ("f(a) ", 5));
    }

    /// TC-005-35 step 4: `**` within a line gives `****`, the cursor in the
    /// middle; the closing pair is typed over.
    #[test]
    fn tc_005_35_bold_closes_and_is_typed_over() {
        let mut editor = editor("so ", 3);
        typed(&mut editor, "**");
        assert_eq!((editor.text().as_str(), editor.cursor()), ("so ****", 5));
        typed(&mut editor, "bé**");
        assert_eq!((editor.text().as_str(), editor.cursor()), ("so **bé**", 9));
    }

    /// BR-019: moving the cursor away forgets the pairs written: a `)`
    /// typed later is typed.
    #[test]
    fn br019_the_cursor_put_elsewhere_forgets_the_pairs() {
        let mut editor = editor("", 0);
        typed(&mut editor, "(");
        editor.set_selection(0, 0);
        editor.set_selection(1, 1);
        typed(&mut editor, ")");
        assert_eq!(editor.text(), "())");
    }

    /// BR-019, BR-004: an enclosed selection is one step for undo; the
    /// pair typed after a word goes with the word.
    #[test]
    fn br019_enclosing_is_one_undo_step() {
        let mut editor = editor("a word", 6);
        editor.set_selection(6, 2);
        typed(&mut editor, "_");
        assert_eq!(editor.text(), "a _word_");
        assert_eq!(editor.selection(), 3..7);
        undo(&mut editor);
        assert_eq!(editor.text(), "a word");
    }

    /// TC-005-36 step 3: a move and a duplicate are a step each.
    #[test]
    fn tc_005_36_moves_and_copies_are_steps_of_their_own() {
        let mut editor = editor("one\ntwo\nthree", 5);
        typed(&mut editor, "x");
        editor.move_lines(false);
        assert_eq!(editor.text(), "one\nthree\ntxwo");
        editor.duplicate_lines();
        assert_eq!(editor.text(), "one\nthree\ntxwo\ntxwo");
        assert_eq!(editor.cursor(), 17);
        undo(&mut editor);
        assert_eq!(editor.text(), "one\nthree\ntxwo");
        undo(&mut editor);
        assert_eq!(editor.text(), "one\ntxwo\nthree");
        undo(&mut editor);
        assert_eq!(editor.text(), "one\ntwo\nthree");
        assert_eq!(editor.move_lines(true).changes.len(), 1);
        assert!(!editor.move_lines(true).edited(), "at the first line");
    }

    /// TC-005-37: Tab in a list is one step; outside it keeps its meaning.
    #[test]
    fn tc_005_37_indent_is_one_step() {
        let mut list = editor("- a\n- b", 6);
        assert!(list.indent_list(MD, false).is_some());
        assert_eq!(list.text(), "- a\n  - b");
        undo(&mut list);
        assert_eq!(list.text(), "- a\n- b");
        let mut text = editor("plain", 2);
        assert!(text.indent_list(MD, false).is_none());
    }

    /// TC-004-33: the cursor goes to the line and column.
    #[test]
    fn tc_004_33_go_to_a_line() {
        let mut editor = editor("a\nbcd\ne", 0);
        let outcome = editor.go_to(LineTarget {
            line: 2,
            column: Some(3),
        });
        assert!(outcome.moved && !outcome.edited());
        assert_eq!(editor.cursor(), 4);
    }

    /// TC-005-35 to TC-005-37, BR-019 to BR-021 (test-monkey): random text
    /// and a random action of the editing comfort never panic, keep the
    /// selection and the closing chars written within the text, and one undo
    /// restores the exact text. The seed is in the failure message.
    #[test]
    fn br019_to_br021_random_actions_keep_the_text_and_undo_restores_it() {
        const PIECES: [&str; 24] = [
            "- ", "* ", "1. ", "  ", "\t", "a", "é", "😀", "\n", "\r\n", "```", "~~~", "----",
            "**", "`", "(", ")", "\"", "*", "# ", "> ", "\u{2028}", "\r", "x y",
        ];
        let state = std::cell::Cell::new(0x9E37_79B9_7F4A_7C15_u64);
        let below = |n: usize| {
            let mut x = state.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            state.set(x);
            usize::try_from(x % n as u64).expect("small")
        };
        for round in 0..3000 {
            let seed = state.get();
            let text: String = (0..below(25)).map(|_| PIECES[below(24)]).collect();
            let len = text.chars().count();
            let mut editor = SourceEditor::new(&text);
            let cursor = below(len + 1);
            let anchor = if below(2) == 0 {
                cursor
            } else {
                below(len + 1)
            };
            editor.set_selection(cursor, anchor);
            let format = if below(2) == 0 {
                MD
            } else {
                NoteFormat::AsciiDoc
            };
            let action = below(7);
            let typed = ["(", "\"", "`", "*", "~", "_", "[", ")"][below(8)];
            let context = format!("round {round}, seed {seed:#x}, {text:?}, {cursor},{anchor}, action {action}, {format:?}");
            let outcome = match action {
                0 => editor.move_lines(true),
                1 => editor.move_lines(false),
                2 => editor.duplicate_lines(),
                3 | 4 => editor.indent_list(format, action == 4).unwrap_or_default(),
                5 => editor.type_paired(typed, format).unwrap_or_default(),
                _ => editor.backspace_paired().unwrap_or_default(),
            };
            let after = editor.text().chars().count();
            assert!(
                editor.cursor() <= after && editor.anchor() <= after,
                "{context}"
            );
            assert!(editor.closers().iter().all(|at| *at < after), "{context}");
            if outcome.edited() {
                undo(&mut editor);
                assert_eq!(editor.text(), text, "undo restores: {context}");
            }
        }
    }

    /// A small seeded generator, so a failure replays from the logged seed.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            usize::try_from(self.next() % n as u64).unwrap()
        }
    }

    /// OQ-260, UC-005 BR-021, TC-005-37 (test-monkey): random notes (CRLF,
    /// tabs, multibyte, list markers of both formats down to depth 5,
    /// fences), random ranges, counts 0 to 100, both directions: no panic,
    /// the cursor within the text, the line count and the line breaks and
    /// every line outside the range unchanged, and one undo restores the
    /// note exactly.
    #[test]
    fn oq260_vim_shift_random_notes_keep_their_shape_and_undo_in_one_step() {
        let pieces = [
            "- a",
            "  - b",
            "\t- c",
            "* x",
            "** y",
            "***** deep",
            "1. one",
            "   2. two",
            "- [ ] t",
            "```",
            "~~~",
            "    code",
            "\tтаб",
            "日本語",
            "e\u{301}",
            "",
            "   ",
            "> - q",
            "plain",
            "- ",
            "*",
            "#",
            "\u{2028}",
            "\x0b",
            "a\rb",
            "😀 - x",
            "  \t  - mixed",
            ". dot",
            ".. dot2",
        ];
        // No lone CR here: see the test for it.
        let breaks = ["\n", "\r\n", "\n", "\r\n"];
        let seed: u64 = std::env::var("UL_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0x5eed_1113);
        eprintln!("seed {seed}");
        let mut rng = Rng(seed);
        for round in 0..4000 {
            let lines = 1 + rng.below(12);
            let mut text = String::new();
            for n in 0..lines {
                text.push_str(pieces[rng.below(pieces.len())]);
                if n + 1 < lines || rng.below(2) == 0 {
                    text.push_str(breaks[rng.below(breaks.len())]);
                }
            }
            let len = text.chars().count();
            let from = rng.below(len + 2);
            let to = rng.below(len + 2);
            let outdent = rng.below(2) == 0;
            let times = [0, 1, 1, 2, 3, 7, 100][rng.below(7)];
            let format = if rng.below(2) == 0 {
                NoteFormat::Markdown
            } else {
                NoteFormat::AsciiDoc
            };
            let context = format!("seed {seed} round {round}: {text:?} {from}..{to} out {outdent} x{times} {format:?}");
            let mut editor = SourceEditor::new(&text);
            editor.set_selection(0, 0);
            let before = editor.text();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                editor.vim_shift((from, to), format, outdent, times);
                editor
            }));
            let mut editor = result.unwrap_or_else(|_| panic!("panic: {context}"));
            let after = editor.text();
            let selection = editor.selection();
            let (cursor, anchor) = (selection.start, selection.end);
            let chars = after.chars().count();
            assert!(
                cursor <= chars && anchor <= chars,
                "cursor outside: {context}"
            );
            let breaks_of = |t: &str| -> String {
                t.chars()
                    .filter(|c| {
                        matches!(
                            c,
                            '\r' | '\n' | '\u{2028}' | '\u{2029}' | '\x0b' | '\x0c' | '\u{85}'
                        )
                    })
                    .collect()
            };
            assert_eq!(
                breaks_of(&before),
                breaks_of(&after),
                "line breaks changed: {context}"
            );
            // Every line outside the range is as before.
            let old = ropey::Rope::from_str(&before);
            let new = ropey::Rope::from_str(&after);
            assert_eq!(old.len_lines(), new.len_lines(), "{context}");
            let (a, b) = (
                old.char_to_line(from.min(old.len_chars())),
                old.char_to_line(to.min(old.len_chars())),
            );
            let (first, last) = (a.min(b), a.max(b));
            for line in 0..old.len_lines() {
                if line < first || line > last {
                    // A list item's children move with it, in their indent
                    // and markers only.
                    let bare = |l: ropey::RopeSlice| {
                        l.to_string()
                            .trim_start_matches([' ', '\t', '*', '.'])
                            .to_owned()
                    };
                    assert_eq!(
                        bare(old.line(line)),
                        bare(new.line(line)),
                        "line {line} outside {first}..={last}: {context}"
                    );
                }
            }
            if before != after {
                editor.undo();
                assert_eq!(
                    editor.text(),
                    before,
                    "one undo does not restore: {context}"
                );
            }
        }
    }

    /// OQ-260, UC-005 BR-021 (test-monkey): `<<` on a line of spaces after a
    /// line that ends in a lone CR joins that CR with the line's own break
    /// into one CRLF; the note must not lose a line or copy the next one
    /// over it. Expected no copy of `b`; actual: "a\rb\nb".
    #[test]
    fn oq260_vim_shift_outdent_of_a_blank_line_after_a_lone_cr_keeps_the_lines() {
        let text = "a\r  \nb";
        let mut editor = SourceEditor::new(text);
        editor.set_selection(0, 0);
        editor.vim_shift((3, 3), NoteFormat::Markdown, true, 1);
        let after = editor.text();
        assert_eq!(after.matches('b').count(), 1, "text copied: {after:?}");
    }

    /// OQ-260 (test-monkey): a huge count (`99999999>>` typed in Vim) must not
    /// build a note of hundreds of megabytes of spaces or hang the editor.
    #[test]
    fn oq260_vim_shift_with_a_huge_count_is_capped() {
        let mut editor = SourceEditor::new("plain\n");
        editor.set_selection(0, 0);
        let started = std::time::Instant::now();
        editor.vim_shift((0, 0), NoteFormat::Markdown, false, 50_000_000);
        let size = editor.text().len();
        assert!(size < 1_000_000, "{size} bytes after one shift");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
