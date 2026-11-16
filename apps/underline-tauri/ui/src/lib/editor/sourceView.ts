// The source view (M-08, UC-005): CodeMirror 6 as a view only (ADR-021,
// ADR-025). The window's editor core holds the text (`wasmBackend.ts`):
// every input goes to it, one after the other, in the same task as the key,
// and its patch is dispatched to CodeMirror; the edits reach the backend
// after the frame (`sync.ts`). CodeMirror has no keymap and no history of
// its own.
//
// - Keys of the editor (`keys.ts`) and typed text go to the core, which
//   applies the egui editor's rules (no moving or deleting by words), the
//   Nano keys (M-11.3), the toolbar keys (M-10.2) and the folding keys
//   (M-08.8).
// - Copy, cut and paste go through the core and the backend's clipboard
//   (M-08.6, `clipboard.ts`): copying puts the markup there, a paste
//   inserts plain text, and an image alone is stored next to the note and
//   inserted as a picture (M-10.6). A finished selection is copied at once
//   while copy on select is on (M-10.7).
// - What the link and image dialogs answer and what is dropped on the
//   window goes in at the cursor through the core (`insert`, M-10.3 to
//   M-10.5); a drop first puts the cursor under the pointer (`placeAt`).
// - A click places the cursor, a drag or Shift+click extends the selection,
//   as CodeMirror does it; a double or triple click is a single click, as in
//   egui (M-08.3). The selection is then given to the core. A click with
//   Ctrl (Cmd on macOS) also follows the link at that place (M-07.3,
//   `onFollow` with the place; the backend finds the link in its text).
// - `jump` puts the cursor at a place, shows it and takes the keyboard: a
//   heading a link leads to (M-07.3, only while the text is still the
//   backend's revision of the answer), an entry of the tool panel (M-16.5).
// - An input method composes in CodeMirror's DOM; at its end the
//   composition is taken back and the finished text sent (M-08.5).
// - The colours come as `ed-highlight` events of the backend for the lines
//   shown (M-08.2), applied once the core is in step with their revision;
//   the sections (M-08.8) and the marked hits of the Nano search (M-11.3)
//   come from the core (`views`), never with the keystroke.
// - The Vim keys are `codemirror-vim` (A-01, `vim.ts`). They change the
//   text in CodeMirror first; each change goes to the core (`apply`) for
//   the revision it was made on. A patch of the core that comes meanwhile
//   (an undo, say) is moved over the changes not yet given, and they over
//   it, as CodeMirror's collaborative editing does, so no key is lost. When
//   the core's text is no longer the one a change was made on, the view
//   shows the core's text again.
// - Line numbers (M-08.7, `numbers.ts`), the fold arrows (`folding.ts`) and
//   the marked hits (`found.ts`) are further extensions.
// - FR-080, UC-005 A23: after typed text (also of the Vim keys' insert mode)
//   the core is asked whether the place of a link is typed at the cursor
//   (`linkContext`), Ctrl+Space asks for it in a written link; the list
//   (`linkCompletion.svelte.ts`) takes its keys before anyone else while it
//   shows, and its choice is written by the core (`completeLink`). A click
//   elsewhere or leaving the text closes it.
// - Package 4 (FR-081 to FR-086): close pairs, moving and duplicating lines
//   and indenting list items are the core's (their keys go there as any
//   key; the palette's commands come as `edit`); going to a line asks the
//   core for the patch (`goToLine`); the current line and the matching
//   bracket are an extension of the view (`currentLine.ts`, BR-022).
// - FR-099, UC-005 BR-023, A28 (ADR-030): the words the spell check marks
//   come as `spell-marks` events (`spellMarks.ts`), applied as the colours
//   are. The context menu (the right mouse button, Shift+F10, the menu key)
//   on a marked word asks the window for the spell menu (`onSpellMenu`);
//   elsewhere the text has no context menu, as in egui. A correction is one
//   change of the core (`correct`).

import {
  Annotation,
  ChangeSet,
  Compartment,
  EditorSelection,
  EditorState,
  Prec,
  type Extension,
  type StateEffect,
  type Text,
  type Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView, drawSelection, type ViewUpdate } from '@codemirror/view';
import type { Refusal } from './refusal';
import {
  type AttributeChangeView,
  type AttributeEdited,
  type ClipAction,
  type EditActionName,
  type Insertion,
  type EditorCommandName,
  type EditorInput,
  type EditorPatch,
  type EditorState as EditorStateView,
  type FindAnswer,
  type FindSpec,
  type FindStep,
  type FoundView,
  type AgentMarkView,
  type HighlightView,
  type KeysStatusView,
  type SpellMarksView,
  type LineTargetView,
  type LinkChoice,
  type LinkContextView,
  type Replaced,
  type SectionsView,
  type TextChange,
  type VimChange,
  type ToolName,
  type UiError,
  type VimState,
} from '../bindings';
import type { CursorMark, ReplaceDone } from '../facade/active';
import { linkCompletion, type CompletionTarget } from './linkCompletion.svelte';
import { onPointerReleased, type Clipboarded } from './clipboard';
import { folding, sectionsField, setSections } from './folding';
import { foundField, foundRanges, setFound } from './found';
import { agentMarksField, agentMarksTheme, setAgentMarks } from './agentMarks';
import { colorField, setColors } from './highlight';
import { setSpellMarks, spellField, spellTheme, spellWordAt, spellWordNext } from './spellMarks';
import { correctionOf, shownWordOf, type ShownWord } from './spellWord';
import { routeKey, type InputModeName } from './keys';
import type { CoreListener } from './sync';
import { lineNumbersOn, type LineNumberSide } from './numbers';
import { lineAndBrackets } from './currentLine';

/**
 * The Vim keys (`vim.ts` with `codemirror-vim`), loaded once the Vim mode
 * is chosen, so the start of the window does not wait for them (NFR-001).
 */
type VimModule = typeof import('./vim');
let vimModule: VimModule | null = null;

async function loadVim(): Promise<VimModule> {
  vimModule ??= await import('./vim');
  return vimModule;
}

/** What the source view needs of the backend (ADR-024). */
export interface EditorBackend {
  open(note: string): Promise<EditorStateView>;
  input(note: string, input: EditorInput): Promise<EditorPatch>;
  /**
   * M-08.6, M-09.14: copy, cut or paste through the system clipboard; with
   * `imageOnly` (Vim outside insert mode) a paste takes only an image.
   */
  clipboard(
    note: string,
    action: ClipAction,
    imageOnly?: boolean,
  ): Promise<Clipboarded<EditorPatch>>;
  /** M-10.6: stores the clipboard's image next to the note and inserts it. */
  pasteImage(note: string, over: boolean): Promise<EditorPatch | null>;
  /** M-10.3 to M-10.5: puts `insertion` in at the selection. */
  insert(note: string, insertion: Insertion): Promise<EditorPatch>;
  /** M-10.7: the selection, once settled, goes to the clipboard (copy on select). */
  copySelection(note: string): Promise<void>;
  /** M-10.5: the hash of the note's text in the window's core; `null` without one. */
  textHash(note: string): string | null;
  /** Sets the selection; gives the place for the status line. */
  select(note: string, rev: number, anchor: number, head: number): Promise<string>;
  /**
   * UC-008 A6, A7, M-13.4: a step of the find field holding `spec`: the hit
   * the core selected, `null` when nothing is found.
   */
  find(note: string, spec: FindSpec, step: FindStep): Promise<FindAnswer | null>;
  /** UC-005 A20, A21: replaces the selected occurrence of `spec`, or all. */
  replace(
    note: string,
    spec: FindSpec,
    replacement: string,
    all: boolean,
  ): Promise<Replaced<EditorPatch>>;
  viewport(note: string, from: number, to: number): Promise<void>;
  /**
   * A-01: the Vim keys' changes; `newStep` marks the start of an insert
   * session. `null` when the revision is gone or a change does not fit.
   */
  apply(
    note: string,
    base: number,
    changes: VimChange[],
    anchor: number,
    head: number,
    newStep: boolean,
  ): Promise<EditorPatch | null>;
  /** M-08.8: folds or expands the section of the heading on `line`. */
  fold(note: string, line: number): Promise<EditorPatch>;
  /** M-11.4: the status line's keys in the input mode of the settings. */
  keysStatus(note: string, vim: VimState | null): Promise<KeysStatusView | null>;
  /** ADR-014: the system clipboard as plain text, for Vim's `"+` and `"*`. */
  readClipboard(): Promise<string | null>;
  writeClipboard(text: string): Promise<void>;
  /**
   * ADR-025: the text lives in the window's editor core (`wasmBackend.ts`).
   * Its changes, sections and hits come from `attach`; the backend's
   * colours apply when the core is in step with their revision (`inStep`).
   */
  attach(note: string, listener: CoreListener): () => void;
  /** FR-097: the view took the keyboard; the window reads the core as this view. */
  focused?(note: string): void;
  /**
   * FR-097, UC-005 BR-016: the changes the note's other view made since
   * revision `base`, with this view's selection; `null` when the view takes
   * the whole text again.
   */
  since?(note: string, base: number): Promise<EditorPatch | null>;
  inStep(note: string, rev: number): boolean;
  /** M-11: the input mode of the settings, for the core's keys. */
  setMode(note: string, mode: InputModeName): void;
  /**
   * FR-080, UC-005 A23: the place of a link typed at the cursor; `manual`
   * for Ctrl+Space. Without it, no link completion.
   */
  linkContext?(note: string, manual: boolean): LinkContextView | null;
  /** FR-080: writes the link chosen; `null` when the text changed meanwhile. */
  completeLink?(note: string, choice: LinkChoice): Promise<EditorPatch | null>;
  /** UC-004 A16, BR-017: the cursor to the line and column of `target`. */
  goToLine?(note: string, target: LineTargetView): Promise<EditorPatch>;
  /** UC-024 step 4: writes `change` of a note attribute, one undo step; or says why not. */
  attribute?(note: string, change: AttributeChangeView): Promise<AttributeEdited<EditorPatch>>;
}

