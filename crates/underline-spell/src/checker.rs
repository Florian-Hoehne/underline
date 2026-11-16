//! ADR-030: what checks a word and suggests corrections. The trait keeps
//! the checker exchangeable (`hunspell-rs` is the fallback should
//! `spellbook` fail us); the spell check uses nothing else of it.

use std::collections::HashSet;

/// Checks words of one language and suggests corrections (FR-099).
pub trait SpellChecker: Send + Sync {
    /// Whether the dictionary knows `word`, as written (a capital at the
    /// start of a sentence counts as known when the word is known).
    fn check(&self, word: &str) -> bool;

    /// UC-005 A28: corrections for `word`, the likeliest first; `out` is
    /// cleared first.
    fn suggest(&self, word: &str, out: &mut Vec<String>);
}

/// A Hunspell dictionary read by `spellbook` (ADR-030).
pub struct Hunspell(spellbook::Dictionary);

impl Hunspell {
    /// The dictionary of the affix file `aff` and the word list `dic`, as
    /// text.
    ///
    /// # Errors
    ///
    /// The files are no Hunspell dictionary `spellbook` reads; the reason.
    pub fn new(aff: &str, dic: &str) -> Result<Self, String> {
        spellbook::Dictionary::new(aff, dic)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

impl std::fmt::Debug for Hunspell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hunspell")
    }
}

impl SpellChecker for Hunspell {
    fn check(&self, word: &str) -> bool {
        self.0.check(word)
    }

    fn suggest(&self, word: &str, out: &mut Vec<String>) {
        self.0.suggest(word, out);
    }
}

/// A plain list of words: a word is known when it is in the list, or when
/// it starts with a capital and the list holds it with a small letter.
/// Suggestions are the words one edit away. For tests and the window's
/// mock, not for real languages.
#[derive(Debug, Clone, Default)]
pub struct WordList {
    words: HashSet<String>,
}

impl WordList {
    /// The list of `words`.
    #[must_use]
    pub fn new<'a>(words: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            words: words.into_iter().map(str::to_owned).collect(),
        }
    }
}

/// `word` with its first character in lower case.
pub(crate) fn decapitalized(word: &str) -> Option<String> {
    let mut chars = word.chars();
    let first = chars.next()?;
    if !first.is_uppercase() {
        return None;
    }
    Some(first.to_lowercase().chain(chars).collect())
}

/// How many single edits (insert, delete, replace, swap of neighbours) turn
/// `a` into `b`, counting at most to 2.
fn edits(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut before: Vec<usize> = previous.clone();
    for i in 1..=a.len() {
        let mut row = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            row[j] = (previous[j] + 1)
                .min(row[j - 1] + 1)
                .min(previous[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                row[j] = row[j].min(before[j - 2] + 1);
            }
        }
        before = std::mem::replace(&mut previous, row);
    }
    previous[b.len()].min(2)
}

impl SpellChecker for WordList {
    fn check(&self, word: &str) -> bool {
        self.words.contains(word)
            || decapitalized(word).is_some_and(|lower| self.words.contains(&lower))
    }

    fn suggest(&self, word: &str, out: &mut Vec<String>) {
        out.clear();
        let lower = decapitalized(word).unwrap_or_else(|| word.to_owned());
        let mut near: Vec<&String> = self
            .words
            .iter()
            .filter(|known| edits(&lower, known) == 1)
            .collect();
        near.sort();
        out.extend(near.into_iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_list_knows_its_words_and_capitals_at_the_start() {
        let list = WordList::new(["ferry", "leaves"]);
        assert!(list.check("ferry"));
        assert!(list.check("Ferry"));
        assert!(!list.check("ferri"));
        assert!(!list.check("FERRy"));
    }

    #[test]
    fn a_word_list_suggests_the_words_one_edit_away() {
        let list = WordList::new(["ferry", "fern", "berry", "leaves"]);
        let mut out = vec!["stale".to_owned()];
        list.suggest("ferri", &mut out);
        assert_eq!(out, ["ferry"]);
        list.suggest("fenr", &mut out);
        assert_eq!(out, ["fern"]);
    }
}
