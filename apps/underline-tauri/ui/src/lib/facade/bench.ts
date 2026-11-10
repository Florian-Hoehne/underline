// The measuring point of feature F5a (plan section 15): opening the facade
// of a long note, switching between the facade and the source view, and
// scrolling through the virtualized facade, with the real backend. Only in
// a build made with `VITE_UL_BENCH=1 VITE_UL_BENCH_F5=1`
// (`editor/bench.ts` runs it and writes the results into the note).
//
// - fa_open: the command alone (parse, conversion, IPC, JSON), the first
//   time after the facade was let go, then once more with the blocks kept.
// - open: the facade mounted until its first blocks are painted.
// - switch: as the switch "Facade"/"Source" does it: the view is set in
//   the backend (`tab_set_view`, which lets the facade's blocks go), the old
//   view is taken down and the new one shown, until it is painted.
// - scroll: one screen down at a time through the whole note, from setting
//   the scroll position until it is painted with its blocks.

import { mount, unmount } from 'svelte';
import { commands } from '../bindings';
import { SourceView, tauriBackend } from '../editor/sourceView';
import { tauriFacadeBackend } from './backend';
import type { BlockView } from '../bindings';
import { activeFacade } from './active';
import { applyBlocks, foldingOf } from './model';
import FacadeView from './FacadeView.svelte';

type Stats = { n: number; median: number; p95: number; max: number };

function stats(values: number[]): Stats {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  const q = (p: number) =>
    v.length ? v[Math.min(v.length - 1, Math.ceil(p * v.length) - 1)] : NaN;
  return { n: v.length, median: q(0.5), p95: q(0.95), max: v[v.length - 1] ?? NaN };
}

/** Resolves after the second animation frame and a task: the change is painted. */
function painted(): Promise<number> {
  return new Promise((resolve) => {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => setTimeout(() => resolve(performance.now()), 0)),
    );
  });
}

