<!--
  The window (feature F1, the shell): the top bar with the menu (M-03), the
  central area, and the status bar (M-04.7). Without a workspace the central
  area offers to open or create one (M-02). Opening and creating a
  workspace come with feature F2.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import EmptyScreen from './lib/EmptyScreen.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TopBar from './lib/TopBar.svelte';
  import type { Shell } from './lib/shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  // F2 wires these to the Open and Create dialogs (M-21.1, M-21.2).
  function openWorkspace() {}
  function newWorkspace() {}

  onDestroy(() => shell.disconnect());
</script>

<div class="window">
  <TopBar {shell} onOpenWorkspace={openWorkspace} onNewWorkspace={newWorkspace} />
  <main class="central">
    {#if shell.workspaceRoot === null}
      <EmptyScreen onOpen={openWorkspace} onCreate={newWorkspace} />
    {/if}
  </main>
  <StatusBar root={shell.workspaceRoot} status={shell.status} />
</div>

<style>
  .window {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    height: 100%;
  }

  .central {
    min-height: 0;
    overflow: auto;
  }
</style>
