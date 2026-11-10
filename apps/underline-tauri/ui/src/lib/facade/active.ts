// The facade shown now, for the window (the switch of the view, the
// toolbar) and for later features that jump in it (M-09.16: the tool
// panel's outline reveals a block). Its blocks come from the window's editor
// core (ADR-025), not from the backend's events.

import type { Insertion } from '../bindings';

/** M-10.5: where the cursor stood (web units) in the text of a hash. */
export type CursorMark = { head: number; hash: string };

/** What the window asks of the facade shown. */
export interface ActiveFacade {
  readonly note: string;
  /** M-09.16: scrolls the block at the markup offset `offset` to the top, without moving the cursor. */
  reveal(offset: number): void;
  focus(): void;
  /** M-10.3 to M-10.5: puts `insertion` in at the cursor; resolves once it is shown. */
  insert(insertion: Insertion): Promise<void>;
  /** M-10.5: the cursor to the place under the point (CSS pixels); false off the text. */
  placeAt(x: number, y: number): boolean;
  /** M-10.5: where the cursor is, once the inputs before are in; `null` once gone. */
  mark(): Promise<CursorMark | null>;
  /** M-10.5: the cursor back to `mark`, when the text did not change since. */
  back(mark: CursorMark): void;
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
