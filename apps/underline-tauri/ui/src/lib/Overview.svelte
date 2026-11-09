<!--
  M-05.1, M-05.2 (UC-002 step 7, FR-017): the note overview on the left.

  Its width comes from the settings when it first shows and is dragged at
  its right edge with the mouse (Q-06), from 100 to 1000 pixels; let go more than a pixel away
  from the stored width, the width is stored at once (UC-010 A4). The
  notes are listed recently opened first; a click opens a note, which then
  moves to the top. The note of the active tab is marked.

  Later features add the git letters (M-05.3, F12), the context menu, the
  Delete key and the deleted notes (M-05.4 to M-05.6, F8), and the dialog of
  "New note…" (F8).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { overviewWidthWithin, type Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  let panel = $state<HTMLElement | null>(null);
  /** Where a drag of the edge started: pointer and width. */
  let drag: { x: number; width: number } | null = null;

  const width = $derived(
    shell.overviewWidth ?? overviewWidthWithin(shell.settings?.overviewWidth ?? 200),
  );
  const active = $derived(shell.tabs.active);

  onMount(() => shell.pinOverviewWidth());

  /**
   * The note last clicked. Moving its entry to the top takes the keyboard
   * from it in some webviews; it gets it back, as egui keeps the keyboard
   * in the overview after a click.
   */
  let chosen: string | null = null;

  $effect(() => {
    void shell.overview;
    if (chosen === null) {
      return;
    }
    const note = chosen;
    void tick().then(() => {
      if (document.activeElement === document.body) {
        panel?.querySelector<HTMLElement>(`button.entry[data-note="${CSS.escape(note)}"]`)?.focus();
      }
    });
  });

  function open(note: string) {
    chosen = note;
    void shell.openNote(note);
  }

  function startDrag(event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { x: event.clientX, width: panel?.getBoundingClientRect().width ?? width };
  }

  function moveDrag(event: PointerEvent) {
    if (drag) {
      shell.overviewWidth = overviewWidthWithin(drag.width + event.clientX - drag.x);
    }
  }

  function endDrag() {
    if (!drag) {
      return;
    }
    drag = null;
    const shown = panel?.getBoundingClientRect().width ?? width;
    void shell.overviewReleased(shown);
  }
</script>

<aside
  class="overview"
  style:width="{width}px"
  aria-labelledby="overview-heading"
  tabindex="-1"
  data-focus-fallback
  bind:this={panel}
>
  <div class="head">
    <h2 id="overview-heading">Notes</h2>
    <!-- F8 opens the New note dialog from here (M-14). -->
    <button type="button" class="ui-button">New note…</button>
  </div>
  <ul class="notes">
    {#each shell.overview as item (item.note)}
      <li>
        <button
          type="button"
          class="entry"
          data-note={item.note}
          aria-current={item.note === active ? 'true' : undefined}
          onclick={() => open(item.note)}>{item.label}</button
        >
      </li>
    {/each}
  </ul>
  <div
    class="edge"
    role="separator"
    aria-orientation="vertical"
    onpointerdown={startDrag}
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
  ></div>
</aside>

<style>
  .overview {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    box-sizing: border-box;
    max-width: calc(100% - 120px);
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--ui-rule);
  }

  .overview:focus {
    outline: none;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    line-height: 1.3;
  }

  .notes {
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: var(--space-1);
    overflow-y: auto;
    list-style: none;
  }

  .entry {
    display: block;
    box-sizing: border-box;
    width: 100%;
    min-height: 24px;
    padding: 2px var(--space-2);
    overflow: hidden;
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  /* The active note: the chosen colour and, not by colour alone, a bar on
     the left (NFR-010, paritaet.md A-15). Pointing is weaker. */
  .entry[aria-current='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 3px 0 0 var(--ui-text);
  }

  .entry:hover:not([aria-current='true']) {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  /* The edge to drag, a little wider than the line it sits on. */
  .edge {
    position: absolute;
    top: 0;
    right: -4px;
    bottom: 0;
    z-index: 1;
    width: 8px;
    cursor: col-resize;
    touch-action: none;
  }

  .edge:hover {
    background: linear-gradient(
      to right,
      transparent 3px,
      var(--ui-border) 3px 5px,
      transparent 5px
    );
  }
</style>
