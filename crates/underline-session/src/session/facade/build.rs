//! Making the views of a facade (M-09, ADR-024), off the session: a [`Job`]
//! owns what it needs (a snapshot of the text, the theme's colours, the
//! diagrams), runs on a thread of its own, and gives back the blocks and
//! their views ([`Built`]); the session takes them in only while they are
//! still for the text it has (`facade.rs`).
//!
//! Ported from `crates/underline-editor/src/facade_view.rs`: what a block
//! shows (`block_job`, `prefix_text`), the figures (`figure`,
//! `picture_state`, `image_source`) with their numbering, and the colours
//! of code blocks.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use ropey::Rope;
use underline_core::{resolve_link, LinkTarget, NoteFormat, Theme};
use underline_facade::blocks::{parse_blocks, Block, BlockKind, CellAlign, ListMarker, Run};
use underline_facade::diagram::{DiagramError, DiagramLanguage};
use underline_facade::highlight::{highlight_code, HighlightSet};
use underline_facade::images::ImageError;

use super::super::diagrams::{SvgDiagrams, SvgState};
use crate::assets::{check_file, file_asset_path, FileProblem};
use crate::editor_view::{units, SpanStyle};
use crate::facade_view::{
    BlockKindView, BlockView, CellAlignName, CodeColors, FigureView, RunView, TableView,
};

/// Code blocks whose colours are kept; more are coloured again.
const CODE_COLORS_KEPT: usize = 4096;
/// How long what is known of an image file is trusted.
const IMAGE_FRESH: Duration = Duration::from_secs(2);
/// What the image crate says of a file that is no image it knows; egui
/// shows the decoder's reason (`ImageError::Unreadable`).
const NOT_AN_IMAGE: &str = "The image format could not be determined";

/// The look the blocks were made in: the theme, and whether its colours
/// for code were ready.
pub(super) type Looks = (Theme, bool);

/// M-09.4: the colours of code blocks, by theme, language and code, kept by
/// the session, so switching the view does not colour them again.
pub(in crate::session) type CodeColorCache = HashMap<u64, Option<CodeColors>>;

/// M-09.6: what was found of the image files, by their place, and when.
pub(in crate::session) type ImageCache = HashMap<PathBuf, (Instant, Result<String, FileProblem>)>;

/// Everything making the views needs, owned, so it runs without the
/// session.
pub(in crate::session) struct Job {
    /// Counts the jobs of one facade; a later job wins.
    pub(super) seq: u64,
    pub(super) rev: u64,
    pub(super) looks: Looks,
    pub(super) generation: u64,
    /// A snapshot of the text (a rope is copied in no time).
    pub(super) rope: Rope,
    pub(super) format: NoteFormat,
    /// The blocks already parsed from this revision, if any.
    pub(super) blocks: Option<Arc<Vec<Block>>>,
    pub(super) root: PathBuf,
    pub(super) note: PathBuf,
    pub(super) colors: Option<Arc<HighlightSet>>,
    pub(super) dark: bool,
    pub(super) diagrams: Option<Arc<SvgDiagrams>>,
    pub(super) code_colors: Arc<Mutex<CodeColorCache>>,
    pub(super) images: Arc<Mutex<ImageCache>>,
    pub(super) last_pictures: HashMap<usize, String>,
}

/// What a [`Job`] made.
pub(in crate::session) struct Built {
    pub(super) seq: u64,
    pub(super) rev: u64,
    pub(super) looks: Looks,
    pub(super) generation: u64,
    pub(super) blocks: Arc<Vec<Block>>,
    pub(super) views: Vec<BlockView>,
    pub(super) last_pictures: HashMap<usize, String>,
}

/// Turns byte offsets of the markup into web offsets (`utf16`), walking
/// forward from the last one asked: the blocks are converted in order, so
/// the whole text is walked about once. A `\r` of a `\r\n` pair counts
/// nothing, so the pair is one unit, as in the window.
struct WebWalker<'a> {
    text: &'a str,
    byte: usize,
    web: usize,
    /// Where the block being converted starts; going back starts there.
    mark: (usize, usize),
}

