//! Making the views of a facade (M-09, ADR-024), off the session: a [`Job`]
//! owns what it needs (a snapshot of the text, the theme's colours, the
//! diagrams), runs on a thread of its own, and gives back the blocks and
//! their views ([`Built`]); the session takes them in only while they are
//! still for the text it has (`facade.rs`).
//!
//! Ported from `crates/underline-editor/src/facade_view.rs`: the figures
//! (`figure`, `picture_state`, `image_source`) and the colours of code
//! blocks. What a block shows otherwise, with the numbers of the captions,
//! is the editor's (`underline_editor_view::views`, ADR-025), the same in
//! the webview.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use ropey::Rope;
use underline_core::{resolve_link, LinkTarget, NoteFormat, Theme};
use underline_editor_view::views::{
    message_figure, picture_figure, rendering_figure, Enrich, ViewMaker,
};
use underline_facade::blocks::Block;
use underline_facade::diagram::{DiagramError, DiagramLanguage};
use underline_facade::highlight::{highlight_code, HighlightSet};
use underline_facade::images::ImageError;

use super::super::diagrams::{SvgDiagrams, SvgState};
use crate::assets::{check_file, file_asset_path, FileProblem};
use crate::editor_view::{units, SpanStyle};
use crate::facade_view::{BlockView, CodeColors, FigureView};

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
pub(in crate::session) type CodeColorCache = HashMap<String, Option<CodeColors>>;

/// M-09.6: what was found of the image files, by their place, and when.
pub(in crate::session) type ImageCache = HashMap<PathBuf, (Instant, Result<String, FileProblem>)>;

/// What making the views of blocks needs besides the blocks: the look,
/// the colours for code, the diagrams, and where the note's images are.
pub(in crate::session) struct Context {
    pub(super) looks: Looks,
    pub(super) colors: Option<Arc<HighlightSet>>,
    pub(super) dark: bool,
    pub(super) diagrams: Option<Arc<SvgDiagrams>>,
    pub(super) code_colors: Arc<Mutex<CodeColorCache>>,
    pub(super) images: Arc<Mutex<ImageCache>>,
    pub(super) root: PathBuf,
    pub(super) note: PathBuf,
}

/// Everything making the views needs, owned, so it runs without the
/// session.
pub(in crate::session) struct Job {
    /// Counts the jobs of one facade; a later job wins.
    pub(super) seq: u64,
    pub(super) rev: u64,
    pub(super) generation: u64,
    /// A snapshot of the text (a rope is copied in no time).
    pub(super) rope: Rope,
    pub(super) format: NoteFormat,
    /// The blocks already parsed from this revision, if any.
    pub(super) blocks: Option<Arc<Vec<Block>>>,
    pub(super) last_pictures: HashMap<usize, String>,
    pub(super) ctx: Context,
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
    /// Whether a Markdown note may hold link reference definitions, when
    /// the job parsed the blocks.
    pub(super) refs: Option<bool>,
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
fn image_figure(ctx: &Context, target: &str, alt: &str) -> FigureView {
    let root = ctx.root.as_path();
    let file = match resolve_link(root, &ctx.note, target) {
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
            ctx.note.parent().unwrap_or(root).join(target)
        }
        LinkTarget::Outside => {
            return message_figure(format!("{target} lies outside the workspace"), true);
        }
    };
    let cannot =
        |why: String| message_figure(format!("The image {alt} cannot be shown: {why}"), true);
    match image_path(&ctx.images, root, &file) {
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

/// Makes the views of blocks with the session's highlighting, diagrams and
/// image files (ADR-025: the views themselves are the editor's,
/// [`ViewMaker`]).
pub(super) struct Maker<'a> {
    pub(super) ctx: &'a Context,
    pub(super) last_pictures: HashMap<usize, String>,
    /// Whether code is coloured when its colours are not kept yet; the
    /// keystroke command leaves that to the tick (ADR-024).
    pub(super) colour_now: bool,
}

impl<'a> Maker<'a> {
    pub(super) fn new(ctx: &'a Context, last_pictures: HashMap<usize, String>) -> Self {
        Self {
            ctx,
            last_pictures,
            colour_now: true,
        }
    }

