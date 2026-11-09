//! The own AsciiDoc parser of the facade (ADR-003): the blocks Underline
//! shows formatted — section titles, paragraphs, lists, source and literal
//! blocks, quotes, admonitions, and breaks — each with its source span, and
//! the inline formatting and links of their text as Asciidoctor reads them.
//! Every other block (tables, block images and macros, comments, attribute
//! entries, passthroughs) stays a markup block that the facade shows and
//! writes back unchanged (NFR-007).

use std::ops::Range;

use crate::blocks::{Block, BlockKind, CellAlign, ListMarker, Run, Style, Table};

/// The labels of admonitions (`NOTE: …`, `[NOTE]`).
const ADMONITIONS: [&str; 5] = ["NOTE", "TIP", "IMPORTANT", "WARNING", "CAUTION"];

/// Splits an AsciiDoc `source` into the blocks of the facade.
pub(crate) fn asciidoc_blocks(source: &str) -> Vec<Block> {
    let lines = Line::split(source);
    let mut parser = Parser {
        source,
        lines: &lines,
        blocks: Vec::new(),
        numbers: Vec::new(),
    };
    parser.parse(0..lines.len(), 0);
    parser.blocks
}

/// One line of the source: its text without the line ending, where it
/// starts, and where the next line starts.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
    next: usize,
}

impl<'a> Line<'a> {
    fn split(source: &'a str) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        let mut start = 0;
        for piece in source.split_inclusive('\n') {
            let text = piece.trim_end_matches('\n').trim_end_matches('\r');
            lines.push(Line {
                text,
                start,
                next: start + piece.len(),
            });
            start += piece.len();
        }
        lines
    }

    fn blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Lines that only set something up for the block after them.
#[derive(Default)]
struct Pending {
    /// The style of a block attribute line (`source`, `NOTE`, `quote`, ...).
    style: Option<String>,
    /// The language of a `[source,lang]` line.
    language: Option<String>,
    /// The first line of attribute or title lines not used by the block.
    unused: Option<usize>,
    /// The text of a block title (`.Title`), a caption for an image.
    title: Option<String>,
    /// The other block attribute lines, for a table's options.
    attributes: String,
}

struct Parser<'s, 'l> {
    source: &'s str,
    lines: &'l [Line<'s>],
    blocks: Vec<Block>,
    /// The next number of the ordered list at each depth.
    numbers: Vec<usize>,
}

