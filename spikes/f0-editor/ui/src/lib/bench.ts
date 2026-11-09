// The measurement of spike F0: an in-page driver dispatches `beforeinput`
// events to either view, one after the other, and collects the samples.

import { invoke } from '@tauri-apps/api/core';
import { BackendOwner, WasmOwnerB } from './owners';
import { SourceView } from './sourceView';
import { FacadeView, type FacadeMode } from './facadeView';
import { doubleRaf, stalls, stats, type Sample, type Stats } from './timing';
import type { BenchPosition, Owner } from './types';

const TEXT = 'the quick brown fox *jumps* over the lazy_dog, ';

export interface Row {
  variant: string;
  view: 'source' | 'facade';
  position: string;
  action: 'type' | 'backspace' | 'enter+backspace';
  note: string;
  proc: Stats;
  e2e: Stats;
  paint: Stats;
  owner: Stats;
  dom: Stats;
  reparsed: number;
  /** Share of inputs whose patch made the first frame after the event. */
  firstFrame: number;
}

export interface Load {
  variant: string;
  what: string;
  ms: number;
  detail?: Record<string, number | string | boolean>;
}

export interface Check {
  variant: string;
  view: string;
  name: string;
  ok: boolean;
  detail: string;
}

export interface Results {
  started: string;
  userAgent: string;
  keys: number;
  coldStartMs: number;
  /** Smallest step of performance.now() seen, in ms. */
  timerResolutionMs: number;
  crossOriginIsolated: boolean;
  /** Inputs after which no frame came within a second. */
  stalls: number;
  rows: Row[];
  loads: Load[];
  checks: Check[];
}

type Log = (msg: string) => void;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const MODE_NOTE: Record<FacadeMode, string> = {
  window: `${'window of 160 blocks'}`,
  all: 'all blocks in the DOM',
  cv: 'all blocks, content-visibility auto',
};

/** Deterministic jitter, so keys arrive at any phase of the frame. */
let seed = 12345;
function jitter(): number {
  seed = (seed * 1103515245 + 12345) & 0x7fffffff;
  return (seed % 1700) / 100;
}

function row(
  base: Omit<Row, 'proc' | 'e2e' | 'paint' | 'owner' | 'dom' | 'reparsed' | 'firstFrame'>,
  samples: Sample[],
): Row {
  return {
    ...base,
    proc: stats(samples.map((s) => s.proc)),
    e2e: stats(samples.map((s) => s.e2e)),
    paint: stats(samples.map((s) => s.paint)),
    owner: stats(samples.map((s) => s.owner)),
    dom: stats(samples.map((s) => s.dom)),
    reparsed: samples.filter((s) => s.reparsed).length,
    firstFrame: samples.filter((s) => s.firstFrame).length / (samples.length || 1),
  };
}

export function makeOwner(variant: string): Owner {
  return variant === 'B' ? new WasmOwnerB() : new BackendOwner(variant === 'A-async');
}

interface Driven {
  drive(inputType: string, data?: string | null): Promise<Sample | null>;
}

async function series(
  view: Driven,
  inputs: [string, string | null][],
  warm: number,
  progress?: (msg: string) => void,
): Promise<Sample[]> {
  const out: Sample[] = [];
  for (let i = 0; i < inputs.length; i++) {
    await sleep(jitter());
    const s = await view.drive(inputs[i][0], inputs[i][1]);
    if (s && i >= warm) out.push(s);
    if (i % 200 === 199) progress?.(`${i + 1}/${inputs.length}`);
  }
  return out;
}

const typing = (n: number): [string, string][] =>
  Array.from({ length: n }, (_, i) => ['insertText', TEXT[i % TEXT.length]]);
const backspaces = (n: number): [string, null][] =>
  Array.from({ length: n }, () => ['deleteContentBackward', null]);
const enters = (n: number): [string, null][] =>
  Array.from({ length: 2 * n }, (_, i) => [
    i % 2 ? 'deleteContentBackward' : 'insertParagraph',
    null,
  ]);

export interface Options {
  variants: string[];
  keys: number;
  /** Extra scenarios: CM6 Markdown highlighting, facade without content-visibility. */
  extras: boolean;
}

