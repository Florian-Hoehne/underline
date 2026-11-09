<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { makeOwner, runAll, runChecks, type Results, type Row } from './lib/bench';
  import { SourceView } from './lib/sourceView';
  import { FacadeView } from './lib/facadeView';
  import { runDiag } from './lib/diag';

  type View = 'source' | 'facade';

  let host: HTMLElement;
  let variant = $state('A');
  let view = $state<View>('source');
  let status = $state('Loading the note…');
  let error = $state('');
  let busy = $state(true);
  let results = $state<Results | null>(null);
  let current: { destroy(): void } | null = null;

  const log = (msg: string) => {
    status = msg;
    void invoke('log', { message: msg });
  };

  async function show(): Promise<void> {
    current?.destroy();
    current = null;
    busy = true;
    error = '';
    status = 'Loading the note…';
    try {
      const owner = makeOwner(variant);
      const text = await owner.open();
      if (view === 'source') {
        current = new SourceView(host, text, owner, false);
      } else {
        const facade = new FacadeView(host, owner, 'window');
        await facade.load();
        current = facade;
      }
      status = `Variant ${variant}, ${view} view: type to edit, input goes to the owner.`;
    } catch (e) {
      error = `The note could not be opened: ${String(e)}`;
    } finally {
      busy = false;
    }
  }

  interface Config {
    autorun: boolean;
    keys: number | null;
    variants: string | null;
    extras: boolean;
    checks: boolean;
    diag: number | null;
  }

  async function measure(
    keys: number,
    extras: boolean,
    variants = ['A', 'B', 'A-async'],
    checks = true,
  ): Promise<Results> {
    current?.destroy();
    current = null;
    busy = true;
    error = '';
    try {
      const r = await runAll(host, { variants, keys, extras }, log);
      if (checks) {
        for (const v of ['A', 'B']) r.checks.push(...(await runChecks(host, v)));
      }
      results = r;
      const path = await invoke<string>('save_results', { json: JSON.stringify(r, null, 2) });
      log(`Measurement finished; results written to ${path}.`);
      return r;
    } catch (e) {
      error = `The measurement stopped: ${String(e)}`;
      throw e;
    } finally {
      busy = false;
    }
  }

  function fmt(n: number): string {
    return Number.isFinite(n) ? n.toFixed(2) : '–';
  }

  function typingRows(r: Results): Row[] {
    return r.rows.filter((x) => x.action === 'type');
  }

  onMount(async () => {
    log('window up');
    const config = await invoke<Config>('config');
    if (config.diag !== null) {
      try {
        const rows = await runDiag(host, log, config.diag || undefined);
        await invoke('save_results', { json: JSON.stringify(rows, null, 2) });
      } catch (e) {
        log(`ERROR ${String((e as Error)?.stack ?? e)}`);
      } finally {
        await invoke('quit');
      }
      return;
    }
    if (config.autorun) {
      try {
        const variants = config.variants?.split(',') ?? ['A', 'B', 'A-async'];
        await measure(config.keys ?? 500, config.extras, variants, config.checks);
      } catch (e) {
        log(`ERROR ${String((e as Error)?.stack ?? e)}`);
      } finally {
        await invoke('quit');
      }
      return;
    }
    await show();
  });
</script>

<header class="bar">
  <h1>Spike F0: who owns the text</h1>
  <label>
    Owner
    <select bind:value={variant} onchange={show} disabled={busy}>
      <option value="A">A: backend (IPC per key)</option>
      <option value="A-async">A: backend, async command</option>
      <option value="B">B: WASM in the webview</option>
    </select>
  </label>
  <label>
    View
    <select bind:value={view} onchange={show} disabled={busy}>
      <option value="source">Source</option>
      <option value="facade">Facade</option>
    </select>
  </label>
  <button onclick={() => measure(500, false)} disabled={busy}>Run measurement</button>
  <p class="status" role="status" aria-live="polite">{status}</p>
</header>

{#if error}
  <p class="error" role="alert">{error}</p>
{/if}

<main class="editor" bind:this={host} aria-busy={busy}></main>

{#if results}
  <section class="results" aria-label="Results">
    <table>
      <thead>
        <tr>
          <th>Owner</th><th>View</th><th>Where</th><th>Note</th>
          <th>median</th><th>p95</th><th>p99</th><th>max</th><th>p99 to paint</th>
        </tr>
      </thead>
      <tbody>
        {#each typingRows(results) as r, i (i)}
          <tr class:over={r.proc.p99 > 16}>
            <td>{r.variant}</td><td>{r.view}</td><td>{r.position}</td><td>{r.note}</td>
            <td>{fmt(r.proc.median)}</td><td>{fmt(r.proc.p95)}</td><td>{fmt(r.proc.p99)}</td>
            <td>{fmt(r.proc.max)}</td><td>{fmt(r.paint.p99)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}
