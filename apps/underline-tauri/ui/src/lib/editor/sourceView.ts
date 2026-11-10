// The source view (M-08, UC-005): CodeMirror 6 as a view only (ADR-021,
// ADR-024). The backend owns the text: every input goes to it as one
// command, one after the other, and its patch is dispatched to CodeMirror
// afterwards. CodeMirror has no keymap and no history of its own, and
// nothing it would change by itself reaches the text; only the patches do.
//
// - Keys of the editor (`keys.ts`) and typed text go to the backend, which
//   applies the egui editor's rules (no moving or deleting by words).
// - Copy, cut and paste go through the backend's clipboard (M-08.6).
// - A click places the cursor, a drag or Shift+click extends the selection,
//   as CodeMirror does it; a double or triple click is a single click, as in
//   egui (M-08.3). The selection is then sent to the backend.
// - An input method composes in CodeMirror's DOM; at its end the
//   composition is taken back and the finished text sent (M-08.5).
// - The colours come as `ed-highlight` events for the lines shown
//   (M-08.2), never with the keystroke.
//
// Line numbers, folding, the minimap, the status line and the toolbar come
// with feature F4 as further extensions of `extensions()`; Vim through
// `@replit/codemirror-vim` too.

import {
  Annotation,
  ChangeSet,
  EditorSelection,
  EditorState,
  type Extension,
  type StateEffect,
  type Text,
  type Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView, drawSelection, type ViewUpdate } from '@codemirror/view';
import {
  commands,
  type ClipAction,
  type EditorInput,
  type EditorPatch,
  type EditorState as EditorStateView,
  type HighlightView,
  type UiError,
} from '../bindings';
import { colorField, setColors } from './highlight';
import { keyInput } from './keys';

