// UC-008 Find Note, inventory M-03.2 and M-13: the window's side of the
// search, as egui keeps it in its app state (`search_text`, `search_open`,
// `search_cursor`, `find`): the search overlay with its text, the found
// notes of the newest result as the backend sends them (event `search`,
// page by page, A-89), the entry chosen with the keyboard, and the find field
// of the active note. Nothing of it is stored (BR-010, M-22.4).
//
// The rules are egui's:
// - a result is taken only when it was searched for the text the field
//   holds when it comes; until then the one before stays, and before the
//   first one the overlay says "Searching…";
// - another text chooses no entry yet; Down and Up move through the found
//   notes and their lines, stopping at the ends;
// - closing the overlay (Escape, a click outside, the magnifier, Enter on
//   an entry) empties the field, which ends the search and its marks (A2).
//
// No domain logic lives here: the search itself runs in the backend, the
// marks in the editor cores (`editor/sync.ts`, Q-38).

import {
  commands,
  type FoundNoteView,
  type FoundPlace,
  type SearchResultView,
  type SearchView,
} from './bindings';
import { focusFallback } from './focus';

/** An entry of the found list: a note (`line` null) or one of its lines. */
export type FoundEntry = { note: number; line: number | null };

/** M-13.4: the find field of the active note (UC-008 A6). */
export type FindField = { text: string };

/** What opening an entry needs: the note and the place to select. */
export type FoundChoice = { note: string; place: FoundPlace | null };

/** How close to the end of the loaded notes the next page is asked for. */
const MORE_BEFORE_END = 10;

/** The entries of `notes`: each note, then its lines. */
export function entriesOf(notes: readonly FoundNoteView[]): FoundEntry[] {
  const entries: FoundEntry[] = [];
  notes.forEach((found, note) => {
    entries.push({ note, line: null });
    found.lines.forEach((_, line) => entries.push({ note, line }));
  });
  return entries;
}

/** egui's `choice`: a note opens at its first line found, a line at its word. */
export function choiceOf(found: FoundNoteView, line: number | null): FoundChoice {
  const place = line === null ? found.place : (found.lines[line]?.place ?? null);
  return { note: found.note, place };
}

export class SearchState {
  /** M-03.2: the overlay is open. */
  open = $state(false);
  /** The field's text, as typed. */
  text = $state('');
  /** The result shown: the newest that fit the field's text when it came. */
  result = $state.raw<SearchResultView | null>(null);
  /** The found notes of `result`, as many pages as are loaded. */
  notes = $state.raw<FoundNoteView[]>([]);
  /** BR-007: the entry chosen with the keyboard; `null` while none is. */
  cursor = $state<number | null>(null);
  /** Counts the requests that the overlay's field take the keyboard. */
  focusAsked = $state(0);
  /** M-13.4: the find field of the active note, while it is open. */
  find = $state.raw<FindField | null>(null);
  /** Counts the requests that the find field take the keyboard. */
  findFocusAsked = $state(0);
  /** UX review F9: why the last text could not be searched; shown in the overlay. */
  error = $state<string | null>(null);
  /** UX review F9: where the keyboard was when the overlay opened. */
  #before: HTMLElement | null = null;

  /** The texts go to the backend one after the other, in the order typed. */
  #sending: Promise<unknown> = Promise.resolve();
  #fail: (command: string, message: string) => void;

  /** `fail` reports a failed command in the status bar. */
  constructor(fail: (command: string, message: string) => void) {
    this.#fail = fail;
  }

  /** The field's text without the spaces around it: what is searched. */
  get searched(): string {
    return this.text.trim();
  }

  /** The entries the keyboard moves through, of the notes loaded. */
  get entries(): FoundEntry[] {
    return entriesOf(this.notes);
  }

  /** M-03.2, BR-007: Ctrl+Shift+F, or the magnifier: open, the field takes the keyboard. */
  show(): void {
    if (!this.open) {
      const active = document.activeElement;
      this.#before = active instanceof HTMLElement && active !== document.body ? active : null;
    }
    this.open = true;
    this.focusAsked++;
  }

  /** M-03.2: the magnifier opens the overlay, or closes it. */
  toggle(): void {
    if (this.open) {
      this.close();
    } else {
      this.show();
    }
  }

