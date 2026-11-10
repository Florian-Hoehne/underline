//! The clipboard's formats and the note formats (UC-005 A17, BR-011,
//! ADR-014): HTML from other programs becomes markup of the note format,
//! keeping what the format can hold, and a selection of the facade becomes
//! HTML, so other programs keep its formatting.

use scraper::{ElementRef, Html, Node};
use underline_core::NoteFormat;

// BR-011: moved to `underline-markup` (feature F6), so the editor's core in
// the webview makes the HTML of a copy; reachable here as before.
pub use underline_markup::html::blocks_to_html;

/// A block read from HTML, its inline content already as markup.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    Heading(usize, String),
    Paragraph(String),
    List {
        ordered: bool,
        items: Vec<Vec<Part>>,
    },
    Quote(Vec<Part>),
    Code(String),
    Table(Vec<Vec<String>>, bool),
    Rule,
}

/// UC-005 A17: `html` as markup of `format`: headings, paragraphs, bold,
/// italic, code, links, images, lists, quotes, code blocks, and tables are
/// kept; everything else becomes its plain text.
pub fn html_to_markup(html: &str, format: NoteFormat) -> String {
    let document = Html::parse_fragment(html);
    let mut parts = Vec::new();
    let mut pending = String::new();
    blocks(document.root_element(), format, &mut parts, &mut pending);
    flush(&mut pending, &mut parts);
    let mut out = String::new();
    write_parts(&parts, format, 0, &mut out);
    out.trim_end().to_owned()
}

const BLOCKS: [&str; 30] = [
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "blockquote",
    "pre",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "hr",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "nav",
    "aside",
    "figure",
    "body",
    "html",
    "dl",
];
const SKIPPED: [&str; 6] = ["script", "style", "head", "meta", "title", "template"];

fn is_block(name: &str) -> bool {
    BLOCKS.contains(&name) || name == "dt" || name == "dd" || name == "figcaption"
}

/// Turns pending inline markup into a paragraph.
fn flush(pending: &mut String, parts: &mut Vec<Part>) {
    let text = pending.trim();
    if !text.is_empty() {
        parts.push(Part::Paragraph(text.to_owned()));
    }
    pending.clear();
}

/// Reads the block children of `element` into `parts`; inline content
/// between them collects in `pending` and becomes a paragraph.
fn blocks(element: ElementRef, format: NoteFormat, parts: &mut Vec<Part>, pending: &mut String) {
    for child in element.children() {
        match child.value() {
            Node::Text(text) => pending.push_str(&collapse(text, format)),
            Node::Element(e) => {
                let Some(child) = ElementRef::wrap(child) else {
                    continue;
                };
                let name = e.name();
                if SKIPPED.contains(&name) {
                    continue;
                }
                if !is_block(name) {
                    pending.push_str(&inline(child, format));
                    continue;
                }
                flush(pending, parts);
                match name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = usize::from(name.as_bytes()[1] - b'0');
                        let text = inline_children(child, format);
                        if !text.trim().is_empty() {
                            parts.push(Part::Heading(level, text.trim().to_owned()));
                        }
                    }
                    "p" | "dt" | "dd" | "figcaption" => {
                        let text = inline_children(child, format);
                        if !text.trim().is_empty() {
                            parts.push(Part::Paragraph(text.trim().to_owned()));
                        }
                    }
                    "ul" | "ol" => parts.push(list(child, format, name == "ol")),
                    "blockquote" => {
                        let mut inner = Vec::new();
                        let mut text = String::new();
                        blocks(child, format, &mut inner, &mut text);
                        flush(&mut text, &mut inner);
                        if !inner.is_empty() {
                            parts.push(Part::Quote(inner));
                        }
                    }
                    "pre" => {
                        let code: String = child.text().collect();
                        parts.push(Part::Code(code.trim_end_matches('\n').to_owned()));
                    }
                    "table" => {
                        if let Some(table) = table(child, format) {
                            parts.push(table);
                        }
                    }
                    "hr" => parts.push(Part::Rule),
                    _ => {
                        blocks(child, format, parts, pending);
                        flush(pending, parts);
                    }
                }
            }
            _ => {}
        }
    }
}

fn list(element: ElementRef, format: NoteFormat, ordered: bool) -> Part {
    let mut items = Vec::new();
    for child in element.children().filter_map(ElementRef::wrap) {
        if child.value().name() != "li" {
            continue;
        }
        let mut inner = Vec::new();
        let mut text = String::new();
        blocks(child, format, &mut inner, &mut text);
        flush(&mut text, &mut inner);
        items.push(inner);
    }
    Part::List { ordered, items }
}

