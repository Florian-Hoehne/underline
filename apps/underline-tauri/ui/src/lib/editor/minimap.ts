// M-12 (FR-030, UC-004 BR-013): the minimap beside the source view, as the
// egui widget draws it (`minimap.rs`): a strip of 80 px, 4 px from the
// editor, hidden when the editor and the strip have less than 240 px. The
// whole note is scaled into it, never more than 0.2, each shown line a bar
// as long as the line, headings stronger. The part in view is tinted; a
// click or a drag puts that place in the middle of the view.
//
// M-12.4 (UC-008 BR-008), the sieve: while there are search hits the strip
// is veiled (black, alpha 150) and each hit shines through at its line and
// column with a glow; the hits blink 0.6 s on and 0.4 s off unless the
// setting turns blinking off.
//
// This module works out what to draw; `Minimap.svelte` paints it.

import type { EditorView } from '@codemirror/view';

/** Width of the strip. */
export const WIDTH = 80;
/** Space between the editor and the strip. */
export const GAP = 4;
/** The largest scale: a short note is not stretched over the strip. */
export const MAX_SCALE = 0.2;
/** How long a blinking hit is shown, then hidden, in milliseconds. */
export const FOUND_ON = 600;
export const FOUND_OFF = 400;
/** The bars are built anew at most this often while the text changes. */
export const REBUILD_EVERY = 150;
/** The bars stay this far from the sides of the strip. */
export const INSET = 6;
/** The veil of the sieve: black with alpha 150 of 255. */
export const VEIL = 'rgba(0, 0, 0, 0.588)';

/** One bar: where it is in the view's content, its share of the width (0 to 1). */
export type Mark = { top: number; height: number; width: number; strong: boolean };

/** One search hit: where it is in the view's content, and where in the width (0 to 1). */
export type Hit = { top: number; height: number; left: number; width: number };

/** One shown line of the view, in its content's pixels. */
export type LineBox = { top: number; height: number; chars: number; heading: boolean };

/** One rectangle to paint, in the strip's pixels. */
export type Rect = { x: number; y: number; width: number; height: number };

/** Whether the strip shows: switched on, and room for it (`width` of editor and strip). */
export function minimapShown(shown: boolean, width: number): boolean {
  return shown && width >= 3 * WIDTH;
}

/** The scale from content to strip for `content` pixels of content. */
export function scaleOf(stripHeight: number, content: number): number {
  return Math.min(stripHeight / Math.max(content, 1), MAX_SCALE);
}

/** M-12.2: the bars of `lines`, `columns` chars making a full bar. */
export function marksOf(lines: readonly LineBox[], columns: number): Mark[] {
  const marks: Mark[] = [];
  for (const line of lines) {
    if (line.height > 0 && line.chars > 0) {
      marks.push({
        top: line.top,
        height: line.height * 0.7,
        width: Math.min(line.chars / Math.max(columns, 1), 1),
        strong: line.heading,
      });
    }
  }
  return marks;
}

/** The bar of one pixel row: its share of the width, and whether it is strong. */
export type Bar = { width: number; strong: boolean };

/**
 * One bar per pixel row of the strip: the widest mark there; a heading
 * makes it strong. So a long note costs no more to draw than a short one.
 */
export function barsOf(marks: readonly Mark[], scale: number, rows: number): (Bar | null)[] {
  const bars: (Bar | null)[] = new Array<Bar | null>(Math.max(0, rows)).fill(null);
  for (const mark of marks) {
    if (mark.height <= 0) {
      continue;
    }
    const first = Math.max(0, Math.floor(mark.top * scale));
    const last = Math.ceil((mark.top + mark.height) * scale);
    const end = Math.min(Math.max(last, first + 1), rows);
    for (let row = first; row < end; row++) {
      const bar = bars[row];
      if (bar && bar.width >= mark.width) {
        bar.strong ||= mark.strong;
      } else {
        bars[row] = { width: mark.width, strong: mark.strong };
      }
    }
  }
  return bars;
}

/** The rectangle of the bar in `row`, in a strip `stripWidth` wide. */
export function barRect(row: number, bar: Bar, stripWidth: number): Rect {
  const inner = stripWidth - 2 * INSET;
  return { x: INSET, y: row, width: inner * Math.min(Math.max(bar.width, 0.05), 1), height: 1 };
}

/** M-12.3: the tinted part in view, at least 4 px high. */
export function visibleRect(from: number, to: number, scale: number, stripWidth: number): Rect {
  const top = from * scale;
  return { x: 0, y: top, width: stripWidth, height: Math.max(to * scale, top + 4) - top };
}

/**
 * M-12.4: each hit at its word, at least 3 by 2 pixels, with the glow
 * around it (2.5 px); hits on the same pixel are drawn once.
 */
