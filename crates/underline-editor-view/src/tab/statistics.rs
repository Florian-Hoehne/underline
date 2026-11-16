//! UC-004 BR-020 (FR-090, M-16.2) in the tab: the text statistics of the
//! note and of its selection, counted in the webview's editor core on the
//! text it owns (ADR-025), so no key waits for the backend.
//!
//! What a revision of the text costs is paid once: its text is written out
//! once, its blocks are the facade's when they are of it (else parsed once
//! from that text), and the note's counts are kept for it. A moved
//! selection then counts only the blocks it touches, and asking again for an
//! unchanged text and selection costs nothing.

use std::ops::Range;
use std::sync::Arc;

use underline_markup::blocks::{parse_blocks, Block};
use underline_markup::statistics::{note_counts, selection_counts, Counts};

use super::EditorTab;
use crate::editor_view::{units, SelectionStatisticsView, TextStatisticsView};

/// What the tab keeps of one revision of the text for the statistics.
#[derive(Debug, Clone)]
struct Counted {
    revision: u64,
    text: Arc<str>,
    blocks: Arc<Vec<Block>>,
    note: Counts,
}

/// What the tab keeps of the last statistics.
#[derive(Debug, Clone, Default)]
pub(crate) struct StatisticsCache {
    /// The text, blocks and note counts of the last revision counted.
    counted: Option<Counted>,
    /// The last answer, for the revision and the selection (chars).
    answer: Option<(u64, Range<usize>, TextStatisticsView)>,
}

impl EditorTab {
    /// UC-004 BR-020: the text statistics of the note, with those of the
    /// tab's selection while text is selected.
    pub fn text_statistics(&mut self) -> TextStatisticsView {
        let revision = self.editor.revision();
        let selection = self.editor.selection();
        if let Some((rev, range, answer)) = &self.statistics.answer {
            if *rev == revision && *range == selection {
                return *answer;
            }
        }
        let counted = self.counted(revision);
        let selected = (!selection.is_empty()).then(|| {
            let rope = self.editor.document().rope();
            let bytes = rope.char_to_byte(selection.start)..rope.char_to_byte(selection.end);
            selection_counts(&counted.text, self.format, &counted.blocks, bytes)
        });
        let answer = view_of(counted.note, selected);
        self.statistics.answer = Some((revision, selection, answer));
        answer
    }

    /// The text, blocks and note counts of `revision`, worked out once.
    fn counted(&mut self, revision: u64) -> Counted {
        if let Some(counted) = &self.statistics.counted {
            if counted.revision == revision {
                return counted.clone();
            }
        }
        let text: Arc<str> = Arc::from(self.editor.text());
        let blocks = if self.facade.parsed == Some(revision) {
            Arc::clone(&self.facade.blocks)
        } else {
            Arc::new(parse_blocks(&text, self.format))
        };
        let note = note_counts(&text, self.format, &blocks);
        let counted = Counted {
            revision,
            text,
            blocks,
            note,
        };
        self.statistics.counted = Some(counted.clone());
        counted
    }
}

/// The window's view of the note's counts and the selection's.
fn view_of(note: Counts, selection: Option<Counts>) -> TextStatisticsView {
    TextStatisticsView {
        words: units(note.words),
        characters: units(note.characters),
        reading_minutes: units(note.reading_minutes()),
        selection: selection.map(|selection| SelectionStatisticsView {
            words: units(selection.words),
            characters: units(selection.characters),
        }),
    }
}

#[cfg(test)]
mod tests {
    use underline_markup::NoteFormat;

    use super::*;

    const COUNT_MD: &str = "# Title\n\nIt's a well-known [fact](https://example.org/a-b-c).\n\n```rust\nlet x = 1;\n```\n";

    /// TC-004-39 steps 1 and 2: the note's statistics, then with "well-known
    /// fact" selected (chars, as the tab keeps the selection).
    #[test]
    fn tc004_39_statistics_of_the_note_and_the_selection() {
        let mut tab = EditorTab::new(COUNT_MD, NoteFormat::Markdown);
        let stats = tab.text_statistics();
        assert_eq!(
            (stats.words, stats.characters, stats.reading_minutes),
            (8, 38, 1)
        );
        assert_eq!(stats.selection, None);

        let from = COUNT_MD.find("well-known").unwrap();
        let to = COUNT_MD.find("](").unwrap();
        tab.editor.set_selection(to, from);
        let stats = tab.text_statistics();
        assert_eq!(
            stats.selection,
            Some(SelectionStatisticsView {
                words: 2,
                characters: 15
            })
        );
        assert_eq!(stats.words, 8);
    }

    /// UC-004 BR-020: the statistics follow every change of the text; a
    /// selection of multibyte text is counted by its characters.
    #[test]
    fn uc004_br020_statistics_follow_the_text() {
        let mut tab = EditorTab::new("Grüße aus Zürich\n", NoteFormat::AsciiDoc);
        assert_eq!(tab.text_statistics().words, 3);
        tab.reload("Grüße aus Zürich und Genf\n");
        let stats = tab.text_statistics();
        assert_eq!((stats.words, stats.characters), (5, 25));
        // "Zürich" in chars.
        tab.editor.set_selection(16, 10);
        assert_eq!(
            tab.text_statistics().selection,
            Some(SelectionStatisticsView {
                words: 1,
                characters: 6
            })
        );
    }

    /// The JSON the window reads.
    #[test]
    fn uc004_br020_statistics_as_the_window_reads_them() {
        let mut tab = EditorTab::new("", NoteFormat::Markdown);
        assert_eq!(
            serde_json::to_value(tab.text_statistics()).unwrap(),
            serde_json::json!({
                "words": 0, "characters": 0, "readingMinutes": 0, "selection": null
            })
        );
    }
}
