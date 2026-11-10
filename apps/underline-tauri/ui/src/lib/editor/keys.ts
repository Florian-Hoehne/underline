// M-08.4, M-10.1: which keys of the source view go to the editor in the
// backend (ADR-024). The backend decides what each does, as the egui
// editor does; this table only says which keys are the editor's, so the
// webview's own handling of them is stopped. Every other key keeps its way:
// text comes as `beforeinput`, and shortcuts of the window (Ctrl+N …) reach
// the window.

import type { EditorInput, KeyName } from '../bindings';

/** Keys the editor takes with any modifier (no moving or deleting by words). */
const ALWAYS: Readonly<Record<string, KeyName>> = {
  Backspace: 'Backspace',
  Delete: 'Delete',
  Enter: 'Enter',
  Tab: 'Tab',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
};

/** Keys the editor takes with the command key: select all, undo, redo. */
const WITH_COMMAND: Readonly<Record<string, KeyName>> = { a: 'A', y: 'Y', z: 'Z' };

/** The parts of a key event the editor reads. */
export type KeyEventLike = Pick<
  KeyboardEvent,
  'key' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'isComposing'
>;

/**
 * The editor's input for the key event `event`, or `null` when the key is
 * not the editor's. The command key is Cmd on macOS (`mac`) and Ctrl
 * elsewhere; `page` is how many lines the view shows.
 */
export function keyInput(event: KeyEventLike, mac: boolean, page: number): EditorInput | null {
  if (event.isComposing) {
    return null;
  }
  const command = mac ? event.metaKey : event.ctrlKey;
  const key = ALWAYS[event.key] ?? (command ? WITH_COMMAND[event.key.toLowerCase()] : undefined);
  if (key === undefined) {
    return null;
  }
  return {
    kind: 'key',
    key,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    page: Math.max(1, Math.floor(page)),
  };
}
