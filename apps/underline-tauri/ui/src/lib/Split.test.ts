// Package 7a: the split view (FR-096, FR-097, UC-004 A18 to A20, BR-018,
// UC-005 BR-016, OQ-220, OQ-221), against the mock backend's
// `/home/user/Notes`: opening to the side, the same note on both sides
// (TC-004-34 to TC-004-36, TC-005-41), the active pane and what follows it
// (the tool panel, the find field, go to line, the commands of the note),
// the keys between the panes, and the zoom and the focus mode in a split
// (TC-010-18, TC-010-19). That the note's one text follows every edit of
// either view, with one history and each view's own cursor and folds, is
// the editor core's (Rust: `tab/slots.rs`); here the views of the window
// reach it.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView, shownSourceViews } from './editor/active';
import { coreOf } from './editor/sync';
import { activeFacade } from './facade/active';
import { mockHooks, type MockStart } from './mock-backend';
import { focusedPane } from './paneFocus';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };

async function workbench(start: MockStart = NOTES): Promise<Shell> {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** The tab bar of the pane `pane` of the split view. */
const tabBar = (pane: 'left' | 'right') =>
  screen.getByRole('tablist', { name: `Open notes, ${pane} pane` });

/** The notes of the tabs of a tab bar. */
const notesOf = (bar: HTMLElement) =>
  within(bar)
    .getAllByRole('tab')
    .map((tab) =>
      tab.textContent
        ?.replace(/⇆/, '')
        .replace(/ [•⚠].*$/, '')
        .trim(),
    );

/** The pane element `pane`. */
/** The field of the command palette. */
const paletteField = () =>
  within(document.querySelector<HTMLElement>('.palette')!).getByRole('combobox');

const paneEl = (pane: 'left' | 'right') =>
  document.querySelector<HTMLElement>(`.pane[data-pane="${pane}"]`)!;

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

/** Ctrl (Cmd on macOS, not here) with `key`, as the window takes it. */
function ctrl(key: string, code?: string) {
  fireEvent.keyDown(window, { key, code, ctrlKey: true });
}

describe('TC-004-34 open in the split view (UC-004 A18, BR-018, FR-096)', () => {
  it('opens to the side from the overview; notes open in the active pane; the border moves', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await shell.activateTab('Home');

    await fireEvent.contextMenu(
      within(screen.getByRole('complementary', { name: 'Notes' })).getByRole('button', {
        name: 'Trip',
      }),
      { clientX: 20, clientY: 20 },
    );
    const menu = await screen.findByRole('menu', { name: 'Note Trip' });
    await fireEvent.click(within(menu).getByRole('menuitem', { name: 'Open to the side' }));

    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Trip']));
    expect(notesOf(tabBar('left'))).toEqual(['Home', 'Ideas']);
    expect(shell.activePane).toBe('right');
    expect(focusedPane()).toBe('right');
    // The tool panel shows the note of the active pane.
    await waitFor(() => expect(shell.properties?.note).toBe('Trip'));
    // Both panes start equally wide.
    expect(paneEl('left').style.flexBasis).toBe('50%');
    expect(paneEl('right').style.flexBasis).toBe('50%');

    // Step 2: a note chosen opens in the right pane.
    await shell.openNote('Log');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Trip', 'Log']));

    // Step 3: working in the left pane makes it the active one.
    await fireEvent.pointerDown(paneEl('left'));
    await waitFor(() => expect(shell.activePane).toBe('left'));
    await waitFor(() => expect(shell.properties?.note).toBe('Home'));
    await shell.openNote('Reading-List');
    await waitFor(() => expect(notesOf(tabBar('left'))).toEqual(['Home', 'Reading List', 'Ideas']));
    expect(notesOf(tabBar('right'))).toEqual(['Trip', 'Log']);

    // Step 4: the border between the panes moves (here with the keyboard).
    const border = screen.getByRole('separator', { name: 'Border between the panes' });
    await fireEvent.keyDown(border, { key: 'ArrowRight', shiftKey: true });
    expect(paneEl('left').style.flexBasis).toBe('60%');
    expect(border).toHaveAttribute('aria-valuenow', '60');
  });

  it('UC-004 A18: go to note opens to the side with Ctrl+Enter', async () => {
    const shell = await workbench();
    shell.palette.show('notes');
    const field = await waitFor(paletteField);
    await fireEvent.input(field, { target: { value: 'Trip' } });
    await waitFor(() => expect(shell.palette.list.entries[0]).toMatchObject({ note: 'Trip' }));
    await fireEvent.keyDown(field, { key: 'Enter', ctrlKey: true });

    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Trip']));
    expect(notesOf(tabBar('left'))).toEqual(['Home']);
  });
});

