import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { emit } from '@tauri-apps/api/event';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import type { TabsView } from './bindings';
import { activeSourceView } from './editor/active';
import { mockHandlers, MOCK_SAVE_ERROR, type MockStart } from './mock-backend';

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

  it('M-08.4 typed text goes to the backend and its patch shows; the tab is marked', async () => {
    const handlers = mockHandlers(NOTES);
    const input = vi.fn(handlers.ed_input);
    await workbench(NOTES, { ...handlers, ed_input: input });
    const field = await textField();

    type(field, 'x');

    await waitFor(() => expect(activeSourceView()?.view.state.doc.toString()).toMatch(/^x# Home/));
    expect(input).toHaveBeenCalledWith(
      expect.objectContaining({ note: 'Home', input: { kind: 'text', text: 'x' } }),
    );
    await waitFor(() => expect(tab('Home •')).toBeInTheDocument());
  });

  it('M-08.4 the keys of the editor go to the backend and are not left to the webview', async () => {
    const handlers = mockHandlers(NOTES);
    const input = vi.fn(handlers.ed_input);
    await workbench(NOTES, { ...handlers, ed_input: input });
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
    expect(input).toHaveBeenCalledTimes(1);
  });

  it('M-08.6 copy, cut and paste go through the backend', async () => {
    const handlers = mockHandlers(NOTES);
    const clipboard = vi.fn(handlers.ed_clipboard);
    await workbench(NOTES, { ...handlers, ed_clipboard: clipboard });
    const field = await textField();

    const paste = new Event('paste', { bubbles: true, cancelable: true });
    field.dispatchEvent(paste);

    expect(paste.defaultPrevented).toBe(true);
    await waitFor(() =>
      expect(clipboard).toHaveBeenCalledWith(
        expect.objectContaining({ note: 'Home', action: 'paste' }),
      ),
    );
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
    expect(setView).toHaveBeenCalledWith({ note: 'Home', view: 'source' });
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
