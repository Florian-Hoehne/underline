//! FR-081, UC-005 A25, BR-019, OQ-202: close pairs in the source view.
//!
//! - `(`, `[`, `{`, `"` and `` ` `` write their counterpart after the cursor
//!   when the cursor is at the end of the line, before a space, or before a
//!   closing bracket.
//! - `**` and `~~` typed within a line (after text on it) write the same
//!   pair; a single `*`, `_` or `~` (in AsciiDoc also `#`) only encloses a
//!   selection. Inline markup is never closed in code.
//! - With text selected, an opening bracket, a quote, or the markup of a
//!   pair encloses the selection, which stays selected.
//! - A closing char typed right before the same char that close pairs wrote
//!   moves over it; Backspace right after an opening char takes the closing
//!   one written with it too.
//! - OQ-252: a backtick typed after two backticks is typed as it is, so a
//!   code fence gets its three backticks.
//!
//! Which closing chars close pairs wrote is the editor's to remember (their
//! places move with every edit, `SourceEditor`); the rules here only read
//! them.

use ropey::Rope;
use underline_markup::NoteFormat;

use super::code::in_code;
use super::{line_start, TextEdit};

/// The closing counterpart of an opening bracket or a quote.
#[must_use]
pub fn closer_of(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '`' => Some('`'),
        _ => None,
    }
}

/// BR-019: whether a closing char is written before `next`, the char after
/// the cursor: at the end of the line or text, before a space, or before a
/// closing bracket.
#[must_use]
pub fn closes_before(next: Option<char>) -> bool {
    next.is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ']' | '}'))
}

/// BR-019: the inline markup that encloses a selection in `format`.
#[must_use]
pub fn encloses(c: char, format: NoteFormat) -> bool {
    match format {
        NoteFormat::Markdown => matches!(c, '*' | '_' | '~'),
        NoteFormat::AsciiDoc => matches!(c, '*' | '_' | '~' | '#'),
    }
}

/// What the text and the selection are to close pairs.
#[derive(Debug, Clone, Copy)]
pub struct PairContext<'a> {
    /// The text.
    pub rope: &'a Rope,
    /// The cursor, in chars.
    pub cursor: usize,
    /// The other end of the selection.
    pub anchor: usize,
    /// The places of the closing chars close pairs wrote.
    pub closers: &'a [usize],
    /// The note's format.
    pub format: NoteFormat,
}

/// What close pairs does with a typed char.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paired {
    /// The edit, and the places in the text after it of the closing chars
    /// it wrote.
    Typed(TextEdit, Vec<usize>),
    /// The cursor moves over the closing char at `.0 - 1`, to `.0`.
    Over(usize),
}