    /// The views of all `blocks` of `text`, the whole note in `rope`.
    pub(super) fn views(&mut self, blocks: &[Block], text: &str, rope: &Rope) -> Vec<BlockView> {
        ViewMaker::new(self, blocks).views(text, rope)
    }
}

impl Enrich for Maker<'_> {
    /// M-09.4: the colours of a code block, kept by theme, language and
    /// code.
    fn colors(&mut self, language: &str, code: &str) -> Option<CodeColors> {
        let set = self.ctx.colors.as_deref()?;
        let key = colors_key(self.ctx.looks.0, language, code);
        if let Some(colors) = self
            .ctx
            .code_colors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return colors.clone();
        }
        if !self.colour_now {
            return None;
        }
        remember_colors(&self.ctx.code_colors, key, set, language, code)
    }

    /// M-09.7, UC-004 A11: the picture of diagram block `index`; while a
    /// changed source renders, the last picture stays, as long as it is
    /// still served.
    fn diagram(
        &mut self,
        index: usize,
        count: usize,
        language: DiagramLanguage,
        code: &str,
    ) -> FigureView {
        diagram_figure(
            self.ctx.diagrams.as_deref(),
            &mut self.last_pictures,
            count,
            (index, language, code, self.ctx.dark),
        )
    }

    fn image(&mut self, target: &str, alt: &str) -> FigureView {
        image_figure(self.ctx, target, alt)
    }
}

/// The key the colours of `code` in `language` are kept by, in `theme`:
/// FNV-1a, so the webview makes the same key (ADR-025,
/// `docs/migration/variante-b.md` section 2.3).
pub(super) fn colors_key(theme: Theme, language: &str, code: &str) -> String {
    underline_editor_view::views::colors_key(theme.label(), language, code)
}

/// Colours `code` and keeps its colours by `key`; `None` when nothing in it
/// is coloured.
pub(super) fn remember_colors(
    cache: &Mutex<CodeColorCache>,
    key: String,
    set: &HighlightSet,
    language: &str,
    code: &str,
) -> Option<CodeColors> {
    // Coloured without holding the cache, which another job may use.
    let colors = Some(code_colors(set, language, code)).filter(|c| !c.spans.is_empty());
    let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if cache.len() >= CODE_COLORS_KEPT && !cache.contains_key(&key) {
        cache.clear();
    }
    cache.insert(key, colors.clone());
    colors
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
                    rendering_figure(language)
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
    let (blocks, refs) = match job.blocks.clone() {
        Some(blocks) => (blocks, None),
        None => {
            let (blocks, refs) = underline_editor_core::facade::parse_all(&text, job.format);
            (Arc::new(blocks), Some(refs))
        }
    };
    let mut maker = Maker::new(&job.ctx, job.last_pictures.clone());
    let views = maker.views(&blocks, &text, &job.rope);
    let mut last_pictures = maker.last_pictures;
    last_pictures.retain(|index, _| *index < blocks.len());
    Built {
        seq: job.seq,
        rev: job.rev,
        looks: job.ctx.looks,
        generation: job.generation,
        views,
        last_pictures,
        blocks,
        refs,
    }
}

/// The views of the blocks of `text`, without pictures, colours or
/// workspace, for the tests of the conversion.
#[cfg(test)]
pub(super) fn views_of(text: &str, format: NoteFormat) -> Vec<BlockView> {
    run(Job {
        seq: 0,
        rev: 0,
        generation: 0,
        rope: Rope::from_str(text),
        format,
        blocks: None,
        last_pictures: HashMap::new(),
        ctx: Context {
            looks: (Theme::Light, false),
            colors: None,
            dark: false,
            diagrams: None,
            code_colors: Arc::default(),
            images: Arc::default(),
            root: PathBuf::from("/nowhere"),
            note: PathBuf::from("/nowhere/Note.md"),
        },
    })
    .views
}
