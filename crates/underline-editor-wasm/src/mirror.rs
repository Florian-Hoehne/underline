//! ADR-025: the backend's mirror (`underline_editor_core::sync::Server`)
//! in the webview, for the window's mock backend on the dev server
//! (`ui/src/lib/mock-backend.ts`): the same revisions, transforms and
//! reload barrier as the real backend, without a Tauri window.

use underline_editor_core::sync::{Batch, Server};
use underline_editor_view::sync_view::{SyncAnswer, SyncBatch};

use crate::calls::CallError;

/// The mirror of one note.
#[derive(Debug)]
pub struct Mirror(Server);

fn json<T: serde::Serialize>(value: &T) -> Result<String, CallError> {
    serde_json::to_string(value).map_err(CallError::Answer)
}

impl Mirror {
    /// The mirror of `text` at revision 1.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(Server::new(text))
    }

    /// The revision.
    #[must_use]
    pub fn rev(&self) -> u32 {
        self.0.rev()
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> String {
        self.0.text()
    }

    /// `{ answer: SyncAnswer, reverted }` to `batch` (`SyncBatch`).
    ///
    /// # Errors
    ///
    /// `batch` is not a `SyncBatch`.
    pub fn receive(&mut self, batch: &str) -> Result<String, CallError> {
        let batch: SyncBatch =
            serde_json::from_str(batch).map_err(|reason| CallError::Invalid {
                what: "The batch",
                reason,
            })?;
        let received = self.0.receive(&Batch::from(&batch));
        json(&serde_json::json!({
            "answer": SyncAnswer::from(&received.answer),
            "reverted": received.reverted,
        }))
    }

    /// A change of the backend's own: `remove` chars at char `at` replaced
    /// by `insert`; the new revision, or `None` when it does not fit.
    pub fn backend_edit(
        &mut self,
        at: usize,
        remove: usize,
        insert: &str,
        sealed: bool,
    ) -> Option<u32> {
        self.0
            .backend_edit(
                &[(at..at.saturating_add(remove), insert.to_owned())],
                sealed,
            )
            .ok()
            .map(|(rev, _)| rev)
    }

    /// A change from outside: the text is `text` now.
    pub fn reload(&mut self, text: &str) -> u32 {
        self.0.reload(text)
    }

    /// `SyncAnswer` after the window's `text` was taken.
    ///
    /// # Errors
    ///
    /// The answer could not be written.
    pub fn resync(&mut self, text: &str) -> Result<String, CallError> {
        json(&SyncAnswer::from(&self.0.resync(text).answer))
    }
}