impl Parser<'_, '_> {
    /// Parses the lines `range` into blocks; `quote` is the depth of quote
    /// blocks around them.
    fn parse(&mut self, range: Range<usize>, quote: usize) {
        let mut i = range.start;
        let mut pending = Pending::default();
        let mut header = range.start == 0 && quote == 0;
        while i < range.end {
            let line = self.lines[i];
            let text = line.text;
            if line.blank() {
                // A list goes on across blank lines; its numbers do too.
                pending.unused = self.flush_unused(pending.unused, i);
                pending = Pending::default();
                i += 1;
                continue;
            }
            // Block attribute lines and block titles: kept for the next block.
            if is_attribute_line(text) || is_block_title(text) {
                if is_attribute_line(text) && !text.starts_with("[[") {
                    let inner = &text[1..text.len() - 1];
                    let mut parts = inner.split(',').map(str::trim);
                    let style = parts.next().unwrap_or("");
                    let language = parts.next().filter(|l| !l.is_empty());
                    // `[plantuml]` and `[mermaid]`: diagrams, as code
                    // blocks in their language.
                    if crate::diagram::DiagramLanguage::from_code_language(style).is_some() {
                        pending.style = Some("source".into());
                        pending.language = Some(style.to_owned());
                        i += 1;
                        continue;
                    }
                    if style == "source" || (style.is_empty() && language.is_some()) {
                        pending.style = Some("source".into());
                        pending.language = language.map(str::to_owned);
                        i += 1;
                        continue;
                    }
                    if ADMONITIONS.contains(&style)
                        || matches!(style, "quote" | "listing" | "literal")
                    {
                        pending.style = Some(style.to_owned());
                        i += 1;
                        continue;
                    }
                }
                if is_block_title(text) {
                    pending.title = Some(text[1..].trim().to_owned());
                } else {
                    pending.attributes.push_str(text);
                }
                pending.unused.get_or_insert(i);
                i += 1;
                continue;
            }
            let first_block = header;
            header = false;
            // A table, with its title and options.
            if text.starts_with("|===") && delimiter(text).is_some() {
                let start = pending.unused.take().unwrap_or(i);
                let close = (i + 1..range.end)
                    .find(|&j| self.lines[j].text == text)
                    .unwrap_or(range.end);
                let span_end = self.lines.get(close).map_or(self.source.len(), |l| l.next);
                let table = self.table(i + 1..close, &pending);
                let mut block = markup_block(self.source, self.lines[start].start..span_end);
                block.kind = BlockKind::Table(Box::new(table));
                self.blocks.push(block);
                pending = Pending::default();
                self.numbers.clear();
                i = close + 1;
                continue;
            }
            if let Some(delimiter) = delimiter(text) {
                pending.unused = self.flush_unused(pending.unused, i);
                let close = (i + 1..range.end)
                    .find(|&j| self.lines[j].text == text)
                    .unwrap_or(range.end);
                let span_end = self.lines.get(close).map_or(self.source.len(), |l| l.next);
                let inner = i + 1..close;
                self.delimited(delimiter, &pending, line.start..span_end, inner, quote);
                pending = Pending::default();
                self.numbers.clear();
                i = close + 1;
                continue;
            }
            if let Some(level) = section_level(text) {
                pending.unused = self.flush_unused(pending.unused, i);
                self.numbers.clear();
                let content = line.start + level + 1;
                let title_end = line.start + text.trim_end().len();
                let runs = self.inline(std::slice::from_ref(&(content..title_end)));
                self.blocks.push(Block {
                    kind: BlockKind::Heading(level as u8),
                    span: line.start..line.next,
                    runs,
                });
                i += 1;
                // The document header: author, revision and attribute lines
                // right after the document title.
                if level == 1 && first_block {
                    let end = self.paragraph_end(i, range.end);
                    if end > i {
                        self.markup(i..end);
                        i = end;
                    }
                }
                pending = Pending::default();
                continue;
            }
            if is_break(text) {
                pending.unused = self.flush_unused(pending.unused, i);
                self.numbers.clear();
                self.blocks.push(Block {
                    kind: BlockKind::ThematicBreak,
                    span: line.start..line.next,
                    runs: Vec::new(),
                });
                pending = Pending::default();
                i += 1;
                continue;
            }
            if let Some(item) = list_item(text) {
                pending.unused = self.flush_unused(pending.unused, i);
                i = self.list_item(i, range.end, item);
                pending = Pending::default();
                continue;
            }
            // Blocks shown as markup: comments, attribute entries, block
            // macros (images, includes), description lists.
            // A block image, with its title as caption.
            if let Some((target, alt)) = block_image(text) {
                let start = pending.unused.take().unwrap_or(i);
                self.numbers.clear();
                let span = self.lines[start].start..line.next;
                let mut block = markup_block(self.source, span);
                block.kind = BlockKind::Image {
                    target,
                    alt,
                    caption: pending.title.take(),
                };
                self.blocks.push(block);
                pending = Pending::default();
                i += 1;
                continue;
            }
            if is_markup_line(text) {
                let start = pending.unused.take().unwrap_or(i);
                let end = self.paragraph_end(i, range.end);
                self.numbers.clear();
                self.markup(start..end);
                pending = Pending::default();
                i = end;
                continue;
            }
            pending.unused = self.flush_unused(pending.unused, i);
            self.numbers.clear();
            // A literal paragraph: indented lines.
            if text.starts_with([' ', '\t']) {
                let end = self.paragraph_end(i, range.end);
                self.code_between(i..end, "", self.lines[i].start..self.lines[end - 1].next);
                pending = Pending::default();
                i = end;
                continue;
            }
            let end = self.paragraph_end(i, range.end);
            let (kind, skip) = match ADMONITIONS.iter().find(|label| {
                text.strip_prefix(**label)
                    .is_some_and(|r| r.starts_with(": "))
            }) {
                Some(label) => (BlockKind::Admonition((*label).to_owned()), label.len() + 2),
                None => match pending.style.as_deref() {
                    Some(label) if ADMONITIONS.contains(&label) => {
                        (BlockKind::Admonition(label.to_owned()), 0)
                    }
                    Some("quote") => (BlockKind::Quote(quote + 1), 0),
                    Some("listing" | "literal" | "source") => {
                        let language = pending.language.clone().unwrap_or_default();
                        self.code_between(i..end, &language, line.start..self.lines[end - 1].next);
                        pending = Pending::default();
                        i = end;
                        continue;
                    }
                    _ if quote > 0 => (BlockKind::Quote(quote), 0),
                    _ => (BlockKind::Paragraph, 0),
                },
            };
            self.paragraph(i..end, kind, skip);
            pending = Pending::default();
            i = end;
        }
        self.flush_unused(pending.unused, range.end);
    }

    /// Shows the attribute and title lines from `start` to `end` that no
    /// block used as markup.
    fn flush_unused(&mut self, start: Option<usize>, end: usize) -> Option<usize> {
        if let Some(start) = start {
            self.markup(start..end);
        }
        None
    }

    /// A delimited block spanning `span`, with the lines `inner` inside.
    fn delimited(
        &mut self,
        delimiter: &str,
        pending: &Pending,
        span: Range<usize>,
        inner: Range<usize>,
        quote: usize,
    ) {
        match delimiter.as_bytes()[0] {
            b'-' if delimiter.len() >= 4 => {
                let language = match pending.style.as_deref() {
                    Some("source") => pending.language.clone().unwrap_or_default(),
                    _ => String::new(),
                };
                self.code_between(inner, &language, span);
            }
            b'.' => {
                let language = match pending.style.as_deref() {
                    Some("source") => pending.language.clone().unwrap_or_default(),
                    _ => String::new(),
                };
                self.code_between(inner, &language, span);
            }
            b'_' => self.parse(inner, quote + 1),
            b'=' => match pending.style.as_deref() {
                Some(label) if ADMONITIONS.contains(&label) => {
                    let first = self.blocks.len();
                    self.parse(inner, quote);
                    for block in &mut self.blocks[first..] {
                        if matches!(block.kind, BlockKind::Paragraph) {
                            block.kind = BlockKind::Admonition(label.to_owned());
                        }
                    }
                }
                _ => self.parse(inner, quote),
            },
            b'*' | b'-' => self.parse(inner, quote),
            // Comments, passthroughs and tables are shown as markup.
            _ => self.blocks.push(markup_block(self.source, span)),
        }
    }

