// FR-080, UC-005 A23, A24, BR-018: link completion, one list for both
// views. After each key the view shown asks the window's editor core
// whether the place of a link is typed at the cursor (`linkContext`, in
// the WASM, NFR-018) and offers the answer here; the list then shows the
// notes of the workspace that match what was typed, ranked as go to note
// ranks them (`SuggestionList`, UC-004 BR-014), with their folder, or
// after a `#` the headings of the note named (of the note itself when no
// note was typed), which the backend reads (`links_headings`). The arrow
// keys move in the list, Enter or Tab writes the link chosen through the
// view (its core writes it as the toolbar writes a link to a note), a
// click does too; Escape closes the list and keeps what was typed (A24),
// until another link is started. While nothing matches no list shows, and
// the keys go to the text (A24).

import type {
  FormatName,
  HeadingView,
  LinkChoice,
  LinkContextView,
  OverviewItem,
} from '../bindings';
import { SvelteMap } from 'svelte/reactivity';
import { editorCoreLoaded, loadEditorCore, SuggestionList } from './core';

/** How many entries the list shows at most. */
export const LINKS_SHOWN = 50;

/** One entry of the list: a note, or a heading of one (`note: null`: of the note itself). */
export type CompletionEntry =
  | { kind: 'note'; note: string; title: string; folder: string | null; format: FormatName }
  | {
      kind: 'heading';
      note: string | null;
      format: FormatName | null;
      label: string;
      anchor: string;
      level: number;
    };

/** Where the link starts on the screen; the list shows below it, or above when there is no room. */
export type CompletionPlace = { left: number; top: number; bottom: number };

/** The view whose link is completed. */
export interface CompletionTarget {
  /** The note of the view. */
  readonly note: string;
  /** Writes the link chosen in place of what was typed of it. */
  complete(choice: LinkChoice): void;
  /** Where the link starting at `from` (web units) is on the screen now, after a scroll. */
  place?(from: number): CompletionPlace | null;
}

/** The ids the editor names the list and its chosen entry by (`aria-controls`, `aria-activedescendant`). */
export const LIST_ID = 'link-completion-list';
export const optionId = (at: number) => `link-completion-${at}`;

/** What the list needs of the window. */
export type CompletionHost = {
  /** The notes of the workspace in the order of the note overview (UC-002 BR-008). */
  overview(): readonly OverviewItem[];
  /** The headings of `note`, of its tab when it is open; none when it cannot be read. */
  headings(note: string): Promise<HeadingView[]>;
};

/** The headings of a note: loading, or loaded. */
type Headings = { state: 'loading' } | { state: 'loaded'; list: HeadingView[] };

/** What the list shows for a context: its entries, or that headings load. */
export type CompletionList = {
  entries: CompletionEntry[];
  /** The headings of this note are listed; its title, `null` for the note itself. */
  headingsOf: { title: string | null } | null;
  /** The headings are still being read. */
  loading: boolean;
};

const NOTHING: CompletionList = { entries: [], headingsOf: null, loading: false };

export class LinkCompletion {
  /** The place of the link typed, as the core answered it; `null` while no link is typed. */
  context = $state.raw<LinkContextView | null>(null);
  /** Where the list shows. */
  place = $state.raw<CompletionPlace | null>(null);
  /** The entry chosen in the list. */
  cursor = $state(0);
  /** The ranking's WASM is loaded. */
  ready = $state(editorCoreLoaded());
  /** The headings read, by note; cleared when the list closes. */
  readonly #headings = new SvelteMap<string, Headings>();

  #host: CompletionHost | null = null;
  #target: CompletionTarget | null = null;
  /** A24: the link whose list was closed with Escape, by its note and start. */
  #dismissed: { note: string; from: number } | null = null;
  /** The notes as the ranking holds them, for the overview they were made of. */
  #notes: { of: readonly OverviewItem[]; list: SuggestionList } | null = null;

  /** The window's part: the notes and their headings. */
  attach(host: CompletionHost): void {
    this.#host = host;
  }

