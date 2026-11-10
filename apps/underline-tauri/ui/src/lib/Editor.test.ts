import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { emit } from '@tauri-apps/api/event';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import type { TabsView } from './bindings';
import { activeSourceView } from './editor/active';
import { EditorCore } from './editor/core';
import { mockHandlers, mockHooks, MOCK_SAVE_ERROR, type MockStart } from './mock-backend';

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function workbench(start: MockStart = {}, handlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

// These tests try the source view; a new tab shows the facade (M-07.2).
const NOTES = { givenDir: '/home/user/Notes', view: 'source' as const };
const tabBar = () => screen.getByRole('tablist', { name: 'Open notes' });
const tab = (name: string | RegExp) => within(tabBar()).getByRole('tab', { name });
const tools = () => screen.getByRole('complementary', { name: 'Tools' });

/** The text field of the source view, once its text is in. */
async function textField(): Promise<HTMLElement> {
  await waitFor(() => expect(activeSourceView()).not.toBeNull());
  return document.querySelector<HTMLElement>('.cm-content')!;
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

describe('M-08 source view (UC-005)', () => {
  it('ADR-024 shows the text the backend owns', async () => {
    await workbench(NOTES);

    const field = await textField();

    expect(field).toHaveAttribute('aria-label', 'Home');
    expect(field.textContent).toContain('# Home');
    expect(field).toHaveAttribute('spellcheck', 'false');
  });

  it('M-08.4 typed text shows and goes to the backend after the frame; the tab is marked', async () => {
    const handlers = mockHandlers(NOTES);
    const sync = vi.fn(handlers.ed_sync);
    await workbench(NOTES, { ...handlers, ed_sync: sync });
    const field = await textField();

    type(field, 'x');

    // ADR-025: the editor core answers without the backend.
    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^x# Home/));
    await waitFor(() =>
      expect(sync).toHaveBeenCalledWith({
        note: 'Home',
        batch: expect.objectContaining({ base: 1, edits: [{ at: 0, remove: 0, insert: 'x' }] }),
      }),
    );
    expect(mockHooks().mirrorText('Home')).toMatch(/^x# Home/);
    await waitFor(() => expect(tab('Home •')).toBeInTheDocument());
  });

  it('ADR-025 a change of the backend (an agent) comes into the text; the cursor stays', async () => {
    await workbench(NOTES);
    const field = await textField();
    type(field, 'x');
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^x# Home/));

    await mockHooks().backendEdit('Home', 0, 0, '> ');

    await waitFor(() =>
      expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^> x# Home/),
    );
    expect(activeSourceView()?.view.state.selection.main.head).toBe(3);
    type(field, 'y');
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^> xy# Home/));
  });

  it('UC-004 A7 a change from outside replaces the text', async () => {
    await workbench(NOTES);
    await textField();

    await mockHooks().outsideChange('Home', 'From outside\n');

    await waitFor(() =>
      expect(activeSourceView()?.view.state.doc.toString()).toBe('From outside\n'),
    );
  });

  it('M-08.4 the keys of the editor go to the editor core and are not left to the webview', async () => {
    await workbench(NOTES);
    const field = await textField();

    const tabKey = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    field.dispatchEvent(tabKey);
    const shortcut = new KeyboardEvent('keydown', {
      key: 'n',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    field.dispatchEvent(shortcut);

    expect(tabKey.defaultPrevented).toBe(true);
    expect(shortcut.defaultPrevented).toBe(false);
    await waitFor(() =>
      expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^ {4}# Home/),
    );
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^ {4}# Home/));
  });

  it('M-08.6 pasting reads the system clipboard through the backend', async () => {
    const handlers = mockHandlers(NOTES);
    const read = vi.fn(() => 'pasted ');
    await workbench(NOTES, { ...handlers, clip_read_text: read });
    const field = await textField();

    const paste = new Event('paste', { bubbles: true, cancelable: true });
    field.dispatchEvent(paste);

    expect(paste.defaultPrevented).toBe(true);
    await waitFor(() =>
      expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^pasted # Home/),
    );
    expect(read).toHaveBeenCalled();
  });

  it('M-08.2 takes the colours the backend sends for the lines shown', async () => {
    await workbench(NOTES);
    await textField();

    await waitFor(() =>
      expect(document.querySelector('.cm-line span[style*="font-weight: bold"]')?.textContent).toBe(
        '# Home',
      ),
    );
  });
});

describe('M-07.2 the switch of the editing view', () => {
  it('a new tab shows the facade; the switch changes the view of the tab', async () => {
    const start = { givenDir: '/home/user/Notes' };
    const handlers = mockHandlers(start);
    const setView = vi.fn(handlers.tab_set_view);
    await workbench(start, { ...handlers, tab_set_view: setView });
    const views = screen.getByRole('group', { name: 'Editing view' });
    const facade = within(views).getByRole('button', { name: 'Facade' });
    const source = within(views).getByRole('button', { name: 'Source' });
    const panel = () => screen.getByRole('tabpanel', { name: 'Home' });

    expect(facade).toHaveAttribute('aria-pressed', 'true');
    expect(panel()).toHaveAttribute('data-view', 'facade');
    await waitFor(() => expect(document.querySelector('.facade-text')).not.toBeNull());

    await fireEvent.click(source);

    await waitFor(() => expect(panel()).toHaveAttribute('data-view', 'source'));
    await waitFor(() => expect(setView).toHaveBeenCalledWith({ note: 'Home', view: 'source' }));
    expect(source).toHaveAttribute('aria-pressed', 'true');
    await textField();
    await waitFor(() => expect(document.activeElement).toBe(document.querySelector('.cm-content')));

    await fireEvent.click(facade);

    await waitFor(() => expect(panel()).toHaveAttribute('data-view', 'facade'));
    await waitFor(() =>
      expect(document.activeElement).toBe(document.querySelector('.facade-text')),
    );
  });
});

describe('ADR-025 the text in the window across a switch of the view', () => {
  it('keys typed right before a switch reach the text, in the same task', async () => {
    await workbench(NOTES);
    const field = await textField();
    const facade = within(screen.getByRole('group', { name: 'Editing view' })).getByRole('button', {
      name: 'Facade',
    });

    type(field, 'a');
    type(field, 'b');
    type(field, 'c');
    facade.click();

    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^abc# Home/));
    await waitFor(() =>
      expect(document.querySelector('.facade-text [data-b="0"]')).toHaveTextContent('abc# Home'),
    );
  });

  it('keys typed in the facade right before a switch reach the text', async () => {
    await workbench({ givenDir: '/home/user/Notes' });
    const text = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.facade-text');
      expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
      return found!;
    });
    const source = within(screen.getByRole('group', { name: 'Editing view' })).getByRole('button', {
      name: 'Source',
    });

    for (const data of ['x', 'y', 'z']) {
      text.dispatchEvent(
        new InputEvent('beforeinput', {
          inputType: 'insertText',
          data,
          bubbles: true,
          cancelable: true,
        }),
      );
    }
    source.click();

    await waitFor(() => expect(mockHooks().mirrorText('Home')).toContain('xyz'));
    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toContain('xyz'));
  });

  it('the facade shown again takes the blocks it drew when nothing changed, else all anew', async () => {
    await workbench({ givenDir: '/home/user/Notes' });
    const block = () => document.querySelector('.facade-text [data-b="0"]');
    await waitFor(() => expect(block()).toHaveTextContent('Home'));
    const views = within(screen.getByRole('group', { name: 'Editing view' }));
    const whole = vi.spyOn(EditorCore.prototype, 'facadeState');
    const head = vi.spyOn(EditorCore.prototype, 'facadeHead');

    views.getByRole('button', { name: 'Source' }).click();
    await textField();
    views.getByRole('button', { name: 'Facade' }).click();
    await waitFor(() => expect(block()).toHaveTextContent('Home'));
    expect(head).toHaveBeenCalled();
    expect(whole).not.toHaveBeenCalled();

    views.getByRole('button', { name: 'Source' }).click();
    await waitFor(() => expect(activeSourceView()).not.toBeNull());
    type(await textField(), 'q');
    views.getByRole('button', { name: 'Facade' }).click();
    await waitFor(() => expect(block()).toHaveTextContent('q# Home'));
    expect(whole).toHaveBeenCalled();
  });

  it('M-21.1 the first tab of a workspace opened from the start screen can be edited in the facade', async () => {
    const shell = await workbench({});
    await shell.openWorkspace('/home/user/Notes');
    const text = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.facade-text');
      expect(found?.querySelector('[data-b="0"]')).toHaveTextContent('Home');
      return found!;
    });

    text.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertText',
        data: 'q',
        bubbles: true,
        cancelable: true,
      }),
    );

    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/q/));
    expect(shell.status).not.toMatch(/not open/);
  });

  it('M-11.2 the Vim keys work in the facade of a tab that never showed the source view', async () => {
    await workbench({ givenDir: '/home/user/Notes', settings: { inputMode: 'Vim' } });
    const text = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.facade-text');
      expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
      return found!;
    });

    const status = screen.getByRole('group', { name: 'Editor status' });
    await waitFor(() => expect(status.querySelector('.badge')).toHaveTextContent('NORMAL'));
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toMatch(/^# Home/));

    // Vim's `x` deletes the character at the cursor, it types nothing.
    text.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertText',
        data: 'x',
        bubbles: true,
        cancelable: true,
      }),
    );

    await waitFor(() => expect(mockHooks().mirrorText('Home')).not.toMatch(/^# Home/));
    expect(mockHooks().mirrorText('Home')).not.toMatch(/x/);
  });

  it('a change of the backend comes into the facade', async () => {
    await workbench({ givenDir: '/home/user/Notes' });
    await waitFor(() =>
      expect(document.querySelector('.facade-text [data-b="0"]')).toHaveTextContent('Home'),
    );
    await waitFor(() => expect(mockHooks().mirrorText('Home')).not.toBeNull());

    await mockHooks().backendEdit('Home', 2, 0, 'Agent ');

    await waitFor(() =>
      expect(document.querySelector('.facade-text [data-b="0"]')).toHaveTextContent('Agent Home'),
    );
  });
});

