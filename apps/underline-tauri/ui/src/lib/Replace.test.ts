// Package 1, find and replace: the search switches (FR-075, UC-008 A7,
// A9), replacing in the find field of a note (FR-073, UC-005 A20 to A22)
// with the window's editor core, and the replace of the workspace (FR-074,
// UC-022) in the search overlay, against the mock backend. What matches and
// what a replacement writes is tested in Rust (`underline-markup`,
// `underline-editor-view`, `underline-session`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import {
  mockHandlers,
  mockHooks,
  mockPattern,
  mockReplacement,
  type MockStart,
} from './mock-backend';
import { replaceEntries } from './replace.svelte';
import { NO_SWITCHES, switchKey, switched } from './search.svelte';
import type { ReplaceNoteView } from './bindings';

afterEach(() => {
  try {
    mockHooks().dispose();
  } catch {
    // A test of plain functions made no mock backend.
  }
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };

async function workbench(start: MockStart = NOTES) {
  const handlers = mockHandlers(start);
  const confirm = vi.fn(handlers.replace_confirm);
  const set = vi.fn(handlers.replace_set);
  const shell = await connectedShell(start, {
    ...handlers,
    replace_confirm: confirm,
    replace_set: set,
  });
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return { shell, confirm, set };
}

/** A key pressed on `target`, as the window gets it. */
function press(target: Element, init: KeyboardEventInit) {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}

async function tripText(): Promise<HTMLElement> {
  return waitFor(() => {
    const text = document.querySelector<HTMLElement>('.facade-text');
    expect(text?.querySelector('[data-b="0"]')).toBeTruthy();
    return text!;
  });
}

const findField = () => screen.getByRole('textbox', { name: 'Find in note:' });
const replaceField = () => screen.getByRole('textbox', { name: 'Replace with:' });
const count = () => document.getElementById('find-count')!;
const findBar = () => screen.getByRole('search', { name: 'Find in note' });
const overlay = () => screen.getByRole('search', { name: /notes$/ });
const searchField = () => screen.getByRole('combobox', { name: 'Search notes' });
const places = () => screen.getByRole('listbox', { name: 'Places to replace' });

describe('FR-075 the search switches', () => {
  it('Alt+C, Alt+W and Alt+R name the switch by the key, also when Option types a character', () => {
    const alt = (code: string, key = 'x') =>
      switchKey(new KeyboardEvent('keydown', { code, key, altKey: true }));
    expect(alt('KeyC', 'ç')).toBe('matchCase');
    expect(alt('KeyW')).toBe('wholeWord');
    expect(alt('KeyR')).toBe('regex');
    expect(alt('KeyX')).toBeNull();
    expect(switchKey(new KeyboardEvent('keydown', { code: 'KeyC', ctrlKey: true }))).toBeNull();
    expect(switched(NO_SWITCHES, 'regex')).toEqual({ ...NO_SWITCHES, regex: true });
  });

  it('TC-008-19 Alt+C and Alt+W in the find field switch and count again', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();
    press(text, { key: 'f', ctrlKey: true });
    await waitFor(() => expect(findField()).toHaveFocus());
    await fireEvent.input(findField(), { target: { value: 'ferry' } });
    await waitFor(() => expect(count()).toHaveTextContent('of 3'));

    await fireEvent.keyDown(findField(), { code: 'KeyC', key: 'c', altKey: true });
    const matchCase = within(findBar()).getByRole('button', { name: 'Match case' });
    expect(matchCase).toHaveAttribute('aria-pressed', 'true');
    expect(matchCase).toHaveAttribute('title', 'Match case (Alt+C)');
    await waitFor(() => expect(count()).toHaveTextContent('1 of 1'));

    await fireEvent.keyDown(findField(), { code: 'KeyC', key: 'c', altKey: true });
    await fireEvent.keyDown(findField(), { code: 'KeyW', key: 'w', altKey: true });
    await waitFor(() => expect(count()).toHaveTextContent('of 2'));
    expect(findField()).toHaveFocus();
  });

  it('TC-008-20 an expression that is not valid says what is wrong in the find field', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();
    press(text, { key: 'f', ctrlKey: true });
    await waitFor(() => expect(findField()).toHaveFocus());
    await fireEvent.click(within(findBar()).getByRole('button', { name: 'Regular expression' }));
    await fireEvent.input(findField(), { target: { value: '(fer' } });

    await waitFor(() =>
      expect(document.getElementById('find-count')).toHaveTextContent(
        'Not a valid regular expression: unclosed group.',
      ),
    );
    expect(within(findBar()).getByRole('button', { name: 'Next' })).toBeDisabled();
    expect(findField()).toHaveAttribute('aria-invalid', 'true');
  });

  it('TC-008-18 the overlay searches with its switches; A9 says what is wrong', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(searchField()).toHaveFocus());
    await fireEvent.keyDown(searchField(), { code: 'KeyR', key: 'r', altKey: true });
    expect(within(overlay()).getByRole('button', { name: 'Regular expression' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await fireEvent.input(searchField(), { target: { value: '(plan' } });
    await waitFor(() =>
      expect(overlay().querySelector('.problem')?.textContent).toMatch(
        /^Not a valid regular expression/,
      ),
    );
    expect(screen.queryByRole('listbox', { name: 'Found notes' })).toBeNull();
  });
});

