//! A selection of the facade as HTML (UC-005 BR-011, ADR-014): copying in
//! the facade puts the formatted text on the clipboard next to the markup, so
//! other programs keep its formatting.
//!
//! Moved here from `underline_facade::clip` (feature F6 of the migration), so
//! the editor's core in the webview (ADR-025) makes the HTML of a copy;
//! `underline_facade::clip` re-exports it under its former path.

use std::ops::Range;

use crate::blocks::{Block, BlockKind, ListMarker, Run};

/// BR-011: the part `selection` (source bytes) of the facade's `blocks` as
/// HTML, with the formatting of the facade.
pub fn blocks_to_html(blocks: &[Block], selection: Range<usize>) -> String {
    let mut out = String::new();
    let mut open_list: Option<&'static str> = None;
    for block in blocks {
        if block.span.end <= selection.start || block.span.start >= selection.end {
            continue;
        }
        let whole = selection.start <= block.span.start && block.span.end <= selection.end;
        let runs = runs_html(&block.runs, &selection, whole);
        let list = match &block.kind {
            BlockKind::ListItem {
                marker: Some(ListMarker::Number(_)),
                ..
            } => Some("ol"),
            BlockKind::ListItem { .. } => Some("ul"),
            _ => None,
        };
        if open_list != list {
            if let Some(tag) = open_list {
                out.push_str(&format!("</{tag}>"));
            }
            if let Some(tag) = list {
                out.push_str(&format!("<{tag}>"));
            }
            open_list = list;
        }
        match &block.kind {
            BlockKind::Heading(level) => out.push_str(&format!("<h{level}>{runs}</h{level}>")),
            BlockKind::ListItem { .. } => out.push_str(&format!("<li>{runs}</li>")),
            BlockKind::Quote(_) => out.push_str(&format!("<blockquote><p>{runs}</p></blockquote>")),
            BlockKind::Admonition(label) => out.push_str(&format!(
                "<p><strong>{}:</strong> {runs}</p>",
                escape(label)
            )),
            BlockKind::Code { .. } | BlockKind::Markup => {
                out.push_str(&format!("<pre><code>{runs}</code></pre>"))
            }
            BlockKind::ThematicBreak => out.push_str("<hr>"),
            BlockKind::Image { target, alt, .. } => out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\">",
                escape(target),
                escape(alt)
            )),
            BlockKind::Table(table) => {
                out.push_str("<table>");
                for (r, row) in table.rows.iter().enumerate() {
                    out.push_str("<tr>");
                    let tag = if table.header && r == 0 { "th" } else { "td" };
                    for cell in row {
                        let all = 0..usize::MAX;
                        out.push_str(&format!("<{tag}>{}</{tag}>", runs_html(cell, &all, true)));
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</table>");
            }
            BlockKind::Paragraph => out.push_str(&format!("<p>{runs}</p>")),
        }
    }
    if let Some(tag) = open_list {
        out.push_str(&format!("</{tag}>"));
    }
    out
}

/// The characters of `runs` within `selection` as HTML in their styles.
fn runs_html(runs: &[Run], selection: &Range<usize>, whole: bool) -> String {
    let mut out = String::new();
    for run in runs {
        let text: String = run
            .text
            .chars()
            .zip(&run.offsets)
            .filter(|(_, at)| whole || selection.contains(at))
            .map(|(c, _)| c)
            .collect();
        if text.is_empty() {
            continue;
        }
        let mut html = escape(&text);
        let style = &run.style;
        if style.code {
            html = format!("<code>{html}</code>");
        }
        if style.bold {
            html = format!("<strong>{html}</strong>");
        }
        if style.italic {
            html = format!("<em>{html}</em>");
        }
        if style.strike {
            html = format!("<s>{html}</s>");
        }
        if let Some(link) = &style.link {
            html = format!("<a href=\"{}\">{html}</a>", escape(link));
        }
        out.push_str(&html);
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::parse_blocks;
    use crate::NoteFormat;

    /// BR-011: a selection of the facade as HTML, with its formatting.
    #[test]
    fn uc005_br011_selection_as_html() {
        let note = "# Plan\n\nUse **bold** here and [x](https://x.org).\n\n- one\n- two\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);
        let all = blocks_to_html(&blocks, 0..note.len());
        assert_eq!(
            all,
            "<h1>Plan</h1><p>Use <strong>bold</strong> here and <a href=\"https://x.org\">x</a>.</p><ul><li>one</li><li>two</li></ul>"
        );
        // Part of a paragraph.
        let start = note.find("Use").unwrap();
        let end = note.find(" here").unwrap() + 5;
        assert_eq!(
            blocks_to_html(&blocks, start..end),
            "<p>Use <strong>bold</strong> here</p>"
        );
    }
}
