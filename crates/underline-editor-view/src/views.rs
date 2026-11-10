//! M-09, ADR-024, ADR-025: the views of the facade's blocks as the window
//! draws them ([`BlockView`]), made from the blocks of the facade engine
//! (`underline_markup::blocks`), with the place in the markup each shown
//! character stands for.
//!
//! What a block shows besides its text comes from an [`Enrich`]: the
//! colours of code, the pictures of diagrams and images. In the backend
//! they come from its highlighting, diagrams and files; in the webview from
//! what the backend sent ([`CachedViews`](crate::CachedViews)). Everything
//! else (kinds, runs, offsets, prefixes, captions) is made here, so both
//! make the same views.
//!
//! Moved here from `underline-session` (`facade/build.rs`; plan section 17,
//! `docs/migration/variante-b.md` step B3b).

use ropey::Rope;
use underline_editor_core::sync::Fnv1a;
use underline_markup::blocks::{Block, BlockKind, CellAlign, ListMarker, Run};
use underline_markup::diagram::DiagramLanguage;

use crate::editor_view::units;
use crate::facade_view::{
    BlockKindView, BlockView, CellAlignName, CodeColors, FigureView, RunView, TableView,
};

/// Turns byte offsets of the markup into web offsets (`utf16`), walking
/// forward from the last one asked: the blocks are converted in order, so
/// the text is walked about once. A `\r` of a `\r\n` pair counts nothing,
/// so the pair is one unit, as in the window.
///
/// The text may be a part of the note, starting at byte `origin`, whose web
/// offset is `origin_web`; offsets asked are those of the whole note.
#[derive(Debug)]
pub struct WebWalker<'a> {
    text: &'a str,
    origin: usize,
    origin_web: usize,
    /// Where the walk stands in `text`, and its web offset from there.
    byte: usize,
    web: usize,
    /// Where the block being converted starts; going back starts there.
    mark: (usize, usize),
}

impl<'a> WebWalker<'a> {
    /// A walker over the whole note `text`.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self::part(text, 0, 0)
    }

    /// A walker over `text`, the part of the note from byte `origin` on,
    /// whose web offset is `origin_web`.
    #[must_use]
    pub fn part(text: &'a str, origin: usize, origin_web: usize) -> Self {
        Self {
            text,
            origin,
            origin_web,
            byte: 0,
            web: 0,
            mark: (0, 0),
        }
    }

    /// The web offset of byte offset `byte`; inside a char, of its start.
    fn at(&mut self, byte: usize) -> usize {
        let mut byte = byte.saturating_sub(self.origin).min(self.text.len());
        while !self.text.is_char_boundary(byte) {
            byte -= 1;
        }
        if byte < self.byte {
            (self.byte, self.web) = if byte >= self.mark.0 {
                self.mark
            } else {
                (0, 0)
            };
        }
        let bytes = self.text.as_bytes();
        for (i, c) in self.text[self.byte..byte].char_indices() {
            let at = self.byte + i;
            let pair = c == '\r' && bytes.get(at + 1) == Some(&b'\n');
            if !pair {
                self.web += c.len_utf16();
            }
        }
        self.byte = byte;
        self.origin_web + self.web
    }

    /// Going back from now on starts at `byte`, the start of a block.
    fn mark(&mut self, byte: usize) -> usize {
        let web = self.at(byte);
        self.mark = (self.byte, self.web);
        web
    }

    /// Whether the char at `byte` is the `\r` of a `\r\n` pair.
    fn pair_at(&self, byte: usize) -> bool {
        let Some(at) = byte.checked_sub(self.origin) else {
            return false;
        };
        let bytes = self.text.as_bytes();
        bytes.get(at) == Some(&b'\r') && bytes.get(at + 1) == Some(&b'\n')
    }
}

/// The view of `run`, whose offsets are relative to `base`, the web offset
/// of the block's start.
fn run_view(run: &Run, walker: &mut WebWalker<'_>, base: usize) -> RunView {
    let mut text = String::with_capacity(run.text.len());
    let mut offsets = Vec::new();
    let mut unit = 0;
    let mut next: Option<usize> = None;
    for (c, &source) in run.text.chars().zip(&run.offsets) {
        // The `\r` of a pair is shown with its `\n`.
        if c == '\r' && walker.pair_at(source) {
            continue;
        }
        let shown = if c == '\r' { '\n' } else { c };
        let web = walker.at(source).saturating_sub(base);
        if next != Some(web) {
            offsets.push((units(unit), units(web)));
        }
        let length = shown.len_utf16();
        text.push(shown);
        unit += length;
        next = Some(web + length);
    }
    let end = walker.at(run.end).saturating_sub(base);
    RunView {
        text,
        bold: run.style.bold,
        italic: run.style.italic,
        strike: run.style.strike,
        code: run.style.code,
        link: run.style.link.clone(),
        offsets,
        end: units(end),
    }
}

