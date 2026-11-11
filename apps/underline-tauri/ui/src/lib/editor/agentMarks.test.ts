// M-17.6: which lines the agent cursor marks in the source view.

import { describe, expect, it } from 'vitest';
import { markedLines } from './agentMarks';

describe('M-17.6 markedLines', () => {
  it('marks from the first line to the one before `to`, the first named', () => {
    const marked = markedLines(10, [{ from: 2, to: 4, name: 'Claude', wrote: false }]);

    expect([...marked]).toEqual([
      [2, { name: 'Claude', first: true }],
      [3, { name: 'Claude', first: false }],
    ]);
  });

  it('an empty mark marks its line; lines past the end are left out', () => {
    expect([...markedLines(3, [{ from: 1, to: 1, name: 'A', wrote: false }]).keys()]).toEqual([1]);
    expect([...markedLines(3, [{ from: 2, to: 9, name: 'A', wrote: false }]).keys()]).toEqual([2]);
    expect(markedLines(3, [{ from: 5, to: 6, name: 'A', wrote: false }]).size).toBe(0);
  });

  it('where marks overlap, the one that starts first wins, as in egui', () => {
    const marked = markedLines(10, [
      { from: 1, to: 3, name: 'A', wrote: false },
      { from: 2, to: 4, name: 'B', wrote: false },
    ]);

    expect(marked.get(2)?.name).toBe('A');
    expect(marked.get(3)).toEqual({ name: 'B', first: false });
  });
});
