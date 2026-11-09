import { render, screen, waitFor } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import { defaultHandlers, installMockBackend } from './lib/mock-backend';

afterEach(() => {
  clearMocks();
  vi.restoreAllMocks();
});

describe('M-02 empty screen (UC-002 A2)', () => {
  it('offers to open or create a workspace when none is open', async () => {
    const appStart = vi.fn(defaultHandlers.app_start);
    installMockBackend({ ...defaultHandlers, app_start: appStart });

    render(App);

    expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open workspace…' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'New workspace…' })).toBeInTheDocument();
    await waitFor(() => expect(appStart).toHaveBeenCalledOnce());
  });

  it('logs a failed start and still shows the empty screen', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    installMockBackend({
      app_start: () => {
        throw { kind: 'internal', message: 'The session is gone' };
      },
    });

    render(App);

    await waitFor(() =>
      expect(consoleError).toHaveBeenCalledWith('app_start failed: The session is gone'),
    );
    expect(screen.getByRole('heading', { name: 'No workspace is open' })).toBeInTheDocument();
  });
});