/** Marks the transactions that bring the backend's text. */
const fromBackend = Annotation.define<boolean>();

/** UC-004 A16: how long a jump to a line waits for the sections to open a fold, in ms. */
const REVEAL_MS = 1000;

/** M-08.9: the cursor blinks as egui's: half a second on, half off. */
const BLINK_MS = 1000;

/**
 * M-08.1: DejaVu Sans Mono 14 px with soft wrap, in the theme's colours
 * (UC-010 BR-009, BR-011: times the editor zoom, `--editor-zoom`; soft wrap
 * as the settings say, `setSoftWrap`)
 * (the `--palette-*` variables, lib/theme.ts). egui leaves 6 px around
 * the text. The line numbers and the "…" of a folded heading are 45 % of
 * the text colour (M-08.7); a marked hit is behind the text in the theme's
 * colour for hits and underlined in its colour for a hit (UC-008 BR-008).
 */
const sourceTheme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--palette-foreground)',
    backgroundColor: 'var(--palette-background)',
    border: '1px solid transparent',
    boxSizing: 'border-box',
  },
  // A thin line around the text while it has the keyboard, besides the
  // cursor (ux review F3); the focus colour stands 3:1 apart (NFR-010).
  '&.cm-focused': { outline: 'none', borderColor: 'var(--ui-focus)' },
  '.cm-scroller': {
    fontFamily: 'var(--font-mono)',
    fontSize: 'calc(14px * var(--editor-zoom, 1))',
    lineHeight: '1.25',
  },
  '.cm-content': { padding: '6px 0', caretColor: 'var(--palette-foreground)' },
  '.cm-line': { padding: '0 6px' },
  '.cm-cursor': { borderLeft: '2px solid var(--palette-foreground)' },
  '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground':
    { backgroundColor: 'var(--palette-selection)' },
  '.cm-gutters': {
    backgroundColor: 'var(--palette-background)',
    color: 'color-mix(in srgb, var(--palette-foreground) 45%, transparent)',
    border: 'none',
  },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0', minWidth: '1ch', textAlign: 'right' },
  // egui: 6 px, the 14 px arrow, then 2 px to the numbers.
  '.cm-ul-fold-gutter .cm-gutterElement': {
    width: '14px',
    padding: '0 2px 0 6px',
    overflow: 'visible',
  },
  // The arrow takes clicks over the whole gutter and at least 24 px high
  // (WCAG 2.2 target size), centred on its line.
  '.cm-ul-arrow': {
    display: 'flex',
    position: 'relative',
    height: '24px',
    margin: 'calc((1.25em - 24px) / 2) -2px 0 -6px',
    padding: '0 2px 0 6px',
    alignItems: 'center',
    justifyContent: 'center',
    cursor: 'pointer',
    fill: 'currentColor',
  },
  '.cm-ul-numbers-right': { pointerEvents: 'none' },
  '.cm-ul-number': {
    position: 'absolute',
    textAlign: 'right',
    color: 'color-mix(in srgb, var(--palette-foreground) 45%, transparent)',
  },
  '.cm-ul-folded': { marginLeft: '8px', color: 'var(--ui-text)' },
  '.cm-ul-found, .cm-searchMatch': {
    backgroundColor: 'var(--palette-found-background)',
    boxShadow: 'inset 0 -2px var(--palette-found)',
  },
  // A-01: the command line of the Vim keys (`:` and `/`).
  '.cm-panels': {
    backgroundColor: 'var(--palette-panel)',
    color: 'var(--palette-foreground)',
    borderTop: '1px solid var(--ui-border)',
  },
  '.cm-vim-panel': { fontFamily: 'var(--font-mono)', padding: '2px 6px' },
  // A notice ("3 lines yanked") in the text colour; the hint on the kind of
  // search patterns (`set pcre`) is left out, as egui has none.
  '.cm-vim-panel .cm-vim-message': { color: 'var(--palette-foreground) !important' },
  '.cm-vim-panel > div > span + span': { display: 'none' },
  '.cm-vim-panel input': { color: 'var(--palette-foreground)', fontFamily: 'var(--font-mono)' },
});

/**
 * A-01: the block cursor of the Vim keys in the theme's colours; above the
 * look `codemirror-vim` brings.
 */
const vimTheme = Prec.highest(
  EditorView.theme({
    // Doubled, to win over the look of `codemirror-vim` whatever comes first.
    '.cm-fat-cursor.cm-fat-cursor': {
      background: 'var(--palette-foreground)',
      color: 'var(--palette-background)',
    },
    '&:not(.cm-focused) .cm-fat-cursor.cm-fat-cursor': {
      background: 'none',
      outline: 'solid 1px var(--palette-foreground)',
    },
  }),
);

/** Where each note's view was scrolled to, while the window lives. */
const scrolled = new Map<string, StateEffect<unknown>>();

export type SourceViewOptions = {
  /** The command key is Cmd (macOS) instead of Ctrl. */
  mac: boolean;
  /**
   * UC-005 BR-016: what the view's reading position is remembered under, so
   * the tab of a note in each pane keeps its own; the note when not given.
   */
  place?: string;
  /** The name of the text field for screen readers: the note's title. */
  label: string;
  /** A command failed; the message is a status text. */
  onError: (message: string) => void;
  /** Ask for the colours of the lines shown (off only for measuring). */
  colors?: boolean;
  /** Escape left the text; the keyboard goes elsewhere (M-08.10). */
  onLeave?: () => void;
  /** The input mode of the settings (M-11). */
  mode?: InputModeName;
  /** M-08.7: line numbers on or off, left or right. */
  lineNumbers?: { shown: boolean; side: LineNumberSide };
  /** M-11.4: the place of the cursor changed. */
  onPlace?: (place: string) => void;
  /** M-11.4: the status of the Vim or Nano keys changed (`null`: none). */
  onKeys?: (status: KeysStatusView | null) => void;
  /** M-13.4: the marked hits changed. */
  onFound?: () => void;
  /** M-10.9: the keys ask to store the note or close its tab. */
  onCommand?: (command: EditorCommandName) => void;
  /** UC-005 A2: a toolbar function asks first (Link, Image; feature F6). */
  onRequest?: (tool: ToolName) => void;
  /** M-07.3, UC-004 BR-006: a Ctrl+click (Cmd+click) at `at` (web units) follows the link there. */
  /** M-07.3; UC-004 A18: with Shift held too, into the other pane (`side`). */
  onFollow?: (at: number, side: boolean) => void;
  /** UC-010 BR-011: long lines wrap (the default) or run on. */
  softWrap?: boolean;
  /** UC-010 BR-009: the editor zoom in percent (100 by default). */
  zoom?: number;
  /**
   * UC-005 A28: the context menu was asked for on the marked word `word`
   * at `from`..`to` (web units); the menu opens at `x`, `y`.
   */
  onSpellMenu?: (request: SpellMenuRequest) => void;
};

