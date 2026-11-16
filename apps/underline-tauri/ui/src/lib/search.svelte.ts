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
  type FindSpec,
  type FoundNoteView,
  type FoundPlace,
  type SearchResultView,
  type SearchSwitches,
  type SearchView,
} from './bindings';
import { focusFallback } from './focus';
import { ReplaceState } from './replace.svelte';

/** An entry of the found list: a note (`line` null) or one of its lines. */
export type FoundEntry = { note: number; line: number | null };

/**
 * M-13.4: the find field of the active note (UC-008 A6), open for
 * replacing or not (UC-005 A20).
 */
export type FindField = { text: string; replacing: boolean };

/** FR-075: the name of a search switch. */
export type SwitchName = keyof SearchSwitches;

/** UC-008 BR-011: every search switch off. */
export const NO_SWITCHES: SearchSwitches = { matchCase: false, wholeWord: false, regex: false };

/**
 * UC-008 BR-011, UC-005 BR-017, UX: Alt+C, Alt+W and Alt+R switch the
 * search switches in a search field. The physical key counts (`code`), so
 * Option with a letter on macOS, which types a character, switches too
 * (OQ-195).
 */
export function switchKey(event: KeyboardEvent): SwitchName | null {
  if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
    return null;
  }
  switch (event.code) {
    case 'KeyC':
      return 'matchCase';
    case 'KeyW':
      return 'wholeWord';
    case 'KeyR':
      return 'regex';
    default:
      return null;
  }
}