async function until(check: () => boolean): Promise<void> {
  for (let i = 0; i < 2000 && !check(); i++) {
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
}

/** What F5a measured. */
export type F5Results = {
  blocks: number;
  payloadKb: number;
  faOpenColdMs: number;
  faOpenWarmMs: number;
  openMs: Stats;
  toSourceMs: Stats;
  toFacadeMs: Stats;
  scrollMs: Stats;
  drawnBlocksMax: number;
};

async function showFacade(host: HTMLElement, note: string) {
  const component = mount(FacadeView, {
    target: host,
    props: {
      note,
      label: note,
      platform: 'linux',
      onError: (message: string) => console.error(message),
      backend: tauriFacadeBackend,
    },
  });
  await until(() => host.querySelector('[data-b]') !== null);
  return component;
}

export async function measureF5(host: HTMLElement, note: string): Promise<F5Results> {
  await commands.tabSetView(note, 'source');
  await commands.tabSetView(note, 'facade');
  let start = performance.now();
  const cold = await commands.faOpen(note);
  const faOpenColdMs = performance.now() - start;
  start = performance.now();
  await commands.faOpen(note);
  const faOpenWarmMs = performance.now() - start;
  const state = cold.status === 'ok' ? cold.data : null;
  const payloadKb = state ? JSON.stringify(state).length / 1024 : NaN;

  const open: number[] = [];
  const toSource: number[] = [];
  const toFacade: number[] = [];
  for (let round = 0; round < 5; round++) {
    await commands.tabSetView(note, 'source');
    await commands.tabSetView(note, 'facade');
    start = performance.now();
    let facade = await showFacade(host, note);
    open.push((await painted()) - start);

    // Facade → source.
    start = performance.now();
    await commands.tabSetView(note, 'source');
    await unmount(facade);
    const source = await SourceView.open(host, note, tauriBackend, {
      mac: false,
      label: note,
      onError: (message) => console.error(message),
    });
    toSource.push((await painted()) - start);

    // Source → facade.
    start = performance.now();
    await source.settled();
    await commands.tabSetView(note, 'facade');
    source.destroy();
    facade = await showFacade(host, note);
    toFacade.push((await painted()) - start);
    await unmount(facade);
  }

  // Scrolling through the whole facade, a screen at a time.
  const facade = await showFacade(host, note);
  const scroller = host.querySelector<HTMLElement>('.scroller')!;
  await painted();
  const scroll: number[] = [];
  let drawnBlocksMax = 0;
  while (scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1) {
    const before = scroller.scrollTop;
    start = performance.now();
    scroller.scrollTop = before + scroller.clientHeight;
    scroll.push((await painted()) - start);
    drawnBlocksMax = Math.max(drawnBlocksMax, host.querySelectorAll('[data-b]').length);
    if (scroller.scrollTop <= before || scroll.length > 5000) {
      break;
    }
  }
  await unmount(facade);
  return {
    blocks: state?.blocks.length ?? 0,
    payloadKb,
    faOpenColdMs,
    faOpenWarmMs,
    openMs: stats(open),
    toSourceMs: stats(toSource),
    toFacadeMs: stats(toFacade),
    scrollMs: stats(scroll),
    drawnBlocksMax,
  };
}

// ---------------------------------------------------------------------------
// Feature F5b (`VITE_UL_BENCH_F5B=1`): the measuring point of ADR-024 for
// editing in the facade, with the spike's method: synthetic `beforeinput`
// events on the facade's text, one after the other, each after the previous
// one was painted plus a jitter of 0 to 17 ms. Typing 500 keys at the ends
// of paragraphs at 2 %, 50 % and 98 % of the note (from the end to the
// start), then 100 Backspaces and 30 times Enter followed by Backspace, in
// the Normal input mode; then typing in Vim (insert mode after `A`) and
// Nano. "proc" runs from the start of the handler until the patch is in the
// DOM with the caret; "in first frame" means the patch was in the DOM
// before the first animation frame after the event, within 17.5 ms.

const F5B_TEXT = 'the quick brown fox *jumps* over the lazy_dog, ';
const F5B_WARM = 30;

type Sample = { proc: number; ipc: number; paint: number; firstFrame: boolean };
type Stats99 = Stats & { p99: number };

function stats99(values: number[]): Stats99 {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  const q = (p: number) =>
    v.length ? v[Math.min(v.length - 1, Math.ceil(p * v.length) - 1)] : NaN;
  return { ...stats(values), p99: q(0.99) };
}

let benchSeed = 12345;
function benchJitter(): number {
  benchSeed = (benchSeed * 1103515245 + 12345) & 0x7fffffff;
  return (benchSeed % 1700) / 100;
}

const benchSleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function nextFrame(): { time: number } {
  const frame = { time: 0 };
  requestAnimationFrame(() => {
    frame.time = performance.now();
  });
  return frame;
}

function afterPaint(): Promise<number> {
  return new Promise((resolve) => {
    const guard = setTimeout(() => resolve(NaN), 1000);
    requestAnimationFrame(() =>
      setTimeout(() => {
        clearTimeout(guard);
        resolve(performance.now());
      }, 0),
    );
  });
}

/** One row of the F5b results. */
export type F5bRow = {
  mode: string;
  action: string;
  position: string;
  block: number;
  proc: Stats99;
  /** The command alone, as the window waits for it (IPC and backend). */
  ipc: Stats99;
  /** The rest of `proc`: queue, applying the patch, the DOM, the caret. */
  window: Stats99;
  nextPaint: Stats99;
  firstFrame: number;
};

type Probe = { value: { start: number; patched: number } | null; ipc: number };

/** The facade under measurement and what it reports. */
type Measured = {
  host: HTMLElement;
  text: HTMLElement;
  probe: Probe;
  component: ReturnType<typeof mount>;
  settled: () => Promise<unknown>;
};

async function mountMeasured(host: HTMLElement, note: string, mode: string): Promise<Measured> {
  const probe: Probe = { value: null, ipc: NaN };
  // The command alone, timed around the call.
  const backend = {
    ...tauriFacadeBackend,
    input: async (...args: Parameters<typeof tauriFacadeBackend.input>) => {
      const start = performance.now();
      const patch = await tauriFacadeBackend.input(...args);
      probe.ipc = performance.now() - start;
      return patch;
    },
  };
  const component = mount(FacadeView, {
    target: host,
    props: {
      note,
      label: note,
      platform: 'linux',
      mode: mode as 'Normal' | 'Vim' | 'Nano',
      onError: (message: string) => console.error(message),
      backend,
      probe: (start: number, patched: number) => {
        probe.value = { start, patched };
      },
    },
  });
  await until(() => host.querySelector('[data-b]') !== null);
  const text = host.querySelector<HTMLElement>('.facade-text')!;
  const controller = activeFacade() as unknown as { settled(): Promise<unknown> } | null;
  return {
    host,
    text,
    probe,
    component,
    settled: () => controller?.settled() ?? Promise.resolve(),
  };
}

/** Puts the cursor at the end of block `index` (markup at `start`), as a click there does. */
async function cursorAtEnd(m: Measured, index: number, start: number): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt++) {
    const element = m.host.querySelector<HTMLElement>(`[data-b="${index}"]`);
    const runs = element?.querySelectorAll<HTMLElement>('[data-r]');
    const last = runs?.[runs.length - 1];
    if (element && last) {
      const walker = document.createTreeWalker(last, NodeFilter.SHOW_TEXT);
      let node: Text | null = null;
      for (let n = walker.nextNode() as Text | null; n; n = walker.nextNode() as Text | null) {
        node = n;
      }
      m.text.focus({ preventScroll: true });
      const selection = document.getSelection()!;
      if (node) {
        selection.collapse(node, node.length);
      } else {
        selection.collapse(last, 0);
      }
      await afterPaint();
      await afterPaint();
      await m.settled();
      return;
    }
    // Not drawn yet: the block to the top.
    activeFacade()?.reveal(start);
    await afterPaint();
    await benchSleep(50);
  }
  throw new Error(`block ${index} was not drawn`);
}

