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

  An entry may be one of a group of choices (M-17.1: the levels of agent
  access, as egui's `radio`): it is a `menuitemradio` showing whether it is
  the one chosen. A line may stand before an entry (egui's `separator`).
  Opened from the status bar, the menu opens above the point it is given
  (`above`).

  An entry may open a submenu (M-05.4: "Export", feature F11), as egui's
  `menu_button`: pointing at it or clicking it opens the submenu beside it;
  Right, Enter and Space open it with the keyboard on its first entry,
  Left and Escape close it and give the keyboard back to the entry; Up,
  Down, Home and End to another entry close it. Near the right edge of the
  window it opens left of the menu.
-->
<script lang="ts" module>
  /**
   * One entry: its text, an optional key shown at the right, and what it
   * does; or its text and the entries of its submenu.
   */
  export type MenuEntry =
    | {
        label: string;
        shortcut?: string;
        action: () => void;
        /** One of a group of choices; whether it is the one chosen. */
        checked?: boolean;
        /** A line stands before it. */
        separatorBefore?: boolean;
      }
    | { label: string; entries: MenuEntry[] };

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
  import ContextMenu from './ContextMenu.svelte';

  type Props = {
    /** For screen readers, such as "Note Plan". */
    label: string;
    entries: MenuEntry[];
    /** Where it opens, in CSS pixels of the window. */
    x: number;
    y: number;
    /** Closed without a choice; `back` gives the keyboard back to the opener. */
    onClose: (back: boolean) => void;
    /** The first entry takes the keyboard when it opens (not for a submenu opened by pointing). */
    autofocus?: boolean;
    /** It is a submenu: Left closes it. */
    nested?: boolean;
    /**
     * A submenu that does not fit right of its menu opens left of this
     * edge (the menu's left edge), review F11.
     */
    flipAt?: number;
    /** It opens above `y` rather than below it (from the status bar). */
    above?: boolean;
  };

  let {
    label,
    entries,
    x,
    y,
    onClose,
    autofocus = true,
    nested = false,
    flipAt,
    above = false,
  }: Props = $props();

  /** The open submenu: its entry's index, where it opens, and whether it takes the keyboard. */
  let sub = $state<{ index: number; x: number; y: number; flip: number; focus: boolean } | null>(
    null,
  );

  let menu = $state<HTMLElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  // Its own entries, not those of an open submenu inside it.
  const items = () =>
    Array.from(
      menu?.querySelectorAll<HTMLElement>(
        ':scope > [role="menuitem"], :scope > [role="menuitemradio"]',
      ) ?? [],
    );

  onMount(() => {
    left = x;
    top = y;
    void tick().then(() => {
      // It stays inside the window.
      const box = menu?.getBoundingClientRect();
      if (box) {
        const fits = x + box.width <= window.innerWidth - 4;
        const start = !fits && flipAt !== undefined ? flipAt - box.width : x;
        left = Math.max(4, Math.min(start, window.innerWidth - box.width - 4));
        const from = above ? y - box.height : y;
        top = Math.max(4, Math.min(from, window.innerHeight - box.height - 4));
      }
      if (autofocus) {
        items()[0]?.focus();
      }
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
  function choose(entry: MenuEntry, index: number) {
    if ('entries' in entry) {
      openSub(index, true);
      return;
    }
    onClose(true);
    entry.action();
  }

  /** Opens the submenu of the entry `index` beside it. */
  function openSub(index: number, focus: boolean) {
    const box = items()[index]?.getBoundingClientRect();
    if (!box) {
      return;
    }
    if (sub?.index === index) {
      if (focus && !sub.focus) {
        // Already open by pointing: the keyboard goes into it.
        sub = null;
        void tick().then(() => openSub(index, true));
      }
      return;
    }
    const flip = menu?.getBoundingClientRect().left ?? box.left;
    sub = { index, x: box.right, y: box.top, flip, focus };
  }

  /** The submenu closed; `back` gives the keyboard back to its entry. */
  function closeSub(back: boolean) {
    const index = sub?.index;
    sub = null;
    if (back && index !== undefined) {
      items()[index]?.focus();
    }
  }

  /** A submenu's entries: choosing one closes this menu too, then acts. */
  function subEntries(entry: MenuEntry): MenuEntry[] {
    if (!('entries' in entry)) {
      return [];
    }
    return entry.entries.map((child) =>
      'entries' in child
        ? child
        : {
            ...child,
            action: () => {
              onClose(true);
              child.action();
            },
          },
    );
  }

  /** Pointing at an entry opens its submenu, or closes another one. */
  function onPoint(entry: MenuEntry, index: number) {
    if ('entries' in entry) {
      openSub(index, false);
    } else if (sub !== null) {
      sub = null;
    }
  }

  function onKey(event: KeyboardEvent) {
    const all = items();
    const at = all.indexOf(document.activeElement as HTMLElement);
    // Keys of an open submenu that it did not take are not this menu's.
    if (at < 0 && sub !== null && event.key !== 'Tab') {
      return;
    }
    switch (event.key) {
      case 'ArrowRight':
      case 'Enter':
      case ' ': {
        const entry = entries[at];
        if (entry === undefined || !('entries' in entry)) {
          // Enter and Space click the entry itself.
          return;
        }
        openSub(at, true);
        break;
      }
      case 'ArrowLeft':
        if (!nested) {
          return;
        }
        onClose(true);
        break;
      // Moving to another entry closes an open submenu (review F11).
      case 'ArrowDown':
        sub = null;
        all[(at + 1) % all.length]?.focus();
        break;
      case 'ArrowUp':
        sub = null;
        all[(at - 1 + all.length) % all.length]?.focus();
        break;
      case 'Home':
        sub = null;
        all[0]?.focus();
        break;
      case 'End':
        sub = null;
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
  {#each entries as entry, index (entry.label)}
    {#if 'entries' in entry}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        aria-haspopup="menu"
        aria-expanded={sub?.index === index}
        onclick={() => choose(entry, index)}
        onpointerenter={() => onPoint(entry, index)}
      >
        {entry.label}
        <span class="shortcut" aria-hidden="true">▸</span>
      </button>
    {:else}
      {#if entry.separatorBefore}
        <div class="separator" role="separator"></div>
      {/if}
      {#if entry.checked !== undefined}
        <button
          type="button"
          role="menuitemradio"
          tabindex="-1"
          aria-checked={entry.checked}
          onclick={() => choose(entry, index)}
          onpointerenter={() => onPoint(entry, index)}
        >
          <span class="radio" class:on={entry.checked} aria-hidden="true"></span>
          {entry.label}
        </button>
      {:else}
        <button
          type="button"
          role="menuitem"
          tabindex="-1"
          aria-keyshortcuts={entry.shortcut}
          onclick={() => choose(entry, index)}
          onpointerenter={() => onPoint(entry, index)}
        >
          {entry.label}
          {#if entry.shortcut}
            <span class="shortcut" aria-hidden="true">{entry.shortcut}</span>
          {/if}
        </button>
      {/if}
    {/if}
  {/each}
  {#if sub !== null}
    {@const opened = entries[sub.index]}
    {#key `${sub.index}|${sub.focus}`}
      <ContextMenu
        label={opened.label}
        entries={subEntries(opened)}
        x={sub.x}
        y={sub.y}
        autofocus={sub.focus}
        flipAt={sub.flip}
        nested
        onClose={closeSub}
      />
    {/key}
  {/if}
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

  [role='menuitem'],
  [role='menuitemradio'] {
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
  [role='menuitem']:focus,
  [role='menuitemradio']:hover,
  [role='menuitemradio']:focus {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  [role='menuitem']:focus-visible,
  [role='menuitemradio']:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  /* egui's radio button: a ring, filled when chosen (not by colour alone). */
  .radio {
    flex: none;
    box-sizing: border-box;
    width: 14px;
    height: 14px;
    border: 2px solid currentcolor;
    border-radius: 50%;
  }

  .radio.on {
    background: radial-gradient(circle, currentcolor 0 3px, transparent 3.5px);
  }

  .separator {
    height: 1px;
    margin: var(--space-1) 0;
    background: var(--ui-rule);
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
