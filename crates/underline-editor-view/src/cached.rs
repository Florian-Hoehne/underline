//! ADR-025: the views of the facade's blocks in the webview, made from what
//! the backend sent of them: the colours of code, the pictures of diagrams
//! and images, kept by their content (`docs/migration/variante-b.md`
//! section 2.3). What is not there yet shows as the backend shows it while
//! it works: code without colours (the old ones carried over), a diagram
//! with its last picture or "Rendering …", an image "Loading …".

use std::collections::HashMap;
use std::ops::Range;

use ropey::Rope;
use underline_editor_core::facade::{moved_indices, MakeViews, Part};
use underline_markup::blocks::{Block, BlockKind};
use underline_markup::diagram::DiagramLanguage;

use crate::facade_view::{BlockKindView, BlockView, CodeColors, FigureView};
use crate::sync_view::EnrichView;
use crate::views::{
    carry_colors, colors_key, diagram_key, diagram_language, picture_figure, rendering_figure,
    Enrich, ViewMaker, WebWalker,
};

/// How many colours and figures the backend sends before it starts anew
/// (`clear`): the window keeps exactly what was sent since the last
/// `clear`, so both sides stay bounded, and drop the same entries.
pub const CODE_COLORS_KEPT: usize = 4096;

/// The views of the facade made from what the backend sent (ADR-025).
#[derive(Debug, Clone, Default)]
pub struct CachedViews {
    /// The name of the theme the colours are for.
    theme: String,
    /// Whether the theme is dark, which the diagrams are drawn for.
    dark: bool,
    colors: HashMap<String, Option<CodeColors>>,
    diagrams: HashMap<String, FigureView>,
    images: HashMap<String, FigureView>,
    /// M-09.7: the last picture of each diagram block, by block index.
    last_pictures: HashMap<usize, String>,
}

impl CachedViews {
    /// Views in the theme named `theme`, dark or not, with nothing sent yet.
    #[must_use]
    pub fn new(theme: &str, dark: bool) -> Self {
        Self {
            theme: theme.to_owned(),
            dark,
            ..Self::default()
        }
    }

    /// The colours `colors` of code kept by `key` ([`colors_key`]); `None`
    /// when nothing in the code is coloured. Only the backend's `clear`
    /// drops them ([`CODE_COLORS_KEPT`]), so that it knows what is here.
    pub fn insert_colors(&mut self, key: String, colors: Option<CodeColors>) {
        self.colors.insert(key, colors);
    }

    /// The figure of a diagram kept by `key` ([`diagram_key`]).
    pub fn insert_diagram(&mut self, key: String, figure: FigureView) {
        self.diagrams.insert(key, figure);
    }

    /// The figure of the image `target`, without its caption.
    pub fn insert_image(&mut self, target: &str, figure: FigureView) {
        self.images.insert(target.to_owned(), figure);
    }

    /// Whether these views are for the theme named `theme`, dark or not.
    #[must_use]
    pub fn is_for(&self, theme: &str, dark: bool) -> bool {
        self.theme == theme && self.dark == dark
    }

    /// ADR-025: takes in what the backend sent (`ed-enrich`); a new look or
    /// `clear` drops what came before. The last pictures of the diagrams
    /// stay in the same look (M-09.7).
    pub fn take(&mut self, sent: &EnrichView) {
        if sent.clear || !self.is_for(&sent.theme, sent.dark) {
            let same = self.is_for(&sent.theme, sent.dark);
            let last = std::mem::take(&mut self.last_pictures);
            *self = Self::new(&sent.theme, sent.dark);
            if same {
                self.last_pictures = last;
            }
        }
        for (key, colors) in &sent.colors {
            self.insert_colors(key.clone(), colors.clone());
        }
        for (key, figure) in &sent.diagrams {
            self.insert_diagram(key.clone(), figure.clone());
        }
        for (target, figure) in &sent.images {
            self.insert_image(target, figure.clone());
        }
    }

    /// ADR-025: the views of `blocks` take what was sent of their colours
    /// and figures; the indices of the views that changed. Colours not sent
    /// yet stay as they are (carried over), captions too.
    pub fn enrich(&mut self, blocks: &[Block], views: &mut [BlockView]) -> Vec<usize> {
        let count = blocks.len();
        let mut changed = Vec::new();
        for (index, (block, view)) in blocks.iter().zip(views.iter_mut()).enumerate() {
            let code = || {
                view.runs
                    .iter()
                    .map(|r| r.text.as_str())
                    .collect::<String>()
            };
            match &block.kind {
                BlockKind::Code { language } => {
                    if let Some(diagram) = diagram_language(&block.kind) {
                        let figure = self.diagram(index, count, diagram, &code());
                        if view.figure.as_ref() != Some(&figure) {
                            view.figure = Some(figure);
                            changed.push(index);
                        }
                    } else if let Some(colors) =
                        self.colors.get(&colors_key(&self.theme, language, &code()))
                    {
                        if view.colors != *colors {
                            view.colors.clone_from(colors);
                            changed.push(index);
                        }
                    }
                }
                BlockKind::Image { target, .. } => {
                    if let Some(sent) = self.images.get(target) {
                        let caption = view.figure.as_ref().and_then(|f| f.caption.clone());
                        let figure = FigureView {
                            caption,
                            ..sent.clone()
                        };
                        if view.figure.as_ref() != Some(&figure) {
                            view.figure = Some(figure);
                            changed.push(index);
                        }
                    }
                }
                _ => {}
            }
        }
        changed
    }
}

