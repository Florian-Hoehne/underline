import { describe, expect, it } from 'vitest';
import { keyInput, routeKey, type KeyEventLike } from './keys';

function key(key: string, mods: Partial<KeyEventLike> = {}): KeyEventLike {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    isComposing: false,
    ...mods,
  };
}

describe('M-08.4 keys of the source view', () => {
  it('takes the editing and moving keys with any modifier, as egui does', () => {
    expect(keyInput(key('ArrowLeft', { ctrlKey: true }), false, 20)).toEqual({
      kind: 'key',
      key: 'ArrowLeft',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      page: 20,
    });
    for (const name of [
      'Backspace',
      'Delete',
      'Enter',
      'Tab',
      'Home',
      'End',
      'PageUp',
      'PageDown',
    ]) {
      expect(keyInput(key(name, { altKey: true }), false, 1)?.kind).toBe('key');
    }
  });

  it('M-10.1 takes A, Y and Z only with the command key: Ctrl, or Cmd on macOS', () => {
    expect(keyInput(key('z', { ctrlKey: true }), false, 1)).toMatchObject({ key: 'Z' });
    expect(keyInput(key('Z', { ctrlKey: true, shiftKey: true }), false, 1)).toMatchObject({
      key: 'Z',
      shift: true,
    });
    expect(keyInput(key('y', { metaKey: true }), true, 1)).toMatchObject({ key: 'Y', meta: true });
    expect(keyInput(key('a', { ctrlKey: true }), true, 1)).toBeNull();
    expect(keyInput(key('a'), false, 1)).toBeNull();
  });

  it("leaves typing, the window's shortcuts and compositions alone", () => {
    expect(keyInput(key('x'), false, 1)).toBeNull();
    expect(keyInput(key('n', { ctrlKey: true }), false, 1)).toBeNull();
    expect(keyInput(key('Escape'), false, 1)).toBeNull();
    expect(keyInput(key('Enter', { isComposing: true }), false, 1)).toBeNull();
  });

  it('moves at least one line a page', () => {
    expect(keyInput(key('PageDown'), false, 0.4)).toMatchObject({ page: 1 });
  });
});

describe('M-11, M-10.2, M-08.8 keys by input mode', () => {
  const send = (route: ReturnType<typeof routeKey>) =>
    route !== null && route !== 'swallow' ? route.send : route;

  it('M-10.2 the toolbar keys need the command key, with Shift where the hint says', () => {
    expect(send(routeKey(key('b', { ctrlKey: true }), false, 1))).toMatchObject({ key: 'B' });
    // F6: Link and Image go to the core, which asks for their dialogs.
    expect(send(routeKey(key('K', { ctrlKey: true }), false, 1))).toMatchObject({ key: 'K' });
    expect(send(routeKey(key('I', { ctrlKey: true, shiftKey: true }), false, 1))).toMatchObject({
      key: 'I',
      shift: true,
    });
    expect(send(routeKey(key('i', { ctrlKey: true }), false, 1))).toMatchObject({ key: 'I' });
    expect(send(routeKey(key('H', { ctrlKey: true, shiftKey: true }), false, 1))).toMatchObject({
      key: 'H',
      shift: true,
    });
    expect(send(routeKey(key('h', { ctrlKey: true }), false, 1))).toBeNull();
    expect(send(routeKey(key('b', { metaKey: true }), true, 1))).toMatchObject({ key: 'B' });
    expect(send(routeKey(key('b', { ctrlKey: true }), true, 1))).toBeNull();
  });

  it('M-08.8 Ctrl/Cmd+Shift+[ and ] fold and expand, whatever the layout makes of them', () => {
    expect(send(routeKey(key('{', { ctrlKey: true, shiftKey: true }), false, 1))).toMatchObject({
      key: 'BracketLeft',
    });
    expect(
      send(routeKey(key('*', { code: 'BracketRight', ctrlKey: true, shiftKey: true }), false, 1)),
    ).toMatchObject({ key: 'BracketRight' });
    expect(send(routeKey(key('[', { ctrlKey: true }), false, 1))).toBeNull();
  });

  it('M-11.3 Nano takes Escape, its Ctrl keys and Alt+U/E, Cmd as Ctrl on macOS', () => {
    const nano = { mode: 'Nano' as const };
    expect(send(routeKey(key('Escape'), false, 1, nano))).toMatchObject({ key: 'Escape' });
    for (const letter of ['k', 'u', 'w', 'o', 's', 'x', 'a', 'e']) {
      expect(send(routeKey(key(letter, { ctrlKey: true }), false, 1, nano))).toMatchObject({
        key: letter.toUpperCase(),
      });
    }
    expect(send(routeKey(key('k', { metaKey: true }), true, 1, nano))).toMatchObject({
      key: 'K',
    });
    expect(send(routeKey(key('¨', { code: 'KeyU', altKey: true }), true, 1, nano))).toMatchObject({
      key: 'U',
      alt: true,
    });
    // Ctrl+C copies, unless the search prompt is open.
    expect(send(routeKey(key('c', { ctrlKey: true }), false, 1, nano))).toBeNull();
    expect(
      send(routeKey(key('c', { ctrlKey: true }), false, 1, { ...nano, prompting: true })),
    ).toMatchObject({ key: 'C' });
    expect(send(routeKey(key('Enter'), false, 1, { ...nano, prompting: true }))).toMatchObject({
      key: 'Enter',
    });
  });

  it('A-01 Vim: the editing keys in insert mode, else only toolbar and folding keys', () => {
    const normal = { mode: 'Vim' as const };
    const insert = { mode: 'Vim' as const, vimInsert: true };
    expect(routeKey(key('Tab'), false, 1, normal)).toBe('swallow');
    expect(routeKey(key('Escape'), false, 1, normal)).toBe('swallow');
    expect(routeKey(key('z', { ctrlKey: true }), false, 1, normal)).toBeNull();
    expect(send(routeKey(key('b', { ctrlKey: true }), false, 1, normal))).toMatchObject({
      key: 'B',
    });
    expect(
      send(routeKey(key('[', { ctrlKey: true, shiftKey: true }), false, 1, normal)),
    ).toMatchObject({ key: 'BracketLeft' });
    expect(send(routeKey(key('Backspace'), false, 1, insert))).toMatchObject({
      key: 'Backspace',
    });
    expect(send(routeKey(key('z', { ctrlKey: true }), false, 1, insert))).toMatchObject({
      key: 'Z',
    });
    expect(routeKey(key('x'), false, 1, insert)).toBeNull();
  });
});
