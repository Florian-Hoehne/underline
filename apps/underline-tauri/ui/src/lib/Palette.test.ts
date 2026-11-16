// Package 2: go to note (FR-076, UC-004 A12, A13, BR-014, TC-004-25 to
// TC-004-28) and the command palette (FR-077, UC-023, TC-023-01 to
// TC-023-11), one overlay, against the mock backend and the ranking in the
// editor's WASM.

import { cleanup, fireEvent, render, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { tick } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { NO_GIT, NO_NOTE } from './commandList';
import { keyClashes, keyText } from './commands';
import { LARGE, mockHandlers, mockHooks, PLANS, type MockStart } from './mock-backend';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const START: MockStart = { givenDir: PLANS };

async function workbench(start: MockStart = START) {
  const handlers = mockHandlers(start);
  const quick = vi.fn(handlers.settings_quick);
  const tabOpen = vi.fn(handlers.tab_open);
  const shell = await connectedShell(start, {
    ...handlers,
    settings_quick: quick,
    tab_open: tabOpen,
  });
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return { shell, quick, tabOpen };
}

const overlay = () => document.querySelector<HTMLElement>('.palette');
const field = () => within(overlay()!).getByRole('combobox');
const options = () => within(overlay()!).queryAllByRole('option');
const names = () => options().map((o) => o.querySelector('.name')!.textContent);
const chosen = () => options().find((o) => o.getAttribute('aria-selected') === 'true');

/** Types `text` into the overlay's field. */
async function type(text: string) {
  await fireEvent.input(field(), { target: { value: text } });
}

/** Presses `key` where the keyboard is (or on `target`). */
function press(
  key: string,
  modifiers: KeyboardEventInit = {},
  target: Element = document.activeElement ?? document.body,
): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...modifiers,
  });
  target.dispatchEvent(event);
  return event;
}

/** The text of the note of the active tab, with the keyboard in it. */
async function noteText(): Promise<HTMLElement> {
  const text = await waitFor(() => {
    const editor = document.querySelector<HTMLElement>('.facade-text, .cm-content');
    expect(editor).not.toBeNull();
    return editor!;
  });
  text.focus();
  return text;
}

/** Opens the note `note` in the active tab and waits for its text. */
async function open(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  await waitFor(() => expect(shell.tabs.active).toBe(note));
  return noteText();
}

