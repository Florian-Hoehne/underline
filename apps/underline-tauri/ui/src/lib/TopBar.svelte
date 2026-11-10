<!--
  M-03.1 to M-03.3: the top bar. The burger button shows or hides the menu
  bar (stored like a quick switch, UC-010 A3); the magnifier switches the
  search overlay (UC-008 step 1, M-03.2): opening puts the keyboard into its
  field, closing empties it. With the menu bar hidden, the name of the
  workspace stands in its place.
-->
<script lang="ts">
  import MenuBar from './MenuBar.svelte';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    onOpenWorkspace?: () => void;
    onNewWorkspace?: () => void;
  };

  let { shell, onOpenWorkspace, onNewWorkspace }: Props = $props();

  const shown = $derived(shell.settings?.showMenuBar ?? false);
  const searchHint = $derived(`Search notes  ${shell.command}+Shift+F`);

  /** M-03.3: the folder name of the workspace, as the backend names it. */
  const workspaceName = $derived(shell.workspace?.name ?? 'No workspace open');

  function toggleSearch() {
    shell.search.toggle();
  }
</script>

<header class="top-bar">
  <button
    type="button"
    class="icon"
    aria-label="Menu"
    title={shown ? 'Hide the menu' : 'Show the menu'}
    aria-expanded={shown}
    onclick={() => shell.toggleMenuBar()}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M3 4.5h10M3 8h10M3 11.5h10" />
    </svg>
  </button>
  <button
    type="button"
    class="icon"
    aria-label="Search"
    title={searchHint}
    aria-expanded={shell.search.open}
    data-search-toggle
    onclick={toggleSearch}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="6.75" cy="6.75" r="3.75" />
      <path d="M9.5 9.5l3.5 3.5" />
    </svg>
  </button>
  {#if shown}
    <MenuBar {shell} {onOpenWorkspace} {onNewWorkspace} />
  {:else}
    <strong class="workspace-name" title={workspaceName}>{workspaceName}</strong>
  {/if}
</header>

<style>
  .top-bar {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
    padding: var(--space-1) var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  .icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .icon:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .icon svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }

  .workspace-name {
    min-width: 0;
    margin-left: var(--space-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