async function drive(m: Measured, inputType: string, data: string | null): Promise<Sample | null> {
  m.probe.value = null;
  m.probe.ipc = NaN;
  const event = new InputEvent('beforeinput', {
    inputType,
    data,
    bubbles: true,
    cancelable: true,
  });
  const frame = nextFrame();
  m.text.dispatchEvent(event);
  await m.settled();
  const paint = await afterPaint();
  const done = m.probe.value as { start: number; patched: number } | null;
  if (!done) {
    return null;
  }
  return {
    proc: done.patched - done.start,
    ipc: m.probe.ipc,
    paint: paint - event.timeStamp,
    firstFrame: frame.time >= done.patched && frame.time - event.timeStamp <= 17.5,
  };
}

async function key(m: Measured, name: string): Promise<void> {
  m.text.dispatchEvent(
    new KeyboardEvent('keydown', { key: name, bubbles: true, cancelable: true }),
  );
  await m.settled();
  await afterPaint();
}

function row(
  mode: string,
  action: string,
  position: string,
  block: number,
  samples: Sample[],
): F5bRow {
  return {
    mode,
    action,
    position,
    block,
    proc: stats99(samples.map((s) => s.proc)),
    ipc: stats99(samples.map((s) => s.ipc)),
    window: stats99(samples.map((s) => s.proc - s.ipc)),
    nextPaint: stats99(samples.map((s) => s.paint)),
    firstFrame: samples.filter((s) => s.firstFrame).length / (samples.length || 1),
  };
}

/** The paragraphs at 2 %, 50 % and 98 % of the note, from the end to the start. */
function positions(state: { blocks: { kind: { type: string } }[] }): [string, number][] {
  const near = (fraction: number) => {
    let index = Math.round(state.blocks.length * fraction);
    while (index < state.blocks.length - 1 && state.blocks[index].kind.type !== 'paragraph') {
      index++;
    }
    return index;
  };
  return [
    ['end', near(0.98)],
    ['middle', near(0.5)],
    ['start', near(0.02)],
  ];
}

