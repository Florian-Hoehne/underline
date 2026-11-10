// M-11.2, A-01 (user decision F-2): the Vim keys of the source view are
// `@replit/codemirror-vim`. The backend owns the text (ADR-024), so this
// module ties `codemirror-vim` to it:
//
// - `u`, Ctrl+R, `:undo` and `:redo` use the backend's history, the one
//   history of all input modes and both views (M-10.1).
// - `:w`, `:q`, `:q!`, `:wq` and `:x` store the note or close its tab
//   (M-10.9), as the egui Vim keys do.
// - The registers `"+` and `"*` are the system clipboard through the
//   backend (ADR-014); the webview's own clipboard is never used.
// - The status line learns the mode, what is typed but not done yet, and
//   whether search hits are marked (M-11.4); Escape and `:noh` end the
//   marking, as `codemirror-vim` does by itself.
//
// The changes `codemirror-vim` makes to the text are sent to the backend by
// the source view to the editor core (`sourceView.ts`, `apply`).

import { CodeMirror, Vim, getCM, vim } from '@replit/codemirror-vim';
import type { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import type { VimModeName, VimState } from '../bindings';

/** What a source view gives its Vim keys. */
export interface VimHost {
  /** Takes back the last undo step of the backend's history. */
  undo(): void;
  /** Applies the last undone step again. */
  redo(): void;
  /** M-10.9: store the note at once, or close its tab. */
  command(command: 'store' | 'close'): void;
}

/** The system clipboard as the backend reads and writes it (ADR-014). */
export interface VimClipboard {
  read(): Promise<string | null>;
  write(text: string): void;
}

const hosts = new WeakMap<EditorView, VimHost>();
let clipboard: VimClipboard | null = null;
let installed = false;

type CM = NonNullable<ReturnType<typeof getCM>>;

function hostOf(cm: CM): VimHost | undefined {
  return hosts.get((cm as unknown as { cm6: EditorView }).cm6);
}

/**
 * The register of the system clipboard: what is yanked into it goes to the
 * clipboard; a put reads the clipboard anew (the `paste` action below).
 */
class ClipboardRegister {
  keyBuffer: string[] = [''];
  insertModeChanges: unknown[] = [];
  searchQueries: string[] = [];
  linewise = false;
  blockwise = false;

  setText(text?: string, linewise?: boolean, blockwise?: boolean): void {
    this.keyBuffer = [text ?? ''];
    this.linewise = !!linewise;
    this.blockwise = !!blockwise;
    clipboard?.write(this.toString());
  }

  pushText(text: string, linewise?: boolean): void {
    if (linewise) {
      if (!this.linewise) {
        this.keyBuffer.push('\n');
      }
      this.linewise = true;
    }
    this.keyBuffer.push(text);
    clipboard?.write(this.toString());
  }

  pushInsertModeChanges(): void {}

  pushSearchQuery(): void {}

  clear(): void {
    this.keyBuffer = [];
    this.linewise = false;
  }

  /** Takes the clipboard's `text` without writing it back. */
  took(text: string): void {
    this.keyBuffer = [text];
    this.linewise = text.endsWith('\n');
  }

  toString(): string {
    return this.keyBuffer.join('');
  }
}

/** The registers of the system clipboard. */
const CLIPBOARD_REGISTERS = new Set(['+', '*']);

type Actions = {
  continuePaste(
    cm: CM,
    actionArgs: { registerName?: string },
    vim: unknown,
    text: string,
    register: unknown,
  ): void;
};

/** Ties `codemirror-vim` to the backend, once for the window. */
function install(): void {
  if (installed) {
    return;
  }
  installed = true;
  // M-10.1: one history, the backend's. `codemirror-vim` repeats these for
  // a count (`3u`).
  CodeMirror.commands.undo = (cm: CM) => hostOf(cm)?.undo();
  CodeMirror.commands.redo = (cm: CM) => hostOf(cm)?.redo();

  // M-10.9: the commands of the egui Vim keys. `:q!` is `:q` with a bang.
  const ex = (command: 'store' | 'close', also?: 'close') => (cm: CM) => {
    hostOf(cm)?.command(command);
    if (also) {
      hostOf(cm)?.command(also);
    }
  };
  Vim.defineEx('write', 'w', ex('store'));
  Vim.defineEx('quit', 'q', ex('close'));
  Vim.defineEx('wq', 'wq', ex('store', 'close'));
  Vim.defineEx('xit', 'x', ex('store', 'close'));

  // ADR-014: `"+` and `"*` are the system clipboard, read and written by
  // the backend. `"*` is a register of its own; a yank into `"+` is taken
  // as one into `"*`, so the webview's clipboard is never asked.
  const register = new ClipboardRegister();
  Vim.defineRegister('*', register as never);
  const controller = Vim.getRegisterController() as unknown as {
    pushText: (name: string | null | undefined, ...rest: unknown[]) => void;
    getRegister: (name?: string) => { toString(): string };
  };
  const pushText = controller.pushText.bind(controller);
  controller.pushText = (name, ...rest) => pushText(name === '+' ? '*' : name, ...rest);
  Vim.defineAction('paste', function (
    this: Actions,
    cm: CM,
    actionArgs: { registerName?: string },
    vimState: unknown,
  ) {
    const name = actionArgs.registerName;
    if (name && CLIPBOARD_REGISTERS.has(name)) {
      const read = clipboard?.read() ?? Promise.resolve(null);
      read.then(
        (text) => {
          register.took(text ?? '');
          this.continuePaste(cm, actionArgs, vimState, text ?? '', register);
        },
        () => undefined,
      );
      return;
    }
    const chosen = controller.getRegister(name);
    this.continuePaste(cm, actionArgs, vimState, chosen.toString(), chosen);
  } as never);
}

/**
 * The Vim keys for `view`, working for `host`; `clipboardOf` is the
 * backend's clipboard (the same for every view).
 */
export function vimKeys(view: EditorView, host: VimHost, systemClipboard: VimClipboard): void {
  install();
  clipboard = systemClipboard;
  hosts.set(view, host);
}

/** The extension of the Vim keys; first among the view's extensions. */
export function vimExtension(): Extension {
  return vim();
}

/** Forgets `view`. */
export function dropVimKeys(view: EditorView): void {
  hosts.delete(view);
}

/** The state of the Vim keys of `view` for the status line; `null` without them. */
export function vimState(view: EditorView): VimState | null {
  const cm = getCM(view) as
    | (CM & {
        state: {
          vim?: {
            mode?: string;
            status?: string;
            inputState?: { registerName?: string | null };
          };
          dialog?: HTMLElement | null;
          vimPlugin?: { query: unknown };
        };
      })
    | null;
  const state = cm?.state.vim;
  if (!cm || !state) {
    return null;
  }
  // The command line of `:` and `/` has a field; a notice ("3 lines
  // yanked") has none and leaves the mode as it is.
  const prompt = !!cm.state.dialog?.querySelector?.('input');
  // `"+` is kept as the register of the next command once typed.
  const register = state.inputState?.registerName;
  const status = state.status ?? '';
  const pending = register && !status.includes(`"${register}`) ? `"${register}${status}` : status;
  return {
    mode: modeOf(state.mode, prompt),
    pending: prompt ? '' : pending,
    marked: !!cm.state.vimPlugin?.query,
  };
}

/** Whether Vim is in insert mode (its editing keys are the backend's). */
export function vimInserting(view: EditorView): boolean {
  const cm = getCM(view) as (CM & { state: { vim?: { insertMode?: boolean } } }) | null;
  return !!cm?.state.vim?.insertMode;
}

function modeOf(mode: string | undefined, dialog: boolean): VimModeName {
  if (dialog) {
    return 'command';
  }
  switch (mode) {
    case 'insert':
    case 'replace':
      return 'insert';
    case 'visual':
    case 'visual block':
      return 'visual';
    case 'visual line':
      return 'visualLine';
    default:
      return 'normal';
  }
}

/** Calls `listener` whenever the Vim keys of `view` changed their state. */
export function onVimChange(view: EditorView, listener: () => void): () => void {
  const cm = getCM(view) as
    | (CM & {
        on(event: string, f: () => void): void;
        off(event: string, f: () => void): void;
      })
    | null;
  if (!cm) {
    return () => undefined;
  }
  const events = ['vim-mode-change', 'vim-keypress', 'vim-command-done', 'dialog'];
  for (const event of events) {
    cm.on(event, listener);
  }
  return () => events.forEach((event) => cm.off(event, listener));
}

/**
 * UC-008 BR-008: the hits of the Vim search while they are marked, as
 * `[from, to]` in the text of `view`; at most `limit`.
 */
export function vimHits(view: EditorView, limit = 20_000): [number, number][] {
  const cm = getCM(view) as
    | (CM & {
        state: {
          vimPlugin?: {
            query: {
              highlight(
                state: unknown,
                from: number,
                to: number,
                add: (from: number, to: number) => void,
              ): void;
            } | null;
          };
        };
      })
    | null;
  const query = cm?.state.vimPlugin?.query;
  if (!query) {
    return [];
  }
  const hits: [number, number][] = [];
  try {
    query.highlight(view.state, 0, view.state.doc.length, (from: number, to: number) => {
      if (hits.length < limit) {
        hits.push([from, to]);
      }
    });
  } catch {
    return hits;
  }
  return hits;
}

/**
 * M-10.1, A-01: calls `listener` when a Vim command ended (`false`) or an
 * insert session began (`true`), the bounds of the undo steps.
 */
export function onVimBoundary(view: EditorView, listener: (insert: boolean) => void): () => void {
  const cm = getCM(view) as
    | (CM & {
        on(event: string, f: (e?: { mode?: string }) => void): void;
        off(event: string, f: (e?: { mode?: string }) => void): void;
      })
    | null;
  if (!cm) {
    return () => undefined;
  }
  const done = () => listener(false);
  const mode = (e?: { mode?: string }) => {
    if (e?.mode === 'insert') {
      listener(true);
    } else {
      listener(false);
    }
  };
  cm.on('vim-command-done', done);
  cm.on('vim-mode-change', mode);
  return () => {
    cm.off('vim-command-done', done);
    cm.off('vim-mode-change', mode);
  };
}
