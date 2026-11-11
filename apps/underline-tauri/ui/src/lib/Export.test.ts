// Feature F11: exporting a note (UC-009; inventory M-15, M-04.8, the
// "Export" entries of M-05.4 and M-06.4), against the mock backend with its
// fake save dialog.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { listen } from '@tauri-apps/api/event';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHooks, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
const WORK: MockStart = { givenDir: '/home/user/Work' };

async function workbench(start: MockStart = NOTES) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const overview = () => screen.getByRole('complementary', { name: 'Notes' });
const entry = (name: string) => within(overview()).getByRole('button', { name });
const tabBar = () => document.querySelector<HTMLElement>('.tab-bar')!;
const tab = (name: string) => within(tabBar()).getByRole('tab', { name });
const dialog = (name: string) => screen.findByRole('dialog', { name });
const statusBar = () => document.querySelector<HTMLElement>('.status-bar')!;

describe('M-15.1 Export note (UC-009 step 1, BR-008)', () => {
  it('Ctrl+Shift+E asks for the kind; Up, Down and Enter choose it', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');

    await fireEvent.keyDown(document.body, { key: 'E', ctrlKey: true, shiftKey: true });

    const found = await dialog('Export note');
    expect(within(found).getByText('Export Ideas as:')).toBeTruthy();
    const list = within(found).getByRole('listbox');
    await waitFor(() => expect(list).toHaveFocus());
    const options = within(list).getAllByRole('option');
    expect(options.map((o) => o.textContent?.trim())).toEqual(['AsciiDoc', 'HTML', 'PDF']);
    expect(options[0]).toHaveAttribute('aria-selected', 'true');

    await fireEvent.keyDown(list, { key: 'ArrowDown' });
    await fireEvent.keyDown(list, { key: 'ArrowDown' });
    await fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(within(list).getAllByRole('option')[2]).toHaveAttribute('aria-selected', 'true');
    await fireEvent.keyDown(list, { key: 'ArrowUp' });
    await fireEvent.keyDown(list, { key: 'Enter' });

    // TC-009-16: the dialog proposes Ideas.html in the home folder.
    await waitFor(() => expect(shell.status).toBe('Exported Ideas to /home/user/Ideas.html'));
    expect(mockHooks().exportedFiles()['/home/user/Ideas.html']).toContain('# Ideas');
    expect(screen.queryByRole('dialog', { name: 'Export note' })).toBeNull();
  });

  it('A1: Escape exports nothing', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await fireEvent.keyDown(document.body, { key: 'e', ctrlKey: true, shiftKey: true });
    const found = await dialog('Export note');

    await fireEvent.keyDown(within(found).getByRole('listbox'), { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Export note' })).toBeNull());
    expect(Object.keys(mockHooks().exportedFiles())).toEqual(['/home/user/out/Plan.html']);
  });

  it('M-06.4: "Export…" of the tab menu asks for the kind; a click chooses', async () => {
    const shell = await workbench();
    await shell.openNote('Trip');

    await fireEvent.contextMenu(tab('Trip'), { clientX: 40, clientY: 40 });
    await fireEvent.click(await screen.findByRole('menuitem', { name: /Export…/ }));
    const found = await dialog('Export note');
    await fireEvent.click(within(found).getByRole('option', { name: 'AsciiDoc' }));

    await waitFor(() => expect(shell.status).toBe('Exported Trip to /home/user/Trip.adoc'));
  });

  it('D-11 fixed: the tab menu names Cmd+Shift+E on macOS', async () => {
    const shell = await workbench({ ...NOTES, platform: 'macos' });
    await shell.openNote('Trip');

    await fireEvent.contextMenu(tab('Trip'), { clientX: 40, clientY: 40 });

    const item = await screen.findByRole('menuitem', { name: /Export…/ });
    expect(item).toHaveAttribute('aria-keyshortcuts', 'Cmd+Shift+E');
  });
});

