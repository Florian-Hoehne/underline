// ADR-025, docs/migration/variante-b.md step B5b: the facade's backend when
// the text lives in the window: every input is answered by the editor's
// core in the WASM (`editor/core.ts`, `editor/sync.ts`), in the same task as
// the key; its edits go to the backend after the frame. The blocks' code
// colours and figures come from the backend (`ed-enrich`), until then the
// core shows what the backend shows while it works; the system clipboard
// stays a command of the backend.

import { commands, type BlockView, type UiError } from '../bindings';
import { coreOf, openCore, type TabCore } from '../editor/sync';
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

/** The notes whose facade is shown. */
const shown = new Set<string>();

/**
 * The blocks the facade drew last, of their version: one note's only, so
 * a long note's blocks are not held for every tab.
 */
let kept: { tab: TabCore; version: number; blocks: BlockView[] } | null = null;

/** Runs an input on the core of `note`; its edits and views follow. */
function input<T>(note: string, work: (tab: TabCore) => T): Promise<T> {
  try {
    const tab = tabOf(note);
    tab.core.setNow(performance.now());
    // An input left over from a facade gone does not show it again.
    if (shown.has(note)) {
      tab.core.setView(true);
    }
    const answer = work(tab);
    tab.afterInput();
    return Promise.resolve(answer);
  } catch (error) {
    return Promise.reject(error);
  }
}

/** The facade's backend over the window's editor core. */
export function wasmFacadeBackend(mac: boolean): FacadeBackend {
  const readClipboard = () => unwrap(commands.clipReadText());
  const writeClipboard = async (text: string) => {
    await unwrap(commands.clipWriteText(text));
  };
  return {
    open: async (note, mode) => {
      const tab = await openCore(note, mac);
      // The backend makes the colours and figures while the facade shows;
      // only after `ed_mirror`, so that it never sends the blocks too.
      await setEditingView(note, 'facade');
      tab.core.setView(true);
      if (mode) {
        // M-11: also when the source view was never shown.
        tab.core.setMode(mode);
      }
      shown.add(note);
      // The blocks the facade drew last are taken again when nothing changed
      // since: the whole facade of a long note costs as much as opening it.
      const drawn = kept?.tab === tab ? kept : null;
      const head = drawn ? tab.core.facadeHead() : null;
      const state =
        head && drawn && head.version === drawn.version
          ? { ...head, blocks: drawn.blocks }
          : tab.core.facadeState();
      kept = { tab, version: state.version, blocks: state.blocks };
      // What the backend changed while the facade was not shown comes now.
      tab.ring();
      tab.scheduleViews();
      return state;
    },
    select: (note, rev, anchor, head) => input(note, (tab) => tab.core.select(rev, anchor, head)),
    fold: (note, line) => input(note, (tab) => tab.core.fold(line)),
    input: (note, at, facadeInput, rows, clipboard) =>
      input(note, (tab) => tab.core.facadeInput(at, facadeInput, rows, clipboard)),
    clipboard: async (note, at, action, plain) => {
      const text = action === 'paste' ? await readClipboard() : null;
      const clipped = await input(note, (tab) => tab.core.facadeClipboard(at, action, plain, text));
      if (clipped.copied !== null) {
        await writeClipboard(clipped.copied);
      }
      return clipped.patch;
    },
    keys: async (note) => tabOf(note).core.facadeKeys(),
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
    setMode: (note, mode) => coreOf(note)?.core.setMode(mode),
    leave: (note, drawn) => {
      shown.delete(note);
      const tab = coreOf(note);
      // A facade gone before it drew leaves the blocks kept as they are.
      if (drawn && tab?.alive) {
        kept = { tab, ...drawn };
      }
    },
    refresh: async (note) => {
      const tab = tabOf(note);
      tab.core.setView(true);
      return tab.core.facadeRefresh();
    },
  };
}
