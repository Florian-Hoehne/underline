// What the facade needs of the backend (ADR-024), and where its pictures
// come from (ADR-023): only through the app's own protocol `ul-asset`.

import {
  commands,
  type EditingViewName,
  type EditorInput,
  type EditorPatch,
  type FacadeState,
  type Platform,
  type UiError,
} from '../bindings';

/** The facade's commands. */
export interface FacadeBackend {
  /** The facade of `note` with its cursor (`fa_open`). */
  open(note: string): Promise<FacadeState>;
  /** Sets the selection in web units of revision `rev`; gives the place for the status line. */
  select(note: string, rev: number, anchor: number, head: number): Promise<string>;
  /** M-08.8: folds or expands the section of the heading on `line`. */
  fold(note: string, line: number): Promise<EditorPatch>;
  /**
   * An input the facade takes before feature F5b: the toolbar (M-10.2) and
   * the folding keys (M-08.8). Typing comes with F5b.
   */
  input(note: string, input: EditorInput): Promise<EditorPatch>;
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/** The facade's commands of the backend. */
export const tauriFacadeBackend: FacadeBackend = {
  open: (note) => unwrap(commands.faOpen(note)),
  select: (note, rev, anchor, head) => unwrap(commands.edSelect(note, rev, anchor, head)),
  fold: (note, line) => unwrap(commands.edFold(note, line)),
  input: (note, input) => unwrap(commands.edInput(note, input)),
};

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