    /// The cells of a table between its delimiters, the lines `inner`: each
    /// `|` starts a cell, a line without one continues the cell before,
    /// and the cells fill rows of as many columns as `cols` names or the
    /// first line holds. The first line is the header when the options say
    /// so, or when it holds all its cells and a blank line follows, as
    /// Asciidoctor reads it.
    fn table(&self, inner: Range<usize>, pending: &Pending) -> Table {
        let attributes = pending.attributes.as_str();
        let mut cells: Vec<Vec<Range<usize>>> = Vec::new();
        let mut first_line_cells = None;
        let mut implicit_header = false;
        for j in inner.clone() {
            let line = self.lines[j];
            if line.blank() {
                if first_line_cells.is_some() && j == inner.start + 1 {
                    implicit_header = true;
                }
                continue;
            }
            let parts = cell_parts(line.text);
            if first_line_cells.is_none() {
                first_line_cells = Some(parts.iter().filter(|(_, starts)| *starts).count());
            }
            for (part, starts) in parts {
                let range = line.start + part.start..line.start + part.end;
                let trimmed = trim_range(self.source, range);
                match (starts, cells.last_mut()) {
                    (false, Some(cell)) => cell.push(trimmed),
                    _ => cells.push(vec![trimmed]),
                }
            }
        }
        let columns = cols_count(attributes)
            .or(first_line_cells)
            .unwrap_or(1)
            .max(1);
        let header = if attributes.contains("noheader") {
            false
        } else {
            attributes.contains("header") || implicit_header
        };
        let runs: Vec<Vec<Run>> = cells
            .iter()
            .map(|pieces| {
                let pieces: Vec<Range<usize>> =
                    pieces.iter().filter(|r| !r.is_empty()).cloned().collect();
                self.inline(&pieces)
            })
            .collect();
        let rows = runs.chunks(columns).map(<[Vec<Run>]>::to_vec).collect();
        Table {
            header,
            align: vec![CellAlign::Left; columns],
            rows,
            caption: pending.title.clone(),
        }
    }

    /// A code block spanning `span`: the lines `inner`, one to one.
    fn code_between(&mut self, inner: Range<usize>, language: &str, span: Range<usize>) {
        let at = self.lines.get(inner.start).map_or(span.end, |l| l.start);
        let end = if inner.is_empty() {
            at
        } else {
            self.lines[inner.end - 1].start + self.lines[inner.end - 1].text.len()
        };
        let text = &self.source[at..end];
        self.blocks.push(Block {
            kind: BlockKind::Code {
                language: language.to_owned(),
            },
            span,
            runs: vec![identity_run(
                text,
                at,
                Style {
                    code: true,
                    ..Style::default()
                },
            )],
        });
    }

    fn markup(&mut self, lines: Range<usize>) {
        if lines.is_empty() {
            return;
        }
        let span = self.lines[lines.start].start..self.lines[lines.end - 1].next;
        self.blocks.push(markup_block(self.source, span));
    }

    /// The end of the paragraph starting at line `i`: the next blank line.
    fn paragraph_end(&self, i: usize, end: usize) -> usize {
        (i..end).find(|&j| self.lines[j].blank()).unwrap_or(end)
    }

    /// A paragraph of the lines `lines`, whose first `skip` bytes are a
    /// label the facade does not show as text.
    fn paragraph(&mut self, lines: Range<usize>, kind: BlockKind, skip: usize) {
        let first = self.lines[lines.start];
        let span = first.start..self.lines[lines.end - 1].next;
        let mut pieces = Vec::new();
        for (n, j) in lines.enumerate() {
            let line = self.lines[j];
            let from = line.start + if n == 0 { skip } else { 0 };
            pieces.push(from..line.start + line.text.len());
        }
        let runs = self.inline(&pieces);
        self.blocks.push(Block { kind, span, runs });
    }

    /// A list item at line `i`, with the lines that continue it; returns
    /// the line after it.
    fn list_item(&mut self, i: usize, end: usize, item: Item) -> usize {
        let line = self.lines[i];
        let mut last = i + 1;
        while last < end {
            let text = self.lines[last].text;
            if self.lines[last].blank()
                || list_item(text).is_some()
                || text == "+"
                || delimiter(text).is_some()
                || is_attribute_line(text)
            {
                break;
            }
            last += 1;
        }
        let marker = match item.ordered {
            true => {
                if self.numbers.len() <= item.depth {
                    self.numbers.resize(item.depth + 1, 1);
                }
                self.numbers.truncate(item.depth + 1);
                let number = item.number.unwrap_or(self.numbers[item.depth]);
                self.numbers[item.depth] = number + 1;
                ListMarker::Number(number)
            }
            false => {
                self.numbers.truncate(item.depth);
                ListMarker::Bullet
            }
        };
        let first = line.start + item.text..line.start + line.text.len();
        let pieces: Vec<Range<usize>> = std::iter::once(first)
            .chain((i + 1..last).map(|j| {
                let l = self.lines[j];
                let indent = l.text.len() - l.text.trim_start().len();
                l.start + indent..l.start + l.text.len()
            }))
            .collect();
        let mut runs = self.inline(&pieces);
        if runs.is_empty() {
            // UC-005: an item still empty (just made by Enter); its text
            // starts after the marker.
            runs.push(identity_run("", pieces[0].start, Style::default()));
        }
        self.blocks.push(Block {
            kind: BlockKind::ListItem {
                depth: item.depth,
                marker: Some(marker),
                checked: item.checked,
            },
            span: line.start..self.lines[last - 1].next,
            runs,
        });
        // A list continuation (`+`) attaches the next paragraph to the item.
        let mut next = last;
        while next < end && self.lines[next].text == "+" {
            let start = next + 1;
            if start >= end || self.lines[start].blank() {
                return start;
            }
            if delimiter(self.lines[start].text).is_some()
                || is_attribute_line(self.lines[start].text)
            {
                return start;
            }
            let stop = (start..end)
                .find(|&j| {
                    let t = self.lines[j].text;
                    self.lines[j].blank() || t == "+" || list_item(t).is_some()
                })
                .unwrap_or(end);
            let pieces: Vec<_> = (start..stop)
                .map(|j| {
                    let l = self.lines[j];
                    l.start..l.start + l.text.len()
                })
                .collect();
            let runs = self.inline(&pieces);
            self.blocks.push(Block {
                kind: BlockKind::ListItem {
                    depth: item.depth,
                    marker: None,
                    checked: None,
                },
                span: self.lines[start].start..self.lines[stop - 1].next,
                runs,
            });
            next = stop;
        }
        next
    }

