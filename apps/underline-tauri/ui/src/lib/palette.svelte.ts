// FR-076, FR-077: the field of go to note (UC-004 A12, A13) and of the
// command palette (UC-023), one overlay with one field: it is the command
// palette while its text starts with `>` and go to note without (UC-023 A4,
// BR-007). Not stored (M-22.4), except the recently run commands, which
// the backend keeps for the user across restarts (BR-004, OQ-198): the
// window takes them at the start (`remember`), drops ids it does not know,
// and tells the backend each command run (`PaletteHost.ran`).
//
// The ranking is the rule of `underline_markup::suggest` in the WASM
// (`SuggestionList`), so both overlays match and order alike and the window
// asks the backend nothing on a key (NFR-018). What the window decides here
// is only which suggestions it gives the rule, in which order: the note
// overview's order for notes (BR-014), and for commands the most recently
// run first, then by name (`paletteOrder`), those not available now marked
// to follow the others (UC-023 BR-002).
//
// FR-086, UC-004 A16, A17: "Go to line" asks in the same field: it names
// how many lines the note has, reads the line (and a column after a colon)
// with the rule of the WASM (`lineTarget`), says when it is no line number
// and keeps what was typed, and otherwise closes and has the view shown put
// its cursor there.

import type { LineTargetView, OverviewItem } from './bindings';
import { paletteOrder, RECENT_FIRST, type Command, type CommandChoice } from './commands';
import { editorCoreLoaded, lineTarget, loadEditorCore, SuggestionList } from './editor/core';
import { activeFacade } from './facade/active';
import { focusFallback } from './focus';

/** How many notes the list shows at first, and how many more each time it nears its end. */
export const NOTES_PAGE = 50;

/** UC-023 BR-004, OQ-198: how many recently run commands are kept (the backend's `RECENT_COMMANDS`). */
export const RECENT_COMMANDS = 20;

/** What the overlay needs of the window. */
export type PaletteHost = {
  /** The registry's commands (`commandList.ts`). */
  commands(): readonly Command[];
  /** The note overview, in its order (UC-002 BR-008). */
  overview(): readonly OverviewItem[];
  /** A workspace is open. */
  workspaceOpen(): boolean;
  /** UC-004 A12 step 4: opens the note chosen; its view takes the keyboard. */
  openNote(note: string): void;
  /** UC-004 A18: opens the note chosen to the side (Ctrl+Enter, Cmd+Enter on macOS). */
  openNoteToSide?(note: string): void;
  /** UC-004 A13 step 2: "New note" with the title typed. */
  newNote(title: string): void;
  /** Another overlay closes when this one opens (the search of the workspace). */
  closeOthers(): void;
  /**
   * UC-004 A16 step 1: how many lines the active note has and whether the
   * facade shows it; `null` while unknown.
   */
  lineInfo(): { lines: number; facade: boolean } | null;
  /** UC-004 A16 step 3: the view shown puts its cursor on `target`, with the keyboard. */
  goToLine(target: LineTargetView): void;
  /** UC-023 BR-004, OQ-198: the command `id` ran; the backend remembers it for the user. */
  ran?(id: string): void;
};

/** One entry of the list. */
export type PaletteEntry =
  | { kind: 'note'; note: string; title: string; folder: string | null }
  | { kind: 'command'; command: Command; unavailable: string | null }
  | { kind: 'choice'; choice: CommandChoice }
  | { kind: 'create'; title: string };

/** What the list shows for the field's text. */
export type PaletteList = {
  entries: PaletteEntry[];
  /** How many match in all; the list shows the first ones (notes come page by page). */
  total: number;
  /**
   * BR-004: with an empty field of the command palette, how many entries at
   * the top are the recently run commands ("Recently used", then "All
   * commands"); 0 otherwise.
   */
  recent?: number;
};

const EMPTY: PaletteList = { entries: [], total: 0 };

