// ADR-025, docs/migration/variante-b.md section 3 (step B4c): the window's
// side of the sync. Each open note the window edits has its editor core in
// the WASM (`EditorCore`); its edits go to the backend after the frame, in
// one `ed_sync` per note, with at most one on its way. The backend's
// changes come back in the answer: `ed-changed` and `ed-flush` only ring.
//
// A failed `ed_sync` sends the same batch again (the backend takes it in
// once, by its number), backing off and retrying while edits wait; the
// status line says so once. Copies that went apart send the window's whole
// text (`ed_resync`), keeping what was typed meanwhile. Every command that
// reads the text goes through `flushed`, which runs it only when every edit
// reached the backend; closing Underline first sends every edit and then
// answers the close (`app_request_close`), or asks "Changes not stored". A
// core that failed for good (a trap of the WASM) is opened anew from the
// backend's text, and the status line says so (Q-40, for now).

import {
  commands,
  type BlocksView,
  type CloseQuestion,
  type EditorPatch,
  type EnrichView,
  type FindSpec,
  type FlushView,
  type FormatName,
  type MirrorView,
  type SyncAnswer,
  type SyncBatch,
  type TabViews,
  type UiError,
} from '../bindings';
import { EditorCore, loadEditorCore } from './core';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/** The message of an error of any kind (a `UiError`, an `Error`, …). */
export function messageOf(error: unknown): string {
  return typeof error === 'object' && error !== null && 'message' in error
    ? String(error.message)
    : String(error);
}

/** What the sync needs of the backend. */
export interface SyncIpc {
  mirror(note: string): Promise<MirrorView>;
  sync(note: string, batch: SyncBatch): Promise<SyncAnswer>;
  resync(note: string, text: string): Promise<SyncAnswer>;
  /** Answers the close handshake: whether every edit was sent. */
  requestClose(sent: boolean): Promise<void>;
}

/** The sync commands of the backend. */
export const tauriSyncIpc: SyncIpc = {
  mirror: (note) => unwrap(commands.edMirror(note)),
  sync: (note, batch) => unwrap(commands.edSync(note, batch)),
  resync: (note, text) => unwrap(commands.edResync(note, text)),
  requestClose: async (sent) => {
    await unwrap(commands.appRequestClose(sent));
  },
};

let ipc: SyncIpc = tauriSyncIpc;

/** For the tests: the backend the sync talks to. */
export function setSyncIpc(next: SyncIpc): void {
  ipc = next;
}

/** What a sync step took, for the measuring build (`bench.ts`). */
export type SyncProbe = (step: 'roundtrip' | 'acknowledge' | 'views', ms: number) => void;
let probe: SyncProbe | null = null;

/** For the measuring build: tells `next` what each sync step took. */
export function setSyncProbe(next: SyncProbe | null): void {
  probe = next;
}

/** What the window shows of the sync: a status text, a close question. */
export type SyncReporter = {
  /** A status text about `note` (its title is the window's to add). */
  status(note: string, text: string): void;
  /** Q-40: the core of `note` failed; its latest edits were lost. */
  dead(note: string): void;
  /** Closing: edits could not be sent; "Changes not stored" asks. */
  closeQuestion(question: CloseQuestion): void;
};

let reporter: SyncReporter | null = null;

/** Why the latest edits of a note cannot be stored: they did not reach the backend. */
export function handOverFailed(reason: string): string {
  return `Underline could not hand your latest edits over to be stored: ${reason}`;
}

/** Q-40: what a note shows after its editor failed. */
export const EDITOR_FAILED =
  'The editor failed. It shows the last stored text again; your latest edits were lost.';

/** The shell shows what the sync has to say. */
export function setSyncReporter(next: SyncReporter | null): void {
  reporter = next;
}

/** What the view showing a note takes from its core. */
export type CoreListener = {
  /** The backend's changes, as a patch of the source view. */
  patch(patch: EditorPatch): void;
  /** The sections and the marked hits, once they changed. */
  views(views: TabViews): void;
  /** The core confirmed a revision: results of the backend may apply now. */
  synced(): void;
  /** The core failed and was opened anew: the view shows its text again. */
  reopened(): void;
  /** The facade's blocks took the backend's colours and figures (`ed-enrich`). */
  blocks?(changes: BlocksView[]): void;
  /**
   * FR-097, UC-005 BR-016: another view of the note changed its text; the
   * view brings itself up to date (the source view from the changes since
   * its revision, the facade by its blocks around them).
   */
  sibling?(): void;
};

