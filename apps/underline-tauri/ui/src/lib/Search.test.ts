// Feature F9: the search overlay (M-03.2, M-13.1, M-13.3, UC-008), the
// marks of its words in the notes (M-13.2) and the find field of a note
// (M-13.4, UC-008 A6), against the mock backend and the editor's core.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { mockHandlers, mockHooks, mockQuery, webAtByte, type MockStart } from './mock-backend';
import { choiceOf, entriesOf } from './search.svelte';
import type { FoundNoteView } from './bindings';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };

async function workbench(start: MockStart = NOTES) {
  const handlers = mockHandlers(start);
  const open = vi.fn(handlers.search_open);
  const set = vi.fn(handlers.search_set);
  const shell = await connectedShell(start, { ...handlers, search_open: open, search_set: set });
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return { shell, open, set };
}

const magnifier = () => screen.getByRole('button', { name: 'Search' });
const field = () => screen.getByRole('combobox', { name: 'Search notes' });
const overlay = () => screen.queryByRole('search', { name: 'Search notes' });
const found = () => screen.getByRole('listbox', { name: 'Found notes' });
const results = () => overlay()!.querySelector<HTMLElement>('.results')!;

/** Types `text` into the overlay's field. */
async function type(text: string) {
  await fireEvent.input(field(), { target: { value: text } });
}

/** Waits until the found list names the note titled `title`. */
async function listed(title: string): Promise<HTMLElement> {
  return waitFor(() => {
    const entry = within(found()).getByText(title);
    return entry.closest<HTMLElement>('[role="option"]')!;
  });
}

describe('M-03.2 the magnifier and Ctrl+Shift+F', () => {
  it('UC-008 step 1 the magnifier opens the overlay with the keyboard in the field, and closes and empties it', async () => {
    const { set } = await workbench();
    expect(overlay()).toBeNull();
    expect(magnifier()).toHaveAttribute('title', 'Search notes  Ctrl+Shift+F');

    await fireEvent.click(magnifier());

    expect(overlay()).not.toBeNull();
    await waitFor(() => expect(field()).toHaveFocus());
    expect(field()).toHaveAttribute('placeholder', 'Search notes');
    await type('ferry');
    await fireEvent.pointerDown(magnifier());
    await fireEvent.click(magnifier());
    expect(overlay()).toBeNull();
    await waitFor(() => expect(set).toHaveBeenLastCalledWith({ text: '' }));

    await fireEvent.click(magnifier());
    expect(field()).toHaveValue('');
  });

  it('BR-007 Ctrl+Shift+F opens it from the text of a note, before the editor sees the key', async () => {
    await workbench();
    const text = await waitFor(() => {
      const editor = document.querySelector<HTMLElement>('.facade-text');
      expect(editor).not.toBeNull();
      return editor!;
    });
    text.focus();

    const event = new KeyboardEvent('keydown', {
      key: 'F',
      ctrlKey: true,
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    text.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(field()).toHaveFocus());
  });
});

describe('M-13.1 the found notes (UC-008 step 2)', () => {
  it('TC-008-01 shows the note with its places and lines, the words marked', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('ferry');

    expect(within(results()).getByText('Searching…')).toBeInTheDocument();
    const trip = await listed('Trip');
    expect(trip).toHaveTextContent('3 places');
    expect(trip).toHaveAttribute('title', 'top folder · 3 places');
    const marks = [...found().querySelectorAll('mark')].map((mark) => mark.textContent);
    expect(marks).toEqual(['ferry', 'FERRY', 'Ferry']);
    expect(within(found()).queryByText('Ideas')).toBeNull();
  });

  it('A1 says when no note holds the words', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('  zebra ');

    await waitFor(() =>
      expect(within(results()).getByText('No note holds “zebra”.')).toBeInTheDocument(),
    );
  });

  it('TC-008-11 A5 shows five lines of a note and how many more places it holds', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('deploy');

    const log = await listed('Log');
    expect(log).toHaveTextContent('8 places');
    expect(within(found()).getAllByText('deploy')).toHaveLength(5);
    expect(within(found()).getByText('and 3 more')).toBeInTheDocument();
  });

  it('A2 emptying the field shows nothing and ends the search', async () => {
    const { shell, set } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await listed('Trip');

    await type('   ');

    expect(screen.queryByRole('listbox')).toBeNull();
    expect(shell.search.result).toBeNull();
    await waitFor(() => expect(set).toHaveBeenLastCalledWith({ text: '   ' }));
  });
});