/** What the source view needs of the backend (ADR-024). */
export interface EditorBackend {
  open(note: string): Promise<EditorStateView>;
  input(note: string, input: EditorInput): Promise<EditorPatch>;
  clipboard(note: string, action: ClipAction): Promise<EditorPatch>;
  select(note: string, rev: number, anchor: number, head: number): Promise<void>;
  viewport(note: string, from: number, to: number): Promise<void>;
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

/** The editor commands of the backend. */
export const tauriBackend: EditorBackend = {
  open: (note) => unwrap(commands.edOpen(note)),
  input: (note, input) => unwrap(commands.edInput(note, input)),
  clipboard: (note, action) => unwrap(commands.edClipboard(note, action)),
  select: async (note, rev, anchor, head) => {
    await unwrap(commands.edSelect(note, rev, anchor, head));
  },
  viewport: async (note, from, to) => {
    await unwrap(commands.edViewport(note, from, to));
  },
};

/** Marks the transactions that bring the backend's text. */
const fromBackend = Annotation.define<boolean>();

/** M-08.9: the cursor blinks as egui's: half a second on, half off. */
const BLINK_MS = 1000;

/**
 * M-08.1: DejaVu Sans Mono 14 px with soft wrap, in the theme's colours
 * (the `--palette-*` variables, lib/theme.ts). egui leaves 6 px around
 * the text.
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
});

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
};

export class SourceView {
  readonly view: EditorView;
  readonly note: string;
  #backend: EditorBackend;
  #options: SourceViewOptions;
  #rev: number;
  /** The inputs run one after the other, each on the last patch. */
  #queue: Promise<unknown> = Promise.resolve();
  /** Colours for a revision the view has not reached yet. */
  #pending: HighlightView | null = null;
  /** What an input method changed in CodeMirror, taken back at its end. */
  #composed: { changes: ChangeSet; startDoc: Text } | null = null;
  #viewport: [number, number] | null = null;
  /** The mouse's selection waiting to be sent. */
  #selection: { rev: number; anchor: number; head: number } | null = null;
  #viewportTimer: ReturnType<typeof setTimeout> | null = null;
  #destroyed = false;
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
    this.view = new EditorView({
      parent,
      state: this.#stateOf(state),
      scrollTo: scrolled.get(state.note),
    });
    this.#reportViewport(true);
  }

  /** Opens the source view of `note` in `parent`, with its text from the backend. */
  static async open(
    parent: HTMLElement,
    note: string,
    backend: EditorBackend,
    options: SourceViewOptions,
  ): Promise<SourceView> {
    const state = await backend.open(note);
    return new SourceView(parent, state, backend, options);
  }

  /** The revision of the text shown. */
  get rev(): number {
    return this.#rev;
  }

  /** Waits until every input sent so far is handled. */
  settled(): Promise<unknown> {
    return this.#queue;
  }

  focus(): void {
    this.view.focus();
  }

  /** M-08.2: colours from the backend; kept for a later revision. */
  takeColors(colors: HighlightView): void {
    if (colors.note !== this.note) {
      return;
    }
    if (colors.rev === this.#rev) {
      this.view.dispatch({ effects: setColors.of(colors) });
    } else {
      this.#pending = colors;
    }
  }

  /** UC-004 A7: the backend replaced the text; shown again unless it is this one. */
  replaced(rev: number): void {
    if (rev !== this.#rev) {
      this.#enqueue(() => this.#reload());
    }
  }

  destroy(): void {
    this.#destroyed = true;
    if (this.#viewportTimer !== null) {
      clearTimeout(this.#viewportTimer);
    }
    scrolled.set(this.note, this.view.scrollSnapshot());
    this.view.destroy();
  }

  #extensions(): Extension[] {
    return [
      EditorView.lineWrapping,
      drawSelection({ cursorBlinkRate: BLINK_MS }),
      EditorView.contentAttributes.of({
        spellcheck: 'false',
        autocorrect: 'off',
        autocapitalize: 'off',
        'aria-label': this.#options.label,
        'aria-description': 'Escape leaves the text',
      }),
      colorField,
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
    if (event.key === 'Escape' && !this.view.composing) {
      // M-08.10: in the Normal input mode Escape leaves the editor, as in
      // egui; Vim and Nano keep it (feature F4). The keyboard goes to the
      // note's tab (ux review F3), else nowhere.
      event.preventDefault();
      if (this.#options.onLeave) {
        this.#options.onLeave();
      } else {
        this.view.contentDOM.blur();
      }
      return true;
    }
    const input = keyInput(event, this.#options.mac, this.#page());
    if (input === null || this.view.composing) {
      return false;
    }
    event.preventDefault();
    this.#enqueue(() => this.#backend.input(this.note, input));
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

  /** Only the backend changes the text; an input method may compose. */
  #filter(tr: Transaction): Transaction | readonly TransactionSpec[] {
    if (!tr.docChanged || tr.annotation(fromBackend)) {
      return tr;
    }
    if (this.view?.composing) {
      this.#composed = this.#composed
        ? { ...this.#composed, changes: this.#composed.changes.compose(tr.changes) }
        : { changes: tr.changes, startDoc: tr.startState.doc };
      return tr;
    }
    return [];
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
    const byUser = update.transactions.some(
      (tr) => tr.isUserEvent('select') && !tr.annotation(fromBackend),
    );
    if (update.selectionSet && byUser) {
      // M-08.3: the selection the mouse made, for the revision it was made
      // in. A drag makes many; only the last one waiting is sent.
      const { anchor, head } = update.state.selection.main;
      const waiting = this.#selection !== null;
      this.#selection = { rev: this.#rev, anchor, head };
      if (!waiting) {
        this.#enqueue(async () => {
          const selection = this.#selection;
          this.#selection = null;
          if (selection) {
            await this.#backend.select(this.note, selection.rev, selection.anchor, selection.head);
          }
          return null;
        });
      }
    }
    if (update.viewportChanged || update.docChanged) {
      this.#reportViewport(false);
    }
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
      if (this.#destroyed) {
        return;
      }
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

  async #apply(patch: EditorPatch): Promise<void> {
    if (patch.base !== this.#rev) {
      // The backend's text changed meanwhile (UC-004 A7): show it again.
      await this.#reload();
      return;
    }
    let changes = ChangeSet.empty(this.view.state.doc.length);
    for (const change of patch.changes) {
      changes = changes.compose(
        ChangeSet.of(
          { from: change.from, to: change.to, insert: change.insert },
          changes.newLength,
        ),
      );
    }
    const length = changes.newLength;
    this.view.dispatch({
      changes,
      selection: EditorSelection.single(
        Math.min(patch.anchor, length),
        Math.min(patch.head, length),
      ),
      annotations: fromBackend.of(true),
      effects: patch.scroll ? EditorView.scrollIntoView(Math.min(patch.head, length)) : [],
    });
    this.#rev = patch.rev;
    if (this.#pending?.rev === this.#rev) {
      const pending = this.#pending;
      this.#pending = null;
      this.view.dispatch({ effects: setColors.of(pending) });
    }
  }

  async #reload(): Promise<EditorPatch | null> {
    const state = await this.#backend.open(this.note);
    if (this.#destroyed) {
      return null;
    }
    const snapshot = this.view.scrollSnapshot();
    this.view.setState(this.#stateOf(state));
    this.view.dispatch({ effects: snapshot });
    this.#rev = state.rev;
    this.#pending = null;
    this.#viewport = null;
    this.#reportViewport(true);
    return null;
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
