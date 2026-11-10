//! Offsets at the boundary to a webview (ADR-021, plan section 11).
//!
//! The rope counts chars and bytes; JavaScript strings, and with them
//! CodeMirror and the DOM, count UTF-16 code units. A char outside the
//! Basic Multilingual Plane, such as most emoji, is one char, four bytes and
//! two UTF-16 units.
//!
//! CodeMirror also turns every line break into one `\n`: a `\r\n` pair is
//! one unit there, and so is a lone `\r`. The rope keeps the bytes of the
//! file as they are (NFR-007), so the window counts in *web units*: UTF-16
//! units of the text with its line breaks normalised ([`normalize_breaks`],
//! [`WebOffsets`]). Every offset and every text that crosses to the window
//! is converted here and nowhere else.

use std::borrow::Cow;

use ropey::Rope;

/// The UTF-16 offset of char index `at` in `rope`; past the end, the end.
#[must_use]
pub fn char_to_utf16(rope: &Rope, at: usize) -> usize {
    rope.char_to_utf16_cu(at.min(rope.len_chars()))
}

/// The char index at UTF-16 offset `at` in `rope`; past the end, the end.
/// An offset inside a surrogate pair gives the char the pair belongs to, so
/// no char is ever split.
#[must_use]
pub fn utf16_to_char(rope: &Rope, at: usize) -> usize {
    rope.utf16_cu_to_char(at.min(rope.len_utf16_cu()))
}

/// The length of `text` in UTF-16 units.
#[must_use]
pub fn len_utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The UTF-16 offset of byte offset `byte` in `text`; a byte inside a char
/// counts from the start of that char, past the end gives the end.
#[must_use]
pub fn byte_to_utf16(text: &str, byte: usize) -> usize {
    if text.is_ascii() {
        return byte.min(text.len());
    }
    let mut units = 0;
    for (at, c) in text.char_indices() {
        if at + c.len_utf8() > byte {
            return units;
        }
        units += c.len_utf16();
    }
    units
}

/// `text` with its line breaks as the window has them: `\r\n` and a lone
/// `\r` become `\n`.
#[must_use]
pub fn normalize_breaks(text: &str) -> Cow<'_, str> {
    if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(text)
    }
}

/// The length of `text` in web units: UTF-16 units, a `\r\n` pair counted
/// once.
#[must_use]
pub fn len_web(text: &str) -> usize {
    len_utf16(&normalize_breaks(text))
}

/// Where the `\r\n` pairs of a text are, to convert between char indices of
/// the rope and web units. Made for one state of the text; an empty one fits
/// every text without `\r`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebOffsets {
    /// The UTF-16 offset of the `\r` of each pair, in order.
    pairs: Vec<usize>,
}

impl WebOffsets {
    /// The pairs of `rope`; a text without `\r` is not walked through.
    #[must_use]
    pub fn of(rope: &Rope) -> Self {
        if !rope.chunks().any(|chunk| chunk.contains('\r')) {
            return Self::default();
        }
        let mut pairs = Vec::new();
        let mut units = 0;
        let mut previous_cr = false;
        for c in rope.chars() {
            if previous_cr && c == '\n' {
                pairs.push(units - 1);
            }
            previous_cr = c == '\r';
            units += c.len_utf16();
        }
        Self { pairs }
    }

    /// The web offset of char index `at` in `rope`; past the end, the end.
    #[must_use]
    pub fn to_web(&self, rope: &Rope, at: usize) -> usize {
        let units = char_to_utf16(rope, at);
        // A pair whose `\r` stands before the offset is counted once.
        units - self.pairs.partition_point(|&pair| pair + 1 < units)
    }

    /// The char index at web offset `at` in `rope`; past the end, the end.
    /// No offset falls between the `\r` and the `\n` of a pair.
    #[must_use]
    pub fn from_web(&self, rope: &Rope, at: usize) -> usize {
        // The web offset of the pair `i` is its UTF-16 offset less `i`.
        let before = partition(&self.pairs, at);
        utf16_to_char(rope, at + before)
    }
}