export class PaletteState {
  /** The overlay is open. */
  open = $state(false);
  /** The field's text; with `>` first, the command palette (UC-023 BR-007). */
  text = $state('');
  /** The chosen entry of the list. */
  cursor = $state(0);
  /** UC-023 A3: the command that asks for a choice; the field filters its choices. */
  asking = $state.raw<Command | null>(null);
  /** UC-023 A2: why the command chosen cannot run now. */
  notice = $state<string | null>(null);
  /** BR-004, OQ-198: the ids of the recently run commands, the most recent first. */
  recent = $state.raw<string[]>([]);
  /** How many notes the list shows (page by page). */
  limit = $state(NOTES_PAGE);
  /** Grows when the field should take the keyboard. */
  focusAsked = $state(0);
  /** The ranking's WASM is loaded. */
  ready = $state(editorCoreLoaded());
  /** UC-004 A16: the field asks for a line to go to. */
  askingLine = $state(false);
  /** UC-004 A16 step 1: how many lines the note has, while the field asks for a line. */
  lines = $state<number | null>(null);
  /** The field asks for a line of the facade, which takes no column. */
  linesOfFacade = $state(false);

  #host: PaletteHost;
  /** Where the keyboard was before the overlay opened. */
  #before: HTMLElement | null = null;
  /** The notes as the ranking holds them, for the overview they were made of. */
  #notes: { of: readonly OverviewItem[]; list: SuggestionList } | null = null;

  constructor(host: PaletteHost) {
    this.#host = host;
  }

  /** What the field is now: go to note, the commands, the choices of a command, or a line. */
  get mode(): 'notes' | 'commands' | 'choices' | 'line' {
    if (this.askingLine) {
      return 'line';
    }
    if (this.asking !== null) {
      return 'choices';
    }
    return this.text.startsWith('>') ? 'commands' : 'notes';
  }

  /** What is matched: the text without the `>` of the command palette. */
  get typed(): string {
    return this.mode === 'commands' ? this.text.slice(1) : this.text;
  }

  /** The list for the field's text (UC-004 A12 step 2, UC-023 step 2). */
  list: PaletteList = $derived.by(() => this.#rank());

  /** The chosen entry, if any. */
  get chosen(): PaletteEntry | null {
    return this.list.entries[this.cursor] ?? null;
  }

  /**
   * UC-004 A12, UC-023 step 1: opens the overlay as go to note (`notes`) or
   * as the command palette (`commands`), remembering where the keyboard
   * was; open already, it turns into the other, keeping the text typed.
   */
  show(mode: 'notes' | 'commands'): void {
    this.askingLine = false;
    if (!this.open) {
      const active = document.activeElement;
      this.#before = active instanceof HTMLElement && active !== document.body ? active : null;
      this.#host.closeOthers();
      this.open = true;
      this.#setText(mode === 'commands' ? '>' : '');
    } else {
      this.asking = null;
      const typed = this.text.startsWith('>') ? this.text.slice(1) : this.text;
      this.#setText(mode === 'commands' ? `>${typed}` : typed);
    }
    if (!this.ready) {
      void loadEditorCore().then(() => {
        this.ready = true;
      });
    }
    this.focusAsked++;
  }

  /**
   * UC-004 A16 step 1: opens the field asking for a line of the active
   * note, saying how many it has; open already, it turns into it.
   */
  showLine(): void {
    if (!this.open) {
      const active = document.activeElement;
      this.#before = active instanceof HTMLElement && active !== document.body ? active : null;
      this.#host.closeOthers();
      this.open = true;
    }
    this.asking = null;
    this.askingLine = true;
    const info = this.#host.lineInfo();
    this.lines = info?.lines ?? null;
    this.linesOfFacade = info?.facade ?? false;
    this.#setText('');
    if (!this.ready) {
      void loadEditorCore().then(() => {
        this.ready = true;
      });
    }
    this.focusAsked++;
  }

  /** The field's text changed. */
  setText(text: string): void {
    this.#setText(text);
  }

  #setText(text: string): void {
    this.text = text;
    this.notice = null;
    this.limit = NOTES_PAGE;
    this.cursor = 0;
    if (this.asking !== null) {
      this.cursor = this.#currentChoice();
    }
  }