describe('FR-073 replacing in the find field of a note (UC-005 A20 to A22)', () => {
  it('TC-005-29 Ctrl+H opens it for replacing; Enter replaces the selected one and selects the next', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();

    expect(press(text, { key: 'h', ctrlKey: true }).defaultPrevented).toBe(true);

    await waitFor(() => expect(findField()).toHaveFocus());
    expect(within(findBar()).getByRole('button', { name: 'Replace in this note' })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
    await fireEvent.input(findField(), { target: { value: 'ferry' } });
    await waitFor(() => expect(count()).toHaveTextContent('1 of 3'));
    await fireEvent.input(replaceField(), { target: { value: 'boat' } });

    await fireEvent.keyDown(replaceField(), { key: 'Enter' });

    await waitFor(() => expect(count()).toHaveTextContent('1 of 2'));
    await waitFor(() =>
      expect(mockHooks().mirrorText('Trip')).toContain('Book the boat to Hydra.'),
    );
    expect(mockHooks().mirrorText('Trip')).toContain('the FERRY leaves');
  });

  it('TC-005-30 Ctrl+Alt+Enter replaces all as one change and says how many', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();
    press(text, { key: 'h', ctrlKey: true });
    await waitFor(() => expect(findField()).toHaveFocus());
    await fireEvent.input(findField(), { target: { value: 'ferry' } });
    await waitFor(() => expect(count()).toHaveTextContent('of 3'));
    await fireEvent.input(replaceField(), { target: { value: 'boat' } });

    await fireEvent.keyDown(replaceField(), { key: 'Enter', ctrlKey: true, altKey: true });

    await waitFor(() => expect(count()).toHaveTextContent('3 replaced'));
    await waitFor(() => expect(mockHooks().mirrorText('Trip')).toContain('the boatman knows'));
    expect(mockHooks().mirrorText('Trip')).not.toMatch(/ferry/i);
  });

  it('TC-005-31 A22 no occurrence, or what is wrong: nothing can be replaced', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();
    press(text, { key: 'h', ctrlKey: true });
    await waitFor(() => expect(findField()).toHaveFocus());

    await fireEvent.input(findField(), { target: { value: 'zebra' } });
    await waitFor(() => expect(count()).toHaveTextContent('No occurrence'));
    expect(within(findBar()).getByRole('button', { name: 'Replace all' })).toBeDisabled();

    await fireEvent.click(within(findBar()).getByRole('button', { name: 'Regular expression' }));
    await fireEvent.input(findField(), { target: { value: '(f)' } });
    await fireEvent.input(replaceField(), { target: { value: '$2' } });
    await waitFor(() =>
      expect(document.getElementById('find-count')).toHaveTextContent(
        'The replacement names $2, but the expression has no group 2.',
      ),
    );
    expect(within(findBar()).getByRole('button', { name: /^Replace$/ })).toBeDisabled();
  });
});

