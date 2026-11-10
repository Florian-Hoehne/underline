import { describe, expect, it } from 'vitest';
import { keyInput, type KeyEventLike } from './keys';

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