    /// The inline content of the source pieces `pieces` (one per line) as
    /// styled runs; the line breaks between them are shown as spaces.
    fn inline(&self, pieces: &[Range<usize>]) -> Vec<Run> {
        let mut chars: Vec<(char, usize)> = Vec::new();
        for (n, piece) in pieces.iter().enumerate() {
            let mut text = &self.source[piece.clone()];
            let mut piece_end = piece.end;
            // A hard line break: ` +` at the end of a line.
            if n + 1 < pieces.len() && text.ends_with(" +") {
                text = &text[..text.len() - 2];
                piece_end -= 1;
                chars.extend(text.char_indices().map(|(i, c)| (c, piece.start + i)));
                chars.push((' ', piece_end - 1));
                continue;
            }
            chars.extend(text.char_indices().map(|(i, c)| (c, piece.start + i)));
            if n + 1 < pieces.len() {
                chars.push((' ', piece_end));
            }
        }
        let mut out = Runs::default();
        inline(&chars, &Style::default(), &mut out);
        out.0
    }
}

/// Builds runs from characters and their source offsets.
#[derive(Default)]
struct Runs(Vec<Run>);

impl Runs {
    fn push(&mut self, c: char, offset: usize, style: &Style) {
        match self.0.last_mut() {
            Some(run) if run.style == *style => {
                run.text.push(c);
                run.offsets.push(offset);
                run.end = offset + c.len_utf8();
            }
            _ => self.0.push(Run {
                text: c.to_string(),
                style: style.clone(),
                offsets: vec![offset],
                end: offset + c.len_utf8(),
            }),
        }
    }
}

/// Characters Asciidoctor reads as the start of formatting when a
/// backslash does not stand before them.
const MARKS: [char; 4] = ['*', '_', '`', '#'];

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// The inline formatting of `chars` (characters with source offsets).
fn inline(chars: &[(char, usize)], style: &Style, out: &mut Runs) {
    let at = |i: usize| chars.get(i).map(|(c, _)| *c);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].0;
        // An escaped mark stands for itself.
        if c == '\\' && at(i + 1).is_some_and(|n| MARKS.contains(&n) || n == '\\') {
            out.push(chars[i + 1].0, chars[i + 1].1, style);
            i += 2;
            continue;
        }
        if MARKS.contains(&c) {
            // Unconstrained: `**bold**` anywhere.
            if at(i + 1) == Some(c) {
                if let Some(close) = (i + 3..chars.len())
                    .find(|&j| at(j) == Some(c) && at(j + 1) == Some(c) && at(j - 1) != Some('\\'))
                {
                    inline(&chars[i + 2..close], &styled(style, c), out);
                    i = close + 2;
                    continue;
                }
            }
            // Constrained: `*bold*` between word boundaries.
            let opens = !is_word(i.checked_sub(1).and_then(at))
                && at(i + 1).is_some_and(|n| !n.is_whitespace() && n != c);
            if opens {
                if let Some(close) = (i + 2..chars.len()).find(|&j| {
                    at(j) == Some(c)
                        && at(j - 1).is_some_and(|p| !p.is_whitespace() && p != '\\')
                        && !is_word(at(j + 1))
                }) {
                    inline(&chars[i + 1..close], &styled(style, c), out);
                    i = close + 1;
                    continue;
                }
            }
        }
        if style.link.is_none() && !is_word(i.checked_sub(1).and_then(at)) {
            if let Some(next) = link(chars, i, style, out) {
                i = next;
                continue;
            }
        }
        out.push(c, chars[i].1, style);
        i += 1;
    }
}

fn styled(style: &Style, mark: char) -> Style {
    let mut style = style.clone();
    match mark {
        '*' => style.bold = true,
        '_' => style.italic = true,
        '`' => style.code = true,
        _ => {}
    }
    style
}

