// Feature F6 (ADR-014, ADR-025): the system clipboard as both views use it.
// The backend reads and writes it (`arboard`); the window's editor core
// decides what a paste inserts and when a selection is copied, as the egui
// editor does. The webview's own clipboard is never used.
//
// - M-09.14, M-08.6: copying puts the markup on the clipboard, in the
//   facade with the HTML of the selection for other programs; a paste reads
//   the text, the HTML as markup of the note's format (the facade only),
//   and whether an image alone is there.
// - M-10.6, UC-005 A18: an image alone is stored next to the note by the
//   backend ("Pasted image stored as {name}") and inserted as a picture.
// - M-10.7, FR-031, BR-011: copy on select puts a finished selection on the
//   clipboard (after the mouse button is let go); the backend keeps what
//   the clipboard held before, and a paste over exactly that selection
//   brings it back. It follows the setting `copy_on_select`.

import {
  commands,
  type ClipContentView,
  type FormatName,
  type Insertion,
  type UiError,
} from '../bindings';
import type { TabCore } from './sync';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/**
 * What a copy, cut or paste gave a view: its patch (`null`: nothing to
 * show, the view stays as it is), and when the paste
 * found an image alone, whether it was over the selection copy on select
 * copied (the image is stored and inserted next, `pasteImage`).
 */
export type Clipboarded<P> = { patch: P | null; image: { over: boolean } | null };

/**
 * The clipboard's commands run one after the other, in the order the views
 * call them: a paste right after a selection reads what copy on select left.
 */
let chain: Promise<unknown> = Promise.resolve();

function queued<T>(work: () => Promise<T>): Promise<T> {
  const next = chain.then(work, work);
  chain = next.catch(() => undefined);
  return next;
}

/** M-10.7: whether copy on select is on (the settings; `NoteArea` keeps it). */
let copyOnSelectOn = true;

export function setCopyOnSelect(on: boolean): void {
  copyOnSelectOn = on;
}

/** The mouse buttons held down in the window; copy on select waits for them. */
let buttons = 0;
let watching = false;
/** Called once the buttons are let go. */
const released = new Set<() => void>();

function watchPointer(): void {
  if (watching || typeof window === 'undefined') {
    return;
  }
  watching = true;
  const take = (event: PointerEvent) => {
    const was = buttons;
    buttons = event.buttons;
    if (was !== 0 && buttons === 0) {
      for (const done of [...released]) {
        done();
      }
    }
  };
  window.addEventListener('pointerdown', take, true);
  window.addEventListener('pointerup', take, true);
  window.addEventListener('pointercancel', () => (buttons = 0), true);
  window.addEventListener('blur', () => (buttons = 0));
}

/** Whether a mouse button is down (a selection is still being made). */
export function pointerHeld(): boolean {
  watchPointer();
  return buttons !== 0;
}

/** Calls `done` whenever the mouse buttons are let go; returns the way to stop. */
export function onPointerReleased(done: () => void): () => void {
  watchPointer();
  released.add(done);
  return () => released.delete(done);
}

/**
 * What a paste in `tab` takes: what the clipboard holds, or `over` the
 * selection copy on select copied what it held before; with `rich` (the
 * facade, not Ctrl+Shift+V) the HTML as markup of the note's format.
 */
export function readForPaste(tab: TabCore, rich: boolean, over: boolean): Promise<ClipContentView> {
  const format: FormatName | null = rich ? tab.format : null;
  return queued(() => unwrap(commands.clipRead(tab.note, format, over)));
}

/** Puts copied markup (and the facade's HTML) on the clipboard. */
export async function writeCopy(
  note: string,
  text: string,
  html: string | null,
  first = false,
): Promise<void> {
  await queued(() => unwrap(commands.clipWrite(note, text, html, first)));
}

/**
 * M-10.7: after an input or a selection settled: the selection of `tab`
 * goes to the clipboard once, unless a mouse button is still down (the
 * caller tries again when it is let go).
 */
export async function copyOnSelect(tab: TabCore | undefined): Promise<void> {
  if (!tab) {
    return;
  }
  if (pointerHeld() || !tab.alive) {
    return;
  }
  const { copy, forget } = tab.core.copyOnSelect(copyOnSelectOn);
  if (copy) {
    await writeCopy(tab.note, copy.text, copy.html, copy.first);
  } else if (forget) {
    // BR-011: the backend lets go of what the clipboard held before.
    await queued(() => unwrap(commands.clipForget(tab.note)));
  }
}

/**
 * M-10.6, UC-005 A18: the backend stores the clipboard's image next to the
 * note; the picture to insert, or `null` (the status says why).
 */
export async function storePastedImage(
  tab: TabCore,
  overCopied: boolean,
): Promise<Insertion | null> {
  const name = await queued(() => unwrap(commands.clipStoreImage(tab.note, overCopied)));
  return name === null ? null : { kind: 'image', place: name };
}
