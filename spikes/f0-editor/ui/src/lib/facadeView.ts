// The facade: one contenteditable root over the blocks' HTML. Every
// `beforeinput` is cancelled and sent to the owner as (block, UTF-16 offset
// in the block's text, input); the patch replaces only the changed block
// elements. A composition is let through and committed at its end (M-09.10
// to M-09.12, ADR-021).
//
// Three ways to hold the blocks in the DOM:
// - `all`: every block (the spike's first approach).
// - `cv`: every block, with `content-visibility: auto` (pathological in
//   WebKitGTK 2.52 when the DOM changes, see diag.ts).
// - `window`: only about WINDOW blocks around the cursor/viewport, with
//   spacers of estimated height above and below (plan section 11).

export type FacadeMode = 'all' | 'cv' | 'window';

const WINDOW = 160;

import type { FacadePatch, Input, Owner } from './types';
import { inputOf } from './sourceView';
import { afterPaint, isPromise, nextFrame, type Sample } from './timing';

export class FacadeView {
  readonly root: HTMLDivElement;
  private chain: Promise<unknown> = Promise.resolve();
  last: Promise<Sample | null> = Promise.resolve(null);
  private composing: { block: number; at: number } | null = null;
  private committed = false;
  /** Global index of the first block in the DOM, and the one after the last. */
  private start = 0;
  private end = 0;
  private total = 0;
  /** Children before the first block (the top spacer in window mode). */
  private lead = 0;
  private avg = 28;
  private scrollPending = false;

  constructor(
    parent: HTMLElement,
    private readonly owner: Owner,
    private readonly mode: FacadeMode,
  ) {
    this.root = document.createElement('div');
    this.root.className = mode === 'cv' ? 'facade cv' : 'facade';
    this.root.contentEditable = 'true';
    this.root.spellcheck = false;
    this.root.setAttribute('role', 'textbox');
    this.root.setAttribute('aria-multiline', 'true');
    this.root.setAttribute('aria-label', 'Facade');
    this.root.addEventListener('beforeinput', (e) => this.onBeforeInput(e));
    this.root.addEventListener('compositionstart', () => {
      this.composing = this.position();
      this.committed = false;
    });
    this.root.addEventListener('compositionend', (e) => {
      if (!this.committed) this.commit(e.timeStamp, e.data);
    });
    parent.appendChild(this.root);
    if (mode === 'window') {
      parent.addEventListener('scroll', () => this.onScroll(parent), { passive: true });
    }
  }

  /** Renders the blocks (first load, view switch). Returns the HTML size. */
  async load(): Promise<number> {
    if (this.mode === 'window') {
      const n = this.owner.blockCount();
      this.total = isPromise(n) ? await n : n;
      return this.renderWindow(0);
    }
    const r = this.owner.renderAll();
    const html = isPromise(r) ? await r : r;
    this.root.innerHTML = html;
    this.start = 0;
    this.end = this.total = this.root.children.length;
    this.lead = 0;
    return html.length;
  }

  /** Window mode: puts blocks around `center` into the DOM. */
  private async renderWindow(center: number): Promise<number> {
    if (!Number.isFinite(center)) center = this.start;
    const start = Math.max(0, Math.min(center - WINDOW / 2, this.total - WINDOW));
    const end = Math.min(this.total, start + WINDOW);
    const r = this.owner.renderRange(start, end);
    const html = (isPromise(r) ? await r : r).join('');
    const spacer = (h: number) =>
      `<div class="spacer" contenteditable="false" style="height:${h}px"></div>`;
    this.root.innerHTML = spacer(start * this.avg) + html + spacer((this.total - end) * this.avg);
    this.start = start;
    this.end = end;
    this.lead = 1;
    // Estimate the height of the blocks not in the DOM from those that are.
    const first = this.root.children[1] as HTMLElement | undefined;
    const last = this.root.children[this.root.children.length - 2] as HTMLElement | undefined;
    if (first && last && end > start) {
      const measured = (last.offsetTop + last.offsetHeight - first.offsetTop) / (end - start);
      if (Number.isFinite(measured) && measured > 4 && Math.abs(measured - this.avg) > 1) {
        this.avg = measured;
        this.updateSpacers();
      }
    }
    return html.length;
  }

