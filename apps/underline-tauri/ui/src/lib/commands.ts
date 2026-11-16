// FR-077, UC-023 BR-001, BR-003, BR-005: the one registry of the window's
// commands. Each command has its name and its menu, its key, whether it can
// run now and why not, and what it does; the menu bar shows its name and key
// from here, the window's key handler (App.svelte) runs it from its key, and
// the command palette lists and runs it, so names, keys and actions cannot
// drift apart. The commands themselves are listed in `commandList.ts`.

/** A key with its modifiers; the command key is Ctrl, Cmd on macOS. */
export type KeyChord = {
  /** A letter in capitals (`F`), a function key (`F2`), or a character (`,`). */
  key: string;
  /** Held with the command key (Ctrl, Cmd on macOS). */
  command?: boolean;
  /** Held with Ctrl on every platform, also on macOS (Ctrl+Space, FR-080). */
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
};

/** A command's key, the same on every platform or one for macOS and one for the others. */
export type CommandKey = KeyChord | { mac: KeyChord; other: KeyChord };

/** OQ-243: where a key runs its command: in the text of a note, or elsewhere. */
export type KeyPlace = 'text' | 'outside';

/** UC-023 BR-003, OQ-243: how the palette names the place of a key. */
export const KEY_PLACE_TEXT: Readonly<Record<KeyPlace, string>> = {
  text: 'in the note',
  outside: 'outside the note',
};

/** Whether the keyboard is in the text of a note (the source view or the facade). */
export function keyboardInText(): boolean {
  const active = document.activeElement;
  return active instanceof Element && active.closest('.cm-content, .facade-text') !== null;
}

/** UC-023 A3: one choice of a command that asks before it runs. */
export type CommandChoice = {
  /** What the command gets. */
  value: string;
  /** What the list shows, such as "Nord". */
  label: string;
  /** The choice in effect now, marked in the list. */
  current: boolean;
};

/** One command of Underline (UC-023 BR-001). */
export type Command = {
  /** Stable, for the recently run commands and the menus (`file.newNote`). */
  id: string;
  /** Its name, the same as in its menu (BR-001). */
  title: string;
  /** The menu or place it belongs to, which the palette shows and matches (BR-002). */
  menu: string;
  /** Its key, if it has one; the one shown. */
  key?: CommandKey;
  /** Other keys that run it too, not shown (F1 beside Ctrl+Shift+P). */
  alsoKeys?: CommandKey[];
  /**
   * OQ-243: where its keys run it: only while the keyboard is in the text
   * of a note (`text`), only elsewhere (`outside`), or anywhere (unset). Two
   * commands may share a key only for places that do not overlap.
   */
  keyPlace?: KeyPlace;
  /**
   * The window takes the key wherever the keyboard is, before the editors
   * see it; without, the key belongs to the place that shows the command
   * (the toolbar's keys are the editor's).
   */
  global?: boolean;
  /**
   * Whether the window takes the key now; by default while the command is
   * available, so the key otherwise keeps its way.
   */
  takesKey?: () => boolean;
  /** Why it cannot run now (UC-023 A2), or `null` when it can. */
  unavailable: () => string | null;
  /** UC-023 A3: the choices it asks for before it runs. */
  choices?: () => CommandChoice[];
  /** Runs it, with the choice made when it asks for one. */
  run: (choice?: string) => void;
};

/** The key of `key` on the platform (`mac`). */
export function chordOf(key: CommandKey, mac: boolean): KeyChord {
  return 'other' in key ? (mac ? key.mac : key.other) : key;
}

/** The places on the keyboard (`KeyboardEvent.code`) of the digits and signs the commands use. */
const KEY_CODES: Readonly<Record<string, readonly string[]>> = {
  '0': ['Digit0', 'Numpad0'],
  '1': ['Digit1', 'Numpad1'],
  '2': ['Digit2', 'Numpad2'],
  '.': ['Period'],
  // FR-096: Ctrl+\ by its place where the layout puts another sign there.
  '\\': ['Backslash'],
};

/** How a key is named where its name differs from the event's (`ArrowUp` is Up). */
const KEY_NAMES: Readonly<Record<string, string>> = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  // UC-010 BR-006: Ctrl+Plus and Ctrl+Minus, as the keys are named.
  '+': 'Plus',
  '-': 'Minus',
};

/** BR-003: the key as the platform names it: `Ctrl+Shift+F`, `Cmd+,`, `F2`, `Alt+Up`, `Ctrl+Plus`. */
export function keyText(key: CommandKey | undefined, mac: boolean): string {
  if (key === undefined) {
    return '';
  }
  const chord = chordOf(key, mac);
  return [
    chord.ctrl && !(chord.command && !mac) ? 'Ctrl' : null,
    chord.command ? (mac ? 'Cmd' : 'Ctrl') : null,
    chord.alt ? 'Alt' : null,
    chord.shift ? 'Shift' : null,
    KEY_NAMES[chord.key] ?? chord.key,
  ]
    .filter((part) => part !== null)
    .join('+');
}

