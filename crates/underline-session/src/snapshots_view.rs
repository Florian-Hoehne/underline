//! What the window and the session exchange about the snapshots of a note
//! (UC-026, FR-101, FR-102): the list, the differences of one snapshot
//! with the note, and the whole snapshot.

use serde::Serialize;

use crate::facade_view::BlockView;
use crate::git_view::HunkView;

/// UC-026 step 2, A1: the snapshots of a note, the newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsView {
    /// The note.
    pub note: String,
    /// Its title, for the dialog's heading.
    pub title: String,
    /// The snapshots, the newest first.
    pub items: Vec<SnapshotItemView>,
    /// A1: what the dialog says without a snapshot: none yet, and when
    /// they are taken (BR-001).
    pub none: Option<SnapshotsNoneView>,
    /// BR-002: how long snapshots are kept, "Kept on this computer for 30
    /// days".
    pub kept: String,
    /// Why the list could not be read; the dialog shows it instead.
    pub problem: Option<String>,
}

/// UC-026 A1: a note without snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsNoneView {
    /// "This note has no snapshot yet. Underline takes one:"
    pub lead: String,
    /// When snapshots are taken (BR-001), one line each.
    pub when: Vec<String>,
}

/// UC-026 step 2: one snapshot of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotItemView {
    /// The name that tells it from the note's others, sent back.
    pub key: String,
    /// The day and time it was taken, `2026-11-11 09:30:05`.
    pub taken: String,
    /// Why it was taken: "Editing", "Outside change", "Agent change",
    /// "Replace in workspace", "Git" or "Restore" (BR-001).
    pub cause: String,
    /// How many lines it differs from the note as it is now; `None` when it
    /// cannot be read, or was not compared (NFR-005: only the newest are, in
    /// a bounded time).
    pub differs: Option<u32>,
    /// Why it cannot be read, as "The snapshot is damaged: it holds no
    /// text"; it cannot be restored then.
    pub problem: Option<String>,
}

/// UC-026 step 4: what the differences of a snapshot show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SnapshotDiffBody {
    /// The snapshot holds what the note holds now.
    Same,
    /// The snapshot or the note cannot be read.
    Problem {
        /// Why.
        text: String,
    },
    /// The changed parts: lines of the snapshot taken away, lines of the
    /// note added, as in the diff of the workspace (UC-011).
    Hunks {
        /// The parts, in order.
        hunks: Vec<HunkView>,
    },
}

/// UC-026 step 4, A4: the differences between a snapshot and the note as
/// it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiffView {
    /// The note.
    pub note: String,
    /// The snapshot.
    pub key: String,
    /// What it shows.
    pub body: SnapshotDiffBody,
}

/// UC-026 A2: the whole snapshot as the facade shows it, read only.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotWholeView {
    /// The note.
    pub note: String,
    /// The snapshot.
    pub key: String,
    /// Its blocks; empty with a problem.
    pub blocks: Vec<BlockView>,
    /// Why it cannot be shown.
    pub problem: Option<String>,
}
