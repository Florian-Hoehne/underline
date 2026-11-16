//! FR-083, UC-005 A27, BR-021, OQ-204: Tab and Shift+Tab indent and outdent
//! the list items at the cursor or in the selection by one level, each with
//! the items below it.
//!
//! - Markdown: an item is indented to the text of the item before it on its
//!   level (`- a` puts `- b` two spaces in), and outdented to the indent of
//!   the item it belongs to; its lines and the items below it move alike.
//! - AsciiDoc: an item gets one more `*` or `.`, or one less; the items
//!   below it too. A `-` item has a single level and stays (OQ-253).
//! - An item without an item before it on its level is not indented, an
//!   item on the top level is not outdented ([`ListIndent::Unchanged`]).
//! - Outside a list item the keys keep the meaning they had
//!   ([`ListIndent::NotList`]); lines of fenced code hold no items.
//!
//! The list is found by its lines, the same way in both editing views; the
//! edit is one replacement of the lines it changes, so one step for undo.

use std::collections::BTreeMap;

use ropey::Rope;
use underline_markup::NoteFormat;

use super::code::code_lines;
use super::{content_end, line_of, line_start, line_text, touched_lines, TextEdit};

/// What Tab or Shift+Tab does with the list items at the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListIndent {
    /// The cursor is in no list item: the key keeps its meaning.
    NotList,
    /// The items cannot go deeper or higher: nothing changes.
    Unchanged,
    /// The edit that indents or outdents them.
    Edit(TextEdit),
}

/// A list item's first line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    /// Columns of space before the marker (a tab counts four).
    indent: usize,
    /// Chars of space before the marker.
    lead: usize,
    /// Markdown: the column its text starts at.
    text_col: usize,
    /// AsciiDoc: how many marker chars it has; 0 in Markdown.
    level: usize,
    /// Its marker: `-`, `*`, `+` or `.`, or `1` for a number.
    marker: char,
}

/// The columns of the space `lead`.
fn columns(lead: &str) -> usize {
    columns_from(0, lead)
}

/// The column after `text` written from column `start`: a tab goes on to
/// the next tab stop (every four columns), as editors show it.
fn columns_from(start: usize, text: &str) -> usize {
    text.chars().fold(start, |col, c| {
        if c == '\t' {
            (col / TAB + 1) * TAB
        } else {
            col + 1
        }
    })
}

/// The columns between two tab stops.
const TAB: usize = 4;

/// The deepest level of an AsciiDoc list (`*****`, `.....`).
const DEEPEST: usize = 5;

/// The space at the start of `line`, in chars.
fn lead_of(line: &str) -> usize {
    line.chars().take_while(|c| matches!(c, ' ' | '\t')).count()
}

/// A thematic break (`* * *`, `---`) is no item.
fn thematic_break(rest: &str) -> bool {
    let mut marks = rest.chars().filter(|c| !c.is_whitespace());
    let Some(first) = marks.next() else {
        return false;
    };
    let count = rest.chars().filter(|c| *c == first).count();
    matches!(first, '-' | '*' | '_') && marks.all(|c| c == first) && count >= 3
}

fn markdown_item(line: &str) -> Option<Item> {
    let lead = lead_of(line);
    // The space is ASCII: its chars are its bytes.
    let (space, rest) = line.split_at(lead);
    let (marker, width) = match rest.chars().next()? {
        c @ ('-' | '*' | '+') => (c, 1),
        _ => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            let delimiter = rest[digits..].chars().next()?;
            if digits == 0 || digits > 9 || !matches!(delimiter, '.' | ')') {
                return None;
            }
            ('1', digits + 1)
        }
    };
    let after = &rest[width..];
    let gap = lead_of(after);
    if gap == 0 && !after.is_empty() {
        return None;
    }
    if marker != '1' && thematic_break(rest) {
        return None;
    }
    let indent = columns(space);
    let marked = indent + width;
    let text_col = if after.trim().is_empty() || gap > 4 {
        marked + 1
    } else {
        columns_from(marked, &after[..gap])
    };
    Some(Item {
        indent,
        lead,
        text_col,
        level: 0,
        marker,
    })
}

