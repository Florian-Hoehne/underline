//! The blocks of a note for the facade (ADR-003, ADR-012): each block keeps
//! its source span, and each character the facade shows keeps the source
//! position it stands for, so the facade can edit the markup directly and
//! leave every other block unchanged (NFR-007).

use std::ops::Range;

use crate::NoteFormat;
use comrak::nodes::{AstNode, ListType, NodeValue, TableAlignment};
use comrak::{parse_document, Arena, Options};

/// How a piece of text looks in the facade.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// The address of the link the text belongs to.
    pub link: Option<String>,
    /// FR-098, UC-004 BR-019: the text is the markup of an inline formula,
    /// which the facade shows typeset.
    pub formula: Option<Formula>,
}

/// FR-098, UC-004 BR-019: a formula of a note: its source in the notation
/// of LaTeX, and whether it is set as a display formula (`$$…$$` within a
/// paragraph) or in the line (`$…$`, `latexmath:[…]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Formula {
    /// The formula in LaTeX, without its delimiters.
    pub source: String,
    /// Set as a display formula, in display style.
    pub display: bool,
}

/// A run of text in one style; `offsets[i]` is the source byte offset of
/// character `i`, and `end` the source offset right after the run.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Run {
    pub text: String,
    pub style: Style,
    pub offsets: Vec<usize>,
    pub end: usize,
}

/// What a block is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Heading(u8),
    Paragraph,
    /// A paragraph of a list item: its nesting depth (0 for the outer list),
    /// its marker (`None` for further paragraphs of an item), and its check
    /// mark for task items.
    ListItem {
        depth: usize,
        marker: Option<ListMarker>,
        checked: Option<bool>,
    },
    /// A paragraph inside a quote, with the depth of quotes.
    Quote(usize),
    /// A fenced or indented code block; its runs are the code, one to one.
    Code {
        language: String,
    },
    ThematicBreak,
    /// A paragraph set apart with a label: `NOTE`, `TIP`, `IMPORTANT`,
    /// `WARNING` or `CAUTION`.
    Admonition(String),
    /// A block the facade shows and edits as its markup (UC-005 BR-008).
    Markup,
    /// A table (UC-004 step 3); its runs are its markup, edited in place
    /// below the formatted table (UC-005 BR-008).
    Table(Box<Table>),
    /// An image on its own (UC-004 step 3): where it is, its alternative
    /// text, and its caption. Its runs are its markup, edited in place
    /// below the picture.
    Image {
        target: String,
        alt: String,
        caption: Option<String>,
    },
    /// FR-098, UC-004 BR-019: a formula on its own, set as a display
    /// formula and centred: a paragraph holding only `$$…$$`, a code block
    /// of the language `math`, or an AsciiDoc `[latexmath]` block. Its
    /// runs are its markup, edited in place below the typeset formula
    /// (UC-005 BR-008).
    Formula {
        /// The formula's source, without its delimiters.
        source: String,
    },
}

/// How the text of a table column is aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CellAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// The content of a table: its rows of cells, each cell its styled text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Table {
    /// The first row is the header.
    pub header: bool,
    /// The alignment of each column.
    pub align: Vec<CellAlign>,
    pub rows: Vec<Vec<Vec<Run>>>,
    /// The title of an AsciiDoc table, shown as its caption.
    pub caption: Option<String>,
    /// UC-024 BR-001, OQ-216: the front matter of a Markdown note, shown as
    /// GitHub shows it: its names as the header, its values below.
    pub front_matter: bool,
}

impl Table {
    /// The number of columns: that of the widest row.
    pub fn columns(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ListMarker {
    Bullet,
    Number(usize),
}

/// One block of the facade.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    pub kind: BlockKind,
    /// The source bytes of the whole block, markers included.
    pub span: Range<usize>,
    /// What the facade shows, in order.
    pub runs: Vec<Run>,
}

impl Block {
    /// The text the facade shows for this block.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// The source offset of facade character `index` of this block; the
    /// position after the last character for `index == text().chars().count()`.
    pub fn source_offset(&self, index: usize) -> usize {
        let mut rest = index;
        for run in &self.runs {
            let count = run.offsets.len();
            if rest < count {
                return run.offsets[rest];
            }
            rest -= count;
        }
        self.runs.last().map_or(self.span.start, |r| r.end)
    }

    /// The facade character index nearest to source offset `offset`, or
    /// `None` when the offset lies outside the block's span.
    pub fn facade_index(&self, offset: usize) -> Option<usize> {
        if offset < self.span.start || offset > self.span.end {
            return None;
        }
        let mut index = 0;
        for run in &self.runs {
            for &at in &run.offsets {
                if at >= offset {
                    return Some(index);
                }
                index += 1;
            }
            if offset <= run.end {
                return Some(index);
            }
        }
        Some(index)
    }
}

/// Takes note of `raw` inserted at source offset `at` for facade text `text`
/// in block `index`, before facade character `facade_at`, without parsing
/// again (ADR-012): the characters join the run they were typed into, and
/// every source position after `at` moves by the length of `raw`.
///
/// Only valid for an insertion that cannot change the structure of the
/// blocks: no line break, and formatting characters escaped. Returns `false`
/// when the insertion does not fit the block; then the caller parses again.
pub fn insert_typed(
    blocks: &mut [Block],
    index: usize,
    facade_at: usize,
    at: usize,
    text: &str,
    raw: &str,
) -> bool {
    let Some(block) = blocks.get(index) else {
        return false;
    };
    if text.contains('\n') || block.source_offset(facade_at) != at || block.runs.is_empty() {
        return false;
    }
    // Source offsets of the new characters: escapes stand before them.
    let mut new_offsets = Vec::new();
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '\\' {
            if let Some(&(j, next)) = chars.peek() {
                if next.is_ascii_punctuation() {
                    new_offsets.push(at + j);
                    chars.next();
                    continue;
                }
            }
        }
        new_offsets.push(at + i);
    }
    if new_offsets.len() != text.chars().count() {
        return false;
    }
    let shift = raw.len();
    for block in blocks.iter_mut() {
        if block.span.start >= at && !(block.span.start == at && block.span.start == block.span.end)
        {
            block.span.start += shift;
        }
        if block.span.end >= at {
            block.span.end += shift;
        }
        for run in &mut block.runs {
            for offset in &mut run.offsets {
                if *offset >= at {
                    *offset += shift;
                }
            }
            if run.end >= at {
                run.end += shift;
            }
        }
    }
    // The run that holds facade character `facade_at`, or the last one.
    let block = &mut blocks[index];
    let span_start = block.span.start;
    if span_start > at {
        block.span.start = at;
    }
    let mut rest = facade_at;
    let count = block.runs.len();
    for (i, run) in block.runs.iter_mut().enumerate() {
        let len = run.offsets.len();
        if rest < len || (rest == len && i + 1 == count) {
            let byte = run
                .text
                .char_indices()
                .nth(rest)
                .map_or(run.text.len(), |(b, _)| b);
            run.text.insert_str(byte, text);
            run.offsets.splice(rest..rest, new_offsets);
            if rest == len {
                run.end = at + shift;
            }
            return true;
        }
        rest -= len;
    }
    false
}

