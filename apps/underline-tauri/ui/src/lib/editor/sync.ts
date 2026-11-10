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
  /** The sections and the Nano hits, once they changed. */
  views(views: TabViews): void;
  /** The core confirmed a revision: results of the backend may apply now. */
  synced(): void;
  /** The core failed and was opened anew: the view shows its text again. */
  reopened(): void;
  /** The facade's blocks took the backend's colours and figures (`ed-enrich`). */
  blocks?(changes: BlocksView[]): void;
};

/** Runs `work` after the next frame was painted (the spike's method). */
function afterFrame(work: () => void): void {
  const frame = globalThis.requestAnimationFrame ?? ((f: () => void) => setTimeout(f, 16));
  frame(() => setTimeout(work, 0));
}

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

/** The status line says so after this many failures in a row. */
const REPORT_AFTER = 3;
/** The longest wait between two tries, in ms. */
const LONGEST_WAIT = 5000;
/** How long a flush tries before it gives up, in ms. */
const FLUSH_AT_MOST = 3000;

/** One note's core and its sync. */
export class TabCore {
  readonly note: string;
  readonly core: EditorCore;
  /** The note's format (M-09.14: pasted HTML becomes its markup). */
  readonly format: FormatName;
  listener: CoreListener | null = null;
  #scheduled = false;
  #viewsScheduled = false;
  #sending: Promise<boolean> | null = null;
  #retry: ReturnType<typeof setTimeout> | null = null;
  #failures = 0;
  #reported = false;
  #lastError = '';
  /** The tab closed: everything stops, silently. */
  #closed = false;

  constructor(note: string, core: EditorCore, format: FormatName = 'Markdown') {
    this.note = note;
    this.core = core;
    this.format = format;
  }

  /** Whether the core can still be used. */
  get alive(): boolean {
    return !this.#closed && !this.core.dead;
  }

  /** Why the last send failed. */
  get lastError(): string {
    return this.#lastError;
  }

  /** After an input: the edits go out after the frame, the views follow. */
  afterInput(): void {
    this.schedule();
    this.scheduleViews();
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

  /** Works out the sections and hits after the frame. */
  scheduleViews(): void {
    if (this.#viewsScheduled || !this.alive) {
      return;
    }
    this.#viewsScheduled = true;
    afterFrame(() => {
      this.#viewsScheduled = false;
      if (!this.alive) {
        return;
      }
      try {
        const start = performance.now();
        const views = this.core.views(start);
        probe?.('views', performance.now() - start);
        if (views.sections || views.found) {
          this.listener?.views(views);
        }
      } catch (error) {
        this.#problem(error);
      }
    });
  }

  /** ADR-025: the backend's colours and figures of the facade's blocks. */
  enrich(sent: EnrichView): void {
    if (!this.alive) {
      return;
    }
    try {
      const changes = this.core.enrich(sent);
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
    this.core.ring();
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
      batch = this.core.takeOutbox();
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
        this.listener?.patch(applied.patch);
        this.scheduleViews();
      }
      probe?.('acknowledge', performance.now() - taken);
    } catch (error) {
      this.#problem(error);
      return false;
    }
    this.#succeeded();
    this.listener?.synced();
    if (this.alive && !this.core.idle()) {
      this.schedule();
    }
    return true;
  }

  /**
   * ADR-025 section 3.10: the copies went apart; the window's text wins.
   * What is typed while it travels stays to be sent.
   */
  async #resync(): Promise<boolean> {
    const text = this.core.text();
    try {
      const answer = await ipc.resync(this.note, text);
      if (this.alive) {
        this.core.reset(text, answer.rev);
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
      this.core.failed();
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
      this.core.ring();
    } catch (error) {
      this.#problem(error);
      return false;
    }
    const end = performance.now() + atMost;
    while (this.alive && performance.now() < end) {
      if (this.#sending) {
        await this.#sending;
      } else if (this.core.idle()) {
        return true;
      } else if (!(await this.send())) {
        await sleep(Math.min(200, Math.max(0, end - performance.now())));
      }
    }
    return this.alive && this.core.idle();
  }

  /** The tab closed: the core goes, and nothing more is said of it. */
  close(): void {
    this.#closed = true;
    if (this.#retry !== null) {
      clearTimeout(this.#retry);
      this.#retry = null;
    }
    this.listener = null;
    this.core.free();
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
    core.onDead = () => {
      // Q-40: opened anew from the backend's text; the edits not confirmed
      // are lost, and the status line says so.
      console.error(`The editor's core of ${note} failed; it is opened again from the stored text`);
      reporter?.dead(note);
      this.#forget(note, tab);
      tab.listener?.reopened();
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

/** The core of `note` once it is ready. */
export function coreOf(note: string): TabCore | undefined {
  return cores.get(note);
}

/**
 * The core of `note`: opened from the backend's text (`ed_mirror`) the
 * first time, then kept while the tab is open.
 */
export function openCore(note: string, mac: boolean): Promise<TabCore> {
  return cores.open(note, mac);
}

/** The tab of `note` closed: its core goes. */
export function dropCore(note: string): void {
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
  cores.dropBefore(cores.mark());
}

/** The workspace stayed (the open was asked about or failed): its cores stay. */
export function keepCores(): void {
  cores.adopt();
}

/**
 * ADR-025: only notes that are open keep their cores; one the backend
 * closed (renamed, deleted) never sends its old text over the file.
 */
export function keepCoresOf(open: Iterable<string>): void {
  const keep = new Set(open);
  for (const tab of cores.all()) {
    if (!keep.has(tab.note)) {
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
        reporter?.closeQuestion({ tab: null, what: 'Underline', error: handOverFailed(failure) });
      }
    }
  }
}
