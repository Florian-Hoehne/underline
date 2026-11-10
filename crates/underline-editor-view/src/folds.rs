//! M-08.8, UC-004 BR-012: folding the sections of a note, per tab, so both
//! views share it.
//!
//! Ported from `underline-editor`'s `source_view.rs` (`sections`, `hidden`,
//! `toggle_fold`, `fold_key`, `unfold_at_cursor`). The sections come from
//! the facade engine (`underline_markup::sections`) and are kept as lines:
//! the egui widget keeps the bytes of the sections and turns them into
//! lines when it lays out the text; here the lines are worked out once when
//! the sections are found.
//!
//! The sections are found again after the input that changed the text, off
//! the keystroke (ADR-024): at most every [`SECTIONS_EVERY_MS`] while the
//! text changes. While something is folded, an input that adds or removes
//! lines finds them at once, as the egui widget does, so Up and Down never
//! step into a folded body.
//!
//! Moved here from `underline-session` (`docs/migration/variante-b.md`,
//! step B3b); the time is the host's clock in milliseconds
//! ([`EditorTab::set_now`]), as the webview has no `Instant`.

use std::collections::HashSet;
use std::ops::Range;

use underline_editor_core::Outcome;
use underline_markup::sections::{sections, SectionKey};

use crate::editor_view::{revision, units, SectionView, SectionsView};
use crate::EditorTab;

/// How often the sections are found again while the text changes and
/// nothing is folded, as in the egui widget: 300 ms.
pub const SECTIONS_EVERY_MS: f64 = 300.0;

/// One section of the text, by lines counted from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SectionLines {
    key: SectionKey,
    level: usize,
    /// The line of the heading.
    heading: usize,
    /// The lines folding hides; empty when there is nothing to fold.
    body: Range<usize>,
}

/// The fold state of one tab.
#[derive(Debug, Default, Clone)]
pub(crate) struct Folds {
    /// The folded sections, by key, so they stay folded while the text
    /// around them changes.
    folded: HashSet<SectionKey>,
    /// The sections of the text revision they were found in.
    found: Option<(u64, Vec<SectionLines>)>,
    /// How many lines the text had then.
    found_lines: usize,
    /// When they were found last, in milliseconds of the host's clock.
    found_at: Option<f64>,
    /// Counts the changes of `folded`.
    version: u64,
    /// The revision and fold version the window was told of last.
    pub(crate) told: Option<(u64, u64)>,
}

impl EditorTab {
    /// The sections of the text as it is now, found again when it changed.
    fn fresh_sections(&mut self) -> &[SectionLines] {
        let rev = self.editor.revision();
        if self.folds.found.as_ref().is_none_or(|(r, _)| *r != rev) {
            let text = self.editor.text();
            let rope = self.editor.document().rope();
            let bytes = rope.len_bytes();
            let lines = sections(&text, self.format)
                .into_iter()
                .map(|section| {
                    let heading = rope.byte_to_line(section.heading.start.min(bytes));
                    let body = if section.body.is_empty() {
                        heading + 1..heading + 1
                    } else {
                        let first = rope.byte_to_line(section.body.start.min(bytes));
                        let last = rope.byte_to_line((section.body.end - 1).min(bytes));
                        first..last + 1
                    };
                    SectionLines {
                        key: section.key,
                        level: section.level,
                        heading,
                        body,
                    }
                })
                .collect();
            self.folds.found = Some((rev, lines));
            self.folds.found_lines = self.editor.document().len_lines();
            self.folds.found_at = Some(self.now_ms);
        }
        self.folds.found.as_ref().map_or(&[], |(_, lines)| lines)
    }

    /// The sections by line without finding them again while the text has
    /// as many lines as when they were found: typing within a line moves no
    /// section, and the keystroke stays small (ADR-024).
    fn known_sections(&mut self) -> &[SectionLines] {
        let same = self.folds.found.is_some()
            && self.folds.found_lines == self.editor.document().len_lines();
        if same {
            return self.folds.found.as_ref().map_or(&[], |(_, lines)| lines);
        }
        self.fresh_sections()
    }

