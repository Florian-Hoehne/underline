<!--
  The window: the top bar with the menu (M-03), the central area and the
  status bar (M-04.7).

  The central area (UC-002):
  - while the workspace to start with opens, nothing yet (plan section 14),
    so the empty screen does not flash;
  - without a workspace, the offer to open or create one (M-02);
  - with one, the note overview (M-05) beside the tabs and the note (M-06).
  The Open and Create dialogs float above it, not modal (M-21.1, M-21.2),
  and so does "Changes not stored" (M-06.3, M-21.4).
-->
<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { focusFallback } from './lib/focus';
  import CloseQuestionDialog from './lib/CloseQuestionDialog.svelte';
  import CreateDialog from './lib/CreateDialog.svelte';
  import EmptyScreen from './lib/EmptyScreen.svelte';
  import NoteArea from './lib/NoteArea.svelte';
  import OpenDialog from './lib/OpenDialog.svelte';
  import Overview from './lib/Overview.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TopBar from './lib/TopBar.svelte';
  import type { Shell } from './lib/shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const openWorkspace = () => void shell.startOpen();
  const newWorkspace = () => shell.startCreate();

  onDestroy(() => shell.disconnect());

  // When a workspace opens, the screen behind the keyboard may be gone
  // (the empty screen, a closed dialog); the keyboard goes to the overview.
  $effect(() => {
    if (shell.workspace?.root === undefined) {
      return;
    }
    void tick().then(() => {
      if (document.activeElement === document.body) {
        focusFallback();
      }
    });
  });
</script>

<div class="window">
  <TopBar {shell} onOpenWorkspace={openWorkspace} onNewWorkspace={newWorkspace} />
  <main class="central" aria-busy={shell.starting}>
    {#if shell.starting}
      <!-- Q-04: the egui app opens before its first frame and has no text for this. -->
    {:else if shell.workspace === null}
      <EmptyScreen onOpen={openWorkspace} onCreate={newWorkspace} />
    {:else}
      <div class="workbench">
        <Overview {shell} />
        <NoteArea {shell} />
      </div>
    {/if}
    {#if shell.dialog?.kind === 'open'}
      <OpenDialog {shell} dialog={shell.dialog} />
    {:else if shell.dialog?.kind === 'create'}
      <CreateDialog {shell} dialog={shell.dialog} />
    {/if}
    {#if shell.closeQuestion}
      <CloseQuestionDialog {shell} question={shell.closeQuestion} />
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
    position: relative;
    min-height: 0;
    overflow: auto;
  }

  .workbench {
    display: flex;
    height: 100%;
    min-height: 0;
  }
</style>
