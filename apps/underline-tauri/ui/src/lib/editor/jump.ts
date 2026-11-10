// M-07.3, UC-004 A2: a link led to a heading: the cursor of the note's
// view goes there, the view shows it and takes the keyboard (egui's
// `jump_to_heading`). The offset is in the backend's text of a revision;
// the view jumps only while its text is still that one (else nothing
// moves). The note may be in a tab that is only opening: the jump then
// waits until its view shows (`takeJump`), but not for long.

import { activeFacade } from '../facade/active';
import { activeSourceView } from './active';

/** How long a jump waits for the view of its note, in ms. */
const WAITS_MS = 3000;

/** A jump: the place, in web units of the backend's text of `rev`. */
export type Jump = { offset: number; rev: number };

let waiting: { note: string; jump: Jump; until: number } | null = null;

/** M-07.3: the cursor of `note` to `offset` of revision `rev`, in the view that shows it. */
export function jumpTo(note: string, rev: number, offset: number): void {
  waiting = null;
  const facade = activeFacade();
  if (facade?.note === note) {
    facade.jump(offset, rev);
    return;
  }
  const source = activeSourceView();
  if (source?.note === note) {
    source.jump(offset, rev);
    return;
  }
  waiting = { note, jump: { offset, rev }, until: performance.now() + WAITS_MS };
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