/** The key for `aria-keyshortcuts`: `Control+Shift+F`, `Meta+,`. */
export function ariaKeys(key: CommandKey | undefined, mac: boolean): string | undefined {
  if (key === undefined) {
    return undefined;
  }
  const chord = chordOf(key, mac);
  return [
    chord.ctrl && !(chord.command && !mac) ? 'Control' : null,
    chord.command ? (mac ? 'Meta' : 'Control') : null,
    chord.alt ? 'Alt' : null,
    chord.shift ? 'Shift' : null,
    // `+` joins the keys, so the key itself is named.
    chord.key === '+' ? 'Plus' : chord.key,
  ]
    .filter((part) => part !== null)
    .join('+');
}

/** The parts of a key event a key is told by. */
export type KeyEventLike = Pick<
  KeyboardEvent,
  'key' | 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'
>;

/**
 * Whether `event` is the key `key` on the platform (`mac`). A letter is
 * told by the character it types, or, when the modifiers made another
 * character of it (Alt on macOS), by the key's place (`KeyF`).
 */
export function matchesKey(event: KeyEventLike, key: CommandKey, mac: boolean): boolean {
  const chord = chordOf(key, mac);
  const wantsCommand = chord.command ?? false;
  const wantsCtrl = chord.ctrl ?? false;
  const held = mac
    ? event.metaKey === wantsCommand && event.ctrlKey === wantsCtrl
    : event.ctrlKey === (wantsCommand || wantsCtrl) && !event.metaKey;
  if (!held || event.shiftKey !== (chord.shift ?? false) || event.altKey !== (chord.alt ?? false)) {
    return false;
  }
  if (chord.key === 'Space') {
    return event.key === ' ';
  }
  // Package 5 (review): a digit or a sign is also told by its key's place,
  // where the layout or Shift make another character of it (Ctrl+0 is
  // Ctrl+à on AZERTY, Cmd+Shift+. is Cmd+> on a US keyboard).
  if (KEY_CODES[chord.key]?.includes(event.code)) {
    return true;
  }
  if (/^[A-Z]$/.test(chord.key)) {
    if (/^[a-z]$/i.test(event.key)) {
      return event.key.toUpperCase() === chord.key;
    }
    return event.code === `Key${chord.key}`;
  }
  return event.key === chord.key;
}

/**
 * The window's command for the key `event`, among `commands`: a global one
 * whose key it is, for the place the keyboard is in (`inText`, OQ-243),
 * and which takes it now; `null` for none.
 */
export function commandForKey(
  event: KeyEventLike,
  commands: readonly Command[],
  mac: boolean,
  inText: boolean = keyboardInText(),
): Command | null {
  for (const command of commands) {
    const keys = command.key === undefined ? [] : [command.key, ...(command.alsoKeys ?? [])];
    if (!command.global || !keys.some((key) => matchesKey(event, key, mac))) {
      continue;
    }
    if (command.keyPlace !== undefined && (command.keyPlace === 'text') !== inText) {
      continue;
    }
    const takes = command.takesKey ? command.takesKey() : command.unavailable() === null;
    if (takes) {
      return command;
    }
  }
  return null;
}

/** UC-023 BR-004: how many recently run commands an empty field lists first. */
export const RECENT_FIRST = 5;

/**
 * UC-023 BR-002, BR-004: the order the palette gives its commands to the
 * ranking. With an empty field (`empty`) the five most recently run first,
 * the last one on top, then all others by name; while the user types, every
 * command run before by how recently, then the others by name, so within
 * each group of the ranking the most recently run comes first. `recent`
 * holds ids, the most recent first.
 */
export function paletteOrder(
  commands: readonly Command[],
  recent: readonly string[],
  empty: boolean,
): Command[] {
  const first = empty ? recent.slice(0, RECENT_FIRST) : recent;
  const byId = new Map(commands.map((command) => [command.id, command]));
  const ran = first
    .map((id) => byId.get(id))
    .filter((command): command is Command => command !== undefined);
  const taken = new Set(ran.map((command) => command.id));
  const rest = commands
    .filter((command) => !taken.has(command.id))
    .sort(
      (a, b) =>
        a.title.localeCompare(b.title, undefined, { sensitivity: 'base' }) ||
        a.menu.localeCompare(b.menu),
    );
  return [...ran, ...rest];
}

/** Two places of keys overlap: one of them is anywhere, or both are the same. */
export function placesOverlap(a: KeyPlace | undefined, b: KeyPlace | undefined): boolean {
  return a === undefined || b === undefined || a === b;
}

/**
 * OQ-243: the commands that claim the same key for overlapping places on
 * the platform (`mac`), as pairs of ids; none in a sound registry.
 */
export function keyClashes(commands: readonly Command[], mac: boolean): [string, string][] {
  const claims = commands.flatMap((command) =>
    (command.key === undefined ? [] : [command.key, ...(command.alsoKeys ?? [])]).map((key) => ({
      id: command.id,
      text: keyText(key, mac),
      place: command.keyPlace,
    })),
  );
  const clashes: [string, string][] = [];
  claims.forEach((a, at) => {
    for (const b of claims.slice(at + 1)) {
      if (a.id !== b.id && a.text === b.text && placesOverlap(a.place, b.place)) {
        clashes.push([a.id, b.id]);
      }
    }
  });
  return clashes;
}
