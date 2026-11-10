<!--
  M-09.9 (FR-039, UC-004 BR-011): a picture over the whole window, as
  egui's `enlarged.rs` shows it: zoomed with the wheel, a pinch, `+`, `=`
  and `-` around the pointer, moved by dragging, fitted again with `0`,
  closed with Escape, a click beside it or its close button. The zoom
  shows in the hint at the bottom. A diagram is SVG, so it stays sharp at
  every zoom without being rendered again; the picture is laid out at its
  zoomed size, not scaled as pixels.

  It is a modal dialog: it takes the keyboard while open and gives it back
  when it closes.
-->
<script lang="ts">
  import { onMount } from 'svelte';

  type Props = {
    /** The picture's address (`ul-asset`). */
    url: string;
    /** Its size as the facade shows it, for a picture that does not say its own. */
    width: number;
    height: number;
    onClose: () => void;
  };

  let { url, width, height, onClose }: Props = $props();

  const MIN_ZOOM = 0.1;
  const MAX_ZOOM = 16;

  let overlay = $state<HTMLElement | null>(null);
  let area = $state({ width: 0, height: 0 });
  /** The picture's own size, once loaded. */
  let natural = $state<{ width: number; height: number } | null>(null);
  /** Points of the screen per point of the picture; `null` fits it. */
  let zoom = $state<number | null>(null);
  /** How far the picture is moved from the middle of the window. */
  let offset = $state({ x: 0, y: 0 });
  let pointer: { x: number; y: number } | null = null;
  let drag: { x: number; y: number; moved: boolean } | null = null;

  const size = $derived(natural ?? { width: Math.max(width, 1), height: Math.max(height, 1) });
  const fit = $derived(
    Math.min(
      Math.max(
        Math.min((area.width - 64) / size.width, (area.height - 64) / size.height),
        MIN_ZOOM,
      ),
      1,
    ),
  );
  const shownZoom = $derived(zoom ?? fit);
  const rect = $derived({
    width: size.width * shownZoom,
    height: size.height * shownZoom,
    left: area.width / 2 + offset.x - (size.width * shownZoom) / 2,
    top: area.height / 2 + offset.y - (size.height * shownZoom) / 2,
  });

  /** egui's hint, with its spaces. */
  const hint = $derived(
    `${Math.round(shownZoom * 100)} %   ·   wheel or +/− to zoom, drag to move, 0 to fit, Esc to close`,
  );

  /** Zooms by `factor`, keeping the point under the pointer in its place. */
  function zoomBy(factor: number) {
    if (Math.abs(factor - 1) < 1e-6) {
      return;
    }
    const current = shownZoom;
    const at = pointer ?? { x: area.width / 2, y: area.height / 2 };
    const pivot = {
      x: at.x - area.width / 2 - offset.x,
      y: at.y - area.height / 2 - offset.y,
    };
    const next = Math.min(Math.max(current * factor, MIN_ZOOM), MAX_ZOOM);
    offset = {
      x: offset.x - pivot.x * (next / current - 1),
      y: offset.y - pivot.y * (next / current - 1),
    };
    zoom = next;
  }

  function local(event: MouseEvent): { x: number; y: number } {
    const box = overlay?.getBoundingClientRect();
    return { x: event.clientX - (box?.left ?? 0), y: event.clientY - (box?.top ?? 0) };
  }

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    pointer = local(event);
    const lines = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? area.height : 1;
    // egui: scrolling up zooms in; a pinch comes as a wheel with Ctrl.
    zoomBy(Math.exp((-event.deltaY * lines) / 300));
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    } else if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      zoomBy(1.25);
    } else if (event.key === '-') {
      event.preventDefault();
      zoomBy(1 / 1.25);
    } else if (event.key === '0') {
      event.preventDefault();
      zoom = null;
      offset = { x: 0, y: 0 };
    } else if (event.key === 'Tab') {
      // The close button is the only stop; the keyboard stays in the dialog.
      event.preventDefault();
      overlay?.querySelector<HTMLElement>('.close')?.focus();
    }
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0 || (event.target as Element).closest('.close')) {
      return;
    }
    drag = { x: event.clientX, y: event.clientY, moved: false };
    overlay?.setPointerCapture?.(event.pointerId);
  }

  function onPointerMove(event: PointerEvent) {
    pointer = local(event);
    if (!drag) {
      return;
    }
    const dx = event.clientX - drag.x;
    const dy = event.clientY - drag.y;
    if (drag.moved || Math.abs(dx) + Math.abs(dy) > 3) {
      drag.moved = true;
      offset = { x: offset.x + dx, y: offset.y + dy };
      drag.x = event.clientX;
      drag.y = event.clientY;
    }
  }

  function onPointerUp(event: PointerEvent) {
    const was = drag;
    drag = null;
    overlay?.releasePointerCapture?.(event.pointerId);
    if (!was || was.moved) {
      return;
    }
    // A click beside the picture closes the view.
    const at = local(event);
    const inside =
      at.x >= rect.left &&
      at.x <= rect.left + rect.width &&
      at.y >= rect.top &&
      at.y <= rect.top + rect.height;
    if (!inside) {
      onClose();
    }
  }

  /** WebKit's pinch on a touchpad. */
  type GestureEvent = Event & { scale: number; clientX: number; clientY: number };
  let gestureScale = 1;

  onMount(() => {
    const before = document.activeElement as HTMLElement | null;
    overlay?.focus();
    const start = (event: Event) => {
      event.preventDefault();
      gestureScale = (event as GestureEvent).scale || 1;
    };
    const change = (event: Event) => {
      event.preventDefault();
      const scale = (event as GestureEvent).scale || 1;
      zoomBy(scale / gestureScale);
      gestureScale = scale;
    };
    overlay?.addEventListener('gesturestart', start);
    overlay?.addEventListener('gesturechange', change);
    return () => {
      overlay?.removeEventListener('gesturestart', start);
      overlay?.removeEventListener('gesturechange', change);
      before?.focus?.();
    };
  });
