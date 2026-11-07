//! UC-009 Export Note (ADR-015): a note's blocks written as the other note
//! format, as one HTML file, or as a PDF set by Typst. Every export starts
//! from the facade's blocks, so the three agree on what a note holds.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use underline_core::{
    asciidoc_anchor, heading_anchor, relative, resolve_link, LinkTarget, NoteFormat,
};

use crate::blocks::{parse_blocks, Block, BlockKind, CellAlign, ListMarker, Run, Table};
use crate::diagram::{renderer, DiagramLanguage};
use crate::highlight::{highlight_code, HighlightSet};
use crate::images::{self, ImageSource};
use underline_core::Theme;

/// What a note is exported to (UC-009 step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// The other note format (FR-014).
    OtherFormat,
    Html,
    Pdf,
}

impl ExportKind {
    pub const ALL: [ExportKind; 3] = [Self::OtherFormat, Self::Html, Self::Pdf];

    /// The extension of the exported file of a note in `format` (BR-007).
    pub fn extension(self, format: NoteFormat) -> &'static str {
        match self {
            Self::OtherFormat => other(format).extension(),
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }

    /// The name the user chooses it by.
    pub fn label(self, format: NoteFormat) -> &'static str {
        match self {
            Self::OtherFormat => other(format).name(),
            Self::Html => "HTML",
            Self::Pdf => "PDF",
        }
    }
}

/// The other note format.
fn other(format: NoteFormat) -> NoteFormat {
    match format {
        NoteFormat::Markdown => NoteFormat::AsciiDoc,
        NoteFormat::AsciiDoc => NoteFormat::Markdown,
    }
}

/// The paper of a PDF (BR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    Letter,
}

impl Paper {
    /// The paper of the system's region: US Letter in the United States and
    /// Canada, A4 elsewhere.
    pub fn of_region(locale: &str) -> Self {
        let region = locale
            .split(['.', '@'])
            .next()
            .unwrap_or_default()
            .rsplit(['_', '-'])
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(region.as_str(), "US" | "CA" | "MX" | "PH") {
            Self::Letter
        } else {
            Self::A4
        }
    }

    fn typst(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::Letter => "us-letter",
        }
    }
}

/// One export.
#[derive(Debug, Clone)]
pub struct ExportRequest {
    /// The note's text as its tab shows it (BR-001).
    pub text: String,
    pub format: NoteFormat,
    pub title: String,
    /// The workspace and the note's file in it.
    pub root: PathBuf,
    pub note: PathBuf,
    /// Where the export goes.
    pub target: PathBuf,
    pub paper: Paper,
}

/// An export ready to be written.
#[derive(Debug, Clone, Default)]
pub struct Exported {
    pub bytes: Vec<u8>,
    /// Lines of the note whose blocks were only approximated (A2).
    pub approximated: Vec<usize>,
    /// Diagrams and images that could not be shown (A3).
    pub missing: Vec<String>,
}

/// Why an export could not be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    #[error("the PDF could not be set: {0}")]
    Pdf(String),
}

/// UC-009 step 4: the export of the note.
pub fn export(request: &ExportRequest, kind: ExportKind) -> Result<Exported, ExportError> {
    let blocks = parse_blocks(&request.text, request.format);
    match kind {
        ExportKind::OtherFormat => Ok(MarkupWriter::new(request).write(&blocks)),
        ExportKind::Html => Ok(HtmlWriter::new(request).write(&blocks)),
        ExportKind::Pdf => pdf(request, &blocks),
    }
}

/// The line of `text` at byte `offset`, from 1.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].matches('\n').count() + 1
}

/// Whether a link leaves the note for the web.
fn is_web(link: &str) -> bool {
    link.contains("://") || link.starts_with("mailto:")
}

// ---------------------------------------------------------------- markup

/// BR-002: the note in the markup of the other note format.
struct MarkupWriter<'a> {
    request: &'a ExportRequest,
    to: NoteFormat,
    out: String,
    approximated: Vec<usize>,
}

