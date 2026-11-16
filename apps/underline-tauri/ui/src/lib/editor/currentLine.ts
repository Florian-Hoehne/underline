// FR-084, FR-085, UC-005 BR-022: the current line and the matching
// bracket of the source view. The line the cursor is in is highlighted
// while nothing is selected; a bracket `(`, `)`, `[`, `]`, `{` or `}` right
// before or after the cursor is highlighted with its counterpart, or marked
// when it has none (`bracketMatching` of CodeMirror, which scans the text
// as no syntax tree is there). Both only show; nothing in the note changes.
// The colours are mixed from the active theme's palette (ADR-010, the
// `--palette-*` variables), so every theme has its own.

import { RangeSetBuilder, type Extension } from '@codemirror/state';
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import { bracketMatching } from '@codemirror/language';

/** The class of the current line. */
export const CURRENT_LINE = 'cm-ul-current-line';

const lineMark = Decoration.line({ class: CURRENT_LINE });

/** BR-022: the lines of the cursors, none while something is selected. */
export function currentLines(view: EditorView): DecorationSet {
  const { selection, doc } = view.state;
  if (selection.ranges.some((range) => !range.empty)) {
    return Decoration.none;
  }
  const builder = new RangeSetBuilder<Decoration>();
  let last = -1;
  for (const range of selection.ranges) {
    const line = doc.lineAt(range.head);
    if (line.from > last) {
      builder.add(line.from, line.from, lineMark);
      last = line.from;
    }
  }
  return builder.finish();
}

const currentLine = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = currentLines(view);
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet) {
        this.decorations = currentLines(update.view);
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

/**
 * The looks: the current line a shade of the text colour over the
 * background (more of it in dark themes, `--ui-current-line-mix` of
 * `theme.ts`); a matching pair outlined in the link colour; a bracket
 * without a counterpart bold and underlined with a thick wave in the colour
 * of found places, so it is told by its shape too, not only by its colour.
 */
const looks = EditorView.theme({
  [`.${CURRENT_LINE}`]: {
    backgroundColor:
      'color-mix(in srgb, var(--palette-foreground) var(--ui-current-line-mix, 7%), transparent)',
  },
  '.cm-matchingBracket, &.cm-focused .cm-matchingBracket': {
    backgroundColor: 'color-mix(in srgb, var(--palette-link) 22%, transparent)',
    outline: '1px solid var(--palette-link)',
  },
  '.cm-nonmatchingBracket, &.cm-focused .cm-nonmatchingBracket': {
    backgroundColor: 'color-mix(in srgb, var(--palette-found) 25%, transparent)',
    textDecoration: 'underline wavy var(--palette-found) 2px',
    fontWeight: 'bold',
  },
});

/** FR-084, FR-085: the current line and the matching bracket, for the source view. */
export function lineAndBrackets(): Extension {
  return [currentLine, bracketMatching(), looks];
}