fn asciidoc_item(line: &str) -> Option<Item> {
    let lead = lead_of(line);
    let (space, rest) = line.split_at(lead);
    let marker = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '*' | '.' | '-'))?;
    let level = rest.chars().take_while(|c| *c == marker).count();
    if level > DEEPEST || (marker == '-' && level > 1) {
        return None;
    }
    let after = &rest[level..];
    let gap = lead_of(after);
    (gap > 0 && !after.trim().is_empty()).then(|| Item {
        indent: columns(space),
        lead,
        text_col: 0,
        level,
        marker,
    })
}

/// A line that ends the lazy continuation of a Markdown item.
fn markdown_interrupts(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with(['#', '>', '|', '<'])
        || trimmed.starts_with("```")
        || trimmed.starts_with("~~~")
        || thematic_break(trimmed)
}

/// A line that ends an AsciiDoc item's paragraph.
fn asciidoc_interrupts(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with(['=', '[', '|', ':', '/', '.']) || code_delimiter(trimmed)
}

fn code_delimiter(text: &str) -> bool {
    let trimmed = text.trim_end();
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    trimmed.chars().count() >= 4
        && matches!(first, '-' | '.' | '+' | '/' | '*' | '_' | '=')
        && trimmed.chars().all(|c| c == first)
}

/// One change of a line's start: `count` chars from char `at` are
/// replaced by `text` (an insertion when `count` is 0, a removal when
/// `text` is empty).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Op {
    at: usize,
    count: usize,
    text: String,
}

impl Op {
    fn insert(at: usize, text: String) -> Self {
        Self { at, count: 0, text }
    }

    fn delta(&self) -> isize {
        let signed = |n: usize| isize::try_from(n).unwrap_or(isize::MAX);
        signed(self.text.chars().count()) - signed(self.count)
    }

    /// Column `col` of the line after the change; a column within the
    /// replaced chars goes to the end of what replaced them.
    fn column(&self, col: usize) -> usize {
        let length = self.text.chars().count();
        if col >= self.at + self.count {
            col - self.count + length
        } else if col > self.at {
            self.at + length
        } else {
            col
        }
    }

    fn apply(&self, line: &str) -> String {
        let chars: Vec<char> = line.chars().collect();
        let at = self.at.min(chars.len());
        let end = (at + self.count).min(chars.len());
        let mut out: String = chars[..at].iter().collect();
        out.push_str(&self.text);
        out.extend(&chars[end..]);
        out
    }
}

/// The lists of one note: its lines and their items read once, the
/// extents of the items worked out once each.
struct Lists {
    format: NoteFormat,
    /// The lines of fenced code.
    code: Vec<bool>,
    /// The text of each line, without its line break.
    texts: Vec<String>,
    /// The item each line starts, if any.
    items: Vec<Option<Item>>,
    /// The extents found so far, by line.
    extents: std::cell::RefCell<Vec<Option<usize>>>,
}

impl Lists {
    fn of(rope: &Rope, format: NoteFormat) -> Self {
        let count = rope.len_lines();
        let code = code_lines(rope, format, count.saturating_sub(1));
        let texts: Vec<String> = (0..count).map(|line| line_text(rope, line)).collect();
        let items = texts
            .iter()
            .zip(&code)
            .map(|(text, code)| match (code, format) {
                (true, _) => None,
                (false, NoteFormat::Markdown) => markdown_item(text),
                (false, NoteFormat::AsciiDoc) => asciidoc_item(text),
            })
            .collect();
        Self {
            format,
            code,
            texts,
            items,
            extents: std::cell::RefCell::new(vec![None; count]),
        }
    }

    fn count(&self) -> usize {
        self.texts.len()
    }

    fn text(&self, line: usize) -> &str {
        self.texts.get(line).map_or("", String::as_str)
    }

