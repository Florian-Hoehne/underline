// Feature F12: the git changes (UC-011, UC-012; inventory M-19.1 to M-19.5,
// M-05.3, M-16.4) against the mock backend, whose `/home/user/Work` is a
// fake git repository: README changed, Projects/Roadmap new, Old-Minutes
// deleted, assets/logo.png new (no text).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHooks, type MockStart } from './mock-backend';
import { mockChangedLines, mockDiff, mockProposedMessage } from './mock-git';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const WORK: MockStart = { givenDir: '/home/user/Work' };
const NOTES: MockStart = { givenDir: '/home/user/Notes' };

async function workbench(start: MockStart = WORK) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const tools = () => screen.getByRole('complementary', { name: 'Tools' });
const overview = () => screen.getByRole('complementary', { name: 'Notes' });
const changesList = () => within(tools()).getByRole('list', { name: 'Changes' });
const message = () => within(tools()).getByRole('textbox', { name: 'Commit message' });
const dialog = (name: string) => screen.findByRole('dialog', { name });
const commitButton = () =>
  within(tools()).getByRole('button', { name: /^(Commit|Amend)$/ }) as HTMLButtonElement;

/** Waits until the tool panel lists the changes. */
async function changesShown() {
  await waitFor(() => expect(within(tools()).getByText('Changes (4)')).toBeInTheDocument());
}

describe('M-19.2 Changes (N) (UC-011 steps 1–4, BR-002, BR-003, BR-007)', () => {
  it('lists notes, then "Other files", with their letters; folded at first', async () => {
    await workbench();
    await changesShown();

    const summary = within(tools()).getByText('Changes (4)');
    expect(summary.closest('details')).not.toHaveAttribute('open');
    const rows = within(changesList())
      .getAllByRole('listitem')
      .map((row) => row.textContent?.replace(/\s+/g, ' ').trim());
    expect(rows).toEqual([
      'D Old Minutes, deleted Discard…',
      'N Roadmap · Projects, new Discard…',
      'M README, changed Discard…',
      'Other files',
      'N assets/logo.png, new',
    ]);
    // BR-004: every change is chosen.
    const boxes = within(changesList()).getAllByRole('checkbox');
    expect(boxes.every((box) => (box as HTMLInputElement).checked)).toBe(true);
    expect(boxes[0]).toHaveAccessibleName('Commit Old Minutes');
  });

  it('Ctrl+Shift+G opens the list and puts the keyboard into the message', async () => {
    await workbench();
    await changesShown();

    await fireEvent.keyDown(document.body, { key: 'G', ctrlKey: true, shiftKey: true });

    await waitFor(() => expect(message()).toHaveFocus());
    expect(within(tools()).getByText('Changes (4)').closest('details')).toHaveAttribute('open');
  });

  it('Up and Down move through the list', async () => {
    await workbench();
    await changesShown();
    const entries = within(changesList()).getAllByRole('button', {
      name: /, (new|changed|deleted)$/,
    });
    entries[0].focus();

    expect(entries.map((e) => e.tabIndex)).toEqual([0, -1, -1, -1]);
    await fireEvent.keyDown(entries[0], { key: 'ArrowDown' });
    expect(entries[1]).toHaveFocus();
    await fireEvent.keyDown(entries[1], { key: 'ArrowUp' });
    expect(entries[0]).toHaveFocus();
    await fireEvent.keyDown(entries[0], { key: 'End' });
    expect(entries[3]).toHaveFocus();
    expect(entries[3]).toHaveAttribute('tabindex', '0');
    await fireEvent.keyDown(entries[3], { key: 'Home' });
    expect(entries[0]).toHaveFocus();
  });

  it('M-05.3: the overview marks changed notes with their letter', async () => {
    await workbench();
    await changesShown();

    const readme = within(overview()).getByRole('button', { name: 'README, changed' });
    expect(readme.querySelector('.git-letter')).toHaveTextContent('M');
    expect(within(overview()).getByRole('button', { name: 'Roadmap, new' })).toBeTruthy();
    expect(within(overview()).getByRole('button', { name: 'Meetings' })).toBeTruthy();
  });

  it('M-16.4: a workspace without git shows no changes', async () => {
    const shell = await workbench(NOTES);
    await waitFor(() => expect(shell.tabs.active).toBe('Home'));

    expect(within(tools()).queryByText(/^Changes/)).toBeNull();
    expect(within(tools()).queryByText('Diff')).toBeNull();
    expect(within(overview()).queryByText('M')).toBeNull();
  });
});

