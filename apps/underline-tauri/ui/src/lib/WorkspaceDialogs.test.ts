import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import type { MockHandlers, MockStart } from './mock-backend';

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function app(start: MockStart = {}, handlers: Partial<MockHandlers> = {}) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

function dialog(name: string) {
  return screen.getByRole('dialog', { name });
}

/** A tab of the tab bar, by its label. */
function tab(name: string) {
  return within(document.querySelector<HTMLElement>('.tab-bar')!).getByRole('tab', { name });
}

/** A note of the overview, by its label. */
function entry(name: string) {
  return within(screen.getByRole('complementary', { name: 'Notes' })).getByRole('button', { name });
}

describe('M-01.1 start (UC-002 A1, A2, plan section 14)', () => {
  it('shows neither the empty screen nor a workspace while the start opens', async () => {
    let finish: (() => void) | null = null;
    const shell = await connectedShell(
      { givenDir: '/home/user/Notes' },
      {
        ws_start: () =>
          new Promise((resolve) => {
            finish = () =>
              resolve({
                answer: { kind: 'done' },
                snapshot: {
                  workspace: null,
                  overview: [],
                  tabs: { tabs: [], active: null, todoList: { open: false, active: false } },
                },
              });
          }),
      },
    );
    render(App, { props: { shell } });
    const starting = shell.startWorkspace();

    expect(screen.getByRole('main')).toHaveAttribute('aria-busy', 'true');
    expect(screen.queryByRole('heading', { name: 'No workspace is open' })).not.toBeInTheDocument();

    // The command runs once the queue reaches it.
    await waitFor(() => expect(finish).not.toBeNull());
    finish!();
    await starting;
    await waitFor(() =>
      expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument(),
    );
  });

  it('opens the given workspace with its overview and its first note', async () => {
    await app({ givenDir: '/home/user/Notes' });

    expect(screen.getByRole('main')).toHaveAttribute('aria-busy', 'false');
    expect(screen.getByRole('heading', { name: 'Notes' })).toBeInTheDocument();
    expect(tab('Home')).toHaveAttribute('aria-selected', 'true');
    await waitFor(() =>
      expect(screen.getByRole('status').textContent).toBe(
        '/home/user/Notes  |  Opened /home/user/Notes',
      ),
    );
  });

  it('opens the workspace opened last without a given one', async () => {
    await app({ lastWorkspace: '/home/user/Work', recent: ['/home/user/Work'] });

    expect(tab('README')).toHaveAttribute('aria-selected', 'true');
  });

  it('shows the Open dialog with the problem when the given directory is gone', async () => {
    await app({ givenDir: '/home/user/Gone' });

    const open = dialog('Open workspace');
    expect(within(open).getByRole('alert')).toHaveTextContent(
      '/home/user/Gone does not exist. Choose another directory or create a new workspace there.',
    );
    expect(within(open).getByLabelText('Another directory:')).toHaveValue('/home/user/Gone');
    expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument();
  });

  it('shows a failed start in the status bar and the empty screen', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    await app(
      { givenDir: '/home/user/Notes' },
      {
        ws_start: () => {
          throw { kind: 'internal', message: 'The session is gone' };
        },
      },
    );

    expect(screen.getByRole('status')).toHaveTextContent('The session is gone');
    expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument();
  });
});

