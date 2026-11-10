// M-09 (UC-004 step 3, ADR-021, ADR-024): what the facade works out of the
// backend's blocks, without the DOM: applying a change of the blocks
// (`ed-blocks`), the place in the markup a shown character stands for and
// back, the folded sections (M-08.8), the figures (M-09.5 to M-09.8) and the
// colours of a code block (M-09.4). Offsets are web units, as everywhere at
// the boundary (`underline_editor_core::utf16`).

import type { BlockView, BlocksView, CodeColors, RunView, SectionView } from '../bindings';
import type { Area, Mark } from '../editor/minimap';

/** The blocks of a facade at one version. */
export type Blocks = { rev: number; version: number; blocks: BlockView[] };

/**
 * ADR-024: `blocks` after `change`, or `null` when `change` is for another
 * version; then the facade asks for all blocks again.
 */
export function applyBlocks(blocks: Blocks, change: BlocksView): Blocks | null {
  if (change.base !== blocks.version) {
    return null;
  }
  const next = blocks.blocks.slice();
  spliceBlocks(next, change);
  return { rev: change.rev, version: change.version, blocks: next };
}

/**
 * Applies `change` to `blocks` in place: its blocks replace the removed
 * ones, the blocks after them move by its shift.
 */
export function spliceBlocks(blocks: BlockView[], change: BlocksView): void {
  const end = change.from + change.removed;
  if (change.shift === 0 && change.lineShift === 0) {
    blocks.splice(change.from, change.removed, ...change.blocks);
    return;
  }
  const moved = blocks.slice(end).map((block) => ({
    ...block,
    start: block.start + change.shift,
    line: block.line + change.lineShift,
  }));
  blocks.length = change.from;
  for (const block of change.blocks) blocks.push(block);
  for (const block of moved) blocks.push(block);
}

/** The UTF-16 length of `run`'s text. */
export function runLength(run: RunView): number {
  return run.text.length;
}

/** The web offset of the markup that unit `unit` of `run` in `block` shows; the run's end after its text. */
export function sourceAt(block: BlockView, run: RunView, unit: number): number {
  if (unit >= runLength(run) || run.offsets.length === 0) {
    return block.start + run.end;
  }
  let pair = run.offsets[0];
  for (const candidate of run.offsets) {
    if (candidate[0] > unit) {
      break;
    }
    pair = candidate;
  }
  return block.start + pair[1] + unit - pair[0];
}

/** Where a shown character is: the block, the run in it, the unit in the run. */
export type RunPoint = { block: number; run: number; unit: number };

/**
 * The shown place of the markup offset `offset`: the first character that
 * stands at or after it in the block holding it, or the end of a run that
 * ends there. `null` when no block shows text there.
 */
export function pointOf(blocks: readonly BlockView[], offset: number): RunPoint | null {
  const index = blockAt(blocks, offset);
  if (index < 0) {
    return null;
  }
  const block = blocks[index];
  for (let r = 0; r < block.runs.length; r++) {
    const run = block.runs[r];
    for (let unit = 0; unit < runLength(run); unit++) {
      if (sourceAt(block, run, unit) >= offset) {
        return { block: index, run: r, unit };
      }
    }
    if (block.start + run.end >= offset) {
      return { block: index, run: r, unit: runLength(run) };
    }
  }
  const last = block.runs.length - 1;
  return last < 0 ? null : { block: index, run: last, unit: runLength(block.runs[last]) };
}

/**
 * The block holding the markup offset `offset`: the last block starting at
 * or before it; -1 before the first block.
 */
export function blockAt(blocks: readonly BlockView[], offset: number): number {
  let low = 0;
  let high = blocks.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (blocks[mid].start <= offset) {
      low = mid + 1;
    } else {
      high = mid;
    }
  }
  return low - 1;
}

/**
 * M-09.16: the block `reveal` brings to the top for `offset`: the first
 * whose markup ends after it (`reveal` of the egui facade).
 */
export function revealIndex(blocks: readonly BlockView[], offset: number): number {
  const index = blocks.findIndex((block) => offset < block.start + block.length);
  return index < 0 ? blocks.length - 1 : index;
}

/** M-09.5 to M-09.8: whether the block is shown as a figure: a table, an image, a diagram. */
export function isFigure(block: BlockView): boolean {
  return block.kind.type === 'table' || block.figure !== null;
}

/** Whether the block is set in the code font on the code background. */
export function isCodeLike(block: BlockView): boolean {
  const type = block.kind.type;
  return type === 'code' || type === 'markup' || type === 'image' || type === 'table';
}

/** M-08.8: the sections by the block of their heading, and the blocks folded sections hide. */
export type Folding = {
  /** The section of each heading block that has a body to fold. */
  headings: Map<number, SectionView>;
  /** The blocks hidden by folded sections. */
  hidden: Set<number>;
};

/**
 * M-08.8, UC-004 BR-012: the sections `sections` (by lines, from the
 * backend's `ed-sections`) over `blocks`: a heading block whose line starts
 * a section gets its arrow, and a block whose first line is in a folded
 * body is hidden (`update_folds` of the egui facade).
 */
export function foldingOf(blocks: readonly BlockView[], sections: readonly SectionView[]): Folding {
  const byLine = new Map<number, SectionView>();
  for (const section of sections) {
    byLine.set(section.line, section);
  }
  const folded = sections.filter((s) => s.folded && s.last > s.line);
  const headings = new Map<number, SectionView>();
  const hidden = new Set<number>();
  blocks.forEach((block, index) => {
    const section = block.kind.type === 'heading' ? byLine.get(block.line) : undefined;
    if (section && section.last > section.line) {
      headings.set(index, section);
    }
    if (folded.some((s) => s.line < block.line && block.line <= s.last)) {
      hidden.add(index);
    }
  });
  return { headings, hidden };
}

