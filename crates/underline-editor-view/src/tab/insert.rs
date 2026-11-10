//! Feature F6 of the migration: what the clipboard, copy on select, the
//! link and image dialogs and dropped files put into the tab's text, in
//! both views (UC-005 A2, A9 to A13, A17, A18, BR-011, M-09.14, M-10.3 to
//! M-10.7).
//!
//! The rules are those of the egui editor: `SourceView::paste`,
//! `copy_on_select` and `apply_format`, and the window's `answer_tool` and
//! `drop_items` (`apps/underline-desktop/src/app.rs`). What only the
//! backend can do (reading the system clipboard, storing an image, copying
//! a file) stays with it; it hands the tab what to insert.

use std::sync::Arc;

use underline_editor_core::facade::{cursor_in_code, parse_all, Before, Gathered, MakeViews};
use underline_markup::blocks::Block;
use underline_markup::formatting::{apply_format, Format};
use underline_markup::html::blocks_to_html;

use super::{Done, EditorTab, FacadeCall, Handled};
use crate::editor_view::{
    ClipContentView, CopyOnSelectView, EditorPatch, Insertion, SelectionCopy,
};
use crate::facade_view::{BlockView, FacadePatch};

/// What a paste brings (`Pasted` of the egui editor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Pasted {
    /// Plain text, typed (escaped in the facade).
    Text(String),
    /// Markup, as it is.
    Markup(String),
    /// An image alone, for the backend to store (A18).
    Image,
    /// Nothing to paste.
    Nothing,
}

/// UC-005 A17, A18, BR-014: what pasting `content` brings. With `rich` (the
/// facade, not Ctrl+Shift+V) the markup the tab copied last (`own`) comes
/// back as it is and HTML of other programs as markup; otherwise plain
/// text. An image counts only without text (`SourceView::paste` of egui).
pub(super) fn pasted(content: Option<&ClipContentView>, rich: bool, own: Option<&str>) -> Pasted {
    let Some(content) = content else {
        return Pasted::Nothing;
    };
    if let Some(text) = &content.text {
        if rich && own == Some(text.as_str()) {
            return Pasted::Markup(text.clone());
        }
    }
    match (&content.markup, &content.text) {
        (Some(markup), _) if rich => {
            if markup.is_empty() {
                Pasted::Nothing
            } else {
                Pasted::Markup(markup.clone())
            }
        }
        (_, Some(text)) => Pasted::Text(text.clone()),
        (_, None) if content.image => Pasted::Image,
        _ => Pasted::Nothing,
    }
}

/// M-09.14: what copying, cutting or pasting in the facade gave.
#[derive(Debug)]
pub struct FacadeClipped {
    /// The selected markup for the clipboard.
    pub copied: Option<String>,
    /// The selection as HTML for other programs (BR-011).
    pub html: Option<String>,
    /// The paste found only an image (A18).
    pub image: bool,
    /// The answer for the window.
    pub handled: Handled<FacadePatch>,
}

/// What an insertion does to the text.
enum Act {
    /// Applies a format at the selection.
    Format(Format),
    /// Inserts the text as it is.
    Raw(String),
}

/// UC-005 A2, A9 to A11: the format of a file put into the note: a picture,
/// or a link labelled with its file name (`answer_tool` of egui).
fn file_format(place: String, image: bool) -> Format {
    if image {
        Format::Image(place)
    } else {
        let label = Some(place.rsplit('/').next().unwrap_or(&place).to_owned());
        Format::Link {
            address: place,
            label,
        }
    }
}

/// What `insertion` does; `in_code` tells whether the cursor is in a code
/// block, asked only for a dropped file.
fn act(insertion: Insertion, in_code: impl FnOnce() -> bool) -> Act {
    match insertion {
        Insertion::Link { address, label } => Act::Format(Format::Link { address, label }),
        Insertion::Image { place } => Act::Format(Format::Image(place)),
        Insertion::File { place, image } => Act::Format(file_format(place, image)),
        Insertion::Dropped { place, image } => {
            if in_code() {
                Act::Raw(place)
            } else {
                Act::Format(file_format(place, image))
            }
        }
        Insertion::Text { text } => Act::Raw(text),
    }
}

