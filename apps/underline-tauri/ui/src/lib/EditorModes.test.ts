// Feature F4 in the window, against the mock backend: the toolbar (M-10.2),
// the status line (M-11.4), the Nano keys (M-11.3), the Vim keys of the
// source view (M-11.2, A-01), folding (M-08.8), line numbers (M-08.7), the
// marked hits (UC-008 BR-008) and the minimap (M-12.1).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { emit } from '@tauri-apps/api/event';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { mockHandlers, mockHooks, type MockHandlers, type MockStart } from './mock-backend';

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function workbench(start: MockStart = {}, handlers: MockHandlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

// These tests try the source view; a new tab shows the facade (M-07.2).
const NOTES = { givenDir: '/home/user/Notes', view: 'source' as const };

async function textField(): Promise<HTMLElement> {
  await waitFor(() => expect(activeSourceView()).not.toBeNull());
  return document.querySelector<HTMLElement>('.cm-content')!;
}

function press(field: HTMLElement, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  field.dispatchEvent(event);
  return event;
}

function type(field: HTMLElement, text: string) {
  field.dispatchEvent(
    new InputEvent('beforeinput', {
      inputType: 'insertText',
      data: text,
      bubbles: true,
      cancelable: true,
    }),
  );
}

/** ADR-025: the backend's text of Home, once the core sent its edits. */
const backendText = () => mockHooks().mirrorText('Home') ?? '';

const statusLine = () => screen.getByRole('group', { name: 'Editor status' });

describe('M-10.2 the formatting toolbar', () => {
  it('has one button per function with its hint, in the order of egui', async () => {
    await workbench(NOTES);
    const tools = screen.getByRole('group', { name: 'Formatting' });

    const buttons = within(tools).getAllByRole('button');

    expect(buttons.map((b) => b.textContent)).toEqual([
      'H',
      'B',
      'I',
      '</>',
      'List',
      'Quote',
      'Link',
      'Image',
      'Table',
    ]);
    expect(buttons[1]).toHaveAttribute('title', 'Bold (Ctrl+B)');
    expect(buttons[0]).toHaveAccessibleName('Heading (Ctrl+Shift+H)');
  });

  it('a button applies its function in the editor core and puts the keyboard back', async () => {
    await workbench(NOTES);
    const field = await textField();

    await fireEvent.click(screen.getByRole('button', { name: 'Bold (Ctrl+B)' }));

    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^\*\*\*\*/));
    expect(document.activeElement).toBe(field);
    await waitFor(() => expect(backendText()).toMatch(/^\*\*\*\*/));
  });

  it('Ctrl+B is the same function', async () => {
    await workbench(NOTES);
    const field = await textField();

    expect(press(field, 'b', { ctrlKey: true }).defaultPrevented).toBe(true);

    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^\*\*\*\*/));
  });
});

describe('M-11.4 the status line', () => {
  it('shows the place of the cursor and follows typing', async () => {
    await workbench(NOTES);
    const field = await textField();

    expect(within(statusLine()).getByText('Ln 1, Col 1')).toBeInTheDocument();
    type(field, 'x');

    await waitFor(() => expect(within(statusLine()).getByText('Ln 1, Col 2')).toBeInTheDocument());
  });

  it('stores each switch at once', async () => {
    const handlers = mockHandlers(NOTES);
    const quick = vi.fn(handlers.settings_quick);
    await workbench(NOTES, { ...handlers, settings_quick: quick });
    await textField();
    const line = statusLine();

    // jsdom lays nothing out: the line has no width and the short names.
    await fireEvent.click(within(line).getByRole('button', { name: 'Copy' }));
    await fireEvent.click(within(line).getByRole('button', { name: 'Map' }));
    await fireEvent.change(within(line).getByRole('combobox', { name: 'Keys' }), {
      target: { value: 'Nano' },
    });

    expect(quick).toHaveBeenCalledWith({ key: 'copy_on_select', value: 'false' });
    expect(quick).toHaveBeenCalledWith({ key: 'show_minimap', value: 'false' });
    expect(quick).toHaveBeenCalledWith({ key: 'input_mode', value: 'Nano' });
    await waitFor(() =>
      expect(within(line).getByRole('button', { name: 'Map' })).toHaveAttribute(
        'aria-pressed',
        'false',
      ),
    );
  });

  it('without Vim or Nano there is no badge; the choice shows the mode', async () => {
    await workbench(NOTES);
    await textField();

    expect(within(statusLine()).getByRole('combobox', { name: 'Keys' })).toHaveValue('Normal');
    expect(statusLine().querySelector('.badge')).toBeNull();
  });
});

