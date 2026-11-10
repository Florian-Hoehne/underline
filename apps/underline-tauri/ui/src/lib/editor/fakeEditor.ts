// A stand-in for the backend's editor (`underline_editor_core::SourceEditor`)
// for the mock backend, so the source view can be typed in a plain browser
// (plan section 10). It follows the same rules in UTF-16 units: one char or
// line at a time whatever the modifiers, Up and Down keep the column the
// cursor has, Tab inserts four spaces, Ctrl/Cmd+A selects all, and undo
// takes back one input at a time (the backend merges a word into one step;
// its rules are tested in Rust).

import type { EditorInput, EditorPatch, EditorState, TextChange } from '../bindings';

type Step = { from: number; removed: string; inserted: string };

export class FakeEditor {
  text: string;
  anchor = 0;
  head = 0;
  rev = 0;
  #undo: Step[] = [];
  #redo: Step[] = [];

  constructor(
    readonly note: string,
    text: string,
    readonly mac = false,
  ) {
    this.text = text;
  }

  state(): EditorState {
    return {
      note: this.note,
      rev: this.rev,
      text: this.text,
      anchor: this.anchor,
      head: this.head,
    };
  }

  selection(): [number, number] {
    return [Math.min(this.anchor, this.head), Math.max(this.anchor, this.head)];
  }

  selected(): string {
    const [from, to] = this.selection();
    return this.text.slice(from, to);
  }

  select(rev: number, anchor: number, head: number): void {
    if (rev === this.rev) {
      this.anchor = Math.min(anchor, this.text.length);
      this.head = Math.min(head, this.text.length);
    }
  }

  /** The patch for `input`; `unstored` comes from the caller. */
  input(input: EditorInput): EditorPatch {
    const base = this.rev;
    if (input.kind === 'text') {
      return this.#patch(base, this.#replaceSelection(input.text));
    }
    const command = this.mac ? input.meta : input.ctrl;
    const [lineStart, lineEnd] = this.#line(this.head);
    const column = this.head - lineStart;
    let target: number | null = null;
    switch (input.key) {
      case 'Backspace':
        if (this.anchor === this.head && this.head > 0) {
          this.anchor = this.head - 1 - (this.#lowSurrogateBefore(this.head) ? 1 : 0);
        }
        return this.#patch(base, this.#replaceSelection(''));
      case 'Delete':
        if (this.anchor === this.head && this.head < this.text.length) {
          this.anchor = this.head + 1 + (this.#highSurrogateAt(this.head) ? 1 : 0);
        }
        return this.#patch(base, this.#replaceSelection(''));
      case 'Enter':
        return this.#patch(base, this.#replaceSelection('\n'));
      case 'Tab':
        return this.#patch(base, this.#replaceSelection('    '));
      case 'Z':
        if (command) {
          return this.#patch(base, input.shift ? this.#redoStep() : this.#undoStep());
        }
        return this.#patch(base, null, false);
      case 'Y':
        return this.#patch(base, command ? this.#redoStep() : null, false);
      case 'A':
        if (command) {
          this.anchor = 0;
          this.head = this.text.length;
        }
        return this.#patch(base, null, command);
      case 'ArrowLeft':
        target = Math.max(0, this.head - 1 - (this.#lowSurrogateBefore(this.head) ? 1 : 0));
        break;
      case 'ArrowRight':
        target = Math.min(
          this.text.length,
          this.head + 1 + (this.#highSurrogateAt(this.head) ? 1 : 0),
        );
        break;
      case 'ArrowUp':
        target =
          lineStart === 0
            ? this.#atColumn(0, column)
            : this.#atColumn(this.#line(lineStart - 1)[0], column);
        break;
      case 'ArrowDown':
        target =
          lineEnd >= this.text.length
            ? this.#atColumn(lineStart, column)
            : this.#atColumn(lineEnd + 1, column);
        break;
      case 'PageUp':
      case 'PageDown': {
        let start = lineStart;
        for (let i = 0; i < input.page; i++) {
          const next =
            input.key === 'PageUp'
              ? start === 0
                ? null
                : this.#line(start - 1)[0]
              : this.#line(start)[1] >= this.text.length
                ? null
                : this.#line(start)[1] + 1;
          if (next === null) {
            break;
          }
          start = next;
        }
        target = this.#atColumn(start, column);
        break;
      }
      case 'Home':
        target = command ? 0 : lineStart;
        break;
      case 'End':
        target = command ? this.text.length : lineEnd;
        break;
    }
    if (target === null) {
      return this.#patch(base, null, false);
    }
    this.head = target;
    if (!input.shift) {
      this.anchor = target;
    }
    return this.#patch(base, null, true);
  }

  /** M-08.6: cutting takes the selection. */
  cut(): { text: string; patch: EditorPatch } {
    const base = this.rev;
    const text = this.selected();
    return { text, patch: this.#patch(base, text === '' ? null : this.#replaceSelection('')) };
  }

  /** The patch of an input that changed nothing. */
  unchanged(): EditorPatch {
    return this.#patch(this.rev, null, false);
  }

  /** Inserts the pasted `text`. */
  paste(text: string): EditorPatch {
    return this.#patch(this.rev, this.#replaceSelection(text));
  }

  #line(at: number): [number, number] {
    const start = this.text.lastIndexOf('\n', at - 1) + 1;
    const newline = this.text.indexOf('\n', at);
    return [start, newline === -1 ? this.text.length : newline];
  }

  #atColumn(lineStart: number, column: number): number {
    const [, end] = this.#line(lineStart);
    return Math.min(lineStart + column, end);
  }

  #lowSurrogateBefore(at: number): boolean {
    const code = this.text.charCodeAt(at - 1);
    return at > 1 && code >= 0xdc00 && code <= 0xdfff;
  }

  #highSurrogateAt(at: number): boolean {
    const code = this.text.charCodeAt(at);
    return code >= 0xd800 && code <= 0xdbff;
  }

  #replaceSelection(inserted: string): TextChange | null {
    const [from, to] = this.selection();
    if (from === to && inserted === '') {
      return null;
    }
    const step = { from, removed: this.text.slice(from, to), inserted };
    this.#undo.push(step);
    this.#redo = [];
    return this.#applyStep(step);
  }

  #applyStep(step: Step): TextChange {
    this.text =
      this.text.slice(0, step.from) +
      step.inserted +
      this.text.slice(step.from + step.removed.length);
    this.head = this.anchor = step.from + step.inserted.length;
    this.rev += 1;
    return { from: step.from, to: step.from + step.removed.length, insert: step.inserted };
  }

  #undoStep(): TextChange | null {
    const step = this.#undo.pop();
    if (!step) {
      return null;
    }
    this.#redo.push(step);
    return this.#applyStep({ from: step.from, removed: step.inserted, inserted: step.removed });
  }

  #redoStep(): TextChange | null {
    const step = this.#redo.pop();
    if (!step) {
      return null;
    }
    this.#undo.push(step);
    return this.#applyStep(step);
  }

  #patch(base: number, change: TextChange | null, moved = change !== null): EditorPatch {
    return {
      base,
      rev: this.rev,
      changes: change ? [change] : [],
      anchor: this.anchor,
      head: this.head,
      scroll: moved,
      unstored: false,
    };
  }
}
