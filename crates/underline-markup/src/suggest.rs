//! FR-076, FR-077: which suggestions match what the user typed, and in which
//! order: the notes of go to note (UC-004 A12, BR-014) and the commands of
//! the command palette (UC-023 BR-002). Both follow one rule, so it lives
//! here once; nothing here reads files, so the window's editor core ranks
//! with it too (ADR-025).
//!
//! A suggestion has a name (the note's title, the command's name) and a
//! context (the note's folder, the command's menu). It matches when every
//! word typed occurs in the name or the context, in any order, also as a
//! part of a word; upper and lower case do not matter, accents do, as in the
//! search of the workspace (UC-008 BR-002). The matches come in three
//! groups: the name starts with what was typed, then a word of the name
//! starts with it, then the others; within a group they keep the order the
//! suggestions were given in (the note overview's for notes, the most
//! recently run first for commands). Suggestions marked as last (commands
//! not available now, UC-023 BR-002) follow all the others.
//!
//! "What was typed" is read in the words of the search (`Query::parse`):
//! words at spaces, words in double quotes kept together, in lower case,
//! joined by one space (OQ-242). So `"plane tickets"` and `plane  tickets`
//! both put "Plane Tickets" in the first group, and a lone `"` counts for
//! nothing.

use crate::search::{lower, split_words, Query};

/// One suggestion in lower case, as it is matched.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Lowered {
    name: String,
    context: String,
}

/// The suggestions to choose from, in their given order, lowered once so
/// each key only compares (NFR-018).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestions {
    items: Vec<Lowered>,
}

/// How well a name matches what was typed (BR-014, UC-023 BR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    /// The name starts with what was typed.
    Starts,
    /// A word of the name starts with it.
    WordStarts,
    /// The words typed occur elsewhere.
    Other,
}

