// The window's mirror of the backend session: what `app_start` returned,
// kept current by the `status`, `settings`, `workspace`, `overview`,
// `deleted`, `tabs`, `properties`, `indexes` and `close-question` events (ADR-022); the editor's
// `ed-highlight` goes to the source view shown now, `ed-changed`,
// `ed-flush` and `ed-enrich` to the editor cores (ADR-025, `editor/sync.ts`),
// plus the UI state that is not stored (M-22.4): the Open and Create
// dialogs, the note dialogs (M-14), the question before closing, "Open web
// address" (M-07.3), the
// loading state and the panels' widths while they are dragged. The text of the notes is not here: each
// open note's editor core holds it (ADR-025). No domain logic lives here:
// every change goes to the backend, which answers with an event or with the
// dialog's next stage.

import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  commands,
  events,
  type CloseQuestion,
  type CreateAnswer,
  type DeletedNoteView,
  type DeleteQuestion,
  type NewNoteRequest,
  type RestoreAnswer,
  type StatusOffer,
  type EditingViewName,
  type FormatName,
  type IndexesView,
  type OpenAnswer,
  type OpenOffer,
  type OverviewItem,
  type Platform,
  type SettingName,
  type SettingsView,
  type StartInfo,
  type TabPropertiesView,
  type TabsView,
  type UiError,
  type WorkspaceSnapshot,
  type WorkspaceView,
} from './bindings';
import { activeSourceView } from './editor/active';
import { focusNote, jumpTo } from './editor/jump';
import {
  coreHash,
  inStepRevision,
  dropCore,
  enrichCore,
  flushCores,
  flushed,
  keepCoresOf,
  ringCore,
  setSyncReporter,
  EDITOR_FAILED,
  handOverFailed,
  markCores,
  dropCoresBefore,
  keepCores,
  takeFlush,
  holdCore,
  releaseCore,
  renameCore,
} from './editor/sync';
import { applyPalette } from './theme';
import { focusNoteEntry } from './focus';
import { SearchState, type FoundChoice } from './search.svelte';

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

/**
 * M-14.1, UC-003: the dialog "New note". `file` is the file name the note
 * will get, as the backend plans it while the user types (`null` while it
 * cannot be created so).
 */
export type NewNoteDialog = {
  title: string;
  folder: string;
  format: FormatName;
  /** Why the last "Create" was refused, or the link's "… does not exist yet. Create it?". */
  problem: string | null;
  /** A2: the note that exists, for "Open the existing note". */
  existing: string | null;
  file: string | null;
};

/** M-14.2, UC-006: the dialog "Rename note". */
export type RenameDialog = {
  /** The note to rename. */
  note: string;
  title: string;
  folder: string;
  /** BR-009: it is the Home note. */
  home: boolean;
  problem: string | null;
  file: string | null;
};

/** M-14.3, UC-007: the question "Delete note"; `cannotKeep` is A8's second question. */
export type DeleteDialog = DeleteQuestion & { cannotKeep: string | null };

/** M-14.4, UC-007 A6: the dialog "Restore note". */
export type RestoreDialog = { title: string; folder: string; problem: string | null };

/** M-14.4, UC-007 A7: "Remove deleted notes": one by its key, or all (`null`). */
export type PurgeDialog = { key: string | null; count: number };

/** The note dialogs (M-14); several may be open at once, as in egui. */
export type NoteDialogs = {
  newNote: NewNoteDialog | null;
  rename: RenameDialog | null;
  delete: DeleteDialog | null;
  restore: RestoreDialog | null;
  purge: PurgeDialog | null;
};

type NoteDialogKind = keyof NoteDialogs;

const NO_NOTE_DIALOGS: NoteDialogs = {
  newNote: null,
  rename: null,
  delete: null,
  restore: null,
  purge: null,
};

/** How long "File: …" waits after the last key before it is planned. */
const PLAN_DELAY_MS = 150;

type PlanState = { timer: ReturnType<typeof setTimeout> | null; running: boolean; again: boolean };

/** What "New note" sends. */
function newNoteRequest(dialog: NewNoteDialog): NewNoteRequest {
  return { title: dialog.title, folder: dialog.folder, format: dialog.format };
}