describe('M-21.1 Open dialog (UC-002)', () => {
  it('M-02.1 lists the recently opened workspaces and opens one', async () => {
    await app({ recent: ['/home/user/Work', '/home/user/Notes'] });

    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const open = await waitFor(() => dialog('Open workspace'));
    const recent = within(open).getByRole('list', { name: 'Recently opened:' });
    expect(
      within(recent)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['/home/user/Work', '/home/user/Notes']);

    await fireEvent.click(within(recent).getByRole('button', { name: '/home/user/Notes' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(screen.getByRole('heading', { name: 'Notes' })).toBeInTheDocument();
  });

  it('without recent workspaces the path field takes the keyboard', async () => {
    await app();

    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));

    await waitFor(() => expect(screen.getByLabelText('Another directory:')).toHaveFocus());
  });

  it('UC-002 A3, A4: a missing directory offers to create a workspace there', async () => {
    await app();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const field = await waitFor(() => screen.getByLabelText('Another directory:'));

    await fireEvent.input(field, { target: { value: '  /home/user/Later  ' } });
    await fireEvent.submit(field.closest('form')!);
    const offer = await waitFor(() =>
      screen.getByRole('button', { name: 'Create a new workspace there' }),
    );
    await fireEvent.click(offer);

    const create = await waitFor(() => dialog('New workspace'));
    expect(within(create).getByText('Directory: /home/user/Later')).toBeInTheDocument();
    expect(screen.queryByRole('dialog', { name: 'Open workspace' })).not.toBeInTheDocument();
  });

  it('UC-002 A5, A6: a directory inside a workspace offers both', async () => {
    await app();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const field = await waitFor(() => screen.getByLabelText('Another directory:'));

    await fireEvent.input(field, { target: { value: '/home/user/Notes/guide' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(
        '/home/user/Notes/guide lies inside the workspace /home/user/Notes.',
      ),
    );
    expect(screen.getByRole('button', { name: 'Open /home/user/Notes' })).toBeInTheDocument();
    await fireEvent.click(
      screen.getByRole('button', { name: 'Open /home/user/Notes/guide anyway' }),
    );

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(entry('Getting Started')).toBeInTheDocument();
  });

  it('an empty path opens nothing', async () => {
    const wsOpen = vi.fn();
    const shell = await connectedShell({}, { ws_open: wsOpen });
    render(App, { props: { shell } });
    await shell.startWorkspace();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const field = await waitFor(() => screen.getByLabelText('Another directory:'));

    await fireEvent.input(field, { target: { value: '   ' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    expect(wsOpen).not.toHaveBeenCalled();
    expect(dialog('Open workspace')).toBeInTheDocument();
  });

  it('D-02 (fixed): Escape closes the dialog, as Cancel does', async () => {
    await app();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const field = await waitFor(() => screen.getByLabelText('Another directory:'));

    await fireEvent.keyDown(field, { key: 'Escape' });

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    await fireEvent.click(await waitFor(() => screen.getByRole('button', { name: 'Cancel' })));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('M-03.4: opening one dialog closes the other', async () => {
    await app();

    await fireEvent.click(screen.getByRole('button', { name: 'New workspace…' }));
    expect(dialog('New workspace')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));

    await waitFor(() => expect(dialog('Open workspace')).toBeInTheDocument());
    expect(screen.queryByRole('dialog', { name: 'New workspace' })).not.toBeInTheDocument();
  });

  it('UC-002 A9: the open workspace only closes the dialog', async () => {
    const shell = await app({ givenDir: '/home/user/Notes' });
    await fireEvent.click(entry('Ideas'));
    await waitFor(() => expect(tab('Ideas')).toBeInTheDocument());
    await shell.startOpen();
    const field = await waitFor(() => screen.getByLabelText('Another directory:'));

    await fireEvent.input(field, { target: { value: '/home/user/Notes' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(shell.tabs.tabs.map((t) => t.note)).toEqual(['Home', 'Ideas']);
  });
});

describe('M-21.2 Create dialog (UC-001)', () => {
  async function toOptions(path: string) {
    await fireEvent.click(screen.getByRole('button', { name: 'New workspace…' }));
    const field = screen.getByLabelText('Directory for the new workspace (new or empty):');
    await waitFor(() => expect(field).toHaveFocus());
    await fireEvent.input(field, { target: { value: path } });
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
  }

  it('creates a Markdown workspace and opens its Home note', async () => {
    const shell = await app();

    await toOptions('/home/user/Fresh');
    const create = await waitFor(() => dialog('New workspace'));
    expect(within(create).getByText('Directory: /home/user/Fresh')).toBeInTheDocument();
    expect(within(create).getByRole('radio', { name: 'Markdown' })).toBeChecked();
    expect(
      within(create).getByRole('checkbox', { name: 'Create as git repository' }),
    ).not.toBeChecked();
    await fireEvent.click(within(create).getByRole('button', { name: 'Create' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(shell.workspace?.name).toBe('Fresh');
    await waitFor(() =>
      expect(screen.getByRole('status')).toHaveTextContent('Workspace created in /home/user/Fresh'),
    );
    expect(tab('Home')).toHaveAttribute('aria-selected', 'true');
  });

  it('UC-001 A3: the git option asks for the remote, then creates', async () => {
    const handlers = (await import('./mock-backend')).mockHandlers();
    const wsCreate = vi.fn(handlers.ws_create);
    const shell = await connectedShell({}, { ...handlers, ws_create: wsCreate });
    render(App, { props: { shell } });
    await shell.startWorkspace();

    await toOptions('/home/user/Repo');
    await waitFor(() => screen.getByRole('radio', { name: 'AsciiDoc' }));
    await fireEvent.click(screen.getByRole('radio', { name: 'AsciiDoc' }));
    await fireEvent.click(screen.getByRole('checkbox', { name: 'Create as git repository' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    const remote = screen.getByLabelText('Address of the git remote (optional):');
    expect(
      screen.getByText(
        'The remote is not contacted now; the first git sync shows whether it is reachable.',
      ),
    ).toBeInTheDocument();
    await fireEvent.input(remote, { target: { value: 'git@example.org:me/repo.git' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() =>
      expect(wsCreate).toHaveBeenCalledWith({
        request: {
          directory: '/home/user/Repo',
          format: 'AsciiDoc',
          git: true,
          remote: 'git@example.org:me/repo.git',
        },
      }),
    );
  });

  it('UC-001 A1, A2: a directory with files offers to open it instead', async () => {
    await app();

    await toOptions('/home/user/Letters');
    const problem = await waitFor(() => screen.getByRole('alert'));
    expect(problem).toHaveTextContent(
      '/home/user/Letters already holds files or folders. Choose another directory or open it as a workspace.',
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Open it as a workspace instead' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(entry('Draft')).toBeInTheDocument();
  });
});

describe('review F2: dialogs and start', () => {
  it('a problem puts the keyboard in the path field with its text selected', async () => {
    await app();
    await fireEvent.click(screen.getByRole('button', { name: 'Open workspace…' }));
    const field = await waitFor(() =>
      screen.getByLabelText<HTMLInputElement>('Another directory:'),
    );

    await fireEvent.input(field, { target: { value: '/home/user/Gone' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
    const again = screen.getByLabelText<HTMLInputElement>('Another directory:');
    await waitFor(() => expect(again).toHaveFocus());
    expect([again.selectionStart, again.selectionEnd]).toEqual([0, '/home/user/Gone'.length]);
  });

  it('"Open" and "Next" wait for a path', async () => {
    await app();
    await fireEvent.click(screen.getByRole('button', { name: 'New workspace…' }));

    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
    await fireEvent.input(
      screen.getByLabelText('Directory for the new workspace (new or empty):'),
      { target: { value: ' x ' } },
    );
    expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled();
  });

  it('Enter in the options creates, Enter after the git option asks the remote', async () => {
    const shell = await app();
    await fireEvent.click(screen.getByRole('button', { name: 'New workspace…' }));
    const field = screen.getByLabelText('Directory for the new workspace (new or empty):');
    await fireEvent.input(field, { target: { value: '/home/user/Repo' } });
    await fireEvent.submit(field.closest('form')!);
    const git = await waitFor(() =>
      screen.getByRole('checkbox', { name: 'Create as git repository' }),
    );

    await fireEvent.click(git);
    await fireEvent.submit(git.closest('form')!);

    const remote = screen.getByLabelText('Address of the git remote (optional):');
    await waitFor(() => expect(remote).toHaveFocus());
    await fireEvent.submit(remote.closest('form')!);
    await waitFor(() => expect(shell.workspace?.name).toBe('Repo'));
  });

  it('an event sent while ws_start runs wins over its older snapshot', async () => {
    const handlers = (await import('./mock-backend')).mockHandlers({
      givenDir: '/home/user/Notes',
    });
    const { emit } = await import('@tauri-apps/api/event');
    const shell = await connectedShell(
      { givenDir: '/home/user/Notes' },
      {
        ws_start: async () => {
          const started = await handlers.ws_start();
          await emit('tabs', { ...started.snapshot.tabs, active: 'Ideas' });
          return started;
        },
      },
    );

    await shell.startWorkspace();

    expect(shell.tabs.active).toBe('Ideas');
    expect(shell.workspace?.name).toBe('Notes');
  });

  it('workspace commands run in the order they were given', async () => {
    const order: string[] = [];
    const handlers = (await import('./mock-backend')).mockHandlers({
      givenDir: '/home/user/Notes',
    });
    const shell = await connectedShell(
      { givenDir: '/home/user/Notes' },
      {
        tab_open: async (args) => {
          await new Promise((resolve) => setTimeout(resolve, 20));
          order.push(`open ${args.note}`);
          return handlers.tab_open(args);
        },
        tab_activate: async (args) => {
          order.push(`activate ${args.note}`);
          return handlers.tab_activate(args);
        },
      },
    );

    await Promise.all([shell.openNote('Ideas'), shell.activateTab('Ideas')]);

    expect(order).toEqual(['open Ideas', 'activate Ideas']);
  });
});