  private updateSpacers(): void {
    if (this.mode !== 'window') return;
    const top = this.root.firstElementChild as HTMLElement;
    const bottom = this.root.lastElementChild as HTMLElement;
    top.style.height = `${this.start * this.avg}px`;
    bottom.style.height = `${(this.total - this.end) * this.avg}px`;
  }

  private onScroll(scroller: HTMLElement): void {
    if (this.scrollPending) return;
    this.scrollPending = true;
    requestAnimationFrame(() => {
      this.scrollPending = false;
      if (!this.root.isConnected || this.root.offsetParent === null) return;
      const y = scroller.scrollTop - this.root.offsetTop;
      const firstEl = this.el(Math.min(this.start + 20, this.end - 1));
      const lastEl = this.el(Math.max(this.end - 20, this.start));
      const top = firstEl?.offsetTop ?? 0;
      const bottom = lastEl ? lastEl.offsetTop + lastEl.offsetHeight : 0;
      if (y < top && this.start > 0) {
        void this.renderWindow(Math.floor(y / this.avg));
      } else if (y + scroller.clientHeight > bottom && this.end < this.total) {
        void this.renderWindow(Math.floor(y / this.avg));
      }
    });
  }

  /** The element of global block `block`, if it is in the DOM. */
  private el(block: number): HTMLElement | null {
    if (block < this.start || block >= this.end) return null;
    return (this.root.children[block - this.start + this.lead] as HTMLElement) ?? null;
  }

  /** Makes sure block `block` is in the DOM. */
  private async ensure(block: number): Promise<void> {
    if (this.mode === 'window' && (block < this.start || block >= this.end)) {
      await this.renderWindow(block);
    }
  }

  private commit(t0: number, text: string): void {
    const at = this.composing;
    this.composing = null;
    this.committed = true;
    if (!at) return;
    // The IME changed the block's DOM; the patch replaces it with the
    // owner's version anyway. Without text, only the DOM is restored.
    const input: Input = text ? { type: 'insertText', text } : { type: 'insertText', text: '' };
    this.enqueue(t0, input, at);
  }

  private onBeforeInput(e: InputEvent): void {
    if (e.inputType === 'insertCompositionText' || e.inputType === 'deleteCompositionText') {
      return; // Let the IME draw its composition.
    }
    e.preventDefault();
    if (e.inputType === 'insertFromComposition') {
      this.commit(e.timeStamp, e.data ?? '');
      return;
    }
    const input = inputOf(e);
    if (input) this.enqueue(e.timeStamp, input, null);
  }

  private enqueue(t0: number, input: Input, at: { block: number; at: number } | null): void {
    const frame = nextFrame();
    const run = this.chain.then(() => this.process(t0, input, at, frame));
    this.chain = run.catch((err) => console.error(err));
    this.last = run;
  }

  /** The block element holding the cursor and the UTF-16 offset in it. */
  position(): { block: number; at: number } | null {
    const sel = document.getSelection();
    if (!sel || !sel.focusNode || !this.root.contains(sel.focusNode)) return null;
    let node: Node | null = sel.focusNode;
    let offset = sel.focusOffset;
    if (node === this.root) {
      // Between blocks: the start of the block after.
      const i = Math.min(Math.max(offset - this.lead, 0), this.end - this.start - 1);
      return { block: this.start + Math.max(i, 0), at: 0 };
    }
    let el = (node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement) as Element | null;
    while (el && el.parentElement !== this.root) el = el.parentElement;
    if (!el || el.classList.contains('spacer')) return null;
    const block = Array.prototype.indexOf.call(this.root.children, el) - this.lead + this.start;
    const range = document.createRange();
    range.setStart(el, 0);
    range.setEnd(node, offset);
    return { block, at: range.toString().length };
  }