/**
 * What applying one change costs the window's JavaScript alone, on all
 * blocks of the note: the blocks after it moved (`applyBlocks`), and the
 * folded sections worked out again (`foldingOf`), 200 times each.
 */
export function measureApply(
  blocks: BlockView[],
  from: number,
): { apply: Stats99; folding: Stats99 } {
  const apply: number[] = [];
  const folding: number[] = [];
  for (let i = 0; i < 200; i++) {
    const change = {
      note: 'x',
      rev: i + 2,
      base: i + 1,
      version: i + 2,
      from,
      removed: 1,
      blocks: [blocks[from]],
      shift: 1,
      lineShift: 0,
    };
    let start = performance.now();
    const next = applyBlocks({ rev: i + 1, version: i + 1, blocks }, change);
    apply.push(performance.now() - start);
    start = performance.now();
    foldingOf(next?.blocks ?? blocks, []);
    folding.push(performance.now() - start);
  }
  return { apply: stats99(apply), folding: stats99(folding) };
}

/** Feature F5b: typing, Enter and Backspace in the facade, in each input mode. */
export async function measureF5b(
  host: HTMLElement,
  note: string,
  keys: number,
): Promise<{ rows: F5bRow[]; apply: Record<string, { apply: Stats99; folding: Stats99 }> }> {
  await commands.tabSetView(note, 'facade');
  const rows: F5bRow[] = [];
  const apply: Record<string, { apply: Stats99; folding: Stats99 }> = {};
  const modes = (import.meta.env.VITE_UL_BENCH_MODES ?? 'Normal,Vim,Nano').split(',');
  for (const mode of modes) {
    await commands.settingsQuick('input_mode', mode);
    const state = await commands.faOpen(note);
    if (state.status !== 'ok') {
      throw new Error('fa_open failed');
    }
    const m = await mountMeasured(host, note, mode);
    await benchSleep(500);
    for (const [position, block] of positions(state.data)) {
      if (mode === modes[0]) {
        apply[position] = measureApply(state.data.blocks, block);
      }
      await cursorAtEnd(m, block, state.data.blocks[block].start);
      if (mode === 'Vim') {
        // Typing is the insert mode's (`A`: at the end of the block).
        await drive(m, 'insertText', 'A');
      }
      const typed: Sample[] = [];
      for (let i = 0; i < F5B_WARM + keys; i++) {
        await benchSleep(benchJitter());
        const sample = await drive(m, 'insertText', F5B_TEXT[i % F5B_TEXT.length]);
        if (i >= F5B_WARM && sample) typed.push(sample);
      }
      rows.push(row(mode, 'type', position, block, typed));
      if (mode === 'Normal') {
        const back: Sample[] = [];
        for (let i = 0; i < 100; i++) {
          await benchSleep(benchJitter());
          const sample = await drive(m, 'deleteContentBackward', null);
          if (sample) back.push(sample);
        }
        rows.push(row(mode, 'backspace', position, block, back));
        const enter: Sample[] = [];
        const enterBack: Sample[] = [];
        for (let i = 0; i < 30; i++) {
          await benchSleep(benchJitter());
          const a = await drive(m, 'insertParagraph', null);
          await benchSleep(benchJitter());
          const b = await drive(m, 'deleteContentBackward', null);
          if (a) enter.push(a);
          if (b) enterBack.push(b);
        }
        rows.push(row(mode, 'enter', position, block, enter));
        rows.push(row(mode, 'enter+backspace', position, block, [...enter, ...enterBack]));
      }
      if (mode === 'Vim') {
        await key(m, 'Escape');
      }
      console.info(`F5b ${mode} ${position} done`);
    }
    await unmount(m.component);
    host.replaceChildren();
  }
  await commands.settingsQuick('input_mode', 'Normal');
  return { rows, apply };
}
