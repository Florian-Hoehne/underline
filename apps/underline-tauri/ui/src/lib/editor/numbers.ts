// M-08.7 (FR-028, FR-029): the line numbers of the source view, on or off,
// on the left or on the right, as the settings say. The width follows the
// count of digits; the colour is 45 % of the text colour (as egui's
// `gamma_multiply(0.45)`). A folded section shows the number of its
// heading only.
//
// On the left they are a CodeMirror gutter after the fold arrows. On the
// right, CodeMirror has no gutter, so they are a layer of their own beside
// the text, which leaves room for them.

import type { Extension } from '@codemirror/state';
import { EditorView, layer, lineNumbers, type LayerMarker } from '@codemirror/view';

/** Where the line numbers go: "Left" or "Right" (the settings' names). */
export type LineNumberSide = 'Left' | 'Right';

/** egui leaves 6 px between the numbers and the text. */
const PADDING = 6;

/** One line number on the right, right-aligned at `right`. */
class RightNumber implements LayerMarker {
  constructor(
    readonly text: string,
    readonly left: number,
    readonly top: number,
    readonly width: number,
  ) {}

  eq(other: LayerMarker): boolean {
    return (
      other instanceof RightNumber &&
      other.text === this.text &&
      other.left === this.left &&
      other.top === this.top &&
      other.width === this.width
    );
  }

  draw(): HTMLElement {
    const dom = document.createElement('div');
    dom.className = 'cm-ul-number';
    this.update(dom);
    return dom;
  }

  update(dom: HTMLElement): boolean {
    dom.textContent = this.text;
    dom.style.left = `${this.left}px`;
    dom.style.top = `${this.top}px`;
    dom.style.width = `${this.width}px`;
    return true;
  }
}

/** The count of digits of the last line number, at least 1. */
export function digitsOf(lines: number): number {
  return String(Math.max(1, lines)).length;
}

/**
 * Keeps room on the right of the text for the numbers. CodeMirror owns the
 * text field's attributes, so the room is one of them.
 */
const rightRoom = EditorView.contentAttributes.compute(['doc'], (state) => ({
  style: `padding-right: calc(${digitsOf(state.doc.lines)}ch + ${PADDING}px)`,
}));

/** The numbers on the right of the text. */
const rightNumbers = layer({
  above: true,
  class: 'cm-ul-numbers-right',
  update: (update) => update.docChanged || update.viewportChanged || update.geometryChanged,
  markers(view) {
    const scroller = view.scrollDOM.getBoundingClientRect();
    const content = view.contentDOM.getBoundingClientRect();
    const baseLeft = scroller.left - view.scrollDOM.scrollLeft;
    const baseTop = scroller.top - view.scrollDOM.scrollTop;
    const width = digitsOf(view.state.doc.lines) * view.defaultCharacterWidth;
    const left = content.right - baseLeft - PADDING - width;
    return view.viewportLineBlocks.map(
      (block) =>
        new RightNumber(
          String(view.state.doc.lineAt(block.from).number),
          left,
          view.documentTop + block.top - baseTop,
          width,
        ),
    );
  },
});

/** The line numbers: none (`shown` off), on the left or on the right. */
export function lineNumbersOn(shown: boolean, side: LineNumberSide): Extension {
  if (!shown) {
    return [];
  }
  return side === 'Right' ? [rightRoom, rightNumbers] : lineNumbers();
}