describe('UC-023 the command palette', () => {
  it('TC-023-01 Ctrl+Shift+P finds the minimap by "mini", with its menu and key, and runs it', async () => {
    const { shell, quick } = await workbench({ ...START, settings: { showMinimap: true } });
    const text = await open(shell, 'Plan');

    const event = press('P', { ctrlKey: true, shiftKey: true }, text);

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(field()).toHaveFocus());
    expect(field()).toHaveValue('>');
    await type('>mini');
    expect(chosen()).toHaveTextContent('Hide the minimap');
    expect(chosen()).toHaveTextContent('Status line');
    await fireEvent.keyDown(field(), { key: 'Enter' });

    expect(overlay()).toBeNull();
    expect(quick).toHaveBeenCalledWith({ key: 'show_minimap', value: 'false' });
    expect(text).toHaveFocus();
  });

  it('TC-023-01 F1 and "export pdf" lists the export to PDF first; Enter asks where to save (UC-009)', async () => {
    const { shell } = await workbench();
    const text = await open(shell, 'Plan');
    const exporting = vi.spyOn(shell, 'startExport').mockResolvedValue();

    press('F1', {}, text);
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('>export pdf');

    expect(names()[0]).toBe('PDF…');
    expect(options()[0]).toHaveTextContent('Note › Export');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(exporting).toHaveBeenCalledWith('Plan', 'pdf');
  });

  it('TC-023-11 keys are shown as the platform names them', async () => {
    for (const [platform, key] of [
      ['linux', 'Ctrl+F'],
      ['macos', 'Cmd+F'],
    ] as const) {
      const { shell } = await workbench({ ...START, platform });
      shell.palette.show('commands');
      await waitFor(() => expect(overlay()).not.toBeNull());
      await type('>find in note');
      expect(options()[0].querySelector('kbd')).toHaveTextContent(key);
      cleanup();
    }
  });

  it('TC-023-02 words match name and menu in any order and case; a name that starts with it first', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>NOTE NEW');
    expect(names()).toContain('New note…');
    await type('>theme');
    expect(names()[0]).toBe('Theme…');
  });

  it('TC-023-02 BR-004, OQ-198 the command run last is listed first with an empty field, also after a restart', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    const before = names();
    expect(before[0]).not.toMatch(/the minimap$/);

    await type('>minimap');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    // The switch names what it does now (shown or hidden).
    expect(names()[0]).toMatch(/^(Show|Hide) the minimap$/);
    // The backend remembers it for the user.
    const remembered = mockHooks().recentCommands();
    expect(remembered).toHaveLength(1);
    expect(shell.palette.recent).toEqual(remembered);

    // TC-023-02 step 4: after a restart the backend gives them back.
    cleanup();
    const again = await workbench({ ...START, recentCommands: remembered });
    again.shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    expect(names()[0]).toMatch(/^(Show|Hide) the minimap$/);
    expect(names()).not.toEqual(before);
  });

  it('OQ-198 ids no command has any more are dropped at the start, and repeats', async () => {
    const { shell } = await workbench();
    const known = shell.commands.slice(0, 2).map((command) => command.id);
    cleanup();
    const again = await workbench({
      ...START,
      recentCommands: ['gone.command', known[1], known[0], known[1], 'renamed.since'],
    });
    expect(again.shell.palette.recent).toEqual([known[1], known[0]]);
  });

  it('BR-004 an empty palette heads the recent commands "Recently used", the rest "All commands"', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    expect(overlay()!.querySelectorAll('.group')).toHaveLength(0);
    await type('>minimap');
    await fireEvent.keyDown(field(), { key: 'Enter' });

    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    const groups = [...overlay()!.querySelectorAll('.group')].map((g) => g.textContent?.trim());
    expect(groups).toEqual(['Recently used', 'All commands']);
    expect(options()[0]).toHaveAccessibleDescription('Recently used');
    expect(options()[1]).not.toHaveAccessibleDescription('Recently used');
    // While typing, no headings.
    await type('>mini');
    expect(overlay()!.querySelectorAll('.group')).toHaveLength(0);
  });

  it('BR-004 a command that asks for a choice counts as run when chosen ("Theme…")', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('>theme');
    expect(chosen()).toHaveTextContent('Theme…');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    await waitFor(() => expect(shell.palette.mode).toBe('choices'));
    expect(mockHooks().recentCommands()).toEqual(['theme.choose']);
    expect(shell.palette.recent).toEqual(['theme.choose']);
  });

  it('OQ-198 at most 20 recently run commands are kept', async () => {
    const { shell } = await workbench();
    const ids = shell.commands.slice(0, 25).map((command) => command.id);
    expect(ids).toHaveLength(25);
    shell.palette.remember(ids, shell.commands);
    expect(shell.palette.recent).toEqual(ids.slice(0, 20));
  });

  it('TC-023-03 every command of the menus, the toolbar and the context menus is found under its name', async () => {
    const { shell } = await workbench();
    await open(shell, 'Plan');
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    const menus = [
      'New note…',
      'Open workspace…',
      'New workspace…',
      'Theme…',
      'Settings…',
      // The context menus of a note and of its tab.
      'Rename…',
      'Delete…',
      'Export…',
      'AsciiDoc…',
      'HTML…',
      'PDF…',
      // The formatting toolbar.
      ...(shell.start?.tools ?? []).map((tool) =>
        tool.tool === 'link' || tool.tool === 'image'
          ? `Insert ${tool.name.toLowerCase()}…`
          : tool.tool === 'table'
            ? 'Insert table'
            : `Format as ${tool.name.toLowerCase()}`,
      ),
      'Show facade view',
      'Show source view',
      shell.settings?.showMinimap ? 'Hide the minimap' : 'Show the minimap',
      shell.settings?.copyOnSelect ? 'Turn off copy on select' : 'Turn on copy on select',
      'Choose the input mode…',
      'Search notes',
      'Replace in notes',
      'Find in note',
      'Replace in note',
      'Go to note…',
    ];
    for (const name of menus) {
      await type(`>${name}`);
      expect(names(), name).toContain(name);
    }

    await type('>qqx');
    expect(options()).toHaveLength(0);
  });

  it('TC-023-04 A1 says no command matches', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>xyzzy');

    expect(within(overlay()!).getByText('No command matches “xyzzy”.')).toBeInTheDocument();
  });

  it('TC-023-05 A2 a command not available is listed last, marked, and says why when chosen', async () => {
    const { shell } = await workbench();
    expect(shell.tabs.active).toBeNull();
    const commit = vi.spyOn(shell, 'commitChanges');
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>commit');
    const entry = options().at(-1)!;
    expect(entry).toHaveTextContent('Commit');
    expect(entry).toHaveTextContent('not available');
    expect(entry).toHaveAttribute('aria-disabled', 'true');
    await fireEvent.click(entry);
    expect(overlay()).not.toBeNull();
    expect(within(overlay()!).getByRole('alert')).toHaveTextContent(NO_GIT);
    expect(commit).not.toHaveBeenCalled();

    await type('>export');
    expect(options().length).toBeGreaterThan(0);
    for (const option of options()) {
      expect(option).toHaveTextContent('not available');
    }
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(within(overlay()!).getByRole('alert')).toHaveTextContent(NO_NOTE);
  });

  it('TC-023-05 BR-002 a command not available follows those that are', async () => {
    const { shell } = await workbench();
    await open(shell, 'Plan');
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    // "Find next" needs the find field open; "Find in note" can run.
    await type('>find');
    const marked = options().map((o) => o.getAttribute('aria-disabled') === 'true');
    expect(marked).toEqual([...marked].sort((a, b) => Number(a) - Number(b)));
    expect(marked).toContain(true);
    expect(marked).toContain(false);
  });

  it('TC-023-06 A3 the theme is chosen in the same field, the current one marked', async () => {
    const { shell, quick } = await workbench({ ...START, settings: { theme: 'Light' } });
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>theme');
    await fireEvent.keyDown(field(), { key: 'Enter' });

    expect(overlay()).toHaveAccessibleName('Theme');
    expect(field()).toHaveValue('');
    expect(chosen()).toHaveTextContent('Light');
    expect(chosen()).toHaveTextContent('current');
    await type('nord');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(quick).toHaveBeenCalledWith({ key: 'theme', value: 'Nord' });
    expect(overlay()).toBeNull();
  });

  it('TC-023-07 A4 deleting the ">" turns the field into go to note, and ">" back', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>plan');
    expect(overlay()).toHaveAccessibleName('Command palette');
    await type('plan');

    expect(overlay()).toHaveAccessibleName('Go to note');
    expect(names()).toContain('Plan');
    await type('>plan');
    expect(overlay()).toHaveAccessibleName('Command palette');
  });

  it('TC-023-08 A5 Escape or a click outside runs nothing and gives the keyboard back', async () => {
    const { shell, quick } = await workbench();
    const text = await open(shell, 'Plan');

    press('P', { ctrlKey: true, shiftKey: true }, text);
    await waitFor(() => expect(field()).toHaveFocus());
    await type('>mini');
    await fireEvent.keyDown(field(), { key: 'Escape' });
    expect(overlay()).toBeNull();
    expect(text).toHaveFocus();

    press('P', { ctrlKey: true, shiftKey: true }, text);
    await waitFor(() => expect(field()).toHaveFocus());
    await type('>mini');
    await fireEvent.pointerDown(document.body);
    expect(overlay()).toBeNull();
    await waitFor(() => expect(text).toHaveFocus());
    expect(quick).not.toHaveBeenCalledWith(expect.objectContaining({ key: 'show_minimap' }));
  });

  it('TC-023-09 opens with Ctrl+Shift+P in Vim and with F1 in Nano', async () => {
    for (const inputMode of ['Vim', 'Nano']) {
      const { shell } = await workbench({ ...START, settings: { inputMode } });
      const text = await open(shell, 'Plan');
      const event =
        inputMode === 'Vim'
          ? press('P', { ctrlKey: true, shiftKey: true }, text)
          : press('F1', {}, text);
      expect(event.defaultPrevented).toBe(true);
      await waitFor(() => expect(overlay()).toHaveAccessibleName('Command palette'));
      cleanup();
    }
  });

  it('BR-005 a command run from the palette goes back to the text it was asked from first', async () => {
    const { shell } = await workbench();
    const text = await open(shell, 'Plan');
    let focusedWhenRun: Element | null = null;
    vi.spyOn(shell, 'startRename').mockImplementation(() => {
      focusedWhenRun = document.activeElement;
      return Promise.resolve();
    });

    press('F1', {}, text);
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('>rename');
    await fireEvent.keyDown(field(), { key: 'Enter' });

    expect(focusedWhenRun).toBe(text);
  });
});

