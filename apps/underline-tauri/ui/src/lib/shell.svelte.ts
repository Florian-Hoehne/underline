// The window's mirror of the backend session: what `app_start` returned,
// kept current by the `status`, `settings`, `workspace`, `overview`,
// `deleted`, `tabs`, `properties`, `indexes`, `changes`, `diff`, `git-bar`,
// `conflict`, `todo-conflicts`, `todos` and `close-question` events (ADR-022); the editor's
// `ed-highlight` goes to the source view shown now, `ed-changed`,
// `ed-flush` and `ed-enrich` to the editor cores (ADR-025, `editor/sync.ts`),
// plus the UI state that is not stored (M-22.4): the Open and Create
// dialogs, the note dialogs (M-14), the question before closing, "Open web
// address" (M-07.3), the commit box with "Your name for commits" and
// "Discard changes" (M-19), the branch menu, "Abort the sync" and the
// place chosen in the conflict bar (M-19.6 to M-19.10), the todo shown with
// its fields, the row chosen in the tab "Todos", the title typed for a new
// todo and the calendar "Choose a day" (M-18), the
// loading state and the panels' widths while they are dragged. The text of the notes is not here: each
// open note's editor core holds it (ADR-025). No domain logic lives here:
// every change goes to the backend, which answers with an event or with the
// dialog's next stage.

import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  commands,
  events,
  type AgentAction,
  type AgentLevelName,
  type AgentMarkView,
  type AgentMarksView,
  type AgentOpenView,
  type AgentsSectionView,
  type AgentsView,
  type BranchEntryView,
  type BranchMenuView,
  type BranchNameView,
  type BranchTarget,
  type ChangesView,
  type ChoiceName,
  type CloseQuestion,
  type ConflictBarView,
  type GitBarView,
  type SideName,
  type SyncModeName,
  type TodoConflictView,
  type CalendarView,
  type TodoAnswer,
  type TodoCandidatesView,
  type TodoChange,
  type TodoFilterName,
  type TodoSortName,
  type TodosView,
  type CommitAnswer,
  type CreateAnswer,
  type DiffView,
  type DiscardQuestion,
  type DeletedNoteView,
  type DeleteQuestion,
  type ExportChoice,
  type ExportKindName,
  type ExportOfferView,
  type ExportShown,
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
  type Scope,
  type SettingName,
  type SettingsView,
  type SettingsWindowView,
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

/**
 * M-15.1, M-15.2: the window "Export note" (UC-009 steps 1–3): the kinds to
 * choose from with the keyboard (BR-008), the system's file dialog being
 * open (nothing shows meanwhile, as in egui), or the path field when no
 * dialog could be shown (ADR-016).
 */
export type ExportDialog =
  | {
      stage: 'choose';
      note: string;
      title: string;
      choices: ExportChoice[];
      /** The kind chosen with Up and Down, from 0. */
      chosen: number;
    }
  | { stage: 'system'; note: string; kind: ExportKindName }
  | {
      stage: 'path';
      note: string;
      kind: ExportKindName;
      /** "Export {t} as {label} to:". */
      heading: string;
      path: string;
      /** "The file exists. Export again to replace it." */
      problem: string | null;
      /** The path the user may replace with the next "Replace" (D-03: that path only). */
      replace: string | null;
    };

/**
 * M-19.1, UC-012 steps 2–3: the commit box. The message follows the
 * proposal (`ChangesView.proposed`) until the user types (`typed`); it is
 * kept until a commit succeeds (BR-003). `problem` is why the last commit
 * was refused (A1, A2); `busy` while a commit runs.
 */
export type CommitBox = {
  message: string;
  typed: boolean;
  amend: boolean;
  problem: string | null;
  busy: boolean;
};

const EMPTY_COMMIT: CommitBox = {
  message: '',
  typed: false,
  amend: false,
  problem: null,
  busy: false,
};

/**
 * M-19.4, UC-012 A3: "Your name for commits", with why git could not keep
 * the name (review F12; the dialog stays open with what was typed).
 */
export type IdentityDialog = { name: string; email: string; problem: string | null };

/**
 * M-19.6, UC-013: the branch menu. `view` is `null` while git lists the
 * branches. `naming` holds the name of a new branch while it is asked for
 * (A1), with `name` (what git makes of it) once checked. `problem` is A2 or
 * git's reason, `inTheWay` A3.
 */
export type BranchMenu = {
  view: BranchMenuView | null;
  filter: string;
  cursor: number;
  naming: string | null;
  name: BranchNameView | null;
  problem: string | null;
  inTheWay: string | null;
  busy: boolean;
};

/** No git part in the status bar (no workspace, no git). */
const NO_GIT_BAR: GitBarView = { branch: null, problem: null, conflicts: null, sync: null };

/** M-19.6, BR-001: the entries holding the filter's text, ignoring case. */
export function filteredBranches(menu: BranchMenu): BranchEntryView[] {
  const filter = menu.filter.toLowerCase();
  return (menu.view?.branches ?? []).filter((entry) => entry.name.toLowerCase().includes(filter));
}

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

/**
 * M-17.2: the agents before the backend said anything: no workspace, the
 * head comes with `app_start`.
 */
const NO_AGENTS: AgentsView = { menu: null, head: [], workspace: false };

/** M-18.3: the calendar "Choose a day", for which field of which todo, and the month shown. */
export type TodoCalendar = { field: 'due' | 'scheduled'; todo: number; month: CalendarView };

/** M-17.6: no agent cursor in a note. */
const NO_MARKS: AgentMarkView[] = [];

