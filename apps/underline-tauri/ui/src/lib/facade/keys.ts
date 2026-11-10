// M-09.11 to M-09.13, M-10.1, M-10.2, M-08.8, M-11.2, M-11.3: which keys of
// the facade go to the backend (ADR-024), and which inputs of the webview
// (`beforeinput`) become which input of the facade. The backend decides what
// each does, as the egui facade does; this module only says which keys are
// the facade's, so the webview's own handling of them is stopped.
//
// - Normal: the editing keys, Ctrl/Cmd+A/Y/Z, the toolbar keys and the
//   folding keys. Escape leaves the text (M-08.10). Tab, Page Up and Page
//   Down do nothing (M-09.13).
// - Vim and Nano: the facade has the editor core's Vim and Nano keys (A-01:
//   not `codemirror-vim`), so every key of the table goes to the backend, and
//   Escape, Vim's Ctrl+R and Nano's Ctrl/Alt keys too; the keys decide what
//   they take.
// - Text, also Vim's commands such as `dd`, comes as `beforeinput`.
// - Up, Down, Home and End need the layout: the window measures where the
//   cursor would go and sends it along (`rows`).
// - Link (Ctrl+K) and Image (Ctrl+Shift+I) ask first: the core answers with
//   a request, which opens their dialog (feature F6). Nano's Ctrl+K cuts the
//   line.

import type { FacadeInput, KeyName } from '../bindings';
import type { KeysState } from '../editor/keys';

/** What to do with a key in the facade. */
export type FacadeKeyRoute =
  /** Send it to the backend; `rows` when the cursor's rows are needed. */
  | { send: FacadeInput; rows: boolean }
  /** Stop it; nothing happens. */
  | 'swallow'
  /** M-08.10: Escape leaves the text. */
  | 'leave'
  /** M-09.9, A-45: Ctrl/Cmd+Enter enlarges the picture the cursor is in. */
  | 'enlarge'
  /** M-09.14: Ctrl/Cmd+Shift+V pastes plain text. */
  | 'pastePlain'
  /** Leave it to the webview and the window. */
  | null;

/** Keys the facade takes with any modifier. */
const ALWAYS: Readonly<Record<string, KeyName>> = {
  Backspace: 'Backspace',
  Delete: 'Delete',
  Enter: 'Enter',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  Home: 'Home',
  End: 'End',
};

/** Keys whose target the window measures. */
const MEASURED: ReadonlySet<KeyName> = new Set(['ArrowUp', 'ArrowDown', 'Home', 'End']);

/** M-09.13: keys that do nothing in the facade. */
const NOTHING: ReadonlySet<string> = new Set(['Tab', 'PageUp', 'PageDown']);

const WITH_COMMAND: ReadonlySet<string> = new Set(['A', 'Y', 'Z']);
const TOOL_KEYS: ReadonlySet<string> = new Set(['B', 'I', 'E', 'K']);
const TOOL_SHIFT_KEYS: ReadonlySet<string> = new Set(['H', 'L', 'Q', 'I', 'T']);
const NANO_CTRL_KEYS: ReadonlySet<string> = new Set(['K', 'U', 'W', 'O', 'S', 'X', 'A', 'E']);
const NANO_ALT_KEYS: ReadonlySet<string> = new Set(['U', 'E']);

/** The parts of a key event the facade reads. */
export type FacadeKeyEvent = Pick<
  KeyboardEvent,
  'key' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'isComposing'
> & { code?: string };

function letterOf(event: FacadeKeyEvent): string | null {
  if (/^[a-z]$/i.test(event.key)) {
    return event.key.toUpperCase();
  }
  const code = /^Key([A-Z])$/.exec(event.code ?? '');
  return code ? code[1] : null;
}

function bracketOf(event: FacadeKeyEvent): KeyName | null {
  if (event.key === '[' || event.key === '{' || event.code === 'BracketLeft') {
    return 'BracketLeft';
  }
  if (event.key === ']' || event.key === '}' || event.code === 'BracketRight') {
    return 'BracketRight';
  }
  return null;
}