describe('M-05.4 the submenu "Export" of the note overview', () => {
  it('names the kinds and exports at once, without asking for the kind', async () => {
    const shell = await workbench();

    await fireEvent.contextMenu(entry('Reading List'), { clientX: 20, clientY: 20 });
    const menu = await screen.findByRole('menu', { name: 'Note Reading List' });
    const exportItem = within(menu).getByRole('menuitem', { name: /Export/ });
    expect(exportItem).toHaveAttribute('aria-haspopup', 'menu');

    // The keyboard opens it on its first entry; Left closes it again.
    exportItem.focus();
    await fireEvent.keyDown(exportItem, { key: 'ArrowRight' });
    const sub = await screen.findByRole('menu', { name: 'Export' });
    const kinds = within(sub).getAllByRole('menuitem');
    expect(kinds.map((k) => k.textContent?.trim())).toEqual(['AsciiDoc…', 'HTML…', 'PDF…']);
    await waitFor(() => expect(kinds[0]).toHaveFocus());
    await fireEvent.keyDown(kinds[0], { key: 'ArrowLeft' });
    expect(screen.queryByRole('menu', { name: 'Export' })).toBeNull();
    expect(exportItem).toHaveFocus();

    await fireEvent.click(exportItem);
    const again = await screen.findByRole('menu', { name: 'Export' });
    await fireEvent.click(within(again).getByRole('menuitem', { name: 'PDF…' }));

    expect(screen.queryByRole('menu')).toBeNull();
    expect(screen.queryByRole('dialog', { name: 'Export note' })).toBeNull();
    await waitFor(() => expect(shell.status).toBe('Exporting Reading List…'));
    await waitFor(() =>
      expect(shell.status).toBe('Exported Reading List to /home/user/Reading-List.pdf'),
    );
  });
});

describe('M-15.2 to M-15.5 the place and the report (UC-009 steps 2–5)', () => {
  it('BR-001: the tab text with its unsaved changes; the next dialog starts in the last folder', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await mockHooks().backendEdit('Ideas', 0, 0, 'Fresh line\n');
    mockHooks().exportDialog({ answer: 'choose', path: '/home/user/out/Ideas.html' });

    await shell.startExport('Ideas', 'html');

    await waitFor(() => expect(shell.status).toBe('Exported Ideas to /home/user/out/Ideas.html'));
    expect(mockHooks().exportedFiles()['/home/user/out/Ideas.html']).toContain('Fresh line');

    mockHooks().exportDialog({ answer: 'choose' });
    await shell.startExport('Trip', 'pdf');
    await waitFor(() => expect(shell.status).toBe('Exported Trip to /home/user/out/Trip.pdf'));
  });

  it('M-04.8: "Open" opens the export, and goes with the next report', async () => {
    const shell = await workbench();

    await shell.startExport('Trip', 'html');

    const open = await within(statusBar()).findByRole('button', { name: 'Open exported file' });
    expect(open).toHaveTextContent('Open');
    expect(
      within(statusBar()).queryByRole('button', { name: 'Show approximated blocks' }),
    ).toBeNull();
    await fireEvent.click(open);
    await waitFor(() => expect(mockHooks().openedWithSystem()).toEqual(['/home/user/Trip.html']));

    await shell.checkWiki();
    await waitFor(() =>
      expect(within(statusBar()).queryByRole('button', { name: 'Open exported file' })).toBeNull(),
    );
  });

  it('A2, M-15.6: "Show" lists the approximated blocks by line', async () => {
    const shell = await workbench(WORK);

    await shell.startExport('Facade-Tour', 'otherFormat');

    await waitFor(() => expect(shell.status).toMatch(/; 1 block only approximated$/));
    await fireEvent.click(
      await within(statusBar()).findByRole('button', { name: 'Show approximated blocks' }),
    );
    const found = await dialog('Approximated blocks of Facade Tour');
    expect(
      within(found).getByText('These blocks of the note have no exact counterpart in the export:'),
    ).toBeTruthy();
    expect(
      within(found)
        .getAllByRole('listitem')
        .map((i) => i.textContent),
    ).toEqual([expect.stringMatching(/^Line \d+$/)]);
    const close = within(found).getByRole('button', { name: 'Close' });
    await waitFor(() => expect(close).toHaveFocus());
    // UX review F11: it stays when the status moves on, until closed.
    await shell.checkWiki();
    await waitFor(() =>
      expect(within(statusBar()).queryByRole('button', { name: /Show/ })).toBeNull(),
    );
    expect(screen.getByRole('dialog', { name: 'Approximated blocks of Facade Tour' })).toBeTruthy();
    await fireEvent.click(close);
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: /Approximated blocks/ })).toBeNull(),
    );
  });

  it('A1: a cancelled system dialog writes nothing and says nothing', async () => {
    const shell = await workbench();
    const before = shell.status;
    mockHooks().exportDialog({ answer: 'cancel' });

    await shell.startExport('Trip', 'pdf');

    expect(shell.status).toBe(before);
    expect(shell.exporting).toBeNull();
    expect(Object.keys(mockHooks().exportedFiles())).toEqual(['/home/user/out/Plan.html']);
  });

  it('A4: an export that cannot be written says why', async () => {
    const shell = await workbench();
    mockHooks().exportDialog({ answer: 'choose', path: '/home/user/locked/Trip.html' });

    await shell.startExport('Trip', 'html');

    await waitFor(() =>
      expect(shell.status).toBe('Trip could not be exported: Permission denied (os error 13)'),
    );
    // UX review F11: marked as an error, not by colour alone.
    expect(within(statusBar()).getByRole('alert')).toHaveTextContent(
      /Error: .*Trip could not be exported/,
    );
    expect(within(statusBar()).queryByRole('button', { name: /Open/ })).toBeNull();
  });

  it('A5, M-15.3: a note is not replaced; the place is asked again', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    mockHooks().exportDialog({ answer: 'choose', path: '/home/user/Notes/Ideas.md' });
    const started = shell.startExport('Trip', 'otherFormat');

    await waitFor(() =>
      expect(shell.status).toBe('A note cannot be replaced by an export; choose another place'),
    );
    // The dialog shows again; this time the user cancels.
    mockHooks().exportDialog({ answer: 'cancel' });
    await started;
    expect(shell.exporting).toBeNull();
    expect(mockHooks().exportedFiles()['/home/user/Notes/Ideas.md']).toBeUndefined();
  });
});