impl PairContext<'_> {
    fn char(&self, at: usize) -> Option<char> {
        (at < self.rope.len_chars()).then(|| self.rope.char(at))
    }

    /// The selection kept, its direction too, after `open` chars came
    /// before it.
    fn enclosed(&self, open: usize) -> (usize, usize) {
        (self.cursor + open, self.anchor + open)
    }

    /// Whether text that is not a space stands on the line before `at`.
    fn text_before(&self, at: usize) -> bool {
        let line = self.rope.char_to_line(at.min(self.rope.len_chars()));
        self.rope
            .slice(line_start(self.rope, line)..at)
            .chars()
            .any(|c| !c.is_whitespace())
    }

    /// UC-005 A25: what typing `text` does; `None` when it is typed as it
    /// is. Only one char at a time closes a pair.
    #[must_use]
    pub fn typed(&self, text: &str) -> Option<Paired> {
        let mut chars = text.chars();
        let c = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        let (start, end) = (self.cursor.min(self.anchor), self.cursor.max(self.anchor));
        if start != end {
            let close = closer_of(c).or_else(|| {
                (encloses(c, self.format) && !in_code(self.rope, start, self.format)).then_some(c)
            })?;
            let selected = self.rope.slice(start..end).to_string();
            let (cursor, anchor) = self.enclosed(1);
            return Some(Paired::Typed(
                TextEdit {
                    range: start..end,
                    text: format!("{c}{selected}{close}"),
                    cursor,
                    anchor,
                },
                Vec::new(),
            ));
        }
        let at = self.cursor;
        let next = self.char(at);
        if next == Some(c) && self.closers.contains(&at) {
            return Some(Paired::Over(at + 1));
        }
        let pair = |text: String, closers: Vec<usize>| {
            Paired::Typed(
                TextEdit {
                    range: at..at,
                    text,
                    cursor: at + 1,
                    anchor: at + 1,
                },
                closers,
            )
        };
        if let Some(close) = closer_of(c) {
            let fence = c == '`'
                && at >= 2
                && self.char(at - 1) == Some('`')
                && self.char(at - 2) == Some('`');
            return (closes_before(next) && !fence)
                .then(|| pair(format!("{c}{close}"), vec![at + 1]));
        }
        // `**` and `~~` within a line: the second char closes the pair.
        let doubled = matches!(c, '*' | '~')
            && at > 0
            && self.char(at - 1) == Some(c)
            && (at < 2 || self.char(at - 2) != Some(c))
            && closes_before(next)
            && self.text_before(at - 1)
            && !in_code(self.rope, at, self.format);
        doubled.then(|| pair(format!("{c}{c}{c}"), vec![at + 1, at + 2]))
    }

    /// UC-005 A25: Backspace right after an opening char whose closing char
    /// close pairs wrote right after it takes both; `None` otherwise.
    #[must_use]
    pub fn backspace(&self) -> Option<TextEdit> {
        backspace(self.rope, self.cursor, self.anchor, self.closers)
    }
}

