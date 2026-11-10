//! ADR-025: a change of a text as one set over its base text, in chars of
//! the rope: kept, deleted and inserted stretches in order (the form
//! CodeMirror's `ChangeSet` and ot.js's operations have). Sets compose, and
//! two sets made on the same text transform over each other, so a single
//! server can order the edits of the webview and its own
//! (`docs/migration/variante-b.md`, section 3.5).

use ropey::Rope;

/// One edit in chars of the rope: at `at`, `remove` chars are replaced by
/// `insert`. Edits of a list apply one after the other, each on the text the
/// one before it left.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    /// Where the edit starts, in chars.
    pub at: usize,
    /// How many chars it removes.
    pub remove: usize,
    /// What it puts in their place.
    pub insert: String,
}

impl From<&crate::Edit> for Change {
    /// An edit of the document as the sync sends it.
    fn from(edit: &crate::Edit) -> Self {
        Self {
            at: edit.at,
            remove: edit.removed.chars().count(),
            insert: edit.inserted.clone(),
        }
    }
}

/// An edit reaches past the end of the text it is made on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfRange {
    /// Where the edit ends, in chars.
    pub end: usize,
    /// How long the text is, in chars.
    pub len: usize,
}

impl std::fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "an edit ends at char {} of a text of {} chars",
            self.end, self.len
        )
    }
}

impl std::error::Error for OutOfRange {}

/// One stretch of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// Keeps chars.
    Retain(usize),
    /// Inserts text.
    Insert(String),
    /// Deletes chars.
    Delete(usize),
}

/// The length of `text` in chars.
fn chars(text: &str) -> usize {
    text.chars().count()
}

/// `text` split after `count` chars.
fn split(text: &str, count: usize) -> (&str, &str) {
    let byte = text
        .char_indices()
        .nth(count)
        .map_or(text.len(), |(b, _)| b);
    text.split_at(byte)
}

/// A change of a whole text: what it keeps, deletes and inserts, from its
/// start to its end. Inserts stand before deletes at the same place, so
/// each set has one form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    ops: Vec<Op>,
    /// The length of the text it applies to, in chars.
    base: usize,
    /// The length of the text it gives.
    target: usize,
}

impl ChangeSet {
    /// The set that keeps a text of `len` chars as it is.
    #[must_use]
    pub fn identity(len: usize) -> Self {
        let mut set = Self::default();
        set.retain(len);
        set
    }

    /// The length of the text the set applies to, in chars.
    #[must_use]
    pub fn base_len(&self) -> usize {
        self.base
    }

    /// The length of the text the set gives, in chars.
    #[must_use]
    pub fn target_len(&self) -> usize {
        self.target
    }

