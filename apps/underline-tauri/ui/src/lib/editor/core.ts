// ADR-025: the editor's core in the webview, compiled to WASM from
// `crates/underline-editor-wasm` (`npm run wasm` makes `../wasm/pkg/`). This
// is the only module that imports the package; it types the JSON the WASM
// speaks with the DTOs of `bindings.ts`, so the views see the same patches
// the backend's commands give (docs/migration/variante-b.md, section 2).

import init, { initSync, WasmMirror, WasmTab } from '../wasm/pkg/underline_editor_wasm';
import type {
  BlocksView,
  ClipAction,
  ClipContentView,
  Clipped,
  EditorInput,
  EditorPatch,
  EditorState,
  EnrichView,
  FacadeAt,
  FacadeInput,
  FacadePatch,
  FacadeState,
  FigureView,
  FindAnswer,
  FindStatus,
  FindStep,
  FormatName,
  Insertion,
  KeysStatusView,
  RemoteApplied,
  RowsView,
  CopyOnSelectView,
  SyncAnswer,
  SyncBatch,
  TabViews,
  VimChange,
  VimState,
} from '../bindings';
import type { InputModeName } from './keys';

let loading: Promise<void> | null = null;
let loadedIn: number | null = null;

/**
 * Loads and compiles the WASM once; the window starts it at its start and
 * the editors wait for it (ADR-023: the file ships with the frontend).
 */
export function loadEditorCore(): Promise<void> {
  if (!loading) {
    const start = performance.now();
    loading = init().then(() => {
      loadedIn = performance.now() - start;
    });
  }
  return loading;
}

/** How long loading and compiling the WASM took, in ms (Q-34; for the measuring build). */
export function editorCoreLoadTime(): number | null {
  return loadedIn;
}

/** Loads the WASM from its bytes at once, as the tests in Node do. */
export function loadEditorCoreSync(module: BufferSource | WebAssembly.Module): void {
  initSync({ module });
  loading = Promise.resolve();
}

/** What `EditorCore` calls of a tab of the WASM (a `WasmTab`). */
type Tab = Pick<
  WasmTab,
  | 'rename'
  | 'setMode'
  | 'setView'
  | 'setLooks'
  | 'setNow'
  | 'state'
  | 'input'
  | 'apply'
  | 'select'
  | 'fold'
  | 'keysStatus'
  | 'clipboard'
  | 'copyOnSelect'
  | 'overCopied'
  | 'insert'
  | 'facadeInsert'
  | 'facadeState'
  | 'facadeHead'
  | 'facadeInput'
  | 'facadeClipboard'
  | 'facadeKeys'
  | 'enrich'
  | 'facadeRefresh'
  | 'views'
  | 'setSearch'
  | 'find'
  | 'findStatus'
  | 'selectedText'
  | 'text'
  | 'hash'
  | 'syncedRev'
  | 'startSync'
  | 'takeOutbox'
  | 'acknowledge'
  | 'ring'
  | 'failed'
  | 'reset'
  | 'inStep'
  | 'idle'
  | 'free'
>;

/**
 * The tab's core failed for good (a trap of the WASM, such as a panic): it
 * was let go, and every later call throws this. The owner opens it anew from
 * the backend's text (ADR-025; wired with steps B4 and B5 through
 * `EditorCore.onDead`).
 */
export class EditorCoreDead extends Error {
  constructor(
    readonly note: string,
    readonly cause: unknown,
  ) {
    super(`The editor's core of ${note} failed and was closed; it must be opened again`);
    this.name = 'EditorCoreDead';
  }
}

/**
 * Whether `error` means the WASM is broken, not that an argument was wrong:
 * a trap (`unreachable` after a panic, out of memory) or the glue refusing a
 * tab that is still borrowed by a call that trapped.
 */