  /** Down and Up: the next or the previous entry, round the list. */
  move(by: number): void {
    const count = this.list.entries.length;
    if (count === 0) {
      return;
    }
    this.cursor = (this.cursor + by + count) % count;
    this.notice = null;
    if (this.cursor >= count - 6) {
      this.more();
    }
  }

  /** The pointer rests on entry `index`: it becomes the chosen one. */
  point(index: number): void {
    if (index !== this.cursor && index >= 0 && index < this.list.entries.length) {
      this.cursor = index;
    }
  }

  /**
   * Backspace in the empty field of a command's choices (A3): back to the
   * list of commands.
   */
  back(): boolean {
    if (this.asking === null || this.text !== '') {
      return false;
    }
    this.asking = null;
    this.#setText('>');
    return true;
  }

  /** The next page of notes, when the list shows fewer than match. */
  more(): void {
    if (this.mode === 'notes' && this.list.total > this.limit) {
      this.limit += NOTES_PAGE;
    }
  }

  /**
   * Enter or a click on entry `index` (the chosen one by default): opens
   * the note, offers to create one, asks for the choice of a command, or
   * runs the command, after the overlay closed and gave the keyboard back
   * (UC-023 step 4). A command not available now runs nothing and says why
   * (A2). With `side`, a note opens to the side (UC-004 A18, FR-096).
   */
  choose(index: number = this.cursor, side = false): void {
    if (this.askingLine) {
      this.#goToLine();
      return;
    }
    const entry = this.list.entries[index];
    if (entry === undefined) {
      return;
    }
    this.cursor = index;
    switch (entry.kind) {
      case 'note':
        this.close(false);
        if (side && this.#host.openNoteToSide) {
          this.#host.openNoteToSide(entry.note);
        } else {
          this.#host.openNote(entry.note);
        }
        return;
      case 'create':
        this.close(false);
        this.#host.newNote(entry.title);
        return;
      case 'choice': {
        const command = this.asking;
        if (command === null) {
          return;
        }
        this.close(true);
        command.run(entry.choice.value);
        return;
      }
      case 'command': {
        const why = entry.command.unavailable();
        if (why !== null) {
          this.notice = why;
          return;
        }
        if (entry.command.choices) {
          // BR-004 (app-tester): a command counts as run when chosen, also
          // one that asks for a choice next ("Theme…").
          this.#ran(entry.command);
          this.asking = entry.command;
          this.#setText('');
          this.focusAsked++;
          return;
        }
        this.close(true);
        this.#ran(entry.command);
        entry.command.run();
        return;
      }
    }
  }

  /**
   * UC-023 A5: closes the overlay, running nothing. With `restore`, the
   * keyboard goes back where it was before (also after a press outside
   * that left it nowhere).
   */
  close(restore = true, later = false): void {
    if (!this.open) {
      return;
    }
    this.open = false;
    this.text = '';
    this.asking = null;
    this.askingLine = false;
    this.lines = null;
    this.notice = null;
    this.cursor = 0;
    const before = this.#before;
    this.#before = null;
    if (!restore) {
      return;
    }
    const back = () => {
      const facade = activeFacade();
      if (before?.isConnected && facade && before.closest('.facade-text')) {
        // Review: the facade puts its caret back where it was; a plain
        // focus would leave the selection on the text's container.
        facade.focus();
      } else if (before?.isConnected) {
        before.focus();
      } else {
        focusFallback();
      }
    };
    if (!later) {
      back();
      return;
    }
    // After the press that closed it has moved the keyboard, if it does.
    setTimeout(() => {
      const now = document.activeElement;
      if (now === null || now === document.body) {
        back();
      }
    }, 0);
  }

