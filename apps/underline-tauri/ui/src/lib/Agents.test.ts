// Feature F14: the agents (UC-016 to UC-019, inventory M-04.4, M-04.9,
// M-17.1 to M-17.7), against the mock backend's agents (`mock-agents.ts`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHandlers, mockHooks, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function app(start: MockStart = {}, handlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
const SOURCE: MockStart = { ...NOTES, view: 'source' };
const READ_WRITE: MockStart = { ...NOTES, settings: { agentAccess: 'Read and write' } };

const agentsEntry = () => screen.getByRole('button', { name: /Agents: (off|read)/ });
const menu = () => screen.getByRole('menu', { name: 'Agents' });
const settings = () => screen.getByRole('dialog', { name: 'Settings' });
const section = () => within(settings()).getByRole('group', { name: 'Agents' });

async function openSettings() {
  await fireEvent.keyDown(document.body, { key: ',', ctrlKey: true });
  await waitFor(() => within(section()).getByRole('button', { name: 'Install Claude Code' }));
  return within(section());
}

describe('M-04.4, M-17.1 the agents entry of the status bar (UC-017)', () => {
  it('says the access, and opens a menu of the levels and the workspace settings', async () => {
    await app(NOTES);

    await waitFor(() => expect(agentsEntry()).toHaveTextContent('Agents: off'));
    expect(agentsEntry()).toHaveAttribute(
      'title',
      "What agents may do with the notes through Underline's MCP server",
    );
    await fireEvent.click(agentsEntry());

    const levels = within(menu()).getAllByRole('menuitemradio');
    expect(levels.map((l) => [l.textContent?.trim(), l.getAttribute('aria-checked')])).toEqual([
      ['Off', 'true'],
      ['Read only', 'false'],
      ['Read and write', 'false'],
    ]);
    expect(within(menu()).getByRole('menuitem')).toHaveTextContent('Settings of this workspace…');
    expect(within(menu()).getByRole('separator')).toBeInTheDocument();
  });

  it('a level chosen changes the user setting (BR-002)', async () => {
    const shell = await app(NOTES);
    await fireEvent.click(await waitFor(() => agentsEntry()));

    await fireEvent.click(within(menu()).getByRole('menuitemradio', { name: 'Read and write' }));

    await waitFor(() => expect(agentsEntry()).toHaveTextContent('Agents: read and write'));
    expect(mockHooks().settingsFile('user').agent_access).toBe('Read and write');
    expect(shell.settings?.agentAccess).toBe('Read and write');
    expect(screen.queryByRole('menu', { name: 'Agents' })).not.toBeInTheDocument();
  });

  it('names the connected agents; one that asked while access is off (A2, A4)', async () => {
    await app(READ_WRITE);
    await mockHooks().agentConnect('Claude Code');
    await mockHooks().agentConnect('Cursor');

    await waitFor(() =>
      expect(agentsEntry()).toHaveTextContent(
        'Agents: read and write · Agents connected: Claude Code, Cursor',
      ),
    );

    await mockHooks().agentDisconnect('Cursor');
    await waitFor(() =>
      expect(agentsEntry()).toHaveTextContent(
        'Agents: read and write · Agent connected: Claude Code',
      ),
    );
  });

  it('an agent that asked while access is off (A2)', async () => {
    const shell = await app(NOTES);

    await mockHooks().agentConnect('Claude', true);

    // UX review F14: what the agent asked stands first, marked.
    await waitFor(() =>
      expect(agentsEntry()).toHaveTextContent('⚠ Claude asked for access · Agents: off'),
    );
    expect(shell.agentAnnouncement).toBe('Claude asked for access');
  });

  it('the workspace caps the levels (A1)', async () => {
    await app(READ_WRITE);
    await mockHooks().settingsOutside('workspace', 'agent_access', 'Read only');

    await waitFor(() =>
      expect(agentsEntry()).toHaveTextContent('Agents: read only in this workspace'),
    );
    await fireEvent.click(agentsEntry());
    const labels = within(menu())
      .getAllByRole('menuitemradio')
      .map((l) => l.textContent?.trim());
    expect(labels).toEqual(['Off', 'Read only']);
  });

  it('"Settings of this workspace…" opens the settings at "This workspace"', async () => {
    await app(NOTES);
    await fireEvent.click(await waitFor(() => agentsEntry()));

    await fireEvent.click(
      within(menu()).getByRole('menuitem', { name: 'Settings of this workspace…' }),
    );

    await waitFor(() =>
      expect(within(settings()).getByRole('tab', { name: 'This workspace' })).toHaveAttribute(
        'aria-selected',
        'true',
      ),
    );
  });

  it('Escape closes the menu and gives the keyboard back to the entry', async () => {
    await app(NOTES);
    await fireEvent.click(await waitFor(() => agentsEntry()));
    const first = within(menu()).getAllByRole('menuitemradio')[0];
    await waitFor(() => expect(document.activeElement).toBe(first));

    await fireEvent.keyDown(first, { key: 'Escape' });

    expect(screen.queryByRole('menu', { name: 'Agents' })).not.toBeInTheDocument();
    expect(document.activeElement).toBe(agentsEntry());
  });

  it('is not there without a workspace', async () => {
    const shell = await connectedShell({});
    render(App, { props: { shell } });

    expect(screen.queryByRole('button', { name: /^Agents: / })).not.toBeInTheDocument();
    expect(shell.agents.head).toEqual(['Open a workspace to connect agents to it.']);
  });
});

