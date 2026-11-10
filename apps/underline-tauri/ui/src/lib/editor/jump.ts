// M-07.3, UC-004 A2: a link led to a heading: the cursor of the note's
// view goes there, the view shows it and takes the keyboard (egui's
// `jump_to_heading`). UC-008 step 4 (M-13.3): a found place opens with the
// word found selected, or, for a note found by its name only, the note's
// view just takes the keyboard. The offset is in the backend's text of a
// revision; the view jumps only while its text is still that one (else
// nothing moves). The note may be in a tab that is only opening: the jump
// then waits until its view shows (`takeJump`), but not for long.

import { activeFacade } from '../facade/active';
import { activeSourceView } from './active';

/** How long a jump waits for the view of its note, in ms. */
const WAITS_MS = 3000;

/**
 * A jump: the place, in web units of the backend's text of `rev`, and the
 * other end of the selection (`to`, UC-008 step 4); or, with `focusOnly`,
 * only the keyboard into the view.
 */
export type Jump = { offset: number; rev: number; to?: number; focusOnly?: boolean };

/** What a view does for a jump. */
export interface JumpTarget {
  jump(offset: number, rev?: number, to?: number): void;
  focus(): void;
}

let waiting: { note: string; jump: Jump; until: number } | null = null;

/** The view showing `note` now, if any. */
function viewOf(note: string): JumpTarget | null {
  const facade = activeFacade();
  if (facade?.note === note) {
    return facade;
  }
  const source = activeSourceView();
  return source?.note === note ? source : null;
}

/** Does `jump` in `view`. */
export function runJump(view: JumpTarget, jump: Jump): void {
  if (jump.focusOnly) {
    view.focus();
  } else if (jump.to === undefined) {
    view.jump(jump.offset, jump.rev);
  } else {
    view.jump(jump.offset, jump.rev, jump.to);
  }
}

function start(note: string, jump: Jump): void {
  waiting = null;
  const view = viewOf(note);
  if (view) {
    runJump(view, jump);
    return;
  }
  waiting = { note, jump, until: performance.now() + WAITS_MS };
}

/**
 * M-07.3, M-13.3: the cursor of `note` to `offset` of revision `rev`, in
 * the view that shows it; with `to`, the text from `offset` to `to` is
 * selected, the cursor at `to`.
 */
export function jumpTo(note: string, rev: number, offset: number, to?: number): void {
  start(note, { offset, rev, to });
}

/** M-13.3: the view of `note` takes the keyboard, once it shows. */
export function focusNote(note: string): void {
  start(note, { offset: 0, rev: 0, focusOnly: true });
}

/**
 * The view of `note` shows now: the jump that waits for it, if any. A jump
 * for another note is dropped, as the user went elsewhere.
 */
export function takeJump(note: string): Jump | null {
  const found = waiting;
  waiting = null;
  if (!found || found.note !== note || performance.now() > found.until) {
    return null;
  }
  return found.jump;
}