  /** The list shows, for the note `note` when given. */
  shows(note?: string): boolean {
    return (
      this.context !== null &&
      (note === undefined || this.#target?.note === note) &&
      (this.list.entries.length > 0 || this.list.loading)
    );
  }

  /**
   * What the text field of `note` says of the list while it shows
   * (`aria-expanded`, `aria-controls`, `aria-activedescendant`).
   */
  aria(note: string): { expanded: boolean; controls?: string; active?: string } {
    if (!this.shows(note)) {
      return { expanded: false };
    }
    const chosen = this.list.entries.length > 0 ? optionId(this.cursor) : undefined;
    return { expanded: true, controls: LIST_ID, active: chosen };
  }

  /** What screen readers hear of the list: how many entries, or that none matches. */
  get spoken(): string {
    if (this.context === null || this.list.loading) {
      return '';
    }
    const count = this.list.entries.length;
    if (count === 0) {
      return 'No suggestions';
    }
    return `${count} ${count === 1 ? 'suggestion' : 'suggestions'}`;
  }

  /** The window scrolled or changed its size: the list follows its link. */
  relocate(): void {
    const target = this.#target;
    const context = this.context;
    if (!target || !context) {
      return;
    }
    if (!target.place) {
      this.close();
      return;
    }
    const place = target.place(context.from);
    if (place === null) {
      this.close();
    } else {
      this.place = place;
    }
  }

  /** A link is being completed in the view of `note` (shown or not). */
  active(note: string): boolean {
    return this.context !== null && this.#target?.note === note;
  }

  /** What the list shows. */
  list: CompletionList = $derived.by(() => this.#rank());

  /**
   * UC-005 A23 step 1: the view `target` answered `context` for the key
   * just typed (`null`: no place of a link there), with the place of the
   * link on the screen; `manual` (Ctrl+Space) opens it also after Escape.
   */
  offer(
    target: CompletionTarget,
    context: LinkContextView | null,
    place: CompletionPlace | null,
    manual = false,
  ) {
    if (context === null) {
      if (this.#target === null || this.#target.note === target.note) {
        this.#dismissed = null;
        this.close();
      }
      return;
    }
    const dismissed = manual ? null : this.#dismissed;
    if (dismissed && dismissed.note === target.note && dismissed.from === context.from) {
      // A24: closed with Escape; typing on in the same link keeps it closed.
      this.context = null;
      return;
    }
    this.#dismissed = null;
    const before = this.context;
    const same =
      this.#target?.note === target.note &&
      before !== null &&
      before.from === context.from &&
      before.typed === context.typed &&
      before.heading === context.heading;
    this.#target = target;
    this.context = context;
    this.place = place;
    if (!same) {
      this.cursor = 0;
    }
    if (!this.ready) {
      void loadEditorCore().then(() => {
        this.ready = true;
        this.#readHeadings();
      });
    }
    this.#readHeadings();
  }

  /**
   * The keys of the list while it shows for `note`: Down and Up move,
   * Enter and Tab choose, Escape closes it (A24). Whether the key was the
   * list's; the view lets it go then.
   */
  key(event: KeyboardEvent, note: string): boolean {
    if (!this.shows(note) || event.altKey || event.metaKey || event.isComposing) {
      return false;
    }
    if (event.ctrlKey && event.key !== 'Escape') {
      return false;
    }
    const count = this.list.entries.length;
    switch (event.key) {
      case 'ArrowDown':
        if (count > 0) this.cursor = (this.cursor + 1) % count;
        return count > 0;
      case 'ArrowUp':
        if (count > 0) this.cursor = (this.cursor - 1 + count) % count;
        return count > 0;
      case 'Enter':
      case 'Tab':
        if (event.shiftKey || count === 0) {
          return false;
        }
        this.choose();
        return true;
      case 'Escape':
        this.dismiss();
        return true;
      default:
        return false;
    }
  }

  /** The pointer rests on entry `index`: it becomes the chosen one. */
  point(index: number): void {
    if (index >= 0 && index < this.list.entries.length) {
      this.cursor = index;
    }
  }

  /** A23 steps 2 and 3: the entry `index` (the chosen one) is written as a link. */
  choose(index: number = this.cursor): void {
    const entry = this.list.entries[index];
    const context = this.context;
    const target = this.#target;
    if (!entry || !context || !target) {
      return;
    }
    const choice: LinkChoice =
      entry.kind === 'note'
        ? {
            rev: context.rev,
            note: entry.note,
            format: entry.format,
            anchor: null,
            label: entry.title,
          }
        : {
            rev: context.rev,
            note: entry.note,
            format: entry.format,
            anchor: entry.anchor,
            label: entry.label,
          };
    this.close();
    target.complete(choice);
  }

  /** A24: Escape: the list closes and stays closed for this link; the text stays as typed. */
  dismiss(): void {
    const context = this.context;
    const target = this.#target;
    if (context && target) {
      this.#dismissed = { note: target.note, from: context.from };
    }
    this.close();
  }

  /** The list closes, for the note `note` when given. */
  close(note?: string): void {
    if (note !== undefined && this.#target?.note !== note) {
      return;
    }
    this.context = null;
    this.place = null;
    this.cursor = 0;
    this.#target = null;
    if (this.#headings.size > 0) {
      this.#headings.clear();
    }
  }

  /** The notes, or the headings of the note named, that match what was typed. */
  #rank(): CompletionList {
    const context = this.context;
    const host = this.#host;
    if (!context || !host || !this.ready) {
      return NOTHING;
    }
    const overview = host.overview();
    if (context.heading !== null) {
      return this.#rankHeadings(context, context.heading, overview);
    }
    const found = this.#notesList(overview).rank(context.typed);
    const entries: CompletionEntry[] = found.slice(0, LINKS_SHOWN).map((at) => {
      const item = overview[at];
      return {
        kind: 'note',
        note: item.note,
        title: item.title,
        folder: item.folder,
        format: item.format,
      };
    });
    return { entries, headingsOf: null, loading: false };
  }

  #rankHeadings(
    context: LinkContextView,
    typed: string,
    overview: readonly OverviewItem[],
  ): CompletionList {
    const note = this.#headingNote(context, overview);
    if (note === undefined) {
      return NOTHING;
    }
    const key = note?.note ?? this.#target?.note ?? '';
    const read = this.#headings.get(key);
    const headingsOf = { title: note?.title ?? null };
    if (!read || read.state === 'loading') {
      return { entries: [], headingsOf, loading: true };
    }
    const list = new SuggestionList(
      read.list.map((heading) => heading.label),
      read.list.map(() => ''),
    );
    try {
      const entries: CompletionEntry[] = list
        .rank(typed)
        .slice(0, LINKS_SHOWN)
        .map((at) => {
          const heading = read.list[at];
          return {
            kind: 'heading',
            note: note?.note ?? null,
            format: note?.format ?? null,
            label: heading.label,
            anchor: heading.anchor,
            level: heading.level,
          };
        });
      return { entries, headingsOf, loading: false };
    } finally {
      list.free();
    }
  }

