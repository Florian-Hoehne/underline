//! ADR-025: the backend's copy of an open note's text (the mirror): one line
//! of revisions per tab, the edits of the webview taken in with the revision
//! they were made on and moved over the backend's own changes since, and
//! the barrier of a reload (`docs/migration/variante-b.md`, sections 3.1,
//! 3.3, 3.6 and 3.10).

use std::collections::VecDeque;

use ropey::Rope;

use super::changeset::{Change, ChangeSet, OutOfRange};
use super::hash_rope;

/// How many revisions the backend keeps for late edits of the webview;
/// edits made on an older one are refused, and the webview sends its whole
/// text (`resync`).
pub const KEPT: usize = 1024;

/// A change of the backend, as the webview gets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Remote {
    /// Edits one after the other; `sealed` makes them an undo step of their
    /// own (an agent's change).
    Edits {
        /// The edits.
        changes: Vec<Change>,
        /// Whether they are an undo step of their own.
        sealed: bool,
    },
    /// The text was replaced, as after a change from outside.
    Reload {
        /// The new text.
        text: String,
    },
}

/// The edits of the webview, made on revision `base`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Batch {
    /// The revision the edits were made on.
    pub base: u32,
    /// The edits, one after the other.
    pub changes: Vec<Change>,
    /// The batch's number from the webview; a batch sent again with the
    /// number of the last one taken in gets that answer again and changes
    /// nothing (its answer was lost). 0: not numbered.
    pub seq: u32,
}

/// The backend's answer to a [`Batch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The revision the backend's text has now.
    pub rev: u32,
    /// The hash of that text ([`super::hash`]).
    pub hash: String,
    /// The backend's changes the webview has not had, with their revisions.
    pub remote: Vec<(u32, Remote)>,
    /// The webview must send its whole text.
    pub resync: bool,
}

/// Lines a change touched, for the highlighting: in the order the changes
/// were made, each at `line` of the text just before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Touched {
    /// The first line the change touched.
    pub line: usize,
    /// The lines it added; negative when it removed lines.
    pub added_lines: isize,
}

/// What taking in a batch did to the backend's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The answer for the webview.
    pub answer: Answer,
    /// The edits, moved over the backend's changes, as they were applied.
    pub applied: ChangeSet,
    /// The lines they touched.
    pub touched: Vec<Touched>,
    /// UC-004 A9: the batch was made before a reload, which was taken back;
    /// this is the text the mirror had (the newest from outside, with what
    /// the backend did to it since), to keep as a copy: nothing that may
    /// have been stored is lost.
    pub reverted: Option<String>,
}

/// What [`Server::resync`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resynced {
    /// The answer for the webview.
    pub answer: Answer,
    /// The lines touched (the middle that differs).
    pub touched: Vec<Touched>,
    /// UC-004 A9: a reload the webview never confirmed was replaced; this
    /// was the mirror's text, to keep as a copy.
    pub outside: Option<String>,
}

/// How a batch is taken in, worked out before anything changes.
struct Plan {
    /// The batch's edits, moved over the backend's changes it had not seen.
    set: ChangeSet,
    /// Those changes, for the answer.
    remote: Vec<(u32, Remote)>,
    /// The reload taken back: its revision and the text before it.
    revert: Option<(u32, Rope)>,
}

/// One revision of the log.
#[derive(Debug, Clone)]
struct Entry {
    rev: u32,
    kind: Kind,
}

#[derive(Debug, Clone)]
enum Kind {
    /// Edits: the backend's own or the webview's.
    Edits { set: ChangeSet, sealed: bool },
    /// A reload, with the text before it.
    Reload { before: Rope, text: String },
}

/// `lines` as a signed count.
fn signed(lines: usize) -> isize {
    isize::try_from(lines).unwrap_or(isize::MAX)
}

/// Applies `set` to `rope`, change by change from the end, so each one's
/// line is that of the text before it; the lines touched, in that order.
fn apply_touching(rope: &mut Rope, set: &ChangeSet) -> Vec<Touched> {
    let mut touched = Vec::new();
    for (start, end, text) in set.replacements().into_iter().rev() {
        let line = rope.char_to_line(start);
        let before = rope.len_lines();
        rope.remove(start..end);
        rope.insert(start, &text);
        touched.push(Touched {
            line,
            added_lines: signed(rope.len_lines()) - signed(before),
        });
    }
    touched
}

/// The backend's copy of an open note's text (ADR-025).
#[derive(Debug, Clone)]
pub struct Server {
    rope: Rope,
    rev: u32,
    /// The revisions after `floor`, oldest first.
    log: VecDeque<Entry>,
    /// The oldest revision a batch may be made on.
    floor: u32,
    /// A reload the webview has not confirmed yet.
    unconfirmed: Option<u32>,
    /// The number of the last batch taken in, with its answer.
    last: Option<(u32, Answer)>,
}

