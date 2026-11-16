//! UC-005 BR-016, FR-097, OQ-221: one note shown in two tabs of the split
//! view (UC-004 A19), as its source view on one side and its facade on the
//! other, has one text and one history of changes; each view keeps its own
//! cursor, selection, folded sections, Vim and Nano keys, facade blocks and
//! what it was told of the sections and hits.
//!
//! The note's tab ([`EditorTab`]) holds the state of one view at a time:
//! [`ViewSlots`] takes it out ([`ViewState`]) and puts in that of the view
//! that calls next, its places moved over the edits the other view made
//! meanwhile ([`Caret::after`]). Only while a note has two views are its
//! edits logged, by the revision they made, so a view that did not make
//! them can follow them: the source view as text changes
//! ([`ViewSlots::source_since`]), the facade by parsing the blocks around
//! what changed ([`EditorTab::bring_facade_up_to_date`]).

use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use ropey::Rope;
use underline_editor_core::{Caret, Edit, Keys};

use super::{EditorTab, FacadeModel};
use crate::editor_view::{units, EditorPatch, TextChange};
use crate::folds::Folds;
use crate::found::FoundKey;

/// What one view keeps of its own while another view of the note calls.
#[derive(Debug)]
pub struct ViewState {
    caret: Caret,
    keys: Keys,
    facade: FacadeModel,
    folds: Folds,
    found_told: Option<FoundKey>,
    copied_selection: Option<Range<usize>>,
    /// The revision of the text when the state was taken out.
    rev: u64,
    /// The text then, so its places can follow a change not logged edit
    /// by edit (a text replaced from outside, more edits than kept).
    text: Rope,
}

/// The most edits kept for views that did not make them, and the most text
/// they may hold; a view further behind shows its text anew. A view's
/// window may be behind the state the core keeps of it (it was asked for
/// its sections, not yet brought up to date), so the edits are kept by
/// these bounds, not by what each view followed.
const MOST_LOGGED: usize = 4096;
const MOST_LOGGED_BYTES: usize = 4 << 20;

/// The edits of the text, each with the revision it made, in order, within
/// [`MOST_LOGGED`] edits and [`MOST_LOGGED_BYTES`] of text.
#[derive(Debug, Default)]
struct EditLog {
    edits: VecDeque<(u64, Edit)>,
    bytes: usize,
}

/// The bytes of text an edit holds.
fn size(edit: &Edit) -> usize {
    edit.removed.len() + edit.inserted.len()
}

impl EditLog {
    fn clear(&mut self) {
        self.edits.clear();
        self.bytes = 0;
    }

    /// Takes `edits`, the last of which made revision `rev`; when that does
    /// not add up (more edits than revisions), the log starts anew, as
    /// nothing in it can be trusted.
    fn push(&mut self, edits: Vec<Edit>, rev: u64) {
        let Some(first) = u64::try_from(edits.len())
            .ok()
            .and_then(|count| rev.checked_add(1)?.checked_sub(count))
        else {
            self.clear();
            return;
        };
        for (rev, edit) in (first..).zip(edits) {
            self.bytes = self.bytes.saturating_add(size(&edit));
            self.edits.push_back((rev, edit));
        }
        while self.edits.len() > MOST_LOGGED || self.bytes > MOST_LOGGED_BYTES {
            let Some((_, edit)) = self.edits.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(size(&edit));
        }
    }

    /// The edits made after revision `since` up to `now`, in order; `None`
    /// when some are not logged (the text was replaced, or too many came).
    fn since(&self, since: u64, now: u64) -> Option<impl Iterator<Item = &Edit> + Clone> {
        let count = usize::try_from(now.checked_sub(since)?).ok()?;
        let start = self.edits.partition_point(|(rev, _)| *rev <= since);
        let edits = self.edits.range(start..);
        let contiguous = edits.len() == count
            && (count == 0
                || (self
                    .edits
                    .get(start)
                    .is_some_and(|(rev, _)| *rev == since + 1)
                    && self.edits.back().is_some_and(|(rev, _)| *rev == now)));
        contiguous.then(|| edits.map(|(_, edit)| edit))
    }
}

/// The views of one note and the edits the views not calling have not
/// followed yet.
#[derive(Debug, Default)]
pub struct ViewSlots {
    /// The states of the views not calling now, by the window's ids.
    slots: HashMap<u32, ViewState>,
    /// The view whose state the tab holds; `None` before the first and
    /// after it went.
    current: Option<u32>,
    /// The ids of every view, the one calling now too.
    views: Vec<u32>,
    /// The view that made the last input, for sealing the history.
    last_input: Option<u32>,
    /// The edits, while another view may follow them.
    log: EditLog,
}

impl ViewSlots {
    /// How many views show the note.
    #[must_use]
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// Whether no view shows the note.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// The view whose state the tab holds.
    #[must_use]
    pub fn current(&self) -> Option<u32> {
        self.current
    }

    /// A view `id` shows the note from now on. The first one takes the
    /// tab's state as it is; another one starts at the cursor of the view
    /// calling now, without folds.
    pub fn open(&mut self, tab: &EditorTab, id: u32) {
        if self.views.contains(&id) {
            return;
        }
        self.views.push(id);
        if self.current.is_none() && self.slots.is_empty() {
            self.current = Some(id);
            return;
        }
        let mut state = ViewState::fresh(tab);
        state.caret = Caret::at(tab.editor.cursor(), tab.editor.anchor());
        self.slots.insert(id, state);
    }

    /// The view `id` is gone; its state goes with it. The tab keeps the
    /// state it holds until another view calls.
    pub fn close(&mut self, id: u32) {
        self.views.retain(|&view| view != id);
        self.slots.remove(&id);
        if self.current == Some(id) {
            self.current = None;
        }
        if self.last_input == Some(id) {
            self.last_input = None;
        }
        if self.slots.is_empty() {
            self.log.clear();
        }
    }

    /// The edits `edits`, the last of which made the revision `rev`, each in
    /// chars of the text just before it; kept while a view did not follow
    /// them.
    pub fn log(&mut self, edits: Vec<Edit>, rev: u64) {
        if self.slots.is_empty() || edits.is_empty() {
            return;
        }
        self.log.push(edits, rev);
    }

