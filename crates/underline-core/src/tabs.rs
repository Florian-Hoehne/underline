//! The rules for the tabs of a workspace (UC-004 step 2, A1, A5, A6, BR-001,
//! BR-008), independent of what a tab shows.

use crate::note::NoteId;

/// At most this many tabs are open in a workspace (UC-004 BR-008).
pub const MAX_TABS: usize = 100;

struct Entry<T> {
    note: NoteId,
    item: T,
    last_used: u64,
}

/// The open tabs of a workspace, left to right, with the active one.
pub struct TabList<T> {
    tabs: Vec<Entry<T>>,
    active: Option<usize>,
    clock: u64,
}

impl<T> Default for TabList<T> {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            clock: 0,
        }
    }
}

/// What opening a note did.
pub struct Opened<T> {
    /// Index of the note's tab, which is active now.
    pub index: usize,
    /// Whether the note was open already (UC-004 A1).
    pub was_open: bool,
    /// The tab closed to stay within [`MAX_TABS`] (UC-004 A6).
    pub closed: Option<(NoteId, T)>,
}

impl<T> TabList<T> {
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn find(&self, note: &NoteId) -> Option<usize> {
        self.tabs.iter().position(|e| &e.note == note)
    }

    pub fn note(&self, index: usize) -> &NoteId {
        &self.tabs[index].note
    }

    pub fn get(&self, index: usize) -> &T {
        &self.tabs[index].item
    }