impl Enrich for CachedViews {
    fn colors(&mut self, language: &str, code: &str) -> Option<CodeColors> {
        self.colors
            .get(&colors_key(&self.theme, language, code))
            .cloned()
            .flatten()
    }

    fn diagram(
        &mut self,
        index: usize,
        _count: usize,
        language: DiagramLanguage,
        code: &str,
    ) -> FigureView {
        let sent = self.diagrams.get(&diagram_key(language, code, self.dark));
        match sent {
            Some(figure) if figure.picture.is_some() || figure.failed => {
                match &figure.picture {
                    Some(picture) => self.last_pictures.insert(index, picture.clone()),
                    None => self.last_pictures.remove(&index),
                };
                figure.clone()
            }
            // Still rendering: the last picture stays (M-09.7).
            _ => match self.last_pictures.get(&index) {
                Some(last) => picture_figure(last.clone()),
                None => rendering_figure(language),
            },
        }
    }

    fn image(&mut self, target: &str, _alt: &str) -> FigureView {
        self.images
            .get(target)
            .cloned()
            .unwrap_or_else(|| FigureView {
                loading: Some(format!("Loading {target}…")),
                ..FigureView::default()
            })
    }
}

impl MakeViews for CachedViews {
    type View = BlockView;

    fn all(&mut self, blocks: &[Block], text: &str, rope: &Rope) -> Vec<BlockView> {
        let views = ViewMaker::new(self, blocks).views(text, rope);
        self.last_pictures.retain(|index, _| *index < blocks.len());
        views
    }

    fn some(
        &mut self,
        blocks: &[Block],
        range: Range<usize>,
        part: Part<'_>,
        rope: &Rope,
    ) -> Vec<BlockView> {
        let mut walker = WebWalker::part(part.text, part.from, part.origin);
        ViewMaker::new(self, blocks).views_of_range(range, &mut walker, rope)
    }

    fn blocks_moved(&mut self, replaced: Range<usize>, grown: isize) {
        let last = std::mem::take(&mut self.last_pictures);
        self.last_pictures = moved_indices(last, replaced, grown);
    }

    fn made(&mut self, _index: usize, view: &mut BlockView, old: Option<&BlockView>) {
        let code = matches!(view.kind, BlockKindView::Code { .. }) && view.figure.is_none();
        if !code || view.colors.is_some() {
            return;
        }
        if let Some(old) = old.filter(|old| old.kind == view.kind) {
            view.colors = carry_colors(old, view);
        }
    }
}

#[cfg(test)]
mod tests {
    use underline_editor_core::facade::parse_all;
    use underline_markup::NoteFormat;

    use super::*;

    const NOTE: &str =
        "```rust\nfn main() {}\n```\n\n```mermaid\ngraph TD\nA-->B\n```\n\n![Plan](img/plan.png)\n";

    fn views_of(cached: &mut CachedViews, text: &str) -> Vec<BlockView> {
        let (blocks, _) = parse_all(text, NoteFormat::Markdown);
        cached.all(&blocks, text, &Rope::from_str(text))
    }

    /// ADR-025, M-09.4, M-09.6, M-09.7: what the backend did not send yet
    /// shows as the backend shows it while it works.
    #[test]
    fn adr025_what_is_not_sent_yet_shows_as_in_the_works() {
        let mut cached = CachedViews::new("Light", false);

        let views = views_of(&mut cached, NOTE);

        assert_eq!(views[0].colors, None, "code without colours");
        assert_eq!(
            views[1].figure.as_ref().and_then(|f| f.message.as_deref()),
            Some("Rendering the Mermaid diagram…")
        );
        assert_eq!(
            views[2].figure.as_ref().and_then(|f| f.loading.as_deref()),
            Some("Loading img/plan.png…")
        );
    }

