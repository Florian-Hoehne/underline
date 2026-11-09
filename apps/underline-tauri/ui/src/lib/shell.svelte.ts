// The window's mirror of the backend session: what `app_start` returned,
// kept current by the `status`, `settings`, `workspace`, `overview` and
// `tabs` events (ADR-022), plus the UI state that is not stored (M-22.4):
// the Open and Create dialogs, the loading state and the overview's width
// while it is dragged. No domain logic lives here: every change goes to the
// backend, which answers with an event or with the dialog's next stage.

import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  commands,
  events,
  type CreateAnswer,
  type FormatName,
  type OpenAnswer,
  type OpenOffer,
  type OverviewItem,
  type Platform,
  type SettingName,
  type SettingsView,
  type StartInfo,
  type TabsView,
  type UiError,
  type WorkspaceSnapshot,
  type WorkspaceView,
} from './bindings';
import { applyPalette } from './theme';

/** M-21.1: the Open dialog (UC-002 steps 2–4). */
export type OpenDialog = {
  kind: 'open';
  /** "Recently opened:", the most recent first. */
  recent: string[];
  /** "Another directory:". */
  path: string;
  /** Why the chosen directory cannot be opened. */
  problem: string | null;
  offer: OpenOffer | null;
};

/** M-21.2: the Create dialog, one stage per question (UC-001). */
export type CreateDialog =
  | {
      kind: 'create';
      stage: 'directory';
      path: string;
      problem: string | null;
      /** A1: the directory with content, to open it instead. */
      offer: string | null;
    }
  | { kind: 'create'; stage: 'options'; directory: string; format: FormatName; git: boolean }
  | { kind: 'create'; stage: 'remote'; directory: string; format: FormatName; address: string };

/** The Open or the Create dialog; opening one closes the other (M-03.4). */
export type WorkspaceDialog = OpenDialog | CreateDialog;

/** M-05.1: the note overview is 100 to 1000 pixels wide. */
export const OVERVIEW_WIDTH = { min: 100, max: 1000 } as const;

/** `width` within the overview's range, rounded to a pixel. */
export function overviewWidthWithin(width: number): number {
  return Math.min(OVERVIEW_WIDTH.max, Math.max(OVERVIEW_WIDTH.min, Math.round(width)));
}

const NO_TABS: TabsView = { tabs: [], active: null, todoList: { open: false, active: false } };

export class Shell {
  /** What the window started with; `null` when `app_start` failed. */
  start = $state<StartInfo | null>(null);
  /** The settings in effect, with the theme's palette. */
  settings = $state<SettingsView | null>(null);
  /** M-04.7: the status message. */
  status = $state('');
  /** The open workspace (M-21.3), `null` while none is open. */
  workspace = $state<WorkspaceView | null>(null);
  /** M-05.2: the note overview, recently opened first. */
  overview = $state<OverviewItem[]>([]);
  /** M-06: the tab bar. */
  tabs = $state<TabsView>(NO_TABS);
  /**
   * M-01.1, plan section 14: the workspace to start with is being opened;
   * the central area shows neither the empty screen nor a workspace yet.
   */
  starting = $state(false);
  /**
   * M-21.1, M-21.2: the Open or Create dialog, if one is open. Replaced as a
   * whole on every change (`editDialog`), never changed in place, so the
   * dialog components hold no state the window owns.
   */
  dialog = $state.raw<WorkspaceDialog | null>(null);
  /** A dialog waits for the backend's answer. */
  dialogBusy = $state(false);
  /**
   * M-05.1: the overview's width, taken from the settings when the overview
   * first shows and kept while the window lives; `null` before. Reading a
   * settings file again sets it back to the stored width (Q-03).
   */
  overviewWidth = $state<number | null>(null);
  /** M-03.5: the wiki project URL typed in the Workspace menu, not stored. */
  wikiUrl = $state('');
  /** M-03.2: the search overlay (feature F9). */
  searchOpen = $state(false);
  /** M-03.7: the settings window (feature F10). */
  settingsOpen = $state(false);

  #unlisten: UnlistenFn[] = [];
  /**
   * The workspace commands run one after the other, in the order the user
   * gave them, so a tab is never activated before it was opened.
   */
  #queue: Promise<unknown> = Promise.resolve();
  /** The parts sent as events while `ws_start` runs; they are newer. */
  #sentDuringStart: Set<'workspace' | 'overview' | 'tabs'> | null = null;

