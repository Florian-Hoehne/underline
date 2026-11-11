//! What the window and the session exchange about the git changes of the
//! workspace (UC-011 Review Changes, UC-012 Commit Changes, inventory M-19.1
//! to M-19.5, M-05.3): the changed notes and files with the commit box's
//! choice and proposal, the diff the tool panel shows, the question before
//! discarding, and the answers of a commit.

use serde::Serialize;

/// UC-011 step 1, A7, A8: where the workspace stands, as the branch button
/// of the status bar names it (M-04.1, feature F13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
    /// "{name}", "{name} (no commit yet)" or "not on a branch: {commit}",
    /// as egui's status bar writes it.
    pub label: String,
    /// A8: not on a branch.
    pub detached: bool,
}

/// How a file changed since the last commit (UC-011 BR-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ChangeKindName {
    /// Not in the last commit: the letter `N`, green.
    New,
    /// The letter `M`, amber.
    Changed,
    /// The letter `D`, red.
    Deleted,
    /// The letter `R`, blue.
    Renamed,
    /// UC-015: the letter `C`, orange (its actions come with feature F13).
    Conflict,
}

/// M-19.2: one row of "Changes (N)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    /// The file, relative to the workspace with `/`; the window sends it
    /// back to choose the change, leave it out or discard it.
    pub path: String,
    /// How it changed.
    pub kind: ChangeKindName,
    /// The letter of the mark: `N`, `M`, `D`, `R` or `C`.
    pub letter: String,
    /// "{title} · {folder}", "{title}" for a note in the top folder, "{new}
    /// ← {old}" for a renamed note, the path for another file.
    pub name: String,
    /// It is a note: notes come before "Other files", and only a note can
    /// be discarded (UC-011 BR-005).
    pub note: bool,
    /// UC-012 BR-004: chosen for the commit.
    pub chosen: bool,
}

/// UC-011, UC-012, M-19.1, M-19.2: the changes of a git workspace as the
/// tool panel shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChangesView {
    /// UC-011 A9: why git could not be asked; then nothing else is shown
    /// (the status bar says it with feature F13).
    pub problem: Option<String>,
    /// Where the workspace stands; `None` with a problem.
    pub branch: Option<BranchView>,
    /// The changed files: conflicts, then notes, then other files, each by
    /// place (UC-011 BR-002, UC-015 BR-005).
    pub files: Vec<ChangeView>,
    /// UC-012 BR-009: a merge waits for its conflicts to be resolved; the
    /// commit box gives way to a warning.
    pub merging: bool,
    /// UC-012 A6, BR-006: the message of the last commit while it may be
    /// amended.
    pub amendable: Option<String>,
    /// UC-012 BR-002: the message proposed for the changes chosen.
    pub proposed: String,
}

/// UC-011 BR-004: what a line of the diff is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    /// An unchanged line around a changed part.
    Same,
    /// A line of the last commit that is gone.
    Removed,
    /// A line that is new.
    Added,
}

/// A part of a diff line; `strong` for the words removed or added within
/// the line (BR-004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffPartView {
    /// The text.
    pub text: String,
    /// A word removed or added.
    pub strong: bool,
}

/// M-19.3: one line of the diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffLineView {
    /// What it is.
    pub kind: DiffLineKind,
    /// "{old:>4} {new:>4} {sign} ", as egui writes it before the line.
    pub numbers: String,
    /// The line without its line ending, in parts.
    pub parts: Vec<DiffPartView>,
    /// "  (only spaces differ)" or "  (only the line ending differs)".
    pub only: Option<String>,
    /// UC-011 steps 7–8: where a click puts the cursor of the tab, in web
    /// units of the revision `DiffView::rev`; `None` for a diff of a file
    /// without a tab.
    pub offset: Option<u32>,
}

/// One changed part with the lines around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HunkView {
    /// Its lines.
    pub lines: Vec<DiffLineView>,
}

/// M-19.3: what the section "Diff" shows besides the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiffBody {
    /// UC-011 A5: "This file is not text; it is {new|deleted|changed}."
    NotText {
        /// The text, as egui writes it.
        text: String,
    },
    /// A2: "The note did not change since the last commit."
    Unchanged,
    /// git could not be asked for the committed content: "The diff cannot
    /// be shown: {why}" (review F12, new text).
    Problem {
        /// The text.
        text: String,
    },
    /// The changed parts (BR-004).
    Hunks {
        /// The parts, in order.
        hunks: Vec<HunkView>,
    },
}

/// M-19.3, UC-011 step 6: the diff of the chosen file or the active note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffView {
    /// The file, relative to the workspace, shown above the diff.
    pub path: String,
    /// The note of the tab whose text it shows; `None` for a file chosen
    /// in the list without a tab (A4, A5).
    pub note: Option<String>,
    /// The revision of the tab's text the diff was made of.
    pub rev: Option<u32>,
    /// What it shows.
    pub body: DiffBody,
}

/// UC-011 A6 step 1, M-19.5: the question "Discard changes".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiscardQuestion {
    /// The file, which the window sends back with "Discard".
    pub path: String,
    /// "Discard the changes of {title} since the last commit?"
    pub question: String,
    /// "The note is new; it goes to the deleted notes.", "The note comes
    /// back as it was committed." or "N changed line(s) will be lost and
    /// cannot be restored."
    pub consequence: String,
}

/// The answer to "Commit" (UC-012 steps 4–7, A1 to A6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CommitAnswer {
    /// Nothing more for the commit box; the status says what happened (A4,
    /// A5, a refused name).
    Done,
    /// Step 7: committed; the box empties.
    Committed,
    /// A1, A2: refused; the box says why and keeps the message.
    Problem {
        /// "A commit needs a message." or "Nothing is chosen."
        problem: String,
    },
    /// A3: git does not know the user; the window asks "Your name for
    /// commits".
    Identity,
    /// A3: the name and email could not be kept; "Your name for commits"
    /// stays open with what was typed and says why.
    NameNotKept {
        /// "Your name could not be kept: {err}".
        problem: String,
    },
}