impl EditorTab {
    /// The facade's blocks of the text as it is now: those the facade
    /// holds when they are of the text, else parsed for the question.
    fn blocks_now(&self) -> Arc<Vec<Block>> {
        if self.facade.parsed == Some(self.editor.revision()) {
            Arc::clone(&self.facade.blocks)
        } else {
            Arc::new(parse_all(&self.editor.text(), self.format).0)
        }
    }

    /// UC-005 BR-011: the selection as HTML with the facade's formatting.
    pub(super) fn selection_html(&self) -> String {
        let blocks = self.blocks_now();
        blocks_to_html(&blocks, self.editor.selection_bytes())
    }

    /// FR-031, FR-032, BR-011, M-10.7: a finished selection goes to the
    /// clipboard at once (`copy_on_select` of egui), once per selection:
    /// the selected markup and, in the `facade`, its HTML; the tab
    /// remembers the markup (M-09.14). The first copy of a selection asks
    /// the backend to keep what the clipboard held before (`first`), which
    /// pasting over the selection brings back ([`Self::over_copied`]).
    /// Without a selection the copied selection is forgotten; with `on`
    /// false (switched off) nothing is copied.
    ///
    /// The window calls it after every input and selection that settled,
    /// never while a mouse button is down. When the copied selection is
    /// gone, the answer says so (`forget`), so the backend lets go of the
    /// content it kept.
    pub fn copy_on_select(&mut self, on: bool, facade: bool) -> CopyOnSelectView {
        let selection = self.editor.selection();
        if selection.is_empty() {
            let forget = self.copied_selection.take().is_some();
            return CopyOnSelectView { copy: None, forget };
        }
        if !on || self.copied_selection.as_ref() == Some(&selection) {
            return CopyOnSelectView::default();
        }
        let first = self.copied_selection.is_none();
        let text = self.editor.selected_text();
        let html = facade.then(|| self.selection_html());
        self.own_markup = Some(text.clone());
        self.copied_selection = Some(selection);
        CopyOnSelectView {
            copy: Some(SelectionCopy { text, html, first }),
            forget: false,
        }
    }

    /// BR-011: whether the selection is the one copy on select copied, so
    /// pasting over it brings what the clipboard held before.
    #[must_use]
    pub fn over_copied(&self) -> bool {
        let selection = self.editor.selection();
        !selection.is_empty() && self.copied_selection.as_ref() == Some(&selection)
    }

    /// UC-005 A2, A9 to A13, A18: puts `insertion` into the text at the
    /// source view's selection (`apply_format` and `insert_text` of the
    /// egui source view).
    pub fn source_insert(&mut self, insertion: Insertion) -> Handled<EditorPatch> {
        let base = self.editor.revision();
        let act = act(insertion, || {
            let blocks = self.blocks_now();
            cursor_in_code(self.editor.document().rope(), self.editor.cursor(), &blocks)
        });
        let outcome = match act {
            Act::Format(format) => {
                let text = self.editor.text();
                let edit = apply_format(&text, self.format, self.editor.selection_bytes(), &format);
                self.editor
                    .replace_bytes(edit.range, &edit.text, edit.selection)
            }
            Act::Raw(text) => self.editor.type_text(&text),
        };
        let done = Done {
            outcome,
            ..Done::default()
        };
        self.handled(base, done)
    }

    /// UC-005 A2, A9 to A13, A18: puts `insertion` into the text at the
    /// facade's selection `call.at`; the blocks follow.
    pub fn facade_insert<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        insertion: Insertion,
    ) -> Handled<FacadePatch> {
        self.take_selection(call.at);
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let mut gathered = Gathered::default();
        self.with_edit(&mut gathered, (None, None), |edit, _| {
            match act(insertion, || edit.cursor_in_code()) {
                Act::Format(format) => edit.apply_format(&format),
                Act::Raw(text) => edit.insert_raw(&text),
            }
        });
        self.facade_finish(call, before, gathered)
    }
}
