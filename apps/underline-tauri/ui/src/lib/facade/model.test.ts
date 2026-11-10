import { describe, expect, it } from 'vitest';
import type { BlockView, BlocksView, RunView } from '../bindings';
import { fakeBlocks, TOUR_TEXT } from './fakeFacade';
import { Heights } from './heights';
import {
  applyBlocks,
  blockAt,
  blockHits,
  codeSegments,
  estimateHeight,
  facadeHits,
  facadeMarks,
  hitParts,
  mapHits,
  foldingOf,
  gapAfter,
  isFigure,
  pointOf,
  revealIndex,
  sourceAt,
} from './model';

function run(text: string, offsets: [number, number][], end: number): RunView {
  return { text, bold: false, italic: false, strike: false, code: false, link: null, offsets, end };
}

function paragraph(start: number, line: number, runs: RunView[], length = 10): BlockView {
  return {
    kind: { type: 'paragraph' },
    start,
    line,
    length,
    lines: 1,
    prefix: '',
    runs,
    colors: null,
    figure: null,
  };
}

describe('M-09.1 the blocks of the facade', () => {
  it('ADR-024 a change replaces its blocks and moves the ones after it', () => {
    const blocks = [
      paragraph(0, 0, [run('a', [[0, 0]], 1)]),
      paragraph(3, 2, [run('b', [[0, 0]], 1)]),
      paragraph(6, 4, [run('c', [[0, 0]], 1)]),
    ];
    const change: BlocksView = {
      note: 'Home',
      rev: 8,
      base: 4,
      version: 5,
      from: 1,
      removed: 1,
      blocks: [paragraph(3, 2, [run('b🙂', [[0, 0]], 3)])],
      shift: 2,
      lineShift: 1,
    };

    const next = applyBlocks({ rev: 7, version: 4, blocks }, change)!;

    expect(next.rev).toBe(8);
    expect(next.version).toBe(5);
    expect(next.blocks.map((b) => b.runs[0].text)).toEqual(['a', 'b🙂', 'c']);
    expect(next.blocks[2].start).toBe(8);
    expect(next.blocks[2].line).toBe(5);
    expect(next.blocks[0]).toBe(blocks[0]);
    // Another version: the facade asks for all blocks again.
    expect(applyBlocks({ rev: 7, version: 3, blocks }, change)).toBeNull();
  });

  it('ADR-021 a shown character maps to its markup in UTF-16 units and back', () => {
    // "Some \*star 🙂" shown as "Some *star 🙂": the star jumps over its backslash.
    const block = paragraph(
      100,
      0,
      [
        run(
          'Some *star 🙂',
          [
            [0, 0],
            [5, 6],
          ],
          14,
        ),
      ],
      14,
    );
    const shown = block.runs[0];

    expect(sourceAt(block, shown, 0)).toBe(100);
    expect(sourceAt(block, shown, 4)).toBe(104);
    expect(sourceAt(block, shown, 5)).toBe(106);
    expect(sourceAt(block, shown, 11)).toBe(112);
    expect(sourceAt(block, shown, 13)).toBe(114);
    expect(pointOf([block], 106)).toEqual({ block: 0, run: 0, unit: 5 });
    // The backslash is not shown: it goes to the star.
    expect(pointOf([block], 105)).toEqual({ block: 0, run: 0, unit: 5 });
    expect(pointOf([block], 114)).toEqual({ block: 0, run: 0, unit: 13 });
    expect(pointOf([block], 50)).toBeNull();
    expect(blockAt([block, paragraph(120, 2, [])], 119)).toBe(0);
  });

  it('M-09.16 reveal takes the first block whose markup ends after the offset', () => {
    const blocks = [paragraph(0, 0, [], 5), paragraph(7, 2, [], 5), paragraph(14, 4, [], 5)];

    expect(revealIndex(blocks, 0)).toBe(0);
    expect(revealIndex(blocks, 5)).toBe(1);
    expect(revealIndex(blocks, 13)).toBe(2);
    expect(revealIndex(blocks, 99)).toBe(2);
  });

  it('M-08.8 a heading with a body gets its arrow; a folded body hides its blocks', () => {
    const blocks = fakeBlocks('# A\n\ntext\n\n## B\n\nmore\n\n# C\n');
    const sections = [
      { line: 0, last: 7, folded: false },
      { line: 4, last: 7, folded: true },
      { line: 8, last: 8, folded: false },
    ];

    const { headings, hidden } = foldingOf(blocks, sections);

    expect([...headings.keys()]).toEqual([0, 2]);
    expect([...hidden]).toEqual([3]);
  });

  it('M-09.4 a code block is cut at its colours', () => {
    const segments = codeSegments('fn main() {}', {
      spans: [
        [0, 2, 0],
        [3, 7, 1],
      ],
      styles: [
        { color: '#aa0000', bold: false, italic: false },
        { color: '#0000aa', bold: false, italic: false },
      ],
    });

    expect(segments).toEqual([
      { text: 'fn', color: '#aa0000' },
      { text: ' ', color: null },
      { text: 'main', color: '#0000aa' },
      { text: '() {}', color: null },
    ]);
    expect(codeSegments('x', null)).toEqual([{ text: 'x', color: null }]);
  });

  it('heights are guessed from the text before a block is drawn', () => {
    const blocks = fakeBlocks(TOUR_TEXT);
    const heading = blocks[0];
    const code = blocks.find((b) => b.kind.type === 'code' && b.figure === null)!;
    const figure = blocks.find((b) => b.kind.type === 'image')!;

    expect(estimateHeight(heading, 600)).toBeCloseTo(26 * 1.25 + 8);
    expect(estimateHeight(code, 600)).toBeCloseTo(3 * 14 * 1.25 + 8 + 8);
    expect(isFigure(figure)).toBe(true);
    expect(gapAfter(figure)).toBe(16);
    expect(estimateHeight(figure, 600)).toBeGreaterThan(100);
  });
});

