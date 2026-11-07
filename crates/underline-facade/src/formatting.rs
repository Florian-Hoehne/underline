//! The functions of the formatting toolbar (FR-023, UC-005 A2, BR-005): each
//! turns the selected text into the markup of the note's format, the same
//! function giving Markdown in a Markdown note and AsciiDoc in an AsciiDoc
//! note. Applying an inline format to text that has it takes it away again.

use std::ops::Range;

use underline_core::NoteFormat;

/// A function of the formatting toolbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Heading,
    Bold,
    Italic,
    Code,
    List,
    Quote,
    /// A link to `address`: a web address or the place of a note or file;
    /// `label` is its text when nothing is selected (the address otherwise).
    Link {
        address: String,
        label: Option<String>,
    },
    /// An image stored at `place`, relative to the note.
    Image(String),
    Table,
}

/// The change a toolbar function makes: replace `range` (bytes) by `text`,
/// then select `selection` (bytes, in the changed text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatEdit {
    pub range: Range<usize>,
    pub text: String,
    pub selection: Range<usize>,
}

/// UC-005 A2: the edit that applies `format` to `selection` (bytes) of
/// `source`, a note in `note_format`.
pub fn apply_format(
    source: &str,
    note_format: NoteFormat,
    selection: Range<usize>,
    format: &Format,
) -> FormatEdit {
    let markdown = note_format == NoteFormat::Markdown;
    match format {
        Format::Bold => wrap(source, selection, if markdown { "**" } else { "*" }),
        Format::Italic => wrap(source, selection, if markdown { "*" } else { "_" }),
        Format::Code => wrap(source, selection, "`"),
        Format::Heading => prefix_lines(source, selection, if markdown { "# " } else { "== " }),
        Format::List => prefix_lines(source, selection, if markdown { "- " } else { "* " }),
        Format::Quote => prefix_lines(source, selection, "> "),
        Format::Link { address, label } => {
            let selected = &source[selection.clone()];
            let label = if !selected.is_empty() {
                selected
            } else {
                label.as_deref().unwrap_or(address)
            };
            let text = if markdown {
                format!("[{label}]({address})")
            } else if address.contains("://") {
                format!("{address}[{label}]")
            } else {
                format!("link:{address}[{label}]")
            };
            insert(selection, text)
        }
        Format::Image(place) => {
            // An image on a paragraph of its own, so the facade shows it as
            // a picture (UC-005 A9 step 2, A18); the selection is its text.
            let alt = source[selection.clone()].trim();
            let text = if markdown {
                format!("![{alt}]({place})")
            } else {
                format!("image::{place}[{alt}]")
            };
            on_own_lines(source, selection, &text)
        }
        Format::Table => {
            let table = if markdown {
                "| Column 1 | Column 2 |\n| --- | --- |\n|  |  |\n|  |  |\n"
            } else {
                "|===\n| Column 1 | Column 2\n\n|\n|\n\n|\n|\n|===\n"
            };
            let start = selection.start;
            on_own_lines(source, start..start, table.trim_end())
        }
    }
}

/// Puts `block` in place of `range` on lines of its own, with an empty line
/// before and after it, as a block of its own; the cursor goes after it.
fn on_own_lines(source: &str, range: Range<usize>, block: &str) -> FormatEdit {
    let before = source[..range.start].trim_end_matches([' ', '\t']);
    let start = before.len();
    let after = source[range.end..].trim_start_matches([' ', '\t']);
    let end = source.len() - after.len();
    let lead = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    // After the block: the end of the note, an empty line, or one made.
    let trail = if after.is_empty() {
        "\n"
    } else if after == "\n" || after.starts_with("\n\n") {
        ""
    } else if after.starts_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let text = format!("{lead}{block}{trail}");
    let cursor = start + lead.len() + block.len();
    FormatEdit {
        range: start..end,
        text,
        selection: cursor..cursor,
    }
}

fn insert(range: Range<usize>, text: String) -> FormatEdit {
    let end = range.start + text.len();
    FormatEdit {
        selection: end..end,
        range,
        text,
    }
}

/// Wraps the selection in `marker`, or takes the marker away when the
/// selection is wrapped in it already. Without a selection, inserts the
/// markers and puts the cursor between them.
fn wrap(source: &str, selection: Range<usize>, marker: &str) -> FormatEdit {
    // Spaces at the edges of the selection stay outside the markers, since
    // Markdown and AsciiDoc do not read `** word**` as formatting.
    let selected = &source[selection.clone()];
    let start = selection.start + (selected.len() - selected.trim_start().len());
    let end = (selection.end - (selected.len() - selected.trim_end().len())).max(start);
    let selection = start..end;
    let m = marker.len();
    let wrapped = start >= m
        && source.get(start - m..start) == Some(marker)
        && source.get(end..end + m) == Some(marker)
        // `*` is not taken for the half of a `**`.
        && !(marker == "*" && (source[..start - m].ends_with('*') || source[end + m..].starts_with('*')));
    if wrapped && start < end {
        let inner = source[start..end].to_owned();
        let len = inner.len();
        return FormatEdit {
            range: start - m..end + m,
            text: inner,
            selection: start - m..start - m + len,
        };
    }
    let inner = &source[start..end];
    FormatEdit {
        range: selection.clone(),
        text: format!("{marker}{inner}{marker}"),
        selection: start + m..start + m + inner.len(),
    }
}

