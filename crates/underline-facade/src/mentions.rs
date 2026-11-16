//! FR-103, UC-004 BR-021 (OQ-199): the unlinked mentions of a note's title
//! in the text of another note: the places where the title stands as plain
//! text, without a link around it.
//!
//! A mention is the title as whole words, upper and lower case alike, its
//! words apart by spaces, tabs or other spaces such as a no-break space,
//! on one line; a combining mark belongs to the letter before it. Nothing
//! in code, in a link (its text and its place), in a picture, in an
//! included file, in a web address or e-mail address written out, in a
//! formula, in a comment, in the front matter or the attribute lines of
//! the note, in an AsciiDoc macro, reference, attribute reference,
//! conditional or literal paragraph, in a Markdown footnote reference,
//! `[[…]]` or link by reference, nor a tag (`#plan`) is a mention, nor one
//! right after a backslash in AsciiDoc (which would escape the link). A
//! title of fewer than [`LEAST_CHARS`] characters has none: it would match
//! too much. At most [`MOST_PER_NOTE`] mentions of one note are found.
//! Lines end as comrak and Asciidoctor end them: `\n`, `\r\n` or a lone `\r`.
//!
//! The text is searched for the title first, cheaply; only a note that
//! holds it is parsed for what is left out, so a workspace of 10,000 notes
//! is searched within NFR-019's 2 seconds. The words of a note can be kept
//! as [`WordKeys`], so a note that cannot mention a title is not read again
//! when another note becomes the active one.

use std::ops::Range;

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena, Options};
use underline_core::NoteFormat;

use crate::rename::{block_delimiter, found_links, line_starts, lines, math_bytes};

/// BR-021: a title with fewer characters than this, spaces not counted,
/// has no unlinked mentions.
pub const LEAST_CHARS: usize = 3;

/// BR-021: at most this many mentions of one note are found, so a note
/// that repeats the title endlessly costs no more than this (NFR-019).
pub const MOST_PER_NOTE: usize = 1000;

/// A title as mentions of it are looked for: its words, each character in
/// lower case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionTitle {
    words: Vec<Vec<char>>,
    /// The keys of its words, as [`WordKeys`] keeps them.
    keys: Vec<u32>,
}

impl MentionTitle {
    /// BR-021: the title `title` to look for; `None` when it has fewer than
    /// [`LEAST_CHARS`] characters (spaces not counted).
    ///
    /// # Examples
    ///
    /// ```
    /// use underline_facade::mentions::MentionTitle;
    ///
    /// assert!(MentionTitle::new("Road map").is_some());
    /// assert!(MentionTitle::new("Go").is_none());
    /// ```
    #[must_use]
    pub fn new(title: &str) -> Option<Self> {
        let words: Vec<Vec<char>> = title
            .split_whitespace()
            .map(|word| word.chars().map(fold).collect())
            .collect();
        let chars: usize = words.iter().map(Vec::len).sum();
        let keys = WordKeys::of(title).0;
        (chars >= LEAST_CHARS).then_some(Self { words, keys })
    }

    /// The places in `text` where the title stands as whole words, before
    /// anything is left out: the bytes of each, in order, apart.
    fn find_in(&self, text: &str) -> Vec<Range<usize>> {
        let first = self.words[0][0];
        let mut found = Vec::new();
        let mut before: Option<char> = None;
        let mut skip_to = 0;
        for (at, c) in text.char_indices() {
            let starts = at >= skip_to
                && fold(c) == first
                && before.is_none_or(|b| !is_word_char(b) && b != '#');
            before = Some(c);
            if !starts {
                continue;
            }
            if let Some(end) = self.match_at(text, at) {
                found.push(at..end);
                skip_to = end;
                if found.len() == MOST_PER_NOTE {
                    break;
                }
            }
        }
        found
    }