/** Whether both hold the same title, folder and format. */
function sameNewNote(a: NewNoteDialog, b: NewNoteDialog): boolean {
  return a.title === b.title && a.folder === b.folder && a.format === b.format;
}

/** M-05.1: the note overview is 100 to 1000 pixels wide. */
export const OVERVIEW_WIDTH = { min: 100, max: 1000 } as const;

/** `width` within the overview's range, rounded to a pixel. */
export function overviewWidthWithin(width: number): number {
  return Math.min(OVERVIEW_WIDTH.max, Math.max(OVERVIEW_WIDTH.min, Math.round(width)));
}

/** M-16.1: the tool panel is 100 to 1000 pixels wide, like the overview. */
export const TOOL_PANEL_WIDTH = OVERVIEW_WIDTH;

/** `width` within the tool panel's range, rounded to a pixel. */
export function toolPanelWidthWithin(width: number): number {
  return Math.min(TOOL_PANEL_WIDTH.max, Math.max(TOOL_PANEL_WIDTH.min, Math.round(width)));
}

const NO_TABS: TabsView = { tabs: [], active: null, todoList: { open: false, active: false } };

export class Shell {
  /** What the window started with; `null` when `app_start` failed. */
  start = $state<StartInfo | null>(null);
  /** The settings in effect, with the theme's palette. */
  settings = $state<SettingsView | null>(null);
  /** M-04.7: the status message. */
  status = $state('');
  /** UC-007 A4: what the status bar offers with the status message ("Undo"). */
  statusOffer = $state<StatusOffer | null>(null);
  /** M-05.6, UC-007 A5: the deleted notes, most recently deleted first. */
  deleted = $state.raw<DeletedNoteView[]>([]);
  /**
   * M-14: the note dialogs, each replaced as a whole on every change
   * (`editNoteDialog`), like `dialog`.
   */
  notes = $state.raw<NoteDialogs>(NO_NOTE_DIALOGS);
  /** The note dialogs that wait for the backend's answer. */
  notesBusy = $state.raw<Partial<Record<NoteDialogKind, boolean>>>({});
  /** The open workspace (M-21.3), `null` while none is open. */
  workspace = $state<WorkspaceView | null>(null);
  /** M-05.2: the note overview, recently opened first. */
  overview = $state<OverviewItem[]>([]);
  /** Q-40: notes whose editor failed and lost the latest edits, until the user acts there again. */
  editorLost = $state<string[]>([]);
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
  /**
   * M-16.1: the tool panel's width, like `overviewWidth`: taken from the
   * settings when the panel first shows, set back when a settings file is
   * read again.
   */
  toolPanelWidth = $state<number | null>(null);
  /** M-16.2: the properties of the active tab's note, `null` without one. */
  properties = $state<TabPropertiesView | null>(null);
  /**
   * M-16.5: the outline, code and figures the backend made last, of the
   * note then active; the tool panel shows them while that note is.
   */
  indexes = $state.raw<IndexesView | null>(null);
  /** M-07.3, UC-004 A4: the web address "Open web address" asks about. */
  webQuestion = $state<string | null>(null);
  /**
   * M-06.3, M-21.4: "Changes not stored": closing a tab or Underline could
   * not store its changes, and the user decides.
   */
  closeQuestion = $state<CloseQuestion | null>(null);
  /** M-03.5: the wiki project URL typed in the Workspace menu, not stored. */
  wikiUrl = $state('');
  /**
   * M-03.2, M-13: the search overlay and the find field of the active note
   * (UC-008), not stored (M-22.4).
   */
  readonly search = new SearchState((command, message) =>
    this.#fail(command, { kind: 'internal', message }),
  );
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
    // ADR-025: what the sync of the editor cores has to say.
    setSyncReporter({
      status: (note, text) => {
        const title = this.tabs.tabs.find((tab) => tab.note === note)?.title ?? note;
        this.status = `${title}: ${text}`;
      },
      closeQuestion: (question) => {
        this.closeQuestion = question;
      },
      dead: (note) => {
        const title = this.tabs.tabs.find((tab) => tab.note === note)?.title ?? note;
        this.status = `${title}: ${EDITOR_FAILED}`;
        if (!this.editorLost.includes(note)) {
          this.editorLost = [...this.editorLost, note];
        }
      },
    });
    this.#unlisten = await Promise.all([
      events.status.listen((event) => {
        this.status = event.payload.text;
        this.statusOffer = event.payload.offer;
      }),
      events.deleted.listen((event) => {
        this.deleted = event.payload;
      }),
      events.settings.listen((event) => {
        this.#takeSettings(event.payload);
      }),
      events.workspace.listen((event) => {
        this.#sentDuringStart?.add('workspace');
        if (event.payload?.root !== this.workspace?.root) {
          // The note dialogs were of the workspace left.
          this.notes = NO_NOTE_DIALOGS;
          // M-21.3: the find field closes; the search starts anew there.
          this.search.workspaceChanged();
        }
        this.workspace = event.payload;
        // A note of the same name in another workspace is another text.
        this.indexes = null;
      }),
      events.overview.listen((event) => {
        this.#sentDuringStart?.add('overview');
        this.overview = event.payload;
      }),
      events.tabs.listen((event) => {
        this.#sentDuringStart?.add('tabs');
        this.tabs = event.payload;
        // ADR-025: a note the backend closed keeps no editor core.
        keepCoresOf(event.payload.tabs.map((tab) => tab.note));
      }),
      events.properties.listen((event) => {
        this.properties = event.payload;
      }),
      events.indexes.listen((event) => {
        this.indexes = event.payload;
      }),
      events.closeQuestion.listen((event) => {
        this.closeQuestion = event.payload;
      }),
      // M-13.1, UC-008 step 2: the found notes of the search.
      events.search.listen((event) => {
        this.search.take(event.payload);
      }),
      // The editor's events are listened to here, once for the window, and
      // handed to the source view shown now: listening per view left events
      // without a listener after a tab switch.
      events.edHighlight.listen((event) => {
        activeSourceView()?.takeColors(event.payload);
      }),
      // ADR-025: the backend changed a text a window's core holds, or asks
      // for the window's edits (before an agent's change, before closing).
      events.edChanged.listen((event) => {
        ringCore(event.payload.note);
      }),
      events.edEnrich.listen((event) => {
        enrichCore(event.payload);
      }),
      events.edFlush.listen((event) => {
        void takeFlush(event.payload).catch((error: unknown) =>
          this.#fail('ed_sync', { kind: 'internal', message: String(error) }),
        );
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
    let mark = 0;
    const answer = await this.#ask('ws_open', () =>
      flushed(() => {
        mark = markCores();
        return commands.wsOpen(path, anyway);
      }),
    );
    if (answer?.kind === 'done') {
      // ADR-025: the cores of the notes of the workspace left go; the new
      // one's may have opened before this answer came.
      dropCoresBefore(mark);
    } else if (mark !== 0) {
      keepCores();
    }
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
    let mark = 0;
    const answer = await this.#ask('ws_create', () =>
      flushed(() => {
        mark = markCores();
        return commands.wsCreate({ directory, format, git, remote });
      }),
    );
    if (answer?.stage === 'done') {
      dropCoresBefore(mark);
    } else if (mark !== 0) {
      keepCores();
    }
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

