// Where the keyboard goes when the element that had it is gone, such as a
// closed dialog or tab (ux review F2): the first note of the overview, else
// the overview itself. Without a workspace nothing is focused.

/** Moves the keyboard to the note overview. */
export function focusFallback(): void {
  const overview = document.querySelector<HTMLElement>('[data-focus-fallback]');
  const first = overview?.querySelector<HTMLElement>('button.entry');
  (first ?? overview)?.focus();
}