    /// The lines hidden by folded sections, in order and not overlapping,
    /// for the editor, whose Up and Down step over them.
    pub fn refold(&mut self) {
        let hidden = if self.folds.folded.is_empty() {
            Vec::new()
        } else {
            let folded = self.folds.folded.clone();
            let mut hidden: Vec<Range<usize>> = Vec::new();
            for section in self.fresh_sections() {
                if !folded.contains(&section.key) || section.body.is_empty() {
                    continue;
                }
                match hidden.last_mut() {
                    Some(last) if section.body.start <= last.end => {
                        last.end = last.end.max(section.body.end);
                    }
                    _ => hidden.push(section.body.clone()),
                }
            }
            hidden
        };
        self.editor.set_hidden_lines(hidden);
    }

    /// After an input: while something is folded, a change of the lines
    /// finds the sections at once, so the hidden lines stay right.
    pub fn refold_after(&mut self, outcome: &Outcome) {
        let lines_moved = outcome
            .changes
            .iter()
            .any(|change| change.history || change.added_lines != 0);
        if !self.folds.folded.is_empty() && lines_moved {
            self.refold();
        }
    }

    /// The line of the cursor.
    pub fn cursor_line(&self) -> usize {
        self.editor
            .document()
            .rope()
            .char_to_line(self.editor.cursor())
    }

    /// Folds or expands `section`. A cursor inside the hidden part moves to
    /// the end of the heading, so the section stays folded. Returns whether
    /// the cursor moved.
    fn toggle(&mut self, section: &SectionLines) -> bool {
        let mut moved = false;
        if !self.folds.folded.remove(&section.key) {
            self.folds.folded.insert(section.key.clone());
            if section.body.contains(&self.cursor_line()) {
                let rope = self.editor.document().rope();
                let start = rope.line_to_char(section.heading);
                let end = start
                    + self
                        .editor
                        .document()
                        .line_text(section.heading)
                        .chars()
                        .count();
                moved = self.editor.set_selection(end, end);
            }
        }
        self.folds.version += 1;
        self.refold();
        moved
    }

    /// Ctrl/Cmd+Shift+[ folds (`fold`), Ctrl/Cmd+Shift+] expands the
    /// innermost section at the cursor (UC-004 BR-012, NFR-011). Returns
    /// whether the cursor moved.
    pub fn fold_key(&mut self, fold: bool) -> bool {
        let line = self.cursor_line();
        let section = self
            .fresh_sections()
            .iter()
            .filter(|s| s.heading <= line && (line == s.heading || line < s.body.end))
            .max_by_key(|s| s.level)
            .cloned();
        match section {
            Some(section) if fold != self.folds.folded.contains(&section.key) => {
                self.toggle(&section)
            }
            _ => false,
        }
    }

    /// M-08.8: the arrow of the section whose heading is `line` was
    /// clicked. Returns whether the cursor moved.
    pub fn toggle_at_line(&mut self, line: usize) -> bool {
        let section = self
            .fresh_sections()
            .iter()
            .find(|s| s.heading == line && !s.body.is_empty())
            .cloned();
        section.is_some_and(|section| self.toggle(&section))
    }

    /// UC-004 BR-012: a folded section the cursor is in expands.
    pub fn unfold_at_cursor(&mut self) {
        if self.folds.folded.is_empty() {
            return;
        }
        let line = self.cursor_line();
        let open: Vec<SectionKey> = self
            .known_sections()
            .iter()
            .filter(|s| s.body.contains(&line))
            .map(|s| s.key.clone())
            .collect();
        let before = self.folds.folded.len();
        for key in &open {
            self.folds.folded.remove(key);
        }
        if self.folds.folded.len() != before {
            self.folds.version += 1;
            self.refold();
        }
    }