    /// ADR-025, M-09.7: while a changed diagram renders, its last picture
    /// stays; sent colours and pictures show.
    #[test]
    fn adr025_sent_figures_show_and_a_changed_diagram_keeps_its_picture() {
        let mut cached = CachedViews::new("Light", false);
        let code =
            |view: &BlockView| -> String { view.runs.iter().map(|r| r.text.as_str()).collect() };
        let first = views_of(&mut cached, NOTE);
        cached.insert_diagram(
            diagram_key(DiagramLanguage::Mermaid, &code(&first[1]), false),
            picture_figure("diagram/a.svg".into()),
        );
        let colors = CodeColors {
            spans: vec![(0, 2, 0)],
            styles: Vec::new(),
        };
        cached.insert_colors(
            colors_key("Light", "rust", &code(&first[0])),
            Some(colors.clone()),
        );

        let views = views_of(&mut cached, NOTE);
        assert_eq!(views[0].colors, Some(colors));
        assert_eq!(
            views[1].figure.as_ref().and_then(|f| f.picture.as_deref()),
            Some("diagram/a.svg")
        );

        let changed = NOTE.replace("A-->B", "A-->C");
        let views = views_of(&mut cached, &changed);
        assert_eq!(
            views[1].figure.as_ref().and_then(|f| f.picture.as_deref()),
            Some("diagram/a.svg"),
            "the last picture while the new one renders"
        );
    }

    /// ADR-025: what the backend sends later (`ed-enrich`) goes into the
    /// views made before: code colours by key, an image's figure with its
    /// caption kept; a new theme drops what came before.
    #[test]
    fn adr025_sent_entries_enrich_the_views_in_place() {
        let text = "```rust\nfn main() {}\n```\n\n![Plan](img/plan.png)\n";
        let mut cached = CachedViews::new("Light", false);
        let (blocks, _) = parse_all(text, NoteFormat::Markdown);
        let mut views = cached.all(&blocks, text, &Rope::from_str(text));
        let code: String = views[0].runs.iter().map(|r| r.text.as_str()).collect();
        let colors = CodeColors {
            spans: vec![(0, 2, 0)],
            styles: Vec::new(),
        };
        let picture = picture_figure("file/img/plan.png?v=1".into());
        let sent = EnrichView {
            note: "A".into(),
            rev: 2,
            theme: "Light".into(),
            dark: false,
            clear: true,
            colors: vec![(colors_key("Light", "rust", &code), Some(colors.clone()))],
            diagrams: Vec::new(),
            images: vec![("img/plan.png".into(), picture.clone())],
        };

        cached.take(&sent);
        let changed = cached.enrich(&blocks, &mut views);

        assert_eq!(changed, [0, 1]);
        assert_eq!(views[0].colors, Some(colors));
        assert_eq!(
            views[1].figure.as_ref().and_then(|f| f.picture.clone()),
            picture.picture
        );
        assert!(cached.enrich(&blocks, &mut views).is_empty(), "once");

        cached.take(&EnrichView {
            theme: "Nord".into(),
            clear: false,
            colors: Vec::new(),
            images: Vec::new(),
            ..sent
        });
        assert!(cached.is_for("Nord", false));
        assert!(
            cached.enrich(&blocks, &mut views).is_empty(),
            "nothing sent for Nord yet"
        );
    }

    /// ADR-025 (review B5b): `clear` in the same look drops the colours and
    /// figures sent before but keeps the last pictures of the diagrams
    /// (M-09.7); a new look drops everything.
    #[test]
    fn adr025_clear_keeps_the_last_pictures_a_new_look_drops_all() {
        let mut cached = CachedViews::new("Light", false);
        let code =
            |view: &BlockView| -> String { view.runs.iter().map(|r| r.text.as_str()).collect() };
        let first = views_of(&mut cached, NOTE);
        let key = diagram_key(DiagramLanguage::Mermaid, &code(&first[1]), false);
        cached.insert_diagram(key.clone(), picture_figure("diagram/a.svg".into()));
        cached.insert_colors(colors_key("Light", "rust", &code(&first[0])), None);
        views_of(&mut cached, NOTE);
        let sent = |theme: &str, clear: bool| EnrichView {
            note: "A".into(),
            rev: 1,
            theme: theme.into(),
            dark: false,
            clear,
            colors: Vec::new(),
            diagrams: Vec::new(),
            images: Vec::new(),
        };

        cached.take(&sent("Light", false));
        assert!(cached.diagrams.contains_key(&key), "no clear: kept");
        cached.take(&sent("Light", true));
        assert!(cached.colors.is_empty() && cached.diagrams.is_empty());
        let views = views_of(&mut cached, NOTE);
        assert_eq!(
            views[1].figure.as_ref().and_then(|f| f.picture.as_deref()),
            Some("diagram/a.svg"),
            "the last picture stays in the same look"
        );
        cached.take(&sent("Dark", false));
        assert!(cached.last_pictures.is_empty());
        assert!(cached.is_for("Dark", false));
    }
}
