//! The split view (UC-004 A18 to A20, BR-001, BR-008, BR-018, FR-096,
//! FR-097): the middle of the window shows one pane, or a left and a right
//! pane side by side, each with its own tabs and its own active tab.
//!
//! A note is open in at most one tab of each pane (BR-001), so it can be
//! open in both, as its source view on one side and its facade on the
//! other (A19). What a tab shows of its note (the text, its history, its
//! autosave) belongs to the note, not to the tab: the panes only say which
//! notes are shown where, in which editing view, and which pane the user
//! works in (the active pane). OQ-220: two panes side by side, no more.

use serde::{Deserialize, Serialize};

use crate::note::NoteId;
use crate::state::{EditingView, TabState};

/// UC-004 BR-018: one of the two panes of the split view; without the split
/// view, every tab is in the left pane (entity OPEN_TAB, `pane`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneSide {
    /// The left pane, the only one without the split view.
    #[default]
    Left,
    /// The right pane, shown only with the split view.
    Right,
}

/// A pane that is not known (a hand-written or a newer state file: another
/// word, a number, a list, a map) reads as the left one, so the other tabs
/// and the notes' places of the workspace stay (UC-004 step 6, OQ-279).
impl<'de> Deserialize<'de> for PaneSide {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::loose::Loose::read(deserializer, |value| match value {
            crate::loose::Loose::Text(name) if name == "right" => Self::Right,
            _ => Self::Left,
        })
    }
}