describe('FR-074 the replace of the workspace (UC-022)', () => {
  async function replaceIn(text: string, replacement: string) {
    await fireEvent.input(searchField(), { target: { value: text } });
    const field = screen.getByRole('textbox', { name: 'Replace with' });
    await fireEvent.input(field, { target: { value: replacement } });
    return field;
  }

  it('TC-022-01 see, leave out and replace; the status offers to undo', async () => {
    const { shell, confirm } = await workbench();
    press(document.body, { key: 'H', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(searchField()).toHaveFocus());
    expect(screen.getByRole('search', { name: 'Replace in notes' })).toBeInTheDocument();

    await replaceIn('ferry', 'boat');

    await waitFor(() =>
      expect(overlay().querySelector('.summary')?.textContent).toMatch(/places? in \d+ notes?$/),
    );
    const trip = within(places()).getByRole('option', { name: /^Trip, \d+ places/ });
    expect(trip).toHaveAttribute('aria-selected', 'true');
    const line = within(places()).getByRole('option', {
      name: "line 3: 'ferry' becomes 'boat'",
    });
    expect(line.querySelector('del')).toHaveTextContent('ferry');
    expect(line.querySelector('ins')).toHaveTextContent('boat');

    // Leave the first line of Trip out.
    await fireEvent.click(line.querySelector('input')!);
    expect(line).toHaveAttribute('aria-selected', 'false');
    const button = await waitFor(() =>
      within(overlay()).getByRole('button', { name: 'Replace 2 places in 1 note' }),
    );
    expect(overlay().querySelector('.head')).toHaveTextContent('2 of 3 places chosen');

    await fireEvent.click(button);

    await waitFor(() => expect(confirm).toHaveBeenCalled());
    await waitFor(() => expect(shell.status).toBe('Replaced 2 places in 1 note'));
    expect(shell.statusOffer).toBe('undoReplace');
    // UX: the keyboard stays in the overlay, which tells what was done.
    expect(document.activeElement).not.toBe(document.body);
    await waitFor(() =>
      expect(overlay().querySelector('.said')).toHaveTextContent('Replaced 2 places in 1 note'),
    );
    expect(mockHooks().noteText('Trip')).toContain('Book the ferry to Hydra.');
    expect(mockHooks().noteText('Trip')).toContain('the boat leaves');

    await fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(shell.status).toMatch(/^Took back the replacement in 1 note/));
    expect(mockHooks().noteText('Trip')).toContain('the FERRY leaves');
  });

  it('TC-008-22 UC-008 A10 "Replace" takes over the text and the switches of the search', async () => {
    const { shell, set } = await workbench();
    shell.search.show();
    await waitFor(() => expect(searchField()).toHaveFocus());
    await fireEvent.keyDown(searchField(), { code: 'KeyW', key: 'w', altKey: true });
    await fireEvent.input(searchField(), { target: { value: 'ferry' } });

    await fireEvent.click(within(overlay()).getByRole('button', { name: 'Replace in notes' }));

    await waitFor(() =>
      expect(set).toHaveBeenLastCalledWith({
        text: 'ferry',
        replacement: '',
        switches: { ...NO_SWITCHES, wholeWord: true },
      }),
    );
    expect(searchField()).toHaveValue('ferry');
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Replace with' })).toHaveFocus(),
    );
  });

  it('TC-022-12 A8 Escape closes the replace; the search keeps its text', async () => {
    const { shell, confirm } = await workbench();
    shell.search.showReplace();
    await waitFor(() => expect(searchField()).toHaveFocus());
    await replaceIn('ferry', 'boat');

    await fireEvent.keyDown(searchField(), { key: 'Escape' });

    expect(screen.getByRole('search', { name: 'Search notes' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: 'Replace with' })).toBeNull();
    expect(searchField()).toHaveValue('ferry');
    expect(confirm).not.toHaveBeenCalled();
    expect(mockHooks().noteText('Trip')).toContain('Book the ferry');
  });

  it('TC-022-05 A1 nothing found offers no replacement; A2 says what is wrong', async () => {
    const { shell } = await workbench();
    shell.search.showReplace();
    await waitFor(() => expect(searchField()).toHaveFocus());
    await replaceIn('zebra', 'x');
    await waitFor(() =>
      expect(
        within(overlay().querySelector<HTMLElement>('.results')!).getByText(
          /^No note holds “zebra”/,
        ),
      ).toBeInTheDocument(),
    );
    expect(within(overlay()).getByRole('button', { name: /^Replace$/ })).toHaveAttribute(
      'aria-disabled',
      'true',
    );

    await fireEvent.keyDown(searchField(), { code: 'KeyR', key: 'r', altKey: true });
    await replaceIn('(fer', 'x');
    await waitFor(() =>
      expect(overlay().querySelector('.problem')?.textContent).toMatch(
        /^Not a valid regular expression/,
      ),
    );
  });

  it('TC-022-17 BR-008 by keyboard alone: Down into the list, Space leaves out, Ctrl+Alt+Enter replaces', async () => {
    const { shell, confirm } = await workbench();
    press(document.body, { key: 'h', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(searchField()).toHaveFocus());
    const field = await replaceIn('ferry', 'boat');
    await waitFor(() => expect(overlay().querySelector('.summary')).not.toBeNull());

    await fireEvent.keyDown(field, { key: 'ArrowDown' });
    await waitFor(() => expect(places()).toHaveFocus());
    await fireEvent.keyDown(places(), { key: 'ArrowDown' });
    await fireEvent.keyDown(places(), { key: ' ' });
    const place = places().querySelectorAll<HTMLElement>('[role="option"]')[1];
    expect(place).toHaveClass('cursor');
    expect(place).toHaveAttribute('aria-selected', 'false');

    await fireEvent.keyDown(places(), { key: 'Enter', ctrlKey: true, altKey: true });

    await waitFor(() => expect(confirm).toHaveBeenCalled());
    await waitFor(() => expect(shell.status).toMatch(/^Replaced/));
  });
});

