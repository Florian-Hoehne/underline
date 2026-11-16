//! FR-082, UC-005 A26, BR-020, OQ-203: moving the lines at the cursor up or
//! down past the line next to them, and duplicating them below.
//!
//! The source view moves lines of the markup; the facade moves blocks, which
//! it gives here as the ranges of lines they stand on, so both views share
//! [`swap`] and [`duplicate`]. Each is one replacement of the text, so one
//! step for undo; the cursor and the selection move with the lines.

use ropey::Rope;

use super::{content_end, line_start, touched_lines, TextEdit};

/// Lines of the text, the first and the last, both included.
pub type Lines = (usize, usize);

/// The text of `lines`, without the line break after the last.
fn text_of(rope: &Rope, lines: Lines) -> String {
    rope.slice(line_start(rope, lines.0)..content_end(rope, lines.1))
        .to_string()
}

/// Where the region of `lines` ends for a selection: the start of the line
/// after them, or the end of the text.
fn region_end(rope: &Rope, lines: Lines) -> usize {
    if lines.1 + 1 < rope.len_lines() {
        line_start(rope, lines.1 + 1)
    } else {
        rope.len_chars()
    }
}

/// `at` moved by `by` when it lies in `from..=to`: both ends are included,
/// so a selection that ends at the start of the line after a region (a
/// selection of whole lines) moves with the region.
fn shifted(at: usize, (from, to): (usize, usize), by: isize) -> usize {
    if (from..=to).contains(&at) {
        at.saturating_add_signed(by)
    } else {
        at
    }
}

fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

/// NFR-007: the line break the note uses: the one after `line`, else the
/// first one of the text, else `\n`.
#[must_use]
pub fn line_break(rope: &Rope, line: usize) -> String {
    let after = |line: usize| -> Option<String> {
        (line + 1 < rope.len_lines())
            .then(|| {
                rope.slice(content_end(rope, line)..line_start(rope, line + 1))
                    .to_string()
            })
            .filter(|text| !text.is_empty())
    };
    after(line)
        .or_else(|| (0..rope.len_lines().min(1000)).find_map(after))
        .unwrap_or_else(|| "\n".to_owned())
}

/// What is added around two swapped ranges of lines so they keep their
/// structure (the facade's blocks, OQ-255): a line break before them, one
/// after the separator between them, and one after them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Joints {
    /// Written before the range, after the block before it.
    pub before: String,
    /// Written after what stood between the two.
    pub between: String,
    /// Written after the range, before the block after it.
    pub after: String,
}

/// The lines `upper` and the lines `lower` below them change places; what
/// stands between them (line breaks, blank lines) stays between them. The
/// selection moves with the lines it is in: with `lower` when `lower_moves`
/// (moving up), else with `upper`.
///
/// `lower` must start below the last line of `upper`; `None` when it does
/// not (nothing can be swapped then).
#[must_use]
pub fn swap(
    rope: &Rope,
    upper: Lines,
    lower: Lines,
    selection: (usize, usize),
    lower_moves: bool,
) -> Option<TextEdit> {
    swap_joined(
        rope,
        upper,
        lower,
        selection,
        lower_moves,
        &Joints::default(),
    )
}

/// [`swap`] with `joints` written around and between the two ranges.
#[must_use]
pub fn swap_joined(
    rope: &Rope,
    upper: Lines,
    lower: Lines,
    (cursor, anchor): (usize, usize),
    lower_moves: bool,
    joints: &Joints,
) -> Option<TextEdit> {
    if lower.0 <= upper.1 || upper.0 > upper.1 || lower.0 > lower.1 {
        return None;
    }
    let upper_start = line_start(rope, upper.0);
    let between = content_end(rope, upper.1)..line_start(rope, lower.0);
    let upper_text = text_of(rope, upper);
    let lower_text = text_of(rope, lower);
    let separator = format!("{}{}", rope.slice(between.clone()), joints.between);
    let end = content_end(rope, lower.1);
    let before = signed(joints.before.chars().count());
    let (region, by) = if lower_moves {
        let lower_start = between.end;
        (
            (lower_start, region_end(rope, lower)),
            before - signed(lower_start - upper_start),
        )
    } else {
        (
            (upper_start, region_end(rope, upper)),
            before + signed(lower_text.chars().count() + separator.chars().count()),
        )
    };
    let text = format!(
        "{}{lower_text}{separator}{upper_text}{}",
        joints.before, joints.after
    );
    let length = rope.len_chars() - (end - upper_start) + text.chars().count();
    Some(TextEdit {
        range: upper_start..end,
        text,
        cursor: shifted(cursor, region, by).min(length),
        anchor: shifted(anchor, region, by).min(length),
    })
}

/// A copy of `lines` goes below them after `separator` (by default the line
/// break after the last of them, else the note's line break); the selection
/// moves into the copy when it lies within `lines` (up to the start of the
/// line after them).
#[must_use]
pub fn duplicate(
    rope: &Rope,
    lines: Lines,
    separator: Option<&str>,
    (cursor, anchor): (usize, usize),
) -> TextEdit {
    let end = content_end(rope, lines.1);
    let separator = separator.map_or_else(|| line_break(rope, lines.1), str::to_owned);
    let copied = text_of(rope, lines);
    let by = signed(separator.chars().count() + copied.chars().count());
    let region = (line_start(rope, lines.0), region_end(rope, lines));
    TextEdit {
        range: end..end,
        text: format!("{separator}{copied}"),
        cursor: shifted(cursor, region, by),
        anchor: shifted(anchor, region, by),
    }
}

