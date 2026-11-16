//! What the window and the session exchange about the links of the notes
//! (FR-078 to FR-080): the backlinks and the outgoing links of the active
//! note for the tool panel (UC-004 step 4, BR-015, A14), and the headings
//! of a note for link completion (UC-005 A23, BR-018).

use serde::Serialize;

/// UC-004 BR-015: "Backlinks (N)" and "Outgoing links (N)" of the active
/// note, sent as the event `links` when they changed. The notes open in a
/// tab count with their text, also with changes not stored yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinksView {
    /// The active note.
    pub note: String,
    /// The revision of the backend's text of the active note the outgoing
    /// links were found in; their `offset` is in that text.
    pub rev: u32,
    /// FR-078: the notes that link to the active note, in the order of the
    /// note overview, each with its links.
    pub backlinks: Vec<BacklinkNoteView>,
    /// How many backlinks there are in all, the number of the section.
    pub backlink_count: u32,
    /// FR-079: the links of the active note, in the order of the note.
    pub outgoing: Vec<OutgoingLinkView>,
}

/// FR-078: a note that links to the active note, with its links to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BacklinkNoteView {
    /// The linking note, which A14 opens.
    pub note: String,
    /// Its title.
    pub title: String,
    /// Its folder, `None` for the top folder.
    pub folder: Option<String>,
    /// Its links to the active note, in the order of the note.
    pub links: Vec<BacklinkView>,
}

/// FR-078, BR-015: one link to the active note: its line, shown with the
/// link marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BacklinkView {
    /// The line of the link, counted from 1.
    pub line: u32,
    /// The text of the line before the link, shortened at its start.
    pub before: String,
    /// The markup of the link, which the line shows marked.
    pub link: String,
    /// The text of the line after the link, shortened at its end.
    pub after: String,
    /// Where the link is in the text of its note, which A14 puts the cursor
    /// on (`links_open`).
    pub place: LinkPlace,
}

/// A14: where a backlink is in the text of the note that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkPlace {
    /// The first byte of the link.
    pub from: u32,
    /// The byte after it.
    pub to: u32,
    /// A check of the link's markup (FNV-1a), so a link that moved since
    /// the list was made is not taken for another place.
    pub check: u32,
}

impl LinkPlace {
    /// The check of the markup `link` ([`LinkPlace::check`]).
    #[must_use]
    pub fn check_of(link: &str) -> u32 {
        link.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        })
    }
}

/// FR-079: one link of the active note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OutgoingLinkView {
    /// The text of the link as the note shows it; its place when it has
    /// none.
    pub text: String,
    /// Where it leads: a note of the workspace (with the heading it names),
    /// a file, or the web address.
    pub target: String,
    /// What it leads to.
    pub kind: LinkKindName,
    /// BR-015: the note, file or heading it leads to does not exist (a
    /// broken link); a web address is never broken.
    pub broken: bool,
    /// The line of the link, counted from 1.
    pub line: u32,
    /// A14: where the link starts, in web units of `LinksView::rev`.
    pub offset: u32,
}

/// What a link leads to (the entity NOTE_LINK, `kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LinkKindName {
    /// A note of the workspace, or a heading of one.
    Note,
    /// Another file.
    File,
    /// A web address.
    Web,
}

/// UC-005 A23, BR-018: a heading of a note that link completion offers
/// after a `#`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HeadingView {
    /// The heading's text.
    pub label: String,
    /// The place of the heading as GitHub and GitLab name it, which the
    /// link names after its `#`.
    pub anchor: String,
    /// Its level, 1 for the highest.
    pub level: u32,
}