function broken(error: unknown): boolean {
  if (error instanceof WebAssembly.RuntimeError) {
    return true;
  }
  if (error instanceof Error) {
    return /recursive use of an object|null pointer passed to rust/.test(error.message);
  }
  return true;
}

/**
 * One open tab in the editor's core; see `crates/underline-editor-wasm`.
 *
 * An argument that is not what the core takes throws an `Error` with the
 * reason, and the tab goes on. When the core itself fails (see `broken`),
 * the tab is let go, `onDead` is told, and every later call throws
 * `EditorCoreDead`.
 *
 * The clock is the window's `performance.now()`, one epoch per tab and
 * only fed from here: `setNow` before an input, `views(now)` after it.
 */
export class EditorCore {
  #tab: Tab | null;
  #dead: EditorCoreDead | null = null;
  /** The note; a rename gives it the new one (UC-006 step 7). */
  note: string;
  /** Told once when the core of this tab failed; the owner opens it anew. */
  onDead: ((dead: EditorCoreDead) => void) | null = null;

  private constructor(tab: Tab, note: string) {
    this.#tab = tab;
    this.note = note;
  }

  /** The tab of `note` holding `text`; the core must be loaded. */
  static open(note: string, text: string, format: FormatName, mac: boolean): EditorCore {
    return new EditorCore(new WasmTab(note, text, format, mac), note);
  }

  /** For the tests: an `EditorCore` over `tab`. */
  static over(tab: Tab, note: string): EditorCore {
    return new EditorCore(tab, note);
  }

  /** UC-006 step 7: the note is `note` now; the views it reports name it so. */
  rename(note: string): void {
    this.#call((tab) => tab.rename(note));
    this.note = note;
  }

  /** Whether the core of this tab failed and was let go. */
  get dead(): boolean {
    return this.#dead !== null;
  }

  /** Runs `work` on the tab; a failure of the core lets the tab go. */
  #call<T>(work: (tab: Tab) => T): T {
    const tab = this.#tab;
    if (this.#dead || !tab) {
      throw this.#dead ?? new EditorCoreDead(this.note, 'freed');
    }
    try {
      return work(tab);
    } catch (error) {
      if (!broken(error)) {
        throw error;
      }
      this.#dead = new EditorCoreDead(this.note, error);
      this.#tab = null;
      try {
        tab.free();
      } catch {
        // The broken tab may not even be let go; its memory stays.
      }
      console.error(this.#dead.message, error);
      this.onDead?.(this.#dead);
      throw this.#dead;
    }
  }

  /** `#call` with the JSON answer parsed. */
  #json<T>(work: (tab: Tab) => string): T {
    return this.#call((tab) => JSON.parse(work(tab)) as T);
  }

  /** M-11: the input mode of the settings. */
  setMode(mode: InputModeName): void {
    this.#call((tab) => tab.setMode(mode));
  }

  /** M-07.2: the facade or the source view is shown. */
  setView(facade: boolean): void {
    this.#call((tab) => tab.setView(facade));
  }

  /** The theme by its name, and whether it is dark. */
  setLooks(theme: string, dark: boolean): void {
    this.#call((tab) => tab.setLooks(theme, dark));
  }

  /** The window's clock, `performance.now()`, before an input; never back. */
  setNow(ms: number): void {
    this.#call((tab) => tab.setNow(ms));
  }

  state(): EditorState {
    return this.#json((tab) => tab.state());
  }

  input(input: EditorInput): EditorPatch {
    return this.#json((tab) => tab.input(JSON.stringify(input)));
  }

  apply(
    base: number,
    changes: VimChange[],
    anchor: number,
    head: number,
    newStep: boolean,
  ): EditorPatch | null {
    return this.#json((tab) => tab.apply(base, JSON.stringify(changes), anchor, head, newStep));
  }

  /** Sets the selection; gives the place for the status line. */
  select(rev: number, anchor: number, head: number): string {
    return this.#json((tab) => tab.select(rev, anchor, head));
  }

