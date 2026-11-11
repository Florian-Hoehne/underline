// M-17.6 (FR-057, UC-018 BR-006, UC-019 BR-006, feature F14): the agent
// cursor in the source view. The backend sends the lines each connected
// agent read or wrote within the last 10 seconds, as far as access allows
// (`agent-marks`); the lines are tinted blue behind the text with a 3 px
// bar at their left, and the agent's name follows the first of them in 80 %
// of the size. The colours are fixed, the same in every theme, as in egui.
// Like egui, the lines are taken as they are numbered now; a mark moves
// along with the edits made while it shows.

import { StateEffect, StateField, type Text } from '@codemirror/state';
import { Decoration, EditorView, type DecorationSet } from '@codemirror/view';
import type { AgentMarkView } from '../bindings';

/** The agent cursors of the note, as the backend sent them. */
export const setAgentMarks = StateEffect.define<readonly AgentMarkView[]>();

/**
 * The line of each marked line, counted from 0, with the agent that marks
 * it and whether it is the mark's first line; where marks overlap, the one
 * that starts first wins, as in egui.
 */
export function markedLines(
  lines: number,
  marks: readonly AgentMarkView[],
): Map<number, { name: string; first: boolean }> {
  const marked = new Map<number, { name: string; first: boolean }>();
  for (const mark of marks) {
    const end = Math.min(Math.max(mark.to, mark.from + 1), lines);
    for (let line = mark.from; line < end; line++) {
      if (!marked.has(line)) {
        marked.set(line, { name: mark.name, first: line === mark.from });
      }
    }
  }
  return marked;
}

function decorationsOf(doc: Text, marks: readonly AgentMarkView[]): DecorationSet {
  const marked = markedLines(doc.lines, marks);
  const ranges = [...marked.entries()]
    .sort(([a], [b]) => a - b)
    .map(([line, { name, first }]) =>
      Decoration.line({
        attributes: first
          ? { class: 'cm-ul-agent cm-ul-agent-first', 'data-agent': name }
          : { class: 'cm-ul-agent' },
      }).range(doc.line(line + 1).from),
    );
  return Decoration.set(ranges);
}

/** The agent cursors of the view, moved along with every change. */
export const agentMarksField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, tr) {
    let next = decorations.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setAgentMarks)) {
        next = decorationsOf(tr.state.doc, effect.value);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

/**
 * egui's `AGENT_COLOR` (30, 90, 170, 60 premultiplied, added to what is
 * behind) and `AGENT_BAR_COLOR` (40, 120, 220). CSS has no additive blend
 * for a background; the tint is the bar colour at 15 %, light enough that
 * the text keeps its contrast (A-166, UX review F14).
 */
export const AGENT_TINT = 'rgb(40 120 220 / 0.15)';
export const AGENT_BAR = 'rgb(40 120 220)';

/**
 * The look of the marked lines and the name. The bar is a real border, so
 * it shows in forced colours too (`app.css`); the name stands on a pill in
 * the theme's background and text colours with a border in the bar's
 * colour, so it reads in every theme (UX review F14).
 */
export const agentMarksTheme = EditorView.baseTheme({
  '.cm-line.cm-ul-agent': {
    backgroundColor: AGENT_TINT,
    borderLeft: `3px solid ${AGENT_BAR}`,
    paddingLeft: '3px',
  },
  '.cm-ul-agent-first::after': {
    content: 'attr(data-agent)',
    marginLeft: '16px',
    padding: '0 4px',
    fontSize: '80%',
    color: 'var(--palette-foreground)',
    backgroundColor: 'var(--palette-background)',
    border: `1px solid ${AGENT_BAR}`,
    borderRadius: '3px',
    fontFamily: 'var(--ui-font, inherit)',
    pointerEvents: 'none',
  },
});
