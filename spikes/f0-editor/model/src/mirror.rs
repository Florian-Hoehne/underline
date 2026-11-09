//! Variant B: the backend's copy of the text, fed with the edits the WASM
//! owner made, each carrying the revision it was made on.

use serde::{Deserialize, Serialize};
use underline_editor_core::Document;

/// One edit of the WASM owner: on revision `base`, the `remove` chars at
/// char index `at` were replaced by `insert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorEdit {
    pub base: u64,
    pub at: usize,
    pub remove: usize,
    pub insert: String,
}

/// Why the backend could not take an edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MirrorError {
    /// The edit was made on another revision than the backend's (a real
    /// app would rebase here; the spike only reports it).
    Revision { expected: u64, got: u64 },
    /// The edit reaches past the end of the text.
    OutOfRange {
        at: usize,
        remove: usize,
        len: usize,
    },
}

/// The backend's copy of the text in variant B.
#[derive(Debug, Default)]
pub struct Mirror {
    doc: Document,
}

impl Mirror {
    /// A copy holding `text` at revision 0, like a fresh owner.
    pub fn new(text: &str) -> Self {
        Self {
            doc: Document::new(text),
        }
    }

    /// Applies `edits` in order; returns the revision reached.
    ///
    /// # Errors
    /// The first edit made on another revision, or out of range.
    pub fn apply(&mut self, edits: &[MirrorEdit]) -> Result<u64, MirrorError> {
        for edit in edits {
            if edit.base != self.doc.revision() {
                return Err(MirrorError::Revision {
                    expected: self.doc.revision(),
                    got: edit.base,
                });
            }
            let len = self.doc.len_chars();
            if edit.at + edit.remove > len {
                return Err(MirrorError::OutOfRange {
                    at: edit.at,
                    remove: edit.remove,
                    len,
                });
            }
            self.doc
                .replace(edit.at..edit.at + edit.remove, &edit.insert);
        }
        Ok(self.doc.revision())
    }

    /// The backend's text.
    pub fn text(&self) -> String {
        self.doc.text()
    }

    /// The backend's revision.
    pub fn revision(&self) -> u64 {
        self.doc.revision()
    }
}
