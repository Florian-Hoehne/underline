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
    asciidoc_anchor, executable_by_name, executable_kind, heading_anchor, is_file_url,
    is_remote_place, opens_without_asking, resolve_link, shown_scheme, shown_text, LinkTarget,
    NoteFormat, NoteId,
};
use underline_editor_core::utf16::WebOffsets;
use underline_editor_view::link_at_web;
use underline_facade::blocks::{heading_offset, parse_blocks};

use super::editing::not_open;
use super::Session;
use crate::editor_view::{units, LinkAnswer, OpenKindName};
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
    /// - an `http`, `https` or `mailto` address: opened (A4, BR-007, Q-62);
    /// - an address of any other scheme: asked about every time (Q-62);
    /// - a file: opened with the system's program when it exists, else
    ///   "{path} does not exist" (A4); one that may run as a program, or a
    ///   `file://` target, is asked about first, every time (Q-61);
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
        self.follow_link_to(note, at, rev, false)
    }

    /// [`Session::follow_link`]; with `side`, a note the link leads to
    /// opens to the side (UC-004 A18, FR-096), as a Ctrl+Shift+click
    /// (Cmd+Shift+click) asks.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn follow_link_to(
        &mut self,
        note: &NoteId,
        at: u32,
        rev: u32,
        side: bool,
    ) -> Result<Followed, UiError> {
        // Q-61: a question not answered does not outlive the next link.
        self.open_asked = None;
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
        Ok(self.follow_written(note, &link, side))
    }

    /// UC-004 A14 step 2, FR-079: follows the outgoing link of the open tab
    /// `note` that starts at web offset `at` of the backend's revision
    /// `rev`, as the tool panel listed it, as a Ctrl+click follows a link
    /// ([`Session::follow_link`]). The session finds the link in its own
    /// text; another revision, or no link starting there, follows nothing.
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn follow_outgoing(
        &mut self,
        note: &NoteId,
        at: u32,
        rev: u32,
    ) -> Result<Followed, UiError> {
        self.open_asked = None;
        let open = self.open.as_ref().ok_or_else(|| not_open(note))?;
        let tab = open
            .tabs
            .get(open.tabs.find(note).ok_or_else(|| not_open(note))?);
        if tab.mirror.rev() != rev {
            eprintln!(
                "FR-079: a link of {} was chosen in revision {rev}, the text is at {}; not followed",
                tab.title,
                tab.mirror.rev()
            );
            self.status = format!(
                "{} changed since its links were listed; choose the link again",
                tab.title
            );
            return Ok(Followed::done());
        }
        let Some(link) = super::note_links::outgoing_at(tab.mirror.rope(), tab.edit.format, at)
        else {
            return Ok(Followed::done());
        };
        Ok(self.follow_written(note, &link, false))
    }

    /// UC-004 A2 to A4: follows `link`, as written in the open tab `note`;
    /// a note it leads to opens to the side with `side` (A18).
    fn follow_written(&mut self, note: &NoteId, link: &str, side: bool) -> Followed {
        let Some((root, path, format)) = self.open.as_ref().and_then(|open| {
            let tab = open.tabs.get(open.tabs.find(note)?);
            Some((
                open.root().to_owned(),
                tab.path.clone(),
                open.workspace.default_format(),
            ))
        }) else {
            return Followed::done();
        };
        if super::note_links::is_inline_data(link) {
            // OQ-244: the picture is in the note; there is nothing to open.
            self.status =
                "This picture is held in the note itself; there is nothing to open".into();
            return Followed::done();
        }
        match resolve_link(&root, &path, link) {
            LinkTarget::Heading(anchor) => Followed::answer(self.jump_to_heading(note, &anchor)),
            LinkTarget::Note { id, heading } => self.follow_to_note(&id, heading.as_deref(), side),
            // A3: the missing note is offered for creation (UC-003).
            LinkTarget::MissingNote { folder, title } => Followed::answer(LinkAnswer::NewNote {
                problem: format!("{title} does not exist yet. Create it?"),
                folder,
                title,
                format: format.into(),
            }),
            LinkTarget::Web(address) => self.follow_to_web(address),
            LinkTarget::File(path) => self.follow_to_file(&path, is_file_url(link)),
            LinkTarget::Outside => {
                self.status = format!("{link} leads outside the workspace and is not followed");
                Followed::done()
            }
        }
    }

    /// UC-004 A2: opens the note `id`, at the heading `heading` if the link
    /// names one. A10: a note that cannot be read gets no tab; the status
    /// says why, and nothing jumps.
    fn follow_to_note(&mut self, id: &NoteId, heading: Option<&str>, side: bool) -> Followed {
        let opened = if side {
            self.open_note_to_side(id)
        } else {
            self.open_note(id)
        };
        match heading {
            Some(anchor) if opened => Followed::answer(self.jump_to_heading(id, anchor)),
            _ => Followed::done(),
        }
    }

    /// UC-004 A4, BR-007, Q-62: an `http`, `https` or `mailto` address
    /// opens in the default browser or mail program; one of any other
    /// scheme starts another program, so "Open {scheme}: link" asks first,
    /// naming the scheme (as written, when it cannot be read) and showing
    /// the whole address, every time.
    fn follow_to_web(&mut self, address: String) -> Followed {
        if opens_without_asking(&address) {
            return Followed::open(address);
        }
        let scheme = shown_scheme(&address);
        self.ask_open(
            OpenKindName::Link,
            format!("Open {scheme}: link"),
            format!(
                "This link uses the {scheme}: scheme and opens another program on this computer."
            ),
            None,
            address,
        )
    }

    /// UC-004 A4, BR-007: a file opens with the system's program when it
    /// exists. Q-61: one that may run as a program (`.desktop`, `.exe`,
    /// `.sh`, a file with an execute permission, …) asks "Open program"
    /// first, and any `file://` target (`file_url`) "Open file", naming the
    /// file and its kind, every time. A place on another computer or a
    /// device of Windows (`\\server\share`) is asked about from its text
    /// alone, since looking at it may reach out to that computer.
    ///
    /// What is asked about is looked at once, when the link is followed;
    /// a file that changes between the question and the opening is opened
    /// as it is then.
    fn follow_to_file(&mut self, path: &Path, file_url: bool) -> Followed {
        let target = path.display().to_string();
        let name = path
            .file_name()
            .map_or_else(|| target.clone(), |n| n.to_string_lossy().into_owned());
        let name = shortened(&shown_text(&name));
        if is_remote_place(&target) {
            return match executable_by_name(path) {
                Some(kind) => self.ask_program(&name, kind, target),
                None => self.ask_open(
                    OpenKindName::File,
                    "Open file".into(),
                    format!("This link points to a file on another computer or a device: {name}."),
                    None,
                    target,
                ),
            };
        }
        if !path.exists() {
            self.status = format!("{} does not exist", path.display());
            return Followed::done();
        }
        if let Some(kind) = executable_kind(path) {
            return self.ask_program(&name, kind, target);
        }
        if file_url {
            let kind = file_kind(path);
            return self.ask_open(
                OpenKindName::File,
                "Open file".into(),
                format!("This link points to a file on this computer: {name}, {kind}."),
                None,
                target,
            );
        }
        Followed::open(target)
    }

    /// Q-61: "Open program", for `name`, which is `kind` ("a shell script").
    fn ask_program(&mut self, name: &str, kind: &str, target: String) -> Followed {
        self.ask_open(
            OpenKindName::Program,
            "Open program".into(),
            format!("{name} is {kind}."),
            Some("Opening it may run it on this computer.".into()),
            target,
        )
    }

    /// UC-004 A4, Q-61, Q-62: asks whether to open `target`; the answer
    /// comes through [`Session::answer_question`] with the question's id.
    fn ask_open(
        &mut self,
        kind: OpenKindName,
        title: String,
        question: String,
        warning: Option<String>,
        target: String,
    ) -> Followed {
        self.open_asks = self.open_asks.wrapping_add(1);
        let id = self.open_asks;
        let shown = shown_text(&target);
        self.open_asked = Some((id, target));
        Followed::answer(LinkAnswer::AskOpen {
            id,
            about: kind,
            title,
            question,
            warning,
            target: shown,
        })
    }

    /// UC-004 A4, Q-61, Q-62: the answer to the question `id` "Open …":
    /// with `open` ("Open") the address or file asked about is returned,
    /// to be opened with the system's program; "Cancel" opens nothing. An
    /// answer to a question that is no longer asked (another link was
    /// followed since) opens nothing. Nothing is remembered: the next such
    /// link is asked about again.
    pub fn answer_question(&mut self, id: u32, open: bool) -> Option<String> {
        match self.open_asked.take() {
            Some((asked, target)) if asked == id => open.then_some(target),
            other => {
                self.open_asked = other;
                None
            }
        }
    }

    /// [`Session::answer_question`] for the question asked last, in tests.
    #[cfg(test)]
    pub(crate) fn answer_open(&mut self, open: bool) -> Option<String> {
        let id = self.open_asked.as_ref()?.0;
        self.answer_question(id, open)
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

/// Q-61: what the question calls a file that does not run: "folder", or
/// "PDF file" by its ending, else "file".
fn file_kind(path: &Path) -> String {
    if path.is_dir() {
        return "folder".into();
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ending) if !ending.is_empty() => {
            format!("{} file", shown_text(&ending.to_uppercase()))
        }
        _ => "file".into(),
    }
}

/// Q-61: a name of at most 60 characters for the question, the middle
/// left out (the ending stays); the address line shows it whole.
fn shortened(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= 60 {
        return name.to_owned();
    }
    let start: String = chars[..28].iter().collect();
    let end: String = chars[chars.len() - 28..].iter().collect();
    format!("{start}…{end}")
}

#[cfg(test)]
mod tests;
