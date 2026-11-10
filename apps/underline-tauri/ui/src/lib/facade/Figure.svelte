<!--
  M-09.6, M-09.7 (UC-004 step 3, BR-004, A11): the picture of an image or a
  diagram block, at its own size and never wider than the facade, with
  "Figure n. {caption}" below it. The picture comes only through the app's
  own protocol (ADR-023).

  While an image file loads it says "Loading {target}…"; while a changed
  diagram renders, or its new picture loads, the last picture stays. A
  picture the window cannot read says so, naming the image. Without a
  picture, the backend's message stands in its place: a failure in the text
  colour with a rule (as Q-05/Q-10), a coming picture in the weak colour.

  M-09.9: the magnifier at the picture's top right while the pointer is on
  it, or a double click, enlarges it (Ctrl/Cmd+Enter from the keyboard,
  `FacadeView.svelte`).

  The picture is laid out at its own size, taken when it loaded: a Mermaid
  SVG says only `width="100%"` and a view box, which in a box that shrinks
  to its content comes out 0 × 0.
-->
<script lang="ts">
  import type { FigureView, Platform } from '../bindings';
  import { assetUrl } from './backend';

  type Props = {
    figure: FigureView;
    platform: Platform;
    /** The picture's text for screen readers: the alt text or the caption. */
    alt: string;
    /** M-09.9: the picture is to be shown enlarged, at this size on screen. */
    onEnlarge: (url: string, width: number, height: number) => void;
  };

  let { figure, platform, alt, onEnlarge }: Props = $props();

  /** The picture shown, and one that is loading to replace it. */
  let shown = $state<string | null>(null);
  let failed = $state(false);
  let image = $state<HTMLImageElement | null>(null);
  /** The size of the picture shown, in CSS pixels, once known. */
  let size = $state<{ width: number; height: number } | null>(null);

  const wanted = $derived(figure.picture ? assetUrl(figure.picture, platform) : null);

  // The last picture stays until the next one is loaded (M-09.7).
  $effect(() => {
    const url = wanted;
    if (url === null) {
      shown = null;
      failed = false;
      return;
    }
    if (url === shown) {
      return;
    }
    let gone = false;
    const next = new Image();
    next.onload = () => {
      if (!gone) {
        size =
          next.naturalWidth > 0 && next.naturalHeight > 0
            ? { width: next.naturalWidth, height: next.naturalHeight }
            : null;
        shown = url;
        failed = false;
      }
    };
    next.onerror = () => {
      if (!gone) {
        shown = null;
        failed = true;
      }
    };
    next.src = url;
    return () => {
      gone = true;
    };
  });

  const message = $derived.by(() => {
    if (wanted === null) {
      return figure.message;
    }
    if (failed) {
      return figure.unreadable ?? figure.message;
    }
    return shown === null ? (figure.loading ?? figure.message) : null;
  });
  const isFailure = $derived(wanted === null ? figure.failed : failed);

  function enlarge() {
    if (shown && image) {
      onEnlarge(shown, image.clientWidth, image.clientHeight);
    }
  }
</script>

{#if shown}
  <div class="picture">
    <img
      bind:this={image}
      src={shown}
      {alt}
      draggable="false"
      style:width={size ? `${size.width}px` : null}
      style:aspect-ratio={size ? `${size.width} / ${size.height}` : null}
      ondblclick={enlarge}
    />
    <button
      type="button"
      class="magnifier"
      title="Enlarge (double click)"
      aria-label="Enlarge picture"
      onclick={(event) => {
        // The cursor stays where it is (egui's magnifier does not move it).
        event.stopPropagation();
        enlarge();
      }}
    >
      <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
        <circle cx="10" cy="10" r="6" />
        <line x1="14.5" y1="14.5" x2="19" y2="19" />
      </svg>
    </button>
  </div>
{/if}
{#if message}
  <p class="message" class:failure={isFailure}>{message}</p>
{/if}
{#if figure.caption}
  <p class="caption">{figure.caption}</p>
{/if}

<style>
  .picture {
    position: relative;
    display: inline-block;
    max-width: 100%;
    vertical-align: top;
  }

  img {
    display: block;
    max-width: 100%;
    height: auto;
    cursor: zoom-in;
  }

  /* egui: black at 140 of 255, 200 on hover, a white glass. */
  .magnifier {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: rgb(0 0 0 / 55%);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    opacity: 0;
  }

  .picture:hover .magnifier,
  .magnifier:focus-visible {
    opacity: 1;
  }

  .magnifier:hover {
    background: rgb(0 0 0 / 78%);
  }

  .magnifier:focus-visible {
    outline: 2px solid var(--palette-link);
    outline-offset: 1px;
  }

  .magnifier svg {
    fill: none;
    stroke: #fff;
    stroke-width: 2;
  }

  .magnifier line {
    stroke-width: 2.5;
  }

  .message {
    margin: 0;
    font-size: 13px;
    color: var(--facade-weak);
    overflow-wrap: anywhere;
  }

  .message.failure {
    padding-left: 8px;
    font-weight: 600;
    color: var(--palette-foreground);
    border-left: 3px solid var(--palette-foreground);
  }

  .caption {
    margin: 6px 0 0;
    font-size: 14px;
  }
</style>