impl<'a> WebWalker<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            byte: 0,
            web: 0,
            mark: (0, 0),
        }
    }

    /// The web offset of byte offset `byte`; inside a char, of its start.
    fn at(&mut self, byte: usize) -> usize {
        let mut byte = byte.min(self.text.len());
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
        self.web
    }

    /// Going back from now on starts at `byte`, the start of a block.
    fn mark(&mut self, byte: usize) -> usize {
        let web = self.at(byte);
        self.mark = (self.byte, web);
        web
    }

    /// Whether the char at `byte` is the `\r` of a `\r\n` pair.
    fn pair_at(&self, byte: usize) -> bool {
        let bytes = self.text.as_bytes();
        bytes.get(byte) == Some(&b'\r') && bytes.get(byte + 1) == Some(&b'\n')
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

/// `#rrggbb` of an RGBA colour.
fn hex([r, g, b, _]: [u8; 4]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// M-09.4, ADR-010: the colours of `code` in `language`, in UTF-16 units of
/// the code; the parts in the theme's plain colour have none.
fn code_colors(set: &HighlightSet, language: &str, code: &str) -> CodeColors {
    let [r, g, b] = set.palette().foreground;
    let plain = [r, g, b, 0xff];
    let mut styles: Vec<SpanStyle> = Vec::new();
    let mut spans = Vec::new();
    let mut line_start = 0;
    let lines = highlight_code(set, language, code);
    for (line, line_spans) in code.split_inclusive('\n').zip(&lines) {
        for span in line_spans {
            if span.color[..3] == plain[..3] || span.start >= span.end {
                continue;
            }
            // The facade colours code by colour only (`block_job`).
            let style = SpanStyle {
                color: hex(span.color),
                bold: false,
                italic: false,
            };
            let index = styles.iter().position(|s| *s == style).unwrap_or_else(|| {
                styles.push(style);
                styles.len() - 1
            });
            let start = line_start + underline_editor_core::utf16::byte_to_utf16(line, span.start);
            let end = line_start + underline_editor_core::utf16::byte_to_utf16(line, span.end);
            spans.push((units(start), units(end), units(index)));
        }
        line_start += underline_editor_core::utf16::len_utf16(line);
    }
    CodeColors { spans, styles }
}

/// The diagram language of a code block, if it is a diagram.
pub(super) fn diagram_language(kind: &BlockKind) -> Option<DiagramLanguage> {
    match kind {
        BlockKind::Code { language } => DiagramLanguage::from_code_language(language),
        _ => None,
    }
}

/// A figure that only says something.
pub(super) fn message_figure(message: String, failed: bool) -> FigureView {
    FigureView {
        message: Some(message),
        failed,
        ..FigureView::default()
    }
}

/// A figure showing `picture`.
fn picture_figure(picture: String) -> FigureView {
    FigureView {
        picture: Some(picture),
        ..FigureView::default()
    }
}

/// The protocol path of an image file below `root`, with the time of its
/// last change, so a changed file loads again; why it is not served
/// otherwise. Kept for [`IMAGE_FRESH`].
fn image_path(images: &Mutex<ImageCache>, root: &Path, file: &Path) -> Result<String, FileProblem> {
    let now = Instant::now();
    if let Some((checked, known)) = images
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(file)
    {
        if now.duration_since(*checked) < IMAGE_FRESH {
            return known.clone();
        }
    }
    let found = check_file(root, file).map(|(_, relative, meta)| {
        let changed = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_millis());
        format!("{}?v={changed}", file_asset_path(&relative))
    });
    let mut cache = images.lock().unwrap_or_else(PoisonError::into_inner);
    if cache.len() > 1024 {
        cache.clear();
    }
    cache.insert(file.to_owned(), (now, found.clone()));
    found
}

/// M-09.6, UC-004 BR-004, BR-005: the picture of the image `target`, found
/// as links are found (`image_source` and `picture_state` of the egui
/// facade). The window loads files only through `ul-asset`, so an image
/// outside the workspace or in a hidden folder is not shown (ADR-023), and
/// a web image is not loaded until the user's consent is settled (Q-19).
fn image_figure(job: &Job, target: &str, alt: &str) -> FigureView {
    let root = job.root.as_path();
    let file = match resolve_link(root, &job.note, target) {
        LinkTarget::Web(address) => {
            return message_figure(
                format!(
                    "The image {alt} cannot be shown: {}",
                    ImageError::NotLoaded(address)
                ),
                true,
            );
        }
        LinkTarget::File(path) => path,
        LinkTarget::Note { id, .. } => root.join(id.as_str()),
        LinkTarget::MissingNote { .. } | LinkTarget::Heading(_) => {
            job.note.parent().unwrap_or(root).join(target)
        }
        LinkTarget::Outside => {
            return message_figure(format!("{target} lies outside the workspace"), true);
        }
    };
    let cannot =
        |why: String| message_figure(format!("The image {alt} cannot be shown: {why}"), true);
    match image_path(&job.images, root, &file) {
        Ok(path) => FigureView {
            loading: Some(format!("Loading {target}…")),
            unreadable: Some(format!(
                "The image {alt} cannot be shown: the image cannot be read"
            )),
            ..picture_figure(path)
        },
        Err(FileProblem::NotFound) => cannot(format!("{target} was not found")),
        Err(FileProblem::TooLarge) => cannot(ImageError::TooLarge.to_string()),
        Err(FileProblem::NotImage) => {
            cannot(ImageError::Unreadable(NOT_AN_IMAGE.to_owned()).to_string())
        }
        Err(FileProblem::Outside | FileProblem::Hidden) => {
            message_figure(format!("{target} lies outside the workspace"), true)
        }
    }
}

/// Makes the views of all blocks.
struct Maker<'a> {
    job: &'a Job,
    blocks: &'a [Block],
    last_pictures: HashMap<usize, String>,
}