/// M-09.2: what stands before the text of a block (`prefix_text` of the
/// egui facade).
fn prefix_text(kind: &BlockKind) -> String {
    let mut prefix = String::new();
    match kind {
        BlockKind::ListItem {
            depth,
            marker,
            checked,
        } => {
            prefix.push_str(&"    ".repeat(*depth));
            match marker {
                Some(ListMarker::Bullet) => prefix.push_str("•  "),
                Some(ListMarker::Number(n)) => prefix.push_str(&format!("{n}.  ")),
                None => prefix.push_str("   "),
            }
            match checked {
                Some(true) => prefix.push_str("☑ "),
                Some(false) => prefix.push_str("☐ "),
                None => {}
            }
        }
        BlockKind::Quote(depth) => prefix.push_str(&"    ".repeat(*depth)),
        BlockKind::Admonition(label) => {
            prefix.push_str("    ");
            prefix.push_str(label);
            prefix.push_str("  ");
        }
        _ => {}
    }
    prefix
}

/// The diagram language of a code block, if it is a diagram.
#[must_use]
pub fn diagram_language(kind: &BlockKind) -> Option<DiagramLanguage> {
    match kind {
        BlockKind::Code { language } => DiagramLanguage::from_code_language(language),
        _ => None,
    }
}

/// A figure that only says something.
#[must_use]
pub fn message_figure(message: String, failed: bool) -> FigureView {
    FigureView {
        message: Some(message),
        failed,
        ..FigureView::default()
    }
}

/// A figure showing `picture`.
#[must_use]
pub fn picture_figure(picture: String) -> FigureView {
    FigureView {
        picture: Some(picture),
        ..FigureView::default()
    }
}

/// M-09.7: what a diagram in `language` shows while it renders and no
/// earlier picture of it is shown.
#[must_use]
pub fn rendering_figure(language: DiagramLanguage) -> FigureView {
    message_figure(format!("Rendering the {} diagram…", language.name()), false)
}

/// M-09.4, ADR-025: the key the colours of `code` in `language` are kept
/// by, in the theme named `theme`: FNV-1a as 16 hex digits, the same in the
/// backend and in the webview, and exact in JavaScript
/// (`docs/migration/variante-b.md` section 2.3).
#[must_use]
pub fn colors_key(theme: &str, language: &str, code: &str) -> String {
    Fnv1a::new().field(theme).field(language).field(code).hex()
}

/// M-09.7, ADR-025: the hash of the diagram `code` in `language` on a dark
/// or a light theme; the backend names the diagram's picture by it
/// (`diagram/{hash:016x}.svg`).
#[must_use]
pub fn diagram_hash(language: DiagramLanguage, code: &str, dark: bool) -> u64 {
    Fnv1a::new()
        .field(language.name())
        .field(code)
        .field(if dark { "dark" } else { "light" })
        .finish()
}

/// M-09.7, ADR-025: the key the picture of a diagram is kept by:
/// [`diagram_hash`] as 16 hex digits, the name of its picture.
#[must_use]
pub fn diagram_key(language: DiagramLanguage, code: &str, dark: bool) -> String {
    format!("{:016x}", diagram_hash(language, code, dark))
}

/// ADR-025: what a block shows besides its text: the colours of code and
/// the pictures of diagrams and images. The backend makes them from its
/// highlighting, diagrams and files; the webview takes what the backend
/// sent.
pub trait Enrich {
    /// M-09.4: the colours of the code block with `code` in `language`;
    /// `None` without colours (yet).
    fn colors(&mut self, language: &str, code: &str) -> Option<CodeColors>;

    /// M-09.7: the figure of diagram block `index` of `count` blocks, with
    /// `code` in `language`.
    fn diagram(
        &mut self,
        index: usize,
        count: usize,
        language: DiagramLanguage,
        code: &str,
    ) -> FigureView;

    /// M-09.6: the figure of the image `target` described by `alt`, without
    /// its caption.
    fn image(&mut self, target: &str, alt: &str) -> FigureView;
}

/// Makes the views of blocks, in order, with what `enrich` adds to them.
#[derive(Debug)]
pub struct ViewMaker<'a, E: ?Sized> {
    enrich: &'a mut E,
    blocks: &'a [Block],
    /// The figures and tables with a caption so far, for their numbers.
    figures: usize,
    tables: usize,
}

impl<'a, E: Enrich + ?Sized> ViewMaker<'a, E> {
    /// A maker of the views of `blocks`.
    pub fn new(enrich: &'a mut E, blocks: &'a [Block]) -> Self {
        Self {
            enrich,
            blocks,
            figures: 0,
            tables: 0,
        }
    }

    /// The views of all blocks of `text`, the whole note in `rope`.
    pub fn views(&mut self, text: &str, rope: &Rope) -> Vec<BlockView> {
        let mut walker = WebWalker::new(text);
        (0..self.blocks.len())
            .map(|index| self.view(index, &mut walker, rope))
            .collect()
    }