describe('M-04.9, M-17.7 an agent changes notes (UC-019)', () => {
  it('a note not open is reported with "Open"; changes in a row together', async () => {
    const shell = await app(READ_WRITE);
    await mockHooks().agentConnect('Claude');

    const answer = await mockHooks().agentChange('Claude', 'Trip', 'Hydra.', 'Spetses.');

    expect(answer).toBe('changed Trip; the change is in line 3');
    await waitFor(() => expect(shell.status).toBe('Claude changed Trip'));
    expect(
      screen.getByRole('button', { name: 'Open Trip, changed by an agent' }),
    ).toHaveTextContent(/^Open$/);

    await mockHooks().agentCreate('Claude', 'Plans', '# Plans\n');
    await waitFor(() => expect(shell.status).toBe('Claude created Plans and changed Trip'));
    const opens = screen
      .getAllByRole('button', { name: /by an agent$/ })
      .map((b) => b.textContent?.trim());
    expect(opens).toEqual(['Open Plans', 'Open Trip']);
    expect(screen.getByRole('button', { name: 'Open Plans, created by an agent' })).toBeVisible();

    await fireEvent.click(screen.getByRole('button', { name: 'Open Trip, changed by an agent' }));
    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
  });

  it('without writing access it is refused', async () => {
    await app(NOTES);

    const answer = await mockHooks().agentChange('Claude', 'Trip', 'Hydra.', 'Spetses.');

    expect(answer).toBe(
      'the note `Trip` cannot be written: Underline allows agents only to read the notes of this workspace',
    );
  });

  it('an open note takes the change and shows the agent cursor (M-17.6)', async () => {
    await app({ ...READ_WRITE, view: 'source' });
    await waitFor(() => expect(document.querySelector('.cm-content')).not.toBeNull());
    await mockHooks().agentConnect('Claude');

    await mockHooks().agentChange('Claude', 'Home', 'Type here', 'Write here');

    await waitFor(() => expect(mockHooks().mirrorText('Home')).toContain('Write here'));
    await waitFor(() =>
      expect(document.querySelector('.cm-content')?.textContent).toContain('Write here'),
    );
    const marked = document.querySelector('.cm-ul-agent-first');
    expect(marked?.getAttribute('data-agent')).toBe('Claude');
    expect(marked?.textContent).toContain('Write here');
  });

  it('a todo an agent changed is the status (UC-020 A8)', async () => {
    const shell = await app(NOTES);

    await mockHooks().agentTodo('Claude', 'created todo #3');

    await waitFor(() => expect(shell.status).toBe('Claude created todo #3'));
  });
});

