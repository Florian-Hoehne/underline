// ADR-025, step B4c: the window's side of the sync with the real WASM core
// and the real mirror (`MirrorCore`): batching after the frame with one
// batch on its way, a failed `ed_sync` sent again (taken in once), retries
// with the status said once, a desync mended by `ed_resync` while typing
// goes on, flushing before commands and before closing, and a core that
// failed for good opened anew.

import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CloseQuestion, MirrorView, SyncAnswer, SyncBatch } from '../bindings';
import { MirrorCore } from './core';
import {
  dropAllCores,
  flushCores,
  flushed,
  openCore,
  setSyncIpc,
  setSearches,
  setSyncReporter,
  takeFlush,
  tauriSyncIpc,
  type CoreListener,
  type SyncIpc,
} from './sync';

/** A backend of one note `Home` over the real mirror, with its faults. */
function backend(text: string) {
  const mirror = new MirrorCore(text);
  const calls = { sync: 0, resync: 0, closes: [] as boolean[] };
  const faults = { failSync: 0, loseAnswers: 0, failResync: 0, hashWrong: 0 };
  const batches: SyncBatch[] = [];
  let pending: Array<() => void> = [];
  let hold = false;
  const ipc: SyncIpc = {
    mirror: async (): Promise<MirrorView> => ({
      note: 'Home',
      rev: mirror.rev,
      text: mirror.text(),
      format: 'Markdown',
      anchor: 0,
      head: 0,
    }),
    sync: async (_note, batch): Promise<SyncAnswer> => {
      calls.sync += 1;
      if (faults.failSync > 0) {
        faults.failSync -= 1;
        throw { kind: 'internal', message: 'the backend is busy' };
      }
      if (hold) {
        await new Promise<void>((done) => pending.push(done));
      }
      batches.push(batch);
      const { answer } = mirror.receive(batch);
      if (faults.loseAnswers > 0) {
        faults.loseAnswers -= 1;
        throw { kind: 'internal', message: 'the answer was lost' };
      }
      if (faults.hashWrong > 0) {
        faults.hashWrong -= 1;
        return { ...answer, hash: '0000000000000000' };
      }
      return answer;
    },
    resync: async (_note, sent) => {
      calls.resync += 1;
      if (faults.failResync > 0) {
        faults.failResync -= 1;
        throw { kind: 'internal', message: 'no resync now' };
      }
      return mirror.resync(sent);
    },
    requestClose: async (sent) => {
      calls.closes.push(sent);
    },
  };
  setSyncIpc(ipc);
  const said: string[] = [];
  const questions: CloseQuestion[] = [];
  setSyncReporter({
    status: (note, said_) => said.push(`${note}: ${said_}`),
    closeQuestion: (question) => questions.push(question),
    dead: (note) => said.push(`${note}: dead`),
  });
  return {
    mirror,
    calls,
    faults,
    batches,
    said,
    questions,
    hold(on: boolean) {
      hold = on;
      if (!on) {
        const waiting = pending;
        pending = [];
        waiting.forEach((done) => done());
      }
    },
  };
}

function listener(): CoreListener & { patches: number; syncs: number } {
  return {
    patches: 0,
    syncs: 0,
    patch() {
      this.patches += 1;
    },
    views() {},
    synced() {
      this.syncs += 1;
    },
    reopened() {},
  };
}

const typeText = (text: string) => ({ kind: 'text' as const, text });

afterEach(() => {
  dropAllCores();
  setSyncIpc(tauriSyncIpc);
  setSyncReporter(null);
  vi.useRealTimers();
});