/// The address of the link at source offset `at`, if a link is there
/// (UC-004 A2).
pub fn link_at(blocks: &[Block], at: usize) -> Option<String> {
    blocks
        .iter()
        .filter(|b| b.span.start <= at && at <= b.span.end)
        .flat_map(|b| &b.runs)
        .find(|run| {
            run.style.link.is_some()
                && run.offsets.first().is_some_and(|first| *first <= at)
                && at <= run.end
        })
        .and_then(|run| run.style.link.clone())
}

/// The source offset of the heading whose anchor is `anchor` (UC-004 A2).
pub fn heading_offset(
    blocks: &[Block],
    anchor: &str,
    slug: impl Fn(&str) -> String,
) -> Option<usize> {
    blocks
        .iter()
        .find(|b| matches!(b.kind, BlockKind::Heading(_)) && slug(&b.text()) == anchor)
        .map(|b| b.source_offset(0))
}

/// Splits `source` into the blocks of the facade.
pub fn parse_blocks(source: &str, format: NoteFormat) -> Vec<Block> {
    parse_region(source, format, &Region::note(source, format))
}

/// FR-098, OQ-222: the notation the header of an AsciiDoc note gives
/// `stem:[…]` and `[stem]` blocks by its attribute `stem`, as Asciidoctor
/// reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Stem {
    /// No `stem` in the header: `stem:[…]` and `[stem]` stay markup.
    #[default]
    Off,
    /// `:stem: latexmath` (or `latex`, `tex`): LaTeX.
    LatexMath,
    /// `:stem:` without a value, `:stem: asciimath`, or any other value:
    /// AsciiMath, written as LaTeX to be typeset (`crate::asciimath`).
    AsciiMath,
}

/// What a part of a note parsed on its own needs to know of the rest of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Region {
    /// The part is the start of the note, where a Markdown note may have
    /// its front matter (UC-024 BR-001), so a part parsed again on its own
    /// is not taken for one.
    pub at_start: bool,
    /// FR-098, OQ-222: the notation the header of the AsciiDoc note sets
    /// with `:stem:`, so `stem:[…]` and `[stem]` blocks are formulas in it.
    pub stem: Stem,
}

impl Region {
    /// The whole note `source` in `format`.
    #[must_use]
    pub fn note(source: &str, format: NoteFormat) -> Self {
        Self {
            at_start: true,
            stem: Self::stem_of(source, format),
        }
    }

    /// A part of the note `note` after its start.
    #[must_use]
    pub fn inside(note: &str, format: NoteFormat) -> Self {
        Self {
            at_start: false,
            stem: Self::stem_of(note, format),
        }
    }

    fn stem_of(note: &str, format: NoteFormat) -> Stem {
        if format == NoteFormat::AsciiDoc {
            crate::asciidoc::stem_notation(note)
        } else {
            Stem::Off
        }
    }
}

/// The blocks of `source`, a part of a note in `format`, as `region` says
/// it stands in the note.
#[must_use]
pub fn parse_region(source: &str, format: NoteFormat, region: &Region) -> Vec<Block> {
    match format {
        NoteFormat::Markdown if region.at_start => markdown_with_front_matter(source),
        NoteFormat::Markdown => markdown_blocks(source),
        NoteFormat::AsciiDoc => crate::asciidoc::asciidoc_blocks(source, region.stem),
    }
}

/// UC-024 BR-001, OQ-216: a Markdown note whose front matter is shown as a
/// table, as GitHub shows it, or as its markup when it cannot be read (A5).
/// The rest is parsed with the front matter's bytes blanked, so every place
/// stays where it is.
fn markdown_with_front_matter(source: &str) -> Vec<Block> {
    let end = crate::statistics::front_matter_end(source);
    if end == 0 {
        return markdown_blocks(source);
    }
    let start = crate::attributes::bom_len(source);
    let blanked: String = source[..end]
        .char_indices()
        .map(|(at, c)| {
            if at < start || c == '\n' || c == '\r' {
                c.to_string()
            } else {
                " ".repeat(c.len_utf8())
            }
        })
        .collect::<String>()
        + &source[end..];
    let mut blocks = vec![front_matter_block(source, start..end)];
    blocks.extend(markdown_blocks(&blanked));
    blocks
}

/// The block of the front matter spanning `span`.
fn front_matter_block(source: &str, span: Range<usize>) -> Block {
    let mut block = markup_block(source, span);
    let head = crate::attributes::note_head(source, NoteFormat::Markdown);
    if head.problem.is_some() || head.attributes.is_empty() {
        return block;
    }
    let cell = |text: &str, at: usize| {
        vec![Run {
            text: text.to_owned(),
            style: Style::default(),
            offsets: vec![at; text.chars().count()],
            end: at,
        }]
    };
    let names = head
        .attributes
        .iter()
        .map(|a| cell(&a.name, a.span.start))
        .collect();
    // UC-005 BR-023: a value shown as it is written (bare, in quotes or in
    // brackets) keeps the place of each character, so the spell check marks
    // its words in the facade as in the source view.
    let value_cell = |a: &crate::attributes::NoteAttribute| {
        let raw = source.get(a.value_span.clone()).unwrap_or_default();
        let inner = raw
            .get(1..raw.len().saturating_sub(1))
            .filter(|_| raw.len() >= 2 && raw.is_char_boundary(1));
        let at = if raw == a.value {
            Some(a.value_span.start)
        } else if inner == Some(a.value.as_str()) {
            Some(a.value_span.start + 1)
        } else {
            None
        };
        match at {
            Some(at) => vec![Run {
                text: a.value.clone(),
                style: Style::default(),
                offsets: a.value.char_indices().map(|(i, _)| at + i).collect(),
                end: at + a.value.len(),
            }],
            None => cell(&a.value, a.value_span.start),
        }
    };
    let values = head.attributes.iter().map(value_cell).collect();
    block.kind = BlockKind::Table(Box::new(Table {
        header: true,
        align: vec![CellAlign::Left; head.attributes.len()],
        rows: vec![names, values],
        caption: None,
        front_matter: true,
    }));
    block
}

fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    // FR-098, ADR-029: formulas as GitHub reads them: `$…$`, `$$…$$`,
    // `` $`…`$ `` and code blocks of the language `math`.
    options.extension.math_dollars = true;
    options.extension.math_code = true;
    options
}

fn markdown_blocks(source: &str) -> Vec<Block> {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &markdown_options());
    let lines = LineStarts::new(source);
    let mut blocks = Vec::new();
    for node in root.children() {
        collect(node, source, &lines, &mut blocks, Context::default());
    }
    blocks
}

#[derive(Clone, Copy, Default)]
struct Context {
    list_depth: Option<usize>,
    quote: usize,
}

fn collect<'a>(
    node: &'a AstNode<'a>,
    source: &str,
    lines: &LineStarts,
    blocks: &mut Vec<Block>,
    context: Context,
) {
    let data = node.data.borrow();
    let span = lines.block_span(data.sourcepos.start.line, data.sourcepos.end.line, source);
    match &data.value {
        NodeValue::Heading(heading) => blocks.push(Block {
            kind: BlockKind::Heading(heading.level),
            span,
            runs: inline_runs(node, source, lines),
        }),
        NodeValue::Paragraph if context.list_depth.is_none() && context.quote == 0 => {
            if let Some(image) = only_image(node) {
                let mut block = markup_block(source, span);
                block.kind = image;
                blocks.push(block);
            } else if let Some(formula) = only_display_formula(node) {
                let mut block = markup_block(source, span);
                block.kind = formula;
                blocks.push(block);
            } else {
                blocks.push(Block {
                    kind: BlockKind::Paragraph,
                    span,
                    runs: inline_runs(node, source, lines),
                });
            }
        }
        NodeValue::Paragraph => {
            let kind = match (context.list_depth, context.quote) {
                (Some(depth), _) => BlockKind::ListItem {
                    depth,
                    marker: None,
                    checked: None,
                },
                (None, quote) if quote > 0 => BlockKind::Quote(quote),
                _ => BlockKind::Paragraph,
            };
            blocks.push(Block {
                kind,
                span,
                runs: inline_runs(node, source, lines),
            });
        }
        NodeValue::List(list) => {
            let depth = context.list_depth.map_or(0, |d| d + 1);
            for (number, item) in (list.start..).zip(node.children()) {
                let marker = match list.list_type {
                    ListType::Bullet => ListMarker::Bullet,
                    ListType::Ordered => ListMarker::Number(number),
                };
                let checked = match &item.data.borrow().value {
                    NodeValue::TaskItem(mark) => Some(mark.symbol.is_some()),
                    _ => None,
                };
                let first = blocks.len();
                for child in item.children() {
                    collect(
                        child,
                        source,
                        lines,
                        blocks,
                        Context {
                            list_depth: Some(depth),
                            ..context
                        },
                    );
                }
                // UC-005: an item still empty (just made by Enter) is a
                // block of its own, so it shows and holds the cursor.
                if blocks.len() == first {
                    let pos = item.data.borrow().sourcepos;
                    let span = lines.block_span(pos.start.line, pos.end.line, source);
                    // Its text, still empty, starts after the marker.
                    let line = source[span.clone()].lines().next().unwrap_or("");
                    let at = span.start + line.len();
                    blocks.push(Block {
                        kind: BlockKind::ListItem {
                            depth,
                            marker: None,
                            checked: None,
                        },
                        span,
                        runs: vec![identity_run("", at, at, Style::default())],
                    });
                }
                // The first paragraph of the item carries its marker.
                if let Some(Block {
                    kind:
                        BlockKind::ListItem {
                            marker: m,
                            checked: c,
                            depth: d,
                        },
                    ..
                }) = blocks.get_mut(first)
                {
                    if *d == depth {
                        *m = Some(marker);
                        *c = checked;
                    }
                }
            }
        }
        NodeValue::BlockQuote => {
            for child in node.children() {
                collect(
                    child,
                    source,
                    lines,
                    blocks,
                    Context {
                        quote: context.quote + 1,
                        ..context
                    },
                );
            }
        }
        NodeValue::CodeBlock(code) if code.fenced && is_math_language(&code.info) => {
            // FR-098: a code block of the language `math` is a formula.
            let mut block = markup_block(source, span);
            block.kind = BlockKind::Formula {
                source: code.literal.trim_end_matches('\n').to_owned(),
            };
            blocks.push(block);
        }
        NodeValue::CodeBlock(code) => {
            // The code lines between the fences, one to one.
            let inner = if code.fenced {
                let first_line = data.sourcepos.start.line + 1;
                let last_line = data.sourcepos.end.line.saturating_sub(1);
                if last_line >= first_line {
                    lines.block_span(first_line, last_line, source)
                } else {
                    let at = lines.line_end(data.sourcepos.start.line, source);
                    at..at
                }
            } else {
                span.clone()
            };
            // Only the break ending the last line goes; an empty last line
            // (just made by Enter) stays, so it shows and holds the cursor.
            let inner_text = &source[inner.clone()];
            let text = inner_text
                .strip_suffix('\n')
                .unwrap_or(inner_text)
                .to_owned();
            let end = inner.start + text.len();
            blocks.push(Block {
                kind: BlockKind::Code {
                    language: code.info.split_whitespace().next().unwrap_or("").to_owned(),
                },
                span,
                runs: vec![identity_run(
                    &text,
                    inner.start,
                    end,
                    Style {
                        code: true,
                        ..Style::default()
                    },
                )],
            });
        }
        NodeValue::Table(table) => {
            let align = table
                .alignments
                .iter()
                .map(|a| match a {
                    TableAlignment::Center => CellAlign::Center,
                    TableAlignment::Right => CellAlign::Right,
                    _ => CellAlign::Left,
                })
                .collect();
            let mut header = false;
            let rows = node
                .children()
                .map(|row| {
                    if let NodeValue::TableRow(true) = row.data.borrow().value {
                        header = true;
                    }
                    let row_line = row.data.borrow().sourcepos.start.line;
                    let starts = cell_starts(lines.line_text(row_line, source));
                    row.children()
                        .enumerate()
                        .map(|(at, cell)| {
                            let mut runs = inline_runs(cell, source, lines);
                            // comrak may place the cells of a body row
                            // where those of the header row are (a header
                            // starting with a space): the cell's text
                            // starts where the row's line says.
                            let given = runs.iter().find_map(|run| run.offsets.first());
                            if let (Some(Some(start)), Some(&given)) = (starts.get(at), given) {
                                let line = lines.line_text(row_line, source);
                                // The first character shown follows a
                                // backslash that escapes it.
                                let start = start + usize::from(line[*start..].starts_with('\\'));
                                let wanted = lines.offset(row_line, 1) + start;
                                shift_all(&mut runs, signed(wanted) - signed(given));
                            }
                            runs
                        })
                        .collect()
                })
                .collect();
            let mut block = markup_block(source, span);
            block.kind = BlockKind::Table(Box::new(Table {
                header,
                align,
                rows,
                caption: None,
                front_matter: false,
            }));
            blocks.push(block);
        }
        NodeValue::ThematicBreak => blocks.push(Block {
            kind: BlockKind::ThematicBreak,
            span,
            runs: Vec::new(),
        }),
        _ => blocks.push(markup_block(source, span)),
    }
}

