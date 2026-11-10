// Where the keyboard goes when the element that had it is gone, such as a
// closed dialog or tab (ux review F2): the first note of the overview, else
// the overview itself. Without a workspace nothing is focused.

/** Moves the keyboard to the note overview. */
export function focusFallback(): void {
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