    fn blank(&self, line: usize) -> bool {
        self.text(line).trim().is_empty()
    }

    fn item(&self, line: usize) -> Option<Item> {
        self.items.get(line).copied().flatten()
    }

    /// The depth `item` stands at, to compare with others.
    fn depth(&self, item: &Item) -> usize {
        match self.format {
            NoteFormat::Markdown => item.indent,
            NoteFormat::AsciiDoc => item.level,
        }
    }

    /// The last line of the item on `line` with the items below it and
    /// its further lines.
    fn extent(&self, line: usize, item: &Item) -> usize {
        if let Some(Some(end)) = self.extents.borrow().get(line) {
            return *end;
        }
        let end = self.find_extent(line, item);
        if let Some(slot) = self.extents.borrow_mut().get_mut(line) {
            *slot = Some(end);
        }
        end
    }

    fn find_extent(&self, line: usize, item: &Item) -> usize {
        let depth = self.depth(item);
        let mut end = line;
        let mut at = line + 1;
        while at < self.count() {
            if let Some(next) = self.item(at) {
                if self.depth(&next) <= depth {
                    break;
                }
                end = at;
                at += 1;
                continue;
            }
            let text = self.text(at);
            if self.blank(at) {
                let mut next = at + 1;
                while next < self.count() && self.blank(next) {
                    next += 1;
                }
                let goes_on = next < self.count()
                    && match self.format {
                        NoteFormat::Markdown => columns(&text_lead(self.text(next))) > depth,
                        NoteFormat::AsciiDoc => {
                            self.item(next).is_some_and(|i| self.depth(&i) > depth)
                        }
                    };
                if !goes_on {
                    break;
                }
                at = next;
                continue;
            }
            let follows = at == end + 1;
            let belongs = match self.format {
                NoteFormat::Markdown => {
                    columns(&text_lead(text)) > depth
                        || self.code[at]
                        || (follows && !markdown_interrupts(text))
                }
                NoteFormat::AsciiDoc => {
                    text.trim() == "+"
                        || (follows && self.text(end).trim() == "+")
                        || (follows && !asciidoc_interrupts(text))
                }
            };
            if !belongs {
                break;
            }
            end = at;
            at += 1;
        }
        end
    }

    /// The nearest item, from `line` back, whose lines hold `line`.
    fn owner(&self, line: usize) -> Option<(usize, Item)> {
        let mut above: Option<usize> = None;
        let mut at = line;
        loop {
            if let Some(item) = self.item(at) {
                let depth = self.depth(&item);
                if above.is_none_or(|limit| depth < limit) {
                    if self.extent(at, &item) >= line {
                        return Some((at, item));
                    }
                    if depth == 0 || (self.format == NoteFormat::AsciiDoc && depth == 1) {
                        return None;
                    }
                    above = Some(depth);
                }
            }
            at = at.checked_sub(1)?;
        }
    }

    /// BR-021: the item before the item on `line` on its level, if any.
    fn sibling(&self, line: usize, item: &Item) -> Option<(usize, Item)> {
        let depth = self.depth(item);
        let mut at = line.checked_sub(1)?;
        loop {
            if let Some(found) = self.item(at) {
                let other = self.depth(&found);
                if other < depth {
                    return None;
                }
                if other == depth {
                    let same = self.format == NoteFormat::Markdown || found.marker == item.marker;
                    let end = self.extent(at, &found);
                    let joined = (end + 1..line).all(|between| self.blank(between));
                    return (same && joined).then_some((at, found));
                }
            }
            at = at.checked_sub(1)?;
        }
    }

    /// BR-021: the item the item on `line` belongs to, if any.
    fn parent(&self, line: usize, item: &Item) -> Option<Item> {
        let depth = self.depth(item);
        let mut at = line.checked_sub(1)?;
        loop {
            if let Some(found) = self.item(at) {
                if self.depth(&found) < depth {
                    return (self.extent(at, &found) >= line).then_some(found);
                }
            }
            at = at.checked_sub(1)?;
        }
    }