  /**
   * A2: the overlay closes; its text goes, and with it the search and its
   * marks. With `restore` (UX review F9), the keyboard goes back where it
   * was when the overlay opened, unless it went somewhere else meanwhile (a
   * click on a button outside); when that place is gone, to the overview.
   */
  close(restore = true): void {
    this.open = false;
    if (this.text !== '') {
      this.setText('');
    }
    this.cursor = null;
    this.error = null;
    const before = this.#before;
    this.#before = null;
    if (!restore) {
      return;
    }
    // After the press that closed it has moved the keyboard, if it does.
    setTimeout(() => {
      const now = document.activeElement;
      if (now !== null && now !== document.body) {
        return;
      }
      if (before?.isConnected) {
        before.focus();
      } else {
        focusFallback();
      }
    }, 0);
  }

  /** UC-008 step 1: the user typed; the search runs again (BR-005). */
  setText(text: string): void {
    this.text = text;
    // Another text gives other entries: none is chosen yet.
    this.cursor = null;
    if (text.trim() === '') {
      this.result = null;
      this.notes = [];
    }
    const sent = text;
    this.#sending = this.#sending.then(async () => {
      let failure: string | null = null;
      try {
        const answer = await commands.searchSet(sent);
        if (answer.status === 'error') {
          failure = answer.error.message;
        }
      } catch (error) {
        failure = String(error);
      }
      if (failure !== null) {
        this.#fail('search_set', failure);
      }
      // Only the answer to the newest text says whether the search runs.
      if (sent === this.text) {
        this.error = failure;
      }
    });
  }

  /**
   * The backend's search changed (event `search`): its result is taken
   * when it was searched for the field's text; otherwise the one shown
   * stays, as in egui.
   */
  take(view: SearchView): void {
    const result = view.result;
    if (!result || result.text !== this.searched || this.searched === '') {
      return;
    }
    if (result.serial === this.result?.serial) {
      return;
    }
    // The same text searched again (A3): the pages loaded are loaded again,
    // so the list does not shrink under the scrolled position.
    const keep = this.result?.text === result.text ? this.notes.length : 0;
    this.result = result;
    this.notes = result.notes;
    this.error = null;
    if (keep > this.notes.length) {
      void this.more(keep);
    }
    if (this.cursor !== null) {
      this.cursor = Math.min(this.cursor, Math.max(this.entries.length - 1, 0));
    }
  }

  /** BR-007: Down (`1`) or Up (`-1`) moves the chosen entry, stopping at the ends. */
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

  /** BR-007: what Enter opens: the chosen entry, else the first. */
  chosen(): FoundChoice | null {
    const entries = this.entries;
    if (entries.length === 0) {
      return null;
    }
    const entry = entries[Math.min(this.cursor ?? 0, entries.length - 1)];
    return choiceOf(this.notes[entry.note], entry.line);
  }

  /**
   * UX review F9 (A-96): the result shown was searched for another text than
   * the field holds; a newer one is on its way.
   */
  get stale(): boolean {
    return this.result !== null && this.result.text !== this.searched;
  }

  /** Whether the result holds more notes than are loaded. */
  get hasMore(): boolean {
    return this.result !== null && this.notes.length < this.result.total;
  }

  /** A page is being loaded. */
  paging = $state(false);

  /**
   * The next page of found notes, or pages until `upTo` notes are loaded;
   * nothing when a newer result came.
   */
  async more(upTo = 0): Promise<void> {
    const result = this.result;
    if (!result || this.paging || !this.hasMore) {
      return;
    }
    this.paging = true;
    try {
      do {
        const answer = await commands.searchPage(result.serial, this.notes.length);
        if (answer.status === 'error') {
          this.#fail('search_page', answer.error.message);
          return;
        }
        if (answer.data === null || answer.data.length === 0 || this.result !== result) {
          return;
        }
        this.notes = [...this.notes, ...answer.data];
      } while (this.notes.length < upTo && this.hasMore);
    } catch (error) {
      this.#fail('search_page', String(error));
    } finally {
      this.paging = false;
    }
  }

  /** M-13.4, UC-008 A6: Ctrl+F opens the find field with `text`, or takes the keyboard to it. */
  openFind(text: string): void {
    this.find = { text };
    this.findFocusAsked++;
  }

  /** M-13.4: the find field's text changed. */
  setFindText(text: string): void {
    if (this.find) {
      this.find = { text };
    }
  }

  /** M-13.4: Escape or "×" closes the find field. */
  closeFind(): void {
    this.find = null;
  }

  /** M-21.3: another workspace: the find field closes (egui); the overlay keeps its text. */
  workspaceChanged(): void {
    this.find = null;
    this.result = null;
    this.notes = [];
    this.cursor = null;
  }
}
