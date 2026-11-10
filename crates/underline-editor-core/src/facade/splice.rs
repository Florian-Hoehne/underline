//! ADR-024: what of the facade's blocks changed for the window: the
//! blocks before and after a change stay, those after it move by one shift.

/// ADR-024, ADR-025: the view of one block as the window has it. The views
/// themselves (runs, colours, figures) are made by the backend; the facade
/// rules only need to tell where a view starts and to move it.
pub trait ShownBlock: Clone + PartialEq {
    /// Where the view starts in the window's text, in web units.
    fn start(&self) -> u32;
    /// The line of the note the view starts on.
    fn line(&self) -> u32;
    /// Whether `self` is `old` moved by `shift` web units and `line_shift`
    /// lines and otherwise the same.
    fn is_moved(&self, old: &Self, shift: i64, line_shift: i64) -> bool;
    /// Moves the view by `shift` web units and `line_shift` lines.
    fn shift(&mut self, shift: i64, line_shift: i64);
    /// Whether the view is a figure or a table with a caption, which numbers
    /// those after it.
    fn captioned(&self) -> bool;
}

/// One change of the window's blocks: `removed` blocks from `from` make way
/// for `blocks`, and the blocks after them move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Splice<V> {
    /// The first block replaced.
    pub from: usize,
    /// How many blocks are replaced.
    pub removed: usize,
    /// The blocks in their place.
    pub blocks: Vec<V>,
    /// How far the blocks after them move, in web units.
    pub shift: i64,
    /// How many lines the blocks after them move.
    pub line_shift: i64,
}

/// The changes from `old` to `new`: the blocks before and after a change
/// stay (those after it moved by one shift). When the blocks only changed
/// in place, as when a diagram became ready, each changed run of blocks is
/// a change of its own, so the window is sent no block in between.
pub fn splices<V: ShownBlock>(old: &[V], new: &[V]) -> Vec<Splice<V>> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    if prefix == old.len() && prefix == new.len() {
        return Vec::new();
    }
    let (shift, line_shift) = match (old.last(), new.last()) {
        (Some(a), Some(b)) => (
            i64::from(b.start()) - i64::from(a.start()),
            i64::from(b.line()) - i64::from(a.line()),
        ),
        _ => (0, 0),
    };
    let most = old.len().min(new.len()) - prefix;
    let suffix = (0..most)
        .take_while(|k| new[new.len() - 1 - k].is_moved(&old[old.len() - 1 - k], shift, line_shift))
        .count();
    let old_middle = prefix..old.len() - suffix;
    let new_middle = prefix..new.len() - suffix;
    if old_middle.len() == new_middle.len() && shift == 0 && line_shift == 0 {
        let mut out: Vec<Splice<V>> = Vec::new();
        for index in old_middle {
            if old[index] == new[index] {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.from + last.removed == index => {
                    last.removed += 1;
                    last.blocks.push(new[index].clone());
                }
                _ => out.push(Splice {
                    from: index,
                    removed: 1,
                    blocks: vec![new[index].clone()],
                    shift: 0,
                    line_shift: 0,
                }),
            }
        }
        return out;
    }
    vec![Splice {
        from: prefix,
        removed: old_middle.len(),
        blocks: new[new_middle].to_vec(),
        shift,
        line_shift,
    }]
}