/** `switches` with `name` switched. */
export function switched(switches: SearchSwitches, name: SwitchName): SearchSwitches {
  return { ...switches, [name]: !switches[name] };
}

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
  /** FR-075, UC-008 A7: the search switches of the overlay (BR-010: not stored). */
  switches = $state<SearchSwitches>(NO_SWITCHES);
  /** UC-008 A9: what is wrong with the text, as the backend says. */
  problem = $state<string | null>(null);
  /** FR-075: the search switches of the find field, its own (BR-010). */
  findSwitches = $state<SearchSwitches>(NO_SWITCHES);
  /** UC-005 A20: the replacement of the find field, kept while it closes. */
  findReplacement = $state('');
  /** UC-022: the overlay replaces in the workspace (UC-008 A10). */
  replacing = $state(false);
  /** UC-022: the places of the replace of the workspace. */
  readonly replace: ReplaceState;
  /** UX review F9: where the keyboard was when the overlay opened. */
  #before: HTMLElement | null = null;

  /** The texts go to the backend one after the other, in the order typed. */
  #sending: Promise<unknown> = Promise.resolve();
  #fail: (command: string, message: string) => void;

  /** `fail` reports a failed command in the status bar. */
  constructor(fail: (command: string, message: string) => void) {
    this.#fail = fail;
    this.replace = new ReplaceState(fail);
  }

  /**
   * UC-008 BR-008, UC-022: what the overlay marks in the notes: its text
   * with its switches, one text while it replaces.
   */
  get overlaySpec(): FindSpec {
    return {
      text: this.replacing ? this.text : this.searched,
      switches: this.switches,
      oneText: this.replacing,
      shownOnly: false,
    };
  }

  /** UC-008 A7: a switch of the overlay changed; it searches again. */
  toggleSwitch(name: SwitchName): void {
    this.switches = switched(this.switches, name);
    this.setText(this.text);
  }

  /**
   * UC-022 step 1, BR-008, UC-008 A10: Ctrl+Shift+H, or "Replace" in the
   * overlay: the overlay opens for replacing, with the text and the
   * switches of the search taken over; the replace field takes the
   * keyboard when the search text is there already.
   */
  showReplace(): void {
    this.#openOverlay();
    if (!this.replacing) {
      this.replacing = true;
      this.cursor = null;
      this.#sendReplace();
    }
    if (this.text === '') {
      this.focusAsked++;
    } else {
      this.replace.focusAsked++;
    }
  }

  /**
   * UC-022 A8: the replace closes without replacing; the overlay shows the
   * search of the workspace with the search text.
   */
  stopReplacing(): void {
    if (!this.replacing) {
      return;
    }
    this.replacing = false;
    this.replace.close();
    this.setText(this.text);
    this.focusAsked++;
  }

  /** UC-022 step 1: the replacement typed. */
  setReplacement(text: string): void {
    this.replace.replacement = text;
    this.#sendReplace();
  }

  /** UC-022 step 2: the places are found again for the text, replacement and switches. */
  #sendReplace(): void {
    this.replace.set(this.text, this.replace.replacement, this.switches);
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
    this.#openOverlay();
    this.focusAsked++;
  }

  /** The overlay opens, remembering where the keyboard was. */
  #openOverlay(): void {
    if (!this.open) {
      const active = document.activeElement;
      this.#before = active instanceof HTMLElement && active !== document.body ? active : null;
    }
    this.open = true;
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
    if (this.replacing) {
      this.replacing = false;
      this.replace.close();
    }
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
      this.problem = null;
    }
    if (this.replacing) {
      // UC-022: the places of the replace are found instead.
      this.#sendReplace();
      return;
    }
    const sent = text;
    const switches = this.switches;
    this.#sending = this.#sending.then(async () => {
      let failure: string | null = null;
      try {
        const answer = await commands.searchSet(sent, switches);
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
    if (view.text === this.searched && sameSwitches(view.switches, this.switches)) {
      // UC-008 A9: what is wrong with the text held now.
      this.problem = view.problem;
    }
    const result = view.result;
    if (
      !result ||
      result.text !== this.searched ||
      this.searched === '' ||
      !sameSwitches(result.switches, this.switches)
    ) {
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
    return (
      this.result !== null &&
      (this.result.text !== this.searched || !sameSwitches(this.result.switches, this.switches))
    );
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

  /**
   * M-13.4, UC-008 A6, UC-005 A20: Ctrl+F opens the find field with
   * `text`, Ctrl+H open for replacing, or takes the keyboard to it.
   */
  openFind(text: string, replacing = false): void {
    this.find = { text, replacing };
    this.findFocusAsked++;
  }

  /** M-13.4: the find field's text changed. */
  setFindText(text: string): void {
    if (this.find) {
      this.find = { ...this.find, text };
    }
  }

  /** UC-005 A20, UC-008 A6: the replace field of the find field shows or hides. */
  setFindReplacing(replacing: boolean): void {
    if (this.find) {
      this.find = { ...this.find, replacing };
    }
  }

  /** UC-008 A7: a switch of the find field. */
  toggleFindSwitch(name: SwitchName): void {
    this.findSwitches = switched(this.findSwitches, name);
  }

  /**
   * UC-008 A6, A7, UC-005 BR-017: what the find field finds: its text with
   * its switches, one text while it replaces, in the facade only the text
   * it shows (`facade`).
   */
  findSpec(facade: boolean): FindSpec | null {
    const find = this.find;
    if (!find) {
      return null;
    }
    return {
      text: find.text,
      switches: this.findSwitches,
      oneText: find.replacing,
      shownOnly: find.replacing && facade,
    };
  }

  /** M-13.4: Escape or "×" closes the find field. */
  closeFind(): void {
    this.find = null;
  }

  /** M-21.3: another workspace: the find field closes (egui); the overlay keeps its text. */
  workspaceChanged(): void {
    this.find = null;
    this.replace.workspaceChanged();
    this.result = null;
    this.notes = [];
    this.cursor = null;
  }
}

/** Whether two sets of switches are the same. */
export function sameSwitches(a: SearchSwitches, b: SearchSwitches): boolean {
  return a.matchCase === b.matchCase && a.wholeWord === b.wholeWord && a.regex === b.regex;
}