describe('TC-004-35, TC-005-41 the same note on both sides (UC-004 A19, UC-005 BR-016)', () => {
  it('Ctrl+\\ shows the facade left and the source view right; each follows the other', async () => {
    const shell = await workbench();
    await waitFor(() => expect(activeFacade()?.note).toBe('Home'));

    ctrl('\\', 'Backslash');

    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Home']));
    expect(paneEl('left').querySelector('[role="tabpanel"]')).toHaveAttribute(
      'data-view',
      'facade',
    );
    expect(paneEl('right').querySelector('[role="tabpanel"]')).toHaveAttribute(
      'data-view',
      'source',
    );
    await waitFor(() => expect(activeSourceView()?.note).toBe('Home'));

    // TC-004-35 step 2, TC-005-41 step 1: typed in the source view, shown in the facade at once.
    const source = paneEl('right').querySelector<HTMLElement>('.cm-content')!;
    source.focus();
    type(source, 'hello ');
    await waitFor(() =>
      expect(paneEl('left').querySelector('.facade-text')?.textContent).toContain('hello'),
    );
    // One text, one revision line: the view keeps its own cursor in it.
    expect(coreOf('Home')?.viewCount).toBe(2);
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^hello /));

    // TC-005-41 step 1: typed in the facade, shown in the source view.
    await fireEvent.pointerDown(paneEl('left'));
    await waitFor(() => expect(shell.activePane).toBe('left'));
    const facade = paneEl('left').querySelector<HTMLElement>('.facade-text')!;
    facade.focus();
    type(facade, 'X');
    await waitFor(() => expect(source.textContent).toContain('X'));

    // TC-005-41 step 2, OQ-221: Ctrl+Z in the source view takes back the
    // facade's "X", the last change of the note, on both sides.
    await fireEvent.pointerDown(paneEl('right'));
    source.focus();
    await fireEvent.keyDown(source, { key: 'z', ctrlKey: true });
    await waitFor(() => expect(source.textContent).not.toContain('X'));
    await waitFor(() =>
      expect(paneEl('left').querySelector('.facade-text')?.textContent).not.toContain('X'),
    );

    // TC-004-35 step 3: chosen again with the right pane active, no third tab opens.
    await shell.openNote('Home');
    expect(notesOf(tabBar('right'))).toEqual(['Home']);
    expect(notesOf(tabBar('left'))).toEqual(['Home']);
  });

  it('BR-016: closing one of the two tabs keeps the note open in the other', async () => {
    const shell = await workbench();
    await shell.openToSide('Home');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Home']));
    expect(await shell.closeTab('Home', false, 'right')).toBe(true);
    await waitFor(() =>
      expect(screen.queryByRole('separator', { name: 'Border between the panes' })).toBeNull(),
    );
    expect(coreOf('Home')).toBeDefined();
    expect(screen.getByRole('tablist', { name: 'Open notes' })).toBeInTheDocument();
  });
});