/// How many pairs have a web offset below `at`. The web offsets of the
/// pairs (`pairs[i] - i`) rise strictly, so they are searched in halves.
fn partition(pairs: &[usize], at: usize) -> usize {
    let (mut low, mut high) = (0, pairs.len());
    while low < high {
        let mid = low + (high - low) / 2;
        if pairs[mid] - mid < at {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "a🙂日b": a=1 unit, 🙂=2 units, 日=1 unit, b=1 unit.
    const MIXED: &str = "a🙂日b\n🙂";

    #[test]
    fn adr021_chars_and_utf16_units_of_emoji_and_cjk() {
        let rope = Rope::from_str(MIXED);

        let units: Vec<usize> = (0..=rope.len_chars())
            .map(|at| char_to_utf16(&rope, at))
            .collect();

        assert_eq!(units, [0, 1, 3, 4, 5, 6, 8]);
        assert_eq!(char_to_utf16(&rope, 99), 8, "past the end is the end");
    }

    #[test]
    fn adr021_utf16_units_back_to_chars_never_split_a_pair() {
        let rope = Rope::from_str(MIXED);

        assert_eq!(utf16_to_char(&rope, 0), 0);
        assert_eq!(utf16_to_char(&rope, 1), 1);
        assert_eq!(utf16_to_char(&rope, 2), 1, "inside 🙂 is 🙂");
        assert_eq!(utf16_to_char(&rope, 3), 2);
        assert_eq!(utf16_to_char(&rope, 4), 3);
        assert_eq!(utf16_to_char(&rope, 8), 6);
        assert_eq!(utf16_to_char(&rope, 80), 6, "past the end is the end");
    }

    #[test]
    fn adr021_lengths_and_byte_offsets_in_utf16_units() {
        assert_eq!(len_utf16("🙂日本x"), 5);
        assert_eq!(len_utf16(""), 0);
        // a (1 byte), 🙂 (4 bytes), 日 (3 bytes), b (1 byte).
        let text = "a🙂日b";
        assert_eq!(byte_to_utf16(text, 0), 0);
        assert_eq!(byte_to_utf16(text, 1), 1);
        assert_eq!(byte_to_utf16(text, 3), 1, "inside 🙂 counts from its start");
        assert_eq!(byte_to_utf16(text, 5), 3);
        assert_eq!(byte_to_utf16(text, 8), 4);
        assert_eq!(byte_to_utf16(text, 9), 5);
        assert_eq!(byte_to_utf16(text, 50), 5);
        assert_eq!(byte_to_utf16("plain", 3), 3);
    }

    /// MUSS 1 (review F3): a `\r\n` pair is one unit, a lone `\r` one
    /// unit, as in CodeMirror; emoji keep their two units.
    #[test]
    fn nfr007_crlf_and_lone_cr_count_once_with_emoji() {
        // a \r\n 🙂 \r b \n c
        let rope = Rope::from_str("a\r\n🙂\rb\nc");
        let web = WebOffsets::of(&rope);
        let normal = normalize_breaks(&rope.to_string()).into_owned();
        assert_eq!(normal, "a\n🙂\nb\nc");

        let units: Vec<usize> = (0..=rope.len_chars())
            .map(|at| web.to_web(&rope, at))
            .collect();
        // chars: a, \r, \n, 🙂, \r, b, \n, c; the place between the \r and
        // the \n of the pair (char 2) is never asked for.
        assert_eq!(units, [0, 1, 2, 2, 4, 5, 6, 7, 8]);
        assert_eq!(web.from_web(&rope, 0), 0);
        assert_eq!(web.from_web(&rope, 1), 1, "before the pair");
        assert_eq!(web.from_web(&rope, 2), 3, "after the pair");
        assert_eq!(web.from_web(&rope, 3), 3, "inside 🙂 is 🙂");
        assert_eq!(web.from_web(&rope, 4), 4, "before the lone CR");
        assert_eq!(web.from_web(&rope, 5), 5);
        assert_eq!(web.from_web(&rope, 99), 8);
        assert_eq!(len_web("a\r\n🙂\rb"), 6);
    }

    #[test]
    fn nfr007_a_text_without_cr_needs_no_walk() {
        let rope = Rope::from_str("plain\ntext");
        assert_eq!(WebOffsets::of(&rope), WebOffsets::default());
        assert!(matches!(normalize_breaks("plain"), Cow::Borrowed(_)));
    }
}
