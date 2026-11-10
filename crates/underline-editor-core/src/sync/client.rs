//! ADR-025: the webview's side of the sync (`docs/migration/variante-b.md`,
//! section 3.3): the text the backend has (the shadow), the edits sent and
//! not confirmed, the edits made since. The backend's changes are moved
//! over the window's own edits before the window applies them.

use ropey::Rope;

use super::changeset::{Change, ChangeSet, OutOfRange};
use super::hash_rope;
use super::server::{Answer, Batch, Remote};

/// A backend change for the window's editor, moved over its own edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// Edits of the window's text as it is now ([`super::apply_remote`]).
    Edits {
        /// The edits, as one set over the window's text.
        set: ChangeSet,
        /// An undo step of its own (an agent's change).
        sealed: bool,
    },
    /// The text is `text` now; the undo history starts anew, the cursor
    /// stays at its line and column (`SourceEditor::reload`).
    Reload {
        /// The new text.
        text: String,
    },
}

/// The two copies of the text went apart; the window sends its whole text
/// (`docs/migration/variante-b.md` section 3.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desync {
    /// The backend asked for the whole text.
    Asked,
    /// The backend's text has another hash than the window's copy of it.
    Hash,
    /// A change of the backend does not fit the window's copy of its text.
    Unfit(OutOfRange),
}

/// The webview's side of the sync of one open note.
#[derive(Debug, Clone)]
pub struct Client {
    /// The backend's text at `synced`.
    shadow: Rope,
    synced: u32,
    /// The edits sent and not confirmed, on the text at `synced`.
    inflight: Option<ChangeSet>,
    /// The edits made since, on the text after `inflight`.
    buffer: ChangeSet,
    /// The next outbox is sent even without edits, to fetch or confirm.
    pull: bool,
    /// The number of the last batch made ([`Batch::seq`]).
    seq: u32,
    /// The batch on its way is sent again as it was (its answer did not
    /// come).
    resend: bool,
}

impl Client {
    /// The side of a note whose text is `text` at revision `rev`.
    #[must_use]
    pub fn new(text: &str, rev: u32) -> Self {
        let shadow = Rope::from_str(text);
        let len = shadow.len_chars();
        Self {
            shadow,
            synced: rev,
            inflight: None,
            buffer: ChangeSet::identity(len),
            pull: false,
            seq: 0,
            resend: false,
        }
    }

    /// The last revision the backend confirmed.
    #[must_use]
    pub fn synced(&self) -> u32 {
        self.synced
    }

    /// Whether the window shows the backend's text of revision `rev` and
    /// nothing of its own: a result of the backend for `rev` applies as it
    /// is.
    #[must_use]
    pub fn in_step(&self, rev: u32) -> bool {
        rev == self.synced && self.inflight.is_none() && self.buffer.is_identity()
    }

