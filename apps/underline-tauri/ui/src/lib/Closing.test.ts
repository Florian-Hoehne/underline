// Feature F16: closing Underline (M-21.4, UC-005 BR-001 and A14, UC-014
// A9) and the tab limit (M-06.6, UC-004 A6, defect D-07 fixed) against the
// mock backend. The real rules are tested in Rust (`underline-session`);
// these tests check that the window takes part: it answers the close
// handshake, shows "Changes not stored" and closes on "Close anyway".

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { invoke } from '@tauri-apps/api/core';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { MANY, MAX_TABS, MOCK_SAVE_ERROR, mockHooks, TEAM, type MockStart } from './mock-backend';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function workbench(start: MockStart) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/**
 * The shell without the window: a hundred editors would make jsdom slow,
 * and the tab bar's part is tested in `Workbench.test.ts`.
 */
async function shellOnly(start: MockStart) {
  const shell = await connectedShell(start);
  await shell.startWorkspace();
  return shell;
}

const bar = () => screen.getByRole('contentinfo');
const question = () => screen.findByRole('dialog', { name: 'Changes not stored' });
const tabNotes = (shell: Shell) => shell.tabs.tabs.map((tab) => tab.note);

/** Opens `N001` to `N099` after `Home`: `MAX_TABS` tabs, `Home` used longest ago. */
async function fillTabs(shell: Shell) {
  for (let i = 1; i < MAX_TABS; i++) {
    await shell.openNote(`N${String(i).padStart(3, '0')}`);
  }
  await waitFor(() => expect(shell.tabs.tabs).toHaveLength(MAX_TABS));
}

describe('M-21.4 closing Underline (UC-005 BR-001, A14 step 3)', () => {
  it('stores everything and closes', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    await mockHooks().closeWindow();

    await waitFor(() => expect(mockHooks().windowClosed()).toBe(true));
    expect(screen.queryByRole('dialog', { name: 'Changes not stored' })).not.toBeInTheDocument();
  });

  it('asks "Changes not stored"; "Keep open" keeps Underline open', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes', failingSaves: ['Home'] });
    await shell.storeTab('Home');

    await mockHooks().closeWindow();

    const dialog = await question();
    expect(
      within(dialog).getByText(`Changes could not be stored: Home: ${MOCK_SAVE_ERROR}`),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText('Close Underline anyway and lose these changes?'),
    ).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Keep open' })).toHaveFocus();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Keep open' }));

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Changes not stored' })).not.toBeInTheDocument(),
    );
    expect(mockHooks().windowClosed()).toBe(false);
    expect(tabNotes(shell)).toEqual(['Home']);
  });

  it('"Close anyway" closes Underline without the changes', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes', failingSaves: ['Home'] });
    await shell.storeTab('Home');
    await mockHooks().closeWindow();
    const dialog = await question();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close anyway' }));

    await waitFor(() => expect(mockHooks().windowClosed()).toBe(true));
  });

  it('UC-014 A9: with a remote, syncs before closing and then closes', async () => {
    const shell = await workbench({ givenDir: TEAM });
    // The sync of the opened workspace ends first.
    await waitFor(() => expect(within(bar()).getByText('Synced just now')).toBeInTheDocument(), {
      timeout: 3000,
    });

    await mockHooks().closeWindow();

    await waitFor(() => expect(shell.status).toBe('Syncing before closing…'));
    expect(within(bar()).getByText(/\| Syncing before closing…$/)).toBeVisible();
    expect(mockHooks().windowClosed()).toBe(false);
    await waitFor(() => expect(mockHooks().windowClosed()).toBe(true), { timeout: 3000 });
  });
});

describe('M-06.6 at most 100 tabs (UC-004 A6)', () => {
  it('the tab used longest ago closes, and the status says so', async () => {
    const shell = await shellOnly({ givenDir: MANY });
    await fillTabs(shell);

    await shell.openNote('N100');

    await waitFor(() => expect(shell.status).toBe('Closed the tab Home, used longest ago'));
    expect(shell.tabs.tabs).toHaveLength(MAX_TABS);
    expect(tabNotes(shell)).not.toContain('Home');
    expect(tabNotes(shell)).toContain('N100');
  });

  it('D-07 (fixed): a tab whose changes cannot be stored stays, the next one closes', async () => {
    const shell = await shellOnly({ givenDir: MANY, failingSaves: ['Home'] });
    await shell.storeTab('Home');
    await fillTabs(shell);

    await shell.openNote('N100');

    await waitFor(() => expect(shell.status).toBe('Closed the tab N001, used longest ago'));
    expect(tabNotes(shell)).toContain('Home');
    expect(tabNotes(shell)).not.toContain('N001');
    expect(shell.tabs.tabs.find((tab) => tab.note === 'Home')?.label).toBe('Home ⚠');
  });

  it('D-07 (fixed): with no tab to close without losing changes, the note does not open', async () => {
    const shell = await shellOnly({ givenDir: MANY });
    await fillTabs(shell);
    const open = tabNotes(shell);
    mockHooks().failSaves(open);
    for (const note of open) {
      await shell.storeTab(note);
    }

    await shell.openNote('N100');

    await waitFor(() =>
      expect(shell.status).toBe(
        'N100 was not opened: 100 tabs are open, and none can close without losing changes',
      ),
    );
    expect(tabNotes(shell)).toEqual(open);
  });

  it("a change not stored yet in another tab is stored before it closes, and stays the note's", async () => {
    const shell = await shellOnly({ givenDir: MANY });
    // The window's core of Home holds the text; a change comes as the backend's.
    await invoke('ed_mirror', { note: 'Home' });
    await mockHooks().backendEdit('Home', 0, 0, 'typed ');
    await fillTabs(shell);

    await shell.openNote('N100');

    await waitFor(() => expect(shell.status).toBe('Closed the tab Home, used longest ago'));
    expect(mockHooks().noteText('Home').startsWith('typed ')).toBe(true);
  });

  it('review F16: the active tab never closes; then the note does not open', async () => {
    const shell = await shellOnly({ givenDir: MANY });
    await fillTabs(shell);
    const open = tabNotes(shell);
    const others = open.filter((note) => note !== shell.tabs.active);
    mockHooks().failSaves(others);
    for (const note of others) {
      await shell.storeTab(note);
    }

    await shell.openNote('N100');

    await waitFor(() =>
      expect(shell.status).toBe(
        'N100 was not opened: 100 tabs are open, and none can close without losing changes',
      ),
    );
    expect(tabNotes(shell)).toEqual(open);
  });
});
