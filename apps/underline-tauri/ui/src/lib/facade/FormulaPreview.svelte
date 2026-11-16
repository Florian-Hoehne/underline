<!--
  FR-098, UC-005 BR-008, OQ-287: the typeset preview of the inline formula
  the cursor is in, in a small popover above its markup (below it at the
  top of the facade). It shows the picture the formula worker typeset for
  the facade's views, debounced (`formulaPreview.ts`); while a changed
  formula is typeset the last picture stays, dimmed; with an error, the
  reason. It goes when the cursor leaves the formula.

  The popover is outside the editable text, never takes the keyboard or the
  pointer, and is hidden from screen readers: the markup is what is edited,
  and the reason of an error is announced by the text as before. Its
  picture is sized in em of the facade's text, so it follows the editor
  zoom; each pane's facade has its own.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { Platform } from '../bindings';
  import { assetUrl } from './backend';
  import {
    PREVIEW_DEBOUNCE_MS,
    placePreview,
    previewOf,
    type EditedFormula,
    type PreviewPlace,
    type PreviewShown,
  } from './formulaPreview';
  import { em } from './model';

  type Props = {
    /** The inline formula the cursor is in; `null` when it is in none. */
    edited: EditedFormula | null;
    /** The facade's editable text, holding the formula's markup. */
    content: HTMLElement | null;
    /** For the address of the picture. */
    platform: Platform;
    /** Changes whenever the markup may have moved (scrolling, layout, zoom). */
    moved: unknown;
  };

  let { edited, content, platform, moved }: Props = $props();

  let shown = $state.raw<PreviewShown | null>(null);
  let place = $state.raw<PreviewPlace | null>(null);
  let popup = $state<HTMLElement | null>(null);
  /** The formula shown: its block and run. */
  let shownKey: string | null = null;

  // OQ-287: the preview follows the formula once it stood still; a formula
  // just entered shows at once, and leaving it takes the preview away.
  $effect(() => {
    const now = edited;
    if (!now) {
      shown = null;
      shownKey = null;
      return;
    }
    const key = `${now.block}:${now.run}`;
    if (key !== shownKey) {
      shownKey = key;
      shown = previewOf(now.formula, null);
      return;
    }
    const timer = setTimeout(() => {
      shown = previewOf(
        now.formula,
        untrack(() => shown),
      );
    }, PREVIEW_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  // Where the popover stands: measured after the text is laid out.
  $effect(() => {
    void moved;
    const now = edited;
    const element = popup;
    if (!now || !element || !content || shown === null) {
      place = null;
      return;
    }
    const frame = requestAnimationFrame(() => {
      const markup = content?.querySelector(`[data-b="${now.block}"] [data-r="${now.run}"]`);
      const host = element.offsetParent;
      if (!markup || !host) {
        place = null;
        return;
      }
      const box = markup.getBoundingClientRect();
      const frameBox = host.getBoundingClientRect();
      // The markup scrolled out of view: no popover.
      if (box.bottom < frameBox.top || box.top > frameBox.bottom) {
        place = null;
        return;
      }
      place = placePreview(box, frameBox, {
        width: element.offsetWidth,
        height: element.offsetHeight,
      });
    });
    return () => cancelAnimationFrame(frame);
  });
</script>

{#if edited && shown}
  <div
    class="formula-preview"
    class:below={place?.below}
    aria-hidden="true"
    data-formula-preview=""
    bind:this={popup}
    style:left="{place?.left ?? 0}px"
    style:top="{place?.top ?? 0}px"
    style:visibility={place ? 'visible' : 'hidden'}
  >
    {#if shown.kind === 'picture'}
      <img
        src={assetUrl(shown.picture, platform)}
        alt=""
        draggable="false"
        class:stale={shown.stale}
        style:width={em(shown.width)}
        style:aspect-ratio="{shown.width} / {Math.max(shown.height, 1)}"
      />
    {:else if shown.kind === 'error'}
      <span class="reason">{shown.reason}</span>
    {:else}
      <span class="waiting">Typesetting the formula…</span>
    {/if}
  </div>
{/if}

<style>
  /* In the size of the facade's text, so the picture's em follow the zoom;
     on the code background with a rule, set apart from the text below it. */
  .formula-preview {
    position: absolute;
    z-index: 30;
    max-width: calc(100% - 16px);
    padding: 4px 8px;
    overflow: hidden;
    font-family: var(--font-ui);
    font-size: calc(15px * var(--editor-zoom, 1));
    line-height: 1.25;
    color: var(--palette-foreground);
    pointer-events: none;
    background: var(--palette-background);
    border: 1px solid color-mix(in srgb, var(--palette-foreground) 35%, transparent);
    border-radius: 4px;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.18);
  }

  .formula-preview img {
    display: block;
    max-width: 100%;
    height: auto;
    user-select: none;
  }

  /* A newer state of the formula is typeset: the last picture, weaker. */
  .formula-preview img.stale {
    opacity: 0.45;
  }

  /* UC-004 A21: the reason, as after the markup in the text. */
  .reason {
    display: block;
    padding-left: 6px;
    font-size: 13px;
    font-weight: 600;
    border-left: 3px solid var(--palette-foreground);
  }

  .waiting {
    font-size: 13px;
    color: color-mix(in srgb, var(--palette-foreground) 70%, transparent);
  }
</style>