describe('Q-40 a failed editor core', () => {
  it('says above the note that the latest edits were lost, until the user acts there', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });
    await waitFor(() =>
      expect(document.querySelector('.facade-text [data-b="0"]')).toHaveTextContent('Home'),
    );

    shell.editorLost = ['Home'];

    const alert = await screen.findByText(
      'The editor failed. It shows the last stored text again; your latest edits were lost.',
    );
    expect(alert).toHaveAttribute('role', 'alert');
    fireEvent.keyDown(document.querySelector('.facade-text')!, { key: 'ArrowRight' });
    await waitFor(() => expect(alert).not.toBeInTheDocument());
  });
});

describe('M-10.8 autosave and its failures', () => {
  it('says above the note why it was not stored', async () => {
    const shell = await workbench(NOTES);
    const tabs: TabsView = {
      ...shell.tabs,
      tabs: shell.tabs.tabs.map((t) => ({ ...t, label: 'Home ⚠', saveError: 'disk full' })),
    };

    await emit('tabs', tabs);

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Not stored: disk full. Underline keeps trying.',
    );
    expect(tab('Home ⚠')).toBeInTheDocument();
  });
});

describe('M-06.3, M-21.4 "Changes not stored"', () => {
  it('asks before closing a tab that cannot be stored; "Keep open" keeps it', async () => {
    const shell = await workbench({ ...NOTES, failingSaves: ['Home'] });
    type(await textField(), 'x');
    await waitFor(() => expect(tab('Home •')).toBeInTheDocument());

    await fireEvent.click(
      within(tabBar().parentElement!).getByRole('button', { name: /^Close Home/ }),
    );

    const dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });
    expect(dialog).toHaveTextContent(`Changes could not be stored: Home: ${MOCK_SAVE_ERROR}`);
    expect(dialog).toHaveTextContent('Close the tab Home anyway and lose these changes?');
    expect(within(dialog).getByRole('button', { name: 'Keep open' })).toHaveFocus();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Keep open' }));

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home']);
  });

  it('"Close anyway" closes the tab without its changes', async () => {
    const shell = await workbench({ ...NOTES, failingSaves: ['Home'] });
    type(await textField(), 'x');
    await waitFor(() => expect(tab('Home •')).toBeInTheDocument());
    await fireEvent.click(
      within(tabBar().parentElement!).getByRole('button', { name: /^Close Home/ }),
    );
    const dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close anyway' }));

    await waitFor(() => expect(shell.tabs.tabs).toEqual([]));
  });

  it('closing Underline asks the same; "Close anyway" ends it, Escape keeps it', async () => {
    const handlers = mockHandlers(NOTES);
    const closeAnyway = vi.fn(handlers.app_close_anyway);
    await workbench(NOTES, { ...handlers, app_close_anyway: closeAnyway });

    await emit('close-question', { tab: null, what: 'Underline', error: 'Home: disk full' });
    let dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });
    expect(dialog).toHaveTextContent('Close Underline anyway and lose these changes?');
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(closeAnyway).not.toHaveBeenCalled();

    await emit('close-question', { tab: null, what: 'Underline', error: 'Home: disk full' });
    dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close anyway' }));

    await waitFor(() => expect(closeAnyway).toHaveBeenCalled());
  });
});

