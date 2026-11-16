// ADR-025: the editor's core in the webview, compiled to WASM from
// `crates/underline-editor-wasm` (`npm run wasm` makes `../wasm/pkg/`). This
// is the only module that imports the package; it types the JSON the WASM
// speaks with the DTOs of `bindings.ts`, so the views see the same patches
// the backend's commands give (docs/migration/variante-b.md, section 2).

import init, {
  initSync,
  lineTarget as wasmLineTarget,
  WasmMirror,
  WasmSuggestions,
  WasmTab,
} from '../wasm/pkg/underline_editor_wasm';
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
  FormulaView,
  FindAnswer,
  FindSpec,
  FindStatus,
  FindStep,
  FormatName,
  Insertion,
  KeysStatusView,
  LineTargetAnswer,
  LineTargetView,
  LinkChoice,
  LinkContextView,
  RemoteApplied,
  Replaced,
  RowsView,
  CopyOnSelectView,
  SyncAnswer,
  SyncBatch,
  TabViews,
  TextStatisticsView,
  NoteHeadView,
  AttributeChangeView,
  AttributeEdited,
  VimChange,
  VimState,
} from '../bindings';
import type { InputModeName } from './keys';

let loading: Promise<void> | null = null;
let loadedIn: number | null = null;
/** The WASM is loaded and compiled: its calls may be made at once. */
let loaded = false;

/**
 * Loads and compiles the WASM once; the window starts it at its start and
 * the editors wait for it (ADR-023: the file ships with the frontend).
 */