function keyInput(event: FacadeKeyEvent, key: KeyName): FacadeInput {
  return {
    kind: 'key',
    key,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
  };
}

/**
 * Where the key event `event` goes in the facade, in the input mode
 * `state`. The command key is Cmd on macOS (`mac`) and Ctrl elsewhere.
 */
export function routeFacadeKey(
  event: FacadeKeyEvent,
  mac: boolean,
  state: KeysState = { mode: 'Normal' },
): FacadeKeyRoute {
  if (event.isComposing) {
    return null;
  }
  const command = mac ? event.metaKey : event.ctrlKey;
  const send = (key: KeyName): FacadeKeyRoute => ({
    send: keyInput(event, key),
    rows: MEASURED.has(key),
  });
  const letter = letterOf(event);
  if (command && !event.shiftKey && !event.altKey && event.key === 'Enter') {
    return 'enlarge';
  }
  const always = ALWAYS[event.key];
  const bracket = command && event.shiftKey ? bracketOf(event) : null;
  const tool =
    command &&
    letter !== null &&
    (event.shiftKey ? TOOL_SHIFT_KEYS.has(letter) : TOOL_KEYS.has(letter));
  const ctrl = event.ctrlKey || (mac && event.metaKey);

  if (state.mode === 'Nano') {
    if (state.prompting) {
      if (always || event.key === 'Escape') {
        return send(always ?? 'Escape');
      }
      if (ctrl && letter === 'C') {
        return send('C');
      }
      return NOTHING.has(event.key) ? 'swallow' : null;
    }
    if (event.key === 'Escape') {
      return send('Escape');
    }
    if (letter !== null) {
      const nano = ctrl ? NANO_CTRL_KEYS.has(letter) : event.altKey && NANO_ALT_KEYS.has(letter);
      if (nano) {
        return send(letter as KeyName);
      }
    }
  }
  if (state.mode === 'Vim') {
    if (event.key === 'Escape') {
      return send('Escape');
    }
    if (ctrl && letter === 'R') {
      return send('R');
    }
  }
  if (state.mode === 'Normal' && event.key === 'Escape') {
    return 'leave';
  }
  if (NOTHING.has(event.key)) {
    return 'swallow';
  }
  if (always) {
    return send(always);
  }
  if (bracket) {
    return send(bracket);
  }
  if (tool) {
    return send(letter as KeyName);
  }
  if (letter !== null && command && WITH_COMMAND.has(letter)) {
    return send(letter as KeyName);
  }
  if (command && event.shiftKey && letter === 'V') {
    return 'pastePlain';
  }
  return null;
}

/** What a `beforeinput` of the facade is. */
export type BeforeInputRoute =
  /** Send this input. */
  | { send: FacadeInput }
  /** M-09.14: paste through the backend. */
  | 'paste'
  /** M-08.5: an input method composes; it is let through. */
  | 'compose'
  /** Refused: nothing changes the text but the backend. */
  | null;

/**
 * M-09.10 to M-09.12, M-10.1: the facade's input for a `beforeinput` event;
 * undo and redo come with the command key of the platform (Cmd on macOS,
 * `mac`).
 */
export function routeBeforeInput(
  inputType: string,
  data: string | null,
  mac = false,
): BeforeInputRoute {
  const key = (name: KeyName, command = false): BeforeInputRoute => ({
    send: {
      kind: 'key',
      key: name,
      ctrl: command && !mac,
      alt: false,
      shift: false,
      meta: command && mac,
    },
  });
  switch (inputType) {
    case 'insertText':
    case 'insertReplacementText':
      return data ? { send: { kind: 'text', text: data } } : null;
    case 'insertParagraph':
    case 'insertLineBreak':
      return key('Enter');
    case 'deleteContentBackward':
      return key('Backspace');
    case 'deleteContentForward':
      return key('Delete');
    case 'historyUndo':
      return key('Z', true);
    case 'historyRedo':
      return key('Y', true);
    case 'insertFromPaste':
      return 'paste';
    case 'insertCompositionText':
    case 'deleteCompositionText':
      return 'compose';
    default:
      return null;
  }
}
