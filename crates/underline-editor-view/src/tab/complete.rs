//! FR-080, UC-005 A23, A24, BR-018: link completion in the tab, in both
//! views. On every key the window asks whether the place of a link is
//! typed at the cursor ([`EditorTab::link_context`]); when the user chose a
//! note or a heading, the tab writes the link in place of what was typed
//! ([`EditorTab::source_complete`], [`EditorTab::facade_complete`]), as
//! one change. The rules are in [`underline_markup::completion`]; only the
//! current line is read for them, so a key costs no parse of the note
//! (NFR-018).

use std::ops::Range;

use underline_editor_core::facade::{Before, Gathered, MakeViews};
use underline_markup::completion::{
    code_span_open, in_code_block, link_context, link_place, named_note, write_link,
};
use underline_markup::NoteFormat;

use super::{Done, EditorTab, FacadeCall, Handled};
use crate::editor_view::{revision, units, EditorPatch, LinkChoice, LinkContextView};
use crate::facade_view::{BlockView, FacadeAt, FacadePatch};

/// How many lines before the cursor's are looked at for a code span.
const PARAGRAPH_LINES: usize = 20;

/// The note format named `name` (`Markdown`, `AsciiDoc`).
fn format_named(name: &str) -> Option<NoteFormat> {
    NoteFormat::ALL
        .into_iter()
        .find(|format| format.name() == name)
}

impl EditorTab {
    /// The file of `note` in the tab's format, relative to the workspace.
    fn own_file(&self, note: &str) -> String {
        format!("{note}.{}", self.format.extension())
    }

    /// The link typed at the cursor, as bytes of the text: the line of the
    /// cursor is read, nothing else. With a selection there is none.
    fn context_here(
        &self,
        facade: bool,
        manual: bool,
    ) -> Option<underline_markup::completion::LinkContext> {
        if !self.editor.selection().is_empty() {
            return None;
        }
        let rope = self.editor.document().rope();
        let cursor = self.editor.cursor().min(rope.len_chars());
        let line = rope.char_to_line(cursor);
        let line_start = rope.line_to_byte(line);
        let text = rope.line(line).to_string();
        let at = rope.char_to_byte(cursor) - line_start;
        let mut found = link_context(&text, at, self.format, facade, manual)?;
        found.replace = line_start + found.replace.start..line_start + found.replace.end;
        Some(found)
    }

    /// FR-080, UC-005 A23 step 1: the place of a link typed at the cursor
    /// of the tab of `note`, in the `facade` or the source view; with
    /// `manual` (Ctrl+Space) the facade completes at the cursor also
    /// without `[[`. `None` when there is none, or the cursor is in a code
    /// block.
    #[must_use]
    pub fn link_context(&self, note: &str, facade: bool, manual: bool) -> Option<LinkContextView> {
        let found = self.context_here(facade, manual)?;
        let rope = self.editor.document().rope();
        let line = rope.char_to_line(self.editor.cursor().min(rope.len_chars()));
        // The lines before the cursor's, one after the other, none kept.
        if in_code_block(rope.lines().take(line).map(String::from), self.format) {
            return None;
        }
        // A code span may open on an earlier line of the paragraph: those
        // lines (at most 20, back to a blank one) and the line up to the link.
        let mut paragraph = String::new();
        for back in (line.saturating_sub(PARAGRAPH_LINES)..line).rev() {
            let earlier = rope.line(back).to_string();
            if earlier.trim().is_empty() {
                break;
            }
            paragraph.insert_str(0, &earlier);
        }
        let start = rope.line_to_byte(line);
        paragraph.push_str(
            &rope
                .byte_slice(start..found.replace.start.max(start))
                .to_string(),
        );
        if code_span_open(&paragraph) {
            return None;
        }
        let from = rope.byte_to_char(found.replace.start.min(rope.len_bytes()));
        Some(LinkContextView {
            rev: revision(self.editor.revision()),
            from: units(self.editor.web_at_char(from)),
            named: named_note(&self.own_file(note), &found.typed),
            typed: found.typed,
            heading: found.heading,
        })
    }

    /// [`Self::link_context`] at the facade's selection `at`.
    pub fn facade_link_context(
        &mut self,
        note: &str,
        at: FacadeAt,
        manual: bool,
    ) -> Option<LinkContextView> {
        self.take_selection(at);
        self.link_context(note, true, manual)
    }

    /// UC-005 A23 step 3, BR-018: what the link of `choice` replaces and
    /// its markup; `None` when the text is no longer that of the choice or
    /// no link is typed at the cursor any more.
    fn completed(
        &self,
        note: &str,
        facade: bool,
        choice: &LinkChoice,
    ) -> Option<(Range<usize>, String)> {
        if revision(self.editor.revision()) != choice.rev {
            return None;
        }
        // Manual: the facade's completion at the cursor counts too.
        let context = self.context_here(facade, true)?;
        let own = self.own_file(note);
        let target = match (&choice.note, &choice.format) {
            (Some(target), Some(format)) => {
                format!("{target}.{}", format_named(format)?.extension())
            }
            (None, _) => own.clone(),
            (Some(_), None) => return None,
        };
        let place = link_place(&own, &target, choice.anchor.as_deref());
        let link = write_link(&context, self.format, &place, &choice.label);
        Some((context.replace, link))
    }

    /// FR-080, UC-005 A23 step 3: the source view writes the link chosen
    /// in place of what was typed of it, the cursor after it, as one
    /// change; `None` (nothing changed) when the text is no longer that of
    /// the choice.
    pub fn source_complete(
        &mut self,
        note: &str,
        choice: &LinkChoice,
    ) -> Option<Handled<EditorPatch>> {
        let (range, link) = self.completed(note, false, choice)?;
        let base = self.editor.revision();
        let end = range.start + link.len();
        let outcome = self.editor.replace_bytes(range, &link, end..end);
        let done = Done {
            outcome,
            ..Done::default()
        };
        Some(self.handled(base, done))
    }

    /// FR-080, UC-005 A23 step 3, BR-018: the facade writes the link chosen
    /// in place of what was typed of it (also the `[[`, which is never
    /// stored), the cursor after it; the blocks follow. `None` when the
    /// text is no longer that of the choice.
    pub fn facade_complete<M: MakeViews<View = BlockView>>(
        &mut self,
        call: FacadeCall<'_, M>,
        choice: &LinkChoice,
    ) -> Option<Handled<FacadePatch>> {
        self.take_selection(call.at);
        let (range, link) = self.completed(call.note, true, choice)?;
        let rope = self.editor.document().rope();
        let from = rope.byte_to_char(range.start);
        let to = rope.byte_to_char(range.end);
        self.editor.set_selection(to, from);
        let before = Before::of(&self.editor, self.facade.blocks.len());
        let mut gathered = Gathered::default();
        self.with_edit(&mut gathered, (None, None), |edit, _| {
            edit.insert_raw(&link);
        });
        Some(self.facade_finish(call, before, gathered))
    }
}

#[cfg(test)]
mod tests;
