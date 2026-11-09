import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { emit } from '@tauri-apps/api/event';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import type { TabsView } from './bindings';
import { defaultSettings, mockHandlers, type MockStart } from './mock-backend';

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

const overview = () => screen.getByRole('complementary', { name: 'Notes' });
const tabBar = () => document.querySelector<HTMLElement>('.tab-bar')!;
const entry = (name: string) => within(overview()).getByRole('button', { name });
const tab = (name: string) => within(tabBar()).getByRole('button', { name });
const labels = () =>
  within(overview())
    .getAllByRole('listitem')
    .map((item) => item.textContent);

describe('M-05 note overview', () => {
  it('M-05.1 has the heading "Notes" and the button "New note…"', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    expect(within(overview()).getByRole('heading', { name: 'Notes' })).toBeInTheDocument();
    expect(within(overview()).getByRole('button', { name: 'New note…' })).toBeInTheDocument();
  });

  it('M-05.2 lists recently opened notes first, then by title, with folders for twins', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    expect(labels()).toEqual([
      'Home  (top folder)',
      '2026 11 09',
      'Getting Started',
      'Home  (guide)',
      'Ideas',
      'Reading List',
    ]);
    expect(entry('Home (top folder)')).toHaveAttribute('aria-current', 'true');
  });

  it('M-05.2 a click opens the note, which moves to the top', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });

    await fireEvent.click(entry('Ideas'));

    await waitFor(() => expect(labels()[0]).toBe('Ideas'));
    expect(entry('Ideas')).toHaveAttribute('aria-current', 'true');
    expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home', 'Ideas']);
  });

  it('M-05.1 starts at the stored width', async () => {
    await workbench({ givenDir: '/home/user/Notes', settings: { overviewWidth: 260 } });

    expect(overview()).toHaveStyle({ width: '260px' });
  });

  it('M-05.1 a width dragged more than a pixel is stored when let go', async () => {
    const handlers = mockHandlers({ givenDir: '/home/user/Notes' });
    const settingsQuick = vi.fn(handlers.settings_quick);
    await workbench(
      { givenDir: '/home/user/Notes' },
      { ...handlers, settings_quick: settingsQuick },
    );
    const edge = within(overview()).getByRole('separator');
    edge.setPointerCapture = () => {};
    vi.spyOn(overview(), 'getBoundingClientRect').mockImplementation(() => {
      const width = parseFloat(overview().style.width);
      return { width } as DOMRect;
    });

    await fireEvent.pointerDown(edge, { button: 0, clientX: 200, pointerId: 1 });
    await fireEvent.pointerMove(edge, { clientX: 1500, pointerId: 1 });
    expect(overview()).toHaveStyle({ width: '1000px' });
    await fireEvent.pointerUp(edge, { pointerId: 1 });

    expect(settingsQuick).toHaveBeenCalledWith({ key: 'overview_width', value: '1000' });
  });

  it('M-05.1 a width moved by a pixel or less is not stored', async () => {
    const handlers = mockHandlers({ givenDir: '/home/user/Notes' });
    const settingsQuick = vi.fn(handlers.settings_quick);
    await workbench(
      { givenDir: '/home/user/Notes' },
      { ...handlers, settings_quick: settingsQuick },
    );
    const edge = within(overview()).getByRole('separator');
    edge.setPointerCapture = () => {};
    vi.spyOn(overview(), 'getBoundingClientRect').mockImplementation(
      () => ({ width: parseFloat(overview().style.width) }) as DOMRect,
    );

    await fireEvent.pointerDown(edge, { button: 0, clientX: 200, pointerId: 1 });
    await fireEvent.pointerMove(edge, { clientX: 201, pointerId: 1 });
    await fireEvent.pointerUp(edge, { pointerId: 1 });

    expect(settingsQuick).not.toHaveBeenCalled();
  });

  it('Q-03 a settings file read again sets the width back to the stored one', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });
    shell.overviewWidth = 420;
    await waitFor(() => expect(overview()).toHaveStyle({ width: '420px' }));

    // A theme change keeps the dragged width …
    await emit('settings', { ...defaultSettings(), theme: 'Nord' });
    expect(overview()).toHaveStyle({ width: '420px' });

    // … reading the file again does not.
    await emit('settings', { ...defaultSettings(), overviewWidth: 300, widthGeneration: 1 });
    await waitFor(() => expect(overview()).toHaveStyle({ width: '300px' }));
  });
});

