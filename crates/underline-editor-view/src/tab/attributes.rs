//! UC-024 Edit Note Attributes (FR-093) and the tags of the note (FR-091,
//! UC-004 BR-016) in the tab: read by the webview's editor core from the
//! text it owns (ADR-025), so the note properties follow every change at
//! once, and changed as one edit of that text in the view shown: one undo
//! step, stored by autosave like any edit (BR-006).
//!
//! What a revision of the text costs is paid once: the attributes and the
//! tags are kept for it, and the facade's blocks are taken when they are of
//! it.

use std::sync::Arc;

use underline_editor_core::facade::{kept_after, parse_all, Before, Gathered, MakeViews};
use underline_editor_core::utf16::WebOffsets;
use underline_markup::attributes::{
    head_edit, note_head, AttributeChange, AttributeField, AttributeKind, AttributeValue, HeadEdit,
};
use underline_markup::tags::{may_hold_text_tags, note_tags_with, tag_names};
use underline_markup::NoteFormat;

use super::{Done, EditorTab, FacadeCall};
use crate::editor_view::{
    units, AttributeChangeView, AttributeEdited, AttributeFieldName, AttributeKindName,
    AttributeValueView, EditorPatch, HeadProblemView, NoteAttributeView, NoteHeadView,
};
use crate::facade_view::{BlockView, FacadePatch};

/// The note attributes of the last revision asked for.
#[derive(Debug, Clone, Default)]
pub(crate) struct HeadCache {
    answer: Option<(u64, NoteHeadView)>,
}

impl From<AttributeKind> for AttributeKindName {
    fn from(kind: AttributeKind) -> Self {
        match kind {
            AttributeKind::Text => Self::Text,
            AttributeKind::List => Self::List,
            AttributeKind::Number => Self::Number,
            AttributeKind::Date => Self::Date,
            AttributeKind::YesNo => Self::YesNo,
            AttributeKind::Nested => Self::Nested,
        }
    }
}

impl From<AttributeValueView> for AttributeValue {
    fn from(value: AttributeValueView) -> Self {
        match value {
            AttributeValueView::Text { text } => Self::Text(text),
            AttributeValueView::List { items } => Self::List(items),
            AttributeValueView::Number { text } => Self::Number(text),
            AttributeValueView::Date { text } => Self::Date(text),
            AttributeValueView::YesNo { yes } => Self::YesNo(yes),
        }
    }
}

impl From<AttributeChangeView> for AttributeChange {
    fn from(change: AttributeChangeView) -> Self {
        match change {
            AttributeChangeView::Add { name, value } => Self::Add {
                name,
                value: value.into(),
            },
            AttributeChangeView::Set { name, value } => Self::Set {
                name,
                value: value.into(),
            },
            AttributeChangeView::Remove { name } => Self::Remove { name },
        }
    }
}

/// UC-024 step 2: the note attributes and the tags of `text`, a note in
/// `format`, whose blocks are `blocks`; offsets are web offsets of `rope`.
#[must_use]
fn head_view(
    text: &str,
    rope: &ropey::Rope,
    format: NoteFormat,
    blocks: &[underline_markup::blocks::Block],
) -> NoteHeadView {
    let head = note_head(text, format);
    let tags = tag_names(&note_tags_with(text, format, &head, blocks));
    // The web offsets are worked out only for a note that has a head.
    let web = (!head.attributes.is_empty() || head.problem.is_some()).then(|| WebOffsets::of(rope));
    let len = rope.len_bytes();
    let offset = |byte: usize| {
        web.as_ref().map_or(0, |web| {
            units(web.to_web(rope, rope.byte_to_char(byte.min(len))))
        })
    };
    NoteHeadView {
        attributes: head
            .attributes
            .iter()
            .map(|attribute| NoteAttributeView {
                name: attribute.name.clone(),
                kind: attribute.kind.into(),
                value: attribute.value.clone(),
                items: attribute.items.clone(),
                line: units(attribute.line),
                offset: offset(attribute.span.start),
            })
            .collect(),
        problem: head.problem.as_ref().map(|problem| {
            // The line counts `\n` only, as the attributes count it.
            let start = text
                .match_indices('\n')
                .nth(problem.line.saturating_sub(2))
                .filter(|_| problem.line > 1)
                .map_or(0, |(at, _)| at + 1);
            HeadProblemView {
                line: units(problem.line),
                reason: problem.reason.clone(),
                markup: problem.markup.clone(),
                offset: offset(start),
            }
        }),
        tags,
        asciidoc: format == NoteFormat::AsciiDoc,
    }
}