    /// M-08.8, M-12.2: the sections for the window at `now_ms`, once the
    /// text or the folds changed since it was told; `None` while the window
    /// has them, and while typing goes on, until [`SECTIONS_EVERY_MS`]
    /// passed.
    pub fn sections_view(&mut self, note: &str, now_ms: f64) -> Option<SectionsView> {
        self.set_now(now_ms);
        let now_ms = self.now_ms;
        let rev = self.editor.revision();
        let key = (rev, self.folds.version);
        if self.folds.told == Some(key) {
            return None;
        }
        let stale = self.folds.found.as_ref().is_none_or(|(r, _)| *r != rev);
        let recent = self
            .folds
            .found_at
            .is_some_and(|at| (now_ms - at).max(0.0) < SECTIONS_EVERY_MS);
        // While typing goes on, at most every 300 ms; a fold or an expand is
        // told at once. Folded lines stay right meanwhile: an input that
        // adds or removes lines finds them at once (`refold_after`).
        let folds_told = self
            .folds
            .told
            .is_some_and(|(_, version)| version == self.folds.version);
        if stale && recent && folds_told {
            return None;
        }
        if stale && !self.folds.folded.is_empty() {
            // The sections found anew may have other keys (a heading was
            // renamed) or places: the hidden lines follow what the window
            // is told, so Up and Down skip exactly the lines it hides.
            self.refold();
        }
        let folded = self.folds.folded.clone();
        let sections = self
            .fresh_sections()
            .iter()
            .map(|s| SectionView {
                line: units(s.heading),
                last: units(if s.body.is_empty() {
                    s.heading
                } else {
                    s.body.end - 1
                }),
                folded: folded.contains(&s.key) && !s.body.is_empty(),
            })
            .collect();
        self.folds.told = Some(key);
        Some(SectionsView {
            note: note.to_owned(),
            rev: revision(rev),
            sections,
        })
    }

    /// The window shows the note anew: it is told the sections again.
    pub fn forget_sections(&mut self) {
        self.folds.told = None;
    }
}

#[cfg(test)]
mod tests {
    use underline_editor_core::{Key, Modifiers};
    use underline_markup::NoteFormat;

    use super::*;

    const TEXT: &str = "# A\none\ntwo\n## B\nthree\n# C\nfour\n";

    fn tab(text: &str) -> EditorTab {
        EditorTab::new(text, NoteFormat::Markdown)
    }

    fn folded_lines(tab: &mut EditorTab, note: &str) -> Vec<(u32, u32)> {
        tab.folds.told = None;
        tab.sections_view(note, 0.0)
            .unwrap()
            .sections
            .iter()
            .filter(|s| s.folded)
            .map(|s| (s.line, s.last))
            .collect()
    }

    #[test]
    fn uc004_br012_m08_8_sections_by_line_nest_by_level() {
        let mut tab = tab(TEXT);

        let view = tab.sections_view("A", 0.0).unwrap();

        assert_eq!(
            view.sections,
            [
                SectionView {
                    line: 0,
                    last: 4,
                    folded: false
                },
                SectionView {
                    line: 3,
                    last: 4,
                    folded: false
                },
                SectionView {
                    line: 5,
                    last: 6,
                    folded: false
                },
            ]
        );
        assert!(tab.sections_view("A", 0.0).is_none(), "told once");
    }

    #[test]
    fn uc004_br012_m08_8_folding_moves_a_cursor_in_the_body_to_the_heading() {
        let mut tab = tab(TEXT);
        // The cursor on "two" (line 2).
        tab.editor.set_selection(10, 10);

        assert!(tab.fold_key(true));

        assert_eq!(tab.editor.cursor(), 3, "the end of `# A`");
        assert_eq!(folded_lines(&mut tab, "A"), [(0, 4)]);
        // Folding again changes nothing; expanding opens it.
        assert!(!tab.fold_key(true));
        tab.fold_key(false);
        assert!(folded_lines(&mut tab, "A").is_empty());
    }