describe('M-16 tool panel', () => {
  it('M-16.2 shows the properties of the active note', async () => {
    await workbench(NOTES);

    await waitFor(() => expect(within(tools()).getByText('Home.md')).toBeInTheDocument());
    for (const name of ['Place', 'Format', 'Size', 'Last change']) {
      expect(within(tools()).getByText(name)).toBeInTheDocument();
    }
    expect(within(tools()).getByText('Markdown')).toBeInTheDocument();
  });

  it('M-16.2 says "Not saved yet" when the file cannot be read', async () => {
    await workbench(NOTES);

    await emit('properties', { note: 'Home', properties: null });

    expect(await within(tools()).findByText('Not saved yet')).toBeInTheDocument();
  });

  it('M-16.1 a width dragged more than a pixel is stored when let go', async () => {
    const handlers = mockHandlers(NOTES);
    const settingsQuick = vi.fn(handlers.settings_quick);
    await workbench(NOTES, { ...handlers, settings_quick: settingsQuick });
    const edge = tools().querySelector<HTMLElement>('.edge')!;
    edge.setPointerCapture = () => {};
    vi.spyOn(tools(), 'getBoundingClientRect').mockImplementation(
      () => ({ width: parseFloat(tools().style.width) }) as DOMRect,
    );

    await fireEvent.pointerDown(edge, { button: 0, clientX: 800, pointerId: 1 });
    await fireEvent.pointerMove(edge, { clientX: 700, pointerId: 1 });
    expect(tools()).toHaveStyle({ width: '340px' });
    await fireEvent.pointerUp(edge, { pointerId: 1 });

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'tool_panel_width', value: '340' });
  });
});

