//! ADR-024, ADR-025: the views of the facade after an input. The blocks the
//! input changed (its [`Dirty`](super::Dirty) range) get new views, the
//! views after them only move; the views themselves are made behind
//! [`MakeViews`], so highlighting, diagrams and images stay in the backend.

use std::collections::HashMap;
use std::ops::Range;

use ropey::Rope;
use underline_markup::blocks::Block;

use super::rules::{ceil_boundary, ext_end, floor_boundary};
use super::splice::{splices, ShownBlock, Splice};
use crate::SourceEditor;

/// The text and blocks before an input, to tell what it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Before {
    /// The revision of the text.
    pub rev: u64,
    /// The number of blocks.
    pub blocks: usize,
    /// The length of the text in web units.
    pub web: usize,
    /// The number of lines of the text.
    pub lines: usize,
}

impl Before {
    /// The text of `editor` as it is now, with `blocks` blocks.
    #[must_use]
    pub fn of(editor: &SourceEditor, blocks: usize) -> Self {
        let doc = editor.document();
        Self {
            rev: editor.revision(),
            blocks,
            web: editor.web_at_char(doc.len_chars()),
            lines: doc.len_lines(),
        }
    }
}

/// The part of the note that holds the blocks whose views are made anew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part<'a> {
    /// The markup of the part.
    pub text: &'a str,
    /// The source byte the part starts at; always on a char boundary.
    pub from: usize,
    /// The web unit the part starts at.
    pub origin: usize,
}

/// ADR-025: makes the views of the facade's blocks for the window; the
/// backend's highlighting, diagrams and images live behind it.
pub trait MakeViews {
    /// The view of one block.
    type View: ShownBlock;

    /// The views of all `blocks` of the note `text`, held in `rope`.
    fn all(&mut self, blocks: &[Block], text: &str, rope: &Rope) -> Vec<Self::View>;

    /// The views of the blocks `range` of `blocks`, which `part` of the note
    /// in `rope` holds. Figures and tables are numbered after those before.
    fn some(
        &mut self,
        blocks: &[Block],
        range: Range<usize>,
        part: Part<'_>,
        rope: &Rope,
    ) -> Vec<Self::View>;

    /// M-09.7: the blocks `replaced` (by their old indices) were made anew
    /// and those after them moved by `grown` places; what the maker keeps
    /// per block (the last pictures of diagrams) follows them, as
    /// [`moved_indices`] does.
    fn blocks_moved(&mut self, replaced: Range<usize>, grown: isize);

    /// The view `view` of block `index` was made anew in place of `old`, the
    /// old view of the block at that index, if it was replaced; the maker may
    /// carry over what it makes later (M-09.4: code colours, see
    /// [`carry_spans`]).
    fn made(&mut self, index: usize, view: &mut Self::View, old: Option<&Self::View>);
}

/// What [`refresh`] changed of the views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refreshed<V> {
    /// The changes for the window, in order.
    pub splices: Vec<Splice<V>>,
    /// The number of captioned figures and tables changed, so those after
    /// them must be numbered anew.
    pub recaptioned: bool,
}

/// `a - b` as an `i64` shift; a note never holds so many blocks or units
/// that they do not fit.
fn shift_of(a: usize, b: usize) -> i64 {
    i64::try_from(a).unwrap_or(i64::MAX) - i64::try_from(b).unwrap_or(i64::MAX)
}

/// `a - b` as the `isize` [`moved_indices`] takes.
fn difference(a: usize, b: usize) -> isize {
    isize::try_from(a).unwrap_or(isize::MAX) - isize::try_from(b).unwrap_or(isize::MAX)
}

/// ADR-024, M-09.7: `map` (by block index) after the blocks `replaced` were
/// made anew and those after them moved by `grown` places: the entries of
/// the replaced blocks go, those after them move along.
#[must_use]
pub fn moved_indices<T>(
    map: HashMap<usize, T>,
    replaced: Range<usize>,
    grown: isize,
) -> HashMap<usize, T> {
    map.into_iter()
        .filter_map(|(index, value)| {
            if index < replaced.start {
                Some((index, value))
            } else if index >= replaced.end {
                // An index that would leave `usize` belongs to no block: the
                // entry goes rather than wrapping around.
                Some((index.checked_add_signed(grown)?, value))
            } else {
                None
            }
        })
        .collect()
}