    pub fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.tabs[index].item
    }

    pub fn active(&self) -> Option<(&NoteId, &T)> {
        self.active
            .map(|i| (&self.tabs[i].note, &self.tabs[i].item))
    }

    pub fn active_mut(&mut self) -> Option<(&NoteId, &mut T)> {
        let i = self.active?;
        let entry = &mut self.tabs[i];
        Some((&entry.note, &mut entry.item))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NoteId, &T)> {
        self.tabs.iter().map(|e| (&e.note, &e.item))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&NoteId, &mut T)> {
        self.tabs.iter_mut().map(|e| (&e.note, &mut e.item))
    }

    /// The indices of the tabs, the one used longest ago first (UC-004 A6).
    /// [`TabList::open`] closes the first of them at [`MAX_TABS`]; a caller
    /// that must not lose a tab's changes can close another one first.
    #[must_use]
    pub fn longest_unused_first(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.tabs.len()).collect();
        order.sort_by_key(|&i| self.tabs[i].last_used);
        order
    }

    /// Makes the tab at `index` the active one (UC-004 A5).
    pub fn activate(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.clock += 1;
            self.tabs[index].last_used = self.clock;
            self.active = Some(index);
        }
    }

    /// Opens `note` (UC-004 step 2): an open note's tab becomes active (A1,
    /// BR-001); otherwise a new tab is placed right of the active one. With
    /// [`MAX_TABS`] tabs open, the tab used longest ago is closed (A6).
    pub fn open(&mut self, note: NoteId, item: impl FnOnce() -> T) -> Opened<T> {
        if let Some(index) = self.find(&note) {
            self.activate(index);
            return Opened {
                index,
                was_open: true,
                closed: None,
            };
        }
        let mut closed = None;
        if self.tabs.len() >= MAX_TABS {
            // The first of the order: `min_by_key` and the stable sort both
            // take the leftmost tab among equal uses.
            if let Some(&oldest) = self.longest_unused_first().first() {
                closed = Some(self.close(oldest));
            }
        }
        let index = self.active.map_or(self.tabs.len(), |a| a + 1);
        self.tabs.insert(
            index,
            Entry {
                note,
                item: item(),
                last_used: 0,
            },
        );
        self.activate(index);
        Opened {
            index,
            was_open: false,
            closed,
        }
    }

    /// Closes the tab at `index` (UC-004 A5); the tab right of it becomes
    /// active, or the one left of it at the end.
    pub fn close(&mut self, index: usize) -> (NoteId, T) {
        let entry = self.tabs.remove(index);
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
        if let Some(active) = self.active {
            self.clock += 1;
            self.tabs[active].last_used = self.clock;
        }
        (entry.note, entry.item)
    }

    /// UC-006 step 7: the tab of `from` shows the note under its new name
    /// `to`, at the same place.
    pub fn rename(&mut self, from: &NoteId, to: NoteId) {
        if let Some(entry) = self.tabs.iter_mut().find(|e| &e.note == from) {
            entry.note = to;
        }
    }

    /// Takes all tabs out, left to right.
    pub fn drain(&mut self) -> Vec<(NoteId, T)> {
        self.active = None;
        self.tabs.drain(..).map(|e| (e.note, e.item)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> NoteId {
        NoteId::from_title(name)
    }

    fn names<T>(tabs: &TabList<T>) -> Vec<String> {
        tabs.iter().map(|(n, _)| n.to_string()).collect()
    }

    /// TC-004-01 / TC-004-11: a note gets one tab; opening it again activates it.
    #[test]
    fn uc004_tc01_tc11_one_tab_per_note() {
        let mut tabs = TabList::default();
        tabs.open(id("Home"), || ());
        let opened = tabs.open(id("Plan"), || ());
        assert!(!opened.was_open);
        tabs.activate(0);
        let again = tabs.open(id("Plan"), || ());
        assert!(again.was_open);
        assert_eq!(names(&tabs), ["Home", "Plan"]);
        assert_eq!(tabs.active_index(), Some(1));
    }

    /// TC-004-17: a new tab is placed right of the active tab.
    #[test]
    fn uc004_tc17_new_tab_right_of_active() {
        let mut tabs = TabList::default();
        for name in ["A", "B", "C"] {
            tabs.open(id(name), || ());
        }
        tabs.activate(0);
        tabs.open(id("D"), || ());
        assert_eq!(names(&tabs), ["A", "D", "B", "C"]);
        assert_eq!(tabs.active_index(), Some(1));
    }

    /// TC-004-16: closing a tab activates its neighbor.
    #[test]
    fn uc004_tc16_closing_activates_the_neighbor() {
        let mut tabs = TabList::default();
        for name in ["Home", "Plan", "Ideas"] {
            tabs.open(id(name), || ());
        }
        tabs.activate(0);
        tabs.close(0);
        assert_eq!(
            tabs.active().map(|(n, _)| n.to_string()),
            Some("Plan".into())
        );
        tabs.close(1);
        assert_eq!(
            tabs.active().map(|(n, _)| n.to_string()),
            Some("Plan".into())
        );
        tabs.close(0);
        assert!(tabs.active().is_none());
    }

    /// TC-004-18: the 101st tab closes the tab used longest ago.
    #[test]
    fn uc004_tc18_more_than_100_tabs() {
        let mut tabs = TabList::default();
        for i in 0..MAX_TABS {
            tabs.open(id(&format!("N{i}")), || i);
        }
        // N0 is used again, so N1 is the one used longest ago.
        let first = tabs.find(&id("N0")).unwrap();
        tabs.activate(first);
        let opened = tabs.open(id("New"), || 999);
        assert_eq!(opened.closed.map(|(n, _)| n.to_string()), Some("N1".into()));
        assert_eq!(tabs.len(), MAX_TABS);
        assert!(tabs.find(&id("New")).is_some());
    }

    /// UC-004 A6, M-06.6: the order in which the limit closes tabs, the
    /// one used longest ago first; `open` closes the first of them.
    #[test]
    fn uc004_a6_m06_6_tabs_in_the_order_of_their_last_use() {
        let mut tabs = TabList::default();
        for name in ["A", "B", "C"] {
            tabs.open(id(name), || ());
        }
        tabs.activate(tabs.find(&id("A")).unwrap());
        let order: Vec<String> = tabs
            .longest_unused_first()
            .into_iter()
            .map(|i| tabs.note(i).to_string())
            .collect();
        assert_eq!(order, ["B", "C", "A"]);
        assert!(TabList::<()>::default().longest_unused_first().is_empty());
    }
}