export async function runAll(host: HTMLElement, opts: Options, log: Log): Promise<Results> {
  const results: Results = {
    started: new Date().toISOString(),
    userAgent: navigator.userAgent,
    keys: opts.keys,
    coldStartMs: performance.now(),
    timerResolutionMs: timerResolution(),
    crossOriginIsolated: globalThis.crossOriginIsolated ?? false,
    stalls: 0,
    rows: [],
    loads: [],
    checks: [],
  };
  const warm = 30;
  for (const variant of opts.variants) {
    const owner = makeOwner(variant);
    log(`${variant}: opening`);
    // ---- First load of each view.
    let t = performance.now();
    const text = await owner.open();
    const opened = performance.now() - t;
    t = performance.now();
    let source = new SourceView(host, text, owner, false);
    log(`${variant}: source view built, waiting for a frame`);
    await doubleRaf();
    results.loads.push({
      variant,
      what: 'open source view',
      ms: performance.now() - t + opened,
      detail: { ownerOpenMs: opened, chars: text.length },
    });
    source.destroy();
    for (const mode of ['window', 'all', 'cv'] as FacadeMode[]) {
      t = performance.now();
      const facade = new FacadeView(host, owner, mode);
      const htmlSize = await facade.load();
      const inserted = performance.now() - t;
      await doubleRaf();
      results.loads.push({
        variant,
        what: `open facade (${MODE_NOTE[mode]})`,
        ms: performance.now() - t + opened,
        detail: {
          renderAndInsertMs: inserted,
          htmlBytes: htmlSize,
          blocksInDom: facade.blocksInDom(),
          elements: facade.elementCount(),
        },
      });
      facade.destroy();
    }
    log(`${variant}: loaded`);

    const positions: BenchPosition[] = await owner.positions();
    // ---- Source view typing.
    const highlightModes = opts.extras && variant !== 'A-async' ? [false, true] : [false];
    for (const highlight of highlightModes) {
      await owner.open();
      source = new SourceView(host, await owner.text(), owner, highlight);
      // Positions move as text is typed before them: start from the end.
      for (const pos of [...positions].reverse()) {
        if ((highlight || variant === 'A-async') && pos.name !== 'middle') continue;
        source.setCursor(pos.source);
        await doubleRaf();
        const note = highlight ? 'CM6 lang-markdown highlighting' : '';
        const typed = await series(source, typing(opts.keys + warm), warm);
        results.rows.push(
          row({ variant, view: 'source', position: pos.name, action: 'type', note }, typed),
        );
        const back = await series(source, backspaces(100), 0);
        results.rows.push(
          row({ variant, view: 'source', position: pos.name, action: 'backspace', note }, back),
        );
        const en = await series(source, enters(30), 0);
        results.rows.push(
          row({ variant, view: 'source', position: pos.name, action: 'enter+backspace', note }, en),
        );
        log(
          `${variant} source ${pos.name} ${note}: ${results.rows.at(-3)?.proc.p99.toFixed(2)} ms p99`,
        );
        await sleep(50);
      }
      source.destroy();
    }

    // ---- Facade typing.
    // content-visibility: auto is far too slow in WebKitGTK when the DOM
    // changes (see diag.ts), so it is not typed in. The main mode keeps a
    // window of blocks in the DOM; every block in the DOM is an extra.
    const modes: FacadeMode[] =
      opts.extras && variant !== 'A-async' ? ['window', 'all'] : ['window'];
    for (const mode of modes) {
      await owner.open();
      const facade = new FacadeView(host, owner, mode);
      await facade.load();
      const pos0 = await owner.positions();
      for (const pos of [...pos0].reverse()) {
        if ((variant === 'A-async' || mode === 'all') && pos.name !== 'middle') continue;
        const note = MODE_NOTE[mode];
        facade.root.focus();
        await facade.show(pos.block);
        facade.setCursor(pos.block, pos.at);
        await doubleRaf();
        log(`${variant} facade ${pos.name}: typing`);
        const typed = await series(facade, typing(opts.keys + warm), warm, (m) =>
          log(`${variant} facade ${pos.name} ${m}`),
        );
        results.rows.push(
          row({ variant, view: 'facade', position: pos.name, action: 'type', note }, typed),
        );
        log(`${variant} facade ${pos.name}: backspace`);
        const back = await series(facade, backspaces(100), 0);
        results.rows.push(
          row({ variant, view: 'facade', position: pos.name, action: 'backspace', note }, back),
        );
        log(`${variant} facade ${pos.name}: enter`);
        const en = await series(facade, enters(30), 0);
        results.rows.push(
          row({ variant, view: 'facade', position: pos.name, action: 'enter+backspace', note }, en),
        );
        log(
          `${variant} facade ${pos.name} ${note}: ${results.rows.at(-3)?.proc.p99.toFixed(2)} ms p99`,
        );
        await sleep(50);
      }
      // ---- View switch, after the typing above (the facade must parse again).
      for (let i = 0; i < 5; i++) {
        let t1 = performance.now();
        facade.root.style.display = 'none';
        const s = new SourceView(host, await owner.text(), owner, false);
        await doubleRaf();
        results.loads.push({
          variant,
          what: `switch facade → source (facade: ${MODE_NOTE[mode]})`,
          ms: performance.now() - t1,
        });
        s.setCursor(positions[1].source);
        await s.drive('insertText', 'x');
        t1 = performance.now();
        s.destroy();
        await facade.load();
        facade.root.style.display = '';
        await doubleRaf();
        results.loads.push({
          variant,
          what: `switch source → facade (${MODE_NOTE[mode]})`,
          ms: performance.now() - t1,
        });
      }
      await owner.flush();
      facade.destroy();
    }

    results.checks.push(...(await mirrorCheck(variant, owner)));
    log(`${variant}: done`);
  }
  results.stalls = stalls;
  return results;
}