</script>

<div
  class="enlarged"
  role="dialog"
  aria-modal="true"
  aria-label="Enlarged picture"
  tabindex="-1"
  bind:this={overlay}
  bind:clientWidth={area.width}
  bind:clientHeight={area.height}
  onwheel={onWheel}
  onkeydown={onKey}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={() => (drag = null)}
>
  <img
    src={url}
    alt=""
    draggable="false"
    style:left="{rect.left}px"
    style:top="{rect.top}px"
    style:width="{rect.width}px"
    style:height="{rect.height}px"
    onload={(event) => {
      const image = event.currentTarget as HTMLImageElement;
      if (image.naturalWidth > 0 && image.naturalHeight > 0) {
        natural = { width: image.naturalWidth, height: image.naturalHeight };
      }
    }}
  />
  <button type="button" class="close" aria-label="Close" onclick={onClose}>
    <svg viewBox="-9 -9 18 18" width="18" height="18" aria-hidden="true">
      <line x1="-9" y1="-9" x2="9" y2="9" />
      <line x1="-9" y1="9" x2="9" y2="-9" />
    </svg>
  </button>
  <p class="hint">{hint}</p>
</div>

<style>
  /* egui: black at 235 of 255 over the whole window. */
  .enlarged {
    position: fixed;
    inset: 0;
    z-index: 50;
    overflow: hidden;
    background: rgb(0 0 0 / 92%);
    outline: none;
    cursor: grab;
    touch-action: none;
  }

  .enlarged:active {
    cursor: grabbing;
  }

  /* A transparent picture stands on the facade's background. */
  img {
    position: absolute;
    max-width: none;
    background: var(--palette-background);
    user-select: none;
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    padding: 0;
    background: rgb(0 0 0 / 67%);
    border: none;
    border-radius: 50%;
    cursor: pointer;
  }

  .close svg {
    stroke: #bebebe;
    stroke-width: 2;
  }

  .close:hover svg {
    stroke: #fff;
  }

  .close:focus-visible {
    outline: 2px solid #fff;
    outline-offset: 2px;
  }

  .hint {
    position: absolute;
    bottom: 16px;
    left: 50%;
    width: min(500px, calc(100% - 32px));
    margin: 0;
    padding: 4px 0;
    font-size: 12px;
    color: #bebebe;
    text-align: center;
    white-space: pre;
    background: rgb(0 0 0 / 67%);
    border-radius: 6px;
    transform: translateX(-50%);
    pointer-events: none;
  }
</style>
