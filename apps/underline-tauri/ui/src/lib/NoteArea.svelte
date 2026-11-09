<!--
  The central area of an open workspace (UC-004): the tab bar on top and,
  below it, the active tab's note. Without an active tab it asks to select
  or create a note.

  The editor comes with feature F3; until then the active tab's area stays
  empty, in the colour of the notes' background.
-->
<script lang="ts">
  import TabBar from './TabBar.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const todosActive = $derived(shell.tabs.todoList.open && shell.tabs.todoList.active);
  const activeTab = $derived(
    todosActive ? undefined : shell.tabs.tabs.find((tab) => tab.note === shell.tabs.active),
  );
</script>

<section class="note-area">
  <TabBar {shell} />
  {#if activeTab}
    <div class="editor" role="region" aria-label={activeTab.label} data-view={activeTab.view}></div>
  {:else if !todosActive}
    <p class="hint">Select or create a note.</p>
  {/if}
</section>

<style>
  .note-area {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .editor {
    flex: 1 1 auto;
    min-height: 0;
    background: var(--palette-background, var(--ui-field));
  }

  .hint {
    margin: 0;
    padding: var(--space-2);
  }
</style>
