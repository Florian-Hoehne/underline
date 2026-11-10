import { EditorState } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { colorField, setColors } from './highlight';

function ranges(state: EditorState): [number, number, string][] {
  const out: [number, number, string][] = [];
  state.field(colorField).between(0, state.doc.length, (from, to, value) => {
    out.push([from, to, String(value.spec.attributes?.style)]);
  });
  return out;
}

describe('M-08.2 colours from the backend', () => {
  const styles = [
    { color: '#0b57d0', bold: true, italic: false },
    { color: '#22863a', bold: false, italic: true },
  ];

  it('marks the spans of the lines given, in UTF-16 units of each line', () => {
    let state = EditorState.create({ doc: '# Head\ntext `code`\n', extensions: [colorField] });

    state = state.update({
      effects: setColors.of({
        from: 0,
        lines: [[[0, 6, 0]], [[5, 11, 1]]],
        styles,
      }),
    }).state;

    expect(ranges(state)).toEqual([
      [0, 6, 'color: #0b57d0; font-weight: bold'],
      [12, 18, 'color: #22863a; font-style: italic'],
    ]);
  });

  it('replaces the colours of those lines only and moves the others along', () => {
    let state = EditorState.create({ doc: 'aa\nbb\ncc', extensions: [colorField] });
    state = state.update({
      effects: setColors.of({ from: 0, lines: [[[0, 2, 0]], [[0, 2, 0]], [[0, 2, 0]]], styles }),
    }).state;

    state = state.update({ changes: { from: 0, insert: 'x' } }).state;
    state = state.update({ effects: setColors.of({ from: 1, lines: [[]], styles }) }).state;

    // Text typed before a span is not coloured with it.
    expect(ranges(state).map(([from, to]) => [from, to])).toEqual([
      [1, 3],
      [7, 9],
    ]);
  });
});