fn table(element: ElementRef, format: NoteFormat) -> Option<Part> {
    let mut rows = Vec::new();
    let mut header = false;
    for row in element
        .descendants()
        .filter_map(ElementRef::wrap)
        .filter(|e| e.value().name() == "tr")
    {
        let cells: Vec<String> = row
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|c| matches!(c.value().name(), "td" | "th"))
            .map(|c| {
                if rows.is_empty() && c.value().name() == "th" {
                    header = true;
                }
                inline_children(c, format).trim().replace('|', "\\|")
            })
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }
    (!rows.is_empty()).then_some(Part::Table(rows, header))
}

/// Whitespace as HTML shows it: every run of it one space; markup
/// characters of the note format escaped in Markdown.
fn collapse(text: &str, format: NoteFormat) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !space {
                out.push(' ');
            }
            space = true;
            continue;
        }
        space = false;
        if format == NoteFormat::Markdown && matches!(c, '*' | '_' | '`' | '[' | ']' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn inline_children(element: ElementRef, format: NoteFormat) -> String {
    let mut out = String::new();
    for child in element.children() {
        match child.value() {
            Node::Text(text) => out.push_str(&collapse(text, format)),
            Node::Element(_) => {
                if let Some(child) = ElementRef::wrap(child) {
                    out.push_str(&inline(child, format));
                }
            }
            _ => {}
        }
    }
    out
}

/// Puts `marker` around `text`, leaving its spaces outside, since neither
/// note format reads `** bold **` as bold.
fn wrap(text: &str, open: &str, close: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return text.to_owned();
    }
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[text.trim_end().len()..];
    format!("{lead}{open}{trimmed}{close}{trail}")
}

fn inline(element: ElementRef, format: NoteFormat) -> String {
    let md = format == NoteFormat::Markdown;
    let name = element.value().name();
    if SKIPPED.contains(&name) {
        return String::new();
    }
    let content = || inline_children(element, format);
    match name {
        "strong" | "b" => wrap(
            &content(),
            if md { "**" } else { "*" },
            if md { "**" } else { "*" },
        ),
        "em" | "i" => wrap(
            &content(),
            if md { "*" } else { "_" },
            if md { "*" } else { "_" },
        ),
        "s" | "del" | "strike" if md => wrap(&content(), "~~", "~~"),
        "code" | "kbd" | "samp" => {
            let code: String = element.text().collect();
            wrap(&code, "`", "`")
        }
        "a" => {
            let text = content();
            match element.value().attr("href") {
                Some(href) if !href.is_empty() && !href.starts_with("javascript:") => {
                    let label = if text.trim().is_empty() {
                        href
                    } else {
                        text.trim()
                    };
                    if md {
                        format!("[{label}]({href})")
                    } else if href.contains("://") {
                        format!("{href}[{label}]")
                    } else {
                        format!("link:{href}[{label}]")
                    }
                }
                _ => text,
            }
        }
        "img" => {
            let src = element.value().attr("src").unwrap_or("");
            let alt = element.value().attr("alt").unwrap_or("");
            if src.is_empty() || src.starts_with("data:") {
                alt.to_owned()
            } else if md {
                format!("![{alt}]({src})")
            } else {
                format!("image:{src}[{alt}]")
            }
        }
        "br" => {
            if md {
                "  \n".to_owned()
            } else {
                " +\n".to_owned()
            }
        }
        _ => content(),
    }
}