    /// The view `id` calls: the tab holds its state from now on, its places
    /// moved over the edits made since it was taken out. With `input` (the
    /// user acts in that view), the next edit is not undone together with
    /// one another view made (UC-005 BR-016: the one history of the note).
    /// Returns whether the state changed.
    pub fn select(&mut self, tab: &mut EditorTab, id: u32, input: bool) -> bool {
        if !self.views.contains(&id) {
            self.open(tab, id);
        }
        let swapped = self.current != Some(id);
        if swapped {
            if let Some(current) = self.current {
                let state = tab.take_view();
                self.slots.insert(current, state);
            }
            let state = self
                .slots
                .remove(&id)
                .unwrap_or_else(|| ViewState::fresh(tab));
            let now = tab.editor.revision();
            match self.log.since(state.rev, now) {
                Some(edits) => tab.put_view(state, edits),
                None => {
                    // Not logged edit by edit: the places follow what
                    // changed between the text then and now.
                    let place = across(&state.text, tab.editor.document().rope());
                    let mut state = state;
                    state.caret = std::mem::take(&mut state.caret).mapped(place);
                    tab.put_view(state, std::iter::empty());
                }
            }
            self.current = Some(id);
            if self.slots.is_empty() {
                // No other view is left to follow the edits.
                self.log.clear();
            }
        }
        if input {
            if self.last_input.is_some_and(|last| last != id) {
                tab.editor.seal();
            }
            self.last_input = Some(id);
        }
        swapped
    }

    /// FR-097: the patch that brings the source view of the view calling
    /// now from revision `base` to the text as it is, with its own
    /// selection; `None` when it cannot, and the view takes the whole text
    /// again (`EditorTab::state`).
    #[must_use]
    pub fn source_since(&self, tab: &EditorTab, base: u32) -> Option<EditorPatch> {
        let now = tab.editor.revision();
        let since = rebase(base, now)?;
        let edits = self.log.since(since, now)?;
        if tab.editor.may_have_cr() {
            // Web units are UTF-16 units only after the line breaks are
            // normalised: the view takes the whole text again.
            return None;
        }
        let changes = web_changes(tab.editor.document().rope(), edits);
        let mut patch = tab.caught_up(since);
        patch.changes = changes;
        Some(patch)
    }
}

/// How a place (chars) of the text `old` moves into the text `new`: the
/// start both share and the end both share stay where they are; a place
/// in between goes to the end of what took its place (the idea of
/// `changed_bytes` of the facade, in chars).
fn across(old: &Rope, new: &Rope) -> impl Fn(usize) -> usize {
    let (old_len, new_len) = (old.len_chars(), new.len_chars());
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let most = old_len.min(new_len) - prefix;
    let suffix = old
        .chars_at(old_len)
        .reversed()
        .zip(new.chars_at(new_len).reversed())
        .take(most)
        .take_while(|(a, b)| a == b)
        .count();
    move |at: usize| {
        let at = at.min(old_len);
        if at <= prefix {
            at
        } else if at >= old_len - suffix {
            at - (old_len - suffix) + (new_len - suffix)
        } else {
            new_len - suffix
        }
    }
}

/// The full revision `base` names, a `u32` of the window, given the text is
/// at `now`; `None` when it is ahead.
fn rebase(base: u32, now: u64) -> Option<u64> {
    let behind = crate::editor_view::revision(now).wrapping_sub(base);
    now.checked_sub(u64::from(behind))
}

/// `edits`, which made the text `rope` is now (which holds no `\r`), as the
/// changes of the source view in web units, here UTF-16 units.
fn web_changes<'a>(rope: &Rope, edits: impl Iterator<Item = &'a Edit> + Clone) -> Vec<TextChange> {
    let edits: Vec<&Edit> = edits.collect();
    let mut text = rope.clone();
    let mut changes = Vec::with_capacity(edits.len());
    for edit in edits.iter().rev() {
        // Back to the text just before the edit.
        let inserted = edit.inserted.chars().count();
        let end = (edit.at + inserted).min(text.len_chars());
        text.remove(edit.at.min(end)..end);
        text.insert(edit.at.min(text.len_chars()), &edit.removed);
        let from = text.char_to_utf16_cu(edit.at.min(text.len_chars()));
        let removed = edit.removed.chars().count();
        let to = text.char_to_utf16_cu((edit.at + removed).min(text.len_chars()));
        changes.push(TextChange {
            from: units(from),
            to: units(to),
            insert: edit.inserted.clone(),
        });
    }
    changes.reverse();
    changes
}

impl ViewState {
    /// The state of a view that starts at the start of the text of `tab`.
    fn fresh(tab: &EditorTab) -> Self {
        Self {
            caret: Caret::default(),
            keys: Keys::default(),
            facade: FacadeModel::default(),
            folds: Folds::default(),
            found_told: None,
            copied_selection: None,
            rev: tab.editor.revision(),
            text: tab.editor.document().rope().clone(),
        }
    }
}

impl EditorTab {
    /// UC-005 BR-016: takes out the state of the view calling now.
    fn take_view(&mut self) -> ViewState {
        // The register and the searches stay with the window's keys.
        let mut keys = std::mem::take(&mut self.keys);
        self.keys.take_shared(&mut keys);
        ViewState {
            caret: self.editor.take_caret(),
            keys,
            facade: std::mem::take(&mut self.facade),
            folds: std::mem::take(&mut self.folds),
            found_told: self.found.told.take(),
            copied_selection: self.copied_selection.take(),
            rev: self.editor.revision(),
            text: self.editor.document().rope().clone(),
        }
    }

