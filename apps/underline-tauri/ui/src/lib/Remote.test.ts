// Feature F13: branches, syncs and conflicts (UC-013 to UC-015; inventory
// M-04.1 to M-04.3, M-19.6 to M-19.10) against the mock backend, whose
// `/home/user/Team` is a fake git repository with a remote (branches main
// and drafts, origin/review on the remote) and `/home/user/Work` one
// without.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHooks, TEAM, type MockStart } from './mock-backend';
import { mockBranchName, mockConflictPlaces, mockResolvePlace } from './mock-remote';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const TEAM_START: MockStart = { givenDir: TEAM };
const WORK: MockStart = { givenDir: '/home/user/Work' };

async function workbench(start: MockStart = TEAM_START) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const bar = () => screen.getByRole('contentinfo');
const tools = () => screen.getByRole('complementary', { name: 'Tools' });
const menu = () => screen.findByRole('dialog', { name: 'Branches' });

/** Waits until the first sync of Team ended. */
async function synced() {
  await waitFor(() => expect(within(bar()).getByText('Synced just now')).toBeInTheDocument(), {
    timeout: 3000,
  });
}

describe('M-04.1, M-04.3 status bar (UC-011 step 1, UC-014 BR-004)', () => {
  it('shows the branch, syncs when the workspace opens, and offers Sync now', async () => {
    await workbench();
    await synced();

    const branch = within(bar()).getByRole('button', { name: 'Branch main' });
    expect(branch).toHaveAttribute('title', 'Branches  Ctrl+Shift+B');
    const now = within(bar()).getByRole('button', { name: 'Sync now' });
    expect(now).toBeEnabled();
    expect(mockHooks().syncs()).toBe(1);

    await fireEvent.click(now);
    await waitFor(() => expect(mockHooks().syncs()).toBe(2));
    await synced();
  });

  it('a workspace without a remote shows the branch and its changes, no sync', async () => {
    await workbench(WORK);

    await waitFor(() =>
      expect(within(bar()).getByRole('button', { name: 'Branch main · 4 changes' })).toBeTruthy(),
    );
    expect(within(bar()).queryByRole('button', { name: 'Sync now' })).toBeNull();
  });

  it('a failed sync is a warning with the reason at the pointer', async () => {
    const shell = await workbench();
    await synced();
    mockHooks().syncOutcome('failed');

    await fireEvent.click(within(bar()).getByRole('button', { name: 'Sync now' }));

    const failed = await within(bar()).findByText('Sync failed', { exact: false });
    expect(failed.closest('[title]')).toHaveAttribute(
      'title',
      'remote: error: GH006: Protected branch update failed for refs/heads/main.',
    );
    expect(failed.textContent).toContain('⚠');
    expect(shell.gitBar.sync?.warning).toBe(true);
  });

  it('a failed sign-in says how to set one up (A5)', async () => {
    const shell = await workbench();
    await synced();
    mockHooks().syncOutcome('signIn');

    await fireEvent.click(within(bar()).getByRole('button', { name: 'Sync now' }));

    await waitFor(() => expect(shell.status).toMatch(/^Signing in to the remote failed \(/));
    expect(within(bar()).getByText('Sync failed: signing in', { exact: false })).toBeTruthy();
  });

  it('UC-011 A9: when git cannot answer, "Changes cannot be shown"', async () => {
    await workbench();
    await synced();

    await mockHooks().gitProblem('fatal: not a git repository');

    await waitFor(() =>
      expect(
        within(bar()).getByText('Changes cannot be shown: fatal: not a git repository'),
      ).toBeTruthy(),
    );
    expect(within(bar()).queryByRole('button', { name: /^Branch/ })).toBeNull();
  });
});

describe('M-19.6, M-19.7 branch menu (UC-013)', () => {
  it('Ctrl+Shift+B lists the branches; Down and Enter switch', async () => {
    const shell = await workbench();
    await synced();

    await fireEvent.keyDown(document.body, { key: 'B', ctrlKey: true, shiftKey: true });
    const branches = await menu();
    const filter = within(branches).getByRole('combobox', { name: 'Filter branches' });
    await waitFor(() => expect(filter).toHaveFocus());
    const options = await within(branches).findAllByRole('option');
    expect(options.map((o) => o.textContent?.trim())).toEqual([
      'drafts',
      'main  (current)',
      'origin/review  (on the remote)',
    ]);
    expect(options[0]).toHaveAttribute('aria-selected', 'true');

    await fireEvent.input(filter, { target: { value: 'DRA' } });
    expect(within(branches).getAllByRole('option')).toHaveLength(1);
    await fireEvent.keyDown(filter, { key: 'Enter' });

    await waitFor(() => expect(shell.status).toBe('Switched to drafts'));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Branches' })).toBeNull());
    // The branch holds Sketch.
    await waitFor(() =>
      expect(within(screen.getByRole('complementary', { name: 'Notes' })).getByText('Sketch')),
    );
    await shell.openNote('Sketch');

    await fireEvent.click(within(bar()).getByRole('button', { name: /^Branch drafts/ }));
    const again = await menu();
    const main = await within(again).findByRole('option', { name: 'main' });
    await fireEvent.click(main);

    await waitFor(() =>
      expect(shell.status).toBe('Switched to main; closed Sketch, not on this branch'),
    );
    expect(shell.tabs.tabs.some((tab) => tab.note === 'Sketch')).toBe(false);
  });

  it('a remote branch becomes a local one', async () => {
    const shell = await workbench();
    await synced();
    await shell.openBranchMenu();
    const branches = await menu();

    await fireEvent.click(
      await within(branches).findByRole('option', { name: /^origin\/review\s+\(on the remote\)$/ }),
    );

    await waitFor(() => expect(shell.status).toBe('Switched to review'));
  });

  it('New branch… shows the name git makes of it, and a taken name offers the branch', async () => {
    const shell = await workbench();
    await synced();
    await shell.openBranchMenu();
    const branches = await menu();
    await fireEvent.input(within(branches).getByRole('combobox'), {
      target: { value: 'trip ideas' },
    });

    await fireEvent.click(within(branches).getByRole('button', { name: 'New branch…' }));
    const name = await within(branches).findByRole('textbox', { name: 'Name of the new branch' });
    expect(name).toHaveValue('trip ideas');
    expect(
      within(branches).getByText(
        'New branch, from the current commit with the current changes; it goes to the remote with the next synchronization.',
      ),
    ).toBeTruthy();
    await within(branches).findByText('Branch: trip-ideas');

    await fireEvent.input(name, { target: { value: 'a..b' } });
    await within(branches).findByText('a branch name cannot hold `..`.', { exact: false });
    expect(within(branches).getByRole('button', { name: 'Create' })).toBeDisabled();

    await fireEvent.input(name, { target: { value: 'main' } });
    await within(branches).findByText('Branch: main');
    await fireEvent.keyDown(name, { key: 'Enter' });

    await within(branches).findByText(
      'The branch main exists; choose it in the list to switch to it.',
    );
    expect(within(branches).getByRole('combobox')).toHaveValue('main');
    expect(shell.branchMenu?.naming).toBeNull();
  });

  it('A3: changes in the way keep the menu open; "Commit first" shows the changes', async () => {
    const shell = await workbench();
    await synced();
    mockHooks().gitBranchInTheWay(['Plan.md']);
    await shell.openBranchMenu();
    const branches = await menu();

    await fireEvent.click(await within(branches).findByRole('option', { name: 'drafts' }));

    await within(branches).findByText(
      'Changes not committed would be overwritten: Plan.md. Commit them first, or stay.',
    );
    const asked = shell.changesAsked;
    await fireEvent.click(within(branches).getByRole('button', { name: 'Commit first' }));
    expect(screen.queryByRole('dialog', { name: 'Branches' })).toBeNull();
    expect(shell.changesAsked).toBe(asked + 1);
  });

  it('Escape closes the menu (A8)', async () => {
    await workbench();
    await synced();
    await fireEvent.keyDown(document.body, { key: 'b', ctrlKey: true, shiftKey: true });
    const branches = await menu();

    await fireEvent.keyDown(within(branches).getByRole('combobox'), { key: 'Escape' });

    expect(screen.queryByRole('dialog', { name: 'Branches' })).toBeNull();
  });

  it('Pull and Push sync on demand (UC-014 A1)', async () => {
    const shell = await workbench();
    await synced();
    await shell.openBranchMenu();
    const branches = await menu();

    await fireEvent.click(within(branches).getByRole('button', { name: 'Pull' }));

    expect(screen.queryByRole('dialog', { name: 'Branches' })).toBeNull();
    await waitFor(() => expect(mockHooks().syncs()).toBe(2));
  });
});

/** Opens Meetings, makes the next sync meet conflicts and runs it. */
async function conflicts() {
  const shell = await workbench();
  await synced();
  await shell.openNote('Meetings');
  mockHooks().syncOutcome('conflict');
  await fireEvent.click(within(bar()).getByRole('button', { name: 'Sync now' }));
  await waitFor(() => expect(within(bar()).getByText('4 conflicts')).toBeTruthy(), {
    timeout: 3000,
  });
  await waitFor(() => expect(shell.status).toMatch(/^The sync stopped: /));
  mockHooks().syncOutcome('synced');
  return shell;
}

describe('M-04.2, M-19.9, M-19.10 conflicts (UC-015)', () => {
  it('the conflict bar shows the place; Alt+H keeps here', async () => {
    const shell = await conflicts();

    const section = await screen.findByRole('region', { name: 'Conflict' });
    expect(within(section).getByText('Conflict: place 1 of 1 open (line 3)')).toBeTruthy();
    expect(
      within(section).getByRole('region', { name: 'Here (your version of the place)' }),
    ).toHaveTextContent('This note lives in the mock backend.');
    expect(
      within(section).getByRole('region', { name: 'Remote (the version from the remote)' }),
    ).toHaveTextContent('The remote changed this line.');
    expect(within(section).getByText('Or edit the place in the note.')).toBeTruthy();
    expect(within(bar()).getByRole('button', { name: 'Sync now' })).toBeDisabled();
    await waitFor(() =>
      expect(shell.status).toBe(
        'The sync stopped: .todos/3.yaml, Meetings.md, Plan.md, assets/team.png changed here and on the remote in the same lines',
      ),
    );

    await fireEvent.keyDown(document.body, { key: 'h', code: 'KeyH', altKey: true });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Conflict' })).toBeNull());
    expect(mockHooks().mirrorText('Meetings')).toContain('This note lives in the mock backend.');
    expect(mockHooks().mirrorText('Meetings')).not.toContain('<<<<<<<');
    await waitFor(() => expect(within(bar()).getByText('3 conflicts')).toBeTruthy());
  });

  it('the other conflicts resolve from the changes and the todos; then the sync goes on', async () => {
    const shell = await conflicts();
    await shell.chooseConflict('remote', true);
    await waitFor(() => expect(within(bar()).getByText('3 conflicts')).toBeTruthy());
    shell.showChanges();
    const list = await within(tools()).findByRole('list', { name: 'Changes' });

    expect(within(list).getByText('deleted on the remote')).toBeTruthy();
    await fireEvent.click(
      within(list).getByRole('button', { name: 'Keep assets/team.png as here' }),
    );
    await waitFor(() =>
      expect(shell.status).toBe(
        'Kept one version of assets/team.png; the other is assets/team (remote).png',
      ),
    );
    await fireEvent.click(within(list).getByRole('button', { name: 'Delete Plan' }));
    await waitFor(() => expect(within(bar()).getByText('1 conflict')).toBeTruthy());

    const todo = within(tools()).getByRole('region', {
      name: 'Todo #3 was changed here and on the remote',
    });
    expect(within(todo).getByRole('row', { name: /^Urgency.*differs.*Normal High$/ })).toHaveClass(
      'differ',
    );
    await fireEvent.click(within(todo).getByRole('button', { name: 'Keep mine for todo #3' }));

    await waitFor(() => expect(shell.status).toBe('The conflicts are resolved; syncing'));
    await synced();
    expect(mockHooks().gitLog()[0]).toBe('Merge remote changes into main');
    expect(shell.deleted.some((d) => d.title === 'Plan')).toBe(true);
  });

  it('A5: "Abort the sync" asks first and puts the note back', async () => {
    const shell = await conflicts();

    await fireEvent.click(within(bar()).getByRole('button', { name: 'Abort the sync' }));
    const asked = await screen.findByRole('dialog', { name: 'Abort the sync' });
    expect(asked).toHaveTextContent(
      'The workspace goes back to how it was before the sync; places you resolved are lost. Your commits and the changes you left out stay.',
    );
    await waitFor(() =>
      expect(within(asked).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
    );

    await fireEvent.click(within(asked).getByRole('button', { name: 'Abort the sync' }));

    await waitFor(() =>
      expect(shell.status).toBe('The sync was aborted; the workspace is as before it'),
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Abort the sync' })).toBeNull(),
    );
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Conflict' })).toBeNull());
    expect(mockHooks().mirrorText('Meetings')).not.toContain('<<<<<<<');
  });

  it('A7: the branch menu offers no switch while conflicts wait', async () => {
    const shell = await conflicts();
    await shell.openBranchMenu();
    const branches = await menu();

    await within(branches).findByText(
      'A synchronization left notes in conflict; resolve them before switching branches.',
    );
    expect(within(branches).getByRole('button', { name: 'New branch…' })).toBeDisabled();
    expect(within(branches).getByRole('button', { name: 'Pull' })).toBeDisabled();
    await fireEvent.click(within(branches).getByRole('option', { name: 'drafts' }));
    expect(screen.getByRole('dialog', { name: 'Branches' })).toBeTruthy();
    expect(shell.status).not.toMatch(/^Switched/);
  });
});

describe('review F13', () => {
  it('Ctrl+Shift+B opens nothing while git cannot answer (A-143)', async () => {
    const shell = await workbench();
    await synced();
    await mockHooks().gitProblem('fatal: not a git repository');
    await waitFor(() => expect(shell.changes?.problem).toBe('fatal: not a git repository'));

    await fireEvent.keyDown(document.body, { key: 'B', ctrlKey: true, shiftKey: true });

    expect(screen.queryByRole('dialog', { name: 'Branches' })).toBeNull();
    expect(shell.branchMenu).toBeNull();
  });

  it('the conflict keys do nothing while a dialog is open', async () => {
    await conflicts();
    await fireEvent.click(within(bar()).getByRole('button', { name: 'Abort the sync' }));
    await screen.findByRole('dialog', { name: 'Abort the sync' });

    await fireEvent.keyDown(document.body, { key: 'h', code: 'KeyH', altKey: true });

    expect(mockHooks().mirrorText('Meetings')).toContain('<<<<<<<');
  });

  it('a note in conflict is not renamed, deleted or exported (UC-015 BR-007)', async () => {
    const shell = await conflicts();

    await shell.startRename('Meetings');

    await waitFor(() =>
      expect(shell.status).toBe('Meetings is in conflict; resolve it before renaming it'),
    );
    expect(shell.notes.rename).toBeNull();
  });

  it('exporting a note in conflict through the overview is refused too', async () => {
    const shell = await conflicts();

    await shell.startExport('Meetings', 'html');

    await waitFor(() =>
      expect(shell.status).toBe('Meetings is in conflict; resolve it before exporting it'),
    );
    expect(mockHooks().exportedFiles()).toEqual(
      expect.not.objectContaining({ '/home/user/Team/Meetings.html': expect.anything() }),
    );
  });

  it('"All: Here" names what it does for screen readers', async () => {
    await conflicts();
    const section = await screen.findByRole('region', { name: 'Conflict' });

    expect(
      within(section).getByRole('button', { name: 'Resolve all places with Here' }),
    ).toHaveTextContent('All: Here');
  });
});

describe('mock stand-ins (F13)', () => {
  it('branch names and conflict places look plausible', () => {
    expect(mockBranchName(' trip  ideas ')).toEqual({ text: 'Branch: trip-ideas', ok: true });
    expect(mockBranchName('x.lock').ok).toBe(false);
    const text = 'one\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> abc\nthree\n';
    const [place] = mockConflictPlaces(text);
    expect(place).toMatchObject({ line: 2, here: 'mine\n', remote: 'theirs\n' });
    expect(mockResolvePlace(place, 'both')).toBe('mine\ntheirs\n');
  });
});
