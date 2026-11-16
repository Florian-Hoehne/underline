// Package 2 (FR-077, UC-023): the registry's keys and the palette's order,
// without the window.

import { describe, expect, it } from 'vitest';
import {
  ariaKeys,
  commandForKey,
  keyClashes,
  placesOverlap,
  keyText,
  matchesKey,
  paletteOrder,
  type Command,
  type CommandKey,
} from './commands';

function key(
  key: string,
  modifiers: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'shiftKey' | 'altKey' | 'metaKey'>> = {},
  code = '',
) {
  return {
    key,
    code,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ...modifiers,
  };
}

function command(id: string, title: string, extra: Partial<Command> = {}): Command {
  return { id, title, menu: 'File', unavailable: () => null, run: () => undefined, ...extra };
}

describe('UC-023 BR-003 keys shown per platform', () => {
  const find: CommandKey = { key: 'F', command: true };
  const replace: CommandKey = {
    mac: { key: 'F', command: true, alt: true },
    other: { key: 'H', command: true },
  };

  it('TC-023-11 names the command key Ctrl on Linux and Cmd on macOS', () => {
    expect(keyText(find, false)).toBe('Ctrl+F');
    expect(keyText(find, true)).toBe('Cmd+F');
    expect(keyText({ key: 'P', command: true, shift: true }, false)).toBe('Ctrl+Shift+P');
    expect(keyText({ key: ',', command: true }, true)).toBe('Cmd+,');
    expect(keyText({ key: 'F2' }, false)).toBe('F2');
    expect(keyText(undefined, false)).toBe('');
  });

  it('takes the key of the platform when it differs', () => {
    expect(keyText(replace, false)).toBe('Ctrl+H');
    expect(keyText(replace, true)).toBe('Cmd+Alt+F');
    expect(ariaKeys(replace, true)).toBe('Meta+Alt+F');
    expect(ariaKeys({ key: ',', command: true }, false)).toBe('Control+,');
    expect(ariaKeys(undefined, false)).toBeUndefined();
  });

  it('tells a key by its modifiers exactly, the command key per platform', () => {
    expect(matchesKey(key('f', { ctrlKey: true }), find, false)).toBe(true);
    expect(matchesKey(key('F', { ctrlKey: true }), find, false)).toBe(true);
    expect(matchesKey(key('f', { metaKey: true }), find, false)).toBe(false);
    expect(matchesKey(key('f', { metaKey: true }), find, true)).toBe(true);
    expect(matchesKey(key('f', { ctrlKey: true, shiftKey: true }), find, false)).toBe(false);
    expect(matchesKey(key('f', { ctrlKey: true, altKey: true }), find, false)).toBe(false);
    expect(matchesKey(key('g', { ctrlKey: true }), find, false)).toBe(false);
  });

  it('tells a letter by its place when Alt made another character of it (macOS)', () => {
    expect(matchesKey(key('ƒ', { metaKey: true, altKey: true }, 'KeyF'), replace, true)).toBe(true);
    // A letter typed is told by the letter, not by its place (other layouts).
    expect(matchesKey(key('j', { ctrlKey: true }, 'KeyF'), find, false)).toBe(false);
  });

  it('tells function keys and characters by their name', () => {
    expect(matchesKey(key('F2'), { key: 'F2' }, false)).toBe(true);
    expect(matchesKey(key('F2', { shiftKey: true }), { key: 'F2' }, false)).toBe(false);
    expect(matchesKey(key(',', { ctrlKey: true }), { key: ',', command: true }, false)).toBe(true);
  });
});

describe('UC-023 BR-005 the window runs a command from its key', () => {
  it('finds the global command whose key it is and which takes it now', () => {
    const local = command('local', 'Bold', { key: { key: 'B', command: true } });
    const off = command('off', 'Rename…', {
      key: { key: 'F2' },
      global: true,
      unavailable: () => 'No note is open.',
    });
    const palette = command('palette', 'Command palette', {
      key: { key: 'P', command: true, shift: true },
      alsoKeys: [{ key: 'F1' }],
      global: true,
    });
    const always = command('always', 'Export…', {
      key: { key: 'E', command: true, shift: true },
      global: true,
      takesKey: () => true,
      unavailable: () => 'No note is open.',
    });
    const all = [local, off, palette, always];

    expect(commandForKey(key('b', { ctrlKey: true }), all, false)).toBeNull();
    expect(commandForKey(key('F2'), all, false)).toBeNull();
    expect(commandForKey(key('P', { ctrlKey: true, shiftKey: true }), all, false)).toBe(palette);
    expect(commandForKey(key('F1'), all, false)).toBe(palette);
    expect(commandForKey(key('E', { ctrlKey: true, shiftKey: true }), all, false)).toBe(always);
  });
});

describe('UC-023 BR-002, BR-004 the order given to the ranking', () => {
  const names = ['Theme…', 'Bold', 'Minimap', 'Commit', 'Settings…', 'Find in note', 'Source'];
  const all = names.map((title, at) => command(`c${at}`, title));

  it('TC-023-02 step 4 without any command run: all by name', () => {
    expect(paletteOrder(all, [], true).map((c) => c.title)).toEqual([
      'Bold',
      'Commit',
      'Find in note',
      'Minimap',
      'Settings…',
      'Source',
      'Theme…',
    ]);
  });

  it('BR-004 with an empty field the five run last come first, then all others by name', () => {
    const recent = ['c6', 'c5', 'c4', 'c3', 'c2', 'c0'];
    expect(paletteOrder(all, recent, true).map((c) => c.title)).toEqual([
      'Source',
      'Find in note',
      'Settings…',
      'Commit',
      'Minimap',
      'Bold',
      'Theme…',
    ]);
  });

  it('BR-002 while typing every command run comes first by how recently', () => {
    const recent = ['c6', 'c5', 'c4', 'c3', 'c2', 'c0', 'gone'];
    expect(paletteOrder(all, recent, false).map((c) => c.title)).toEqual([
      'Source',
      'Find in note',
      'Settings…',
      'Commit',
      'Minimap',
      'Theme…',
      'Bold',
    ]);
  });
});

describe('OQ-243 a key for a place', () => {
  const heading = command('format.heading', 'Format as heading', {
    key: { key: 'H', command: true, shift: true },
    keyPlace: 'text',
  });
  const replace = command('search.replace', 'Replace in notes', {
    key: { key: 'H', command: true, shift: true },
    keyPlace: 'outside',
    global: true,
  });
  const ctrlShiftH = key('H', { ctrlKey: true, shiftKey: true });

  it('runs the command of the place the keyboard is in', () => {
    expect(commandForKey(ctrlShiftH, [heading, replace], false, false)).toBe(replace);
    expect(commandForKey(ctrlShiftH, [heading, replace], false, true)).toBeNull();
  });

  it('finds commands that claim one key for places that overlap', () => {
    expect(keyClashes([heading, replace], false)).toEqual([]);
    const anywhere = command('other', 'Other', { key: { key: 'H', command: true, shift: true } });
    expect(keyClashes([heading, replace, anywhere], false)).toEqual([
      ['format.heading', 'other'],
      ['search.replace', 'other'],
    ]);
    expect(placesOverlap('text', 'outside')).toBe(false);
    expect(placesOverlap('text', undefined)).toBe(true);
  });
});