describe('M-17.6 the agent cursor (UC-018 BR-006, UC-017 BR-005)', () => {
  it('a cursor that appears is told to screen readers', async () => {
    const shell = await app({ ...SOURCE, settings: { agentAccess: 'Read and write' } });
    await mockHooks().agentConnect('Claude');

    await mockHooks().agentRead('Claude', 'Home', 1, 1, true);

    await waitFor(() => expect(shell.agentAnnouncement).toBe('Claude is editing Home'));
    expect(screen.getByText('Claude is editing Home')).toHaveAttribute('aria-live', 'polite');
  });

  it('marks the lines read in the source view, with the name on the first', async () => {
    await app({ ...SOURCE, settings: { agentAccess: 'Read only' } });
    await waitFor(() => expect(document.querySelectorAll('.cm-line').length).toBeGreaterThan(2));
    await mockHooks().agentConnect('Claude');

    await mockHooks().agentRead('Claude', 'Home', 3, 4);

    await waitFor(() => expect(document.querySelectorAll('.cm-ul-agent')).toHaveLength(2));
    const lines = [...document.querySelectorAll('.cm-line')];
    expect(lines[2]).toHaveClass('cm-ul-agent-first');
    expect(lines[2].getAttribute('data-agent')).toBe('Claude');
    expect(lines[3]).toHaveClass('cm-ul-agent');
    expect(lines[3].hasAttribute('data-agent')).toBe(false);

    // Gone when the agent goes (A3).
    await mockHooks().agentDisconnect('Claude');
    await waitFor(() => expect(document.querySelectorAll('.cm-ul-agent')).toHaveLength(0));
  });

  it('lines written show only with writing access', async () => {
    const shell = await app({ ...SOURCE, settings: { agentAccess: 'Read only' } });
    await waitFor(() => expect(document.querySelectorAll('.cm-line').length).toBeGreaterThan(2));
    await mockHooks().agentConnect('Claude');

    await mockHooks().agentRead('Claude', 'Home', 1, 1, true);
    expect(shell.agentMarksOf('Home')).toEqual([]);

    await shell.setAgentAccess('readWrite');
    await waitFor(() => expect(shell.agentMarksOf('Home')).toHaveLength(1));
  });

  it('marks the blocks of the lines in the facade, naming the agent once', async () => {
    await app({ ...NOTES, settings: { agentAccess: 'Read only' } });
    await waitFor(() => expect(document.querySelector('.slot')).not.toBeNull());
    await mockHooks().agentConnect('Claude');

    await mockHooks().agentRead('Claude', 'Home', 1, 3);

    await waitFor(() => expect(document.querySelectorAll('.slot.agent').length).toBeGreaterThan(0));
    const named = document.querySelectorAll('.slot.agent[data-agent]');
    expect(named).toHaveLength(1);
    expect(named[0].getAttribute('data-agent')).toBe('Claude');
  });
});

