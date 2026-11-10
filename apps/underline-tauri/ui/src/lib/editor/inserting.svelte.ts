// Feature F6: what the link and image dialogs and dropped files put into the
// active note (UC-005 A2, A9 to A13, A19, M-10.3 to M-10.5), as egui's
// `show_tool_dialog`, `answer_tool` and `drop_items` do it. The backend
// copies files and makes links to notes (`ed_embed`, `ed_drop`,
// `ed_note_link`); the view shown puts the answer in at its cursor through
// the window's editor core.
//
// - Link and Image of the toolbar (and Ctrl+K, Ctrl+Shift+I) ask first: the
//   core answers with a request, which opens the dialog (`ask`); a new
//   request replaces the dialog open.
// - An answer goes into the note shown when it comes; the dialog closes,
//   and the keyboard goes back into the text. A file the backend refused
//   closes the dialog, the status bar says why (egui, Q-53).
// - A file over 10 MB turns the image dialog into the question "Copy" /
//   "Link to its place" (A19). While the backend copies, the dialog says so
//   and cannot be closed.
// - Dropping (A-65): while files are dragged over the window, the status
//   says so, and the hint goes when they leave. A drop puts the cursor under
//   the pointer when it is over the text, then the items go in one after
//   the other into the note shown at the drop. A file over 10 MB stops the
//   items there and asks; after the answer the rest follows, in order, at
//   the place the drop reached (when the note did not change meanwhile),
//   and a further large file asks in its turn. With no note shown the
//   status says so (A-69).

import {
  commands,
  type DragName,
  type EmbedAnswer,
  type Insertion,
  type ToolName,
  type UiError,
} from '../bindings';
import type { CursorMark } from '../facade/active';
import type { DropEvent } from './drop';

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: UiError };

async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const answer = await result;
  if (answer.status === 'error') {
    throw answer.error;
  }
  return answer.data;
}

function message(error: unknown): string {
  return typeof error === 'object' && error !== null && 'message' in error
    ? String(error.message)
    : String(error);
}

/** What a view shown does for the dialogs and drops. */
export interface InsertEditor {
  /** Puts `insertion` in at the cursor; resolves once it is shown. */
  insert(insertion: Insertion): Promise<void>;
  /** The cursor to the place under the point (CSS pixels); false off the text. */
  placeAt(x: number, y: number): boolean;
  /** Where the cursor is, once the inputs before it are in; `null` once gone. */
  mark(): Promise<CursorMark | null>;
  /** The cursor back to `mark`, when the text did not change since. */
  back(mark: CursorMark): void;
  focus(): void;
}

/** The note shown now and its view. */
export type InsertTarget = { note: string; editor: InsertEditor };

/** A19: a file over 10 MB, with egui's question. */
export type LargeFile = { path: string; question: string };

/** The dialog of Link or Image. */
export type ToolDialog = { kind: 'link' } | { kind: 'image'; large: LargeFile | null };

/** A19: the note a large file goes into, and what was dropped after it. */
type Waiting = {
  target: InsertTarget;
  /** Where the drop had got to; `null` for the image dialog. */
  mark: CursorMark | null;
  /** The items dropped after the large file, in order. */
  rest: EmbedAnswer[];
};

export class Inserting {
  /** The dialog open, if any. */
  dialog = $state.raw<ToolDialog | null>(null);
  /** The backend works on the dialog's answer. */
  busy = $state(false);

  readonly #target: () => InsertTarget | null;
  readonly #onError: (message: string) => void;
  /** The large file asked about: where it goes, and the drop's rest. */
  #waiting: Waiting | null = null;

  /**
   * `target` is the note shown and its view, asked when an answer comes;
   * `onError` says a failed command in the status bar.
   */
  constructor(target: () => InsertTarget | null, onError: (message: string) => void) {
    this.#target = target;
    this.#onError = onError;
  }

  /** UC-005 A2: Link or Image asks first; a waiting drop goes on without the large file. */
  ask(tool: ToolName): void {
    const waiting = this.#waiting;
    this.#waiting = null;
    this.dialog = tool === 'image' ? { kind: 'image', large: null } : { kind: 'link' };
    if (waiting && waiting.rest.length > 0) {
      void this.#goOn(waiting);
    }
  }

  /**
   * Cancel or Escape: the dialog closes; a drop waiting on a large file
   * goes on without it. Not while the backend works on an answer.
   */
  close(): void {
    if (this.busy) {
      return;
    }
    const waiting = this.#waiting;
    this.#waiting = null;
    this.dialog = null;
    if (waiting) {
      void this.#goOn(waiting);
    }
  }

  /** M-10.3: "Insert": a link to the web address `address`, without a label. */
  address(address: string): Promise<void> {
    return this.#insert({ kind: 'link', address, label: null });
  }

