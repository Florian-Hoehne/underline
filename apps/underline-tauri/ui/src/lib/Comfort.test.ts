// Package 4 (FR-081 to FR-086): close pairs (UC-005 A25, TC-005-35), moving
// and duplicating lines (A26, TC-005-36), indenting list items (A27,
// TC-005-37), the current line and the matching bracket (BR-022,
// TC-005-38), going to a line (UC-004 A16, A17, TC-004-33) and the setting
// of close pairs (UC-010 BR-001, TC-010-22), against the mock backend's
// `/home/user/Comfort`. What each key does to the text is the editor
// core's (in the WASM, tested in Rust); here the keys, the palette and the
// field reach it, and what lands in the note.

import { cleanup, fireEvent, render, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { CURRENT_LINE, lineAndBrackets } from './editor/currentLine';
import { activeFacade } from './facade/active';
import { COMFORT, COMFORT_TEXTS, mockHooks, type MockStart } from './mock-backend';
import { MockSettings } from './mock-settings';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const SOURCE: MockStart = { givenDir: COMFORT, view: 'source' };
const FACADE: MockStart = { givenDir: COMFORT };

async function workbench(start: MockStart): Promise<Shell> {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** The text field of the source view of `note`, once its text is in, with the keyboard. */
async function sourceField(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  await waitFor(() => expect(activeSourceView()?.note).toBe(note));
  const field = document.querySelector<HTMLElement>('.cm-content')!;
  field.focus();
  return field;
}

/** The facade's text of `note`, once its first block is drawn. */
async function facadeField(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('.facade-text');
    expect(activeFacade()?.note).toBe(note);
    expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
    return found!;
  });
}

function type(field: HTMLElement, text: string) {
  for (const c of text) {
    field.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertText',
        data: c,
        bubbles: true,
        cancelable: true,
      }),
    );
  }
}

/** The mirror's text of `note` once it is `text`. */
async function becomes(note: string, text: string): Promise<void> {
  await waitFor(() => expect(mockHooks().mirrorText(note)).toBe(text));
}

const overlay = () => document.querySelector<HTMLElement>('.palette');
const paletteField = () => within(overlay()!).getByRole('combobox');

describe('FR-081 close pairs (UC-005 A25, BR-019)', () => {
  it('TC-005-35 steps 1, 2 and 8: `(` closes, `a)` types over it; switched off only `(`', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Pairs');
    await fireEvent.keyDown(field, { key: 'End' });

    type(field, '(');
    await becomes('Pairs', '# Pairs()\n\nType a word here\n');
    type(field, 'a)');
    await becomes('Pairs', '# Pairs(a)\n\nType a word here\n');

    await shell.quickSetting('close_pairs', 'false');
    await waitFor(() => expect(shell.settings?.closePairs).toBe(false));
    type(field, '(');
    await becomes('Pairs', '# Pairs(a)(\n\nType a word here\n');
  });

  it('TC-010-22 "Close pairs" is a setting of "For me", on at first, and a command of the palette', async () => {
    const settings = new MockSettings({}, []);
    expect(settings.effective().closePairs).toBe(true);
    const row = settings.windowView(null).me.find((each) => each.key === 'close_pairs');
    expect(row?.label).toBe('Close pairs');
    expect(row?.control).toMatchObject({ kind: 'check', on: true, text: 'On' });

    const shell = await workbench(SOURCE);
    shell.palette.show('commands');
    await fireEvent.input(await waitFor(paletteField), { target: { value: '>close pairs' } });
    const option = await waitFor(() => within(overlay()!).getByRole('option'));
    expect(option.textContent).toContain('Turn off close pairs');
    await fireEvent.keyDown(paletteField(), { key: 'Enter' });
    await waitFor(() => expect(shell.settings?.closePairs).toBe(false));
  });
});

describe('FR-082 move and duplicate lines (UC-005 A26, BR-020)', () => {
  it('TC-005-36 steps 1 and 2: Alt+Down moves "two" below "three"; Ctrl+Shift+D copies it', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Lines');
    const rest = '\nfour\nfive\n\n- a\n  - a1\n- b\n';

    await fireEvent.keyDown(field, { key: 'ArrowDown' });
    await fireEvent.keyDown(field, { key: 'ArrowDown', altKey: true });
    await becomes('Lines', `one\nthree\ntwo\n${rest}`);
    await fireEvent.keyDown(field, { key: 'D', code: 'KeyD', ctrlKey: true, shiftKey: true });
    await becomes('Lines', `one\nthree\ntwo\ntwo\n${rest}`);
    // The cursor is in the copy, on line 4.
    await waitFor(() => {
      const view = activeSourceView()!.view;
      expect(view.state.doc.lineAt(view.state.selection.main.head).number).toBe(4);
    });
  });

  it('BR-020 the palette moves the line as Alt+Up does, with its key shown for the text', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Lines');
    await fireEvent.keyDown(field, { key: 'ArrowDown' });

    shell.palette.show('commands');
    await fireEvent.input(await waitFor(paletteField), { target: { value: '>move line up' } });
    const option = await waitFor(() => within(overlay()!).getAllByRole('option')[0]);
    expect(option.textContent).toContain('Move line up');
    expect(option.textContent).toContain('Alt+Up');
    expect(option.textContent).toContain('in the note');
    await fireEvent.keyDown(paletteField(), { key: 'Enter' });

    await becomes('Lines', `two\none\nthree\n\nfour\nfive\n\n- a\n  - a1\n- b\n`);
  });
});