/// UC-005 A26 in the source view: the lines the selection touches go up
/// (`up`) or down past the line next to them; `None` at the first or the
/// last line, where nothing moves.
#[must_use]
pub fn move_lines(rope: &Rope, cursor: usize, anchor: usize, up: bool) -> Option<TextEdit> {
    let lines = touched_lines(rope, cursor, anchor);
    if up {
        let above = lines.0.checked_sub(1)?;
        swap(rope, (above, above), lines, (cursor, anchor), true)
    } else {
        let below = lines.1 + 1;
        if below >= rope.len_lines() {
            return None;
        }
        swap(rope, lines, (below, below), (cursor, anchor), false)
    }
}

/// UC-005 A26 in the source view: a copy of the lines the selection
/// touches goes below them, the cursor in the copy.
#[must_use]
pub fn duplicate_lines(rope: &Rope, cursor: usize, anchor: usize) -> TextEdit {
    duplicate(
        rope,
        touched_lines(rope, cursor, anchor),
        None,
        (cursor, anchor),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(rope: &Rope, edit: &TextEdit) -> String {
        let mut text = rope.clone();
        text.remove(edit.range.clone());
        text.insert(edit.range.start, &edit.text);
        text.to_string()
    }

    /// TC-005-36 step 1: Alt+Down in "two" moves it below "three", the
    /// cursor with it.
    #[test]
    fn tc_005_36_a_line_moves_down_with_the_cursor() {
        let rope = Rope::from_str("one\ntwo\nthree\n");
        let edit = move_lines(&rope, 5, 5, false).expect("a line below");
        assert_eq!(applied(&rope, &edit), "one\nthree\ntwo\n");
        assert_eq!((edit.cursor, edit.anchor), (11, 11), "still after the t");
        let back = move_lines(&Rope::from_str(&applied(&rope, &edit)), 11, 11, true)
            .expect("a line above");
        assert_eq!(back.cursor, 5);
    }

    /// TC-005-36 step 6, BR-020: at the first line moving up does nothing,
    /// at the last moving down does nothing.
    #[test]
    fn tc_005_36_nothing_moves_past_the_ends() {
        let rope = Rope::from_str("one\ntwo");
        assert_eq!(move_lines(&rope, 1, 1, true), None);
        assert_eq!(move_lines(&rope, 5, 5, false), None);
    }

    /// BR-020: a selection moves every line it touches; a line it reaches
    /// only at its start stays; the selection keeps its direction.
    #[test]
    fn br020_a_selection_moves_all_its_lines() {
        let rope = Rope::from_str("a\nb\nc\nd");
        // "b" and "c" selected up to the start of "d".
        let edit = move_lines(&rope, 2, 6, true).expect("moves");
        assert_eq!(applied(&rope, &edit), "b\nc\na\nd");
        assert_eq!((edit.cursor, edit.anchor), (0, 4));
        let down = move_lines(&rope, 6, 2, false).expect("moves");
        assert_eq!(applied(&rope, &down), "a\nd\nb\nc");
        assert_eq!((down.cursor, down.anchor), (7, 4), "to the end of the text");
    }

    /// TC-005-36 step 2, NFR-007: a duplicate goes below with the cursor in
    /// it; a CRLF note keeps its breaks, the last line gets one.
    #[test]
    fn tc_005_36_duplicate_puts_the_cursor_in_the_copy() {
        let rope = Rope::from_str("one\r\ntwö\r\nthree");
        let edit = duplicate_lines(&rope, 6, 6);
        assert_eq!(applied(&rope, &edit), "one\r\ntwö\r\ntwö\r\nthree");
        assert_eq!(edit.cursor, 11);
        let last = duplicate_lines(&Rope::from_str("x 😀"), 1, 1);
        assert_eq!(last.text, "\nx 😀");
        assert_eq!(last.cursor, 5);
        let crlf = duplicate_lines(&Rope::from_str("a\r\nb"), 3, 3);
        assert_eq!(crlf.text, "\r\nb", "the note's line break");
    }

    /// BR-020: blocks with blank lines between them change places, the
    /// blank lines stay between (the facade's paragraphs).
    #[test]
    fn br020_blank_lines_stay_between_swapped_blocks() {
        let rope = Rope::from_str("four\nfive\n\nsix\n");
        let edit = swap(&rope, (0, 1), (3, 3), (12, 12), true).expect("apart");
        assert_eq!(applied(&rope, &edit), "six\n\nfour\nfive\n");
        assert_eq!(edit.cursor, 1);
        assert_eq!(
            swap(&rope, (0, 1), (1, 3), (0, 0), true),
            None,
            "overlapping"
        );
        let copy = duplicate(&rope, (0, 1), Some("\n\n"), (2, 2));
        assert_eq!(applied(&rope, &copy), "four\nfive\n\nfour\nfive\n\nsix\n");
        assert_eq!(copy.cursor, 13);
    }
}