    /// Whether the set changes nothing.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.ops.iter().all(|op| matches!(op, Op::Retain(_)))
    }

    fn retain(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        self.base += n;
        self.target += n;
        if let Some(Op::Retain(last)) = self.ops.last_mut() {
            *last += n;
        } else {
            self.ops.push(Op::Retain(n));
        }
    }

    fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.target += chars(text);
        // An insert goes before a delete at the same place.
        let at = match self.ops.last() {
            Some(Op::Delete(_)) => self.ops.len() - 1,
            _ => self.ops.len(),
        };
        if let Some(Op::Insert(before)) = at.checked_sub(1).and_then(|i| self.ops.get_mut(i)) {
            before.push_str(text);
        } else {
            self.ops.insert(at, Op::Insert(text.to_owned()));
        }
    }

    fn delete(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        self.base += n;
        if let Some(Op::Delete(last)) = self.ops.last_mut() {
            *last += n;
        } else {
            self.ops.push(Op::Delete(n));
        }
    }

    /// The set of `change` on a text of `len` chars.
    ///
    /// # Errors
    ///
    /// The change reaches past the end of the text.
    pub fn of(len: usize, change: &Change) -> Result<Self, OutOfRange> {
        let end = change.at.saturating_add(change.remove);
        if end > len {
            return Err(OutOfRange { end, len });
        }
        let mut set = Self::default();
        set.retain(change.at);
        set.insert(&change.insert);
        set.delete(change.remove);
        set.retain(len - end);
        Ok(set)
    }

    /// The set of `changes`, one after the other, on a text of `len` chars.
    ///
    /// # Errors
    ///
    /// A change reaches past the end of the text it applies to.
    pub fn of_changes(len: usize, changes: &[Change]) -> Result<Self, OutOfRange> {
        let mut set = Self::identity(len);
        for change in changes {
            let next = Self::of(set.target, change)?;
            set = set.compose(&next);
        }
        Ok(set)
    }

    /// The set of `replacements` of a text of `len` chars: the chars of each
    /// range (in order, apart) replaced by its text.
    ///
    /// # Errors
    ///
    /// A range reaches past the end, runs backwards, or overlaps the one
    /// before it.
    pub fn of_replacements(
        len: usize,
        replacements: &[(std::ops::Range<usize>, String)],
    ) -> Result<Self, OutOfRange> {
        let mut set = Self::default();
        let mut at = 0;
        for (range, text) in replacements {
            if range.start < at || range.end < range.start || range.end > len {
                return Err(OutOfRange {
                    end: range.end,
                    len,
                });
            }
            set.retain(range.start - at);
            set.insert(text);
            set.delete(range.end - range.start);
            at = range.end;
        }
        set.retain(len - at);
        Ok(set)
    }

    /// The replacements of the set in the base text, in order and apart:
    /// `(start, end, text)` in chars of the base text.
    #[must_use]
    pub fn replacements(&self) -> Vec<(usize, usize, String)> {
        let mut out: Vec<(usize, usize, String)> = Vec::new();
        let mut at = 0;
        let mut open = false;
        for op in &self.ops {
            match op {
                Op::Retain(n) => {
                    at += n;
                    open = false;
                }
                Op::Insert(text) => {
                    if !open {
                        out.push((at, at, String::new()));
                        open = true;
                    }
                    if let Some(last) = out.last_mut() {
                        last.2.push_str(text);
                    }
                }
                Op::Delete(n) => {
                    if !open {
                        out.push((at, at, String::new()));
                        open = true;
                    }
                    at += n;
                    if let Some(last) = out.last_mut() {
                        last.1 = at;
                    }
                }
            }
        }
        out
    }

    /// The set as edits that apply one after the other, from the start of
    /// the text to its end.
    #[must_use]
    pub fn changes(&self) -> Vec<Change> {
        let mut shift: isize = 0;
        self.replacements()
            .into_iter()
            .map(|(start, end, insert)| {
                let at = start.checked_add_signed(shift).unwrap_or(start);
                let inserted = chars(&insert);
                shift += isize::try_from(inserted).unwrap_or(isize::MAX)
                    - isize::try_from(end - start).unwrap_or(isize::MAX);
                Change {
                    at,
                    remove: end - start,
                    insert,
                }
            })
            .collect()
    }

    /// Applies the set to `rope`, whose length must be the set's base.
    ///
    /// # Errors
    ///
    /// The rope is not as long as the set's base text.
    pub fn apply(&self, rope: &mut Rope) -> Result<(), OutOfRange> {
        let len = rope.len_chars();
        if len != self.base {
            return Err(OutOfRange {
                end: self.base,
                len,
            });
        }
        let mut at = 0;
        for op in &self.ops {
            match op {
                Op::Retain(n) => at += n,
                Op::Insert(text) => {
                    rope.insert(at, text);
                    at += chars(text);
                }
                Op::Delete(n) => rope.remove(at..at + n),
            }
        }
        Ok(())
    }

    /// The set that does `self`, then `next` (whose base is `self`'s
    /// target).
    ///
    /// # Panics
    ///
    /// Never for sets of fitting lengths; with lengths that do not fit, the
    /// rest of the longer set is kept as it is.
    #[must_use]
    pub fn compose(&self, next: &Self) -> Self {
        let mut out = Self::default();
        let mut first = Ops::new(&self.ops);
        let mut second = Ops::new(&next.ops);
        loop {
            match (first.peek(), second.peek()) {
                (None, None) => break,
                (Some(Op::Delete(n)), _) => {
                    out.delete(n);
                    first.take(n);
                }
                (_, Some(Op::Insert(text))) => {
                    out.insert(&text);
                    second.take(chars(&text));
                }
                (Some(Op::Retain(a)), Some(Op::Retain(b))) => {
                    let n = a.min(b);
                    out.retain(n);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Insert(text)), Some(Op::Delete(d))) => {
                    let n = chars(&text).min(d);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Insert(text)), Some(Op::Retain(r))) => {
                    let n = chars(&text).min(r);
                    out.insert(split(&text, n).0);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Retain(r)), Some(Op::Delete(d))) => {
                    let n = r.min(d);
                    out.delete(n);
                    first.take(n);
                    second.take(n);
                }
                // Lengths that do not fit: the rest stays as it is.
                (Some(op), None) => {
                    out.push(&op);
                    first.next();
                }
                (None, Some(op)) => {
                    out.push(&op);
                    second.next();
                }
            }
        }
        out
    }

    fn push(&mut self, op: &Op) {
        match op {
            Op::Retain(n) => self.retain(*n),
            Op::Insert(text) => self.insert(text),
            Op::Delete(n) => self.delete(*n),
        }
    }

    /// `a` and `b`, made on the same text, each moved over the other:
    /// `(a', b')` where `a'` applies after `b` and `b'` after `a`, and both
    /// orders give the same text. At the same place, what `a` inserts comes
    /// first; text one side inserts is never deleted by the other.
    #[must_use]
    pub fn transform(a: &Self, b: &Self) -> (Self, Self) {
        let mut a_out = Self::default();
        let mut b_out = Self::default();
        let mut first = Ops::new(&a.ops);
        let mut second = Ops::new(&b.ops);
        loop {
            match (first.peek(), second.peek()) {
                (None, None) => break,
                (Some(Op::Insert(text)), _) => {
                    a_out.insert(&text);
                    b_out.retain(chars(&text));
                    first.next();
                }
                (_, Some(Op::Insert(text))) => {
                    a_out.retain(chars(&text));
                    b_out.insert(&text);
                    second.next();
                }
                (Some(Op::Retain(x)), Some(Op::Retain(y))) => {
                    let n = x.min(y);
                    a_out.retain(n);
                    b_out.retain(n);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Delete(x)), Some(Op::Delete(y))) => {
                    let n = x.min(y);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Delete(x)), Some(Op::Retain(y))) => {
                    let n = x.min(y);
                    a_out.delete(n);
                    first.take(n);
                    second.take(n);
                }
                (Some(Op::Retain(x)), Some(Op::Delete(y))) => {
                    let n = x.min(y);
                    b_out.delete(n);
                    first.take(n);
                    second.take(n);
                }
                // Lengths that do not fit: the rest stays as it is.
                (Some(op), None) => {
                    a_out.push(&op);
                    first.next();
                }
                (None, Some(op)) => {
                    b_out.push(&op);
                    second.next();
                }
            }
        }
        (a_out, b_out)
    }
}