describe('ADR-025 the window side of the sync', () => {
  it('sends the edits of several keys in one batch after the frame', async () => {
    const back = backend('# Home\n');
    const tab = await openCore('Home', false);
    tab.core.input(typeText('a'));
    tab.afterInput();
    tab.core.input(typeText('b'));
    tab.afterInput();

    await vi.waitFor(() => expect(back.mirror.text()).toBe('ab# Home\n'));

    expect(back.calls.sync).toBe(1);
    expect(back.batches[0].edits).toEqual([{ at: 0, remove: 0, insert: 'ab' }]);
  });

  it('keeps one batch on its way; what was typed meanwhile follows', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.hold(true);
    tab.core.input(typeText('1'));
    const first = tab.send();
    tab.core.input(typeText('2'));
    void tab.send();
    expect(back.calls.sync).toBe(1);

    back.hold(false);
    await first;

    expect(await tab.flush()).toBe(true);
    expect(back.mirror.text()).toBe('12x\n');
  });

  it('sends a batch whose answer was lost again; the backend takes it in once', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.faults.loseAnswers = 1;
    tab.core.input(typeText('a'));

    expect(await tab.flush()).toBe(true);

    expect(back.mirror.text()).toBe('ax\n');
    expect(back.batches[0]).toEqual(back.batches[1]);
  });

  it('keeps trying with growing waits, and says so once', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.faults.failSync = 5;
    tab.core.input(typeText('a'));
    vi.useFakeTimers();

    await tab.send();
    for (let i = 0; i < 10 && back.mirror.text() !== 'ax\n'; i++) {
      await vi.advanceTimersByTimeAsync(2000);
    }

    expect(back.mirror.text()).toBe('ax\n');
    // Five failures, the batch, and the fetch the failures asked for.
    expect(back.calls.sync).toBeGreaterThanOrEqual(6);
    expect(back.calls.sync).toBeLessThanOrEqual(7);
    expect(back.said).toEqual([
      'Home: Underline could not hand your latest edits over to be stored: the backend is busy. Underline keeps trying',
    ]);
  });

  it('a command that reads the text does not run while edits cannot be sent', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.faults.failSync = 1000;
    tab.core.input(typeText('a'));
    const work = vi.fn(async () => ({ status: 'ok' as const, data: null }));

    const result = await flushed(work);

    expect(work).not.toHaveBeenCalled();
    expect(result).toEqual({
      status: 'error',
      error: {
        kind: 'internal',
        message:
          'Underline could not hand your latest edits over to be stored: the backend is busy',
      },
    });
    expect(await flushCores()).toBe('the backend is busy');
  }, 10_000);

  it('runs a command that reads the text after every edit reached the backend', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    tab.core.input(typeText('a'));

    const seen = await flushed(async () => ({ status: 'ok' as const, data: back.mirror.text() }));

    expect(seen).toEqual({ status: 'ok', data: 'ax\n' });
  });

  it('when Underline closes: sends every edit, then answers the close', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    tab.core.input(typeText('a'));

    await takeFlush({ note: null, closing: true });

    expect(back.mirror.text()).toBe('ax\n');
    expect(back.calls.closes).toEqual([true]);
    expect(back.questions).toEqual([]);
  });

  it('when edits cannot be sent before closing, "Changes not stored" asks', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.faults.failSync = 1000;
    tab.core.input(typeText('a'));

    await takeFlush({ note: null, closing: true });

    expect(back.calls.closes).toEqual([false]);
    expect(back.questions).toEqual([
      {
        tab: null,
        what: 'Underline',
        error: 'Underline could not hand your latest edits over to be stored: the backend is busy',
      },
    ]);
  }, 10_000);

  it('takes the backend changes in and tells the view', async () => {
    const back = backend('note\n');
    const tab = await openCore('Home', false);
    const view = listener();
    tab.listener = view;
    back.mirror.backendEdit(0, 0, '> ', true);

    tab.ring();
    await vi.waitFor(() => expect(tab.core.text()).toBe('> note\n'));

    expect(view.patches).toBe(1);
    expect(view.syncs).toBeGreaterThan(0);
    expect(tab.core.inStep(back.mirror.rev)).toBe(true);
  });

  it('sends the whole text when the copies went apart; typing meanwhile is kept', async () => {
    const back = backend('abc');
    const tab = await openCore('Home', false);
    back.faults.hashWrong = 1;
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    tab.core.input(typeText('!'));
    back.hold(true);
    const sent = tab.send();
    back.hold(false);
    // Typed while the answer and the resync travel.
    queueMicrotask(() => tab.core.input(typeText('?')));

    await sent;
    expect(await tab.flush()).toBe(true);

    expect(back.calls.resync).toBe(1);
    expect(back.mirror.text()).toBe(tab.core.text());
    expect(tab.core.text()).toBe('!?abc');
    error.mockRestore();
  });

  it('a resync that fails is tried again', async () => {
    const back = backend('abc');
    const tab = await openCore('Home', false);
    back.faults.hashWrong = 1;
    back.faults.failResync = 1;
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    tab.core.input(typeText('!'));

    expect(await tab.flush()).toBe(true);

    expect(back.calls.resync).toBeGreaterThanOrEqual(1);
    expect(back.mirror.text()).toBe('!abc');
    error.mockRestore();
  });

  it('Q-40: a core that failed for good is opened anew and the loss is said', async () => {
    const back = backend('stored\n');
    const tab = await openCore('Home', false);
    let reopened = 0;
    tab.listener = { ...listener(), reopened: () => (reopened += 1) };
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    // A trap of the WASM, as after a panic.
    tab.core.onDead?.({ note: 'Home' } as never);

    expect(reopened).toBe(1);
    expect(back.said).toEqual(['Home: dead']);
    const again = await openCore('Home', false);
    expect(again).not.toBe(tab);
    expect(again.core.text()).toBe(back.mirror.text());
    error.mockRestore();
  });

  it('a closed tab says nothing more', async () => {
    const back = backend('x\n');
    const tab = await openCore('Home', false);
    back.faults.failSync = 1000;
    tab.core.input(typeText('a'));
    void tab.send();

    dropAllCores();
    await new Promise((done) => setTimeout(done, 50));

    expect(back.said).toEqual([]);
    expect(tab.alive).toBe(false);
  });
});

describe('M-13.2 the hits after a burst of typing (app-tester F9)', () => {
  it('asks the core once more after the pause, so the last edit gets its marks', async () => {
    backend('ferry\n');
    const tab = await openCore('Home', false);
    const found: number[] = [];
    tab.listener = {
      ...listener(),
      views(views) {
        if (views.found) found.push(views.found.count);
      },
    };
    setSearches({ overlay: 'ferry', find: null });
    await vi.waitFor(() => expect(found).toEqual([1]));

    // Typed within the 300 ms the core waits: the views right after the
    // input report nothing new.
    tab.core.setNow(performance.now());
    tab.core.input({ kind: 'text', text: 'ferry ' });
    tab.afterInput();
    await new Promise((done) => setTimeout(done, 60));
    expect(found).toEqual([1]);

    await vi.waitFor(() => expect(found).toEqual([1, 2]), { timeout: 2000 });
    setSearches({ overlay: '', find: null });
  });
});
