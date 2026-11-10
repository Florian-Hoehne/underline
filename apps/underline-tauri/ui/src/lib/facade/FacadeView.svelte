<!--
  M-09 (FR-009, UC-004 step 3): the facade of one note: the note formatted,
  block by block, from the backend's blocks (`fa_open`, then `ed-blocks`).
  Feature F5a shows it; typing in it comes with F5b, so every input that
  would change the text is refused for now, but the cursor and the
  selection can be set with the mouse and the arrows and are the note's
  (`ed_select`), as in the source view.

  - Virtualized (ADR-024): only the blocks near the part in view are in the
    DOM; the others stand as their heights, measured once drawn, estimated
    before (`heights.ts`). A block above the view that changes its height
    moves the scroll position by as much, so the text in view stays.
  - M-09.3: "Markdown"/"AsciiDoc" at the top right.
  - M-07.4: pointing at a link shows its address and "Ctrl+click to follow"
    (following it comes with F7).
  - M-08.8: fold arrows and "…" on headings, from the sections the backend
    sends (`ed-sections`), shared with the source view.
  - M-09.8: while the cursor is in a figure, its markup shows below it.
  - M-09.9: a picture enlarged over the window.
  - M-09.16: `reveal(offset)` scrolls a block to the top without moving the
    cursor.
  - M-12.2: the minimap gets one bar per row and figures as areas.
-->
<script module lang="ts">
  /**
   * A-25: where each note's facade was scrolled to, while the window lives:
   * the block at the top and how far into it, so the place comes back
   * however the heights were guessed.
   */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- read once when the facade opens
  const scrolled = new Map<string, { index: number; into: number }>();
</script>

