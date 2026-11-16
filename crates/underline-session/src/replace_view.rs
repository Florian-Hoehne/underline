//! What the window and the session exchange about the replace of the
//! workspace (UC-022, FR-074): the places a replacement changes, each with
//! its line as it is and as it will be, the notes and places the user left
//! out, page by page.

use serde::Serialize;

/// UC-022 step 2, A1, A2: the replace of the workspace. Sent as the event
/// `replace` when its problem or its result changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReplaceView {
    /// The number of the newest `replace_set`; a result of an older one is
    /// of another text.
    pub asked: u32,
    /// A2: what is wrong with the search text or the replacement; no
    /// places are found then.
    pub problem: Option<String>,
    /// The newest places; `None` until the first came, or while nothing is
    /// searched. While a new text is searched, the places of the text
    /// before stay.
    pub result: Option<ReplaceResultView>,
}

/// UC-022 step 2, A7: the places found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReplaceResultView {
    /// The number of this result in the session; the window asks for more
    /// of its notes, leaves out and confirms by it.
    pub serial: u32,
    /// The `replace_set` it answers.
    pub asked: u32,
    /// How many places there are in all.
    pub places: u32,
    /// In how many notes.
    pub notes: u32,
    /// "{n} places in {m} notes".
    pub summary: String,
    /// A7: "{n} more places are not shown; they are replaced too.", when
    /// more than [`underline_core::SHOWN_REPLACE_PLACES`] are found.
    pub more: Option<String>,
    /// The first notes with their places shown, [`ReplaceResultView::PAGE`]
    /// places or a little more.
    pub list: Vec<ReplaceNoteView>,
    /// How many notes the list holds in all; the window asks for the
    /// others (`replace_page`).
    pub listed: u32,
    /// "{n} note(s) could not be searched.", when some could not.
    pub unreadable: Option<String>,
    /// Step 3: what is chosen to be replaced now.
    pub chosen: ChosenView,
}

/// UC-022 step 3, 4: how many places in how many notes are chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChosenView {
    /// How many places are chosen.
    pub places: u32,
    /// In how many notes.
    pub notes: u32,
    /// "{k} of {n} places chosen".
    pub summary: String,
    /// What the button says: "Replace {k} places in {m} notes", or
    /// "Replace" when nothing is chosen.
    pub action: String,
}

impl ReplaceResultView {
    /// About how many places one page of notes holds.
    pub const PAGE: usize = 1_000;
}

/// UC-022 step 2, 3: a note with its places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReplaceNoteView {
    /// The note.
    pub note: String,
    /// Its title.
    pub title: String,
    /// Its folder; `None` in the top folder.
    pub folder: Option<String>,
    /// "1 place" or "{n} places".
    pub count: String,
    /// Step 3: the user left the whole note out.
    pub left_out: bool,
    /// Its places shown (A7: some may not be).
    pub places: Vec<ReplacePlaceView>,
}

/// UC-022 step 2: a place with its line before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReplacePlaceView {
    /// Where the place starts, in bytes of the text searched: what the
    /// window names it by.
    pub at: u32,
    /// The number of its line, from 1.
    pub line: u32,
    /// The text before it in its line, shortened with "…" when long.
    pub before: String,
    /// The place found.
    pub old: String,
    /// What takes its place.
    pub new: String,
    /// The text after it in its line.
    pub after: String,
    /// Step 3: the user left it out.
    pub left_out: bool,
}
