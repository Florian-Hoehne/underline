// ADR-025, docs/migration/variante-b.md step B5a: the source view's
// backend when the text lives in the window: every input is answered by the
// editor's core in the WASM (`core.ts`, `sync.ts`), in the same task as the
// key, and its edits go to the backend after the frame. Only the colours
// (`ed_viewport`), the system clipboard (`clipboard.ts`) and storing files
// and images stay commands of the backend.

import { commands, type UiError } from '../bindings';
import { copyOnSelect, readForPaste, storePastedImage, writeCopy } from './clipboard';
import type { EditorBackend } from './sourceView';
import { coreOf, openCore, type TabCore } from './sync';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/** The core of `note`, which `open` made. */
function tabOf(note: string): TabCore {
  const tab = coreOf(note);
  if (!tab) {
    throw new Error(`The editor of ${note} is not open`);
  }
  return tab;
}

/**
 * Runs an input on the core of `note`; its edits and views follow, and a
 * selection it left goes to the clipboard (M-10.7).
 */
function input<T>(note: string, work: (tab: TabCore) => T): Promise<T> {
  try {
    const tab = tabOf(note);
    tab.core.setNow(performance.now());
    tab.core.setView(false);
    const answer = work(tab);
    tab.afterInput();
    copyOnSelect(tab).catch((error: unknown) => console.error('Copy on select failed', error));
    return Promise.resolve(answer);
  } catch (error) {
    return Promise.reject(error);
  }
}

/** The source view's backend over the window's editor core. */
export function wasmBackend(mac: boolean): EditorBackend {
  const readClipboard = () => unwrap(commands.clipReadText());
  const writeClipboard = async (text: string) => {
    await unwrap(commands.clipWriteText(text));
  };
  return {
    open: async (note) => {
      const tab = await openCore(note, mac);
      tab.core.setView(false);
      const state = tab.core.state();
      // What the backend changed while the view was not shown comes now.
      tab.ring();
      tab.scheduleViews();
      return state;
    },
    input: (note, editorInput) => input(note, (tab) => tab.core.input(editorInput)),
    clipboard: async (note, action, imageOnly = false) => {
      if (action !== 'paste') {
        const clipped = await input(note, (tab) => tab.core.clipboard(action, null));
        if (clipped.copied !== null) {
          await writeCopy(note, clipped.copied, clipped.html);
        }
        return { patch: clipped.patch, image: null };
      }
      // M-08.6, UC-005 A17: plain text; over the copied selection what the
      // clipboard held before (BR-011).
      const tab = tabOf(note);
      const over = tab.core.overCopied();
      const read = await readForPaste(tab, false, over);
      const pictureAlone = read.image && read.text === null;
      if (imageOnly && !pictureAlone) {
        // Nothing for Vim outside insert mode: the view and its selection
        // (a visual block, say) stay as they are.
        return { patch: null, image: null };
      }
      const content = imageOnly ? { text: null, markup: null, image: true } : read;
      const clipped = await input(note, (core) => core.core.clipboard('paste', content));
      return { patch: clipped.patch, image: clipped.image ? { over } : null };
    },
    pasteImage: async (note, over) => {
      const picture = await storePastedImage(tabOf(note), over);
      return picture ? input(note, (tab) => tab.core.insert(picture)) : null;
    },
    insert: (note, insertion) => input(note, (tab) => tab.core.insert(insertion)),
    copySelection: (note) => copyOnSelect(coreOf(note)),
    textHash: (note) => coreOf(note)?.core.hash() ?? null,
    select: (note, rev, anchor, head) => input(note, (tab) => tab.core.select(rev, anchor, head)),
    viewport: async (note, from, to) => {
      await unwrap(commands.edViewport(note, from, to));
    },
    apply: (note, base, changes, anchor, head, newStep) =>
      input(note, (tab) => tab.core.apply(base, changes, anchor, head, newStep)),
    fold: (note, line) => input(note, (tab) => tab.core.fold(line)),
    keysStatus: async (note, vim) => tabOf(note).core.keysStatus(vim),
    readClipboard,
    writeClipboard,
    attach: (note, listener) => {
      const tab = tabOf(note);
      tab.listener = listener;
      return () => {
        if (tab.listener === listener) {
          tab.listener = null;
        }
      };
    },
    inStep: (note, rev) => {
      const tab = coreOf(note);
      return !!tab && !tab.core.dead && tab.core.inStep(rev);
    },
    setMode: (note, mode) => coreOf(note)?.core.setMode(mode),
  };
}