  private async process(
    t0: number,
    input: Input,
    at: { block: number; at: number } | null,
    frame: { time: number },
  ): Promise<Sample | null> {
    const start = performance.now();
    const pos = at ?? this.position();
    if (!pos) return null;
    const r = this.owner.facadeInput(pos.block, pos.at, input);
    const patch = isPromise(r) ? await r : r;
    const ownerMs = isPromise(r) ? this.owner.lastOwnerMs : performance.now() - start;
    const applying = performance.now();
    const sync = this.apply(patch);
    if (sync) await sync;
    const patched = performance.now();
    const { paint, e2e } = await afterPaint();
    return {
      proc: patched - start,
      e2e: e2e - t0,
      paint: paint - t0,
      owner: ownerMs,
      dom: patched - applying,
      firstFrame: frame.time >= patched && frame.time - t0 <= 17.5,
      reparsed: patch.reparsed,
    };
  }

  /** Applies a patch; returns a promise only when the window had to be
   * rendered again (patch outside the DOM, variant A then waits for IPC). */
  apply(patch: FacadePatch): Promise<void> | null {
    const delta = patch.html.length - (patch.to - patch.from);
    this.total = patch.blocks;
    if (patch.from < this.start || patch.to > this.end) {
      return this.ensure(patch.cursorBlock).then(() => {
        this.setCursor(patch.cursorBlock, patch.cursorAt);
      });
    }
    const children = this.root.children;
    const template = document.createElement('template');
    template.innerHTML = patch.html.join('');
    const fresh = Array.from(template.content.children);
    const from = patch.from - this.start + this.lead;
    const to = patch.to - this.start + this.lead;
    if (to - from === 1 && fresh.length === 1) {
      children[from].replaceWith(fresh[0]);
    } else {
      const before = children[to] ?? null;
      for (let i = to - 1; i >= from; i--) children[i].remove();
      for (const el of fresh) this.root.insertBefore(el, before);
      this.end += delta;
      // Later blocks moved: renumber them (debugging aid only).
      if (delta !== 0) {
        for (let b = patch.from + fresh.length; b < this.end; b++) {
          const el = this.el(b);
          if (el) el.dataset.block = String(b);
        }
      }
      this.updateSpacers();
    }
    if (this.el(patch.cursorBlock)) {
      this.setCursor(patch.cursorBlock, patch.cursorAt);
      return null;
    }
    return this.ensure(patch.cursorBlock).then(() => {
      this.setCursor(patch.cursorBlock, patch.cursorAt);
    });
  }

  /** Puts the cursor at UTF-16 offset `at` of block `block` (it must be in
   * the DOM; see `show`). */
  setCursor(block: number, at: number): void {
    const el = this.el(block);
    if (!el) return;
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    let rest = at;
    let node: Node | null = walker.nextNode();
    let target: [Node, number] = [el, 0];
    while (node) {
      const len = node.nodeValue?.length ?? 0;
      if (rest <= len) {
        target = [node, rest];
        break;
      }
      rest -= len;
      target = [node, len];
      node = walker.nextNode();
    }
    const sel = document.getSelection();
    sel?.collapse(target[0], target[1]);
  }

  /** Brings block `block` into the DOM and the viewport. */
  async show(block: number): Promise<void> {
    await this.ensure(block);
    this.el(block)?.scrollIntoView({ block: 'center' });
  }

  blockText(block: number): string {
    return this.el(block)?.textContent ?? '';
  }

  blocksInDom(): number {
    return this.end - this.start;
  }

  elementCount(): number {
    return this.root.getElementsByTagName('*').length;
  }

  async drive(inputType: string, data: string | null = null): Promise<Sample | null> {
    const e = new InputEvent('beforeinput', { inputType, data, bubbles: true, cancelable: true });
    this.root.dispatchEvent(e);
    return this.last;
  }

  /** A synthetic composition (no real IME): start, preedit, commit. */
  async driveComposition(text: string): Promise<Sample | null> {
    this.root.dispatchEvent(new CompositionEvent('compositionstart', { data: '' }));
    this.root.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertCompositionText',
        data: text,
        bubbles: true,
      }),
    );
    this.root.dispatchEvent(new CompositionEvent('compositionend', { data: text }));
    return this.last;
  }

  destroy(): void {
    this.root.remove();
  }
}