  /** The root of the open workspace, as the status bar shows it. */
  get workspaceRoot(): string | null {
    return this.workspace?.root ?? null;
  }

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
      events.workspace.listen((event) => {
        this.#sentDuringStart?.add('workspace');
        this.workspace = event.payload;
      }),
      events.overview.listen((event) => {
        this.#sentDuringStart?.add('overview');
        this.overview = event.payload;
      }),
      events.tabs.listen((event) => {
        this.#sentDuringStart?.add('tabs');
        this.tabs = event.payload;
      }),
    ]);
    try {
      const result = await commands.appStart();
      if (result.status === 'ok') {
        this.start = result.data;
        this.status = result.data.status;
        this.#takeSettings(result.data.settings);
        // M-01.1: a workspace to start with is opened next (startWorkspace).
        this.starting = (result.data.givenDir ?? result.data.lastWorkspace) !== null;
      } else {
        this.#fail('app_start', result.error);
      }
    } catch (error) {
      this.#fail('app_start', { kind: 'internal', message: String(error) });
    }
  }

  /**
   * M-01.1, UC-002 A1/A2: opens the workspace given on the command line or
   * opened last. Until the answer comes, `starting` holds; a directory that
   * cannot be opened shows the Open dialog, as in the egui app.
   */
  async startWorkspace(): Promise<void> {
    // Plain, not reactive: only read when the snapshot arrives.
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    this.#sentDuringStart = new Set();
    try {
      const result = await this.#serial(() => commands.wsStart());
      if (result.status === 'ok') {
        this.#takeSnapshot(result.data.snapshot);
        this.#takeOpenAnswer(result.data.answer);
      } else {
        this.#fail('ws_start', result.error);
      }
    } catch (error) {
      this.#fail('ws_start', { kind: 'internal', message: String(error) });
    } finally {
      this.#sentDuringStart = null;
      this.starting = false;
    }
  }

  /** M-02.1, M-03.4: "Open workspace…" shows the Open dialog. */
  async startOpen(): Promise<void> {
    if (this.dialog?.kind === 'open') {
      return;
    }
    // The list comes first, so the keyboard does not move when it arrives.
    let recent: string[] = [];
    const result = await this.#serial(() => commands.wsRecent());
    if (result.status === 'error') {
      this.#fail('ws_recent', result.error);
    } else {
      recent = result.data;
    }
    // The dialog may have opened meanwhile, while the list was asked for.
    const meanwhile = this.dialog as WorkspaceDialog | null;
    if (meanwhile?.kind !== 'open') {
      // The Create dialog closes (M-03.4).
      this.dialog = { kind: 'open', recent, path: '', problem: null, offer: null };
    }
  }

  /** M-02.1, M-03.4: "New workspace…" shows the Create dialog. */
  startCreate(): void {
    if (this.dialog?.kind === 'create') {
      return;
    }
    this.dialog = { kind: 'create', stage: 'directory', path: '', problem: null, offer: null };
  }

  /**
   * Changes the open dialog `dialog` by `change`, as typed by the user; a
   * dialog that was replaced meanwhile stays as it is.
   */
  editDialog<D extends WorkspaceDialog>(dialog: D, change: Partial<D>): void {
    if (this.dialog === dialog) {
      this.dialog = { ...dialog, ...change };
    }
  }

  /** UC-002 A10, UC-001 A4: Cancel closes the dialog; nothing changes. */
  closeDialog(): void {
    this.dialog = null;
  }

  /**
   * M-21.1, UC-002 step 3: opens the directory `path`; with `anyway`, also
   * when it lies inside a workspace (A5 step 2, A6).
   */
  async openWorkspace(path: string, anyway = false): Promise<void> {
    const answer = await this.#ask('ws_open', () => commands.wsOpen(path, anyway));
    if (answer) {
      this.#takeOpenAnswer(answer);
    }
  }

  /** UC-002 A4: "Create a new workspace there" goes to the Create dialog. */
  async createThere(directory: string): Promise<void> {
    this.dialog = null;
    const answer = await this.#ask('ws_check_new', () => commands.wsCheckNew(directory));
    if (answer) {
      this.#takeCreateAnswer(answer, directory, 'Markdown');
    }
  }

  /** UC-001 step 2: "Next" after the directory. */
  async checkNewDirectory(path: string): Promise<void> {
    const answer = await this.#ask('ws_check_new', () => commands.wsCheckNew(path));
    if (answer) {
      this.#takeCreateAnswer(answer, path, 'Markdown');
    }
  }

  /** UC-001 A3 step 1: with the git option, the remote is asked next. */
  askRemote(directory: string, format: FormatName): void {
    this.dialog = { kind: 'create', stage: 'remote', directory, format, address: '' };
  }

  /** UC-001 step 5, A3: "Create". */
  async createWorkspace(
    directory: string,
    format: FormatName,
    git: boolean,
    remote = '',
  ): Promise<void> {
    const answer = await this.#ask('ws_create', () =>
      commands.wsCreate({ directory, format, git, remote }),
    );
    if (answer) {
      this.#takeCreateAnswer(answer, directory, format);
    }
  }

  /** UC-001 A2: "Open it as a workspace instead". */
  async openInstead(directory: string): Promise<void> {
    this.dialog = null;
    await this.openWorkspace(directory);
  }

  /** M-05.2: a click on a note of the overview opens it. */
  async openNote(note: string): Promise<void> {
    const result = await this.#serial(() => commands.tabOpen(note));
    if (result.status === 'error') {
      this.#fail('tab_open', result.error);
    }
  }

  /** M-06.2: a click on a tab activates it. */
  async activateTab(note: string): Promise<void> {
    const result = await this.#serial(() => commands.tabActivate(note));
    if (result.status === 'error') {
      this.#fail('tab_activate', result.error);
    }
  }

  /** M-06.3: "×" or a middle click closes a tab. */
  async closeTab(note: string): Promise<void> {
    const result = await this.#serial(() => commands.tabClose(note));
    if (result.status === 'error') {
      this.#fail('tab_close', result.error);
    }
  }

  /** M-05.1: the overview shows; its width comes from the settings. */
  pinOverviewWidth(): void {
    if (this.overviewWidth === null) {
      this.overviewWidth = overviewWidthWithin(this.settings?.overviewWidth ?? 200);
    }
  }

  /**
   * M-05.1, UC-010 A4: the overview was let go at `shown` pixels; a width
   * more than 1 pixel from the stored one is stored at once.
   */
  async overviewReleased(shown: number): Promise<void> {
    const stored = this.settings?.overviewWidth ?? 200;
    if (Math.abs(Math.round(shown) - stored) > 1) {
      await this.quickSetting('overview_width', String(overviewWidthWithin(shown)));
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
    // Q-03: a settings file read again sets the panels back to the widths
    // in effect, as the egui app does.
    const reread =
      this.settings !== null && settings.widthGeneration !== this.settings.widthGeneration;
    this.settings = settings;
    if (reread && this.overviewWidth !== null) {
      this.overviewWidth = overviewWidthWithin(settings.overviewWidth);
    }
    applyPalette(settings.palette);
  }

  /** Takes the parts of `snapshot` no newer event has replaced yet. */
  #takeSnapshot(snapshot: WorkspaceSnapshot): void {
    const newer = this.#sentDuringStart;
    if (!newer?.has('workspace')) {
      this.workspace = snapshot.workspace;
    }
    if (!newer?.has('overview')) {
      this.overview = snapshot.overview;
    }
    if (!newer?.has('tabs')) {
      this.tabs = snapshot.tabs;
    }
  }

  /** Runs `work` after every command queued before it. */
  #serial<T>(work: () => Promise<T>): Promise<T> {
    const run = this.#queue.then(work, work);
    this.#queue = run.catch(() => undefined);
    return run;
  }

  #takeOpenAnswer(answer: OpenAnswer): void {
    if (answer.kind === 'done') {
      this.dialog = null;
      return;
    }
    this.dialog = {
      kind: 'open',
      recent: answer.recent,
      path: answer.path,
      problem: answer.problem,
      offer: answer.offer,
    };
  }

  #takeCreateAnswer(answer: CreateAnswer, directory: string, format: FormatName): void {
    switch (answer.stage) {
      case 'done':
        this.dialog = null;
        break;
      case 'options':
        this.dialog = {
          kind: 'create',
          stage: 'options',
          directory: answer.directory,
          format,
          git: false,
        };
        break;
      case 'directory':
        this.dialog = {
          kind: 'create',
          stage: 'directory',
          path: answer.path || directory,
          problem: answer.problem,
          offer: answer.offer,
        };
        break;
    }
  }

  /** Asks the backend for a dialog's next step, marking the dialog busy. */
  async #ask<T>(
    command: string,
    call: () => Promise<{ status: 'ok'; data: T } | { status: 'error'; error: UiError }>,
  ): Promise<T | null> {
    this.dialogBusy = true;
    try {
      const result = await this.#serial(call);
      if (result.status === 'error') {
        this.#fail(command, result.error);
        return null;
      }
      return result.data;
    } catch (error) {
      this.#fail(command, { kind: 'internal', message: String(error) });
      return null;
    } finally {
      this.dialogBusy = false;
    }
  }

  /** A failed command: its message is a status text (UiError). */
  #fail(command: string, error: UiError): void {
    console.error(`${command} failed: ${error.message}`);
    this.status = error.message;
  }
}