describe('TC-004-36 end the split view (UC-004 A20)', () => {
  it('the right tabs join the left ones; closing the last right tab ends it too', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await shell.activateTab('Home');
    await shell.openToSide('Home');
    await shell.openNote('Trip');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Home', 'Trip']));

    // From the command palette.
    shell.palette.show('commands');
    const field = await waitFor(paletteField);
    await fireEvent.input(field, { target: { value: '>End the split view' } });
    await waitFor(() => expect(shell.palette.list.entries[0]).toMatchObject({ kind: 'command' }));
    await fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() =>
      expect(notesOf(screen.getByRole('tablist', { name: 'Open notes' }))).toEqual([
        'Home',
        'Ideas',
        'Trip',
      ]),
    );

    await shell.openToSide('Trip');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Trip']));
    await shell.closeTab('Trip', false, 'right');
    await waitFor(() =>
      expect(notesOf(screen.getByRole('tablist', { name: 'Open notes' }))).toEqual([
        'Home',
        'Ideas',
        'Trip',
      ]),
    );
  });
});

describe('UC-004 BR-018: the active pane and what follows it', () => {
  it('Ctrl+1 and Ctrl+2 move the keyboard between the panes; the palette has them too', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    await waitFor(() => expect(activeFacade()?.note).toBe('Trip'));

    ctrl('1', 'Digit1');
    await waitFor(() => expect(shell.activePane).toBe('left'));
    await waitFor(() => expect(activeFacade()?.note).toBe('Home'));
    ctrl('2', 'Digit2');
    await waitFor(() => expect(shell.activePane).toBe('right'));
    expect(activeFacade()?.note).toBe('Trip');

    const titles = shell.commands.map((command) => command.title);
    expect(titles).toEqual(
      expect.arrayContaining([
        'Open to the side',
        'End the split view',
        'Focus the left pane',
        'Focus the right pane',
        'Open to the side: link at the cursor',
      ]),
    );
  });

  it('UC-023 BR-002 (review): "side" finds "Open to the side" first', async () => {
    const shell = await workbench();
    shell.palette.show('commands');
    const field = await waitFor(paletteField);
    await fireEvent.input(field, { target: { value: '>side' } });
    await waitFor(() =>
      expect(shell.palette.list.entries[0]).toMatchObject({
        kind: 'command',
        command: expect.objectContaining({ title: 'Open to the side' }),
      }),
    );
  });

  it('without the split view the pane keys say so and keep their way', async () => {
    const shell = await workbench();
    const focusRight = shell.commandById('view.focusRightPane')!;
    expect(focusRight.unavailable()).toBe('The split view is not shown.');
    expect(shell.commandById('view.endSplit')!.unavailable()).toBe('The split view is not shown.');
  });

  it('the find field and go to line act in the active pane; the other pane keeps its cursor', async () => {
    const shell = await workbench();
    await shell.openToSide('Home');
    await waitFor(() => expect(activeSourceView()?.note).toBe('Home'));
    expect(shownSourceViews()).toHaveLength(1);

    // The find field stands above the note of the active pane only.
    shell.noteActions?.find(false);
    await waitFor(() => expect(within(paneEl('right')).getByRole('search')).toBeInTheDocument());
    expect(within(paneEl('left')).queryByRole('search')).toBeNull();
    shell.search.closeFind();

    // Go to line: the cursor of the active pane's source view moves.
    shell.noteActions?.goToLine({ line: 2, column: null });
    await waitFor(() => {
      const view = activeSourceView()!;
      const head = view.view.state.selection.main.head;
      expect(view.view.state.doc.lineAt(head).number).toBe(2);
    });
    // The facade of the left pane did not move: its cursor is its own.
    expect(paneEl('left').querySelector('.editor-status')?.textContent).toContain('Ln 1');
  });

  it('the status line of each pane says its own place and keys', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    await waitFor(() => expect(paneEl('right').querySelector('.editor-status')).not.toBeNull());
    expect(paneEl('left').querySelector('.editor-status')).not.toBeNull();
  });
});