  /** M-10.3: the note `note` was chosen: a relative link with its title. */
  note(note: string): Promise<void> {
    return this.#busy(this.#target(), async (target) => {
      const link = await unwrap(commands.edNoteLink(target.note, note));
      await this.#insert(link);
    });
  }

  /** M-10.4: "Insert": the image file at `path`. */
  file(path: string): Promise<void> {
    return this.#busy(this.#target(), async (target) => {
      const answer = await unwrap(commands.edEmbed(target.note, path, false));
      await this.#takeAll([answer], target, null, true);
    });
  }

  /** A19 "Copy": the large file is copied all the same, into its note. */
  copyLarge(): Promise<void> {
    const large = this.#large();
    const waiting = this.#waiting;
    if (!large || !waiting) {
      return Promise.resolve();
    }
    return this.#busy(waiting.target, async (target) => {
      const answer = await unwrap(commands.edEmbed(target.note, large.path, true));
      this.#waiting = null;
      this.dialog = null;
      await this.#takeAll([answer, ...waiting.rest], target, waiting.mark, true);
    });
  }

  /** A19 "Link to its place": a link to the large file where it is. */
  async linkPlace(): Promise<void> {
    const large = this.#large();
    const waiting = this.#waiting;
    if (!large || !waiting) {
      return;
    }
    this.#waiting = null;
    this.dialog = null;
    const link: EmbedAnswer = {
      kind: 'insert',
      insertion: { kind: 'link', address: large.path, label: null },
    };
    await this.#takeAll([link, ...waiting.rest], waiting.target, waiting.mark, true);
  }

  /** M-10.5, UC-005 A9 to A13: files dragged over the window, or dropped. */
  async drop(event: DropEvent): Promise<void> {
    try {
      if (event.type === 'enter') {
        await this.#drag('over');
        return;
      }
      if (event.type === 'leave') {
        await this.#drag('left');
        return;
      }
      const target = this.#target();
      if (!target) {
        await this.#drag('noNote');
        return;
      }
      // Off the text the cursor stays where it is, as egui's
      // `place_cursor_at`, and the items go in there.
      target.editor.placeAt(event.x, event.y);
      const answers = await unwrap(commands.edDrop(target.note, event.paths));
      await this.#takeAll(answers, target, null, false);
    } catch (error) {
      this.#onError(message(error));
    }
  }

  #drag(state: DragName): Promise<null> {
    return unwrap(commands.edDrag(state));
  }

  /** The large file asked about, if the dialog asks. */
  #large(): LargeFile | null {
    return this.dialog?.kind === 'image' ? this.dialog.large : null;
  }

  /** A drop waiting on a large file goes on with the items after it. */
  async #goOn(waiting: Waiting): Promise<void> {
    try {
      await this.#takeAll(waiting.rest, waiting.target, waiting.mark, false);
    } catch (error) {
      this.#onError(message(error));
    }
  }

  /**
   * What the backend made of files, in order, into `target`: each goes in,
   * is asked about (and the rest waits), or was refused (the status says
   * why). `mark` is where the items before reached, restored first;
   * `focus` puts the keyboard into the text after (a dialog's answer).
   */
  async #takeAll(
    answers: EmbedAnswer[],
    target: InsertTarget,
    mark: CursorMark | null,
    focus: boolean,
  ): Promise<void> {
    if (mark) {
      target.editor.back(mark);
    }
    for (const [index, answer] of answers.entries()) {
      if (answer.kind === 'insert') {
        await target.editor.insert(answer.insertion);
      } else if (answer.kind === 'large') {
        this.#waiting = {
          target,
          mark: await target.editor.mark(),
          rest: answers.slice(index + 1),
        };
        this.dialog = { kind: 'image', large: { path: answer.path, question: answer.question } };
        return;
      } else if (focus && this.dialog?.kind === 'image') {
        // Refused: egui closes the dialog; the status bar says why.
        this.dialog = null;
      }
    }
    if (focus) {
      this.dialog = null;
      target.editor.focus();
    }
  }

  /** An answer of the dialog goes into the note shown; the dialog closes. */
  async #insert(insertion: Insertion): Promise<void> {
    this.dialog = null;
    const target = this.#target();
    if (target) {
      await target.editor.insert(insertion);
      target.editor.focus();
    }
  }

  /** Runs `work` for `target` while the dialog waits; without one the dialog closes. */
  async #busy(
    target: InsertTarget | null,
    work: (target: InsertTarget) => Promise<void>,
  ): Promise<void> {
    if (!target) {
      this.dialog = null;
      return;
    }
    this.busy = true;
    try {
      await work(target);
    } catch (error) {
      this.#onError(message(error));
    } finally {
      this.busy = false;
    }
  }
}