  /**
   * The note whose headings are listed after a `#`: `null` for the note
   * itself when no note was typed, the note the place names, else the
   * first that matches what was typed; `undefined` when none does.
   */
  #headingNote(
    context: LinkContextView,
    overview: readonly OverviewItem[],
  ): OverviewItem | null | undefined {
    if (context.typed.trim() === '') {
      return null;
    }
    const named = overview.find((item) => item.note === context.named);
    if (named) {
      return named.note === this.#target?.note ? null : named;
    }
    const best = this.#notesList(overview).rank(context.typed)[0];
    if (best === undefined) {
      return undefined;
    }
    const item = overview[best];
    return item.note === this.#target?.note ? null : item;
  }

  /** Reads the headings of the note named, once while the list is open. */
  #readHeadings(): void {
    const context = this.context;
    const host = this.#host;
    if (!context || context.heading === null || !host || !this.ready) {
      return;
    }
    const note = this.#headingNote(context, host.overview());
    if (note === undefined) {
      return;
    }
    const key = note?.note ?? this.#target?.note;
    if (key === undefined || this.#headings.has(key)) {
      return;
    }
    this.#headings.set(key, { state: 'loading' });
    host
      .headings(key)
      .catch((error: unknown) => {
        console.error('The headings could not be read', error);
        return [];
      })
      .then((list) => {
        // Not for a list closed meanwhile.
        if (this.#headings.get(key)?.state === 'loading') {
          this.#headings.set(key, { state: 'loaded', list });
        }
      });
  }

  /** The ranking's copy of `overview`, made anew when the overview changed. */
  #notesList(overview: readonly OverviewItem[]): SuggestionList {
    if (this.#notes?.of !== overview) {
      this.#notes?.list.free();
      this.#notes = {
        of: overview,
        // OQ-251: the place of the note matches too (`Hub.md`,
        // `travel/Check`), as a link names it.
        list: new SuggestionList(
          overview.map((item) => item.title),
          overview.map(
            (item) =>
              `${item.folder ?? ''} ${item.note}.${item.format === 'AsciiDoc' ? 'adoc' : 'md'}`,
          ),
        ),
      };
    }
    return this.#notes.list;
  }
}

/** The one list of the window. */
export const linkCompletion = new LinkCompletion();
