//! What the window and the session exchange about the search of the
//! workspace (UC-008, inventory M-13): the found notes as the search overlay
//! shows them, page by page, and where a found note opens.

use serde::{Deserialize, Serialize};

/// UC-008 step 2, A1: the search of the overlay. Sent as the event
/// `search` when its text or its result changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchView {
    /// The text searched for, without the spaces around it; empty when
    /// nothing is searched.
    pub text: String,
    /// The newest result that fits a text searched for; `None` until the
    /// first came ("Searching…"). While a new text is searched, the result
    /// of the text before stays (egui).
    pub result: Option<SearchResultView>,
}

/// UC-008 step 2, A4: one result of the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchResultView {
    /// The number of this result in the session; the window asks for more
    /// of its notes by it (`search_page`).
    pub serial: u32,
    /// What it was searched for.
    pub text: String,
    /// How many notes were found.
    pub total: u32,
    /// The first [`SearchResultView::PAGE`] found notes, title and folder
    /// matches first (BR-003).
    pub notes: Vec<FoundNoteView>,
    /// A4: "{n} note(s) could not be searched.", when some could not.
    pub unreadable: Option<String>,
}

impl SearchResultView {
    /// How many found notes the window gets at once.
    pub const PAGE: usize = 100;
}

/// UC-008 step 2, A5: a found note with its first lines found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundNoteView {
    /// The note.
    pub note: String,
    /// Its title.
    pub title: String,
    /// Its folder; `None` in the top folder.
    pub folder: Option<String>,
    /// "in the title or folder", "1 place" or "{n} places".
    pub places: String,
    /// The first lines found (BR-004).
    pub lines: Vec<FoundLineView>,
    /// A5: how many more places the note holds than lines are shown.
    pub more: u32,
    /// Where the note opens when it is chosen itself: at its first line
    /// found, or without moving the cursor when none is shown.
    pub place: Option<FoundPlace>,
}

/// BR-004: a line found, as stored, shortened around the words, with the
/// words found marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundLineView {
    /// The line in parts, the words found marked.
    pub parts: Vec<FoundPartView>,
    /// Where the note opens when the line is chosen: its first word.
    pub place: FoundPlace,
}

/// A part of a line found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundPartView {
    /// The text.
    pub text: String,
    /// A word found, marked.
    pub hit: bool,
}

/// UC-008 step 4: a word found, as bytes of the note's text the search
/// read; the session finds it in the text the tab holds then, as egui does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FoundPlace {
    /// The first byte of the word.
    pub from: u32,
    /// The byte after it.
    pub to: u32,
}

/// UC-008 step 4: the found note is open; the word found to select, in
/// web units of the backend's text of `rev`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchOpened {
    /// The note.
    pub note: String,
    /// The backend's revision of its text.
    pub rev: u32,
    /// The start of the word.
    pub anchor: u32,
    /// Its end, where the cursor goes.
    pub head: u32,
}