describe('M-19.1 commit box (UC-012 steps 2–3, BR-002 to BR-004, A6)', () => {
  it('the message follows the choice until the user types', async () => {
    const shell = await workbench();
    await changesShown();
    await waitFor(() =>
      expect(message()).toHaveValue(
        'Update README; Add Roadmap; Delete Old Minutes and 1 other file',
      ),
    );

    await fireEvent.click(within(changesList()).getByRole('checkbox', { name: 'Commit README' }));
    await waitFor(() =>
      expect(message()).toHaveValue('Add Roadmap; Delete Old Minutes and 1 other file'),
    );

    await fireEvent.input(message(), { target: { value: 'My own words' } });
    await fireEvent.click(within(changesList()).getByRole('checkbox', { name: 'Commit README' }));
    await waitFor(() => expect(shell.changes?.files.every((f) => f.chosen)).toBe(true));
    expect(message()).toHaveValue('My own words');
  });

  it('"None" chooses nothing: "Nothing is chosen." and no commit; "All" again', async () => {
    await workbench();
    await changesShown();

    await fireEvent.click(within(tools()).getByRole('button', { name: 'Choose no change' }));

    await waitFor(() => expect(within(tools()).getByText('Nothing is chosen.')).toBeTruthy());
    expect(commitButton()).toBeDisabled();
    await fireEvent.click(within(tools()).getByRole('button', { name: 'Choose all changes' }));
    await waitFor(() => expect(within(tools()).queryByText('Nothing is chosen.')).toBeNull());
    expect(commitButton()).toBeEnabled();
  });

  it('BR-003: a first line over 72 characters is pointed out', async () => {
    await workbench();
    await changesShown();

    await fireEvent.input(message(), { target: { value: 'x'.repeat(73) + '\nmore' } });

    expect(
      within(tools()).getByText('The first line is long; many programs show only its start.'),
    ).toBeTruthy();
  });

  it('A6: "Amend the last commit" takes over its message; the button says "Amend"', async () => {
    await workbench();
    await changesShown();

    await fireEvent.click(within(tools()).getByRole('checkbox', { name: 'Amend the last commit' }));

    expect(message()).toHaveValue('Add the meeting notes');
    expect(commitButton()).toHaveTextContent('Amend');
  });

  it('BR-009: during a merge a warning stands in place of the box', async () => {
    await workbench();
    await changesShown();

    await mockHooks().gitMerging(true);

    await waitFor(() =>
      expect(
        within(tools()).getByText(
          'A synchronization left notes in conflict; resolve them before committing.',
        ),
      ).toBeTruthy(),
    );
    expect(within(tools()).queryByRole('textbox', { name: 'Commit message' })).toBeNull();
    expect(within(changesList()).queryAllByRole('checkbox')).toHaveLength(0);
  });
});

describe('M-19.4 commit (UC-012 steps 4–7, A1 to A4)', () => {
  it('A1: an empty message is refused in the box', async () => {
    await workbench();
    await changesShown();
    await fireEvent.input(message(), { target: { value: '   ' } });

    await fireEvent.click(commitButton());

    expect(await within(tools()).findByRole('alert')).toHaveTextContent(
      'A commit needs a message.',
    );
    expect(mockHooks().gitLog()).toEqual(['Add the meeting notes']);
  });

  it('A3: an unknown user is asked first; then Ctrl+Enter commits and the box empties', async () => {
    const shell = await workbench();
    await changesShown();
    await fireEvent.input(message(), { target: { value: 'Plan the roadmap' } });

    await fireEvent.keyDown(message(), { key: 'Enter', ctrlKey: true });

    const asked = await dialog('Your name for commits');
    const name = within(asked).getByRole('textbox', { name: 'Name' });
    const email = within(asked).getByRole('textbox', { name: 'Email' });
    const confirm = within(asked).getByRole('button', { name: 'Commit' });
    await waitFor(() => expect(name).toHaveFocus());
    expect(confirm).toBeDisabled();
    await fireEvent.input(name, { target: { value: 'Ann' } });
    await fireEvent.input(email, { target: { value: 'ann' } });
    expect(confirm).toBeDisabled();
    await fireEvent.input(email, { target: { value: 'ann@example.com' } });
    await fireEvent.click(confirm);

    await waitFor(() => expect(shell.status).toMatch(/^Committed \w+ holding 3 notes and 1 file$/));
    expect(mockHooks().gitLog()[0]).toBe('Plan the roadmap');
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Your name for commits' })).toBeNull(),
    );
    await waitFor(() => expect(within(tools()).getByText('Changes (0)')).toBeTruthy());
    expect(within(tools()).getByText('Nothing changed since the last commit.')).toBeTruthy();
    // UX review F12: the keyboard goes to "Changes (N)".
    await waitFor(() => expect(within(tools()).getByText('Changes (0)')).toHaveFocus());
  });

  it('A3 step 4: Escape commits nothing and keeps the message', async () => {
    await workbench();
    await changesShown();
    await fireEvent.input(message(), { target: { value: 'Keep me' } });
    await fireEvent.click(commitButton());
    const asked = await dialog('Your name for commits');

    await fireEvent.keyDown(within(asked).getByRole('textbox', { name: 'Name' }), {
      key: 'Escape',
    });

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(message()).toHaveValue('Keep me');
    expect(mockHooks().gitLog()).toHaveLength(1);
  });

  it('A4: a refused commit says why in the status; the message stays', async () => {
    const shell = await workbench();
    await changesShown();
    mockHooks().gitIdentity(true);
    mockHooks().gitCommitFails('the checks failed');
    await fireEvent.input(message(), { target: { value: 'Try' } });

    await fireEvent.click(commitButton());

    await waitFor(() =>
      expect(shell.status).toBe('The commit failed: `git commit` failed: the checks failed'),
    );
    expect(message()).toHaveValue('Try');
  });

  it('A6: amending reports the amended commit', async () => {
    const shell = await workbench();
    await changesShown();
    mockHooks().gitIdentity(true);
    await fireEvent.click(within(tools()).getByRole('checkbox', { name: 'Amend the last commit' }));

    await fireEvent.click(commitButton());

    await waitFor(() => expect(shell.status).toMatch(/^Amended the last commit, now \w+ holding/));
    expect(mockHooks().gitLog()).toEqual(['Add the meeting notes']);
  });
});

