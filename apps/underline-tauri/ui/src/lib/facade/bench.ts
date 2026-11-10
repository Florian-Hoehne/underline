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
