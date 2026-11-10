<!--
  The window: the top bar with the menu (M-03), the central area and the
  status bar (M-04.7).

  The central area (UC-002):
  - while the workspace to start with opens, nothing yet (plan section 14),
    so the empty screen does not flash;
  - without a workspace, the offer to open or create one (M-02);
  - with one, the note overview (M-05) beside the tabs and the note (M-06).
  The Open and Create dialogs float above it, not modal (M-21.1, M-21.2),
  and so do "Open web address" (M-07.3), "Changes not stored" (M-06.3,
  M-21.4) and the note dialogs "New note", "Rename note", "Delete note",
  "Restore note" and "Remove deleted notes" (M-14).

  F2 renames the note of the active tab (M-14.2, UC-006 step 1), wherever
  the keyboard is, as in egui.
-->
<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { focusFallback } from './lib/focus';
  import CloseQuestionDialog from './lib/CloseQuestionDialog.svelte';
  import CreateDialog from './lib/CreateDialog.svelte';
  import DeleteDialog from './lib/DeleteDialog.svelte';
  import NewNoteDialog from './lib/NewNoteDialog.svelte';
  import PurgeDialog from './lib/PurgeDialog.svelte';
  import RenameDialog from './lib/RenameDialog.svelte';
  import RestoreDialog from './lib/RestoreDialog.svelte';
  import EmptyScreen from './lib/EmptyScreen.svelte';
  import NoteArea from './lib/NoteArea.svelte';
  import OpenDialog from './lib/OpenDialog.svelte';
  import OpenWebDialog from './lib/OpenWebDialog.svelte';
  import Overview from './lib/Overview.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TopBar from './lib/TopBar.svelte';
  import type { Shell } from './lib/shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const openWorkspace = () => void shell.startOpen();
  const newWorkspace = () => shell.startCreate();

  onDestroy(() => shell.disconnect());

  /** M-14.2, UC-006 step 1: F2 renames the note of the active tab. */
  function renameKey(event: KeyboardEvent) {
    if (
      event.key !== 'F2' ||
      event.shiftKey ||
      event.ctrlKey ||
      event.altKey ||
      event.metaKey ||
      shell.notes.rename !== null
    ) {
      return;
    }
    const active = shell.tabs.todoList.active ? null : shell.tabs.active;
    if (active !== null) {
      event.preventDefault();
      event.stopPropagation();
      void shell.startRename(active);
    }
  }

  // Before the editors see it, as egui takes F2 first.
  onMount(() => {
    window.addEventListener('keydown', renameKey, true);
    return () => window.removeEventListener('keydown', renameKey, true);
  });

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
    {#if shell.webQuestion !== null}
      <OpenWebDialog {shell} address={shell.webQuestion} />
    {/if}
    {#if shell.notes.newNote}
      <NewNoteDialog {shell} dialog={shell.notes.newNote} />
    {/if}
    {#if shell.notes.rename}
      <RenameDialog {shell} dialog={shell.notes.rename} />
    {/if}
    {#if shell.notes.delete}
      <DeleteDialog {shell} dialog={shell.notes.delete} />
    {/if}
    {#if shell.notes.restore}
      <RestoreDialog {shell} dialog={shell.notes.restore} />
    {/if}
    {#if shell.notes.purge}
      <PurgeDialog {shell} dialog={shell.notes.purge} />
    {/if}
    {#if shell.closeQuestion}
      <CloseQuestionDialog {shell} question={shell.closeQuestion} />
    {/if}
  </main>
  <StatusBar
    root={shell.workspaceRoot}
    status={shell.status}
    offer={shell.statusOffer}
    onUndo={() => void shell.undoDelete()}
  />
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
