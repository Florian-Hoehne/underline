//! Test helpers (ADR-025): what a window's editor core sends the session.
//! The window holds the text and sends its edits as batches (`ed_sync`);
//! the rules of each input are the editor core's and are tested there
//! (`underline-editor-view`, `underline-editor-wasm`).

use underline_core::NoteId;

use super::Session;
use crate::sync_view::{SyncAnswer, SyncBatch, SyncEdit};

impl Session {
    /// As a window's core: `remove` chars at char `at` of the mirror's text
    /// (a `\r\n` pair is two) replaced by `insert`, sent as one batch; a
    /// copy to keep is written as the command writes it.
    pub(crate) fn window_edit(
        &mut self,
        note: &NoteId,
        at: usize,
        remove: usize,
        insert: &str,
    ) -> SyncAnswer {
        let rev = self.mirror_of(note).rev();
        let batch = SyncBatch {
            base: rev,
            edits: vec![SyncEdit {
                at: u32::try_from(at).unwrap(),
                remove: u32::try_from(remove).unwrap(),
                insert: insert.to_owned(),
            }],
            // Without a number: every batch is taken in.
            seq: 0,
        };
        let synced = self.editor_sync(note, &batch).expect("the tab is open");
        if let Some(copy) = &synced.copy {
            let written = copy.write();
            self.copy_kept(copy, written);
        }
        synced.answer
    }

    /// As a window's core: `insert` typed at char `at`.
    pub(crate) fn window_type(&mut self, note: &NoteId, at: usize, insert: &str) -> SyncAnswer {
        self.window_edit(note, at, 0, insert)
    }

    /// The backend's text of `note` (its mirror), as the window's core
    /// holds it after a sync.
    pub(crate) fn window_text(&self, note: &NoteId) -> String {
        self.mirror_of(note).text()
    }

    /// The revision of the backend's text of `note`, which a window's core
    /// in step with it sends along.
    pub(crate) fn mirror_rev(&self, note: &NoteId) -> u32 {
        self.mirror_of(note).rev()
    }

    fn mirror_of(&self, note: &NoteId) -> &underline_editor_core::sync::Server {
        let open = self.open.as_ref().expect("a workspace is open");
        let index = open.tabs.find(note).expect("the tab is open");
        &open.tabs.get(index).mirror
    }
}
