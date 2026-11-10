import { describe, expect, it } from 'vitest';
import { routeBeforeInput, routeFacadeKey, type FacadeKeyEvent } from './keys';

function key(name: string, init: Partial<FacadeKeyEvent> = {}): FacadeKeyEvent {
  return {
    key: name,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    isComposing: false,
    ...init,
  };
}

describe('M-09.13 the keys of the facade', () => {
  it('the editing keys go to the backend; Up, Down, Home and End with the rows', () => {
    expect(routeFacadeKey(key('Backspace'), false)).toEqual({
      send: { kind: 'key', key: 'Backspace', ctrl: false, alt: false, shift: false, meta: false },
      rows: false,
    });
    expect(routeFacadeKey(key('ArrowUp', { shiftKey: true }), false)).toMatchObject({
      send: { key: 'ArrowUp', shift: true },
      rows: true,
    });
    expect(routeFacadeKey(key('End'), false)).toMatchObject({ rows: true });
    expect(routeFacadeKey(key('ArrowLeft'), false)).toMatchObject({ rows: false });
  });

  it('Tab, Page Up and Page Down do nothing; Escape leaves; Ctrl+Enter enlarges', () => {
    expect(routeFacadeKey(key('Tab'), false)).toBe('swallow');
    expect(routeFacadeKey(key('PageDown'), false)).toBe('swallow');
    expect(routeFacadeKey(key('Escape'), false)).toBe('leave');
    expect(routeFacadeKey(key('Enter', { ctrlKey: true }), false)).toBe('enlarge');
    expect(routeFacadeKey(key('Enter', { metaKey: true }), true)).toBe('enlarge');
  });

  it('M-10.1, M-10.2, M-08.8 command keys: undo, toolbar, folding; Link and Image ask (F6)', () => {
    expect(routeFacadeKey(key('z', { ctrlKey: true }), false)).toMatchObject({
      send: { key: 'Z', ctrl: true },
    });
    expect(routeFacadeKey(key('b', { metaKey: true }), true)).toMatchObject({
      send: { key: 'B', meta: true },
    });
    expect(routeFacadeKey(key('k', { ctrlKey: true }), false)).toMatchObject({
      send: { key: 'K', ctrl: true },
    });
    expect(routeFacadeKey(key('{', { ctrlKey: true, shiftKey: true }), false)).toMatchObject({
      send: { key: 'BracketLeft' },
    });
    expect(routeFacadeKey(key('V', { ctrlKey: true, shiftKey: true }), false)).toBe('pastePlain');
    // Plain letters are text, from `beforeinput`.
    expect(routeFacadeKey(key('b'), false)).toBeNull();
  });

  it('M-11.2, M-11.3 Vim and Nano get Escape and their own keys', () => {
    expect(routeFacadeKey(key('Escape'), false, { mode: 'Vim' })).toMatchObject({
      send: { key: 'Escape' },
    });
    expect(routeFacadeKey(key('r', { ctrlKey: true }), false, { mode: 'Vim' })).toMatchObject({
      send: { key: 'R', ctrl: true },
    });
    expect(routeFacadeKey(key('k', { ctrlKey: true }), false, { mode: 'Nano' })).toMatchObject({
      send: { key: 'K', ctrl: true },
    });
    expect(routeFacadeKey(key('u', { altKey: true }), false, { mode: 'Nano' })).toMatchObject({
      send: { key: 'U', alt: true },
    });
    expect(
      routeFacadeKey(key('c', { ctrlKey: true }), false, { mode: 'Nano', prompting: true }),
    ).toMatchObject({ send: { key: 'C' } });
    expect(routeFacadeKey(key('Escape'), false, { mode: 'Nano' })).toMatchObject({
      send: { key: 'Escape' },
    });
  });

  it('M-08.5 a key during a composition is the input method’s', () => {
    expect(routeFacadeKey(key('Enter', { isComposing: true }), false)).toBeNull();
  });
});

describe('ADR-021 every beforeinput becomes an input of the facade', () => {
  it('text, Enter, Backspace, Delete, undo, redo, paste and composition', () => {
    expect(routeBeforeInput('insertText', 'x')).toEqual({ send: { kind: 'text', text: 'x' } });
    expect(routeBeforeInput('insertParagraph', null)).toMatchObject({ send: { key: 'Enter' } });
    expect(routeBeforeInput('insertLineBreak', null)).toMatchObject({ send: { key: 'Enter' } });
    expect(routeBeforeInput('deleteContentBackward', null)).toMatchObject({
      send: { key: 'Backspace' },
    });
    expect(routeBeforeInput('deleteContentForward', null)).toMatchObject({
      send: { key: 'Delete' },
    });
    expect(routeBeforeInput('historyUndo', null)).toMatchObject({
      send: { key: 'Z', ctrl: true, meta: false },
    });
    expect(routeBeforeInput('historyRedo', null, true)).toMatchObject({
      send: { key: 'Y', ctrl: false, meta: true },
    });
    expect(routeBeforeInput('insertFromPaste', null)).toBe('paste');
    expect(routeBeforeInput('insertCompositionText', '日')).toBe('compose');
    expect(routeBeforeInput('deleteWordBackward', null)).toBeNull();
    expect(routeBeforeInput('insertText', '')).toBeNull();
  });
});
