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

  Later features add the todos (M-16.3, F15), the git changes (M-16.4, F12)
  and the outline, code and figures (M-16.5, F7).
-->
<script lang="ts">
  import { onMount } from 'svelte';
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