describe('TC-010-18, TC-010-19 the zoom and the focus mode in a split', () => {
  it('the editor zoom changes both panes; the focus mode keeps both panes and their tabs', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    await waitFor(() => expect(paneEl('right').querySelector('.facade')).not.toBeNull());

    await shell.zoomEditor(1);
    await shell.zoomEditor(1);
    await waitFor(() => {
      for (const pane of ['left', 'right'] as const) {
        const facade = paneEl(pane).querySelector<HTMLElement>('.facade');
        expect(facade?.style.getPropertyValue('--editor-zoom')).toBe('1.2');
      }
    });

    await shell.setFocusMode(true);
    await waitFor(() => expect(shell.focusMode).toBe(true));
    expect(tabBar('left')).toBeInTheDocument();
    expect(tabBar('right')).toBeInTheDocument();
    expect(screen.getByRole('separator', { name: 'Border between the panes' })).toBeVisible();
    expect(document.querySelector('.views')).toBeNull();
  });
});

describe('Package 7a review: the split view for the keyboard and screen readers', () => {
  it('each pane is a group, the active one current; the border names its share', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    const right = await screen.findByRole('group', { name: 'Right pane' });
    const left = screen.getByRole('group', { name: 'Left pane' });
    expect(right).toHaveAttribute('aria-current', 'true');
    expect(left).not.toHaveAttribute('aria-current');
    const border = screen.getByRole('separator', { name: 'Border between the panes' });
    expect(border).toHaveAttribute('aria-valuetext', 'Left pane 50 percent');
    await fireEvent.keyDown(border, { key: 'ArrowLeft', shiftKey: true });
    expect(border).toHaveAttribute('aria-valuetext', 'Left pane 40 percent');
    await fireEvent.dblClick(border);
    expect(border).toHaveAttribute('aria-valuetext', 'Left pane 50 percent');

    // The change of the active pane is told politely.
    await shell.focusPane('left');
    await waitFor(() => expect(shell.paneAnnouncement).toBe('Left pane, Home'));
  });

  it('the button and Ctrl+Shift+M end the split view; the keyboard goes to the note left', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Trip']));
    await fireEvent.click(screen.getByRole('button', { name: 'End the split view' }));
    await waitFor(() => expect(shell.status).toBe('Split view ended.'));
    expect(shell.split).toBe(false);
    await waitFor(() => expect(activeFacade()?.note).toBe('Trip'));
    await waitFor(() =>
      expect(document.activeElement?.closest('.facade-text, [role="tab"]')).not.toBeNull(),
    );

    await shell.openToSide('Home');
    await waitFor(() => expect(shell.split).toBe(true));
    fireEvent.keyDown(window, { key: 'M', code: 'KeyM', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(shell.split).toBe(false));
  });

  it('Ctrl+F6 goes to the other pane; Ctrl+Shift+S opens to the side', async () => {
    const shell = await workbench();
    await waitFor(() => expect(activeFacade()?.note).toBe('Home'));
    fireEvent.keyDown(window, { key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(shell.split).toBe(true));
    expect(shell.activePane).toBe('right');
    fireEvent.keyDown(window, { key: 'F6', code: 'F6', ctrlKey: true });
    await waitFor(() => expect(shell.activePane).toBe('left'));
    fireEvent.keyDown(window, { key: 'F6', code: 'F6', ctrlKey: true });
    await waitFor(() => expect(shell.activePane).toBe('right'));
  });

  it('a tab whose note is open on both sides says so', async () => {
    const shell = await workbench();
    await shell.openToSide('Home');
    await waitFor(() => expect(notesOf(tabBar('right'))).toEqual(['Home']));
    for (const pane of ['left', 'right'] as const) {
      const tab = within(tabBar(pane)).getByRole('tab', { name: 'Home' });
      expect(tab).toHaveAccessibleDescription('also open in the other pane');
      expect(tab.querySelector('[title="Also open in the other pane"]')).not.toBeNull();
    }
  });

  it('OQ-279: with the right pane active, the tool panel shows its note while the left shows the todos', async () => {
    const shell = await workbench();
    await shell.openToSide('Trip');
    await shell.showTodoList();
    await waitFor(() => expect(shell.todoListShown).toBe(true));
    await shell.focusPane('right');
    await waitFor(() => expect(shell.todoListShown).toBe(false));
    await waitFor(() => expect(shell.properties?.note).toBe('Trip'));
    expect(document.querySelector('.todo-list')).not.toBeNull();
  });
});