/**
 * FR-097, UC-005 BR-016: the id of each tab's view of its note in the core,
 * by pane and note: a tab keeps its cursor, folds and blocks while it
 * switches between the source view and the facade, and the tab of the same
 * note in the other pane has its own.
 */
const paneViews = new Map<string, number>();
let lastViewId = 0;

function paneViewKey(pane: string, note: string): string {
  return `${pane}\u0000${note}`;
}

/** FR-097: the id of the view of `note` in the tab of the pane `pane`. */
export function viewIdOf(pane: string, note: string): number {
  const key = paneViewKey(pane, note);
  let id = paneViews.get(key);
  if (id === undefined) {
    lastViewId += 1;
    id = lastViewId;
    paneViews.set(key, id);
  }
  return id;
}

/**
 * FR-097: only the tabs `open` (pane and note) keep their views in the
 * cores; the view of a tab closed goes, with its cursor and folds.
 */
export function keepViewsOf(open: Iterable<readonly [string, string]>): void {
  const keep = new Set([...open].map(([pane, note]) => paneViewKey(pane, note)));
  for (const [key, id] of [...paneViews]) {
    if (!keep.has(key)) {
      paneViews.delete(key);
      const note = key.slice(key.indexOf('\u0000') + 1);
      cores.get(note)?.closeView(id);
    }
  }
}

/** UC-006 step 7: the views of `from` are those of `to` now. */
function renameViews(from: string, to: string): void {
  for (const [key, id] of [...paneViews]) {
    const at = key.indexOf('\u0000');
    if (key.slice(at + 1) === from) {
      paneViews.delete(key);
      paneViews.set(paneViewKey(key.slice(0, at), to), id);
    }
  }
}

/** The view a listener set through `TabCore.listener` stands for (one view only). */
const ONLY_VIEW = 0;

/** Runs `work` after the next frame was painted (the spike's method). */
function afterFrame(work: () => void): void {
  const frame = globalThis.requestAnimationFrame ?? ((f: () => void) => setTimeout(f, 16));
  frame(() => setTimeout(work, 0));
}

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

/**
 * How long after the last views the core is asked once more: just over the
 * 300 ms it waits between finding sections or hits again.
 */
export const VIEWS_TRAILING_MS = 320;

/** The status line says so after this many failures in a row. */
const REPORT_AFTER = 3;
/** The longest wait between two tries, in ms. */
const LONGEST_WAIT = 5000;
/** How long a flush tries before it gives up, in ms. */
const FLUSH_AT_MOST = 3000;

/**
 * One note's core and its sync.
 *
 * FR-097, UC-005 BR-016: the note may be shown by two views at once (its
 * source view in one pane of the split view, its facade in the other).
 * They share the core, so one text and one history; each calls it as
 * itself (`coreFor`), which gives it its own cursor, folds and blocks, and
 * the others follow its edits after the frame (`CoreListener.sibling`).
 * What reads the core for the window (the find field, the statistics)
 * reads it as the view the user works in (`core`).
 */
export class TabCore {
  /** The note; a rename gives it the new one (UC-006 step 7). */
  note: string;
  readonly #core: EditorCore;
  /** The note's format (M-09.14: pasted HTML becomes its markup). */
  readonly format: FormatName;
  /** The listeners of the views, by their ids. */
  readonly #listeners = new Map<number, CoreListener>();
  /** The views of the note, by their ids (FR-097). */
  readonly #views = new Set<number>();
  /** The view the user works in: the one that took an input or the keyboard last. */
  #userView: number | null = null;
  /** The views that follow another one's edits after the frame. */
  readonly #siblingsDue = new Set<number>();
  #siblingsScheduled = false;
  #scheduled = false;
  #viewsScheduled = false;
  /** The views asked once more after a burst of inputs. */
  #trailing: ReturnType<typeof setTimeout> | null = null;
  #sending: Promise<boolean> | null = null;
  #retry: ReturnType<typeof setTimeout> | null = null;
  #failures = 0;
  #reported = false;
  #lastError = '';
  /** The tab closed: everything stops, silently. */
  #closed = false;