describe('M-17.2 to M-17.5 the section "Agents" of the settings (UC-016)', () => {
  it('Escape in the question is its "Cancel" (Q-109), the window stays', async () => {
    await app(NOTES);
    const agents = await openSettings();
    await fireEvent.click(agents.getByRole('button', { name: 'Install Claude Code' }));
    const cancel = await waitFor(() => agents.getByRole('button', { name: 'Cancel' }));

    await fireEvent.keyDown(cancel, { key: 'Escape' });

    await waitFor(() =>
      expect(agents.queryByRole('button', { name: 'Cancel' })).not.toBeInTheDocument(),
    );
    expect(settings()).toBeInTheDocument();
  });

  it('lists the tools with their states, "Install" and "Remove"', async () => {
    await app(NOTES);

    const agents = await openSettings();

    expect(
      agents.getByText(/^Let an agent tool reach the notes of \/home\/user\/Notes/),
    ).toBeInTheDocument();
    expect(
      agents.getByText('Agent access is off; agents reach the notes only while it is on.'),
    ).toBeInTheDocument();
    expect(agents.getByRole('button', { name: 'Install Cursor' })).toBeEnabled();
    expect(agents.getByRole('button', { name: 'Install Codex' })).toBeDisabled();
    expect(agents.getAllByText('not found')).toHaveLength(4);
    expect(agents.getByRole('button', { name: 'Copy configuration' })).toHaveAttribute(
      'title',
      'For any other agent tool',
    );
  });

  it('install with access off asks first; allowing installs and sets the access (A1)', async () => {
    const shell = await app(NOTES);
    const agents = await openSettings();

    await fireEvent.click(agents.getByRole('button', { name: 'Install Claude Code' }));

    await waitFor(() =>
      expect(
        agents.getByText(
          'Agents reach the notes only while agent access is on. Allow agents to read, or to read and write, for you?',
        ),
      ).toBeInTheDocument(),
    );
    const allow = agents.getByRole('button', { name: 'Allow reading and writing and install' });
    await waitFor(() =>
      expect(document.activeElement).toBe(agents.getAllByRole('button', { name: /^Allow/ })[0]),
    );
    await fireEvent.click(allow);

    await waitFor(() =>
      expect(agents.getByRole('status')).toHaveTextContent(
        'Installed Notes in Claude Code as underline-Notes; restart Claude Code if it does not show it yet.',
      ),
    );
    expect(agents.getByText('installed as underline-Notes')).toBeInTheDocument();
    expect(shell.settings?.agentAccess).toBe('Read and write');

    await fireEvent.click(agents.getByRole('button', { name: 'Remove Claude Code' }));
    await waitFor(() =>
      expect(agents.getByRole('status')).toHaveTextContent('Removed Notes from Claude Code.'),
    );
  });

  it('"Install only" says the access stays off', async () => {
    await app(NOTES);
    const agents = await openSettings();
    await fireEvent.click(agents.getByRole('button', { name: 'Install Cursor' }));

    await fireEvent.click(
      await waitFor(() => agents.getByRole('button', { name: 'Install only' })),
    );

    await waitFor(() =>
      expect(agents.getByRole('status')).toHaveTextContent(
        'Agent access is off, so the agent cannot reach the notes yet.',
      ),
    );
  });

  it('an entry for another place is replaced after asking (A2)', async () => {
    await app(READ_WRITE);
    mockHooks().agentTool('cursor', 'otherPlace');
    const agents = await openSettings();
    expect(agents.getByText('underline-Notes: installed for another place')).toBeInTheDocument();

    await fireEvent.click(agents.getByRole('button', { name: 'Install Cursor' }));

    // App-tester F14: the question above its buttons, as A1's.
    const question = await waitFor(() =>
      agents.getByRole('group', {
        name: 'An entry for this workspace starts another Underline. Replace it?',
      }),
    );
    await fireEvent.click(within(question).getByRole('button', { name: 'Replace' }));
    await waitFor(() =>
      expect(agents.getAllByText('installed as underline-Notes').length).toBeGreaterThan(0),
    );
  });

  it('"Cancel" of A2 takes the question away with its text', async () => {
    await app(READ_WRITE);
    mockHooks().agentTool('cursor', 'otherPlace');
    const agents = await openSettings();
    await fireEvent.click(agents.getByRole('button', { name: 'Install Cursor' }));
    const cancel = await waitFor(() => agents.getByRole('button', { name: 'Cancel' }));

    await fireEvent.click(cancel);

    await waitFor(() =>
      expect(agents.queryByText(/starts another Underline/)).not.toBeInTheDocument(),
    );
  });

  it('an unreadable configuration is marked and has no button (A6)', async () => {
    await app(NOTES);
    mockHooks().agentTool('cursor', 'unreadable');

    const agents = await openSettings();

    const state = agents.getByText(/cannot be read$/, { selector: '.tool-state' });
    expect(state).toHaveTextContent('⚠');
    expect(agents.queryByRole('button', { name: 'Install Cursor' })).not.toBeInTheDocument();
  });

  it('"Copy configuration" copies it and says its command (A4)', async () => {
    await app(NOTES);
    const agents = await openSettings();

    await fireEvent.click(agents.getByRole('button', { name: 'Copy configuration' }));

    await waitFor(() =>
      expect(agents.getByRole('status')).toHaveTextContent(
        'Copied the configuration of Notes. Its command: /home/user/.local/bin/underline mcp /home/user/Notes',
      ),
    );
    expect(mockHooks().clipboardContent().text).toContain('"mcpServers"');
  });

  it('without a workspace it says to open one', async () => {
    const shell = await connectedShell({});
    render(App, { props: { shell } });
    await fireEvent.keyDown(document.body, { key: ',', ctrlKey: true });

    await waitFor(() =>
      expect(
        within(section()).getByText('Open a workspace to connect agents to it.'),
      ).toBeInTheDocument(),
    );
    expect(within(section()).queryByRole('button')).not.toBeInTheDocument();
  });
});
