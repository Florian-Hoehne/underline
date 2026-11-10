//! UC-008 Find Note, inventory M-13.1 and M-13.3: the search of the
//! workspace for the search overlay. Ported from `run_search`,
//! `open_found`, `found_list` and `choice` of
//! `apps/underline-desktop/src/app.rs`.
//!
//! The search runs on the thread of [`Searcher`] (`underline-core`), never
//! while the session is held: the session only hands it the notes in the
//! order of the overview and the texts of the open tabs (BR-001), and takes
//! its answer in with the tick. As in egui:
//!
//! - a result counts only when it was searched for the text the window
//!   holds now; until a newer one fits, the one before stays;
//! - the search runs again when the text, the overview (any refresh of it)
//!   or the text of an open tab changed (A3);
//! - opening a found note opens it as the overview does (UC-004), at the
//!   word found, looked up in the text the tab holds then.
//!
//! The words are marked in the open tabs by the window's editor cores
//! (ADR-025, `docs/migration/offene-fragen.md` Q-38), not here.
//!
//! The window gets the found notes page by page ([`SearchResultView::PAGE`]),
//! so a search that finds every note of a large workspace is not one event
//! of megabytes (paritaet.md A-89).

use std::collections::HashMap;
use std::sync::Arc;

use underline_core::{
    FoundLine, FoundNote, NoteEntry, NoteFormat, NoteId, SearchRequest, SearchResult,
};
use underline_editor_core::utf16::WebOffsets;

use super::editing::not_open;
use super::Session;
use crate::editor_view::units;
use crate::search_view::{
    FoundLineView, FoundNoteView, FoundPartView, FoundPlace, SearchOpened, SearchResultView,
    SearchView,
};
use crate::{Event, UiError};

/// The search of the window: its text, the newest result for it, and what
/// was asked and told.
#[derive(Debug, Default)]
pub(super) struct Search {
    /// The text of the overlay, without the spaces around it.
    text: String,
    /// The newest result that fit the text when it came, with its number.
    result: Option<(u32, Arc<SearchResult>)>,
    /// The number of the last result taken in; grows for the whole session.
    serial: u32,
    /// The text and the state of the notes the searcher was asked for last.
    asked: Option<(String, u64)>,
    /// The texts of the open tabs sent last, by the revision of each.
    texts: HashMap<NoteId, (u32, String)>,
    /// The text and the result the window was told of last; at first no
    /// text and no result, as the window starts.
    told: (String, Option<u32>),
}

/// "1 note", "2 notes", as egui's `count`.
fn count(n: usize, thing: &str) -> String {
    if n == 1 {
        format!("1 {thing}")
    } else {
        format!("{n} {thing}s")
    }
}

/// The place of a line found: its first word.
fn place_of(line: &FoundLine) -> FoundPlace {
    FoundPlace {
        from: units(line.first.start),
        to: units(line.first.end),
    }
}

/// BR-004: a line found in parts, its words marked.
fn line_view(line: &FoundLine) -> FoundLineView {
    let mut parts = Vec::new();
    let mut at = 0;
    for mark in &line.marks {
        if mark.start > at {
            parts.push(FoundPartView {
                text: line.shown[at..mark.start].to_owned(),
                hit: false,
            });
        }
        parts.push(FoundPartView {
            text: line.shown[mark.clone()].to_owned(),
            hit: true,
        });
        at = mark.end;
    }
    if at < line.shown.len() {
        parts.push(FoundPartView {
            text: line.shown[at..].to_owned(),
            hit: false,
        });
    }
    FoundLineView {
        parts,
        place: place_of(line),
    }
}

/// UC-008 step 2, A5: a found note as `found_list` of egui shows it.
fn note_view(found: &FoundNote) -> FoundNoteView {
    let places = match found.places {
        0 => "in the title or folder".to_owned(),
        1 => "1 place".to_owned(),
        p => format!("{p} places"),
    };
    FoundNoteView {
        note: found.id.as_str().to_owned(),
        title: found.id.title(),
        folder: found.id.folder().map(str::to_owned),
        places,
        lines: found.lines.iter().map(line_view).collect(),
        more: units(found.places.saturating_sub(found.lines.len())),
        // egui's `choice`: a note opens at its first line shown.
        place: found.lines.first().map(place_of),
    }
}

impl Session {
    /// UC-008 steps 1–2, A2: the overlay holds `text`; the search runs
    /// again at once (a running one is given up, BR-005), or ends when it
    /// holds no more than spaces.
    pub fn search_set(&mut self, text: &str) {
        text.trim().clone_into(&mut self.search.text);
        self.run_search();
    }

    /// UC-008 step 2: the found notes of the result `serial` from the
    /// `from`th on, at most [`SearchResultView::PAGE`]; `None` when that
    /// result is no longer the newest.
    #[must_use]
    pub fn search_page(&self, serial: u32, from: u32) -> Option<Vec<FoundNoteView>> {
        let (taken, result) = self.search.result.as_ref()?;
        if *taken != serial {
            return None;
        }
        let found = result.found.get(from as usize..).unwrap_or_default();
        Some(
            found
                .iter()
                .take(SearchResultView::PAGE)
                .map(note_view)
                .collect(),
        )
    }

