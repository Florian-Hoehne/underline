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
  D-11 fixed), then "Snapshots…" (UC-026 step 1); it also opens with
  Shift+F10 or the context menu key (A-80).

  M-06.5 (feature F15, UC-021 BR-006): while the todo list is open, the
  one tab "Todos (N)" follows the note tabs, N the open todos ("Todos"
  until they were read). A click shows it in place of the note; "×", a
  middle click or Delete closes it. Opening a note makes its tab the active
  one; the tab "Todos" stays open. While it is active, no note tab is.

  The active tab scrolls into view. After a tab closed with the keyboard,
  the keyboard goes to the tab now at its place, else to the one left of
  it, else to the note overview.

  Package 7a (FR-096, FR-097, UC-004 A18 to A20, BR-018): each pane of the
  split view has its own tab bar (`pane`); the tab "Todos" is in the pane
  that was active when it was opened (OQ-279). A tab's context menu has "Open to the side" (A18; for the active
  tab's note A19 shows its other editing view) and, while the split view
  shows, "End the split view" (A20). A click on a tab makes its pane the
  active one. After the review: the right pane's tab bar ends with a
  button "End the split view" (and, while the window is too narrow for the
  tool panel beside two panes, a switch of the tool panel, `trailing`); a
  tab whose note is open in the other pane too carries "⇆" with the hint
  "Also open in the other pane"; the active tab of the pane not active is
  dimmed.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import ContextMenu, { asksForMenu, menuPlace, type MenuEntry } from './ContextMenu.svelte';
  import { keyText } from './commands';
  import type { Snippet } from 'svelte';
  import type { PaneName } from './bindings';
  import { COMMAND } from './commandList';
  import { openElsewhere, paneOf } from './panes';
  import type { Shell } from './shell.svelte';
  import { focusTabAt, markDescription, notePanelId, TODO_TAB_ID, tabId } from './tabs';

  type Props = {
    shell: Shell;
    pane?: PaneName;
    /** UC-004 BR-018: whether this pane is the active one (only with the split view). */
    paneActive?: boolean;
    /** Buttons at the end of the bar. */
    trailing?: Snippet;
  };

  let { shell, pane = 'left', paneActive = true, trailing }: Props = $props();

  const mac = $derived(shell.platform === 'macos');

  /** UC-004 BR-018: the tabs of this pane. */
  const own = $derived(paneOf(shell.tabs, pane));
  const tabs = $derived(own.tabs);
  /** M-06.5, OQ-279: the tab "Todos" is in the pane it was opened in; the left one without the split view. */
  const todoTab = $derived(
    pane === (shell.tabs.split ? shell.tabs.todoList.pane : 'left') && shell.tabs.todoList.open,
  );
  /** M-06.5: while the tab "Todos" is active, no note tab is. */
  const todosActive = $derived(todoTab && shell.tabs.todoList.active);
  const active = $derived(todosActive ? null : own.active);
  const panelId = $derived(notePanelId(pane));
  /** The tab "Todos" stands for itself in `reachable`. */
  const TODOS = '\u0000todos';
  /** The tab in the Tab sequence: the active one, else the first. */
  const reachable = $derived(
    todosActive ? TODOS : (active ?? tabs[0]?.note ?? (todoTab ? TODOS : null)),
  );
  /** M-06.5: "Todos (N)". */
  const todosLabel = $derived(shell.todos === null ? 'Todos' : `Todos (${shell.todos.openCount})`);

  let bar = $state<HTMLElement | null>(null);

  $effect(() => {
    if ((active === null && !todosActive) || !bar) {
      return;
    }
    // The whole tab, its "×" too.
    const tab = bar.querySelector<HTMLElement>('[aria-selected="true"]')?.closest('.tab');
    tab?.scrollIntoView?.({ inline: 'nearest', block: 'nearest' });
  });

  const tabButtons = () => Array.from(bar?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []);

  /** M-06.3: closes the tab of `note`; with `moveFocus` the keyboard moves on. */
  async function close(note: string, moveFocus: boolean) {
    const at = tabs.findIndex((tab) => tab.note === note);
    const closed = await shell.closeTab(note, false, pane);
    if (!moveFocus || !closed) {
      return;
    }
    await tick();
    // The pane may be gone (A20); the keyboard goes to the tabs left then.
    focusTabAt(at, bar?.isConnected ? bar : document);
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
    const side: MenuEntry[] = [
      // UC-004 A18, A19: to the other pane; the active note in its other view.
      {
        label: 'Open to the side',
        separatorBefore: true,
        shortcut: keyText(shell.commandById(COMMAND.split)?.key, mac),
        action: () => void shell.openToSide(note),
      },
      ...(shell.split
        ? [{ label: 'End the split view', action: () => void shell.endSplit() }]
        : []),
    ];
    return [
      {
        label: 'Rename…',
        shortcut: keyText(shell.commandById('note.rename')?.key, mac),
        action: () => void shell.startRename(note),
      },
      { label: 'Delete…', action: () => void shell.startDelete(note) },
      // UC-009 step 1: asks for the kind first.
      {
        label: 'Export…',
        shortcut: keyText(shell.commandById('note.export')?.key, mac),
        action: () => void shell.startExport(note),
      },
      // UC-026 step 1: the snapshots of the note.
      { label: 'Snapshots…', action: () => void shell.showSnapshots(note) },
      ...side,
    ];
  }

  const menuEntries = $derived(menu ? entriesFor(menu.note) : []);

  function closeMenu(back: boolean) {
    menu = null;
    if (back) {
      menuOpener?.focus();
    }
  }

  /** M-06.5: closes the tab "Todos"; with `moveFocus` the keyboard moves on. */
  async function closeTodos(moveFocus: boolean) {
    const at = tabButtons().findIndex((tab) => tab.id === TODO_TAB_ID);
    await shell.closeTodoList();
    if (!moveFocus) {
      return;
    }
    await tick();
    focusTabAt(at);
  }

  function onTodosAuxClick(event: MouseEvent) {
    if (event.button === 1) {
      event.preventDefault();
      void closeTodos(false);
    }
  }

  /** Arrow keys, Home and End move among the tabs; Delete closes one. */
  function onKey(event: KeyboardEvent, note: string | null) {
    if (note !== null && asksForMenu(event)) {
      const tab = tabs.find((t) => t.note === note);
      openMenu(event, note, tab?.title ?? note);
      return;
    }
    const buttons = tabButtons();
    const at = buttons.indexOf(event.currentTarget as HTMLElement);
    const to = {
      ArrowLeft: at - 1,
      ArrowRight: at + 1,
      Home: 0,
      End: buttons.length - 1,
    }[event.key];
    if (to !== undefined) {
      event.preventDefault();
      buttons[(to + buttons.length) % buttons.length]?.focus();
    } else if (event.key === 'Delete') {
      event.preventDefault();
      if (note === null) {
        void closeTodos(true);
      } else {
        void close(note, true);
      }
    }
  }
