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
        self.undo.push(edit);
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

#[cfg(test)]
mod tests {
    use super::*;

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