function timerResolution(): number {
  let min = Infinity;
  let last = performance.now();
  const end = last + 50;
  while (last < end) {
    const now = performance.now();
    if (now > last) min = Math.min(min, now - last);
    last = now;
  }
  return min;
}

/** Variant B: after all the typing, the backend's copy equals the owner's. */
async function mirrorCheck(variant: string, owner: Owner): Promise<Check[]> {
  const ownerText = await owner.text();
  const backend = await owner.backendText();
  const [h1, h2] = await Promise.all([
    invoke<string>('hash_of', { text: ownerText }),
    invoke<string>('hash_of', { text: backend }),
  ]);
  return [
    {
      variant,
      view: 'both',
      name: 'backend text equals owner text after the run (hash)',
      ok: h1 === h2 && ownerText === backend && owner.mirrorErrors.length === 0,
      detail: `owner ${h1}, backend ${h2}, ${ownerText.length} UTF-16 units, mirror errors: ${owner.mirrorErrors.length}`,
    },
  ];
}

/** Emoji, CJK, escapes and a synthetic composition in both views. */
export async function runChecks(host: HTMLElement, variant: string): Promise<Check[]> {
  const checks: Check[] = [];
  const owner = makeOwner(variant);
  const push = (view: string, name: string, ok: boolean, detail: string) =>
    checks.push({ variant, view, name, ok, detail });

  // ---- Source view.
  const original = await owner.open();
  const pos = (await owner.positions())[1];
  const source = new SourceView(host, original, owner, false);
  source.setCursor(pos.source);
  for (const t of ['🙂', '日本', 'x']) await source.drive('insertText', t);
  const p = pos.source;
  const expected = original.slice(0, p) + '🙂日本x' + original.slice(p);
  let backend = await owner.backendText();
  push(
    'source',
    'emoji + CJK typed: view, owner, backend byte-exact',
    source.text() === expected && (await owner.text()) === expected && backend === expected,
    `cursor after: ${source.view.state.selection.main.head} (expected ${p + 5})`,
  );
  push(
    'source',
    'cursor after emoji + CJK at the right UTF-16 offset',
    source.view.state.selection.main.head === p + 5,
    '',
  );
  for (let i = 0; i < 4; i++) await source.drive('deleteContentBackward');
  backend = await owner.backendText();
  push(
    'source',
    'backspace removes x, 本, 日, then 🙂 as a whole',
    source.text() === original && backend === original,
    `diff at ${firstDiff(source.text(), original)}`,
  );
  source.destroy();

  // ---- Facade.
  await owner.open();
  const fpos = (await owner.positions())[1];
  const facade = new FacadeView(host, owner, 'window');
  await facade.load();
  facade.root.focus();
  await facade.show(fpos.block);
  facade.setCursor(fpos.block, fpos.at);
  for (const t of ['🙂', '日本', 'x', '*']) await facade.drive('insertText', t);
  let ownerText = await owner.text();
  backend = await owner.backendText();
  const sourceAt = fpos.source;
  const fexpected = original.slice(0, sourceAt) + '🙂日本x\\*' + original.slice(sourceAt);
  push(
    'facade',
    'emoji + CJK + escaped * land byte-exact in the source',
    ownerText === fexpected && backend === fexpected,
    `diff at ${firstDiff(ownerText, fexpected)}`,
  );
  const domText = facade.blockText(fpos.block);
  const blockText = await owner.blockText(fpos.block);
  push(
    'facade',
    'block DOM text equals owner block text',
    domText === blockText,
    JSON.stringify(domText.slice(0, 60)),
  );
  const cur = facade.position();
  push(
    'facade',
    'cursor after the typed text',
    cur?.block === fpos.block && cur?.at === fpos.at + 6,
    JSON.stringify(cur),
  );
  for (let i = 0; i < 5; i++) await facade.drive('deleteContentBackward');
  ownerText = await owner.text();
  push(
    'facade',
    'backspace removes \\*, x, 本, 日, 🙂',
    ownerText === original,
    `diff at ${firstDiff(ownerText, original)}`,
  );
  // Synthetic composition (no real IME).
  await facade.driveComposition('日本');
  ownerText = await owner.text();
  push(
    'facade',
    'synthetic composition commits 日本 at compositionend',
    ownerText === original.slice(0, sourceAt) + '日本' + original.slice(sourceAt),
    '',
  );
  for (let i = 0; i < 2; i++) await facade.drive('deleteContentBackward');
  // Enter, then Backspace.
  await facade.drive('insertParagraph');
  const afterEnter = await owner.text();
  await facade.drive('deleteContentBackward');
  ownerText = await owner.text();
  push(
    'facade',
    'Enter then Backspace gives the original text back',
    ownerText === original,
    `after Enter: ${JSON.stringify(afterEnter.slice(sourceAt - 5, sourceAt + 8))}; after Backspace: ${JSON.stringify(ownerText.slice(sourceAt - 5, sourceAt + 8))}`,
  );
  facade.destroy();
  return checks;
}

function firstDiff(a: string, b: string): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) if (a[i] !== b[i]) return i;
  return a.length === b.length ? -1 : n;
}
