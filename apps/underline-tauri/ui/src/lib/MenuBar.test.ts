import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { connectedShell } from '../test/shell';
import MenuBar from './MenuBar.svelte';
import { mockHandlers, type MockStart } from './mock-backend';

afterEach(() => {
  clearMocks();
  vi.restoreAllMocks();
  document.documentElement.removeAttribute('style');
});

async function menuBar(start: MockStart = {}, handlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  const onOpenWorkspace = vi.fn();
  const onNewWorkspace = vi.fn();
  render(MenuBar, { props: { shell, onOpenWorkspace, onNewWorkspace } });
  return { shell, onOpenWorkspace, onNewWorkspace };
}

function title(name: string) {
  return screen.getByRole('menuitem', { name });
}

describe('M-03.4 to M-03.7 menu bar', () => {
  it('has the four menus, reached by Tab once', async () => {
    await menuBar();

    const titles = ['File', 'Workspace', 'Theme', 'Settings'].map(title);

    expect(titles.map((t) => t.tabIndex)).toEqual([0, -1, -1, -1]);
    expect(titles.map((t) => t.getAttribute('aria-expanded'))).toEqual([
      'false',
      'false',
      'false',
      'false',
    ]);
  });

  it('M-03.4 File: New note is disabled without a workspace, the others call F2', async () => {
    const { onOpenWorkspace, onNewWorkspace } = await menuBar();

    await fireEvent.click(title('File'));

    const menu = screen.getByRole('menu');
    expect(title('File')).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menuitem', { name: 'New note…' })).toHaveAttribute(
      'aria-disabled',
      'true',
    );
    expect(menu).toContainElement(screen.getByRole('separator'));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Open workspace…' }));
    expect(onOpenWorkspace).toHaveBeenCalledOnce();
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    await fireEvent.click(title('File'));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'New workspace…' }));
    expect(onNewWorkspace).toHaveBeenCalledOnce();
  });

  it('M-03.4 a disabled New note keeps the menu open', async () => {
    await menuBar();
    await fireEvent.click(title('File'));

    await fireEvent.click(screen.getByRole('menuitem', { name: 'New note…' }));

    expect(screen.getByRole('menu')).toBeInTheDocument();
  });

  it('M-03.5 Workspace: Check reports the wiki repository and closes the menu', async () => {
    const { shell } = await menuBar();
    await fireEvent.click(title('Workspace'));

    const field = screen.getByLabelText('Wiki project URL');
    await fireEvent.input(field, { target: { value: 'https://github.com/owner/repo' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Check' }));

    await waitFor(() =>
      expect(shell.status).toBe('Wiki repository: https://github.com/owner/repo.wiki.git'),
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(shell.wikiUrl).toBe('https://github.com/owner/repo');
  });

  it('M-03.5 Workspace: Enter in the field checks too', async () => {
    const handlers = mockHandlers();
    const wikiCheck = vi.fn(handlers.wiki_check);
    await menuBar({}, { ...handlers, wiki_check: wikiCheck });
    await fireEvent.keyDown(title('Workspace'), { key: 'ArrowDown' });

    const field = screen.getByLabelText('Wiki project URL');
    await waitFor(() => expect(field).toHaveFocus());
    await fireEvent.input(field, { target: { value: 'nothing' } });
    await fireEvent.submit(field.closest('form')!);

    expect(wikiCheck).toHaveBeenCalledWith({ url: 'nothing' });
    await waitFor(() => expect(title('Workspace')).toHaveFocus());
  });

  it('M-03.6 Theme: a radio list of all themes with the current one checked', async () => {
    await menuBar({ settings: { theme: 'Nord' } });

    await fireEvent.click(title('Theme'));

    const radios = screen.getAllByRole('menuitemradio');
    expect(radios).toHaveLength(11);
    expect(screen.getByRole('menuitemradio', { name: 'Nord' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(screen.getByRole('menuitemradio', { name: 'Light' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
  });

  it('M-03.6 Theme: a choice is stored, applied at once and closes the menu', async () => {
    const handlers = mockHandlers();
    const settingsQuick = vi.fn(handlers.settings_quick);
    const { shell } = await menuBar({}, { ...handlers, settings_quick: settingsQuick });
    await fireEvent.click(title('Theme'));

    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Dracula' }));

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'theme', value: 'Dracula' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    await waitFor(() => expect(shell.settings?.theme).toBe('Dracula'));
    const html = document.documentElement;
    expect(html.style.getPropertyValue('--ui-surface')).toBe('#21222c');
    expect(html.dataset.scheme).toBe('dark');
  });

  it.each([
    ['linux', 'Ctrl+,'],
    ['macos', 'Cmd+,'],
  ] as const)('M-03.7 Settings: "Settings…" shows %s %s', async (platform, shortcut) => {
    const { shell } = await menuBar({ platform });
    await fireEvent.click(title('Settings'));

    const entry = screen.getByRole('menuitem', { name: 'Settings…' });

    expect(entry).toHaveTextContent(shortcut);
    await fireEvent.click(entry);
    expect(shell.settingsOpen).toBe(true);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });
});

describe('NFR-011 menu bar by keyboard', () => {
  it('Down opens a menu at its first entry, Up and Down move, Escape returns', async () => {
    await menuBar();

    await fireEvent.keyDown(title('File'), { key: 'ArrowDown' });
    await waitFor(() => expect(screen.getByRole('menuitem', { name: 'New note…' })).toHaveFocus());

    const menu = screen.getByRole('menu');
    await fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitem', { name: 'Open workspace…' })).toHaveFocus();
    await fireEvent.keyDown(menu, { key: 'End' });
    expect(screen.getByRole('menuitem', { name: 'New workspace…' })).toHaveFocus();
    await fireEvent.keyDown(menu, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitem', { name: 'New note…' })).toHaveFocus();
    await fireEvent.keyDown(menu, { key: 'ArrowUp' });
    expect(screen.getByRole('menuitem', { name: 'New workspace…' })).toHaveFocus();

    await fireEvent.keyDown(menu, { key: 'Escape' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(title('File')).toHaveFocus();
  });

  it('Up opens a menu at its last entry', async () => {
    await menuBar();

    await fireEvent.keyDown(title('Theme'), { key: 'ArrowUp' });

    await waitFor(() =>
      expect(screen.getByRole('menuitemradio', { name: 'Gruvbox' })).toHaveFocus(),
    );
  });

  it('Right and Left move between the titles and keep a menu open', async () => {
    await menuBar();
    title('File').focus();

    await fireEvent.keyDown(title('File'), { key: 'ArrowLeft' });
    expect(title('Settings')).toHaveFocus();
    expect(title('Settings').tabIndex).toBe(0);

    await fireEvent.keyDown(title('Settings'), { key: 'ArrowDown' });
    await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Settings…' })).toHaveFocus());
    await fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowRight' });

    await waitFor(() => expect(screen.getByRole('menuitem', { name: 'New note…' })).toHaveFocus());
    expect(title('File')).toHaveAttribute('aria-expanded', 'true');
  });

  it('Enter on an entry chooses it and returns to the title', async () => {
    const handlers = mockHandlers();
    const settingsQuick = vi.fn(handlers.settings_quick);
    await menuBar({}, { ...handlers, settings_quick: settingsQuick });

    await fireEvent.keyDown(title('Theme'), { key: 'Enter' });
    await waitFor(() => expect(screen.getByRole('menuitemradio', { name: 'Light' })).toHaveFocus());
    // A button's Enter is its click.
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Light' }));

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'theme', value: 'Light' });
    expect(title('Theme')).toHaveFocus();
  });

  it('Escape in the Workspace menu closes it', async () => {
    await menuBar();
    await fireEvent.click(title('Workspace'));

    await fireEvent.keyDown(screen.getByLabelText('Wiki project URL'), { key: 'Escape' });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(title('Workspace')).toHaveFocus();
  });

  it('pointing at another title while a menu is open opens that one', async () => {
    await menuBar();
    await fireEvent.click(title('File'));

    await fireEvent.pointerEnter(title('Theme'));

    expect(title('Theme')).toHaveAttribute('aria-expanded', 'true');
    expect(title('File')).toHaveAttribute('aria-expanded', 'false');
  });

  it('a click that follows pointing at a title keeps its menu open', async () => {
    await menuBar();
    await fireEvent.click(title('File'));

    await fireEvent.pointerEnter(title('Theme'));
    await fireEvent.click(title('Theme'));

    expect(title('Theme')).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('menu')).toHaveAttribute('id', 'menu-theme-popup');
  });

  it('a second click on a title closes its menu', async () => {
    await menuBar();
    await fireEvent.click(title('File'));
    await fireEvent.pointerEnter(title('Theme'));
    await fireEvent.click(title('Theme'));

    await fireEvent.click(title('Theme'));

    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('pointing away and back resets it: then a click closes', async () => {
    await menuBar();
    await fireEvent.click(title('File'));
    await fireEvent.pointerEnter(title('Theme'));

    await fireEvent.pointerLeave(title('Theme'));
    await fireEvent.click(title('Theme'));

    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('a click elsewhere closes the menu', async () => {
    await menuBar();
    await fireEvent.click(title('File'));

    await fireEvent.pointerDown(document.body);

    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });
});