/** UC-005 A28: where the spell menu opens, and for which word. */
export type SpellMenuRequest = {
  /** The word as shown, with the markup offset of each unit. */
  shown: ShownWord;
  /** The markup the mark covers, `from`..`to` in web units. */
  markup: string;
  from: number;
  to: number;
  /** Where the menu opens: below the word's line. */
  x: number;
  y: number;
  /** The core's revision of the text the word was marked in. */
  rev: number;
};

/** What changed for those who follow the view (`subscribe`). */
export type ViewChange = 'content' | 'scroll';

/** One change as the backend takes it: the whole span the change touched. */
function covering(changes: ChangeSet, after: Text): TextChange | null {
  let fromA = Infinity;
  let toA = -1;
  let fromB = Infinity;
  let toB = -1;
  changes.iterChangedRanges((fa, ta, fb, tb) => {
    fromA = Math.min(fromA, fa);
    toA = Math.max(toA, ta);
    fromB = Math.min(fromB, fb);
    toB = Math.max(toB, tb);
  });
  if (toA < 0) {
    return null;
  }
  return { from: fromA, to: toA, insert: after.sliceString(fromB, toB) };
}

/** FR-080: the attributes of the text naming the list of link completion. */
function linkAriaOf(aria: { expanded: boolean; controls?: string; active?: string }): Extension {
  // `aria-expanded` belongs to a combobox, not to the text box the text is
  // (ARIA 1.2); the list is named by `aria-controls` and its chosen entry by
  // `aria-activedescendant` while it shows.
  const attributes: Record<string, string> = { 'aria-haspopup': 'listbox' };
  if (aria.controls) attributes['aria-controls'] = aria.controls;
  if (aria.active) attributes['aria-activedescendant'] = aria.active;
  return EditorView.contentAttributes.of(attributes);
}