impl Maker<'_> {
    /// M-09.7, UC-004 A11: the picture of diagram block `index`; while a
    /// changed source renders, the last picture stays, as long as it is
    /// still served.
    fn diagram_figure(
        &mut self,
        index: usize,
        language: DiagramLanguage,
        source: &str,
    ) -> FigureView {
        diagram_figure(
            self.job.diagrams.as_deref(),
            &mut self.last_pictures,
            self.blocks.len(),
            (index, language, source, self.job.dark),
        )
    }

    /// M-09.4: the colours of a code block, kept by theme, language and
    /// code.
    fn colors_of(&self, language: &str, code: &str) -> Option<CodeColors> {
        let set = self.job.colors.as_deref()?;
        let mut hasher = DefaultHasher::new();
        (self.job.looks.0.label(), language, code).hash(&mut hasher);
        let key = hasher.finish();
        if let Some(colors) = self
            .job
            .code_colors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return colors.clone();
        }
        // Coloured without holding the cache, which another job may use.
        let colors = Some(code_colors(set, language, code)).filter(|c| !c.spans.is_empty());
        let mut cache = self
            .job
            .code_colors
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= CODE_COLORS_KEPT {
            cache.clear();
        }
        cache.insert(key, colors.clone());
        colors
    }

    fn views(&mut self, text: &str) -> Vec<BlockView> {
        let rope = &self.job.rope;
        let mut walker = WebWalker::new(text);
        let (mut figures, mut tables) = (0, 0);
        let mut views = Vec::with_capacity(self.blocks.len());
        for (index, block) in self.blocks.iter().enumerate() {
            let base = walker.mark(block.span.start);
            let runs: Vec<RunView> = block
                .runs
                .iter()
                .map(|run| run_view(run, &mut walker, base))
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
                        figure = Some(self.diagram_figure(index, diagram, &code));
                    } else {
                        colors = self.colors_of(language, &code);
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
                        tables += 1;
                        format!("Table {tables}. {caption}")
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
                                            cell.iter()
                                                .map(|run| run_view(run, &mut walker, base))
                                                .collect()
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
                    let mut image = image_figure(self.job, target, alt);
                    // TC-004-02: numbered as Asciidoctor numbers figures.
                    image.caption = caption.as_ref().map(|caption| {
                        figures += 1;
                        format!("Figure {figures}. {caption}")
                    });
                    figure = Some(image);
                    BlockKindView::Image {
                        target: target.clone(),
                        alt: alt.clone(),
                    }
                }
            };
            let end = walker.at(block.span.end);
            let first = rope.byte_to_line(block.span.start.min(text.len()));
            let last = rope.byte_to_line(
                block
                    .span
                    .end
                    .saturating_sub(1)
                    .max(block.span.start)
                    .min(text.len()),
            );
            views.push(BlockView {
                kind,
                start: units(base),
                line: units(first),
                length: units(end.saturating_sub(base)),
                lines: units(last - first + 1),
                prefix: prefix_text(&block.kind),
                runs,
                colors,
                figure,
            });
        }
        views
    }
}