<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type {
    BlockView,
    BlocksView,
    EditorPatch,
    FacadeState,
    Platform,
    SectionsView,
    SectionView,
    ToolName,
  } from '../bindings';
  import type { EditorInfo } from '../editor/info.svelte';
  import type { MinimapChange, MinimapSource } from '../editor/minimap';
  import { tabId } from '../tabs';
  import { setActiveFacade, type ActiveFacade } from './active';
  import { tauriFacadeBackend, type FacadeBackend } from './backend';
  import Block from './Block.svelte';
  import Enlarged from './Enlarged.svelte';
  import { Heights } from './heights';
  import {
    applyBlocks,
    blockAt,
    estimateHeight,
    facadeMarks,
    foldingOf,
    isFigure,
    pointOf,
    revealIndex,
    sourceAt,
  } from './model';

  type Props = {
    /** The note of the tab. */
    note: string;
    /** Its title, the name of the text. */
    label: string;
    platform: Platform;
    /** The keyboard goes into the facade once it is shown. */
    focus?: boolean;
    /** What the status line and the minimap show of the facade. */
    info?: EditorInfo;
    onError: (message: string) => void;
    backend?: FacadeBackend;
  };

  let {
    note,
    label,
    platform,
    focus = false,
    info,
    onError,
    backend = tauriFacadeBackend,
  }: Props = $props();

  /** egui's `PADDING` around the blocks. */
  const PADDING = 12;
  /**
   * The space left of the blocks: egui's padding and 12 more, so the fold
   * arrows take the pointer over 24 × 24 px (A-44).
   */
  const LEFT = 24;

  let scroller = $state<HTMLElement | null>(null);
  let content = $state<HTMLElement | null>(null);
  let loading = $state(true);
  let blocks = $state.raw<BlockView[]>([]);
  let format = $state('');
  let sections = $state.raw<SectionView[]>([]);
  /** The cursor and the other end of the selection, in web units. */
  let head = $state(0);
  let anchor = $state(0);
  let scrollTop = $state(0);
  let viewHeight = $state(0);
  let width = $state(0);
  /** Counts the changes of the heights, which are not reactive themselves. */
  let layout = $state(0);
  let enlarged = $state<{ url: string; width: number; height: number } | null>(null);
  let tooltip = $state<{ text: string; x: number; y: number } | null>(null);

  let rev = 0;
  let version = 0;
  let destroyed = false;
  const heights = new Heights();
  /**
   * A place the view is kept at while the heights around it settle: a
   * block brought to the top by `reveal`, or the place shown when the
   * facade was last left.
   */
  let holding: { index: number; into: number; until: number } | null = null;
  /** The remembered place was restored; scrolling is remembered from now on. */
  let restored = false;
  /** The cursor the backend set (toolbar), placed once its blocks are in. */
  let pendingCaret: { rev: number; anchor: number; head: number } | null = null;
  let selecting: Promise<unknown> | null = null;
  let selectionDirty = false;
  let lastWidth = 0;
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the minimap's listeners, not state
  const listeners = new Set<(change: MinimapChange) => void>();

  const textWidth = $derived(Math.max(width - PADDING - LEFT, 80));
  const folding = $derived(foldingOf(blocks, sections));
  /** M-09.8: the figure holding the cursor. */
  const editing = $derived.by(() => {
    const index = blockAt(blocks, head);
    if (index < 0) {
      return -1;
    }
    const block = blocks[index];
    return isFigure(block) && head <= block.start + block.length ? index : -1;
  });
  /** The blocks in the DOM: those near the part in view, without the hidden ones. */
  const shown = $derived.by(() => {
    void layout;
    const top = scrollTop - PADDING;
    // Before the first measurement, the window's height stands in.
    const height = viewHeight || window.innerHeight;
    const overscan = Math.max(height, 400);
    const first = heights.indexAt(top - overscan);
    const last = Math.min(blocks.length, heights.indexAt(top + height + overscan) + 1);
    const indices: number[] = [];
    for (let i = first; i < last; i++) {
      if (!folding.hidden.has(i)) {
        indices.push(i);
      }
    }
    return indices;
  });
  const spaceAbove = $derived.by(() => {
    void layout;
    return shown.length > 0 ? heights.top(shown[0]) : 0;
  });
  const spaceBelow = $derived.by(() => {
    void layout;
    if (shown.length === 0) {
      return heights.total();
    }
    const last = shown[shown.length - 1];
    return Math.max(heights.total() - heights.top(last + 1), 0);
  });

  function estimate(index: number): number {
    return folding.hidden.has(index) ? 0 : estimateHeight(blocks[index], textWidth);
  }

  function notify(change: MinimapChange) {
    for (const listener of listeners) {
      listener(change);
    }
  }

  function relayout() {
    layout++;
    notify('content');
  }

  let relayoutFrame: number | null = null;

  /**
   * Re-renders in the next frame: within the resize observer's callback a
   * new window of blocks would be observed in the same frame, which the
   * webview reports as a resize loop.
   */
  function relayoutSoon() {
    if (relayoutFrame === null) {
      relayoutFrame = requestAnimationFrame(() => {
        relayoutFrame = null;
        if (!destroyed) {
          relayout();
        }
      });
    }
  }

  function message(error: unknown): string {
    return typeof error === 'object' && error !== null && 'message' in error
      ? String(error.message)
      : String(error);
  }

  /** Takes the whole facade from the backend. */
  function take(state: FacadeState) {
    rev = state.rev;
    version = state.version;
    format = state.format;
    blocks = state.blocks;
    heights.reset(state.blocks.map((block) => estimateHeight(block, textWidth)));
    anchor = state.anchor;
    head = state.head;
    if (info) info.place = state.place;
    loading = false;
    relayout();
  }

  async function open(): Promise<void> {
    try {
      const state = await backend.open(note);
      if (!destroyed) {
        take(state);
      }
    } catch (error) {
      loading = false;
      onError(message(error));
    }
  }

  /** The first block in view and how far into it the view starts. */
  function anchorOf(): { index: number; into: number } {
    const top = (scroller?.scrollTop ?? 0) - PADDING;
    const index = heights.indexAt(top);
    return { index, into: top - heights.top(index) };
  }

  /** ADR-024: blocks changed; the text in view stays where it is. */
  function takeBlocks(change: BlocksView) {
    if (change.note !== note || loading) {
      return;
    }
    const next = applyBlocks({ rev, version, blocks }, change);
    if (!next) {
      void open();
      return;
    }
    const before = anchorOf();
    heights.splice(
      change.from,
      change.removed,
      change.blocks.map((block) => estimateHeight(block, textWidth)),
    );
    rev = next.rev;
    version = next.version;
    blocks = next.blocks;
    let index = before.index;
    if (index >= change.from + change.removed) {
      index += change.blocks.length - change.removed;
    } else if (index >= change.from) {
      index = change.from;
    }
    if (scroller && before.index !== index) {
      scroller.scrollTop = PADDING + heights.top(index) + before.into;
    }
    relayout();
    if (pendingCaret && pendingCaret.rev === rev) {
      const caret = pendingCaret;
      pendingCaret = null;
      void tick().then(() => placeSelection(caret.anchor, caret.head));
    }
  }

  function takeSections(view: SectionsView) {
    if (view.note === note) {
      sections = view.sections;
    }
  }

  /** Scrolls to `into` pixels into block `index`, and keeps it there while heights settle. */
  function holdAt(index: number, into: number, time = 500) {
    if (!scroller) {
      return;
    }
    holding = { index, into, until: performance.now() + time };
    scroller.scrollTop = PADDING + heights.top(index) + into;
  }

  /** M-09.16: block `offset` to the top, the cursor where it is. */
  function reveal(offset: number) {
    if (blocks.length === 0) {
      return;
    }
    holdAt(revealIndex(blocks, offset), -PADDING);
  }

  // The hidden blocks stand as nothing; shown again, they are estimated.
  $effect(() => {
    const { hidden } = folding;
    let changed = false;
    for (let i = 0; i < blocks.length; i++) {
      if (hidden.has(i)) {
        changed = heights.set(i, 0, false) !== 0 || changed;
      } else if (heights.get(i) === 0) {
        changed = heights.set(i, estimateHeight(blocks[i], textWidth), false) !== 0 || changed;
      }
    }
    if (changed) {
      relayout();
    }
  });

  // Another width wraps the text anew: every height is a guess again, and
  // the block in view stays at the top.
  $effect(() => {
    const now = textWidth;
    if (lastWidth === 0 || Math.abs(now - lastWidth) < 1 || blocks.length === 0) {
      lastWidth = now;
      return;
    }
    lastWidth = now;
    const before = anchorOf();
    heights.reestimate(estimate, true);
    if (scroller) {
      scroller.scrollTop = PADDING + heights.top(before.index) + before.into;
    }
    relayout();
  });

  /** Measures the drawn blocks; a block above the view keeps the view still. */
  const resizes =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver((entries) => {
          let changed = false;
          for (const entry of entries) {
            const element = entry.target as HTMLElement;
            const index = Number(element.dataset.slot);
            if (!(index < blocks.length) || !element.isConnected) {
              continue;
            }
            const height = element.offsetHeight;
            const end = heights.top(index) + heights.get(index);
            const delta = heights.set(index, height);
            if (delta === 0) {
              continue;
            }
            changed = true;
            if (scroller && end <= scroller.scrollTop - PADDING) {
              scroller.scrollTop += delta;
            }
          }
          if (holding && scroller) {
            if (performance.now() < holding.until) {
              scroller.scrollTop = PADDING + heights.top(holding.index) + holding.into;
            } else {
              holding = null;
            }
          }
          if (changed) {
            relayoutSoon();
          }
        });

  function measure(element: HTMLElement) {
    resizes?.observe(element);
    return () => resizes?.unobserve(element);
  }

  /** The markup offset of a place in the DOM, `null` outside the blocks' text. */
  function offsetOf(node: Node, offset: number): number | null {
    const element = node.nodeType === Node.ELEMENT_NODE ? (node as Element) : node.parentElement;
    const blockElement = element?.closest<HTMLElement>('[data-b]');
    if (!blockElement || !content?.contains(blockElement)) {
      return null;
    }
    const block = blocks[Number(blockElement.dataset.b)];
    if (!block) {
      return null;
    }
    const runElement = element?.closest<HTMLElement>('[data-r]');
    if (!runElement || !blockElement.contains(runElement)) {
      const first = block.runs[0];
      return first ? sourceAt(block, first, 0) : block.start;
    }
    const run = block.runs[Number(runElement.dataset.r)];
    if (!run) {
      return null;
    }
    const range = document.createRange();
    range.setStart(runElement, 0);
    range.setEnd(node, offset);
    return sourceAt(block, run, range.toString().length);
  }

  /** The DOM place of the markup offset `offset`, if its text is drawn. */
  function domPoint(offset: number): { node: Node; offset: number } | null {
    const point = pointOf(blocks, offset);
    const runElement = point
      ? content?.querySelector(`[data-b="${point.block}"] [data-r="${point.run}"]`)
      : null;
    if (!point || !runElement) {
      return null;
    }
    const walker = document.createTreeWalker(runElement, NodeFilter.SHOW_TEXT);
    let left = point.unit;
    let last: Text | null = null;
    for (
      let node = walker.nextNode() as Text | null;
      node;
      node = walker.nextNode() as Text | null
    ) {
      if (left <= node.length) {
        return { node, offset: left };
      }
      left -= node.length;
      last = node;
    }
    return last ? { node: last, offset: last.length } : { node: runElement, offset: 0 };
  }

  /** Puts the DOM selection at these markup offsets, where their text is drawn. */
  function placeSelection(from: number, to: number) {
    const start = domPoint(from);
    const end = domPoint(to);
    const selection = document.getSelection();
    if (!start || !end || !selection) {
      return;
    }
    selection.setBaseAndExtent(start.node, start.offset, end.node, end.offset);
  }

  /** Sends the selection, the latest only, one call at a time. */
  function sendSelection() {
    selectionDirty = true;
    if (selecting) {
      return;
    }
    selectionDirty = false;
    selecting = backend
      .select(note, rev, anchor, head)
      .then((place) => {
        if (info && !destroyed) info.place = place;
      })
      .catch((error: unknown) => onError(message(error)))
      .finally(() => {
        selecting = null;
        if (selectionDirty && !destroyed) {
          sendSelection();
        }
      });
  }

  function onSelectionChange() {
    const selection = document.getSelection();
    if (!selection || selection.rangeCount === 0 || !content) {
      return;
    }
    const { anchorNode, focusNode } = selection;
    if (
      !anchorNode ||
      !focusNode ||
      !content.contains(anchorNode) ||
      !content.contains(focusNode)
    ) {
      return;
    }
    const from = offsetOf(anchorNode, selection.anchorOffset);
    const to = offsetOf(focusNode, selection.focusOffset);
    if (from === null || to === null || (from === anchor && to === head)) {
      return;
    }
    anchor = from;
    head = to;
    sendSelection();
  }

  /** M-09.8: a click on a figure puts the cursor at the start of its markup. */
  async function onFigure(index: number) {
    const block = blocks[index];
    const first = block?.runs[0];
    if (!first) {
      return;
    }
    const at = sourceAt(block, first, 0);
    anchor = at;
    head = at;
    sendSelection();
    content?.focus({ preventScroll: true });
    await tick();
    placeSelection(at, at);
  }

  /** Scrolls the cursor's block into view and puts the keyboard there. */
  async function focusCursor() {
    if (!scroller || !content) {
      return;
    }
    const index = blockAt(blocks, head);
    if (index >= 0) {
      const top = PADDING + heights.top(index);
      const bottom = top + heights.get(index);
      if (top < scroller.scrollTop || bottom > scroller.scrollTop + scroller.clientHeight) {
        scroller.scrollTop = Math.max(top - scroller.clientHeight / 3, 0);
      }
    }
    content.focus({ preventScroll: true });
    await tick();
    placeSelection(anchor, head);
  }

  /** Before F5b, nothing changes the text from the facade but the toolbar. */
  function refuse(event: Event) {
    event.preventDefault();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      // NFR-011: the facade has no keys of its own before F5b; Escape
      // leaves it for the note's tab, as in the source view.
      event.preventDefault();
      document.getElementById(tabId(note))?.focus();
      return;
    }
    if (event.key === 'Tab') {
      // M-09.13: Tab does nothing in the facade.
      event.preventDefault();
      return;
    }
    const command = platform === 'macos' ? event.metaKey : event.ctrlKey;
    if (command && !event.shiftKey && !event.altKey && event.key === 'Enter') {
      // M-09.9, NFR-011: Ctrl/Cmd+Enter enlarges the picture the cursor is in (A-45).
      event.preventDefault();
      enlargeAtCursor();
      return;
    }
    if (
      command &&
      event.shiftKey &&
      (event.code === 'BracketLeft' || event.code === 'BracketRight')
    ) {
      // M-08.8: Ctrl/Cmd+Shift+[ folds, Ctrl/Cmd+Shift+] expands.
      event.preventDefault();
      const key = event.code === 'BracketLeft' ? 'BracketLeft' : 'BracketRight';
      backend
        .input(note, {
          kind: 'key',
          key,
          ctrl: event.ctrlKey,
          alt: event.altKey,
          shift: event.shiftKey,
          meta: event.metaKey,
          page: 1,
        })
        .then(takePatch)
        .catch((error: unknown) => onError(message(error)));
    }
  }

  /** M-07.4: the address of the link under the pointer. */
  function onPointerMove(event: PointerEvent) {
    const link = (event.target as Element | null)?.closest<HTMLElement>('[data-link]');
    if (!link?.dataset.link) {
      tooltip = null;
      return;
    }
    const key = platform === 'macos' ? 'Cmd' : 'Ctrl';
    tooltip = {
      text: `${link.dataset.link}\n${key}+click to follow`,
      x: event.clientX,
      y: event.clientY,
    };
  }

  /**
   * As the source view: Escape leaves the text (NFR-011). Set as an
   * attribute, as Svelte's types do not know `aria-description` yet.
   */
  function describe(element: HTMLElement) {
    element.setAttribute('aria-description', 'Escape leaves the text');
  }

  /** M-09.9: the picture of the figure holding the cursor, enlarged. */
  function enlargeAtCursor() {
    const image =
      editing >= 0
        ? content?.querySelector<HTMLImageElement>(`[data-b="${editing}"] .picture img`)
        : null;
    const url = image?.getAttribute('src');
    if (image && url) {
      enlarged = { url, width: image.clientWidth, height: image.clientHeight };
    }
  }

  function onScroll() {
    if (!scroller || destroyed) {
      return;
    }
    scrollTop = scroller.scrollTop;
    if (restored && !loading) {
      scrolled.set(note, anchorOf());
    }
    tooltip = null;
    notify('scroll');
  }

  /**
   * The backend moved the cursor (toolbar, folding): the status line and
   * the caret follow, the caret once the blocks of its revision are in.
   */
  function takePatch(patch: EditorPatch) {
    if (destroyed) {
      return;
    }
    if (info) info.place = patch.place;
    anchor = patch.anchor;
    head = patch.head;
    if (patch.rev === rev) {
      pendingCaret = null;
      // The folded blocks leave the DOM first.
      void tick().then(() => placeSelection(patch.anchor, patch.head));
    } else {
      pendingCaret = { rev: patch.rev, anchor: patch.anchor, head: patch.head };
    }
  }

  /** M-08.8: the arrow of a heading folds or expands its section. */
  function fold(line: number) {
    backend
      .fold(note, line)
      .then(takePatch)
      .catch((error: unknown) => onError(message(error)));
  }

  /** M-10.2: a button of the toolbar, at the facade's selection. */
  function tool(name: ToolName) {
    backend
      .input(note, { kind: 'tool', tool: name })
      .then(takePatch)
      .catch((error: unknown) => onError(message(error)));
    content?.focus({ preventScroll: true });
  }

  /** M-12: what the minimap shows of the facade. */
  const minimap: MinimapSource = {
    geometry: () => {
      const { marks, areas } = facadeMarks(
        blocks,
        folding.hidden,
        (i) => heights.top(i),
        (i) => heights.get(i),
        textWidth,
        PADDING,
      );
      return { marks, areas, content: minimap.contentHeight(), visible: minimap.visible() };
    },
    hits: () => [],
    visible: () => {
      const top = scroller?.scrollTop ?? 0;
      return [top, top + (scroller?.clientHeight ?? 0)];
    },
    contentHeight: () => scroller?.scrollHeight ?? 0,
    scrollTo: (offset) => {
      if (scroller) scroller.scrollTop = offset;
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };

  const controller: ActiveFacade & { tool(name: ToolName): void } = {
    get note() {
      return note;
    },
    takeBlocks,
    takeSections,
    // UC-004 A7: a text replaced from outside comes as `ed-blocks`.
    replaced: () => {},
    reveal,
    focus: () => void focusCursor(),
    tool,
  };

  onMount(() => {
    setActiveFacade(controller);
    if (info) {
      info.facade = controller;
      info.minimap = minimap;
      info.keys = null;
    }
    document.addEventListener('selectionchange', onSelectionChange);
    void open().then(async () => {
      if (destroyed || !scroller) {
        return;
      }
      await tick();
      if (destroyed || !scroller) {
        return;
      }
      const place = scrolled.get(note);
      if (place && place.index < blocks.length) {
        holdAt(place.index, place.into, 800);
      }
      scrollTop = scroller.scrollTop;
      restored = true;
      if (focus) {
        await focusCursor();
      }
    });
    return () => {
      destroyed = true;
      document.removeEventListener('selectionchange', onSelectionChange);
      resizes?.disconnect();
      if (relayoutFrame !== null) {
        cancelAnimationFrame(relayoutFrame);
      }
      setActiveFacade(null);
      if (info?.facade === controller) {
        info.facade = null;
        info.place = '';
      }
      if (info?.minimap === minimap) {
        info.minimap = null;
      }
    };
  });
</script>

<div class="facade" aria-busy={loading} bind:clientWidth={width}>
  <div class="scroller" bind:this={scroller} bind:clientHeight={viewHeight} onscroll={onScroll}>
    <div
      class="facade-text"
      bind:this={content}
      contenteditable="true"
      spellcheck="false"
      role="textbox"
      tabindex="0"
      aria-multiline="true"
      aria-readonly="true"
      aria-label={label}
      {@attach describe}
      style:padding="{PADDING}px {PADDING}px {PADDING}px {LEFT}px"
      onbeforeinput={refuse}
      onpaste={refuse}
      oncut={refuse}
      ondrop={refuse}
      ondragstart={refuse}
      onkeydown={onKeyDown}
      onpointermove={onPointerMove}
      onpointerleave={() => (tooltip = null)}
    >
      <div class="space" style:height="{spaceAbove}px" contenteditable="false"></div>
      {#each shown as index (index)}
        <div class="slot" data-slot={index} {@attach measure}>
          <Block
            block={blocks[index]}
            {index}
            {platform}
            editing={editing === index}
            section={folding.headings.get(index) ?? null}
            onFold={fold}
            onEnlarge={(url, w, h) => (enlarged = { url, width: w, height: h })}
            {onFigure}
          />
        </div>
      {/each}
      <div class="space" style:height="{spaceBelow}px" contenteditable="false"></div>
    </div>
  </div>
  {#if format}
    <span class="indicator">{format}</span>
  {/if}
  {#if tooltip}
    <div
      class="tooltip"
      role="tooltip"
      style:left="{tooltip.x + 12}px"
      style:top="{tooltip.y + 16}px"
    >
      {tooltip.text}
    </div>
  {/if}
  {#if enlarged}
    <Enlarged
      url={enlarged.url}
      width={enlarged.width}
      height={enlarged.height}
      onClose={() => {
        enlarged = null;
      }}
    />
  {/if}
</div>

<style>
  .facade {
    --facade-weak: color-mix(in srgb, var(--palette-foreground) 55%, transparent);

    position: relative;
    box-sizing: border-box;
    height: 100%;
    min-height: 0;
    color: var(--palette-foreground);
    background: var(--palette-background, var(--ui-field));
    border: 1px solid transparent;
  }

  /* As the source view: a thin line around the text while it has the
     keyboard, besides the cursor (ux review F3, NFR-010). */
  .facade:has(.facade-text:focus) {
    border-color: var(--ui-focus);
  }

  .scroller {
    height: 100%;
    overflow-y: auto;
    overflow-anchor: none;
  }

  .facade-text {
    min-height: 100%;
    box-sizing: border-box;
    font-family: var(--font-ui);
    font-size: 15px;
    caret-color: var(--palette-foreground);
    outline: none;
  }

  .facade-text ::selection {
    background: var(--palette-selection);
  }

  .space {
    user-select: none;
  }

  /* M-09.3: egui writes it at the top right in 11 px, weak. */
  /* On the facade's background, so text running under it stays apart. */
  .indicator {
    position: absolute;
    top: 4px;
    right: 6px;
    padding: 0 4px;
    background: var(--palette-background);
    font-size: 11px;
    color: var(--facade-weak);
    pointer-events: none;
  }

  .tooltip {
    position: fixed;
    z-index: 40;
    max-width: 480px;
    padding: 4px 8px;
    font-size: 13px;
    color: var(--ui-text);
    white-space: pre-line;
    overflow-wrap: anywhere;
    pointer-events: none;
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }
</style>
