//! ADR-025: the backend's mirror (`underline_editor_core::sync::Server`)
//! in the webview, for the window's mock backend on the dev server
//! (`ui/src/lib/mock-backend.ts`): the same revisions, transforms and
//! reload barrier as the real backend, without a Tauri window.

use std::collections::HashMap;

use ropey::Rope;
use underline_editor_core::facade::{parse_all, MakeViews};
use underline_editor_core::sync::{Batch, Server};
use underline_editor_view::facade_view::{BlockKindView, FigureView, FormulaView};
use underline_editor_view::sync_view::{EnrichView, SyncAnswer, SyncBatch};
use underline_editor_view::views::{diagram_key, diagram_language, formula_key};
use underline_editor_view::CachedViews;
use underline_markup::blocks::BlockKind;
use underline_markup::NoteFormat;

use crate::calls::CallError;

/// The mirror of one note.
#[derive(Debug)]
pub struct Mirror(Server);

fn json<T: serde::Serialize>(value: &T) -> Result<String, CallError> {
    serde_json::to_string(value).map_err(CallError::Answer)
}

impl Mirror {
    /// The mirror of `text` at revision 1.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(Server::new(text))
    }

    /// The revision.
    #[must_use]
    pub fn rev(&self) -> u32 {
        self.0.rev()
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// `{ answer: SyncAnswer, reverted }` to `batch` (`SyncBatch`).
    ///
    /// # Errors
    ///
    /// `batch` is not a `SyncBatch`.
    pub fn receive(&mut self, batch: &str) -> Result<String, CallError> {
        let batch: SyncBatch =
            serde_json::from_str(batch).map_err(|reason| CallError::Invalid {
                what: "The batch",
                reason,
            })?;
        let received = self.0.receive(&Batch::from(&batch));
        json(&serde_json::json!({
            "answer": SyncAnswer::from(&received.answer),
            "reverted": received.reverted,
        }))
    }

    /// A change of the backend's own: `remove` chars at char `at` replaced
    /// by `insert`; the new revision, or `None` when it does not fit.
    pub fn backend_edit(
        &mut self,
        at: usize,
        remove: usize,
        insert: &str,
        sealed: bool,
    ) -> Option<u32> {
        self.0
            .backend_edit(
                &[(at..at.saturating_add(remove), insert.to_owned())],
                sealed,
            )
            .ok()
            .map(|(rev, _)| rev)
    }

    /// A change from outside: the text is `text` now.
    pub fn reload(&mut self, text: &str) -> u32 {
        self.0.reload(text)
    }

    /// `EnrichView` of the note `note` in `format` for the mock: the
    /// figures of its images from `images` (by target), every diagram with
    /// `diagram`, every formula with the placeholder `formula` (FR-098), or
    /// with an error when its braces do not match (UC-004 A21); keyed as the
    /// backend keys them.
    ///
    /// # Errors
    ///
    /// An argument is not the JSON it should be.
    pub fn enrichment(
        &self,
        note: &str,
        markdown: bool,
        (theme, dark): (&str, bool),
        images: &str,
        diagram: &str,
        formula: &str,
    ) -> Result<String, CallError> {
        let images: HashMap<String, FigureView> =
            serde_json::from_str(images).map_err(|reason| CallError::Invalid {
                what: "The images",
                reason,
            })?;
        let diagram: FigureView =
            serde_json::from_str(diagram).map_err(|reason| CallError::Invalid {
                what: "The diagram",
                reason,
            })?;
        let formula: FormulaView =
            serde_json::from_str(formula).map_err(|reason| CallError::Invalid {
                what: "The formula",
                reason,
            })?;
        let text = self.0.text();
        let format = if markdown {
            NoteFormat::Markdown
        } else {
            NoteFormat::AsciiDoc
        };
        let (blocks, _) = parse_all(&text, format);
        let views = CachedViews::new(theme, dark).all(&blocks, &text, &Rope::from_str(&text));
        let mut sent = EnrichView {
            note: note.to_owned(),
            rev: self.0.rev(),
            theme: theme.to_owned(),
            dark,
            clear: false,
            colors: Vec::new(),
            diagrams: Vec::new(),
            images: Vec::new(),
            formulas: Vec::new(),
        };
        for (block, view) in blocks.iter().zip(&views) {
            let block_formula = match &view.kind {
                BlockKindView::Formula { formula } => Some(formula),
                _ => None,
            };
            let inline = view.runs.iter().filter_map(|run| run.formula.as_ref());
            for shown in block_formula.into_iter().chain(inline) {
                sent.formulas.push((
                    formula_key(theme, shown.display, &shown.source),
                    mock_formula(&formula, &shown.source, shown.display),
                ));
            }
            let code: String = view.runs.iter().map(|r| r.text.as_str()).collect();
            match (&block.kind, &view.kind) {
                (BlockKind::Code { .. }, _) => {
                    if let Some(language) = diagram_language(&block.kind) {
                        sent.diagrams
                            .push((diagram_key(language, &code, dark), diagram.clone()));
                    }
                }
                (_, BlockKindView::Image { target, .. }) => {
                    if let Some(figure) = images.get(target) {
                        sent.images.push((target.clone(), figure.clone()));
                    }
                }
                _ => {}
            }
        }
        json(&sent)
    }

    /// `SyncAnswer` after the window's `text` was taken.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn resync(&mut self, text: &str) -> Result<String, CallError> {
        json(&SyncAnswer::from(&self.0.resync(text).answer))
    }
}

/// FR-098: the mock's formula with `source`: the placeholder `template`,
/// or, when its braces do not match, the error the backend would give
/// (UC-004 A21).
fn mock_formula(template: &FormulaView, source: &str, display: bool) -> FormulaView {
    let mut depth = 0_i32;
    for c in source.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            break;
        }
    }
    if depth != 0 {
        return FormulaView {
            source: source.to_owned(),
            display,
            error: Some(
                "This formula cannot be typeset: Expected '}', got 'EOF' (at character 1)".into(),
            ),
            ..FormulaView::default()
        };
    }
    // The placeholder is as wide as the source is long.
    let chars = u32::try_from(source.chars().count()).unwrap_or(u32::MAX);
    FormulaView {
        source: source.to_owned(),
        display,
        width: template.width.max(1).saturating_mul(chars.clamp(1, 40)),
        ..template.clone()
    }
}
