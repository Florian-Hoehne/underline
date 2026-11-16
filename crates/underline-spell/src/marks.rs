//! UC-005 BR-023, ADR-030: the marks of a note: the bytes of the words its
//! dictionary does not know and the user did not add. A block whose markup
//! did not change since the last check keeps the words its dictionary does
//! not know, so after an edit only the changed blocks are checked; a word's
//! verdict is kept too. The user's words are applied to them at the end, so
//! adding a word keeps all of this.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use underline_markup::blocks::{parse_blocks, Block};
use underline_markup::NoteFormat;

use crate::checker::SpellChecker;
use crate::user_words::UserWords;
use crate::words::{block_words, is_checked_word, Context};

/// UC-005 A28: the most corrections offered for a word.
pub const MAX_SUGGESTIONS: usize = 5;

/// UC-005 A28: the longest word corrections are looked for, in characters;
/// the dictionary's suggestions grow steeply with the length of the word.
pub const MAX_SUGGESTED_LENGTH: usize = 40;

/// A word of a block its dictionary does not know, relative to the block.
#[derive(Debug, Clone)]
struct Unknown {
    range: Range<usize>,
    word: String,
}

/// How many verdicts are kept before they are forgotten; a note of many
/// thousand different words fits.
const MAX_VERDICTS: usize = 100_000;

/// Checks the notes of one language with the user's words, keeping the
/// verdicts of words and the marks of blocks.
pub struct NoteSpeller {
    checker: Arc<dyn SpellChecker>,
    user: Arc<UserWords>,
    verdicts: HashMap<String, bool>,
    /// The words its dictionary does not know of each block by its key,
    /// relative to its start.
    blocks: HashMap<u64, Arc<[Unknown]>>,
}

impl std::fmt::Debug for NoteSpeller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoteSpeller")
            .field("verdicts", &self.verdicts.len())
            .field("blocks", &self.blocks.len())
            .finish_non_exhaustive()
    }
}

impl NoteSpeller {
    /// A speller checking with `checker`, never marking `user`'s words.
    #[must_use]
    pub fn new(checker: Arc<dyn SpellChecker>, user: Arc<UserWords>) -> Self {
        Self {
            checker,
            user,
            verdicts: HashMap::new(),
            blocks: HashMap::new(),
        }
    }

    /// UC-005 A28: the user's words are `user` from now on; what was
    /// checked is kept.
    pub fn set_user_words(&mut self, user: Arc<UserWords>) {
        self.user = user;
    }

    /// UC-005 BR-023: whether `word` is known: one of the user's words, or
    /// known to the dictionary.
    pub fn knows(&mut self, word: &str) -> bool {
        self.user.contains(word) || self.dictionary_knows(word)
    }

    /// Whether the dictionary knows `word`, its verdict kept.
    fn dictionary_knows(&mut self, word: &str) -> bool {
        if let Some(&known) = self.verdicts.get(word) {
            return known;
        }
        let known = self.checker.check(word);
        if self.verdicts.len() >= MAX_VERDICTS {
            self.verdicts.clear();
        }
        self.verdicts.insert(word.to_owned(), known);
        known
    }

    /// UC-005 BR-023: the marks of `source`, a note in `format`, as bytes,
    /// sorted. The blocks touching `first` (the part of the note in view)
    /// are checked first; when blocks not checked before remain after them,
    /// `partial` gets the marks known by then, so they show before the rest
    /// is done. Blocks of other notes and of earlier texts are forgotten
    /// when a check leaves more of them than it used.
    pub fn check_note(
        &mut self,
        source: &str,
        format: NoteFormat,
        first: Option<Range<usize>>,
        partial: &mut dyn FnMut(&[Range<usize>]),
    ) -> Vec<Range<usize>> {
        let blocks = parse_blocks(source, format);
        let context = Context::of(source, format, &blocks);
        let keys: Vec<u64> = blocks
            .iter()
            .map(|block| block_key(source, block, &context))
            .collect();
        let mut found: Vec<Option<Arc<[Unknown]>>> = keys
            .iter()
            .map(|key| self.blocks.get(key).cloned())
            .collect();
        let touches = |block: &Block| {
            first
                .as_ref()
                .is_some_and(|f| block.span.start <= f.end && f.start <= block.span.end)
        };
        for (index, block) in blocks.iter().enumerate() {
            if found[index].is_none() && touches(block) {
                found[index] = Some(self.check_block(source, block, &context, keys[index]));
            }
        }
        if first.is_some() && found.iter().any(Option::is_none) {
            partial(&absolute(&blocks, &found, &self.user));
        }
        for (index, block) in blocks.iter().enumerate() {
            if found[index].is_none() {
                found[index] = Some(self.check_block(source, block, &context, keys[index]));
            }
        }
        if self.blocks.len() > 2 * keys.len() + 64 {
            let used: std::collections::HashSet<u64> = keys.into_iter().collect();
            self.blocks.retain(|key, _| used.contains(key));
        }
        absolute(&blocks, &found, &self.user)
    }