impl<'a> MarkupWriter<'a> {
    fn new(request: &'a ExportRequest) -> MarkupWriter<'a> {
        MarkupWriter {
            request,
            to: other(request.format),
            out: String::new(),
            approximated: Vec::new(),
        }
    }

    fn approximate(&mut self, block: &Block) {
        let line = line_of(&self.request.text, block.span.start);
        if !self.approximated.contains(&line) {
            self.approximated.push(line);
        }
    }

    /// A relative place of the note, as seen from the export (BR-002).
    fn relink(&self, place: &str) -> String {
        if is_web(place) || place.starts_with('#') || Path::new(place).is_absolute() {
            return place.to_owned();
        }
        let (path, heading) = match place.split_once('#') {
            Some((path, heading)) => (path, Some(heading)),
            None => (place, None),
        };
        let request = self.request;
        let from = request.note.parent().unwrap_or(&request.root);
        let Some(absolute) = normalize(&from.join(path)) else {
            return place.to_owned();
        };
        let to = request.target.parent().unwrap_or(Path::new("."));
        let mut moved = relative(to, &absolute);
        if let Some(heading) = heading {
            moved.push('#');
            moved.push_str(heading);
        }
        moved
    }

    fn write(mut self, blocks: &[Block]) -> Exported {
        let mut previous: Option<&BlockKind> = None;
        for block in blocks {
            let tight = matches!(
                (previous, &block.kind),
                (
                    Some(BlockKind::ListItem { .. }),
                    BlockKind::ListItem {
                        marker: Some(_),
                        ..
                    }
                )
            );
            if !self.out.is_empty() {
                self.out.push_str(if tight { "\n" } else { "\n\n" });
            }
            self.block(block);
            previous = Some(&block.kind);
        }
        self.out.push('\n');
        Exported {
            bytes: self.out.into_bytes(),
            approximated: self.approximated,
            missing: Vec::new(),
        }
    }

    fn block(&mut self, block: &Block) {
        let md = self.to == NoteFormat::Markdown;
        match &block.kind {
            BlockKind::Heading(level) => {
                let marker = if md { "#" } else { "=" };
                let text = self.inline(&block.runs);
                let _ = write!(self.out, "{} {text}", marker.repeat(usize::from(*level)));
            }
            BlockKind::Paragraph => {
                let text = self.inline(&block.runs);
                self.out.push_str(&text);
            }
            BlockKind::ListItem {
                depth,
                marker,
                checked,
            } => {
                let text = self.inline(&block.runs);
                let check = match checked {
                    Some(true) => "[x] ",
                    Some(false) => "[ ] ",
                    None => "",
                };
                match (md, marker) {
                    (true, Some(ListMarker::Bullet)) => {
                        let _ = write!(self.out, "{}- {check}{text}", "  ".repeat(*depth));
                    }
                    (true, Some(ListMarker::Number(n))) => {
                        let _ = write!(self.out, "{}{n}. {check}{text}", "   ".repeat(*depth));
                    }
                    (true, None) => {
                        let _ = write!(self.out, "{}{text}", "  ".repeat(depth + 1));
                    }
                    (false, Some(ListMarker::Bullet)) => {
                        let _ = write!(self.out, "{} {check}{text}", "*".repeat(depth + 1));
                    }
                    (false, Some(ListMarker::Number(_))) => {
                        let _ = write!(self.out, "{} {check}{text}", ".".repeat(depth + 1));
                    }
                    (false, None) => {
                        // A further paragraph of an item is attached with `+`.
                        if self.out.ends_with("\n\n") {
                            self.out.truncate(self.out.len() - 1);
                        }
                        let _ = write!(self.out, "+\n{text}");
                    }
                }
            }
            BlockKind::Quote(depth) => {
                let text = self.inline(&block.runs);
                if md {
                    let _ = write!(self.out, "{} {text}", ">".repeat(*depth));
                } else {
                    if *depth > 1 {
                        self.approximate(block);
                    }
                    let _ = write!(self.out, "____\n{text}\n____");
                }
            }
            BlockKind::Code { language } => {
                let code = block.text();
                let code = code.trim_end_matches('\n');
                if md {
                    let fence = if code.contains("```") { "````" } else { "```" };
                    let _ = write!(self.out, "{fence}{language}\n{code}\n{fence}");
                } else if DiagramLanguage::from_code_language(language).is_some() {
                    let _ = write!(
                        self.out,
                        "[{}]\n----\n{code}\n----",
                        language.to_lowercase()
                    );
                } else if language.is_empty() {
                    let _ = write!(self.out, "----\n{code}\n----");
                } else {
                    let _ = write!(self.out, "[source,{language}]\n----\n{code}\n----");
                }
            }
            BlockKind::ThematicBreak => self.out.push_str(if md { "---" } else { "'''" }),
            BlockKind::Admonition(label) => {
                let text = self.inline(&block.runs);
                if md {
                    // No counterpart in Markdown: a quote led by the label.
                    self.approximate(block);
                    let label = title_case(label);
                    let _ = write!(self.out, "> **{label}:** {text}");
                } else {
                    let _ = write!(self.out, "{}: {text}", label.to_uppercase());
                }
            }
            BlockKind::Markup => {
                // Markup of the source format the facade shows as it is.
                self.approximate(block);
                let text = block.text();
                if md {
                    let _ = write!(self.out, "```\n{text}\n```");
                } else {
                    let _ = write!(self.out, "....\n{text}\n....");
                }
            }
            BlockKind::Table(table) => self.table(block, table),
            BlockKind::Image {
                target,
                alt,
                caption,
            } => {
                let target = self.relink(target);
                if md {
                    let _ = write!(self.out, "![{alt}]({target})");
                    if let Some(caption) = caption {
                        self.approximate(block);
                        let _ = write!(self.out, "\n\n*{caption}*");
                    }
                } else {
                    if let Some(caption) = caption {
                        let _ = writeln!(self.out, ".{caption}");
                    }
                    let _ = write!(self.out, "image::{target}[{alt}]");
                }
            }
        }
    }

    fn table(&mut self, block: &Block, table: &Table) {
        let columns = table.columns().max(1);
        let cell = |writer: &Self, row: &[Vec<Run>], i: usize| {
            row.get(i)
                .map(|runs| writer.inline(runs).replace('|', "\\|"))
                .unwrap_or_default()
        };
        if self.to == NoteFormat::Markdown {
            if let Some(caption) = &table.caption {
                self.approximate(block);
                let _ = write!(self.out, "*{caption}*\n\n");
            }
            if !table.header {
                self.approximate(block);
            }
            let rows: Vec<Vec<String>> = table
                .rows
                .iter()
                .map(|row| (0..columns).map(|i| cell(self, row, i)).collect())
                .collect();
            let mut lines = Vec::new();
            let empty = vec![String::new(); columns];
            let (head, body) = if table.header {
                (rows.first().unwrap_or(&empty), rows.get(1..).unwrap_or(&[]))
            } else {
                (&empty, &rows[..])
            };
            lines.push(format!("| {} |", head.join(" | ")));
            let rule: Vec<&str> = (0..columns)
                .map(|i| match table.align.get(i) {
                    Some(CellAlign::Center) => ":---:",
                    Some(CellAlign::Right) => "---:",
                    _ => "---",
                })
                .collect();
            lines.push(format!("|{}|", rule.join("|")));
            for row in body {
                lines.push(format!("| {} |", row.join(" | ")));
            }
            self.out.push_str(&lines.join("\n"));
        } else {
            if let Some(caption) = &table.caption {
                let _ = writeln!(self.out, ".{caption}");
            }
            let cols: Vec<&str> = (0..columns)
                .map(|i| match table.align.get(i) {
                    Some(CellAlign::Center) => "^",
                    Some(CellAlign::Right) => ">",
                    _ => "<",
                })
                .collect();
            let options = if table.header { "%header," } else { "" };
            let _ = writeln!(self.out, "[{options}cols=\"{}\"]", cols.join(","));
            self.out.push_str("|===");
            for row in &table.rows {
                self.out.push('\n');
                let cells: Vec<String> = (0..columns)
                    .map(|i| format!("| {}", cell(self, row, i)))
                    .collect();
                self.out.push_str(&cells.join(" "));
            }
            self.out.push_str("\n|===");
        }
    }

    /// The runs in the markup of the other format, links following the
    /// export's place.
    fn inline(&self, runs: &[Run]) -> String {
        let md = self.to == NoteFormat::Markdown;
        let mut out = String::new();
        let mut i = 0;
        while i < runs.len() {
            let link = runs[i].style.link.clone();
            let mut j = i;
            let mut inner = String::new();
            while j < runs.len() && runs[j].style.link == link {
                inner.push_str(&self.styled(&runs[j], out.is_empty() && inner.is_empty()));
                j += 1;
            }
            match link {
                None => out.push_str(&inner),
                Some(place) => {
                    let place = self.relink(&place);
                    if md {
                        let _ = write!(out, "[{inner}]({place})");
                    } else if is_web(&place) {
                        let _ = write!(out, "{place}[{inner}]");
                    } else if is_note_place(&place) || place.starts_with('#') {
                        let _ = write!(out, "xref:{place}[{inner}]");
                    } else {
                        let _ = write!(out, "link:{place}[{inner}]");
                    }
                }
            }
            i = j;
        }
        out
    }

    fn styled(&self, run: &Run, at_start: bool) -> String {
        let style = &run.style;
        if self.to == NoteFormat::Markdown {
            if style.code {
                let fence = if run.text.contains('`') { "``" } else { "`" };
                return format!("{fence}{}{fence}", run.text);
            }
            let mut text = escape_markdown(&run.text, at_start);
            if style.bold {
                text = format!("**{text}**");
            }
            if style.italic {
                text = format!("*{text}*");
            }
            if style.strike {
                text = format!("~~{text}~~");
            }
            text
        } else {
            if style.code {
                return format!("`+{}+`", run.text);
            }
            let mut text = run.text.clone();
            if style.bold {
                text = format!("**{text}**");
            }
            if style.italic {
                text = format!("__{text}__");
            }
            if style.strike {
                text = format!("[.line-through]#{text}#");
            }
            text
        }
    }
}

/// Whether a place names a note file.
fn is_note_place(place: &str) -> bool {
    let path = place.split('#').next().unwrap_or_default();
    path.rsplit_once('.')
        .and_then(|(_, e)| NoteFormat::from_extension(e))
        .is_some()
}

fn title_case(label: &str) -> String {
    let lower = label.to_lowercase();
    let mut chars = lower.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Escapes characters that Markdown would read as formatting.
fn escape_markdown(text: &str, at_start: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.chars().enumerate() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '~')
            || (at_start && i == 0 && matches!(c, '#' | '-' | '+' | '|'))
        {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `path` without `.` and `..`.
fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

// ---------------------------------------------------------------- assets

/// An image the export holds.
struct Asset {
    bytes: Vec<u8>,
    mime: &'static str,
    extension: &'static str,
}

/// The bytes of the image at `target` of the note (BR-006: web images load
/// as when reading the note).
fn image_asset(request: &ExportRequest, target: &str) -> Result<Asset, String> {
    let source = match resolve_link(&request.root, &request.note, target) {
        LinkTarget::Web(address) => ImageSource::Web(address),
        LinkTarget::File(path) => ImageSource::File(path),
        LinkTarget::Note { id, .. } => ImageSource::File(request.root.join(id.as_str())),
        _ => ImageSource::File(request.note.parent().unwrap_or(&request.root).join(target)),
    };
    let bytes = images::load(&source).map_err(|e| e.to_string())?;
    let name = target
        .split(['?', '#'])
        .next()
        .unwrap_or(target)
        .to_lowercase();
    let (mime, extension) = if bytes.starts_with(b"\x89PNG") {
        ("image/png", "png")
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        ("image/jpeg", "jpg")
    } else if bytes.starts_with(b"GIF8") {
        ("image/gif", "gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        ("image/webp", "webp")
    } else if name.ends_with(".svg") || bytes.windows(4).take(512).any(|w| w == b"<svg") {
        ("image/svg+xml", "svg")
    } else {
        return Err(format!("{target} is no image"));
    };
    Ok(Asset {
        bytes,
        mime,
        extension,
    })
}

/// The diagram of a code block as SVG in light colors, or why not.
fn diagram_svg(language: &str, source: &str) -> Option<Result<String, String>> {
    let language = DiagramLanguage::from_code_language(language)?;
    Some(
        renderer(language)
            .render(source, false)
            .map_err(|e| e.to_string()),
    )
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(TABLE[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

// ---------------------------------------------------------------- HTML

const STYLE: &str = r#"
:root { color-scheme: light dark; --text: #1f2328; --weak: #59636e; --bg: #ffffff; --code: #f6f8fa; --rule: #d1d9e0; --link: #0969da; --note: #0969da; }
@media (prefers-color-scheme: dark) { :root { --text: #e6edf3; --weak: #8d96a0; --bg: #0d1117; --code: #161b22; --rule: #3d444d; --link: #4493f8; --note: #4493f8; } pre span { color: var(--d) !important; } }
body { margin: 0; background: var(--bg); color: var(--text); font: 16px/1.5 -apple-system, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif; }
article { max-width: 880px; margin: 0 auto; padding: 32px 24px; }
a { color: var(--link); }
h1, h2 { border-bottom: 1px solid var(--rule); padding-bottom: .3em; }
pre, code { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 85%; }
code { background: var(--code); padding: .2em .4em; border-radius: 6px; }
pre { background: var(--code); padding: 16px; border-radius: 6px; overflow: auto; }
pre code { background: none; padding: 0; font-size: 100%; }
pre span { color: var(--l); }
blockquote { margin: 0; padding: 0 1em; color: var(--weak); border-left: .25em solid var(--rule); }
table { border-collapse: collapse; margin: 1em 0; }
th, td { border: 1px solid var(--rule); padding: 6px 13px; }
th { background: var(--code); }
img, svg { max-width: 100%; height: auto; }
figure { margin: 1em 0; }
figure.diagram { display: inline-block; background: #ffffff; padding: 8px; border-radius: 6px; }
figcaption, caption { color: var(--weak); font-size: 90%; }
.admonition { border-left: .25em solid var(--note); padding: .5em 1em; margin: 1em 0; }
.missing { color: var(--weak); font-style: italic; }
ul.checks { list-style: none; padding-left: 1.2em; }
"#;

/// BR-003: one HTML file that holds the note's images and diagrams.
struct HtmlWriter<'a> {
    request: &'a ExportRequest,
    out: String,
    missing: Vec<String>,
    /// The open lists: their depth and tag.
    lists: Vec<(usize, &'static str)>,
    light: HighlightSet,
    dark: HighlightSet,
}

impl<'a> HtmlWriter<'a> {
    fn new(request: &'a ExportRequest) -> Self {
        Self {
            request,
            out: String::new(),
            missing: Vec::new(),
            lists: Vec::new(),
            light: HighlightSet::for_theme(Theme::GitHubLight),
            dark: HighlightSet::for_theme(Theme::GitHubDark),
        }
    }

    fn write(mut self, blocks: &[Block]) -> Exported {
        for block in blocks {
            self.block(block);
        }
        self.close_lists(0);
        let page = format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<article>\n{}</article>\n</body>\n</html>\n",
            escape_html(&self.request.title),
            self.out
        );
        Exported {
            bytes: page.into_bytes(),
            approximated: Vec::new(),
            missing: self.missing,
        }
    }

    fn close_lists(&mut self, depth: usize) {
        while self.lists.last().is_some_and(|(d, _)| *d >= depth) {
            if let Some((_, tag)) = self.lists.pop() {
                let _ = writeln!(self.out, "</li></{tag}>");
            }
        }
    }

    fn anchor(&self, text: &str) -> String {
        match self.request.format {
            NoteFormat::Markdown => heading_anchor(text),
            NoteFormat::AsciiDoc => asciidoc_anchor(text),
        }
    }

    fn block(&mut self, block: &Block) {
        if !matches!(block.kind, BlockKind::ListItem { .. }) {
            self.close_lists(0);
        }
        match &block.kind {
            BlockKind::Heading(level) => {
                let level = (*level).clamp(1, 6);
                let id = self.anchor(&block.text());
                let text = self.inline(&block.runs);
                let _ = writeln!(
                    self.out,
                    "<h{level} id=\"{}\">{text}</h{level}>",
                    escape_html(&id)
                );
            }
            BlockKind::Paragraph => {
                let text = self.inline(&block.runs);
                let _ = writeln!(self.out, "<p>{text}</p>");
            }
            BlockKind::ListItem {
                depth,
                marker,
                checked,
            } => {
                let text = self.inline(&block.runs);
                let Some(marker) = marker else {
                    let _ = writeln!(self.out, "<p>{text}</p>");
                    return;
                };
                let tag = match marker {
                    ListMarker::Bullet => "ul",
                    ListMarker::Number(_) => "ol",
                };
                self.close_lists(depth + 1);
                match self.lists.last() {
                    Some((d, t)) if d == depth && *t == tag => {
                        self.out.push_str("</li>\n");
                    }
                    Some((d, _)) if d == depth => {
                        self.close_lists(*depth);
                        self.open_list(*depth, tag, marker, checked.is_some());
                    }
                    _ => self.open_list(*depth, tag, marker, checked.is_some()),
                }
                let check = match checked {
                    Some(true) => "<input type=\"checkbox\" checked disabled> ",
                    Some(false) => "<input type=\"checkbox\" disabled> ",
                    None => "",
                };
                let _ = write!(self.out, "<li>{check}{text}");
            }
            BlockKind::Quote(depth) => {
                let text = self.inline(&block.runs);
                let _ = writeln!(
                    self.out,
                    "{}<p>{text}</p>{}",
                    "<blockquote>".repeat(*depth),
                    "</blockquote>".repeat(*depth)
                );
            }
            BlockKind::Code { language } => {
                let code = block.text();
                match diagram_svg(language, &code) {
                    Some(Ok(svg)) => {
                        let svg = svg.trim_start_matches(|c| c != '<');
                        let svg = match svg.strip_prefix("<?xml") {
                            Some(rest) => rest.split_once("?>").map_or(svg, |(_, s)| s),
                            None => svg,
                        };
                        let _ = writeln!(self.out, "<figure class=\"diagram\">{svg}</figure>");
                    }
                    Some(Err(error)) => {
                        self.missing.push(format!("a {language} diagram ({error})"));
                        let _ =
                            writeln!(self.out, "<pre><code>{}</code></pre>", escape_html(&code));
                    }
                    None => {
                        let colored = self.code(language, &code);
                        let _ = writeln!(self.out, "<pre><code>{colored}</code></pre>");
                    }
                }
            }
            BlockKind::ThematicBreak => self.out.push_str("<hr>\n"),
            BlockKind::Admonition(label) => {
                let text = self.inline(&block.runs);
                let _ = writeln!(
                    self.out,
                    "<div class=\"admonition\"><strong>{}</strong><br>{text}</div>",
                    escape_html(&title_case(label))
                );
            }
            BlockKind::Markup => {
                let _ = writeln!(self.out, "<pre>{}</pre>", escape_html(&block.text()));
            }
            BlockKind::Table(table) => self.table(table),
            BlockKind::Image {
                target,
                alt,
                caption,
            } => {
                let image = match image_asset(self.request, target) {
                    Ok(asset) => format!(
                        "<img src=\"data:{};base64,{}\" alt=\"{}\">",
                        asset.mime,
                        base64(&asset.bytes),
                        escape_html(alt)
                    ),
                    Err(error) => {
                        self.missing.push(format!("the image {target} ({error})"));
                        format!("<span class=\"missing\">{}</span>", escape_html(alt))
                    }
                };
                let caption = caption
                    .as_ref()
                    .map(|c| format!("<figcaption>{}</figcaption>", escape_html(c)))
                    .unwrap_or_default();
                let _ = writeln!(self.out, "<figure>{image}{caption}</figure>");
            }
        }
    }

    fn open_list(&mut self, depth: usize, tag: &'static str, marker: &ListMarker, checks: bool) {
        let start = match marker {
            ListMarker::Number(n) if *n != 1 => format!(" start=\"{n}\""),
            _ => String::new(),
        };
        let class = if checks { " class=\"checks\"" } else { "" };
        let _ = writeln!(self.out, "<{tag}{start}{class}>");
        self.lists.push((depth, tag));
    }

    fn table(&mut self, table: &Table) {
        self.out.push_str("<table>\n");
        if let Some(caption) = &table.caption {
            let _ = writeln!(self.out, "<caption>{}</caption>", escape_html(caption));
        }
        for (r, row) in table.rows.iter().enumerate() {
            let tag = if table.header && r == 0 { "th" } else { "td" };
            self.out.push_str("<tr>");
            for (c, cell) in row.iter().enumerate() {
                let align = match table.align.get(c) {
                    Some(CellAlign::Center) => " style=\"text-align:center\"",
                    Some(CellAlign::Right) => " style=\"text-align:right\"",
                    _ => "",
                };
                let text = self.inline(cell);
                let _ = write!(self.out, "<{tag}{align}>{text}</{tag}>");
            }
            self.out.push_str("</tr>\n");
        }
        self.out.push_str("</table>\n");
    }

    /// Code colored as in Underline (ADR-010), light and dark.
    fn code(&self, language: &str, code: &str) -> String {
        let light = highlight_code(&self.light, language, code);
        let dark = highlight_code(&self.dark, language, code);
        let mut out = String::new();
        for (n, line) in code.split_inclusive('\n').enumerate() {
            let (l, d) = (light.get(n), dark.get(n));
            if l.is_none_or(Vec::is_empty) {
                out.push_str(&escape_html(line));
                continue;
            }
            let mut cuts: Vec<usize> = l
                .into_iter()
                .chain(d)
                .flatten()
                .flat_map(|s| [s.start, s.end])
                .chain([0, line.len()])
                .filter(|&at| at <= line.len() && line.is_char_boundary(at))
                .collect();
            cuts.sort_unstable();
            cuts.dedup();
            let color_at = |spans: Option<&Vec<crate::highlight::Span>>, at: usize| {
                spans
                    .and_then(|s| s.iter().find(|s| s.start <= at && at < s.end))
                    .map(|s| format!("#{:02x}{:02x}{:02x}", s.color[0], s.color[1], s.color[2]))
            };
            for pair in cuts.windows(2) {
                let piece = escape_html(&line[pair[0]..pair[1]]);
                match (color_at(l, pair[0]), color_at(d, pair[0])) {
                    (Some(light), dark) => {
                        let dark = dark.unwrap_or_else(|| light.clone());
                        let _ =
                            write!(out, "<span style=\"--l:{light};--d:{dark}\">{piece}</span>");
                    }
                    _ => out.push_str(&piece),
                }
            }
        }
        out
    }

    /// BR-005: web links and links to headings stay links; links to notes
    /// and files of the workspace keep their text only.
    fn inline(&self, runs: &[Run]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < runs.len() {
            let link = runs[i].style.link.clone();
            let mut j = i;
            let mut inner = String::new();
            while j < runs.len() && runs[j].style.link == link {
                inner.push_str(&styled_html(&runs[j]));
                j += 1;
            }
            match link {
                Some(place) if is_web(&place) || place.starts_with('#') => {
                    let _ = write!(out, "<a href=\"{}\">{inner}</a>", escape_html(&place));
                }
                _ => out.push_str(&inner),
            }
            i = j;
        }
        out
    }
}

fn styled_html(run: &Run) -> String {
    let mut text = escape_html(&run.text);
    let style = &run.style;
    if style.code {
        text = format!("<code>{text}</code>");
    }
    if style.bold {
        text = format!("<strong>{text}</strong>");
    }
    if style.italic {
        text = format!("<em>{text}</em>");
    }
    if style.strike {
        text = format!("<del>{text}</del>");
    }
    text
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------- PDF

/// BR-004, BR-009: the note set by Typst into a PDF.
fn pdf(request: &ExportRequest, blocks: &[Block]) -> Result<Exported, ExportError> {
    let mut writer = TypstWriter {
        request,
        out: String::new(),
        files: Vec::new(),
        missing: Vec::new(),
    };
    writer.preamble();
    for block in blocks {
        writer.block(block);
        writer.out.push_str("\n\n");
    }
    let bytes = typeset(&writer.out, writer.files).map_err(ExportError::Pdf)?;
    Ok(Exported {
        bytes,
        approximated: Vec::new(),
        missing: writer.missing,
    })
}

struct TypstWriter<'a> {
    request: &'a ExportRequest,
    out: String,
    /// The images and diagrams the document reads, by name.
    files: Vec<(String, Vec<u8>)>,
    missing: Vec<String>,
}

/// A Typst string literal.
fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl TypstWriter<'_> {
    fn preamble(&mut self) {
        let _ = write!(
            self.out,
            r##"#set document(title: {title})
#set page(paper: "{paper}", margin: 2.5cm, numbering: "1")
#set text(size: 11pt)
#set par(justify: false)
#show heading: set block(sticky: true)
#show link: set text(fill: rgb("#0969da"))
#show raw.where(block: true): block.with(fill: luma(246), inset: 8pt, radius: 4pt, width: 100%)
#let fitimg(path) = context {{
  let size = measure(image(path))
  let room-w = page.width - 5cm
  let room-h = page.height - 6cm
  let scale = calc.min(1.0, room-w / size.width, room-h / size.height)
  image(path, width: size.width * scale)
}}

"##,
            title = typst_string(&self.request.title),
            paper = self.request.paper.typst(),
        );
    }

    fn file(&mut self, extension: &str, bytes: Vec<u8>) -> String {
        let name = format!("file-{}.{extension}", self.files.len());
        self.files.push((name.clone(), bytes));
        name
    }

    fn anchor(&self, text: &str) -> String {
        match self.request.format {
            NoteFormat::Markdown => heading_anchor(text),
            NoteFormat::AsciiDoc => asciidoc_anchor(text),
        }
    }

    fn block(&mut self, block: &Block) {
        match &block.kind {
            BlockKind::Heading(level) => {
                let text = inline_typst(&block.runs);
                let label = typst_string(&self.anchor(&block.text()));
                let _ = write!(
                    self.out,
                    "#heading(level: {})[{text}] #label({label})",
                    (*level).clamp(1, 6)
                );
            }
            BlockKind::Paragraph => self.out.push_str(&inline_typst(&block.runs)),
            BlockKind::ListItem {
                depth,
                marker,
                checked,
            } => {
                let text = inline_typst(&block.runs);
                let check = match checked {
                    Some(true) => "#\"☒ \"",
                    Some(false) => "#\"☐ \"",
                    None => "",
                };
                let indent = "  ".repeat(*depth);
                match marker {
                    Some(ListMarker::Bullet) => {
                        let _ = write!(self.out, "{indent}- {check}{text}");
                    }
                    Some(ListMarker::Number(_)) => {
                        let _ = write!(self.out, "{indent}+ {check}{text}");
                    }
                    None => {
                        let _ = write!(self.out, "{indent}  {text}");
                    }
                }
            }
            BlockKind::Quote(_) => {
                let text = inline_typst(&block.runs);
                let _ = write!(self.out, "#quote(block: true)[{text}]");
            }
            BlockKind::Code { language } => {
                let code = block.text();
                match diagram_svg(language, &code) {
                    Some(Ok(svg)) => {
                        let name = self.file("svg", svg.into_bytes());
                        let _ = write!(
                            self.out,
                            "#block(breakable: false, width: 100%, align(center, fitimg({})))",
                            typst_string(&name)
                        );
                    }
                    Some(Err(error)) => {
                        self.missing.push(format!("a {language} diagram ({error})"));
                        let _ = write!(self.out, "#raw(block: true, {})", typst_string(&code));
                    }
                    None => {
                        let _ = write!(
                            self.out,
                            "#raw(block: true, lang: {}, {})",
                            typst_string(language),
                            typst_string(code.trim_end_matches('\n'))
                        );
                    }
                }
            }
            BlockKind::ThematicBreak => self.out.push_str("#line(length: 100%, stroke: luma(200))"),
            BlockKind::Admonition(label) => {
                let text = inline_typst(&block.runs);
                let _ = write!(
                    self.out,
                    "#block(stroke: (left: 3pt + rgb(\"#0969da\")), inset: (left: 10pt, y: 6pt))[#strong({})\\ {text}]",
                    typst_string(&title_case(label))
                );
            }
            BlockKind::Markup => {
                let _ = write!(
                    self.out,
                    "#raw(block: true, {})",
                    typst_string(&block.text())
                );
            }
            BlockKind::Table(table) => self.table(table),
            BlockKind::Image {
                target,
                alt,
                caption,
            } => match image_asset(self.request, target) {
                Ok(asset) => {
                    let name = self.file(asset.extension, asset.bytes);
                    let caption = caption
                        .as_ref()
                        .map(|c| format!(", caption: [#{}]", typst_string(c)))
                        .unwrap_or_default();
                    let _ = write!(
                        self.out,
                        "#figure(fitimg({}){caption})",
                        typst_string(&name)
                    );
                }
                Err(error) => {
                    self.missing.push(format!("the image {target} ({error})"));
                    let _ = write!(self.out, "#emph({})", typst_string(alt));
                }
            },
        }
    }

    fn table(&mut self, table: &Table) {
        let columns = table.columns().max(1);
        let align: Vec<&str> = (0..columns)
            .map(|i| match table.align.get(i) {
                Some(CellAlign::Center) => "center",
                Some(CellAlign::Right) => "right",
                _ => "left",
            })
            .collect();
        let cells = |row: &Vec<Vec<Run>>| -> Vec<String> {
            (0..columns)
                .map(|i| {
                    format!(
                        "[{}]",
                        row.get(i).map(|r| inline_typst(r)).unwrap_or_default()
                    )
                })
                .collect()
        };
        let mut parts = vec![
            format!("columns: {columns}"),
            format!("align: ({},)", align.join(", ")),
            "stroke: luma(200)".to_owned(),
        ];
        let mut rows = table.rows.iter();
        if table.header {
            if let Some(head) = rows.next() {
                parts.push(format!("table.header({})", cells(head).join(", ")));
            }
        }
        for row in rows {
            parts.extend(cells(row));
        }
        let body = format!("table({})", parts.join(", "));
        match &table.caption {
            Some(caption) => {
                let _ = write!(
                    self.out,
                    "#figure({body}, caption: [#{}])",
                    typst_string(caption)
                );
            }
            None => {
                let _ = write!(self.out, "#{body}");
            }
        }
    }
}

/// The runs as Typst expressions; text goes in as strings, so no markup of
/// Typst can sneak in. BR-005 decides which links stay links.
fn inline_typst(runs: &[Run]) -> String {
    let mut out = String::new();
    for run in runs {
        let style = &run.style;
        let mut expr = if style.code {
            format!("raw({})", typst_string(&run.text))
        } else {
            typst_string(&run.text)
        };
        if style.bold {
            expr = format!("strong({expr})");
        }
        if style.italic {
            expr = format!("emph({expr})");
        }
        if style.strike {
            expr = format!("strike({expr})");
        }
        if let Some(place) = &style.link {
            if is_web(place) {
                expr = format!("link({}, {expr})", typst_string(place));
            } else if let Some(anchor) = place.strip_prefix('#') {
                expr = format!("link(label({}), {expr})", typst_string(anchor));
            }
        }
        let _ = write!(out, "#{expr}");
    }
    out
}

/// Sets `source` with Typst into a PDF; `files` are the images and diagrams
/// it reads. Fonts: those Typst ships and those of the system (ADR-015).
fn typeset(source: &str, files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    use typst::diag::{FileError, FileResult};
    use typst::foundations::{Bytes, Datetime, Duration};
    use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
    use typst::text::{Font, FontBook};
    use typst::utils::LazyHash;
    use typst::{Library, LibraryExt, World};

    struct Document {
        library: LazyHash<Library>,
        fonts: &'static typst_kit::fonts::FontStore,
        main: Source,
        files: Vec<(String, Bytes)>,
    }

    impl World for Document {
        fn library(&self) -> &LazyHash<Library> {
            &self.library
        }
        fn book(&self) -> &LazyHash<FontBook> {
            self.fonts.book()
        }
        fn main(&self) -> FileId {
            self.main.id()
        }
        fn source(&self, id: FileId) -> FileResult<Source> {
            if id == self.main.id() {
                Ok(self.main.clone())
            } else {
                Err(FileError::NotFound(id.vpath().get_without_slash().into()))
            }
        }
        fn file(&self, id: FileId) -> FileResult<Bytes> {
            let name = id.vpath().get_without_slash();
            self.files
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, bytes)| bytes.clone())
                .ok_or_else(|| FileError::NotFound(name.into()))
        }
        fn font(&self, index: usize) -> Option<Font> {
            self.fonts.font(index)
        }
        fn today(&self, _: Option<Duration>) -> Option<Datetime> {
            None
        }
    }

    let path = VirtualPath::new("note.typ").map_err(|e| format!("{e:?}"))?;
    let document = Document {
        library: LazyHash::new(Library::default()),
        fonts: fonts(),
        main: Source::new(
            FileId::new(RootedPath::new(VirtualRoot::Project, path)),
            source.to_owned(),
        ),
        files: files
            .into_iter()
            .map(|(name, bytes)| (name, Bytes::new(bytes)))
            .collect(),
    };
    let compiled = typst::compile::<typst_layout::PagedDocument>(&document)
        .output
        .map_err(|errors| {
            errors
                .iter()
                .map(|e| e.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
    typst_pdf::pdf(&compiled, &typst_pdf::PdfOptions::default()).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// The fonts, found once per run of Underline.
fn fonts() -> &'static typst_kit::fonts::FontStore {
    static FONTS: std::sync::OnceLock<typst_kit::fonts::FontStore> = std::sync::OnceLock::new();
    FONTS.get_or_init(|| {
        let mut fonts = typst_kit::fonts::FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        fonts.extend(typst_kit::fonts::system());
        fonts
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("underline-export-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("ws/img")).unwrap();
            fs::create_dir_all(root.join("out")).unwrap();
            // A 1x1 PNG.
            let png = [
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, 0x49, 0x48, 0x44,
                0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0x0A,
                0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0, 1, 0, 0, 5, 0, 1, 0x0D, 0x0A, 0x2D,
                0xB4, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
            ];
            fs::write(root.join("ws/img/shot.png"), png).unwrap();
            Self(root)
        }

        fn request(&self, text: &str, format: NoteFormat, target: &str) -> ExportRequest {
            let name = match format {
                NoteFormat::Markdown => "Guide.md",
                NoteFormat::AsciiDoc => "Guide.adoc",
            };
            ExportRequest {
                text: text.to_owned(),
                format,
                title: "Guide".into(),
                root: self.0.join("ws"),
                note: self.0.join("ws").join(name),
                target: self.0.join(target),
                paper: Paper::A4,
            }
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const MARKDOWN: &str = "# Getting Started\n\nSome **bold** and *italic* and `code`, see [site](https://example.org) and [ideas](Ideas.md).\n\n- one\n  - inner\n- [x] done\n\n1. first\n2. second\n\n> quoted\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |\n\n![Shot](img/shot.png)\n\n---\n";

    /// TC-009-01: Markdown to AsciiDoc, every block with its meaning and the
    /// places following the export.
    #[test]
    fn uc009_tc01_markdown_to_asciidoc() {
        let sandbox = Sandbox::new("tc01");
        let request = sandbox.request(MARKDOWN, NoteFormat::Markdown, "out/Getting-Started.adoc");
        let exported = export(&request, ExportKind::OtherFormat).unwrap();
        let adoc = String::from_utf8(exported.bytes).unwrap();
        for expected in [
            "= Getting Started",
            "**bold**",
            "__italic__",
            "`+code+`",
            "https://example.org[site]",
            "xref:../ws/Ideas.md[ideas]",
            "* one",
            "** inner",
            "* [x] done",
            ". first",
            "____\nquoted\n____",
            "[source,rust]\n----\nfn main() {}\n----",
            "|===",
            "image::../ws/img/shot.png[Shot]",
            "'''",
        ] {
            assert!(adoc.contains(expected), "missing {expected:?} in\n{adoc}");
        }
        assert!(exported.approximated.is_empty());
        // Read back, it gives the same kinds of blocks.
        let kinds = |text: &str, format| {
            parse_blocks(text, format)
                .iter()
                .map(|b| std::mem::discriminant(&b.kind))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(&adoc, NoteFormat::AsciiDoc),
            kinds(MARKDOWN, NoteFormat::Markdown)
        );
    }

    /// TC-009-02 / TC-009-09: AsciiDoc to Markdown; an admonition is only
    /// approximated and named by its line.
    #[test]
    fn uc009_tc02_tc09_asciidoc_to_markdown() {
        let sandbox = Sandbox::new("tc02");
        let adoc = "= Setup\n\nSee xref:Install.adoc[Install].\n\nNOTE: Mind the gap.\n\n[plantuml]\n----\nA -> B\n----\n";
        let request = sandbox.request(adoc, NoteFormat::AsciiDoc, "out/Setup.md");
        let exported = export(&request, ExportKind::OtherFormat).unwrap();
        let md = String::from_utf8(exported.bytes).unwrap();
        assert!(md.contains("# Setup"), "{md}");
        assert!(md.contains("[Install](../ws/Install.adoc)"), "{md}");
        assert!(md.contains("> **Note:** Mind the gap."), "{md}");
        assert!(md.contains("```plantuml\nA -> B\n```"), "{md}");
        assert_eq!(exported.approximated, [5]);
    }

    /// TC-009-03 / TC-009-06: one HTML file with the image inside; web and
    /// heading links stay links, note links are text.
    #[test]
    fn uc009_tc03_tc06_html_stands_alone() {
        let sandbox = Sandbox::new("tc03");
        let text = format!("{MARKDOWN}\n[goals](#getting-started)\n");
        let request = sandbox.request(&text, NoteFormat::Markdown, "out/Guide.html");
        let exported = export(&request, ExportKind::Html).unwrap();
        let html = String::from_utf8(exported.bytes).unwrap();
        assert!(html.contains("<title>Guide</title>"));
        assert!(html.contains("<img src=\"data:image/png;base64,iVBORw0KGgo"));
        assert!(html.contains("<a href=\"https://example.org\">site</a>"));
        assert!(html.contains("<a href=\"#getting-started\">goals</a>"));
        assert!(html.contains("<h1 id=\"getting-started\">"));
        assert!(html.contains(" and ideas."), "{html}");
        assert!(html.contains("prefers-color-scheme: dark"));
        assert!(html.contains("--l:#"), "code is colored");
        assert!(exported.missing.is_empty(), "{:?}", exported.missing);
    }

    /// TC-009-10: a missing image is named, its text stands in its place.
    #[test]
    fn uc009_tc10_missing_image() {
        let sandbox = Sandbox::new("tc10");
        let request = sandbox.request(
            "![Plan](img/missing.png)\n",
            NoteFormat::Markdown,
            "out/x.html",
        );
        let exported = export(&request, ExportKind::Html).unwrap();
        let html = String::from_utf8(exported.bytes).unwrap();
        assert!(html.contains("<span class=\"missing\">Plan</span>"));
        assert_eq!(exported.missing.len(), 1);
    }

    /// TC-009-05: a PDF with the note's text, umlauts, a table, an image,
    /// and the paper of the region.
    #[test]
    fn uc009_tc05_pdf() {
        let sandbox = Sandbox::new("tc05");
        let text = format!("{MARKDOWN}\nGrüße aus \"Köln\" #nottypst $x$ <b>\n");
        let request = sandbox.request(&text, NoteFormat::Markdown, "out/Guide.pdf");
        let exported = export(&request, ExportKind::Pdf).unwrap();
        assert!(exported.bytes.starts_with(b"%PDF"));
        assert!(exported.missing.is_empty(), "{:?}", exported.missing);
        assert_eq!(Paper::of_region("en_US.UTF-8"), Paper::Letter);
        assert_eq!(Paper::of_region("de_DE.UTF-8"), Paper::A4);
        assert_eq!(Paper::of_region("en-CA"), Paper::Letter);
    }

    #[test]
    fn base64_as_the_standard_says() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
