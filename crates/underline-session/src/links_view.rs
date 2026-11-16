//! What the window and the session exchange about the links of the notes
//! (FR-078 to FR-080, FR-103): the backlinks, the outgoing links and the
//! unlinked mentions of the active note for the tool panel (UC-004 step 4,
//! BR-015, BR-021, A14, A22), and the headings of a note for link
//! completion (UC-005 A23, BR-018).

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
    /// FR-103, BR-021: the unlinked mentions of the active note's title.
    pub mentions: MentionsView,
}

/// FR-103, UC-004 BR-021: "Unlinked mentions (N)": the places in the other
/// notes where the active note's title stands as plain text, grouped by
/// the note that holds them, in the order of the note overview.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MentionsView {
    /// The notes with mentions, each with those listed.
    pub notes: Vec<MentionNoteView>,
    /// How many mentions there are in all, the number of the section; more
    /// than are listed when `more` is set.
    pub count: u32,
    /// How many more there are than are listed; "Show more" lists them.
    pub more: u32,
    /// BR-021: the title is shorter than [`MENTION_LEAST_CHARS`] characters,
    /// so no mention of it is looked for.
    pub too_short: bool,
}

/// BR-021: titles with fewer characters than this (spaces not counted)
/// have no unlinked mentions.
pub use underline_facade::mentions::LEAST_CHARS as MENTION_LEAST_CHARS;

/// FR-103: a note whose text mentions the active note's title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MentionNoteView {
    /// The mentioning note, which A22 changes.
    pub note: String,
    /// Its title.
    pub title: String,
    /// Its folder, `None` for the top folder.
    pub folder: Option<String>,
    /// Its mentions, in the order of the note.
    pub mentions: Vec<MentionView>,
}

/// FR-103, BR-021: one mention: its line, shown with the mention marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MentionView {
    /// The line of the mention, counted from 1.
    pub line: u32,
    /// The text of the line before the mention, shortened at its start.
    pub before: String,
    /// The mention as the note writes it.
    pub mention: String,
    /// The text of the line after it, shortened at its end.
    pub after: String,
    /// Where it is in the text of its note, which A22 turns into a link
    /// (`links_link_mention`) and A14 opens.
    pub place: LinkPlace,
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
    /// OQ-244: a link, a picture or an included file.
    pub form: LinkFormName,
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
    /// OQ-244: a link, a picture or an included file.
    pub form: LinkFormName,
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
    /// OQ-244: a picture held in the note itself (`data:`), which is never
    /// broken and never followed.
    Inline,
}

/// OQ-244, UC-004 BR-015: how a note names what it links to: a link (or a
/// web address written out), a picture shown in it, or a file it includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LinkFormName {
    /// A link: `[text](place)`, `link:`, `xref:`, `<<…>>`, an address.
    Link,
    /// A picture: `![alt](place)`, `image:`, `image::`.
    Picture,
    /// An included file: AsciiDoc `include::`.
    Include,
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
