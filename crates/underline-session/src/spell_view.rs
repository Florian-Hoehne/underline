//! FR-099, UC-005 BR-023, A28 (ADR-030): what the window learns of the
//! spell check.

use serde::Serialize;

/// UC-005 BR-023: the words of a note the spell check marks, for the
/// revision of the mirror they were found in. The window shows them once
/// its editor core is in step with that revision; until then the marks it
/// shows move along with its edits (ADR-030, as the colours of ADR-025).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SpellMarksView {
    /// The note.
    pub note: String,
    /// The revision of the text the marks are in.
    pub rev: u32,
    /// The marked words, `[from, to]` in web units of the markup, sorted;
    /// at most [`SpellMarksView::MAX`]. Empty when the spell check is off
    /// or has no dictionary.
    pub marks: Vec<(u32, u32)>,
}

impl SpellMarksView {
    /// The most marks sent for one note, so the event stays bounded in a
    /// note that is not in the language of the spell check.
    pub const MAX: usize = 20_000;
}

/// FR-100, UC-005 BR-023: what the status bar shows of the spell check: a
/// problem that lasts, such as a language without a dictionary; `None`
/// while there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SpellStateView {
    /// The short text of the status bar, such as "Spell check: French
    /// (France), no dictionary".
    pub problem: Option<String>,
    /// The longer text for its hover and screen readers, saying what to do.
    pub detail: Option<String>,
}