    /// The changes of the lines of the item on `line`, for Tab or
    /// Shift+Tab; none when it cannot go deeper or higher.
    fn ops(&self, line: usize, item: &Item, outdent: bool, into: &mut BTreeMap<usize, Op>) {
        let end = self.extent(line, item);
        match (self.format, outdent) {
            (NoteFormat::Markdown, false) => {
                let Some((_, sibling)) = self.sibling(line, item) else {
                    return;
                };
                let by = sibling.text_col.saturating_sub(item.indent).max(1);
                for at in line..=end {
                    if !self.blank(at) {
                        // After the space already there, so a tab before
                        // it does not swallow the new columns.
                        let lead = lead_of(self.text(at));
                        into.insert(at, Op::insert(lead, " ".repeat(by)));
                    }
                }
            }
            (NoteFormat::Markdown, true) => {
                let Some(parent) = self.parent(line, item) else {
                    return;
                };
                let by = item.indent - parent.indent;
                for at in line..=end {
                    if let Some(op) = outdented(self.text(at), by) {
                        into.insert(at, op);
                    }
                }
            }
            (NoteFormat::AsciiDoc, false) => {
                if item.marker == '-' || self.sibling(line, item).is_none() {
                    return;
                }
                let items: Vec<(usize, Item)> = (line..=end)
                    .filter_map(|at| self.item(at).map(|each| (at, each)))
                    .filter(|(_, each)| each.marker != '-')
                    .collect();
                // OQ-253: no item goes past the fifth level, which would
                // make it plain text.
                if items.iter().any(|(_, each)| each.level >= DEEPEST) {
                    return;
                }
                for (at, each) in items {
                    into.insert(at, Op::insert(each.lead, each.marker.to_string()));
                }
            }
            (NoteFormat::AsciiDoc, true) => {
                if item.level < 2 {
                    return;
                }
                for at in line..=end {
                    if let Some(each) = self.item(at).filter(|i| i.level >= 2) {
                        into.insert(
                            at,
                            Op {
                                at: each.lead,
                                count: 1,
                                text: String::new(),
                            },
                        );
                    }
                }
            }
        }
    }
}

/// The space at the start of `text`.
fn text_lead(text: &str) -> String {
    text.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .collect()
}

/// The change that takes `by` columns from the space starting `text`;
/// `None` when it has none. Spaces alone are taken away; a space that
/// holds a tab is written anew as spaces, so a tab wider than `by` does not
/// take more than `by` columns.
fn outdented(text: &str, by: usize) -> Option<Op> {
    let lead = text_lead(text);
    if lead.is_empty() {
        return None;
    }
    if !lead.contains('\t') {
        let count = by.min(lead.chars().count());
        return Some(Op {
            at: 0,
            count,
            text: String::new(),
        });
    }
    let kept = columns(&lead).saturating_sub(by);
    Some(Op {
        at: 0,
        count: lead.chars().count(),
        text: " ".repeat(kept),
    })
}

/// UC-005 A27, BR-021: Tab (`outdent` false) or Shift+Tab with the
/// selection from `cursor` to `anchor` in `rope`, a note in `format`.
#[must_use]
pub fn indent(
    rope: &Rope,
    cursor: usize,
    anchor: usize,
    format: NoteFormat,
    outdent: bool,
) -> ListIndent {
    let lists = Lists::of(rope, format);
    let (first, last) = touched_lines(rope, cursor, anchor);
    let Some(owner) = lists.owner(first) else {
        return ListIndent::NotList;
    };
    // The items the selection touches; one below another chosen moves
    // with it, not by itself.
    let mut roots: Vec<(usize, Item, usize)> = Vec::new();
    let candidates = std::iter::once(owner)
        .chain((first + 1..=last).filter_map(|line| lists.item(line).map(|item| (line, item))));
    for (line, item) in candidates {
        if roots.iter().any(|&(at, _, end)| at < line && line <= end) {
            continue;
        }
        roots.push((line, item, lists.extent(line, &item)));
    }
    let mut ops = BTreeMap::new();
    for (line, item, _) in &roots {
        lists.ops(*line, item, outdent, &mut ops);
    }
    match edit_of(rope, &ops, cursor, anchor) {
        Some(edit) => ListIndent::Edit(edit),
        None => ListIndent::Unchanged,
    }
}