    fn check_block(
        &mut self,
        source: &str,
        block: &Block,
        context: &Context,
        key: u64,
    ) -> Arc<[Unknown]> {
        let start = block.span.start;
        // A word outside its block (a parser's place gone astray) is left
        // out rather than ending the check.
        let marks: Arc<[Unknown]> = block_words(source, block, context)
            .into_iter()
            .filter(|word| block.span.start <= word.range.start && word.range.end <= block.span.end)
            .filter(|word| !self.dictionary_knows(&word.text))
            .map(|word| Unknown {
                range: word.range.start - start..word.range.end - start,
                word: word.text,
            })
            .collect();
        self.blocks.insert(key, Arc::clone(&marks));
        marks
    }

    /// UC-005 A28: up to [`MAX_SUGGESTIONS`] corrections for `word` from
    /// `checker`, the likeliest first; none for a word the spell check does
    /// not look at, or one longer than [`MAX_SUGGESTED_LENGTH`].
    #[must_use]
    pub fn suggest(checker: &dyn SpellChecker, word: &str) -> Vec<String> {
        if !is_checked_word(word) || word.chars().count() > MAX_SUGGESTED_LENGTH {
            return Vec::new();
        }
        let mut out = Vec::new();
        checker.suggest(word, &mut out);
        out.retain(|suggestion| suggestion != word);
        out.dedup();
        out.truncate(MAX_SUGGESTIONS);
        out
    }
}

/// The key of a block's marks: its markup, its kind, and the bytes in it
/// that are left out, relative to its start.
fn block_key(source: &str, block: &Block, context: &Context) -> u64 {
    let mut hasher = DefaultHasher::new();
    source
        .get(block.span.clone())
        .unwrap_or_default()
        .hash(&mut hasher);
    std::mem::discriminant(&block.kind).hash(&mut hasher);
    for skip in context
        .skips
        .iter()
        .filter(|s| s.start < block.span.end && block.span.start < s.end)
    {
        (skip.start.saturating_sub(block.span.start)).hash(&mut hasher);
        (skip.end.saturating_sub(block.span.start)).hash(&mut hasher);
    }
    hasher.finish()
}