describe('M-15.2 without a system dialog (ADR-016)', () => {
  it('the path is typed; an existing file asks first, for that path only (D-03 fixed)', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    mockHooks().exportDialog({ answer: 'unavailable' });

    await shell.startExport('Ideas', 'html');

    const found = await dialog('Export note');
    const field = within(found).getByLabelText('Export Ideas as HTML to:');
    expect(field).toHaveValue('/home/user/Ideas.html');
    await waitFor(() => expect(field).toHaveFocus());

    await fireEvent.input(field, { target: { value: '/home/user/out/Plan.html' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Export' }));
    const again = await dialog('Export note');
    expect(await within(again).findByRole('alert')).toHaveTextContent(
      'The file exists. Export again to replace it.',
    );
    expect(within(again).getByRole('button', { name: 'Replace' })).toBeTruthy();

    // Another path: the agreement does not hold, the button says "Export" again.
    const path = within(again).getByLabelText('Export Ideas as HTML to:');
    await fireEvent.input(path, { target: { value: '/home/user/out/Other.html' } });
    expect(within(again).getByRole('button', { name: 'Export' })).toBeTruthy();
    expect(within(again).queryByRole('alert')).toBeNull();
    await fireEvent.input(path, { target: { value: '/home/user/out/Plan.html' } });
    expect(within(again).getByRole('button', { name: 'Replace' })).toBeTruthy();

    await fireEvent.click(within(again).getByRole('button', { name: 'Replace' }));
    await waitFor(() => expect(shell.status).toBe('Exported Ideas to /home/user/out/Plan.html'));
    expect(mockHooks().exportedFiles()['/home/user/out/Plan.html']).toContain('# Ideas');
  });
});

describe('Review F11', () => {
  it('the path field: empty cannot be exported, a folder or a note says why in the field', async () => {
    const shell = await workbench();
    mockHooks().exportDialog({ answer: 'unavailable' });
    await shell.startExport('Ideas', 'html');
    let found = await dialog('Export note');
    const field = () =>
      within(screen.getByRole('dialog', { name: 'Export note' })).getByRole('textbox');

    await fireEvent.input(field(), { target: { value: '  ' } });
    expect(within(found).getByRole('button', { name: 'Export' })).toBeDisabled();

    await fireEvent.input(field(), { target: { value: '/home/user/out' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Export' }));
    found = await dialog('Export note');
    expect(await within(found).findByRole('alert')).toHaveTextContent(
      '/home/user/out is a folder. Enter a file name in it.',
    );
    expect(field()).toHaveValue('/home/user/out');
    expect(field()).toHaveAttribute('aria-describedby');

    await fireEvent.input(field(), { target: { value: '/home/user/Notes/.todos/1.yaml' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Export' }));
    found = await dialog('Export note');
    expect(await within(found).findByRole('alert')).toHaveTextContent(
      'A note cannot be replaced by an export; choose another place',
    );
    expect(shell.status).toBe('A note cannot be replaced by an export; choose another place');
  });

  it('a failed export puts the typed path back with the reason', async () => {
    const shell = await workbench();
    mockHooks().exportDialog({ answer: 'unavailable' });
    await shell.startExport('Ideas', 'html');
    const found = await dialog('Export note');

    await fireEvent.input(within(found).getByRole('textbox'), {
      target: { value: '/home/user/locked/Ideas.html' },
    });
    await fireEvent.click(within(found).getByRole('button', { name: 'Export' }));

    const again = await dialog('Export note');
    expect(await within(again).findByRole('alert')).toHaveTextContent(
      'Ideas could not be exported: Permission denied (os error 13)',
    );
    expect(within(again).getByRole('textbox')).toHaveValue('/home/user/locked/Ideas.html');
    expect(shell.statusError).toBe(true);
  });

  it('the status says where the dialog is, and the bar is busy while exporting', async () => {
    const shell = await workbench();
    const statuses: string[] = [];
    const busy: boolean[] = [];
    const stop = await listen<{ text: string; busy: boolean }>('status', (event) => {
      statuses.push(event.payload.text);
      busy.push(event.payload.busy);
    });

    await shell.startExport('Trip', 'pdf');
    stop();

    expect(statuses).toContain('Choosing where to save Trip…');
    expect(statuses).toContain('Exporting Trip…');
    expect(busy).toContain(true);
    expect(shell.statusBusy).toBe(false);
    expect(statusBar()).toHaveAttribute('aria-busy', 'false');
  });

  it('two exports ending together keep both reports, named by note', async () => {
    const shell = await workbench();

    const trip = shell.startExport('Trip', 'pdf');
    await waitFor(() => expect(shell.status).toBe('Exporting Trip…'));
    // One dialog at a time: a second start while the PDF is set works.
    await shell.startExport('Ideas', 'html');
    await trip;

    await waitFor(() =>
      expect(shell.status).toBe(
        'Exported Ideas to /home/user/Ideas.html\nExported Trip to /home/user/Trip.pdf',
      ),
    );
    expect(
      within(statusBar()).getByRole('button', { name: 'Open exported file Trip' }),
    ).toHaveTextContent('Open Trip');
    expect(
      within(statusBar()).getByRole('button', { name: 'Open exported file Ideas' }),
    ).toBeTruthy();
  });

  it('Home and End in the kinds; Ctrl+Shift+E brings the open window back', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await fireEvent.keyDown(document.body, { key: 'E', ctrlKey: true, shiftKey: true });
    const found = await dialog('Export note');
    const list = within(found).getByRole('listbox');

    await fireEvent.keyDown(list, { key: 'End' });
    expect(within(list).getAllByRole('option')[2]).toHaveAttribute('aria-selected', 'true');
    await fireEvent.keyDown(list, { key: 'Home' });
    expect(within(list).getAllByRole('option')[0]).toHaveAttribute('aria-selected', 'true');

    within(found).getByRole('button', { name: 'Cancel' }).focus();
    await fireEvent.keyDown(document.body, { key: 'E', ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(within(found).getByRole('listbox')).toHaveFocus());
  });

  it('Ctrl+Shift+E does nothing while another dialog is open', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await shell.startRename('Ideas');
    await dialog('Rename note');
    // The dialog plans its file name first, so no plan outlives the test.
    await new Promise((done) => setTimeout(done, 300));

    await fireEvent.keyDown(document.body, { key: 'E', ctrlKey: true, shiftKey: true });

    expect(shell.exporting).toBeNull();
    expect(screen.queryByRole('dialog', { name: 'Export note' })).toBeNull();
  });

  it('moving to another entry closes the open submenu', async () => {
    await workbench();
    await fireEvent.contextMenu(entry('Trip'), { clientX: 20, clientY: 20 });
    const menu = await screen.findByRole('menu', { name: 'Note Trip' });
    const exportItem = within(menu).getByRole('menuitem', { name: /Export/ });
    await fireEvent.pointerEnter(exportItem);
    expect(await screen.findByRole('menu', { name: 'Export' })).toBeTruthy();
    exportItem.focus();

    await fireEvent.keyDown(exportItem, { key: 'ArrowUp' });

    expect(screen.queryByRole('menu', { name: 'Export' })).toBeNull();
    expect(within(menu).getByRole('menuitem', { name: /Delete/ })).toHaveFocus();
  });
});

describe('Retest F11', () => {
  it('a note typed in the path field is refused before "Replace" is asked', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    mockHooks().exportDialog({ answer: 'unavailable' });
    await shell.startExport('Ideas', 'html');
    const found = await dialog('Export note');

    await fireEvent.input(within(found).getByRole('textbox'), {
      target: { value: '/home/user/Notes/Trip.md' },
    });
    await fireEvent.click(within(found).getByRole('button', { name: 'Export' }));

    const again = await dialog('Export note');
    expect(await within(again).findByRole('alert')).toHaveTextContent(
      'A note cannot be replaced by an export; choose another place',
    );
    expect(within(again).getByRole('button', { name: 'Export' })).toBeTruthy();
  });

  it('a cancelled system dialog gives back the report with Open and Show', async () => {
    const shell = await workbench(WORK);
    await shell.startExport('Facade-Tour', 'otherFormat');
    await within(statusBar()).findByRole('button', { name: 'Show approximated blocks' });
    const report = shell.status;
    mockHooks().exportDialog({ answer: 'cancel' });

    await shell.startExport('README', 'pdf');

    await waitFor(() => expect(shell.status).toBe(report));
    expect(within(statusBar()).getByRole('button', { name: 'Open exported file' })).toBeTruthy();
    await fireEvent.click(
      within(statusBar()).getByRole('button', { name: 'Show approximated blocks' }),
    );
    expect(await dialog('Approximated blocks of Facade Tour')).toBeTruthy();
  });
});
