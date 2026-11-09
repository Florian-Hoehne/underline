import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { connectedShell } from '../test/shell';
import { mockHandlers } from './mock-backend';
import TopBar from './TopBar.svelte';

afterEach(() => {
  clearMocks();
  vi.restoreAllMocks();
});

describe('M-03 top bar', () => {
  it('M-03.1 the burger button is "Menu" and offers to show the hidden menu', async () => {
    const shell = await connectedShell();

    render(TopBar, { props: { shell } });

    const burger = screen.getByRole('button', { name: 'Menu' });
    expect(burger).toHaveAttribute('title', 'Show the menu');
    expect(burger).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('menubar')).not.toBeInTheDocument();
  });

  it('M-03.1 the burger button shows the menu bar and stores the switch', async () => {
    const handlers = mockHandlers();
    const settingsQuick = vi.fn(handlers.settings_quick);
    const shell = await connectedShell({}, { settings_quick: settingsQuick });
    render(TopBar, { props: { shell } });

    await fireEvent.click(screen.getByRole('button', { name: 'Menu' }));

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'show_menu_bar', value: 'true' });
    await waitFor(() => expect(screen.getByRole('menubar')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Menu' })).toHaveAttribute('title', 'Hide the menu');
  });

  it('M-03.1 the burger button hides a shown menu bar', async () => {
    const handlers = mockHandlers({ settings: { showMenuBar: true } });
    const settingsQuick = vi.fn(handlers.settings_quick);
    const shell = await connectedShell(
      { settings: { showMenuBar: true } },
      { settings_quick: settingsQuick },
    );
    render(TopBar, { props: { shell } });

    await fireEvent.click(screen.getByRole('button', { name: 'Menu' }));

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'show_menu_bar', value: 'false' });
  });

  it.each([
    ['linux', 'Search notes  Ctrl+Shift+F'],
    ['windows', 'Search notes  Ctrl+Shift+F'],
    ['macos', 'Search notes  Cmd+Shift+F'],
  ] as const)('M-03.2 the magnifier is "Search" with its hint on %s', async (platform, hint) => {
    const shell = await connectedShell({ platform });

    render(TopBar, { props: { shell } });

    expect(screen.getByRole('button', { name: 'Search' })).toHaveAttribute('title', hint);
  });

  it('M-03.3 without the menu bar, says that no workspace is open', async () => {
    const shell = await connectedShell();

    render(TopBar, { props: { shell } });

    expect(screen.getByText('No workspace open')).toBeInTheDocument();
  });

  it('M-03.3 without the menu bar, names the workspace by its folder', async () => {
    const shell = await connectedShell();
    shell.workspace = { root: '/home/me/Notes', name: 'Notes' };

    render(TopBar, { props: { shell } });

    expect(screen.getByText('Notes')).toBeInTheDocument();
  });

  it('M-03.3 with the menu bar, the menus stand in place of the name', async () => {
    const shell = await connectedShell({ settings: { showMenuBar: true } });

    render(TopBar, { props: { shell } });

    expect(screen.getByRole('menubar')).toBeInTheDocument();
    expect(screen.queryByText('No workspace open')).not.toBeInTheDocument();
  });
});
