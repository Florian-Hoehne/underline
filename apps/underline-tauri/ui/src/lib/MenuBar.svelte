<!--
  M-03.4 to M-03.7: the menu bar with File, Workspace, Theme and Settings.

  Keyboard (NFR-011), after the WAI-ARIA menubar pattern: Tab reaches the
  bar once; Left and Right move between the menus (and keep one open);
  Down, Enter or Space open a menu at its first entry, Up at its last; Up,
  Down, Home and End move within it; Escape closes it and returns to its
  title; Tab leaves and closes it. With a menu open, pointing at another
  title opens that one, as in the egui app. Choosing an entry closes the
  menu.

  The Workspace menu holds a text field, so it is a small dialog rather than
  a menu.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    onOpenWorkspace?: () => void;
    onNewWorkspace?: () => void;
  };

  let { shell, onOpenWorkspace, onNewWorkspace }: Props = $props();

  type MenuId = 'file' | 'workspace' | 'theme' | 'settings';
  const MENUS: { id: MenuId; label: string; popup: 'menu' | 'dialog' }[] = [
    { id: 'file', label: 'File', popup: 'menu' },
    { id: 'workspace', label: 'Workspace', popup: 'dialog' },
    { id: 'theme', label: 'Theme', popup: 'menu' },
    { id: 'settings', label: 'Settings', popup: 'menu' },
  ];

  /** The open menu, if any. */
  let open = $state<MenuId | null>(null);
  /** The title Tab reaches (roving tabindex). */
  let current = $state<MenuId>('file');

  let bar = $state<HTMLElement | null>(null);
  let popup = $state<HTMLElement | null>(null);
  const titles = $state<Partial<Record<MenuId, HTMLButtonElement>>>({});

  const workspaceOpen = $derived(shell.workspaceRoot !== null);
  const theme = $derived(shell.settings?.theme ?? '');

  async function openMenu(id: MenuId, focus: 'first' | 'last' | 'none') {
    open = id;
    current = id;
    await tick();
    if (!popup) {
      return;
    }
    keepInWindow(popup);
    if (focus === 'none') {
      return;
    }
    if (id === 'workspace') {
      popup.querySelector('input')?.focus();
      return;
    }
    const all = entries();
    (focus === 'first' ? all[0] : all.at(-1))?.focus();
  }

  /** Moves a popup left as far as it would stick out of the window. */
  function keepInWindow(element: HTMLElement) {
    const overflow = element.getBoundingClientRect().right - (window.innerWidth - 4);
    if (overflow > 0) {
      element.style.left = `${-overflow}px`;
    }
  }

  function close(returnFocus: boolean) {
    const id = open;
    open = null;
    openedByHover = null;
    if (returnFocus && id) {
      titles[id]?.focus();
    }
  }

  function entries(): HTMLElement[] {
    return popup ? Array.from(popup.querySelectorAll<HTMLElement>('[role^="menuitem"]')) : [];
  }

  /** Moves to the menu `delta` titles away; an open menu goes along. */
  function move(from: MenuId, delta: number) {
    const at = MENUS.findIndex((menu) => menu.id === from);
    const next = MENUS[(at + delta + MENUS.length) % MENUS.length].id;
    current = next;
    if (open) {
      void openMenu(next, 'first');
    } else {
      titles[next]?.focus();
    }
  }

  /**
   * The title whose menu the pointer opened by passing over it. The click
   * that usually follows must keep that menu open, not close it again.
   */
  let openedByHover: MenuId | null = null;

  function onTitleClick(id: MenuId) {
    if (open === id && openedByHover === id) {
      openedByHover = null;
    } else if (open === id) {
      close(false);
    } else {
      void openMenu(id, 'none');
    }
  }

  function onTitleKey(event: KeyboardEvent, id: MenuId) {
    switch (event.key) {
      case 'ArrowRight':
        move(id, 1);
        break;
      case 'ArrowLeft':
        move(id, -1);
        break;
      case 'ArrowDown':
      case 'Enter':
      case ' ':
        void openMenu(id, 'first');
        break;
      case 'ArrowUp':
        void openMenu(id, 'last');
        break;
      case 'Home':
        current = MENUS[0].id;
        titles[current]?.focus();
        break;
      case 'End':
        current = MENUS[MENUS.length - 1].id;
        titles[current]?.focus();
        break;
      case 'Escape':
        close(true);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function onTitlePointer(id: MenuId) {
    if (open && open !== id) {
      openedByHover = id;
      void openMenu(id, 'none');
    }
  }

  function onTitleLeave(id: MenuId) {
    if (openedByHover === id) {
      openedByHover = null;
    }
  }

  function onMenuKey(event: KeyboardEvent, id: MenuId) {
    const all = entries();
    const at = all.indexOf(document.activeElement as HTMLElement);
    switch (event.key) {
      case 'ArrowDown':
        all[(at + 1) % all.length]?.focus();
        break;
      case 'ArrowUp':
        all[(at - 1 + all.length) % all.length]?.focus();
        break;
      case 'Home':
        all[0]?.focus();
        break;
      case 'End':
        all.at(-1)?.focus();
        break;
      case 'ArrowRight':
        move(id, 1);
        break;
      case 'ArrowLeft':
        move(id, -1);
        break;
      case 'Escape':
        close(true);
        break;
      case 'Tab':
        close(false);
        return;
      default:
        return;
    }
    event.preventDefault();
  }

  function onDialogKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      close(true);
    }
  }

  /** Runs an entry's action and closes the menu, back on its title. */
  function choose(action: () => void) {
    close(true);
    action();
  }

  function checkWiki(event: SubmitEvent) {
    event.preventDefault();
    choose(() => void shell.checkWiki());
  }

  /** A menu closes when the focus or a click goes elsewhere. */
  function onFocusOut(event: FocusEvent) {
    const to = event.relatedTarget as Node | null;
    if (open && to && !bar?.contains(to)) {
      close(false);
    }
  }

  $effect(() => {
    if (!open) {
      return;
    }
    const outside = (event: PointerEvent) => {
      if (!bar?.contains(event.target as Node)) {
        close(false);
      }
    };
    window.addEventListener('pointerdown', outside, true);
    return () => window.removeEventListener('pointerdown', outside, true);
  });