    /// The end of the title when it stands at byte `at` of `text` as whole
    /// words.
    fn match_at(&self, text: &str, at: usize) -> Option<usize> {
        let mut chars = text[at..].char_indices().peekable();
        for (index, word) in self.words.iter().enumerate() {
            if index > 0 {
                let mut gap = 0;
                while chars.peek().is_some_and(|(_, c)| is_gap(*c)) {
                    chars.next();
                    gap += 1;
                }
                if gap == 0 {
                    return None;
                }
            }
            for wanted in word {
                let (_, c) = chars.next()?;
                if fold(c) != *wanted {
                    return None;
                }
            }
        }
        let end = chars.peek().map_or(text.len(), |(offset, _)| at + offset);
        let after = text[end..].chars().next();
        after.is_none_or(|c| !is_word_char(c)).then_some(end)
    }
}

/// The words of a text, each kept as a small key of its characters in lower
/// case, so whether a note may mention a title is known without its text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WordKeys(Vec<u32>);

impl WordKeys {
    /// The keys of the words of `text`, each once.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let mut keys = Vec::new();
        let mut key: Option<u32> = None;
        for c in text.chars() {
            if is_word_char(c) {
                let mut buffer = [0; 4];
                let folded = fold(c).encode_utf8(&mut buffer);
                let start = key.unwrap_or(0x811c_9dc5);
                key = Some(folded.bytes().fold(start, |hash, byte| {
                    (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
                }));
            } else if let Some(done) = key.take() {
                keys.push(done);
            }
        }
        keys.extend(key);
        keys.sort_unstable();
        keys.dedup();
        keys.shrink_to_fit();
        Self(keys)
    }

    /// How many distinct words are kept: four bytes each, so a note of
    /// 2,000 distinct words keeps 8 KB, whatever its length (NFR-019).
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no word is kept.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether the text may mention `title`: it holds every word of it.
    #[must_use]
    pub fn may_mention(&self, title: &MentionTitle) -> bool {
        title
            .keys
            .iter()
            .all(|key| self.0.binary_search(key).is_ok())
    }
}

/// A character as mentions compare it: in lower case (its first character
/// when that is more than one), a final sigma as any sigma.
fn fold(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    if lower == 'ς' {
        'σ'
    } else {
        lower
    }
}

/// Whether `c` belongs to a word: a letter, a digit, or a combining mark
/// (which belongs to the letter before it, `cafe` + U+0301 is `café`). A
/// `_` does not, since Markdown marks emphasis with it (`_plan_`).
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_combining_mark(c)
}

/// The blocks of combining marks of Unicode.
fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05BD}'
            | '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0900}'..='\u{0903}'
            | '\u{093A}'..='\u{094F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{200C}'..='\u{200D}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{302A}'..='\u{302F}'
            | '\u{3099}'..='\u{309A}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Whether `c` parts the words of a title in a line: a space of any kind,