export class Shell {
  /** What the window started with; `null` when `app_start` failed. */
  start = $state<StartInfo | null>(null);
  /** The settings in effect, with the theme's palette. */
  settings = $state<SettingsView | null>(null);
  /** M-04.7: the status message. */
  status = $state('');
  /** UC-007 A4: what the status bar offers with the status message ("Undo"). */
  statusOffer = $state<StatusOffer | null>(null);
  /** M-04.8: "Open" and "Show" for each export the status reports. */
  statusExports = $state.raw<ExportOfferView[]>([]);
  /** UC-009 A4: the status reports a failed export. */
  statusError = $state(false);
  /** M-04.9: "Open" for the notes an agent changed that the status reports. */
  statusAgents = $state.raw<AgentOpenView[]>([]);
  /** Review F11: an export is running. */
  statusBusy = $state(false);
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
  /**
   * M-19.1, M-19.2 (UC-011, UC-012): the git changes of the workspace, as
   * the backend sent them last; `null` without git or before git answered.
   */
  changes = $state.raw<ChangesView | null>(null);
  /** M-19.3: the diff of the file chosen in the list or of the active note. */
  diff = $state.raw<DiffView | null>(null);
  /** M-19.1: the commit box, kept while the user looks at other notes (BR-003). */
  commit = $state.raw<CommitBox>(EMPTY_COMMIT);
  /**
   * M-19.2, UC-011 BR-007: Ctrl+Shift+G (or "Show changes", F13) asked to
   * open the changes; grows each time, so the tool panel opens the list
   * and puts the keyboard into the message.
   */
  changesAsked = $state(0);
  /** M-19.4, UC-012 A3: "Your name for commits", while open. */
  identity = $state.raw<IdentityDialog | null>(null);
  /** UX review F12: grows after each commit; the tool panel puts the keyboard on "Changes (N)". */
  committedCount = $state(0);
  /** M-19.5, UC-011 A6: "Discard changes", while open. */
  discarding = $state.raw<DiscardQuestion | null>(null);
  /** "Discard changes" waits for the backend. */
  discardBusy = $state(false);
  /** M-04.1 to M-04.3: the branch, the conflicts and the syncs of the status bar. */
  gitBar = $state.raw<GitBarView>(NO_GIT_BAR);
  /** M-19.6, UC-013: the branch menu, while open. */
  branchMenu = $state.raw<BranchMenu | null>(null);
  /** M-19.9, UC-015 step 3: the conflict bar of the active note, if it holds places. */
  conflictBar = $state.raw<ConflictBarView | null>(null);
  /** M-19.9: the place the conflict bar shows (egui's `conflict_cursor`). */
  conflictPlace = $state(0);
  /** M-19.10, UC-020 A10: the todos in conflict. */
  todoConflicts = $state.raw<TodoConflictView[]>([]);
  /**
   * M-16.3, M-18, M-04.5 (UC-020, UC-021): the todos of the open workspace
   * as the backend sent them last; `null` before they were read.
   */
  todos = $state.raw<TodosView | null>(null);
  /** M-18.3: the todo shown with its fields in the section "Todos" (egui's `open`). */
  todoShown = $state<number | null>(null);
  /** M-18.3: why a field of the todo shown was not taken (egui's `edit.problem`). */
  todoProblem = $state<string | null>(null);
  /** UX review F15: the kind of change `todoProblem` is about (its field is marked invalid). */
  todoProblemField = $state<TodoChange['kind'] | null>(null);
  /** M-18.1, M-18.6: the title typed for a new todo, shared by both fields as in egui. */
  todoNewTitle = $state('');
  /** UC-020 A3: why the new todo was not created. */
  todoNewProblem = $state<string | null>(null);
  /** M-18.7 (UC-021 BR-008): the row chosen in the tab "Todos". */
  todoChosen = $state<number | null>(null);
  /** M-18.3: the calendar "Choose a day", while open. */
  todoCalendar = $state.raw<TodoCalendar | null>(null);
  /** M-18.3: the day chosen in the calendar last; the field of that todo shows it. */
  todoDayPicked = $state.raw<{ todo: number; field: 'due' | 'scheduled'; day: string } | null>(
    null,
  );
  /** Grows when a todo is shown from the tab "Todos"; the section scrolls to it. */
  todoShownAsked = $state(0);
  /**
   * M-04.4, M-17.1, M-17.2 (UC-017): the agents entry of the status bar and
   * the head of the section "Agents" of the settings.
   */
  agents = $state.raw<AgentsView>(NO_AGENTS);
  /** M-17.6: the agent cursors in the open notes. */
  agentMarks = $state.raw<AgentMarksView[]>([]);
  /**
   * UX review F14: what screen readers are told politely of the agents:
   * "{agent} is editing {title}", "{agent} is reading {title}" when a
   * cursor appears, "{names} asked for access" (UC-017 A2).
   */
  agentAnnouncement = $state('');
  /**
   * M-17.2 to M-17.5 (UC-016): the tools, question and report of the
   * section "Agents", as checked last; `null` until the settings asked.
   */
  agentsSection = $state.raw<AgentsSectionView | null>(null);
  /** The section "Agents" waits for the tools' configurations. */
  agentsSectionBusy = $state(false);
  /** M-04.2, M-19.10, UC-015 A5: "Abort the sync" asks, while open. */
  abortAsked = $state(false);
  /** "Abort the sync" waits for git. */
  abortBusy = $state(false);
  /** M-15.1, M-15.2: the window "Export note", or the system's dialog, while open. */
  exporting = $state.raw<ExportDialog | null>(null);
  /** M-15.6, UC-009 A2: what the window "Approximated blocks" lists, until it is closed. */
  approximated = $state.raw<ExportShown | null>(null);
  /** Grows when the open "Export note" is asked for again; it takes the keyboard. */
  exportAskedAgain = $state(0);
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
  /** M-03.7, M-20.1: the settings window is open (UC-010 step 1). */
  settingsOpen = $state(false);
  /**
   * M-20.2 to M-20.4: what the settings window shows, as the backend
   * answered or sent last; `null` until it was asked.
   */
  settingsWindow = $state.raw<SettingsWindowView | null>(null);
  /**
   * M-20.2: the part of the settings window shown, "For me" or "This
   * workspace"; kept while Underline runs, as in egui, and "For me" while
   * no workspace is open (UC-010 A9).
   */
  settingsScope = $state<Scope>('me');
  /**
   * Why the settings window could not be filled, or what went wrong with the
   * last change made in it (UC-010 A6, A7, in the status bar's words);
   * cleared by the next change that applies and is stored.
   */
  settingsProblem = $state<string | null>(null);
  /**
   * The settings window's work before it closes: a number typed but not
   * sent yet goes to the backend (ux review F10).
   */
  settingsBeforeClose: (() => void) | null = null;