    #[test]
    fn uc004_br012_m08_8_the_innermost_section_folds_and_up_down_step_over_it() {
        let mut tab = tab(TEXT);
        // The cursor on "three" (line 4), inside `## B` inside `# A`.
        tab.editor.set_selection(20, 20);

        tab.fold_key(true);

        assert_eq!(folded_lines(&mut tab, "A"), [(3, 4)]);
        assert_eq!(tab.cursor_line(), 3);
        let down = tab.editor.key(Key::ArrowDown, Modifiers::default(), 1);
        assert!(down.moved);
        assert_eq!(tab.cursor_line(), 5, "line 4 is folded away");
    }

    #[test]
    fn uc004_br012_m08_8_the_arrow_toggles_and_the_cursor_unfolds() {
        let mut tab = tab(TEXT);

        assert!(!tab.toggle_at_line(5), "the cursor is not in `# C`");
        assert!(!tab.toggle_at_line(1), "no heading there");
        assert_eq!(folded_lines(&mut tab, "A"), [(5, 6)]);

        // The cursor lands in the folded body: it expands.
        tab.editor.set_selection(30, 30);
        tab.unfold_at_cursor();
        assert!(folded_lines(&mut tab, "A").is_empty());
    }

    #[test]
    fn uc004_br012_m08_8_folds_follow_lines_added_above() {
        let mut tab = tab(TEXT);
        tab.toggle_at_line(5);
        tab.editor.set_selection(0, 0);

        let outcome = tab.editor.type_text("intro\n");
        tab.refold_after(&outcome);

        assert_eq!(folded_lines(&mut tab, "A"), [(6, 7)]);
        let heading = tab.editor.document().rope().line_to_char(6);
        tab.editor.set_selection(heading, heading);
        tab.editor.key(Key::ArrowDown, Modifiers::default(), 1);
        assert_eq!(tab.cursor_line(), 8, "line 7 is folded away");
    }

    #[test]
    fn m12_2_typing_finds_the_sections_again_at_most_every_300_ms() {
        let mut tab = tab(TEXT);
        let start = 1000.0;
        assert!(tab.sections_view("A", start).is_some());

        tab.editor.set_selection(0, 0);
        tab.editor.type_text("x");
        assert!(tab.sections_view("A", start).is_none(), "too soon");

        let view = tab.sections_view("A", start + SECTIONS_EVERY_MS).unwrap();
        assert_eq!(view.rev, revision(tab.editor.revision()));
    }

    #[test]
    fn uc004_br012_m08_8_a_renamed_folded_heading_unfolds_for_view_and_keys() {
        let mut tab = tab(TEXT);
        tab.toggle_at_line(5);
        // `# C` becomes `# D` without a new line.
        tab.editor.set_selection(26, 25);
        let outcome = tab.editor.type_text("D");
        tab.refold_after(&outcome);
        tab.unfold_at_cursor();

        assert!(
            folded_lines(&mut tab, "A").is_empty(),
            "the window shows it open"
        );
        tab.editor.set_selection(23, 23);
        tab.editor.key(Key::ArrowDown, Modifiers::default(), 1);
        assert_eq!(tab.cursor_line(), 6, "Down lands in the former body");
    }

    /// ADR-025: a time earlier than the clock (a host that asks with an
    /// older time) neither moves the throttle back nor blocks the sections:
    /// they come once 300 ms of the clock passed.
    #[test]
    fn adr025_an_earlier_time_does_not_throttle_anew() {
        let mut tab = tab(TEXT);
        assert!(tab.sections_view("A", 1000.0).is_some());
        tab.editor.set_selection(0, 0);
        tab.editor.type_text("x");

        assert!(tab.sections_view("A", 200.0).is_none(), "too soon");
        assert!(tab.sections_view("A", 1000.0 + SECTIONS_EVERY_MS).is_some());
        assert_eq!(tab.folds.found_at, Some(1000.0 + SECTIONS_EVERY_MS));
    }
}
