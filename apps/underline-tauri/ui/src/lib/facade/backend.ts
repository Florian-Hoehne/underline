// What the facade needs of its backend, the window's editor core
// (ADR-025, `wasmBackend.ts`), and where its pictures come from (ADR-023):
// only through the app's own protocol `ul-asset`.

import {
  commands,
  type BlockView,
  type ClipAction,
  type EditingViewName,
  type EditorPatch,
  type FacadeAt,
  type FacadeInput,
  type FacadePatch,
  type FacadeState,
  type FindAnswer,
  type FindStep,
  type Insertion,
  type KeysStatusView,
  type Platform,
  type RowsView,
  type UiError,
} from '../bindings';
import type { Clipboarded } from '../editor/clipboard';
import type { CoreListener } from '../editor/sync';
import type { InputModeName } from '../editor/keys';

/** The facade's backend. */
export interface FacadeBackend {
  /** The facade of `note` with its cursor, its keys those of `mode` when given. */
  open(note: string, mode?: InputModeName): Promise<FacadeState>;
  /** M-11: the keys of the facade follow the input mode `mode` of the settings. */
  setMode(note: string, mode: InputModeName): void;
  /** Sets the selection in web units of revision `rev`; gives the place for the status line. */
  select(note: string, rev: number, anchor: number, head: number): Promise<string>;
  /** M-08.8: folds or expands the section of the heading on `line`. */
  fold(note: string, line: number): Promise<EditorPatch>;
  /**
   * UC-008 A6, M-13.4: a step of the find field holding `text`: the hit it
   * selected in the core, or `null` when nothing is found.
   */
  find(note: string, text: string, step: FindStep): Promise<FindAnswer | null>;
  /**
   * UC-005, M-09.10 to M-09.13: one input at the selection `at`, with the
   * rows measured around the cursor and, when the Vim keys asked, the
   * clipboard's text.
   */
  input(
    note: string,
    at: FacadeAt,
    input: FacadeInput,
    rows: RowsView | null,
    clipboard: string | null,
  ): Promise<FacadePatch>;
  /**
   * M-09.14: copy, cut or paste (`plain`: as typed) at the selection `at`
   * through the system clipboard: copying gives the markup and its HTML, a
   * paste HTML of other programs as markup.
   */
  clipboard(
    note: string,
    at: FacadeAt,
    action: ClipAction,
    plain: boolean,
  ): Promise<Clipboarded<FacadePatch>>;
  /** M-10.6: stores the clipboard's image next to the note and inserts it at `at`. */
  pasteImage(note: string, at: FacadeAt, over: boolean): Promise<FacadePatch | null>;
  /** M-10.3 to M-10.5: puts `insertion` in at the selection `at`. */
  insert(note: string, at: FacadeAt, insertion: Insertion): Promise<FacadePatch>;
  /** M-10.7: the selection, once settled, goes to the clipboard (copy on select). */
  copySelection(note: string): Promise<void>;
  /** M-10.5: the hash of the note's text in the window's core; `null` without one. */
  textHash(note: string): string | null;
  /** M-11.4: the status of the Vim or Nano keys in the facade. */
  keys(note: string): Promise<KeysStatusView | null>;
  /** M-11.2: the system clipboard's text, for Vim's `"+`. */
  readClipboard(): Promise<string | null>;
  /** M-11.2: puts what Vim's `"+` yanked on the system clipboard. */
  writeClipboard(text: string): Promise<void>;
  /**
   * ADR-025: the text lives in the window's editor core
   * (`wasmBackend.ts`): its changes, sections and the backend's colours and
   * figures come from `attach`.
   */
  attach(note: string, listener: CoreListener): () => void;
  /** ADR-025: the facade after a change of the backend. */
  refresh(note: string): Promise<FacadePatch>;
  /**
   * ADR-025: the facade of `note` is no longer shown; `kept` are the blocks
   * it drew, of their version, which `open` may take again.
   */
  leave(note: string, kept: { version: number; blocks: BlockView[] } | null): void;
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/** M-07.2: the tab of `note` shows `view` from now on. */
export async function setEditingView(note: string, view: EditingViewName): Promise<void> {
  await unwrap(commands.tabSetView(note, view));
}

/**
 * ADR-023: the address of the asset `path` (such as `diagram/….svg`) of the
 * app's own protocol: `ul-asset://localhost/…`, on Windows (WebView2)
 * `http://ul-asset.localhost/…`.
 */
export function assetUrl(path: string, platform: Platform): string {
  // The mock backend of the browser (plan section 10) gives its pictures inline.
  if (path.startsWith('data:')) {
    return path;
  }
  return platform === 'windows'
    ? `http://ul-asset.localhost/${path}`
    : `ul-asset://localhost/${path}`;
}