impl EditorTab {
    /// UC-024 step 2, FR-091: the note attributes and the tags of the note,
    /// kept for the revision of the text.
    pub fn note_head(&mut self) -> NoteHeadView {
        let revision = self.editor.revision();
        if let Some((rev, answer)) = &self.head.answer {
            if *rev == revision {
                return answer.clone();
            }
        }
        let text = self.editor.text();
        // The blocks are needed only for tags in the text of a Markdown
        // note, and parsed only when the text may hold one.
        let blocks = if self.facade.parsed == Some(revision) {
            Arc::clone(&self.facade.blocks)
        } else if self.format == NoteFormat::Markdown && may_hold_text_tags(&text) {
            Arc::new(parse_all(&text, self.format).0)
        } else {
            Arc::new(Vec::new())
        };
        let answer = head_view(&text, self.editor.document().rope(), self.format, &blocks);
        self.head.answer = Some((revision, answer.clone()));
        answer
    }

    /// The edit that makes `change`, or why it is not written and what about.
    fn head_change(
        &self,
        change: AttributeChangeView,
    ) -> Result<HeadEdit, (String, AttributeFieldName)> {
        head_edit(&self.editor.text(), self.format, &change.into()).map_err(|error| {
            let about = match error.about() {
                AttributeField::Name => AttributeFieldName::Name,
                AttributeField::Value => AttributeFieldName::Value,
                AttributeField::Note => AttributeFieldName::Note,
            };
            (error.to_string(), about)
        })
    }

    /// UC-024 step 4, BR-003, BR-006: `change` written into the text in the
    /// source view, as one undo step, the selection kept on its text; A3 to
    /// A6: nothing written and the reason.
    pub fn source_attribute(
        &mut self,
        change: AttributeChangeView,
    ) -> AttributeEdited<EditorPatch> {
        let edit = match self.head_change(change) {
            Ok(edit) => edit,
            Err((problem, about)) => {
                return AttributeEdited {
                    patch: None,
                    problem: Some(problem),
                    about: Some(about),
                }
            }
        };
        let base = self.editor.revision();
        let selection = self.editor.selection_bytes();
        let len = edit.text.len();
        let select = kept_after(selection.start, &edit.range, len)
            ..kept_after(selection.end, &edit.range, len);
        let outcome = self.editor.replace_bytes(edit.range, &edit.text, select);
        let done = Done {
            outcome,
            ..Done::default()
        };
        AttributeEdited {
            patch: Some(self.handled(base, done).patch),
            problem: None,
            about: None,
        }
    }

    /// UC-024 step 4, BR-003, BR-006: `change` written into the text in the
    /// facade at the selection `call.at`, as one undo step; the blocks
    /// follow, the front matter's table among them (OQ-216).
    pub fn facade_attribute<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        change: AttributeChangeView,
    ) -> AttributeEdited<FacadePatch> {
        self.take_selection(call.at);
        let edit = match self.head_change(change) {
            Ok(edit) => edit,
            Err((problem, about)) => {
                return AttributeEdited {
                    patch: None,
                    problem: Some(problem),
                    about: Some(about),
                }
            }
        };
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let mut gathered = Gathered::default();
        self.with_edit(&mut gathered, (None, None), |facade, _| {
            facade.replace_keeping_selection(edit.range, &edit.text);
        });
        AttributeEdited {
            patch: Some(self.facade_finish(call, before, gathered).patch),
            problem: None,
            about: None,
        }
    }
}

#[cfg(test)]
mod tests;