describe('M-19.4 Your name for commits (UC-012 A3, review F12)', () => {
  it('says why a field is wrong once left, and stays open when git cannot keep the name', async () => {
    const shell = await workbench();
    await changesShown();
    mockHooks().gitIdentityFails('could not lock config file');
    await fireEvent.input(message(), { target: { value: 'Keep me' } });
    await fireEvent.click(commitButton());
    const asked = await dialog('Your name for commits');
    const name = within(asked).getByRole('textbox', { name: 'Name' });
    const email = within(asked).getByRole('textbox', { name: 'Email' });

    await fireEvent.blur(name);
    expect(within(asked).getByText('Enter your name.')).toBeTruthy();
    expect(name).toHaveAttribute('aria-invalid', 'true');
    await fireEvent.input(name, { target: { value: 'Ann' } });
    await fireEvent.input(email, { target: { value: 'ann' } });
    await fireEvent.blur(email);
    expect(within(asked).getByText('The email needs an @.')).toBeTruthy();
    expect(email).toHaveAccessibleDescription('The email needs an @.');
    await fireEvent.input(email, { target: { value: 'ann@example.com' } });
    await fireEvent.click(within(asked).getByRole('button', { name: 'Commit' }));

    expect(await within(asked).findByRole('alert')).toHaveTextContent(
      'Your name could not be kept: could not lock config file',
    );
    expect(name).toHaveValue('Ann');
    expect(shell.status).toBe('Your name could not be kept: could not lock config file');
    expect(mockHooks().gitLog()).toHaveLength(1);
  });
});

describe('M-19.3 Diff (UC-011 steps 5–8, A2, A4, A5, BR-004)', () => {
  it('shows the active note against the last commit, with numbers and signs', async () => {
    await workbench();
    await changesShown();

    await waitFor(() =>
      expect(within(tools()).getByText('README.md', { selector: 'p' })).toBeTruthy(),
    );
    const lines = [...tools().querySelectorAll('.hunk .line')].map((l) => l.textContent);
    expect(lines).toContain('removed:    4      - Type to try the source view.');
    expect(lines).toContain('added:         4 + Type here to try the source view.');
    // One Tab stop for the diff; Home and End move within it.
    const buttons = [...tools().querySelectorAll<HTMLElement>('button.line')];
    expect(buttons.filter((b) => b.tabIndex === 0)).toHaveLength(1);
    buttons[0].focus();
    await fireEvent.keyDown(buttons[0], { key: 'End' });
    expect(buttons.at(-1)).toHaveFocus();
    expect(buttons.at(-1)).toHaveAttribute('tabindex', '0');
    expect(tools().querySelector('.line[data-kind="added"] .strong')).toHaveTextContent('here');
  });

  it('A4, A5: a deleted note and another file are shown without a tab', async () => {
    const shell = await workbench();
    await changesShown();

    await fireEvent.click(
      within(changesList()).getByRole('button', { name: 'Old Minutes, deleted' }),
    );
    await waitFor(() => expect(within(tools()).getByText('Old-Minutes.md')).toBeTruthy());
    const kinds = [...tools().querySelectorAll('.hunk .line')].map((l) =>
      l.getAttribute('data-kind'),
    );
    expect(new Set(kinds)).toEqual(new Set(['removed']));
    expect(shell.tabs.active).toBe('README');

    await fireEvent.click(
      within(changesList()).getByRole('button', { name: 'assets/logo.png, new' }),
    );
    expect(
      await within(tools()).findByText('This file is not text; it is new.'),
    ).toBeInTheDocument();
  });

  it('A2, step 5: a changed note opens in a tab; an unchanged note says so', async () => {
    const shell = await workbench();
    await changesShown();

    await fireEvent.click(
      within(changesList()).getByRole('button', { name: 'Roadmap · Projects, new' }),
    );
    await waitFor(() => expect(shell.tabs.active).toBe('Projects/Roadmap'));

    await shell.openNote('Meetings');
    expect(
      await within(tools()).findByText('The note did not change since the last commit.'),
    ).toBeTruthy();
  });
});

