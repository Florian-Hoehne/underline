// Feature F8: creating, renaming, deleting and restoring notes (UC-003,
// UC-006, UC-007; inventory M-05.4 to M-05.6, M-06.4 and M-14), against
// the mock backend.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { coreOf } from './editor/sync';
import { mockHooks, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
const FOLDER_LABEL = 'Folder (empty for the top folder, new folders are created):';

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

/** Types `text` key by key at the caret, over a selection, as a user does. */
async function typeKeys(field: HTMLElement, text: string) {
  const input = field as HTMLInputElement;
  for (const char of text) {
    const start = input.selectionStart ?? input.value.length;
    const end = input.selectionEnd ?? start;
    input.value = input.value.slice(0, start) + char + input.value.slice(end);
    input.setSelectionRange(start + 1, start + 1);
    await fireEvent.input(input);
    await new Promise((done) => setTimeout(done, 5));
  }
}

/** A beforeinput on the facade's text. */
function facadeInput(inputType: string, data: string | null = null) {
  document
    .querySelector('.facade-text')!
    .dispatchEvent(
      new InputEvent('beforeinput', { inputType, data, bubbles: true, cancelable: true }),
    );
}

describe('M-14.1 New note (UC-003)', () => {
  it('starts in the folder of the active note, shows the file and creates the note', async () => {
    const shell = await workbench();
    await shell.openNote('guide/Getting-Started');

    await fireEvent.click(within(overview()).getByRole('button', { name: 'New note…' }));

    const found = await dialog('New note');
    const title = within(found).getByLabelText('Title:');
    await waitFor(() => expect(title).toHaveFocus());
    expect(within(found).getByLabelText(FOLDER_LABEL)).toHaveValue('guide');
    expect(within(found).getByRole('radio', { name: 'Markdown' })).toBeChecked();

    await fireEvent.input(title, { target: { value: 'First  Steps' } });
    await waitFor(() => expect(within(found).getByText('File: guide/First-Steps.md')).toBeTruthy());
    await fireEvent.click(within(found).getByRole('radio', { name: 'AsciiDoc' }));
    await waitFor(() =>
      expect(within(found).getByText('File: guide/First-Steps.adoc')).toBeTruthy(),
    );
    await fireEvent.submit(title);

    await waitFor(() => expect(shell.tabs.active).toBe('guide/First-Steps'));
    await waitFor(() => expect(shell.status).toBe('Created the note First Steps'));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull());
  });

  it('the File menu opens it too, and Escape closes it (D-02 fixed)', async () => {
    await workbench({ ...NOTES, settings: { showMenuBar: true } });
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'File' }));

    await fireEvent.click(screen.getByRole('menuitem', { name: 'New note…' }));

    const found = await dialog('New note');
    await fireEvent.keyDown(within(found).getByLabelText('Title:'), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull());
  });

  it('A2, A3: a note that exists is offered to open', async () => {
    const shell = await workbench();
    await fireEvent.click(within(overview()).getByRole('button', { name: 'New note…' }));
    const found = await dialog('New note');

    await fireEvent.input(within(found).getByLabelText('Title:'), { target: { value: 'ideas' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Create' }));

    expect(await within(found).findByRole('alert')).toHaveTextContent(
      'The note Ideas exists. Change the title or the folder, or open it.',
    );
    await fireEvent.click(within(found).getByRole('button', { name: 'Open the existing note' }));
    await waitFor(() => expect(shell.tabs.active).toBe('Ideas'));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull());
  });

  it('A1 keeps the dialog with the reason; A5 closes it and says why', async () => {
    const shell = await workbench();
    await fireEvent.click(within(overview()).getByRole('button', { name: 'New note…' }));
    const found = await dialog('New note');
    const title = within(found).getByLabelText('Title:');

    await fireEvent.input(title, { target: { value: 'a:b' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Create' }));
    expect(await within(found).findByRole('alert')).toHaveTextContent(
      'The title cannot hold the character `:`.',
    );

    await fireEvent.input(title, { target: { value: 'Plan' } });
    await fireEvent.input(within(found).getByLabelText(FOLDER_LABEL), {
      target: { value: 'locked' },
    });
    await fireEvent.click(within(found).getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull());
    await waitFor(() => expect(shell.status).toMatch(/^The note could not be created: /));
  });
});

describe('M-14.2 Rename note (UC-006)', () => {
  it('F2 renames the active note; the tab keeps its editor core', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await waitFor(() => expect(coreOf('Ideas')).toBeDefined());
    const core = coreOf('Ideas');

    await fireEvent.keyDown(document.body, { key: 'F2' });

    const found = await dialog('Rename note');
    const title = within(found).getByLabelText('Title:') as HTMLInputElement;
    await waitFor(() => expect(title).toHaveFocus());
    expect(title.value).toBe('Ideas');
    expect([title.selectionStart, title.selectionEnd]).toEqual([0, 5]);
    expect(within(found).queryByText(/start page of a GitHub wiki/)).toBeNull();

    await fireEvent.input(title, { target: { value: 'Big Ideas' } });
    await waitFor(() => expect(within(found).getByText('File: Big-Ideas.md')).toBeTruthy());
    await fireEvent.submit(title);

    await waitFor(() => expect(shell.tabs.active).toBe('Big-Ideas'));
    await waitFor(() =>
      expect(shell.status).toBe('Renamed Ideas to Big Ideas; updated 0 links in 0 notes'),
    );
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Rename note' })).toBeNull());
    expect(coreOf('Big-Ideas')).toBe(core);
    expect(coreOf('Ideas')).toBeUndefined();
    expect(tab('Big Ideas')).toBeInTheDocument();
  });

  it('the Home note warns about the wiki, and a taken name keeps the dialog', async () => {
    await workbench();

    await fireEvent.keyDown(document.body, { key: 'F2' });

    const found = await dialog('Rename note');
    expect(
      within(found).getByText(
        'This is the Home note, the start page of a GitHub wiki. Renamed or moved, the wiki has no start page.',
      ),
    ).toBeTruthy();
    await fireEvent.input(within(found).getByLabelText('Title:'), { target: { value: 'Ideas' } });
    await fireEvent.click(within(found).getByRole('button', { name: 'Rename' }));
    expect(await within(found).findByRole('alert')).toHaveTextContent(
      'A note named Ideas exists in that folder. Change the title or the folder.',
    );
    await fireEvent.click(within(found).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Rename note' })).toBeNull());
  });

  it('M-06.4: the context menu of a tab offers "Rename…  F2", "Delete…" and "Export…"', async () => {
    await workbench();
    const home = tab('Home');

    await fireEvent.contextMenu(home, { clientX: 40, clientY: 40 });

    const menu = await screen.findByRole('menu', { name: 'Tab Home' });
    const items = within(menu).getAllByRole('menuitem');
    expect(items.map((item) => item.textContent?.replace(/\s+/g, ' ').trim())).toEqual([
      'Rename… F2',
      'Delete…',
      'Export… Ctrl+Shift+E',
    ]);
    await waitFor(() => expect(items[0]).toHaveFocus());
    await fireEvent.keyDown(items[0], { key: 'ArrowDown' });
    expect(items[1]).toHaveFocus();
    await fireEvent.keyDown(items[1], { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    expect(home).toHaveFocus();

    await fireEvent.keyDown(home, { key: 'F10', shiftKey: true });
    const again = await screen.findByRole('menu', { name: 'Tab Home' });
    await fireEvent.click(within(again).getByRole('menuitem', { name: /Rename…/ }));
    expect(await dialog('Rename note')).toBeInTheDocument();
  });
});

describe('M-14.3 Delete note (UC-007), M-05.4, M-05.5', () => {
  it('the overview menu asks, the tab closes, and Undo brings it back', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');

    await fireEvent.contextMenu(entry('Ideas'), { clientX: 20, clientY: 20 });
    const menu = await screen.findByRole('menu', { name: 'Note Ideas' });
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent?.replace(/\s+/g, ' ').trim()),
    ).toEqual(['Rename…', 'Delete… Delete', 'Export ▸']);
    await fireEvent.click(within(menu).getByRole('menuitem', { name: 'Delete…' }));

    const found = await dialog('Delete note');
    expect(within(found).getByText('Delete the note Ideas?')).toBeTruthy();
    expect(within(found).getByText('No links in other notes lead to it.')).toBeTruthy();
    expect(within(found).getByText('It is kept among the deleted notes for 30 days.')).toBeTruthy();
    const confirm = within(found).getByRole('button', { name: 'Delete' });
    await waitFor(() => expect(confirm).toHaveFocus());
    await fireEvent.click(confirm);

    await waitFor(() => expect(shell.status).toBe('Deleted Ideas'));
    expect(shell.tabs.tabs.map((t) => t.note)).not.toContain('Ideas');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Delete note' })).toBeNull());
    const undo = within(statusBar()).getByRole('button', { name: 'Undo' });

    await fireEvent.click(undo);

    await waitFor(() => expect(shell.status).toBe('Restored Ideas'));
    expect(shell.tabs.tabs.map((t) => t.note)).toContain('Ideas');
    expect(within(statusBar()).queryByRole('button', { name: 'Undo' })).toBeNull();
  });

  it('M-05.5: Delete on the note last clicked asks; Escape changes nothing', async () => {
    const shell = await workbench();
    await fireEvent.click(entry('Reading List'));
    await waitFor(() => expect(entry('Reading List')).toHaveFocus());

    await fireEvent.keyDown(entry('Reading List'), { key: 'Delete' });

    const found = await dialog('Delete note');
    expect(within(found).getByText('Delete the note Reading List?')).toBeTruthy();
    await fireEvent.keyDown(within(found).getByRole('button', { name: 'Delete' }), {
      key: 'Escape',
    });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Delete note' })).toBeNull());
    expect(shell.overview.map((item) => item.note)).toContain('Reading-List');
  });

  it('the Home note warns about the wiki', async () => {
    await workbench();

    await fireEvent.contextMenu(tab('Home'), { clientX: 40, clientY: 40 });
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete…' }));

    const found = await dialog('Delete note');
    expect(
      within(found).getByText(
        'This is the Home note, the start page of a GitHub wiki. Deleted, the wiki has no start page.',
      ),
    ).toBeTruthy();
  });

  it('A8: a note that cannot be kept is deleted for good after a second question', async () => {
    const shell = await workbench();
    mockHooks().cannotKeep(true);
    await fireEvent.contextMenu(entry('Ideas'), { clientX: 20, clientY: 20 });
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete…' }));
    const found = await dialog('Delete note');

    await fireEvent.click(within(found).getByRole('button', { name: 'Delete' }));

    expect(await within(found).findByRole('alert')).toHaveTextContent(
      /^The note cannot be kept among the deleted notes \(.*\), so it cannot be restored\. Delete it for good\?$/,
    );
    expect(within(found).queryByText('It is kept among the deleted notes for 30 days.')).toBeNull();
    await fireEvent.click(within(found).getByRole('button', { name: 'Delete for good' }));

    await waitFor(() => expect(shell.status).toBe('Deleted Ideas'));
    expect(within(statusBar()).queryByRole('button', { name: 'Undo' })).toBeNull();
  });
});

describe('M-05.6, M-14.4 the deleted notes (UC-007 A5 to A7)', () => {
  async function unfolded() {
    const shell = await workbench();
    const fold = within(overview()).getByRole('button', { name: 'Deleted notes (3)' });
    expect(fold).toHaveAttribute('aria-expanded', 'false');
    await fireEvent.click(fold);
    expect(fold).toHaveAttribute('aria-expanded', 'true');
    return shell;
  }

  it('are folded at first and list title, time, Restore and Remove', async () => {
    await unfolded();

    const draft = within(overview()).getByText('Draft');
    expect(draft.getAttribute('title')).toMatch(/^guide · deleted \d{4}-\d\d-\d\d \d\d:\d\d$/);
    expect(within(overview()).getByRole('button', { name: /^Restore Old Plan \(/ })).toBeTruthy();
    expect(within(overview()).getByRole('button', { name: /^Remove Old Plan \(/ })).toBeTruthy();
    expect(
      within(overview()).getByRole('button', { name: 'Empty the deleted notes' }),
    ).toBeTruthy();
  });

  it('Restore puts a note back without a tab (BR-006)', async () => {
    const shell = await unfolded();

    await fireEvent.click(within(overview()).getByRole('button', { name: /^Restore Old Plan \(/ }));

    await waitFor(() => expect(shell.status).toBe('Restored Old Plan'));
    expect(shell.overview.map((item) => item.note)).toContain('Old-Plan');
    expect(shell.tabs.tabs.map((t) => t.note)).not.toContain('Old-Plan');
    expect(within(overview()).getByRole('button', { name: 'Deleted notes (2)' })).toBeTruthy();
  });

  it('A6: a taken place asks for another title or folder', async () => {
    const shell = await unfolded();

    await fireEvent.click(within(overview()).getByRole('button', { name: /^Restore Ideas \(/ }));

    const found = await dialog('Restore note');
    expect(within(found).getByRole('alert')).toHaveTextContent(
      'A note named Ideas exists in that folder. Choose another title or folder.',
    );
    const title = within(found).getByLabelText('Title:');
    await waitFor(() => expect(title).toHaveFocus());
    await fireEvent.input(title, { target: { value: 'Older Ideas' } });
    await fireEvent.submit(title);

    await waitFor(() => expect(shell.status).toBe('Restored Older Ideas'));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Restore note' })).toBeNull());
  });

  it('A7: Remove and "Empty the deleted notes" ask first', async () => {
    const shell = await unfolded();

    await fireEvent.click(within(overview()).getByRole('button', { name: /^Remove Old Plan \(/ }));
    let found = await dialog('Remove deleted notes');
    expect(
      within(found).getByText(
        'Remove 1 deleted note for good? They cannot be restored afterwards.',
      ),
    ).toBeTruthy();
    await fireEvent.click(within(found).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(shell.status).toBe('Removed 1 deleted note for good'));

    await fireEvent.click(
      within(overview()).getByRole('button', { name: 'Empty the deleted notes' }),
    );
    found = await dialog('Remove deleted notes');
    expect(
      within(found).getByText(
        'Remove 2 deleted notes for good? They cannot be restored afterwards.',
      ),
    ).toBeTruthy();
    await fireEvent.click(within(found).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(shell.status).toBe('Removed 2 deleted notes for good'));
    expect(within(overview()).queryByRole('button', { name: /Deleted notes/ })).toBeNull();
  });
});

describe('review F8', () => {
  it('M-14.2 the title typed key by key keeps every key (app-tester HIGH)', async () => {
    await workbench();
    await fireEvent.keyDown(document.body, { key: 'F2' });
    const found = await dialog('Rename note');
    const title = within(found).getByLabelText('Title:');
    await waitFor(() => expect(title).toHaveFocus());

    await typeKeys(title, 'Start page');
    await new Promise((done) => setTimeout(done, 250));
    await typeKeys(title, 's');

    expect(title).toHaveValue('Start pages');
    await waitFor(() => expect(within(found).getByText('File: Start-pages.md')).toBeTruthy());
  });

  it('M-14.1 a problem goes when the title changes', async () => {
    await workbench();
    await fireEvent.click(within(overview()).getByRole('button', { name: 'New note…' }));
    const found = await dialog('New note');
    const title = within(found).getByLabelText('Title:');
    await typeKeys(title, 'Ideas');
    await fireEvent.click(within(found).getByRole('button', { name: 'Create' }));
    await within(found).findByRole('alert');

    await typeKeys(title, '2');

    expect(within(found).queryByRole('alert')).toBeNull();
    expect(within(found).queryByRole('button', { name: 'Open the existing note' })).toBeNull();
  });

  it('M-14.2 undo after a rename shows in the facade (app-tester MEDIUM)', async () => {
    const shell = await workbench();
    await shell.openNote('Ideas');
    await waitFor(() => expect(document.querySelector('.facade-text [data-b="0"]')).toBeTruthy());
    facadeInput('insertText', '!');
    await waitFor(() => expect(mockHooks().mirrorText('Ideas')).toContain('!'));
    await fireEvent.keyDown(document.body, { key: 'F2' });
    const found = await dialog('Rename note');
    await typeKeys(within(found).getByLabelText('Title:'), 'Big Ideas');
    await fireEvent.click(within(found).getByRole('button', { name: 'Rename' }));
    await waitFor(() => expect(shell.tabs.active).toBe('Big-Ideas'));
    await waitFor(() => expect(document.querySelector('.facade-text [data-b="0"]')).toBeTruthy());

    facadeInput('historyUndo');

    await waitFor(() => expect(mockHooks().mirrorText('Big-Ideas')).not.toContain('!'));
    await waitFor(() =>
      expect(document.querySelector('.facade-text')!.textContent).not.toContain('!'),
    );
  });

  it('M-14.2 a tab whose links a rename changed shows no mark (app-tester)', async () => {
    const shell = await workbench({ givenDir: '/home/user/Work' });
    await shell.openNote('Link-Tour');
    await waitFor(() => expect(mockHooks().mirrorText('Link-Tour')).not.toBeNull());
    const labels: string[] = [];
    const watch = setInterval(() => labels.push(...shell.tabs.tabs.map((t) => t.label)), 10);

    await shell.startRename('Projects/Roadmap');
    shell.editNoteDialog('rename', shell.notes.rename!, { title: 'Roadmap 2' });
    await shell.renameNote();
    await waitFor(() => expect(mockHooks().mirrorText('Link-Tour')).toContain('Roadmap-2.md'));
    await new Promise((done) => setTimeout(done, 1300));
    clearInterval(watch);

    expect(labels.filter((label) => label.includes('•'))).toEqual([]);
    expect(shell.status).toBe('Renamed Roadmap to Roadmap 2; updated 1 link in 1 note');
  });

  it('Escape from Rename opened by the overview menu goes back to the entry', async () => {
    await workbench();
    await fireEvent.contextMenu(entry('Reading List'), { clientX: 20, clientY: 20 });
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Rename…' }));
    const found = await dialog('Rename note');
    await waitFor(() => expect(within(found).getByLabelText('Title:')).toHaveFocus());

    await fireEvent.keyDown(within(found).getByLabelText('Title:'), { key: 'Escape' });

    await waitFor(() => expect(entry('Reading List')).toHaveFocus());
  });

  it('UX: Restore and Remove leave the keyboard on the next deleted note; Undo on the entry', async () => {
    const shell = await workbench();
    await fireEvent.click(within(overview()).getByRole('button', { name: 'Deleted notes (3)' }));
    const restoreOf = (name: RegExp) => within(overview()).getByRole('button', { name });

    expect(restoreOf(/^Restore Old Plan \(top folder\)$/)).toBeTruthy();
    restoreOf(/^Restore Old Plan/).focus();
    await fireEvent.click(restoreOf(/^Restore Old Plan/));
    await waitFor(() => expect(restoreOf(/^Restore Draft \(guide\)$/)).toHaveFocus());

    await fireEvent.click(within(overview()).getByRole('button', { name: /^Remove Draft/ }));
    const found = await dialog('Remove deleted notes');
    await fireEvent.click(within(found).getByRole('button', { name: 'Remove' }));
    await waitFor(() => expect(restoreOf(/^Restore Ideas/)).toHaveFocus());

    await shell.startDelete('Reading-List');
    await fireEvent.click(
      within(await dialog('Delete note')).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() =>
      expect(within(statusBar()).getByRole('status')).toHaveTextContent(
        'Deleted Reading List. Undo available',
      ),
    );
    await fireEvent.click(within(statusBar()).getByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(entry('Reading List')).toHaveFocus());
  });

  it('UX: a held Enter does not remove right away; the count stays as asked', async () => {
    const shell = await workbench();
    await fireEvent.click(within(overview()).getByRole('button', { name: 'Deleted notes (3)' }));
    await fireEvent.click(
      within(overview()).getByRole('button', { name: 'Empty the deleted notes' }),
    );
    const found = await dialog('Remove deleted notes');
    const remove = within(found).getByRole('button', { name: 'Remove' });

    const repeated = await fireEvent.keyDown(remove, { key: 'Enter', repeat: true });

    expect(repeated).toBe(false);
    await shell.startDelete('Ideas');
    await fireEvent.click(
      within(await dialog('Delete note')).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() => expect(shell.deleted).toHaveLength(4));
    expect(
      within(found).getByText(
        'Remove 3 deleted notes for good? They cannot be restored afterwards.',
      ),
    ).toBeTruthy();
  });
});
