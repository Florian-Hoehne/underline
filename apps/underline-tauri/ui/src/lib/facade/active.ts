// The facade shown now, for the window's events (`ed-blocks`,
// `ed-sections`, `ed-replaced`), which the shell listens to once for the
// whole window, and for later features that jump in it (M-09.16: the tool
// panel's outline reveals a block).

import type { BlocksView, SectionsView } from '../bindings';

/** What the window asks of the facade shown. */
export interface ActiveFacade {
  readonly note: string;
  /** ADR-024: blocks changed. */
  takeBlocks(change: BlocksView): void;
  /** M-08.8: the sections with what is folded. */
  takeSections(sections: SectionsView): void;
  /** UC-004 A7: the text was replaced; shown again unless it is this revision. */
  replaced(rev: number): void;
  /** M-09.16: scrolls the block at the markup offset `offset` to the top, without moving the cursor. */
  reveal(offset: number): void;
  focus(): void;
}

let active: ActiveFacade | null = null;

/** The facade shown now, if any. */
export function activeFacade(): ActiveFacade | null {
  return active;
}

/** The facade `facade` is shown now (`null`: none). */
export function setActiveFacade(facade: ActiveFacade | null): void {
  active = facade;
}
