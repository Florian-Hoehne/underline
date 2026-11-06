//! The blocks of a note for the facade (ADR-003, ADR-012): each block keeps
//! its source span, and each character the facade shows keeps the source
//! position it stands for, so the facade can edit the markup directly and
//! leave every other block unchanged (NFR-007).

use std::ops::Range;

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{parse_document, Arena, Options};
use underline_core::NoteFormat;

/// How a piece of text looks in the facade.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// The address of the link the text belongs to.
    pub link: Option<String>,
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
    /// An image on its own (UC-004 step 3): where it is, its alternative
    /// text, and its caption. Its runs are its markup, edited in place
    /// below the picture.
    Image {
        target: String,
        alt: String,
        caption: Option<String>,
    },
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
    match format {
        NoteFormat::Markdown => markdown_blocks(source),
        NoteFormat::AsciiDoc => crate::asciidoc::asciidoc_blocks(source),
    }
}

fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
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
            let text = source[inner.clone()].trim_end_matches('\n').to_owned();
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

/// A block shown as its markup, one to one.
fn markup_block(source: &str, span: Range<usize>) -> Block {
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
            let inner = raw.find(code.literal.as_str()).map_or(start, |i| start + i);
            runs.push(aligned_run(
                &code.literal,
                &source[inner..(inner + code.literal.len()).min(source.len())],
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
        _ => runs.push(aligned_run(raw, raw, start, style.clone())),
    }
}

/// A run of `text` whose characters are found in order in `raw`, which
/// starts at source offset `start`; backslash escapes and entities in the
/// source stand for one character each.
fn aligned_run(text: &str, raw: &str, start: usize, style: Style) -> Run {
    let raw_chars: Vec<(usize, char)> = raw.char_indices().collect();
    let mut offsets = Vec::with_capacity(text.chars().count());
    let mut at = 0;
    for c in text.chars() {
        // Skip what stands before the character in the source: a backslash
        // of an escape, or the rest of an entity.
        while at < raw_chars.len() && raw_chars[at].1 != c {
            at += 1;
        }
        let offset = raw_chars
            .get(at)
            .map_or(start + raw.len(), |(i, _)| start + i);
        offsets.push(offset);
        at += 1;
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

/// Byte offsets of the starts of the lines of a source.
struct LineStarts(Vec<usize>);

impl LineStarts {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
        Self(starts)
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
        blocks.iter().map(|b| format!("{:?}", b.kind)).collect()
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
                "Markup",
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
}