describe('FR-083 indent list items (UC-005 A27, BR-021)', () => {
  it('TC-005-37 step 1: Tab in "b" of the facade makes it an item of "a"; Shift+Tab back', async () => {
    const shell = await workbench(FACADE);
    const field = await facadeField(shell, 'List');
    activeFacade()!.jump(COMFORT_TEXTS.List.indexOf('b'));
    await activeFacade()!.mark();

    await fireEvent.keyDown(field, { key: 'Tab' });
    await becomes('List', '- a\n  - b\n- c\n');
    await fireEvent.keyDown(field, { key: 'Tab', shiftKey: true });
    await becomes('List', '- a\n- b\n- c\n');
  });

  it('TC-005-37 step 5: Tab in the source view of an AsciiDoc list writes `** b`', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Bullets');
    await fireEvent.keyDown(field, { key: 'ArrowDown' });

    await fireEvent.keyDown(field, { key: 'Tab' });
    await becomes('Bullets', '* a\n** b\n');
  });
});

describe('OQ-260 Vim `>>` and `<<` on list items (UC-005 BR-021)', () => {
  const VIM_SOURCE: MockStart = { ...SOURCE, settings: { inputMode: 'Vim' } };
  const VIM_FACADE: MockStart = { ...FACADE, settings: { inputMode: 'Vim' } };

  function press(field: HTMLElement, keys: string) {
    for (const key of keys) {
      field.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
    }
  }

  /** The cursor to the start of line `line` (from 0); jsdom lays out no lines for `j`. */
  function toLine(line: number) {
    const view = activeSourceView()!.view;
    view.dispatch({ selection: { anchor: view.state.doc.line(line + 1).from } });
  }

  it('TC-005-37 step 6: `>>` and `<<` on an AsciiDoc item of the source view follow the list rule, `u` undoes each', async () => {
    const shell = await workbench(VIM_SOURCE);
    const field = await sourceField(shell, 'Bullets');
    toLine(1);
    press(field, '>>');
    await becomes('Bullets', '* a\n** b\n');
    // The cursor is the core's: on the marker of "b", so `<<` acts on it.
    await waitFor(() => expect(activeSourceView()!.view.state.selection.main.head).toBe(4));
    press(field, '<<');
    await becomes('Bullets', '* a\n* b\n');
    press(field, 'u');
    await becomes('Bullets', '* a\n** b\n');
  });

  it('a count and visual mode take every item line; elsewhere `>>` shifts as Vim does', async () => {
    const shell = await workbench(VIM_SOURCE);
    const list = await sourceField(shell, 'List');

    toLine(1);
    press(list, 'VG>');
    await becomes('List', '- a\n  - b\n  - c\n');
    press(list, 'u');
    await becomes('List', '- a\n- b\n- c\n');
    press(list, 'gg2>>');
    await becomes('List', '- a\n  - b\n- c\n');

    const lines = await sourceField(shell, 'Lines');
    press(lines, 'gg>>');
    await becomes('Lines', '  one\ntwo\nthree\n\nfour\nfive\n\n- a\n  - a1\n- b\n');
  });

  it('in the facade, `>>` and `<<` on a list item indent and outdent it', async () => {
    const shell = await workbench(VIM_FACADE);
    const field = await facadeField(shell, 'List');
    activeFacade()!.jump(COMFORT_TEXTS.List.indexOf('b'));
    await activeFacade()!.mark();

    type(field, '>>');
    await becomes('List', '- a\n  - b\n- c\n');
    type(field, '<<');
    await becomes('List', '- a\n- b\n- c\n');
  });
});

