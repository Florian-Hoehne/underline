<!--
  The context menu of a note in the note overview (M-05.4) or of a tab
  (M-06.4), as egui opens it with the right mouse button at the pointer.

  For the keyboard (NFR-011, paritaet.md A-80) it also opens with Shift+F10
  or the context menu key, under the element; the opener decides that.
  Inside, after the WAI-ARIA menu pattern: the first entry has the
  keyboard, Up, Down, Home and End move, Enter and Space choose, Escape
  closes it and gives the keyboard back to the opener, Tab closes it. A
  click elsewhere closes it. Choosing an entry closes the menu first, then
  acts, so a dialog the entry opens takes the keyboard.
-->
<script lang="ts" module>
  /** One entry: its text, an optional key shown at the right, and what it does. */
  export type MenuEntry = { label: string; shortcut?: string; action: () => void };

  /**
   * Where a menu opens for the event that asked for it: at the pointer, or
   * under the element for a key (Shift+F10, the context menu key).
   */
  export function menuPlace(event: MouseEvent | KeyboardEvent): { x: number; y: number } {
    if (event instanceof MouseEvent && (event.clientX !== 0 || event.clientY !== 0)) {
      return { x: event.clientX, y: event.clientY };
    }
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: box.left, y: box.bottom };
  }

  /** Whether `event` asks for the context menu with the keyboard. */
  export function asksForMenu(event: KeyboardEvent): boolean {
    return (event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu';
  }
</script>

<script lang="ts">
  import { onMount, tick } from 'svelte';

  type Props = {
    /** For screen readers, such as "Note Plan". */
    label: string;
    entries: MenuEntry[];
    /** Where it opens, in CSS pixels of the window. */
    x: number;
    y: number;
    /** Closed without a choice; `back` gives the keyboard back to the opener. */
    onClose: (back: boolean) => void;
  };

  let { label, entries, x, y, onClose }: Props = $props();

  let menu = $state<HTMLElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  const items = () => Array.from(menu?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);

  onMount(() => {
    left = x;
    top = y;
    void tick().then(() => {
      // It stays inside the window.
      const box = menu?.getBoundingClientRect();
      if (box) {
        left = Math.max(4, Math.min(x, window.innerWidth - box.width - 4));
        top = Math.max(4, Math.min(y, window.innerHeight - box.height - 4));
      }
      items()[0]?.focus();
    });
    const outside = (event: PointerEvent) => {
      if (!menu?.contains(event.target as Node)) {
        onClose(false);
      }
    };
    window.addEventListener('pointerdown', outside, true);
    return () => window.removeEventListener('pointerdown', outside, true);
  });

  /**
   * The keyboard goes back to the opener first, so a dialog the entry
   * opens gives it back there when it closes (app-tester F8).
   */
  function choose(entry: MenuEntry) {
    onClose(true);
    entry.action();
  }

  function onKey(event: KeyboardEvent) {
    const all = items();
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
      case 'Escape':
        onClose(true);
        break;
      case 'Tab':
        onClose(false);
        return;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }

  function onFocusOut(event: FocusEvent) {
    const to = event.relatedTarget as Node | null;
    if (to && !menu?.contains(to)) {
      onClose(false);
    }
  }
</script>

<div
  class="context-menu"
  role="menu"
  aria-label={label}
  tabindex="-1"
  style:left="{left}px"
  style:top="{top}px"
  bind:this={menu}
  onkeydown={onKey}
  onfocusout={onFocusOut}
  oncontextmenu={(event) => event.preventDefault()}
>
  {#each entries as entry (entry.label)}
    <button
      type="button"
      role="menuitem"
      tabindex="-1"
      aria-keyshortcuts={entry.shortcut}
      onclick={() => choose(entry)}
    >
      {entry.label}
      {#if entry.shortcut}
        <span class="shortcut" aria-hidden="true">{entry.shortcut}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .context-menu {
    position: fixed;
    z-index: 30;
    display: flex;
    flex-direction: column;
    min-width: 10rem;
    padding: var(--space-1);
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.25);
  }

  .context-menu:focus {
    outline: none;
  }

  [role='menuitem'] {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    white-space: nowrap;
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  [role='menuitem']:hover,
  [role='menuitem']:focus {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  [role='menuitem']:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
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
</style>
