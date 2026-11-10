import { describe, expect, it } from 'vitest';
import type { EditorInput, KeyName } from '../bindings';
import { FakeEditor } from './fakeEditor';

const press = (key: KeyName, mods: Partial<Extract<EditorInput, { kind: 'key' }>> = {}) =>
  ({
    kind: 'key',
    key,
    ctrl: false,
    alt: false,
    shift: false,
    meta: false,
    page: 2,
    ...mods,
  }) as EditorInput;

describe('the mock backend edits like the backend (UTF-16 units)', () => {
  it('types, deletes a whole emoji and moves by lines keeping the column', () => {
    const doc = new FakeEditor('Home', 'abcdef\nab\nabcdef');
    doc.select(0, 5, 5);

    doc.input(press('ArrowDown'));
    expect(doc.head).toBe(9);
    doc.input(press('ArrowDown'));
    expect(doc.head).toBe(12);

    const typed = doc.input({ kind: 'text', text: '🙂' });
    expect(typed.changes).toEqual([{ from: 12, to: 12, insert: '🙂' }]);
    const deleted = doc.input(press('Backspace', { ctrl: true }));
    expect(deleted.changes).toEqual([{ from: 12, to: 14, insert: '' }]);
  });

  it('undoes and redoes, selects all and inserts four spaces for Tab', () => {
    const doc = new FakeEditor('Home', 'x');
    doc.input(press('Tab'));
    expect(doc.text).toBe('    x');
    doc.input(press('Z', { ctrl: true }));
    expect(doc.text).toBe('x');
    doc.input(press('Y', { ctrl: true }));
    expect(doc.text).toBe('    x');
    doc.input(press('A', { ctrl: true }));
    expect(doc.selection()).toEqual([0, 5]);
  });
});