describe('plan section 14: the tab bar is a tablist', () => {
  it('only the active tab is in the Tab sequence; arrows move, Delete closes', async () => {
    const shell = await workbench(NOTES);
    await shell.openNote('Ideas');
    await waitFor(() => expect(tab('Ideas')).toHaveAttribute('aria-selected', 'true'));

    expect(tab('Ideas')).toHaveAttribute('tabindex', '0');
    expect(tab('Home')).toHaveAttribute('tabindex', '-1');
    expect(tab('Ideas')).toHaveAttribute('aria-controls', 'note-panel');

    tab('Ideas').focus();
    await fireEvent.keyDown(tab('Ideas'), { key: 'ArrowLeft' });
    expect(tab('Home')).toHaveFocus();
    await fireEvent.keyDown(tab('Home'), { key: 'End' });
    expect(tab('Ideas')).toHaveFocus();

    await fireEvent.keyDown(tab('Ideas'), { key: 'Delete' });

    await waitFor(() => expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home']));
    await waitFor(() => expect(tab('Home')).toHaveFocus());
  });
});

describe('review F3: keyboard and screen readers', () => {
  it('M-08.10 Escape leaves the text for the note tab', async () => {
    await workbench(NOTES);
    const field = await textField();
    expect(field).toHaveAttribute('aria-description', 'Escape leaves the text');
    field.focus();

    field.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );

    expect(tab('Home')).toHaveFocus();
  });

  it('M-06.1 the marks of a tab are described in words', async () => {
    const shell = await workbench(NOTES);
    await emit('tabs', {
      ...shell.tabs,
      tabs: shell.tabs.tabs.map((t) => ({ ...t, label: 'Home •' })),
    });

    await waitFor(() => expect(tab('Home •')).toHaveAccessibleDescription('unsaved changes'));
    expect(tab('Home •')).toHaveAttribute('aria-keyshortcuts', 'Delete');
  });

  it('M-06.3 after "Close anyway" the keyboard goes on to the neighbouring tab', async () => {
    const shell = await workbench({ ...NOTES, failingSaves: ['Ideas'] });
    await shell.openNote('Ideas');
    await waitFor(() => expect(tab('Ideas')).toHaveAttribute('aria-selected', 'true'));
    type(await textField(), 'x');
    await waitFor(() => expect(tab('Ideas •')).toBeInTheDocument());
    await fireEvent.keyDown(tab('Ideas •'), { key: 'Delete' });
    const dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close anyway' }));

    await waitFor(() => expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home']));
    await waitFor(() => expect(tab('Home')).toHaveFocus());
  });

  it('a dialog keeps Tab and Shift+Tab inside', async () => {
    await workbench(NOTES);
    await emit('close-question', { tab: null, what: 'Underline', error: 'Home: disk full' });
    const dialog = await screen.findByRole('dialog', { name: 'Changes not stored' });
    const keep = within(dialog).getByRole('button', { name: 'Keep open' });
    const anyway = within(dialog).getByRole('button', { name: 'Close anyway' });
    await waitFor(() => expect(keep).toHaveFocus());

    await fireEvent.keyDown(keep, { key: 'Tab', shiftKey: true });
    expect(anyway).toHaveFocus();
    await fireEvent.keyDown(anyway, { key: 'Tab' });
    expect(keep).toHaveFocus();
  });
});