  #unlisten: UnlistenFn[] = [];
  /** The number of the last call of `agentsAct`. */
  #agentsAsked = 0;
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
        this.statusExports = event.payload.exports;
        this.statusError = event.payload.error;
        this.statusBusy = event.payload.busy;
        this.statusAgents = event.payload.agents;
      }),
      // M-04.4, M-17.1, M-17.2, M-17.6 (UC-017 to UC-019): the agents.
      events.agents.listen((event) => {
        const asked = event.payload.menu?.asked ?? null;
        if (asked !== null && asked !== (this.agents.menu?.asked ?? null)) {
          this.agentAnnouncement = asked;
        }
        this.agents = event.payload;
      }),
      events.agentMarks.listen((event) => {
        this.#announceMarks(this.agentMarks, event.payload);
        this.agentMarks = event.payload;
      }),
      events.deleted.listen((event) => {
        this.deleted = event.payload;
      }),
      events.settings.listen((event) => {
        this.#takeSettings(event.payload);
      }),
      events.settingsWindow.listen((event) => {
        this.#takeSettingsWindow(event.payload);
      }),
      events.workspace.listen((event) => {
        this.#sentDuringStart?.add('workspace');
        if (event.payload?.root !== this.workspace?.root) {
          // The note dialogs were of the workspace left.
          this.notes = NO_NOTE_DIALOGS;
          // So were "Discard changes" and "Your name for commits", the
          // branch menu and "Abort the sync".
          this.discarding = null;
          this.identity = null;
          this.branchMenu = null;
          this.abortAsked = false;
          if (this.exporting?.stage !== 'system') {
            this.exporting = null;
          }
          // M-21.3: the find field closes; the search starts anew there.
          this.search.workspaceChanged();
          // M-18: the todos were of the workspace left.
          this.todos = null;
          this.todoShown = null;
          this.todoProblem = null;
          this.todoChosen = null;
          this.todoCalendar = null;
          this.todoNewProblem = null;
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
      // M-19, UC-011, UC-012: the git changes and the diff.
      events.changes.listen((event) => {
        this.#takeChanges(event.payload);
      }),
      events.diff.listen((event) => {
        this.diff = event.payload;
      }),
      // M-04.1 to M-04.3, M-19.9, M-19.10: branches, syncs and conflicts.
      events.gitBar.listen((event) => {
        this.gitBar = event.payload;
      }),
      events.conflict.listen((event) => {
        this.#takeConflictBar(event.payload);
      }),
      events.todoConflicts.listen((event) => {
        this.todoConflicts = event.payload;
      }),
      // M-16.3, M-18, M-04.5: the todos.
      events.todos.listen((event) => {
        this.#takeTodos(event.payload);
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
        this.agents = result.data.agents;
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

  /**
   * M-04.9 (UC-019 BR-010): "Open" of the status bar opens a note an agent
   * created or changed.
   */
  openAgentNote(note: string): Promise<void> {
    return this.openNote(note);
  }

  /** M-17.6: the agent cursors in `note`. */
  agentMarksOf(note: string): AgentMarkView[] {
    return this.agentMarks.find((marks) => marks.note === note)?.marks ?? NO_MARKS;
  }

  /**
   * M-04.4, M-17.1 (UC-017 step 2, BR-002): a level chosen in the agents
   * menu changes the user setting.
   */
  async setAgentAccess(level: AgentLevelName): Promise<void> {
    const result = await commands.agentsSetAccess(level);
    if (result.status === 'error') {
      this.#fail('agents_set_access', result.error);
    }
  }

  /**
   * M-17.2 to M-17.5 (UC-016): checks the agent tools (`action` null, when
   * the settings open or another workspace opened), or does what a button
   * of the section "Agents" says and checks them again (BR-006).
   */
  async agentsAct(action: AgentAction | null = null): Promise<void> {
    // Only the answer to the latest call shows; answers can cross.
    const asked = ++this.#agentsAsked;
    this.agentsSectionBusy = true;
    try {
      const result = await commands.agentsSection(action);
      if (asked !== this.#agentsAsked) {
        return;
      }
      if (result.status === 'ok') {
        this.agentsSection = result.data;
      } else {
        this.#fail('agents_section', result.error);
      }
    } catch (error) {
      this.#fail('agents_section', { kind: 'internal', message: String(error) });
    } finally {
      if (asked === this.#agentsAsked) {
        this.agentsSectionBusy = false;
      }
    }
  }

  /** UX review F14: a cursor that appeared is told to screen readers. */
  #announceMarks(before: AgentMarksView[], now: AgentMarksView[]): void {
    const key = (note: string, mark: AgentMarkView) =>
      `${note}\u0000${mark.name}\u0000${mark.wrote}`;
    // Made for this one comparison, read by nothing reactive.
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const seen = new Set(before.flatMap((m) => m.marks.map((mark) => key(m.note, mark))));
    for (const { note, marks } of now) {
      for (const mark of marks) {
        if (!seen.has(key(note, mark))) {
          const title = this.tabs.tabs.find((tab) => tab.note === note)?.title ?? note;
          this.agentAnnouncement = `${mark.name} is ${mark.wrote ? 'editing' : 'reading'} ${title}`;
        }
      }
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
          switching: false,
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
   * M-15.1, UC-009 step 1: exports `note`; with `kind` (the submenu of the
   * note overview, M-05.4) the system's dialog comes at once, else "Export
   * {t} as:" asks first (Ctrl+Shift+E, "Export…" of the tab menu). Refused
   * in the status bar for a note in conflict. While an export is being
   * asked for, another is not started (egui).
   */
  async startExport(note: string, kind?: ExportKindName): Promise<void> {
    if (this.exporting !== null) {
      // UX review F11: the open "Export note" takes the keyboard again;
      // while the system's dialog is open the status says so already.
      if (this.exporting.stage !== 'system') {
        this.exportAskedAgain += 1;
      }
      return;
    }
    if (kind !== undefined) {
      await this.#askExportPlace(note, kind);
      return;
    }
    const result = await this.#serial(() => commands.exportStart(note));
    if (result.status === 'error') {
      this.#fail('export_start', result.error);
      return;
    }
    if (result.data === null || this.exporting !== null) {
      return;
    }
    this.exporting = { stage: 'choose', ...result.data, chosen: 0 };
  }

  /** M-15.1, BR-008: Up and Down move the choice of "Export {t} as:". */
  moveExportChoice(by: number): void {
    const dialog = this.exporting;
    if (dialog?.stage === 'choose') {
      const chosen = Math.min(dialog.choices.length - 1, Math.max(0, dialog.chosen + by));
      this.exporting = { ...dialog, chosen };
    }
  }

  /** M-15.1: Enter or a click chooses the kind `index`; the system's dialog follows. */
  async chooseExportKind(index: number): Promise<void> {
    const dialog = this.exporting;
    const choice = dialog?.stage === 'choose' ? dialog.choices[index] : undefined;
    if (dialog?.stage !== 'choose' || choice === undefined) {
      return;
    }
    this.exporting = null;
    await this.#askExportPlace(dialog.note, choice.kind);
  }

  /** M-15.2: the path typed in the field (ADR-016); the problem of the last try goes. */
  editExportPath(path: string): void {
    const dialog = this.exporting;
    if (dialog?.stage === 'path') {
      this.exporting = { ...dialog, path, problem: null };
    }
  }

  /** UC-009 A1: Cancel or Escape: nothing is exported. */
  closeExport(): void {
    if (this.exporting?.stage !== 'system') {
      this.exporting = null;
    }
  }

  /**
   * UC-009 steps 2–3, M-15.2: the system's file dialog (in the backend,
   * ADR-016); nothing else shows while it is open. Its file is exported;
   * without a dialog the path field asks.
   */
  async #askExportPlace(note: string, kind: ExportKindName): Promise<void> {
    this.exporting = { stage: 'system', note, kind };
    let answer;
    try {
      // Not queued: the dialog waits for the user, the window goes on.
      answer = await commands.exportChoose(note, kind);
    } catch (error) {
      answer = {
        status: 'error' as const,
        error: { kind: 'internal' as const, message: String(error) },
      };
    }
    this.exporting = null;
    if (answer.status === 'error') {
      this.#fail('export_choose', answer.error);
      return;
    }
    const place = answer.data;
    if (place.kind === 'chosen') {
      await this.#runExport(note, kind, null);
    } else if (place.kind === 'typed') {
      this.exporting = {
        stage: 'path',
        note,
        kind,
        heading: place.heading,
        path: place.path,
        problem: null,
        replace: null,
      };
    }
  }

  /** M-15.2: "Export" or "Replace" (or Enter) of the path field. */
  async exportToTypedPath(): Promise<void> {
    const dialog = this.exporting;
    if (dialog?.stage !== 'path') {
      return;
    }
    // The field closes at once, as in egui; it comes back if the file exists.
    this.exporting = null;
    await this.#runExport(dialog.note, dialog.kind, dialog);
  }

  /**
   * UC-009 steps 4–5, A5, M-15.3 to M-15.5: the export, once the window's
   * edits reached the backend (ADR-025: it holds the tab's text with
   * changes not yet saved). Not queued: a PDF may take a while, and the
   * status says "Exporting {t}…" meanwhile.
   */
  async #runExport(
    note: string,
    kind: ExportKindName,
    typed: Extract<ExportDialog, { stage: 'path' }> | null,
  ): Promise<void> {
    let result;
    try {
      result = await flushed(() =>
        commands.exportRun(
          note,
          kind,
          typed === null ? null : { path: typed.path, replace: typed.replace },
        ),
      );
    } catch (error) {
      this.#fail('export_run', { kind: 'internal', message: String(error) });
      return;
    }
    if (result.status === 'error') {
      this.#fail('export_run', result.error);
      return;
    }
    const answer = result.data;
    if (answer.kind === 'askAgain') {
      // A5: another place is asked for.
      if (this.exporting === null) {
        await this.#askExportPlace(note, kind);
      }
    } else if (answer.kind === 'exists' && typed !== null && this.exporting === null) {
      this.exporting = { ...typed, problem: answer.problem, replace: answer.path };
    } else if (answer.kind === 'problem' && typed !== null && this.exporting === null) {
      // The field comes back with what was typed and why (A5, an empty
      // path, a folder, a failed export).
      this.exporting = { ...typed, problem: answer.problem };
    }
  }

  /** M-04.8, UC-009 step 5: "Open" opens the export `index` with the system's program. */
  async openExport(index: number): Promise<void> {
    const result = await commands.exportOpen(index);
    if (result.status === 'error') {
      this.#fail('export_open', result.error);
    }
  }

  /** M-04.8, M-15.6, UC-009 A2: "Show" lists the approximated blocks of the export `index`. */
  async showApproximated(index: number): Promise<void> {
    const result = await commands.exportApproximated(index);
    if (result.status === 'error') {
      this.#fail('export_approximated', result.error);
      return;
    }
    if (result.data !== null && result.data.lines.length > 0) {
      this.approximated = result.data;
    }
  }

  /** M-15.6: "Close" of "Approximated blocks". */
  closeApproximated(): void {
    this.approximated = null;
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

  /** M-19.1: the changes came; the message follows the proposal until the user typed. */
  #takeChanges(changes: ChangesView | null): void {
    this.changes = changes;
    if (changes === null) {
      return;
    }
    let box = this.commit;
    // UC-012 A6: amending is offered only while the last commit may be amended.
    if (changes.amendable === null && box.amend) {
      box = { ...box, amend: false };
    }
    if (!box.typed && box.message !== changes.proposed) {
      box = { ...box, message: changes.proposed };
    }
    this.commit = box;
  }

  /** M-19.1, UC-012 step 3: the user typed the message; it no longer follows the proposal. */
  editCommitMessage(message: string): void {
    this.commit = { ...this.commit, message, typed: true, problem: null };
  }

  /** M-19.1, UC-012 A6: "Amend the last commit" takes over the last commit's message. */
  setAmend(amend: boolean): void {
    const last = this.changes?.amendable ?? null;
    if (amend && last !== null) {
      this.commit = { ...this.commit, amend, message: last, typed: true };
    } else {
      this.commit = { ...this.commit, amend: false };
    }
  }

  /**
   * M-19.1, UC-012 BR-004: the change of `path` chosen for the commit or
   * left out; `null`: "All" (`chosen`) or "None".
   */
  async chooseForCommit(path: string | null, chosen: boolean): Promise<void> {
    const result = await commands.gitCommitChoice(path, chosen);
    if (result.status === 'error') {
      this.#fail('git_commit_choice', result.error);
    }
  }

  /** M-19.2, UC-011 step 5: a click on a change opens its note or shows its diff. */
  async chooseChange(path: string): Promise<void> {
    const result = await this.#serial(() => commands.gitChoose(path));
    if (result.status === 'error') {
      this.#fail('git_choose', result.error);
    }
  }

  /** M-19.2, UC-011 BR-007: Ctrl+Shift+G opens the changes and puts the keyboard into the message. */
  showChanges(): void {
    if (this.changes !== null) {
      this.changesAsked += 1;
    }
  }

  /**
   * M-19.4, UC-012 steps 4–7: "Commit"/"Amend" or Ctrl+Enter, once the
   * window's edits reached the backend (ADR-025). The box empties after a
   * commit; a refusal stays in it (A1, A2); git not knowing the user asks
   * for the name (A3).
   */
  async commitChanges(): Promise<void> {
    const box = this.commit;
    if (box.busy) {
      return;
    }
    await this.#runCommit(() => commands.gitCommit(box.message, box.amend), 'git_commit');
  }

  /**
   * M-19.4, UC-012 A3: "Commit" of "Your name for commits": the name and
   * email are kept in the user's git settings, then the commit runs.
   */
  async answerIdentity(name: string, email: string): Promise<void> {
    const box = this.commit;
    if (box.busy) {
      return;
    }
    await this.#runCommit(
      () => commands.gitSetIdentity(name, email, box.message, box.amend),
      'git_set_identity',
    );
  }

  /** M-19.4, UC-012 A3 step 4: Cancel or Escape: nothing is committed, the box stays. */
  closeIdentity(): void {
    this.identity = null;
  }

  async #runCommit(
    call: () => Promise<{ status: 'ok'; data: CommitAnswer } | { status: 'error'; error: UiError }>,
    command: string,
  ): Promise<void> {
    this.commit = { ...this.commit, busy: true, problem: null };
    let answer: CommitAnswer | null = null;
    try {
      const result = await flushed(call);
      if (result.status === 'error') {
        this.#fail(command, result.error);
      } else {
        answer = result.data;
      }
    } catch (error) {
      this.#fail(command, { kind: 'internal', message: String(error) });
    }
    const box = { ...this.commit, busy: false };
    switch (answer?.kind) {
      case 'committed':
        // Step 7: the box empties; the message follows the proposal again.
        this.commit = { ...EMPTY_COMMIT, message: this.changes?.proposed ?? '' };
        this.identity = null;
        this.committedCount += 1;
        return;
      case 'problem':
        this.commit = { ...box, problem: answer.problem };
        this.identity = null;
        return;
      case 'identity':
        // A3: asked; asked again (with what was typed) when git still does not know the user.
        this.commit = box;
        this.identity = { name: '', email: '', ...this.identity, problem: null };
        return;
      case 'nameNotKept':
        // Review F12: the dialog stays with what was typed and says why.
        this.commit = box;
        if (this.identity !== null) {
          this.identity = { ...this.identity, problem: answer.problem };
        }
        return;
      default:
        this.commit = box;
        this.identity = null;
    }
  }

  /** M-19.4: the name or email typed in "Your name for commits". */
  editIdentity(dialog: IdentityDialog, change: Partial<IdentityDialog>): void {
    if (this.identity === dialog) {
      this.identity = { ...dialog, ...change, problem: null };
    }
  }

  /**
   * M-19.5, UC-011 A6 step 1: "Discard…" asks first, naming the note and
   * what is lost, once the window's edits reached the backend.
   */
  async startDiscard(path: string): Promise<void> {
    if (this.discarding !== null) {
      return;
    }
    const result = await this.#serial(() => flushed(() => commands.gitDiscardStart(path)));
    if (result.status === 'error') {
      this.#fail('git_discard_start', result.error);
      return;
    }
    if (result.data !== null && this.discarding === null) {
      this.discarding = result.data;
    }
  }

  /** M-19.5, UC-011 A6 step 4: Cancel or Escape: nothing changes. */
  closeDiscard(): void {
    this.discarding = null;
  }

  /**
   * M-19.5, UC-011 A6 steps 2–3: "Discard", once the window's edits reached
   * the backend; an open tab shows the committed text again (ADR-025: a
   * reload of its editor core).
   */
  async discard(): Promise<void> {
    const question = this.discarding;
    if (question === null || this.discardBusy) {
      return;
    }
    this.discardBusy = true;
    try {
      const result = await this.#serial(() => flushed(() => commands.gitDiscard(question.path)));
      if (result.status === 'error') {
        this.#fail('git_discard', result.error);
      }
    } catch (error) {
      this.#fail('git_discard', { kind: 'internal', message: String(error) });
    } finally {
      this.discardBusy = false;
      if (this.discarding === question) {
        this.discarding = null;
      }
    }
  }

  /**
   * M-19.6, UC-013 steps 1–2, BR-006: the branch status button or
   * Ctrl+Shift+B opens the branch menu, with the keyboard in its filter;
   * git lists the branches. Nothing opens without git (as in egui).
   */
  async openBranchMenu(): Promise<void> {
    // A-143: only while git answers.
    if (this.changes === null || this.changes.problem !== null) {
      return;
    }
    const menu: BranchMenu = {
      view: null,
      filter: '',
      cursor: 0,
      naming: null,
      name: null,
      problem: null,
      inTheWay: null,
      busy: true,
    };
    this.branchMenu = menu;
    const result = await this.#serial(() => commands.gitBranches());
    if (this.branchMenu !== menu) {
      return;
    }
    if (result.status === 'error') {
      this.branchMenu = null;
      this.#fail('git_branches', result.error);
      return;
    }
    this.branchMenu = result.data === null ? null : { ...menu, view: result.data, busy: false };
  }

  /** M-19.6, UC-013 A8: Escape or a click on the current branch closes the menu. */
  closeBranchMenu(): void {
    this.branchMenu = null;
  }

  /** M-19.6, BR-001: the filter typed; the choice goes back to the first entry. */
  editBranchFilter(filter: string): void {
    if (this.branchMenu !== null) {
      this.branchMenu = { ...this.branchMenu, filter, cursor: 0 };
    }
  }

  /** M-19.6, BR-006: Up and Down move the choice within the list shown. */
  moveBranchCursor(by: number): void {
    const menu = this.branchMenu;
    if (menu === null) {
      return;
    }
    const count = filteredBranches(menu).length;
    if (count === 0) {
      return;
    }
    const cursor = Math.max(0, Math.min(count - 1, menu.cursor + by));
    this.branchMenu = { ...menu, cursor };
  }

  /**
   * M-19.7, UC-013 steps 3–6, A4: a branch chosen; the current one closes
   * the menu, another is switched to once the window's edits reached the
   * backend.
   */
  async chooseBranch(entry: BranchEntryView): Promise<void> {
    const menu = this.branchMenu;
    if (menu === null || menu.busy || menu.view?.merging) {
      return;
    }
    if (entry.current) {
      this.branchMenu = null;
      return;
    }
    await this.#switch(
      entry.remote ? { kind: 'remote', name: entry.name } : { kind: 'local', name: entry.name },
    );
  }

  /** M-19.6, UC-013 A1 step 1: "New branch…", filled with the filter. */
  startNewBranch(): void {
    const menu = this.branchMenu;
    if (menu === null || menu.view?.merging) {
      return;
    }
    this.branchMenu = { ...menu, naming: menu.filter, name: null, problem: null };
    void this.editNewBranch(menu.filter);
  }

  /** M-19.6, BR-005: the name typed, and what git makes of it ("Branch: {clean}"). */
  async editNewBranch(name: string): Promise<void> {
    const menu = this.branchMenu;
    if (menu === null || menu.naming === null) {
      return;
    }
    this.branchMenu = { ...menu, naming: name, problem: null };
    const result = await commands.gitBranchName(name);
    const now = this.branchMenu;
    if (now === null || now.naming !== name) {
      return;
    }
    if (result.status === 'ok') {
      this.branchMenu = { ...now, name: result.data };
    }
  }

  /** M-19.6, UC-013 A1 step 2: "Create" or Enter. */
  async createBranch(): Promise<void> {
    const menu = this.branchMenu;
    if (menu === null || menu.naming === null || menu.busy || !menu.name?.ok) {
      return;
    }
    await this.#switch({ kind: 'new', name: menu.naming });
  }

  /** M-19.6, A8: "Cancel" of the new branch's name closes the menu, as in egui. */
  cancelNewBranch(): void {
    this.branchMenu = null;
  }

  /** M-19.6, BR-002: "Show changes" (and "Commit first", A3) opens the changes. */
  branchShowChanges(): void {
    this.branchMenu = null;
    this.showChanges();
  }

  async #switch(target: BranchTarget): Promise<void> {
    const menu = this.branchMenu;
    if (menu === null) {
      return;
    }
    this.branchMenu = { ...menu, busy: true };
    const result = await this.#serial(() => flushed(() => commands.gitSwitch(target)));
    const now = this.branchMenu;
    if (now === null) {
      return;
    }
    if (result.status === 'error') {
      this.branchMenu = null;
      this.#fail('git_switch', result.error);
      return;
    }
    const answer = result.data;
    switch (answer.kind) {
      case 'done':
        this.branchMenu = null;
        return;
      case 'inTheWay':
        this.branchMenu = { ...now, busy: false, naming: null, inTheWay: answer.text };
        return;
      case 'exists':
        // A2: the list again, filtered by the name, which can be chosen.
        this.branchMenu = {
          ...now,
          busy: false,
          naming: null,
          filter: answer.name,
          cursor: 0,
          problem: answer.problem,
        };
        return;
      case 'invalid':
        this.branchMenu = { ...now, busy: false, problem: answer.problem };
        return;
    }
  }

  /**
   * M-04.3, M-19.6, M-19.8, UC-014 step 1, A1: "Sync now", "Pull" or
   * "Push", once the window's edits reached the backend; the branch menu
   * closes.
   */
  async sync(mode: SyncModeName): Promise<void> {
    this.branchMenu = null;
    const result = await this.#serial(() => flushed(() => commands.syncRequest(mode)));
    if (result.status === 'error') {
      this.#fail('sync_request', result.error);
    }
  }

  /** The conflict bar came; the place shown stays within its places. */
  #takeConflictBar(bar: ConflictBarView | null): void {
    this.conflictBar = bar;
    if (bar !== null && this.conflictPlace >= bar.places.length) {
      this.conflictPlace = bar.places.length - 1;
    }
  }

  /** M-19.9, BR-006: Next (F7) and Previous (Shift+F7); the place shown. */
  moveConflictPlace(by: number): number | null {
    const bar = this.conflictBar;
    if (bar === null) {
      return null;
    }
    this.conflictPlace = Math.max(0, Math.min(bar.places.length - 1, this.conflictPlace + by));
    return this.conflictPlace;
  }

  /**
   * M-19.9, UC-015 steps 4–5, A1: "Here", "Remote", "Both" for the place
   * shown (Alt+H, Alt+R, Alt+B), or "All: Here"/"All: Remote" (`all`), once
   * the window's edits reached the backend.
   */
  async chooseConflict(choice: ChoiceName, all = false): Promise<void> {
    const bar = this.conflictBar;
    if (bar === null) {
      return;
    }
    const place = all ? null : this.conflictPlace;
    // S3 (review F13): the bar's revision; a choice of an older text does nothing.
    const result = await this.#serial(() =>
      flushed(() => commands.conflictChoose(bar.note, bar.rev, place, choice)),
    );
    if (result.status === 'error') {
      this.#fail('conflict_choose', result.error);
    }
  }

  /** M-19.2, UC-015 A2: "Keep here" or "Keep remote" of a file that is not a note. */
  async takeConflictSide(path: string, side: SideName): Promise<void> {
    const result = await this.#serial(() => commands.conflictTakeSide(path, side));
    if (result.status === 'error') {
      this.#fail('conflict_take_side', result.error);
    }
  }

  /** M-19.2, UC-015 A3: "Keep" or "Delete" of a note or file deleted on one side. */
  async keepConflict(path: string, keep: boolean): Promise<void> {
    const result = await this.#serial(() => flushed(() => commands.conflictKeep(path, keep)));
    if (result.status === 'error') {
      this.#fail('conflict_keep', result.error);
    }
  }

  /** M-19.10, UC-020 A10: "Keep mine" or "Take remote" of a todo in conflict. */
  async resolveTodoConflict(todo: number, mine: boolean): Promise<void> {
    const result = await this.#serial(() => commands.todoConflictResolve(todo, mine));
    if (result.status === 'error') {
      this.#fail('todo_conflict_resolve', result.error);
    }
  }

  /** M-04.2, M-19.9, UC-015 A5 step 1: "Abort the sync" asks first. */
  askAbort(): void {
    this.abortAsked = true;
  }

  /** M-19.10, UC-015 A5: Cancel or Escape: nothing changes. */
  closeAbort(): void {
    this.abortAsked = false;
  }

  /** M-19.10, UC-015 A5 step 2: "Abort the sync" confirmed, once the edits reached the backend. */
  async abortSync(): Promise<void> {
    if (this.abortBusy) {
      return;
    }
    this.abortBusy = true;
    try {
      const result = await this.#serial(() => flushed(() => commands.conflictAbort()));
      if (result.status === 'error') {
        this.#fail('conflict_abort', result.error);
      }
    } catch (error) {
      this.#fail('conflict_abort', { kind: 'internal', message: String(error) });
    } finally {
      this.abortBusy = false;
      this.abortAsked = false;
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
    } else if (question?.switching) {
      // F15 review: the other workspace opens without the todos not stored.
      const result = await this.#serial(() => commands.wsSwitchAnyway());
      if (result.status === 'error') {
        this.#fail('ws_switch_anyway', result.error);
      }
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

  /** UC-020 A7: the todos as they are now; a todo gone is not shown any more. */
  #takeTodos(todos: TodosView): void {
    this.todos = todos;
    const there = (number: number | null) =>
      number !== null && todos.todos.some((todo) => todo.number === number);
    if (!there(this.todoShown)) {
      this.todoShown = null;
      this.todoProblem = null;
    }
    if (this.todoCalendar !== null && this.todoCalendar.todo !== this.todoShown) {
      this.todoCalendar = null;
    }
  }

  /** M-18.1, M-18.6 (UC-020 steps 3–4, A3): creates a todo from the title typed. */
  async createTodo(): Promise<void> {
    const answer = await this.#todo('todo_create', () => commands.todoCreate(this.todoNewTitle));
    if (answer?.kind === 'done') {
      this.todoNewTitle = '';
      this.todoNewProblem = null;
    } else if (answer?.kind === 'problem') {
      this.todoNewProblem = answer.problem;
    }
  }

  /** M-18.1: what is typed for a new todo. */
  editNewTodo(title: string): void {
    this.todoNewTitle = title;
  }

  /** M-18.3: a click on the title of a todo shows its fields, or hides them again. */
  toggleTodo(number: number): void {
    this.todoShown = this.todoShown === number ? null : number;
    this.todoProblem = null;
    this.todoProblemField = null;
    this.todoCalendar = null;
  }

  /** M-18.8 (UC-021 A4): the title of a row shows the todo in the section "Todos". */
  showTodo(number: number): void {
    this.todoChosen = number;
    this.todoShown = number;
    this.todoProblem = null;
    this.todoProblemField = null;
    this.todoCalendar = null;
    this.todoShownAsked += 1;
  }

  /** M-18.7: the row chosen with the arrow keys. */
  chooseTodo(number: number | null): void {
    this.todoChosen = number;
  }

  /**
   * M-18.2 to M-18.4 (UC-020 steps 5–10, A1, A2, A4): a change of the todo
   * `number`; a title or day that is not taken is said under the fields.
   */
  async changeTodo(number: number, change: TodoChange): Promise<TodoAnswer | null> {
    const answer = await this.#todo('todo_change', () => commands.todoChange(number, change));
    if (this.todoShown !== number) {
      return answer;
    }
    if (answer?.kind === 'problem') {
      this.todoProblem = answer.problem;
      this.todoProblemField = change.kind;
    } else if (answer?.kind === 'done') {
      this.todoProblem = null;
      this.todoProblemField = null;
    }
    return answer;
  }

  /** M-18.7, M-18.8 (UC-021 steps 7–8): marked done or open again in the tab "Todos". */
  async markTodo(number: number, done: boolean): Promise<TodoAnswer | null> {
    return this.#todo('todo_mark', () => commands.todoMark(number, done));
  }

  /** M-18.5 (UC-020 A5): "Delete", without asking. */
  async deleteTodo(number: number): Promise<boolean> {
    const answer = await this.#todo('todo_delete', () => commands.todoDelete(number));
    if (answer?.kind === 'done' && this.todoShown === number) {
      this.todoShown = null;
      this.todoProblem = null;
      this.todoCalendar = null;
    }
    return answer?.kind === 'done';
  }

  /** The done state of todo `number` as shown now. */
  todoDone(number: number): boolean {
    return this.todos?.todos.find((todo) => todo.number === number)?.done ?? false;
  }

  /** M-04.6: "Undo" of the status bar after a todo was deleted or marked. */
  async undoTodo(): Promise<void> {
    await this.#todo('todo_undo', () => commands.todoUndo());
  }

  /** M-18.4, M-18.8 (UC-020 step 8, A6): a linked note opens, or is said to be missing. */
  async openTodoLink(link: string): Promise<void> {
    const result = await this.#serial(() => commands.todoOpenLink(link));
    if (result.status === 'error') {
      this.#fail('todo_open_link', result.error);
    }
  }

  /** M-18.4 (UC-020 A1): the notes "Link a note…" offers for `query`. */
  async todoCandidates(number: number, query: string): Promise<TodoCandidatesView | null> {
    const result = await commands.todoCandidates(number, query);
    if (result.status === 'error') {
      this.#fail('todo_candidates', result.error);
      return null;
    }
    return result.data;
  }

  /** M-18.3: "Pick" opens the calendar on the month of the day set, else of today. */
  async openTodoCalendar(
    field: 'due' | 'scheduled',
    todo: number,
    day: string | null,
  ): Promise<void> {
    const month = await this.#month(day, 0);
    if (month !== null) {
      this.todoCalendar = { field, todo, month };
    }
  }

  /** M-18.3: ◀ and ▶ of the calendar. */
  async moveTodoCalendar(step: number): Promise<void> {
    const calendar = this.todoCalendar;
    if (calendar === null) {
      return;
    }
    const month = await this.#month(calendar.month.month, step);
    if (month !== null && this.todoCalendar === calendar) {
      this.todoCalendar = { ...calendar, month };
    }
  }

  /** M-18.3: a day chosen in the calendar (`null`: "No day") is stored at once. */
  async chooseTodoDay(day: string | null): Promise<void> {
    const calendar = this.todoCalendar;
    if (calendar === null) {
      return;
    }
    this.todoCalendar = null;
    const text = day ?? '';
    this.todoDayPicked = { todo: calendar.todo, field: calendar.field, day: text };
    await this.changeTodo(
      calendar.todo,
      calendar.field === 'due' ? { kind: 'due', day: text } : { kind: 'scheduled', day: text },
    );
  }

  /** M-18.3: the calendar closes without a choice. */
  closeTodoCalendar(): void {
    this.todoCalendar = null;
  }

  /**
   * M-06.5, M-04.5 (UC-021 step 1, A2): "Open list", the tab "Todos" or the
   * overdue todos of the status bar (with the filter "Open") show the tab.
   */
  async showTodoList(filter: TodoFilterName | null = null): Promise<void> {
    const result = await this.#serial(() => commands.todoListShow(filter));
    if (result.status === 'error') {
      this.#fail('todo_list_show', result.error);
    }
  }

  /** M-06.5: "×" or a middle click closes the tab "Todos". */
  async closeTodoList(): Promise<void> {
    const result = await this.#serial(() => commands.todoListClose());
    if (result.status === 'error') {
      this.#fail('todo_list_close', result.error);
    }
  }

  /** M-18.6, M-18.8 (UC-021 steps 3–4, A1, BR-007): the sort and the filter of the tab. */
  async setTodoList(sort: TodoSortName, filter: TodoFilterName): Promise<void> {
    const result = await commands.todoListSet(sort, filter);
    if (result.status === 'error') {
      this.#fail('todo_list_set', result.error);
    }
  }

  async #month(day: string | null, step: number): Promise<CalendarView | null> {
    const result = await commands.todoMonth(day, step);
    if (result.status === 'error') {
      this.#fail('todo_month', result.error);
      return null;
    }
    return result.data;
  }

  /** A todo command; `null` when it failed (said in the status bar). */
  async #todo(
    command: string,
    call: () => Promise<{ status: 'ok'; data: TodoAnswer } | { status: 'error'; error: UiError }>,
  ): Promise<TodoAnswer | null> {
    try {
      const result = await call();
      if (result.status === 'error') {
        this.#fail(command, result.error);
        return null;
      }
      return result.data;
    } catch (error) {
      this.#fail(command, { kind: 'internal', message: String(error) });
      return null;
    }
  }

  /** M-06.5: the tab "Todos" is the active one; it shows in place of a note. */
  get todoListShown(): boolean {
    return this.tabs.todoList.open && this.tabs.todoList.active;
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

  /**
   * M-03.7, M-20.1, UC-010 BR-006: "Settings…" and Ctrl/Cmd+, open the
   * settings window or close it.
   */
  toggleSettings(): Promise<void> {
    if (this.settingsOpen) {
      this.closeSettings();
      return Promise.resolve();
    }
    return this.openSettings();
  }

  /**
   * M-20.1: opens the settings window, at `scope` if given (UC-017 A1 opens
   * "This workspace"); what it shows is asked anew each time.
   */
  async openSettings(scope?: Scope): Promise<void> {
    if (scope === 'me' || (scope === 'workspace' && this.workspace !== null)) {
      this.settingsScope = scope;
    }
    this.settingsOpen = true;
    this.settingsProblem = null;
    const result = await commands.settingsWindow();
    if (result.status === 'ok') {
      this.#takeSettingsWindow(result.data);
    } else {
      this.settingsProblem = result.error.message;
      this.#fail('settings_window', result.error);
    }
  }

  /** M-20.1: closes the settings window; the changes stay (UC-010 step 5). */
  closeSettings(): void {
    this.settingsBeforeClose?.();
    this.settingsOpen = false;
  }

  /**
   * M-20.5, UC-010 steps 3 and 4, A1, A2, A8: `key` changed in the part
   * `scope` of the settings window; `null` for "As for me" and "Follow the
   * notes". The backend answers with what the window shows now.
   */
  async changeSetting(scope: Scope, key: SettingName, value: string | null): Promise<void> {
    const result = await commands.settingsSet(scope, key, value);
    if (result.status === 'ok') {
      this.#takeSettingsWindow(result.data.view);
      this.settingsProblem = result.data.problem;
    } else {
      this.settingsProblem = result.error.message;
      this.#fail('settings_set', result.error);
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

  #takeSettingsWindow(view: SettingsWindowView): void {
    // An answer and an event can cross: the newer view stays.
    if (this.settingsWindow !== null && view.revision < this.settingsWindow.revision) {
      return;
    }
    this.settingsWindow = view;
    // UC-010 A9: without a workspace only "For me" is there.
    if (!view.workspace) {
      this.settingsScope = 'me';
    }
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
