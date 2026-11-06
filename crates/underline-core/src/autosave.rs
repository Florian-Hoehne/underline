//! When a changed note is stored (UC-005 BR-001, NFR-005, ADR-005): 1 second
//! after the last change, and at most 2 seconds after the first change that
//! is not stored.

use std::time::{Duration, Instant};

/// A note is stored this long after the last change.
pub const AUTOSAVE_QUIET: Duration = Duration::from_secs(1);
/// A note is stored at the latest this long after its first unstored change.
pub const AUTOSAVE_LATEST: Duration = Duration::from_secs(2);
/// A note that could not be stored is tried again this often (UC-005 A14).
pub const AUTOSAVE_RETRY: Duration = Duration::from_secs(10);

/// The autosave clock of one open note.
#[derive(Debug, Clone, Default)]
pub struct Autosave {
    first_unstored: Option<Instant>,
    last_change: Option<Instant>,
    failed_at: Option<Instant>,
}

impl Autosave {
    /// Records a change made at `now`.
    pub fn changed(&mut self, now: Instant) {
        self.first_unstored.get_or_insert(now);
        self.last_change = Some(now);
    }

    /// Whether the note holds changes that are not stored.
    pub fn is_unstored(&self) -> bool {
        self.first_unstored.is_some()
    }

    /// Whether the last attempt to store failed (UC-005 A14).
    pub fn has_failed(&self) -> bool {
        self.failed_at.is_some()
    }

    /// Whether the note should be stored at `now`.
    pub fn is_due(&self, now: Instant) -> bool {
        let (Some(first), Some(last)) = (self.first_unstored, self.last_change) else {
            return false;
        };
        if let Some(failed) = self.failed_at {
            // After a failure: again with a further change, or after a while.
            return last > failed || now.duration_since(failed) >= AUTOSAVE_RETRY;
        }
        now.duration_since(last) >= AUTOSAVE_QUIET || now.duration_since(first) >= AUTOSAVE_LATEST
    }

    /// The note was stored with every change made up to `started`.
    pub fn stored(&mut self, started: Instant) {
        self.failed_at = None;
        if self.last_change.is_some_and(|last| last <= started) {
            self.first_unstored = None;
            self.last_change = None;
        } else {
            self.first_unstored = self.last_change;
        }
    }

    /// Storing the note failed at `now`.
    pub fn failed(&mut self, now: Instant) {
        self.failed_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    /// TC-005-05: stored 1 second after the last change, at most 2 seconds
    /// after the first.
    #[test]
    fn uc005_tc05_autosave_timing() {
        let t = Instant::now();
        let mut autosave = Autosave::default();
        assert!(!autosave.is_due(at(t, 5_000)));
        autosave.changed(t);
        assert!(!autosave.is_due(at(t, 900)));
        assert!(autosave.is_due(at(t, 1_000)));
        // Typing without pause: due 2 seconds after the first change.
        let mut typing = Autosave::default();
        for ms in (0..=1_900).step_by(300) {
            typing.changed(at(t, ms));
        }
        assert!(!typing.is_due(at(t, 1_950)));
        assert!(typing.is_due(at(t, 2_000)));
        typing.stored(at(t, 2_000));
        assert!(!typing.is_unstored());
    }

    /// A change made while the note was being stored stays unstored.
    #[test]
    fn uc005_change_during_store_stays_unstored() {
        let t = Instant::now();
        let mut autosave = Autosave::default();
        autosave.changed(t);
        autosave.changed(at(t, 1_500));
        autosave.stored(at(t, 1_000));
        assert!(autosave.is_unstored());
    }

    /// TC-005-23: after a failure, retried with a change or after 10 seconds.
    #[test]
    fn uc005_tc23_retry_after_failure() {
        let t = Instant::now();
        let mut autosave = Autosave::default();
        autosave.changed(t);
        autosave.failed(at(t, 1_000));
        assert!(autosave.has_failed());
        assert!(!autosave.is_due(at(t, 5_000)));
        assert!(autosave.is_due(at(t, 11_000)));
        autosave.changed(at(t, 6_000));
        assert!(autosave.is_due(at(t, 6_000)));
        autosave.stored(at(t, 7_000));
        assert!(!autosave.has_failed() && !autosave.is_unstored());
    }
}
