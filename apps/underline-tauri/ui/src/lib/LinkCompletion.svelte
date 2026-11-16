<!--
  FR-080, UC-005 A23, A24: the list of link completion, below the start of
  the link typed, or above it when there is more room there; it is as high
  as the room allows and follows its link when the window scrolls or
  changes its size. It shows the notes that match what was typed, each with
  its folder, or after a `#` the headings of the note named, indented by
  level. The text keeps the keyboard: the view gives the list its keys
  (Down, Up, Enter, Tab, Escape; `editor/linkCompletion.svelte.ts`) and
  names it for screen readers (`aria-controls`, `aria-activedescendant`);
  a click chooses an entry without taking the keyboard from the text.
  While nothing matches no list shows (A24), and screen readers hear "No
  suggestions"; while the headings are read, the list says so.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { linkCompletion, LIST_ID, optionId } from './editor/linkCompletion.svelte';

  /** The list's height at most, and the gap to the link and the window's edges. */
  const MAX_HEIGHT = 280;
  const GAP = 4;
  /** Below this room under the link, the list goes above it when there is more room there. */
  const ENOUGH = 160;

  const completion = linkCompletion;
  const list = $derived(completion.list);
  const shown = $derived(completion.context !== null && (list.entries.length > 0 || list.loading));

  let listElement = $state<HTMLElement | null>(null);
  let width = $state(320);
  let viewport = $state({ width: window.innerWidth, height: window.innerHeight });

  /** Below the link when there is room, else on the side with more room; within the window. */
  const position = $derived.by(() => {
    // Without a place (nothing laid out), at the top left of the window.
    const place = completion.place ?? { left: GAP, top: 0, bottom: 0 };
    const below = viewport.height - place.bottom - 2 * GAP;
    const aboveRoom = place.top - 2 * GAP;
    const above = below < ENOUGH && aboveRoom > below;
    const room = Math.max(above ? aboveRoom : below, 80);
    const left = Math.max(GAP, Math.min(place.left, viewport.width - width - GAP));
    return {
      left,
      top: above ? place.top - GAP : place.bottom + GAP,
      above,
      maxHeight: Math.min(MAX_HEIGHT, room),
    };
  });

  /** The chosen entry stays in view as the arrow keys move. */
  $effect(() => {
    const at = completion.cursor;
    if (shown) {
      listElement?.querySelector(`#${optionId(at)}`)?.scrollIntoView?.({ block: 'nearest' });
    }
  });

  onMount(() => {
    // The list follows its link; a scroll of the list itself is its own.
    const follow = (event: Event) => {
      if (event.target instanceof Node && listElement?.parentElement?.contains(event.target)) {
        return;
      }
      viewport = { width: window.innerWidth, height: window.innerHeight };
      if (completion.context !== null) {
        completion.relocate();
      }
    };
    window.addEventListener('scroll', follow, true);
    window.addEventListener('resize', follow);
    return () => {
      window.removeEventListener('scroll', follow, true);
      window.removeEventListener('resize', follow);
    };
  });

  /** The text keeps the keyboard when the list is clicked. */
  function keepFocus(event: MouseEvent) {
    event.preventDefault();
  }
</script>

<!-- Always there, so a screen reader hears each change of the list. -->
<p class="visually-hidden" aria-live="polite">{completion.spoken}</p>
{#if shown}
  <div
    class="completion"
    class:above={position.above}
    style:left="{position.left}px"
    style:top="{position.top}px"
    style:max-height="{position.maxHeight}px"
    role="presentation"
    bind:offsetWidth={width}
    onmousedown={keepFocus}
  >
    {#if list.headingsOf}
      <p class="caption">
        {list.headingsOf.title === null
          ? 'Headings of this note'
          : `Headings of ${list.headingsOf.title}`}
      </p>
    {/if}
    {#if list.loading}
      <p class="weak" aria-live="polite">Reading the headings…</p>
    {:else}
      <ul
        id={LIST_ID}
        class="entries"
        role="listbox"
        aria-label={list.headingsOf ? 'Headings to link to' : 'Notes to link to'}
        bind:this={listElement}
      >
        {#each list.entries as entry, at (`${at}:${entry.kind === 'note' ? entry.note : entry.anchor}`)}
          <!-- The keyboard stays in the text; the entries are for the pointer. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li
            id={optionId(at)}
            class="entry"
            role="option"
            aria-selected={at === completion.cursor}
            onpointermove={() => completion.point(at)}
            onclick={() => completion.choose(at)}
          >
            {#if entry.kind === 'note'}
              <span class="name">{entry.title}</span>
              {#if entry.folder}
                <span class="weak folder">{entry.folder}</span>
              {/if}
            {:else}
              <span
                class="name heading"
                style:padding-left="{Math.min(Math.max(entry.level - 1, 0), 4) * 12}px"
                >{entry.label}</span
              >
              <span class="visually-hidden">, level {entry.level}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  /* The palette's surface (PaletteOverlay.svelte), small and at the link. */
  .completion {
    position: fixed;
    z-index: 27;
    box-sizing: border-box;
    width: min(320px, calc(100vw - 8px));
    overflow-y: auto;
    padding: var(--space-1);
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.25);
  }

  .completion.above {
    transform: translateY(-100%);
  }

  .caption {
    margin: 0 0 var(--space-1);
    padding: 2px var(--space-2) var(--space-1);
    font-size: 0.85em;
    color: var(--ui-hint);
    border-bottom: 1px solid var(--ui-rule);
  }

  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* One row: the title, then its folder. */
  .entry {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
    min-height: 24px;
    padding: 2px var(--space-2);
    border-radius: var(--radius);
    cursor: pointer;
  }

  /* Chosen with the keyboard: shown by a line too, not only by colour. */
  .entry[aria-selected='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 2px 0 var(--ui-chosen-text);
  }

  .entry[aria-selected='true'] .weak {
    color: inherit;
  }

  .name {
    min-width: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .heading {
    font-weight: normal;
  }

  .folder {
    font-size: 0.85em;
  }

  .weak {
    margin: 0;
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }

  p.weak {
    padding: 2px var(--space-2);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
