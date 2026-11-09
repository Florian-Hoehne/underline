// Test helpers: a shell connected to the mock backend.

import {
  installMockBackend,
  mockHandlers,
  type MockHandlers,
  type MockStart,
} from '../lib/mock-backend';
import { Shell } from '../lib/shell.svelte';

/**
 * A shell connected to a mock backend that started with `start`; `handlers`
 * replace single commands.
 */
export async function connectedShell(
  start: MockStart = {},
  handlers: Partial<MockHandlers> = {},
): Promise<Shell> {
  installMockBackend({ ...mockHandlers(start), ...handlers });
  const shell = new Shell();
  await shell.connect();
  return shell;
}