describe('package 1 review fixes', () => {
  it('UX a click outside keeps the replace with its replacement; Tab goes from the search field to "Replace with"', async () => {
    const { shell } = await workbench();
    shell.search.showReplace();
    await waitFor(() => expect(searchField()).toHaveFocus());
    await fireEvent.input(searchField(), { target: { value: 'ferry' } });
    const field = screen.getByRole('textbox', { name: 'Replace with' });
    await fireEvent.input(field, { target: { value: 'boat' } });

    const order = [...overlay().querySelectorAll<HTMLElement>('input, button')].filter(
      (each) => each.tabIndex >= 0,
    );
    expect(order.indexOf(field)).toBe(order.indexOf(searchField()) + 1);

    await fireEvent.pointerDown(document.body);
    expect(screen.getByRole('textbox', { name: 'Replace with' })).toHaveValue('boat');
    await waitFor(() =>
      expect(overlay().querySelector('.legend')).toHaveTextContent(
        'Space: leave out · Ctrl+Enter: replace in this note',
      ),
    );
  });

  it('app-tester a change from outside keeps the keyboard in the find field', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();
    press(text, { key: 'f', ctrlKey: true });
    await waitFor(() => expect(findField()).toHaveFocus());

    await mockHooks().outsideChange('Trip', '# Trip\n\nBook the ferry, changed.\n');

    await new Promise((done) => setTimeout(done, 200));
    expect(findField()).toHaveFocus();
  });
});

describe('the replace state', () => {
  it('lists each note, then its places', () => {
    const note = (name: string, n: number): ReplaceNoteView => ({
      note: name,
      title: name,
      folder: null,
      count: `${n} places`,
      leftOut: false,
      places: Array.from({ length: n }, (_, i) => ({
        at: i,
        line: i + 1,
        before: '',
        old: 'a',
        new: 'b',
        after: '',
        leftOut: false,
      })),
    });
    expect(replaceEntries([note('A', 2), note('B', 1)])).toEqual([
      { note: 0, place: null },
      { note: 0, place: 0 },
      { note: 0, place: 1 },
      { note: 1, place: null },
      { note: 1, place: 0 },
    ]);
  });

  it('the mock matches and writes roughly as the backend', () => {
    const plain = mockPattern('a.b', NO_SWITCHES);
    expect('regex' in plain && 'xa.bx A.B axb'.match(plain.regex)).toEqual(['a.b', 'A.B']);
    const whole = mockPattern('plan', { ...NO_SWITCHES, wholeWord: true });
    expect('regex' in whole && 'plan planning Plan'.match(whole.regex)).toEqual(['plan', 'Plan']);
    expect(mockReplacement('$2', { ...NO_SWITCHES, regex: true }, 1)).toEqual({
      problem: 'The replacement names $2, but the expression has no group 2.',
    });
    expect(mockReplacement('$1', NO_SWITCHES, 0)).toEqual({ with: '$$1' });
  });
});
