//! FR-086, UC-004 A16, A17, BR-017: going to a line of the note: what the
//! user typed (`120`, or `120:8` with a column), and the place it names in
//! the markup.
//!
//! Lines count the lines of the markup from 1, as the line numbers of the
//! source view do; columns count chars from 1, as the status line does. A
//! line beyond the last goes to the last, a column beyond the end of the
//! line to its end.

use ropey::Rope;

use super::{content_end, line_start};

/// A line to go to, and a column in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineTarget {
    /// The line, from 1.
    pub line: usize,
    /// The column, from 1; `None`: the start of the line.
    pub column: Option<usize>,
}

/// UC-004 A17: what the user typed is no line number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LineTargetError {
    /// Not a line number, or a column that is none.
    #[error("A line number is expected, such as 120 or 120:8.")]
    NotALine,
    /// Line or column 0.
    #[error("Lines and columns start at 1.")]
    StartsAtOne,
}

/// UC-004 BR-017: how a place beyond the note was taken in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clamped {
    /// The line is beyond the last: the last line.
    LastLine,
    /// The column is beyond the end of the line: its end.
    LineEnd,
}

impl Clamped {
    /// What the status bar says of it.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Clamped::LastLine => "Went to the last line.",
            Clamped::LineEnd => "Went to the end of the line.",
        }
    }
}

/// A whole number of at least 1; one too large for a `usize` is the
/// largest, which goes to the last line or the end of it.
fn number(text: &str) -> Result<usize, LineTargetError> {
    let text = text.trim();
    if text.is_empty() || !text.chars().all(|c| c.is_ascii_digit()) {
        return Err(LineTargetError::NotALine);
    }
    let value = text.parse::<usize>().unwrap_or(usize::MAX);
    if value == 0 {
        return Err(LineTargetError::StartsAtOne);
    }
    Ok(value)
}

impl LineTarget {
    /// UC-004 A16 step 1, A17: the line, and the column after a colon, that
    /// `text` names; spaces around them do not count.
    ///
    /// # Errors
    ///
    /// [`LineTargetError::NotALine`] when `text` is empty, holds anything
    /// but digits and one colon, or names line or column 0.
    pub fn parse(text: &str) -> Result<Self, LineTargetError> {
        match text.split_once(':') {
            Some((line, column)) => Ok(Self {
                line: number(line)?,
                column: Some(number(column)?),
            }),
            None => Ok(Self {
                line: number(text)?,
                column: None,
            }),
        }
    }

    /// The line of `rope` it names, from 0: the last one for a line beyond
    /// it.
    #[must_use]
    pub fn line_in(&self, rope: &Rope) -> usize {
        (self.line - 1).min(rope.len_lines().saturating_sub(1))
    }

    /// UC-004 BR-017: whether the line or the column lies beyond `rope`,
    /// and was taken to the last line or the end of the line.
    #[must_use]
    pub fn clamped(&self, rope: &Rope) -> Option<Clamped> {
        if self.line > rope.len_lines() {
            return Some(Clamped::LastLine);
        }
        let line = self.line_in(rope);
        let length = content_end(rope, line) - line_start(rope, line);
        self.column
            .filter(|column| column - 1 > length)
            .map(|_| Clamped::LineEnd)
    }

    /// BR-017: the char it names in `rope`: the start of the line, or the
    /// column, at most the end of the line.
    #[must_use]
    pub fn char_in(&self, rope: &Rope) -> usize {
        let line = self.line_in(rope);
        let start = line_start(rope, line);
        let length = content_end(rope, line) - start;
        start + self.column.map_or(0, |column| (column - 1).min(length))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-004-33 step 2, step 4: a line, a line with a column, and both
    /// beyond the note go to the last line or the end of the line.
    #[test]
    fn tc_004_33_a_line_and_a_column_name_a_place() {
        let rope = Rope::from_str("one\ntwö 😀 x\nthree");
        let target = LineTarget::parse(" 2:5 ").expect("a line");
        assert_eq!(
            target,
            LineTarget {
                line: 2,
                column: Some(5)
            }
        );
        assert_eq!(target.char_in(&rope), 4 + 4, "the column counts chars");
        assert_eq!(LineTarget::parse("999").expect("a line").char_in(&rope), 12);
        assert_eq!(
            LineTarget::parse("2:400").expect("a line").char_in(&rope),
            11
        );
        let huge = LineTarget::parse("99999999999999999999999").expect("a line");
        assert_eq!(huge.line_in(&rope), 2);
        assert_eq!(huge.clamped(&rope), Some(Clamped::LastLine));
        let wide = LineTarget::parse("2:400").expect("a line");
        assert_eq!(wide.clamped(&rope), Some(Clamped::LineEnd));
        assert_eq!(
            LineTarget::parse("2:8").expect("a line").clamped(&rope),
            None
        );
        assert_eq!(Clamped::LastLine.message(), "Went to the last line.");
    }

    /// TC-004-33 step 5, UC-004 A17: what is no line number is said so.
    #[test]
    fn tc_004_33_what_is_no_line_number_is_refused() {
        for text in ["0", "0:5", "5:0"] {
            assert_eq!(
                LineTarget::parse(text),
                Err(LineTargetError::StartsAtOne),
                "{text}"
            );
        }
        for text in ["abc", "", " ", "12:", ":3", "-3", "1.5", "1:2:3", "٣"] {
            assert_eq!(
                LineTarget::parse(text),
                Err(LineTargetError::NotALine),
                "{text:?}"
            );
        }
        assert_eq!(
            LineTargetError::NotALine.to_string(),
            "A line number is expected, such as 120 or 120:8."
        );
    }
}
