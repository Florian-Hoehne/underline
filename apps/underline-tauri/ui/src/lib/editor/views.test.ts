// FR-097, UC-005 BR-016, OQ-221 (package 7a): one note shown by two views
// (its source view in one pane, its facade in the other) has one editor
// core: one text, one history, one revision line with the backend; each
// view calls the core as itself (`coreFor`) and keeps its own cursor, and
// the other view follows its edits after the frame (`CoreListener.sibling`).
// With the real WASM core and the real mirror, as `sync.test.ts`.

import { afterEach, describe, expect, it } from 'vitest';
import type { EditorPatch, MirrorView, SyncAnswer } from '../bindings';
import { MirrorCore } from './core';
import {
  dropAllCores,
  flushCores,
  openCore,
  setSyncIpc,
  setSyncReporter,
  tauriSyncIpc,
  viewIdOf,
  keepViewsOf,
  type CoreListener,
  type SyncIpc,
} from './sync';

function backend(text: string) {
  const mirror = new MirrorCore(text);
  const ipc: SyncIpc = {
    mirror: async (): Promise<MirrorView> => ({
      note: 'Home',
      rev: mirror.rev,
      text: mirror.text(),
      format: 'Markdown',
      anchor: 0,
      head: 0,
    }),
    sync: async (_note, batch): Promise<SyncAnswer> => mirror.receive(batch).answer,
    resync: async (_note, sent) => mirror.resync(sent),
    requestClose: async () => {},
  };
  setSyncIpc(ipc);
  setSyncReporter({ status: () => {}, closeQuestion: () => {}, dead: () => {} });
  return mirror;
}

function follower(): CoreListener & { siblings: number; patches: EditorPatch[] } {
  return {
    siblings: 0,
    patches: [],
    patch(patch) {
      this.patches.push(patch);
    },
    views() {},
    synced() {},
    reopened() {},
    sibling() {
      this.siblings += 1;
    },
  };
}

const typeText = (text: string) => ({ kind: 'text' as const, text });

afterEach(() => {
  dropAllCores();
  setSyncIpc(tauriSyncIpc);
  setSyncReporter(null);
});

describe('TC-005-41, UC-005 BR-016: one note in two panes', () => {
  it('step 1: an edit of one view is followed by the other; each keeps its own cursor', async () => {
    const mirror = backend('alpha\n');
    const tab = await openCore('Home', false);
    const left = viewIdOf('left', 'Home');
    const right = viewIdOf('right', 'Home');
    const leftFollows = follower();
    const rightFollows = follower();
    tab.attach(left, leftFollows);
    tab.attach(right, rightFollows);
    tab.openView(left);
    tab.openView(right);

    // The right view types at the end of the line.
    const rev = tab.coreFor(right, true).state().rev;
    tab.coreFor(right, true).select(rev, 5, 5);
    tab.coreFor(right, true).input(typeText('!'));
    tab.afterInput(right);
    await new Promise((done) => setTimeout(done, 40));
    expect(leftFollows.siblings).toBe(1);
    expect(rightFollows.siblings).toBe(0);

    // The left view, still at the start, gets the change as a patch with its own cursor.
    const patch = tab.coreFor(left, false).sourceSince(rev);
    expect(patch?.changes).toEqual([{ from: 5, to: 5, insert: '!' }]);
    expect([patch?.anchor, patch?.head]).toEqual([0, 0]);

    // The left view types at its own cursor; the right one's moved with it.
    tab.coreFor(left, true).input(typeText('>'));
    tab.afterInput(left);
    expect(tab.coreFor(right, false).state()).toMatchObject({ head: 7, anchor: 7 });
    expect(tab.coreFor(left, false).state()).toMatchObject({ head: 1, anchor: 1 });

    // One text, one revision line: the backend gets both edits in one sync.
    expect(await flushCores('Home')).toBeNull();
    expect(mirror.text()).toBe('>alpha!\n');
  });

  it('step 2, OQ-221: undo in either view takes back the last change of the note', async () => {
    backend('');
    const tab = await openCore('Home', false);
    const facade = viewIdOf('left', 'Home');
    const source = viewIdOf('right', 'Home');
    tab.openView(facade);
    tab.openView(source);
    tab.coreFor(facade, true).input(typeText('left'));
    const rev = tab.coreFor(source, false).state().rev;
    tab.coreFor(source, true).select(rev, 4, 4);
    tab.coreFor(source, true).input(typeText('right'));
    expect(tab.core.text()).toBe('leftright');
    tab.coreFor(facade, true).input({
      kind: 'key',
      key: 'Z',
      ctrl: true,
      alt: false,
      shift: false,
      meta: false,
      page: 1,
    });
    expect(tab.core.text()).toBe('left');
  });

  it('the window reads the core as the view the user works in; a closed tab lets its view go', async () => {
    backend('one two three\n');
    const tab = await openCore('Home', false);
    const left = viewIdOf('left', 'Home');
    const right = viewIdOf('right', 'Home');
    tab.openView(left);
    tab.openView(right);
    const rev = tab.coreFor(right, true).state().rev;
    tab.coreFor(right, true).select(rev, 4, 7);
    // The find field takes the selection of the view the user works in.
    expect(tab.core.selectedText()).toBe('two');
    tab.focusView(left);
    expect(tab.core.selectedText()).toBe('');

    // The right tab closed: its view goes; the left one keeps its state.
    keepViewsOf([['left', 'Home']]);
    expect(tab.viewCount).toBe(1);
    expect(tab.core.selectedText()).toBe('');
  });
});

describe('NFR-002 with two views of a 10,000-line note (FR-097)', () => {
  it('a key in the source view and the facade following it stay within the frame', async () => {
    const lines = Array.from({ length: 10_000 }, (_, i) =>
      i % 50 === 0 ? `## Section ${i}` : `Line ${i} with some words and *emphasis* to parse.`,
    );
    backend(`${lines.join('\n')}\n`);
    const tab = await openCore('Home', false);
    const facade = viewIdOf('left', 'Home');
    const source = viewIdOf('right', 'Home');
    tab.openView(facade);
    tab.openView(source);
    // The facade shows its blocks.
    tab.coreFor(facade, false).setView(true);
    tab.coreFor(facade, false).facadeState();
    tab.coreFor(source, false).setView(false);
    const rev = tab.coreFor(source, true).state().rev;
    const middle = lines.slice(0, 5000).join('\n').length;
    tab.coreFor(source, true).select(rev, middle, middle);

    const keys: number[] = [];
    const follows: number[] = [];
    for (let i = 0; i < 200; i++) {
      let start = performance.now();
      const core = tab.coreFor(source, true);
      core.setNow(start);
      core.setView(false);
      const patch = core.input(typeText('x'));
      keys.push(performance.now() - start);
      // The facade follows (after the frame in the window).
      start = performance.now();
      const other = tab.coreFor(facade, false);
      other.setView(true);
      const blocks = other.facadeRefresh();
      follows.push(performance.now() - start);
      expect(blocks.rev).toBe(patch.rev);
    }
    const p = (list: number[], q: number) =>
      [...list].sort((a, b) => a - b)[Math.floor(q * (list.length - 1))];
    const report = {
      keyP50: p(keys, 0.5),
      keyP95: p(keys, 0.95),
      followP50: p(follows, 0.5),
      followP95: p(follows, 0.95),
      bothP95: p(
        keys.map((k, i) => k + follows[i]),
        0.95,
      ),
    };
    console.info('NFR-002 two views, ms:', JSON.stringify(report));
    // Within one frame of 16 ms together, with room for the DOM.
    expect(report.bothP95).toBeLessThan(16);
  });
});
