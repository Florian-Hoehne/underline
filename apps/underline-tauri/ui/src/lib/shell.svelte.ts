// The window's mirror of the backend session for the shell (feature F1):
// what `app_start` returned, kept current by the `status` and `settings`
// events (ADR-022), plus the UI state that is not stored (M-22.4). No domain
// logic lives here: every change goes to the backend, which answers with an
// event.

import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  commands,
  events,
  type Platform,
  type SettingName,
  type SettingsView,
  type StartInfo,
  type UiError,
} from './bindings';
import { applyPalette } from './theme';

export class Shell {
  /** What the window started with; `null` when `app_start` failed. */
  start = $state<StartInfo | null>(null);
  /** The settings in effect, with the theme's palette. */
  settings = $state<SettingsView | null>(null);
  /** M-04.7: the status message. */
  status = $state('');
  /** The root of the open workspace; opening one comes with feature F2. */
  workspaceRoot = $state<string | null>(null);
  /** M-03.5: the wiki project URL typed in the Workspace menu, not stored. */
  wikiUrl = $state('');
  /** M-03.2: the search overlay (feature F9). */
  searchOpen = $state(false);
  /** M-03.7: the settings window (feature F10). */
  settingsOpen = $state(false);

  #unlisten: UnlistenFn[] = [];

  /** "Cmd" on macOS, "Ctrl" elsewhere, for shortcut texts. */
  get command(): string {
    return this.platform === 'macos' ? 'Cmd' : 'Ctrl';
  }

  get platform(): Platform {
    return this.start?.platform ?? 'linux';
  }

  /** The names of all themes, for the Theme menu (M-03.6). */
  get themes(): string[] {
    return this.start?.themes ?? [];
  }

  /**
   * Listens to the backend's events, then asks what the window started
   * with. Listening first means no change is missed in between.
   */
  async connect(): Promise<void> {
    this.#unlisten = await Promise.all([
      events.status.listen((event) => {
        this.status = event.payload.text;
      }),
      events.settings.listen((event) => {
        this.#takeSettings(event.payload);
      }),
    ]);
    try {
      const result = await commands.appStart();
      if (result.status === 'ok') {
        this.start = result.data;
        this.status = result.data.status;
        this.#takeSettings(result.data.settings);
      } else {
        this.#fail('app_start', result.error);
      }
    } catch (error) {
      this.#fail('app_start', { kind: 'internal', message: String(error) });
    }
  }

  /** Stops listening to the backend. */
  disconnect(): void {
    this.#unlisten.forEach((unlisten) => unlisten());
    this.#unlisten = [];
  }

  /** UC-010 A3, M-20.5: a setting changed outside the settings window. */
  async quickSetting(key: SettingName, value: string): Promise<void> {
    const result = await commands.settingsQuick(key, value);
    if (result.status === 'error') {
      this.#fail('settings_quick', result.error);
    }
  }

  /** M-03.6: the theme chosen in the Theme menu. */
  chooseTheme(name: string): Promise<void> {
    return this.quickSetting('theme', name);
  }

  /** M-03.1: the burger button shows or hides the menu bar. */
  toggleMenuBar(): Promise<void> {
    const shown = this.settings?.showMenuBar ?? false;
    return this.quickSetting('show_menu_bar', String(!shown));
  }

  /** M-03.5: checks the wiki project URL. */
  async checkWiki(): Promise<void> {
    const result = await commands.wikiCheck(this.wikiUrl);
    if (result.status === 'error') {
      this.#fail('wiki_check', result.error);
    }
  }

  #takeSettings(settings: SettingsView): void {
    this.settings = settings;
    applyPalette(settings.palette);
  }

  /** A failed command: its message is a status text (UiError). */
  #fail(command: string, error: UiError): void {
    console.error(`${command} failed: ${error.message}`);
    this.status = error.message;
  }
}