impl PaneSide {
    /// Whether this is the left pane (which a state file does not name).
    #[must_use]
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if`
    pub fn is_left(&self) -> bool {
        *self == Self::Left
    }

    /// The pane on the other side.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

/// One tab of a pane: its note and the editing view it shows the note in
/// (UC-004 BR-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneTab {
    /// The note the tab shows.
    pub note: NoteId,
    /// The editing view of the tab.
    pub view: EditingView,
    /// When the tab was last made active, by the clock of [`Panes`].
    last_used: u64,
}

/// The tabs of one pane, left to right, with the active one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pane {
    tabs: Vec<PaneTab>,
    active: Option<usize>,
}

impl Pane {
    /// The tabs, left to right.
    #[must_use]
    pub fn tabs(&self) -> &[PaneTab] {
        &self.tabs
    }

    /// The active tab, `None` without tabs.
    #[must_use]
    pub fn active(&self) -> Option<&PaneTab> {
        self.active.map(|index| &self.tabs[index])
    }

    /// The index of the tab of `note` in this pane.
    #[must_use]
    pub fn find(&self, note: &NoteId) -> Option<usize> {
        self.tabs.iter().position(|tab| &tab.note == note)
    }

    /// Whether the pane has no tab.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Removes the tab at `index`; the tab right of it becomes active, or
    /// the one left of it at the end (UC-004 A5).
    fn remove(&mut self, index: usize) -> PaneTab {
        let tab = self.tabs.remove(index);
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
        tab
    }
}

/// What opening a note in a pane did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneOpened {
    /// The pane the note's tab is in, which is the active pane now.
    pub side: PaneSide,
    /// The note had a tab in that pane already (UC-004 A1).
    pub was_open: bool,
    /// The split view was not shown and is now (A18 step 1).
    pub split: bool,
}

/// The panes of the middle of the window: the left one, the right one while
/// the split view is shown, and the active pane (UC-004 BR-018).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Panes {
    left: Pane,
    right: Option<Pane>,
    active: PaneSide,
    clock: u64,
}

impl Panes {
    /// Whether the split view is shown.
    #[must_use]
    pub fn is_split(&self) -> bool {
        self.right.is_some()
    }

    /// The active pane: the one the user last worked in.
    #[must_use]
    pub fn active_side(&self) -> PaneSide {
        self.active
    }

    /// The pane `side`, `None` for the right one without the split view.
    #[must_use]
    pub fn pane(&self, side: PaneSide) -> Option<&Pane> {
        match side {
            PaneSide::Left => Some(&self.left),
            PaneSide::Right => self.right.as_ref(),
        }
    }

    fn pane_mut(&mut self, side: PaneSide) -> Option<&mut Pane> {
        match side {
            PaneSide::Left => Some(&mut self.left),
            PaneSide::Right => self.right.as_mut(),
        }
    }

    /// The active tab of the active pane: the note the tool panel shows.
    #[must_use]
    pub fn active_tab(&self) -> Option<&PaneTab> {
        self.pane(self.active).and_then(Pane::active)
    }

    /// The note of [`Panes::active_tab`].
    #[must_use]
    pub fn active_note(&self) -> Option<&NoteId> {
        self.active_tab().map(|tab| &tab.note)
    }

    /// The panes shown, the left one first.
    pub fn sides(&self) -> impl Iterator<Item = (PaneSide, &Pane)> {
        std::iter::once((PaneSide::Left, &self.left))
            .chain(self.right.iter().map(|pane| (PaneSide::Right, pane)))
    }

    /// Every tab of both panes, with its pane.
    pub fn tabs(&self) -> impl Iterator<Item = (PaneSide, &PaneTab)> {
        self.sides()
            .flat_map(|(side, pane)| pane.tabs.iter().map(move |tab| (side, tab)))
    }

    /// How many tabs are open, both panes together (UC-004 BR-008).
    #[must_use]
    pub fn len(&self) -> usize {
        self.left.tabs.len() + self.right.as_ref().map_or(0, |pane| pane.tabs.len())
    }

    /// Whether no tab is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `note` has a tab in either pane.
    #[must_use]
    pub fn contains(&self, note: &NoteId) -> bool {
        self.tabs().any(|(_, tab)| &tab.note == note)
    }

    /// Whether `note` has a tab in the pane `side`.
    #[must_use]
    pub fn has(&self, side: PaneSide, note: &NoteId) -> bool {
        self.pane(side)
            .is_some_and(|pane| pane.find(note).is_some())
    }

    /// Whether a tab of `note` shows its facade, in either pane.
    #[must_use]
    pub fn shows_facade(&self, note: &NoteId) -> bool {
        self.tabs()
            .any(|(_, tab)| &tab.note == note && tab.view == EditingView::Facade)
    }

    /// ADR-025: the notes whose facade a pane shows now (its active tab
    /// shows the facade), the active pane's first, without repeats.
    #[must_use]
    pub fn shown_facades(&self) -> Vec<NoteId> {
        let mut shown: Vec<NoteId> = Vec::new();
        let order = [self.active, self.active.other()];
        for side in order {
            if let Some(tab) = self.pane(side).and_then(Pane::active) {
                if tab.view == EditingView::Facade && !shown.contains(&tab.note) {
                    shown.push(tab.note.clone());
                }
            }
        }
        shown
    }

    /// The notes of the active tabs of the panes shown, without repeats.
    #[must_use]
    pub fn shown_notes(&self) -> Vec<NoteId> {
        let mut shown: Vec<NoteId> = Vec::new();
        for (_, pane) in self.sides() {
            if let Some(tab) = pane.active() {
                if !shown.contains(&tab.note) {
                    shown.push(tab.note.clone());
                }
            }
        }
        shown
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    fn activate_index(&mut self, side: PaneSide, index: usize) {
        let now = self.tick();
        if let Some(pane) = self.pane_mut(side) {
            if index < pane.tabs.len() {
                pane.tabs[index].last_used = now;
                pane.active = Some(index);
            }
        }
    }

    /// UC-004 BR-018: the user works in the pane `side` now; nothing
    /// happens for the right pane without the split view. Returns whether
    /// the active pane changed.
    pub fn focus(&mut self, side: PaneSide) -> bool {
        if self.pane(side).is_none() || self.active == side {
            return false;
        }
        self.active = side;
        if let Some(index) = self.pane(side).and_then(|pane| pane.active) {
            self.activate_index(side, index);
        }
        true
    }

    /// M-06.2, UC-004 A5: the tab of `note` in the pane `side` becomes its
    /// active tab, and the pane the active pane. Returns whether the pane
    /// has a tab of `note`.
    pub fn activate(&mut self, side: PaneSide, note: &NoteId) -> bool {
        let Some(index) = self.pane(side).and_then(|pane| pane.find(note)) else {
            return false;
        };
        self.active = side;
        self.activate_index(side, index);
        true
    }

    /// Puts a tab of `note` showing `view` into the pane `side`, right of its
    /// active tab, and makes it active; the pane must be shown.
    fn insert(&mut self, side: PaneSide, note: NoteId, view: EditingView) {
        let Some(pane) = self.pane_mut(side) else {
            return;
        };
        let index = pane.active.map_or(pane.tabs.len(), |active| active + 1);
        pane.tabs.insert(
            index,
            PaneTab {
                note,
                view,
                last_used: 0,
            },
        );
        self.active = side;
        self.activate_index(side, index);
    }

    /// UC-004 step 2, A1, BR-001, BR-018: opens `note` in the active pane:
    /// its tab there becomes active, or a new tab showing `view` is placed
    /// right of the active tab (BR-008).
    pub fn open(&mut self, note: &NoteId, view: EditingView) -> PaneOpened {
        let side = self.active;
        let was_open = self.activate(side, note);
        if !was_open {
            self.insert(side, note.clone(), view);
        }
        PaneOpened {
            side,
            was_open,
            split: false,
        }
    }

    /// UC-004 A18, A19: opens `note` to the side: the split view is shown
    /// when it is not (the open tabs stay in the left pane), and the note
    /// opens in the other pane, or its tab there becomes active; that pane
    /// becomes the active pane. When `note` is the note of the active tab
    /// and gets a new tab, the new tab shows the other editing view (A19),
    /// else `view`.
    pub fn open_side(&mut self, note: &NoteId, view: EditingView) -> PaneOpened {
        if self.is_empty() {
            // A18, A20: with no tab open there is no side; a pane without
            // tabs is never shown, so the note opens in the one pane.
            self.active = PaneSide::Left;
            return self.open(note, view);
        }
        let split = self.right.is_none();
        let from = self.active;
        let shown = self
            .active_tab()
            .filter(|tab| &tab.note == note)
            .map(|tab| tab.view);
        if split {
            self.right = Some(Pane::default());
        }
        let side = from.other();
        let was_open = self.activate(side, note);
        if !was_open {
            let view = match shown {
                Some(EditingView::Facade) => EditingView::Source,
                Some(EditingView::Source) => EditingView::Facade,
                None => view,
            };
            self.insert(side, note.clone(), view);
        }
        PaneOpened {
            side,
            was_open,
            split,
        }
    }

    /// M-07.2, UC-004 BR-003: the tab of `note` in the pane `side` shows
    /// `view` from now on. Returns whether it changed.
    pub fn set_view(&mut self, side: PaneSide, note: &NoteId, view: EditingView) -> bool {
        let Some(pane) = self.pane_mut(side) else {
            return false;
        };
        let Some(index) = pane.find(note) else {
            return false;
        };
        let changed = pane.tabs[index].view != view;
        pane.tabs[index].view = view;
        changed
    }

    /// M-06.3, UC-004 A5, A20: closes the tab of `note` in the pane `side`.
    /// Closing the last tab of a pane ends the split view. Returns the tab
    /// closed.
    pub fn close(&mut self, side: PaneSide, note: &NoteId) -> Option<PaneTab> {
        let pane = self.pane_mut(side)?;
        let index = pane.find(note)?;
        let tab = pane.remove(index);
        let emptied = pane.is_empty();
        if let Some(active) = self.pane(side).and_then(|pane| pane.active) {
            if self.active == side {
                self.activate_index(side, active);
            }
        }
        if emptied && self.is_split() {
            // A20: the pane that is left becomes the one pane.
            if side == PaneSide::Left {
                self.left = self.right.take().unwrap_or_default();
            } else {
                self.right = None;
            }
            self.active = PaneSide::Left;
        }
        Some(tab)
    }

    /// UC-004 A20: ends the split view: the tabs of the right pane join the
    /// left pane after its own, and a note open on both sides keeps only the
    /// tab of the left pane. The active note stays active. Returns whether
    /// the split view was shown.
    pub fn unsplit(&mut self) -> bool {
        let active = self.active_note().cloned();
        let Some(right) = self.right.take() else {
            return false;
        };
        for tab in right.tabs {
            if self.left.find(&tab.note).is_none() {
                self.left.tabs.push(tab);
            }
        }
        self.active = PaneSide::Left;
        let index = active
            .and_then(|note| self.left.find(&note))
            .or(self.left.active)
            .or((!self.left.tabs.is_empty()).then_some(0));
        self.left.active = index;
        if let Some(index) = index {
            self.activate_index(PaneSide::Left, index);
        }
        true
    }

    /// Removes every tab of `note`, in both panes, as when the note is gone
    /// (deleted, or closed for good); a pane left without tabs ends the split
    /// view (A20). Returns whether a tab was removed.
    pub fn remove_note(&mut self, note: &NoteId) -> bool {
        let sides: Vec<PaneSide> = self
            .tabs()
            .filter(|(_, tab)| &tab.note == note)
            .map(|(side, _)| side)
            .collect();
        for &side in sides.iter().rev() {
            self.close(side, note);
        }
        !sides.is_empty()
    }

    /// UC-006 step 7: the tabs of `from` show the note under its new name
    /// `to`, at the same places.
    pub fn rename(&mut self, from: &NoteId, to: &NoteId) {
        for pane in std::iter::once(&mut self.left).chain(self.right.as_mut()) {
            for tab in &mut pane.tabs {
                if &tab.note == from {
                    tab.note = to.clone();
                }
            }
        }
    }

    /// UC-004 A6, M-06.6: the tabs of both panes, the one used longest ago
    /// first; the active tab of each pane is not among them.
    #[must_use]
    pub fn longest_unused_first(&self) -> Vec<(PaneSide, NoteId)> {
        let mut tabs: Vec<(u64, PaneSide, NoteId)> = Vec::new();
        for (side, pane) in self.sides() {
            for (index, tab) in pane.tabs.iter().enumerate() {
                if Some(index) != pane.active {
                    tabs.push((tab.last_used, side, tab.note.clone()));
                }
            }
        }
        // Stable: the leftmost among equal uses first.
        tabs.sort_by_key(|(used, _, _)| *used);
        tabs.into_iter()
            .map(|(_, side, note)| (side, note))
            .collect()
    }

    /// Takes every tab out and shows one pane again.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// M-22.2, UC-004 step 6: the tabs as they are remembered, each with its
    /// pane, its place in it and whether it is the pane's active tab.
    #[must_use]
    pub fn states(&self) -> Vec<TabState> {
        self.sides()
            .flat_map(|(side, pane)| {
                pane.tabs
                    .iter()
                    .enumerate()
                    .map(move |(index, tab)| TabState {
                        note: tab.note.clone(),
                        pane: side,
                        position: u32::try_from(index + 1).unwrap_or(u32::MAX),
                        editing_view: tab.view,
                        active: Some(index) == pane.active,
                    })
            })
            .collect()
    }

    /// UC-004 step 6, BR-018: a remembered tab comes back at the end of its
    /// pane (`push` them in their order); the right pane is shown with its
    /// first tab. A note that has a tab in that pane already gets none.
    pub fn push(&mut self, side: PaneSide, note: NoteId, view: EditingView) {
        if side == PaneSide::Right && self.right.is_none() {
            self.right = Some(Pane::default());
        }
        let Some(pane) = self.pane_mut(side) else {
            return;
        };
        if pane.find(&note).is_some() {
            return;
        }
        pane.tabs.push(PaneTab {
            note,
            view,
            last_used: 0,
        });
        if pane.active.is_none() {
            pane.active = Some(pane.tabs.len() - 1);
        }
    }

    /// UC-004 BR-018: after the remembered tabs came back, the left pane is
    /// the active pane; a right pane without tabs is not shown.
    pub fn restored(&mut self) {
        if self.right.as_ref().is_some_and(Pane::is_empty) {
            self.right = None;
        }
        if self.left.is_empty() {
            if let Some(right) = self.right.take() {
                self.left = right;
            }
        }
        self.active = PaneSide::Left;
        if let Some(index) = self.left.active {
            self.activate_index(PaneSide::Left, index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> NoteId {
        NoteId::from_title(name)
    }

    fn names(panes: &Panes, side: PaneSide) -> Vec<String> {
        panes
            .pane(side)
            .map(|pane| pane.tabs().iter().map(|t| t.note.to_string()).collect())
            .unwrap_or_default()
    }

    fn active(panes: &Panes, side: PaneSide) -> Option<String> {
        panes
            .pane(side)
            .and_then(Pane::active)
            .map(|tab| tab.note.to_string())
    }

    fn with(left: &[&str]) -> Panes {
        let mut panes = Panes::default();
        for name in left {
            panes.open(&id(name), EditingView::Facade);
        }
        panes
    }

    /// TC-004-34 step 1, UC-004 A18, BR-018, FR-096: opening to the side
    /// splits the middle; the tabs stay left, the note opens right, and the
    /// right pane is the active pane.
    #[test]
    fn uc004_tc34_open_to_the_side_splits() {
        let mut panes = with(&["Plan", "Ideas"]);
        panes.activate(PaneSide::Left, &id("Plan"));
        let opened = panes.open_side(&id("Trip"), EditingView::Facade);
        assert!(opened.split);
        assert_eq!(opened.side, PaneSide::Right);
        assert_eq!(names(&panes, PaneSide::Left), ["Plan", "Ideas"]);
        assert_eq!(names(&panes, PaneSide::Right), ["Trip"]);
        assert_eq!(panes.active_side(), PaneSide::Right);
        assert_eq!(
            panes.active_note().map(ToString::to_string),
            Some("Trip".into())
        );
    }

    /// TC-004-34 steps 2 and 3, BR-018: a note chosen opens in the active
    /// pane; working in the other pane makes it the active pane.
    #[test]
    fn uc004_tc34_notes_open_in_the_active_pane() {
        let mut panes = with(&["Plan", "Ideas"]);
        panes.open_side(&id("Trip"), EditingView::Facade);
        panes.open(&id("Packing"), EditingView::Facade);
        assert_eq!(names(&panes, PaneSide::Right), ["Trip", "Packing"]);
        assert!(panes.focus(PaneSide::Left));
        panes.open(&id("Log"), EditingView::Facade);
        assert_eq!(names(&panes, PaneSide::Left), ["Plan", "Ideas", "Log"]);
        assert_eq!(names(&panes, PaneSide::Right), ["Trip", "Packing"]);
    }

    /// TC-004-35, UC-004 A19, BR-001, FR-097: the note of the active tab
    /// opened to the side shows the other editing view; choosing it again
    /// in the right pane opens no third tab.
    #[test]
    fn uc004_tc35_the_same_note_on_both_sides() {
        let mut panes = with(&["Plan"]);
        let opened = panes.open_side(&id("Plan"), EditingView::Facade);
        assert!(!opened.was_open);
        let left = panes.pane(PaneSide::Left).and_then(Pane::active).unwrap();
        let right = panes.pane(PaneSide::Right).and_then(Pane::active).unwrap();
        assert_eq!(left.view, EditingView::Facade);
        assert_eq!(right.view, EditingView::Source);
        let again = panes.open(&id("Plan"), EditingView::Facade);
        assert!(again.was_open);
        assert_eq!(panes.len(), 2);
        assert!(panes.shows_facade(&id("Plan")));
    }

    /// TC-004-36 step 1, UC-004 A20: ending the split view joins the right
    /// tabs after the left ones; a note on both sides keeps its left tab.
    #[test]
    fn uc004_tc36_ending_the_split_joins_the_tabs() {
        let mut panes = with(&["Plan", "Ideas"]);
        panes.activate(PaneSide::Left, &id("Plan"));
        panes.open_side(&id("Plan"), EditingView::Facade);
        panes.open(&id("Trip"), EditingView::Facade);
        assert_eq!(names(&panes, PaneSide::Right), ["Plan", "Trip"]);
        assert!(panes.unsplit());
        assert!(!panes.is_split());
        assert_eq!(names(&panes, PaneSide::Left), ["Plan", "Ideas", "Trip"]);
        assert_eq!(active(&panes, PaneSide::Left), Some("Trip".into()));
        assert!(!panes.unsplit());
    }

    /// TC-004-36 step 2, A20: closing the last tab of the right pane ends
    /// the split view; the left pane is unchanged.
    #[test]
    fn uc004_tc36_closing_the_last_tab_of_a_pane_ends_the_split() {
        let mut panes = with(&["Plan", "Ideas"]);
        panes.open_side(&id("Trip"), EditingView::Facade);
        let closed = panes.close(PaneSide::Right, &id("Trip"));
        assert_eq!(closed.map(|t| t.note.to_string()), Some("Trip".into()));
        assert!(!panes.is_split());
        assert_eq!(names(&panes, PaneSide::Left), ["Plan", "Ideas"]);
        assert_eq!(panes.active_side(), PaneSide::Left);
    }

    /// UC-004 A20: closing the last tab of the left pane leaves the right
    /// pane's tabs as the one pane.
    #[test]
    fn uc004_a20_closing_the_left_pane_keeps_the_right_one() {
        let mut panes = with(&["Plan"]);
        panes.open_side(&id("Trip"), EditingView::Facade);
        panes.close(PaneSide::Left, &id("Plan"));
        assert!(!panes.is_split());
        assert_eq!(names(&panes, PaneSide::Left), ["Trip"]);
        assert_eq!(active(&panes, PaneSide::Left), Some("Trip".into()));
    }

    /// UC-004 BR-008: at most 100 tabs counts both panes; the active tab of
    /// each pane is never the one used longest ago.
    #[test]
    fn uc004_br008_both_panes_count_and_actives_stay() {
        let mut panes = with(&["A", "B"]);
        panes.open_side(&id("C"), EditingView::Facade);
        panes.open(&id("D"), EditingView::Facade);
        assert_eq!(panes.len(), 4);
        let order = panes.longest_unused_first();
        let notes: Vec<String> = order.iter().map(|(_, n)| n.to_string()).collect();
        assert_eq!(notes, ["A", "C"]);
    }

    /// M-22.2, UC-004 step 6, BR-018: the tabs are remembered with their
    /// panes; after a restart the left pane is the active pane.
    #[test]
    fn uc004_step6_remembered_with_their_panes() {
        let mut panes = with(&["Plan", "Ideas"]);
        panes.activate(PaneSide::Left, &id("Plan"));
        panes.open_side(&id("Plan"), EditingView::Facade);
        let states = panes.states();
        assert_eq!(states.len(), 3);
        assert_eq!(states[2].pane, PaneSide::Right);
        assert_eq!(states[2].editing_view, EditingView::Source);
        assert!(states[2].active);
        let mut back = Panes::default();
        for state in &states {
            back.push(state.pane, state.note.clone(), state.editing_view);
        }
        back.restored();
        assert!(back.is_split());
        assert_eq!(back.active_side(), PaneSide::Left);
        assert_eq!(names(&back, PaneSide::Right), ["Plan"]);
    }

    /// UC-006 step 7, UC-007: a renamed note keeps its tabs; a deleted one
    /// loses them on both sides, which ends the split view.
    #[test]
    fn uc006_uc007_rename_and_remove_in_both_panes() {
        let mut panes = with(&["Plan"]);
        panes.open_side(&id("Plan"), EditingView::Facade);
        panes.rename(&id("Plan"), &id("Goals"));
        assert!(panes.has(PaneSide::Left, &id("Goals")));
        assert!(panes.has(PaneSide::Right, &id("Goals")));
        assert!(panes.remove_note(&id("Goals")));
        assert!(panes.is_empty());
        assert!(!panes.is_split());
    }

    /// A small deterministic generator; every failure prints its seed.
    struct Lcg(u64);

    impl Lcg {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 33) as usize) % n
        }
    }

    /// The invariants of the panes (UC-004 BR-001, BR-018, A20).
    fn check(panes: &Panes, context: &str) {
        for (side, pane) in panes.sides() {
            let mut notes: Vec<_> = pane.tabs().iter().map(|t| t.note.to_string()).collect();
            let before = notes.len();
            notes.sort();
            notes.dedup();
            assert_eq!(
                before,
                notes.len(),
                "{context}: {side:?} holds a note twice"
            );
            assert_eq!(
                pane.active().is_some(),
                !pane.is_empty(),
                "{context}: {side:?}"
            );
            if panes.is_split() {
                assert!(!pane.is_empty(), "{context}: {side:?} is shown empty");
            }
        }
        if panes.active_side() == PaneSide::Right {
            assert!(
                panes.is_split(),
                "{context}: the right pane is active, not shown"
            );
        }
        let states = panes.states();
        assert_eq!(states.len(), panes.len(), "{context}");
        for side in [PaneSide::Left, PaneSide::Right] {
            let of: Vec<_> = states.iter().filter(|t| t.pane == side).collect();
            assert_eq!(
                of.iter().filter(|t| t.active).count(),
                usize::from(!of.is_empty()),
                "{context}: one active tab per pane in the states"
            );
            for (index, tab) in of.iter().enumerate() {
                assert_eq!(tab.position as usize, index + 1, "{context}");
            }
        }
    }

    /// UC-004 A18 to A20, BR-001, BR-018 (property, seed printed): any
    /// sequence of opens (also to the side), closes, focus changes, ends of
    /// the split view, removals and renames keeps the panes consistent; ending
    /// the split view keeps the active note, and the states come back as the
    /// same panes.
    #[test]
    fn uc004_br018_random_operations_keep_the_panes_consistent() {
        let names: Vec<String> = (0..7).map(|i| format!("N{i}")).collect();
        for seed in 0..3000u64 {
            let mut rng = Lcg(seed + 1);
            let mut panes = Panes::default();
            let mut log: Vec<String> = Vec::new();
            for _ in 0..50 {
                let note = id(&names[rng.below(names.len())]);
                let side = if rng.below(2) == 0 {
                    PaneSide::Left
                } else {
                    PaneSide::Right
                };
                let view = if rng.below(2) == 0 {
                    EditingView::Source
                } else {
                    EditingView::Facade
                };
                let what = match rng.below(10) {
                    0 | 1 => {
                        panes.open(&note, view);
                        format!("open {note}")
                    }
                    2 | 3 => {
                        if panes.is_empty() {
                            continue;
                        }
                        panes.open_side(&note, view);
                        format!("side {note}")
                    }
                    4 | 5 => {
                        panes.close(side, &note);
                        format!("close {side:?} {note}")
                    }
                    6 => {
                        panes.focus(side);
                        format!("focus {side:?}")
                    }
                    7 => {
                        let before = panes.active_note().cloned();
                        panes.unsplit();
                        assert_eq!(
                            panes.active_note().cloned(),
                            before,
                            "seed {seed}: unsplit changed the active note {log:#?}"
                        );
                        "unsplit".to_owned()
                    }
                    8 => {
                        panes.remove_note(&note);
                        format!("remove {note}")
                    }
                    _ => {
                        panes.activate(side, &note);
                        format!("activate {side:?} {note}")
                    }
                };
                log.push(what);
                check(&panes, &format!("seed {seed}: {log:#?}"));
            }
            // The remembered states come back as the same panes.
            let mut back = Panes::default();
            for state in panes.states() {
                back.push(state.pane, state.note, state.editing_view);
            }
            back.restored();
            let shape = |p: &Panes| -> Vec<(PaneSide, String, EditingView)> {
                p.tabs()
                    .map(|(s, t)| (s, t.note.to_string(), t.view))
                    .collect()
            };
            assert_eq!(shape(&panes), shape(&back), "seed {seed}: {log:#?}");
            check(&back, &format!("seed {seed} restored"));
        }
    }
}
