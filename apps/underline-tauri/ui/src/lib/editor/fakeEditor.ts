// A stand-in for the backend's editor (`underline_editor_core::SourceEditor`
// and the session's tab) for the mock backend, so the source view can be
// tried in a plain browser (plan section 10). It follows the same rules in
// UTF-16 units: one char or line at a time whatever the modifiers, Up and
// Down keep the column the cursor has and step over folded lines, Tab
// inserts four spaces, Ctrl/Cmd+A selects all, and undo takes back one
// input at a time (the backend merges a word into one step; its rules are
// tested in Rust).
//
// Feature F4 adds rough stand-ins for the Nano keys (M-11.3), the toolbar
// (M-10.2, Markdown only), the sections and folds (M-08.8), the place of the
// cursor and the status of the input modes (M-11.4). The texts of the
// status line are those of `underline_editor_core::InputStatus`.

import type {
  EditorCommandName,
  EditorInput,
  EditorPatch,
  EditorState,
  KeyHintView,
  KeysStatusView,
  SectionView,
  TextChange,
  ToolName,
  VimChange,
  VimState,
} from '../bindings';

type Step = { from: number; removed: string; inserted: string };

/** The input mode of the settings. */
export type FakeMode = 'Normal' | 'Vim' | 'Nano';

const hints = (pairs: [string, string][]): KeyHintView[] =>
  pairs.map(([keys, action]) => ({ keys, action }));

const VIM_NORMAL: [string, string][] = [
  ['i', 'insert'],
  ['v', 'visual'],
  [':w', 'store'],
  [':q', 'close'],
  ['dd', 'delete line'],
  ['yy', 'yank'],
  ['p', 'put'],
  ['u', 'undo'],
  ['Ctrl+R', 'redo'],
  ['/ n', 'search'],
];

const NANO: [string, string][] = [
  ['^O', 'Store'],
  ['^X', 'Close'],
  ['^K', 'Cut line'],
  ['^U', 'Paste'],
  ['^W', 'Where is'],
  ['M-U', 'Undo'],
  ['M-E', 'Redo'],
  ['^A/^E', 'Line start/end'],
];

/** FR-026: the status of the Vim keys in `vim` (`InputStatus::vim`). */
export function fakeVimStatus(vim: VimState | null): KeysStatusView {
  const state = vim ?? { mode: 'normal', pending: '', marked: false };
  const modes = {
    normal: ['NORMAL', 0],
    insert: ['INSERT', 1],
    visual: ['VISUAL', 2],
    visualLine: ['VISUAL LINE', 2],
    command: ['COMMAND', 3],
  } as const;
  const [mode, color] = modes[state.mode];
  let pairs: [string, string][];
  switch (state.mode) {
    case 'normal':
      pairs = [...(state.marked ? [['Esc', 'end search'] as [string, string]] : []), ...VIM_NORMAL];
      break;
    case 'insert':
      pairs = [['Esc', 'normal mode']];
      break;
    case 'command':
      pairs = [
        ['Enter', 'run'],
        ['Esc', 'cancel'],
      ];
      break;
    default:
      pairs = [
        ['d', 'delete'],
        ['y', 'yank'],
        ['c', 'change'],
        ['Esc', 'normal mode'],
      ];
  }
  const pending = state.mode === 'normal' || state.mode === 'command' ? state.pending : '';
  return { keymap: 'VIM', mode, color, pending, hints: hints(pairs), prompting: false };
}

/** A heading of the fake sections: Markdown `#` lines. */
type FakeSection = { key: string; level: number; line: number; last: number };

