//! FR-080, UC-005 A23, A24, BR-018: link completion. Whether the user is
//! typing the place of a link at the cursor, what they typed of it, and the
//! link written when they chose a note or a heading. Nothing here reads
//! files, so the window's editor core answers it on every key (ADR-025,
//! NFR-018); which notes match is the rule of go to note
//! ([`crate::suggest`]).
//!
//! The place of a link is typed
//!
//! - in the source view of a Markdown note after `](`,
//! - in the source view of an AsciiDoc note after `xref:` or `<<`,
//! - in the facade of either after `[[`, which only starts the completion:
//!   the link replaces it, so the note never holds a wiki link (C-003,
//!   C-004).
//!
//! Ctrl+Space asks for the completion in the place of a link of the source
//! view, and in the facade at the cursor. The completed link is a normal
//! link of the note format, as the formatting toolbar writes a link to a
//! note ([`apply_format`]): relative to the note that holds it, with the
//! note's extension (OQ-201), and with the anchor of a heading after `#`.

use std::ops::Range;

use crate::formatting::{apply_format, Format};
use crate::NoteFormat;

/// How the user started the link being completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Markdown, source view: `[text](place`.
    Place,
    /// AsciiDoc, source view: `xref:place`.
    Xref,
    /// AsciiDoc, source view: `<<place`.
    Reference,
    /// The facade: `[[place`; the `[[` goes when the link is written.
    Brackets,
    /// The facade: Ctrl+Space at the cursor, nothing typed yet.
    Here,
}

/// UC-005 A23: the place of a link the user is typing at the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkContext {
    /// How it was started.
    pub trigger: Trigger,
    /// The bytes of the text the completed link replaces: the whole link
    /// typed so far, with what of it follows the cursor.
    pub replace: Range<usize>,
    /// What was typed of the place before the cursor, without a `#` and
    /// what follows it.
    pub typed: String,
    /// What was typed after a `#`: the headings are listed then.
    pub heading: Option<String>,
    /// The text the user gave the link, kept when the link is written.
    pub label: Option<String>,
}

/// UC-005 A23 step 1: the link whose place is typed at byte `cursor` of
/// `text`, a note in `format`, in the `facade` or the source view; with
/// `manual` (Ctrl+Space) the facade completes at the cursor also without
/// `[[`. `None` when no place of a link is typed there. Whether the cursor
/// is in code is the caller's question.
///
/// # Examples
///
/// ```
/// use underline_markup::completion::{link_context, Trigger};
/// use underline_markup::NoteFormat;
///
/// let text = "See [list](travel/pack";
/// let found = link_context(text, text.len(), NoteFormat::Markdown, false, false).unwrap();
/// assert_eq!(found.trigger, Trigger::Place);
/// assert_eq!(found.typed, "travel/pack");
/// assert_eq!(found.label.as_deref(), Some("list"));
/// assert_eq!(&text[found.replace], "[list](travel/pack");
/// ```
#[must_use]
pub fn link_context(
    text: &str,
    cursor: usize,
    format: NoteFormat,
    facade: bool,
    manual: bool,
) -> Option<LinkContext> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let line_start = text[..cursor].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[cursor..]
        .find('\n')
        .map_or(text.len(), |at| cursor + at);
    let before = &text[line_start..cursor];
    let after = text[cursor..line_end].trim_end_matches('\r');
    let found = if facade {
        brackets(before).or_else(|| {
            manual.then(|| Found {
                trigger: Trigger::Here,
                start: before.len(),
                typed: String::new(),
                end: 0,
                label: None,
            })
        })
    } else {
        match format {
            NoteFormat::Markdown => markdown_place(before, after),
            NoteFormat::AsciiDoc => asciidoc_place(before, after),
        }
    }?;
    // In a code span no link is typed.
    if in_code_span(&before[..found.start.min(before.len())]) {
        return None;
    }
    let (typed, heading) = match found.typed.split_once('#') {
        Some((typed, heading)) => (typed.to_owned(), Some(heading.to_owned())),
        None => (found.typed, None),
    };
    Some(LinkContext {
        trigger: found.trigger,
        replace: line_start + found.start..cursor + found.end,
        typed,
        heading,
        label: found.label,
    })
}