export function loadEditorCore(): Promise<void> {
  if (!loading) {
    const start = performance.now();
    loading = init().then(() => {
      loadedIn = performance.now() - start;
      loaded = true;
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
  loaded = true;
}

/** Whether the WASM is loaded, so its calls may be made at once. */
export function editorCoreLoaded(): boolean {
  return loaded;
}

/**
 * FR-076, FR-077 (UC-004 BR-014, UC-023 BR-002): the notes of go to note or
 * the commands of the command palette, ranked against what the user typed
 * by the rule of `underline_markup::suggest` in the WASM. Made once per list
 * (the names are lowered then), ranked on every key (NFR-018); `free` lets
 * the WASM's copy go. Needs the WASM loaded (`editorCoreLoaded`).
 */
export class SuggestionList {
  #wasm: WasmSuggestions | null;

  /** `names[i]` with its context `contexts[i]` (folder or menu), in the order of an empty field. */
  constructor(names: string[], contexts: string[]) {
    this.#wasm = new WasmSuggestions(names, contexts);
  }

  /** The indices of the suggestions matching `typed`, best first; those in `last` after all others. */
  rank(typed: string, last: readonly number[] = []): number[] {
    if (this.#wasm === null) {
      return [];
    }
    return Array.from(this.#wasm.rank(typed, Uint32Array.from(last)));
  }

  /** Lets the WASM's copy go; `rank` finds nothing afterwards. */
  free(): void {
    this.#wasm?.free();
    this.#wasm = null;
  }
}

/**
 * FR-086, UC-004 A16, A17: the line, and the column after a colon, that
 * `text` names, or why it is no line number. Needs the WASM loaded.
 */
export function lineTarget(text: string): LineTargetAnswer {
  return JSON.parse(wasmLineTarget(text)) as LineTargetAnswer;
}

/** What `EditorCore` calls of a tab of the WASM (a `WasmTab`). */
type Tab = Pick<
  WasmTab,
  | 'rename'
  | 'setMode'
  | 'setView'
  | 'setLooks'
  | 'setClosePairs'
  | 'lineCount'
  | 'textStatistics'
  | 'noteHead'
  | 'attribute'
  | 'facadeAttribute'
  | 'lineNote'
  | 'goToLine'
  | 'facadeGoToLine'
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
  | 'linkContext'
  | 'completeLink'
  | 'facadeLinkContext'
  | 'facadeCompleteLink'
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
  | 'replace'
  | 'facadeReplace'
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
> &
  // FR-097: the views of one note; a tab of the tests may leave them out.
  Partial<Pick<WasmTab, 'viewOpen' | 'viewClose' | 'useView' | 'sourceSince'>>;

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

  /** FR-097: the view `id` of the window shows the note from now on. */
  viewOpen(id: number): void {
    this.#call((tab) => tab.viewOpen?.(id));
  }

  /** FR-097: the view `id` is gone. */
  viewClose(id: number): void {
    this.#call((tab) => tab.viewClose?.(id));
  }

  /**
   * UC-005 BR-016: the calls from now on are those of the view `id`, with
   * its own cursor, folds and blocks; `input` when the user acts in it.
   */
  useView(id: number, input: boolean): void {
    this.#call((tab) => tab.useView?.(id, input));
  }

  /**
   * FR-097: the changes that bring the source view of the view calling now
   * from revision `base` to the text as it is; `null`: it takes the whole
   * text again (`state`).
   */
  sourceSince(base: number): EditorPatch | null {
    return this.#call((tab) =>
      tab.sourceSince ? (JSON.parse(tab.sourceSince(base)) as EditorPatch | null) : null,
    );
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

  /** UC-010 BR-001, UC-005 BR-019: close pairs is on or off. */
  setClosePairs(on: boolean): void {
    this.#call((tab) => tab.setClosePairs(on));
  }

  /** UC-004 A16 step 1: how many lines the note has. */
  lineCount(): number {
    return this.#call((tab) => tab.lineCount());
  }

  /**
   * UC-004 BR-020 (FR-090): the words, characters and reading time of the
   * note, and of the selection while text is selected; kept by the core for
   * an unchanged text and selection, so asking often costs nothing.
   */
  textStatistics(): TextStatisticsView {
    return this.#json((tab) => tab.textStatistics());
  }

  /**
   * UC-024 step 2, FR-091: the note attributes and the tags of the note,
   * kept by the core for an unchanged text, so asking often costs nothing.
   */
  noteHead(): NoteHeadView {
    return this.#json((tab) => tab.noteHead());
  }

  /** UC-024 step 4: `change` written in the source view, one undo step; or why not. */
  attribute(change: AttributeChangeView): AttributeEdited<EditorPatch> {
    return this.#json((tab) => tab.attribute(JSON.stringify(change)));
  }

  /** UC-024 step 4: `change` written in the facade at the selection `at`; or why not. */
  facadeAttribute(at: FacadeAt, change: AttributeChangeView): AttributeEdited<FacadePatch> {
    return this.#json((tab) => tab.facadeAttribute(JSON.stringify(at), JSON.stringify(change)));
  }

  /** UC-004 BR-017: what the status bar says when `target` lies beyond the note; `null`: nothing. */
  lineNote(target: LineTargetView): string | null {
    return this.#json((tab) => tab.lineNote(JSON.stringify(target)));
  }

  /** UC-004 A16, BR-017: the source view's cursor to the line and column of `target`. */
  goToLine(target: LineTargetView): EditorPatch {
    return this.#json((tab) => tab.goToLine(JSON.stringify(target)));
  }

  /** UC-004 A16, BR-017: the facade's cursor to the block holding the line of `target`. */
  facadeGoToLine(at: FacadeAt, target: LineTargetView): FacadePatch {
    return this.#json((tab) => tab.facadeGoToLine(JSON.stringify(at), JSON.stringify(target)));
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

  /**
   * FR-080, UC-005 A23: the place of a link typed at the source view's
   * cursor; `manual` for Ctrl+Space. `null`: none.
   */
  linkContext(manual: boolean): LinkContextView | null {
    return this.#json((tab) => tab.linkContext(manual));
  }

  /** FR-080, UC-005 A23 step 3: writes the link chosen in the source view; `null`: the text changed. */
  completeLink(choice: LinkChoice): EditorPatch | null {
    return this.#json((tab) => tab.completeLink(JSON.stringify(choice)));
  }

  /** FR-080: the place of a link typed at the facade's selection `at` (`[[`, or Ctrl+Space). */
  facadeLinkContext(at: FacadeAt, manual: boolean): LinkContextView | null {
    return this.#json((tab) => tab.facadeLinkContext(JSON.stringify(at), manual));
  }

  /** FR-080, BR-018: writes the link chosen at the facade's selection `at`, `[[` and all. */
  facadeCompleteLink(at: FacadeAt, choice: LinkChoice): FacadePatch | null {
    return this.#json((tab) => tab.facadeCompleteLink(JSON.stringify(at), JSON.stringify(choice)));
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

  /**
   * UC-008 BR-008, BR-011, M-13.2: the places of `spec` (its text with its
   * search switches) are marked from now on; `null`: none.
   */
  setSearch(spec: FindSpec | null): void {
    this.#call((tab) => tab.setSearch(JSON.stringify(spec)));
  }

  /** UC-008 A6, A7, M-13.4: a step of the find field holding `spec`; `null`: nothing found. */
  find(spec: FindSpec, step: FindStep): FindAnswer | null {
    return this.#json((tab) => tab.find(JSON.stringify(spec), JSON.stringify(step)));
  }

  /** UC-005 A20, A21: replaces the selected occurrence of `spec`, or all, in the source view. */
  replace(spec: FindSpec, replacement: string, all: boolean): Replaced<EditorPatch> {
    return this.#json((tab) => tab.replace(JSON.stringify(spec), replacement, all));
  }

  /** UC-005 A20, A21, BR-017: the same in the facade, in the text it shows. */
  facadeReplace(spec: FindSpec, replacement: string, all: boolean): Replaced<FacadePatch> {
    return this.#json((tab) => tab.facadeReplace(JSON.stringify(spec), replacement, all));
  }

  /**
   * M-13.4, UC-005 A22: the hit selected, how many there are, and what is
   * wrong with the search text or, while replacing, with `replacement`.
   */
  findStatus(replacement: string | null = null): FindStatus {
    return this.#json((tab) => tab.findStatus(JSON.stringify(replacement)));
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
   * figures from `images` by target, every diagram with `diagram`, every
   * formula with the placeholder `formula` (FR-098).
   */
  enrichment(
    note: string,
    markdown: boolean,
    theme: string,
    dark: boolean,
    images: Record<string, FigureView>,
    diagram: FigureView,
    formula: FormulaView,
  ): EnrichView {
    return JSON.parse(
      this.#mirror.enrichment(
        note,
        markdown,
        theme,
        dark,
        JSON.stringify(images),
        JSON.stringify(diagram),
        JSON.stringify(formula),
      ),
    ) as EnrichView;
  }

  resync(text: string): SyncAnswer {
    return JSON.parse(this.#mirror.resync(text)) as SyncAnswer;
  }
}
