// FR-098, UC-005 BR-008, OQ-287: while the cursor is in an inline formula,
// the facade shows its markup in its place and, in a small popover above
// it, the formula typeset. The picture comes from the formula worker of the
// backend, as for every formula (ADR-029); the preview only shows what the
// facade's views already hold, so it is never on the key path:
//
// - It follows the changes debounced: a new state shows once the formula
//   stood still for `PREVIEW_DEBOUNCE_MS`, so typing does not flicker
//   between pictures, errors and the wait.
// - While a changed formula is typeset, the last picture stays, dimmed.
// - With an error it shows the reason; the reason after the markup in the
//   text stays what screen readers hear (the popover is `aria-hidden`).

import type { BlockView, FormulaView } from '../bindings';
import { inFormula } from './model';

/** How long a formula stands still before the preview follows it, in ms. */
export const PREVIEW_DEBOUNCE_MS = 150;

/** The space between the formula's markup and the popover, in px. */
export const PREVIEW_GAP = 4;

/** What the preview shows. */
export type PreviewShown =
  | {
      kind: 'picture';
      /** The picture's path of the app's own protocol. */
      picture: string;
      /** Its size in thousandths of an em (`FormulaView`). */
      width: number;
      height: number;
      /** The picture is of an earlier state of the formula: a newer one is typeset. */
      stale: boolean;
    }
  | {
      kind: 'error';
      /** UC-004 A21: why the formula cannot be typeset. */
      reason: string;
    }
  | {
      /** Nothing typeset yet. */
      kind: 'waiting';
    };

/** The inline formula the cursor is in: its block and run, and the formula. */
export type EditedFormula = {
  block: number;
  run: number;
  formula: FormulaView;
};

/**
 * OQ-287: the inline formula of block `index` the cursor `cursor` (a web
 * offset) is in, whose markup the facade shows; `null` when there is none.
 */
export function editedFormula(
  block: BlockView | undefined,
  index: number,
  cursor: number,
): EditedFormula | null {
  if (!block) {
    return null;
  }
  for (let run = 0; run < block.runs.length; run++) {
    const each = block.runs[run];
    if (each.formula && inFormula(block, each, cursor)) {
      return { block: index, run, formula: each.formula };
    }
  }
  return null;
}

/**
 * OQ-287: what the preview shows of `formula` now, after it showed
 * `before`: its picture; the reason of its error; or, while it is typeset,
 * the last picture, marked stale.
 */
export function previewOf(formula: FormulaView, before: PreviewShown | null): PreviewShown {
  if (formula.picture !== null) {
    return {
      kind: 'picture',
      picture: formula.picture,
      width: formula.width,
      height: formula.height,
      stale: false,
    };
  }
  if (formula.error !== null) {
    return { kind: 'error', reason: formula.error };
  }
  if (before?.kind === 'picture') {
    return { ...before, stale: true };
  }
  return { kind: 'waiting' };
}

/** A box on the screen, as `getBoundingClientRect` gives it. */
export type Box = { left: number; top: number; right: number; bottom: number };

/** Where the popover stands within the facade, in px from its top left. */
export type PreviewPlace = { left: number; top: number; below: boolean };

/**
 * OQ-287: where the popover of size `popup` stands for the formula's markup
 * at `markup` in the facade `host` (both on the screen): above the markup,
 * its left edge at the markup's, kept within the facade; below the markup
 * when there is no room above it.
 */
export function placePreview(
  markup: Box,
  host: Box,
  popup: { width: number; height: number },
): PreviewPlace {
  const room = markup.top - host.top;
  const below = room < popup.height + PREVIEW_GAP;
  const top = below
    ? markup.bottom - host.top + PREVIEW_GAP
    : markup.top - host.top - popup.height - PREVIEW_GAP;
  const widest = host.right - host.left - popup.width;
  const left = Math.max(0, Math.min(markup.left - host.left, widest));
  return { left, top, below };
}