describe('M-19.5 Discard changes (UC-011 A6, BR-005)', () => {
  it('asks with the lines lost; "Discard" puts the committed text back, also in the tab', async () => {
    const shell = await workbench();
    await changesShown();

    await fireEvent.click(
      within(changesList()).getByRole('button', { name: 'Discard the changes of README…' }),
    );

    const asked = await dialog('Discard changes');
    expect(asked).toHaveTextContent('Discard the changes of README since the last commit?');
    expect(asked).toHaveTextContent('2 changed lines will be lost and cannot be restored.');
    const discard = within(asked).getByRole('button', { name: 'Discard' });
    await waitFor(() => expect(discard).toHaveFocus());
    await fireEvent.click(discard);

    await waitFor(() => expect(shell.status).toBe('Discarded the changes of README'));
    await waitFor(() =>
      expect(mockHooks().mirrorText('README')).toContain('Type to try the source view.'),
    );
    await waitFor(() => expect(within(tools()).getByText('Changes (3)')).toBeTruthy());
  });

  it('A6 step 4: Cancel changes nothing', async () => {
    const shell = await workbench();
    await changesShown();
    await fireEvent.click(
      within(changesList()).getByRole('button', { name: 'Discard the changes of README…' }),
    );
    const asked = await dialog('Discard changes');

    await fireEvent.click(within(asked).getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(shell.status).not.toContain('Discarded');
    expect(within(tools()).getByText('Changes (4)')).toBeTruthy();
  });

  it('a new note goes to the deleted notes', async () => {
    const shell = await workbench();
    await changesShown();
    await fireEvent.click(
      within(changesList()).getByRole('button', {
        name: 'Discard the changes of Roadmap · Projects…',
      }),
    );
    const asked = await dialog('Discard changes');
    expect(asked).toHaveTextContent('The note is new; it goes to the deleted notes.');

    await fireEvent.click(within(asked).getByRole('button', { name: 'Discard' }));

    await waitFor(() => expect(shell.status).toBe('Discarded the changes of Roadmap'));
    expect(shell.deleted.map((d) => d.title)).toContain('Roadmap');
    expect(shell.overview.map((o) => o.note)).not.toContain('Projects/Roadmap');
  });
});

describe('mock-git stand-ins', () => {
  it('diffs lines with three around and counts lines lost', () => {
    const old = 'a\nb\nc\nd\ne\nf\ng\nh\n';
    const hunks = mockDiff(old, old.replace('d\n', 'D\n'));
    expect(hunks).toHaveLength(1);
    expect(hunks[0].lines.map((l) => l.numbers.trimEnd())).toEqual([
      '   1    1',
      '   2    2',
      '   3    3',
      '   4      -',
      '        4 +',
      '   5    5',
      '   6    6',
      '   7    7',
    ]);
    expect(mockChangedLines(old, old.replace('d\n', 'D\n'))).toBe(2);
    expect(mockDiff('a \n', 'a\n')[0].lines[0].only).toBe('  (only spaces differ)');
  });

  it('proposes a message as UC-012 BR-002 says', () => {
    expect(
      mockProposedMessage([
        { kind: 'changed', title: 'Plan', from: null },
        { kind: 'new', title: 'New', from: null },
        { kind: 'renamed', title: 'Final', from: 'Draft' },
        { kind: 'new', title: null, from: null },
        { kind: 'new', title: null, from: null },
      ]),
    ).toBe('Update Plan; Add New; Rename Draft to Final and 2 other files');
  });
});