/// UC-005 A25: Backspace at the selection from `cursor` to `anchor` of
/// `rope`, right after an opening char whose closing char close pairs
/// wrote right after it (`closers`), takes both; `None` otherwise. The
/// format plays no part: a pair is a pair in every format.
#[must_use]
pub fn backspace(rope: &Rope, cursor: usize, anchor: usize, closers: &[usize]) -> Option<TextEdit> {
    let at = cursor;
    if at != anchor || at == 0 || at >= rope.len_chars() || !closers.contains(&at) {
        return None;
    }
    let (open, close) = (rope.char(at - 1), rope.char(at));
    let pair = closer_of(open) == Some(close) || (open == close && matches!(open, '*' | '~'));
    pair.then(|| TextEdit {
        range: at - 1..at + 1,
        text: String::new(),
        cursor: at - 1,
        anchor: at - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: NoteFormat = NoteFormat::Markdown;

    fn context<'a>(
        rope: &'a Rope,
        cursor: usize,
        anchor: usize,
        closers: &'a [usize],
    ) -> PairContext<'a> {
        PairContext {
            rope,
            cursor,
            anchor,
            closers,
            format: MD,
        }
    }

    fn edit(paired: Option<Paired>) -> TextEdit {
        match paired {
            Some(Paired::Typed(edit, _)) => edit,
            other => panic!("no edit: {other:?}"),
        }
    }

    /// TC-005-35 step 1, step 7: a bracket closes at the end of a line or
    /// before a space or a closing bracket, not before a letter.
    #[test]
    fn tc_005_35_a_bracket_closes_only_where_nothing_follows() {
        let rope = Rope::from_str("ab\nx y]");
        let at_end = context(&rope, 2, 2, &[]).typed("(");
        assert_eq!(
            at_end,
            Some(Paired::Typed(
                TextEdit {
                    range: 2..2,
                    text: "()".into(),
                    cursor: 3,
                    anchor: 3
                },
                vec![3]
            ))
        );
        assert_eq!(context(&rope, 3, 3, &[]).typed("("), None, "before x");
        assert!(
            context(&rope, 4, 4, &[]).typed("[").is_some(),
            "before a space"
        );
        assert!(context(&rope, 6, 6, &[]).typed("{").is_some(), "before ]");
        assert_eq!(
            context(&rope, 2, 2, &[]).typed("(("),
            None,
            "more than one char"
        );
    }

    /// TC-005-35 step 2: a closing char right before the one close pairs
    /// wrote moves over it; before one the user typed it is typed.
    #[test]
    fn tc_005_35_typing_the_closing_char_moves_over_the_written_one() {
        let rope = Rope::from_str("(a)");
        assert_eq!(context(&rope, 2, 2, &[2]).typed(")"), Some(Paired::Over(3)));
        assert_eq!(context(&rope, 2, 2, &[]).typed(")"), None);
        let quotes = Rope::from_str("\"\"");
        assert_eq!(
            context(&quotes, 1, 1, &[1]).typed("\""),
            Some(Paired::Over(2))
        );
    }

    /// TC-005-35 step 3: Backspace after the opening char takes the
    /// closing one written with it.
    #[test]
    fn tc_005_35_backspace_takes_the_written_closing_char_too() {
        let rope = Rope::from_str("x()");
        let gone = context(&rope, 2, 2, &[2]).backspace().expect("the pair");
        assert_eq!((gone.range, gone.text.as_str(), gone.cursor), (1..3, "", 1));
        assert_eq!(
            context(&rope, 2, 2, &[]).backspace(),
            None,
            "typed by the user"
        );
        let stars = Rope::from_str("a ****");
        assert!(context(&stars, 4, 4, &[4, 5]).backspace().is_some());
        let other = Rope::from_str("(]");
        assert_eq!(context(&other, 1, 1, &[1]).backspace(), None);
    }

    /// TC-005-35 step 4, step 5: `**` within a line writes its pair; a
    /// single `*` and `**` at the start of a line do not.
    #[test]
    fn tc_005_35_double_markup_within_a_line_closes() {
        let rope = Rope::from_str("text *");
        let both = context(&rope, 6, 6, &[]).typed("*");
        assert_eq!(
            both,
            Some(Paired::Typed(
                TextEdit {
                    range: 6..6,
                    text: "***".into(),
                    cursor: 7,
                    anchor: 7
                },
                vec![7, 8]
            ))
        );
        let start = Rope::from_str("*");
        assert_eq!(
            context(&start, 1, 1, &[]).typed("*"),
            None,
            "a list or a break"
        );
        assert_eq!(
            context(&Rope::from_str("text "), 5, 5, &[]).typed("*"),
            None
        );
        let strike = Rope::from_str("so ~");
        assert!(context(&strike, 4, 4, &[]).typed("~").is_some());
        let three = Rope::from_str("so **");
        assert_eq!(context(&three, 5, 5, &[]).typed("*"), None, "not a third");
        let code = Rope::from_str("`so *");
        assert_eq!(context(&code, 5, 5, &[]).typed("*"), None, "in code");
    }

    /// TC-005-35 step 6: markup of a pair encloses the selection, which
    /// stays selected in its direction; multibyte text counts as chars.
    #[test]
    fn tc_005_35_a_selection_is_enclosed_and_stays_selected() {
        let rope = Rope::from_str("a wörd 😀");
        let forward = edit(context(&rope, 6, 2, &[]).typed("_"));
        assert_eq!(forward.text, "_wörd_");
        assert_eq!(
            (forward.range, forward.cursor, forward.anchor),
            (2..6, 7, 3)
        );
        let backward = edit(context(&rope, 7, 8, &[]).typed("("));
        assert_eq!(backward.text, "(😀)");
        assert_eq!((backward.cursor, backward.anchor), (8, 9));
        assert_eq!(context(&rope, 6, 2, &[]).typed("x"), None);
        let adoc = PairContext {
            format: NoteFormat::AsciiDoc,
            ..context(&rope, 6, 2, &[])
        };
        assert_eq!(edit(adoc.typed("#")).text, "#wörd#");
        assert_eq!(
            context(&rope, 6, 2, &[]).typed("#"),
            None,
            "no pair in Markdown"
        );
    }

    /// OQ-252: a third backtick is typed as it is, for a code fence.
    #[test]
    fn oq252_a_third_backtick_opens_a_fence() {
        let rope = Rope::from_str("``");
        assert_eq!(context(&rope, 2, 2, &[]).typed("`"), None);
        assert!(context(&Rope::from_str(""), 0, 0, &[]).typed("`").is_some());
    }
}
