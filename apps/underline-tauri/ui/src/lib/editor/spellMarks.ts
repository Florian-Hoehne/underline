// FR-099, UC-005 BR-023 (ADR-030): the words the spell check marks, as
// CodeMirror mark decorations of the source view. The backend sends them as
// `spell-marks` for a revision of its mirror; they replace the marks shown
// once the window's core is in step with that revision, and until then the
// marks shown move along with every change, as the colours do (ADR-025).
// A mark is a wavy line in the theme's colour for errors (`--ui-spell`,
// app.css); it changes nothing in the note.

import { RangeSet, StateEffect, StateField, type Text } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';

/** A marked word, `[from, to]` in web units, as the event brings it. */
export type SpellRange = readonly [number, number];

/** Replaces the marked words of the view. */
export const setSpellMarks = StateEffect.define<readonly SpellRange[]>();

const spellMark = Decoration.mark({
  class: 'cm-ul-spell',
  // Screen readers name the word misspelled.
  attributes: { 'data-spell': '', 'aria-invalid': 'spelling' },
});

/** The decorations of `marks` (sorted) in `doc`; marks past its end are left out. */
export function spellDecorations(doc: Text, marks: readonly SpellRange[]): DecorationSet {
  const ranges = [];
  for (const [from, to] of marks) {
    if (from < to && to <= doc.length) {
      ranges.push(spellMark.range(from, to));
    }
  }
  return Decoration.set(ranges, true);
}

/** The marked words of the view, moved along with every change. */
export const spellField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    let next = decorations.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setSpellMarks)) {
        next = spellDecorations(tr.state.doc, effect.value);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/**
 * UC-005 A28: the marked word at the place `at` of the view (its end
 * counts), `null` when there is none.
 */
export function spellWordAt(view: EditorView, at: number): SpellRange | null {
  let found: SpellRange | null = null;
  const marks: RangeSet<Decoration> = view.state.field(spellField, false) ?? Decoration.none;
  marks.between(at, at, (from, to) => {
    if (from <= at && at <= to) {
      found = [from, to];
      return false;
    }
    return undefined;
  });
  return found;
}

/**
 * UC-005 A28: the marked word after the place `at` (`back`: before it),
 * from the start (the end) when there is none; `null` without marks.
 */
export function spellWordNext(view: EditorView, at: number, back: boolean): SpellRange | null {
  const all: SpellRange[] = [];
  view.state.field(spellField, false)?.between(0, view.state.doc.length, (from, to) => {
    all.push([from, to]);
  });
  if (all.length === 0) {
    return null;
  }
  if (back) {
    return [...all].reverse().find(([, to]) => to < at) ?? all[all.length - 1];
  }
  return all.find(([from]) => from > at) ?? all[0];
}

/** The look of a marked word. */
export const spellTheme = EditorView.baseTheme({
  '.cm-ul-spell': {
    textDecoration: 'underline wavy var(--ui-spell)',
    textDecorationThickness: 'var(--ui-spell-thickness)',
    textDecorationSkipInk: 'none',
    textUnderlineOffset: 'var(--ui-spell-offset)',
  },
});
