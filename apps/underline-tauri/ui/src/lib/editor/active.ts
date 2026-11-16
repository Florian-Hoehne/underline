// The source view shown now in each pane of the split view (UC-004 BR-018),
// for the window and the measuring driver (bench.ts): what acts on the
// active editor acts on the one of the active pane (`paneFocus.ts`).

import type { PaneName } from '../bindings';
import { focusedPane } from '../paneFocus';
import type { SourceView } from './sourceView';

const shown = new Map<PaneName, SourceView>();

/** The source view shown now in the active pane, if any. */
export function activeSourceView(): SourceView | null {
  return shown.get(focusedPane()) ?? null;
}

/** FR-096: the source views shown now, in every pane. */
export function shownSourceViews(): SourceView[] {
  return [...shown.values()];
}

/**
 * The source view `view` is shown now in the pane `pane` (`null`: none);
 * with `was`, only when that one is still the pane's (a view going does not
 * clear the one that came after it).
 */
export function setActiveSourceView(
  view: SourceView | null,
  pane: PaneName = 'left',
  was: SourceView | null = null,
): void {
  if (view === null) {
    if (was === null || shown.get(pane) === was) {
      shown.delete(pane);
    }
  } else {
    shown.set(pane, view);
  }
}