describe('FR-086 go to line (UC-004 A16, A17, BR-017)', () => {
  it('TC-004-33 steps 1, 2 and 5: Ctrl+G asks, names 300 lines, refuses "abc", goes to 120:8', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Long');

    await fireEvent.keyDown(field, { key: 'g', code: 'KeyG', ctrlKey: true });
    const asked = await waitFor(() => {
      expect(overlay()).not.toBeNull();
      return paletteField();
    });
    expect(asked).toHaveAccessibleName('Go to line');
    expect(overlay()!.textContent).toContain('The note has 300 lines.');

    await fireEvent.input(asked, { target: { value: 'abc' } });
    await fireEvent.keyDown(asked, { key: 'Enter' });
    const alert = await waitFor(() => within(overlay()!).getByRole('alert'));
    expect(alert.textContent).toBe('A line number is expected, such as 120 or 120:8.');
    expect(paletteField()).toHaveValue('abc');

    await fireEvent.input(asked, { target: { value: '120:8' } });
    await fireEvent.keyDown(asked, { key: 'Enter' });
    await waitFor(() => expect(overlay()).toBeNull());
    await waitFor(() => {
      const view = activeSourceView()!.view;
      expect(view.state.selection.main.head).toBe(view.state.doc.line(120).from + 7);
    });
  });

  it('TC-004-33 step 3: in the facade the cursor goes to the block holding the line', async () => {
    const shell = await workbench(FACADE);
    await facadeField(shell, 'Long');

    shell.palette.showLine();
    const asked = await waitFor(paletteField);
    await fireEvent.input(asked, { target: { value: '120' } });
    await fireEvent.keyDown(asked, { key: 'Enter' });

    await waitFor(() => expect(overlay()).toBeNull());
    const lines = COMFORT_TEXTS.Long.split('\n');
    const line120 = lines.slice(0, 119).join('\n').length + 1;
    await waitFor(async () => expect((await activeFacade()!.mark())?.head).toBe(line120));
  });

  it("BR-017 Ctrl+G is the Vim keys' in the text; the palette still asks", async () => {
    const shell = await workbench({ ...SOURCE, settings: { inputMode: 'Vim' } });
    const field = await sourceField(shell, 'Long');

    await fireEvent.keyDown(field, { key: 'g', code: 'KeyG', ctrlKey: true });
    expect(overlay()).toBeNull();
    shell.palette.show('commands');
    await fireEvent.input(await waitFor(paletteField), { target: { value: '>go to line' } });
    await fireEvent.keyDown(paletteField(), { key: 'Enter' });
    await waitFor(() => expect(paletteField()).toHaveAccessibleName('Go to line'));
  });
});

describe('FR-086 review: messages of go to line', () => {
  it('0 says lines start at 1; a line beyond the note says so in the status bar', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Long');
    shell.palette.showLine();
    const asked = await waitFor(paletteField);
    await fireEvent.input(asked, { target: { value: '0:5' } });
    await fireEvent.keyDown(asked, { key: 'Enter' });
    const alert = await waitFor(() => within(overlay()!).getByRole('alert'));
    expect(alert.textContent).toBe('Lines and columns start at 1.');

    await fireEvent.input(asked, { target: { value: '999' } });
    await fireEvent.keyDown(asked, { key: 'Enter' });
    await waitFor(() => expect(overlay()).toBeNull());
    expect(shell.status).toBe('Went to the last line.');
    await waitFor(() => {
      const view = activeSourceView()!.view;
      expect(view.state.doc.lineAt(view.state.selection.main.head).number).toBe(300);
    });
  });

  it('the facade counts the lines of the markup and takes no column hint', async () => {
    const shell = await workbench(FACADE);
    await facadeField(shell, 'Long');
    shell.palette.showLine();
    await waitFor(paletteField);
    expect(overlay()!.textContent).toContain('The note has 300 lines of markup.');
    expect(overlay()!.textContent).not.toContain('colon');
  });

  it('Ctrl+G without an open note says "Open a note first."', async () => {
    const shell = await workbench(SOURCE);
    await fireEvent.keyDown(document.body, { key: 'g', code: 'KeyG', ctrlKey: true });
    expect(overlay()).toBeNull();
    expect(shell.status).toBe('Open a note first.');
  });
});

describe('FR-084, FR-085 current line and matching bracket (UC-005 BR-022)', () => {
  function viewOf(text: string, anchor: number, head = anchor): EditorView {
    const parent = document.createElement('div');
    document.body.append(parent);
    return new EditorView({
      parent,
      state: EditorState.create({
        doc: text,
        selection: { anchor, head },
        extensions: lineAndBrackets(),
      }),
    });
  }

  it('TC-005-38: the line of the cursor and a pair of brackets are marked; none while selected', () => {
    const text = COMFORT_TEXTS.Code;
    const view = viewOf(text, text.indexOf('[') + 1);
    const current = view.dom.querySelectorAll(`.${CURRENT_LINE}`);
    expect(current).toHaveLength(1);
    expect(current[0].textContent).toBe('f(a[1], {b})');
    const pair = [...view.dom.querySelectorAll('.cm-matchingBracket')].map((m) => m.textContent);
    expect(pair).toEqual(['[', ']']);

    view.dispatch({ selection: { anchor: 2, head: 6 } });
    expect(view.dom.querySelectorAll(`.${CURRENT_LINE}`)).toHaveLength(0);

    const unmatched = text.indexOf('g((') + 2;
    view.dispatch({ selection: { anchor: unmatched } });
    expect(view.dom.querySelectorAll('.cm-nonmatchingBracket')).toHaveLength(1);
    view.destroy();
  });
});
