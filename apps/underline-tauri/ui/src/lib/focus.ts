// Where the keyboard goes when the element that had it is gone, such as a
// closed dialog or tab (ux review F2): the first note of the overview, else
// the overview itself. Without a workspace nothing is focused.

/** Where the keyboard goes once instead of the overview's first note. */
let hint: (() => HTMLElement | null | undefined) | null = null;

/**
 * The next `focusFallback` asks `where` first (UX review F8: after a
 * deleted note was removed, the keyboard goes to the next one).
 */
export function setFocusHint(where: (() => HTMLElement | null | undefined) | null): void {
  hint = where;
}

/**
 * Moves the keyboard to the entry of `note` in the note overview, once it
 * shows (at most about a second).
 */
export async function focusNoteEntry(note: string): Promise<void> {
  for (let tries = 0; tries < 20; tries += 1) {
    const entry = document.querySelector<HTMLElement>(
      `[data-focus-fallback] button.entry[data-note="${CSS.escape(note)}"]`,
    );
    if (entry) {
      entry.focus();
      return;
    }
    await new Promise((done) => setTimeout(done, 50));
  }
}

/** Moves the keyboard to the note overview. */
export function focusFallback(): void {
  const hinted = hint?.();
  hint = null;
  if (hinted?.isConnected) {
    hinted.focus();
    return;
  }
  const overview = document.querySelector<HTMLElement>('[data-focus-fallback]');
  const first = overview?.querySelector<HTMLElement>('button.entry');
  (first ?? overview)?.focus();
}

/**
 * NFR-011 (A-33): F6 and Shift+F6 move the keyboard between the regions of
 * the note area, also out of the text in the Vim and Nano modes, where
 * Escape and Tab stay in it. `regions` are the elements to go to, in order
 * (absent ones left out); `current` is the index of the region holding the
 * keyboard, -1 for none. Gives the index to go to.
 */
export function nextRegion(count: number, current: number, back: boolean): number {
  if (count === 0) {
    return -1;
  }
  if (current < 0) {
    return back ? count - 1 : 0;
  }
  return (current + (back ? count - 1 : 1)) % count;
}