/// a no-break space too, but no line end.
fn is_gap(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// FR-103, BR-021: the unlinked mentions of `title` in `source`, a note in
/// `format`: the bytes of each, in the order of the note.
///
/// # Examples
///
/// ```
/// use underline_core::NoteFormat;
/// use underline_facade::mentions::{unlinked_mentions, MentionTitle};
///
/// let title = MentionTitle::new("Road map").unwrap();
/// let text = "The road map, [Road map](Road-map.md), `road map`.\n";
/// assert_eq!(unlinked_mentions(text, NoteFormat::Markdown, &title), [4..12]);
/// ```
#[must_use]
pub fn unlinked_mentions(
    source: &str,
    format: NoteFormat,
    title: &MentionTitle,
) -> Vec<Range<usize>> {
    let mut found = title.find_in(source);
    if found.is_empty() {
        return found;
    }
    let left_out = left_out(source, format);
    found.retain(|range| {
        let at = left_out.partition_point(|out| out.end <= range.start);
        left_out.get(at).is_none_or(|out| out.start >= range.end)
    });
    if format == NoteFormat::AsciiDoc {
        // A link right after an odd run of `\` would be escaped.
        found.retain(|range| odd_backslashes_before(source, range.start).is_none());
    }
    found
}

/// Whether an odd number of `\` stands right before byte `at` of `text`;
/// then the byte where that run starts.
#[must_use]
pub fn odd_backslashes_before(text: &str, at: usize) -> Option<usize> {
    let run = text.as_bytes()[..at]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count();
    (run % 2 == 1).then_some(at - run)
}

/// The bytes of `source` that hold no mention, sorted and merged.
fn left_out(source: &str, format: NoteFormat) -> Vec<Range<usize>> {
    let mut out = match format {
        NoteFormat::Markdown => markdown_left_out(source),
        NoteFormat::AsciiDoc => asciidoc_left_out(source),
    };
    // Links, pictures, included files and web addresses written out, with
    // their text.
    // A reference definition (`[r]: place "title"`) to the end of its line.
    out.extend(found_links(source, format).into_iter().map(|link| {
        let definition = format == NoteFormat::Markdown
            && link.text.is_none()
            && source[link.span.clone()].starts_with('[')
            && !source[link.span.clone()].contains("](");
        if definition {
            let end = source[link.span.end..]
                .find('\n')
                .map_or(source.len(), |at| link.span.end + at);
            link.span.start..end
        } else {
            link.span
        }
    }));
    out.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(out.len());
    for range in out {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// The front matter at the start of a Markdown note: from a line `---` to
/// the next line `---` or `...`.
fn front_matter(source: &str) -> Option<Range<usize>> {
    let start = usize::from(source.starts_with('\u{feff}')) * '\u{feff}'.len_utf8();
    let mut lines = lines(&source[start..]);
    let first = lines.next()?;
    if first.trim_end() != "---" || !first.ends_with(['\n', '\r']) {
        return None;
    }
    let mut at = start + first.len();
    for line in lines {
        at += line.len();
        if matches!(line.trim_end(), "---" | "...") {
            return Some(0..at);
        }
    }
    Some(0..source.len())
}

/// What holds no mention in a Markdown note: the front matter, code, HTML
/// and comments, formulas, links and pictures.
fn markdown_left_out(source: &str) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = front_matter(source).into_iter().collect();
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.math_dollars = true;
    options.extension.math_code = true;
    let root = parse_document(&arena, source, &options);
    let starts = line_starts(source);
    let offset = |line: usize, column: usize| {
        starts.get(line.saturating_sub(1)).copied().unwrap_or(0) + column.saturating_sub(1)
    };
    let mut code_lines = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let block = matches!(
            data.value,
            NodeValue::CodeBlock(_) | NodeValue::HtmlBlock(_) | NodeValue::FrontMatter(_)
        );
        let inline = matches!(
            data.value,
            NodeValue::Code(_)
                | NodeValue::HtmlInline(_)
                | NodeValue::Link(_)
                | NodeValue::Image(_)
                | NodeValue::Math(_)
        );
        if !block && !inline {
            continue;
        }
        let (from, to) = (data.sourcepos.start, data.sourcepos.end);
        if block {
            code_lines.push(from.line..=to.line);
            // Whole lines, so an indented fence is left out too.
            let end = starts.get(to.line).copied().unwrap_or(source.len());
            out.push(offset(from.line, 1)..end);
        } else {
            let start = offset(from.line, from.column);
            let end = (offset(to.line, to.column) + 1).min(source.len());
            out.push(start..end.max(start));
        }
    }
    out.extend(math_bytes(source, &starts, &code_lines));
    out.extend(markdown_brackets(source));
    out
}

/// Markdown brackets that are no plain text: a footnote reference
/// (`[^…]`), a wiki-like `[[…]]`, and a link by reference without its
/// definition (`[text][label]`), which a mention linked inside would break.
fn markdown_brackets(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = source[at..].find('[').map(|o| at + o) {
        let escaped = open > 0 && bytes[open - 1] == b'\\';
        let double = bytes.get(open + 1) == Some(&b'[');
        let Some(close) = source[open + 1..]
            .find([']', '\n', '\r'])
            .map(|c| open + 1 + c)
            .filter(|c| bytes[*c] == b']')
        else {
            at = open + 1;
            continue;
        };
        if escaped {
            at = open + 1;
            continue;
        }
        if double {
            let end = source[open..]
                .find("]]")
                .map(|c| open + c + 2)
                .filter(|end| !source[open..*end].contains(['\n', '\r']));
            if let Some(end) = end {
                out.push(open..end);
                at = end;
                continue;
            }
        }
        let footnote = bytes.get(open + 1) == Some(&b'^');
        let reference = bytes.get(close + 1) == Some(&b'[');
        if footnote {
            out.push(open..close + 1);
        } else if reference {
            let end = source[close + 2..]
                .find([']', '\n'])
                .map_or(close + 1, |c| close + 2 + c + 1);
            out.push(open..end);
            at = end;
            continue;
        }
        at = close + 1;
    }
    out
}

/// What holds no mention in an AsciiDoc note: delimited code, literal,
/// passthrough and comment blocks, comment lines, attribute lines (the
/// note's attributes and those of a block), and inline code, passthroughs
/// and formulas.
fn asciidoc_left_out(source: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut block: Option<(String, usize)> = None;
    // A paragraph styled as code (`[source]`, `[listing]`, `[literal]`)
    // or one that starts indented (a literal paragraph) runs to the next
    // empty line.
    let mut literal = false;
    let mut at = 0;
    for line in lines(source) {
        let start = at;
        at += line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        if let Some((open, from)) = &block {
            if content.trim_end() == open {
                out.push(*from..at);
                block = None;
            }
            continue;
        }
        let trimmed = content.trim();
        if literal {
            if trimmed.is_empty() {
                literal = false;
            } else {
                out.push(start..at);
            }
            continue;
        }
        if let Some(delimiter) = block_delimiter(content, NoteFormat::AsciiDoc) {
            block = Some((delimiter, start));
            continue;
        }
        let attribute = trimmed.starts_with(':')
            && trimmed[1..]
                .find(':')
                .is_some_and(|end| end > 0 && !trimmed[1..=end].contains(char::is_whitespace));
        let block_attributes = trimmed.starts_with('[') && trimmed.ends_with(']');
        let preprocessor = ["ifdef::", "ifndef::", "ifeval::", "endif::", "include::"]
            .iter()
            .any(|name| trimmed.starts_with(name));
        let indented = content.starts_with([' ', '\t']) && !is_list_line(trimmed);
        if content.starts_with("//") || attribute || block_attributes || preprocessor || indented {
            out.push(start..at);
            let style = trimmed
                .trim_start_matches('[')
                .split([',', ']', '.', '#', '%'])
                .next()
                .unwrap_or_default();
            literal =
                indented || (block_attributes && matches!(style, "source" | "listing" | "literal"));
            continue;
        }
        asciidoc_inline_left_out(content, start, &mut out);
    }
    if let Some((_, from)) = block {
        out.push(from..source.len());
    }
    out
}

/// Whether an indented AsciiDoc line is a list item (`* x`, `- x`,
/// `. x`, `1. x`), not a literal paragraph.
fn is_list_line(trimmed: &str) -> bool {
    let marker = trimmed
        .trim_start_matches(['*', '-', '.'])
        .len()
        .abs_diff(trimmed.len());
    let numbered = trimmed
        .split_once(". ")
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    (marker > 0 && trimmed[marker..].starts_with(' ')) || numbered
}

/// The inline code (`` `…` ``), passthroughs (`+…+`, `$$…$$`), formulas
/// (`\(…\)`), macros (`name:target[…]`, `name::target[…]`: `xref:`,
/// `kbd:`, `btn:`, `menu:`, `anchor:`, `icon:`, `video::`, `stem:`,
/// `pass:` and every other), references (`<<…>>`), attribute references
/// (`{name}`) and e-mail addresses of one AsciiDoc line starting at byte
/// `start`.
fn asciidoc_inline_left_out(line: &str, start: usize, out: &mut Vec<Range<usize>>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let paired = [("$$", "$$"), ("\\(", "\\)"), ("<<", ">>"), ("{", "}")]
            .iter()
            .find(|(open, _)| rest.starts_with(open))
            .and_then(|(open, close)| {
                rest[open.len()..]
                    .find(close)
                    .map(|end| i + open.len() + end + close.len())
            });
        if let Some(end) = paired {
            out.push(start + i..start + end);
            i = end;
            continue;
        }
        if let Some(end) = inline_macro_end(line, i) {
            out.push(start + i..start + end);
            i = end;
            continue;
        }
        if bytes[i] == b'@' {
            let word = |b: &u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+');
            let from = i - bytes[..i].iter().rev().take_while(|b| word(b)).count();
            let to = i + 1 + bytes[i + 1..].iter().take_while(|b| word(b)).count();
            if from < i && to > i + 1 {
                out.push(start + from..start + to);
                i = to;
                continue;
            }
        }
        if matches!(bytes[i], b'`' | b'+') {
            let mark = bytes[i];
            let run = bytes[i..].iter().take_while(|b| **b == mark).count();
            let fence = &line[i..i + run];
            if let Some(close) = line[i + run..].find(fence) {
                let end = i + run + close + run;
                out.push(start + i..start + end);
                i = end;
                continue;
            }
            i += run;
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
}

/// The end of an AsciiDoc inline macro starting at byte `i` of `line`: a
/// name of ASCII letters at a word start, `:` or `::`, a target without
/// spaces, and `[…]` (an unclosed one to the end of the line).
fn inline_macro_end(line: &str, i: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    if i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
        return None;
    }
    let name = bytes[i..]
        .iter()
        .take_while(|b| b.is_ascii_alphabetic())
        .count();
    if name == 0 || bytes.get(i + name) != Some(&b':') {
        return None;
    }
    let colons = if bytes.get(i + name + 1) == Some(&b':') {
        2
    } else {
        1
    };
    let target = i + name + colons;
    let open = target + line[target..].find(['[', ' ', '\t'])?;
    if bytes[open] != b'[' {
        return None;
    }
    let close = line[open..].find(']').map_or(line.len(), |c| open + c + 1);
    Some(close)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mentions<'a>(source: &'a str, format: NoteFormat, title: &str) -> Vec<&'a str> {
        let title = MentionTitle::new(title).expect("a title long enough");
        unlinked_mentions(source, format, &title)
            .into_iter()
            .map(|range| &source[range])
            .collect()
    }

    /// UC-004 BR-021, TC-004-42: whole words, upper and lower case alike,
    /// the words of the title apart by spaces or tabs.
    #[test]
    fn uc_004_br_021_whole_words_and_case() {
        let text = "Road map, ROAD  MAP and road\tmap; roadmap, road maps, Road-map, broad map.\n";
        assert_eq!(
            mentions(text, NoteFormat::Markdown, "Road map"),
            ["Road map", "ROAD  MAP", "road\tmap"]
        );
        // Words apart over two lines are no mention.
        assert!(mentions("road\nmap\n", NoteFormat::Markdown, "Road map").is_empty());
        // A part of a word is none.
        assert!(mentions(
            "Planning, plans, plan2, 2plan\n",
            NoteFormat::Markdown,
            "Plan"
        )
        .is_empty());
        // In emphasis it is one.
        assert_eq!(
            mentions(
                "**Plan** and _plan_ and *plan*\n",
                NoteFormat::Markdown,
                "Plan"
            ),
            ["Plan", "plan", "plan"]
        );
    }

    /// NFR-019: a text that holds the words of a title may mention it; one
    /// that lacks a word cannot.
    #[test]
    fn nfr_019_word_keys() {
        let keys = WordKeys::of("The ROAD map, über Größe; C++ tips\r\n");
        let may = |title: &str| keys.may_mention(&MentionTitle::new(title).unwrap());
        assert!(may("road map"));
        assert!(may("Map Road"), "the order is not kept");
        assert!(may("über größe"));
        assert!(may("C++ tips"));
        assert!(!may("road maps"));
        assert!(!may("Plan"));
        assert!(!WordKeys::default().may_mention(&MentionTitle::new("abc").unwrap()));
    }

    /// BR-021: a title shorter than three characters has no mentions.
    #[test]
    fn uc_004_br_021_short_titles() {
        assert!(MentionTitle::new("Go").is_none());
        assert!(MentionTitle::new(" a b ").is_none());
        assert!(MentionTitle::new("").is_none());
        assert!(MentionTitle::new("Ufo").is_some());
        assert!(MentionTitle::new("ab c").is_some());
    }

    /// BR-021: multibyte titles and text, upper and lower case alike, and
    /// the places of the mentions in bytes.
    #[test]
    fn uc_004_br_021_multibyte() {
        let text = "Über Größe: ÜBER GRÖSSE nicht, über größe ja. Ὀδυσσεύς ΟΔΥΣΣΕΥΣ\n";
        assert_eq!(
            mentions(text, NoteFormat::Markdown, "Über Größe"),
            ["Über Größe", "über größe"]
        );
        assert_eq!(
            mentions("ΟΔΟΣ und οδος und οδοσ\n", NoteFormat::Markdown, "Οδός"),
            Vec::<&str>::new(),
            "accents count"
        );
        assert_eq!(
            mentions("ΟΔΟΣ und οδος und οδοσ\n", NoteFormat::Markdown, "οδος"),
            ["ΟΔΟΣ", "οδος", "οδοσ"]
        );
        // A word of other letters right after is part of the word.
        assert!(mentions("Plané\n", NoteFormat::Markdown, "Plan").is_empty());
    }

    /// BR-021: in Markdown, code, links, pictures, addresses, formulas,
    /// HTML, front matter and tags hold no mention; headings and tables
    /// do.
    #[test]
    fn uc_004_br_021_markdown_left_out() {
        let text = "---\ntitle: Plan\n---\n# Plan for May\n\n\
            Plan `plan` [plan](Other.md) [the plan][r] ![plan](plan.png) \
            <https://x.org/plan> https://plan.org/plan $plan$ <b>plan</b> #plan plan.\n\n\
            ```\nplan\n```\n\n    plan in indented code\n\n<!-- plan -->\n\n$$\nplan\n$$\n\n\
            | a | plan |\n|---|---|\n\n[r]: Plan.md \"plan\"\n";
        let found = mentions(text, NoteFormat::Markdown, "plan");
        assert_eq!(found, ["Plan", "Plan", "plan", "plan", "plan"], "{text}");
        let title = MentionTitle::new("plan").unwrap();
        let places = unlinked_mentions(text, NoteFormat::Markdown, &title);
        assert_eq!(&text[places[0].start - 2..places[0].end], "# Plan");
        assert!(text[places[1].end..].starts_with(" `plan`"));
        // Text in HTML markup is text.
        assert!(text[..places[2].start].ends_with("<b>"));
        assert!(text[..places[3].start].ends_with("#plan "));
        assert!(text[..places[4].start].ends_with("| a | "));
    }

    /// BR-021: in AsciiDoc, delimited blocks, comments, attribute lines,
    /// inline code, passthroughs, formulas and macros hold no mention.
    #[test]
    fn uc_004_br_021_asciidoc_left_out() {
        let text = "= Plan\n:description: plan\n// plan\n[quote, plan]\nplan `plan` +plan+ \
            stem:[plan] pass:[plan] link:Other.adoc[plan] xref:x.adoc[plan] <<o.adoc#,plan>> \
            image:plan.png[plan] https://plan.org[plan] plan\ninclude::plan.adoc[]\n\
            ----\nplan\n----\n////\nplan\n////\n";
        assert_eq!(
            mentions(text, NoteFormat::AsciiDoc, "Plan"),
            ["Plan", "plan", "plan"]
        );
    }

    /// BR-021: lines ending in CRLF, and a note that starts with a byte
    /// order mark and front matter.
    #[test]
    fn uc_004_br_021_crlf_and_bom() {
        let text = "\u{feff}---\r\ntags: [plan]\r\n---\r\nThe plan.\r\n`plan`\r\nplan\r\n";
        let title = MentionTitle::new("Plan").unwrap();
        let found = unlinked_mentions(text, NoteFormat::Markdown, &title);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|range| &text[range.clone()] == "plan"));
        let adoc = "= Notes\r\nThe plan.\r\n----\r\nplan\r\n----\r\nplan\r\n";
        assert_eq!(
            mentions(adoc, NoteFormat::AsciiDoc, "Plan"),
            ["plan", "plan"]
        );
    }

    /// BR-021 (chaos): a code span across lines, an open fence, an open
    /// block and a stray backtick never panic and leave out what they open.
    #[test]
    fn uc_004_br_021_chaos_unclosed_markup() {
        assert_eq!(
            mentions("plan `plan\nplan` plan\n", NoteFormat::Markdown, "plan"),
            ["plan", "plan"]
        );
        assert_eq!(
            mentions("plan\n```\nplan\n", NoteFormat::Markdown, "plan"),
            ["plan"]
        );
        assert_eq!(
            mentions("plan\n----\nplan\n", NoteFormat::AsciiDoc, "plan"),
            ["plan"]
        );
        assert_eq!(
            mentions("a ` plan\n", NoteFormat::AsciiDoc, "plan"),
            ["plan"]
        );
        for text in [
            "",
            "\n",
            "plan",
            "[plan",
            "![",
            "`",
            "+",
            "stem:[plan",
            "---\nplan",
        ] {
            for format in NoteFormat::ALL {
                let _ = mentions(text, format, "plan");
            }
        }
    }

    /// UC-004 BR-021 (monkey): a note whose lines end in a lone CR (the old
    /// Mac convention) has its code left out as any other; the lines are
    /// counted by the line ends the parser knows.
    #[test]
    fn uc_004_br_021_monkey_lone_cr_code_is_left_out() {
        assert!(mentions("x\r\r\r\rab `zork`", NoteFormat::Markdown, "zork").is_empty());
        assert!(mentions("x\ry\r----\rzork\r----\r", NoteFormat::AsciiDoc, "zork").is_empty());
    }

    /// UC-004 BR-021 (monkey): in AsciiDoc a cross reference to a place of
    /// the note (`xref:zork[]`, `<<zork>>`, `<<zork,zork>>`) is a link: its
    /// place and its text hold no mention; "Link" would write a link into
    /// the address of the reference.
    #[test]
    fn uc_004_br_021_monkey_asciidoc_internal_references_are_links() {
        for text in [
            "See xref:zork[].\n",
            "See <<zork>>.\n",
            "See <<zork,zork>>.\n",
        ] {
            assert!(
                mentions(text, NoteFormat::AsciiDoc, "zork").is_empty(),
                "{text}"
            );
        }
    }

    /// UC-004 BR-021 (monkey): an attribute reference (`{zork}`), a
    /// conditional (`ifdef::zork[]`) and an e-mail address
    /// (`a@zork.org`) are no plain text; a link inside them breaks them.
    #[test]
    fn uc_004_br_021_monkey_asciidoc_references_conditionals_and_addresses() {
        for text in [
            "The {zork} value.\n",
            "ifdef::zork[]\ntext\nendif::[]\n",
            "Write to a@zork.org today.\n",
        ] {
            assert!(
                mentions(text, NoteFormat::AsciiDoc, "zork").is_empty(),
                "{text}"
            );
        }
    }

    /// UC-004 BR-021 (monkey): a paragraph styled as source or listing
    /// (`[source]` and the lines after it, no delimiters) is code.
    #[test]
    fn uc_004_br_021_monkey_asciidoc_styled_paragraph_is_code() {
        let text = "[source]\nlet zork = 1;\n\nzork\n";
        assert_eq!(mentions(text, NoteFormat::AsciiDoc, "zork"), ["zork"]);
    }

    /// UC-004 BR-021 (monkey): "whole words": a letter with a combining
    /// accent after it (`cafe` + U+0301, which shows `café`) belongs to
    /// the word, so the title "cafe" is not mentioned there, and no
    /// mention ends inside a letter and its mark.
    #[test]
    fn uc_004_br_021_monkey_combining_marks_belong_to_the_word() {
        assert!(mentions("un cafe\u{301} noir\n", NoteFormat::Markdown, "cafe").is_empty());
    }

    /// BR-021 (review): AsciiDoc inline macros, passthroughs and formulas,
    /// and indented literal lines hold no mention; list items and the
    /// text around them do.
    #[test]
    fn uc_004_br_021_asciidoc_macros_and_literals() {
        let text = "kbd:[zork] btn:[zork] menu:zork[Save] anchor:zork[] icon:zork[] \
            video::zork.mp4[] footnote:[zork] $$zork$$ \\(zork\\) zork\n\n  zork literal\n  more zork\n\n\
            * zork item\n  . zork step\n";
        assert_eq!(
            mentions(text, NoteFormat::AsciiDoc, "zork"),
            ["zork", "zork", "zork"]
        );
    }

    /// BR-021 (review): a mention right after a backslash in AsciiDoc is
    /// none, since a link there would be escaped; in Markdown it is one.
    #[test]
    fn uc_004_br_021_after_a_backslash() {
        assert!(mentions("C:\\zork\n", NoteFormat::AsciiDoc, "zork").is_empty());
        assert_eq!(
            mentions("C:\\\\zork\n", NoteFormat::AsciiDoc, "zork"),
            ["zork"]
        );
        assert_eq!(
            mentions("C:\\zork\n", NoteFormat::Markdown, "zork"),
            ["zork"]
        );
    }

    /// BR-021 (review): Markdown footnote references, `[[…]]` and links by
    /// reference without a definition hold no mention; plain brackets do.
    #[test]
    fn uc_004_br_021_markdown_brackets() {
        let text = "a[^zork] [[zork]] [zork][x] [the zork] \\[zork] zork\n";
        assert_eq!(
            mentions(text, NoteFormat::Markdown, "zork"),
            ["zork", "zork", "zork"]
        );
    }

    /// BR-021, OQ-304 (review): a no-break space in the title or the text
    /// parts words like a space; a line end does not.
    #[test]
    fn uc_004_br_021_no_break_spaces() {
        assert_eq!(
            mentions(
                "road\u{a0}map and road map\n",
                NoteFormat::Markdown,
                "Road map"
            ),
            ["road\u{a0}map", "road map"]
        );
        assert_eq!(
            mentions("road map\n", NoteFormat::Markdown, "Road\u{a0}map"),
            ["road map"]
        );
        assert!(mentions("road\u{2028}map\n", NoteFormat::Markdown, "Road map").is_empty());
    }

    /// BR-021, NFR-019 (review): at most 1000 mentions of one note are
    /// found; the word keys of a note grow with its distinct words only.
    #[test]
    fn nfr_019_bounded_costs() {
        let many = "zork ".repeat(5000);
        assert_eq!(
            mentions(&many, NoteFormat::Markdown, "zork").len(),
            MOST_PER_NOTE
        );
        assert_eq!(WordKeys::of(&many).len(), 1);
        let distinct: String = (0..2000).map(|n| format!("w{n} ")).collect();
        assert_eq!(WordKeys::of(&distinct).len(), 2000);
        assert!(WordKeys::default().is_empty());
    }
}