/// The image a paragraph holds when it holds nothing else.
fn only_image<'a>(paragraph: &'a AstNode<'a>) -> Option<BlockKind> {
    let mut children = paragraph
        .children()
        .filter(|c| !matches!(&c.data.borrow().value, NodeValue::Text(t) if t.trim().is_empty()));
    let image = children.next()?;
    if children.next().is_some() {
        return None;
    }
    let data = image.data.borrow();
    let NodeValue::Image(link) = &data.value else {
        return None;
    };
    let mut alt = String::new();
    for node in image.descendants() {
        if let NodeValue::Text(text) = &node.data.borrow().value {
            alt.push_str(text);
        }
    }
    Some(BlockKind::Image {
        target: link.url.clone(),
        alt,
        caption: None,
    })
}

/// FR-098: whether the info string of a fenced code block names the
/// language `math`, as GitHub reads it.
fn is_math_language(info: &str) -> bool {
    info.split_whitespace().next() == Some("math")
}

/// FR-098, ADR-029: the block formula a paragraph is when it holds nothing
/// but one display formula (`$$…$$`).
fn only_display_formula<'a>(paragraph: &'a AstNode<'a>) -> Option<BlockKind> {
    let mut children = paragraph.children().filter(|c| {
        !matches!(&c.data.borrow().value, NodeValue::Text(t) if t.trim().is_empty())
            && !matches!(
                &c.data.borrow().value,
                NodeValue::SoftBreak | NodeValue::LineBreak
            )
    });
    let formula = children.next()?;
    if children.next().is_some() {
        return None;
    }
    let data = formula.data.borrow();
    match &data.value {
        NodeValue::Math(math) if math.display_math => Some(BlockKind::Formula {
            source: math.literal.trim().to_owned(),
        }),
        _ => None,
    }
}

/// A block shown as its markup, one to one.
pub(crate) fn markup_block(source: &str, span: Range<usize>) -> Block {
    let text = source[span.clone()].trim_end_matches('\n').to_owned();
    let end = span.start + text.len();
    Block {
        kind: BlockKind::Markup,
        runs: vec![identity_run(&text, span.start, end, Style::default())],
        span,
    }
}

fn identity_run(text: &str, start: usize, end: usize, style: Style) -> Run {
    let offsets = text.char_indices().map(|(i, _)| start + i).collect();
    Run {
        text: text.to_owned(),
        style,
        offsets,
        end,
    }
}

/// The inline content of a block as styled runs with source offsets.
fn inline_runs<'a>(node: &'a AstNode<'a>, source: &str, lines: &LineStarts) -> Vec<Run> {
    let mut runs = Vec::new();
    for child in node.children() {
        inline(child, source, lines, &Style::default(), &mut runs);
    }
    runs
}

fn inline<'a>(
    node: &'a AstNode<'a>,
    source: &str,
    lines: &LineStarts,
    style: &Style,
    runs: &mut Vec<Run>,
) {
    let data = node.data.borrow();
    let start = lines.offset(data.sourcepos.start.line, data.sourcepos.start.column);
    let end = lines.offset(data.sourcepos.end.line, data.sourcepos.end.column) + 1;
    let raw = source.get(start..end.min(source.len())).unwrap_or("");
    let mut nested = |style: Style| {
        for child in node.children() {
            inline(child, source, lines, &style, runs);
        }
    };
    match &data.value {
        NodeValue::Text(text) => runs.push(aligned_run(text, raw, start, style.clone())),
        NodeValue::Code(code) => {
            let literal = code_literal(&code.literal);
            let inner = raw.find(literal.as_str()).map_or(start, |i| start + i);
            runs.push(aligned_run(
                &literal,
                // A literal not found as it stands (an inline code span
                // whose spaces were stripped) may end inside a character.
                source
                    .get(inner..(inner + literal.len()).min(source.len()))
                    .unwrap_or(""),
                inner,
                Style {
                    code: true,
                    ..style.clone()
                },
            ));
        }
        NodeValue::SoftBreak | NodeValue::LineBreak => runs.push(Run {
            text: " ".into(),
            style: style.clone(),
            offsets: vec![start],
            end: start + 1,
        }),
        NodeValue::Emph => nested(Style {
            italic: true,
            ..style.clone()
        }),
        NodeValue::Strong => nested(Style {
            bold: true,
            ..style.clone()
        }),
        NodeValue::Strikethrough => nested(Style {
            strike: true,
            ..style.clone()
        }),
        NodeValue::Link(link) | NodeValue::Image(link) => nested(Style {
            link: Some(link.url.clone()),
            ..style.clone()
        }),
        // FR-098: an inline formula shows its markup, one to one, until the
        // facade typesets it.
        NodeValue::Math(math) => {
            let (from, to) = math_markup(source, start, end.min(source.len()), math);
            runs.push(identity_run(
                &source[from..to],
                from,
                to,
                Style {
                    formula: Some(Formula {
                        source: math.literal.trim().to_owned(),
                        display: math.display_math,
                    }),
                    ..style.clone()
                },
            ));
        }
        _ => runs.push(aligned_run(raw, raw, start, style.clone())),
    }
}