/// Walks the ops of a set, taking parts of them.
struct Ops<'a> {
    ops: &'a [Op],
    index: usize,
    /// What is left of the op at `index`.
    rest: Option<Op>,
}

impl<'a> Ops<'a> {
    fn new(ops: &'a [Op]) -> Self {
        Self {
            ops,
            index: 0,
            rest: ops.first().cloned(),
        }
    }

    fn peek(&self) -> Option<Op> {
        self.rest.clone()
    }

    fn next(&mut self) {
        self.index += 1;
        self.rest = self.ops.get(self.index).cloned();
    }

    /// Takes `n` chars of the current op.
    fn take(&mut self, n: usize) {
        let left = match &self.rest {
            Some(Op::Retain(m)) if *m > n => Some(Op::Retain(m - n)),
            Some(Op::Delete(m)) if *m > n => Some(Op::Delete(m - n)),
            Some(Op::Insert(text)) if chars(text) > n => {
                Some(Op::Insert(split(text, n).1.to_owned()))
            }
            _ => None,
        };
        match left {
            Some(op) => self.rest = Some(op),
            None => self.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use ropey::Rope;

    use super::*;

    /// A small xorshift generator: the sets are the same each run.
    struct Random(u64);

    impl Random {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            usize::try_from(self.0 % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
        }

        fn text(&mut self) -> String {
            const PIECES: [&str; 6] = ["a", "😀", "\r\n", "\n", "ß", ""];
            (0..self.below(3))
                .map(|_| PIECES[self.below(PIECES.len())])
                .collect()
        }

        /// A random set of up to three edits on a text of `len` chars.
        fn set(&mut self, len: usize) -> ChangeSet {
            let mut set = ChangeSet::identity(len);
            for _ in 0..self.below(4) {
                let now = set.target_len();
                let at = self.below(now + 1);
                let remove = self.below(now - at + 1).min(3);
                let insert = self.text();
                let next = ChangeSet::of(now, &Change { at, remove, insert }).expect("fits");
                set = set.compose(&next);
            }
            set
        }

        fn base(&mut self) -> String {
            (0..self.below(12)).map(|_| self.text()).collect()
        }
    }

    fn applied(text: &str, sets: &[&ChangeSet]) -> String {
        let mut rope = Rope::from_str(text);
        for set in sets {
            set.apply(&mut rope).expect("fits");
        }
        rope.to_string()
    }

    /// ADR-025 (TP1): two sets on one text, each moved over the other, give
    /// the same text in both orders; the moved sets fit.
    #[test]
    fn adr025_transform_converges_for_random_sets() {
        let mut random = Random(0x5EED_1234_ABCD_0001);
        for _ in 0..20_000 {
            let text = random.base();
            let len = text.chars().count();
            let (a, b) = (random.set(len), random.set(len));
            let (a_moved, b_moved) = ChangeSet::transform(&a, &b);
            assert_eq!(a_moved.base_len(), b.target_len());
            assert_eq!(b_moved.base_len(), a.target_len());
            assert_eq!(
                applied(&text, &[&a, &b_moved]),
                applied(&text, &[&b, &a_moved]),
                "{text:?} {a:?} {b:?}"
            );
        }
    }

    /// ADR-025: a composed set does what its parts do one after the other,
    /// and its edits rebuild it.
    #[test]
    fn adr025_compose_and_changes_for_random_sets() {
        let mut random = Random(0x5EED_1234_ABCD_0002);
        for _ in 0..20_000 {
            let text = random.base();
            let a = random.set(text.chars().count());
            let b = random.set(a.target_len());
            let both = a.compose(&b);
            assert_eq!(applied(&text, &[&both]), applied(&text, &[&a, &b]));
            let rebuilt = ChangeSet::of_changes(both.base_len(), &both.changes()).expect("fits");
            assert_eq!(applied(&text, &[&rebuilt]), applied(&text, &[&both]));
        }
    }
}
