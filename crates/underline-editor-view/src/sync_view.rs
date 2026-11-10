//! ADR-025: what the window and the backend exchange to keep their copies
//! of an open note's text alike (`docs/migration/variante-b.md`, sections
//! 2.3, 3.2 and 3.4). Edits count chars of the rope, not web units: both
//! ends hold the same rope, and a `\r\n` stays two chars (NFR-007).

use serde::{Deserialize, Serialize};
use underline_editor_core::sync::{Answer, Batch, Change, Remote};

use crate::editor_view::units;
use crate::facade_view::{CodeColors, FigureView};

/// One edit: at char `at`, `remove` chars are replaced by `insert`. The
/// edits of a list apply one after the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncEdit {
    /// Where the edit starts, in chars.
    pub at: u32,
    /// How many chars it removes.
    pub remove: u32,
    /// What it puts in their place.
    pub insert: String,
}

impl From<&SyncEdit> for Change {
    fn from(edit: &SyncEdit) -> Self {
        Self {
            at: edit.at as usize,
            remove: edit.remove as usize,
            insert: edit.insert.clone(),
        }
    }
}

impl From<&Change> for SyncEdit {
    fn from(change: &Change) -> Self {
        Self {
            at: units(change.at),
            remove: units(change.remove),
            insert: change.insert.clone(),
        }
    }
}

/// The window's edits since revision `base`, sent after the frame
/// (`ed_sync`); none to fetch the backend's changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncBatch {
    /// The revision the edits were made on.
    pub base: u32,
    /// The edits, one after the other.
    pub edits: Vec<SyncEdit>,
    /// The batch's number; one sent again after its answer was lost is taken
    /// in once.
    pub seq: u32,
}

impl From<&SyncBatch> for Batch {
    fn from(batch: &SyncBatch) -> Self {
        Self {
            base: batch.base,
            changes: batch.edits.iter().map(Change::from).collect(),
            seq: batch.seq,
        }
    }
}

impl From<&Batch> for SyncBatch {
    fn from(batch: &Batch) -> Self {
        Self {
            base: batch.base,
            edits: batch.changes.iter().map(SyncEdit::from).collect(),
            seq: batch.seq,
        }
    }
}

/// A change of the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemoteChange {
    /// Edits one after the other; `sealed` makes them an undo step of their
    /// own (an agent's change, UC-019 BR-007).
    Edits {
        /// The edits.
        edits: Vec<SyncEdit>,
        /// Whether they are an undo step of their own.
        sealed: bool,
    },
    /// The text is replaced, as after a change from outside (UC-004 A7).
    Reload {
        /// The new text.
        text: String,
    },
}

/// A change of the backend with its revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteOp {
    /// The revision the change made.
    pub rev: u32,
    /// The change.
    pub change: RemoteChange,
}

/// The backend's answer to `ed_sync` and `ed_resync`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncAnswer {
    /// The revision of the backend's text now.
    pub rev: u32,
    /// The hash of that text (FNV-1a, 16 hex digits).
    pub hash: String,
    /// The backend's changes the window has not had, in order.
    pub remote: Vec<RemoteOp>,
    /// The window must send its whole text (`ed_resync`).
    pub resync: bool,
}

impl From<&Answer> for SyncAnswer {
    fn from(answer: &Answer) -> Self {
        Self {
            rev: answer.rev,
            hash: answer.hash.clone(),
            remote: answer
                .remote
                .iter()
                .map(|(rev, remote)| RemoteOp {
                    rev: *rev,
                    change: match remote {
                        Remote::Edits { changes, sealed } => RemoteChange::Edits {
                            edits: changes.iter().map(SyncEdit::from).collect(),
                            sealed: *sealed,
                        },
                        Remote::Reload { text } => RemoteChange::Reload { text: text.clone() },
                    },
                })
                .collect(),
            resync: answer.resync,
        }
    }
}

