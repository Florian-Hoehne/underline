//! The text of one note as a rope, changed only through edits that can be
//! undone (ADR-012, UC-005 BR-004).

use ropey::Rope;

/// One change: at char index `at`, `removed` was replaced by `inserted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub at: usize,
    pub removed: String,
    pub inserted: String,
}

/// The markup of a note with its undo and redo history.
#[derive(Debug, Clone, Default)]
pub struct Document {
    rope: Rope,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    revision: u64,
}

impl Document {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            ..Self::default()
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Counts every change, so views can tell that the text changed.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// The text of `line` with its line ending.
    pub fn line(&self, line: usize) -> String {
        if line < self.rope.len_lines() {
            self.rope.line(line).to_string()
        } else {
            String::new()
        }
    }

    /// The text of `line` without its line ending.
    pub fn line_text(&self, line: usize) -> String {
        self.line(line).trim_end_matches(['\n', '\r']).to_owned()
    }

    /// Replaces the chars in `range` by `text`; returns the first changed
    /// line. Clears the redo history.
    pub fn replace(&mut self, range: std::ops::Range<usize>, text: &str) -> usize {
        let edit = self.apply(Edit {
            at: range.start,
            removed: self.rope.slice(range.clone()).to_string(),
            inserted: text.to_owned(),
        });
        let line = self.rope.char_to_line(edit.at.min(self.rope.len_chars()));
        match self.undo.last_mut() {
            Some(last) if merges(last, &edit) => merge(last, edit),
            _ => self.undo.push(edit),
        }
        self.redo.clear();
        line
    }

    /// Takes back the last edit; returns the char index to put the cursor at
    /// and the first changed line.
    pub fn undo(&mut self) -> Option<(usize, usize)> {
        let edit = self.undo.pop()?;
        let inverse = self.apply(Edit {
            at: edit.at,
            removed: edit.inserted.clone(),
            inserted: edit.removed.clone(),
        });
        self.redo.push(edit);
        Some((
            inverse.at + inverse.inserted.chars().count(),
            self.rope.char_to_line(inverse.at),
        ))
    }

    /// Applies the last undone edit again.
    pub fn redo(&mut self) -> Option<(usize, usize)> {
        let edit = self.redo.pop()?;
        let applied = self.apply(edit.clone());
        self.undo.push(edit);
        Some((
            applied.at + applied.inserted.chars().count(),
            self.rope.char_to_line(applied.at),
        ))
    }

    fn apply(&mut self, edit: Edit) -> Edit {
        let removed = edit.removed.chars().count();
        self.rope.remove(edit.at..edit.at + removed);
        self.rope.insert(edit.at, &edit.inserted);
        self.revision += 1;
        edit
    }
}

/// Whether `next` continues the typing or deleting of `last`, so both are
/// undone in one step (UC-005 BR-004): text typed right after it within a
/// word, or backspaces right before it, never across a line break, and a
/// space after a word starts a new step.
fn merges(last: &Edit, next: &Edit) -> bool {
    let typed = |e: &Edit| e.removed.is_empty() && !e.inserted.is_empty();
    let deleted = |e: &Edit| e.inserted.is_empty() && !e.removed.is_empty();
    if next.inserted.contains('\n') || next.removed.contains('\n') {
        return false;
    }
    if typed(last) && typed(next) {
        let follows = next.at == last.at + last.inserted.chars().count();
        let word_ends = !last.inserted.ends_with(char::is_whitespace)
            && next.inserted.starts_with(char::is_whitespace);
        return follows && !word_ends && !last.inserted.contains('\n');
    }
    if deleted(last) && deleted(next) {
        return next.at + next.removed.chars().count() == last.at && !last.removed.contains('\n');
    }
    false
}

fn merge(last: &mut Edit, next: Edit) {
    if next.removed.is_empty() {
        last.inserted.push_str(&next.inserted);
    } else {
        last.removed.insert_str(0, &next.removed);
        last.at = next.at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UC-005 BR-004: typing and deleting are undone a word at a time.
    #[test]
    fn uc005_undo_takes_back_a_word_at_a_time() {
        let mut doc = Document::new("");
        for (i, c) in "hello world".chars().enumerate() {
            doc.replace(i..i, &c.to_string());
        }
        doc.undo();
        assert_eq!(doc.text(), "hello");
        doc.undo();
        assert_eq!(doc.text(), "");
        doc.redo();
        doc.redo();
        assert_eq!(doc.text(), "hello world");
        // Backspaces in a row are one step.
        for at in (6..11).rev() {
            doc.replace(at..at + 1, "");
        }
        assert_eq!(doc.text(), "hello ");
        doc.undo();
        assert_eq!(doc.text(), "hello world");
        // A line break starts a new step.
        doc.replace(11..11, "\n");
        doc.replace(12..12, "x");
        doc.undo();
        assert_eq!(doc.text(), "hello world\n");
    }

    /// UC-005 BR-004: edits can be undone and redone.
    #[test]
    fn uc005_undo_and_redo_edits() {
        let mut doc = Document::new("alpha\nbeta\n");
        assert_eq!(doc.replace(6..10, "gamma"), 1);
        assert_eq!(doc.text(), "alpha\ngamma\n");
        doc.replace(0..0, "# ");
        assert_eq!(doc.undo(), Some((0, 0)));
        assert_eq!(doc.undo(), Some((10, 1)));
        assert_eq!(doc.text(), "alpha\nbeta\n");
        doc.redo();
        assert_eq!(doc.text(), "alpha\ngamma\n");
        doc.replace(0..0, "x");
        assert_eq!(doc.redo(), None);
    }
}
