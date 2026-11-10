<!--
  M-16 (FR-019, UC-004 step 4): the tool panel on the right of the note.

  M-16.1: its width comes from the settings when it first shows and is
  dragged at its left edge with the mouse only (Q-06), from 100 to 1000
  pixels, leaving the note at least 200 pixels; let go
  more than a pixel away from the stored width with a note tab active, the
  width is stored at once (UC-010 A4). Reading a settings file again sets it
  back (Q-03).

  M-16.2: "Properties", open at first: Place, Format, Size and Last change
  of the active note, or "Not saved yet" when its file cannot be read. The
  backend reads them when the note is stored, changed outside or shown,
  not for every look.

  M-16.5 (FR-041 to FR-043): "Outline (N)", "Code (N)" and "Figures (N)",
  open at first, each "None" when empty, made by the backend from the
  note's blocks for one revision (event `indexes`); until the first lists
  of the note come, the three are not shown, as in egui before it made
  them. A heading is indented 14 px per level below the note's highest
  level, at most 4 levels deep, and screen readers hear its level; a long
  entry wraps. A click shows the place: the facade scrolls its block to the top
  and leaves the cursor (M-09.16), the source view puts the cursor there
  and takes the keyboard.

  Later features add the todos (M-16.3, F15) and the git changes (M-16.4,
  F12) between the properties and the outline, as in egui.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { IndexEntryView } from './bindings';
  import { activeSourceView } from './editor/active';
  import { activeFacade } from './facade/active';
  import { toolPanelWidthWithin, type Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  let panel = $state<HTMLElement | null>(null);
  /** Where a drag of the edge started: pointer and width. */
  let drag: { x: number; width: number } | null = null;

  const width = $derived(
    shell.toolPanelWidth ?? toolPanelWidthWithin(shell.settings?.toolPanelWidth ?? 240),
  );
  const properties = $derived(shell.properties?.properties ?? null);
  /** M-16.5: the lists of the active note, once the backend made them. */
  const lists = $derived(
    shell.indexes !== null && shell.indexes.note === shell.tabs.active ? shell.indexes : null,
  );
  const sections = $derived<{ title: string; entries: IndexEntryView[] }[]>(
    lists
      ? [
          { title: 'Outline', entries: lists.outline },
          { title: 'Code', entries: lists.code },
          { title: 'Figures', entries: lists.figures },
        ]
      : [],
  );

  /** Headings deeper than this many levels below the highest are indented no further (ux review F7). */
  const DEEPEST = 4;

  /** M-16.5: 14 px per level below the note's highest heading, as egui, at most `DEEPEST` levels. */
  function indent(depth: number): number {
    return Math.min(depth, DEEPEST) * 14;
  }

  /**
   * M-16.5, UC-004 step 5: the place of an entry: the facade scrolls its
   * block to the top, the source view jumps there and takes the keyboard.
   */
  function show(note: string, offset: number) {
    const facade = activeFacade();
    if (facade?.note === note) {
      facade.reveal(offset);
      return;
    }
    const source = activeSourceView();
    if (source?.note === note) {
      source.jump(offset);
    }
  }

  onMount(() => shell.pinToolPanelWidth());

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
      // The edge is on the left: moving it left widens the panel.
      shell.toolPanelWidth = toolPanelWidthWithin(drag.width - (event.clientX - drag.x));
    }
  }

  function endDrag() {
    if (!drag) {
      return;
    }
    drag = null;
    const shown = panel?.getBoundingClientRect().width ?? width;
    void shell.toolPanelReleased(shown);
  }
</script>

<aside class="tool-panel" style:width="{width}px" aria-label="Tools" bind:this={panel}>
  <!-- Q-06: the edge is dragged with the mouse only, so it is no control
       for screen readers. -->
  <div
    class="edge"
    role="presentation"
    onpointerdown={startDrag}
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
  ></div>
  <div class="sections">
    <details open>
      <summary>Properties</summary>
      {#if properties}
        <dl class="properties">
          <dt>Place</dt>
          <dd>{properties.place}</dd>
          <dt>Format</dt>
          <dd>{properties.format}</dd>
          <dt>Size</dt>
          <dd>{properties.size}</dd>
          <dt>Last change</dt>
          <dd>{properties.lastChange}</dd>
        </dl>
      {:else}
        <p class="weak">Not saved yet</p>
      {/if}
    </details>
    {#if lists}
      {#each sections as section (section.title)}
        <details open>
          <summary>{section.title} ({section.entries.length})</summary>
          {#if section.entries.length === 0}
            <p class="weak">None</p>
          {:else}
            <ul class="entries" aria-label={section.title}>
              {#each section.entries as entry (`${entry.offset}:${entry.label}`)}
                <li>
                  <button
                    type="button"
                    class="entry"
                    style:padding-left="calc(var(--space-1) + {indent(entry.depth)}px)"
                    aria-label={section.title === 'Outline'
                      ? `${entry.label}, level ${entry.depth + 1}`
                      : undefined}
                    onclick={() => show(lists.note, entry.offset)}>{entry.label}</button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </details>
      {/each}
    {/if}
  </div>
</aside>

<style>
  .tool-panel {
    position: relative;
    flex: none;
    box-sizing: border-box;
    /* A-13: the panel leaves the note at least 200 px of the room beside
       the overview. */
    max-width: calc(100% - 200px);
    min-height: 0;
    border-left: 1px solid var(--ui-rule);
  }

  .sections {
    height: 100%;
    overflow: auto;
    padding: var(--space-2) var(--space-3);
    box-sizing: border-box;
  }

  summary {
    min-height: 24px;
    overflow: hidden;
    font-weight: 700;
    white-space: nowrap;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .properties {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--space-1) var(--space-3);
    margin: var(--space-2) 0 0;
  }

  dt,
  .weak {
    color: var(--ui-hint);
  }

  dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: break-word;
  }

  .weak {
    margin: var(--space-2) 0 0;
  }

  details + details {
    margin-top: var(--space-2);
  }

  .entries {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
  }

  /* egui's selectable labels: plain text, tinted under the pointer. A long
     label wraps, so it can be read without a pointer (ux review F7). */
  .entry {
    display: block;
    box-sizing: border-box;
    width: 100%;
    min-height: 24px;
    padding: 2px var(--space-1);
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    overflow-wrap: anywhere;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .entry:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    width: 6px;
    cursor: col-resize;
    touch-action: none;
  }
</style>