/** One piece of a code block's text in one colour (`null`: the plain colour). */
export type CodeSegment = { text: string; color: string | null };

/** M-09.4: `text` cut at the spans of `colors`. */
export function codeSegments(text: string, colors: CodeColors | null): CodeSegment[] {
  if (!colors || colors.spans.length === 0) {
    return [{ text, color: null }];
  }
  const segments: CodeSegment[] = [];
  let at = 0;
  for (const [start, end, style] of colors.spans) {
    if (start > at) {
      segments.push({ text: text.slice(at, start), color: null });
    }
    if (end > Math.max(start, at)) {
      segments.push({
        text: text.slice(Math.max(start, at), end),
        color: colors.styles[style]?.color ?? null,
      });
      at = end;
    }
  }
  if (at < text.length) {
    segments.push({ text: text.slice(at), color: null });
  }
  return segments;
}

/** The text a block shows, run after run. */
export function blockText(block: BlockView): string {
  return block.runs.map((run) => run.text).join('');
}

/** The font sizes of the facade (M-09.2): body text, code, headings by level. */
export const BODY_SIZE = 15;
export const CODE_SIZE = 14;
const HEADING_SIZES = [26, 22, 19, 17, 16, 16];

/** The font size of the block's text. */
export function fontSize(block: BlockView): number {
  if (block.kind.type === 'heading') {
    return HEADING_SIZES[Math.min(Math.max(block.kind.level, 1), 6) - 1];
  }
  return isCodeLike(block) ? CODE_SIZE : BODY_SIZE;
}

/** The line height of the facade's text, as a share of the font size. */
export const LINE_HEIGHT = 1.25;

/** Space below each block (egui `BLOCK_GAP`), and the extra below code-like blocks and breaks. */
export const BLOCK_GAP = 8;

/**
 * The space below the block, as egui adds it after the block's height: 16
 * more below a break, 8 more below a figure; code and markup get their 8
 * from the 4 px of code background around them.
 */
export function gapAfter(block: BlockView): number {
  if (block.kind.type === 'thematicBreak') {
    return BLOCK_GAP + 16;
  }
  return BLOCK_GAP + (isFigure(block) ? 8 : 0);
}

/** How high a figure is guessed before it is drawn. */
const FIGURE_GUESS = 200;

/**
 * The height of a block before it is drawn, in a facade `width` pixels
 * wide: its lines of text wrapped at a usual character width, a figure a
 * guess. Drawn blocks are measured; this only keeps the scroll bar near the
 * truth for the others.
 */
export function estimateHeight(block: BlockView, width: number): number {
  const size = fontSize(block);
  const line = size * LINE_HEIGHT;
  const gap = gapAfter(block);
  const type = block.kind.type;
  if (type === 'thematicBreak') {
    return 16 + gap;
  }
  if (type === 'table') {
    const rows = block.kind.type === 'table' ? block.kind.table.rows.length : 1;
    return rows * (BODY_SIZE * LINE_HEIGHT + 10) + gap;
  }
  if (block.figure !== null) {
    return FIGURE_GUESS + (block.figure.caption ? line + 6 : 0) + gap;
  }
  const charWidth = size * (isCodeLike(block) ? 0.61 : 0.55);
  const columns = Math.max(8, Math.floor(Math.max(width, 80) / charWidth));
  const text = block.prefix + blockText(block);
  let rows = 0;
  for (const part of text.split('\n')) {
    rows += Math.max(1, Math.ceil(part.length / columns));
  }
  return rows * line + (isCodeLike(block) ? 8 : 0) + gap;
}

/**
 * M-12.2: the minimap of the facade, one bar per layout row, a figure as an
 * area (`build_marks` of the egui facade). The rows are not measured but
 * worked out from each block's height and text (A-35): a block's height is
 * cut into rows of its line height; a code block's rows are its lines, a
 * prose block's rows are full but the last.
 *
 * `top(i)` and `height(i)` give the place of block `i` in the content
 * (without the padding above the first), `width` the width of the text.
 */
export function facadeMarks(
  blocks: readonly BlockView[],
  hidden: ReadonlySet<number>,
  top: (index: number) => number,
  height: (index: number) => number,
  width: number,
  padding: number,
): { marks: Mark[]; areas: Area[] } {
  const marks: Mark[] = [];
  const areas: Area[] = [];
  blocks.forEach((block, index) => {
    if (hidden.has(index)) {
      return;
    }
    const at = padding + top(index);
    const shown = Math.max(height(index) - gapAfter(block), 0);
    if (isFigure(block)) {
      areas.push({ top: at, height: Math.max(shown, 1) });
      return;
    }
    const size = fontSize(block);
    const rows = Math.max(1, Math.round(shown / (size * LINE_HEIGHT)));
    const row = shown / rows;
    const columns = Math.max(
      1,
      Math.floor(Math.max(width, 80) / (size * (isCodeLike(block) ? 0.61 : 0.55))),
    );
    const text = block.prefix + blockText(block);
    const lines = text.split('\n');
    const strong = block.kind.type === 'heading';
    for (let r = 0; r < rows; r++) {
      let chars: number;
      if (lines.length === rows) {
        chars = lines[r].length;
      } else {
        chars = Math.min(Math.max(text.length - r * columns, 0), columns);
      }
      if (chars > 0) {
        marks.push({
          top: at + r * row,
          height: row * 0.7,
          width: Math.min(chars / columns, 1),
          strong,
        });
      }
    }
  });
  return { marks, areas };
}