impl Server {
    /// The copy of `text`, at revision 1.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            rev: 1,
            log: VecDeque::new(),
            floor: 1,
            unconfirmed: None,
            last: None,
        }
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// The text as a rope.
    #[must_use]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// The revision of the text.
    #[must_use]
    pub fn rev(&self) -> u32 {
        self.rev
    }

    /// The hash of the text.
    #[must_use]
    pub fn hash(&self) -> String {
        hash_rope(&self.rope)
    }

    /// Whether a reload waits for the webview to take it (agents are told
    /// to wait meanwhile, `docs/migration/variante-b.md` section 3.6).
    #[must_use]
    pub fn reload_unconfirmed(&self) -> bool {
        self.unconfirmed.is_some()
    }

    /// The webview runs in the same process and has every change at once
    /// (the session's own editor, variant A): a reload needs no confirmation.
    pub fn confirm_reload(&mut self) {
        self.unconfirmed = None;
    }

    fn next_rev(&mut self) -> u32 {
        self.rev = self.rev.saturating_add(1);
        self.rev
    }

    fn push(&mut self, kind: Kind) -> u32 {
        let rev = self.next_rev();
        self.log.push_back(Entry { rev, kind });
        while self.log.len() > KEPT {
            if let Some(old) = self.log.pop_front() {
                self.floor = old.rev;
            }
        }
        rev
    }

    /// A change of the backend's own: the chars `range` of each replacement
    /// (in order, apart, in chars of the text now) replaced by its text;
    /// `sealed` makes it an undo step of its own in the webview. The new
    /// revision and the lines touched.
    ///
    /// # Errors
    ///
    /// A replacement reaches past the end of the text, or they overlap.
    pub fn backend_edit(
        &mut self,
        replacements: &[(std::ops::Range<usize>, String)],
        sealed: bool,
    ) -> Result<(u32, Vec<Touched>), OutOfRange> {
        let set = ChangeSet::of_replacements(self.rope.len_chars(), replacements)?;
        let touched = apply_touching(&mut self.rope, &set);
        let rev = self.push(Kind::Edits { set, sealed });
        Ok((rev, touched))
    }

    /// UC-004 A7: the text is `text` now, as after a change from outside.
    /// The webview takes it unless it has edits of its own, which take it
    /// back (UC-004 A9, [`Received::reverted`]). The new revision.
    pub fn reload(&mut self, text: &str) -> u32 {
        let before = std::mem::replace(&mut self.rope, Rope::from_str(text));
        let rev = self.push(Kind::Reload {
            before,
            text: text.to_owned(),
        });
        self.unconfirmed = Some(rev);
        rev
    }

    /// The length the text had at revision `base`, from the log.
    fn len_at(&self, base: u32) -> usize {
        let mut len = self.rope.len_chars();
        for entry in self.log.iter().rev().take_while(|e| e.rev > base) {
            len = match &entry.kind {
                Kind::Edits { set, .. } => len + set.base_len() - set.target_len(),
                Kind::Reload { before, .. } => before.len_chars(),
            };
        }
        len
    }

    /// The answer of a refused batch: the webview sends its whole text.
    fn refused(&self) -> Received {
        Received {
            answer: Answer {
                rev: self.rev,
                hash: self.hash(),
                remote: Vec::new(),
                resync: true,
            },
            applied: ChangeSet::identity(self.rope.len_chars()),
            touched: Vec::new(),
            reverted: None,
        }
    }

    /// Takes in `batch` of the webview, made on revision `batch.base`: its
    /// edits are moved over the backend's changes since and applied, and
    /// the answer carries those changes. A batch with edits made before a
    /// reload takes the reload back (UC-004 A9), with everything after it.
    /// A batch that does not fit (a revision not kept, an edit past the end)
    /// changes nothing and asks for the whole text.
    pub fn receive(&mut self, batch: &Batch) -> Received {
        if let Some((_, answer)) = self
            .last
            .as_ref()
            .filter(|(seq, _)| batch.seq != 0 && *seq == batch.seq)
        {
            // Sent again after its answer was lost: taken in already.
            return Received {
                answer: answer.clone(),
                applied: ChangeSet::identity(self.rope.len_chars()),
                touched: Vec::new(),
                reverted: None,
            };
        }
        let Some(plan) = self.plan(batch) else {
            return self.refused();
        };
        // Nothing is changed before here.
        let reverted = plan.revert.map(|(reload_rev, before)| {
            let mirror = std::mem::replace(&mut self.rope, before);
            self.log.retain(|e| e.rev < reload_rev);
            self.unconfirmed = None;
            mirror.to_string()
        });
        if batch.base >= self.unconfirmed.unwrap_or(u32::MAX) {
            self.unconfirmed = None;
        }
        debug_assert_eq!(plan.set.base_len(), self.rope.len_chars());
        let touched = apply_touching(&mut self.rope, &plan.set);
        if !plan.set.is_identity() || reverted.is_some() {
            self.push(Kind::Edits {
                set: plan.set.clone(),
                sealed: false,
            });
        }
        // Revisions up to `base` are known to the webview now.
        while self.log.front().is_some_and(|e| e.rev <= batch.base) {
            if let Some(old) = self.log.pop_front() {
                self.floor = self.floor.max(old.rev);
            }
        }
        let answer = Answer {
            rev: self.rev,
            hash: self.hash(),
            remote: plan.remote,
            resync: false,
        };
        if batch.seq != 0 {
            self.last = Some((batch.seq, answer.clone()));
        }
        Received {
            answer,
            applied: plan.set,
            touched,
            reverted,
        }
    }

    /// How `batch` is taken in; `None` when it does not fit.
    fn plan(&self, batch: &Batch) -> Option<Plan> {
        if batch.base > self.rev || batch.base < self.floor {
            return None;
        }
        let mut set = ChangeSet::of_changes(self.len_at(batch.base), &batch.changes).ok()?;
        let since: Vec<&Entry> = self.log.iter().filter(|e| e.rev > batch.base).collect();
        // A9: edits (not a batch that comes to nothing) made on the text
        // before a reload take it back; the backend's changes after the
        // reload were made on the text from outside and go with it.
        let revert = if set.is_identity() {
            None
        } else {
            since.iter().find_map(|e| match &e.kind {
                Kind::Reload { before, .. } => Some((e.rev, before.clone())),
                Kind::Edits { .. } => None,
            })
        };
        let seen = since
            .iter()
            .take_while(|e| revert.as_ref().is_none_or(|(rev, _)| e.rev < *rev));
        let mut remote = Vec::new();
        for entry in seen {
            match &entry.kind {
                Kind::Edits {
                    set: theirs,
                    sealed,
                } => {
                    // The backend's change goes first at the same place.
                    set = ChangeSet::transform(theirs, &set).1;
                    remote.push((
                        entry.rev,
                        Remote::Edits {
                            changes: theirs.changes(),
                            sealed: *sealed,
                        },
                    ));
                }
                Kind::Reload { text, .. } => {
                    // Only a batch without edits passes a reload.
                    set = ChangeSet::identity(text.chars().count());
                    remote.push((entry.rev, Remote::Reload { text: text.clone() }));
                }
            }
        }
        let target = revert
            .as_ref()
            .map_or(self.rope.len_chars(), |(_, before)| before.len_chars());
        (set.base_len() == target).then_some(Plan {
            set,
            remote,
            revert,
        })
    }

    /// Edits the session's own editor made (variant A), on the text as it
    /// is: logged so a webview's late batch moves over them. The lines
    /// touched.
    ///
    /// # Errors
    ///
    /// An edit reaches past the end of the text.
    pub fn local_edit(&mut self, changes: &[Change]) -> Result<Vec<Touched>, OutOfRange> {
        let set = ChangeSet::of_changes(self.rope.len_chars(), changes)?;
        if set.is_identity() {
            return Ok(Vec::new());
        }
        let touched = apply_touching(&mut self.rope, &set);
        self.push(Kind::Edits { set, sealed: false });
        Ok(touched)
    }

    /// ADR-025 section 3.10: the webview's text wins after the two copies
    /// went apart: the text is `text` now, at a new revision, and the log
    /// starts anew. A reload the webview never confirmed is replaced too;
    /// the mirror's text is then handed back to keep as a copy (UC-004 A9).
    pub fn resync(&mut self, text: &str) -> Resynced {
        let old = self.rope.to_string();
        let outside = (self.unconfirmed.is_some() && old != text).then(|| old.clone());
        let old_len = old.chars().count();
        let change = super::client::difference(&old, text);
        let touched = ChangeSet::of(old_len, &change)
            .map(|set| apply_touching(&mut self.rope, &set))
            .unwrap_or_default();
        let rev = self.next_rev();
        self.log.clear();
        self.floor = rev;
        self.unconfirmed = None;
        self.last = None;
        Resynced {
            answer: Answer {
                rev,
                hash: self.hash(),
                remote: Vec::new(),
                resync: false,
            },
            touched,
            outside,
        }
    }
}