describe('M-11.3 the Nano keys', () => {
  const NANO = { ...NOTES, settings: { inputMode: 'Nano' } };

  it('shows the badge "NANO" with its commands', async () => {
    await workbench(NANO);
    await textField();

    await waitFor(() => expect(statusLine().querySelector('.badge')).toHaveTextContent('NANO'));
    expect(within(statusLine()).getByText('^O')).toBeInTheDocument();
    expect(within(statusLine()).getByText('Cut line')).toBeInTheDocument();
  });

  it('Ctrl+K cuts the line in the editor core; Escape stays in the text', async () => {
    await workbench(NANO);
    const field = await textField();

    press(field, 'k', { ctrlKey: true });
    const escape = press(field, 'Escape');

    await waitFor(() =>
      expect(activeSourceView()?.view.state.doc.toString()).not.toMatch(/^# Home/),
    );
    expect(escape.defaultPrevented).toBe(true);
    expect(screen.getByRole('tab', { name: /Home/ })).not.toHaveFocus();
    await waitFor(() => expect(backendText()).not.toMatch(/^# Home/));
  });

  it('Ctrl+W opens the search prompt in the status line; Enter finds and marks', async () => {
    await workbench(NANO);
    const field = await textField();

    press(field, 'w', { ctrlKey: true });
    await waitFor(() => expect(within(statusLine()).getByText('Search:')).toBeInTheDocument());
    type(field, 'note');
    await waitFor(() => expect(within(statusLine()).getByText('Search: note')).toBeInTheDocument());
    press(field, 'Enter');

    await waitFor(() => expect(document.querySelector('.cm-ul-found')).toHaveTextContent('note'));
    expect(within(statusLine()).getByText('End search')).toBeInTheDocument();
    expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^# Home/);
  });

  it('M-10.9 Ctrl+O stores the note at once', async () => {
    const handlers = mockHandlers(NANO);
    const store = vi.fn(handlers.tab_store);
    const shell = await workbench(NANO, { ...handlers, tab_store: store });
    const field = await textField();

    press(field, 'o', { ctrlKey: true });

    await waitFor(() => expect(store).toHaveBeenCalledWith({ note: 'Home' }));
    await waitFor(() => expect(shell.status).toBe('Stored Home'));
  });
});

describe('M-11.2 the Vim keys of the source view (A-01)', () => {
  const VIM = { ...NOTES, settings: { inputMode: 'Vim' } };

  it('shows the mode it is in (narrow: without "VIM · ")', async () => {
    await workbench(VIM);
    const field = await textField();

    await waitFor(() => expect(statusLine().querySelector('.badge')).toHaveTextContent('NORMAL'));
    expect(within(statusLine()).getByText(':w')).toBeInTheDocument();
    press(field, 'i');
    await waitFor(() => expect(statusLine().querySelector('.badge')).toHaveTextContent('INSERT'));
    expect(within(statusLine()).getByText('normal mode')).toBeInTheDocument();
  });

  it('sends what it changes to the backend after the frame (ADR-025)', async () => {
    const handlers = mockHandlers(VIM);
    const sync = vi.fn(handlers.ed_sync);
    await workbench(VIM, { ...handlers, ed_sync: sync });
    const field = await textField();

    press(field, 'x');

    await waitFor(() =>
      expect(sync).toHaveBeenCalledWith(
        expect.objectContaining({
          batch: expect.objectContaining({ base: 1, edits: [{ at: 0, remove: 1, insert: '' }] }),
        }),
      ),
    );
    expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^ Home/);
    expect(backendText()).toMatch(/^ Home/);
  });

  it('u undoes in the editor core, as one history with the other modes', async () => {
    await workbench(VIM);
    const field = await textField();

    press(field, 'x');
    press(field, 'u');

    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^# Home/));
    await waitFor(() => expect(backendText()).toMatch(/^# Home/));
  });

  it('M-10.9 :w stores the note', async () => {
    const handlers = mockHandlers(VIM);
    const store = vi.fn(handlers.tab_store);
    const shell = await workbench(VIM, { ...handlers, tab_store: store });
    const field = await textField();

    press(field, ':');
    const command = await waitFor(() => {
      const input = document.querySelector<HTMLInputElement>('.cm-vim-panel input');
      expect(input).not.toBeNull();
      return input!;
    });
    command.value = 'w';
    command.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Enter', keyCode: 13, bubbles: true }),
    );

    await waitFor(() => expect(store).toHaveBeenCalledWith({ note: 'Home' }));
    await waitFor(() => expect(shell.status).toBe('Stored Home'));
  });

  it('M-08.10 Escape and Tab stay in the text in normal mode', async () => {
    await workbench(VIM);
    const field = await textField();

    expect(press(field, 'Tab').defaultPrevented).toBe(true);
    press(field, 'Escape');

    expect(screen.getByRole('tab', { name: /Home/ })).not.toHaveFocus();
  });
});

describe('M-10.1, A-01 undo steps of the Vim keys', () => {
  const VIM = { ...NOTES, settings: { inputMode: 'Vim' } };

  it('each command is a step of its own', async () => {
    await workbench(VIM);
    const field = await textField();
    const text = () => activeSourceView()?.view.state.doc.toString();

    press(field, 'x');
    press(field, 'x');
    await waitFor(() => expect(text()).toMatch(/^Home/));
    press(field, 'u');

    await waitFor(() => expect(text()).toMatch(/^ Home/));
  });

  it('shows the register "+ as typed but not done', async () => {
    await workbench(VIM);
    const field = await textField();

    press(field, '"');
    press(field, '+');

    await waitFor(() => expect(statusLine().querySelector('.pending')).toHaveTextContent('"+'));
  });
});

describe('Q-14, NFR-011 Link and Image wait for F6; F6 goes from region to region', () => {
  it('Link and Image are disabled, and Ctrl+K does nothing', async () => {
    const handlers = mockHandlers(NOTES);
    const input = vi.fn(handlers.ed_input);
    await workbench(NOTES, { ...handlers, ed_input: input });
    const field = await textField();

    expect(screen.getByRole('button', { name: 'Link (Ctrl+K)' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Image (Ctrl+Shift+I)' })).toBeDisabled();
    expect(press(field, 'k', { ctrlKey: true }).defaultPrevented).toBe(true);
    await activeSourceView()!.settled();
    expect(input).not.toHaveBeenCalled();
  });

  it('F6 and Shift+F6 cycle tab bar, toolbar, text and status line, also from Vim', async () => {
    await workbench({ ...NOTES, settings: { inputMode: 'Vim' } });
    const field = await textField();
    field.focus();

    const f6 = (shiftKey = false) =>
      document.activeElement!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'F6', shiftKey, bubbles: true, cancelable: true }),
      );
    f6();
    expect(document.activeElement).toBe(
      within(statusLine()).getByRole('button', { name: /^Copy/ }),
    );
    f6();
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: /Home/ }));
    f6();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Facade' }));
    f6(true);
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: /Home/ }));
  });
});