  /**
   * M-06.3: "×" or a middle click closes a tab, after storing it; when that
   * fails, "Changes not stored" asks first (UC-005 A14 step 3). With
   * `force` ("Close anyway") it closes all the same. Resolves to whether
   * the tab closed.
   */
  async closeTab(note: string, force = false): Promise<boolean> {
    if (!force) {
      // ADR-025: the tab's edits reach the backend first; when they cannot,
      // "Changes not stored" asks, as when storing fails (UC-005 A14).
      const failure = await flushCores(note);
      if (failure !== null) {
        const title = this.tabs.tabs.find((tab) => tab.note === note)?.title ?? note;
        this.closeQuestion = {
          tab: note,
          what: `the tab ${title}`,
          error: handOverFailed(failure),
        };
        return false;
      }
    }
    const result = await this.#serial(() => commands.tabClose(note, force));
    if (result.status === 'error') {
      this.#fail('tab_close', result.error);
      return false;
    }
    this.closeQuestion = result.data;
    if (result.data === null) {
      dropCore(note);
    }
    return result.data === null;
  }

  /**
   * M-07.2: the switch "Facade"/"Source": the tab of `note` shows `view`
   * from now on, remembered with the tabs. The tab bar shows it at once;
   * the backend's `tabs` event confirms it.
   */
  async setView(note: string, view: EditingViewName): Promise<void> {
    this.tabs = {
      ...this.tabs,
      tabs: this.tabs.tabs.map((tab) => (tab.note === note ? { ...tab, view } : tab)),
    };
    // ADR-025: the other view starts from what the backend has.
    try {
      const result = await this.#serial(() => flushed(() => commands.tabSetView(note, view)));
      if (result.status === 'error') {
        this.#fail('tab_set_view', result.error);
      }
    } catch (error) {
      // The IPC itself failed, as when the window is going (a test ended).
      this.#fail('tab_set_view', { kind: 'internal', message: String(error) });
    }
  }

