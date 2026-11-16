// ADR-025, docs/migration/variante-b.md step B5a: the source view's
// backend when the text lives in the window: every input is answered by the
// editor's core in the WASM (`core.ts`, `sync.ts`), in the same task as the
// key, and its edits go to the backend after the frame. Only the colours
// (`ed_viewport`), the system clipboard (`clipboard.ts`) and storing files
// and images stay commands of the backend.

import { commands, type UiError } from '../bindings';
import { copyOnSelect, readForPaste, storePastedImage, writeCopy } from './clipboard';
import type { EditorBackend } from './sourceView';
import type { EditorCore } from './core';
import { coreOf, openCore, viewIdOf, type TabCore } from './sync';
import type { PaneName } from '../bindings';

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
 * The source view's backend over the window's editor core. Each source view
 * makes its own: it calls the core as its view (FR-097), so a note shown in
 * two panes has one text and one history, and each view its own cursor and
 * folds (UC-005 BR-016).
 */
export function wasmBackend(mac: boolean, pane: PaneName = 'left'): EditorBackend {
  /** FR-097: the id of the view of `note` in the tab of this pane. */
  const viewOf = (note: string): number => viewIdOf(pane, note);

  /** The core of `note` as this view sees it. */
  const coreOf_ = (note: string, input: boolean): EditorCore =>
    tabOf(note).coreFor(viewOf(note), input);

  /**
   * Runs an input on the core of `note`; its edits and views follow, the
   * note's other view follows them, and a selection it left goes to the
   * clipboard (M-10.7).
   */
  function input<T>(note: string, work: (core: EditorCore) => T): Promise<T> {
    try {
      const tab = tabOf(note);
      const view = viewOf(note);
      const core = tab.coreFor(view, true);
      core.setNow(performance.now());
      core.setView(false);
      const answer = work(core);
      tab.afterInput(view);
      copyOnSelect(tab).catch((error: unknown) => console.error('Copy on select failed', error));
      return Promise.resolve(answer);
    } catch (error) {
      return Promise.reject(error);
    }
  }

  const readClipboard = () => unwrap(commands.clipReadText());
  const writeClipboard = async (text: string) => {
    await unwrap(commands.clipWriteText(text));
  };
  return {
    open: async (note) => {
      const tab = await openCore(note, mac);
      const core = tab.coreFor(viewOf(note), false);
      core.setView(false);
      const state = core.state();
      // What the backend changed while the view was not shown comes now.
      tab.ring();
      tab.scheduleViews();
      return state;
    },
    input: (note, editorInput) => input(note, (core) => core.input(editorInput)),
    find: (note, spec, step) => input(note, (core) => core.find(spec, step)),
    replace: (note, spec, replacement, all) =>
      input(note, (core) => core.replace(spec, replacement, all)),
    clipboard: async (note, action, imageOnly = false) => {
      if (action !== 'paste') {
        const clipped = await input(note, (core) => core.clipboard(action, null));
        if (clipped.copied !== null) {
          await writeCopy(note, clipped.copied, clipped.html);
        }
        return { patch: clipped.patch, image: null };
      }
      // M-08.6, UC-005 A17: plain text; over the copied selection what the
      // clipboard held before (BR-011).
      const tab = tabOf(note);
      const over = coreOf_(note, true).overCopied();
      const read = await readForPaste(tab, false, over);
      const pictureAlone = read.image && read.text === null;
      if (imageOnly && !pictureAlone) {
        // Nothing for Vim outside insert mode: the view and its selection
        // (a visual block, say) stay as they are.
        return { patch: null, image: null };
      }
      const content = imageOnly ? { text: null, markup: null, image: true } : read;
      const clipped = await input(note, (core) => core.clipboard('paste', content));
      return { patch: clipped.patch, image: clipped.image ? { over } : null };
    },
    pasteImage: async (note, over) => {
      const picture = await storePastedImage(tabOf(note), over);
      return picture ? input(note, (core) => core.insert(picture)) : null;
    },
    insert: (note, insertion) => input(note, (core) => core.insert(insertion)),
    copySelection: (note) => copyOnSelect(coreOf(note)),
    textHash: (note) => coreOf(note)?.core.hash() ?? null,
    select: (note, rev, anchor, head) => input(note, (core) => core.select(rev, anchor, head)),
    viewport: async (note, from, to) => {
      await unwrap(commands.edViewport(note, from, to));
    },
    apply: (note, base, changes, anchor, head, newStep) =>
      input(note, (core) => core.apply(base, changes, anchor, head, newStep)),
    fold: (note, line) => input(note, (core) => core.fold(line)),
    keysStatus: async (note, vim) => coreOf_(note, false).keysStatus(vim),
    readClipboard,
    writeClipboard,
    attach: (note, listener) => {
      const tab = tabOf(note);
      const view = viewOf(note);
      tab.attach(view, listener);
      return () => tab.detach(view, listener);
    },
    // FR-097: the window reads the core as this view, which has the keyboard.
    focused: (note) => coreOf(note)?.focusView(viewOf(note)),
    // FR-097: the other view of the note changed the text.
    since: async (note, base) => coreOf_(note, false).sourceSince(base),
    inStep: (note, rev) => {
      const tab = coreOf(note);
      return !!tab && !tab.core.dead && tab.core.inStep(rev);
    },
    setMode: (note, mode) => {
      const tab = coreOf(note);
      if (tab?.alive) {
        tab.coreFor(viewOf(note), false).setMode(mode);
      }
    },
    // FR-080: link completion, answered by the core on every key.
    linkContext: (note, manual) => {
      const tab = coreOf(note);
      return tab && tab.alive ? tab.coreFor(viewOf(note), false).linkContext(manual) : null;
    },
    completeLink: (note, choice) => input(note, (core) => core.completeLink(choice)),
    // UC-004 A16: going to a line, answered by the core.
    goToLine: (note, target) => input(note, (core) => core.goToLine(target)),
    // UC-024: a note attribute changed from the note properties.
    attribute: (note, change) => input(note, (core) => core.attribute(change)),
  };
}
