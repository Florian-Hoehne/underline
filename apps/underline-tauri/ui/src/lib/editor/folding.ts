// M-08.8 (UC-004 BR-012): the folded sections of the source view, as the
// editor core reports them (`views`, ADR-025). The core owns what is
// folded; this module only shows it:
//
// - a folded section hides its body, and its heading ends on "…" in the
//   colour of the line numbers;
// - the gutter on the left shows the arrow of a heading while the pointer
//   is on its line or while it is folded; a click asks the backend to fold
//   or expand it;
// - the headings are kept for the minimap (M-12.2).
//
// Between two reports the folds and headings move along with the changes.

import {
  RangeSet,
  RangeSetBuilder,
  StateEffect,
  StateField,
  type Extension,
  type Text,
} from '@codemirror/state';
import {
  Decoration,
  EditorView,
  GutterMarker,
  ViewPlugin,
  WidgetType,
  gutter,
  type DecorationSet,
} from '@codemirror/view';
import type { SectionView } from '../bindings';

/** A heading of the note, at the start of its line. */
class Heading extends GutterMarker {
  constructor(readonly folded: boolean) {
    super();
  }

  eq(other: GutterMarker): boolean {
    return other instanceof Heading && other.folded === this.folded;
  }
}

/** The sections of the text, as the backend reported them. */
export type Sections = {
  /** The headings, by the start of their line. */
  headings: RangeSet<Heading>;
  /** The folded bodies, from the end of the heading to the end of the body. */
  folds: DecorationSet;
};

/** The sections the backend reported, for the text the view shows. */
export const setSections = StateEffect.define<readonly SectionView[]>();

/** The line under the pointer, by its start; `null` when none. */
const setHover = StateEffect.define<number | null>();

/** "…" after a folded heading. */
class Ellipsis extends WidgetType {
  eq(): boolean {
    return true;
  }

  toDOM(): HTMLElement {
    const dom = document.createElement('span');
    dom.className = 'cm-ul-folded';
    dom.textContent = '…';
    // Screen readers read the mark (NFR-011); egui has no text for it.
    dom.setAttribute('role', 'img');
    dom.setAttribute('aria-label', 'folded');
    return dom;
  }
}

const folded = Decoration.replace({ widget: new Ellipsis() });

/** The sections `views` in `doc`. */
export function sectionsOf(doc: Text, views: readonly SectionView[]): Sections {
  const headings = new RangeSetBuilder<Heading>();
  const folds = new RangeSetBuilder<Decoration>();
  for (const section of views) {
    if (section.line >= doc.lines) {
      continue;
    }
    const heading = doc.line(section.line + 1);
    const isFolded = section.folded && section.last > section.line;
    headings.add(heading.from, heading.from, new Heading(isFolded));
    if (isFolded) {
      const last = doc.line(Math.min(section.last, doc.lines - 1) + 1);
      if (last.to > heading.to) {
        folds.add(heading.to, last.to, folded);
      }
    }
  }
  return { headings: headings.finish(), folds: folds.finish() };
}

/** The sections of the text, moved along with every change. */
export const sectionsField = StateField.define<Sections>({
  create: () => ({ headings: RangeSet.empty, folds: Decoration.none }),
  update(value, tr) {
    let next = tr.docChanged
      ? { headings: value.headings.map(tr.changes), folds: value.folds.map(tr.changes) }
      : value;
    for (const effect of tr.effects) {
      if (effect.is(setSections)) {
        next = sectionsOf(tr.state.doc, effect.value);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field, (sections) => sections.folds),
});

/** The line under the pointer. */
const hoverField = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setHover)) {
        value = effect.value;
      }
    }
    return value;
  },
});

/** The line of a heading the user clicked the arrow of, counted from 0. */
export type FoldRequest = (line: number) => void;

/** The arrow of a heading: pointing right when folded, down when not. */
class Arrow extends GutterMarker {
  constructor(
    readonly isFolded: boolean,
    readonly shown: boolean,
  ) {
    super();
  }

  eq(other: GutterMarker): boolean {
    return other instanceof Arrow && other.isFolded === this.isFolded && other.shown === this.shown;
  }

