// UC-022 Replace in Workspace (FR-074): the window's side of the replace
// of the workspace, in the search overlay. It sends the search text, the
// replacement and the search switches (`replace_set`), takes the places the
// backend found (event `replace`, page by page), keeps the entry chosen with
// the keyboard, and leaves notes and places out (`replace_leave_out`).
// Replacing itself goes through the shell (`Shell.confirmReplace`), after
// the window's edits reached the backend (ADR-025).
//
// No domain logic lives here: what matches, what is written, and which
// places were left out are the backend's; the window shows them and sends
// what the user did.

import {
  commands,
  type ChosenView,
  type ReplaceNoteView,
  type ReplaceResultView,
  type ReplaceView,
  type SearchSwitches,
} from './bindings';

/** An entry of the list: a note (`place` null) or one of its places. */
export type ReplaceEntry = { note: number; place: number | null };

/** How close to the end of the loaded notes the next page is asked for. */
const MORE_BEFORE_END = 20;

/** The entries of `notes`: each note, then its places. */
export function replaceEntries(notes: readonly ReplaceNoteView[]): ReplaceEntry[] {
  const entries: ReplaceEntry[] = [];
  notes.forEach((found, note) => {
    entries.push({ note, place: null });
    found.places.forEach((_, place) => entries.push({ note, place }));
  });
  return entries;
}

export class ReplaceState {
  /** The replacement as typed. */
  replacement = $state('');
  /** The number of the newest `replace_set` the backend answered. */
  asked = $state(0);
  /** A2: what is wrong with the search text or the replacement. */
  problem = $state<string | null>(null);
  /** The places shown: the newest that answer the newest request. */
  result = $state.raw<ReplaceResultView | null>(null);
  /** The notes of `result`, as many pages as are loaded. */
  notes = $state.raw<ReplaceNoteView[]>([]);
  /** BR-008: the entry chosen with the keyboard; `null` while none is. */
  cursor = $state<number | null>(null);
  /** Counts the requests that the replace field take the keyboard. */
  focusAsked = $state(0);
  /** A page is being loaded. */
  paging = $state(false);
  /** Replacing runs. */
  replacing = $state(false);
  /** Why the last request could not be sent. */
  error = $state<string | null>(null);
  /** Step 3: what is chosen now, as the backend counts it. */
  chosen = $state.raw<ChosenView | null>(null);
  /** Step 6: what the last replacement did, told in the overlay. */
  said = $state('');

  /** The requests go to the backend one after the other, in the order typed. */
  #sending: Promise<unknown> = Promise.resolve();
  /** How many requests are on their way. */
  #waiting = 0;
  /** The last view the backend sent, kept until its request is answered. */
  #view: ReplaceView | null = null;
  #fail: (command: string, message: string) => void;

  constructor(fail: (command: string, message: string) => void) {
    this.#fail = fail;
  }

  /** The entries the keyboard moves through, of the notes loaded. */
  get entries(): ReplaceEntry[] {
    return replaceEntries(this.notes);
  }

  /** Waiting for the places of the text held now. */
  get searching(): boolean {
    return this.#waiting > 0 || (this.result !== null && this.result.asked !== this.asked);
  }

  /** Whether the result holds more notes than are loaded. */
  get hasMore(): boolean {
    return this.result !== null && this.notes.length < this.result.listed;
  }

