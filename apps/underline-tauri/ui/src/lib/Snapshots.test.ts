// Package 8a (FR-101, FR-102): the snapshots of a note (UC-026,
// TC-026-01, TC-026-05 to TC-026-08, TC-026-10, TC-026-14), wired to the
// mock backend. When snapshots are taken, kept and restored is tested in
// Rust (`underline-core` `snapshots/tests.rs`, `session/snapshots/tests.rs`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHooks, MOCK_NO_SNAPSHOT, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
/** 2026-11-11 09:30 in the local time zone. */
const AT_0930 = new Date(2026, 10, 11, 9, 30, 0).getTime();

async function workbench(start: MockStart = NOTES) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const dialog = () => screen.findByRole('dialog', { name: /^Snapshots of / });
const statusBar = () => document.querySelector<HTMLElement>('.status-bar')!;

describe('UC-026 Restore Snapshot', () => {
  it('TC-026-01, TC-026-07: lists, compares, restores and takes the restore back', async () => {
    const shell = await workbench();
    await shell.openNote('Trip');
    const before = mockHooks().noteText('Trip');
    mockHooks().addSnapshot('Trip', 'Version A\n', 'Editing', AT_0930);

    shell.commandById('note.snapshots')!.run();

    const found = await dialog();
    expect(found).toHaveAccessibleName('Snapshots of Trip');
    const list = await within(found).findByRole('listbox', { name: 'Snapshots, the newest first' });
    const option = await within(list).findByRole('option', { selected: true });
    expect(option.textContent).toContain('2026-11-11 09:30:00');
    expect(option.textContent).toContain('Before editing');
    expect(option.textContent).toMatch(/\d+ lines? differs?/);
    // Step 4: the snapshot's line taken away, the note's added; nothing changes.
    await waitFor(() => expect(within(found).getByText('Version A')).toBeTruthy());
    expect(found.querySelector('.line[data-kind="removed"]')?.textContent).toContain('Version A');
    expect(mockHooks().noteText('Trip')).toBe(before);

    await fireEvent.click(within(found).getByRole('button', { name: 'Restore' }));

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: /^Snapshots of / })).toBeNull(),
    );
    await waitFor(() =>
      expect(shell.status).toBe('Restored Trip to the snapshot of 2026-11-11 09:30:00'),
    );
    expect(mockHooks().noteText('Trip')).toBe('Version A\n');
    expect(mockHooks().snapshots('Trip')[0]).toEqual({ cause: 'Restore', text: before });

    // A3: "Undo" in the status bar.
    await fireEvent.click(within(statusBar()).getByRole('button', { name: 'Undo' }));

    await waitFor(() => expect(shell.status).toBe('Took back the restore of Trip'));
    expect(mockHooks().noteText('Trip')).toBe(before);
  });

  it('TC-026-05: a note without snapshots says when they are taken', async () => {
    const shell = await workbench();

    await shell.showSnapshots('Ideas');

    const found = await dialog();
    expect(within(found).getByText(MOCK_NO_SNAPSHOT)).toBeTruthy();
    expect(within(found).getAllByRole('listitem')).toHaveLength(6);
    expect(within(found).getByText('Kept on this computer for 30 days.')).toBeTruthy();
    await waitFor(() =>
      expect(within(found).getByText('Close', { selector: 'button' })).toHaveFocus(),
    );
    expect(within(found).queryByRole('button', { name: 'Restore' })).toBeNull();
  });

  it('TC-026-06: shows the whole snapshot, read only', async () => {
    const shell = await workbench();
    mockHooks().addSnapshot('Ideas', '# Old ideas\n\nA paragraph kept.\n', 'Git', AT_0930);
    await shell.showSnapshots('Ideas');
    const found = await dialog();
    await within(found).findByRole('option', { selected: true });

    await fireEvent.click(within(found).getByRole('button', { name: 'Whole snapshot' }));

    await waitFor(() => expect(within(found).getByText('Old ideas')).toBeTruthy());
    expect(within(found).getByRole('button', { name: 'Whole snapshot' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(found.querySelector('[contenteditable="true"]')).toBeNull();
  });

  it('TC-026-08: follows a change of the note while it is shown', async () => {
    const shell = await workbench();
    await shell.openNote('Log');
    mockHooks().addSnapshot('Log', 'old log\n', 'Editing', AT_0930);
    await shell.showSnapshots('Log');
    const found = await dialog();
    await within(found).findByRole('option', { selected: true });

    // Another snapshot is taken while the dialog shows (here: by the mock).
    mockHooks().addSnapshot('Log', 'older\n', 'OutsideChange', AT_0930 + 60_000);

    await waitFor(() => expect(within(found).getAllByRole('option')).toHaveLength(2));
    expect(within(found).getAllByRole('option')[0].textContent).toContain(
      'Before an outside change',
    );
  });

  it('TC-026-10, TC-026-14: the keyboard chooses, Escape closes without a change', async () => {
    const shell = await workbench();
    const before = mockHooks().noteText('Log');
    mockHooks().addSnapshot('Log', 'one\n', 'Editing', AT_0930);
    mockHooks().addSnapshot('Log', 'two\n', 'AgentChange', AT_0930 + 60_000);
    await shell.showSnapshots('Log');
    const found = await dialog();
    const list = await within(found).findByRole('listbox');
    await waitFor(() => expect(list).toHaveFocus());
    expect(within(list).getByRole('option', { selected: true }).textContent).toContain(
      "Before an agent's change",
    );

    await fireEvent.keyDown(list, { key: 'ArrowDown' });

    await waitFor(() =>
      expect(within(list).getByRole('option', { selected: true }).textContent).toContain(
        'Before editing',
      ),
    );
    await waitFor(() => expect(within(found).getByText('one')).toBeTruthy());
    await fireEvent.keyDown(list, { key: 'Escape' });
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: /^Snapshots of / })).toBeNull(),
    );
    expect(mockHooks().noteText('Log')).toBe(before);
    expect(mockHooks().snapshots('Log')).toHaveLength(2);
  });

  it('UC-026 step 1: the context menus of the overview and the tab offer "Snapshots…"', async () => {
    const shell = await workbench();
    await shell.openNote('Trip');
    const tabBar = document.querySelector<HTMLElement>('.tab-bar')!;
    await fireEvent.contextMenu(within(tabBar).getByRole('tab', { name: /Trip/ }), {
      clientX: 40,
      clientY: 40,
    });

    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Snapshots…' }));

    expect(await dialog()).toHaveAccessibleName('Snapshots of Trip');
    await shell.snapshots.close();

    const overview = document.querySelector<HTMLElement>('.overview')!;
    await fireEvent.contextMenu(within(overview).getByRole('button', { name: 'Ideas' }), {
      clientX: 20,
      clientY: 20,
    });
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Snapshots…' }));

    expect(await dialog()).toHaveAccessibleName('Snapshots of Ideas');
  });

  it('UX review: "Restore" says why it cannot be used, and what restoring keeps', async () => {
    const shell = await workbench();
    const now = mockHooks().noteText('Log');
    mockHooks().addSnapshot('Log', 'older\n', 'Editing', AT_0930);
    mockHooks().addSnapshot('Log', now, 'Git', AT_0930 + 60_000);
    await shell.showSnapshots('Log');
    const found = await dialog();
    const restore = within(found).getByRole('button', { name: 'Restore' });

    await waitFor(() => expect(restore).toBeDisabled());
    expect(within(found).getByText('The note holds this snapshot already.')).toBeTruthy();
    expect(within(found).getByText('Same as the note')).toBeTruthy();
    expect(within(found).getByText('Kept on this computer for 30 days')).toBeTruthy();

    await fireEvent.keyDown(within(found).getByRole('listbox'), { key: 'ArrowDown' });

    await waitFor(() => expect(restore).not.toBeDisabled());
    expect(
      within(found).getByText(
        'Restoring keeps the current text as a snapshot; you can undo it in the status bar.',
      ),
    ).toBeTruthy();
    expect(within(found).getByText('Restore brings back')).toBeTruthy();
  });

  it('TC-026-03, TC-026-12: outside and agent changes take snapshots; they follow a rename', async () => {
    const shell = await workbench();
    await mockHooks().agentWrite(true);
    await mockHooks().agentConnect('Claude');
    await shell.openNote('Log');
    const first = mockHooks().noteText('Log');
    await mockHooks().outsideChange('Log', 'from outside\n');
    await mockHooks().agentChange('Claude', 'Log', 'from outside', 'from the agent');
    expect(mockHooks().snapshots('Log')).toEqual([
      { cause: 'AgentChange', text: 'from outside\n' },
      { cause: 'OutsideChange', text: first },
    ]);
    await shell.startRename('Log');
    const found = await screen.findByRole('dialog', { name: 'Rename note' });
    await fireEvent.input(within(found).getByLabelText('Title:'), {
      target: { value: 'Journal Log' },
    });
    await fireEvent.submit(within(found).getByLabelText('Title:'));

    await waitFor(() => expect(mockHooks().snapshots('Journal-Log')).toHaveLength(2));
    expect(mockHooks().snapshots('Log')).toHaveLength(0);
    // The dialog's last plan of the new place is not asked for after the
    // test (it would reach no backend).
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Rename note' })).toBeNull());
  });

  it('TC-026-08: the counts of the list follow a change of the note, also one with no new snapshot', async () => {
    const shell = await workbench();
    await shell.openNote('Log');
    const now = mockHooks().noteText('Log');
    mockHooks().addSnapshot('Log', now, 'Before editing', AT_0930);
    await shell.showSnapshots('Log');
    const found = await dialog();
    await waitFor(() => expect(within(found).getByText('Same as the note')).toBeTruthy());

    // The snapshot taken holds the newest's text, so none is added.
    await mockHooks().outsideChange('Log', 'changed outside\n');

    await waitFor(() => expect(within(found).queryByText('Same as the note')).toBeNull());
    expect(within(found).getAllByRole('option')).toHaveLength(1);
    expect(() => mockHooks().addSnapshot('Log', 'x', 'Nonsense')).toThrow(/No snapshot cause/);
  });

  it('UC-007, TC-026-12: deleted and restored from the deleted notes, the snapshots come back', async () => {
    const shell = await workbench();
    mockHooks().addSnapshot('Ideas', 'one\n', 'Editing', AT_0930);
    await shell.startDelete('Ideas');
    const found = await screen.findByRole('dialog', { name: 'Delete note' });
    await fireEvent.click(within(found).getByRole('button', { name: 'Delete' }));
    await waitFor(() => expect(shell.status).toBe('Deleted Ideas'));
    expect(mockHooks().snapshots('Ideas')).toHaveLength(0);
    const deleted = shell.deleted.find((d) => d.title === 'Ideas')!;

    await shell.restoreDeleted(deleted.key);

    await waitFor(() => expect(mockHooks().snapshots('Ideas')).toHaveLength(1));
  });
});
