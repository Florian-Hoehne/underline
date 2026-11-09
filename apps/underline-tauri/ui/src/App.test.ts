import { cleanup, render, screen } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import { connectedShell } from './test/shell';

afterEach(() => {
  // Unmounting stops listening to the mock, so it comes first.
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

describe('M-02 empty screen (UC-002 A2)', () => {
  it('M-02.1 offers to open or create a workspace, between the bars', async () => {
    const shell = await connectedShell();

    render(App, { props: { shell } });

    const main = screen.getByRole('main');
    expect(main).toContainElement(screen.getByRole('heading', { name: 'No workspace is open' }));
    expect(main).toContainElement(screen.getByRole('button', { name: 'Open workspace…' }));
    expect(main).toContainElement(screen.getByRole('button', { name: 'New workspace…' }));
    expect(screen.getByRole('banner')).toBeInTheDocument();
    expect(screen.getByRole('contentinfo')).toBeInTheDocument();
  });

  it('shows the message of a failed start in the status bar', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const shell = await connectedShell(
      {},
      {
        app_start: () => {
          throw { kind: 'internal', message: 'The session is gone' };
        },
      },
    );

    render(App, { props: { shell } });

    expect(consoleError).toHaveBeenCalledWith('app_start failed: The session is gone');
    expect(screen.getByRole('status')).toHaveTextContent('The session is gone');
    expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument();
  });

  it('M-01.3 shows the note about the state file at start', async () => {
    const shell = await connectedShell({ status: 'Note: the remembered workspaces …' });

    render(App, { props: { shell } });

    expect(screen.getByRole('status')).toHaveTextContent('Note: the remembered workspaces …');
  });
});