describe('UC-004 A12 go to note', () => {
  it('TC-004-25 Ctrl+P and "pack" lists Packing with its folder; Enter opens it; an open note becomes active (A1)', async () => {
    const { shell, tabOpen } = await workbench();
    const text = await open(shell, 'Ideas');

    const event = press('p', { ctrlKey: true }, text);

    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(overlay()).toHaveAccessibleName('Go to note'));
    expect(field()).toHaveValue('');
    await type('pack');
    expect(chosen()).toHaveTextContent('Packing');
    expect(chosen()).toHaveTextContent('travel');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(overlay()).toBeNull();
    await waitFor(() => expect(shell.tabs.active).toBe('travel/Packing'));
    expect(tabOpen).toHaveBeenLastCalledWith({ note: 'travel/Packing' });
    await waitFor(() => expect(shell.overview[0].note).toBe('travel/Packing'));

    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('ide');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    await waitFor(() => expect(shell.tabs.active).toBe('Ideas'));
  });

  it('TC-004-26 the title start first, then a word start, then the rest; within each the overview order', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());
    const order = shell.overview.map((item) => item.title);
    const inOrder = (titles: string[]) =>
      [...titles].sort((a, b) => order.indexOf(a) - order.indexOf(b));

    await type('plan');
    // A13: creating a note with the text is offered last.
    expect(names()).toEqual([
      ...inOrder(['Plan', 'Plane Tickets']),
      'Release Plan',
      'Explanation',
      'New note “plan”…',
    ]);

    await type('travel tick');
    expect(names()).toEqual(['Plane Tickets', 'New note “travel tick”…']);

    await type('uber');
    expect(names()).toEqual(['New note “uber”…']);
    await type('ÜBER');
    expect(names()).toEqual(['Über uns', 'New note “ÜBER”…']);

    await type('');
    expect(names()).toEqual(order);
  });

  it('TC-004-27 A13 says no note matches and offers to create it; accepting opens "New note" with the title', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('Budget 2027');

    expect(within(overlay()!).getByText('No note matches “Budget 2027”.')).toBeInTheDocument();
    expect(chosen()).toHaveTextContent('New note “Budget 2027”…');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    await waitFor(() => expect(shell.notes.newNote?.title).toBe('Budget 2027'));
    expect(overlay()).toBeNull();
    // The dialog's "File: …" is planned with what it holds.
    await waitFor(() => expect(shell.notes.newNote?.file).toContain('Budget'));
  });

  it("BR-014 Ctrl+P is the Vim keys' in the text, but opens go to note elsewhere", async () => {
    const { shell } = await workbench({ ...START, settings: { inputMode: 'Vim' } });
    const text = await open(shell, 'Plan');

    const inText = press('p', { ctrlKey: true }, text);
    expect(inText.defaultPrevented).toBe(false);
    expect(overlay()).toBeNull();

    const entry = document.querySelector<HTMLElement>('[data-focus-fallback] button.entry')!;
    entry.focus();
    press('p', { ctrlKey: true }, entry);
    await waitFor(() => expect(overlay()).toHaveAccessibleName('Go to note'));
  });

  it('says so without a workspace', async () => {
    const shell = await connectedShell({});
    render(App, { props: { shell } });
    await shell.startWorkspace();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());

    expect(within(overlay()!).getByText('No workspace is open.')).toBeInTheDocument();
    await type('plan');
    expect(options()).toHaveLength(0);
  });

  it('Down and Up move round the list; a click opens the note', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('plan');

    await fireEvent.keyDown(field(), { key: 'ArrowUp' });
    expect(chosen()).toHaveTextContent('New note “plan”…');
    await fireEvent.keyDown(field(), { key: 'ArrowUp' });
    expect(chosen()).toHaveTextContent('Explanation');
    await fireEvent.keyDown(field(), { key: 'ArrowDown' });
    await fireEvent.keyDown(field(), { key: 'ArrowDown' });
    expect(chosen()).toBe(options()[0]);
    expect(field()).toHaveAttribute('aria-activedescendant', options()[0].id);

    await fireEvent.click(options()[2]);
    await waitFor(() => expect(shell.tabs.active).toBe('Release-Plan'));
  });
});