    /// UC-008 step 4, M-13.3: opens the found note `note` as the overview
    /// does (UC-004), and gives the word found at `place` (bytes of the text
    /// searched) in the text the tab holds now, clamped to its end, as egui
    /// selects it. `None` when the note has no place or did not open (the
    /// status says why, UC-004 A10).
    ///
    /// # Errors
    ///
    /// The note opened, but its tab is not there.
    pub fn search_open(
        &mut self,
        note: &NoteId,
        place: Option<FoundPlace>,
    ) -> Result<Option<SearchOpened>, UiError> {
        if !self.open_note(note) {
            return Ok(None);
        }
        let Some(place) = place else {
            return Ok(None);
        };
        let open = self.open.as_ref().ok_or_else(|| not_open(note))?;
        let tab = open
            .tabs
            .get(open.tabs.find(note).ok_or_else(|| not_open(note))?);
        let rope = tab.mirror.rope();
        let len = rope.len_bytes();
        let chars = |byte: u32| rope.byte_to_char((byte as usize).min(len));
        let web = WebOffsets::of(rope);
        Ok(Some(SearchOpened {
            note: note.as_str().to_owned(),
            rev: tab.mirror.rev(),
            anchor: units(web.to_web(rope, chars(place.from))),
            head: units(web.to_web(rope, chars(place.to.max(place.from)))),
        }))
    }

    /// UC-008 step 2, A3: takes in the searcher's newest answer when it is
    /// for the text held now, and asks again when the text, the overview
    /// or a text open in a tab changed (egui's `run_search`).
    pub(super) fn run_search(&mut self) {
        let text = self.search.text.clone();
        let Some(open) = &self.open else {
            return;
        };
        // The staleness rule is by text, as in egui: an answer counts only
        // when it was searched for the text held now; any other is dropped
        // (a newer request for this text is on its way or follows below).
        if let Some(result) = open.searcher.poll() {
            if result.text == text {
                self.search.serial = self.search.serial.wrapping_add(1);
                self.search.result = Some((self.search.serial, Arc::new(result)));
            }
        }
        if text.is_empty() {
            self.search.result = None;
            self.search.asked = None;
            self.search.texts.clear();
            return;
        }
        let revisions = open
            .tabs
            .iter()
            .fold(open.search_generation, |sum, (_, tab)| {
                sum.wrapping_mul(31)
                    .wrapping_add(u64::from(tab.mirror.rev()))
            });
        let stamp = (text.clone(), revisions);
        if self.search.asked.as_ref() == Some(&stamp) {
            return;
        }
        let formats: HashMap<&NoteId, NoteFormat> = open
            .workspace
            .listing()
            .notes
            .iter()
            .map(|n| (&n.id, n.format))
            .collect();
        let notes = open
            .overview
            .iter()
            .filter_map(|entry| {
                formats.get(&entry.id).map(|format| NoteEntry {
                    id: entry.id.clone(),
                    format: *format,
                })
            })
            .collect();
        // BR-001: the open tabs' texts. Only a tab whose revision changed
        // is read from its rope again; the others are copied from the last
        // request (the request owns its strings, a type egui shares).
        let cached = &mut self.search.texts;
        cached.retain(|id, _| open.tabs.find(id).is_some());
        let texts = open
            .tabs
            .iter()
            .map(|(id, tab)| {
                let rev = tab.mirror.rev();
                let entry = cached
                    .entry(id.clone())
                    .or_insert_with(|| (rev, tab.mirror.text()));
                if entry.0 != rev {
                    *entry = (rev, tab.mirror.text());
                }
                (id.clone(), entry.1.clone())
            })
            .collect();
        open.searcher.search(SearchRequest {
            text,
            notes,
            open: texts,
        });
        self.search.asked = Some(stamp);
    }

    /// M-21.3: a workspace opened; the search starts anew in it with the
    /// text the overlay holds (egui keeps the text).
    pub(super) fn search_switched(&mut self) {
        self.search.texts.clear();
        self.search.result = None;
        self.search.asked = None;
    }

    /// The tick's part of the search: the answer is taken in, a new search
    /// started when needed, and the window told of a changed text or result.
    pub(super) fn search_events(&mut self) -> Vec<Event> {
        self.run_search();
        let told = (
            self.search.text.clone(),
            self.search.result.as_ref().map(|(serial, _)| *serial),
        );
        if self.search.told == told {
            return Vec::new();
        }
        self.search.told = told;
        vec![Event::Search(self.search_view())]
    }

    /// UC-008 step 2: the search as the overlay shows it.
    pub(super) fn search_view(&self) -> SearchView {
        SearchView {
            text: self.search.text.clone(),
            result: self
                .search
                .result
                .as_ref()
                .map(|(serial, result)| SearchResultView {
                    serial: *serial,
                    text: result.text.clone(),
                    total: units(result.found.len()),
                    notes: result
                        .found
                        .iter()
                        .take(SearchResultView::PAGE)
                        .map(note_view)
                        .collect(),
                    unreadable: (result.unreadable > 0).then(|| {
                        format!(
                            "{} could not be searched.",
                            count(result.unreadable, "note")
                        )
                    }),
                }),
        }
    }
}

#[cfg(test)]
mod tests;
