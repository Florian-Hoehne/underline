// OQ-287, UC-005 BR-008, TC-005-42 step 4: the typeset preview of the inline
// formula the cursor is in.

import { cleanup, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { BlockView, FormulaView } from '../bindings';
import FormulaPreview from './FormulaPreview.svelte';
import {
  PREVIEW_DEBOUNCE_MS,
  PREVIEW_GAP,
  editedFormula,
  placePreview,
  previewOf,
  type EditedFormula,
} from './formulaPreview';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function formula(picture: string | null, error: string | null = null): FormulaView {
  return {
    source: 'x^2',
    display: false,
    picture,
    width: 1500,
    height: 1100,
    depth: 200,
    error,
  };
}

/** `See $x^2$ here.` with the formula as its second run. */
function paragraph(shown: FormulaView): BlockView {
  const plain = (text: string, at: number) => ({
    text,
    bold: false,
    italic: false,
    strike: false,
    code: false,
    link: null,
    offsets: [[0, at]] as [number, number][],
    end: at + text.length,
    formula: null,
  });
  return {
    kind: { type: 'paragraph' },
    start: 0,
    line: 0,
    length: 16,
    lines: 1,
    prefix: '',
    runs: [plain('See ', 0), { ...plain('$x^2$', 4), formula: shown }, plain(' here.', 9)],
    colors: null,
    figure: null,
  };
}

describe('OQ-287 the formula the cursor is in', () => {
  it('is the inline formula whose markup holds the cursor, after its first character', () => {
    const block = paragraph(formula('formula/a.svg'));
    expect(editedFormula(block, 3, 6)).toEqual({
      block: 3,
      run: 1,
      formula: block.runs[1].formula,
    });
    expect(editedFormula(block, 3, 4)).toBeNull();
    expect(editedFormula(block, 3, 9)).toBeNull();
    expect(editedFormula(undefined, 0, 6)).toBeNull();
  });
});

describe('OQ-287 what the preview shows', () => {
  it('the picture, the reason of an error, and the last picture while a change is typeset', () => {
    const picture = previewOf(formula('formula/a.svg'), null);
    expect(picture).toEqual({
      kind: 'picture',
      picture: 'formula/a.svg',
      width: 1500,
      height: 1100,
      stale: false,
    });
    expect(previewOf(formula(null), picture)).toEqual({ ...picture, stale: true });
    expect(previewOf(formula(null), null)).toEqual({ kind: 'waiting' });
    const failed = previewOf(formula(null, 'Expected group'), picture);
    expect(failed).toEqual({ kind: 'error', reason: 'Expected group' });
    expect(previewOf(formula(null), failed)).toEqual({ kind: 'waiting' });
  });
});

describe('OQ-287 where the preview stands', () => {
  const host = { left: 100, top: 50, right: 500, bottom: 650 };
  const popup = { width: 120, height: 40 };

  it('above the markup, its left edge at the markup', () => {
    const place = placePreview({ left: 180, top: 300, right: 230, bottom: 318 }, host, popup);
    expect(place).toEqual({ left: 80, top: 300 - 50 - 40 - PREVIEW_GAP, below: false });
  });

  it('below the markup at the top of the facade, and within its width', () => {
    const place = placePreview({ left: 470, top: 60, right: 495, bottom: 78 }, host, popup);
    expect(place).toEqual({ left: 400 - 120, top: 78 - 50 + PREVIEW_GAP, below: true });
    expect(placePreview({ left: 80, top: 300, right: 90, bottom: 318 }, host, popup).left).toBe(0);
  });
});

describe('OQ-287 the popover follows the formula, debounced', () => {
  it('shows at once, follows a change once it stood still, and goes when the cursor leaves', async () => {
    vi.useFakeTimers();
    const edited = (shown: FormulaView): EditedFormula => ({ block: 0, run: 1, formula: shown });
    const view = render(FormulaPreview, {
      props: {
        edited: edited(formula('formula/a.svg')),
        content: null,
        platform: 'linux',
        moved: 0,
      },
    });
    const popover = () => document.querySelector<HTMLElement>('[data-formula-preview]');
    expect(popover()?.querySelector('img')?.getAttribute('src')).toBe(
      'ul-asset://localhost/formula/a.svg',
    );
    expect(popover()).toHaveAttribute('aria-hidden', 'true');

    // Typed on: the new formula is typeset; the last picture stays meanwhile.
    await view.rerender({ edited: edited(formula(null)) });
    expect(popover()?.querySelector('img')).not.toHaveClass('stale');
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    expect(popover()?.querySelector('img')).toHaveClass('stale');

    // A change within the wait starts it again.
    await view.rerender({ edited: edited(formula(null, 'Expected group')) });
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS / 2);
    await view.rerender({ edited: edited(formula('formula/b.svg')) });
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS / 2);
    expect(popover()?.querySelector('img')?.getAttribute('src')).toContain('formula/a.svg');
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    expect(popover()?.querySelector('img')?.getAttribute('src')).toContain('formula/b.svg');

    await view.rerender({ edited: edited(formula(null, 'Expected group')) });
    await vi.advanceTimersByTimeAsync(PREVIEW_DEBOUNCE_MS);
    expect(popover()).toHaveTextContent('Expected group');

    await view.rerender({ edited: null });
    expect(popover()).toBeNull();
  });
});