export class SourceView {
  readonly view: EditorView;
  readonly note: string;
  #backend: EditorBackend;
  #options: SourceViewOptions;
  #rev: number;
  /** The text of `#rev`: what the backend has, without the changes not sent. */
  #baseDoc: Text;
  /**
   * A-01: the Vim keys' changes not sent yet, each on the text before it,
   * and whether it starts an undo step (M-10.1).
   */
  #local: { changes: ChangeSet; newStep: boolean }[] = [];
  /** A Vim command ended or an insert session began: the next change is a new step. */
  #stepStart = true;
  /** An insert session began and is to be marked in the backend. */
  #insertStart = false;
  /** The user moved the cursor or the selection since it was sent. */
  #selectionDirty = false;
  /** A job sending the changes and the selection waits in the queue. */
  #syncQueued = false;
  /** The inputs run one after the other, each on the last patch. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Colours, sections and hits for a revision the view has not reached yet. */
  #pending: HighlightView | null = null;
  #pendingSections: SectionsView | null = null;
  #pendingFound: FoundView | null = null;
  /** FR-099: the spell check's marks for a revision the view has not reached yet. */
  #pendingSpell: SpellMarksView | null = null;
  /** What an input method changed in CodeMirror, taken back at its end. */
  #composed: { changes: ChangeSet; startDoc: Text } | null = null;
  #composing = new WeakSet<Transaction>();
  #viewport: [number, number] | null = null;
  #viewportTimer: ReturnType<typeof setTimeout> | null = null;
  #destroyed = false;
  #mode: InputModeName;
  /** The mode of the settings, while the Vim keys load. */
  #wantedMode: InputModeName;
  #modeSlot = new Compartment();
  #numbersSlot = new Compartment();
  /** UC-010 BR-009: a theme per editor zoom, so CodeMirror measures its lines anew. */
  #zoomSlot = new Compartment();
  /** UC-010 BR-011: soft wrap on or off. */
  #wrapSlot = new Compartment();
  /** FR-080: what the text says of the list of link completion, for screen readers. */
  #linkAriaSlot = new Compartment();
  /** The Nano search prompt is open (M-11.3). */
  #prompting = false;
  #unwatchVim: (() => void) | null = null;
  #lastVim = '';
  #keysAsked = 0;
  #listeners = new Set<(change: ViewChange) => void>();
  /** ADR-025: lets the core's listener go. */
  #detach: (() => void) | null = null;
  /** M-10.7: stops waiting for the mouse buttons to be let go. */
  #unwatchPointer: (() => void) | null = null;
  /** UC-004 A16: until when (`performance.now()`) a jump to a line waits for the sections; 0: none. */
  #revealUntil = 0;
  /** FR-080: the next patch is that of typed text, after which a link may be typed. */
  #linkAfter = false;
  /** FR-080: this view as the link completion writes into it. */
  #linkTarget: CompletionTarget = {
    note: '',
    complete: (choice) => {
      const complete = this.#backend.completeLink;
      if (complete) {
        this.#enqueue(() => complete(this.note, choice));
      }
    },
    place: (from) => this.#linkPlace(from),
  };
  /** Measuring (`bench.ts`): when an input's handling began and its patch was in. */
  probe: ((start: number, patched: number) => void) | null = null;

  private constructor(
    parent: HTMLElement,
    state: EditorStateView,
    backend: EditorBackend,
    options: SourceViewOptions,
  ) {
    this.note = state.note;
    this.#backend = backend;
    this.#options = options;
    this.#rev = state.rev;
    this.#mode = options.mode ?? 'Normal';
    this.#wantedMode = this.#mode;
    const editorState = this.#stateOf(state);
    this.#baseDoc = editorState.doc;
    this.view = new EditorView({
      parent,
      state: editorState,
      scrollTo: scrolled.get(options.place ?? state.note),
    });
    this.view.dom.style.setProperty('--editor-zoom', String((options.zoom ?? 100) / 100));
    this.view.scrollDOM.addEventListener('scroll', this.#scrolled, { passive: true });
    // A paste is the view's before the Vim keys see it: `codemirror-vim`
    // would enter insert mode and paste there (A-66); the Vim command line
    // keeps the webview's paste.
    this.view.dom.addEventListener('paste', this.#pasteFirst, true);
    // M-10.7: a selection made with the mouse is copied once it is let go,
    // after the selection reached the core.
    this.#unwatchPointer = onPointerReleased(() => {
      if (this.view.hasFocus) {
        this.#enqueue(async () => {
          await this.#backend.copySelection(this.note);
          return null;
        });
      }
    });
    this.#attach();
    this.#linkTarget = { ...this.#linkTarget, note: state.note };
    // FR-080: the list's keys come before CodeMirror's and the Vim keys'.
    this.view.contentDOM.addEventListener('keydown', this.#linkKey, true);
    this.view.contentDOM.addEventListener('blur', this.#linkBlur);
    this.view.contentDOM.addEventListener('focus', this.#focused);
    this.#modeStarted();
    this.#options.onPlace?.(state.place);
    if (this.#mode === 'Nano') {
      this.#takeKeys(state.keys);
    }
    this.#reportViewport(true);
  }

  /** Opens the source view of `note` in `parent`, with its text from the backend. */
  static async open(
    parent: HTMLElement,
    note: string,
    backend: EditorBackend,
    options: SourceViewOptions,
  ): Promise<SourceView> {
    const [state] = await Promise.all([
      backend.open(note),
      options.mode === 'Vim' ? loadVim() : null,
    ]);
    return new SourceView(parent, state, backend, options);
  }

  /** The revision of the text shown. */
  get rev(): number {
    return this.#rev;
  }

  /** The input mode the view follows. */
  get mode(): InputModeName {
    return this.#mode;
  }

  /** Waits until every input sent so far is handled. */
  settled(): Promise<unknown> {
    return this.#queue;
  }

  focus(): void {
    this.view.focus();
  }

  /**
   * Calls `listener` when the text, its layout or the hits changed
   * (`content`), or when the view scrolled (`scroll`).
   */
  subscribe(listener: (change: ViewChange) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** M-12.4: the marked hits, `[from, to]`: of the Nano or of the Vim search. */
  hits(): [number, number][] {
    if (this.#mode === 'Vim') {
      return vimModule?.vimHits(this.view) ?? [];
    }
    return foundRanges(this.view.state.field(foundField));
  }

  /**
   * M-11: follows the input mode `mode` of the settings; the Vim keys take
   * over once they are loaded.
   */
  setMode(mode: InputModeName): void {
    this.#wantedMode = mode;
    if (mode === 'Vim' && !vimModule) {
      loadVim().then(
        () => this.setMode(this.#wantedMode),
        (error: unknown) => this.#fail(error),
      );
      return;
    }
    if (mode === this.#mode || this.#destroyed) {
      return;
    }
    this.#unwatchVim?.();
    this.#unwatchVim = null;
    vimModule?.dropVimKeys(this.view);
    this.#mode = mode;
    this.#prompting = false;
    this.view.dispatch({ effects: this.#modeSlot.reconfigure(this.#modeExtension()) });
    this.#modeStarted();
  }

  /** M-08.7: follows the line numbers of the settings. */
  setLineNumbers(shown: boolean, side: LineNumberSide): void {
    if (this.#destroyed) {
      return;
    }
    this.#options.lineNumbers = { shown, side };
    this.view.dispatch({ effects: this.#numbersSlot.reconfigure(lineNumbersOn(shown, side)) });
  }

  /**
   * UC-010 A3, BR-011: soft wrap on or off. The note is not changed: the
   * line numbers and go to line count its lines either way. The line at
   * the top stays there.
   */
  setSoftWrap(on: boolean): void {
    if (this.#destroyed || (this.#options.softWrap !== false) === on) {
      return;
    }
    this.#options.softWrap = on;
    this.#keepingPlace(() =>
      this.view.dispatch({
        effects: this.#wrapSlot.reconfigure(on ? EditorView.lineWrapping : []),
      }),
    );
  }

  /**
   * UC-010 A11, BR-009: the text `zoom` percent of its normal size; the line
   * at the top stays there, and the cursor stays in view when it was.
   */
  setZoom(zoom: number): void {
    const now = String(zoom / 100);
    if (this.#destroyed || this.view.dom.style.getPropertyValue('--editor-zoom') === now) {
      return;
    }
    this.#keepingPlace(() => {
      this.view.dom.style.setProperty('--editor-zoom', now);
      this.view.requestMeasure();
    });
    // UX review M1: CodeMirror keeps the gutter's line heights until it
    // measures again; once the new size is laid out, the gutter is made
    // anew and measured, so the numbers stand beside their lines.
    const frame = globalThis.requestAnimationFrame ?? ((f: () => void) => setTimeout(f, 16));
    frame(() => this.#remeasureGutter());
  }

  /** The line numbers made anew and the view measured again. */
  #remeasureGutter(): void {
    if (this.#destroyed) {
      return;
    }
    const numbers = this.#options.lineNumbers;
    // A changed theme is what makes CodeMirror measure the height of its
    // lines again (a CSS variable alone does not); the gutter follows.
    const zoom = this.view.dom.style.getPropertyValue('--editor-zoom') || '1';
    this.view.dispatch({
      effects: [
        this.#zoomSlot.reconfigure(EditorView.theme({ '&': { '--editor-zoom-measured': zoom } })),
        this.#numbersSlot.reconfigure(
          lineNumbersOn(numbers?.shown ?? false, numbers?.side ?? 'Left'),
        ),
      ],
    });
    this.view.requestMeasure();
  }

  /**
   * Runs `change`, which lays the text out anew, and scrolls back to the
   * line that was at the top; the cursor, if it was in view, stays so.
   */
  #keepingPlace(change: () => void): void {
    const view = this.view;
    const before = this.#shownLines();
    change();
    if (before === null) {
      return;
    }
    const head = view.state.selection.main.head;
    const cursorShown = before.from <= head && head <= before.to;
    view.dispatch({ effects: EditorView.scrollIntoView(before.from, { y: 'start' }) });
    if (!cursorShown) {
      return;
    }
    // Once laid out anew: the cursor back into view if it left it.
    const frame = globalThis.requestAnimationFrame ?? ((f: () => void) => setTimeout(f, 16));
    frame(() => {
      const after = this.#shownLines();
      const now = view.state.selection.main.head;
      if (!this.#destroyed && after !== null && (now < after.from || now > after.to)) {
        view.dispatch({ effects: EditorView.scrollIntoView(now, { y: 'nearest' }) });
      }
    });
  }

  /** The text from the line at the top of the view to the end of the line at its bottom. */
  #shownLines(): { from: number; to: number } | null {
    const view = this.view;
    try {
      const height = view.scrollDOM.getBoundingClientRect().top - view.documentTop;
      const top = view.lineBlockAtHeight(Math.max(0, height));
      const bottom = view.lineBlockAtHeight(Math.max(0, height + view.scrollDOM.clientHeight));
      return { from: top.from, to: bottom.to };
    } catch {
      // Without a layout (the tests) there is no place to keep.
      return null;
    }
  }

  /** M-10.2: a button of the toolbar; the keyboard goes back into the text. */
  tool(tool: ToolName): void {
    this.#enqueue(() => this.#backend.input(this.note, { kind: 'tool', tool }));
    this.focus();
  }

  /**
   * UC-005 A26, A27: an action of the editing comfort from the command
   * palette, as its key does it; the keyboard goes back into the text.
   */
  edit(action: EditActionName): void {
    this.#enqueue(() => this.#backend.input(this.note, { kind: 'edit', action }));
    this.focus();
  }

  /** UC-004 A16 step 1: how many lines the note has, as the line numbers count them. */
  lineCount(): number {
    return this.view.state.doc.lines;
  }

  /**
   * UC-004 A16 step 3, BR-017: the cursor to the line and column of
   * `target`, after the inputs before it; shown, with the keyboard.
   */
  goToLine(target: LineTargetView): void {
    const go = this.#backend.goToLine;
    if (go) {
      this.#enqueue(async () => {
        const patch = await go(this.note, target);
        // A folded section that holds the line opens only with the next
        // sections; the cursor is shown again once they are laid out.
        this.#revealUntil = performance.now() + REVEAL_MS;
        return patch;
      });
    }
    this.focus();
  }

  /**
   * UC-004 A16, BR-017: after the sections changed, the cursor is scrolled
   * into view again, centred, while a jump to a line waits for them.
   */
  #revealAfterSections(): void {
    if (this.#revealUntil === 0) {
      return;
    }
    const waiting = performance.now() <= this.#revealUntil;
    this.#revealUntil = 0;
    if (waiting && !this.#destroyed) {
      const head = this.view.state.selection.main.head;
      this.view.dispatch({ effects: EditorView.scrollIntoView(head, { y: 'center' }) });
    }
  }

  /**
   * UC-024 step 4, BR-006: writes `change` of a note attribute into the
   * text, after the inputs before it, as one undo step; resolves once it is
   * shown, with why nothing was written and what about (A3, A4), else
   * `null`.
   */
  attribute(change: AttributeChangeView): Promise<Refusal | null> {
    const write = this.#backend.attribute;
    if (!write) {
      return Promise.resolve({
        problem: 'The note attributes cannot be changed here.',
        about: 'note',
      });
    }
    let refusal: Refusal | null = null;
    this.#enqueue(async () => {
      const edited = await write(this.note, change);
      refusal = edited.problem === null ? null : { problem: edited.problem, about: edited.about };
      return edited.patch;
    });
    return this.#queue.then(() => refusal);
  }

  /** UC-024 BR-006: undoes the last change of the note, as Ctrl+Z (Cmd+Z) in it does. */
  undo(): void {
    this.#history('Z');
  }

  /**
   * M-10.3 to M-10.5: puts `insertion` in at the cursor, after the inputs
   * before it; resolves once it is shown.
   */
  insert(insertion: Insertion): Promise<void> {
    this.#enqueue(() => this.#backend.insert(this.note, insertion));
    return this.#queue.then(() => undefined);
  }

  /**
   * M-10.5, UC-005 A9: puts the cursor at the place under the point `x`,
   * `y` (CSS pixels of the window), where something is dropped; false when
   * the point is not over the text, as egui's `place_cursor_at`.
   */
  placeAt(x: number, y: number): boolean {
    const box = this.view.contentDOM.getBoundingClientRect();
    if (x < box.left || x > box.right || y < box.top || y > box.bottom) {
      return false;
    }
    const at = this.view.posAtCoords({ x, y });
    if (at === null) {
      return false;
    }
    this.view.dispatch({ selection: { anchor: at }, userEvent: 'select.pointer' });
    return true;
  }

  /** M-10.5: where the cursor is, once the inputs before it are in. */
  async mark(): Promise<CursorMark | null> {
    await this.settled();
    const hash = this.#backend.textHash(this.note);
    if (this.#destroyed || hash === null) {
      return null;
    }
    return { head: this.view.state.selection.main.head, hash };
  }

  /**
   * M-07.3, M-16.5: puts the cursor at `offset` (web units), after the
   * inputs before it, scrolls it into view and takes the keyboard (egui's
   * `jump_to_byte` and `focus_cursor`). With `rev`, the offset is in the
   * backend's text of that revision: when the view's text is no longer it,
   * nothing moves.
   */
  jump(offset: number, rev?: number, to?: number): void {
    this.#enqueue(async () => {
      if (this.#destroyed || (rev !== undefined && !this.#inStepWith(rev))) {
        return null;
      }
      const length = this.view.state.doc.length;
      const at = Math.min(Math.max(offset, 0), length);
      const end = to === undefined ? at : Math.min(Math.max(to, 0), length);
      this.view.dispatch({
        selection: { anchor: at, head: end },
        scrollIntoView: true,
        userEvent: 'select',
      });
      this.focus();
      return null;
    });
  }

  /**
   * UC-008 A6, A7, M-13.4: a step of the find field holding `spec`, after
   * the inputs before it: the core selects the hit, the view shows it
   * (egui's `select_chars`, which scrolls to it); the keyboard stays where
   * it is.
   */
  find(spec: FindSpec, step: FindStep): Promise<FindAnswer | null> {
    return new Promise((resolve) => {
      this.#enqueue(async () => {
        let answer: FindAnswer | null = null;
        try {
          if (!this.#destroyed) {
            answer = await this.#backend.find(this.note, spec, step);
          }
          if (answer && !this.#destroyed && this.#inStep(answer.rev)) {
            this.view.dispatch({
              selection: { anchor: answer.anchor, head: answer.head },
              scrollIntoView: true,
              userEvent: 'select',
            });
            this.#options.onPlace?.(answer.place);
          }
        } finally {
          resolve(answer);
        }
        return null;
      });
    });
  }

  /**
   * UC-005 A20, A21, FR-073: replaces the selected occurrence of `spec`
   * with `replacement`, or all of them, after the inputs before it; the
   * view shows the change and the next occurrence selected. What the find
   * field says then, and how many were replaced; `null` once the view is
   * gone.
   */
  replace(spec: FindSpec, replacement: string, all: boolean): Promise<ReplaceDone | null> {
    return new Promise((resolve, reject) => {
      this.#enqueue(async () => {
        if (this.#destroyed) {
          resolve(null);
          return null;
        }
        try {
          const done = await this.#backend.replace(this.note, spec, replacement, all);
          resolve({ replaced: done.replaced, status: done.status });
          return done.patch;
        } catch (error) {
          reject(error instanceof Error ? error : new Error(String(error)));
          return null;
        }
      });
    });
  }

  /** M-10.5: the cursor back to `mark`, when the text is still the same. */
  back(mark: CursorMark): void {
    if (this.#destroyed || this.#backend.textHash(this.note) !== mark.hash) {
      return;
    }
    const at = Math.min(mark.head, this.view.state.doc.length);
    this.view.dispatch({ selection: { anchor: at }, userEvent: 'select.pointer' });
  }

  /** ADR-025: the core's changes, sections and hits reach the view here. */
  #attach(): void {
    this.#detach?.();
    this.#detach = this.#backend.attach(this.note, {
      patch: (patch) => this.#enqueue(async () => patch),
      views: (views) => {
        if (views.sections) {
          this.#ownSections(views.sections);
        }
        if (views.found) {
          this.#ownFound(views.found);
        }
      },
      synced: () => this.#takePending(),
      reopened: () => this.#enqueue(() => this.#reload()),
      // FR-097: the note's other view changed the text; it shows here at once.
      sibling: () =>
        this.#enqueue(async () => {
          const since = this.#backend.since;
          const patch = since ? await since(this.note, this.#rev) : null;
          return patch ?? this.#reload();
        }),
    });
  }

  /** M-08.2: colours from the backend; kept for a later revision. */
  takeColors(colors: HighlightView): void {
    if (colors.note !== this.note) {
      return;
    }
    if (this.#colorsInStep(colors.rev)) {
      this.view.dispatch({ effects: setColors.of(colors) });
    } else {
      this.#pending = colors;
    }
  }

  /**
   * Whether colours of the backend's revision `rev` fit the text shown: the
   * window's core (ADR-025) is in step with `rev`, and no Vim change waits.
   */
  #colorsInStep(rev: number): boolean {
    return this.#local.length === 0 && this.#backend.inStep(this.note, rev);
  }

  /**
   * FR-099, UC-005 BR-023: the words the spell check marks; kept for a
   * later revision, while the marks shown move along with the edits.
   */
  takeSpell(marks: SpellMarksView): void {
    if (marks.note !== this.note || this.#destroyed) {
      return;
    }
    if (this.#colorsInStep(marks.rev)) {
      this.#pendingSpell = null;
      this.view.dispatch({ effects: setSpellMarks.of(marks.marks) });
    } else {
      this.#pendingSpell = marks;
    }
  }

  /**
   * UC-005 A28 step 2, TC-005-39: the marked word of `request` becomes
   * `suggestion`, as one change of the core, undone alone; only the units
   * that differ change, so markup inside the word stays. When the text
   * there is no longer the word, nothing changes.
   */
  correct(request: SpellMenuRequest, suggestion: string): Promise<boolean> {
    const change = correctionOf(request.shown, suggestion);
    if (
      change === null ||
      request.rev !== this.#rev ||
      this.#local.length > 0 ||
      this.view.state.sliceDoc(request.from, request.to) !== request.markup
    ) {
      // ADR-030: the text changed since the word was marked.
      return Promise.resolve(false);
    }
    this.#enqueue(async () => {
      await this.#backend.select(this.note, this.#rev, change.from, change.to);
      return this.#backend.insert(this.note, { kind: 'correction', text: change.text });
    });
    return this.#queue.then(() => true);
  }

  /**
   * UC-005 A28: the cursor to the next marked word (`back`: the one
   * before), shown, with the keyboard; false when no word is marked.
   */
  stepSpell(back: boolean): boolean {
    const word = spellWordNext(this.view, this.view.state.selection.main.head, back);
    if (!word) {
      return false;
    }
    this.view.dispatch({
      selection: { anchor: word[0] },
      effects: EditorView.scrollIntoView(word[0], { y: 'center' }),
    });
    this.view.focus();
    return true;
  }

  /**
   * UC-005 A28: asks for the spell menu of the marked word at `at`, opening
   * at `x`, `y`; false when no word is marked there.
   */
  #spellMenu(at: number, x: number, y: number): boolean {
    const word = this.#options.onSpellMenu ? spellWordAt(this.view, at) : null;
    if (!word) {
      return false;
    }
    const [from, to] = word;
    const markup = this.view.state.sliceDoc(from, to);
    // Below the word's line, so the word stays readable.
    const box = this.view.coordsAtPos(from);
    const place = box ? { x: box.left, y: box.bottom } : { x, y };
    this.#options.onSpellMenu?.({
      shown: shownWordOf(markup, from),
      markup,
      from,
      to,
      ...place,
      rev: this.#rev,
    });
    return true;
  }

  /** The context menu by the mouse: the spell menu on a marked word, else none. */
  #onContextMenu(event: MouseEvent, view: EditorView): boolean {
    event.preventDefault();
    const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (at !== null) {
      this.#spellMenu(at, event.clientX, event.clientY);
    }
    return true;
  }

  /** Shift+F10, the menu key or the palette: the spell menu of the marked word at the cursor. */
  spellMenuAtCursor(): boolean {
    const head = this.view.state.selection.main.head;
    // Where the cursor is drawn; the text's corner when it is not laid out.
    const box = this.view.coordsAtPos(head) ?? this.view.contentDOM.getBoundingClientRect();
    return this.#spellMenu(head, box.left, box.bottom);
  }

  /** M-08.8: the sections the core found; kept for a later revision. */
  #ownSections(sections: SectionsView): void {
    if (sections.note !== this.note) {
      return;
    }
    if (this.#inStep(sections.rev)) {
      this.view.dispatch({ effects: setSections.of(sections.sections) });
      this.#revealAfterSections();
    } else {
      this.#pendingSections = sections;
    }
  }

  /** M-11.3, M-13.2: the marked hits (searches, Nano); kept for a later revision. */
  #ownFound(found: FoundView): void {
    if (found.note !== this.note) {
      return;
    }
    if (this.#inStep(found.rev)) {
      this.view.dispatch({ effects: setFound.of(found.ranges) });
      this.#options.onFound?.();
    } else {
      this.#pendingFound = found;
    }
  }

  /** M-17.6: the agent cursors in this note, as the backend sent them last. */
  setAgentMarks(marks: readonly AgentMarkView[]): void {
    if (this.#destroyed) {
      return;
    }
    this.view.dispatch({ effects: setAgentMarks.of(marks) });
  }

  destroy(): void {
    // The Vim keys' changes not sent yet still go (A-01).
    if (this.#local.length > 0) {
      this.#scheduleSync();
    }
    this.#destroyed = true;
    this.#detach?.();
    this.view.contentDOM.removeEventListener('focus', this.#focused);
    this.#unwatchPointer?.();
    if (this.#viewportTimer !== null) {
      clearTimeout(this.#viewportTimer);
    }
    this.#unwatchVim?.();
    vimModule?.dropVimKeys(this.view);
    this.view.scrollDOM.removeEventListener('scroll', this.#scrolled);
    this.view.dom.removeEventListener('paste', this.#pasteFirst, true);
    this.view.contentDOM.removeEventListener('keydown', this.#linkKey, true);
    this.view.contentDOM.removeEventListener('blur', this.#linkBlur);
    linkCompletion.close(this.note);
    this.#listeners.clear();
    scrolled.set(this.#options.place ?? this.note, this.view.scrollSnapshot());
    this.view.destroy();
  }

  #modeExtension(): Extension {
    const leaves = this.#mode === 'Normal';
    return [
      this.#mode === 'Vim' && vimModule ? [vimModule.vimExtension(), vimTheme] : [],
      EditorView.contentAttributes.of(
        leaves ? { 'aria-description': 'Escape leaves the text' } : {},
      ),
    ];
  }

  /** The view now follows `#mode`: the Vim keys are tied, the status line told. */
  #modeStarted(): void {
    this.#backend.setMode(this.note, this.#mode);
    if (this.#mode === 'Vim' && vimModule) {
      const { onVimChange, vimKeys } = vimModule;
      this.#stepStart = true;
      vimKeys(
        this.view,
        {
          undo: () => this.#history('Z'),
          redo: () => this.#history('Y'),
          command: (command) => this.#command(command),
        },
        {
          read: () => this.#backend.readClipboard(),
          write: (text) => {
            this.#backend.writeClipboard(text).catch((error: unknown) => this.#fail(error));
          },
        },
      );
      const unwatch = onVimChange(this.view, () => this.#vimChanged());
      const unbound = vimModule.onVimBoundary(this.view, (insert) => this.#vimBoundary(insert));
      this.#unwatchVim = () => {
        unwatch();
        unbound();
      };
      this.#lastVim = '';
      this.#vimChanged();
    } else if (this.#mode === 'Nano') {
      this.#askKeys(null);
    } else {
      this.#options.onKeys?.(null);
    }
  }

  #extensions(): Extension[] {
    return [
      // A-01: the Vim keys see the keys first.
      this.#modeSlot.of(this.#modeExtension()),
      this.#linkAriaSlot.of(linkAriaOf({ expanded: false })),
      this.#wrapSlot.of(this.#options.softWrap === false ? [] : EditorView.lineWrapping),
      drawSelection({ cursorBlinkRate: BLINK_MS }),
      EditorView.contentAttributes.of({
        spellcheck: 'false',
        autocorrect: 'off',
        autocapitalize: 'off',
        'aria-label': this.#options.label,
      }),
      folding((line) => this.#enqueue(() => this.#backend.fold(this.note, line))),
      this.#zoomSlot.of([]),
      this.#numbersSlot.of(
        lineNumbersOn(
          this.#options.lineNumbers?.shown ?? false,
          this.#options.lineNumbers?.side ?? 'Left',
        ),
      ),
      colorField,
      // FR-084, FR-085: the current line and the matching bracket.
      lineAndBrackets(),
      foundField,
      // FR-099, UC-005 BR-023: the words the spell check marks.
      spellField,
      spellTheme,
      // M-17.6: the agent cursor.
      agentMarksField,
      agentMarksTheme,
      sourceTheme,
      EditorView.domEventHandlers({
        keydown: (event) => this.#onKey(event),
        beforeinput: (event) => this.#onBeforeInput(event),
        mousedown: (event, view) => this.#onMouseDown(event, view),
        click: (event, view) => this.#onClick(event, view),
        copy: (event) => this.#onClipboard(event, 'copy'),
        cut: (event) => this.#onClipboard(event, 'cut'),
        // Files come through the window's drop event (M-10.5, `drop.ts`);
        // text is not dropped or dragged (FR-034 open, D-15).
        drop: (event) => this.#refuse(event),
        dragstart: (event) => this.#refuse(event),
        // The egui editor has no context menu; a word the spell check
        // marks has one (UC-005 A28).
        contextmenu: (event, view) => this.#onContextMenu(event, view),
        compositionend: () => {
          // CodeMirror takes in the last change of the composition after
          // this event.
          setTimeout(() => this.#commitComposition(), 0);
          return false;
        },
      }),
      EditorState.transactionFilter.of((tr) => this.#filter(tr)),
      EditorView.updateListener.of((update) => this.#updated(update)),
    ];
  }

  #stateOf(state: EditorStateView): EditorState {
    const length = state.text.length;
    return EditorState.create({
      doc: state.text,
      selection: EditorSelection.single(
        Math.min(state.anchor, length),
        Math.min(state.head, length),
      ),
      extensions: this.#extensions(),
    });
  }

  /** How many lines Page Up and Page Down move: the lines shown. */
  #page(): number {
    const height = this.view.scrollDOM.clientHeight;
    return Math.max(1, Math.floor(height / this.view.defaultLineHeight));
  }

  #onKey(event: KeyboardEvent): boolean {
    if (event.defaultPrevented) {
      // The Vim keys took it.
      return true;
    }
    if (
      ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') &&
      !this.view.composing &&
      this.spellMenuAtCursor()
    ) {
      // UC-005 A28: the spell menu of the word at the cursor.
      event.preventDefault();
      return true;
    }
    if (event.key === 'Escape' && !this.view.composing && this.#mode === 'Normal') {
      // M-08.10: in the Normal input mode Escape leaves the editor, as in
      // egui; Vim and Nano keep it. The keyboard goes to the note's tab
      // (ux review F3), else nowhere.
      event.preventDefault();
      if (this.#options.onLeave) {
        this.#options.onLeave();
      } else {
        this.view.contentDOM.blur();
      }
      return true;
    }
    const route = routeKey(event, this.#options.mac, this.#page(), {
      mode: this.#mode,
      vimInsert: this.#mode === 'Vim' && !!vimModule?.vimInserting(this.view),
      prompting: this.#prompting,
    });
    if (route === null || this.view.composing) {
      return false;
    }
    event.preventDefault();
    if (route !== 'swallow') {
      this.#enqueue(() => this.#backend.input(this.note, route.send));
    }
    return true;
  }

  #onBeforeInput(event: InputEvent): boolean {
    if (this.view.composing || event.inputType === 'insertCompositionText') {
      return false;
    }
    event.preventDefault();
    const text = event.data;
    if (event.inputType === 'insertText' && text) {
      this.#linkAfter = true;
      this.#enqueue(() => this.#backend.input(this.note, { kind: 'text', text }));
    }
    return true;
  }

  /** M-08.3: no word or line selection by double or triple click. */
  #onMouseDown(event: MouseEvent, view: EditorView): boolean {
    if (event.button !== 0 || event.detail < 2) {
      return false;
    }
    event.preventDefault();
    const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (at !== null) {
      const anchor = event.shiftKey ? view.state.selection.main.anchor : at;
      view.dispatch({ selection: { anchor, head: at }, userEvent: 'select.pointer' });
    }
    view.focus();
    return true;
  }

  /**
   * M-07.3, UC-004 BR-006: a click with Ctrl (Cmd on macOS) follows the
   * link at the place clicked; the click also placed the cursor there. The
   * place is given once the inputs before it reached the core.
   */
  #onClick(event: MouseEvent, view: EditorView): boolean {
    const command = this.#options.mac ? event.metaKey : event.ctrlKey;
    if (event.button !== 0 || !command || !this.#options.onFollow) {
      return false;
    }
    const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (at === null) {
      return false;
    }
    const follow = this.#options.onFollow;
    const side = event.shiftKey;
    this.#enqueue(async () => {
      if (!this.#destroyed) {
        follow(at, side);
      }
      return null;
    });
    return false;
  }

  #onClipboard(event: ClipboardEvent, action: ClipAction): boolean {
    event.preventDefault();
    // As egui's Vim keys: outside insert mode a paste brings no text (that
    // is `p`); an image alone still comes (A-66).
    const imageOnly =
      action === 'paste' && this.#mode === 'Vim' && !vimModule?.vimInserting(this.view);
    this.#enqueue(async () => {
      const done = await this.#backend.clipboard(this.note, action, imageOnly);
      if (done.image) {
        // M-10.6: the image goes in once the backend stored it, after this
        // patch is shown.
        const { over } = done.image;
        this.#enqueue(() => this.#backend.pasteImage(this.note, over));
      }
      return done.patch;
    });
    return true;
  }

  #refuse(event: Event): boolean {
    event.preventDefault();
    return true;
  }

  /**
   * Only the backend changes the text, an input method may compose, and
   * the Vim keys change it first (A-01).
   */
  #filter(tr: Transaction): Transaction | readonly TransactionSpec[] {
    if (!tr.docChanged || tr.annotation(fromBackend)) {
      return tr;
    }
    if (this.view?.composing) {
      this.#composed = this.#composed
        ? { ...this.#composed, changes: this.#composed.changes.compose(tr.changes) }
        : { changes: tr.changes, startDoc: tr.startState.doc };
      this.#composing.add(tr);
      return tr;
    }
    return this.#mode === 'Vim' ? tr : [];
  }

  /** M-08.5: takes back what the composition did and sends its text. */
  #commitComposition(): void {
    const composed = this.#composed;
    this.#composed = null;
    if (!composed || this.#destroyed) {
      return;
    }
    let text = '';
    composed.changes.iterChanges((_from, _to, _fromB, _toB, inserted) => {
      text += inserted.toString();
    });
    this.view.dispatch({
      changes: composed.changes.invert(composed.startDoc),
      annotations: fromBackend.of(true),
    });
    if (text !== '') {
      this.#linkAfter = true;
      this.#enqueue(() => this.#backend.input(this.note, { kind: 'text', text }));
    }
  }

  #updated(update: ViewUpdate): void {
    let sync = false;
    for (const tr of update.transactions) {
      if (tr.annotation(fromBackend) || this.#composing.has(tr)) {
        continue;
      }
      if (tr.docChanged) {
        // A-01: a change of the Vim keys, sent with the next sync.
        this.#local.push({ changes: tr.changes, newStep: this.#takeStepStart() });
        sync = true;
      }
      if (tr.selection) {
        // M-08.3: the selection the mouse (or the Vim keys) made.
        this.#selectionDirty = true;
        sync = true;
        if (!tr.docChanged && !tr.selection.eq(tr.startState.selection)) {
          // FR-080: the cursor went elsewhere; the list closes. A selection
          // read back unchanged from the page (after Ctrl+Space) keeps it.
          linkCompletion.close(this.note);
        }
      }
    }
    if (sync) {
      this.#scheduleSync();
    }
    if (this.#mode === 'Vim') {
      // A search marks its hits after the key that ran it (UC-008 BR-008).
      this.#vimChanged();
    }
    if (update.viewportChanged || update.docChanged) {
      this.#reportViewport(false);
    }
    if (
      update.docChanged ||
      update.viewportChanged ||
      update.geometryChanged ||
      update.startState.field(sectionsField) !== update.state.field(sectionsField) ||
      update.startState.field(foundField) !== update.state.field(foundField)
    ) {
      this.#notify();
    }
  }

  #notify(change: ViewChange = 'content'): void {
    for (const listener of this.#listeners) {
      listener(change);
    }
  }

  #scrolled = (): void => this.#notify('scroll');

  /** FR-097: the keyboard came to this view. */
  #focused = (): void => this.#backend.focused?.(this.note);

  /** M-08.6, M-10.6: a paste into the text, taken before anyone else sees it. */
  #pasteFirst = (event: ClipboardEvent): void => {
    if (!(event.target instanceof Node) || !this.view.contentDOM.contains(event.target)) {
      return;
    }
    event.stopPropagation();
    this.#onClipboard(event, 'paste');
  };

  /** Sends the changes and the selection the user made, after the inputs before them. */
  #scheduleSync(): void {
    if (this.#syncQueued) {
      return;
    }
    this.#syncQueued = true;
    this.#enqueue(() => this.#sync());
  }

  async #sync(): Promise<null> {
    this.#syncQueued = false;
    const sent = this.#local;
    this.#local = [];
    const dirty = this.#selectionDirty;
    this.#selectionDirty = false;
    const insertStart = this.#insertStart;
    this.#insertStart = false;
    const { anchor, head } = this.view.state.selection.main;
    if (sent.length > 0 || insertStart) {
      const changes: VimChange[] = [];
      let doc = this.#baseDoc;
      for (const { changes: change, newStep } of sent) {
        doc = change.apply(doc);
        const whole = covering(change, doc);
        if (whole) {
          changes.push({ ...whole, newStep });
        }
      }
      const answer = await this.#backend.apply(
        this.note,
        this.#rev,
        changes,
        anchor,
        head,
        insertStart,
      );
      if (this.#destroyed) {
        return null;
      }
      if (answer === null) {
        // The backend's text changed meanwhile (UC-004 A7): show it again.
        await this.#reload();
        return null;
      }
      this.#baseDoc = doc;
      this.#rev = answer.rev;
      // FR-080: text typed in the insert mode of the Vim keys.
      this.#linkAfter = !!vimModule?.vimInserting(this.view);
      this.#took(answer);
    } else if (dirty) {
      const place = await this.#backend.select(this.note, this.#rev, anchor, head);
      this.#options.onPlace?.(place);
    }
    return null;
  }

  /** M-08.2: tells the backend which lines are shown, once they changed. */
  #reportViewport(force: boolean): void {
    if (this.#options.colors === false || this.#viewportTimer !== null) {
      return;
    }
    this.#viewportTimer = setTimeout(() => {
      this.#viewportTimer = null;
      if (this.#destroyed) {
        return;
      }
      const { doc } = this.view.state;
      const { from, to } = this.view.viewport;
      const lines: [number, number] = [doc.lineAt(from).number - 1, doc.lineAt(to).number];
      if (!force && this.#viewport?.[0] === lines[0] && this.#viewport?.[1] === lines[1]) {
        return;
      }
      this.#viewport = lines;
      this.#backend.viewport(this.note, lines[0], lines[1]).catch((error: unknown) => {
        this.#fail(error);
      });
    }, 0);
  }

  /** Runs `work` after every input before it and applies its patch. */
  #enqueue(work: () => Promise<EditorPatch | null | void>): void {
    this.#queue = this.#queue.then(async () => {
      // An input still runs once the view is gone: its key reaches the
      // text (ADR-025, a switch of the view right after typing); only its
      // patch is not shown.
      const start = performance.now();
      try {
        const patch = await work();
        if (patch && !this.#destroyed) {
          await this.#apply(patch);
          this.probe?.(start, performance.now());
        }
      } catch (error) {
        this.#fail(error);
      }
    });
  }

  /** Whether the view and its core hold the backend's text of `rev` and nothing of their own. */
  #inStepWith(rev: number): boolean {
    return this.#local.length === 0 && this.#backend.inStep(this.note, rev);
  }

  /** Whether the view shows the text of revision `rev` and nothing of its own. */
  #inStep(rev: number): boolean {
    return rev === this.#rev && this.#local.length === 0;
  }

  async #apply(patch: EditorPatch): Promise<void> {
    if (patch.base !== this.#rev) {
      // The backend's text changed meanwhile (UC-004 A7): show it again.
      await this.#reload();
      return;
    }
    let changes = ChangeSet.empty(this.#baseDoc.length);
    for (const change of patch.changes) {
      changes = changes.compose(
        ChangeSet.of(
          { from: change.from, to: change.to, insert: change.insert },
          changes.newLength,
        ),
      );
    }
    this.#baseDoc = changes.apply(this.#baseDoc);
    // A-01: the Vim keys' changes not sent yet move over the patch, and the
    // patch over them.
    let shown = changes;
    this.#local = this.#local.map((local) => {
      const moved = local.changes.map(shown);
      shown = shown.map(local.changes, true);
      return { ...local, changes: moved };
    });
    const length = shown.newLength;
    const keepOwn = this.#selectionDirty || this.#local.length > 0;
    const selection = keepOwn
      ? this.view.state.selection.map(shown)
      : EditorSelection.single(Math.min(patch.anchor, length), Math.min(patch.head, length));
    this.view.dispatch({
      changes: shown,
      selection,
      annotations: fromBackend.of(true),
      effects:
        patch.scroll && !keepOwn ? EditorView.scrollIntoView(Math.min(patch.head, length)) : [],
    });
    this.#rev = patch.rev;
    this.#took(patch);
  }

  /** What comes with every patch: the place, the keys, the commands, a request. */
  #took(patch: EditorPatch): void {
    this.#options.onPlace?.(patch.place);
    if (this.#mode === 'Nano') {
      // Newer than any status asked for before.
      this.#keysAsked += 1;
      this.#takeKeys(patch.keys);
    }
    for (const command of patch.commands) {
      this.#options.onCommand?.(command);
    }
    if (patch.request) {
      this.#options.onRequest?.(patch.request);
    }
    this.#takePending();
    this.#afterPatch();
  }

  /**
   * FR-080: after typed text, or any patch while the list shows, the core
   * is asked again whether a link is typed at the cursor.
   */
  #afterPatch(): void {
    const typed = this.#linkAfter;
    this.#linkAfter = false;
    if (typed || linkCompletion.active(this.note)) {
      this.#askLink(false);
    }
  }

  /** FR-080, OQ-247: link completion in the place of the link at the cursor, as Ctrl+Space asks. */
  completeLink(): void {
    this.focus();
    this.#enqueue(async () => {
      this.#askLink(true);
      return null;
    });
  }

  /** FR-080, UC-005 A23: asks the core for the link typed at the cursor and offers it to the list. */
  #askLink(manual: boolean): void {
    const ask = this.#backend.linkContext;
    if (!ask || this.#destroyed || this.#local.length > 0) {
      return;
    }
    let context: LinkContextView | null;
    try {
      context = ask(this.note, manual);
    } catch (error) {
      this.#fail(error);
      return;
    }
    const place = context !== null ? this.#linkPlace(context.from) : null;
    linkCompletion.offer(this.#linkTarget, context, place, manual);
  }

  /** FR-080: where the link starting at `from` is on the screen; `null` when it is not laid out. */
  #linkPlace(from: number): { left: number; top: number; bottom: number } | null {
    if (this.#destroyed) {
      return null;
    }
    const at = Math.min(from, this.view.state.doc.length);
    let coords = null;
    try {
      coords =
        this.view.coordsAtPos(at) ?? this.view.coordsAtPos(this.view.state.selection.main.head);
    } catch {
      // Nothing laid out (a test's DOM): the list shows without a place.
    }
    return coords ? { left: coords.left, top: coords.top, bottom: coords.bottom } : null;
  }

  /** FR-080: tells screen readers of the list of link completion while it shows. */
  setLinkAria(aria: { expanded: boolean; controls?: string; active?: string }): void {
    if (this.#destroyed) {
      return;
    }
    this.view.dispatch({ effects: this.#linkAriaSlot.reconfigure(linkAriaOf(aria)) });
  }

  /**
   * FR-080: the keys of the list while it shows (UC-005 A23 step 2, A24),
   * and Ctrl+Space, which asks for the completion in the place of a link.
   */
  #linkKey = (event: KeyboardEvent): void => {
    if (!this.#backend.linkContext || this.view.composing) {
      return;
    }
    if (linkCompletion.key(event, this.note)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.key === ' ' && event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      event.stopPropagation();
      this.#enqueue(async () => {
        this.#askLink(true);
        return null;
      });
    }
  };

  /** FR-080: leaving the text closes the list. */
  #linkBlur = (): void => {
    linkCompletion.close(this.note);
  };

  /** Colours, sections and hits that waited for the revision shown now. */
  #takePending(): void {
    const colors = this.#pending;
    if (colors && this.#colorsInStep(colors.rev)) {
      this.#pending = null;
      this.view.dispatch({ effects: setColors.of(colors) });
    }
    const sections = this.#pendingSections;
    if (sections && this.#inStep(sections.rev)) {
      this.#pendingSections = null;
      this.view.dispatch({ effects: setSections.of(sections.sections) });
      this.#revealAfterSections();
    }
    const spell = this.#pendingSpell;
    if (spell && this.#colorsInStep(spell.rev)) {
      this.#pendingSpell = null;
      this.view.dispatch({ effects: setSpellMarks.of(spell.marks) });
    }
    const found = this.#pendingFound;
    if (found && this.#inStep(found.rev)) {
      this.#pendingFound = null;
      this.view.dispatch({ effects: setFound.of(found.ranges) });
      this.#options.onFound?.();
    }
  }

  async #reload(): Promise<EditorPatch | null> {
    const state = await this.#backend.open(this.note);
    if (this.#destroyed) {
      return null;
    }
    const snapshot = this.view.scrollSnapshot();
    this.#unwatchVim?.();
    this.#unwatchVim = null;
    this.view.setState(this.#stateOf(state));
    this.view.dispatch({ effects: snapshot });
    this.#rev = state.rev;
    this.#baseDoc = this.view.state.doc;
    this.#local = [];
    this.#selectionDirty = false;
    this.#pending = null;
    this.#pendingSections = null;
    this.#pendingFound = null;
    this.#viewport = null;
    this.#attach();
    this.#linkTarget = { ...this.#linkTarget, note: state.note };
    // FR-080: the list's keys come before CodeMirror's and the Vim keys'.
    this.view.contentDOM.addEventListener('keydown', this.#linkKey, true);
    this.view.contentDOM.addEventListener('blur', this.#linkBlur);
    this.#modeStarted();
    this.#options.onPlace?.(state.place);
    this.#reportViewport(true);
    return null;
  }

  /** M-10.1: the backend's undo (`Z`) or redo (`Y`), for the Vim keys. */
  #history(key: 'Z' | 'Y'): void {
    const mac = this.#options.mac;
    this.#enqueue(() =>
      this.#backend.input(this.note, {
        kind: 'key',
        key,
        ctrl: !mac,
        alt: false,
        shift: false,
        meta: mac,
        page: 1,
      }),
    );
  }

  /** M-10.9: a command of the Vim keys, after the changes before it are sent. */
  #command(command: EditorCommandName): void {
    this.#enqueue(async () => {
      this.#options.onCommand?.(command);
      return null;
    });
  }

  /** Whether the next change starts an undo step; it is taken by it. */
  #takeStepStart(): boolean {
    const start = this.#stepStart;
    this.#stepStart = this.#mode !== 'Vim';
    return start;
  }

  /**
   * M-10.1, A-01: a Vim command ended (the next change is a step of its
   * own) or an insert session began (its typing is a step of its own,
   * marked in the backend even before it changes anything).
   */
  #vimBoundary(insert: boolean): void {
    this.#stepStart = true;
    if (insert) {
      this.#insertStart = true;
      this.#scheduleSync();
    }
  }

  /** M-11.4: the Vim keys changed their mode or what is typed. */
  #vimChanged(): void {
    const state = vimModule?.vimState(this.view) ?? null;
    const key = JSON.stringify(state);
    if (key === this.#lastVim) {
      return;
    }
    this.#lastVim = key;
    this.#askKeys(state);
    this.#notify();
  }

  /** Asks the backend for the status line's keys; a later answer wins. */
  #askKeys(vim: VimState | null): void {
    const asked = ++this.#keysAsked;
    this.#backend.keysStatus(this.note, vim).then(
      (status) => {
        if (asked === this.#keysAsked && !this.#destroyed) {
          this.#takeKeys(status);
        }
      },
      (error: unknown) => this.#fail(error),
    );
  }

  #takeKeys(status: KeysStatusView | null): void {
    this.#prompting = status?.prompting ?? false;
    this.#options.onKeys?.(status);
  }

  #fail(error: unknown): void {
    const message =
      typeof error === 'object' && error !== null && 'message' in error
        ? String((error as UiError).message)
        : String(error);
    console.error(`The editor failed: ${message}`);
    this.#options.onError(message);
  }
}
