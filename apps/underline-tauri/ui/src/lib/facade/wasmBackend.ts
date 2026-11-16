// ADR-025, docs/migration/variante-b.md step B5b: the facade's backend when
// the text lives in the window: every input is answered by the editor's
// core in the WASM (`editor/core.ts`, `editor/sync.ts`), in the same task as
// the key; its edits go to the backend after the frame. The blocks' code
// colours and figures come from the backend (`ed-enrich`), until then the
// core shows what the backend shows while it works; the system clipboard
// (`editor/clipboard.ts`) and storing files and images stay commands of the
// backend.

import { commands, type BlockView, type PaneName, type UiError } from '../bindings';
import { copyOnSelect, readForPaste, storePastedImage, writeCopy } from '../editor/clipboard';
import type { EditorCore } from '../editor/core';
import { coreInStep, coreOf, openCore, viewIdOf, type TabCore } from '../editor/sync';
import { setEditingView, type FacadeBackend } from './backend';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

function tabOf(note: string): TabCore {
  const tab = coreOf(note);
  if (!tab) {
    throw new Error(`The editor of ${note} is not open`);
  }
  return tab;
}

/**
 * The blocks a facade drew last, of their version: one view's only, so a
 * long note's blocks are not held for every tab.
 */
let kept: { tab: TabCore; view: number; version: number; blocks: BlockView[] } | null = null;

/**
 * The facade's backend over the window's editor core. Each facade makes its
 * own: it calls the core as its view (FR-097), so a note shown in two panes
 * has one text and one history, and each view its own cursor, folds and
 * blocks (UC-005 BR-016).
 */
export function wasmFacadeBackend(mac: boolean, pane: PaneName = 'left'): FacadeBackend {
  /** FR-097: the id of the view of `note` in the tab of this pane. */
  const viewOf = (note: string): number => viewIdOf(pane, note);
  /** The facade is shown (opened and not left). */
  let shown = false;

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
      // An input left over from a facade gone does not show it again.
      if (shown) {
        core.setView(true);
      }
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
    open: async (note, mode) => {
      const tab = await openCore(note, mac);
      // The backend makes the colours and figures while the facade shows;
      // only after `ed_mirror`, so that it never sends the blocks too.
      await setEditingView(note, 'facade', pane);
      const view = viewOf(note);
      const core = tab.coreFor(view, false);
      core.setView(true);
      if (mode) {
        // M-11: also when the source view was never shown.
        core.setMode(mode);
      }
      shown = true;
      // The blocks the facade drew last are taken again when nothing changed
      // since: the whole facade of a long note costs as much as opening it.
      const drawn = kept?.tab === tab && kept.view === view ? kept : null;
      const head = drawn ? core.facadeHead() : null;
      const state =
        head && drawn && head.version === drawn.version
          ? { ...head, blocks: drawn.blocks }
          : core.facadeState();
      kept = { tab, view, version: state.version, blocks: state.blocks };
      // What the backend changed while the facade was not shown comes now.
      tab.ring();
      tab.scheduleViews();
      return state;
    },
    select: (note, rev, anchor, head) => input(note, (core) => core.select(rev, anchor, head)),
    fold: (note, line) => input(note, (core) => core.fold(line)),
    find: (note, spec, step) => input(note, (core) => core.find(spec, step)),
    replace: (note, spec, replacement, all) =>
      input(note, (core) => core.facadeReplace(spec, replacement, all)),
    input: (note, at, facadeInput, rows, clipboard) =>
      input(note, (core) => core.facadeInput(at, facadeInput, rows, clipboard)),
    clipboard: async (note, at, action, plain) => {
      if (action !== 'paste') {
        const clipped = await input(note, (core) => core.facadeClipboard(at, action, plain, null));
        if (clipped.copied !== null) {
          await writeCopy(note, clipped.copied, clipped.html);
        }
        return { patch: clipped.patch, image: null };
      }
      // UC-005 A17: HTML as markup unless Ctrl+Shift+V; over the copied
      // selection what the clipboard held before (BR-011).
      const tab = tabOf(note);
      const over = tab.coreFor(viewOf(note), true).overCopied();
      const content = await readForPaste(tab, !plain, over);
      const clipped = await input(note, (core) =>
        core.facadeClipboard(at, 'paste', plain, content),
      );
      return { patch: clipped.patch, image: clipped.image ? { over } : null };
    },
    pasteImage: async (note, at, over) => {
      const picture = await storePastedImage(tabOf(note), over);
      return picture ? input(note, (core) => core.facadeInsert(at, picture)) : null;
    },
    insert: (note, at, insertion) => input(note, (core) => core.facadeInsert(at, insertion)),
    copySelection: (note) => copyOnSelect(coreOf(note)),
    textHash: (note) => coreOf(note)?.core.hash() ?? null,
    keys: async (note) => tabOf(note).coreFor(viewOf(note), false).facadeKeys(),
    readClipboard,
    writeClipboard,
    attach: (note, listener) => {
      const tab = tabOf(note);
      const view = viewOf(note);
      tab.attach(view, listener);
      return () => tab.detach(view, listener);
    },
    setMode: (note, mode) => {
      const tab = coreOf(note);
      if (tab?.alive) {
        tab.coreFor(viewOf(note), false).setMode(mode);
      }
    },
    leave: (note, drawn) => {
      shown = false;
      const tab = coreOf(note);
      // A facade gone before it drew leaves the blocks kept as they are.
      if (drawn && tab?.alive) {
        kept = { tab, view: viewOf(note), ...drawn };
      }
    },
    // FR-097: the window reads the core as this view, which has the keyboard.
    focused: (note) => coreOf(note)?.focusView(viewOf(note)),
    refresh: async (note) => {
      const core = tabOf(note).coreFor(viewOf(note), false);
      core.setView(true);
      return core.facadeRefresh();
    },
    // FR-080: link completion, answered by the core on every key.
    linkContext: (note, at, manual) => {
      const tab = coreOf(note);
      return tab && tab.alive
        ? tab.coreFor(viewOf(note), false).facadeLinkContext(at, manual)
        : null;
    },
    completeLink: (note, at, choice) => input(note, (core) => core.facadeCompleteLink(at, choice)),
    // UC-004 A16: going to a line, answered by the core.
    goToLine: (note, at, target) => input(note, (core) => core.facadeGoToLine(at, target)),
    // UC-024: a note attribute changed from the note properties.
    attribute: (note, at, change) => input(note, (core) => core.facadeAttribute(at, change)),
    lineCount: (note) => {
      const tab = coreOf(note);
      return tab && tab.alive ? tab.coreFor(viewOf(note), false).lineCount() : null;
    },
    // FR-099: the spell check's marks apply once the core is in step.
    inStep: (note, rev) => coreInStep(note, rev),
    markupOf: (note, from, to) => {
      const tab = coreOf(note);
      if (!tab?.alive) return null;
      // Web units count a line break once (`utf16.rs`).
      const text = tab.core.text().replace(/\r\n?/g, '\n');
      return text.slice(from, to);
    },
  };
}
