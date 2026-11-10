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
//   (M-08.6).
// - A click places the cursor, a drag or Shift+click extends the selection,
//   as CodeMirror does it; a double or triple click is a single click, as in
//   egui (M-08.3). The selection is then given to the core.
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
import {
  type ClipAction,
  type EditorCommandName,
  type EditorInput,
  type EditorPatch,
  type EditorState as EditorStateView,
  type FoundView,
  type HighlightView,
  type KeysStatusView,
  type SectionsView,
  type TextChange,
  type VimChange,
  type ToolName,
  type UiError,
  type VimState,
} from '../bindings';
import { folding, sectionsField, setSections } from './folding';
import { foundField, foundRanges, setFound } from './found';
import { colorField, setColors } from './highlight';
import { routeKey, type InputModeName } from './keys';
import type { CoreListener } from './sync';
import { lineNumbersOn, type LineNumberSide } from './numbers';

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
  clipboard(note: string, action: ClipAction): Promise<EditorPatch>;
  /** Sets the selection; gives the place for the status line. */
  select(note: string, rev: number, anchor: number, head: number): Promise<string>;
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
  inStep(note: string, rev: number): boolean;
  /** M-11: the input mode of the settings, for the core's keys. */
  setMode(note: string, mode: InputModeName): void;
}

/** Marks the transactions that bring the backend's text. */
const fromBackend = Annotation.define<boolean>();

/** M-08.9: the cursor blinks as egui's: half a second on, half off. */
const BLINK_MS = 1000;

/**
 * M-08.1: DejaVu Sans Mono 14 px with soft wrap, in the theme's colours
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
    fontSize: '14px',
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
  /** M-10.9: the keys ask to store the note or close its tab. */
  onCommand?: (command: EditorCommandName) => void;
  /** UC-005 A2: a toolbar function asks first (Link, Image; feature F6). */
  onRequest?: (tool: ToolName) => void;
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
  /** The Nano search prompt is open (M-11.3). */
  #prompting = false;
  #unwatchVim: (() => void) | null = null;
  #lastVim = '';
  #keysAsked = 0;
  #listeners = new Set<(change: ViewChange) => void>();
  /** ADR-025: lets the core's listener go. */
  #detach: (() => void) | null = null;
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
      scrollTo: scrolled.get(state.note),
    });
    this.view.scrollDOM.addEventListener('scroll', this.#scrolled, { passive: true });
    this.#attach();
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

  /** M-10.2: a button of the toolbar; the keyboard goes back into the text. */
  tool(tool: ToolName): void {
    this.#enqueue(() => this.#backend.input(this.note, { kind: 'tool', tool }));
    this.focus();
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

  /** M-08.8: the sections the core found; kept for a later revision. */
  #ownSections(sections: SectionsView): void {
    if (sections.note !== this.note) {
      return;
    }
    if (this.#inStep(sections.rev)) {
      this.view.dispatch({ effects: setSections.of(sections.sections) });
    } else {
      this.#pendingSections = sections;
    }
  }

  /** M-11.3: the marked hits of the Nano search; kept for a later revision. */
  #ownFound(found: FoundView): void {
    if (found.note !== this.note) {
      return;
    }
    if (this.#inStep(found.rev)) {
      this.view.dispatch({ effects: setFound.of(found.ranges) });
    } else {
      this.#pendingFound = found;
    }
  }

  destroy(): void {
    // The Vim keys' changes not sent yet still go (A-01).
    if (this.#local.length > 0) {
      this.#scheduleSync();
    }
    this.#destroyed = true;
    this.#detach?.();
    if (this.#viewportTimer !== null) {
      clearTimeout(this.#viewportTimer);
    }
    this.#unwatchVim?.();
    vimModule?.dropVimKeys(this.view);
    this.view.scrollDOM.removeEventListener('scroll', this.#scrolled);
    this.#listeners.clear();
    scrolled.set(this.note, this.view.scrollSnapshot());
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
      EditorView.lineWrapping,
      drawSelection({ cursorBlinkRate: BLINK_MS }),
      EditorView.contentAttributes.of({
        spellcheck: 'false',
        autocorrect: 'off',
        autocapitalize: 'off',
        'aria-label': this.#options.label,
      }),
      folding((line) => this.#enqueue(() => this.#backend.fold(this.note, line))),
      this.#numbersSlot.of(
        lineNumbersOn(
          this.#options.lineNumbers?.shown ?? false,
          this.#options.lineNumbers?.side ?? 'Left',
        ),
      ),
      colorField,
      foundField,
      sourceTheme,
      EditorView.domEventHandlers({
        keydown: (event) => this.#onKey(event),
        beforeinput: (event) => this.#onBeforeInput(event),
        mousedown: (event, view) => this.#onMouseDown(event, view),
        copy: (event) => this.#onClipboard(event, 'copy'),
        cut: (event) => this.#onClipboard(event, 'cut'),
        paste: (event) => this.#onClipboard(event, 'paste'),
        // Files are dropped with feature F6; text is not dropped or dragged.
        drop: (event) => this.#refuse(event),
        dragstart: (event) => this.#refuse(event),
        // The egui editor has no context menu.
        contextmenu: (event) => this.#refuse(event),
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

  #onClipboard(event: ClipboardEvent, action: ClipAction): boolean {
    event.preventDefault();
    this.#enqueue(() => this.#backend.clipboard(this.note, action));
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
  }

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
    }
    const found = this.#pendingFound;
    if (found && this.#inStep(found.rev)) {
      this.#pendingFound = null;
      this.view.dispatch({ effects: setFound.of(found.ranges) });
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