  constructor(note: string, core: EditorCore, format: FormatName = 'Markdown') {
    this.note = note;
    this.#core = core;
    this.format = format;
  }

  /**
   * The core, as the view the user works in sees it (its cursor and
   * selection): for the find field, the statistics, the sync.
   */
  get core(): EditorCore {
    if (this.#views.size > 1 && this.#userView !== null && !this.#core.dead) {
      this.#core.useView(this.#userView, false);
    }
    return this.#core;
  }

  /**
   * FR-097: the core as the view `view` sees it; with `input`, the user
   * acts in that view (its edits start their own undo steps after the
   * other view's, BR-016).
   */
  coreFor(view: number, input: boolean): EditorCore {
    if (!this.#views.has(view)) {
      this.openView(view);
    }
    if (input || this.#userView === null) {
      this.#userView = view;
    }
    if (this.#views.size > 1) {
      this.#core.useView(view, input);
    }
    return this.#core;
  }

  /** FR-097: the view `view` shows the note from now on. */
  openView(view: number): void {
    if (this.#views.has(view) || !this.alive) {
      return;
    }
    this.#views.add(view);
    this.#core.viewOpen(view);
  }

  /** FR-097: the view `view` is gone. */
  closeView(view: number): void {
    this.#listeners.delete(view);
    this.#siblingsDue.delete(view);
    if (!this.#views.delete(view)) {
      return;
    }
    if (this.#userView === view) {
      this.#userView = this.#views.values().next().value ?? null;
    }
    if (this.alive) {
      this.#core.viewClose(view);
      // The view left takes the state the core holds as its own again.
      const left = this.#views.values().next().value;
      if (left !== undefined) {
        this.#core.useView(left, false);
      }
    }
  }

  /** FR-097: the keyboard went to the view `view`; the window reads the core as that view. */
  focusView(view: number): void {
    if (this.#views.has(view)) {
      this.#userView = view;
    }
  }

  /** How many views show the note. */
  get viewCount(): number {
    return this.#views.size;
  }

  /** The listener of the view `view` takes the core's changes from now on. */
  attach(view: number, listener: CoreListener): void {
    this.#listeners.set(view, listener);
  }

  /** The listener `listener` of the view `view` goes, if it is still the one. */
  detach(view: number, listener: CoreListener): void {
    if (this.#listeners.get(view) === listener) {
      this.#listeners.delete(view);
    }
  }

  /** The one view's listener (one view only; the tests). */
  get listener(): CoreListener | null {
    return this.#listeners.values().next().value ?? null;
  }

  set listener(listener: CoreListener | null) {
    if (listener === null) {
      this.#listeners.clear();
    } else {
      this.#listeners.set(ONLY_VIEW, listener);
    }
  }

  /** The listener of the view the user works in, else any. */
  #userListener(): CoreListener | null {
    return (
      (this.#userView === null ? undefined : this.#listeners.get(this.#userView)) ?? this.listener
    );
  }

  /**
   * FR-097: the views other than `view` follow its edits after the frame,
   * all edits of a frame at once.
   */
  #followed(view: number | null): void {
    for (const other of this.#listeners.keys()) {
      if (other !== view && other !== ONLY_VIEW) {
        this.#siblingsDue.add(other);
      }
    }
    if (this.#siblingsDue.size === 0 || this.#siblingsScheduled) {
      return;
    }
    this.#siblingsScheduled = true;
    afterFrame(() => {
      this.#siblingsScheduled = false;
      const due = [...this.#siblingsDue];
      this.#siblingsDue.clear();
      for (const id of due) {
        if (this.alive) {
          this.#listeners.get(id)?.sibling?.();
        }
      }
    });
  }

  /** Whether the core can still be used. */
  get alive(): boolean {
    return !this.#closed && !this.#core.dead;
  }

  /** Why the last send failed. */
  get lastError(): string {
    return this.#lastError;
  }