/// Puts `prefix` before every line the selection touches, or takes it away
/// when every one of them has it.
fn prefix_lines(source: &str, selection: Range<usize>, prefix: &str) -> FormatEdit {
    let start = source[..selection.start].rfind('\n').map_or(0, |i| i + 1);
    let end = source[selection.end..]
        .find('\n')
        .map_or(source.len(), |i| selection.end + i);
    let block = &source[start..end];
    let all_have = block.lines().all(|line| line.starts_with(prefix));
    let text: String = if all_have {
        block
            .split('\n')
            .map(|line| line.strip_prefix(prefix).unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        block
            .split('\n')
            .map(|line| format!("{prefix}{line}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let len = text.len();
    FormatEdit {
        range: start..end,
        text,
        selection: start + len..start + len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, selected: &str, format: Format, note_format: NoteFormat) -> String {
        let start = source.find(selected).unwrap();
        let edit = apply_format(source, note_format, start..start + selected.len(), &format);
        let mut out = source.to_owned();
        out.replace_range(edit.range, &edit.text);
        out
    }

    /// TC-005-09: each function gives the markup of the note's format.
    #[test]
    fn uc005_tc09_toolbar_functions_in_both_formats() {
        let md = NoteFormat::Markdown;
        let adoc = NoteFormat::AsciiDoc;
        assert_eq!(run("word\n", "word", Format::Heading, md), "# word\n");
        assert_eq!(run("word\n", "word", Format::Bold, md), "**word**\n");
        assert_eq!(run("word\n", "word", Format::Italic, md), "*word*\n");
        assert_eq!(run("word\n", "word", Format::Code, md), "`word`\n");
        assert_eq!(run("word\n", "word", Format::List, md), "- word\n");
        assert_eq!(run("word\n", "word", Format::Quote, md), "> word\n");
        assert_eq!(run("word\n", "word", Format::Heading, adoc), "== word\n");
        assert_eq!(run("word\n", "word", Format::Bold, adoc), "*word*\n");
        assert_eq!(run("word\n", "word", Format::Italic, adoc), "_word_\n");
        assert_eq!(run("word\n", "word", Format::List, adoc), "* word\n");
        assert_eq!(
            run(
                "see docs\n",
                "docs",
                Format::Link {
                    address: "Plan.md".into(),
                    label: None
                },
                md
            ),
            "see [docs](Plan.md)\n"
        );
        assert_eq!(
            run(
                "see docs\n",
                "docs",
                Format::Link {
                    address: "https://x.org".into(),
                    label: None
                },
                adoc
            ),
            "see https://x.org[docs]\n"
        );
        // An image goes on a paragraph of its own.
        assert_eq!(
            run("a pic\n", "pic", Format::Image("photo.png".into()), md),
            "a\n\n![pic](photo.png)\n"
        );
        assert_eq!(
            run("a pic b\n", "pic", Format::Image("photo.png".into()), adoc),
            "a\n\nimage::photo.png[pic]\n\nb\n"
        );
    }

    /// Applying an inline format twice takes it away; `*` is not taken from
    /// a `**`.
    #[test]
    fn inline_formats_toggle() {
        let md = NoteFormat::Markdown;
        let bold = "a **word** b\n";
        let start = bold.find("word").unwrap();
        let edit = apply_format(bold, md, start..start + 4, &Format::Bold);
        assert_eq!(edit.text, "word");
        assert_eq!(edit.range, start - 2..start + 6);
        let italic = apply_format(bold, md, start..start + 4, &Format::Italic);
        assert_eq!(italic.text, "*word*");
    }

    /// Spaces at the edges of the selection stay outside the markers; a link
    /// without a selection gets its label.
    #[test]
    fn spaces_stay_outside_markers() {
        assert_eq!(
            run(
                "this word is\n",
                " word ",
                Format::Bold,
                NoteFormat::Markdown
            ),
            "this **word** is\n"
        );
        let edit = apply_format(
            "ab",
            NoteFormat::Markdown,
            1..1,
            &Format::Link {
                address: "Plan.md".into(),
                label: Some("Plan".into()),
            },
        );
        assert_eq!(edit.text, "[Plan](Plan.md)");
    }

    /// Without a selection the markers go around the cursor.
    #[test]
    fn markers_around_the_cursor() {
        let edit = apply_format("ab", NoteFormat::Markdown, 1..1, &Format::Bold);
        assert_eq!(edit.text, "****");
        assert_eq!(edit.selection, 3..3);
    }

    /// Line formats apply to every selected line and are taken away again.
    #[test]
    fn line_formats_apply_to_every_line() {
        let source = "one\ntwo\nthree\n";
        let edit = apply_format(source, NoteFormat::Markdown, 1..6, &Format::List);
        assert_eq!(edit.range, 0..7);
        assert_eq!(edit.text, "- one\n- two");
        let listed = "- one\n- two\n";
        let back = apply_format(listed, NoteFormat::Markdown, 0..5, &Format::List);
        assert_eq!(back.text, "one");
    }

    /// TC-005-09 step 4: a table of 2 columns and 2 rows on lines of its own.
    #[test]
    fn uc005_tc09_inserts_a_table() {
        let out = run("text\n", "text", Format::Table, NoteFormat::Markdown);
        assert_eq!(
            out,
            "| Column 1 | Column 2 |\n| --- | --- |\n|  |  |\n|  |  |\n\ntext\n"
        );
        let blocks = crate::blocks::parse_blocks(&out, NoteFormat::Markdown);
        assert_eq!(blocks.last().unwrap().text(), "text");
        let edit = apply_format("text\n\n", NoteFormat::Markdown, 6..6, &Format::Table);
        assert_eq!(edit.text.lines().count(), 4);
    }
}
