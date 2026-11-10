<!--
  M-12 (FR-030, UC-004 BR-013): the minimap beside the source view, painted
  on a canvas from what `editor/minimap.ts` works out. The bars are built
  anew at most every 150 ms while the text changes; scrolling only moves the
  tinted part. A click or a drag puts the place under the pointer in the
  middle of the view. With search hits it is a sieve that blinks (M-12.4).

  Like the egui strip it is for the pointer only; the text scrolls by
  keyboard as usual. Screen readers skip it.
-->
<script lang="ts">
  import type { PaletteView } from './bindings';
  import { foldedLines, headingLines } from './editor/folding';
  import {
    REBUILD_EVERY,
    VEIL,
    WIDTH,
    barRect,
    barsOf,
    geometryOf,
    hitRects,
    hitsOf,
    hitsShown,
    scaleOf,
    scrollFor,
    visibleOf,
    visibleRect,
    FOUND_OFF,
    FOUND_ON,
    type Geometry,
    type Hit,
  } from './editor/minimap';
  import type { SourceView } from './editor/sourceView';

  type Props = {
    /** The source view the strip shows. */
    view: SourceView | null;
    /** The theme's colours; a new palette paints the strip anew. */
    palette: PaletteView | null;
    /** UC-008 BR-008: the hits blink (setting "Search hits in the minimap"). */
    blink: boolean;
  };

  let { view, palette, blink }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let height = $state(0);

  let geometry: Geometry | null = null;
  let hits: Hit[] = [];
  let builtAt = -Infinity;
  let rebuild: ReturnType<typeof setTimeout> | null = null;
  let frame: number | null = null;
  let blinkTimer: ReturnType<typeof setTimeout> | null = null;
  let dragging = false;

  /** The bars and hits are worked out anew, at most every 150 ms. */
  function contentChanged() {
    if (rebuild !== null) {
      return;
    }
    const wait = Math.max(0, builtAt + REBUILD_EVERY - performance.now());
    rebuild = setTimeout(() => {
      rebuild = null;
      build();
    }, wait);
  }

  function build() {
    if (!view) {
      return;
    }
    builtAt = performance.now();
    geometry = geometryOf(view.view, headingLines(view.view), foldedLines(view.view));
    hits = hitsOf(view.view, view.hits());
    paint();
  }

  /** Paints in the next frame. */
  function paint() {
    if (frame !== null) {
      return;
    }
    frame = requestAnimationFrame(() => {
      frame = null;
      draw();
    });
  }

  function cssColor(name: string, fallback: string): string {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return value || fallback;
  }

  function draw() {
    const context = canvas?.getContext('2d');
    if (!canvas || !context || !view || !geometry) {
      return;
    }
    const ratio = window.devicePixelRatio || 1;
    const rows = Math.max(1, Math.ceil(height));
    if (canvas.width !== Math.round(WIDTH * ratio) || canvas.height !== Math.round(rows * ratio)) {
      canvas.width = Math.round(WIDTH * ratio);
      canvas.height = Math.round(rows * ratio);
    }
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, WIDTH, rows);
    const foreground = cssColor('--palette-foreground', '#000');
    const selection = cssColor('--palette-selection', '#88f');
    const found = cssColor('--palette-found', '#fc0');
    const visible = visibleOf(view.view);
    const scale = scaleOf(rows, geometry.content);

    // The bars: the text at 40 %, headings in the full text colour.
    for (const [row, bar] of barsOf(geometry.marks, scale, rows).entries()) {
      if (!bar) {
        continue;
      }
      const rect = barRect(row, bar, WIDTH);
      context.globalAlpha = bar.strong ? 1 : 0.4;
      context.fillStyle = foreground;
      context.fillRect(rect.x, rect.y, rect.width, rect.height);
    }
    context.globalAlpha = 1;
    // M-12.4: the veil of the sieve, under the part in view.
    if (hits.length > 0) {
      context.fillStyle = VEIL;
      context.fillRect(0, 0, WIDTH, rows);
    }
    // M-12.3: the part in view, tinted 35 % with a line around it.
    const tint = visibleRect(visible[0], visible[1], scale, WIDTH);
    context.fillStyle = selection;
    context.globalAlpha = 0.35;
    roundRect(context, tint.x, tint.y, tint.width, tint.height, 2);
    context.fill();
    context.globalAlpha = 1;
    context.strokeStyle = selection;
    context.lineWidth = 1;
    roundRect(context, tint.x + 0.5, tint.y + 0.5, tint.width - 1, tint.height - 1, 2);
    context.stroke();
    // M-12.4: the hits with their glow, blinking.
    if (hits.length > 0 && hitsShown(blink, performance.now())) {
      context.fillStyle = found;
      for (const { core, glow } of hitRects(hits, scale, WIDTH)) {
        context.globalAlpha = 0.3;
        roundRect(context, glow.x, glow.y, glow.width, glow.height, 3);
        context.fill();
        context.globalAlpha = 1;
        roundRect(context, core.x, core.y, core.width, core.height, 1);
        context.fill();
      }
    }
    context.globalAlpha = 1;
    scheduleBlink();
  }

  /** Paints again when the hits turn on or off. */
  function scheduleBlink() {
    if (blinkTimer !== null) {
      clearTimeout(blinkTimer);
      blinkTimer = null;
    }
    if (!blink || hits.length === 0) {
      return;
    }
    const period = FOUND_ON + FOUND_OFF;
    const phase = performance.now() % period;
    const next = (phase < FOUND_ON ? FOUND_ON : period) - phase;
    blinkTimer = setTimeout(
      () => {
        blinkTimer = null;
        paint();
      },
      Math.max(next, 10),
    );
  }

  function roundRect(
    context: CanvasRenderingContext2D,
    x: number,
    y: number,
    width: number,
    h: number,
    radius: number,
  ) {
    context.beginPath();
    if (typeof context.roundRect === 'function') {
      context.roundRect(x, y, width, h, radius);
    } else {
      context.rect(x, y, width, h);
    }
  }

  /** M-12.3: the place under the pointer comes to the middle of the view. */
  function scrollTo(event: PointerEvent) {
    if (!view || !geometry || !canvas) {
      return;
    }
    const top = canvas.getBoundingClientRect().top;
    const scale = scaleOf(Math.max(1, Math.ceil(height)), geometry.content);
    const offset = scrollFor(
      event.clientY - top,
      scale,
      visibleOf(view.view),
      view.view.scrollDOM.scrollHeight,
    );
    view.view.scrollDOM.scrollTop = offset;
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    dragging = true;
    canvas?.setPointerCapture?.(event.pointerId);
    scrollTo(event);
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging) {
      scrollTo(event);
    }
  }

  function onPointerUp(event: PointerEvent) {
    dragging = false;
    canvas?.releasePointerCapture?.(event.pointerId);
  }

  $effect(() => {
    const shown = view;
    if (!shown) {
      geometry = null;
      return;
    }
    build();
    return shown.subscribe((change) => {
      if (change === 'scroll') {
        paint();
      } else {
        contentChanged();
      }
    });
  });

  // A new theme, a new height or blinking switched: paint anew.
  $effect(() => {
    void palette;
    void height;
    void blink;
    paint();
  });

  $effect(() => {
    return () => {
      if (rebuild !== null) clearTimeout(rebuild);
      if (frame !== null) cancelAnimationFrame(frame);
      if (blinkTimer !== null) clearTimeout(blinkTimer);
    };
  });
</script>

<div class="minimap" bind:clientHeight={height}>
  <canvas
    bind:this={canvas}
    aria-hidden="true"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  ></canvas>
</div>

<style>
  .minimap {
    flex: 0 0 auto;
    width: 80px;
    min-height: 0;
    /* egui: the editor's background at 90 %. */
    background: color-mix(in srgb, var(--palette-background) 90%, transparent);
  }

  canvas {
    display: block;
    width: 80px;
    height: 100%;
    cursor: pointer;
    touch-action: none;
  }
</style>