    /// The views of the blocks `range`; `walker` walks a part of the note
    /// holding them. Figures and tables are numbered after those before.
    pub fn views_of_range(
        &mut self,
        range: std::ops::Range<usize>,
        walker: &mut WebWalker<'_>,
        rope: &Rope,
    ) -> Vec<BlockView> {
        for block in &self.blocks[..range.start] {
            match &block.kind {
                BlockKind::Table(table) if table.caption.is_some() => self.tables += 1,
                BlockKind::Image { caption, .. } if caption.is_some() => self.figures += 1,
                _ => {}
            }
        }
        range.map(|index| self.view(index, walker, rope)).collect()
    }

    /// The view of block `index`.
    fn view(&mut self, index: usize, walker: &mut WebWalker<'_>, rope: &Rope) -> BlockView {
        let block = &self.blocks[index];
        let len = rope.len_bytes();
        let base = walker.mark(block.span.start);
        let runs: Vec<RunView> = block
            .runs
            .iter()
            .map(|run| run_view(run, walker, base))
            .collect();
        let mut figure = None;
        let mut colors = None;
        let kind = match &block.kind {
            BlockKind::Heading(level) => BlockKindView::Heading { level: *level },
            BlockKind::Paragraph => BlockKindView::Paragraph,
            BlockKind::ListItem { depth, .. } => BlockKindView::ListItem {
                depth: units(*depth),
            },
            BlockKind::Quote(depth) => BlockKindView::Quote {
                depth: units(*depth),
            },
            BlockKind::Code { language } => {
                let code: String = runs.iter().map(|r| r.text.as_str()).collect();
                if let Some(diagram) = diagram_language(&block.kind) {
                    figure = Some(
                        self.enrich
                            .diagram(index, self.blocks.len(), diagram, &code),
                    );
                } else {
                    colors = self.enrich.colors(language, &code);
                }
                BlockKindView::Code {
                    language: language.clone(),
                }
            }
            BlockKind::ThematicBreak => BlockKindView::ThematicBreak,
            BlockKind::Admonition(label) => BlockKindView::Admonition {
                label: label.clone(),
            },
            BlockKind::Markup => BlockKindView::Markup,
            BlockKind::Table(table) => {
                // TC-004-02: numbered as Asciidoctor numbers tables.
                let caption = table.caption.as_ref().map(|caption| {
                    self.tables += 1;
                    format!("Table {}. {caption}", self.tables)
                });
                BlockKindView::Table {
                    table: TableView {
                        header: table.header,
                        align: table
                            .align
                            .iter()
                            .map(|align| match align {
                                CellAlign::Left => CellAlignName::Left,
                                CellAlign::Center => CellAlignName::Center,
                                CellAlign::Right => CellAlignName::Right,
                            })
                            .collect(),
                        rows: table
                            .rows
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .map(|cell| {
                                        cell.iter().map(|run| run_view(run, walker, base)).collect()
                                    })
                                    .collect()
                            })
                            .collect(),
                        caption,
                    },
                }
            }
            BlockKind::Image {
                target,
                alt,
                caption,
            } => {
                let mut image = self.enrich.image(target, alt);
                // TC-004-02: numbered as Asciidoctor numbers figures.
                image.caption = caption.as_ref().map(|caption| {
                    self.figures += 1;
                    format!("Figure {}. {caption}", self.figures)
                });
                figure = Some(image);
                BlockKindView::Image {
                    target: target.clone(),
                    alt: alt.clone(),
                }
            }
        };
        let end = walker.at(block.span.end);
        let first = rope.byte_to_line(block.span.start.min(len));
        let last = rope.byte_to_line(
            block
                .span
                .end
                .saturating_sub(1)
                .max(block.span.start)
                .min(len),
        );
        BlockView {
            kind,
            start: units(base),
            line: units(first),
            length: units(end.saturating_sub(base)),
            lines: units(last - first + 1),
            prefix: prefix_text(&block.kind),
            runs,
            colors,
            figure,
        }
    }
}

/// The UTF-16 units of the text of `view`'s runs.
fn units_of(view: &BlockView) -> Vec<u16> {
    view.runs
        .iter()
        .flat_map(|run| run.text.encode_utf16())
        .collect()
}

/// M-09.4, ADR-024: the colours of a code block whose text changed from
/// `old` to `new`, carried over until it is coloured anew: the colours
/// before the change stay, those after it move with the text
/// ([`carry_spans`](underline_editor_core::facade::carry_spans)).
#[must_use]
pub fn carry_colors(old: &BlockView, new: &BlockView) -> Option<CodeColors> {
    let colors = old.colors.as_ref()?;
    Some(CodeColors {
        spans: underline_editor_core::facade::carry_spans(
            &units_of(old),
            &units_of(new),
            &colors.spans,
        ),
        styles: colors.styles.clone(),
    })
}