/// FR-098: the bytes of the markup of the formula `math` whose place comrak
/// gives as `start..end`, found in `source` itself: comrak's places lie
/// short over an indented continuation line, and in a table cell an
/// escaped pipe moves them, so the delimiters are looked for from `start`
/// on, as comrak reads them.
fn math_markup(
    source: &str,
    start: usize,
    end: usize,
    math: &comrak::nodes::NodeMath,
) -> (usize, usize) {
    let (open, close) = match (math.dollar_math, math.display_math) {
        (false, _) => ("$`", "`$"),
        (true, true) => ("$$", "$$"),
        (true, false) => ("$", "$"),
    };
    let mut from = start.min(source.len());
    while !source.is_char_boundary(from) {
        from -= 1;
    }
    // Where comrak says it ends, at a char boundary, when the delimiters are
    // not found as comrak reads them.
    let fallback = |from: usize| {
        let mut to = end.clamp(from, source.len());
        while !source.is_char_boundary(to) {
            to += 1;
        }
        (from, to)
    };
    let Some(found) = source[from..].find(open) else {
        return fallback(from);
    };
    let from = from + found;
    let body = from + open.len();
    let inline_dollar = math.dollar_math && !math.display_math;
    let rest = &source[body..];
    let mut chars = rest.char_indices().peekable();
    let mut previous: Option<char> = None;
    while let Some((at, c)) = chars.next() {
        if c == '\\' {
            previous = chars.next().map(|(_, n)| n);
            continue;
        }
        if inline_dollar {
            let next = chars.peek().map(|(_, n)| *n);
            if c == '$'
                && previous.is_some_and(|p| !p.is_whitespace())
                && !next.is_some_and(|n| n.is_ascii_digit())
            {
                return (from, body + at + 1);
            }
        } else if rest[at..].starts_with(close) && (at > 0 || close == "$$") {
            return (from, body + at + close.len());
        }
        previous = Some(c);
    }
    fallback(from)
}

/// A run of `text` whose characters are found in order in `raw`, which
/// starts at source offset `start`; backslash escapes and entities in the
/// source stand for one character each.
fn aligned_run(text: &str, raw: &str, start: usize, style: Style) -> Run {
    let raw_chars: Vec<(usize, char)> = raw.char_indices().collect();
    let mut offsets = Vec::with_capacity(text.chars().count());
    let mut at = 0;
    for c in text.chars() {
        // An entity (`&copy;`) stands for its character: the character's
        // place is the `&`, and the source goes on after the `;`.
        if let Some(&(i, '&')) = raw_chars.get(at) {
            let close = raw_chars[at..]
                .iter()
                .take(40)
                .position(|(_, ch)| *ch == ';')
                .filter(|&n| n > 1 && c != '&');
            if let Some(n) = close {
                offsets.push(start + i);
                at += n + 1;
                continue;
            }
        }
        // Skip what stands before the character in the source: a backslash
        // of an escape. A character not found keeps the place before it,
        // so the characters after it are still found.
        match raw_chars[at.min(raw_chars.len())..]
            .iter()
            .position(|(_, ch)| *ch == c)
        {
            Some(n) => {
                at += n;
                offsets.push(start + raw_chars[at].0);
                at += 1;
            }
            None => offsets.push(
                offsets
                    .last()
                    .copied()
                    .unwrap_or(start)
                    .min(start + raw.len()),
            ),
        }
    }
    let end = raw_chars
        .get(at.saturating_sub(1))
        .map_or(start + raw.len(), |(i, ch)| start + i + ch.len_utf8());
    Run {
        text: text.to_owned(),
        style,
        offsets,
        end,
    }
}

/// The text of an inline code span as CommonMark shows it: comrak turns
/// only the `\n` of a line break into a space and keeps the `\r` of a
/// `\r\n` (OQ-261: a note with `\r\n` shows what it shows with `\n`), and
/// then strips one space at both ends only when both are spaces.
fn code_literal(literal: &str) -> String {
    if !literal.contains('\r') {
        return literal.to_owned();
    }
    let spaced = literal.replace("\r ", " ").replace('\r', " ");
    let stripped = spaced
        .strip_prefix(' ')
        .and_then(|rest| rest.strip_suffix(' '))
        .filter(|inner| !inner.is_empty() && !spaced.trim().is_empty());
    stripped.map_or(spaced.clone(), str::to_owned)
}

/// Where the text of each cell of a table row's `line` starts (bytes into
/// the line), `None` for an empty cell: the line split at `|` that no
/// backslash escapes, without the outer pipes.
fn cell_starts(line: &str) -> Vec<Option<usize>> {
    let mut cells = Vec::new();
    let mut from = 0;
    let mut escaped = false;
    for (at, c) in line.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '|' => {
                cells.push(from..at);
                from = at + 1;
            }
            _ => {}
        }
    }
    cells.push(from..line.len());
    if line.trim_start().starts_with('|') && cells.len() > 1 {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && cells.len() > 1 {
        cells.pop();
    }
    cells
        .into_iter()
        .map(|cell| {
            let text = &line[cell.clone()];
            // Only spaces and tabs part a cell from its pipe: a character
            // such as U+2028 belongs to the cell's text.
            let blank = text.len() - text.trim_start_matches([' ', '\t']).len();
            (blank < text.len()).then_some(cell.start + blank)
        })
        .collect()
}