/**
 * Opening the mock's workspace of 10,000 notes takes seconds on a busy
 * machine; the 200 ms are measured per key inside the test.
 */
const LARGE_TIMEOUT = 30_000;

describe('NFR-018 suggestions within 200 ms in 10,000 notes', () => {
  it(
    'TC-004-28, TC-023-10 the list follows 20 different texts within 200 ms each',
    async () => {
      const { shell } = await workbench({ givenDir: LARGE });
      expect(shell.overview).toHaveLength(10_000);
      // Opening makes the ranking's copy of the 10,000 notes and lists the first.
      const opening = performance.now();
      shell.palette.show('notes');
      await tick();
      const opened = performance.now() - opening;
      expect(overlay()).not.toBeNull();
      expect(opened).toBeLessThan(200);
      const texts = [
        'm',
        'me',
        'mee',
        'meet',
        'meeting',
        'meeting 0012',
        'project 5',
        'area 3',
        'area-12 9',
        'xyz',
        '9999',
        'project-96',
        'MEETING',
        'proj',
        'g-01',
        '',
        'a',
        't 4',
        'ing-0',
        '07',
      ];
      const times: number[] = [];
      for (const text of texts) {
        const started = performance.now();
        await type(text);
        await tick();
        expect(overlay()!.querySelector('.results') ?? overlay()).not.toBeNull();
        times.push(performance.now() - started);
      }
      console.info(
        `NFR-018 go to note, 10,000 notes, opened in ${opened.toFixed(1)} ms, ms per key: ${times.map((t) => t.toFixed(1)).join(' ')}`,
      );
      expect(Math.max(...times)).toBeLessThan(200);

      shell.palette.show('commands');
      const commandTimes: number[] = [];
      for (const text of ['>n', '>ne', '>new', '>git', '>theme', '>export pdf', '>xyz']) {
        const started = performance.now();
        await type(text);
        await tick();
        commandTimes.push(performance.now() - started);
      }
      console.info(
        `NFR-018 command palette, ms per key: ${commandTimes.map((t) => t.toFixed(1)).join(' ')}`,
      );
      expect(Math.max(...commandTimes)).toBeLessThan(200);
    },
    LARGE_TIMEOUT,
  );

  it(
    'shows the notes page by page as the keyboard nears the end',
    async () => {
      const { shell } = await workbench({ givenDir: LARGE });
      shell.palette.show('notes');
      await waitFor(() => expect(overlay()).not.toBeNull());

      expect(options()).toHaveLength(50);
      expect(
        within(overlay()!).getByText('Showing 50 of 10000 notes. Scroll for more.'),
      ).toBeInTheDocument();
      for (let i = 0; i < 46; i += 1) {
        await fireEvent.keyDown(field(), { key: 'ArrowDown' });
      }
      expect(options()).toHaveLength(100);
    },
    LARGE_TIMEOUT,
  );
});