describe('M-06 tabs', () => {
  it('M-06.1 a tab shows its title and its file', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    expect(tab('Home')).toHaveAttribute('title', '/home/user/Notes/Home.md');
  });

  it('M-06.2 a click activates the tab and moves its note up', async () => {
    await workbench({ givenDir: '/home/user/Notes' });
    await fireEvent.click(entry('Ideas'));
    await waitFor(() => expect(tab('Ideas')).toHaveAttribute('aria-current', 'page'));

    await fireEvent.click(tab('Home'));

    await waitFor(() => expect(tab('Home')).toHaveAttribute('aria-current', 'page'));
    expect(labels()[0]).toBe('Home  (top folder)');
  });

  it('M-06.3 "×" closes a tab', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });
    await fireEvent.click(entry('Ideas'));
    await waitFor(() => expect(tab('Ideas')).toBeInTheDocument());

    await fireEvent.click(within(tabBar()).getByRole('button', { name: 'Close Ideas' }));

    await waitFor(() => expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home']));
  });

  it('M-06.3 a middle click closes a tab', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });

    await fireEvent(tab('Home'), new MouseEvent('auxclick', { button: 1, bubbles: true }));

    await waitFor(() => expect(shell.tabs.tabs).toEqual([]));
    expect(screen.getByText('Select or create a note.')).toBeInTheDocument();
  });

  it('M-06.5 while the tab "Todos" is active, no note tab is', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });
    const tabs: TabsView = { ...shell.tabs, todoList: { open: true, active: true } };

    await emit('tabs', tabs);

    await waitFor(() => expect(tab('Home')).not.toHaveAttribute('aria-current'));
    expect(entry('Home (top folder)')).toHaveAttribute('aria-current', 'true');
    expect(screen.queryByText('Select or create a note.')).not.toBeInTheDocument();
  });

  it('the active tab shows the area of its note', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    expect(screen.getByRole('region', { name: 'Home' })).toHaveAttribute('data-view', 'facade');
    expect(screen.queryByText('Select or create a note.')).not.toBeInTheDocument();
  });

  it('M-06.7 without a tab it asks to select or create a note', async () => {
    await workbench({ givenDir: '/home/user/Letters' });

    expect(screen.getByText('Select or create a note.')).toBeInTheDocument();
  });
});

describe('review F2: tabs and focus', () => {
  it('after "×" the keyboard goes to the tab now at its place', async () => {
    await workbench({ givenDir: '/home/user/Notes' });
    await fireEvent.click(entry('Ideas'));
    await waitFor(() => expect(tab('Ideas')).toBeInTheDocument());
    await fireEvent.click(tab('Home'));

    await fireEvent.click(within(tabBar()).getByRole('button', { name: 'Close Home' }));

    await waitFor(() => expect(tab('Ideas')).toHaveFocus());
  });

  it('after the last tab closes the keyboard goes to the overview', async () => {
    await workbench({ givenDir: '/home/user/Notes' });

    await fireEvent.click(within(tabBar()).getByRole('button', { name: 'Close Home' }));

    await waitFor(() => expect(entry('Home (top folder)')).toHaveFocus());
  });

  it('the active tab scrolls into view', async () => {
    const scroll = vi.fn();
    Element.prototype.scrollIntoView = scroll;
    await workbench({ givenDir: '/home/user/Notes' });

    await fireEvent.click(entry('Ideas'));

    await waitFor(() =>
      expect(scroll).toHaveBeenLastCalledWith({ inline: 'nearest', block: 'nearest' }),
    );
  });
});