    /// Whether nothing waits: no batch on its way, no edits to send, no
    /// fetch asked for.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.inflight.is_none() && self.buffer.is_identity() && !self.pull
    }

    /// The window made `change` on its text.
    ///
    /// # Errors
    ///
    /// The change does not fit the window's text as the client knows it.
    pub fn local(&mut self, change: &Change) -> Result<(), OutOfRange> {
        let set = ChangeSet::of(self.buffer.target_len(), change)?;
        self.buffer = self.buffer.compose(&set);
        Ok(())
    }

    /// The backend rang (`ed-changed`): the next outbox fetches its changes.
    pub fn ring(&mut self) {
        self.pull = true;
    }

    /// The edits to send, unless a batch is on its way or there is nothing
    /// to send.
    pub fn take_outbox(&mut self) -> Option<Batch> {
        if let Some(sent) = &self.inflight {
            // A batch whose answer did not come goes again as it was, with
            // its number, so the backend takes it in once.
            return std::mem::take(&mut self.resend).then(|| Batch {
                base: self.synced,
                changes: sent.changes(),
                seq: self.seq,
            });
        }
        if self.buffer.is_identity() && !self.pull {
            return None;
        }
        self.pull = false;
        let len = self.buffer.target_len();
        let sent = std::mem::replace(&mut self.buffer, ChangeSet::identity(len));
        self.seq = self.seq.wrapping_add(1).max(1);
        let batch = Batch {
            base: self.synced,
            changes: sent.changes(),
            seq: self.seq,
        };
        self.inflight = Some(sent);
        Some(batch)
    }

    /// Takes in the backend's `answer` to the last batch: its changes,
    /// moved over the window's edits, for the window's editor, in order.
    ///
    /// # Errors
    ///
    /// The copies went apart; the window sends its whole text.
    pub fn acknowledge(&mut self, answer: &Answer) -> Result<Vec<Incoming>, Desync> {
        if answer.resync {
            return Err(Desync::Asked);
        }
        let len = self.shadow.len_chars();
        self.resend = false;
        let mut inflight = self
            .inflight
            .take()
            .unwrap_or_else(|| ChangeSet::identity(len));
        let mut incoming = Vec::new();
        for (rev, remote) in &answer.remote {
            match remote {
                Remote::Edits { changes, sealed } => {
                    let theirs = ChangeSet::of_changes(self.shadow.len_chars(), changes)
                        .map_err(Desync::Unfit)?;
                    let (over_sent, sent) = ChangeSet::transform(&theirs, &inflight);
                    let (over_mine, mine) = ChangeSet::transform(&over_sent, &self.buffer);
                    inflight = sent;
                    self.buffer = mine;
                    theirs.apply(&mut self.shadow).map_err(Desync::Unfit)?;
                    incoming.push(Incoming::Edits {
                        set: over_mine,
                        sealed: *sealed,
                    });
                }
                Remote::Reload { text } => {
                    if !inflight.is_identity() {
                        // The backend takes a reload back for edits made
                        // before it; it never sends one with them.
                        return Err(Desync::Hash);
                    }
                    if !self.buffer.is_identity() {
                        // UC-004 A9: the window's own edits keep the text;
                        // they go to the backend on the revision before the
                        // reload, which takes it back. Should they come to
                        // nothing meanwhile, the next outbox fetches the
                        // reload again.
                        self.pull = true;
                        return Ok(incoming);
                    }
                    self.shadow = Rope::from_str(text);
                    let len = self.shadow.len_chars();
                    inflight = ChangeSet::identity(len);
                    self.buffer = ChangeSet::identity(len);
                    // Confirms the reload with the next outbox.
                    self.pull = true;
                    incoming.push(Incoming::Reload { text: text.clone() });
                }
            }
            self.synced = *rev;
        }
        inflight.apply(&mut self.shadow).map_err(Desync::Unfit)?;
        self.synced = answer.rev;
        if hash_rope(&self.shadow) != answer.hash {
            return Err(Desync::Hash);
        }
        Ok(incoming)
    }

    /// The last batch did not reach the backend, or its answer did not come
    /// (an IPC error of `ed_sync`; the window calls this on any error of
    /// it): the next outbox sends it again as it was, with its number; the
    /// backend takes it in once ([`Batch::seq`]). What was typed since
    /// follows in the batch after.
    pub fn failed(&mut self) {
        self.resend = self.inflight.is_some();
        self.pull = true;
    }

    /// The text the window must have: the backend's at `synced` with the
    /// edits sent and made since (for the tests).
    #[cfg(test)]
    pub(super) fn projected(&self) -> String {
        let mut rope = self.shadow.clone();
        if let Some(sent) = &self.inflight {
            let _ = sent.apply(&mut rope);
        }
        let _ = self.buffer.apply(&mut rope);
        rope.to_string()
    }

    /// After a resync: the backend took `sent`, the window's text when it
    /// was sent, at `rev`. What the window typed while the resync travelled
    /// (its text is `now`) is kept as edits still to send.
    pub fn reset(&mut self, sent: &str, rev: u32, now: &str) {
        *self = Self::new(sent, rev);
        if sent != now {
            let change = difference(sent, now);
            // `difference` fits `sent` by its making.
            let _ = self.local(&change);
        }
    }
}

/// The one edit that makes `after` of `before`: the middle between their
/// common start and end, in chars.
#[must_use]
pub fn difference(before: &str, after: &str) -> Change {
    let prefix = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let before_len = before.chars().count();
    let after_len = after.chars().count();
    let suffix = before
        .chars()
        .rev()
        .zip(after.chars().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(before_len - prefix)
        .min(after_len - prefix);
    Change {
        at: prefix,
        remove: before_len - prefix - suffix,
        insert: after
            .chars()
            .skip(prefix)
            .take(after_len - prefix - suffix)
            .collect(),
    }
}
