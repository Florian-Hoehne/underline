//! FR-081 to FR-086 in the facade: close pairs for brackets and quotes
//! (UC-005 A25, BR-019; inline markup closes only in the source view),
//! moving and duplicating blocks (A26, BR-020: a line is a block, a list
//! item goes with the items below it), indenting list items (A27, BR-021),
//! and going to a line (UC-004 A16, BR-017: to the block that holds it).
//!
//! What the facade types is escaped as it shows it (`type_text`), so a
//! bracket of a Markdown note may be written `\[`; the closing chars close
//! pairs wrote are the editor's, shared with the source view.

use ropey::Rope;
use underline_markup::blocks::BlockKind;
use underline_markup::NoteFormat;

use super::super::rules::escape_markdown;
use super::FacadeEdit;
use crate::comfort::goto::LineTarget;
use crate::comfort::lines::{self, Lines};
use crate::comfort::lists::{self, ListIndent};
use crate::comfort::pairs::{closer_of, closes_before, encloses};
use crate::comfort::TextEdit;

/// The char at `at` of `rope`, if any.
fn char_at(rope: &Rope, at: usize) -> Option<char> {
    (at < rope.len_chars()).then(|| rope.char(at))
}

impl FacadeEdit<'_> {
    /// A char `c` as the facade shows it at char `at` of the text: its
    /// markup and how many chars it takes (an escaped char takes two).
    fn shown_at(&self, at: usize) -> Option<(char, usize)> {
        let rope = self.rope();
        match char_at(rope, at)? {
            '\\' => Some(
                char_at(rope, at + 1)
                    .filter(char::is_ascii_punctuation)
                    .map_or(('\\', 1), |c| (c, 2)),
            ),
            c => Some((c, 1)),
        }
    }

    /// The char the facade shows just before char `at`, and where its
    /// markup starts.
    fn shown_before(&self, at: usize) -> Option<(char, usize)> {
        let c = char_at(self.rope(), at.checked_sub(1)?)?;
        let escaped =
            at >= 2 && c.is_ascii_punctuation() && char_at(self.rope(), at - 2) == Some('\\');
        Some((c, if escaped { at - 2 } else { at - 1 }))
    }

    /// UC-005 A25, BR-019: `text` typed with close pairs on; whether close
    /// pairs took it (else it is typed as usual).
    pub fn type_paired(&mut self, text: &str) -> bool {
        let mut chars = text.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return false;
        };
        let selection = self.editor.selection();
        let at = self.editor.cursor();
        if selection.is_empty() && self.editor.closers().contains(&at) {
            if let Some((_, width)) = self.shown_at(at).filter(|(shown, _)| *shown == c) {
                self.editor.take_closer(at);
                self.editor.place(at + width, at + width);
                self.gathered.moved(true);
                return true;
            }
        }
        let Some(close) = closer_of(c) else {
            // BR-019: the markup of a pair encloses a selection.
            return !selection.is_empty() && encloses(c, self.format) && self.enclose(c, c, true);
        };
        if !selection.is_empty() {
            return self.enclose(c, close, false);
        }
        let next = self.shown_at(at).map(|(c, _)| c);
        let fence = c == '`'
            && at >= 2
            && char_at(self.rope(), at - 1) == Some('`')
            && char_at(self.rope(), at - 2) == Some('`');
        if !closes_before(next) || fence {
            return false;
        }
        // One write of both, so the blocks follow one edit (also on the
        // empty lines between two blocks).
        let start = self.editor.cursor();
        self.type_text(&format!("{c}{close}"));
        let end = self.editor.cursor();
        let escaped = end >= start + 2
            && char_at(self.rope(), end - 2) == Some('\\')
            && char_at(self.rope(), end - 1) == Some(close);
        let between = end - if escaped { 2 } else { 1 };
        self.editor.place(between, between);
        self.editor.add_closer(between);
        self.gathered.moved(true);
        true
    }

    /// BR-019: the selection, within one block, enclosed in `open` and
    /// `close`, written as the facade writes typed chars, or, for the
    /// markup of a pair (`markup`), as markup, so `*` makes it emphasised;
    /// it stays selected, one step for undo. A selection across blocks, or
    /// markup in code, is replaced as usual.
    fn enclose(&mut self, open: char, close: char, markup: bool) -> bool {
        let selection = self.editor.selection();
        let (Some((first, from)), Some((last, _))) = (
            self.position_at(selection.start),
            self.position_at(selection.end),
        ) else {
            return false;
        };
        if first != last {
            return false;
        }
        let raw = matches!(
            &self.blocks[first].kind,
            BlockKind::Code { .. }
                | BlockKind::Markup
                | BlockKind::Image { .. }
                | BlockKind::Table(_)
                | BlockKind::Formula { .. }
        ) || super::in_formula(
            &self.blocks[first],
            self.rope().char_to_byte(selection.start),
        );
        if markup && raw {
            return false;
        }
        let markdown = self.markdown();
        let written = |c: char, start: bool| {
            if raw || markup || !markdown {
                c.to_string()
            } else {
                escape_markdown(&c.to_string(), start)
            }
        };
        let (open, close) = (written(open, from == 0), written(close, false));
        let selected = self.rope().slice(selection.clone()).to_string();
        let forward = self.editor.cursor() >= self.editor.anchor();
        self.editor.seal();
        self.replace(selection.clone(), &format!("{open}{selected}{close}"));
        self.editor.seal();
        let start = selection.start + open.chars().count();
        let end = start + selected.chars().count();
        let (cursor, anchor) = if forward { (end, start) } else { (start, end) };
        self.set_selection(cursor, anchor);
        true
    }

    /// UC-005 A25, BR-019: Backspace with close pairs on, right after an
    /// opening char whose closing char close pairs wrote right after it,
    /// takes both; whether it did.
    pub fn backspace_paired(&mut self) -> bool {
        let at = self.editor.cursor();
        if !self.editor.selection().is_empty() || !self.editor.closers().contains(&at) {
            return false;
        }
        let (Some((close, width)), Some((open, start))) =
            (self.shown_at(at), self.shown_before(at))
        else {
            return false;
        };
        if closer_of(open) != Some(close) {
            return false;
        }
        self.replace(start..at + width, "");
        true
    }

    /// Whether block `index` holds no text (an AsciiDoc `+` before the
    /// next item gives one): it is no unit and stands on no line of its own.
    fn void(&self, index: usize) -> bool {
        self.blocks[index].span.is_empty()
    }

    /// The first block from `index` on in the direction of `step` that
    /// holds text.
    fn solid(&self, index: usize, step: isize) -> Option<usize> {
        let mut at = index;
        loop {
            if at >= self.blocks.len() {
                return None;
            }
            if !self.void(at) {
                return Some(at);
            }
            at = at.checked_add_signed(step)?;
        }
    }

    /// The block at char `at`, by its index; one that holds text.
    fn block_at(&self, at: usize) -> Option<usize> {
        let (index, _) = self.position_at(at)?;
        self.solid(index, 1).or_else(|| self.solid(index, -1))
    }

    /// The depth of block `index` when it is a list item.
    fn item_depth(&self, index: usize) -> Option<usize> {
        match self.blocks.get(index)?.kind {
            BlockKind::ListItem { depth, .. } => Some(depth),
            _ => None,
        }
    }

    /// Whether block `index` is a further paragraph of a list item.
    fn further(&self, index: usize) -> bool {
        matches!(
            self.blocks[index].kind,
            BlockKind::ListItem { marker: None, .. }
        )
    }

    /// BR-020: the last block of the unit block `index` starts: a list
    /// item with its further paragraphs and the items below it.
    fn unit_end(&self, index: usize) -> usize {
        let Some(depth) = self.item_depth(index) else {
            return index;
        };
        let mut end = index;
        while end + 1 < self.blocks.len() {
            match self.item_depth(end + 1) {
                Some(next) if next > depth || (next == depth && self.further(end + 1)) => end += 1,
                _ => break,
            }
        }
        // A block of no text at its end belongs to what follows it.
        while end > index && self.void(end) {
            end -= 1;
        }
        end
    }

    /// BR-020: the first block of the unit that ends with block `last`,
    /// for a unit of `depth` (a list item's, else the top level).
    fn unit_start(&self, last: usize, depth: usize) -> usize {
        let mut at = last;
        while at > 0 {
            let Some(here) = self.item_depth(at) else {
                break;
            };
            let below = here > depth || (here == depth && self.further(at)) || self.void(at);
            if !below || self.item_depth(at - 1).is_none() {
                break;
            }
            at -= 1;
        }
        self.solid(at, 1)
            .filter(|solid| *solid <= last)
            .unwrap_or(last)
    }

    /// The lines blocks `first` to `last` stand on; a line `+` of AsciiDoc
    /// right before them belongs to them, as it joins them to the item
    /// before. `None` when they hold no text.
    fn lines_of(&self, first: usize, last: usize) -> Option<Lines> {
        let rope = self.rope();
        let mut lines: Option<Lines> = None;
        for index in first..=last {
            if self.void(index) {
                continue;
            }
            let span = &self.blocks[index].span;
            let from = self.char_of_byte(span.start);
            let mut end = self.char_of_byte(span.end).max(from);
            while end > from && rope.char(end - 1).is_whitespace() {
                end -= 1;
            }
            let top = rope.char_to_line(from);
            let bottom = rope.char_to_line(end.saturating_sub(1).max(from)).max(top);
            lines = Some(lines.map_or((top, bottom), |(a, b)| (a.min(top), b.max(bottom))));
        }
        let (mut top, bottom) = lines?;
        if self.format == NoteFormat::AsciiDoc
            && top > 0
            && crate::comfort::line_text(rope, top - 1).trim() == "+"
        {
            top -= 1;
        }
        Some((top, bottom))
    }

    /// The blocks the selection touches, widened to whole units.
    fn selected_units(&self) -> Option<(usize, usize)> {
        let selection = self.editor.selection();
        let first = self.block_at(selection.start)?;
        let (mut last, at) = self.position_at(selection.end)?;
        if !selection.is_empty() && at == 0 && last > first {
            last -= 1;
        }
        let last = self
            .solid(last, -1)
            .filter(|last| *last >= first)
            .unwrap_or(first);
        Some((first, self.unit_end(last)))
    }

    /// Applies `edit`, the blocks following it, as one step for undo.
    fn apply_step(&mut self, edit: &TextEdit) {
        self.editor.seal();
        self.replace(edit.range.clone(), &edit.text);
        self.set_selection(edit.cursor, edit.anchor);
        self.gathered.moved(true);
        self.editor.seal();
    }

    /// BR-020, OQ-255: whether a blank line must stand between block `a`
    /// and block `b` after it, so neither joins the other: always, but
    /// between two list items.
    fn apart(&self, a: usize, b: usize) -> bool {
        self.item_depth(a).is_none() || self.item_depth(b).is_none()
    }

    /// Whether the text between line `upper` and line `lower` holds a
    /// blank line.
    fn blank_between(&self, upper: usize, lower: usize) -> bool {
        (upper + 1..lower).any(|line| {
            crate::comfort::line_text(self.rope(), line)
                .trim()
                .is_empty()
        })
    }

    /// BR-020, OQ-255: the line breaks that keep the blocks apart after the
    /// units `upper` and `lower` (first and last blocks, with their lines)
    /// change places.
    fn joints(&self, upper: (usize, usize, Lines), lower: (usize, usize, Lines)) -> lines::Joints {
        let br = lines::line_break(self.rope(), upper.2 .1);
        let needed = |apart: bool, blank: bool| {
            if apart && !blank {
                br.clone()
            } else {
                String::new()
            }
        };
        let before = upper
            .0
            .checked_sub(1)
            .and_then(|at| self.solid(at, -1))
            .and_then(|at| Some((at, self.lines_of(at, at)?)));
        let after = self
            .solid(lower.1 + 1, 1)
            .and_then(|at| Some((at, self.lines_of(at, at)?)));
        lines::Joints {
            before: before.map_or_else(String::new, |(at, lines)| {
                needed(
                    self.apart(at, lower.0),
                    self.blank_between(lines.1, upper.2 .0),
                )
            }),
            between: needed(
                self.apart(lower.1, upper.0),
                self.blank_between(upper.2 .1, lower.2 .0),
            ),
            after: after.map_or_else(String::new, |(at, lines)| {
                needed(
                    self.apart(upper.1, at),
                    self.blank_between(lower.2 .1, lines.0),
                )
            }),
        }
    }

    /// UC-005 A26, BR-020: the blocks at the cursor go up or down past the
    /// block (or list item with its items) next to them; nothing at the
    /// first or the last. Blank lines are added where a block would
    /// otherwise join the one next to it (OQ-255).
    pub fn move_blocks(&mut self, up: bool) {
        let Some((first, last)) = self.selected_units() else {
            return;
        };
        let depth = self.item_depth(first).unwrap_or(0);
        let selection = (self.editor.cursor(), self.editor.anchor());
        let (upper, lower) = if up {
            let Some(before) = first.checked_sub(1).and_then(|at| self.solid(at, -1)) else {
                return;
            };
            ((self.unit_start(before, depth), before), (first, last))
        } else {
            let Some(after) = self.solid(last + 1, 1) else {
                return;
            };
            ((first, last), (after, self.unit_end(after)))
        };
        let (Some(upper_lines), Some(lower_lines)) = (
            self.lines_of(upper.0, upper.1),
            self.lines_of(lower.0, lower.1),
        ) else {
            return;
        };
        let joints = self.joints(
            (upper.0, upper.1, upper_lines),
            (lower.0, lower.1, lower_lines),
        );
        let Some(edit) = lines::swap_joined(
            self.rope(),
            upper_lines,
            lower_lines,
            selection,
            up,
            &joints,
        ) else {
            return;
        };
        self.apply_step(&edit);
    }

    /// UC-005 A26, BR-020: a copy of the blocks at the cursor goes below
    /// them, a list item's on the next line, other blocks after a blank
    /// line, with the note's line break (NFR-007); the cursor goes into the
    /// copy.
    pub fn duplicate_blocks(&mut self) {
        let Some((first, last)) = self.selected_units() else {
            return;
        };
        let Some(lines) = self.lines_of(first, last) else {
            return;
        };
        let br = lines::line_break(self.rope(), lines.1);
        let separator = if self.item_depth(first).is_some() {
            br
        } else {
            format!("{br}{br}")
        };
        let selection = (self.editor.cursor(), self.editor.anchor());
        let edit = lines::duplicate(self.rope(), lines, Some(&separator), selection);
        self.apply_step(&edit);
    }

    /// UC-005 A27, BR-021: Tab (`outdent` false) or Shift+Tab on the list
    /// items at the cursor; whether the cursor is in a list item (else the
    /// key keeps its meaning: nothing in the facade, M-09.13).
    pub fn indent_items(&mut self, outdent: bool) -> bool {
        let (cursor, anchor) = (self.editor.cursor(), self.editor.anchor());
        match lists::indent(self.rope(), cursor, anchor, self.format, outdent) {
            ListIndent::NotList => false,
            ListIndent::Unchanged => true,
            ListIndent::Edit(edit) => {
                self.apply_step(&edit);
                true
            }
        }
    }

    /// UC-004 A16, BR-017: the cursor to the block that holds the line of
    /// `target`, at that line and column (a column beyond the line at its
    /// end) where the facade shows it; a place in markup the facade does not
    /// show goes to the nearest char it shows.
    pub fn go_to_line(&mut self, target: LineTarget) {
        let place = target.char_in(self.rope());
        let at = match self.position_at(place) {
            Some((index, at)) => self.char_of(index, at),
            None => place,
        };
        self.set_selection(at, at);
        self.gathered.moved(true);
    }
}