/// `count` as a web unit offset.
fn units(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// M-09.4, ADR-024: the colour spans `spans` (`[start, end, style]` in
/// UTF-16 units) of a code block whose text changed from the units `old` to
/// `new`, carried over until the backend colours it anew: the spans before
/// the change stay, those after it move with the text, and spans that
/// shrink to nothing go.
#[must_use]
pub fn carry_spans<S: Copy>(
    old: &[u16],
    new: &[u16],
    spans: &[(u32, u32, S)],
) -> Vec<(u32, u32, S)> {
    let prefix = old.iter().zip(new).take_while(|(x, y)| x == y).count();
    let most = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(most)
        .take_while(|(x, y)| x == y)
        .count();
    let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
    let moved = |at: u32| -> u32 {
        let at = at as usize;
        let moved = if at <= prefix {
            at
        } else if at >= old_end {
            at - old_end + new_end
        } else {
            new_end
        };
        units(moved)
    };
    spans
        .iter()
        .map(|&(start, end, style)| (moved(start), moved(end), style))
        .filter(|(start, end, _)| start < end)
        .collect()
}

/// How many figures and tables of `views` carry a caption, which numbers
/// those after them.
#[must_use]
pub fn captions<V: ShownBlock>(views: &[V]) -> usize {
    views.iter().filter(|view| view.captioned()).count()
}

/// ADR-024: the views `views` follow an input that changed the blocks
/// `dirty` (by their indices now) of `blocks`, the text of `editor` having
/// been `before`. Only the views of the changed blocks are made anew (by
/// `maker`), the views after them move; when the views are out of step
/// with the blocks before the input, all are made anew. Returns the changes
/// for the window.
#[must_use]
pub fn refresh<M: MakeViews>(
    views: &mut Vec<M::View>,
    blocks: &[Block],
    editor: &SourceEditor,
    maker: &mut M,
    dirty: Range<usize>,
    before: Before,
) -> Refreshed<M::View> {
    let count = blocks.len();
    let lo = dirty.start.min(count);
    let rope = editor.document().rope();
    if views.len() != before.blocks || lo > views.len() {
        // Out of step with the blocks, or a dirty range past the old views:
        // all views are made anew.
        let text = editor.text();
        let made = maker.all(blocks, &text, rope);
        let changes = splices(views, &made);
        *views = made;
        return Refreshed {
            splices: changes,
            recaptioned: false,
        };
    }
    let hi = dirty.end.min(count).max(lo);
    // The old blocks the changed ones took the place of end where as many
    // blocks are left after them as after `hi` now.
    let hi_old = hi
        .saturating_add(before.blocks)
        .saturating_sub(count)
        .clamp(lo, views.len());
    let doc = editor.document();
    let shift = shift_of(editor.web_at_char(doc.len_chars()), before.web);
    let line_shift = shift_of(doc.len_lines(), before.lines);
    if count != before.blocks {
        // M-09.7: the last pictures stay with their blocks.
        maker.blocks_moved(lo..hi_old, difference(count, before.blocks));
    }
    let mut made = Vec::new();
    if lo < hi {
        let from = floor_boundary(rope, blocks[lo].span.start);
        // One byte past the blocks (their line break, mostly), up to the
        // next char boundary: a non-ASCII char may follow directly.
        let to = blocks[lo..hi]
            .iter()
            .map(ext_end)
            .max()
            .unwrap_or(from)
            .saturating_add(1);
        let to = ceil_boundary(rope, to).max(from);
        let text = rope.byte_slice(from..to).to_string();
        let origin = editor.web_at_char(rope.byte_to_char(from));
        let part = Part {
            text: &text,
            from,
            origin,
        };
        made = maker.some(blocks, lo..hi, part, rope);
        for (k, view) in made.iter_mut().enumerate() {
            let index = lo + k;
            let old = views.get(index).filter(|_| index < hi_old);
            maker.made(index, view, old);
        }
    }
    let recaptioned = captions(&views[lo..hi_old]) != captions(&made);
    if shift != 0 || line_shift != 0 {
        for view in &mut views[hi_old..] {
            view.shift(shift, line_shift);
        }
    }
    views.splice(lo..hi_old, made.iter().cloned());
    Refreshed {
        splices: vec![Splice {
            from: lo,
            removed: hi_old - lo,
            blocks: made,
            shift,
            line_shift,
        }],
        recaptioned,
    }
}
