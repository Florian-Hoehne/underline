// ADR-025: the editor's core in the webview, compiled to WASM from
// `crates/underline-editor-wasm` (`npm run wasm` makes `../wasm/pkg/`). This
// is the only module that imports the package; it types the JSON the WASM
// speaks with the DTOs of `bindings.ts`, so the views see the same patches
// the backend's commands give (docs/migration/variante-b.md, section 2).

import init, { initSync, WasmTab } from '../wasm/pkg/underline_editor_wasm';
import type {
  ClipAction,
  Clipped,
  EditorInput,
  EditorPatch,
  EditorState,
  FacadeAt,
  FacadeInput,
  FacadePatch,
  FacadeState,
  FormatName,
  KeysStatusView,
  RowsView,
  TabViews,
  VimChange,
  VimState,
} from '../bindings';
import type { InputModeName } from './keys';

let loading: Promise<void> | null = null;

/**
 * Loads and compiles the WASM once; the window starts it at its start and
 * the editors wait for it (ADR-023: the file ships with the frontend).
 */
export function loadEditorCore(): Promise<void> {
  loading ??= init().then(() => undefined);
  return loading;
}

/** Loads the WASM from its bytes at once, as the tests in Node do. */
export function loadEditorCoreSync(module: BufferSource | WebAssembly.Module): void {
  initSync({ module });
  loading = Promise.resolve();
}

/** What `EditorCore` calls of a tab of the WASM (a `WasmTab`). */
type Tab = Pick<
  WasmTab,
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
  | 'facadeState'
  | 'facadeInput'
  | 'facadeClipboard'
  | 'facadeKeys'
  | 'views'
  | 'text'
  | 'hash'
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
  readonly note: string;
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

  /** Copy, cut, or paste the clipboard's `text` in the source view. */
  clipboard(action: ClipAction, text: string | null): Clipped<EditorPatch> {
    return this.#json((tab) => tab.clipboard(JSON.stringify(action), text));
  }

  facadeState(): FacadeState {
    return this.#json((tab) => tab.facadeState());
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
    text: string | null,
  ): Clipped<FacadePatch> {
    return this.#json((tab) =>
      tab.facadeClipboard(JSON.stringify(at), JSON.stringify(action), plain, text),
    );
  }

  facadeKeys(): KeysStatusView | null {
    return this.#json((tab) => tab.facadeKeys());
  }

  /** The sections and the Nano hits at `ms` (`performance.now()`), once they changed. */
  views(ms: number): TabViews {
    return this.#json((tab) => tab.views(ms));
  }

  text(): string {
    return this.#call((tab) => tab.text());
  }

  hash(): string {
    return this.#call((tab) => tab.hash());
  }

  /** Lets the tab's memory in the WASM go; later calls throw. */
  free(): void {
    const tab = this.#tab;
    this.#tab = null;
    tab?.free();
  }
}