export class FakeEditor {
  text: string;
  anchor = 0;
  head = 0;
  rev = 0;
  mode: FakeMode = 'Normal';
  #undo: Step[] = [];
  #redo: Step[] = [];
  /** Nano: the lines cut, whether cuts add up, the prompt, the search. */
  #cut = '';
  #cutting = false;
  #prompt: string | null = null;
  #search: string | null = null;
  #marked = false;
  /** M-08.8: the folded headings, by their text. */
  #folds = new Set<string>();

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
      place: this.place(),
      keys: this.keys(),
    };
  }

  selection(): [number, number] {
    return [Math.min(this.anchor, this.head), Math.max(this.anchor, this.head)];
  }

  selected(): string {
    const [from, to] = this.selection();
    return this.text.slice(from, to);
  }

  /** Sets the selection; gives the place for the status line. */
  select(rev: number, anchor: number, head: number): string {
    if (rev === this.rev) {
      this.anchor = Math.min(anchor, this.text.length);
      this.head = Math.min(head, this.text.length);
      this.#unfoldAtCursor();
    }
    return this.place();
  }

  /** UC-005: "Ln 3, Col 5 · 4 chars" (`CursorPlace::label`). */
  place(): string {
    const lineOf = (at: number) => this.text.slice(0, at).split('\n').length - 1;
    const line = lineOf(this.head);
    const column =
      this.head === 0 ? 0 : this.head - (this.text.lastIndexOf('\n', this.head - 1) + 1);
    const [from, to] = this.selection();
    const chars = [...this.text.slice(from, to)].length;
    const lines = from === to ? 0 : lineOf(to - 1) - lineOf(from) + 1;
    const place = `Ln ${line + 1}, Col ${column + 1}`;
    if (chars === 0) return place;
    if (chars === 1) return `${place} · 1 char`;
    return lines <= 1 ? `${place} · ${chars} chars` : `${place} · ${lines} lines`;
  }

  /** FR-026: the status of the Nano keys; `null` in the other modes. */
  keys(): KeysStatusView | null {
    if (this.mode !== 'Nano') {
      return null;
    }
    if (this.#prompt !== null) {
      return {
        keymap: 'NANO',
        mode: null,
        color: 0,
        pending: `Search: ${this.#prompt}`,
        hints: hints([
          ['Enter', 'find'],
          ['^C', 'cancel'],
        ]),
        prompting: true,
      };
    }
    const marked: [string, string][] = this.#markedSearch() ? [['Esc', 'End search']] : [];
    return {
      keymap: 'NANO',
      mode: null,
      color: 0,
      pending: '',
      hints: hints([...marked, ...NANO]),
      prompting: false,
    };
  }

  /** The patch for `input`; `unstored` comes from the caller. */
  input(input: EditorInput): EditorPatch {
    const base = this.rev;
    if (input.kind === 'tool') {
      return this.#tool(base, input.tool);
    }
    if (this.mode === 'Nano') {
      const nano = this.#nano(base, input);
      if (nano) {
        return nano;
      }
    }
    if (input.kind === 'text') {
      return this.#patch(base, this.#replaceSelection(input.text));
    }
    const command = this.mac ? input.meta : input.ctrl;
    if (command && input.shift && (input.key === 'BracketLeft' || input.key === 'BracketRight')) {
      return this.#patch(base, null, this.#foldKey(input.key === 'BracketLeft'));
    }
    const tool = command ? toolOf(input.key, input.shift) : null;
    if (tool) {
      return this.#tool(base, tool);
    }
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
      case 'ArrowDown':
        target = this.#vertical(input.key === 'ArrowUp' ? -1 : 1, column);
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

  /** A-01: the Vim keys' changes on revision `base`; `null` when it is gone. */
  apply(base: number, changes: VimChange[], anchor: number, head: number): EditorPatch | null {
    if (base !== this.rev) {
      return null;
    }
    for (const change of changes) {
      const step = {
        from: change.from,
        removed: this.text.slice(change.from, change.to),
        inserted: change.insert,
      };
      this.#undo.push(step);
      this.#redo = [];
      this.#applyStep(step);
    }
    this.anchor = Math.min(anchor, this.text.length);
    this.head = Math.min(head, this.text.length);
    const patch = this.#patch(base, null, true);
    return { ...patch, scroll: false };
  }

  /** M-08.8: the arrow of the heading on `line` folds or expands it. */
  fold(line: number): EditorPatch {
    const section = this.#sections().find((s) => s.line === line && s.last > s.line);
    let moved = false;
    if (section) {
      moved = this.#toggle(section);
    }
    return this.#patch(this.rev, null, moved);
  }

  /** M-08.8: the sections, with what is folded. */
  sections(): SectionView[] {
    return this.#sections().map((s) => ({
      line: s.line,
      last: s.last,
      folded: this.#folds.has(s.key) && s.last > s.line,
    }));
  }

  /** M-11.3: the marked hits of the Nano search. */
  found(): [number, number][] {
    const pattern = this.#markedSearch();
    if (!pattern) {
      return [];
    }
    const ranges: [number, number][] = [];
    for (let at = this.text.indexOf(pattern); at >= 0; at = this.text.indexOf(pattern, at + 1)) {
      ranges.push([at, at + pattern.length]);
    }
    return ranges;
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
    this.#cutting = false;
    return this.#patch(this.rev, this.#replaceSelection(text));
  }

  #markedSearch(): string | null {
    return this.mode === 'Nano' && this.#marked ? this.#search : null;
  }

  /** M-11.3: the Nano keys; `null` for what they leave to the usual keys. */
  #nano(base: number, input: EditorInput): EditorPatch | null {
    if (input.kind === 'tool') {
      return null;
    }
    if (this.#prompt !== null) {
      if (input.kind === 'text') {
        this.#prompt += input.text;
      } else if (input.key === 'Enter') {
        const pattern = this.#prompt;
        this.#prompt = null;
        if (pattern !== '') {
          this.#search = pattern;
        }
        this.#marked = this.#search !== null;
        const at = this.#search === null ? -1 : this.#find(this.#search);
        if (at >= 0) {
          this.head = this.anchor = at;
          return this.#patch(base, null, true);
        }
      } else if (input.key === 'Escape' || (input.key === 'C' && input.ctrl)) {
        this.#prompt = null;
      } else if (input.key === 'Backspace') {
        this.#prompt = this.#prompt.slice(0, -1);
      }
      return this.#patch(base, null, false);
    }
    if (input.kind === 'text') {
      this.#cutting = false;
      return null;
    }
    const ctrl = input.ctrl || (this.mac && input.meta);
    const wasCutting = this.#cutting;
    this.#cutting = false;
    const [lineStart, lineEnd] = this.#line(this.head);
    const commands: EditorCommandName[] = [];
    if (ctrl && !input.alt) {
      switch (input.key) {
        case 'K': {
          const end = lineEnd < this.text.length ? lineEnd + 1 : lineEnd;
          const cut = this.text.slice(lineStart, end);
          this.#cut = wasCutting ? this.#cut + cut : cut;
          this.anchor = lineStart;
          this.head = end;
          const change = this.#replaceSelection('');
          this.#cutting = true;
          return this.#patch(base, change);
        }
        case 'U':
          if (this.#cut === '') {
            return this.#patch(base, null, false);
          }
          this.anchor = this.head = lineStart;
          return this.#patch(base, this.#replaceSelection(this.#cut));
        case 'W':
          this.#prompt = '';
          return this.#patch(base, null, false);
        case 'X':
          commands.push('close');
          break;
        case 'O':
        case 'S':
          commands.push('store');
          break;
        case 'A':
          this.head = this.anchor = lineStart;
          return this.#patch(base, null, true);
        case 'E':
          this.head = this.anchor = lineEnd;
          return this.#patch(base, null, true);
        default:
          return null;
      }
      return { ...this.#patch(base, null, false), commands };
    }
    if (input.alt && !ctrl && (input.key === 'U' || input.key === 'E')) {
      return this.#patch(base, input.key === 'U' ? this.#undoStep() : this.#redoStep());
    }
    if (input.key === 'Escape' && !ctrl && !input.alt) {
      this.#marked = false;
      return this.#patch(base, null, false);
    }
    return null;
  }

  #find(pattern: string): number {
    const after = this.text.indexOf(pattern, this.head + 1);
    return after >= 0 ? after : this.text.indexOf(pattern);
  }

  /** M-10.2: the toolbar in Markdown; Link and Image ask first. */
  #tool(base: number, tool: ToolName): EditorPatch {
    if (tool === 'link' || tool === 'image') {
      return { ...this.#patch(base, null, false), request: tool };
    }
    const marks: Partial<Record<ToolName, string>> = { bold: '**', italic: '*', code: '`' };
    const prefixes: Partial<Record<ToolName, string>> = {
      heading: '# ',
      list: '- ',
      quote: '> ',
    };
    const [from, to] = this.selection();
    const mark = marks[tool];
    if (mark) {
      const selected = this.text.slice(from, to);
      const wrapped =
        this.text.slice(from - mark.length, from) === mark &&
        this.text.slice(to, to + mark.length) === mark;
      if (wrapped) {
        this.anchor = from - mark.length;
        this.head = to + mark.length;
        const change = this.#replaceSelection(selected);
        this.anchor = from - mark.length;
        this.head = this.anchor + selected.length;
        return this.#patch(base, change);
      }
      const change = this.#replaceSelection(`${mark}${selected}${mark}`);
      this.anchor = from + mark.length;
      this.head = this.anchor + selected.length;
      return this.#patch(base, change);
    }
    const prefix = prefixes[tool];
    if (prefix) {
      const start = this.#line(from)[0];
      this.anchor = this.head = start;
      const change = this.#replaceSelection(prefix);
      return this.#patch(base, change);
    }
    // Table: a small table at the cursor.
    return this.#patch(base, this.#replaceSelection('| | |\n|---|---|\n| | |\n'));
  }

  #sections(): FakeSection[] {
    const lines = this.text.split('\n');
    const headings: { key: string; level: number; line: number }[] = [];
    lines.forEach((line, index) => {
      const match = /^(#{1,6}) /.exec(line);
      if (match) {
        headings.push({ key: line, level: match[1].length, line: index });
      }
    });
    return headings.map((heading, index) => {
      const next = headings.slice(index + 1).find((h) => h.level <= heading.level);
      let last = (next?.line ?? lines.length) - 1;
      // The empty last line after the final break is not part of a body.
      if (!next && lines.at(-1) === '') {
        last -= 1;
      }
      return { ...heading, last: Math.max(last, heading.line) };
    });
  }

  #hidden(line: number): boolean {
    return this.#sections().some((s) => this.#folds.has(s.key) && line > s.line && line <= s.last);
  }

  #lineNumber(at: number): number {
    return this.text.slice(0, at).split('\n').length - 1;
  }

  #toggle(section: FakeSection): boolean {
    if (this.#folds.delete(section.key)) {
      return false;
    }
    this.#folds.add(section.key);
    const line = this.#lineNumber(this.head);
    if (line > section.line && line <= section.last) {
      const end = this.#line(this.#lineStart(section.line))[1];
      this.anchor = this.head = end;
      return true;
    }
    return false;
  }

  #foldKey(fold: boolean): boolean {
    const line = this.#lineNumber(this.head);
    const section = this.#sections()
      .filter((s) => s.line <= line && line <= s.last)
      .sort((a, b) => b.level - a.level)[0];
    if (!section || fold === this.#folds.has(section.key)) {
      return false;
    }
    return this.#toggle(section);
  }

  #unfoldAtCursor(): void {
    const line = this.#lineNumber(this.head);
    for (const s of this.#sections()) {
      if (line > s.line && line <= s.last) {
        this.#folds.delete(s.key);
      }
    }
  }

  #lineStart(line: number): number {
    let at = 0;
    for (let i = 0; i < line; i++) {
      at = this.text.indexOf('\n', at) + 1;
    }
    return at;
  }

  /** Up (-1) or Down (1), over folded lines, keeping `column`. */
  #vertical(step: -1 | 1, column: number): number {
    const lines = this.text.split('\n').length;
    const current = this.#lineNumber(this.head);
    let line = current + step;
    while (line >= 0 && line < lines && this.#hidden(line)) {
      line += step;
    }
    if (line < 0 || line >= lines) {
      line = current;
    }
    return this.#atColumn(this.#lineStart(line), column);
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
    if (moved) {
      this.#unfoldAtCursor();
    }
    return {
      base,
      rev: this.rev,
      changes: change ? [change] : [],
      anchor: this.anchor,
      head: this.head,
      scroll: moved,
      unstored: false,
      place: this.place(),
      keys: this.keys(),
      commands: [],
      request: null,
    };
  }
}

/** M-10.2: the toolbar function of a key with the command key. */
function toolOf(key: string, shift: boolean): ToolName | null {
  const plain: Record<string, ToolName> = { B: 'bold', I: 'italic', E: 'code', K: 'link' };
  const shifted: Record<string, ToolName> = {
    H: 'heading',
    L: 'list',
    Q: 'quote',
    I: 'image',
    T: 'table',
  };
  return (shift ? shifted[key] : plain[key]) ?? null;
}