  /**
   * UC-004 A16 steps 2 and 3, A17: Enter in the field asking for a line:
   * what is no line number is said, the text kept and nothing moved;
   * otherwise the field closes, giving the keyboard back, and the view
   * shown goes to the line.
   */
  #goToLine(): void {
    if (!this.ready) {
      return;
    }
    const answer = lineTarget(this.text);
    if (answer.target === null) {
      this.notice = answer.problem;
      return;
    }
    const target = answer.target;
    this.close(true);
    this.#host.goToLine(target);
  }

  /**
   * BR-004, OQ-198: the recently run commands remembered for the user, as
   * the backend gives them at the start; ids no command of `known` has (a
   * command removed or renamed since) are dropped, and repeats.
   */
  remember(ids: readonly string[], known: readonly Command[]): void {
    const ours = known.map((command) => command.id);
    this.recent = ids
      .filter((id, at) => ids.indexOf(id) === at && ours.includes(id))
      .slice(0, RECENT_COMMANDS);
  }

  /** BR-004, OQ-198: `command` ran; it is listed first next time, also after a restart. */
  #ran(command: Command): void {
    this.recent = [command.id, ...this.recent.filter((id) => id !== command.id)].slice(
      0,
      RECENT_COMMANDS,
    );
    this.#host.ran?.(command.id);
  }

  /** A3: the index of the choice in effect, chosen at first while nothing is typed. */
  #currentChoice(): number {
    if (this.text.trim() !== '') {
      return 0;
    }
    const at = this.list.entries.findIndex(
      (entry) => entry.kind === 'choice' && entry.choice.current,
    );
    return Math.max(at, 0);
  }

  #rank(): PaletteList {
    if (!this.open || !this.ready) {
      return EMPTY;
    }
    switch (this.mode) {
      case 'line':
        return EMPTY;
      case 'choices':
        return this.#rankChoices();
      case 'commands':
        return this.#rankCommands();
      case 'notes':
        return this.#rankNotes();
    }
  }

  #rankChoices(): PaletteList {
    const choices = this.asking?.choices?.() ?? [];
    const list = new SuggestionList(
      choices.map((choice) => choice.label),
      choices.map(() => ''),
    );
    try {
      const found = list.rank(this.text);
      return {
        entries: found.map((at) => ({ kind: 'choice', choice: choices[at] })),
        total: found.length,
      };
    } finally {
      list.free();
    }
  }

  #rankCommands(): PaletteList {
    const typed = this.typed;
    const ordered = paletteOrder(this.#host.commands(), this.recent, typed.trim() === '');
    const unavailable = ordered.map((command) => command.unavailable());
    const last = unavailable.flatMap((why, at) => (why === null ? [] : [at]));
    const list = new SuggestionList(
      ordered.map((command) => command.title),
      ordered.map((command) => command.menu),
    );
    try {
      const found = list.rank(typed, last);
      const entries: PaletteEntry[] = found.map((at) => ({
        kind: 'command',
        command: ordered[at],
        unavailable: unavailable[at],
      }));
      const firsts = this.recent.slice(0, RECENT_FIRST);
      const recent =
        typed.trim() === ''
          ? entries.findIndex(
              (entry) => entry.kind !== 'command' || !firsts.includes(entry.command.id),
            )
          : 0;
      return {
        entries,
        total: found.length,
        recent: recent < 0 ? entries.length : recent,
      };
    } finally {
      list.free();
    }
  }

  #rankNotes(): PaletteList {
    if (!this.#host.workspaceOpen()) {
      return EMPTY;
    }
    const overview = this.#host.overview();
    const found = this.#notesList(overview).rank(this.text);
    const typed = this.text.trim();
    const entries: PaletteEntry[] = found.slice(0, this.limit).map((at) => {
      const item = overview[at];
      return { kind: 'note', note: item.note, title: item.title, folder: item.folder };
    });
    // A13, UX review: creating a note with the text typed is offered last
    // whenever something is typed, chosen when nothing matches.
    if (typed !== '') {
      entries.push({ kind: 'create', title: typed });
    }
    return { entries, total: found.length };
  }

  /** The ranking's copy of `overview`, made anew when the overview changed. */
  #notesList(overview: readonly OverviewItem[]): SuggestionList {
    if (this.#notes?.of !== overview) {
      this.#notes?.list.free();
      this.#notes = {
        of: overview,
        list: new SuggestionList(
          overview.map((item) => item.title),
          overview.map((item) => item.folder ?? ''),
        ),
      };
    }
    return this.#notes.list;
  }
}