describe('M-12.2 the minimap of the facade', () => {
  it('one bar per row, headings strong, figures as areas, folded blocks left out', () => {
    const blocks = fakeBlocks('# Title\n\nshort text\n\n![The plan](img/plan.svg)\n');
    const heights = [40, 30, 216];
    const tops = [0, 40, 70];

    const { marks, areas } = facadeMarks(
      blocks,
      new Set(),
      (i) => tops[i],
      (i) => heights[i],
      600,
      12,
    );

    expect(marks).toHaveLength(2);
    expect(marks[0]).toMatchObject({ top: 12, strong: true });
    expect(marks[1]).toMatchObject({ top: 52, strong: false });
    expect(areas).toEqual([{ top: 82, height: 200 }]);
    const folded = facadeMarks(
      blocks,
      new Set([1, 2]),
      (i) => tops[i],
      (i) => heights[i],
      600,
      12,
    );
    expect(folded.marks).toHaveLength(1);
    expect(folded.areas).toEqual([]);
  });
});

describe('ADR-024 the heights of a virtualized facade', () => {
  it('finds blocks by their place and keeps measured heights over a change', () => {
    const heights = new Heights();
    heights.reset([10, 20, 30, 40]);

    expect(heights.total()).toBe(100);
    expect(heights.top(2)).toBe(30);
    expect(heights.indexAt(29)).toBe(1);
    expect(heights.indexAt(30)).toBe(2);
    expect(heights.indexAt(-5)).toBe(0);
    expect(heights.indexAt(1000)).toBe(3);
    expect(heights.set(1, 25)).toBe(5);
    expect(heights.measured(1)).toBe(true);

    heights.splice(2, 1, [5, 5]);

    expect(heights.length).toBe(5);
    expect(heights.top(4)).toBe(45);
    expect(heights.get(1)).toBe(25);
    heights.reestimate(() => 1, false);
    expect(heights.get(1)).toBe(25);
    expect(heights.get(0)).toBe(1);
  });
});

describe('M-13.2 the hits of a search in the facade (UC-008 BR-008)', () => {
  // "**bold** ferry" shows "bold ferry": "bold" stands at 2, " ferry" at 8.
  const shown = paragraph(10, 0, [run('bold', [[0, 2]], 8), run(' ferry', [[0, 8]], 14)], 15);

  it('marks the units whose markup lies in a hit, the selected hit apart', () => {
    const hits = [[19, 24]] as const;
    expect(hitParts(shown, shown.runs[1], 0, 6, hits, null)).toEqual([
      { from: 0, to: 1, hit: false, current: false },
      { from: 1, to: 6, hit: true, current: false },
    ]);
    expect(hitParts(shown, shown.runs[1], 0, 6, hits, [19, 24])[1].current).toBe(true);
    // A hit over the markup of "**" marks only what is shown.
    expect(hitParts(shown, shown.runs[0], 0, 4, [[10, 14]], null)).toEqual([
      { from: 0, to: 2, hit: true, current: false },
      { from: 2, to: 4, hit: false, current: false },
    ]);
    expect(hitParts(shown, shown.runs[0], 0, 4, [], null)).toEqual([
      { from: 0, to: 4, hit: false, current: false },
    ]);
  });

  it('finds the hits of a block, also one that starts before it', () => {
    const hits = [
      [0, 3],
      [8, 12],
      [12, 14],
      [24, 26],
      [30, 31],
    ] as const;
    expect(blockHits(shown, hits)).toEqual([
      [8, 12],
      [12, 14],
      [24, 26],
    ]);
  });

  it('moves the hits after a change of the text, and drops those in its blocks', () => {
    const blocks = [
      paragraph(0, 0, [run('a', [[0, 0]], 1)], 10),
      paragraph(10, 1, [run('b', [[0, 0]], 1)], 10),
      paragraph(20, 2, [run('c', [[0, 0]], 1)], 10),
    ];
    const change: BlocksView = {
      note: 'A',
      rev: 2,
      base: 1,
      version: 2,
      from: 1,
      removed: 1,
      blocks: [paragraph(10, 1, [run('bb', [[0, 0]], 2)], 12)],
      shift: 2,
      lineShift: 0,
    };
    expect(
      mapHits(
        [
          [2, 4],
          [12, 13],
          [22, 25],
        ],
        blocks,
        change,
      ),
    ).toEqual([
      [2, 4],
      [24, 27],
    ]);
  });

  it('M-12.4 places a hit of the sieve in the row of its block', () => {
    const hits = facadeHits(
      [shown],
      [[19, 24]],
      new Set(),
      () => 0,
      () => 19 * 3,
      400,
      6,
    );
    expect(hits).toHaveLength(1);
    expect(hits[0].top).toBe(6);
    expect(hits[0].left).toBeGreaterThan(0);
    expect(
      facadeHits(
        [shown],
        [[19, 24]],
        new Set([0]),
        () => 0,
        () => 57,
        400,
        6,
      ),
    ).toEqual([]);
  });
});
