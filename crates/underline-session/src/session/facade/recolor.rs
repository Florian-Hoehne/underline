//! M-09.4, ADR-025: the colours of code blocks the facade's views were
//! made without (the job colours only what it has at hand), made in the
//! tick, so that the window's editor core gets them (`ed-enrich`).

use underline_editor_view::views::diagram_language;
use underline_editor_view::{FacadeModel, Versioned};
use underline_facade::blocks::BlockKind;

use super::build::{colors_key, remember_colors, Context};
use super::{Facade, Splice};

impl Facade {
    /// M-09.4, ADR-024: the code blocks of `model` sent without their
    /// colours get them; the changes for the window.
    pub(super) fn recolored(&mut self, model: &mut FacadeModel, ctx: &Context) -> Vec<Versioned> {
        let wanted = std::mem::take(&mut self.recolor);
        let Some(set) = ctx.colors.as_deref() else {
            return Vec::new();
        };
        let mut changes = Vec::new();
        let mut done = Vec::new();
        for index in wanted {
            if done.contains(&index) {
                continue;
            }
            done.push(index);
            let (Some(block), Some(view)) = (model.blocks.get(index), model.views.get(index))
            else {
                continue;
            };
            let BlockKind::Code { language } = &block.kind else {
                continue;
            };
            if diagram_language(&block.kind).is_some() {
                continue;
            }
            let code: String = view.runs.iter().map(|r| r.text.as_str()).collect();
            let key = colors_key(ctx.looks.0, language, &code);
            let known = ctx
                .code_colors
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&key)
                .cloned();
            let colors = match known {
                Some(colors) => colors,
                None => remember_colors(&ctx.code_colors, key, set, language, &code),
            };
            if view.colors != colors {
                let view = &mut model.views[index];
                view.colors = colors;
                changes.push(Splice {
                    from: index,
                    removed: 1,
                    blocks: vec![view.clone()],
                    shift: 0,
                    line_shift: 0,
                });
            }
        }
        model.versioned(changes)
    }
}