  toDOM(): Node {
    const dom = document.createElement('span');
    dom.className = 'cm-ul-arrow';
    dom.dataset.folded = String(this.isFolded);
    dom.style.visibility = this.shown ? 'visible' : 'hidden';
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('viewBox', '-5 -5 10 10');
    svg.setAttribute('width', '10');
    svg.setAttribute('height', '10');
    svg.setAttribute('aria-hidden', 'true');
    const shape = document.createElementNS(ns, 'polygon');
    // As egui paints it (`paint_fold_arrow`), radius 4.
    shape.setAttribute('points', this.isFolded ? '-2.4,-4 3.6,0 -2.4,4' : '-4,-2.4 4,-2.4 0,3.6');
    svg.appendChild(shape);
    dom.appendChild(svg);
    return dom;
  }
}

/** The heading at the start of `from`, if any. */
function headingAt(sections: Sections, from: number): Heading | null {
  let found: Heading | null = null;
  sections.headings.between(from, from, (start, _end, heading) => {
    if (start === from) {
      found = heading;
    }
  });
  return found;
}

/**
 * Tracks the line under the pointer, for the arrows: over the text and
 * over the gutter, so the arrow stays while the pointer goes to it.
 */
const hoverTracker = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {
      view.dom.addEventListener('mousemove', this.move);
      view.dom.addEventListener('mouseleave', this.leave);
    }

    move = (event: MouseEvent) => {
      const view = this.view;
      const block = view.lineBlockAtHeight(event.clientY - view.documentTop);
      const inside = event.clientY >= view.documentTop && block.to <= view.state.doc.length;
      this.set(inside ? block.from : null);
    };

    leave = () => this.set(null);

    set(from: number | null) {
      if (this.view.state.field(hoverField) !== from) {
        this.view.dispatch({ effects: setHover.of(from) });
      }
    }

    destroy() {
      this.view.dom.removeEventListener('mousemove', this.move);
      this.view.dom.removeEventListener('mouseleave', this.leave);
    }
  },
);

/**
 * The folding of the source view: the folded bodies, the gutter of arrows
 * and the line under the pointer; `fold` asks the backend to fold or
 * expand the section of a heading.
 */
export function folding(fold: FoldRequest): Extension {
  return [
    sectionsField,
    hoverField,
    hoverTracker,
    gutter({
      class: 'cm-ul-fold-gutter',
      lineMarker(view, line) {
        const heading = headingAt(view.state.field(sectionsField), line.from);
        if (!heading) {
          return null;
        }
        const hovered = view.state.field(hoverField) === line.from;
        return new Arrow(heading.folded, heading.folded || hovered);
      },
      lineMarkerChange: (update) =>
        update.startState.field(sectionsField) !== update.state.field(sectionsField) ||
        update.startState.field(hoverField) !== update.state.field(hoverField),
      initialSpacer: () => new Arrow(false, false),
      domEventHandlers: {
        mousedown(view, line, event) {
          if (!headingAt(view.state.field(sectionsField), line.from)) {
            return false;
          }
          event.preventDefault();
          fold(view.state.doc.lineAt(line.from).number - 1);
          return true;
        },
      },
    }),
  ];
}

/** The lines of the headings, counted from 0, for the minimap (M-12.2). */
export function headingLines(view: EditorView): Set<number> {
  const lines = new Set<number>();
  const doc = view.state.doc;
  for (let cursor = view.state.field(sectionsField).headings.iter(); cursor.value; cursor.next()) {
    if (cursor.from <= doc.length) {
      lines.add(doc.lineAt(cursor.from).number - 1);
    }
  }
  return lines;
}

/** The folded lines under the headings, `[first, last]` from 0, for the minimap. */
export function foldedLines(view: EditorView): [number, number][] {
  const doc = view.state.doc;
  const lines: [number, number][] = [];
  for (let cursor = view.state.field(sectionsField).folds.iter(); cursor.value; cursor.next()) {
    const first = doc.lineAt(Math.min(cursor.from, doc.length)).number;
    const last = doc.lineAt(Math.min(cursor.to, doc.length)).number - 1;
    if (last >= first) {
      lines.push([first, last]);
    }
  }
  return lines;
}
