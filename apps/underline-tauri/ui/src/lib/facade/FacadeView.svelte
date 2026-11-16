<!--
  M-09 (FR-009, UC-004 step 3, UC-005): the facade of one note: the note
  formatted, block by block, and edited there (feature F5b). The text lives
  in the window's editor core (ADR-025, `wasmBackend.ts`): every input of
  the one `contenteditable` root is cancelled and given to the core in the
  same task; the answer carries the blocks that changed and the cursor,
  which the facade puts in place before the next frame. The core's later
  changes (the backend's, the colours and figures of `ed-enrich`) come
  through `attach`.

  - Input (M-09.10 to M-09.13): `beforeinput` becomes text, Enter,
    Backspace, Delete, undo and redo; keys come from `keydown`
    (`keys.ts`). Up, Down, Home and End need the layout: the facade
    measures where the cursor would go (`Selection.modify`) and sends it
    along. An input method composes in the text; at its end the blocks are
    drawn anew and the finished text is sent (M-08.5). Spellcheck,
    autocorrect and autocapitalisation are off.
  - M-09.11: after Enter the cursor may stand on empty lines before a block
    (the core's "gap"); they are drawn as such.
  - M-09.14: copy and cut give the markup and its HTML, paste goes through
    the core: HTML of other programs as markup, Ctrl/Cmd+Shift+V plain
    text, an image alone stored next to the note (M-10.6). A finished
    selection is copied at once while copy on select is on (M-10.7).
  - M-10.3 to M-10.5: the link and image dialogs' answers and dropped
    files go in at the cursor (`insert`); a drop first puts the cursor
    under the pointer (`placeAt`).
  - M-11.2, M-11.3: Vim and Nano are the editor core's (A-01: not
    `codemirror-vim`); a line is a block.
  - Virtualized (ADR-024): only the blocks near the part in view are in the
    DOM; the others stand as their heights, measured once drawn, estimated
    before (`heights.ts`). A block above the view that changes its height
    moves the scroll position by as much, so the text in view stays. A
    selection whose ends are not drawn is painted where blocks are.
  - M-09.3: "Markdown"/"AsciiDoc" at the top right.
  - M-07.4: pointing at a link shows its address and "Ctrl+click to follow";
    M-07.3: a click with Ctrl (Cmd on macOS) follows it (`onFollow`), the
    click placing the cursor too.
  - M-08.8: fold arrows and "…" on headings, from the sections the core
    finds, shared with the source view.
  - M-09.8: while the cursor is in a figure, its markup shows below it and
    is edited there.
  - M-09.9: a picture enlarged over the window.
  - M-09.16: `reveal(offset)` scrolls a block to the top without moving the
    cursor (the tool panel's entries, M-16.5); `jump(offset)` puts the
    cursor there, shows it and takes the keyboard (a heading a link leads
    to, M-07.3).
  - M-12.2: the minimap gets one bar per row and figures as areas.
  - FR-080, UC-005 A23, A24: after typed text the core is asked whether
    `[[` and the place of a link are typed at the cursor (Ctrl+Space asks
    at the cursor without them); the list (`linkCompletion.svelte.ts`)
    takes its keys first while it shows, and the core writes the link
    chosen in place of `[[` and what follows it. A click elsewhere or
    leaving the text closes it.
  - M-17.6: the agent cursors: a block holding lines an agent read or wrote
    is tinted blue with a 3 px bar left of it, and the first such block of
    each agent names it, in 80 % of the size (fixed colours, as egui).
  - Package 4 (FR-081 to FR-083, FR-086): close pairs, moving and
    duplicating blocks and indenting list items are the core's; the
    palette's commands come as `edit`, and going to a line asks the core
    for the patch (`goToLine`, UC-004 BR-017).
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
  import type { Refusal } from '../editor/refusal';
  import { onMount, tick } from 'svelte';
  import { DIALOG_OPENED } from '../DialogWindow.svelte';
  import type {
    AttributeChangeView,
    AgentMarkView,
    BlockView,
    BlocksView,
    PaneName,
    ClipAction,
    EditorCommandName,
    EditorPatch,
    FacadeAt,
    FacadeInput,
    EditActionName,
    FacadePatch,
    FacadeState,
    FindAnswer,
    FindSpec,
    FindStep,
    GapView,
    Insertion,
    KeysStatusView,
    LineTargetView,
    LinkContextView,
    Platform,
    RowsView,
    SectionView,
    SpellMarksView,
    ToolName,
  } from '../bindings';
  import SpellMenu, { CHANGED_SINCE } from '../SpellMenu.svelte';
  import { correctionOf, type ShownWord } from '../editor/spellWord';
  import { onPointerReleased } from '../editor/clipboard';
  import type { EditorInfo } from '../editor/info.svelte';
  import type { InputModeName, KeysState } from '../editor/keys';
  import type { MinimapChange, MinimapSource } from '../editor/minimap';
  import { tabId } from '../tabs';
  import { runJump, takeJump } from '../editor/jump';
  import { coreInStep } from '../editor/sync';
  import { linkCompletion, type CompletionTarget } from '../editor/linkCompletion.svelte';
  import { setActiveFacade, type ActiveFacade, type CursorMark, type ReplaceDone } from './active';
  import { type FacadeBackend } from './backend';
  import { wasmFacadeBackend } from './wasmBackend';
  import Block from './Block.svelte';
  import Enlarged from './Enlarged.svelte';
  import FormulaPreview from './FormulaPreview.svelte';
  import { editedFormula } from './formulaPreview';
  import { Heights } from './heights';
  import { routeBeforeInput, routeFacadeKey } from './keys';
  import {
    BLOCK_GAP,
    BODY_SIZE,
    blockAt,
    blockHits,
    estimateHeight,
    facadeHits,
    facadeMarks,
    mapHits,
    type HitRange,
    foldingOf,
    isFigure,
    LINE_HEIGHT,
    pointOf,
    revealIndex,
    shownWordIn,
    sourceAt,
    spellAt,
    spliceBlocks,
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
    /** M-07.3, UC-004 BR-006: a Ctrl+click (Cmd+click) on a link follows the link at `at` (markup, web units). */
    onFollow?: (at: number, side: boolean) => void;
    backend?: FacadeBackend;
    /** ADR-024 measuring point: called when a patch is in place. */
    probe?: ((start: number, patched: number) => void) | null;
    /** M-17.6: the agent cursors in this note. */
    agentMarks?: AgentMarkView[];
    /** FR-099: the last marks of the spell check in this note. */
    spellMarks?: SpellMarksView | null;
    /** UC-005 A28: a correction was made ("Corrected to ferry"). */
    onCorrected?: (suggestion: string) => void;
    /** UC-010 BR-009: the editor zoom in percent; figures keep their size. */
    zoom?: number;
    /**
     * UC-010 BR-011, OQ-211: tables, code and markup wrap; paragraphs, lists
     * and headings always do.
     */
    softWrap?: boolean;
    /** UC-004 BR-018: the pane of the split view the facade is in. */
    pane?: PaneName;
    /**
     * OQ-287: the facade is in the active pane; only there the formula the
     * cursor is in shows its preview.
     */
    active?: boolean;
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
    onFollow,
    pane = 'left',
    backend = wasmFacadeBackend(platform === 'macos', pane),
    probe = null,
    agentMarks = [],
    spellMarks = null,
    onCorrected,
    zoom = 100,
    softWrap = true,
    active = true,
  }: Props = $props();

  /** UC-010 BR-009: the text's size as a share of its normal size. */
  const scale = $derived(zoom / 100);

  /**
   * M-17.6 (UC-019 BR-006): the agent marking each block that holds lines
   * it read or wrote (the first mark wins, as in egui), and whether the
   * block names it: only the first block of each agent does.
   */
  const agentBlocks = $derived.by(() => {
    // Made anew from the props each time, never changed in place.
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const marked = new Map<number, { name: string; named: boolean }>();
    if (agentMarks.length === 0) {
      return marked;
    }
    // eslint-disable-next-line svelte/prefer-svelte-reactivity
    const named = new Set<string>();
    blocks.forEach((block, index) => {
      const first = block.line;
      const last = block.line + Math.max(block.lines, 1);
      const mark = agentMarks.find((m) => m.from < last && first < Math.max(m.to, m.from + 1));
      if (mark) {
        marked.set(index, { name: mark.name, named: !named.has(mark.name) });
        named.add(mark.name);
      }
    });
    return marked;
  });

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
  /** M-13.2: the marked hits, sorted, in web units of the markup. */
  let found = $state.raw<HitRange[]>([]);
  /**
   * FR-099, UC-005 BR-023: the words the spell check marks, sorted, in web
   * units of the markup; they follow the edits until the next marks come.
   */
  let spell = $state.raw<HitRange[]>([]);
  /** FR-099: marks of a revision the core has not reached yet. */
  let pendingSpell: SpellMarksView | null = null;
  /** UC-005 A28: the spell menu open now: the mark, the word shown, where. */
  let spellMenu = $state<{
    from: number;
    to: number;
    shown: ShownWord;
    x: number;
    y: number;
    /** The revision of the text the word was marked in. */
    rev: number;
  } | null>(null);
  /** The cursor and the other end of the selection, in web units. */
  let head = $state(0);
  let anchor = $state(0);
  /** A6: the hit that is the selection, shown also while the keyboard is elsewhere. */
  const current = $derived<HitRange | null>(
    anchor === head ? null : [Math.min(anchor, head), Math.max(anchor, head)],
  );
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
  /**
   * M-07.4: the hint stands above the pointer, so it does not cover the
   * line below the link (app-tester F7); near the top of the window, below.
   */
  const TOOLTIP_ABOVE = 80;

  let rev = 0;
  let version = 0;
  let destroyed = false;
  /** Resolves once the facade's blocks are in (`open`). */
  let opened: Promise<void> = Promise.resolve();
  /** ADR-025: lets the core's listener go. */
  let detach: (() => void) | null = null;
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
  /**
   * OQ-268: the user went into the text (a key or a click); until then the
   * front matter keeps its table, though the cursor starts in it.
   */
  let touched = $state(false);
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
    if (block.kind.type === 'table' && block.kind.table.frontMatter && !touched) {
      return -1;
    }
    return isFigure(block) && head <= block.start + block.length ? index : -1;
  });
  /** FR-098: the block holding the cursor, for the inline formulas it shows as markup. */
  const cursorBlock = $derived(gap ? -1 : blockAt(blocks, head));
  /** OQ-287: the inline formula the cursor is in, previewed typeset above its markup. */
  const previewed = $derived(
    !active || cursorBlock < 0 ? null : editedFormula(blocks[cursorBlock], cursorBlock, head),
  );
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
    return folding.hidden.has(index)
      ? 0
      : estimateHeight(blocks[index], textWidth, scale, softWrap);
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
    heights.reset(state.blocks.map((block) => estimateHeight(block, textWidth, scale, softWrap)));
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
      const state = await backend.open(note, mode);
      if (!destroyed) {
        take(state);
        attach();
      }
    } catch (error) {
      loading = false;
      onError(message(error));
    }
  }

  /**
   * ADR-025: with the text in the window, the core's changes, sections and
   * the backend's colours and figures reach the facade here.
   */
  function attach() {
    detach?.();
    detach = backend.attach(note, {
      patch: () => enqueue(performance.now(), () => backend.refresh(note)),
      views: (views) => {
        if (views.sections?.note === note) {
          sections = views.sections.sections;
        }
        if (views.found?.note === note) {
          // M-13.2, M-12.4: the words of the searches and the Nano hits.
          found = views.found.ranges;
          notify('content');
          if (info) info.found++;
        }
      },
      // FR-099: the spell check's marks wait for the core's revision.
      synced: () => takePendingSpell(),
      reopened: () => void open(),
      blocks: (changes) => takeBlocks(changes),
      // FR-097: the note's other view changed the text; its blocks follow.
      sibling: () => enqueue(performance.now(), () => backend.refresh(note)),
    });
  }

  /** The first block in view and how far into it the view starts. */
  function anchorOf(): { index: number; into: number } {
    const top = (scroller?.scrollTop ?? 0) - PADDING;
    const index = heights.indexAt(top);
    return { index, into: top - heights.top(index) };
  }

  /**
   * Applies `changes`, each following the one before and the first the
   * blocks shown; the text in view stays. A burst (the backend's colours
   * and figures) costs one copy of the blocks and one layout, not one each.
   */
  function applyChanges(changes: readonly BlocksView[]) {
    const before = anchorOf();
    let index = before.index;
    const next = blocks.slice();
    let hits = found;
    let marks = spell;
    for (const change of changes) {
      // M-13.2: the hits follow a change of the text until the core finds
      // them again; the backend's colours and figures change no text.
      if (change.rev !== rev && hits.length > 0) {
        hits = mapHits(hits, next, change);
      }
      // FR-099: so do the spell check's marks, until the next ones come.
      if (change.rev !== rev && marks.length > 0) {
        marks = mapHits(marks, next, change);
      }
      // A block changed in place keeps its height until it is measured again.
      const kept = Math.min(change.removed, change.blocks.length);
      heights.splice(
        change.from,
        change.removed,
        change.blocks.map((block, k) =>
          k < kept && heights.measured(change.from + k)
            ? heights.get(change.from + k)
            : estimateHeight(block, textWidth, scale, softWrap),
        ),
      );
      const end = change.from + change.removed;
      spliceBlocks(next, change);
      rev = change.rev;
      version = change.version;
      if (index >= end) {
        index += change.blocks.length - change.removed;
      } else if (index >= change.from) {
        index = change.from;
      }
    }
    blocks = next;
    if (hits !== found) {
      found = hits;
    }
    if (marks !== spell) {
      spell = marks;
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
  function takeBlocks(changes: readonly BlocksView[]) {
    if (loading) {
      return;
    }
    const ready: BlocksView[] = [];
    let at = version;
    for (const change of changes) {
      if (change.note !== note) {
        continue;
      }
      if (change.base !== at) {
        // One older than the blocks shown was taken in already.
        if (change.base > at) {
          waiting.set(change.base, change);
          waitingTimer ??= setTimeout(() => {
            waitingTimer = null;
            if (waiting.size > 0 && !destroyed) {
              waiting.clear();
              void open();
            }
          }, OUT_OF_STEP_MS);
        }
        if (change.base > at + 1000 || change.base < at - 1000) {
          void open();
        }
        continue;
      }
      ready.push(change);
      at = change.version;
      for (let next = waiting.get(at); next; next = waiting.get(at)) {
        waiting.delete(next.base);
        ready.push(next);
        at = next.version;
      }
    }
    if (ready.length === 0) {
      return;
    }
    applyChanges(ready);
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

  /** Scrolls to `into` pixels into block `index`, and keeps it there while heights settle. */
  function holdAt(index: number, into: number, time = 500) {
    if (!scroller) {
      return;
    }
    holding = { index, into, until: performance.now() + time };
    scroller.scrollTop = PADDING + heights.top(index) + into;
  }

  /**
   * M-07.3: the cursor to `offset` (markup, web units) once the blocks are
   * in, shown, with the keyboard (egui: `jump_to_byte`, then the facade's
   * `request_focus`). With `rev`, the offset is in the backend's text of
   * that revision: when the core's text is no longer it, nothing moves.
   */
  function jump(offset: number, rev?: number, to?: number) {
    void opened.then(async () => {
      await queue;
      if (destroyed || (rev !== undefined && !coreInStep(note, rev))) {
        return;
      }
      selections++;
      anchor = offset;
      head = to ?? offset;
      gap = null;
      sendSelection();
      // The place the facade came back to gives way.
      holding = null;
      await focusCursor();
    });
  }

  /**
   * UC-008 A6, M-13.4: a step of the find field: the core selects the hit,
   * the facade shows it (egui's `scroll_to_cursor`); the keyboard stays in
   * the find field, so the hit shows as the selection without it.
   */
  function find(spec: FindSpec, step: FindStep): Promise<FindAnswer | null> {
    return opened.then(async () => {
      await queue;
      if (destroyed) {
        return null;
      }
      const answer = await backend.find(note, spec, step);
      if (!answer || destroyed || answer.rev !== rev) {
        return answer;
      }
      selections++;
      anchor = answer.anchor;
      head = answer.head;
      gap = null;
      holding = null;
      if (info) info.place = answer.place;
      if (content && document.activeElement === content) {
        await focusCursor();
      } else {
        showCursor();
      }
      return answer;
    });
  }

  /**
   * UC-005 A20, A21, BR-017: replaces the selected occurrence of `spec`, or
   * all, in the text the facade shows, after the inputs before it; the
   * facade shows the change and the next occurrence selected.
   */
  function replace(spec: FindSpec, replacement: string, all: boolean): Promise<ReplaceDone | null> {
    return opened.then(
      () =>
        new Promise<ReplaceDone | null>((resolve, reject) => {
          enqueue(performance.now(), async () => {
            if (destroyed) {
              resolve(null);
              return null;
            }
            try {
              const done = await backend.replace(note, spec, replacement, all);
              resolve({ replaced: done.replaced, status: done.status });
              return done.patch;
            } catch (error) {
              reject(error instanceof Error ? error : new Error(message(error)));
              return null;
            }
          });
        }),
    );
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
        changed =
          heights.set(i, estimateHeight(blocks[i], textWidth, scale, softWrap), false) !== 0 ||
          changed;
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

  // UC-010 A11, A3: another zoom or soft wrap lays the text out anew: every
  // height is a guess again until measured, and the block in view stays at
  // the top, so the place the user reads stays in view.
  let laidOut: string | null = null;
  $effect(() => {
    const now = `${scale}:${softWrap}`;
    if (laidOut === null || laidOut === now || blocks.length === 0) {
      laidOut = now;
      return;
    }
    laidOut = now;
    const before = anchorOf();
    heights.reestimate(estimate, true);
    if (scroller) {
      scroller.scrollTop = PADDING + heights.top(before.index) + before.into;
    }
    relayout();
  });

  // M-11.4: the status of the keys follows the input mode; the first comes
  // with the facade (`open`).
  let modeSeen: string | null = null;
  $effect(() => {
    const now = mode;
    if (modeSeen === null || modeSeen === now) {
      modeSeen = now;
      return;
    }
    modeSeen = now;
    backend.setMode(note, now);
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
    // UC-004 A21: the reason after a formula's source stands for the end
    // of that formula.
    const reason = element?.closest<HTMLElement>('.reason');
    const reasonOf = reason?.previousElementSibling;
    if (reason && reasonOf instanceof HTMLElement && reasonOf.dataset.r !== undefined) {
      const run = block.runs[Number(reasonOf.dataset.r)];
      if (run) {
        return block.start + run.end;
      }
    }
    const runElement = element?.closest<HTMLElement>('[data-r]');
    if (!runElement || !blockElement.contains(runElement)) {
      // FR-098: a place between the elements of the runs, as next to a
      // typeset formula: the end of the run before it, or the start of
      // the one after it.
      if (node.nodeType === Node.ELEMENT_NODE) {
        const before = node.childNodes[offset - 1];
        const after = node.childNodes[offset];
        const ran = (child: ChildNode | undefined) =>
          child instanceof HTMLElement && child.dataset.r !== undefined
            ? block.runs[Number(child.dataset.r)]
            : undefined;
        const previous = ran(before);
        if (previous) {
          return block.start + previous.end;
        }
        const next = ran(after);
        if (next) {
          return sourceAt(block, next, 0);
        }
      }
      const first = block.runs[0];
      return first ? sourceAt(block, first, 0) : block.start;
    }
    const run = block.runs[Number(runElement.dataset.r)];
    if (!run) {
      return null;
    }
    if (runElement.dataset.formula !== undefined) {
      // FR-098: before or after a typeset formula.
      const atStart = node === runElement && offset === 0;
      return atStart ? sourceAt(block, run, 0) : block.start + run.end;
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
    const parent = runElement.parentNode;
    if (runElement instanceof HTMLElement && runElement.dataset.formula !== undefined && parent) {
      // FR-098: a typeset formula has no text: before it or after it.
      const at = Array.prototype.indexOf.call(parent.childNodes, runElement) as number;
      return { node: parent, offset: point.unit === 0 ? at : at + 1 };
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
    formula: null,
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
      // An input still runs once the facade is gone: its key reaches the
      // text (ADR-025, a switch of the view right after typing).
      try {
        const seen = selections;
        const patch = await work();
        if (patch && !destroyed) {
          await applyPatch(patch, seen === selections);
          probe?.(start, performance.now());
        }
      } catch (error) {
        // Once the facade is gone (its tab closed), a late input's failure is no one's.
        if (!destroyed) {
          onError(message(error));
        }
      }
    });
  }

  /** Sends `input`; `rows` when the cursor's rows are needed (M-09.13). */
  function send(input: FacadeInput, rows = false, start = performance.now()) {
    enqueue(start, async () => {
      const clipboard = wantsClipboard ? await backend.readClipboard() : null;
      const measured = rows && !destroyed ? measureRows() : null;
      return backend.input(note, at(), input, measured, clipboard);
    });
    // FR-080: after typed text, or any input while the list shows.
    if (input.kind === 'text' || linkCompletion.active(note)) {
      queue = queue.then(() => askLink(false));
    }
  }

  /** FR-080: this facade as the link completion writes into it. */
  const linkTarget: CompletionTarget = {
    get note() {
      return note;
    },
    complete: (choice) => {
      const complete = backend.completeLink;
      if (complete) {
        enqueue(performance.now(), () => complete(note, at(), choice));
      }
    },
    place: (from) => (destroyed ? null : linkPlace(from)),
  };
  /** FR-080: the text names the list of link completion while it shows. */
  const linkAria = $derived(linkCompletion.aria(note));

  /** FR-080, UC-005 A23: asks the core for the link typed at the cursor and offers it to the list. */
  function askLink(manual: boolean) {
    const ask = backend.linkContext;
    if (!ask || destroyed) {
      return;
    }
    let context: LinkContextView | null;
    try {
      context = ask(note, at(), manual);
    } catch (error) {
      onError(message(error));
      return;
    }
    linkCompletion.offer(linkTarget, context, context ? linkPlace(context.from) : null, manual);
  }

  /** Where the link starting at `from` is on the screen, else the cursor. */
  function linkPlace(from: number): { left: number; top: number; bottom: number } | null {
    const point = domPoint(from) ?? domPoint(head);
    if (!point) {
      return null;
    }
    let rect: DOMRect | undefined;
    try {
      const range = document.createRange();
      range.setStart(point.node, point.offset);
      range.collapse(true);
      rect = range.getClientRects?.()[0] ?? range.getBoundingClientRect?.();
    } catch {
      // A webview that lays out no ranges (or a test's) has no place for it.
      rect = undefined;
    }
    if (!rect || (rect.top === 0 && rect.bottom === 0 && rect.left === 0)) {
      const box = content?.getBoundingClientRect();
      return box ? { left: box.left + LEFT, top: box.top, bottom: box.top + 20 } : null;
    }
    return { left: rect.left, top: rect.top, bottom: rect.bottom };
  }

  /** M-09.14: copy, cut or paste through the backend. */
  function clipboardAction(action: ClipAction, plain: boolean) {
    const start = performance.now();
    enqueue(start, async () => {
      const done = await backend.clipboard(note, at(), action, plain);
      if (done.image) {
        // M-10.6: the image goes in once the backend stored it, after this
        // patch is in place.
        const { over } = done.image;
        enqueue(start, () => backend.pasteImage(note, at(), over));
      }
      return done.patch;
    });
  }

  /** M-10.3 to M-10.5: puts `insertion` in at the cursor; resolves once shown. */
  function insert(insertion: Insertion): Promise<void> {
    enqueue(performance.now(), () => backend.insert(note, at(), insertion));
    return queue.then(() => undefined);
  }

  /**
   * M-10.5, UC-005 A9: the cursor to the place under the point `x`, `y`
   * (CSS pixels of the window), where something is dropped; false when the
   * point is not over the drawn text, as egui's `place_cursor_at`.
   */
  function placeAt(x: number, y: number): boolean {
    if (!content) {
      return false;
    }
    const box = content.getBoundingClientRect();
    if (x < box.left || x > box.right || y < box.top || y > box.bottom) {
      return false;
    }
    const point = caretAt(x, y);
    const offset = point ? offsetOf(point.node, point.offset) : null;
    if (offset === null) {
      return false;
    }
    selections++;
    anchor = offset;
    head = offset;
    gap = null;
    sendSelection();
    void tick().then(() => placeSelection(offset, offset));
    return true;
  }

  /** M-10.5: where the cursor is, once the inputs before it are in. */
  async function mark(): Promise<CursorMark | null> {
    await queue;
    const hash = backend.textHash(note);
    return destroyed || hash === null ? null : { head, hash };
  }

  /** M-10.5: the cursor back to `mark`, when the text is still the same. */
  function back(to: CursorMark) {
    if (destroyed || backend.textHash(note) !== to.hash) {
      return;
    }
    selections++;
    anchor = to.head;
    head = to.head;
    gap = null;
    sendSelection();
    void tick().then(() => placeSelection(to.head, to.head));
  }

  /** The DOM place under the point, as the webview finds it. */
  function caretAt(x: number, y: number): { node: Node; offset: number } | null {
    const position = document.caretPositionFromPoint?.(x, y);
    if (position) {
      return { node: position.offsetNode, offset: position.offset };
    }
    const range = (
      document as Document & { caretRangeFromPoint?: (x: number, y: number) => Range | null }
    ).caretRangeFromPoint?.(x, y);
    return range ? { node: range.startContainer, offset: range.startOffset } : null;
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
    takeBlocks(patch.blocks);
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
    // UC-022 A4, app-tester: the selection is put into the text only while
    // the text has the keyboard; else it would take it from the find field
    // or the search overlay (a change from outside, an agent, a replace).
    const active = document.activeElement;
    if (content && active && active !== document.body && !content.contains(active)) {
      if (patch.scroll) {
        showCursor();
      }
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
    // FR-080: the cursor went elsewhere; the list closes.
    linkCompletion.close(note);
  }

  /** FR-098, TC-005-42 step 3: a click on an inline formula puts the cursor into its source. */
  async function onFormula(offset: number) {
    touched = true;
    selections++;
    anchor = offset;
    head = offset;
    gap = null;
    sendSelection();
    content?.focus({ preventScroll: true });
    await tick();
    placeSelection(offset, offset);
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
  /** Scrolls the block of the cursor into view when it is not. */
  function showCursor() {
    if (!scroller) {
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
  }

  async function focusCursor() {
    if (!scroller || !content) {
      return;
    }
    showCursor();
    content.focus({ preventScroll: true });
    await tick();
    placeSelection(anchor, head);
  }

  /** Escape: the keyboard goes to the note's tab (M-08.10, NFR-011). */
  function leave() {
    document.getElementById(tabId(note, pane))?.focus();
  }

  /** FR-099: whether the core holds the backend's text of revision `of`. */
  function spellInStep(of: number): boolean {
    return backend.inStep ? backend.inStep(note, of) : coreInStep(note, of);
  }

  /** FR-099, UC-005 BR-023: the marks of the spell check, once of the core's text. */
  function takeSpell(marks: SpellMarksView) {
    if (marks.note !== note || destroyed) {
      return;
    }
    if (!loading && spellInStep(marks.rev)) {
      pendingSpell = null;
      spell = marks.marks;
    } else {
      pendingSpell = marks;
    }
  }

  function takePendingSpell() {
    const marks = pendingSpell;
    if (marks && !loading && spellInStep(marks.rev)) {
      pendingSpell = null;
      spell = marks.marks;
    }
  }

  $effect(() => {
    if (spellMarks) {
      takeSpell(spellMarks);
    }
  });

  /**
   * UC-005 A28: opens the spell menu for the marked word at the markup
   * offset `offset`, at `x`, `y`; false when no word is marked there.
   */
  function openSpellMenu(offset: number, x: number, y: number): boolean {
    const word = spellAt(spell, offset);
    const shown = word ? shownWordIn(blocks, word[0], word[1]) : null;
    if (!word || !shown) {
      return false;
    }
    const first = shown.places[0];
    const end = shown.places[shown.places.length - 1];
    const markup = backend.markupOf?.(note, first, end) ?? undefined;
    // Below the word's line, so the word stays readable.
    const below = linkPlace(first);
    spellMenu = {
      from: word[0],
      to: word[1],
      shown: markup === undefined ? shown : { ...shown, markup, from: first },
      x: below?.left ?? x,
      y: below?.bottom ?? y,
      rev,
    };
    return true;
  }

  /** UC-005 A28: the cursor to the next marked word (`back`: the one before). */
  function stepSpell(back: boolean): boolean {
    if (spell.length === 0) {
      return false;
    }
    const next = back
      ? ([...spell].reverse().find(([, to]) => to < head) ?? spell[spell.length - 1])
      : (spell.find(([from]) => from > head) ?? spell[0]);
    jump(next[0]);
    return true;
  }

  /** The context menu by the mouse: the spell menu on a marked word. */
  function onContextMenu(event: MouseEvent) {
    const target = event.target instanceof Element ? event.target : null;
    if (!target?.closest('[data-spell]')) {
      return;
    }
    const point = caretAt(event.clientX, event.clientY);
    const offset = point ? offsetOf(point.node, point.offset) : null;
    if (offset !== null && openSpellMenu(offset, event.clientX, event.clientY)) {
      event.preventDefault();
    }
  }

  /** Shift+F10 or the menu key: the spell menu of the marked word at the cursor. */
  function spellMenuAtCursor(): boolean {
    const place = linkPlace(head);
    return place !== null && openSpellMenu(head, place.left, place.bottom);
  }

  /**
   * UC-005 A28 step 2, TC-005-39: the marked word becomes `suggestion`, as
   * one change undone alone; only the units that differ change, so markup
   * inside the word stays. Nothing changes when the word is gone.
   */
  function correct(menu: NonNullable<typeof spellMenu>, suggestion: string) {
    const now = shownWordIn(blocks, menu.from, menu.to);
    const change = correctionOf(menu.shown, suggestion);
    if (!now || now.word !== menu.shown.word || change === null || menu.rev !== rev) {
      // ADR-030: the text changed since the word was marked.
      onError(CHANGED_SINCE);
      return;
    }
    onCorrected?.(suggestion);
    enqueue(performance.now(), () =>
      backend.insert(
        note,
        { rev, anchor: change.from, head: change.to },
        { kind: 'correction', text: change.text },
      ),
    );
  }

  /**
   * The menu closes before the entry chosen acts; the mark stays until
   * then, so a correction still knows its word.
   */
  function closeSpellMenu(back: boolean) {
    const closing = spellMenu;
    queueMicrotask(() => {
      if (spellMenu === closing) spellMenu = null;
    });
    if (back) {
      content?.focus();
    }
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.defaultPrevented) {
      return;
    }
    if (
      ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') &&
      !composing &&
      spellMenuAtCursor()
    ) {
      // UC-005 A28: the spell menu of the word at the cursor.
      event.preventDefault();
      return;
    }
    // FR-080: the list's keys while it shows, and Ctrl+Space.
    if (backend.linkContext && !composing) {
      if (linkCompletion.key(event, note)) {
        event.preventDefault();
        return;
      }
      if (
        event.key === ' ' &&
        event.ctrlKey &&
        !event.altKey &&
        !event.metaKey &&
        !event.shiftKey
      ) {
        event.preventDefault();
        queue = queue.then(() => askLink(true));
        return;
      }
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
        if (movesBlocks(route.send)) {
          // Review: a key right after a click acts on the new caret, also
          // before the webview told the click (`selectionchange`).
          onSelectionChange();
        }
        send(route.send, route.rows, start);
    }
  }

  /** UC-005 A26, A27: a key that moves, copies or indents the blocks at the caret. */
  function movesBlocks(input: FacadeInput): boolean {
    if (input.kind !== 'key') {
      return false;
    }
    return (
      input.key === 'Tab' ||
      input.key === 'D' ||
      (input.alt && (input.key === 'ArrowUp' || input.key === 'ArrowDown'))
    );
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

  /**
   * Drops and drags of the webview are refused: files come through the
   * window's drop event (M-10.5, `editor/drop.ts`); text is not dropped
   * (FR-034 open, D-15).
   */
  function refuse(event: Event) {
    event.preventDefault();
  }

  /**
   * M-07.3, UC-004 BR-006: a click with Ctrl (Cmd on macOS) on a link
   * follows it; the click placed the cursor as any click. The window gives
   * the place of the link's first character, not its address: the backend
   * finds the link there in its own text. A link in a table cell is not
   * followed, as in egui (its runs are not mapped to the markup).
   */
  function onClick(event: MouseEvent) {
    const command = platform === 'macos' ? event.metaKey : event.ctrlKey;
    if (event.button !== 0 || !command || !onFollow) {
      return;
    }
    const element = (event.target as Element | null)?.closest<HTMLElement>('[data-link][data-r]');
    const index = Number(element?.closest<HTMLElement>('[data-b]')?.dataset.b);
    const block = blocks[index];
    const run = block?.runs[Number(element?.dataset.r)];
    if (run?.link) {
      // UC-004 A18: with Shift held too, the note opens to the side.
      onFollow(sourceAt(block, run, 0), event.shiftKey);
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
      scrolled.set(`${pane}:${note}`, anchorOf());
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

  /** UC-005 A26, A27: an action of the editing comfort from the command palette. */
  function edit(action: EditActionName) {
    send({ kind: 'edit', action });
    content?.focus({ preventScroll: true });
  }

  /**
   * UC-024 step 4, BR-006: writes `change` of a note attribute, after the
   * inputs before it, as one undo step; resolves once shown, with why
   * nothing was written (A3, A4), else `null`.
   */
  function attribute(change: AttributeChangeView): Promise<Refusal | null> {
    const write = backend.attribute;
    if (!write) {
      return Promise.resolve({
        problem: 'The note attributes cannot be changed here.',
        about: 'note',
      });
    }
    let refusal: Refusal | null = null;
    enqueue(performance.now(), async () => {
      const edited = await write(note, at(), change);
      refusal = edited.problem === null ? null : { problem: edited.problem, about: edited.about };
      return edited.patch;
    });
    return queue.then(() => refusal);
  }

  /** UC-024 BR-006: undoes the last change of the note, as Ctrl+Z (Cmd+Z) in it does. */
  function undo() {
    send({ kind: 'key', key: 'Z', ctrl: !mac, alt: false, shift: false, meta: mac });
  }

  /** UC-004 A16 step 3, BR-017: the cursor to the block holding the line of `target`. */
  function goToLine(target: LineTargetView) {
    const go = backend.goToLine;
    if (go) {
      enqueue(performance.now(), () => go(note, at(), target));
    }
    void focusCursor();
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
    hits: () =>
      facadeHits(
        blocks,
        found,
        folding.hidden,
        (i) => heights.top(i),
        (i) => heights.get(i),
        textWidth,
        PADDING,
      ),
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
    reveal,
    jump,
    find,
    replace,
    focus: () => void focusCursor(),
    tool,
    settled: () => queue,
    insert,
    placeAt,
    mark,
    completeLink: () => {
      void focusCursor();
      queue = queue.then(() => askLink(true));
    },
    back,
    edit,
    lineCount: () => backend.lineCount?.(note) ?? null,
    goToLine,
    attribute,
    undo,
    stepSpell,
    spellMenuAtCursor,
  };

  onMount(() => {
    setActiveFacade(controller, pane);
    // A dialog that shows hides the hint of a link (app-tester F8).
    const hideTooltip = () => (tooltip = null);
    // The note this view showed; the prop may name the next tab's by the
    // time the view goes (a key change, a rename).
    const shownNote = note;
    window.addEventListener(DIALOG_OPENED, hideTooltip);
    if (info) {
      info.facade = controller;
      info.minimap = minimap;
      info.keys = null;
    }
    document.addEventListener('selectionchange', onSelectionChange);
    // M-10.7: a selection made with the mouse is copied once it is let go,
    // after the selection reached the core.
    const unwatchPointer = onPointerReleased(() => {
      if (content && document.activeElement === content) {
        queue = queue.then(() =>
          backend.copySelection(note).catch((error: unknown) => onError(message(error))),
        );
      }
    });
    opened = open();
    void opened.then(async () => {
      if (destroyed || !scroller) {
        return;
      }
      await tick();
      if (destroyed || !scroller) {
        return;
      }
      const place = scrolled.get(`${pane}:${note}`);
      if (place && place.index < blocks.length) {
        holdAt(place.index, place.into, 800);
      }
      scrollTop = scroller.scrollTop;
      restored = true;
      if (focus) {
        await focusCursor();
      }
    });
    // M-07.3: a link led here, to a heading; M-13.3: a found place.
    const waiting = takeJump(note);
    if (waiting !== null) {
      runJump(controller, waiting);
    }
    return () => {
      destroyed = true;
      linkCompletion.close(shownNote);
      window.removeEventListener(DIALOG_OPENED, hideTooltip);
      backend.leave(shownNote, detach && !loading ? { version, blocks } : null);
      detach?.();
      document.removeEventListener('selectionchange', onSelectionChange);
      unwatchPointer();
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
      setActiveFacade(null, pane, controller);
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

<div
  class="facade"
  class:nowrap={!softWrap}
  aria-busy={loading}
  bind:clientWidth={width}
  style:--editor-zoom={scale}
>
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
      aria-haspopup="listbox"
      aria-controls={linkAria.controls}
      aria-activedescendant={linkAria.active}
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
      onkeydowncapture={() => (touched = true)}
      onblur={() => linkCompletion.close(note)}
      onfocus={() => backend.focused?.(note)}
      onmousedown={onMouseDown}
      onmousedowncapture={() => (touched = true)}
      onclick={onClick}
      onpointermove={onPointerMove}
      onpointerleave={() => (tooltip = null)}
      oncontextmenu={onContextMenu}
    >
      <div class="space" style:height="{spaceAbove}px" contenteditable="false"></div>
      {#key drawn}
        {#each shown as index (index)}
          {@const agent = agentBlocks.get(index)}
          {#if index === gapBefore}
            {@render emptyLines()}
          {/if}
          <div
            class="slot"
            class:agent={agent !== undefined}
            data-slot={index}
            data-agent={agent?.named ? agent.name : undefined}
            {@attach measure}
          >
            <Block
              block={blocks[index]}
              {index}
              {platform}
              editing={editing === index}
              section={folding.headings.get(index) ?? null}
              hits={found.length > 0 ? blockHits(blocks[index], found) : []}
              spell={spell.length > 0 ? blockHits(blocks[index], spell) : []}
              {current}
              onFold={fold}
              onEnlarge={(url, w, h) => (enlarged = { url, width: w, height: h })}
              {onFigure}
              cursor={cursorBlock === index ? head : null}
              {onFormula}
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
  <FormulaPreview
    edited={previewed}
    {content}
    {platform}
    moved={`${scrollTop}:${layout}:${drawn}:${width}:${viewHeight}:${scale}`}
  />
  {#if tooltip}
    <div
      class="tooltip"
      class:above={tooltip.y > TOOLTIP_ABOVE}
      role="tooltip"
      style:left="{tooltip.x + 12}px"
      style:top="{tooltip.y > TOOLTIP_ABOVE ? tooltip.y - 8 : tooltip.y + 16}px"
    >
      {tooltip.text}
    </div>
  {/if}
  {#if spellMenu}
    {@const menu = spellMenu}
    <SpellMenu
      word={menu.shown.word}
      {note}
      x={menu.x}
      y={menu.y}
      onChoose={(suggestion) => correct(menu, suggestion)}
      {onError}
      onClose={closeSpellMenu}
    />
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
    style:--row="{BODY_SIZE * scale * LINE_HEIGHT}px"
  >
    {#each gapLines as line (line)}<div class="gap-line"><br /></div>{/each}
  </div>
{/snippet}

<style>
  /* M-17.6: egui's agent colours, fixed (A-166): a light tint, a real 3 px
     border 6 px left of the block (also in forced colours, app.css), and
     the name on a pill in the theme's colours above the first block of
     each agent, where it covers neither text nor the format label (UX
     review and app-tester F14). */
  .slot.agent {
    position: relative;
    margin-left: -6px;
    padding-left: 3px;
    background-color: rgb(40 120 220 / 0.15);
    border-left: 3px solid rgb(40 120 220);
  }

  .slot.agent[data-agent] {
    padding-top: 1.5em;
  }

  .slot.agent[data-agent]::before {
    position: absolute;
    top: 2px;
    left: 3px;
    padding: 0 4px;
    font-size: 80%;
    line-height: 1.3;
    color: var(--palette-foreground);
    content: attr(data-agent);
    background: var(--palette-background);
    border: 1px solid rgb(40 120 220);
    border-radius: 3px;
    pointer-events: none;
  }

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
    font-size: calc(15px * var(--editor-zoom, 1));
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
    font-size: calc(15px * var(--editor-zoom, 1));
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

  .tooltip.above {
    transform: translateY(-100%);
  }
</style>
