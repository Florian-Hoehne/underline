// The measuring point of ADR-024 for the source view: the in-page driver of
// spike F0 (`spikes/f0-editor/ui/src/lib/bench.ts`), run against this
// source view and the real backend. Only in a build made with
// `VITE_UL_BENCH=1`; a normal build leaves this module out.
//
// It opens its own source view of the active note over the window (the
// window's editor stays unmounted in such a build), then types 30 keys to
// warm up and `VITE_UL_BENCH_KEYS` keys (500) as synthetic `beforeinput`
// events at the ends of the lines at 2 %, 50 % and 98 % of the note, each
// after the previous one was painted plus a jitter of 0 to 17 ms. It does so
// without colours (no viewport reported, so the backend computes none) and
// with them (the backend sends `ed-highlight` after every key). At the end
// it replaces the note's text by the results as JSON and stores it, so the
// script that started the app can read them from the note's file.

import { EditorView } from '@codemirror/view';
import { commands, events } from '../bindings';
import type { Shell } from '../shell.svelte';
import { SourceView, tauriBackend } from './sourceView';

const TEXT = 'the quick brown fox *jumps* over the lazy_dog, ';
const WARM = 30;

type Sample = { proc: number; paint: number; firstFrame: boolean };

type Stats = { n: number; median: number; p95: number; p99: number; max: number };

type Row = {
  colors: boolean;
  position: string;
  line: number;
  proc: Stats;
  nextPaint: Stats;
  firstFrame: number;
};

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

let seed = 12345;
/** Deterministic jitter, so keys arrive at any phase of the frame. */
function jitter(): number {
  seed = (seed * 1103515245 + 12345) & 0x7fffffff;
  return (seed % 1700) / 100;
}

function stats(values: number[]): Stats {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  const q = (p: number) =>
    v.length ? v[Math.min(v.length - 1, Math.ceil(p * v.length) - 1)] : NaN;
  return { n: v.length, median: q(0.5), p95: q(0.95), p99: q(0.99), max: v[v.length - 1] ?? NaN };
}

/** The time of the first animation frame after now (0 until it comes). */
function nextFrame(): { time: number } {
  const frame = { time: 0 };
  requestAnimationFrame(() => {
    frame.time = performance.now();
  });
  return frame;
}

/** Resolves just after the next paint (rAF, then a task). */
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

async function typeAt(view: SourceView, line: number, keys: number): Promise<Sample[]> {
  const doc = view.view.state.doc;
  const at = doc.line(line).to;
  view.view.dispatch({ selection: { anchor: at }, userEvent: 'select.pointer' });
  view.view.dispatch({ effects: EditorView.scrollIntoView(at, { y: 'center' }) });
  await view.settled();
  await afterPaint();
  await afterPaint();
  const samples: Sample[] = [];
  for (let i = 0; i < WARM + keys; i++) {
    if (i % 100 === 0) {
      console.info(`measuring at line ${line}: key ${i}`);
    }
    await sleep(jitter());
    let measured: { start: number; patched: number } | null = null;
    view.probe = (start, patched) => {
      measured = { start, patched };
    };
    const event = new InputEvent('beforeinput', {
      inputType: 'insertText',
      data: TEXT[i % TEXT.length],
      bubbles: true,
      cancelable: true,
    });
    const frame = nextFrame();
    view.view.contentDOM.dispatchEvent(event);
    await view.settled();
    const paint = await afterPaint();
    const done = measured as { start: number; patched: number } | null;
    if (i >= WARM && done) {
      samples.push({
        proc: done.patched - done.start,
        paint: paint - event.timeStamp,
        firstFrame: frame.time >= done.patched && frame.time - event.timeStamp <= 17.5,
      });
    }
  }
  view.probe = null;
  return samples;
}

/** Runs the measurement on the active note and stores the results in it. */
export async function runBench(shell: Shell): Promise<void> {
  try {
    await measure(shell);
  } catch (error) {
    // Nothing else of the webview reaches the script: the failure goes into
    // the note, where the script looks for the results.
    const note = shell.tabs.active;
    if (note !== null) {
      const text = `UL-BENCH-ERROR ${String(error)} ${(error as Error)?.stack ?? ''}\n`;
      await tauriBackend.input(note, { kind: 'text', text });
      await commands.tabStore(note);
    }
    throw error;
  }
}

async function measure(shell: Shell): Promise<void> {
  const keys = Number(import.meta.env.VITE_UL_BENCH_KEYS ?? 500);
  // The window's editor is not mounted in a measuring build; wait for the tab.
  for (let i = 0; i < 200 && shell.tabs.active === null; i++) {
    await sleep(100);
  }
  const note = shell.tabs.active;
  if (note === null) {
    console.error('The measurement found no open note');
    return;
  }
  const host = document.createElement('div');
  host.style.cssText =
    'position: fixed; inset: 0; z-index: 100; background: var(--palette-background)';
  document.body.append(host);
  const rows: Row[] = [];
  const opened: Record<string, number> = {};
  for (const colors of [false, true]) {
    const start = performance.now();
    const view = await SourceView.open(host, note, tauriBackend, {
      mac: false,
      label: note,
      onError: (message) => console.error(message),
      colors,
    });
    opened[colors ? 'with colours' : 'without colours'] = performance.now() - start;
    const unlisten = await events.edHighlight.listen((event) => view.takeColors(event.payload));
    const lines = view.view.state.doc.lines;
    // From the end to the start, so earlier positions do not move.
    const positions: [string, number][] = [
      ['end', Math.round(lines * 0.98)],
      ['middle', Math.round(lines * 0.5)],
      ['start', Math.round(lines * 0.02)],
    ];
    for (const [position, line] of positions) {
      const samples = await typeAt(view, line, keys);
      rows.push({
        colors,
        position,
        line,
        proc: stats(samples.map((s) => s.proc)),
        nextPaint: stats(samples.map((s) => s.paint)),
        firstFrame: samples.filter((s) => s.firstFrame).length / (samples.length || 1),
      });
    }
    unlisten();
    view.destroy();
  }
  host.remove();
  const results = {
    started: new Date().toISOString(),
    userAgent: navigator.userAgent,
    keys,
    timerResolutionMs: timerResolution(),
    openMs: opened,
    rows,
  };
  // The results replace the note's text and are stored at once.
  await tauriBackend.input(note, {
    kind: 'key',
    key: 'A',
    ctrl: true,
    alt: false,
    shift: false,
    meta: false,
    page: 1,
  });
  await tauriBackend.input(note, { kind: 'text', text: `UL-BENCH ${JSON.stringify(results)}\n` });
  await commands.tabStore(note);
  console.info('The measurement is done', results);
}

function timerResolution(): number {
  let smallest = Infinity;
  let last = performance.now();
  for (let i = 0; i < 10000; i++) {
    const now = performance.now();
    if (now > last) {
      smallest = Math.min(smallest, now - last);
      last = now;
    }
  }
  return smallest;
}