/// The marks of the blocks checked so far, as bytes of the note, sorted;
/// the user's words are not marked.
fn absolute(
    blocks: &[Block],
    found: &[Option<Arc<[Unknown]>>],
    user: &UserWords,
) -> Vec<Range<usize>> {
    let mut marks: Vec<Range<usize>> = blocks
        .iter()
        .zip(found)
        .filter_map(|(block, marks)| marks.as_ref().map(|m| (block.span.start, m)))
        .flat_map(|(start, marks)| {
            marks
                .iter()
                .filter(|m| !user.contains(&m.word))
                .map(move |m| m.range.start + start..m.range.end + start)
        })
        .collect();
    marks.sort_by_key(|m| m.start);
    marks.dedup();
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::WordList;

    fn speller(user: &[&str]) -> NoteSpeller {
        let list = WordList::new([
            "the", "ferry", "leaves", "and", "a", "word", "here", "is", "one", "two",
        ]);
        let mut words = UserWords::default();
        for word in user {
            words.add(word).unwrap();
        }
        NoteSpeller::new(Arc::new(list), Arc::new(words))
    }

    fn marked<'a>(source: &'a str, marks: &[Range<usize>]) -> Vec<&'a str> {
        marks.iter().map(|m| &source[m.clone()]).collect()
    }

    #[test]
    fn tc005_39_unknown_words_are_marked_and_the_users_are_not() {
        let source = "The ferri leaves\n\nUnderline and Kubernetes\n";
        let mut speller = speller(&["Kubernetes"]);
        let marks = speller.check_note(source, NoteFormat::Markdown, None, &mut |_| {});
        assert_eq!(marked(source, &marks), ["ferri", "Underline"]);
    }

    #[test]
    fn adr030_unchanged_blocks_keep_their_marks_and_are_not_checked_again() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Count(WordList, AtomicUsize);
        impl SpellChecker for Count {
            fn check(&self, word: &str) -> bool {
                self.1.fetch_add(1, Ordering::SeqCst);
                self.0.check(word)
            }
            fn suggest(&self, word: &str, out: &mut Vec<String>) {
                self.0.suggest(word, out);
            }
        }
        let count = Arc::new(Count(WordList::new(["one", "two"]), AtomicUsize::new(0)));
        let mut speller = NoteSpeller::new(
            Arc::clone(&count) as Arc<dyn SpellChecker>,
            Arc::new(UserWords::default()),
        );
        let before = "one twoo\n\nalpha beta\n";
        let marks = speller.check_note(before, NoteFormat::Markdown, None, &mut |_| {});
        assert_eq!(marked(before, &marks), ["twoo", "alpha", "beta"]);
        let checked = count.1.load(Ordering::SeqCst);
        // The first block changes; the second moves but keeps its markup.
        let after = "one two three\n\nalpha beta\n";
        let marks = speller.check_note(after, NoteFormat::Markdown, None, &mut |_| {});
        assert_eq!(marked(after, &marks), ["three", "alpha", "beta"]);
        // Only "two" and "three" were new to the checker.
        assert_eq!(count.1.load(Ordering::SeqCst) - checked, 2);
    }

    #[test]
    fn uc005_a28_adding_a_word_keeps_what_was_checked() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Count(WordList, AtomicUsize);
        impl SpellChecker for Count {
            fn check(&self, word: &str) -> bool {
                self.1.fetch_add(1, Ordering::SeqCst);
                self.0.check(word)
            }
            fn suggest(&self, word: &str, out: &mut Vec<String>) {
                self.0.suggest(word, out);
            }
        }
        let count = Arc::new(Count(WordList::new(["one"]), AtomicUsize::new(0)));
        let mut speller = NoteSpeller::new(
            Arc::clone(&count) as Arc<dyn SpellChecker>,
            Arc::new(UserWords::default()),
        );
        let source = "one Kubernetes\n\nzettel one\n";
        let marks = speller.check_note(source, NoteFormat::Markdown, None, &mut |_| {});
        assert_eq!(marked(source, &marks), ["Kubernetes", "zettel"]);
        let checked = count.1.load(Ordering::SeqCst);
        speller.set_user_words(Arc::new(UserWords::parse("Kubernetes\n")));
        let marks = speller.check_note(source, NoteFormat::Markdown, None, &mut |_| {});
        assert_eq!(marked(source, &marks), ["zettel"]);
        assert_eq!(
            count.1.load(Ordering::SeqCst),
            checked,
            "nothing checked again"
        );
    }

    #[test]
    fn adr030_the_part_in_view_is_checked_first() {
        let source = "first wordd\n\nsecond wordd\n\nthird wordd\n";
        let third = source.find("third").unwrap();
        let mut speller = speller(&[]);
        let mut early = Vec::new();
        let marks = speller.check_note(
            source,
            NoteFormat::Markdown,
            Some(third..third + 5),
            &mut |marks| early = marks.to_vec(),
        );
        assert_eq!(marked(source, &early), ["third", "wordd"]);
        assert_eq!(marks.len(), 6);
    }

    #[test]
    fn uc005_a28_at_most_five_suggestions_without_the_word_itself() {
        let list = WordList::new(["cat", "bat", "hat", "mat", "rat", "sat", "vat", "at"]);
        let out = NoteSpeller::suggest(&list, "xat");
        assert_eq!(out.len(), MAX_SUGGESTIONS);
        assert!(!out.contains(&"xat".to_owned()));
        assert!(NoteSpeller::suggest(&list, "XAT").is_empty());
        assert!(NoteSpeller::suggest(&list, "x4t").is_empty());
        assert!(NoteSpeller::suggest(&list, &"x".repeat(41)).is_empty());
    }
}
