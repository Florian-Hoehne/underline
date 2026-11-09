// A stand-in for the Rust backend, so the window runs in a plain browser
// (Playwright against http://localhost:1420, plan section 10). Each handler
// returns what the command returns on success; the types come from the
// generated bindings, so a changed command breaks the mock at compile time.

import { mockIPC } from '@tauri-apps/api/mocks';
import type { commands, UiError } from './bindings';

/** The success value of a command in the generated bindings. */
type Ok<C extends (...args: never[]) => Promise<{ status: string }>> = Extract<
  Awaited<ReturnType<C>>,
  { status: 'ok' }
>['data'];

/** One handler per backend command, keyed by the command's Rust name. */
export type MockHandlers = {
  app_start: () => Ok<typeof commands.appStart>;
};

/** Handlers answering like a window started without a workspace (M-02). */
export const defaultHandlers: MockHandlers = {
  app_start: () => ({ givenDir: null, platform: 'linux' }),
};

/** Whether the page runs inside the Tauri window. */
export function insideTauri(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

/**
 * Answers every command from `handlers`. A handler reports a failure by
 * throwing a `UiError`, which reaches the caller as `{ status: "error" }`.
 */
export function installMockBackend(handlers: MockHandlers = defaultHandlers): void {
  mockIPC((cmd) => {
    if (cmd in handlers) {
      return handlers[cmd as keyof MockHandlers]();
    }
    const error: UiError = { kind: 'internal', message: `The mock backend has no command ${cmd}` };
    throw error;
  });
}
