<!--
  M-06.1 to M-06.3 (UC-004 A5, FR-018): the tab bar, left to right, scrolling
  sideways when the tabs do not fit. A tab shows its note's title, marked
  " (deleted)" when the note was deleted outside; resting the pointer on it
  shows the note's file. A click activates the tab; "×" or a middle click
  closes it.

  Later features add the marks for unstored changes and failed saves and
  the question when closing fails to store (F3), the context menu (M-06.4,
  F8), and the tab "Todos (N)" (M-06.5, F15). Until then a tab "Todos" that
  was active is remembered, but no note tab shows as active.

  The active tab scrolls into view. After a tab closed, the keyboard goes to
  the tab now at its place, else to the one left of it, else to the note
  overview. (A tablist with arrow keys is left for F3, plan section 14.)
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { focusFallback } from './focus';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  /** M-06.5: while the tab "Todos" is active, no note tab is. */
  const active = $derived(
    shell.tabs.todoList.open && shell.tabs.todoList.active ? null : shell.tabs.active,
  );

  let bar = $state<HTMLElement | null>(null);

  $effect(() => {
    if (active === null || !bar) {
      return;
    }
    const tab = bar.querySelector<HTMLElement>('[aria-current="page"]');
    tab?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  });

  /** M-06.3: closes the tab of `note`; with `moveFocus` the keyboard moves on. */
  async function close(note: string, moveFocus: boolean) {
    const at = shell.tabs.tabs.findIndex((tab) => tab.note === note);
    await shell.closeTab(note);
    if (!moveFocus) {
      return;
    }
    await tick();
    const labels = bar?.querySelectorAll<HTMLElement>('button.label') ?? [];
    const next = labels[Math.min(at, labels.length - 1)];
    if (next) {
      next.focus();
    } else {
      focusFallback();
    }
  }

  function onAuxClick(event: MouseEvent, note: string) {
    if (event.button === 1) {
      event.preventDefault();
      void close(note, false);
    }
  }

  /** No autoscroll on a middle press; the click closes the tab. */
  function onMouseDown(event: MouseEvent) {
    if (event.button === 1) {
      event.preventDefault();
    }
  }
</script>

<div class="tab-bar" bind:this={bar}>
  <ul class="tabs">
    {#each shell.tabs.tabs as tab (tab.note)}
      <li class="tab" class:active={tab.note === active}>
        <button
          type="button"
          class="label"
          title={tab.path}
          aria-current={tab.note === active ? 'page' : undefined}
          onclick={() => shell.activateTab(tab.note)}
          onmousedown={onMouseDown}
          onauxclick={(event) => onAuxClick(event, tab.note)}>{tab.label}</button
        >
        <button
          type="button"
          class="close"
          aria-label="Close {tab.label}"
          onmousedown={onMouseDown}
          onauxclick={(event) => onAuxClick(event, tab.note)}
          onclick={() => close(tab.note, true)}>×</button
        >
      </li>
    {/each}
  </ul>
</div>

<style>
  .tab-bar {
    flex: none;
    overflow-x: auto;
    overflow-y: hidden;
    border-bottom: 1px solid var(--ui-rule);
  }

  .tabs {
    display: flex;
    align-items: center;
    width: max-content;
    min-height: 32px;
    margin: 0;
    padding: 2px var(--space-1);
    list-style: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: var(--space-1);
    margin-right: var(--space-1);
    border-right: 1px solid var(--ui-rule);
  }

  .label,
  .close {
    min-height: 24px;
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .label {
    padding: 0 var(--space-2);
    white-space: nowrap;
  }

  .close {
    min-width: 24px;
    padding: 0;
    line-height: 1;
  }

  /* The active tab: the chosen colour and, not by colour alone, a line
     under it (NFR-010, paritaet.md A-15). Pointing is weaker. */
  .active .label {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 0 -2px 0 var(--ui-text);
  }

  .close:hover,
  .tab:not(.active) .label:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .label:focus-visible,
  .close:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }
</style>