/// A link starting at `chars[i]`: a URL, `link:` and `xref:` macros,
/// `<<id,text>>` references, and inline images. Pushes what the facade shows
/// and returns the index after the link.
fn link(chars: &[(char, usize)], i: usize, style: &Style, out: &mut Runs) -> Option<usize> {
    if !matches!(chars[i].0, '<' | 'l' | 'x' | 'i' | 'h' | 'f' | 'm') {
        return None;
    }
    let rest: String = chars[i..].iter().take(12).map(|(c, _)| *c).collect();
    // `<<id>>` and `<<id,text>>`.
    if rest.starts_with("<<") {
        let close = (i + 2..chars.len().saturating_sub(1))
            .find(|&j| chars[j].0 == '>' && chars[j + 1].0 == '>')?;
        let inner = &chars[i + 2..close];
        let comma = inner.iter().position(|(c, _)| *c == ',');
        let target: String = inner[..comma.unwrap_or(inner.len())]
            .iter()
            .map(|(c, _)| *c)
            .collect();
        let target = target.trim();
        if target.is_empty() || target.contains(char::is_whitespace) {
            return None;
        }
        let address = if target.contains('#') || target.contains('.') {
            target.to_owned()
        } else {
            format!("#{target}")
        };
        let link_style = Style {
            link: Some(address),
            ..style.clone()
        };
        match comma {
            Some(comma) => {
                let text = &inner[comma + 1..];
                let skip = text.iter().take_while(|(c, _)| c.is_whitespace()).count();
                inline(&text[skip..], &link_style, out);
            }
            None => inline(inner, &link_style, out),
        }
        return Some(close + 2);
    }
    let (macro_len, kind) = if rest.starts_with("link:") {
        (5, "link")
    } else if rest.starts_with("xref:") {
        (5, "xref")
    } else if rest.starts_with("image:") && !rest.starts_with("image::") {
        (6, "image")
    } else if ["https://", "http://", "ftp://", "irc://", "mailto:"]
        .iter()
        .any(|scheme| rest.starts_with(scheme))
    {
        (0, "url")
    } else {
        return None;
    };
    let target_start = i + macro_len;
    let mut j = target_start;
    while j < chars.len() && !chars[j].0.is_whitespace() && chars[j].0 != '[' {
        j += 1;
    }
    if j == target_start {
        return None;
    }
    let bracket = chars.get(j).is_some_and(|(c, _)| *c == '[');
    if !bracket {
        if kind != "url" {
            return None;
        }
        // A bare URL: trailing punctuation is not part of it.
        while j > target_start && matches!(chars[j - 1].0, '.' | ',' | ';' | ':' | '!' | '?' | ')')
        {
            j -= 1;
        }
    }
    let target: String = chars[target_start..j].iter().map(|(c, _)| *c).collect();
    let link_style = Style {
        link: Some(target),
        ..style.clone()
    };
    if !bracket {
        for &(c, offset) in &chars[i..j] {
            out.push(c, offset, &link_style);
        }
        return Some(j);
    }
    let close = (j + 1..chars.len()).find(|&k| chars[k].0 == ']')?;
    let text = &chars[j + 1..close];
    // `[text,window=_blank]`: the text is before the first named attribute.
    let text_end = text
        .iter()
        .position(|(c, _)| *c == ',')
        .filter(|&p| text[p..].iter().any(|(c, _)| *c == '='))
        .unwrap_or(text.len());
    let text = &text[..text_end];
    let text = if text.last().is_some_and(|(c, _)| *c == '^') {
        &text[..text.len() - 1]
    } else {
        text
    };
    if text.is_empty() {
        for &(c, offset) in &chars[target_start..j] {
            out.push(c, offset, &link_style);
        }
    } else {
        inline(text, &link_style, out);
    }
    Some(close + 1)
}

/// The parts of a list item line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    depth: usize,
    ordered: bool,
    /// An explicit number (`3.`).
    number: Option<usize>,
    checked: Option<bool>,
    /// Where the text starts in the line, in bytes.
    text: usize,
}

/// A list item line: `* x`, `** x`, `- x`, `. x`, `.. x`, `3. x`, `* [x] x`.
fn list_item(line: &str) -> Option<Item> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let first = rest.chars().next()?;
    let (count, ordered, number) = match first {
        '*' | '.' => {
            let count = rest.chars().take_while(|&c| c == first).count();
            (count, first == '.', None)
        }
        '-' => (1, false, None),
        c if c.is_ascii_digit() => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            if rest[digits..].starts_with('.') {
                (digits + 1, true, rest[..digits].parse().ok())
            } else {
                return None;
            }
        }
        _ => return None,
    };
    if count > 5 && number.is_none() {
        return None;
    }
    let after = &rest[count..];
    // UC-005: `* ` with nothing after it yet is the item Enter just made.
    if !after.starts_with(' ') {
        return None;
    }
    let depth = if number.is_some() || first == '-' {
        0
    } else {
        count - 1
    };
    let mut text = indent + count + 1;
    let body = &line[text..];
    let checked = if !ordered {
        match body.get(..4) {
            Some("[ ] ") => Some(false),
            Some("[x] " | "[X] " | "[*] ") => Some(true),
            _ => None,
        }
    } else {
        None
    };
    if checked.is_some() {
        text += 4;
    }
    Some(Item {
        depth,
        ordered,
        number,
        checked,
        text,
    })
}

/// The level of a section title line (`== Title` is 2), if it is one.
fn section_level(line: &str) -> Option<usize> {
    let level = line.chars().take_while(|&c| c == '=').count();
    if (1..=6).contains(&level)
        && line[level..].starts_with(' ')
        && !line[level..].trim().is_empty()
    {
        Some(level)
    } else {
        None
    }
}