describe('M-13.3 opening a found place (UC-008 steps 3–4)', () => {
  it('TC-008-01 a click on a line opens the note at the word and keeps the overlay', async () => {
    const { shell, open } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await listed('Trip');
    const line = within(found()).getAllByRole('option')[1];

    await fireEvent.click(line);

    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
    const place = open.mock.calls[0][0].place!;
    const opened = await open.mock.results[0].value;
    expect(opened).toMatchObject({ note: 'Trip' });
    expect(place.to - place.from).toBe(5);
    expect(overlay()).not.toBeNull();
    expect(field()).toHaveValue('ferry');
  });

  it('BR-007 the arrows choose an entry and Enter opens it and closes the overlay', async () => {
    const { shell, open } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await listed('Trip');

    await fireEvent.keyDown(field(), { key: 'ArrowDown' });
    await fireEvent.keyDown(field(), { key: 'ArrowDown' });

    const second = within(found()).getAllByRole('option')[1];
    expect(second).toHaveAttribute('aria-selected', 'true');
    expect(field()).toHaveAttribute('aria-activedescendant', second.id);
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(overlay()).toBeNull();
    await waitFor(() => expect(open).toHaveBeenCalled());
    expect(open.mock.calls[0][0]).toMatchObject({ note: 'Trip' });
    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
  });

  it('Escape anywhere and a press outside close the overlay', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');

    await fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(overlay()).toBeNull();

    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    expect(field()).toHaveValue('');
    await fireEvent.pointerDown(document.body);
    expect(overlay()).toBeNull();
  });
});

describe('M-13.2 the words are marked in the notes (UC-008 BR-008)', () => {
  it('marks the words in the facade of an open note, and the marks go with the search', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('ferry');

    const hits = await waitFor(() => {
      const marked = [...document.querySelectorAll('.facade-text .hit')].map(
        (hit) => hit.textContent,
      );
      expect(marked).toEqual(['ferry', 'FERRY', 'Ferry']);
      return marked;
    });
    expect(hits).toHaveLength(3);

    shell.search.close();

    await waitFor(() => expect(document.querySelectorAll('.facade-text .hit')).toHaveLength(0));
  });

  it('marks the words in the source view', async () => {
    const { shell } = await workbench({ ...NOTES, view: 'source' });
    await shell.openNote('Trip');
    await waitFor(() => expect(activeSourceView()?.note).toBe('Trip'));
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('hydra');

    await waitFor(() => expect(activeSourceView()!.hits()).toHaveLength(2));
  });
});