impl From<&SyncAnswer> for Answer {
    fn from(answer: &SyncAnswer) -> Self {
        Self {
            rev: answer.rev,
            hash: answer.hash.clone(),
            remote: answer
                .remote
                .iter()
                .map(|op| {
                    (
                        op.rev,
                        match &op.change {
                            RemoteChange::Edits { edits, sealed } => Remote::Edits {
                                changes: edits.iter().map(Change::from).collect(),
                                sealed: *sealed,
                            },
                            RemoteChange::Reload { text } => Remote::Reload { text: text.clone() },
                        },
                    )
                })
                .collect(),
            resync: answer.resync,
        }
    }
}

/// `ed-changed`: the backend changed the text of `note`; the window fetches
/// the change with its next `ed_sync`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChangedView {
    /// The note, by its id.
    pub note: String,
    /// The backend's revision now.
    pub rev: u32,
}

/// `ed-flush`: the window sends its edits now, even none, for `note` or for
/// every open note (before an agent's change, before closing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FlushView {
    /// The note, or `None` for all.
    pub note: Option<String>,
    /// Underline is to close: after sending, the window calls
    /// `app_request_close` (`docs/migration/variante-b.md` section 3.9).
    pub closing: bool,
}

/// ADR-025: the backend's text of an open note as the window's editor core
/// starts from it (`ed_mirror`): the text with its line breaks as they are,
/// its revision, and the cursor the tab opened with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MirrorView {
    /// The note, by its id.
    pub note: String,
    /// The revision of the text.
    pub rev: u32,
    /// The whole text, its line breaks as the file has them.
    pub text: String,
    /// "Markdown" or "AsciiDoc".
    pub format: String,
    /// The other end of the selection, in web units.
    pub anchor: u32,
    /// The cursor, in web units.
    pub head: u32,
}

/// ADR-025: what the backend's answer did in the window's editor core: the
/// patch for the source view (the backend's changes moved over the
/// window's own edits), or that the copies went apart and the window sends
/// its whole text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteApplied {
    /// The patch for the source view; `None` when nothing changed.
    pub patch: Option<crate::editor_view::EditorPatch>,
    /// The copies went apart: the window sends its whole text (`ed_resync`).
    pub desync: bool,
    /// Why they went apart, for the log.
    pub reason: Option<String>,
}

/// `ed-enrich`: what the backend made of the facade's blocks of `note`, by
/// their content (section 2.3): the colours of code (`colors_key`), the
/// figures of diagrams (`diagram_key`) and of images (by their target,
/// without the caption). `clear` drops what was sent before (another theme).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EnrichView {
    /// The note, by its id.
    pub note: String,
    /// The backend's revision the entries were made at.
    pub rev: u32,
    /// The theme the colours are for, by its name.
    pub theme: String,
    /// Whether the theme is dark, which the diagrams are drawn for.
    pub dark: bool,
    /// What was sent before is dropped first.
    pub clear: bool,
    /// The colours of code by key; `None` when nothing in it is coloured.
    pub colors: Vec<(String, Option<CodeColors>)>,
    /// The figures of diagrams by key.
    pub diagrams: Vec<(String, FigureView)>,
    /// The figures of images by target.
    pub images: Vec<(String, FigureView)>,
}

#[cfg(test)]
mod tests {
    use underline_editor_core::sync::{Client, Server};

    use super::*;

    /// ADR-025: the batches and answers cross as these DTOs and come back
    /// as they were; a round over them keeps both copies alike.
    #[test]
    fn adr025_batches_and_answers_cross_unchanged() {
        let mut server = Server::new("a😀\r\nb");
        let mut client = Client::new("a😀\r\nb", server.rev());
        client
            .local(&Change {
                at: 2,
                remove: 2,
                insert: "\n".into(),
            })
            .unwrap();
        server.backend_edit(&[(0..1, "A".into())], true).unwrap();
        let batch = SyncBatch::from(&client.take_outbox().unwrap());
        let json = serde_json::to_string(&batch).unwrap();
        let back: SyncBatch = serde_json::from_str(&json).unwrap();

        let answer = SyncAnswer::from(&server.receive(&Batch::from(&back)).answer);
        let json = serde_json::to_string(&answer).unwrap();
        assert!(json.contains("\"kind\":\"edits\""), "{json}");
        let back: SyncAnswer = serde_json::from_str(&json).unwrap();

        assert!(client.acknowledge(&Answer::from(&back)).is_ok());
        assert_eq!(server.text(), "A😀\nb");
    }
}