  fold(line: number): EditorPatch {
    return this.#json((tab) => tab.fold(line));
  }

  keysStatus(vim: VimState | null): KeysStatusView | null {
    return this.#json((tab) => tab.keysStatus(JSON.stringify(vim)));
  }

  /** Copy, cut, or paste what the clipboard holds (`content`) in the source view. */
  clipboard(action: ClipAction, content: ClipContentView | null): Clipped<EditorPatch> {
    return this.#json((tab) => tab.clipboard(JSON.stringify(action), JSON.stringify(content)));
  }

  /**
   * M-10.7, FR-031: the selection to put on the clipboard, once per
   * selection, when copy on select is `on`; and whether the copied
   * selection is gone (`forget`).
   */
  copyOnSelect(on: boolean): CopyOnSelectView {
    return this.#json((tab) => tab.copyOnSelect(on));
  }

  /** BR-011: the selection is the one copy on select copied. */
  overCopied(): boolean {
    return this.#call((tab) => tab.overCopied());
  }

  /** M-10.3 to M-10.6: puts `insertion` in at the source view's selection. */
  insert(insertion: Insertion): EditorPatch {
    return this.#json((tab) => tab.insert(JSON.stringify(insertion)));
  }

  /** M-10.3 to M-10.6: puts `insertion` in at the facade's selection `at`. */
  facadeInsert(at: FacadeAt, insertion: Insertion): FacadePatch {
    return this.#json((tab) => tab.facadeInsert(JSON.stringify(at), JSON.stringify(insertion)));
  }

  facadeState(): FacadeState {
    return this.#json((tab) => tab.facadeState());
  }

  /** The facade without its blocks (`blocks` empty): the window holds those of its `version`. */
  facadeHead(): FacadeState {
    return this.#json((tab) => tab.facadeHead());
  }

  facadeInput(
    at: FacadeAt,
    input: FacadeInput,
    rows: RowsView | null,
    clipboard: string | null,
  ): FacadePatch {
    return this.#json((tab) =>
      tab.facadeInput(JSON.stringify(at), JSON.stringify(input), JSON.stringify(rows), clipboard),
    );
  }

  facadeClipboard(
    at: FacadeAt,
    action: ClipAction,
    plain: boolean,
    content: ClipContentView | null,
  ): Clipped<FacadePatch> {
    return this.#json((tab) =>
      tab.facadeClipboard(
        JSON.stringify(at),
        JSON.stringify(action),
        plain,
        JSON.stringify(content),
      ),
    );
  }

  /** ADR-025: the backend's colours and figures (`ed-enrich`); the changed blocks. */
  enrich(sent: EnrichView): BlocksView[] {
    return this.#json((tab) => tab.enrich(JSON.stringify(sent)));
  }

  /** ADR-025: the facade after a change of the backend. */
  facadeRefresh(): FacadePatch {
    return this.#json((tab) => tab.facadeRefresh());
  }

  facadeKeys(): KeysStatusView | null {
    return this.#json((tab) => tab.facadeKeys());
  }

  /** The sections and the marked hits at `ms` (`performance.now()`), once they changed. */
  views(ms: number): TabViews {
    return this.#json((tab) => tab.views(ms));
  }

  /** UC-008 BR-008, M-13.2: the words of `text` are marked from now on; `null`: none. */
  setSearch(text: string | null): void {
    this.#call((tab) => tab.setSearch(JSON.stringify(text)));
  }

  /** UC-008 A6, M-13.4: a step of the find field holding `text`; `null`: nothing found. */
  find(text: string, step: FindStep): FindAnswer | null {
    return this.#json((tab) => tab.find(JSON.stringify(text), JSON.stringify(step)));
  }

  /** M-13.4: the hit selected and how many there are. */
  findStatus(): FindStatus {
    return this.#json((tab) => tab.findStatus());
  }

  /** M-13.4: the selected markup, for the find field. */
  selectedText(): string {
    return this.#call((tab) => tab.selectedText());
  }

  text(): string {
    return this.#call((tab) => tab.text());
  }

  hash(): string {
    return this.#call((tab) => tab.hash());
  }

  /** M-07.3: the backend's revision the sync last confirmed. */
  syncedRev(): number {
    return this.#call((tab) => tab.syncedRev());
  }

  /** ADR-025: the sync starts from the backend's text at `rev` (`ed_mirror`). */
  startSync(rev: number): void {
    this.#call((tab) => tab.startSync(rev));
  }

  /** The edits to send (`ed_sync`), unless a batch is on its way or nothing waits. */
  takeOutbox(): SyncBatch | null {
    return this.#json((tab) => tab.takeOutbox());
  }

  /** Takes the backend's answer in: the patch for the source view, or a desync. */
  acknowledge(answer: SyncAnswer): RemoteApplied {
    return this.#json((tab) => tab.acknowledge(JSON.stringify(answer)));
  }

  /** The backend rang (`ed-changed`, `ed-flush`): the next outbox fetches. */
  ring(): void {
    this.#call((tab) => tab.ring());
  }

  /** The last `ed_sync` failed: its edits go again with the next outbox. */
  failed(): void {
    this.#call((tab) => tab.failed());
  }

  /**
   * After `ed_resync`: the backend took `sent`, the text sent, at `rev`;
   * what was typed while it travelled stays to be sent.
   */
  reset(sent: string, rev: number): void {
    this.#call((tab) => tab.reset(sent, rev));
  }

  /** Whether a result of the backend for `rev` applies as it is. */
  inStep(rev: number): boolean {
    return this.#call((tab) => tab.inStep(rev));
  }

  /** Whether nothing waits to be sent or fetched. */
  idle(): boolean {
    return this.#call((tab) => tab.idle());
  }

  /** Lets the tab's memory in the WASM go; later calls throw. */
  free(): void {
    const tab = this.#tab;
    this.#tab = null;
    tab?.free();
  }
}