  /**
   * After an input: the edits go out after the frame, the views follow;
   * FR-097: the other views of the note follow the input of `view`.
   */
  afterInput(view: number | null = null): void {
    this.schedule();
    this.scheduleViews();
    if (this.#views.size > 1) {
      this.#followed(view);
    }
  }

  /** Sends after the frame, unless a send waits already. */
  schedule(): void {
    if (this.#scheduled || !this.alive) {
      return;
    }
    this.#scheduled = true;
    afterFrame(() => {
      this.#scheduled = false;
      void this.send();
    });
  }

  /** What this core marks now, as JSON (`undefined`: not set yet). */
  #search: string | undefined = undefined;

  /**
   * UC-008 BR-008, BR-011, M-13.2: the core marks the places of `spec`
   * (`null`: none); the hits follow with the views.
   */
  setSearch(spec: FindSpec | null): void {
    const key = JSON.stringify(spec);
    if (!this.alive || this.#search === key) {
      return;
    }
    try {
      this.#core.setSearch(spec);
      this.#search = key;
      this.scheduleViews();
    } catch (error) {
      this.#problem(error);
    }
  }

  /** Works out the sections and hits after the frame. */
  scheduleViews(): void {
    if (this.#viewsScheduled || !this.alive) {
      return;
    }
    this.#viewsScheduled = true;
    afterFrame(() => {
      this.#viewsScheduled = false;
      this.#runViews();
      // The core finds sections and hits at most every 300 ms while the
      // text changes (A-30, A-90): the last input of a burst may fall in
      // that pause, so they are asked once more after it (app-tester F9:
      // the facade's marks of an edited block came back only with the next
      // key).
      if (this.#trailing !== null) {
        clearTimeout(this.#trailing);
      }
      this.#trailing = setTimeout(() => {
        this.#trailing = null;
        this.#runViews();
      }, VIEWS_TRAILING_MS);
    });
  }

  /** Asks the core for the sections and hits that changed, for the view. */
  #runViews(): void {
    if (!this.alive) {
      return;
    }
    try {
      const start = performance.now();
      if (this.#views.size > 1) {
        // FR-097: each view is told its own sections and hits.
        for (const [id, listener] of this.#listeners) {
          if (id === ONLY_VIEW || !this.#views.has(id)) {
            continue;
          }
          this.#core.useView(id, false);
          const views = this.#core.views(start);
          if (views.sections || views.found) {
            listener.views(views);
          }
        }
        // The window reads the core as the view the user works in again.
        void this.core;
      } else {
        const views = this.#core.views(start);
        if (views.sections || views.found) {
          this.listener?.views(views);
        }
      }
      probe?.('views', performance.now() - start);
    } catch (error) {
      this.#problem(error);
    }
  }

  /** ADR-025: the backend's colours and figures of the facade's blocks. */
  enrich(sent: EnrichView): void {
    if (!this.alive) {
      return;
    }
    try {
      if (this.#views.size > 1) {
        // FR-097: each facade of the note takes them into its own blocks.
        for (const [id, listener] of this.#listeners) {
          if (id === ONLY_VIEW || !this.#views.has(id) || !listener.blocks) {
            continue;
          }
          this.#core.useView(id, false);
          const changes = this.#core.enrich(sent);
          if (changes.length > 0) {
            listener.blocks(changes);
          }
        }
        void this.core;
        return;
      }
      const changes = this.#core.enrich(sent);
      if (changes.length > 0) {
        this.listener?.blocks?.(changes);
      }
    } catch (error) {
      this.#problem(error);
    }
  }

  /** The backend changed the text (`ed-changed`, `ed-flush`): fetch it. */
  ring(): void {
    if (!this.alive) {
      return;
    }
    this.#core.ring();
    this.schedule();
  }

  /**
   * Sends what waits, unless a batch is on its way; resolves to whether it
   * (or nothing) went through.
   */
  send(): Promise<boolean> {
    if (this.#sending) {
      return this.#sending;
    }
    if (!this.alive) {
      return Promise.resolve(false);
    }
    let batch: SyncBatch | null;
    try {
      batch = this.#core.takeOutbox();
    } catch (error) {
      this.#problem(error);
      return Promise.resolve(false);
    }
    if (!batch) {
      return Promise.resolve(true);
    }
    const sent = batch;
    this.#sending = this.#exchange(sent).finally(() => {
      this.#sending = null;
    });
    return this.#sending;
  }

  async #exchange(batch: SyncBatch): Promise<boolean> {
    let answer: SyncAnswer;
    const sent = performance.now();
    try {
      answer = await ipc.sync(this.note, batch);
      probe?.('roundtrip', performance.now() - sent);
    } catch (error) {
      this.#failed(error);
      return false;
    }
    if (!this.alive) {
      return false;
    }
    try {
      const taken = performance.now();
      const applied = this.core.acknowledge(answer);
      if (applied.desync) {
        console.error(
          `The text of ${this.note} went apart from the backend's (${applied.reason ?? 'unknown'}); sending it whole`,
        );
        if (!(await this.#resync())) {
          return false;
        }
      } else if (applied.patch) {
        // The patch carries the selection of the view the user works in;
        // the other views follow the backend's change (FR-097).
        this.#userListener()?.patch(applied.patch);
        this.scheduleViews();
        if (this.#views.size > 1) {
          this.#followed(this.#userView);
        }
      }
      probe?.('acknowledge', performance.now() - taken);
    } catch (error) {
      this.#problem(error);
      return false;
    }
    this.#succeeded();
    for (const listener of [...this.#listeners.values()]) {
      listener.synced();
    }
    if (this.alive && !this.#core.idle()) {
      this.schedule();
    }
    return true;
  }

  /**
   * ADR-025 section 3.10: the copies went apart; the window's text wins.
   * What is typed while it travels stays to be sent.
   */
  async #resync(): Promise<boolean> {
    const text = this.#core.text();
    try {
      const answer = await ipc.resync(this.note, text);
      if (this.alive) {
        this.#core.reset(text, answer.rev);
      }
      return true;
    } catch (error) {
      // Sent again later: the core still asks for the whole text then.
      this.#failed(error);
      return false;
    }
  }

  /** A send failed: the batch goes again after a wait growing with each. */
  #failed(error: unknown): void {
    this.#lastError = messageOf(error);
    if (!this.alive) {
      return;
    }
    try {
      this.#core.failed();
    } catch (problem) {
      this.#problem(problem);
      return;
    }
    this.#failures += 1;
    if (this.#failures >= REPORT_AFTER && !this.#reported) {
      this.#reported = true;
      reporter?.status(this.note, `${handOverFailed(this.#lastError)}. Underline keeps trying`);
    }
    if (this.#retry === null) {
      const wait = Math.min(LONGEST_WAIT, 100 * 2 ** (this.#failures - 1));
      this.#retry = setTimeout(() => {
        this.#retry = null;
        void this.send();
      }, wait);
    }
  }

  #succeeded(): void {
    this.#failures = 0;
    this.#reported = false;
    this.#lastError = '';
  }

  /** Something failed that sending again does not mend; the log has it. */
  #problem(error: unknown): void {
    this.#lastError = messageOf(error);
    if (this.alive) {
      console.error(`The editor of ${this.note}: ${this.#lastError}`);
    }
  }

  /**
   * Sends every edit and fetches what the backend changed, until in step;
   * whether that worked within `atMost` ms.
   */
  async flush(atMost = FLUSH_AT_MOST): Promise<boolean> {
    if (!this.alive) {
      // A closed tab has nothing to send; a failed core lost its edits.
      return this.#closed;
    }
    try {
      this.#core.ring();
    } catch (error) {
      this.#problem(error);
      return false;
    }
    const end = performance.now() + atMost;
    while (this.alive && performance.now() < end) {
      if (this.#sending) {
        await this.#sending;
      } else if (this.#core.idle()) {
        return true;
      } else if (!(await this.send())) {
        await sleep(Math.min(200, Math.max(0, end - performance.now())));
      }
    }
    return this.alive && this.#core.idle();
  }

  /** The tab closed: the core goes, and nothing more is said of it. */
  close(): void {
    this.#closed = true;
    if (this.#trailing !== null) {
      clearTimeout(this.#trailing);
      this.#trailing = null;
    }
    if (this.#retry !== null) {
      clearTimeout(this.#retry);
      this.#retry = null;
    }
    this.#listeners.clear();
    this.#siblingsDue.clear();
    this.#views.clear();
    this.#core.free();
  }

  /** Q-40: every view of the note shows its text anew. */
  reopened(): void {
    for (const listener of [...this.#listeners.values()]) {
      listener.reopened();
    }
  }
}

/** The cores of the open notes, by note. */
class Cores {
  /** Being opened or open, with the generation each was opened in. */
  readonly #made = new Map<string, { core: Promise<TabCore>; generation: number }>();
  /** Open, for the synchronous calls of the views. */
  readonly #ready = new Map<string, TabCore>();
  /** The generation of each open core. */
  readonly #generations = new WeakMap<TabCore, number>();
  /** The generation of cores opened from now on. */
  #generation = 0;
  /** Cores of a generation below this one were dropped, also those still opening. */
  #droppedBelow = 0;

  get(note: string): TabCore | undefined {
    return this.#ready.get(note);
  }

  all(): TabCore[] {
    return [...this.#ready.values()];
  }

  open(note: string, mac: boolean): Promise<TabCore> {
    let made = this.#made.get(note);
    // A core of before a workspace was being opened is not of its notes.
    if (!made || made.generation < this.#generation) {
      const generation = this.#generation;
      const entry = { core: this.#make(note, mac, generation), generation };
      made = entry;
      this.#made.set(note, entry);
      entry.core.catch(() => {
        if (this.#made.get(note) === entry) {
          this.#made.delete(note);
        }
      });
    }
    return made.core;
  }

  async #make(note: string, mac: boolean, generation: number): Promise<TabCore> {
    const [, mirror] = await Promise.all([loadEditorCore(), ipc.mirror(note)]);
    const format = mirror.format as FormatName;
    const core = EditorCore.open(note, mirror.text, format, mac);
    const tab = new TabCore(note, core, format);
    if (generation < this.#droppedBelow) {
      // Its workspace was left while it opened.
      tab.close();
      throw new Error(`The editor of ${note} is not open`);
    }
    core.startSync(mirror.rev);
    core.select(core.state().rev, mirror.anchor, mirror.head);
    // M-13.2: the words searched now are marked in a note opened meanwhile.
    tab.setSearch(searchFor(note));
    // UC-010 BR-001, UC-005 BR-019: close pairs as the settings say.
    core.setClosePairs(closePairs);
    core.onDead = () => {
      // Q-40: opened anew from the backend's text; the edits not confirmed
      // are lost, and the status line says so. The note may have been
      // renamed since (`tab.note`).
      console.error(
        `The editor's core of ${tab.note} failed; it is opened again from the stored text`,
      );
      reporter?.dead(tab.note);
      this.#forget(tab.note, tab);
      tab.reopened();
    };
    this.#ready.set(note, tab);
    this.#generations.set(tab, generation);
    return tab;
  }

  #forget(note: string, tab: TabCore): void {
    if (this.#ready.get(note) === tab) {
      this.#ready.delete(note);
      this.#made.delete(note);
    }
  }

  drop(note: string): void {
    const tab = this.#ready.get(note);
    this.#made.delete(note);
    this.#ready.delete(note);
    tab?.close();
  }

  /**
   * UC-006 step 7: the note `from` is `to` now; its core, with its text,
   * cursor and history, goes on under the new name.
   */
  rename(from: string, to: string): void {
    const made = this.#made.get(from);
    const tab = this.#ready.get(from);
    if (made) {
      this.#made.delete(from);
      this.#made.set(to, made);
    }
    if (tab) {
      this.#ready.delete(from);
      this.#ready.set(to, tab);
      tab.note = to;
      // The views the core reports name the note (`BlocksView.note`).
      tab.core.rename(to);
    }
  }

  /** Cores opened from now on belong to a new generation, which `dropBefore` keeps. */
  mark(): number {
    this.#generation += 1;
    return this.#generation;
  }

  /** The workspace stays after all: every core counts as of now. */
  adopt(): void {
    for (const made of this.#made.values()) {
      made.generation = this.#generation;
    }
    for (const tab of this.#ready.values()) {
      this.#generations.set(tab, this.#generation);
    }
  }

  /** Drops every core opened, or opening, before `mark`. */
  dropBefore(mark: number): void {
    this.#droppedBelow = Math.max(this.#droppedBelow, mark);
    for (const [note, made] of [...this.#made]) {
      if (made.generation < mark) {
        this.#made.delete(note);
      }
    }
    for (const [note, tab] of [...this.#ready]) {
      if ((this.#generations.get(tab) ?? 0) < mark) {
        this.#ready.delete(note);
        tab.close();
      }
    }
  }
}

const cores = new Cores();

/**
 * UC-006: notes being renamed: their cores stay while the rename runs,
 * though a tab bar without them may come first.
 */
const renaming = new Map<string, { done: Promise<void>; end: () => void }>();
/** UC-006 step 7: renamed cores that wait for a tab bar with their new name. */
const renamed = new Set<string>();

/**
 * UC-008 BR-008, A6, A7, M-13.2, M-13.4: what the cores mark: the search
 * overlay's text with its switches in every note (one text while it
 * replaces, UC-022), the find field's instead in the note it finds in (as
 * egui's `run_search`); a text with no word marks none.
 */
export type Searches = {
  overlay: FindSpec | null;
  find: { note: string; spec: FindSpec } | null;
};

let searches: Searches = { overlay: null, find: null };

/** Whether `spec` finds anything: one text when not empty, words when not only spaces. */
function searching(spec: FindSpec | null): spec is FindSpec {
  return spec !== null && (spec.oneText ? spec.text !== '' : spec.text.trim() !== '');
}

/** What the core of `note` marks; `null`: nothing. */
export function searchFor(note: string): FindSpec | null {
  const find = searches.find;
  if (find && find.note === note && searching(find.spec)) {
    return find.spec;
  }
  return searching(searches.overlay) ? searches.overlay : null;
}

/** The searches changed: every core marks what `searchFor` says. */
export function setSearches(next: Searches): void {
  searches = next;
  for (const tab of cores.all()) {
    tab.setSearch(searchFor(tab.note));
  }
}

/** UC-010 BR-001, UC-005 BR-019: close pairs is on (the built-in default). */
let closePairs = true;

/** UC-010 BR-004: close pairs is switched; every open core follows at once. */
export function setClosePairs(on: boolean): void {
  closePairs = on;
  for (const tab of cores.all()) {
    if (tab.alive) {
      try {
        tab.core.setClosePairs(on);
      } catch (error) {
        console.error('Close pairs could not be switched', error);
      }
    }
  }
}

/** The core of `note` once it is ready. */
export function coreOf(note: string): TabCore | undefined {
  return cores.get(note);
}

/** M-07.3: the hash of the text of `note`'s core; `null` without a live one. */
export function coreHash(note: string): string | null {
  const tab = cores.get(note);
  return tab && !tab.core.dead ? tab.core.hash() : null;
}

/**
 * M-07.3: the backend's revision the core of `note` is in step with: its
 * text is the backend's of that revision, nothing waits to be sent. `null`
 * while edits wait, or without a live core.
 */
export function inStepRevision(note: string): number | null {
  const tab = cores.get(note);
  if (!tab || tab.core.dead) {
    return null;
  }
  try {
    const rev = tab.core.syncedRev();
    return tab.core.inStep(rev) ? rev : null;
  } catch {
    // The sync of this core has not started.
    return null;
  }
}

/** M-07.3: whether the core of `note` holds the backend's text of revision `rev` and nothing of its own. */
export function coreInStep(note: string, rev: number): boolean {
  return inStepRevision(note) === rev;
}

/**
 * The core of `note`: opened from the backend's text (`ed_mirror`) the
 * first time, then kept while the tab is open.
 */
export async function openCore(note: string, mac: boolean): Promise<TabCore> {
  // UC-006: a view of the new name may show before the rename's answer
  // came (the tab bar can come first); it waits, so it gets the renamed
  // core instead of opening a second one (app-tester F8).
  if (!cores.get(note) && renaming.size > 0) {
    await Promise.all([...renaming.values()].map((held) => held.done));
  }
  return cores.open(note, mac);
}

/** The tab of `note` closed: its core goes. */
export function dropCore(note: string): void {
  renamed.delete(note);
  cores.drop(note);
}

/**
 * ADR-025: another workspace opens. Cores opened from now on are of it;
 * `dropCoresBefore` with the mark then drops those of the workspace left,
 * also when the new one's first tab opened its core before the answer came.
 */
export function markCores(): number {
  return cores.mark();
}

/** The workspace was left: the cores opened before `mark` go. */
export function dropCoresBefore(mark: number): void {
  cores.dropBefore(mark);
}

/** Every core goes (the tests start anew). */
export function dropAllCores(): void {
  paneViews.clear();
  for (const held of renaming.values()) {
    held.end();
  }
  renaming.clear();
  renamed.clear();
  cores.dropBefore(cores.mark());
}

/** The workspace stayed (the open was asked about or failed): its cores stay. */
export function keepCores(): void {
  cores.adopt();
}

/** UC-006: the note `note` is being renamed; its core stays meanwhile. */
export function holdCore(note: string): void {
  let end = () => {};
  const done = new Promise<void>((resolve) => {
    end = resolve;
  });
  renaming.set(note, { done, end });
}

/** UC-006: the rename of `note` ended, done or not. */
export function releaseCore(note: string): void {
  renaming.get(note)?.end();
  renaming.delete(note);
}

/**
 * UC-006 step 7: the note `from` is `to` now (`RenameAnswer.renamed`): its
 * core, with its text, cursor and history, goes on under the new name, as
 * egui's tab keeps its widget; it stays until a tab bar names it.
 */
export function renameCore(from: string, to: string): void {
  cores.rename(from, to);
  renameViews(from, to);
  renamed.add(to);
}

/**
 * ADR-025: only notes that are open keep their cores; one the backend
 * closed (deleted) never sends its old text over the file. A core being
 * renamed, or renamed and not yet in a tab bar, stays (`renameCore`).
 */
export function keepCoresOf(open: Iterable<string>): void {
  const keep = new Set(open);
  for (const note of [...renamed]) {
    if (keep.has(note)) {
      renamed.delete(note);
    }
  }
  for (const tab of cores.all()) {
    if (!keep.has(tab.note) && !renaming.has(tab.note) && !renamed.has(tab.note)) {
      cores.drop(tab.note);
    }
  }
}

/** `ed-enrich`: the backend's colours and figures for the facade of a note. */
export function enrichCore(sent: EnrichView): void {
  cores.get(sent.note)?.enrich(sent);
}

/** The backend rang for `note` (`ed-changed`). */
export function ringCore(note: string): void {
  cores.get(note)?.ring();
}

/**
 * Sends the edits of `note`, or of every note, and fetches the backend's;
 * why not, when some could not be sent.
 */
export async function flushCores(note?: string | null): Promise<string | null> {
  const tabs = note ? [cores.get(note)].filter((t): t is TabCore => !!t) : cores.all();
  const done = await Promise.all(tabs.map(async (tab) => ((await tab.flush()) ? null : tab)));
  const failed = done.find((tab) => tab !== null);
  return failed ? failed.lastError || 'the editor did not answer' : null;
}

/**
 * ADR-025 section 3.9: runs `work`, a command that reads the text (store,
 * close, switch the view or the workspace…), only after every edit reached
 * the backend; else answers with why not, as a failed command does. Every
 * new command that reads the text goes through here.
 */
export async function flushed<T>(work: () => Promise<Result<T>>): Promise<Result<T>> {
  const failure = await flushCores();
  if (failure !== null) {
    return {
      status: 'error',
      error: { kind: 'internal', message: handOverFailed(failure) },
    };
  }
  return work();
}

/**
 * `ed-flush`: sends as asked. When Underline closes, the close is answered
 * in any case: with the edits sent it goes on; else "Changes not stored"
 * asks, as when storing fails (UC-005 A14 step 3).
 */
export async function takeFlush(view: FlushView): Promise<void> {
  let failure: string | null = 'the editor did not answer';
  try {
    failure = await flushCores(view.note);
  } finally {
    if (view.closing) {
      await ipc.requestClose(failure === null);
      if (failure !== null) {
        reporter?.closeQuestion({
          tab: null,
          what: 'Underline',
          error: handOverFailed(failure),
          switching: false,
        });
      }
    }
  }
}