</script>

<div class="menu-bar" role="menubar" tabindex="-1" bind:this={bar} onfocusout={onFocusOut}>
  {#each MENUS as menu (menu.id)}
    <div class="menu" role="none">
      <button
        type="button"
        class="title"
        role="menuitem"
        id="menu-{menu.id}"
        aria-haspopup={menu.popup}
        aria-expanded={open === menu.id}
        aria-controls={open === menu.id ? `menu-${menu.id}-popup` : undefined}
        tabindex={current === menu.id ? 0 : -1}
        bind:this={titles[menu.id]}
        onclick={() => onTitleClick(menu.id)}
        onkeydown={(event) => onTitleKey(event, menu.id)}
        onpointerenter={() => onTitlePointer(menu.id)}
        onpointerleave={() => onTitleLeave(menu.id)}
      >
        {menu.label}
      </button>

      {#if open === menu.id}
        {#if menu.id === 'file'}
          <div
            class="popup"
            role="menu"
            id="menu-file-popup"
            aria-labelledby="menu-file"
            tabindex="-1"
            bind:this={popup}
            onkeydown={(event) => onMenuKey(event, 'file')}
          >
            <button
              type="button"
              role="menuitem"
              tabindex="-1"
              aria-disabled={!workspaceOpen}
              onclick={() => {
                if (workspaceOpen) {
                  close(true);
                }
              }}
            >
              New note…
            </button>
            <div class="separator" role="separator"></div>
            <button
              type="button"
              role="menuitem"
              tabindex="-1"
              onclick={() => choose(() => onOpenWorkspace?.())}
            >
              Open workspace…
            </button>
            <button
              type="button"
              role="menuitem"
              tabindex="-1"
              onclick={() => choose(() => onNewWorkspace?.())}
            >
              New workspace…
            </button>
          </div>
        {:else if menu.id === 'workspace'}
          <div
            class="popup dialog"
            role="dialog"
            id="menu-workspace-popup"
            aria-labelledby="menu-workspace"
            tabindex="-1"
            bind:this={popup}
            onkeydown={onDialogKey}
          >
            <form class="wiki" onsubmit={checkWiki}>
              <label for="wiki-url">Wiki project URL</label>
              <div class="row">
                <input
                  id="wiki-url"
                  type="text"
                  spellcheck="false"
                  autocomplete="off"
                  bind:value={shell.wikiUrl}
                />
                <button type="submit" class="ui-button">Check</button>
              </div>
            </form>
          </div>
        {:else if menu.id === 'theme'}
          <div
            class="popup"
            role="menu"
            id="menu-theme-popup"
            aria-labelledby="menu-theme"
            tabindex="-1"
            bind:this={popup}
            onkeydown={(event) => onMenuKey(event, 'theme')}
          >
            {#each shell.themes as name (name)}
              <button
                type="button"
                role="menuitemradio"
                tabindex="-1"
                aria-checked={name === theme}
                onclick={() => choose(() => void shell.chooseTheme(name))}
              >
                <span class="radio" aria-hidden="true"></span>
                {name}
              </button>
            {/each}
          </div>
        {:else}
          <div
            class="popup"
            role="menu"
            id="menu-settings-popup"
            aria-labelledby="menu-settings"
            tabindex="-1"
            bind:this={popup}
            onkeydown={(event) => onMenuKey(event, 'settings')}
          >
            <button
              type="button"
              role="menuitem"
              tabindex="-1"
              onclick={() => choose(() => (shell.settingsOpen = !shell.settingsOpen))}
            >
              Settings…
              <span class="shortcut" aria-hidden="true">{shell.command}+,</span>
            </button>
          </div>
        {/if}
      {/if}
    </div>
  {/each}
</div>

<style>
  /* In a narrow window the titles wrap to a second row rather than leave
     the window. */
  .menu-bar {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    min-width: 0;
    margin-left: var(--space-1);
  }

  .menu-bar:focus {
    outline: none;
  }

  .menu {
    position: relative;
  }

  .title,
  .popup [role^='menuitem'] {
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .title {
    min-height: 28px;
    padding: 0 var(--space-2);
  }

  .title:hover,
  .title[aria-expanded='true'],
  .popup [role^='menuitem']:hover,
  .popup [role^='menuitem']:focus {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .popup {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    min-width: 12rem;
    padding: var(--space-1);
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.25);
  }

  .popup:focus {
    outline: none;
  }

  .popup [role^='menuitem'] {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
    padding: 0 var(--space-2);
    text-align: left;
    white-space: nowrap;
  }

  .popup [role^='menuitem']:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .popup [aria-disabled='true'] {
    color: var(--ui-hint);
    cursor: default;
  }

  .popup [aria-disabled='true']:hover:not(:focus) {
    color: var(--ui-hint);
    background: transparent;
  }

  .separator {
    height: 1px;
    margin: var(--space-1) 0;
    background: var(--ui-rule);
  }

  .radio {
    flex: none;
    box-sizing: border-box;
    width: 14px;
    height: 14px;
    border: 1.5px solid currentColor;
    border-radius: 50%;
  }

  [aria-checked='true'] .radio {
    background: radial-gradient(circle, currentColor 0 3px, transparent 3.5px);
  }

  .shortcut {
    margin-left: auto;
    padding-left: var(--space-4);
    color: var(--ui-hint);
  }

  [role='menuitem']:hover .shortcut,
  [role='menuitem']:focus .shortcut {
    color: inherit;
  }

  .dialog {
    min-width: 0;
    padding: var(--space-2);
  }

  .wiki {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  input {
    width: min(280px, calc(100vw - 140px));
    min-width: 80px;
    min-height: 28px;
    box-sizing: border-box;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }
</style>