/// Whether a code span is open at the end of `line`: an odd number of
/// backticks not escaped with `\\`.
fn in_code_span(line: &str) -> bool {
    let mut ticks = 0usize;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '`' {
            ticks += 1;
        }
    }
    ticks % 2 == 1
}

/// What was found on the line: where the link starts in `before`, what was
/// typed of its place, how many bytes of `after` it takes, its text.
struct Found {
    trigger: Trigger,
    start: usize,
    typed: String,
    end: usize,
    label: Option<String>,
}

/// Characters that end the place of a link in the source view.
fn ends_place(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | '[' | ']' | '"')
}

/// Markdown: `[text](place` before the cursor; a `)` right after the rest
/// of the place belongs to the link.
fn markdown_place(before: &str, after: &str) -> Option<Found> {
    let open = before.rfind("](")?;
    let typed = &before[open + 2..];
    if typed.contains(ends_place) {
        return None;
    }
    let bracket = before[..open].rfind('[')?;
    let lead = &before[..bracket];
    // A picture or an escaped bracket is no link to a note.
    if lead.ends_with('!') || lead.ends_with('\\') {
        return None;
    }
    let rest = after.find(ends_place).unwrap_or(after.len());
    let end = if after[rest..].starts_with(')') {
        rest + 1
    } else {
        0
    };
    Some(Found {
        trigger: Trigger::Place,
        start: bracket,
        typed: typed.to_owned(),
        end,
        label: Some(before[bracket + 1..open].to_owned()),
    })
}

/// The last occurrence of `word` in `line` that starts a word.
fn word_start(line: &str, word: &str) -> Option<usize> {
    line.match_indices(word)
        .map(|(at, _)| at)
        .filter(|at| {
            line[..*at]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric())
        })
        .last()
}

/// AsciiDoc: `xref:place` or `<<place` before the cursor, whichever comes
/// later; a `[text]` or `,text>>` after the rest of the place belongs to
/// the link, and its text is kept.
fn asciidoc_place(before: &str, after: &str) -> Option<Found> {
    let xref = word_start(before, "xref:").map(|at| (at, Trigger::Xref, at + 5));
    let reference = before
        .rfind("<<")
        .map(|at| (at, Trigger::Reference, at + 2));
    let (start, trigger, from) = match (xref, reference) {
        (Some(x), Some(r)) => {
            if x.0 > r.0 {
                x
            } else {
                r
            }
        }
        (x, r) => x.or(r)?,
    };
    let typed = &before[from..];
    let stops = |c: char| ends_place(c) || c == ',';
    if typed.contains(stops) {
        return None;
    }
    let rest = after.find(stops).unwrap_or(after.len());
    let tail = &after[rest..];
    let (end, label) = match trigger {
        Trigger::Xref => match tail.strip_prefix('[').and_then(|inner| inner.find(']')) {
            Some(close) => (rest + close + 2, Some(tail[1..=close].to_owned())),
            None => (0, None),
        },
        _ => {
            if let Some(inner) = tail.strip_prefix(',') {
                match inner.find(">>") {
                    Some(close) => (rest + close + 3, Some(inner[..close].to_owned())),
                    None => (0, None),
                }
            } else if tail.starts_with(">>") {
                (rest + 2, None)
            } else {
                (0, None)
            }
        }
    };
    Some(Found {
        trigger,
        start,
        typed: typed.to_owned(),
        end,
        label,
    })
}

/// The facade: the last `[[` before the cursor, also as the facade escapes
/// it (`\[\[`), with what was typed after it as the facade shows it.
fn brackets(before: &str) -> Option<Found> {
    // The characters as the facade shows them, each with the byte its
    // markup starts at.
    let mut shown: Vec<(usize, char)> = Vec::new();
    let mut chars = before.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '\\' {
            if let Some(&(_, next)) = chars.peek() {
                if next.is_ascii_punctuation() {
                    chars.next();
                    shown.push((at, next));
                    continue;
                }
            }
        }
        shown.push((at, c));
    }
    let open = shown
        .windows(2)
        .rposition(|pair| pair[0].1 == '[' && pair[1].1 == '[')?;
    let typed: String = shown[open + 2..].iter().map(|(_, c)| *c).collect();
    if typed.contains("]]") {
        return None;
    }
    Some(Found {
        trigger: Trigger::Brackets,
        start: shown[open].0,
        typed,
        end: 0,
        label: None,
    })
}

