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
use underline_markup::blocks::Block;
use underline_markup::diagram::DiagramLanguage;

use crate::facade_view::{BlockKindView, BlockView, CodeColors, FigureView};
use crate::views::{
    carry_colors, colors_key, diagram_key, picture_figure, rendering_figure, Enrich, ViewMaker,
    WebWalker,
};

/// How many colourings of code blocks are kept, as in the backend.
pub const CODE_COLORS_KEPT: usize = 4096;

/// The views of the facade made from what the backend sent (ADR-025).
#[derive(Debug, Clone, Default)]
pub struct CachedViews {
    /// The name of the theme the colours are for.
    theme: String,
    /// Whether the theme is dark, which the diagrams are drawn for.
    dark: bool,
    colors: HashMap<u64, Option<CodeColors>>,
    diagrams: HashMap<u64, FigureView>,
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
    /// when nothing in the code is coloured.
    pub fn insert_colors(&mut self, key: u64, colors: Option<CodeColors>) {
        if self.colors.len() >= CODE_COLORS_KEPT {
            self.colors.clear();
        }
        self.colors.insert(key, colors);
    }

    /// The figure of a diagram kept by `key` ([`diagram_key`]).
    pub fn insert_diagram(&mut self, key: u64, figure: FigureView) {
        self.diagrams.insert(key, figure);
    }

    /// The figure of the image `target`, without its caption.
    pub fn insert_image(&mut self, target: &str, figure: FigureView) {
        self.images.insert(target.to_owned(), figure);
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
}