export function hitRects(
  hits: readonly Hit[],
  scale: number,
  stripWidth: number,
): { core: Rect; glow: Rect }[] {
  const inner = stripWidth - 2 * INSET;
  const drawn = new Set<string>();
  const rects: { core: Rect; glow: Rect }[] = [];
  for (const hit of hits) {
    const core = {
      x: INSET + Math.min(Math.max(hit.left, 0), 1) * inner,
      y: hit.top * scale,
      width: Math.max(hit.width * inner, 3),
      height: Math.max(hit.height * scale, 2),
    };
    const key = `${Math.trunc(core.x)},${Math.trunc(core.y)}`;
    if (drawn.has(key)) {
      continue;
    }
    drawn.add(key);
    rects.push({
      core,
      glow: {
        x: core.x - 2.5,
        y: core.y - 2.5,
        width: core.width + 5,
        height: core.height + 5,
      },
    });
  }
  return rects;
}

/** M-12.4: whether blinking hits are shown at `time` (milliseconds). */
export function hitsShown(blink: boolean, time: number): boolean {
  return !blink || time % (FOUND_ON + FOUND_OFF) < FOUND_ON;
}

/** M-12.3: the scroll offset that puts the place under the pointer in the middle. */
export function scrollFor(
  y: number,
  scale: number,
  visible: readonly [number, number],
  content: number,
): number {
  const at = y / scale;
  const height = visible[1] - visible[0];
  return Math.min(Math.max(at - height / 2, 0), Math.max(content - height, 0));
}

/** What the minimap shows of a source view, in its content's pixels. */
export type Geometry = {
  /** The bars of the shown lines. */
  marks: Mark[];
  /** The height of the whole content. */
  content: number;
  /** The part in view, `[from, to]`. */
  visible: [number, number];
};

/** The place of the text's start in the view's scrolled content. */
function documentOffset(view: EditorView): number {
  const scroller = view.scrollDOM.getBoundingClientRect();
  return view.documentTop - scroller.top + view.scrollDOM.scrollTop;
}

/** How many chars make a full bar: the chars that fit on a row. */
function columnsOf(view: EditorView): number {
  const width = view.contentDOM.clientWidth - 12;
  return Math.max(1, Math.floor(width / Math.max(view.defaultCharacterWidth, 1)));
}

/** The part of the content in view. */
export function visibleOf(view: EditorView): [number, number] {
  const top = view.scrollDOM.scrollTop;
  return [top, top + view.scrollDOM.clientHeight];
}

/**
 * M-12.2: the bars of every shown line of `view`; a folded section is the
 * bar of its heading. `headings` are the lines of the headings, from 0, and
 * `folded` the folded line ranges `[first, last]` (from 0) under them.
 *
 * Asking CodeMirror for the place of each of 10,000 lines costs too much
 * while typing (ADR-024), so the lines are laid out here as CodeMirror
 * estimates lines it has not drawn: rows of the line height, a row per
 * `columns` chars; then fitted to the height of the whole content.
 */
export function geometryOf(
  view: EditorView,
  headings: ReadonlySet<number>,
  folded: readonly (readonly [number, number])[] = [],
): Geometry {
  const offset = documentOffset(view);
  const columns = columnsOf(view);
  const row = view.defaultLineHeight;
  const lines: LineBox[] = [];
  let top = 0;
  let number = 0;
  let fold = 0;
  for (const iter = view.state.doc.iterLines(); !iter.next().done; number++) {
    while (fold < folded.length && folded[fold][1] < number) {
      fold++;
    }
    if (fold < folded.length && folded[fold][0] <= number) {
      continue;
    }
    const height = row * Math.max(1, Math.ceil(iter.value.length / columns));
    lines.push({ top, height, chars: iter.value.length, heading: headings.has(number) });
    top += height;
  }
  const content = view.scrollDOM.scrollHeight;
  const text = Math.max(content - 2 * offset, 1);
  const fit = top > 0 ? text / top : 1;
  for (const line of lines) {
    line.top = offset + line.top * fit;
    line.height *= fit;
  }
  return { marks: marksOf(lines, columns), content, visible: visibleOf(view) };
}

/** M-12.4: the hits `ranges` of `view`, each at its word. */
export function hitsOf(view: EditorView, ranges: readonly (readonly [number, number])[]): Hit[] {
  const doc = view.state.doc;
  const offset = documentOffset(view);
  const columns = columnsOf(view);
  const row = view.defaultLineHeight;
  const hits: Hit[] = [];
  for (const [from, to] of ranges) {
    if (from > doc.length) {
      continue;
    }
    const line = doc.lineAt(from);
    const block = view.lineBlockAt(from);
    const column = from - line.from;
    // A hit in a folded section is shown at its heading, as in egui.
    const wrapped = block.from === line.from ? Math.floor(column / columns) : 0;
    const at = column % columns;
    const length = Math.min(Math.max(to - from, 1), columns - at);
    hits.push({
      top: offset + block.top + wrapped * row,
      height: row,
      left: at / columns,
      width: length / columns,
    });
  }
  return hits;
}
