// UC-004 BR-018 (FR-096): the pane the user works in, the active pane of
// the split view. What acts on "the note shown" (the find field, go to
// line, the toolbar, the outline's jumps, link completion…) acts on the
// view of this pane; without the split view it is the left one.

import type { PaneName } from './bindings';

let focused: PaneName = 'left';

/** The active pane. */
export function focusedPane(): PaneName {
  return focused;
}

/** The active pane is `pane` from now on (the shell follows the backend's tab bar). */
export function setFocusedPane(pane: PaneName): void {
  focused = pane;
}
