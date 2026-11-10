// The facade shown now, for the window (the switch of the view, the
// toolbar) and for later features that jump in it (M-09.16: the tool
// panel's outline reveals a block). Its blocks come from the window's editor
// core (ADR-025), not from the backend's events.

/** What the window asks of the facade shown. */
export interface ActiveFacade {
  readonly note: string;
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
