// The source view: CodeMirror 6 as a view only. No CM keymap, no CM history:
// every `beforeinput` is cancelled and sent to the owner; its patch is then
// dispatched to CodeMirror. Whatever CodeMirror changes by itself (a
// composition, a paste, a drop) is caught by a transaction filter, taken
// back and sent to the owner as well (M-08.4, M-08.5).

import { Annotation, ChangeSet, EditorState, type Extension } from '@codemirror/state';
import { EditorView, lineNumbers } from '@codemirror/view';
import { markdown } from '@codemirror/lang-markdown';
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Input, Owner, SourcePatch } from './types';
import { afterPaint, isPromise, nextFrame, type Sample } from './timing';

const fromOwner = Annotation.define<boolean>();

export function inputOf(e: InputEvent): Input | null {
  switch (e.inputType) {
    case 'insertText':
    case 'insertReplacementText':
      return e.data ? { type: 'insertText', text: e.data } : null;
    case 'insertParagraph':
    case 'insertLineBreak':
      return { type: 'insertParagraph' };
    case 'deleteContentBackward':
      return { type: 'deleteBackward' };
    default:
      return null;
  }
}

export class SourceView {
  readonly view: EditorView;
  private chain: Promise<unknown> = Promise.resolve();
  /** The sample of the last input handled, for the driver. */
  last: Promise<Sample | null> = Promise.resolve(null);
  /** Changes CodeMirror made during a composition, sent at its end. */
  private composed: { changes: ChangeSet; startDoc: EditorState['doc'] } | null = null;

  constructor(
    parent: HTMLElement,
    text: string,
    private readonly owner: Owner,
    private readonly highlight: boolean,
  ) {
    this.view = new EditorView({ parent, state: this.stateOf(text) });
  }

  private stateOf(text: string): EditorState {
    const extensions: Extension[] = [
      lineNumbers(),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: 'false', autocorrect: 'off' }),
      EditorView.domEventHandlers({
        beforeinput: (e) => this.onBeforeInput(e),
        compositionend: () => {
          // CodeMirror applies the last DOM change after this event.
          setTimeout(() => this.commitComposition(), 0);
          return false;
        },
      }),
      EditorState.transactionFilter.of((tr) => {
        if (!tr.docChanged || tr.annotation(fromOwner)) return tr;
        if (this.view?.composing) {
          // Let the IME work on CodeMirror's DOM; collect what it does.
          this.composed = this.composed
            ? { ...this.composed, changes: this.composed.changes.compose(tr.changes) }
            : { changes: tr.changes, startDoc: tr.startState.doc };
          return tr;
        }
        // Anything else goes to the owner instead.
        const ranges: { from: number; to: number; text: string }[] = [];
        tr.changes.iterChanges((from, to, _f, _t, inserted) =>
          ranges.push({ from, to, text: inserted.toString() }),
        );
        for (const r of ranges.reverse()) {
          this.enqueue(performance.now(), r.from, { type: 'replace', to: r.to, text: r.text });
        }
        return [];
      }),
    ];
    if (this.highlight) {
      extensions.push(markdown(), syntaxHighlighting(defaultHighlightStyle));
    }
    return EditorState.create({ doc: text, extensions });
  }

  /** Takes back what the composition did and sends it to the owner. */
  private commitComposition(): void {
    const c = this.composed;
    this.composed = null;
    if (!c) return;
    const ranges: { from: number; to: number; text: string }[] = [];
    c.changes.iterChanges((from, to, _f, _t, inserted) =>
      ranges.push({ from, to, text: inserted.toString() }),
    );
    this.view.dispatch({ changes: c.changes.invert(c.startDoc), annotations: fromOwner.of(true) });
    for (const r of ranges.reverse()) {
      this.enqueue(performance.now(), r.from, { type: 'replace', to: r.to, text: r.text });
    }
  }

  private onBeforeInput(e: InputEvent): boolean {
    if (e.inputType === 'insertCompositionText' || this.view.composing) return false;
    const input = inputOf(e);
    if (input) {
      this.enqueue(e.timeStamp, null, input);
    }
    // Cancel everything else as well: the owner decides what changes.
    return true;
  }

  /** Handles inputs one after the other, so each sees the last patch. */
  private enqueue(t0: number, at: number | null, input: Input): void {
    const frame = nextFrame();
    const run = this.chain.then(() => this.process(t0, at, input, frame));
    this.chain = run.catch(() => null);
    this.last = run;
  }

  private async process(
    t0: number,
    at: number | null,
    input: Input,
    frame: { time: number },
  ): Promise<Sample> {
    const start = performance.now();
    const cursor = at ?? this.view.state.selection.main.head;
    const r = this.owner.sourceInput(cursor, input);
    const patch = isPromise(r) ? await r : r;
    const ownerMs = isPromise(r) ? this.owner.lastOwnerMs : performance.now() - start;
    const applying = performance.now();
    this.apply(patch);
    const patched = performance.now();
    const { paint, e2e } = await afterPaint();
    return {
      proc: patched - start,
      e2e: e2e - t0,
      paint: paint - t0,
      owner: ownerMs,
      dom: patched - applying,
      firstFrame: frame.time >= patched && frame.time - t0 <= 17.5,
    };
  }

  apply(patch: SourcePatch): void {
    this.view.dispatch({
      changes: { from: patch.from, to: patch.to, insert: patch.insert },
      selection: { anchor: patch.cursor },
      annotations: fromOwner.of(true),
      scrollIntoView: true,
    });
  }

  setCursor(at: number): void {
    this.view.focus();
    this.view.dispatch({
      selection: { anchor: at },
      effects: EditorView.scrollIntoView(at, { y: 'center' }),
    });
  }

  /** Replaces the whole text (after edits in the facade). */
  setText(text: string): void {
    this.view.setState(this.stateOf(text));
  }

  text(): string {
    return this.view.state.doc.toString();
  }

  /** Dispatches `input` as a synthetic `beforeinput` and waits for its sample. */
  async drive(inputType: string, data: string | null = null): Promise<Sample | null> {
    const e = new InputEvent('beforeinput', {
      inputType,
      data,
      bubbles: true,
      cancelable: true,
    });
    this.view.contentDOM.dispatchEvent(e);
    return this.last;
  }

  destroy(): void {
    this.view.destroy();
  }
}
