// The source view shown now, for the measuring driver (bench.ts) and for
// later features that act on the active editor.

import type { SourceView } from './sourceView';

let active: SourceView | null = null;

/** The source view shown now, if any. */
export function activeSourceView(): SourceView | null {
  return active;
}

/** The source view `view` is shown now (`null`: none). */
export function setActiveSourceView(view: SourceView | null): void {
  active = view;
}