/// UC-005 A23 step 3, BR-018: the markup of the completed link to `place`
/// in a note in `format`, for the link `context`: the text the user gave
/// it, else `label` (the title of the note or the heading). A Markdown
/// link and a link of the facade are written as the toolbar writes a link
/// to a note; `xref:` and `<<` keep the form the user started.
///
/// # Examples
///
/// ```
/// use underline_markup::completion::{link_context, write_link};
/// use underline_markup::NoteFormat;
///
/// let text = "[[plan";
/// let found = link_context(text, text.len(), NoteFormat::Markdown, true, false).unwrap();
/// assert_eq!(write_link(&found, NoteFormat::Markdown, "Plan.md", "Plan"), "[Plan](Plan.md)");
/// ```
#[must_use]
pub fn write_link(context: &LinkContext, format: NoteFormat, place: &str, label: &str) -> String {
    // The text the user gave is markup already; a title or a heading is
    // text, so the brackets that would end the link are escaped.
    let label = match context
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
    {
        Some(given) => given.to_owned(),
        None => escape_label(label, format),
    };
    match context.trigger {
        Trigger::Xref => format!("xref:{place}[{label}]"),
        Trigger::Reference => format!("<<{place},{}>>", label.replace(">>", "\\>>")),
        // One way of writing for the toolbar and the completion: `[text](place)`
        // in Markdown (a place after `](` is Markdown only), `link:place[text]`
        // in AsciiDoc.
        Trigger::Place | Trigger::Brackets | Trigger::Here => {
            let link = Format::Link {
                address: place.to_owned(),
                label: Some(label),
            };
            let format = if context.trigger == Trigger::Place {
                NoteFormat::Markdown
            } else {
                format
            };
            apply_format("", format, 0..0, &link).text
        }
    }
}