    /// UC-005 BR-016: puts in the state of the view that calls from now
    /// on, its places moved over `edits`, made since it was taken out. The
    /// keys it takes over the register and the searches the keys taken out
    /// held (one register per window, so a yank in one pane is put in the
    /// other).
    fn put_view<'a>(&mut self, state: ViewState, edits: impl Iterator<Item = &'a Edit>) {
        let changed = state.rev != self.editor.revision();
        self.editor.put_caret(state.caret.after(edits));
        let mut keys = state.keys;
        keys.take_shared(&mut self.keys);
        self.keys = keys;
        self.facade = state.facade;
        self.folds = state.folds;
        self.found.told = state.found_told;
        self.copied_selection = state.copied_selection.filter(|_| !changed);
        // The lines its folds hide, in the text as it is now.
        self.refold();
    }

    /// The patch of no change from revision `since` to the text as it is,
    /// with the selection of the view calling now; the view does not scroll.
    fn caught_up(&self, since: u64) -> EditorPatch {
        let mut patch = self.patch(since, &underline_editor_core::Outcome::default());
        patch.scroll = false;
        patch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_view::{EditorInput, KeyName};
    use underline_markup::NoteFormat;

    fn typed(tab: &mut EditorTab, text: &str) {
        tab.source_input(
            EditorInput::Text {
                text: text.to_owned(),
            },
            false,
        );
    }

    fn drained(slots: &mut ViewSlots, tab: &mut EditorTab) {
        let edits = tab.editor.take_edits();
        slots.log(edits, tab.editor.revision());
    }

    /// TC-005-41 step 1, UC-005 BR-016: an edit in one view shows in the
    /// other at once; each keeps its own cursor.
    #[test]
    fn uc005_tc41_each_view_follows_the_other_and_keeps_its_cursor() {
        let mut tab = EditorTab::new("alpha\nbeta\n", NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        // View 1 types at the end of "beta".
        slots.select(&mut tab, 1, true);
        tab.select(tab_rev(&tab), 10, 10);
        typed(&mut tab, "!");
        drained(&mut slots, &mut tab);
        // View 2, still at the start, follows as a patch of the source view.
        let base = tab_rev(&tab) - 1;
        slots.select(&mut tab, 2, false);
        let patch = slots.source_since(&tab, base).expect("the edit is logged");
        assert_eq!(patch.changes.len(), 1);
        assert_eq!(patch.changes[0].from, 10);
        assert_eq!(patch.changes[0].insert, "!");
        assert_eq!((patch.anchor, patch.head), (0, 0));
        // View 2 types at its own cursor.
        slots.select(&mut tab, 2, true);
        typed(&mut tab, "X");
        drained(&mut slots, &mut tab);
        assert_eq!(tab.editor.text(), "Xalpha\nbeta!\n");
        // View 1's cursor moved with the edit before it.
        slots.select(&mut tab, 1, true);
        assert_eq!(tab.editor.selection_web(), (12, 12));
    }

    /// TC-005-41 step 2, OQ-221: the views share one history; undo in either
    /// takes back the last change of the note, which the other view made.
    #[test]
    fn uc005_tc41_oq221_undo_takes_back_the_other_views_change() {
        let mut tab = EditorTab::new("", NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        slots.select(&mut tab, 1, true);
        typed(&mut tab, "left");
        drained(&mut slots, &mut tab);
        slots.select(&mut tab, 2, true);
        tab.select(tab_rev(&tab), 4, 4);
        typed(&mut tab, "right");
        drained(&mut slots, &mut tab);
        slots.select(&mut tab, 1, true);
        tab.source_input(
            EditorInput::Key {
                key: KeyName::Z,
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
                page: 1,
            },
            false,
        );
        assert_eq!(tab.editor.text(), "left");
    }

    /// TC-005-41 step 3: folding in one view leaves the other's sections
    /// expanded.
    #[test]
    fn uc005_tc41_folds_are_each_views_own() {
        let mut tab = EditorTab::new("# A\n\nbody\n\n# B\n\nmore\n", NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        slots.select(&mut tab, 1, true);
        tab.fold_line(0);
        assert!(!tab.editor.hidden_lines().is_empty());
        slots.select(&mut tab, 2, true);
        assert!(tab.editor.hidden_lines().is_empty());
        slots.select(&mut tab, 1, false);
        assert!(!tab.editor.hidden_lines().is_empty());
    }

    /// FR-097: a text replaced (a change from outside) is not logged; the
    /// view takes the whole text again.
    #[test]
    fn fr097_a_view_too_far_behind_takes_the_whole_text() {
        let mut tab = EditorTab::new("one", NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        slots.select(&mut tab, 1, true);
        let base = tab_rev(&tab);
        tab.reload("two");
        drained(&mut slots, &mut tab);
        slots.select(&mut tab, 2, false);
        assert!(slots.source_since(&tab, base).is_none());
        assert!(slots.source_since(&tab, tab_rev(&tab)).is_some());
    }

    /// TC-005-41, FR-097: the facade follows the source view's edits by
    /// parsing around them; its blocks are those a full parse gives.
    #[test]
    fn fr097_the_facade_follows_the_source_view_as_a_full_parse() {
        use crate::CachedViews;
        use underline_editor_core::facade::{parse_all, MakeViews};
        let text = "# A\n\nfirst para\n\n- one\n- two\n\n# B\n\nlast\n".repeat(20);
        let mut tab = EditorTab::new(&text, NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut views = CachedViews::new("Light", false);
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        // View 2 is the facade.
        slots.select(&mut tab, 2, false);
        tab.bring_facade_up_to_date(&mut views);
        // View 1 types in the source view.
        slots.select(&mut tab, 1, true);
        let at = u32::try_from(text.match_indices("first para").nth(5).unwrap().0 + 5).unwrap();
        tab.select(tab_rev(&tab), at, at);
        typed(&mut tab, "\n\nmore *text* ");
        drained(&mut slots, &mut tab);
        // The facade follows.
        slots.select(&mut tab, 2, false);
        let patch = tab.facade_refresh("Home", &mut views);
        assert!(!patch.blocks.is_empty());
        let now = tab.editor.text();
        let (blocks, _) = parse_all(&now, NoteFormat::Markdown);
        let full = CachedViews::new("Light", false).all(&blocks, &now, &Rope::from_str(&now));
        assert_eq!(tab.facade.views, full);
        assert_eq!(tab.facade.parsed, Some(tab.editor.revision()));
        // Only the blocks around the edit were sent, not the note's.
        let sent: usize = patch.blocks.iter().map(|change| change.blocks.len()).sum();
        assert!(sent < full.len() / 4, "{sent} of {}", full.len());
    }

    /// FR-097 (review): a text replaced as a whole is not logged edit by
    /// edit; the other view's cursor follows what changed between the
    /// texts, not only kept within the text's end.
    #[test]
    fn fr097_a_cursor_follows_a_replaced_text_by_what_stayed() {
        let mut tab = EditorTab::new("head\nmiddle\ntail end\n", NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        // View 2's cursor before "end".
        slots.select(&mut tab, 2, true);
        tab.select(tab_rev(&tab), 17, 17);
        slots.select(&mut tab, 1, true);
        tab.reload("head\nnew middle part\ntail end\n");
        drained(&mut slots, &mut tab);
        slots.select(&mut tab, 2, false);
        assert_eq!(tab.editor.selection_web(), (26, 26));
    }

    /// FR-097 (review): edits that do not add up to their revision leave
    /// the log empty instead of numbering them wrongly.
    #[test]
    fn fr097_edits_that_do_not_add_up_clear_the_log() {
        let mut log = EditLog::default();
        let edit = || Edit {
            at: 0,
            removed: String::new(),
            inserted: "x".into(),
        };
        log.push(vec![edit()], 5);
        assert!(log.since(4, 5).is_some());
        log.push(vec![edit(), edit()], 0);
        assert!(log.edits.is_empty());
        assert_eq!(log.bytes, 0);
    }

    fn tab_rev(tab: &EditorTab) -> u32 {
        crate::editor_view::revision(tab.editor.revision())
    }

    // ------------------------------------------------------------------
    // Property tests of the views of one note (monkey, package 7a).
    // ------------------------------------------------------------------

    use std::collections::{HashMap, HashSet};

    use underline_editor_core::facade::{parse_all, MakeViews};
    use underline_editor_core::sync::{apply_remote, Change, ChangeSet};

    use crate::facade_view::{FacadeAt, FacadeInput};
    use crate::tab::FacadeCall;
    use crate::CachedViews;

    /// A small deterministic generator; every failure prints its seed.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as usize
        }

        fn below(&mut self, n: usize) -> usize {
            self.next() % n.max(1)
        }
    }

    const WORDS: &[&str] = &[
        "alpha",
        "b",
        "\u{1F600}",
        "\u{e9}t\u{e9}",
        "x y",
        "\n",
        "\n\n",
        "# T\n",
        "- li\n",
        "```\n",
        "| a | b |\n",
        "\u{4e2d}\u{6587}",
        "z",
        "\r\n",
        "a\r\nb\r\n",
    ];

    /// The UTF-16 offset `units` as a byte index of `text`.
    fn byte_of(text: &str, units: u32) -> usize {
        let mut seen = 0;
        for (at, c) in text.char_indices() {
            if seen == units {
                return at;
            }
            seen += u32::try_from(c.len_utf16()).unwrap();
            assert!(
                seen <= units,
                "{units} is inside a surrogate pair of {text:?}"
            );
        }
        assert_eq!(seen, units, "{units} is past the end of {text:?}");
        text.len()
    }

    /// What a source view does with the patch of `source_since`: each change
    /// relative to the text after the ones before it.
    fn apply_changes(shown: &mut String, changes: &[crate::editor_view::TextChange]) {
        for change in changes {
            let (from, to) = (byte_of(shown, change.from), byte_of(shown, change.to));
            assert!(from <= to, "{change:?}");
            shown.replace_range(from..to, &change.insert);
        }
    }

    /// Two source views (1, 3) and a facade (2) of one note, as the window's
    /// core drives them, and the reference of what they must show.
    struct Rig {
        seed: u64,
        rng: Lcg,
        tab: EditorTab,
        slots: ViewSlots,
        views: CachedViews,
        /// The text of each source view's window and its revision.
        shown: HashMap<u32, (String, u32)>,
        /// Where each view's cursor and anchor must be, by the one
        /// independent rule: moved over the edits of the other views.
        pos: HashMap<u32, (usize, usize)>,
        /// Views whose places cannot be known (the text was replaced).
        unknown: HashSet<u32>,
        folded: HashSet<u32>,
        /// The text after each change, since the last replacement.
        past: Vec<String>,
        /// The revision of the last replacement of the text.
        replaced_at: u64,
        /// A `\\r` was typed at some time: no patch is due for a view
        /// that did not see the edits.
        ever_cr: bool,
        last_rev: u64,
        steps: Vec<String>,
    }

    fn moved_over(at: usize, edit: &Edit) -> usize {
        let removed = edit.removed.chars().count();
        let inserted = edit.inserted.chars().count();
        if at <= edit.at {
            at
        } else if at >= edit.at + removed {
            at + inserted - removed
        } else {
            edit.at + inserted
        }
    }

    impl Rig {
        fn new(seed: u64, text: &str) -> Self {
            let mut tab = EditorTab::new(text, NoteFormat::Markdown);
            tab.editor.record_edits();
            let mut slots = ViewSlots::default();
            for id in [1, 2, 3] {
                slots.open(&tab, id);
            }
            let rev = tab_rev(&tab);
            let mut shown = HashMap::new();
            shown.insert(1, (tab.editor.web_text(), rev));
            shown.insert(3, (tab.editor.web_text(), rev));
            let pos = HashMap::from([(1, (0, 0)), (2, (0, 0)), (3, (0, 0))]);
            Self {
                seed,
                rng: Lcg(seed.wrapping_mul(0x9E37_79B9).wrapping_add(7)),
                last_rev: tab.editor.revision(),
                tab,
                slots,
                views: CachedViews::new("Light", false),
                shown,
                pos,
                unknown: HashSet::new(),
                folded: HashSet::new(),
                past: vec![text.to_owned()],
                replaced_at: 0,
                ever_cr: false,
                steps: Vec::new(),
            }
        }

        fn fail(&self, what: &str) -> ! {
            panic!(
                "seed {}: {what}\n steps: {:#?}\n text {:?}",
                self.seed,
                self.steps,
                self.tab.editor.text()
            );
        }

        fn len(&self) -> usize {
            self.tab.editor.document().len_chars()
        }

        /// The edits so far go to the log; the places of the views not
        /// calling move over them.
        fn drain(&mut self, caller: u32) -> Vec<Edit> {
            let edits = self.tab.editor.take_edits();
            self.slots.log(edits.clone(), self.tab.editor.revision());
            for (view, place) in &mut self.pos {
                if *view == caller {
                    continue;
                }
                for edit in &edits {
                    place.0 = moved_over(place.0, edit);
                    place.1 = moved_over(place.1, edit);
                }
            }
            edits
        }

        /// The window's core: the calls from now on are those of `view`.
        fn enter(&mut self, view: u32, input: bool) {
            let current = self.slots.current();
            if current != Some(view) {
                self.drain(current.unwrap_or(0));
            }
            self.slots.select(&mut self.tab, view, input);
            let got = (self.tab.editor.cursor(), self.tab.editor.anchor());
            if got.0 > self.len() || got.1 > self.len() {
                self.fail(&format!("view {view}: {got:?} past the end"));
            }
            if !self.unknown.remove(&view) && self.pos[&view] != got {
                self.fail(&format!(
                    "view {view}: places {got:?}, expected {:?}",
                    self.pos[&view]
                ));
            }
            self.pos.insert(view, got);
            if !self.folded.contains(&view) && !self.tab.editor.hidden_lines().is_empty() {
                self.fail(&format!("view {view} shows folds it never made"));
            }
            let rev = self.tab.editor.revision();
            if rev < self.last_rev {
                self.fail("the revision went back");
            }
            self.last_rev = rev;
        }

        fn after_edit(&mut self, caller: u32) {
            self.drain(caller);
            let place = (self.tab.editor.cursor(), self.tab.editor.anchor());
            self.pos.insert(caller, place);
            let rev = self.tab.editor.revision();
            if rev < self.last_rev {
                self.fail("the revision went back");
            }
            self.last_rev = rev;
            let text = self.tab.editor.text();
            if self.past.last() != Some(&text) {
                self.past.push(text);
            }
        }

        /// A source view takes what the others changed, as the window does.
        fn refresh(&mut self, view: u32) {
            let (mut text, base) = self.shown[&view].clone();
            let edits = self.tab.editor.take_edits();
            self.slots.log(edits, self.tab.editor.revision());
            let now = tab_rev(&self.tab);
            let expected = self.tab.editor.web_text();
            match self.slots.source_since(&self.tab, base) {
                Some(patch) => {
                    if patch.base != base || patch.rev != now {
                        self.fail(&format!(
                            "patch {}..{} for {base}..{now}",
                            patch.base, patch.rev
                        ));
                    }
                    apply_changes(&mut text, &patch.changes);
                    if text != expected {
                        self.fail(&format!(
                            "view {view}: the patch of {base}..{now} gives {text:?}, not {expected:?}"
                        ));
                    }
                    if (patch.head as usize, patch.anchor as usize)
                        != self.tab.editor.selection_web()
                    {
                        self.fail("the patch's selection is not the view's");
                    }
                }
                None => {
                    let cr = self.ever_cr || self.tab.editor.text().contains('\r');
                    if u64::from(base) >= self.replaced_at && !cr && base != now {
                        self.fail(&format!(
                            "view {view}: no patch from {base} to {now}, no replacement since"
                        ));
                    }
                }
            }
            self.shown.insert(view, (expected, now));
        }

        fn select_random(&mut self) {
            let len = self.len();
            let (a, b) = (self.rng.below(len + 1), self.rng.below(len + 1));
            let (a, b) = if self.rng.below(2) == 0 {
                (a, a)
            } else {
                (a, b)
            };
            let (anchor, head) = (
                self.tab.editor.web_at_char(a),
                self.tab.editor.web_at_char(b),
            );
            self.tab
                .select(tab_rev(&self.tab), anchor as u32, head as u32);
            if let Some(view) = self.slots.current() {
                let place = (self.tab.editor.cursor(), self.tab.editor.anchor());
                self.pos.insert(view, place);
            }
        }

        fn facade_checked(&mut self) {
            self.facade_check(true);
        }

        fn facade_check(&mut self, strict: bool) {
            let text = self.tab.editor.text();
            self.tab.bring_facade_up_to_date(&mut self.views);
            let (blocks, _) = parse_all(&text, NoteFormat::Markdown);
            let full = CachedViews::new("Light", false).all(&blocks, &text, &Rope::from_str(&text));
            // The runs of a paragraph's lines are cut at other places after
            // an input of the facade (a defect of ADR-024, see
            // `adr024_typing_at_the_end_of_a_line...`); the text they hold is
            // compared, not where they are cut.
            let shape = |views: &[crate::facade_view::BlockView]| -> Vec<String> {
                views
                    .iter()
                    .map(|v| {
                        let runs: String = v.runs.iter().map(|r| r.text.as_str()).collect();
                        format!(
                            "{:?} {} {} {} {} {:?} {runs}",
                            v.kind, v.start, v.line, v.length, v.lines, v.prefix
                        )
                    })
                    .collect()
            };
            let differs = self.tab.facade.blocks.len() != blocks.len()
                || shape(&self.tab.facade.views) != shape(&full);
            if !strict && differs {
                // An input of the facade itself left blocks apart from a full
                // parse; the facade starts anew, so that this does not hide
                // what the follow of another view's edits does.
                self.tab.facade.parsed = None;
            }
            if strict && differs {
                self.fail("the facade's blocks are not those of a full parse");
            }
        }

        fn step(&mut self) {
            let view = [1, 2, 3][self.rng.below(3)];
            let op = self.rng.below(10);
            self.steps.push(format!("view {view} op {op}"));
            let input = !matches!(op, 6 | 7);
            self.enter(view, input);
            if view != 2 && !matches!(op, 6) {
                self.refresh(view);
            }
            if view == 2 {
                self.facade_checked();
            }
            match op {
                0..=2 => {
                    let word = WORDS[self.rng.below(WORDS.len())];
                    if view == 2 {
                        self.select_random();
                        let (head, anchor) = self.tab.editor.selection_web();
                        let at = FacadeAt {
                            rev: tab_rev(&self.tab),
                            anchor: anchor as u32,
                            head: head as u32,
                        };
                        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
                        let call = FacadeCall {
                            note: "Home",
                            at,
                            mac: false,
                            earlier,
                            views: &mut self.views,
                        };
                        let text = word.replace(['\n', '\r', '#', '`'], "");
                        self.tab
                            .facade_input(call, FacadeInput::Text { text }, None, None);
                        self.after_edit(view);
                        self.facade_check(false);
                    } else {
                        self.select_random();
                        let range = self.tab.editor.selection();
                        let mut expected = self.tab.editor.text();
                        let bytes: Vec<usize> = expected
                            .char_indices()
                            .map(|(at, _)| at)
                            .chain([expected.len()])
                            .collect();
                        expected.replace_range(bytes[range.start]..bytes[range.end], word);
                        typed(&mut self.tab, word);
                        self.ever_cr |= word.contains('\r');
                        self.after_edit(view);
                        if self.tab.editor.text() != expected {
                            self.fail("typing did not give the reference text");
                        }
                        let (text, _) = self.shown[&view].clone();
                        let now = tab_rev(&self.tab);
                        self.shown.insert(view, (text, now));
                        self.refresh_after_own(view);
                    }
                }
                3 | 4 => {
                    // Undo (3) and redo (4), in either view.
                    let before = self.tab.editor.text();
                    let (key, shift) = if op == 3 {
                        (KeyName::Z, false)
                    } else {
                        (KeyName::Y, false)
                    };
                    if view == 2 {
                        let (head, anchor) = self.tab.editor.selection_web();
                        let at = FacadeAt {
                            rev: tab_rev(&self.tab),
                            anchor: anchor as u32,
                            head: head as u32,
                        };
                        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
                        let call = FacadeCall {
                            note: "Home",
                            at,
                            mac: false,
                            earlier,
                            views: &mut self.views,
                        };
                        self.tab.facade_input(
                            call,
                            FacadeInput::Key {
                                key,
                                ctrl: true,
                                alt: false,
                                shift,
                                meta: false,
                            },
                            None,
                            None,
                        );
                    } else {
                        self.tab.source_input(
                            EditorInput::Key {
                                key,
                                ctrl: true,
                                alt: false,
                                shift,
                                meta: false,
                                page: 1,
                            },
                            false,
                        );
                    }
                    self.after_edit(view);
                    if view == 2 {
                        self.facade_check(false);
                    } else {
                        self.refresh_after_own(view);
                    }
                    let _ = before;
                }
                5 => {
                    // The backend or an agent edits.
                    let len = self.len();
                    let at = self.rng.below(len + 1);
                    let remove = self.rng.below((len - at).min(6) + 1);
                    let insert = WORDS[self.rng.below(WORDS.len())].to_owned();
                    self.ever_cr |= insert.contains('\r');
                    let set = ChangeSet::of(len, &Change { at, remove, insert }).unwrap();
                    let sealed = self.rng.below(2) == 0;
                    apply_remote(&mut self.tab.editor, &set, sealed).unwrap();
                    self.after_edit(view);
                    if view == 2 {
                        self.facade_check(false);
                    } else {
                        self.refresh_after_own(view);
                    }
                }
                6 => {
                    // The note is replaced from outside.
                    let text = format!("{}{}", WORDS[self.rng.below(WORDS.len())], "tail\nmore\n");
                    self.ever_cr |= text.contains('\r');
                    self.tab.reload(&text);
                    self.replaced_at = self.tab.editor.revision();
                    self.drain(view);
                    for id in [1, 2, 3] {
                        if id != view {
                            self.unknown.insert(id);
                        }
                    }
                    let place = (self.tab.editor.cursor(), self.tab.editor.anchor());
                    self.pos.insert(view, place);
                    self.past = vec![self.tab.editor.text()];
                    self.last_rev = self.tab.editor.revision();
                }
                7 => {
                    // Only a fold or a move of the cursor.
                    let lines = self.tab.editor.document().len_lines();
                    let line = self.rng.below(lines);
                    self.tab.fold_line(line as u32);
                    self.folded.insert(view);
                    self.select_random();
                    let place = (self.tab.editor.cursor(), self.tab.editor.anchor());
                    self.pos.insert(view, place);
                }
                _ => {
                    // Backspace over a selection.
                    self.select_random();
                    if view == 2 {
                        let (head, anchor) = self.tab.editor.selection_web();
                        let at = FacadeAt {
                            rev: tab_rev(&self.tab),
                            anchor: anchor as u32,
                            head: head as u32,
                        };
                        let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
                        let call = FacadeCall {
                            note: "Home",
                            at,
                            mac: false,
                            earlier,
                            views: &mut self.views,
                        };
                        self.tab.facade_input(
                            call,
                            FacadeInput::Key {
                                key: KeyName::Backspace,
                                ctrl: false,
                                alt: false,
                                shift: false,
                                meta: false,
                            },
                            None,
                            None,
                        );
                    } else {
                        self.tab.source_input(
                            EditorInput::Key {
                                key: KeyName::Backspace,
                                ctrl: false,
                                alt: false,
                                shift: false,
                                meta: false,
                                page: 1,
                            },
                            false,
                        );
                    }
                    self.after_edit(view);
                    if view == 2 {
                        self.facade_check(false);
                    } else {
                        self.refresh_after_own(view);
                    }
                }
            }
        }

        /// The calling source view has its own edit in its window.
        fn refresh_after_own(&mut self, view: u32) {
            let text = self.tab.editor.web_text();
            let now = tab_rev(&self.tab);
            self.shown.insert(view, (text, now));
        }
    }

    /// UC-005 TC-005-41, BR-016, FR-097 (property, seeds printed): two source
    /// views and a facade of one note, random inputs, folds, undo and redo,
    /// agent edits and replacements. Every view follows as a patch that gives
    /// the text, its places move over the others' edits, the facade equals a
    /// full parse, and the revisions only go forward.
    #[test]
    fn uc005_tc41_br016_views_of_one_note_follow_each_other_over_random_steps() {
        for seed in 0..1500u64 {
            let mut rig = Rig::new(
                seed,
                "# One\n\nalpha \u{1F600} beta\n\n- a\n- b\n\n```\ncode\n```\n\nend\n",
            );
            for _ in 0..50 {
                rig.step();
            }
        }
    }

    /// Two views of `text`; the first calls, the second lags.
    fn two_views(text: &str) -> (EditorTab, ViewSlots) {
        let mut tab = EditorTab::new(text, NoteFormat::Markdown);
        tab.editor.record_edits();
        let mut slots = ViewSlots::default();
        slots.open(&tab, 1);
        slots.open(&tab, 2);
        slots.select(&mut tab, 1, true);
        (tab, slots)
    }

    /// `text` as the lagging view's window holds it at revision `base`, taken
    /// forward by `source_since`; `None` when it asks for the whole text.
    fn followed(tab: &EditorTab, slots: &ViewSlots, mut text: String, base: u32) -> Option<String> {
        let patch = slots.source_since(tab, base)?;
        apply_changes(&mut text, &patch.changes);
        Some(text)
    }

    /// UC-004 A19, FR-097: the log keeps the last 4096 edits; a view further
    /// behind takes the whole text, and a view within them gets edits that
    /// give the text.
    #[test]
    fn fr097_a_view_4097_edits_behind_takes_the_whole_text() {
        let (mut tab, mut slots) = two_views("start\n");
        let base_text = tab.editor.web_text();
        let oldest = tab_rev(&tab);
        let mut texts = vec![base_text];
        for k in 0..4100 {
            typed(&mut tab, if k % 7 == 0 { "\u{1F600}" } else { "k" });
            drained(&mut slots, &mut tab);
            texts.push(tab.editor.web_text());
        }
        let now = tab_rev(&tab);
        for behind in [1u32, 2, 100, 4095, 4096, 4097, 4100] {
            let base = now - behind;
            let shown = texts[(base - oldest) as usize].clone();
            let got = followed(&tab, &slots, shown, base);
            if behind <= 4096 {
                assert_eq!(
                    got.as_deref(),
                    Some(tab.editor.web_text().as_str()),
                    "{behind} edits behind"
                );
            } else {
                assert!(
                    got.is_none(),
                    "{behind} edits behind give edits, not the text"
                );
            }
        }
    }

    /// FR-097: the log keeps at most 4 MB of text; a view behind more than
    /// that takes the whole text, never edits that are not all there.
    #[test]
    fn fr097_a_view_behind_more_than_4_mb_takes_the_whole_text() {
        let (mut tab, mut slots) = two_views("");
        let chunk = "\u{e9}".repeat(1_000_000);
        let mut bases = Vec::new();
        let mut texts = Vec::new();
        for _ in 0..4 {
            bases.push(tab_rev(&tab));
            texts.push(tab.editor.web_text());
            typed(&mut tab, &chunk);
            drained(&mut slots, &mut tab);
        }
        // 4 chunks of two bytes hold 8 MB, past the bound: the oldest ones
        // are gone, and no base gives a patch that is wrong.
        let now = tab_rev(&tab);
        assert!(now > 0);
        let want = tab.editor.web_text();
        for (k, (base, shown)) in bases.into_iter().zip(texts).enumerate() {
            let got = followed(&tab, &slots, shown, base);
            if k < 2 {
                assert!(got.is_none(), "{k} edits of 2 MB behind: not kept");
            } else {
                assert!(
                    got.as_deref() == Some(want.as_str()),
                    "{k}: the kept edits give the text"
                );
            }
        }
        // And the one right behind the last edit is always served.
        let last = followed(
            &tab,
            &slots,
            tab.editor.web_text()[..chunk.len() * 3].to_owned(),
            now - 1,
        );
        assert_eq!(last.as_deref(), Some(want.as_str()));
    }

    /// FR-097: one edit of more than the bound is not kept; the views behind
    /// take the whole text, the next edits are served again.
    #[test]
    fn fr097_an_edit_over_the_bound_is_not_kept_and_later_ones_are() {
        let (mut tab, mut slots) = two_views("");
        let base = tab_rev(&tab);
        typed(&mut tab, &"a".repeat(5 << 20));
        drained(&mut slots, &mut tab);
        assert!(slots.source_since(&tab, base).is_none());
        let after = tab_rev(&tab);
        let shown = tab.editor.web_text();
        typed(&mut tab, "tail");
        drained(&mut slots, &mut tab);
        assert_eq!(
            followed(&tab, &slots, shown, after).as_deref(),
            Some(tab.editor.web_text().as_str())
        );
    }

    /// Opening and closing views in any order, with edits between: the log
    /// is dropped with the last other view, a later view starts from the
    /// text as it is, and a view's places stay on the text.
    #[test]
    fn br016_views_opened_and_closed_in_any_order_keep_their_places_on_the_text() {
        for seed in 0..2000u64 {
            let mut rng = Lcg(seed + 99);
            let mut tab = EditorTab::new("one\ntwo\nthree\n", NoteFormat::Markdown);
            tab.editor.record_edits();
            let mut slots = ViewSlots::default();
            let mut open: Vec<u32> = Vec::new();
            let mut shown: HashMap<u32, (String, u32)> = HashMap::new();
            for step in 0..60 {
                let id = u32::try_from(rng.below(5)).unwrap();
                match rng.below(5) {
                    0 => {
                        slots.open(&tab, id);
                        if !open.contains(&id) {
                            open.push(id);
                            shown.insert(id, (tab.editor.web_text(), tab_rev(&tab)));
                        }
                    }
                    1 => {
                        slots.close(id);
                        open.retain(|&view| view != id);
                        shown.remove(&id);
                    }
                    _ => {
                        if !open.contains(&id) {
                            continue;
                        }
                        let edits = tab.editor.take_edits();
                        slots.log(edits, tab.editor.revision());
                        slots.select(&mut tab, id, true);
                        let len = tab.editor.document().len_chars();
                        assert!(
                            tab.editor.cursor() <= len && tab.editor.anchor() <= len,
                            "seed {seed} step {step}: view {id} at {} of {len}",
                            tab.editor.cursor()
                        );
                        let (text, base) = shown[&id].clone();
                        let now = tab_rev(&tab);
                        let edits = tab.editor.take_edits();
                        slots.log(edits, tab.editor.revision());
                        let wanted = tab.editor.web_text();
                        let got =
                            followed(&tab, &slots, text, base).unwrap_or_else(|| wanted.clone());
                        assert_eq!(
                            got, wanted,
                            "seed {seed} step {step}: view {id} from {base} to {now}"
                        );
                        shown.insert(id, (wanted, now));
                        let at = rng.below(len + 1);
                        tab.select(
                            now,
                            u32::try_from(tab.editor.web_at_char(at)).unwrap(),
                            u32::try_from(tab.editor.web_at_char(at)).unwrap(),
                        );
                        typed(&mut tab, WORDS[rng.below(WORDS.len())]);
                        let edits = tab.editor.take_edits();
                        slots.log(edits, tab.editor.revision());
                        let own = tab.editor.web_text();
                        shown.insert(id, (own, tab_rev(&tab)));
                    }
                }
            }
        }
    }

    impl Rig {
        /// Ctrl+`key` in `view`, as the window sends it.
        fn press(&mut self, view: u32, key: KeyName) {
            self.enter(view, true);
            if view == 2 {
                self.facade_check(true);
                let (head, anchor) = self.tab.editor.selection_web();
                let at = FacadeAt {
                    rev: tab_rev(&self.tab),
                    anchor: u32::try_from(anchor).unwrap(),
                    head: u32::try_from(head).unwrap(),
                };
                let earlier = self.tab.bring_facade_up_to_date(&mut self.views);
                let call = FacadeCall {
                    note: "Home",
                    at,
                    mac: false,
                    earlier,
                    views: &mut self.views,
                };
                let input = FacadeInput::Key {
                    key,
                    ctrl: true,
                    alt: false,
                    shift: false,
                    meta: false,
                };
                self.tab.facade_input(call, input, None, None);
                self.after_edit(view);
                self.facade_check(false);
            } else {
                self.refresh(view);
                let input = EditorInput::Key {
                    key,
                    ctrl: true,
                    alt: false,
                    shift: false,
                    meta: false,
                    page: 1,
                };
                self.tab.source_input(input, false);
                self.after_edit(view);
                self.refresh_after_own(view);
            }
        }

        /// Undoes in the views by turns until nothing changes; the texts it
        /// went through, from the last on.
        fn walk(&mut self, key: KeyName) -> Vec<String> {
            let mut seen = Vec::new();
            let mut still = 0;
            while still < 3 && seen.len() < 500 {
                let before = self.tab.editor.text();
                let view = [1, 2, 3][self.rng.below(3)];
                self.press(view, key);
                let after = self.tab.editor.text();
                if after == before {
                    still += 1;
                } else {
                    still = 0;
                    seen.push(after);
                }
            }
            seen
        }
    }

    /// UC-005 BR-004, BR-016, OQ-221 (property, seeds printed): undoing
    /// through the views by turns goes back through texts the note had, in
    /// order, to the first; redoing goes the same way forward to the last.
    #[test]
    fn uc005_br016_undo_and_redo_in_any_view_restore_the_earlier_texts_in_order() {
        let mut steps_undone = 0;
        for seed in 0..600u64 {
            let mut rig = Rig::new(seed + 5000, "# One\n\nalpha beta\n\n- a\n- b\n");
            for _ in 0..30 {
                let view = [1, 2, 3][rig.rng.below(3)];
                // No replacement: the history is the whole run.
                let op = [0, 0, 0, 5, 8][rig.rng.below(5)];
                rig.enter(view, true);
                if view != 2 {
                    rig.refresh(view);
                }
                match op {
                    5 => {
                        let len = rig.len();
                        let at = rig.rng.below(len + 1);
                        let insert = WORDS[rig.rng.below(WORDS.len())].replace('\r', "");
                        let set = ChangeSet::of(
                            len,
                            &Change {
                                at,
                                remove: 0,
                                insert,
                            },
                        )
                        .unwrap();
                        let sealed = rig.rng.below(2) == 0;
                        apply_remote(&mut rig.tab.editor, &set, sealed).unwrap();
                        rig.after_edit(view);
                    }
                    8 => {
                        rig.select_random();
                        if view == 2 {
                            continue;
                        }
                        rig.tab.source_input(
                            EditorInput::Key {
                                key: KeyName::Backspace,
                                ctrl: false,
                                alt: false,
                                shift: false,
                                meta: false,
                                page: 1,
                            },
                            false,
                        );
                        rig.after_edit(view);
                        rig.refresh_after_own(view);
                    }
                    _ => {
                        if view == 2 {
                            continue;
                        }
                        rig.select_random();
                        let word = WORDS[rig.rng.below(WORDS.len())].replace('\r', "");
                        typed(&mut rig.tab, &word);
                        rig.after_edit(view);
                        rig.refresh_after_own(view);
                    }
                }
            }
            let last = rig.tab.editor.text();
            let past = rig.past.clone();
            let undone = rig.walk(KeyName::Z);
            steps_undone += undone.len();
            // Each text undone to is one the note had, going back.
            let mut at = past.len();
            for text in &undone {
                match past[..at].iter().rposition(|earlier| earlier == text) {
                    Some(found) => at = found,
                    None => rig.fail(&format!(
                        "undo went to {text:?}, a text the note never had before"
                    )),
                }
            }
            if rig.tab.editor.text() != past[0] {
                rig.fail("undoing in turns did not end at the first text");
            }
            let redone = rig.walk(KeyName::Y);
            let mut back: Vec<String> = undone[..undone.len().saturating_sub(1)].to_vec();
            back.reverse();
            back.push(last.clone());
            if redone != back {
                rig.fail(&format!("redo went {redone:?}, not back over {back:?}"));
            }
        }
        assert!(
            steps_undone > 1000,
            "the run did undo something: {steps_undone}"
        );
    }
}
