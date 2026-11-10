// M-08.4, M-10.1, M-10.2, M-08.8, M-11: which keys of the source view go to
// the editor in the backend (ADR-024). The backend decides what each does,
// as the egui editor does; this table only says which keys are the
// editor's, so the webview's own handling of them is stopped. Every other
// key keeps its way: text comes as `beforeinput`, and shortcuts of the
// window (Ctrl+N …) reach the window.
//
// - Normal: the editing keys, Ctrl/Cmd+A/Y/Z, the toolbar keys (Ctrl+B …)
//   and the folding keys (Ctrl/Cmd+Shift+[ and ]).
// - Nano (M-11.3): the same, and Escape, Ctrl+K/U/W/O/S/X/A/E, Alt+U/E
//   (Cmd counts as Ctrl on macOS); while the search prompt is open every
//   key of the table and Ctrl+C go to the backend, which takes them for the
//   prompt.
// - Vim (A-01): `codemirror-vim` takes its keys first. What it leaves goes
//   here: in insert mode as in Normal; in the other modes only the toolbar
//   and folding keys, and the editing keys are swallowed so that Tab, for
//   example, never leaves the text (M-08.10).
// - Link (Ctrl+K) and Image (Ctrl+Shift+I) ask first; their dialogs come
//   with feature F6, so until then their keys do nothing (Q-14). Nano's
//   Ctrl+K still cuts the line.

import type { EditorInput, KeyName } from '../bindings';

/** The input mode of the settings: "Normal", "Vim" or "Nano". */
export type InputModeName = 'Normal' | 'Vim' | 'Nano';

/** What the key routing needs to know of the input mode. */
export type KeysState = {
  mode: InputModeName;
  /** Vim is in insert mode. */
  vimInsert?: boolean;
  /** The Nano search prompt is open. */
  prompting?: boolean;
};

/** What to do with a key. */
export type KeyRoute =
  /** Send it to the backend. */
  | { send: EditorInput }
  /** Stop it; nothing happens (Vim's normal mode, M-08.10). */
  | 'swallow'
  /** Leave it to the webview and the window. */
  | null;

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
const WITH_COMMAND: ReadonlySet<string> = new Set(['A', 'Y', 'Z']);

/** M-10.2: the toolbar keys with the command key, without and with Shift. */
const TOOL_KEYS: ReadonlySet<string> = new Set(['B', 'I', 'E', 'K']);
const TOOL_SHIFT_KEYS: ReadonlySet<string> = new Set(['H', 'L', 'Q', 'I', 'T']);

/** M-11.3: the Nano keys with Ctrl (Cmd on macOS) and with Alt. */
const NANO_CTRL_KEYS: ReadonlySet<string> = new Set(['K', 'U', 'W', 'O', 'S', 'X', 'A', 'E']);
const NANO_ALT_KEYS: ReadonlySet<string> = new Set(['U', 'E']);

const LETTERS: ReadonlySet<string> = new Set([
  ...WITH_COMMAND,
  ...TOOL_KEYS,
  ...TOOL_SHIFT_KEYS,
  ...NANO_CTRL_KEYS,
  'C',
]);

/** The parts of a key event the editor reads. */
export type KeyEventLike = Pick<
  KeyboardEvent,
  'key' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'isComposing'
> & { code?: string };

/**
 * The letter of `event` in capitals, whatever the layout and the modifiers
 * made of it (Alt+U is "¨" on macOS); `null` for other keys.
 */
function letterOf(event: KeyEventLike): string | null {
  if (/^[a-z]$/i.test(event.key)) {
    return event.key.toUpperCase();
  }
  const code = /^Key([A-Z])$/.exec(event.code ?? '');
  return code ? code[1] : null;
}

/** M-08.8: `[` or `]` with Shift, whatever the layout makes of it. */
function bracketOf(event: KeyEventLike): KeyName | null {
  if (event.key === '[' || event.key === '{' || event.code === 'BracketLeft') {
    return 'BracketLeft';
  }
  if (event.key === ']' || event.key === '}' || event.code === 'BracketRight') {
    return 'BracketRight';
  }
  return null;
}

function input(event: KeyEventLike, key: KeyName, page: number): EditorInput {
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

/**
 * Where the key event `event` goes in the input mode `state`. The command
 * key is Cmd on macOS (`mac`) and Ctrl elsewhere; `page` is how many lines
 * the view shows.
 */
export function routeKey(
  event: KeyEventLike,
  mac: boolean,
  page: number,
  state: KeysState = { mode: 'Normal' },
): KeyRoute {
  if (event.isComposing) {
    return null;
  }
  const command = mac ? event.metaKey : event.ctrlKey;
  const send = (key: KeyName): KeyRoute => ({ send: input(event, key, page) });
  const always = ALWAYS[event.key];
  const letter = letterOf(event);
  const bracket = command && event.shiftKey ? bracketOf(event) : null;
  const tool =
    command &&
    letter !== null &&
    (event.shiftKey ? TOOL_SHIFT_KEYS.has(letter) : TOOL_KEYS.has(letter));
  const asksFirst = tool && (event.shiftKey ? letter === 'I' : letter === 'K');
  const toolRoute = (): KeyRoute => (asksFirst ? 'swallow' : send(letter as KeyName));

  if (state.mode === 'Nano') {
    const ctrl = event.ctrlKey || (mac && event.metaKey);
    if (state.prompting) {
      if (always || event.key === 'Escape') {
        return send(always ?? 'Escape');
      }
      return ctrl && letter === 'C' ? send('C') : null;
    }
    if (event.key === 'Escape') {
      return send('Escape');
    }
    if (letter !== null && LETTERS.has(letter)) {
      const nano = ctrl ? NANO_CTRL_KEYS.has(letter) : event.altKey && NANO_ALT_KEYS.has(letter);
      if (nano) {
        return send(letter as KeyName);
      }
      if (tool) {
        return toolRoute();
      }
      if (command && WITH_COMMAND.has(letter)) {
        return send(letter as KeyName);
      }
    }
  }

  if (state.mode === 'Vim' && !state.vimInsert) {
    if (bracket) {
      return send(bracket);
    }
    if (tool) {
      return toolRoute();
    }
    return always || event.key === 'Escape' ? 'swallow' : null;
  }

  if (always) {
    return send(always);
  }
  if (bracket) {
    return send(bracket);
  }
  if (tool) {
    return toolRoute();
  }
  if (letter !== null && command && WITH_COMMAND.has(letter)) {
    return send(letter as KeyName);
  }
  if (state.mode === 'Vim' && event.key === 'Escape') {
    return 'swallow';
  }
  return null;
}

/**
 * The editor's input for the key event `event` in the Normal input mode, or
 * `null` when the key is not the editor's (feature F3's table).
 */
export function keyInput(event: KeyEventLike, mac: boolean, page: number): EditorInput | null {
  const route = routeKey(event, mac, page);
  return route !== null && route !== 'swallow' ? route.send : null;
}
