// Where the facade's time goes in WebKitGTK: the steps of one keystroke's DOM
// work taken apart, with and without content-visibility and contenteditable.
// Run with `F0_DIAG=1`.

import { invoke } from '@tauri-apps/api/core';
import { doubleRaf, stats } from './timing';

type Log = (msg: string) => void;

export interface DiagRow {
  config: string;
  step: string;
  median: number;
  p99: number;
}

export async function runDiag(
  host: HTMLElement,
  log: Log,
  blocksLimit?: number,
): Promise<DiagRow[]> {
  await invoke('a_open');
  const html = await invoke<string>('a_render_all');
  const rows: DiagRow[] = [];
  const configs = [
    { cv: true, editable: true },
    { cv: false, editable: true },
    { cv: true, editable: false },
    { cv: false, editable: false },
  ];
  for (const { cv, editable } of configs) {
    const root = document.createElement('div');
    root.className = cv ? 'facade cv' : 'facade';
    if (editable) root.contentEditable = 'true';
    root.spellcheck = false;
    root.innerHTML = html;
    if (blocksLimit) while (root.children.length > blocksLimit) root.lastElementChild?.remove();
    host.appendChild(root);
    const n = root.children.length;
    let k = Math.floor(n / 2);
    while (k < n && root.children[k].tagName !== 'P') k++;
    root.children[k].scrollIntoView({ block: 'center' });
    if (editable) root.focus();
    await doubleRaf();
    const t: Record<string, number[]> = {
      replace: [],
      select: [],
      layout: [],
      position: [],
      frame: [],
      total: [],
    };
    for (let i = 0; i < 40; i++) {
      const old = root.children[k];
      const fresh = old.cloneNode(true) as HTMLElement;
      if (fresh.lastChild?.nodeType === Node.TEXT_NODE) fresh.lastChild.nodeValue += 'e';
      const a = performance.now();
      old.replaceWith(fresh);
      const b = performance.now();
      if (editable) {
        const tn = fresh.lastChild;
        document
          .getSelection()
          ?.collapse(
            tn ?? fresh,
            tn?.nodeType === Node.TEXT_NODE ? (tn.nodeValue?.length ?? 0) : 0,
          );
      }
      const c = performance.now();
      void fresh.getBoundingClientRect().top;
      const d = performance.now();
      if (editable) {
        const sel = document.getSelection();
        if (sel?.focusNode) {
          const r = document.createRange();
          r.setStart(fresh, 0);
          r.setEnd(sel.focusNode, sel.focusOffset);
          void r.toString().length;
        }
      }
      const e = performance.now();
      const f = await doubleRaf();
      if (i >= 5) {
        t.replace.push(b - a);
        t.select.push(c - b);
        t.layout.push(d - c);
        t.position.push(e - d);
        t.frame.push(f - e);
        t.total.push(f - a);
      }
    }
    const config = `${n} blocks, content-visibility ${cv ? 'auto' : 'off'}, ${editable ? 'contenteditable' : 'not editable'}`;
    for (const [step, values] of Object.entries(t)) {
      const s = stats(values);
      rows.push({ config, step, median: s.median, p99: s.p99 });
    }
    log(`diag ${config}: ${JSON.stringify(rows.slice(-6).map((r) => [r.step, r.median, r.p99]))}`);
    root.remove();
  }
  return rows;
}
