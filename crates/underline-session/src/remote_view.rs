//! What the window and the session exchange about branches, syncs and
//! conflicts (feature F13: UC-013 Manage Branches, UC-014 Synchronize with
//! Remote, UC-015 Resolve Sync Conflict; inventory M-04.1 to M-04.3, M-19.6
//! to M-19.10): the git part of the status bar, the branch menu, the
//! conflict bar of a note and the todos in conflict. The texts are egui's.

use serde::{Deserialize, Serialize};

/// M-04.1 to M-04.3: the git part of the status bar, left of the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitBarView {
    /// M-04.1, UC-011 step 1: "Branch {b}" or "Branch {b} · N change(s)",
    /// the button that opens the branch menu; only while git answers.
    pub branch: Option<String>,
    /// M-04.1, UC-011 A9: "Changes cannot be shown: {e}", weak.
    pub problem: Option<String>,
    /// M-04.2, UC-015 step 1: "1 conflict" or "N conflicts", with "Abort
    /// the sync" beside it.
    pub conflicts: Option<String>,
    /// M-04.3: the state of the syncs, only for a workspace with a remote.
    pub sync: Option<SyncBarView>,
}

/// M-04.3, UC-014 BR-004: the state of the syncs and "Sync now".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncBarView {
    /// "Syncing…", "Not synced yet", "Synced just now", "Offline, will
    /// retry (synced 2 min ago)", "Sync failed", "Sync failed: signing in",
    /// "Sync stopped: conflict", "Sync stopped", "Push refused" or "No
    /// sync".
    pub text: String,
    /// In egui's warning colour.
    pub warning: bool,
    /// The reason or the files, shown at the pointer.
    pub hover: Option<String>,
    /// "N to push", when commits wait to be pushed.
    pub to_push: Option<String>,
    /// "N to pull", when commits wait to be pulled.
    pub to_pull: Option<String>,
    /// A sync runs.
    pub running: bool,
    /// "Sync now" can be chosen: no sync runs and no conflict waits (A10).
    pub can_sync: bool,
}

/// UC-014 step 1, A1: what a sync does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncModeName {
    /// "Sync now": commit, pull, push.
    Full,
    /// "Pull" of the branch menu: commit and pull.
    Pull,
    /// "Push" of the branch menu: commit and push.
    Push,
}

/// M-19.6, UC-013 step 2, BR-001: one entry of the branch menu's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchEntryView {
    /// The branch, `origin/review` for one of the remote.
    pub name: String,
    /// "{name}  (current)", "{name}  (on the remote)" or the name.
    pub label: String,
    /// The branch the workspace is on.
    pub current: bool,
    /// A branch of the remote without a local one (A4).
    pub remote: bool,
}

/// M-19.6, UC-013 step 2: what the branch menu lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchMenuView {
    /// The local branches, then those of the remote (BR-001).
    pub branches: Vec<BranchEntryView>,
    /// Why git could not list them.
    pub problem: Option<String>,
    /// A7: a sync left notes in conflict; no switch and no new branch.
    pub merging: bool,
}

/// M-19.6, UC-013 A1, BR-005: the name of a new branch as typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchNameView {
    /// "Branch: {clean}" (weak), or "{problem}." (a warning).
    pub text: String,
    /// The name is allowed; "Create" can be chosen.
    pub ok: bool,
}

/// M-19.6, M-19.7: the branch chosen in the menu, or the new one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BranchTarget {
    /// A local branch (step 3).
    Local {
        /// Its name.
        name: String,
    },
    /// A4: a branch of the remote, `origin/review`.
    Remote {
        /// Its name with the remote's.
        name: String,
    },
    /// A1: a new branch, as typed.
    New {
        /// The name typed (spaces become `-`, BR-005).
        name: String,
    },
}

/// The answer to choosing a branch in the menu (UC-013 steps 3–6, A1 to A6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SwitchAnswer {
    /// The menu closes; the status says what happened.
    Done,
    /// A3: "Changes not committed would be overwritten: {files}. Commit
    /// them first, or stay." with "Commit first"; the menu stays.
    InTheWay {
        /// The text.
        text: String,
    },
    /// A2: "The branch {name} exists; choose it in the list to switch to
    /// it."; the menu shows the list again, filtered by the name.
    Exists {
        /// The text.
        problem: String,
        /// The name, for the filter.
        name: String,
    },
    /// BR-005: the name typed is not allowed ("{problem}."); the menu
    /// keeps it.
    Invalid {
        /// The text.
        problem: String,
    },
}

/// UC-015 step 4, BR-006: a choice for a conflicting place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ChoiceName {
    /// "Here", Alt+H.
    Here,
    /// "Remote", Alt+R.
    Remote,
    /// "Both", Alt+B: here first, then remote.
    Both,
}

/// UC-015 BR-002: a side of a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SideName {
    /// This computer's version.
    Here,
    /// The version that came with the sync.
    Remote,
}

/// M-19.9, UC-015 step 3: one conflicting place of the active note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictPlaceView {
    /// The line it starts on, from 1.
    pub line: u32,
    /// Where it starts, in web units of [`ConflictBarView::rev`].
    pub offset: u32,
    /// "Here", without its last line break.
    pub here: String,
    /// "Remote", likewise.
    pub remote: String,
}

/// M-19.9, UC-015 steps 3–5: the conflict bar over the active note, while
/// it holds conflicting places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictBarView {
    /// The note.
    pub note: String,
    /// The revision of the backend's text the places are of.
    pub rev: u32,
    /// The open places, in order; never empty.
    pub places: Vec<ConflictPlaceView>,
}

/// M-19.10, UC-020 A10: a row of a todo in conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoConflictRowView {
    /// "Title", "Due", "Scheduled", "Urgency", "Status" or "Notes".
    pub name: String,
    /// This computer's value, "–" for none, "(deleted)".
    pub here: String,
    /// The remote's.
    pub remote: String,
    /// The two differ: both in bold.
    pub differ: bool,
}

/// M-19.10, UC-020 A10: a todo changed here and on the remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoConflictView {
    /// Its number, sent back with "Keep mine" or "Take remote".
    pub number: u32,
    /// "Todo #{n} was changed here and on the remote".
    pub heading: String,
    /// Both versions, field by field.
    pub rows: Vec<TodoConflictRowView>,
}