  /**
   * M-07.3, UC-004 A2 to A4: follows the link at `at` (web units) of the
   * note `note`, once the window's edits reached the backend (ADR-025),
   * with the revision they gave; the backend finds the link there. When
   * the text changed after the click, nothing is followed. A heading is
   * jumped to, possibly in a tab that opens; a web address may be asked
   * about first. What else happened, the status bar says.
   */
  async followLink(note: string, at: number): Promise<void> {
    const clicked = coreHash(note);
    if (clicked === null) {
      return;
    }
    const result = await this.#serial(() =>
      flushed(async () => {
        const rev = inStepRevision(note);
        if (rev === null || coreHash(note) !== clicked) {
          return { status: 'ok' as const, data: { kind: 'done' as const } };
        }
        return commands.edFollowLink(note, at, rev);
      }),
    );
    if (result.status === 'error') {
      this.#fail('ed_follow_link', result.error);
      return;
    }
    const answer = result.data;
    switch (answer.kind) {
      case 'jump':
        jumpTo(answer.note, answer.rev, answer.offset);
        break;
      case 'askWeb':
        this.webQuestion = answer.address;
        break;
      case 'newNote':
        // UC-004 A3: "New note" offers the missing note, with the link's
        // folder and title and egui's question.
        this.#showNewNote({
          title: answer.title,
          folder: answer.folder,
          format: answer.format,
          problem: answer.problem,
          existing: null,
          file: null,
        });
        break;
      case 'done':
        break;
    }
  }

  /**
   * M-13.3, UC-008 step 4: opens a found note as the overview does, once the
   * window's edits reached the backend (ADR-025): at the word found, which
   * is selected, or as it is when it was found by its title or folder; the
   * note's view takes the keyboard. Enter closes the overlay too (`close`),
   * a click keeps it open, as in egui.
   */
  async openFound(choice: FoundChoice, close = false): Promise<void> {
    if (close) {
      // The found note's view takes the keyboard instead.
      this.search.close(false);
    }
    const result = await this.#serial(() =>
      flushed(() => commands.searchOpen(choice.note, choice.place)),
    );
    if (result.status === 'error') {
      this.#fail('search_open', result.error);
      return;
    }
    if (result.data) {
      jumpTo(result.data.note, result.data.rev, result.data.anchor, result.data.head);
    } else if (choice.place === null) {
      focusNote(choice.note);
    }
  }

  /**
   * M-14.1, UC-003 step 2: "New note…" shows the dialog, with the folder of
   * the active note and the default format; one open already starts anew,
   * as in egui.
   */
  async startNewNote(): Promise<void> {
    const result = await commands.noteNewStart();
    if (result.status === 'error') {
      this.#fail('note_new_start', result.error);
      return;
    }
    if (result.data === null) {
      return;
    }
    this.#showNewNote({
      title: '',
      folder: result.data.folder,
      format: result.data.format,
      problem: null,
      existing: null,
      file: null,
    });
  }

  #showNewNote(dialog: NewNoteDialog): void {
    this.notes = { ...this.notes, newNote: dialog };
    this.#schedulePlan('newNote', 0);
  }

  /** The planning of "File: …" per dialog: waiting, running, to run again. */
  #plans: Record<'newNote' | 'rename', PlanState> = {
    newNote: { timer: null, running: false, again: false },
    rename: { timer: null, running: false, again: false },
  };

  /**
   * "File: …" is planned `delay` ms after the last edit (it may run git in
   * the backend), never twice at once (review F8).
   */
  #schedulePlan(kind: 'newNote' | 'rename', delay = PLAN_DELAY_MS): void {
    const plan = this.#plans[kind];
    if (plan.timer !== null) {
      clearTimeout(plan.timer);
    }
    plan.timer = setTimeout(() => {
      plan.timer = null;
      void this.#runPlan(kind);
    }, delay);
  }

  async #runPlan(kind: 'newNote' | 'rename'): Promise<void> {
    const plan = this.#plans[kind];
    if (plan.running) {
      plan.again = true;
      return;
    }
    plan.running = true;
    try {
      await (kind === 'newNote' ? this.#planNewNote() : this.#planRename());
    } finally {
      plan.running = false;
      if (plan.again) {
        plan.again = false;
        void this.#runPlan(kind);
      }
    }
  }

  /**
   * Changes the note dialog `kind` by `change`, as typed by the user; a
   * dialog that was replaced or closed meanwhile stays as it is. "New note"
   * and "Rename note" plan their file name again.
   */
  editNoteDialog<K extends NoteDialogKind>(
    kind: K,
    dialog: NonNullable<NoteDialogs[K]>,
    change: Partial<NonNullable<NoteDialogs[K]>>,
  ): void {
    if (this.notes[kind] !== dialog) {
      return;
    }
    // A problem was about what the fields held; a change of them ends it
    // (UX review F8).
    const typed = 'title' in change || 'folder' in change || 'format' in change;
    const cleared = typed
      ? { problem: null, ...(kind === 'newNote' ? { existing: null } : {}) }
      : {};
    const next = { ...dialog, ...cleared, ...change } as NonNullable<NoteDialogs[K]>;
    this.notes = { ...this.notes, [kind]: next };
    if (kind === 'newNote' || kind === 'rename') {
      this.#schedulePlan(kind);
    }
  }

  /** Closes the note dialog `kind`; nothing changes (UC-003 A6, UC-006 A8, UC-007 A1). */
  closeNoteDialog(kind: NoteDialogKind): void {
    this.notes = { ...this.notes, [kind]: null };
  }

  /** The dialog `kind` waits for the backend. */
  noteBusy(kind: NoteDialogKind): boolean {
    return this.notesBusy[kind] === true;
  }

  /** UC-003 step 2: "File: {name}" for what the dialog holds now. */
  async #planNewNote(): Promise<void> {
    const dialog = this.notes.newNote;
    if (dialog === null) {
      return;
    }
    const result = await commands.notePlan(newNoteRequest(dialog));
    const file = result.status === 'ok' ? result.data : null;
    const now = this.notes.newNote;
    if (now !== null && sameNewNote(now, dialog)) {
      this.notes = { ...this.notes, newNote: { ...now, file } };
    }
  }

  /**
   * M-14.1, UC-003 steps 4–6: "Create": the note opens with the cursor at
   * its end and the keyboard in it; a problem keeps the dialog.
   */
  async createNote(): Promise<void> {
    const dialog = this.notes.newNote;
    if (dialog === null) {
      return;
    }
    const answer = await this.#askNote('newNote', 'note_create', () =>
      commands.noteCreate(newNoteRequest(dialog)),
    );
    if (answer === null) {
      return;
    }
    if (answer.kind === 'problem') {
      this.editNoteDialog('newNote', dialog, {
        problem: answer.problem,
        existing: answer.existing,
      });
      return;
    }
    this.closeNoteDialog('newNote');
    if (answer.kind === 'created') {
      jumpTo(answer.note, answer.rev, answer.offset);
    }
  }

  /** M-14.1, UC-003 A3: "Open the existing note" opens it unchanged. */
  async openExistingNote(note: string): Promise<void> {
    this.closeNoteDialog('newNote');
    await this.openNote(note);
  }

  /**
   * M-14.2, UC-006 step 1: "Rename…" or F2 shows "Rename note" for `note`,
   * its title selected; refused in the status bar for a note in conflict.
   * While one is open, another is not started (egui).
   */
  async startRename(note: string): Promise<void> {
    if (this.notes.rename !== null) {
      return;
    }
    const result = await this.#serial(() => commands.noteRenameStart(note));
    if (result.status === 'error') {
      this.#fail('note_rename_start', result.error);
      return;
    }
    if (result.data === null || this.notes.rename !== null) {
      return;
    }
    const dialog: RenameDialog = { ...result.data, problem: null, file: null };
    this.notes = { ...this.notes, rename: dialog };
    this.#schedulePlan('rename', 0);
  }

  /** UC-006 step 2: "File: …" for what the dialog holds now. */
  async #planRename(): Promise<void> {
    const dialog = this.notes.rename;
    if (dialog === null) {
      return;
    }
    const result = await commands.notePlanRename(dialog.note, dialog.title, dialog.folder);
    const file = result.status === 'ok' ? result.data : null;
    const now = this.notes.rename;
    if (
      now !== null &&
      now.note === dialog.note &&
      now.title === dialog.title &&
      now.folder === dialog.folder
    ) {
      this.notes = { ...this.notes, rename: { ...now, file } };
    }
  }

  /**
   * M-14.2, UC-006 steps 4–7: "Rename", once the window's edits reached the
   * backend (ADR-025). The tab's editor core goes on under the new name.
   */
  async renameNote(): Promise<void> {
    const dialog = this.notes.rename;
    if (dialog === null) {
      return;
    }
    holdCore(dialog.note);
    try {
      const answer = await this.#askNote('rename', 'note_rename', () =>
        flushed(() => commands.noteRename(dialog.note, dialog.title, dialog.folder)),
      );
      if (answer?.kind === 'problem') {
        this.editNoteDialog('rename', dialog, { problem: answer.problem });
        return;
      }
      if (answer?.kind === 'renamed') {
        renameCore(answer.from, answer.to);
      }
      if (answer !== null) {
        this.closeNoteDialog('rename');
      }
    } finally {
      releaseCore(dialog.note);
    }
  }

  /**
   * M-14.3, UC-007 steps 1–2: "Delete…" or the Delete key asks first;
   * refused in the status bar for a note in conflict. While the question
   * is open, another is not asked (egui).
   */
  async startDelete(note: string): Promise<void> {
    if (this.notes.delete !== null) {
      return;
    }
    const result = await this.#serial(() => commands.noteDeleteStart(note));
    if (result.status === 'error') {
      this.#fail('note_delete_start', result.error);
      return;
    }
    if (result.data === null || this.notes.delete !== null) {
      return;
    }
    this.notes = { ...this.notes, delete: { ...result.data, cannotKeep: null } };
  }

  /**
   * M-14.3, UC-007 steps 3–5, A8: "Delete" (or "Delete for good"), once the
   * window's edits reached the backend (ADR-025).
   */
  async deleteNote(): Promise<void> {
    const dialog = this.notes.delete;
    if (dialog === null) {
      return;
    }
    const answer = await this.#askNote('delete', 'note_delete', () =>
      flushed(() => commands.noteDelete(dialog.note, dialog.cannotKeep !== null)),
    );
    if (answer?.kind === 'cannotKeep') {
      this.editNoteDialog('delete', dialog, { cannotKeep: answer.question });
    } else if (answer !== null) {
      this.closeNoteDialog('delete');
    }
  }

  /** M-14.3, UC-007 A4: "Undo" in the status bar. */
  async undoDelete(): Promise<void> {
    const answer = await this.#askNote('restore', 'note_undo_delete', () =>
      commands.noteUndoDelete(),
    );
    const restored = this.#takeRestoreAnswer(answer);
    if (restored !== null) {
      // UX review F8: the keyboard goes to the note that came back.
      await focusNoteEntry(restored);
    }
  }

  /** M-05.6, UC-007 A5: "Restore" of a deleted note. */
  async restoreDeleted(key: string): Promise<void> {
    const answer = await this.#askNote('restore', 'ws_restore', () => commands.wsRestore(key));
    this.#takeRestoreAnswer(answer);
  }

  /** M-14.4, UC-007 A6: "Restore" in "Restore note". */
  async restoreAs(): Promise<void> {
    const dialog = this.notes.restore;
    if (dialog === null) {
      return;
    }
    const answer = await this.#askNote('restore', 'ws_restore_as', () =>
      commands.wsRestoreAs(dialog.title, dialog.folder),
    );
    this.#takeRestoreAnswer(answer);
  }

  /** Shows "Restore note" or closes it; the note restored, if one was. */
  #takeRestoreAnswer(answer: RestoreAnswer | null): string | null {
    if (answer?.kind === 'ask') {
      this.notes = {
        ...this.notes,
        restore: { title: answer.title, folder: answer.folder, problem: answer.problem },
      };
    } else if (answer !== null) {
      this.closeNoteDialog('restore');
    }
    return answer?.kind === 'restored' ? answer.note : null;
  }

  /** M-05.6, UC-007 A7: "Remove" of one deleted note, or "Empty the deleted notes" (`null`). */
  askPurge(key: string | null): void {
    // UX review F8: the number asked about stays while the question shows.
    const count = key === null ? this.deleted.length : 1;
    this.notes = { ...this.notes, purge: { key, count } };
  }

  /** M-14.4, UC-007 A7: "Remove" in "Remove deleted notes". */
  async purge(): Promise<void> {
    const dialog = this.notes.purge;
    if (dialog === null) {
      return;
    }
    // The question closes in any case, as in egui; a failure is in the status.
    await this.#askNote('purge', 'ws_purge', () => commands.wsPurge(dialog.key));
    if (this.notes.purge === dialog) {
      this.closeNoteDialog('purge');
    }
  }

  /** Asks the backend for a note dialog's next step, marking the dialog busy. */
  async #askNote<T>(
    kind: NoteDialogKind,
    command: string,
    call: () => Promise<{ status: 'ok'; data: T } | { status: 'error'; error: UiError }>,
  ): Promise<T | null> {
    this.notesBusy = { ...this.notesBusy, [kind]: true };
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
      this.notesBusy = { ...this.notesBusy, [kind]: false };
    }
  }

  /** M-07.3, UC-004 A4: "Open" or "Cancel" of "Open web address". */
  async answerWeb(open: boolean): Promise<void> {
    this.webQuestion = null;
    const result = await this.#serial(() => commands.linkOpenWeb(open));
    if (result.status === 'error') {
      this.#fail('link_open_web', result.error);
    }
  }

  /** M-10.9: `Store` stores the note of the tab at once. */
  async storeTab(note: string): Promise<void> {
    const result = await this.#serial(() => flushed(() => commands.tabStore(note)));
    if (result.status === 'error') {
      this.#fail('tab_store', result.error);
    }
  }

  /** M-21.4: "Keep open": nothing closes. */
  keepOpen(): void {
    this.closeQuestion = null;
  }

  /** M-06.3, M-21.4: "Close anyway": the tab or Underline closes without the changes. */
  async closeAnyway(): Promise<void> {
    const question = this.closeQuestion;
    this.closeQuestion = null;
    if (question?.tab) {
      await this.closeTab(question.tab, true);
    } else if (question) {
      const result = await commands.appCloseAnyway();
      if (result.status === 'error') {
        this.#fail('app_close_anyway', result.error);
      }
    }
  }

  /** M-16.1: the tool panel shows; its width comes from the settings. */
  pinToolPanelWidth(): void {
    if (this.toolPanelWidth === null) {
      this.toolPanelWidth = toolPanelWidthWithin(this.settings?.toolPanelWidth ?? 240);
    }
  }

  /**
   * M-16.1, UC-010 A4: the tool panel was let go at `shown` pixels; as in
   * egui, a width more than 1 pixel from the stored one is stored at once,
   * but only while a note tab is active.
   */
  async toolPanelReleased(shown: number): Promise<void> {
    const stored = this.settings?.toolPanelWidth ?? 240;
    if (this.tabs.active !== null && Math.abs(Math.round(shown) - stored) > 1) {
      await this.quickSetting('tool_panel_width', String(toolPanelWidthWithin(shown)));
    }
  }

  /** Reports a failure of the editor in the status bar, as other commands do. */
  editorFailed(message: string): void {
    this.status = message;
  }

  /** Q-40: the user acted in `note` again; the note on its lost edits goes. */
  editorLostSeen(note: string): void {
    if (this.editorLost.includes(note)) {
      this.editorLost = this.editorLost.filter((lost) => lost !== note);
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
    if (reread && this.toolPanelWidth !== null) {
      this.toolPanelWidth = toolPanelWidthWithin(settings.toolPanelWidth);
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