describe('M-13.4 the find field of a note (UC-008 A6)', () => {
  /** Ctrl+F in the note's text. */
  function ctrlF(target: HTMLElement) {
    const event = new KeyboardEvent('keydown', {
      key: 'f',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
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
  const count = () => document.getElementById('find-count')!;

  it('TC-008-12 Ctrl+F opens it; typing selects the first hit, Enter, F3 and Shift+Enter step with wrapping', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    const text = await tripText();

    expect(ctrlF(text).defaultPrevented).toBe(true);

    await waitFor(() => expect(findField()).toHaveFocus());
    expect(screen.getByText('Find in note:')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Previous' })).toHaveAttribute(
      'title',
      'Shift+Enter, Shift+F3',
    );
    expect(screen.getByRole('button', { name: 'Next' })).toHaveAttribute('title', 'Enter, F3');
    expect(screen.getByRole('button', { name: 'Close' })).toHaveAttribute('title', 'Escape');

    await fireEvent.input(findField(), { target: { value: 'FERRY' } });
    await waitFor(() => expect(count()).toHaveTextContent('1 of 3'));

    await fireEvent.keyDown(findField(), { key: 'Enter' });
    await waitFor(() => expect(count()).toHaveTextContent('2 of 3'));
    await fireEvent.keyDown(document.body, { key: 'F3' });
    await waitFor(() => expect(count()).toHaveTextContent('3 of 3'));
    await fireEvent.keyDown(findField(), { key: 'Enter' });
    await waitFor(() => expect(count()).toHaveTextContent('1 of 3'));
    await fireEvent.keyDown(findField(), { key: 'Enter', shiftKey: true });
    await waitFor(() => expect(count()).toHaveTextContent('3 of 3'));
    expect(findField()).toHaveFocus();
    // The hit selected shows while the keyboard is in the field.
    await waitFor(() =>
      expect(document.querySelector('.facade-text .hit.current')).toHaveTextContent('Ferry'),
    );

    await fireEvent.keyDown(findField(), { key: 'Escape' });

    expect(screen.queryByRole('search', { name: 'Find in note' })).toBeNull();
    await waitFor(() => expect(text).toHaveFocus());
  });

  it('TC-008-13 BR-009 Ctrl+F belongs to the Vim keys in the Vim mode', async () => {
    const { shell } = await workbench({ ...NOTES, settings: { inputMode: 'Vim' } });
    await shell.openNote('Trip');
    const text = await tripText();

    const event = ctrlF(text);

    expect(event.defaultPrevented).toBe(false);
    expect(screen.queryByRole('search', { name: 'Find in note' })).toBeNull();
  });
});

describe('the search state', () => {
  const note: FoundNoteView = {
    note: 'Trip',
    title: 'Trip',
    folder: null,
    places: '2 places',
    lines: [
      { parts: [{ text: 'ferry', hit: true }], place: { from: 3, to: 8 } },
      { parts: [{ text: 'ferry', hit: true }], place: { from: 20, to: 25 } },
    ],
    more: 0,
    place: { from: 3, to: 8 },
  };

  it('egui `choice`: a note opens at its first line, a line at its word', () => {
    expect(entriesOf([note])).toEqual([
      { note: 0, line: null },
      { note: 0, line: 0 },
      { note: 0, line: 1 },
    ]);
    expect(choiceOf(note, null)).toEqual({ note: 'Trip', place: { from: 3, to: 8 } });
    expect(choiceOf(note, 1)).toEqual({ note: 'Trip', place: { from: 20, to: 25 } });
  });

  it('the mock splits words as BR-002 and finds web offsets of bytes', () => {
    expect(mockQuery('Plan  "release plan" PLAN')).toEqual(['plan', 'release plan']);
    expect(webAtByte('😀 ferry', 5)).toBe(3);
    expect(webAtByte('ab', 99)).toBe(2);
  });
});

describe('F9 review fixes', () => {
  it('app-tester #1 the facade marks a block again after it changed within the pause', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await waitFor(() => expect(document.querySelectorAll('.facade-text .hit')).toHaveLength(3));

    // An edit of the first block with hits, right after the marks came.
    const text = mockHooks().mirrorText('Trip')!;
    const end = text.indexOf('Hydra.') + 'Hydra.'.length;
    await mockHooks().backendEdit('Trip', end, 0, ' ferry ferry', true);

    await waitFor(() => expect(document.querySelectorAll('.facade-text .hit')).toHaveLength(5), {
      timeout: 3000,
    });
  });

  it('UX A-96 a list of an older text is dimmed and busy, and Enter waits for the new one', async () => {
    const { shell, open } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await listed('Trip');

    await type('hydra');
    expect(found()).toHaveAttribute('aria-busy', 'true');
    expect(within(results()).getByText('Searching…')).toBeInTheDocument();
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(open).not.toHaveBeenCalled();

    await waitFor(() => expect(open).toHaveBeenCalled());
    expect(overlay()).toBeNull();
  });

  it('UX a failed search says why in the overlay', async () => {
    const shell = await connectedShell(NOTES, {
      ...mockHandlers(NOTES),
      search_set: async () => {
        throw { kind: 'internal', message: 'The search could not start' };
      },
    });
    render(App, { props: { shell } });
    await shell.startWorkspace();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('ferry');

    await waitFor(() =>
      expect(within(overlay()!).getByRole('alert')).toHaveTextContent('The search could not start'),
    );
  });

  it('UX the keyboard goes back where it was when the overlay closes', async () => {
    await workbench();
    const button = screen.getByRole('button', { name: 'Menu' });
    button.focus();
    await fireEvent.keyDown(button, { key: 'F', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(field()).toHaveFocus());

    await fireEvent.keyDown(field(), { key: 'Escape' });

    await waitFor(() => expect(button).toHaveFocus());
  });

  it('UX a live region tells how many notes were found', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await waitFor(() =>
      expect(overlay()!.querySelector('[aria-live]')).toHaveTextContent('1 note found'),
    );
    const line = within(found()).getAllByRole('option')[1];
    expect(line).toHaveAttribute('aria-label', 'Trip: Book the ferry to Hydra.');
  });

  it('A3 a note changed outside is found by the search', async () => {
    const { shell } = await workbench();
    shell.search.show();
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ferry');
    await listed('Trip');

    await mockHooks().outsideChange('Ideas', '# Ideas\n\nferry times\n');

    await listed('Ideas');
  });

  it('app-tester #4 Escape on a button of the find field closes it', async () => {
    const { shell } = await workbench();
    await shell.openNote('Trip');
    shell.search.openFind('ferry');
    const next = await waitFor(() => screen.getByRole('button', { name: 'Next' }));
    next.focus();

    await fireEvent.keyDown(next, { key: 'Escape' });

    expect(shell.search.find).toBeNull();
  });
});