/// `value` as a signed number (offsets in a note are far below `isize::MAX`).
fn signed(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

/// Moves every source place of `runs` by `by` bytes.
fn shift_all(runs: &mut [Run], by: isize) {
    if by == 0 {
        return;
    }
    for run in runs {
        for offset in &mut run.offsets {
            *offset = offset.saturating_add_signed(by);
        }
        run.end = run.end.saturating_add_signed(by);
    }
}

/// Byte offsets of the starts of the lines of a source. A line ends with
/// `\n`, `\r\n` or a lone `\r`, as comrak counts lines (UC-005 BR-023:
/// a note of another system's line breaks keeps its places).
struct LineStarts(Vec<usize>);

impl LineStarts {
    fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut starts = vec![0];
        for (i, &b) in bytes.iter().enumerate() {
            let lone_cr = b == b'\r' && bytes.get(i + 1) != Some(&b'\n');
            if b == b'\n' || lone_cr {
                starts.push(i + 1);
            }
        }
        Self(starts)
    }

    /// The text of 1-based `line`, without its line break.
    fn line_text<'s>(&self, line: usize, source: &'s str) -> &'s str {
        let start = self.offset(line, 1).min(source.len());
        let rest = &source[start..];
        let end = rest.find(['\n', '\r']).unwrap_or(rest.len());
        &rest[..end]
    }

    /// Byte offset of 1-based `line` and 1-based byte `column`.
    fn offset(&self, line: usize, column: usize) -> usize {
        let line_start = self.0.get(line.saturating_sub(1)).copied().unwrap_or(0);
        line_start + column.saturating_sub(1)
    }

    fn line_end(&self, line: usize, source: &str) -> usize {
        self.0.get(line).copied().unwrap_or(source.len())
    }

    /// Whole lines `first..=last` (1-based), with their line endings.
    fn block_span(&self, first: usize, last: usize, source: &str) -> Range<usize> {
        let start = self
            .0
            .get(first.saturating_sub(1))
            .copied()
            .unwrap_or(source.len());
        start..self.line_end(last.max(first), source).max(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "# Hi *there*\n\nSome **bold \\* x** and [a link](http://x) ~~gone~~.\nnext line\n\n- one\n- [x] two\n  1. inner\n\n> quoted\n\n```rust\nfn a() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n";

    fn kinds(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b.kind {
                BlockKind::Table(_) => "Table".to_owned(),
                _ => format!("{:?}", b.kind),
            })
            .collect()
    }

    #[test]
    fn splits_markdown_into_blocks() {
        let blocks = parse_blocks(NOTE, NoteFormat::Markdown);
        assert_eq!(
            kinds(&blocks),
            [
                "Heading(1)",
                "Paragraph",
                "ListItem { depth: 0, marker: Some(Bullet), checked: None }",
                "ListItem { depth: 0, marker: Some(Bullet), checked: Some(true) }",
                "ListItem { depth: 1, marker: Some(Number(1)), checked: None }",
                "Quote(1)",
                "Code { language: \"rust\" }",
                "Table",
                "ThematicBreak",
            ]
        );
        assert_eq!(blocks[0].text(), "Hi there");
        assert_eq!(blocks[1].text(), "Some bold * x and a link gone. next line");
        assert_eq!(blocks[6].text(), "fn a() {}");
        assert!(blocks[7].text().starts_with("| a | b |"));
    }

    /// Every shown character maps to the source character it stands for.
    #[test]
    fn facade_characters_map_to_their_source() {
        let blocks = parse_blocks(NOTE, NoteFormat::Markdown);
        for block in &blocks {
            for (i, c) in block.text().chars().enumerate() {
                let offset = block.source_offset(i);
                let source_char = NOTE[offset..].chars().next().unwrap();
                let fits = source_char == c || (c == ' ' && source_char == '\n');
                assert!(
                    fits,
                    "{:?} char {i} {c:?} maps to {source_char:?}",
                    block.kind
                );
                assert_eq!(
                    block.facade_index(offset),
                    Some(i),
                    "{:?} char {i}",
                    block.kind
                );
            }
        }
        // The escaped star maps to the star, not to the backslash.
        let bold = &blocks[1];
        let star = bold.text().find('*').unwrap();
        assert_eq!(
            &NOTE[bold.source_offset(star)..bold.source_offset(star) + 1],
            "*"
        );
        // After the last character comes the end of its run.
        let heading = &blocks[0];
        assert_eq!(heading.source_offset(8), NOTE.find("*\n").unwrap());
    }

    /// ADR-012: typed text joins its run without parsing again, with the
    /// same blocks a new parse gives.
    #[test]
    fn typed_text_matches_a_new_parse() {
        // A position the facade cannot reach is refused; the caller parses.
        let note = "Some *it*, more.\n";
        let mut blocks = parse_blocks(note, NoteFormat::Markdown);
        let inside = note.find("*,").unwrap();
        assert!(!insert_typed(&mut blocks, 0, 7, inside, "abc", "abc"));

        let cases = [
            ("Some *it*, more.\n\n- item\n", "Some *i", "abc", "abc"),
            ("Some *it*, more.\n\n- item\n", "Some *it*", " x", " x"),
            ("# Head\n\nPara\n", "# He", "2\\*3", "2*3"),
            ("- item\n- two\n\nEnd\n", "- item", "s ü", "s ü"),
            (
                "```rust\nfn a() {}\n```\n\nEnd\n",
                "fn a(",
                "x: u8",
                "x: u8",
            ),
        ];
        for (note, after, raw, text) in cases {
            let at = note.find(after).unwrap() + after.len();
            let mut blocks = parse_blocks(note, NoteFormat::Markdown);
            let index = blocks
                .iter()
                .position(|b| b.span.contains(&at) || b.span.end == at)
                .unwrap();
            let facade_at = blocks[index].facade_index(at).unwrap();
            assert!(
                insert_typed(&mut blocks, index, facade_at, at, text, raw),
                "{note:?}"
            );
            let mut edited = note.to_owned();
            edited.insert_str(at, raw);
            let fresh = parse_blocks(&edited, NoteFormat::Markdown);
            for (mine, theirs) in blocks.iter().zip(&fresh) {
                assert_eq!(mine.text(), theirs.text(), "{edited:?}");
                assert_eq!(mine.kind, theirs.kind);
                for i in 0..=mine.text().chars().count() {
                    assert_eq!(
                        mine.source_offset(i),
                        theirs.source_offset(i),
                        "{edited:?} char {i}"
                    );
                }
            }
            assert_eq!(blocks.len(), fresh.len());
        }
    }

    /// UC-004 step 3: a paragraph that is only an image is an image block;
    /// an image inside text stays in the text.
    #[test]
    fn images_on_their_own_are_image_blocks() {
        let note = "![The plan](img/plan.png)\n\nSee ![icon](i.png) here.\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);
        assert_eq!(
            blocks[0].kind,
            BlockKind::Image {
                target: "img/plan.png".into(),
                alt: "The plan".into(),
                caption: None
            }
        );
        assert_eq!(blocks[0].text(), "![The plan](img/plan.png)");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
    }

    /// UC-004 step 3: a table with its header, alignment, and formatted
    /// cells; its text is its markup, for editing.
    #[test]
    fn markdown_tables() {
        let note = "| Name | Size |\n|:---|---:|\n| **a** | 1 |\n| [b](x.md) | 22 |\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);
        let BlockKind::Table(table) = &blocks[0].kind else {
            panic!("{:?}", blocks[0].kind);
        };
        assert!(table.header);
        assert_eq!(table.align, [CellAlign::Left, CellAlign::Right]);
        assert_eq!(table.columns(), 2);
        let text = |cell: &Vec<Run>| cell.iter().map(|r| r.text.as_str()).collect::<String>();
        let rows: Vec<Vec<String>> = table
            .rows
            .iter()
            .map(|r| r.iter().map(text).collect())
            .collect();
        assert_eq!(rows, [["Name", "Size"], ["a", "1"], ["b", "22"]]);
        assert!(table.rows[1][0][0].style.bold);
        assert_eq!(table.rows[2][0][0].style.link.as_deref(), Some("x.md"));
        assert_eq!(blocks[0].text(), note.trim_end());
    }

    #[test]
    fn finds_links_and_headings() {
        let note = "# Getting Started\n\nSee [the plan](Plan.md) now.\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);
        let inside = note.find("plan]").unwrap();
        assert_eq!(link_at(&blocks, inside).as_deref(), Some("Plan.md"));
        assert_eq!(link_at(&blocks, note.find("now").unwrap()), None);
        let slug = |t: &str| t.to_lowercase().replace(' ', "-");
        assert_eq!(heading_offset(&blocks, "getting-started", slug), Some(2));
    }

    /// UC-005: an empty list item and an empty last code line, as Enter
    /// makes them, are kept, so the facade can show the cursor there.
    #[test]
    fn uc005_empty_items_and_code_lines_are_kept() {
        let empty_item = |note: &str, format| {
            let blocks = parse_blocks(note, format);
            let items: Vec<_> = blocks
                .iter()
                .filter(|b| matches!(b.kind, BlockKind::ListItem { .. }))
                .map(|b| (b.text(), b.kind.clone()))
                .collect();
            assert_eq!(items.len(), 3, "{note:?}: {blocks:?}");
            assert!(
                items.iter().any(|(text, kind)| text.is_empty()
                    && matches!(
                        kind,
                        BlockKind::ListItem {
                            marker: Some(_),
                            ..
                        }
                    )),
                "{note:?}: {items:?}"
            );
        };
        empty_item("- a\n- b\n- \n", NoteFormat::Markdown);
        empty_item("- a\n- \n- b\n", NoteFormat::Markdown);
        empty_item("* a\n* b\n* \n", NoteFormat::AsciiDoc);

        let code = |note: &str| parse_blocks(note, NoteFormat::Markdown)[0].text();
        assert_eq!(code("```\na\n```\n"), "a");
        assert_eq!(code("```\na\n\n```\n"), "a\n");
        assert_eq!(code("```\n\n```\n"), "");
    }

    /// UC-004 BR-020 (test monkey): the cells of a body row keep their own
    /// source places when the header row starts with a space.
    #[test]
    fn table_cells_keep_their_places_after_an_indented_header() {
        let source = " a|b\n-|-\nb|c\n";
        let blocks = parse_blocks(source, NoteFormat::Markdown);
        let BlockKind::Table(table) = &blocks[0].kind else {
            panic!("a table: {blocks:?}");
        };
        let first = |row: usize, cell: usize| table.rows[row][cell][0].offsets[0];
        assert_eq!((first(0, 0), first(0, 1)), (1, 3));
        assert_eq!((first(1, 0), first(1, 1)), (9, 11));
        let escaped = "| \\|x | y |\n|---|---|\n| \\|z | w |\n";
        let blocks = parse_blocks(escaped, NoteFormat::Markdown);
        let BlockKind::Table(table) = &blocks[0].kind else {
            panic!("a table: {blocks:?}");
        };
        let cell = &table.rows[1][0][0];
        assert_eq!(&escaped[cell.offsets[0]..cell.offsets[0] + 1], "|");
    }

    /// OQ-261 (test monkey): an inline code span over a `\r\n` shows what
    /// it shows over a `\n`.
    #[test]
    fn code_spans_over_crlf_show_as_over_lf() {
        let shown = |source: &str| parse_blocks(source, NoteFormat::Markdown)[0].text();
        assert_eq!(shown("a `b\r\nc` d\r\n"), shown("a `b\nc` d\n"));
        assert_eq!(shown(")```\r\n) ```"), "))");
    }

    /// TC-024-01 step 2, OQ-216: the front matter is a table of its names
    /// and values, as GitHub shows it; the blocks after it keep their
    /// places, also after a byte order mark and multibyte values.
    #[test]
    fn tc024_01_front_matter_as_a_table() {
        let source = "\u{feff}---\nstatus: Grüße\ndue: 2026-12-01\n---\n# Plan\n";
        let blocks = parse_blocks(source, NoteFormat::Markdown);
        let BlockKind::Table(table) = &blocks[0].kind else {
            panic!("a table: {blocks:?}");
        };
        assert!(table.front_matter && table.header);
        let cells: Vec<Vec<String>> = table
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell[0].text.clone()).collect())
            .collect();
        assert_eq!(cells, [["status", "due"], ["Grüße", "2026-12-01"]]);
        assert_eq!(blocks[0].span.start, 3);
        assert_eq!(blocks[1].kind, BlockKind::Heading(1));
        assert_eq!(&source[blocks[1].span.clone()], "# Plan\n");
        assert_eq!(blocks[1].text(), "Plan");
    }

    /// UC-005 BR-023: a lone `\r` ends a line, as comrak reads it, so
    /// every shown character keeps its place in the markup.
    #[test]
    fn br023_a_lone_carriage_return_ends_a_line() {
        for source in [
            "wrnga\rwrngb wrngc",
            "a\r- wrnga wrngb\r",
            "\rwrnga",
            "- a\r- wrnga",
        ] {
            for block in parse_blocks(source, NoteFormat::Markdown) {
                for run in &block.runs {
                    for (c, &at) in run.text.chars().zip(&run.offsets) {
                        assert!(
                            block.span.contains(&at),
                            "{source:?}: {at} outside {:?}",
                            block.span
                        );
                        if c != ' ' {
                            assert_eq!(source[at..].chars().next(), Some(c), "{source:?}");
                        }
                    }
                }
            }
        }
    }

    /// TC-024-08, UC-024 A5: a front matter that cannot be read is shown as
    /// its markup; one not at the start of a note, or parsed as a part of
    /// it, is no front matter.
    #[test]
    fn uc024_a5_front_matter_shown_as_markup() {
        let broken = "---\nstatus: draft\njust text\n---\nBody\n";
        let blocks = parse_blocks(broken, NoteFormat::Markdown);
        assert_eq!(blocks[0].kind, BlockKind::Markup);
        assert_eq!(blocks[0].text(), "---\nstatus: draft\njust text\n---");
        assert_eq!(blocks[1].text(), "Body");
        let later = parse_region("---\na: b\n---\n", NoteFormat::Markdown, &Region::default());
        assert!(!matches!(&later[0].kind, BlockKind::Table(table) if table.front_matter));
        let crlf = parse_blocks("---\r\na: b\r\n---\r\nText\r\n", NoteFormat::Markdown);
        assert!(matches!(&crlf[0].kind, BlockKind::Table(table) if table.front_matter));
        assert_eq!(crlf[1].text(), "Text");
    }

    /// The inline formulas of `block`: their markup and source, and whether
    /// each is a display formula.
    fn formulas(block: &Block) -> Vec<(String, String, bool)> {
        block
            .runs
            .iter()
            .filter_map(|run| {
                let formula = run.style.formula.as_ref()?;
                Some((run.text.clone(), formula.source.clone(), formula.display))
            })
            .collect()
    }

    /// TC-004-38, FR-098, UC-004 BR-019: inline formulas are read as GitHub
    /// reads them; a price stays text; block formulas are `$$…$$` on their
    /// own and code blocks of the language `math`.
    #[test]
    fn tc004_38_markdown_formulas_inline_and_as_blocks() {
        let note = "Energy $E=mc^2$ and price 5$ or 6$\n\n$$\\sum_{i=1}^n i$$\n\n```math\n\\sqrt{2}\n```\n\n$$\\frac{1}{$$\n";
        let blocks = parse_blocks(note, NoteFormat::Markdown);

        assert_eq!(
            formulas(&blocks[0]),
            [("$E=mc^2$".into(), "E=mc^2".into(), false)]
        );
        assert!(blocks[0].text().ends_with(" and price 5$ or 6$"));
        let sources: Vec<_> = blocks[1..].iter().map(|b| b.kind.clone()).collect();
        assert_eq!(
            sources,
            [
                BlockKind::Formula {
                    source: "\\sum_{i=1}^n i".into()
                },
                BlockKind::Formula {
                    source: "\\sqrt{2}".into()
                },
                BlockKind::Formula {
                    source: "\\frac{1}{".into()
                },
            ]
        );
        assert_eq!(blocks[2].text(), "```math\n\\sqrt{2}\n```", "its markup");
    }

    /// FR-098, ADR-029, C-003: comrak's rules for the dollar against
    /// GitHub's. Checked: an opening `$` is followed by no space, a closing
    /// one is preceded by no space and followed by no digit, so prices stay
    /// text; `$$` within a paragraph is a display formula in the line;
    /// an escaped `\$` is a dollar; `` $`…`$ `` is a formula. Comrak reads
    /// `$22 and $2+2$` as text and the formula `2+2` (OQ-282).
    #[test]
    fn fr098_dollar_rules_as_github_reads_them() {
        let first = |note: &str| parse_blocks(note, NoteFormat::Markdown).remove(0);
        assert_eq!(
            formulas(&first("$22 and $2+2$\n")),
            [("$2+2$".into(), "2+2".into(), false)]
        );
        assert!(formulas(&first("Costs 5$ or 6$.\n")).is_empty());
        assert!(formulas(&first("$x$1 and $ y$\n")).is_empty());
        let display = first("a $$x^2$$ b\n");
        assert_eq!(display.kind, BlockKind::Paragraph);
        assert_eq!(formulas(&display), [("$$x^2$$".into(), "x^2".into(), true)]);
        let escaped = first("\\$x$ and $y\\$\n");
        assert!(formulas(&escaped).is_empty());
        assert_eq!(escaped.text(), "$x$ and $y$");
        assert_eq!(
            formulas(&first("$`a+b`$ here\n")),
            [("$`a+b`$".into(), "a+b".into(), false)]
        );
        // `$$` over several lines on its own is a block formula.
        assert_eq!(
            first("$$\nx^2\n$$\n").kind,
            BlockKind::Formula {
                source: "x^2".into()
            }
        );
    }

    /// FR-098, UC-005 BR-008: the markup of an inline formula maps one to
    /// one to the source, so the cursor can go into it and edit it.
    #[test]
    fn fr098_formula_markup_maps_to_its_source() {
        let note = "A *$x$* and $$y$$.\n";
        let block = &parse_blocks(note, NoteFormat::Markdown)[0];
        let run = block.runs.iter().find(|r| r.text == "$x$").unwrap();
        assert!(run.style.italic, "formatting around it stays");
        assert_eq!(run.offsets, [3, 4, 5]);
        assert_eq!(run.end, 6);
        for (i, c) in block.text().chars().enumerate() {
            let at = block.source_offset(i);
            assert_eq!(note[at..].chars().next(), Some(c));
        }
    }

    /// The markup of every inline formula of `blocks` must be the source
    /// text its offsets name (the facade shows and edits that markup).
    fn assert_formula_markup_is_source(note: &str, blocks: &[Block]) {
        let mut runs: Vec<&Run> = blocks.iter().flat_map(|b| &b.runs).collect();
        for block in blocks {
            if let BlockKind::Table(table) = &block.kind {
                runs.extend(table.rows.iter().flatten().flatten());
            }
        }
        for run in runs.into_iter().filter(|r| r.style.formula.is_some()) {
            let first = *run.offsets.first().expect("a formula run has text");
            assert_eq!(
                note.get(first..run.end),
                Some(run.text.as_str()),
                "the markup of a formula is its source text: {note:?}"
            );
            assert!(
                run.text.starts_with('$') && run.text.ends_with('$'),
                "the markup of a formula keeps both its delimiters: {:?} in {note:?}",
                run.text
            );
        }
    }

    /// TC-004-38, FR-098, UC-005 BR-008 (found by the test monkey): a
    /// formula that starts a line of a paragraph and goes on over an
    /// indented line keeps all of its markup (comrak's end place of such a
    /// formula lies short, or inside a character).
    #[test]
    fn tc004_38_a_formula_over_an_indented_line_keeps_its_markup() {
        for note in ["a\n$x\n  y$ b\n", "a\n$x\n  é$ b\n", "a\n$x\n    y$ b\n"] {
            let blocks = parse_blocks(note, NoteFormat::Markdown);
            assert_formula_markup_is_source(note, &blocks);
        }
    }

    /// TC-004-38, FR-098 (found by the test monkey): in a table cell an
    /// escaped pipe `\|` (`$\|x\|$`, the norm) moves comrak's places by one,
    /// and the formula's markup lost its closing `$` or took a space.
    #[test]
    fn tc004_38_a_formula_in_a_table_cell_with_an_escaped_pipe() {
        for note in [
            "| a |\n|---|\n| $\\|x\\|$ |\n",
            "| a |\n|---|\n| \\| $x$ |\n",
            "| a |\n|---|\n| pre $a\\|b$ tail |\n",
        ] {
            let blocks = parse_blocks(note, NoteFormat::Markdown);
            assert_formula_markup_is_source(note, &blocks);
        }
    }
}