fn write_parts(parts: &[Part], format: NoteFormat, depth: usize, out: &mut String) {
    let md = format == NoteFormat::Markdown;
    for (n, part) in parts.iter().enumerate() {
        if n > 0 && !out.ends_with("\n\n") {
            out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
        }
        match part {
            Part::Heading(level, text) => {
                if md {
                    out.push_str(&format!("{} {text}\n", "#".repeat((*level).min(6))));
                } else {
                    out.push_str(&format!("{} {text}\n", "=".repeat((level + 1).min(6))));
                }
            }
            Part::Paragraph(text) => {
                out.push_str(text);
                out.push('\n');
            }
            Part::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    let marker = match (md, ordered) {
                        (true, false) => format!("{}- ", "  ".repeat(depth)),
                        (true, true) => format!("{}{}. ", "   ".repeat(depth), i + 1),
                        (false, false) => format!("{} ", "*".repeat(depth + 1)),
                        (false, true) => format!("{} ", ".".repeat(depth + 1)),
                    };
                    out.push_str(&marker);
                    let mut first = true;
                    for part in item {
                        match part {
                            Part::List { .. } => {
                                if !out.ends_with('\n') {
                                    out.push('\n');
                                }
                                let mut nested = String::new();
                                write_parts(
                                    std::slice::from_ref(part),
                                    format,
                                    depth + 1,
                                    &mut nested,
                                );
                                out.push_str(nested.trim_end());
                                out.push('\n');
                            }
                            Part::Paragraph(text) | Part::Heading(_, text) => {
                                if !first {
                                    out.push_str(if md { "\n" } else { "\n+\n" });
                                }
                                out.push_str(text);
                                out.push('\n');
                            }
                            other => {
                                let mut inner = String::new();
                                write_parts(std::slice::from_ref(other), format, 0, &mut inner);
                                out.push_str(inner.trim_end());
                                out.push('\n');
                            }
                        }
                        first = false;
                    }
                    if item.is_empty() {
                        out.push('\n');
                    }
                }
            }
            Part::Quote(inner) => {
                let mut text = String::new();
                write_parts(inner, format, 0, &mut text);
                if md {
                    for line in text.trim_end().lines() {
                        out.push_str(if line.is_empty() { ">" } else { "> " });
                        out.push_str(line);
                        out.push('\n');
                    }
                } else {
                    out.push_str("____\n");
                    out.push_str(text.trim_end());
                    out.push_str("\n____\n");
                }
            }
            Part::Code(code) => {
                let fence = if md { "```" } else { "----" };
                out.push_str(&format!("{fence}\n{code}\n{fence}\n"));
            }
            Part::Table(rows, header) => {
                let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
                let cell = |row: &Vec<String>, c: usize| row.get(c).cloned().unwrap_or_default();
                if md {
                    // GitHub needs a header row; without one it is empty.
                    let (head, body): (Vec<String>, &[Vec<String>]) = if *header {
                        (
                            (0..columns).map(|c| cell(&rows[0], c)).collect(),
                            &rows[1..],
                        )
                    } else {
                        (vec![String::new(); columns], &rows[..])
                    };
                    out.push_str(&format!("| {} |\n", head.join(" | ")));
                    out.push_str(&format!("|{}\n", " --- |".repeat(columns)));
                    for row in body {
                        let cells: Vec<String> = (0..columns).map(|c| cell(row, c)).collect();
                        out.push_str(&format!("| {} |\n", cells.join(" | ")));
                    }
                } else {
                    out.push_str("|===\n");
                    for (r, row) in rows.iter().enumerate() {
                        let cells: Vec<String> = (0..columns)
                            .map(|c| format!("| {}", cell(row, c)))
                            .collect();
                        out.push_str(&cells.join(" "));
                        out.push('\n');
                        if r == 0 && *header {
                            out.push('\n');
                        }
                    }
                    out.push_str("|===\n");
                }
            }
            Part::Rule => out.push_str(if md { "---\n" } else { "'''\n" }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{parse_blocks, BlockKind};

    /// What a web page puts on the clipboard (TC-005-26).
    const PAGE: &str = r##"<meta charset="utf-8"><h2 style="color:red">Install</h2>
<p>Use <b>bold</b> text, <span style="color:#c00">red text</span>, and <a href="https://x.org/a">a link</a>.</p>
<ul><li>one</li><li>two<ul><li>inner</li></ul></li></ul>
<table><thead><tr><th>Key</th><th>Value</th></tr></thead><tbody><tr><td>a</td><td><code>1</code></td></tr></tbody></table>
<script>alert(1)</script>"##;

    /// TC-005-26 step 1: kept and stored as Markdown; the color is dropped.
    #[test]
    fn uc005_tc26_html_to_markdown() {
        let markdown = html_to_markup(PAGE, NoteFormat::Markdown);
        assert_eq!(
            markdown,
            "## Install\n\nUse **bold** text, red text, and [a link](https://x.org/a).\n\n- one\n- two\n  - inner\n\n| Key | Value |\n| --- | --- |\n| a | `1` |"
        );
        // It reads back as the same structure.
        let blocks = parse_blocks(&markdown, NoteFormat::Markdown);
        assert!(matches!(blocks[0].kind, BlockKind::Heading(2)));
        assert!(matches!(blocks.last().unwrap().kind, BlockKind::Table(_)));
    }

    /// TC-005-26 step 2: the same stored as AsciiDoc.
    #[test]
    fn uc005_tc26_html_to_asciidoc() {
        let asciidoc = html_to_markup(PAGE, NoteFormat::AsciiDoc);
        assert_eq!(
            asciidoc,
            "=== Install\n\nUse *bold* text, red text, and https://x.org/a[a link].\n\n* one\n* two\n** inner\n\n|===\n| Key | Value\n\n| a | `1`\n|==="
        );
        let blocks = parse_blocks(&asciidoc, NoteFormat::AsciiDoc);
        assert!(matches!(blocks.last().unwrap().kind, BlockKind::Table(_)));
    }

    #[test]
    fn quotes_code_and_plain_characters() {
        let md = html_to_markup(
            "<blockquote><p>Said *so*</p></blockquote><pre><code>fn a() {}\n</code></pre><p>a<br>b</p>",
            NoteFormat::Markdown,
        );
        assert_eq!(md, "> Said \\*so\\*\n\n```\nfn a() {}\n```\n\na  \nb");
        // Only text: a paragraph of it.
        assert_eq!(
            html_to_markup("just <i>text</i>", NoteFormat::AsciiDoc),
            "just _text_"
        );
    }
}
