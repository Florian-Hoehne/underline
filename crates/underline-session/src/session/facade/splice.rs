//! ADR-024: what of the facade's blocks changed for the window: the
//! blocks before and after a change stay, those after it move by one shift.

use crate::facade_view::BlockView;

/// The same block moved by `shift` web units and `line_shift` lines.
fn moved(old: &BlockView, new: &BlockView, shift: i64, line_shift: i64) -> bool {
    let BlockView {
        kind,
        start,
        line,
        length,
        lines,
        prefix,
        runs,
        colors,
        figure,
    } = old;
    i64::from(*start) + shift == i64::from(new.start)
        && i64::from(*line) + line_shift == i64::from(new.line)
        && *kind == new.kind
        && *length == new.length
        && *lines == new.lines
        && *prefix == new.prefix
        && *runs == new.runs
        && *colors == new.colors
        && *figure == new.figure
}

/// One change of the window's blocks: `removed` blocks from `from` make way
/// for `blocks`, and the blocks after them move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Splice {
    pub(super) from: usize,
    pub(super) removed: usize,
    pub(super) blocks: Vec<BlockView>,
    pub(super) shift: i64,
    pub(super) line_shift: i64,
}

/// The changes from `old` to `new`: the blocks before and after a change
/// stay (those after it moved by one shift). When the blocks only changed
/// in place, as when a diagram became ready, each changed run of blocks is
/// a change of its own, so the window is sent no block in between.
pub(super) fn splices(old: &[BlockView], new: &[BlockView]) -> Vec<Splice> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    if prefix == old.len() && prefix == new.len() {
        return Vec::new();
    }
    let (shift, line_shift) = match (old.last(), new.last()) {
        (Some(a), Some(b)) => (
            i64::from(b.start) - i64::from(a.start),
            i64::from(b.line) - i64::from(a.line),
        ),
        _ => (0, 0),
    };
    let most = old.len().min(new.len()) - prefix;
    let suffix = (0..most)
        .take_while(|k| {
            moved(
                &old[old.len() - 1 - k],
                &new[new.len() - 1 - k],
                shift,
                line_shift,
            )
        })
        .count();
    let old_middle = prefix..old.len() - suffix;
    let new_middle = prefix..new.len() - suffix;
    if old_middle.len() == new_middle.len() && shift == 0 && line_shift == 0 {
        let mut out: Vec<Splice> = Vec::new();
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