  /**
   * UC-022 steps 1–2: the search text, the replacement or a switch changed;
   * the places are found again (BR-007: typing is never held up).
   */
  set(text: string, replacement: string, switches: SearchSwitches): void {
    this.replacement = replacement;
    this.cursor = null;
    this.said = '';
    if (text === '') {
      this.result = null;
      this.notes = [];
      this.problem = null;
    }
    this.#waiting += 1;
    this.#sending = this.#sending.then(async () => {
      try {
        const answer = await commands.replaceSet(text, replacement, switches);
        if (answer.status === 'error') {
          this.error = answer.error.message;
          this.#fail('replace_set', answer.error.message);
        } else {
          this.error = null;
          this.asked = answer.data;
        }
      } catch (error) {
        this.error = String(error);
        this.#fail('replace_set', String(error));
      } finally {
        this.#waiting -= 1;
      }
      if (this.#view !== null) {
        this.take(this.#view);
      }
    });
  }

  /**
   * The backend's places changed (event `replace`): they are shown when
   * they answer the newest request; otherwise the ones shown stay.
   */
  take(view: ReplaceView): void {
    this.#view = view;
    if (view.asked !== this.asked || this.#waiting > 0) {
      return;
    }
    this.problem = view.problem;
    const result = view.result;
    if (result === null) {
      if (view.problem !== null) {
        this.result = null;
        this.notes = [];
      }
      return;
    }
    if (result.asked !== this.asked || result.serial === this.result?.serial) {
      return;
    }
    // A4: the same request found again; the pages loaded are loaded again.
    const keep = this.result?.asked === result.asked ? this.notes.length : 0;
    this.result = result;
    this.notes = result.list;
    this.chosen = result.chosen;
    if (keep > this.notes.length) {
      void this.more(keep);
    }
    if (this.cursor !== null) {
      this.cursor = Math.min(this.cursor, Math.max(this.entries.length - 1, 0));
    }
  }

  /** The next page of notes, or pages until `upTo` notes are loaded. */
  async more(upTo = 0): Promise<void> {
    const result = this.result;
    if (!result || this.paging || !this.hasMore) {
      return;
    }
    this.paging = true;
    try {
      do {
        const answer = await commands.replacePage(result.serial, this.notes.length);
        if (answer.status === 'error') {
          this.#fail('replace_page', answer.error.message);
          return;
        }
        if (answer.data === null || answer.data.length === 0 || this.result !== result) {
          return;
        }
        this.notes = [...this.notes, ...answer.data];
      } while (this.notes.length < upTo && this.hasMore);
    } catch (error) {
      this.#fail('replace_page', String(error));
    } finally {
      this.paging = false;
    }
  }

  /** BR-008: Down (`1`) or Up (`-1`) moves the chosen entry, stopping at the ends. */
  move(by: 1 | -1): void {
    const last = this.entries.length - 1;
    if (last < 0) {
      return;
    }
    this.cursor = this.cursor === null ? 0 : Math.min(Math.max(this.cursor + by, 0), last);
    if (this.cursor >= last - MORE_BEFORE_END) {
      void this.more();
    }
  }

  /** The note of the chosen entry, if one is chosen. */
  get chosenNote(): ReplaceNoteView | null {
    const entry = this.cursor === null ? undefined : this.entries[this.cursor];
    return entry ? (this.notes[entry.note] ?? null) : null;
  }

  /**
   * Step 3: leaves the note `n` (and with `p` its place `p`) out, or takes
   * it in again; the list shows it at once, the backend keeps it.
   */
  toggle(n: number, p: number | null): void {
    const result = this.result;
    const found = this.notes[n];
    if (!result || !found) {
      return;
    }
    let out: boolean;
    let at: number | null = null;
    let next: ReplaceNoteView;
    if (p === null) {
      out = !found.leftOut;
      next = { ...found, leftOut: out };
    } else {
      const place = found.places[p];
      if (!place) {
        return;
      }
      out = !place.leftOut;
      at = place.at;
      next = {
        ...found,
        places: found.places.map((each, i) => (i === p ? { ...each, leftOut: out } : each)),
      };
    }
    this.notes = this.notes.map((each, i) => (i === n ? next : each));
    void commands.replaceLeaveOut(result.serial, found.note, at, out).then(
      (answer) => {
        if (answer.status === 'error') {
          this.#fail('replace_leave_out', answer.error.message);
        } else if (answer.data !== null && this.result === result) {
          this.chosen = answer.data;
        }
      },
      (error: unknown) => this.#fail('replace_leave_out', String(error)),
    );
  }

  /** Step 3, BR-008: Space leaves the chosen entry out, or takes it in again. */
  toggleChosen(): void {
    const entry = this.cursor === null ? undefined : this.entries[this.cursor];
    if (entry) {
      this.toggle(entry.note, entry.place);
    }
  }

  /** A8: the replace closed; the backend stops finding its places. */
  close(): void {
    this.cursor = null;
    this.chosen = null;
    this.said = '';
    this.result = null;
    this.notes = [];
    this.problem = null;
    this.#view = null;
    this.#sending = this.#sending.then(async () => {
      try {
        await commands.replaceClose();
      } catch (error) {
        this.#fail('replace_close', String(error));
      }
    });
  }

  /** M-21.3: another workspace: the places of the one before go. */
  workspaceChanged(): void {
    this.result = null;
    this.notes = [];
    this.cursor = null;
    this.#view = null;
  }
}