/**
 * ADR-025: the backend's mirror of a note (`underline_editor_core::sync::Server`)
 * in the webview, for the mock backend of the dev server: the same
 * revisions, transforms and reload barrier as the real backend.
 */
export class MirrorCore {
  readonly #mirror: WasmMirror;

  constructor(text: string) {
    this.#mirror = new WasmMirror(text);
  }

  get rev(): number {
    return this.#mirror.rev();
  }

  text(): string {
    return this.#mirror.text();
  }

  /** The answer to `batch`, and the version a late batch took the place of (UC-004 A9). */
  receive(batch: SyncBatch): { answer: SyncAnswer; reverted: string | null } {
    return JSON.parse(this.#mirror.receive(JSON.stringify(batch))) as {
      answer: SyncAnswer;
      reverted: string | null;
    };
  }

  /** A change of the backend's own, in chars; the new revision, if it fits. */
  backendEdit(at: number, remove: number, insert: string, sealed: boolean): number | undefined {
    return this.#mirror.backendEdit(at, remove, insert, sealed);
  }

  /** A change from outside; the new revision. */
  reload(text: string): number {
    return this.#mirror.reload(text);
  }

  /**
   * For the mock: the backend's `ed-enrich` of this text, the images'
   * figures from `images` by target, every diagram with `diagram`.
   */
  enrichment(
    note: string,
    markdown: boolean,
    theme: string,
    dark: boolean,
    images: Record<string, FigureView>,
    diagram: FigureView,
  ): EnrichView {
    return JSON.parse(
      this.#mirror.enrichment(
        note,
        markdown,
        theme,
        dark,
        JSON.stringify(images),
        JSON.stringify(diagram),
      ),
    ) as EnrichView;
  }

  resync(text: string): SyncAnswer {
    return JSON.parse(this.#mirror.resync(text)) as SyncAnswer;
  }
}