/// OQ-204, OQ-256: Shift+Tab in the source view outside a list item: the
/// lines the selection touches lose up to one tab stop of the space they
/// start with, as one edit; `None` when none has any.
#[must_use]
pub fn outdent_lines(rope: &Rope, cursor: usize, anchor: usize) -> Option<TextEdit> {
    let (first, last) = touched_lines(rope, cursor, anchor);
    let ops: BTreeMap<usize, Op> = (first..=last)
        .filter_map(|line| outdented(&line_text(rope, line), TAB).map(|op| (line, op)))
        .filter(|(_, op)| op.delta() != 0)
        .collect();
    edit_of(rope, &ops, cursor, anchor)
}

/// The edit of `ops`, with the selection moved along; `None` without ops.
fn edit_of(
    rope: &Rope,
    ops: &BTreeMap<usize, Op>,
    cursor: usize,
    anchor: usize,
) -> Option<TextEdit> {
    let (&first, _) = ops.first_key_value()?;
    let (&last, _) = ops.last_key_value()?;
    let mut text = String::new();
    for line in first..=last {
        let content = line_text(rope, line);
        match ops.get(&line) {
            Some(op) => text.push_str(&op.apply(&content)),
            None => text.push_str(&content),
        }
        if line < last {
            let end = content_end(rope, line);
            text.extend(rope.slice(end..line_start(rope, line + 1)).chars());
        }
    }
    let total: isize = ops.values().map(Op::delta).sum();
    let map = |at: usize| -> usize {
        let line = line_of(rope, at);
        if line < first {
            return at;
        }
        if line > last {
            return at.saturating_add_signed(total);
        }
        let before: isize = ops.range(..line).map(|(_, op)| op.delta()).sum();
        let col = at - line_start(rope, line);
        let col = ops.get(&line).map_or(col, |op| op.column(col));
        (line_start(rope, line) + col).saturating_add_signed(before)
    };
    Some(TextEdit {
        range: line_start(rope, first)..content_end(rope, last),
        text,
        cursor: map(cursor),
        anchor: map(anchor),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: NoteFormat = NoteFormat::Markdown;
    const ADOC: NoteFormat = NoteFormat::AsciiDoc;

    /// The text after `indent` at the char of `marker` (selected up to
    /// `to`), with the selection after it.
    fn tab(
        text: &str,
        from: &str,
        to: Option<&str>,
        format: NoteFormat,
        outdent: bool,
    ) -> (String, usize, usize) {
        let rope = Rope::from_str(text);
        let at = |marker: &str| rope.byte_to_char(text.find(marker).expect("marker"));
        let anchor = at(from);
        let cursor = to.map_or(anchor, at);
        match indent(&rope, cursor, anchor, format, outdent) {
            ListIndent::Edit(edit) => {
                let mut changed = rope.clone();
                changed.remove(edit.range.clone());
                changed.insert(edit.range.start, &edit.text);
                (changed.to_string(), edit.cursor, edit.anchor)
            }
            other => panic!("no edit: {other:?}"),
        }
    }

    fn outcome(text: &str, at: &str, format: NoteFormat, outdent: bool) -> ListIndent {
        let rope = Rope::from_str(text);
        let at = rope.byte_to_char(text.find(at).expect("marker"));
        indent(&rope, at, at, format, outdent)
    }

    /// TC-005-37 step 1: Tab in "b" indents `- b` to the text of "a".
    #[test]
    fn tc_005_37_tab_makes_an_item_an_item_of_the_one_before() {
        let (text, cursor, _) = tab("- a\n- b\n- c\n", "b", None, MD, false);
        assert_eq!(text, "- a\n  - b\n- c\n");
        assert_eq!(cursor, 8, "still before the b");
        let (numbered, ..) = tab("1. a\n2. b\n", "b", None, MD, false);
        assert_eq!(numbered, "1. a\n   2. b\n");
    }

    /// TC-005-37 step 2, BR-021: with "b" already below "a", Tab on "b" and
    /// "c" makes "c" an item of "a" too; "b" has no item before it on its
    /// level and stays.
    #[test]
    fn tc_005_37_each_selected_item_goes_at_most_one_level_deeper() {
        let (text, ..) = tab("- a\n  - b\n- c\n", "b", Some("c\n"), MD, false);
        assert_eq!(text, "- a\n  - b\n  - c\n");
    }

    /// TC-005-37 step 3: Shift+Tab twice: back to the top level, then
    /// nothing changes.
    #[test]
    fn tc_005_37_shift_tab_outdents_until_the_top_level() {
        let (text, cursor, anchor) = tab("- a\n  - b\n  - c\n", "b", Some("c\n"), MD, true);
        assert_eq!(text, "- a\n- b\n- c\n");
        assert_eq!((anchor, cursor), (6, 10), "on b and on c, as before");
        assert_eq!(outcome("- a\n- b\n", "b", MD, true), ListIndent::Unchanged);
    }

    /// TC-005-37 step 4: the first item has no item before it.
    #[test]
    fn tc_005_37_the_first_item_is_not_indented() {
        assert_eq!(outcome("- a\n- b\n", "a", MD, false), ListIndent::Unchanged);
        assert_eq!(
            outcome("* a\n* b\n", "a", ADOC, false),
            ListIndent::Unchanged
        );
    }

    /// TC-005-37 step 5: AsciiDoc writes the level with one more `*`, the
    /// items below with it; Shift+Tab takes one away.
    #[test]
    fn tc_005_37_asciidoc_counts_the_markers() {
        let (text, ..) = tab("* a\n* b\n** b1\n* c\n", "b\n", None, ADOC, false);
        assert_eq!(text, "* a\n** b\n*** b1\n* c\n");
        let (back, ..) = tab(&text, "b\n", None, ADOC, true);
        assert_eq!(back, "* a\n* b\n** b1\n* c\n");
        let (ordered, ..) = tab(". one\n. twö\n", "twö", None, ADOC, false);
        assert_eq!(ordered, ". one\n.. twö\n");
        assert_eq!(
            outcome("- a\n- b\n", "b", ADOC, false),
            ListIndent::Unchanged
        );
        assert_eq!(
            outcome("* a\n* b\n", "b", ADOC, true),
            ListIndent::Unchanged
        );
    }

    /// BR-021: an item goes with the items below it and its further lines;
    /// a selection of an item and its own item moves them once.
    #[test]
    fn br021_an_item_moves_with_its_items_and_lines() {
        let text = "- a\n- b\n  more of b\n  - b1\n\n    b1 again\n- c\n";
        let (once, ..) = tab(text, "b\n", Some("b1\n"), MD, false);
        assert_eq!(
            once,
            "- a\n  - b\n    more of b\n    - b1\n\n      b1 again\n- c\n"
        );
        let (back, ..) = tab(&once, "b\n", None, MD, true);
        assert_eq!(back, text);
    }

    /// BR-021, OQ-204: outside a list item, in code, after a list, and on a
    /// thematic break, Tab keeps its meaning.
    #[test]
    fn br021_outside_a_list_the_key_keeps_its_meaning() {
        assert_eq!(
            outcome("para\n- a\n", "para", MD, false),
            ListIndent::NotList
        );
        assert_eq!(
            outcome("```\n- a\n- b\n```\n", "b", MD, false),
            ListIndent::NotList
        );
        assert_eq!(
            outcome("- a\n\nafter\n", "after", MD, false),
            ListIndent::NotList
        );
        assert_eq!(
            outcome("- a\n* * *\n", "* * *", MD, false),
            ListIndent::NotList
        );
        assert_eq!(
            outcome("----\n* a\n* b\n----\n", "b", ADOC, false),
            ListIndent::NotList
        );
        assert!(matches!(
            outcome("- a\nlazy\n", "lazy", MD, false),
            ListIndent::Unchanged
        ));
    }

    /// BR-021: the cursor in a further paragraph of an item finds its item,
    /// also below an item of its own.
    #[test]
    fn br021_a_further_paragraph_belongs_to_its_item() {
        let text = "- a\n- b\n  - b1\n\n  para of b\n";
        let (changed, ..) = tab(text, "para", None, MD, false);
        assert_eq!(changed, "- a\n  - b\n    - b1\n\n    para of b\n");
    }

    /// The selection keeps its place in the text with multibyte chars and
    /// tabs as indent.
    #[test]
    fn multibyte_text_and_tabs() {
        let (text, cursor, _) = tab("- 😀 a\n- é b\n", "b", None, MD, false);
        assert_eq!(text, "- 😀 a\n  - é b\n");
        assert_eq!(cursor, 12);
        let (tabs, ..) = tab("- a\n\t- b\n", "b", None, MD, true);
        assert_eq!(tabs, "- a\n- b\n");
    }

    /// The columns of the space starting `line`, tab stops every four.
    fn visual_indent(line: &str) -> usize {
        let mut col = 0;
        for c in line.chars() {
            match c {
                ' ' => col += 1,
                '\t' => col = (col / 4 + 1) * 4,
                _ => break,
            }
        }
        col
    }

    /// TC-005-37, BR-021 (test-monkey): Tab on an item whose sibling is
    /// indented with a tab goes deeper than the sibling. `  \t- c` is as
    /// deep as `\t- b` (a tab ends at the next tab stop), so the edit
    /// changed the text and not the list.
    #[test]
    fn tc_005_37_tab_after_a_tab_indented_sibling_goes_deeper() {
        let (changed, ..) = tab("- a\n\t- b\n\t- c", "c", None, MD, false);
        let lines: Vec<&str> = changed.lines().collect();
        assert!(
            visual_indent(lines[2]) > visual_indent(lines[1]),
            "c is deeper than b in {changed:?}"
        );
    }

    /// TC-005-37, BR-021, OQ-253 (test-monkey): AsciiDoc has five levels of
    /// `*` and `.`; Tab on an item of the fifth level wrote a sixth marker
    /// char, which is no list item (this crate's own `asciidoc_item` refuses
    /// it), so the item turned into plain text.
    #[test]
    fn tc_005_37_asciidoc_tab_never_writes_a_sixth_level() {
        // Changed after test-monkey: the helper `tab` asserted an edit, but
        // an item that cannot go deeper is left as it is (BR-021, as for an
        // item with no item before it), so `Unchanged` is the answer.
        for marker in ["*", "."] {
            let five = marker.repeat(5);
            let text = format!("{five} a\n{five} b");
            assert_eq!(outcome(&text, "b", ADOC, false), ListIndent::Unchanged);
            let four = marker.repeat(4);
            let nested = format!("{four} a\n{four} b\n{five} b1");
            match outcome(&nested, "b\n", ADOC, false) {
                ListIndent::Edit(edit) => {
                    assert!(!edit.text.contains(&marker.repeat(6)), "{edit:?}");
                }
                ListIndent::Unchanged => {}
                ListIndent::NotList => panic!("a list item"),
            }
        }
    }

    /// OQ-256: Shift+Tab outside a list takes up to one tab stop of space
    /// from each line; a line without space stays.
    #[test]
    fn oq256_shift_tab_outside_a_list_outdents_the_lines() {
        let rope = Rope::from_str("      deep\nnone\n\tтаб\n");
        let edit = outdent_lines(&rope, 0, 17).expect("an edit");
        assert_eq!(edit.text, "  deep\nnone\nтаб");
        assert_eq!(outdent_lines(&Rope::from_str("none"), 1, 1), None);
    }
}
