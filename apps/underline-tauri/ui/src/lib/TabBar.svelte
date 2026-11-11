<!--
  M-06.1 to M-06.3 (UC-004 A5, FR-018): the tab bar, left to right, scrolling
  sideways when the tabs do not fit. A tab shows its note's title, marked
  " ⚠" when its last save failed, else " •" with changes not stored, and
  " (deleted)" when the note was deleted outside; resting the pointer on it
  shows the note's file. A click activates the tab; "×" or a middle click
  closes it, after storing it. When storing fails, "Changes not stored"
  asks first (CloseQuestionDialog).

  Plan section 14: the bar is a `tablist` over the note's `tabpanel`. Only
  the active tab is in the Tab sequence; the arrow keys, Home and End move
  among the tabs, Enter and Space activate one, and Delete closes it (Q-08:
  "×" is no longer in the Tab sequence).

  M-06.4 (features F8, F11): the context menu of a tab, "Rename…  F2",
  "Delete…" and "Export…  Ctrl+Shift+E" ("Cmd+Shift+E" on macOS, inventory
  D-11 fixed); it also opens with Shift+F10 or the context menu key (A-80).

  A later feature adds the tab "Todos (N)" (M-06.5, F15). Until then a tab "Todos" that was active is remembered,
  but no note tab shows as active.

  The active tab scrolls into view. After a tab closed with the keyboard,
  the keyboard goes to the tab now at its place, else to the one left of
  it, else to the note overview.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import ContextMenu, { asksForMenu, menuPlace, type MenuEntry } from './ContextMenu.svelte';
  import type { Shell } from './shell.svelte';
  import { focusTabAt, markDescription, NOTE_PANEL_ID, tabId } from './tabs';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  /** M-06.5: while the tab "Todos" is active, no note tab is. */
  const active = $derived(
    shell.tabs.todoList.open && shell.tabs.todoList.active ? null : shell.tabs.active,
  );
  /** The tab in the Tab sequence: the active one, else the first. */
  const reachable = $derived(active ?? shell.tabs.tabs[0]?.note ?? null);

  let bar = $state<HTMLElement | null>(null);

  $effect(() => {
    if (active === null || !bar) {
      return;
    }
    // The whole tab, its "×" too.
    const tab = bar.querySelector<HTMLElement>('[aria-selected="true"]')?.closest('.tab');
    tab?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  });

  const tabButtons = () => Array.from(bar?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []);

  /** M-06.3: closes the tab of `note`; with `moveFocus` the keyboard moves on. */
  async function close(note: string, moveFocus: boolean) {
    const at = shell.tabs.tabs.findIndex((tab) => tab.note === note);
    const closed = await shell.closeTab(note);
    if (!moveFocus || !closed) {
      return;
    }
    await tick();
    focusTabAt(at);
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

  /** M-06.4: the context menu of a tab, with the element it opened on. */
  let menu = $state<{ note: string; label: string; x: number; y: number } | null>(null);
  let menuOpener: HTMLElement | null = null;

  function openMenu(event: MouseEvent | KeyboardEvent, note: string, label: string) {
    event.preventDefault();
    menuOpener = event.currentTarget as HTMLElement;
    menu = { note, label, ...menuPlace(event) };
  }

  /** The entries of the menu of `note`; they act on it after the menu closed. */
  function entriesFor(note: string): MenuEntry[] {
    return [
      { label: 'Rename…', shortcut: 'F2', action: () => void shell.startRename(note) },
      { label: 'Delete…', action: () => void shell.startDelete(note) },
      // UC-009 step 1: asks for the kind first.
      {
        label: 'Export…',
        shortcut: `${shell.command}+Shift+E`,
        action: () => void shell.startExport(note),
      },
    ];
  }

  const menuEntries = $derived(menu ? entriesFor(menu.note) : []);

  function closeMenu(back: boolean) {
    menu = null;
    if (back) {
      menuOpener?.focus();
    }
  }

  /** Arrow keys, Home and End move among the tabs; Delete closes one. */
  function onKey(event: KeyboardEvent, note: string) {
    if (asksForMenu(event)) {
      const tab = shell.tabs.tabs.find((t) => t.note === note);
      openMenu(event, note, tab?.title ?? note);
      return;
    }
    const tabs = tabButtons();
    const at = tabs.indexOf(event.currentTarget as HTMLElement);
    const to = {
      ArrowLeft: at - 1,
      ArrowRight: at + 1,
      Home: 0,
      End: tabs.length - 1,
    }[event.key];
    if (to !== undefined) {
      event.preventDefault();
      tabs[(to + tabs.length) % tabs.length]?.focus();
    } else if (event.key === 'Delete') {
      event.preventDefault();
      void close(note, true);
    }
  }
</script>

<div class="tab-bar" bind:this={bar}>
  <div class="tabs" role="tablist" aria-label="Open notes">
    {#each shell.tabs.tabs as tab (tab.note)}
      <div class="tab" class:active={tab.note === active} role="presentation">
        <button
          type="button"
          role="tab"
          id={tabId(tab.note)}
          class="label"
          title={tab.path}
          aria-selected={tab.note === active}
          aria-controls={tab.note === active ? NOTE_PANEL_ID : undefined}
          tabindex={tab.note === reachable ? 0 : -1}
          aria-describedby={markDescription(tab.label, tab.title)
            ? `${tabId(tab.note)}-marks`
            : undefined}
          aria-keyshortcuts="Delete Shift+F10"
          onclick={() => shell.activateTab(tab.note)}
          onkeydown={(event) => onKey(event, tab.note)}
          onmousedown={onMouseDown}
          oncontextmenu={(event) => openMenu(event, tab.note, tab.title)}
          onauxclick={(event) => onAuxClick(event, tab.note)}>{tab.label}</button
        >
        {#if markDescription(tab.label, tab.title)}
          <span id="{tabId(tab.note)}-marks" hidden>{markDescription(tab.label, tab.title)}</span>
        {/if}
        <button
          type="button"
          class="close"
          tabindex="-1"
          aria-label="Close {tab.label}"
          onmousedown={onMouseDown}
          onauxclick={(event) => onAuxClick(event, tab.note)}
          onclick={() => close(tab.note, true)}>×</button
        >
      </div>
    {/each}
  </div>
</div>
{#if menu}
  <ContextMenu
    label="Tab {menu?.label ?? ''}"
    x={menu?.x ?? 0}
    y={menu?.y ?? 0}
    onClose={closeMenu}
    entries={menuEntries}
  />
{/if}

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
