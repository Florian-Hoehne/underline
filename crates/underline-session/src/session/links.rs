//! UC-004 A2 to A4, BR-005 to BR-007, M-07.3: following a link of an open
//! note. Ported from `follow_link`, `jump_to_heading`, `open_with_system`
//! and `show_web_confirm` of `apps/underline-desktop/src/app.rs`.
//!
//! The window sends where a Ctrl+click (Cmd+click) landed, in web units of
//! the text of the backend's revision it is in step with, after its edits
//! reached the backend (`flushed`, ADR-025). The session finds the link
//! there in its own copy of the text ([`link_at_web`], the function egui's
//! source view uses on its blocks), so the window never names what the
//! system opens. A place of another revision is not followed. The link is
//! resolved ([`resolve_link`]), and the answer says what the window does
//! next ([`LinkAnswer`]); what the system opens (a web address, a file) is
//! opened by the caller after the session was let go ([`Followed::open`]),
//! and a failure comes back through [`Session::open_failed`].

use std::path::Path;

use underline_core::{
    asciidoc_anchor, heading_anchor, resolve_link, LinkTarget, NoteFormat, NoteId,
};
use underline_editor_core::utf16::WebOffsets;
use underline_editor_view::link_at_web;
use underline_facade::blocks::{heading_offset, parse_blocks};

use super::editing::not_open;
use super::Session;
use crate::editor_view::{units, LinkAnswer};
use crate::UiError;

/// What following a link gave: the answer for the window, and what the
/// caller opens with the system's program, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    /// What the window does next.
    pub answer: LinkAnswer,
    /// UC-004 BR-007: a web address or a file to open with the program the
    /// system uses for it, once the session is let go.
    pub open: Option<String>,
}

impl Followed {
    fn answer(answer: LinkAnswer) -> Self {
        Self { answer, open: None }
    }

    fn done() -> Self {
        Self::answer(LinkAnswer::Done)
    }

    fn open(target: String) -> Self {
        Self {
            answer: LinkAnswer::Done,
            open: Some(target),
        }
    }
}

impl Session {
    /// UC-004 A2 to A4, M-07.3: follows the link at web offset `at` of the
    /// text of the open tab `note`, as the window had it at the backend's
    /// revision `rev`:
    ///
    /// - a heading of the same note: the tab jumps there (A2), else "The
    ///   heading {anchor} was not found";
    /// - another note: it opens in a tab (A1, A2), at the heading the link
    ///   names;
    /// - a note that does not exist: "New note" offers it (A3);
    /// - a web address: asked once in the session, then opened (A4);
    /// - a file: opened with the system's program when it exists, else
    ///   "{path} does not exist" (A4);
    /// - a relative link leading outside the workspace: "{link} leads
    ///   outside the workspace and is not followed" (BR-005).
    ///
    /// No link there, or `rev` is not the revision of the backend's text
    /// (the text changed since the click): nothing happens.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn follow_link(&mut self, note: &NoteId, at: u32, rev: u32) -> Result<Followed, UiError> {
        let open = self.open.as_ref().ok_or_else(|| not_open(note))?;
        let tab = open
            .tabs
            .get(open.tabs.find(note).ok_or_else(|| not_open(note))?);
        if tab.mirror.rev() != rev {
            eprintln!(
                "M-07.3: a link of {} was clicked in revision {rev}, the text is at {}; not followed",
                tab.title,
                tab.mirror.rev()
            );
            return Ok(Followed::done());
        }
        let Some(link) = link_at_web(tab.mirror.rope(), tab.edit.format, at) else {
            return Ok(Followed::done());
        };
        let target = resolve_link(open.root(), &tab.path, &link);
        Ok(match target {
            LinkTarget::Heading(anchor) => Followed::answer(self.jump_to_heading(note, &anchor)),
            LinkTarget::Note { id, heading } => self.follow_to_note(&id, heading.as_deref()),
            // A3: the missing note is offered for creation (UC-003).
            LinkTarget::MissingNote { folder, title } => Followed::answer(LinkAnswer::NewNote {
                problem: format!("{title} does not exist yet. Create it?"),
                folder,
                title,
                format: open.workspace.default_format().into(),
            }),
            LinkTarget::Web(address) => self.follow_to_web(address),
            LinkTarget::File(path) => self.follow_to_file(&path),
            LinkTarget::Outside => {
                self.status = format!("{link} leads outside the workspace and is not followed");
                Followed::done()
            }
        })
    }

    /// UC-004 A2: opens the note `id`, at the heading `heading` if the link
    /// names one. A10: a note that cannot be read gets no tab; the status
    /// says why, and nothing jumps.
    fn follow_to_note(&mut self, id: &NoteId, heading: Option<&str>) -> Followed {
        match heading {
            Some(anchor) if self.open_note(id) => {
                Followed::answer(self.jump_to_heading(id, anchor))
            }
            Some(_) => Followed::done(),
            None => {
                self.open_note(id);
                Followed::done()
            }
        }
    }

    /// UC-004 A4: a web address opens once the user agreed in the session;
    /// until then "Open web address" asks.
    fn follow_to_web(&mut self, address: String) -> Followed {
        if self.web_allowed {
            Followed::open(address)
        } else {
            self.web_asked = Some(address.clone());
            Followed::answer(LinkAnswer::AskWeb { address })
        }
    }

    /// UC-004 A4: a file opens with the system's program when it exists.
    fn follow_to_file(&mut self, path: &Path) -> Followed {
        if path.exists() {
            Followed::open(path.display().to_string())
        } else {
            self.status = format!("{} does not exist", path.display());
            Followed::done()
        }
    }

    /// UC-004 A4: the answer to "Open web address": with `open` ("Open")
    /// the address asked about is opened, and web addresses are no longer
    /// asked about until Underline closes; "Cancel" opens nothing. Returns
    /// the address to open with the system's program.
    pub fn answer_web(&mut self, open: bool) -> Option<String> {
        let address = self.web_asked.take()?;
        if !open {
            return None;
        }
        self.web_allowed = true;
        Some(address)
    }

    /// UC-004 BR-007: the system's program could not open `target`.
    pub fn open_failed(&mut self, target: &str, error: &str) {
        self.status = format!("{target} could not be opened: {error}");
    }

    /// UC-004 A2: where the heading with `anchor` starts in the tab of
    /// `note`, found in the backend's text, as a jump for the window; when
    /// there is none, "The heading {anchor} was not found".
    fn jump_to_heading(&mut self, note: &NoteId, anchor: &str) -> LinkAnswer {
        let found = self.open.as_ref().and_then(|open| {
            let tab = open.tabs.get(open.tabs.find(note)?);
            let rope = tab.mirror.rope();
            let text = rope.to_string();
            let blocks = parse_blocks(&text, tab.edit.format);
            let slug = match tab.edit.format {
                NoteFormat::Markdown => heading_anchor,
                NoteFormat::AsciiDoc => asciidoc_anchor,
            };
            let byte = heading_offset(&blocks, anchor, slug)?;
            let at = rope.byte_to_char(byte.min(rope.len_bytes()));
            Some((tab.mirror.rev(), WebOffsets::of(rope).to_web(rope, at)))
        });
        match found {
            Some((rev, offset)) => LinkAnswer::Jump {
                note: note.as_str().to_owned(),
                rev,
                offset: units(offset),
            },
            None => {
                self.status = format!("The heading {anchor} was not found");
                LinkAnswer::Done
            }
        }
    }
}

#[cfg(test)]
mod tests;