/// M-09.7, UC-004 A11: the picture of the diagram `(index, language,
/// source, dark)` among `count` blocks. The diagrams nearer the start of
/// the note render first; while a changed source renders, the last picture
/// stays as long as it is served. Without the diagram service, it says it
/// cannot be rendered.
pub(super) fn diagram_figure(
    diagrams: Option<&SvgDiagrams>,
    last_pictures: &mut HashMap<usize, String>,
    count: usize,
    (index, language, source, dark): (usize, DiagramLanguage, &str, bool),
) -> FigureView {
    let ahead = u64::try_from(count.saturating_sub(index)).unwrap_or(u64::MAX);
    let state = diagrams.map_or_else(
        || {
            SvgState::Failed(DiagramError::Engine(
                "the renderer could not be started".into(),
            ))
        },
        |d| d.get(language, source, dark, Duration::from_micros(ahead)),
    );
    match state {
        SvgState::Ready(picture) => {
            last_pictures.insert(index, picture.clone());
            picture_figure(picture)
        }
        SvgState::Pending => {
            let last = last_pictures
                .get(&index)
                .filter(|last| diagrams.is_some_and(|d| d.has(last)))
                .cloned();
            match last {
                Some(last) => picture_figure(last),
                None => {
                    last_pictures.remove(&index);
                    message_figure(format!("Rendering the {} diagram…", language.name()), false)
                }
            }
        }
        SvgState::Failed(error) => {
            last_pictures.remove(&index);
            FigureView {
                source_shown: true,
                ..message_figure(
                    format!("This {} diagram cannot be shown: {error}", language.name()),
                    true,
                )
            }
        }
    }
}

/// Makes the blocks (parsing them when the job has none) and their views.
pub(in crate::session) fn run(job: Job) -> Built {
    let text = job.rope.to_string();
    let blocks = job
        .blocks
        .clone()
        .unwrap_or_else(|| Arc::new(parse_blocks(&text, job.format)));
    let mut maker = Maker {
        job: &job,
        blocks: &blocks,
        last_pictures: job.last_pictures.clone(),
    };
    let views = maker.views(&text);
    let mut last_pictures = maker.last_pictures;
    last_pictures.retain(|index, _| *index < blocks.len());
    Built {
        seq: job.seq,
        rev: job.rev,
        looks: job.looks,
        generation: job.generation,
        views,
        last_pictures,
        blocks,
    }
}

/// The views of the blocks of `text`, without pictures, colours or
/// workspace, for the tests of the conversion.
#[cfg(test)]
pub(super) fn views_of(text: &str, format: NoteFormat) -> Vec<BlockView> {
    run(Job {
        seq: 0,
        rev: 0,
        looks: (Theme::Light, false),
        generation: 0,
        rope: Rope::from_str(text),
        format,
        blocks: None,
        root: PathBuf::from("/nowhere"),
        note: PathBuf::from("/nowhere/Note.md"),
        colors: None,
        dark: false,
        diagrams: None,
        code_colors: Arc::default(),
        images: Arc::default(),
        last_pictures: HashMap::new(),
    })
    .views
}