/// The delimiter of a delimited block, if `line` is one.
fn delimiter(line: &str) -> Option<&str> {
    if line == "--" {
        return Some(line);
    }
    if line.starts_with("|===") && line[1..].chars().all(|c| c == '=') {
        return Some(line);
    }
    let first = line.chars().next()?;
    if line.len() >= 4
        && matches!(first, '-' | '.' | '=' | '*' | '_' | '+' | '/')
        && line.chars().all(|c| c == first)
    {
        Some(line)
    } else {
        None
    }
}

fn is_attribute_line(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']') && line.len() >= 2 && !line.contains("] ")
}

/// The target and alternative text of a block image `image::target[alt]`.
fn block_image(line: &str) -> Option<(String, String)> {
    let rest = line.trim_end().strip_prefix("image::")?;
    let open = rest.find('[')?;
    let attributes = rest[open + 1..].strip_suffix(']')?;
    let target = rest[..open].trim();
    if target.is_empty() {
        return None;
    }
    let alt = attributes.split(',').next().unwrap_or("").trim();
    let alt = alt.trim_matches('"');
    let alt = if alt.is_empty() || alt.contains('=') {
        // Asciidoctor's default: the file name without its extension.
        let name = target.rsplit('/').next().unwrap_or(target);
        name.rsplit_once('.')
            .map_or(name, |(stem, _)| stem)
            .to_owned()
    } else {
        alt.to_owned()
    };
    Some((target.to_owned(), alt))
}

/// The pieces of a table line: the text of each cell, and whether a `|`
/// starts it (a piece before the first `|` continues the cell before).
fn cell_parts(line: &str) -> Vec<(Range<usize>, bool)> {
    let bytes = line.as_bytes();
    let mut bars: Vec<usize> = Vec::new();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
            bars.push(i);
        }
    }
    let mut parts = Vec::new();
    let first = bars.first().copied().unwrap_or(line.len());
    // Text before the first bar is a cell spec (`2+|`, `a|`) when short and
    // made of spec characters; otherwise it continues the cell before.
    let spec = &line[..first];
    let is_spec = !spec.is_empty()
        && spec
            .chars()
            .all(|c| c.is_ascii_digit() || "+*.<>^aehlmsdv".contains(c));
    if first > 0 && !is_spec {
        parts.push((0..first, false));
    }
    for (n, &bar) in bars.iter().enumerate() {
        let end = bars.get(n + 1).copied().unwrap_or(line.len());
        // A spec right before the next bar belongs to that cell.
        let mut stop = end;
        if n + 1 < bars.len() {
            let before = &line[bar + 1..end];
            let tail = before.len()
                - before
                    .trim_end_matches(|c: char| c.is_ascii_digit() || "+*.<>^".contains(c))
                    .len();
            if tail > 0 && before[..before.len() - tail].ends_with(' ') {
                stop = end - tail;
            }
        }
        parts.push((bar + 1..stop, true));
    }
    parts
}

/// `range` without the spaces at its ends.
fn trim_range(source: &str, range: Range<usize>) -> Range<usize> {
    let text = &source[range.clone()];
    let start = range.start + (text.len() - text.trim_start().len());
    let end = range.end - (text.len() - text.trim_end().len());
    start..end.max(start)
}

/// The number of columns a `cols="1,2,1"` or `cols="3*"` attribute names.
fn cols_count(attributes: &str) -> Option<usize> {
    let at = attributes.find("cols=")?;
    let rest = attributes[at + 5..].trim_start_matches(['"', '\'']);
    let value: String = rest
        .chars()
        .take_while(|c| *c != '"' && *c != '\'' && *c != ']')
        .collect();
    if let Some((count, _)) = value.split_once('*') {
        return count.trim().parse().ok();
    }
    Some(value.split([',', ';']).count())
}

/// `.Title` before a block.
fn is_block_title(line: &str) -> bool {
    line.starts_with('.')
        && line[1..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace() && c != '.')
}

fn is_break(line: &str) -> bool {
    matches!(
        line.trim_end(),
        "'''" | "<<<" | "---" | "***" | "* * *" | "- - -"
    )
}

/// Lines that start a block the facade shows as markup.
fn is_markup_line(line: &str) -> bool {
    // A comment line, an attribute entry, a block macro (`image::x[]`).
    if line.starts_with("//") {
        return true;
    }
    if let Some(entry) = line.strip_prefix(':') {
        if let Some(end) = entry.find(':') {
            let name = &entry[..end];
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '!'))
            {
                return true;
            }
        }
    }
    if let Some(colons) = line.find("::") {
        let name = &line[..colons];
        let after = &line[colons + 2..];
        // `image::photo.png[]`, `include::part.adoc[]`, `toc::[]`.
        if !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && line.ends_with(']')
            && !after.starts_with(' ')
        {
            return true;
        }
        // A description list: `Term:: definition`.
        if after.is_empty() || after.starts_with(' ') {
            return true;
        }
    }
    false
}

fn markup_block(source: &str, span: Range<usize>) -> Block {
    let text = source[span.clone()].trim_end_matches('\n');
    Block {
        kind: BlockKind::Markup,
        runs: vec![identity_run(text, span.start, Style::default())],
        span,
    }
}