describe('Package 2 review: keys and places', () => {
  it('OQ-243 no two commands claim the same key for the same place, on Linux and macOS', async () => {
    const { shell } = await workbench();
    await open(shell, 'Plan');

    expect(keyClashes(shell.commands, false)).toEqual([]);
    expect(keyClashes(shell.commands, true)).toEqual([]);
    expect(shell.commands.filter((c) => keyText(c.key, false) === 'Ctrl+Shift+H')).toHaveLength(2);
  });

  it('OQ-243 Ctrl+Shift+H in the text of a note is the heading, elsewhere Replace in notes, which it toggles', async () => {
    const { shell } = await workbench();
    const text = await open(shell, 'Plan');

    press('H', { ctrlKey: true, shiftKey: true }, text);
    expect(shell.search.open).toBe(false);

    const entry = document.querySelector<HTMLElement>('[data-focus-fallback] button.entry')!;
    entry.focus();
    const outside = press('H', { ctrlKey: true, shiftKey: true }, entry);
    expect(outside.defaultPrevented).toBe(true);
    await waitFor(() => expect(shell.search.replacing).toBe(true));

    // In the search overlay it closes the replace again (UC-022 A8).
    const searchField = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.search-overlay input');
      expect(found).not.toBeNull();
      return found!;
    });
    searchField.focus();
    press('H', { ctrlKey: true, shiftKey: true }, searchField);
    expect(shell.search.replacing).toBe(false);
    expect(shell.search.open).toBe(true);
  });

  it('OQ-243 the palette shows where a shared key runs its command', async () => {
    const { shell } = await workbench();
    await open(shell, 'Plan');
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>format as heading');
    expect(options()[0]).toHaveTextContent('Ctrl+Shift+H');
    expect(options()[0]).toHaveTextContent('in the note');
    await type('>replace in notes');
    expect(options()[0]).toHaveTextContent('outside the note');
  });

  it('UX review: Ctrl+P, Ctrl+Shift+P and F1 do not open over a dialog that waits for an answer', async () => {
    const { shell } = await workbench();
    const text = await open(shell, 'Plan');
    await shell.startRename('Plan');
    const dialog = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('[role="dialog"]');
      expect(found).not.toBeNull();
      return found!;
    });
    const input = dialog.querySelector('input') ?? dialog;

    for (const [key, modifiers] of [
      ['p', { ctrlKey: true }],
      ['P', { ctrlKey: true, shiftKey: true }],
      ['F1', {}],
    ] as const) {
      const event = press(key, modifiers, input);
      expect(event.defaultPrevented, key).toBe(false);
    }
    expect(overlay()).toBeNull();
    shell.closeNoteDialog('rename');
    await waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
    press('F1', {}, text);
    await waitFor(() => expect(overlay()).not.toBeNull());
  });

  it("OQ-240 Ctrl+P in Nano is the input mode's in the text, go to note elsewhere", async () => {
    const { shell } = await workbench({ ...START, settings: { inputMode: 'Nano' } });
    const text = await open(shell, 'Plan');

    press('p', { ctrlKey: true }, text);
    expect(overlay()).toBeNull();
    const entry = document.querySelector<HTMLElement>('[data-focus-fallback] button.entry')!;
    entry.focus();
    press('p', { ctrlKey: true }, entry);
    await waitFor(() => expect(overlay()).toHaveAccessibleName('Go to note'));
  });
});

