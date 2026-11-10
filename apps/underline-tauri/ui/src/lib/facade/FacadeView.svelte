<!--
  M-09 (FR-009, UC-004 step 3, UC-005): the facade of one note: the note
  formatted, block by block, from the backend's blocks (`fa_open`, then
  `ed-blocks`), and edited there (feature F5b). The backend owns the text
  (ADR-024): every input of the one `contenteditable` root is cancelled and
  sent as one command (`fa_input`); the answer carries the blocks that
  changed and the cursor, which the facade puts in place before the next
  frame.

  - Input (M-09.10 to M-09.13): `beforeinput` becomes text, Enter,
    Backspace, Delete, undo and redo; keys come from `keydown`
    (`keys.ts`). Up, Down, Home and End need the layout: the facade
    measures where the cursor would go (`Selection.modify`) and sends it
    along. An input method composes in the text; at its end the blocks are
    drawn anew and the finished text is sent (M-08.5). Spellcheck,
    autocorrect and autocapitalisation are off.
  - M-09.11: after Enter the cursor may stand on empty lines before a block
    (the backend's "gap"); they are drawn as such.
  - M-09.14: copy and cut give the markup, paste goes through the backend;
    Ctrl/Cmd+Shift+V pastes plain text. HTML comes with feature F6.
  - M-11.2, M-11.3: Vim and Nano are the editor core's (A-01: not
    `codemirror-vim`); a line is a block.
  - Virtualized (ADR-024): only the blocks near the part in view are in the
    DOM; the others stand as their heights, measured once drawn, estimated
    before (`heights.ts`). A block above the view that changes its height
    moves the scroll position by as much, so the text in view stays. A
    selection whose ends are not drawn is painted where blocks are.
  - M-09.3: "Markdown"/"AsciiDoc" at the top right.
  - M-07.4: pointing at a link shows its address and "Ctrl+click to follow"
    (following it comes with F7).
  - M-08.8: fold arrows and "…" on headings, from the sections the backend
    sends (`ed-sections`), shared with the source view.
  - M-09.8: while the cursor is in a figure, its markup shows below it and
    is edited there.
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
    ClipAction,
    EditorCommandName,
    EditorPatch,
    FacadeAt,
    FacadeInput,
    FacadePatch,
    FacadeState,
    GapView,
    KeysStatusView,
    Platform,
    RowsView,
    SectionsView,
    SectionView,
    ToolName,
  } from '../bindings';
  import type { EditorInfo } from '../editor/info.svelte';
  import type { InputModeName, KeysState } from '../editor/keys';
  import type { MinimapChange, MinimapSource } from '../editor/minimap';
  import { tabId } from '../tabs';
  import { setActiveFacade, type ActiveFacade } from './active';
  import { tauriFacadeBackend, type FacadeBackend } from './backend';
  import Block from './Block.svelte';
  import Enlarged from './Enlarged.svelte';
  import { Heights } from './heights';
  import { routeBeforeInput, routeFacadeKey } from './keys';
  import {
    applyBlocks,
    BLOCK_GAP,
    BODY_SIZE,
    blockAt,
    estimateHeight,
    facadeMarks,
    foldingOf,
    isFigure,
    LINE_HEIGHT,
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
    /** The input mode of the settings. */
    mode?: InputModeName;
    /** The keyboard goes into the facade once it is shown. */
    focus?: boolean;
    /** What the status line and the minimap show of the facade. */
    info?: EditorInfo;
    onError: (message: string) => void;
    /** M-10.9: the keys ask to store the note or close its tab. */
    onCommand?: (command: EditorCommandName) => void;
    backend?: FacadeBackend;
    /** ADR-024 measuring point: called when a patch is in place. */
    probe?: ((start: number, patched: number) => void) | null;
  };

  let {
    note,
    label,
    platform,
    mode = 'Normal',
    focus = false,
    info,
    onError,
    onCommand,
    backend = tauriFacadeBackend,
    probe = null,
  }: Props = $props();

  /** egui's `PADDING` around the blocks. */
  const PADDING = 12;
  /**
   * The space left of the blocks: egui's padding and 12 more, so the fold
   * arrows take the pointer over 24 × 24 px (A-44).
   */
  const LEFT = 24;
  /** How long a change of blocks may wait for the one before it. */
  const OUT_OF_STEP_MS = 150;

  let scroller = $state<HTMLElement | null>(null);
  let content = $state<HTMLElement | null>(null);
  let loading = $state(true);
  let blocks = $state.raw<BlockView[]>([]);
  let format = $state('');
  let sections = $state.raw<SectionView[]>([]);
  /** The cursor and the other end of the selection, in web units. */
  let head = $state(0);
  let anchor = $state(0);
  /** M-09.11: the empty lines the cursor stands on. */
  let gap = $state.raw<GapView | null>(null);
  /** FR-026: the status of the Vim or Nano keys. */
  let keys = $state.raw<KeysStatusView | null>(null);
  let scrollTop = $state(0);
  let viewHeight = $state(0);
  let width = $state(0);
  /** Counts the changes of the heights, which are not reactive themselves. */
  let layout = $state(0);
  /** Counts the times the drawn blocks are made anew (after a composition). */
  let drawn = $state(0);
  let enlarged = $state<{ url: string; width: number; height: number } | null>(null);
  let tooltip = $state<{ text: string; x: number; y: number } | null>(null);

  let rev = 0;
  let version = 0;
  let destroyed = false;
  const heights = new Heights();
  /** Changes of the blocks that came before the one they follow. */
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a buffer, not state
  const waiting = new Map<number, BlocksView>();
  let waitingTimer: ReturnType<typeof setTimeout> | null = null;
  /**
   * A place the view is kept at while the heights around it settle: a
   * block brought to the top by `reveal`, or the place shown when the
   * facade was last left.
   */
  let holding: { index: number; into: number; until: number } | null = null;
  /** The remembered place was restored; scrolling is remembered from now on. */
  let restored = false;
  /** The cursor the backend set (folding), placed once its blocks are in. */
  let pendingCaret: { rev: number; anchor: number; head: number } | null = null;
  /** The inputs and selections, sent one after the other. */
  let queue: Promise<unknown> = Promise.resolve();
  let selectionQueued = false;
  /** Counts the selections the user made, so a patch never undoes a newer one. */
  let selections = 0;
  /** The DOM selection the facade set itself, which is not the user's. */
  let placed: { anchor: number; head: number } | null = null;
  /** M-11.2: the Vim keys wait for the clipboard (`"+`). */
  let wantsClipboard = false;
  /** M-08.5: an input method composes; its text was taken already. */
  let composing = false;
  let committed = false;
  let lastWidth = 0;
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- the minimap's listeners, not state
  const listeners = new Set<(change: MinimapChange) => void>();

  const mac = $derived(platform === 'macos');
  const textWidth = $derived(Math.max(width - PADDING - LEFT, 80));
  const folding = $derived(foldingOf(blocks, sections));
  const keysState = $derived<KeysState>({
    mode,
    vimInsert: keys?.mode === 'INSERT',
    prompting: keys?.prompting ?? false,
  });
  /** M-09.8: the figure holding the cursor. */
  const editing = $derived.by(() => {
    if (gap) {
      return -1;
    }
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
  /** M-09.11: the gap is drawn before this block (the number of blocks: at the end). */
  const gapBefore = $derived(gap ? gap.before : -1);
  const gapLines = $derived(gap ? Array.from({ length: gap.lines }, (_, i) => i) : []);
  /** The gap stands after the last block drawn. */
  const gapAtEnd = $derived(
    gap !== null && (shown.length === 0 || gap.before > shown[shown.length - 1]),
  );

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
    waiting.clear();
    format = state.format;
    blocks = state.blocks;
    heights.reset(state.blocks.map((block) => estimateHeight(block, textWidth)));
    anchor = state.anchor;
    head = state.head;
    gap = state.gap;
    keys = state.keys;
    if (info) {
      info.place = state.place;
      info.keys = state.keys;
    }
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

  /** Applies `change`, which follows the blocks shown; the text in view stays. */
  function applyChange(change: BlocksView) {
    const next = applyBlocks({ rev, version, blocks }, change);
    if (!next) {
      return;
    }
    const before = anchorOf();
    // A block changed in place keeps its height until it is measured again.
    const kept = Math.min(change.removed, change.blocks.length);
    heights.splice(
      change.from,
      change.removed,
      change.blocks.map((block, k) =>
        k < kept && heights.measured(change.from + k)
          ? heights.get(change.from + k)
          : estimateHeight(block, textWidth),
      ),
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
  }

  /**
   * ADR-024: blocks changed, after an input of either view or from the
   * tick. A change that comes before the one it follows waits for it a
   * moment; a change of another version asks for all blocks again.
   */
  function takeBlocks(change: BlocksView) {
    if (change.note !== note || loading) {
      return;
    }
    if (change.base !== version) {
      if (change.base > version) {
        waiting.set(change.base, change);
        waitingTimer ??= setTimeout(() => {
          waitingTimer = null;
          if (waiting.size > 0 && !destroyed) {
            waiting.clear();
            void open();
          }
        }, OUT_OF_STEP_MS);
      } else {
        // Older than the blocks shown: taken in already.
        void 0;
      }
      if (change.base > version + 1000 || change.base < version - 1000) {
        void open();
      }
      return;
    }
    applyChange(change);
    for (let next = waiting.get(version); next; next = waiting.get(version)) {
      waiting.delete(next.base);
      applyChange(next);
    }
    if (waiting.size === 0 && waitingTimer !== null) {
      clearTimeout(waitingTimer);
      waitingTimer = null;
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

  // M-11.4: the status of the keys follows the input mode; the first comes
  // with the facade (`fa_open`).
  let modeSeen: string | null = null;
  $effect(() => {
    const now = mode;
    if (modeSeen === null || modeSeen === now) {
      modeSeen = now;
      return;
    }
    modeSeen = now;
    backend
      .keys(note)
      .then((status) => {
        if (destroyed) {
          return;
        }
        keys = status;
        if (info) info.keys = status;
      })
      .catch((error: unknown) => onError(message(error)));
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

  /**
   * The DOM place of `offset`; an end of a selection whose text is not
   * drawn stands at the start of the first or the end of the last block
   * drawn, so the drawn part of the selection is painted.
   */
  function drawnPoint(offset: number): { node: Node; offset: number } | null {
    const exact = domPoint(offset);
    if (exact || shown.length === 0) {
      return exact;
    }
    const first = blocks[shown[0]];
    if (first && offset < first.start) {
      return domPoint(first.start) ?? domPoint(sourceAt(first, first.runs[0] ?? emptyRun, 0));
    }
    const last = blocks[shown[shown.length - 1]];
    return last ? domPoint(last.start + last.length) : null;
  }

  const emptyRun = {
    text: '',
    bold: false,
    italic: false,
    strike: false,
    code: false,
    link: null,
    offsets: [],
    end: 0,
  };

  /** Puts the DOM selection at these markup offsets, where their text is drawn. */
  function placeSelection(from: number, to: number) {
    const selection = document.getSelection();
    if (!selection) {
      return;
    }
    if (gap && from === to) {
      // M-09.11: the cursor on the last of the empty lines.
      const lines = content?.querySelectorAll<HTMLElement>('[data-gap] .gap-line');
      const line = lines?.[lines.length - 1];
      if (line) {
        selection.collapse(line, 0);
        placed = { anchor: from, head: to };
      }
      return;
    }
    const start = from === to ? domPoint(from) : drawnPoint(from);
    const end = from === to ? start : drawnPoint(to);
    if (!start || !end) {
      return;
    }
    selection.setBaseAndExtent(start.node, start.offset, end.node, end.offset);
    const mapped = {
      anchor: offsetOf(start.node, start.offset) ?? from,
      head: offsetOf(end.node, end.offset) ?? to,
    };
    placed = mapped.anchor === from && mapped.head === to ? null : mapped;
  }

  /** The text revision and selection the facade shows, for an input. */
  function at(): FacadeAt {
    return { rev, anchor, head };
  }

  /** Runs `work` after every input before it; applies its patch. */
  function enqueue(start: number, work: () => Promise<FacadePatch | null>): void {
    queue = queue.then(async () => {
      if (destroyed) {
        return;
      }
      try {
        const seen = selections;
        const patch = await work();
        if (patch && !destroyed) {
          await applyPatch(patch, seen === selections);
          probe?.(start, performance.now());
        }
      } catch (error) {
        onError(message(error));
      }
    });
  }

  /** Sends `input`; `rows` when the cursor's rows are needed (M-09.13). */
  function send(input: FacadeInput, rows = false, start = performance.now()) {
    enqueue(start, async () => {
      const clipboard = wantsClipboard ? await backend.readClipboard() : null;
      const measured = rows ? measureRows() : null;
      return backend.input(note, at(), input, measured, clipboard);
    });
  }

  /** M-09.14: copy, cut or paste through the backend. */
  function clipboardAction(action: ClipAction, plain: boolean) {
    enqueue(performance.now(), () => backend.clipboard(note, at(), action, plain));
  }

  /** Sends the selection the user made, after the inputs before it. */
  function sendSelection() {
    if (selectionQueued) {
      return;
    }
    selectionQueued = true;
    queue = queue.then(async () => {
      selectionQueued = false;
      if (destroyed) {
        return;
      }
      try {
        const place = await backend.select(note, rev, anchor, head);
        if (info && !destroyed) info.place = place;
      } catch (error) {
        onError(message(error));
      }
    });
  }

  /**
   * ADR-024: takes the answer to an input: the changed blocks, then the
   * cursor, placed before the next frame. `own` is false when the user
   * moved the selection meanwhile; it stays then.
   */
  async function applyPatch(patch: FacadePatch, own: boolean) {
    for (const change of patch.blocks) {
      takeBlocks(change);
    }
    if (patch.blocks.length === 0) {
      relayout();
    }
    if (info) {
      info.place = patch.place;
      info.keys = patch.keys;
      if (patch.request) info.request = patch.request;
    }
    keys = patch.keys;
    wantsClipboard = patch.wantsClipboard;
    if (patch.copied !== null) {
      backend.writeClipboard(patch.copied).catch((error: unknown) => onError(message(error)));
    }
    for (const command of patch.commands) {
      onCommand?.(command);
    }
    if (!own) {
      return;
    }
    anchor = patch.anchor;
    head = patch.head;
    gap = patch.gap;
    if (patch.scroll) {
      bringIntoView();
    }
    await tick();
    if (destroyed) {
      return;
    }
    placeSelection(anchor, head);
    if (patch.scroll) {
      keepCaretInView();
    }
  }

  /** Scrolls the cursor's block into the DOM when it is far from the view. */
  function bringIntoView() {
    if (!scroller) {
      return;
    }
    const index = gap ? Math.min(gap.before, blocks.length - 1) : blockAt(blocks, head);
    if (index < 0 || shown.includes(index)) {
      return;
    }
    scroller.scrollTop = Math.max(PADDING + heights.top(index) - scroller.clientHeight / 3, 0);
    scrollTop = scroller.scrollTop;
  }

  /** Scrolls the least so the caret is in view. */
  function keepCaretInView() {
    const selection = document.getSelection();
    if (!scroller || !selection || selection.rangeCount === 0) {
      return;
    }
    const range = selection.getRangeAt(0).cloneRange();
    range.collapse(false);
    const rect =
      range.getClientRects?.()[0] ?? range.startContainer.parentElement?.getBoundingClientRect?.();
    if (!rect || (rect.top === 0 && rect.bottom === 0)) {
      return;
    }
    const view = scroller.getBoundingClientRect();
    if (rect.top < view.top) {
      scroller.scrollTop -= view.top - rect.top + 4;
    } else if (rect.bottom > view.bottom) {
      scroller.scrollTop += rect.bottom - view.bottom + 4;
    }
  }

  /** The DOM position the caret moves to by `modify`, or null outside `blockElement`. */
  function moved(
    selection: Selection,
    start: { node: Node; offset: number },
    blockElement: Element,
    direction: 'backward' | 'forward',
    unit: 'line' | 'lineboundary',
  ): { offset: number; top: number } | null {
    selection.collapse(start.node, start.offset);
    selection.modify('move', direction, unit);
    const node = selection.focusNode;
    if (!node || !blockElement.contains(node)) {
      return null;
    }
    const offset = offsetOf(node, selection.focusOffset);
    if (offset === null) {
      return null;
    }
    const range = document.createRange();
    range.setStart(node, selection.focusOffset);
    const rect = range.getClientRects?.()[0];
    return { offset, top: rect?.top ?? Number.NaN };
  }

  /**
   * M-09.13: where the cursor goes one row up or down in its block, and
   * the ends of its row, as the webview lays the block out.
   */
  function measureRows(): RowsView | null {
    const selection = document.getSelection();
    if (gap || !selection || typeof selection.modify !== 'function') {
      return null;
    }
    const start = domPoint(head);
    const blockElement =
      start &&
      (start.node.nodeType === Node.ELEMENT_NODE
        ? (start.node as Element)
        : start.node.parentElement
      )?.closest('[data-b]');
    if (!start || !blockElement) {
      return null;
    }
    const saved =
      selection.rangeCount > 0
        ? {
            anchorNode: selection.anchorNode,
            anchorOffset: selection.anchorOffset,
            focusNode: selection.focusNode,
            focusOffset: selection.focusOffset,
          }
        : null;
    const here = document.createRange();
    here.setStart(start.node, start.offset);
    const top = here.getClientRects?.()[0]?.top ?? Number.NaN;
    const rowOf = (place: { offset: number; top: number } | null) =>
      place && !(Math.abs(place.top - top) < 1) ? place.offset : null;
    const up = rowOf(moved(selection, start, blockElement, 'backward', 'line'));
    const down = rowOf(moved(selection, start, blockElement, 'forward', 'line'));
    const rowStart = moved(selection, start, blockElement, 'backward', 'lineboundary');
    const rowEnd = moved(selection, start, blockElement, 'forward', 'lineboundary');
    if (saved?.anchorNode && saved.focusNode) {
      selection.setBaseAndExtent(
        saved.anchorNode,
        saved.anchorOffset,
        saved.focusNode,
        saved.focusOffset,
      );
    }
    return {
      at: head,
      up,
      down,
      start: rowStart?.offset ?? head,
      end: rowEnd?.offset ?? head,
    };
  }

  function onSelectionChange() {
    const selection = document.getSelection();
    if (!selection || selection.rangeCount === 0 || !content || composing) {
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
    if (from === null || to === null) {
      return;
    }
    if (placed && placed.anchor === from && placed.head === to) {
      // The facade put it there itself (a part of a longer selection).
      return;
    }
    placed = null;
    if (from === anchor && to === head) {
      return;
    }
    selections++;
    anchor = from;
    head = to;
    gap = null;
    sendSelection();
  }

  /** M-09.8: a click on a figure puts the cursor at the start of its markup. */
  async function onFigure(index: number) {
    const block = blocks[index];
    const first = block?.runs[0];
    if (!first) {
      return;
    }
    const start = sourceAt(block, first, 0);
    selections++;
    anchor = start;
    head = start;
    gap = null;
    sendSelection();
    content?.focus({ preventScroll: true });
    await tick();
    placeSelection(start, start);
  }

  /** Scrolls the cursor's block into view and puts the keyboard there. */
  async function focusCursor() {
    if (!scroller || !content) {
      return;
    }
    const index = gap ? Math.min(gap.before, blocks.length - 1) : blockAt(blocks, head);
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

  /** Escape: the keyboard goes to the note's tab (M-08.10, NFR-011). */
  function leave() {
    document.getElementById(tabId(note))?.focus();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.defaultPrevented) {
      return;
    }
    const start = performance.now();
    const route = routeFacadeKey(event, mac, keysState);
    if (route === null) {
      return;
    }
    event.preventDefault();
    switch (route) {
      case 'swallow':
        return;
      case 'leave':
        leave();
        return;
      case 'enlarge':
        // M-09.9, NFR-011: Ctrl/Cmd+Enter enlarges the picture the cursor is in (A-45).
        enlargeAtCursor();
        return;
      case 'pastePlain':
        clipboardAction('paste', true);
        return;
      default:
        send(route.send, route.rows, start);
    }
  }

  /**
   * ADR-021: every input of the webview is cancelled and goes to the
   * backend; an input method composes in the text until it is done.
   */
  function onBeforeInput(event: InputEvent) {
    const start = performance.now();
    if (event.inputType === 'insertFromComposition') {
      event.preventDefault();
      commitComposition(event.data ?? '', start);
      return;
    }
    const route = routeBeforeInput(event.inputType, event.data, mac);
    if (route === 'compose' || composing) {
      return;
    }
    event.preventDefault();
    if (route === null) {
      return;
    }
    if (route === 'paste') {
      clipboardAction('paste', false);
      return;
    }
    // M-11.2: Vim's j and k move by rows the window measures.
    const vimRows =
      keysState.mode === 'Vim' &&
      !keysState.vimInsert &&
      route.send.kind === 'text' &&
      /^[jk]$/.test(route.send.text);
    send(route.send, vimRows, start);
  }

  function onCompositionStart() {
    composing = true;
    committed = false;
  }

  /** M-08.5: the composed text is drawn anew from the blocks and sent. */
  function commitComposition(text: string, start = performance.now()) {
    composing = false;
    if (committed) {
      return;
    }
    committed = true;
    // The input method changed the DOM of a block; it is drawn anew.
    drawn++;
    if (text) {
      send({ kind: 'text', text }, false, start);
    } else {
      void tick().then(() => placeSelection(anchor, head));
    }
  }

  function onCompositionEnd(event: CompositionEvent) {
    commitComposition(event.data ?? '');
  }

  function onClipboard(event: ClipboardEvent, action: ClipAction) {
    event.preventDefault();
    clipboardAction(action, false);
  }

  /** M-08.3: no word or line selection by double or triple click. */
  function onMouseDown(event: MouseEvent) {
    if (event.button === 0 && event.detail >= 2) {
      event.preventDefault();
    }
  }

  /** Drops and drags are refused; dropping files comes with feature F6. */
  function refuse(event: Event) {
    event.preventDefault();
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
    element.setAttribute('autocorrect', 'off');
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

  let paintFrame: number | null = null;

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
    // A selection reaching beyond the drawn blocks is painted on those drawn now.
    if (anchor !== head && paintFrame === null && document.activeElement === content) {
      paintFrame = requestAnimationFrame(() => {
        paintFrame = null;
        if (!destroyed) placeSelection(anchor, head);
      });
    }
  }

  /**
   * The backend moved the cursor (folding): the status line and the caret
   * follow, the caret once the blocks of its revision are in.
   */
  function takePatch(patch: EditorPatch) {
    if (destroyed) {
      return;
    }
    if (info) info.place = patch.place;
    anchor = patch.anchor;
    head = patch.head;
    gap = null;
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
    queue = queue.then(() =>
      backend
        .fold(note, line)
        .then(takePatch)
        .catch((error: unknown) => onError(message(error))),
    );
  }

  /** M-10.2: a button of the toolbar, at the facade's selection. */
  function tool(name: ToolName) {
    send({ kind: 'tool', tool: name });
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

  const controller: ActiveFacade & {
    tool(name: ToolName): void;
    settled(): Promise<unknown>;
  } = {
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
    settled: () => queue,
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
      if (paintFrame !== null) {
        cancelAnimationFrame(paintFrame);
      }
      if (waitingTimer !== null) {
        clearTimeout(waitingTimer);
      }
      setActiveFacade(null);
      if (info?.facade === controller) {
        info.facade = null;
        info.place = '';
        info.keys = null;
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
      autocapitalize="off"
      role="textbox"
      tabindex="0"
      aria-multiline="true"
      aria-label={label}
      {@attach describe}
      style:padding="{PADDING}px {PADDING}px {PADDING}px {LEFT}px"
      onbeforeinput={onBeforeInput}
      oncompositionstart={onCompositionStart}
      oncompositionend={onCompositionEnd}
      oncopy={(event) => onClipboard(event, 'copy')}
      oncut={(event) => onClipboard(event, 'cut')}
      onpaste={(event) => onClipboard(event, 'paste')}
      ondrop={refuse}
      ondragstart={refuse}
      onkeydown={onKeyDown}
      onmousedown={onMouseDown}
      onpointermove={onPointerMove}
      onpointerleave={() => (tooltip = null)}
    >
      <div class="space" style:height="{spaceAbove}px" contenteditable="false"></div>
      {#key drawn}
        {#each shown as index (index)}
          {#if index === gapBefore}
            {@render emptyLines()}
          {/if}
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
        {#if gapAtEnd}
          {@render emptyLines()}
        {/if}
      {/key}
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

<!-- M-09.11: the empty lines of a new paragraph, the cursor on the last. -->
{#snippet emptyLines()}
  <div
    class="gap"
    data-gap
    style:padding-bottom="{BLOCK_GAP}px"
    style:--row="{BODY_SIZE * LINE_HEIGHT}px"
  >
    {#each gapLines as line (line)}<div class="gap-line"><br /></div>{/each}
  </div>
{/snippet}

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

  .gap-line {
    height: var(--row);
    font-size: 15px;
    line-height: 1.25;
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