/// What a name has to start with: the words of `text` as the search reads
/// them (quotes keep words together), in lower case, joined by one space.
fn typed_prefix(text: &str) -> String {
    split_words(text)
        .iter()
        .map(|word| lower(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a word of `name` other than its first starts with `prefix`; a
/// word starts after a character that is no letter or digit. Every word
/// start is tried, so a start inside an earlier, overlapping occurrence of
/// `prefix` counts too (`"xab ab ab"` with `"ab ab"`).
fn word_starts_with(name: &str, prefix: &str) -> bool {
    let mut before: Option<char> = None;
    for (at, c) in name.char_indices() {
        if before.is_some_and(|b| !b.is_alphanumeric()) && name[at..].starts_with(prefix) {
            return true;
        }
        before = Some(c);
    }
    false
}

impl Suggestions {
    /// The suggestions `items`, each as its name and its context (a note's
    /// title and folder, `""` for the top folder; a command's name and
    /// menu), in the order they are listed with an empty field.
    #[must_use]
    pub fn new<'a>(items: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            items: items
                .into_iter()
                .map(|(name, context)| Lowered {
                    name: lower(name),
                    context: lower(context),
                })
                .collect(),
        }
    }

    /// How many suggestions there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The suggestions that match `typed`, best first, as their indices in
    /// the given order; `last` says which ones follow all others (a
    /// command not available now). With nothing typed, every suggestion in
    /// the given order.
    ///
    /// # Examples
    ///
    /// ```
    /// use underline_markup::suggest::Suggestions;
    ///
    /// let notes = Suggestions::new([
    ///     ("Release Plan", ""),
    ///     ("Explanation", ""),
    ///     ("Plane Tickets", "travel"),
    /// ]);
    /// assert_eq!(notes.rank("plan", |_| false), [2, 0, 1]);
    /// assert_eq!(notes.rank("travel tick", |_| false), [2]);
    /// ```
    #[must_use]
    pub fn rank(&self, typed: &str, last: impl Fn(usize) -> bool) -> Vec<usize> {
        let query = Query::parse(typed);
        let prefix = typed_prefix(typed);
        let mut found: Vec<(bool, Group, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| query.matches_lowered(&[item.name.as_str(), item.context.as_str()]))
            .map(|(at, item)| {
                let group = if item.name.starts_with(&prefix) {
                    Group::Starts
                } else if word_starts_with(&item.name, &prefix) {
                    Group::WordStarts
                } else {
                    Group::Other
                };
                (last(at), group, at)
            })
            .collect();
        found.sort_unstable();
        found.into_iter().map(|(_, _, at)| at).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// TC-004-26: the notes of its preconditions, in the order of the note
    /// overview: the recently opened first ("Release Plan" today, "Plan"
    /// yesterday), then the others by title.
    fn tc_004_26() -> Suggestions {
        Suggestions::new([
            ("Release Plan", ""),
            ("Plan", ""),
            ("Explanation", ""),
            ("Plane Tickets", "travel"),
            ("Über uns", ""),
        ])
    }

    fn rank(suggestions: &Suggestions, typed: &str) -> Vec<usize> {
        suggestions.rank(typed, |_| false)
    }

    #[test]
    fn tc_004_26_title_start_then_word_start_then_the_rest_in_overview_order() {
        // "Plan" and "Plane Tickets" start with it, the recently opened
        // first; then "Release Plan"; then "Explanation".
        assert_eq!(rank(&tc_004_26(), "plan"), [1, 3, 0, 2]);
    }

    #[test]
    fn tc_004_26_words_match_title_and_folder_in_any_order() {
        assert_eq!(rank(&tc_004_26(), "travel tick"), [3]);
        assert_eq!(rank(&tc_004_26(), "tick travel"), [3]);
        assert_eq!(rank(&tc_004_26(), "TRAVEL"), [3]);
    }

    #[test]
    fn tc_004_26_case_does_not_matter_accents_do() {
        assert!(rank(&tc_004_26(), "uber").is_empty());
        assert_eq!(rank(&tc_004_26(), "ÜBER"), [4]);
    }

    #[test]
    fn tc_004_26_an_empty_field_lists_all_in_the_given_order() {
        assert_eq!(rank(&tc_004_26(), ""), [0, 1, 2, 3, 4]);
        assert_eq!(rank(&tc_004_26(), "   "), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn tc_004_27_nothing_matches() {
        assert!(rank(&tc_004_26(), "Budget 2027").is_empty());
    }

    #[test]
    fn uc_004_br_014_a_part_of_a_word_matches() {
        // No name starts with it, nor a word: the overview's order.
        assert_eq!(rank(&tc_004_26(), "lan"), [0, 1, 2, 3]);
    }

    #[test]
    fn uc_004_br_014_runs_of_spaces_count_as_one_for_the_start() {
        assert_eq!(rank(&tc_004_26(), "  plane   tick "), [3]);
        // The name starts with it, so it comes before a word start.
        let s = Suggestions::new([("New plane", ""), ("Plane ticket", "")]);
        assert_eq!(rank(&s, "plane  ti"), [1]);
        assert_eq!(rank(&s, "plane"), [1, 0]);
    }

    #[test]
    fn tc_023_02_commands_match_name_and_menu_in_any_order_and_case() {
        let commands = Suggestions::new([
            ("New note…", "File"),
            ("Open workspace…", "File"),
            ("Theme…", "Theme"),
            ("PDF…", "Note › Export"),
        ]);
        assert_eq!(rank(&commands, "NOTE NEW"), [0]);
        assert_eq!(rank(&commands, "export pdf"), [3]);
        assert_eq!(rank(&commands, "file"), [0, 1]);
    }

    #[test]
    fn tc_023_02_a_name_that_starts_with_it_comes_before_one_that_holds_it() {
        let commands = Suggestions::new([("Choose a theme", "View"), ("Theme…", "Theme")]);
        assert_eq!(rank(&commands, "theme"), [1, 0]);
    }

    #[test]
    fn tc_023_05_commands_not_available_follow_the_others() {
        let commands = Suggestions::new([
            ("Commit", "Git"),
            ("Show changes", "Git"),
            ("Sync now", "Git"),
        ]);
        assert_eq!(commands.rank("git", |at| at == 0), [1, 2, 0]);
        // Also before a better group.
        assert_eq!(commands.rank("commit", |at| at == 0), [0]);
        assert_eq!(commands.rank("", |at| at == 1), [0, 2, 1]);
    }

    #[test]
    fn tc_023_04_nothing_matches_xyzzy() {
        let commands = Suggestions::new([("New note…", "File")]);
        assert!(rank(&commands, "xyzzy").is_empty());
    }

    #[test]
    fn a_word_start_needs_a_separator_before_it() {
        assert!(word_starts_with("release plan", "plan"));
        assert!(word_starts_with("notes/plan", "plan"));
        assert!(!word_starts_with("explanation", "plan"));
        // The start of the name is no other word.
        assert!(!word_starts_with("plan", "plan"));
        assert!(word_starts_with("plan plan", "plan"));
    }

    #[test]
    fn a_word_start_inside_an_overlapping_occurrence_counts() {
        // `match_indices` would see only the first "ab ab" (at 1, no word start).
        assert!(word_starts_with("xab ab ab", "ab ab"));
        assert!(!word_starts_with("xab", "ab"));
        assert!(!word_starts_with("", "ab"));
    }

    #[test]
    fn oq_242_quoted_words_are_one_word_of_the_start() {
        let s = Suggestions::new([("New plane tickets", ""), ("Plane tickets", "")]);
        assert_eq!(rank(&s, "\"plane tickets\""), [1, 0]);
        // An open quote runs to the end, spaces in it as typed.
        assert_eq!(rank(&s, "\"plane tickets"), [1, 0]);
        assert!(rank(&s, "\"plane  tickets").is_empty());
        // A lone quote counts for nothing.
        assert_eq!(rank(&s, "\""), [0, 1]);
    }

    #[test]
    fn unicode_case_is_folded_by_lower_and_accents_stay() {
        let s = Suggestions::new([
            ("Straße", ""),
            ("İstanbul", ""),
            ("ΣΟΦΙΑ", ""),
            ("Cafe\u{301}", ""),
        ]);
        assert_eq!(rank(&s, "STRASSE"), Vec::<usize>::new());
        assert_eq!(rank(&s, "straße"), [0]);
        assert_eq!(rank(&s, "i\u{307}stanbul"), [1]);
        assert_eq!(rank(&s, "σοφια"), [2]);
        assert_eq!(rank(&s, "cafe\u{301}"), [3]);
        assert_eq!(rank(&s, "café"), Vec::<usize>::new());
    }

    #[test]
    fn equal_names_keep_their_given_order() {
        let s = Suggestions::new([("Plan", "beta"), ("Plan", "alpha"), ("Plan", "")]);
        assert_eq!(rank(&s, "plan"), [0, 1, 2]);
        assert_eq!(rank(&s, "plan alpha"), [1]);
    }

    #[test]
    fn no_suggestions_rank_to_nothing() {
        let none = Suggestions::default();
        assert!(none.is_empty());
        assert_eq!(none.len(), 0);
        assert_eq!(rank(&none, "x"), Vec::<usize>::new());
        assert_eq!(rank(&none, ""), Vec::<usize>::new());
    }

    /// NFR-018, TC-004-28, TC-023-10: the suggestions of 20 different
    /// texts follow each key within 200 ms in 10,000 notes. The 200 ms hold
    /// for an optimised build (`cargo test --release`, as the app is built);
    /// a debug build is checked against a generous bound only, so a slow
    /// machine does not fail it.
    #[test]
    fn tc_004_28_twenty_texts_in_ten_thousand_notes_within_200_ms() {
        let titles: Vec<(String, String)> = (0..10_000)
            .map(|n| {
                (
                    format!("Meeting notes {n} about project {}", n % 97),
                    format!("area {}/team {}", n % 13, n % 7),
                )
            })
            .collect();
        let started = Instant::now();
        let notes = Suggestions::new(titles.iter().map(|(t, f)| (t.as_str(), f.as_str())));
        let built = started.elapsed();
        assert_eq!(notes.len(), 10_000);
        let texts = [
            "m",
            "me",
            "mee",
            "meet",
            "meeting",
            "notes 12",
            "project 5",
            "area 3",
            "team",
            "about",
            "xyz",
            "9999",
            "team 6 area",
            "MEETING",
            "proj",
            "es 1",
            "",
            "a",
            "t 4",
            "notes",
        ];
        let mut slowest = Duration::ZERO;
        for text in texts {
            let started = Instant::now();
            let found = notes.rank(text, |_| false);
            slowest = slowest.max(started.elapsed());
            assert!(found.len() <= 10_000);
        }
        println!("NFR-018: built in {built:?}, slowest of 20 texts {slowest:?}");
        let bound = if cfg!(debug_assertions) {
            Duration::from_secs(2)
        } else {
            Duration::from_millis(200)
        };
        assert!(built < bound, "built in {built:?}");
        assert!(slowest < bound, "slowest {slowest:?}");
    }
}
