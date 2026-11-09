// A stand-in for the Rust backend, so the window runs in a plain browser
// (Playwright against http://localhost:1420, plan section 10). Each handler
// returns what the command returns on success; the types come from the
// generated bindings, so a changed command breaks the mock at compile time.
// Like the backend, the mock sends `status` and `settings` events after a
// command changed them.

import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import type {
  commands,
  PaletteView,
  SettingName,
  SettingsView,
  StartInfo,
  UiError,
} from './bindings';
import palettes from './palettes.generated.json';

/** The success value of a command in the generated bindings. */
type Ok<C extends (...args: never[]) => Promise<{ status: string }>> = Extract<
  Awaited<ReturnType<C>>,
  { status: 'ok' }
>['data'];

/** One handler per backend command, keyed by the command's Rust name. */
export type MockHandlers = {
  app_start: () => Ok<typeof commands.appStart>;
  settings_quick: (args: {
    key: SettingName;
    value: string;
  }) => Ok<typeof commands.settingsQuick> | Promise<Ok<typeof commands.settingsQuick>>;
  wiki_check: (args: {
    url: string;
  }) => Ok<typeof commands.wikiCheck> | Promise<Ok<typeof commands.wikiCheck>>;
};

/** The palettes of all themes, as the backend exports them. */
export const PALETTES: readonly PaletteView[] = palettes;

/** The palette of the theme named `name`. */
export function paletteOf(name: string): PaletteView | undefined {
  return PALETTES.find((palette) => palette.theme === name);
}

/** The built-in settings (UC-010 BR-001): light theme, menu bar hidden. */
export function defaultSettings(): SettingsView {
  return {
    theme: 'Light',
    palette: PALETTES[0],
    inputMode: 'Normal',
    showLineNumbers: true,
    lineNumberSide: 'Left',
    showMinimap: true,
    copyOnSelect: true,
    gitSyncInterval: 5,
    agentAccess: 'Off',
    agentAccessLowered: false,
    agentAccessRefused: false,
    overviewWidth: 200,
    toolPanelWidth: 240,
    showMenuBar: false,
    blinkFoundInMinimap: true,
  };
}

/** What the mock starts with; every part can be replaced. */
export type MockStart = Partial<Omit<StartInfo, 'settings'>> & {
  settings?: Partial<SettingsView>;
};

/**
 * Handlers answering like a window started without a workspace (M-02),
 * with their own settings and status that the commands change.
 */
export function mockHandlers(start: MockStart = {}): MockHandlers {
  let settings: SettingsView = { ...defaultSettings(), ...start.settings };
  let status = start.status ?? '';
  const setStatus = async (text: string) => {
    status = text;
    await emit('status', { text });
  };
  return {
    app_start: () => ({
      givenDir: null,
      lastWorkspace: null,
      recentWorkspaces: [],
      platform: 'linux',
      themes: PALETTES.map((palette) => palette.theme),
      ...start,
      settings,
      status,
    }),
    settings_quick: async ({ key, value }) => {
      if (key === 'theme') {
        const palette = paletteOf(value);
        if (!palette) {
          const names = PALETTES.map((p) => p.theme).join(', ');
          await setStatus(`${JSON.stringify(value)} is none of ${names}`);
          return null;
        }
        settings = { ...settings, theme: value, palette };
      } else if (key === 'show_menu_bar') {
        settings = { ...settings, showMenuBar: value === 'true' };
      } else {
        return null;
      }
      await emit('settings', settings);
      return null;
    },
    wiki_check: async ({ url }) => {
      await setStatus(wikiStatus(url));
      return null;
    },
  };
}

/**
 * A rough stand-in for `underline_wiki::WikiRemote` (the mock only needs
 * plausible texts; the real rules are tested in Rust).
 */
function wikiStatus(url: string): string {
  const match = /^https?:\/\/([^/]+)\/(.+?)\/?$/.exec(url);
  const project = match?.[2].replace(/\.git$/, '').replace(/\.wiki$/, '');
  const segments = project?.split('/').filter(Boolean).length ?? 0;
  const github = match?.[1] === 'github.com';
  if (!match || !project || (github ? segments !== 2 : segments < 2)) {
    return `\`${url}\` is not a GitHub or GitLab project URL`;
  }
  return `Wiki repository: https://${match[1]}/${project}.wiki.git`;
}

/** Whether the page runs inside the Tauri window. */
export function insideTauri(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

/**
 * Answers every command from `handlers` and delivers events to listeners.
 * A handler reports a failure by throwing a `UiError`, which reaches the
 * caller as `{ status: "error" }`.
 */
export function installMockBackend(handlers: Partial<MockHandlers> = mockHandlers()): void {
  mockIPC(
    (cmd, args) => {
      if (cmd in handlers) {
        const handler = handlers[cmd as keyof MockHandlers] as (args: unknown) => unknown;
        return handler(args);
      }
      const error: UiError = {
        kind: 'internal',
        message: `The mock backend has no command ${cmd}`,
      };
      throw error;
    },
    { shouldMockEvents: true },
  );
}
