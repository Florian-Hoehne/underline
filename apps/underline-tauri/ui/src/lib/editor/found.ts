// UC-008 BR-008, M-11.3: the marked hits of the Nano search, as the editor
// core reports them (`views`, ADR-025): behind the text in the theme's colour for hits
// and underlined 2 px in its colour for a hit, as egui marks them. The Vim
// search marks its hits by itself (`.cm-searchMatch`), in the same look.
// The search of feature F9 will use the same marks.

import { StateEffect, StateField, type Text } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';

/** The hits, `[from, to]` in the text the view shows. */
export const setFound = StateEffect.define<readonly (readonly [number, number])[]>();

const hit = Decoration.mark({ class: 'cm-ul-found' });

function marksOf(doc: Text, ranges: readonly (readonly [number, number])[]): DecorationSet {
  return Decoration.set(
    ranges
      .filter(([from, to]) => from < to && to <= doc.length)
      .map(([from, to]) => hit.range(from, to)),
    true,
  );
}

/** The marked hits, moved along with every change. */
export const foundField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(marks, tr) {
    let next = marks.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setFound)) {
        next = marksOf(tr.state.doc, effect.value);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/** The marked hits of `view`, `[from, to]`. */
export function foundRanges(marks: DecorationSet): [number, number][] {
  const ranges: [number, number][] = [];
  for (let cursor = marks.iter(); cursor.value; cursor.next()) {
    ranges.push([cursor.from, cursor.to]);
  }
  return ranges;
}
