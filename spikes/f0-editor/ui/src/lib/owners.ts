// The two owners of the spike. A: every input is an IPC command to the
// backend. B: the owner runs here as WASM; its edits are mirrored to the
// backend after the frame is painted, each with the revision it was made on.

import { invoke } from '@tauri-apps/api/core';
import init, { WasmOwner } from '../wasm-pkg/f0_wasm.js';
import wasmUrl from '../wasm-pkg/f0_wasm_bg.wasm?url';
import type { BenchPosition, FacadePatch, Input, Owner, SourcePatch } from './types';

interface Timed<T> {
  patch: T;
  ownerUs: number;
}

/** Variant A: the backend owns the text. `async` uses Tauri's async commands. */
export class BackendOwner implements Owner {
  readonly name: string;
  lastOwnerMs = 0;
  mirrorErrors: string[] = [];

  constructor(private readonly async: boolean) {
    this.name = async ? 'A-async' : 'A';
  }

  async open(): Promise<string> {
    await invoke('a_open');
    return invoke<string>('note');
  }

  async sourceInput(at: number, input: Input): Promise<SourcePatch> {
    const cmd = this.async ? 'a_source_input_async' : 'a_source_input';
    const r = await invoke<Timed<SourcePatch>>(cmd, { input: { at, input } });
    this.lastOwnerMs = r.ownerUs / 1000;
    return r.patch;
  }

  async facadeInput(block: number, at: number, input: Input): Promise<FacadePatch> {
    const cmd = this.async ? 'a_facade_input_async' : 'a_facade_input';
    const r = await invoke<Timed<FacadePatch>>(cmd, { input: { block, at, input } });
    this.lastOwnerMs = r.ownerUs / 1000;
    return r.patch;
  }

  renderAll(): Promise<string> {
    return invoke<string>('a_render_all');
  }

  renderRange(from: number, to: number): Promise<string[]> {
    return invoke<string[]>('a_render_range', { from, to });
  }

  blockCount(): Promise<number> {
    return invoke<number>('a_block_count');
  }

  text(): Promise<string> {
    return invoke<string>('a_text');
  }

  async flush(): Promise<void> {}

  backendText(): Promise<string> {
    return invoke<string>('a_text');
  }

  positions(): Promise<BenchPosition[]> {
    return invoke<BenchPosition[]>('a_bench_positions');
  }

  blockText(index: number): Promise<string> {
    return invoke<string>('a_block_text', { index });
  }
}

let wasmReady: Promise<unknown> | null = null;

/** Variant B: the WASM core owns the text; the backend keeps a mirror. */
export class WasmOwnerB implements Owner {
  readonly name = 'B';
  lastOwnerMs = 0;
  mirrorErrors: string[] = [];
  private core: WasmOwner | null = null;
  private sending: Promise<void> = Promise.resolve();
  private scheduled = false;

  private get c(): WasmOwner {
    if (!this.core) throw new Error('the WASM owner is not open');
    return this.core;
  }

  async open(): Promise<string> {
    wasmReady ??= init({ module_or_path: wasmUrl });
    await wasmReady;
    const [text] = await Promise.all([invoke<string>('note'), invoke('b_open')]);
    this.core?.free();
    this.core = new WasmOwner(text);
    return text;
  }

  /** Sends the edits after the next paint, so typing never waits for IPC. */
  private schedule(): void {
    if (this.scheduled) return;
    this.scheduled = true;
    requestAnimationFrame(() =>
      setTimeout(() => {
        this.scheduled = false;
        this.send();
      }, 0),
    );
  }

  private send(): Promise<void> {
    const edits = JSON.parse(this.c.takeMirror());
    if (edits.length === 0) return this.sending;
    this.sending = this.sending.then(() =>
      invoke('b_mirror', { edits }).then(
        () => undefined,
        (e) => {
          this.mirrorErrors.push(JSON.stringify(e));
        },
      ),
    );
    return this.sending;
  }

  sourceInput(at: number, input: Input): SourcePatch {
    const patch = JSON.parse(this.c.sourceInput(JSON.stringify({ at, input })));
    this.schedule();
    return patch;
  }

  facadeInput(block: number, at: number, input: Input): FacadePatch {
    const patch = JSON.parse(this.c.facadeInput(JSON.stringify({ block, at, input })));
    this.schedule();
    return patch;
  }

  renderAll(): string {
    return this.c.renderAll();
  }

  renderRange(from: number, to: number): string[] {
    return JSON.parse(this.c.renderRange(from, to));
  }

  blockCount(): number {
    return this.c.blockCount();
  }

  text(): string {
    return this.c.text();
  }

  flush(): Promise<void> {
    return this.send();
  }

  async backendText(): Promise<string> {
    await this.flush();
    return invoke<string>('b_text');
  }

  hash(): string {
    return this.c.hash();
  }

  positions(): BenchPosition[] {
    return JSON.parse(this.c.benchPositions());
  }

  blockText(index: number): string {
    return this.c.blockText(index);
  }
}