describe('Package 2 review: the overlay', () => {
  it('UX review: the top bar and the empty screen offer the palette with its key', async () => {
    const shell = await connectedShell({});
    render(App, { props: { shell } });
    await shell.startWorkspace();

    const button = document.querySelector<HTMLElement>('button[aria-label="Command palette"]')!;
    expect(button.title).toBe('Command palette  Ctrl+Shift+P\nGo to note  Ctrl+P');
    const hint = document.querySelector<HTMLElement>('.palette-hint')!;
    expect(hint).toHaveTextContent('Command palette Ctrl+Shift+P');
    await fireEvent.click(hint);
    await waitFor(() => expect(overlay()).toHaveAccessibleName('Command palette'));

    await fireEvent.click(button);
    expect(overlay()).toBeNull();
    await fireEvent.click(button);
    await waitFor(() => expect(overlay()).not.toBeNull());
  });

  it('UX review: go to note says how to reach the commands and shows what the field is', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());

    expect(field()).toHaveAttribute('placeholder', 'Go to note (type > for commands)');
    expect(overlay()!.querySelector('.mode')).toHaveTextContent('Go to note');
    await type('>');
    expect(overlay()!.querySelector('.mode')).toHaveTextContent('Command palette');
  });

  it('A2 the chosen row says why it is not available, every such row tells it', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());

    await type('>commit');
    const entry = options()[0];
    expect(entry).toHaveTextContent(NO_GIT);
    const why = document.getElementById(entry.getAttribute('aria-describedby')!);
    expect(why).toHaveTextContent(NO_GIT);
  });

  it('A3 Backspace in the empty field of choices goes back to the commands', async () => {
    const { shell } = await workbench();
    shell.palette.show('commands');
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('>theme');
    await fireEvent.keyDown(field(), { key: 'Enter' });
    expect(overlay()).toHaveAccessibleName('Theme');

    await fireEvent.keyDown(field(), { key: 'Backspace' });

    expect(overlay()).toHaveAccessibleName('Command palette');
    expect(field()).toHaveValue('>');
  });

  it('the pointer resting on an entry chooses it', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('plan');

    await fireEvent.pointerMove(options()[2]);

    expect(chosen()).toBe(options()[2]);
  });

  it('app-tester: the list is no stop of Tab, and leaving the field closes the overlay', async () => {
    const { shell } = await workbench();
    shell.palette.show('notes');
    await waitFor(() => expect(overlay()).not.toBeNull());

    expect(overlay()!.querySelector('.results')).toHaveAttribute('tabindex', '-1');
    const outside = document.querySelector<HTMLElement>('[data-focus-fallback] button.entry')!;
    await fireEvent.focusOut(field(), { relatedTarget: outside });
    expect(overlay()).toBeNull();
  });

  it('app-tester: after "Close tab" the keyboard goes to the note overview, never to nowhere', async () => {
    const { shell } = await workbench();
    const text = await open(shell, 'Plan');
    press('F1', {}, text);
    await waitFor(() => expect(overlay()).not.toBeNull());
    await type('>close tab');
    await fireEvent.keyDown(field(), { key: 'Enter' });

    await waitFor(() => expect(shell.tabs.active).toBeNull());
    await waitFor(() =>
      expect(document.activeElement?.closest('[data-focus-fallback]')).not.toBeNull(),
    );
  });
});