fn identity_run(text: &str, start: usize, style: Style) -> Run {
    Run {
        text: text.to_owned(),
        style,
        offsets: text.char_indices().map(|(i, _)| start + i).collect(),
        end: start + text.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "= Guide\nAda Lovelace\n:toc:\n\n== Getting *Started*\n\nSome *bold* and _italic_ and `code`, **un**constrained,\nsee https://x.org[the site] or link:Plan.adoc[the plan], <<setup,Setup>>.\nA bare https://example.org.\n\n* one\n** inner\n* [x] done\n\n. first\n. second\n\n[source,rust]\n----\nfn a() {}\n----\n\nNOTE: Mind the gap.\n\n____\nQuoted text\n____\n\n|===\n| a | b\n|===\n\n'''\n\n// a comment\n\nimage::shot.png[Shot]\n";

    fn kinds(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b.kind {
                BlockKind::Table(_) => "Table".to_owned(),
                _ => format!("{:?}", b.kind),
            })
            .collect()
    }

    /// ADR-003: the blocks of an AsciiDoc note as Asciidoctor reads them.
    #[test]
    fn splits_asciidoc_into_blocks() {
        let blocks = asciidoc_blocks(NOTE);
        assert_eq!(
            kinds(&blocks),
            [
                "Heading(1)",
                "Markup",
                "Heading(2)",
                "Paragraph",
                "ListItem { depth: 0, marker: Some(Bullet), checked: None }",
                "ListItem { depth: 1, marker: Some(Bullet), checked: None }",
                "ListItem { depth: 0, marker: Some(Bullet), checked: Some(true) }",
                "ListItem { depth: 0, marker: Some(Number(1)), checked: None }",
                "ListItem { depth: 0, marker: Some(Number(2)), checked: None }",
                "Code { language: \"rust\" }",
                "Admonition(\"NOTE\")",
                "Quote(1)",
                "Table",
                "ThematicBreak",
                "Markup",
                "Image { target: \"shot.png\", alt: \"Shot\", caption: None }",
            ]
        );
        assert_eq!(blocks[0].text(), "Guide");
        assert_eq!(blocks[1].text(), "Ada Lovelace\n:toc:");
        assert_eq!(blocks[2].text(), "Getting Started");
        assert_eq!(
            blocks[3].text(),
            "Some bold and italic and code, unconstrained, see the site or the plan, Setup. A bare https://example.org."
        );
        assert_eq!(blocks[6].text(), "done");
        assert_eq!(blocks[9].text(), "fn a() {}");
        assert_eq!(blocks[10].text(), "Mind the gap.");
        assert_eq!(blocks[11].text(), "Quoted text");
        assert!(blocks[12].text().starts_with("|==="));
    }

    #[test]
    fn inline_styles_and_links() {
        let blocks = asciidoc_blocks(NOTE);
        let style_of = |block: &Block, text: &str| {
            block
                .runs
                .iter()
                .find(|r| r.text.contains(text))
                .unwrap_or_else(|| panic!("{text}"))
                .style
                .clone()
        };
        let p = &blocks[3];
        assert!(style_of(p, "bold").bold);
        assert!(style_of(p, "italic").italic);
        assert!(style_of(p, "code").code);
        assert!(style_of(p, "un").bold);
        assert_eq!(
            style_of(p, "the site").link.as_deref(),
            Some("https://x.org")
        );
        assert_eq!(style_of(p, "the plan").link.as_deref(), Some("Plan.adoc"));
        assert_eq!(style_of(p, "Setup").link.as_deref(), Some("#setup"));
        assert_eq!(
            style_of(p, "example").link.as_deref(),
            Some("https://example.org")
        );
        assert!(style_of(&blocks[2], "Started").bold);
    }

    /// Every shown character maps to the source character it stands for.
    #[test]
    fn facade_characters_map_to_their_source() {
        let blocks = asciidoc_blocks(NOTE);
        for block in &blocks {
            let mut last = None;
            for (i, c) in block.text().chars().enumerate() {
                let offset = block.source_offset(i);
                assert!(last < Some(offset), "{:?} char {i} goes back", block.kind);
                last = Some(offset);
                let source_char = NOTE[offset..].chars().next().unwrap();
                let fits = source_char == c || (c == ' ' && source_char == '\n');
                assert!(fits, "{:?} char {i} {c:?} is {source_char:?}", block.kind);
                assert_eq!(block.facade_index(offset), Some(i), "{:?} {i}", block.kind);
            }
        }
    }

    #[test]
    fn constrained_marks_need_word_boundaries() {
        let text = |s: &str| asciidoc_blocks(s)[0].text();
        assert_eq!(text("snake_case_name\n"), "snake_case_name");
        assert_eq!(text("2 * 3 * 4\n"), "2 * 3 * 4");
        assert_eq!(text("a \\*not bold*\n"), "a *not bold*");
        assert_eq!(text("line one +\nline two\n"), "line one line two");
        let runs = &asciidoc_blocks("*a*b\n")[0].runs;
        assert!(runs.iter().all(|r| !r.style.bold));
    }

    #[test]
    fn lists_number_and_continue() {
        let blocks = asciidoc_blocks(". a\n.. b\n. c\n+\nmore about c\n\n3. d\n");
        assert_eq!(
            kinds(&blocks),
            [
                "ListItem { depth: 0, marker: Some(Number(1)), checked: None }",
                "ListItem { depth: 1, marker: Some(Number(1)), checked: None }",
                "ListItem { depth: 0, marker: Some(Number(2)), checked: None }",
                "ListItem { depth: 0, marker: None, checked: None }",
                "ListItem { depth: 0, marker: Some(Number(3)), checked: None }",
            ]
        );
        assert_eq!(blocks[3].text(), "more about c");
    }

    #[test]
    fn admonition_and_quote_blocks() {
        let blocks =
            asciidoc_blocks("[WARNING]\n====\nCareful.\n====\n\n[quote, Ada]\nAll of it.\n\n....\nraw *text*\n....\n");
        assert_eq!(
            kinds(&blocks),
            [
                "Admonition(\"WARNING\")",
                "Quote(1)",
                "Code { language: \"\" }"
            ]
        );
        assert_eq!(blocks[2].text(), "raw *text*");
        // The delimiters belong to the code block.
        assert_eq!(blocks[2].span.end, blocks[2].span.start + 21);
    }

    /// FR-036, FR-037: diagram blocks are code blocks in their language.
    #[test]
    fn diagram_blocks_name_their_language() {
        let blocks = asciidoc_blocks(
            "[plantuml, flow, svg]\n----\nA -> B\n----\n\n[mermaid]\n....\ngraph TD\n....\n",
        );
        assert_eq!(
            kinds(&blocks),
            [
                "Code { language: \"plantuml\" }",
                "Code { language: \"mermaid\" }"
            ]
        );
        assert_eq!(blocks[0].text(), "A -> B");
    }

    /// TC-004-02: a block image with its title as caption.
    #[test]
    fn block_images_with_captions() {
        let note =
            ".The architecture\nimage::img/arch.png[Architecture,600]\n\nimage::shot.png[]\n";
        let blocks = asciidoc_blocks(note);
        assert_eq!(
            kinds(&blocks),
            [
                "Image { target: \"img/arch.png\", alt: \"Architecture\", caption: Some(\"The architecture\") }",
                "Image { target: \"shot.png\", alt: \"shot\", caption: None }",
            ]
        );
        // The block is its markup, title included, for editing.
        assert_eq!(
            blocks[0].text(),
            ".The architecture\nimage::img/arch.png[Architecture,600]"
        );
    }

    /// UC-004 step 3, TC-004-02: AsciiDoc tables with header, caption,
    /// cells over several lines, and formatting in cells.
    #[test]
    fn asciidoc_tables() {
        let cell_text = |t: &Table| -> Vec<Vec<String>> {
            t.rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|c| c.iter().map(|x| x.text.as_str()).collect())
                        .collect()
                })
                .collect()
        };
        // Implicit header: the first line holds its cells, a blank line follows.
        let note = ".Sizes\n|===\n| Name | Size\n\n| *a* | 1\n| b | 22\n|===\n";
        let blocks = asciidoc_blocks(note);
        let BlockKind::Table(table) = &blocks[0].kind else {
            panic!("{:?}", blocks[0].kind);
        };
        assert!(table.header);
        assert_eq!(table.caption.as_deref(), Some("Sizes"));
        assert_eq!(
            cell_text(table),
            [["Name", "Size"], ["a", "1"], ["b", "22"]]
        );
        assert!(table.rows[1][0][0].style.bold);
        assert_eq!(blocks[0].text(), note.trim_end());

        // Cells one per line, columns from `cols`, header from the options.
        let note = "[cols=\"1,2\", options=\"header\"]\n|===\n|Key\n|Meaning\n\n|Ctrl+B\n|Bold,\nalso in the source\n|===\n";
        let BlockKind::Table(table) = &asciidoc_blocks(note)[0].kind else {
            panic!();
        };
        assert!(table.header);
        assert_eq!(
            cell_text(table),
            [["Key", "Meaning"], ["Ctrl+B", "Bold, also in the source"]]
        );
        // Without a blank line after the first line, there is no header.
        let BlockKind::Table(table) = &asciidoc_blocks("|===\n|a |b\n|c |d\n|===\n")[0].kind else {
            panic!();
        };
        assert!(!table.header);
        assert_eq!(cell_text(table), [["a", "b"], ["c", "d"]]);
    }

    #[test]
    fn unused_attribute_lines_are_markup() {
        let blocks = asciidoc_blocks("[#intro.lead]\n.A title\nText here.\n");
        assert_eq!(kinds(&blocks), ["Markup", "Paragraph"]);
        assert_eq!(blocks[0].text(), "[#intro.lead]\n.A title");
    }

    #[test]
    fn unclosed_blocks_run_to_the_end() {
        let blocks = asciidoc_blocks("----\ncode\nmore");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text(), "code\nmore");
    }
}

#[cfg(test)]
mod benchmark {
    /// ADR-012: parsing a note of 10,000 lines again stays within a frame
    /// (`cargo test --release -p underline-facade -- --ignored`).
    #[test]
    #[ignore]
    fn adr012_asciidoc_parse_of_10000_lines() {
        let mut note = String::from("= Big\n\n");
        let mut n = 0;
        while note.lines().count() < 10_000 {
            n += 1;
            note.push_str(&format!("== Section {n}\n\nSome _prose_ with *bold* and https://x.org/{n}[a link].\n\n* item one\n* item two\n\n[source,rust]\n----\nfn f{n}() -> u32 {{ {n} }}\n----\n\n"));
        }
        let mut times = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            let blocks = super::asciidoc_blocks(&note);
            times.push(start.elapsed());
            assert!(blocks.len() > 3000);
        }
        times.sort();
        println!(
            "asciidoc parse of 10,000 lines: median {:?}, max {:?}",
            times[10], times[19]
        );
    }
}