/// `label` as the text of a link in `format`: the brackets that would end
/// it are escaped (`\[`, `\]` in Markdown, `\]` in AsciiDoc).
fn escape_label(label: &str, format: NoteFormat) -> String {
    let mut out = String::with_capacity(label.len());
    for c in label.chars() {
        let escaped = match format {
            NoteFormat::Markdown => matches!(c, '[' | ']' | '\\'),
            NoteFormat::AsciiDoc => c == ']',
        };
        if escaped {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// UC-005 BR-018: the place of the file `to` (relative to the workspace,
/// with `/`) as a link in the note whose file is `from` names it: relative
/// to the folder of `from`, as the toolbar writes it, with `#anchor` for a
/// heading; only `#anchor` for a heading of `from` itself. Characters that
/// would end the place or start a heading are percent-encoded (OQ-248).
///
/// # Examples
///
/// ```
/// use underline_markup::completion::link_place;
///
/// assert_eq!(link_place("Ideas.md", "travel/Packing.md", None), "travel/Packing.md");
/// assert_eq!(link_place("travel/Trip.adoc", "Plan.md", Some("goals")), "../Plan.md#goals");
/// assert_eq!(link_place("Plan.md", "Plan.md", Some("goals")), "#goals");
/// ```
#[must_use]
pub fn link_place(from: &str, to: &str, anchor: Option<&str>) -> String {
    let mut place = if from == to && anchor.is_some() {
        String::new()
    } else {
        let folder: Vec<&str> = from
            .rsplit_once('/')
            .map_or_else(Vec::new, |(folder, _)| folder.split('/').collect());
        let parts: Vec<&str> = to.split('/').collect();
        let common = folder
            .iter()
            .zip(&parts)
            .take_while(|(a, b)| a == b)
            .count();
        let mut out: Vec<String> = vec!["..".to_owned(); folder.len() - common];
        out.extend(parts[common..].iter().map(|part| encode_place(part)));
        out.join("/")
    };
    if let Some(anchor) = anchor {
        place.push('#');
        place.push_str(anchor);
    }
    place
}

/// A part of a place as a link writes it (OQ-248): `%`, `#`, spaces,
/// brackets and angle brackets percent-encoded, so the place is read as one
/// and its `#` does not start a heading.
fn encode_place(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for c in part.chars() {
        match c {
            '%' | '#' | ' ' | '(' | ')' | '[' | ']' | '<' | '>' | '\t' => {
                out.push_str(&format!("%{:02X}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out
}

/// The note (its place in the workspace without the extension) that the
/// place `typed` names in the note whose file is `from`, relative to its
/// folder; `None` when nothing is typed or it leads outside the workspace.
///
/// # Examples
///
/// ```
/// use underline_markup::completion::named_note;
///
/// assert_eq!(named_note("Ideas.md", "travel/Packing.md").as_deref(), Some("travel/Packing"));
/// assert_eq!(named_note("travel/Trip.adoc", "../Plan").as_deref(), Some("Plan"));
/// assert_eq!(named_note("Ideas.md", "../x.md"), None);
/// ```
#[must_use]
pub fn named_note(from: &str, typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = from
        .rsplit_once('/')
        .map_or_else(Vec::new, |(folder, _)| folder.split('/').collect());
    for part in typed.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    let joined = parts.join("/");
    let stem = joined
        .rsplit_once('.')
        .filter(|(stem, extension)| {
            !stem.is_empty() && NoteFormat::from_extension(extension).is_some()
        })
        .map_or(joined.as_str(), |(stem, _)| stem);
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// UC-005 A23: whether a code, literal, passthrough or comment block is
/// open after `lines` (the lines before the cursor's, in order), so no
/// place of a link is typed on the next line. Read line by line without a
/// parse, as the rename finds links outside such blocks.
pub fn in_code_block<L: AsRef<str>>(
    lines: impl IntoIterator<Item = L>,
    format: NoteFormat,
) -> bool {
    let mut open: Option<String> = None;
    // Markdown: the indent of the content of the list item the lines are
    // in; a fence may stand that much further in (CommonMark).
    let mut item = 0usize;
    for line in lines {
        let line = line.as_ref().trim_end_matches(['\n', '\r']);
        if let Some(fence) = &open {
            if closes(line, fence, format) {
                open = None;
            }
            continue;
        }
        if format == NoteFormat::Markdown && !line.trim().is_empty() {
            let indent = line.len() - line.trim_start_matches(' ').len();
            if let Some(content) = list_content_indent(line) {
                item = content;
            } else if indent < item {
                item = 0;
            }
            open = fence_of(&line[indent.min(item)..], format);
        } else {
            open = fence_of(line, format);
        }
    }
    open.is_some()
}

/// Markdown: the indent of the content of the list item `line` starts
/// (`- `, `* `, `+ `, `1. `, `1) `), if it starts one.
fn list_content_indent(line: &str) -> Option<usize> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let marker = if rest.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let after = rest[digits..].chars().next();
        if (1..=9).contains(&digits) && matches!(after, Some('.' | ')')) {
            digits + 1
        } else {
            return None;
        }
    };
    let spaces = rest[marker..].chars().take_while(|c| *c == ' ').count();
    (1..=4)
        .contains(&spaces)
        .then_some(indent + marker + spaces)
}

/// UC-005 A23: whether a code span is open at the end of `text` (the
/// paragraph before the cursor): an odd number of backticks not escaped.
#[must_use]
pub fn code_span_open(text: &str) -> bool {
    in_code_span(text)
}

/// The fence `line` opens a block with, if it opens one.
fn fence_of(line: &str, format: NoteFormat) -> Option<String> {
    let line = line.trim_end();
    match format {
        NoteFormat::Markdown => {
            let trimmed = line.trim_start();
            let fence = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
            let run = trimmed.chars().take_while(|c| *c == fence).count();
            (run >= 3 && line.len() - trimmed.len() < 4).then(|| trimmed[..run].to_owned())
        }
        NoteFormat::AsciiDoc => {
            let first = line.chars().next()?;
            let delimited = matches!(first, '-' | '.' | '+' | '/')
                && line.len() >= 4
                && line.chars().all(|c| c == first);
            (delimited || line == "```").then(|| line.to_owned())
        }
    }
}

/// Whether `line` closes the block opened with `fence`.
fn closes(line: &str, fence: &str, format: NoteFormat) -> bool {
    let line = line.trim_end();
    match format {
        NoteFormat::Markdown => {
            let trimmed = line.trim_start();
            let kind = fence.chars().next().unwrap_or('`');
            trimmed.len() >= fence.len() && trimmed.chars().all(|c| c == kind)
        }
        NoteFormat::AsciiDoc => line == fence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(text: &str, format: NoteFormat, facade: bool) -> Option<LinkContext> {
        link_context(text, text.len(), format, facade, false)
    }

    /// TC-005-33 step 1: `[list](pack` in the source view of a Markdown
    /// note is the place of a link.
    #[test]
    fn tc_005_33_step_1_markdown_place_after_bracket_paren() {
        let text = "Intro\n[list](pack";
        let found = at_end(text, NoteFormat::Markdown, false).unwrap();
        assert_eq!(found.trigger, Trigger::Place);
        assert_eq!(found.typed, "pack");
        assert_eq!(found.heading, None);
        assert_eq!(found.label.as_deref(), Some("list"));
        assert_eq!(&text[found.replace.clone()], "[list](pack");
        assert_eq!(
            write_link(&found, NoteFormat::Markdown, "travel/Packing.md", "Packing"),
            "[list](travel/Packing.md)"
        );
    }

    /// TC-005-33 step 3: after `#` the headings are asked for; an empty
    /// text takes the heading's.
    #[test]
    fn tc_005_33_step_3_heading_after_hash() {
        let text = "[](travel/Packing.md#pa";
        let found = at_end(text, NoteFormat::Markdown, false).unwrap();
        assert_eq!(found.typed, "travel/Packing.md");
        assert_eq!(found.heading.as_deref(), Some("pa"));
        assert_eq!(
            write_link(
                &found,
                NoteFormat::Markdown,
                "travel/Packing.md#papers",
                "Papers"
            ),
            "[Papers](travel/Packing.md#papers)"
        );
    }

    /// TC-005-33 step 4: `[[plan` in the facade, escaped as the facade
    /// writes it; the `[[` goes with the link.
    #[test]
    fn tc_005_33_step_4_facade_brackets_escaped_or_not() {
        for text in ["Go \\[\\[plan", "Go [[plan"] {
            let found = at_end(text, NoteFormat::Markdown, true).unwrap();
            assert_eq!(found.trigger, Trigger::Brackets);
            assert_eq!(found.typed, "plan");
            assert_eq!(found.replace.start, 3, "{text}");
            assert_eq!(found.replace.end, text.len());
            assert_eq!(
                write_link(&found, NoteFormat::Markdown, "Plan.md", "Plan"),
                "[Plan](Plan.md)"
            );
            assert_eq!(
                write_link(&found, NoteFormat::AsciiDoc, "Plan.md", "Plan"),
                "link:Plan.md[Plan]"
            );
        }
        // Words with spaces, and a heading of the active note.
        let found = at_end("\\[\\[plane tick\\#go", NoteFormat::Markdown, true).unwrap();
        assert_eq!(found.typed, "plane tick");
        assert_eq!(found.heading.as_deref(), Some("go"));
    }

    /// TC-005-33 step 5: `xref:pla` in an AsciiDoc note keeps `xref:`.
    #[test]
    fn tc_005_33_step_5_asciidoc_xref_and_reference() {
        let text = "See xref:pla";
        let found = at_end(text, NoteFormat::AsciiDoc, false).unwrap();
        assert_eq!(found.trigger, Trigger::Xref);
        assert_eq!(found.typed, "pla");
        assert_eq!(&text[found.replace.clone()], "xref:pla");
        assert_eq!(
            write_link(&found, NoteFormat::AsciiDoc, "Plan.md", "Plan"),
            "xref:Plan.md[Plan]"
        );
        let text = "See <<pla";
        let found = at_end(text, NoteFormat::AsciiDoc, false).unwrap();
        assert_eq!(found.trigger, Trigger::Reference);
        assert_eq!(
            write_link(&found, NoteFormat::AsciiDoc, "Plan.adoc", "Plan"),
            "<<Plan.adoc,Plan>>"
        );
    }

    /// UC-005 A23: Ctrl+Space in the place of a written link takes the
    /// whole place and the text the link has.
    #[test]
    fn uc_005_a23_inside_a_written_link() {
        let text = "[the plan](Pl.md) after";
        let cursor = text.find(".md").unwrap();
        let found = link_context(text, cursor, NoteFormat::Markdown, false, true).unwrap();
        assert_eq!(found.typed, "Pl");
        assert_eq!(&text[found.replace.clone()], "[the plan](Pl.md)");
        assert_eq!(
            write_link(&found, NoteFormat::Markdown, "Plan.md", "Plan"),
            "[the plan](Plan.md)"
        );
        let text = "xref:Pl.adoc[Old] <<Gu.adoc,Guide>>";
        let cursor = text.find(".adoc[").unwrap();
        let found = link_context(text, cursor, NoteFormat::AsciiDoc, false, true).unwrap();
        assert_eq!(&text[found.replace.clone()], "xref:Pl.adoc[Old]");
        assert_eq!(found.label.as_deref(), Some("Old"));
        let cursor = text.find(".adoc,").unwrap();
        let found = link_context(text, cursor, NoteFormat::AsciiDoc, false, true).unwrap();
        assert_eq!(&text[found.replace.clone()], "<<Gu.adoc,Guide>>");
        assert_eq!(found.label.as_deref(), Some("Guide"));
    }

    /// UC-005 A24: no place of a link, so no completion: text, a closed
    /// link, a picture, a space in the place, `]]` after `[[`, `[[` in the
    /// source view, `](` in AsciiDoc.
    #[test]
    fn uc_005_a24_no_place_no_completion() {
        let md = NoteFormat::Markdown;
        assert!(at_end("plain text", md, false).is_none());
        assert!(at_end("[a](b) more", md, false).is_none());
        assert!(at_end("![cat](ca", md, false).is_none());
        assert!(at_end("[a](two words", md, false).is_none());
        assert!(at_end("\\[x](pl", md, false).is_none());
        assert!(at_end("[[plan]] x", md, true).is_none());
        assert!(at_end("[[plan", md, false).is_none());
        assert!(at_end("[a](pl", NoteFormat::AsciiDoc, false).is_none());
        assert!(at_end("axref:pl", NoteFormat::AsciiDoc, false).is_none());
        assert!(at_end("<<a,b>> x", NoteFormat::AsciiDoc, false).is_none());
        // Ctrl+Space in the source view outside a link: nothing.
        assert!(link_context("text", 4, md, false, true).is_none());
        // A cursor inside a character or past the end: nothing.
        assert!(link_context("é", 1, md, false, true).is_none());
        assert!(link_context("x", 5, md, false, true).is_none());
    }

    /// UC-005 A23: Ctrl+Space in the facade completes at the cursor.
    #[test]
    fn uc_005_a23_facade_at_the_cursor() {
        let found = link_context("ab", 1, NoteFormat::Markdown, true, true).unwrap();
        assert_eq!(found.trigger, Trigger::Here);
        assert_eq!(found.replace, 1..1);
        assert_eq!(found.typed, "");
    }

    /// UC-005 A23: in a code span no place of a link is typed; an escaped
    /// backtick opens none.
    #[test]
    fn uc_005_a23_code_spans() {
        let md = NoteFormat::Markdown;
        assert!(at_end("`[x](pl", md, false).is_none());
        assert!(at_end("`a` [x](pl", md, false).is_some());
        assert!(at_end("\\` [x](pl", md, false).is_some());
        assert!(at_end("`\\[\\[pl", md, true).is_none());
        assert!(at_end("\\`\\[\\[pl", md, true).is_some());
    }

    /// UC-005 BR-018: a title or heading with brackets is escaped in the
    /// link's text; a text the user typed is kept as it is.
    #[test]
    fn uc_005_br_018_brackets_in_labels() {
        let md = NoteFormat::Markdown;
        let found = at_end("[[pl", md, true).unwrap();
        assert_eq!(
            write_link(&found, md, "P.md", "Plan [v2]"),
            "[Plan \\[v2\\]](P.md)"
        );
        let found = at_end("[[pl", NoteFormat::AsciiDoc, true).unwrap();
        assert_eq!(
            write_link(&found, NoteFormat::AsciiDoc, "P.adoc", "Plan [v2]"),
            "link:P.adoc[Plan [v2\\]]"
        );
        let found = at_end("[a *b*](pl", md, false).unwrap();
        assert_eq!(write_link(&found, md, "P.md", "Plan"), "[a *b*](P.md)");
    }

    /// UC-005 A23: in a code block no place of a link is typed.
    #[test]
    fn uc_005_a23_code_blocks() {
        let md = NoteFormat::Markdown;
        assert!(in_code_block(["text\n", "```rust\n", "fn x() {}\n"], md));
        assert!(!in_code_block(["```\n", "x\n", "```\n"], md));
        assert!(in_code_block(["~~~~\n", "```\n"], md));
        assert!(!in_code_block(["    ```\n"], md));
        let adoc = NoteFormat::AsciiDoc;
        assert!(in_code_block(["----\n", "x\n"], adoc));
        assert!(!in_code_block(["----\n", "x\n", "----\n"], adoc));
        assert!(!in_code_block(["== Title\n"], adoc));
    }

    /// UC-005 BR-018: places relative to the note, as the toolbar writes
    /// them (`relative` of `underline-core`).
    #[test]
    fn uc_005_br_018_places() {
        assert_eq!(link_place("a/b/N.md", "a/c/T.md", None), "../c/T.md");
        assert_eq!(link_place("N.md", "T.adoc", None), "T.adoc");
        assert_eq!(link_place("a/N.md", "T.md", Some("x")), "../T.md#x");
        assert_eq!(link_place("a/N.md", "a/N.md", None), "N.md");
        assert_eq!(named_note("a/N.md", "./T.adoc").as_deref(), Some("a/T"));
        assert_eq!(named_note("N.md", "  "), None);
        assert_eq!(
            named_note("N.md", "notes.txt").as_deref(),
            Some("notes.txt")
        );
    }

    /// UC-005 A23 (chaos, seed logged): for any text and cursor the context
    /// does not panic, and what it replaces lies on character boundaries
    /// around the cursor; completing never yields an empty replacement text.
    #[test]
    fn uc_005_a23_chaos_context_ranges_are_sane() {
        const PIECES: [&str; 22] = [
            "[", "]", "(", ")", "`", "\\", "<<", ">>", "xref:", "[[", "]]", "!", "é", "😀", " ",
            "\n", "\r\n", ",", "#", "Plan.md", "\\[", "\\]",
        ];
        let mut seed: u64 = 0xD1B5_4A32_D192_ED03;
        eprintln!("seed {seed:#x}");
        for round in 0..20_000 {
            let mut text = String::new();
            for _ in 0..(round % 14 + 1) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                text.push_str(PIECES[(seed % PIECES.len() as u64) as usize]);
            }
            for cursor in 0..=text.len() + 1 {
                for (format, facade, manual) in [
                    (NoteFormat::Markdown, false, false),
                    (NoteFormat::AsciiDoc, false, false),
                    (NoteFormat::Markdown, true, true),
                    (NoteFormat::AsciiDoc, true, false),
                ] {
                    let Some(found) = link_context(&text, cursor, format, facade, manual) else {
                        continue;
                    };
                    let r = found.replace.clone();
                    assert!(
                        text.get(r.clone()).is_some()
                            && r.start <= cursor
                            && cursor <= r.end.max(cursor),
                        "round {round} cursor {cursor} {format:?}: {found:?} in {text:?}"
                    );
                    let written = write_link(&found, format, "Plan.md", "Plan");
                    assert!(!written.is_empty());
                }
            }
        }
    }
}
