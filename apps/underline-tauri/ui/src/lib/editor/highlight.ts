// M-08.2 (ADR-010, ADR-024): the backend's syntect colours as CodeMirror
// decorations. They come as an event after the input that changed the
// text; until then the decorations of the lines are moved along with the
// changes, and lines not coloured yet show in the plain text colour.

import { StateEffect, StateField, type Text } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import type { HighlightView, SpanStyle } from '../bindings';

/** The colours of the lines `from` on, as the event brings them. */
export type LineColors = Pick<HighlightView, 'from' | 'lines' | 'styles'>;

/** Replaces the colours of the lines in `LineColors`. */
export const setColors = StateEffect.define<LineColors>();

const marks = new Map<string, Decoration>();

/** One decoration per look, made once. */
function markOf(style: SpanStyle): Decoration {
  const css = [
    `color: ${style.color}`,
    style.bold ? 'font-weight: bold' : '',
    style.italic ? 'font-style: italic' : '',
  ]
    .filter(Boolean)
    .join('; ');
  let mark = marks.get(css);
  if (!mark) {
    mark = Decoration.mark({ attributes: { style: css } });
    marks.set(css, mark);
  }
  return mark;
}

/** `colors` applied to `decorations` of the text `doc`. */
export function withColors(
  decorations: DecorationSet,
  doc: Text,
  colors: LineColors,
): DecorationSet {
  const first = colors.from + 1;
  const last = Math.min(colors.from + colors.lines.length, doc.lines);
  if (first > last) {
    return decorations;
  }
  const start = doc.line(first).from;
  const end = doc.line(last).to;
  const added = [];
  for (let n = first; n <= last; n++) {
    const line = doc.line(n);
    for (const [from, to, style] of colors.lines[n - first]) {
      const at = Math.min(line.from + from, line.to);
      const until = Math.min(line.from + to, line.to);
      if (at < until && colors.styles[style]) {
        added.push(markOf(colors.styles[style]).range(at, until));
      }
    }
  }
  return decorations.update({
    filterFrom: start,
    filterTo: end,
    filter: (from, to) => to < start || from > end,
    add: added,
    sort: true,
  });
}

/** The colours of the source view, moved along with every change. */
export const colorField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    let next = decorations.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setColors)) {
        next = withColors(next, tr.state.doc, effect.value);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});