</script>

<div class="tab-bar" class:inactive={!paneActive} bind:this={bar}>
  <div class="scroll">
    <div
      class="tabs"
      role="tablist"
      aria-label={shell.split ? `Open notes, ${pane} pane` : 'Open notes'}
    >
      {#each tabs as tab (tab.note)}
        <div class="tab" class:active={tab.note === active} role="presentation">
          <button
            type="button"
            role="tab"
            id={tabId(tab.note, pane)}
            class="label"
            title={tab.path}
            aria-selected={tab.note === active}
            aria-controls={tab.note === active ? panelId : undefined}
            tabindex={tab.note === reachable ? 0 : -1}
            aria-describedby={[
              markDescription(tab.label, tab.title) ? `${tabId(tab.note, pane)}-marks` : '',
              openElsewhere(shell.tabs, pane, tab.note) ? `${tabId(tab.note, pane)}-both` : '',
            ]
              .filter(Boolean)
              .join(' ') || undefined}
            aria-keyshortcuts="Delete Shift+F10"
            onclick={() => shell.activateTab(tab.note, pane)}
            onkeydown={(event) => onKey(event, tab.note)}
            onmousedown={onMouseDown}
            oncontextmenu={(event) => openMenu(event, tab.note, tab.title)}
            onauxclick={(event) => onAuxClick(event, tab.note)}
            >{tab.label}{#if openElsewhere(shell.tabs, pane, tab.note)}<span
                class="both"
                title="Also open in the other pane"
                aria-hidden="true">⇆</span
              >{/if}</button
          >
          {#if openElsewhere(shell.tabs, pane, tab.note)}
            <span id="{tabId(tab.note, pane)}-both" hidden>also open in the other pane</span>
          {/if}
          {#if markDescription(tab.label, tab.title)}
            <span id="{tabId(tab.note, pane)}-marks" hidden
              >{markDescription(tab.label, tab.title)}</span
            >
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
      {#if todoTab}
        <div class="tab" class:active={todosActive} role="presentation">
          <button
            type="button"
            role="tab"
            id={TODO_TAB_ID}
            class="label"
            aria-selected={todosActive}
            aria-controls={todosActive ? panelId : undefined}
            tabindex={reachable === TODOS ? 0 : -1}
            aria-keyshortcuts="Delete"
            onclick={async () => {
              // OQ-279: the tab's pane becomes the active one first, so it stays there.
              if (pane !== shell.activePane) {
                await shell.focusPane(pane);
              }
              if (!todosActive) {
                void shell.showTodoList();
              }
            }}
            onkeydown={(event) => onKey(event, null)}
            onmousedown={onMouseDown}
            onauxclick={onTodosAuxClick}>{todosLabel}</button
          >
          <button
            type="button"
            class="close"
            tabindex="-1"
            aria-label="Close Todos"
            onmousedown={onMouseDown}
            onauxclick={onTodosAuxClick}
            onclick={() => closeTodos(true)}>×</button
          >
        </div>
      {/if}
    </div>
  </div>
  {#if trailing}
    <div class="trailing">{@render trailing()}</div>
  {/if}
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
    display: flex;
    flex: none;
    align-items: center;
    border-bottom: 1px solid var(--ui-rule);
  }

  .scroll {
    flex: 1 1 auto;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
  }

  .trailing {
    display: flex;
    flex: none;
    gap: 2px;
    padding: 0 var(--space-1);
  }

  .both {
    margin-left: var(--space-1);
    color: var(--ui-hint);
  }

  /* UC-004 BR-018: the active tab of the pane not active is dimmed. */
  .inactive .active .label {
    background: color-mix(in srgb, var(--ui-chosen) 45%, transparent);
    box-shadow: inset 0 -2px 0 var(--ui-hint);
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
