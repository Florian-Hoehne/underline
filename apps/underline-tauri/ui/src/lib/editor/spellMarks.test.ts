// FR-099, UC-005 BR-023 (ADR-030): the spell check's marks in the source
// view follow the edits made after their revision, until new ones come.

import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, describe, expect, it } from 'vitest';
import { setSpellMarks, spellField, spellWordAt } from './spellMarks';

let view: EditorView | null = null;

afterEach(() => {
  view?.destroy();
  view = null;
});

function viewOf(doc: string): EditorView {
  view = new EditorView({
    state: EditorState.create({ doc, extensions: [spellField] }),
    parent: document.body,
  });
  return view;
}

function marks(of: EditorView): [number, number][] {
  const found: [number, number][] = [];
  of.state.field(spellField).between(0, of.state.doc.length, (from, to) => {
    found.push([from, to]);
  });
  return found;
}

describe('the spell marks of the source view (ADR-030)', () => {
  it('br023 marks follow an edit before them and drop one past the end', () => {
    const v = viewOf('The ferri leaves');
    v.dispatch({
      effects: setSpellMarks.of([
        [4, 9],
        [40, 44],
      ]),
    });
    expect(marks(v)).toEqual([[4, 9]]);
    v.dispatch({ changes: { from: 0, insert: 'So: ' } });
    expect(marks(v)).toEqual([[8, 13]]);
    expect(v.state.sliceDoc(8, 13)).toBe('ferri');
  });

  it('br023 new marks replace the old ones', () => {
    const v = viewOf('teh ferri');
    v.dispatch({
      effects: setSpellMarks.of([
        [0, 3],
        [4, 9],
      ]),
    });
    v.dispatch({ effects: setSpellMarks.of([[4, 9]]) });
    expect(marks(v)).toEqual([[4, 9]]);
    v.dispatch({ effects: setSpellMarks.of([]) });
    expect(marks(v)).toEqual([]);
  });

  it('uc005_a28 the marked word at a place, its ends included', () => {
    const v = viewOf('The ferri leaves');
    v.dispatch({ effects: setSpellMarks.of([[4, 9]]) });
    expect(spellWordAt(v, 4)).toEqual([4, 9]);
    expect(spellWordAt(v, 9)).toEqual([4, 9]);
    expect(spellWordAt(v, 2)).toBeNull();
  });
});