describe('M-08.8 folding', () => {
  it('the arrow folds the section, which then ends on "…"; again expands it', async () => {
    await workbench(NOTES);
    await textField();

    // ADR-025: the editor core finds the sections; the last arrow is the
    // heading's (the first one only keeps the gutter's width).
    const arrows = () => [...document.querySelectorAll<HTMLElement>('.cm-ul-arrow')];
    await waitFor(() => expect(arrows().length).toBeGreaterThan(1));
    await fireEvent.mouseDown(arrows().at(-1)!, { button: 0 });

    await waitFor(() => expect(document.querySelector('.cm-ul-folded')).toHaveTextContent('…'));
    const arrow = document.querySelector<HTMLElement>('.cm-ul-arrow[data-folded="true"]')!;
    expect(arrow.style.visibility).toBe('visible');
    await fireEvent.mouseDown(arrow, { button: 0 });
    await waitFor(() => expect(document.querySelector('.cm-ul-folded')).toBeNull());
  });

  it('Ctrl+Shift+[ folds the section at the cursor', async () => {
    await workbench(NOTES);
    const field = await textField();
    await waitFor(() => expect(document.querySelector('.cm-ul-arrow')).not.toBeNull());

    press(field, '{', { ctrlKey: true, shiftKey: true, code: 'BracketLeft' });

    await waitFor(() => expect(document.querySelector('.cm-ul-folded')).toHaveTextContent('…'));
  });
});

describe('M-08.7 line numbers and M-12.1 the minimap', () => {
  it('shows the numbers on the left, or on the right, or none, as the settings say', async () => {
    const shell = await workbench({ ...NOTES, settings: { showLineNumbers: true } });
    await textField();
    expect(document.querySelector('.cm-lineNumbers')).not.toBeNull();

    await emit('settings', { ...shell.settings!, lineNumberSide: 'Right' });
    await waitFor(() => expect(document.querySelector('.cm-lineNumbers')).toBeNull());
    expect(document.querySelector('.cm-ul-numbers-right')).not.toBeNull();

    await emit('settings', { ...shell.settings!, showLineNumbers: false });
    await waitFor(() => expect(document.querySelector('.cm-ul-numbers-right')).toBeNull());
  });

  it('the minimap is hidden when the editor is narrower than 240 px', async () => {
    await workbench(NOTES);
    await textField();

    // jsdom lays nothing out: the editor has no width.
    expect(document.querySelector('.minimap')).toBeNull();
  });
});
