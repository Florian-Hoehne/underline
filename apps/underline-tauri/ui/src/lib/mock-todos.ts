// Feature F15 in the mock backend (UC-020, UC-021): rough stand-ins for the
// todos of `underline_core::todos` and the session's `todos.rs`, so every
// todo flow can be tried in a browser; the real rules are tested in Rust
// (`underline-session/src/session/todos/tests.rs`).
//
// Each fake workspace has its todos in memory. `/home/user/Notes` starts
// with four: #1 "Book the ferry" (high, overdue by three days, linked to
// Trip), #2 "Renew passport" (critical, scheduled after its due day, four
// notes linked), #3 "Read chapter 4" (low, no due day, one note linked that
// no longer exists) and #4 "Write the log" (done). `/home/user/Team` has #1
// "Prepare slides" and #3 "Send the agenda" (the todo in conflict after a
// sync, F13). `/home/user/Work` has none. The dates are relative to today.
//
// The hooks of `window.__ulMock` (mock-backend.ts) change todos from
// outside (`todoOutside`, as another program or a sync), as an agent
// (`agentTodo`), make the files unwritable (`todoWriteFails`, UC-020 A9)
// or a file unreadable (`todoUnreadable`).

import type {
  CalendarView,
  TodoAnswer,
  TodoCandidatesView,
  TodoChange,
  TodoFilterName,
  TodoSortName,
  TodoView,
  TodosView,
  UrgencyName,
} from './bindings';

/** A todo as the mock keeps it. */
export type MockTodo = {
  number: number;
  title: string;
  urgency: UrgencyName;
  due: string | null;
  scheduled: string | null;
  created: string;
  done: string | null;
  links: string[];
};

/** The tab "Todos" of a workspace (`TodoListState`). */
export type MockTodoList = {
  open: boolean;
  active: boolean;
  sort: TodoSortName;
  filter: TodoFilterName;
};

const URGENCY: Record<UrgencyName, { label: string; rank: number }> = {
  low: { label: 'Low', rank: 0 },
  normal: { label: 'Normal', rank: 1 },
  high: { label: 'High', rank: 2 },
  critical: { label: 'Critical', rank: 3 },
};

const pad = (n: number) => String(n).padStart(2, '0');

/** A day `offset` days from today, `YYYY-MM-DD`, by the local calendar. */
export function mockDay(offset = 0, from: Date = new Date()): string {
  const day = new Date(from.getFullYear(), from.getMonth(), from.getDate() + offset);
  return `${day.getFullYear()}-${pad(day.getMonth() + 1)}-${pad(day.getDate())}`;
}

/** Now, as a todo stores it. */
function nowText(): string {
  const now = new Date();
  return `${mockDay(0, now)}T${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}+01:00`;
}

/** Days from `a` to `b` (`YYYY-MM-DD`). */
function daysBetween(a: string, b: string): number {
  const at = (d: string) => Date.UTC(+d.slice(0, 4), +d.slice(5, 7) - 1, +d.slice(8, 10));
  return Math.round((at(b) - at(a)) / 86_400_000);
}

/** UC-020 BR-003: a day typed, roughly as `Day::from_str`. */
function parseDay(text: string): string | null | Error {
  const day = text.trim();
  if (day === '') {
    return null;
  }
  const valid = /^\d{4}-\d{2}-\d{2}$/.test(day);
  const [y, m, d] = [+day.slice(0, 4), +day.slice(5, 7), +day.slice(8, 10)];
  const date = new Date(Date.UTC(y, m - 1, d));
  if (!valid || date.getUTCMonth() !== m - 1 || date.getUTCDate() !== d) {
    return new Error(`\`${day}\` is not a day; write it as YYYY-MM-DD`);
  }
  return day;
}

/** UC-020 A3: a title of 1 to 200 characters, on one line. */
function checkTitle(title: string): string | Error {
  const clean = title.split(/\s+/).filter(Boolean).join(' ');
  if (clean === '') {
    return new Error('a todo needs a title');
  }
  if ([...clean].length > 200) {
    return new Error('the title of a todo has at most 200 characters');
  }
  return clean;
}

const titleOf = (link: string) => (link.split('/').at(-1) ?? link).replace(/-/g, ' ');
const minutes = (time: string) => time.slice(0, 16).replace('T', ' ');
const nameOf = (todo: MockTodo) => `#${todo.number} ${todo.title}`;

/** The todos each fake workspace starts with. */
function seeds(): Record<string, MockTodo[]> {
  const created = `${mockDay(-20)}T09:15:00+01:00`;
  return {
    '/home/user/Notes': [
      {
        number: 1,
        title: 'Book the ferry',
        urgency: 'high',
        due: mockDay(-3),
        scheduled: null,
        created,
        done: null,
        links: ['Trip'],
      },
      {
        number: 2,
        title: 'Renew passport',
        urgency: 'critical',
        due: mockDay(10),
        scheduled: mockDay(12),
        created,
        done: null,
        links: ['Trip', 'Journal/2026-11-09', 'Ideas', 'Log'],
      },
      {
        number: 3,
        title: 'Read chapter 4',
        urgency: 'low',
        due: null,
        scheduled: null,
        created,
        done: null,
        links: ['Reading-List', 'Old-Plan'],
      },
      {
        number: 4,
        title: 'Write the log',
        urgency: 'normal',
        due: mockDay(-1),
        scheduled: null,
        created,
        done: `${mockDay(-1)}T18:40:00+01:00`,
        links: ['Log'],
      },
    ],
    '/home/user/Team': [
      {
        number: 1,
        title: 'Prepare slides',
        urgency: 'normal',
        due: mockDay(4),
        scheduled: null,
        created,
        done: null,
        links: ['Plan'],
      },
      {
        number: 3,
        title: 'Send the agenda',
        urgency: 'normal',
        due: '2026-11-12',
        scheduled: null,
        created,
        done: null,
        links: ['Meetings'],
      },
    ],
  };
}

/** What a todo command did: its answer, and what the status says. */
export type MockTodoDone = { answer: TodoAnswer; status?: string; offer?: 'delete' | 'mark' };

/** The todos of the fake workspaces. */
export class MockTodos {
  readonly todos: Record<string, MockTodo[]> = seeds();
  readonly lists: Record<string, MockTodoList> = {};
  /** UC-020 A9: changes not stored, by workspace and number, with the reason. */
  readonly unstored: Record<string, Map<number, { todo: MockTodo; why: string }>> = {};
  /** The files cannot be written, with this reason (`todoWriteFails`). */
  writeFails: string | null = null;
  /** Lines of todo files that cannot be read (`todoUnreadable`). */
  unreadable: string[] = [];
  /** A5, UC-021 step 8: what "Undo" brings back, with its report. */
  deleted: { root: string; todo: MockTodo; status: string } | null = null;
  marked: { root: string; todo: MockTodo; status: string } | null = null;
  /** BR-002: the highest number given per workspace. */
  readonly highest: Record<string, number> = {};

  list(root: string | null): MockTodoList {
    const none: MockTodoList = { open: false, active: false, sort: 'due', filter: 'open' };
    if (root === null) {
      return none;
    }
    return (this.lists[root] ??= none);
  }

  of(root: string): MockTodo[] {
    return (this.todos[root] ??= []);
  }

  #unstored(root: string): Map<number, { todo: MockTodo; why: string }> {
    return (this.unstored[root] ??= new Map());
  }

  /** The todo as shown: not stored, else as stored. */
  shown(root: string, number: number): MockTodo | undefined {
    return this.#unstored(root).get(number)?.todo ?? this.of(root).find((t) => t.number === number);
  }

  #all(root: string): MockTodo[] {
    const stored = this.of(root).map((t) => this.shown(root, t.number) ?? t);
    const extra = [...this.#unstored(root).values()]
      .map((u) => u.todo)
      .filter((t) => !stored.some((s) => s.number === t.number));
    return [...stored, ...extra].sort((a, b) => a.number - b.number);
  }

  /** Stores `todo`, or keeps it as not stored (A9). */
  #store(root: string, todo: MockTodo): string | null {
    if (this.writeFails !== null) {
      return `the todos cannot be stored: ${this.writeFails}`;
    }
    const list = this.of(root);
    const at = list.findIndex((t) => t.number === todo.number);
    if (at < 0) {
      list.push(todo);
    } else {
      list[at] = todo;
    }
    list.sort((a, b) => a.number - b.number);
    this.highest[root] = Math.max(this.highest[root] ?? 0, todo.number);
    return null;
  }

  /** F15 review: the last attempt before another workspace opens; what still fails. */
  settle(root: string): string[] {
    this.retry(root);
    return [...this.#unstored(root).entries()].map(
      ([number, { why }]) => `Todo #${number} could not be stored: ${why}`,
    );
  }

  /** F15 review: "Close anyway": the changes not stored are given up. */
  giveUp(root: string): void {
    this.#unstored(root).clear();
  }

  /** M-18.9, A9: what was not stored is tried again; whether any was stored. */
  retry(root: string | null): boolean {
    if (root === null || this.writeFails !== null) {
      return false;
    }
    const waiting = this.#unstored(root);
    const any = waiting.size > 0;
    for (const { todo } of waiting.values()) {
      this.#store(root, todo);
    }
    waiting.clear();
    return any;
  }

  create(root: string, title: string): MockTodoDone {
    const clean = checkTitle(title);
    if (clean instanceof Error) {
      return { answer: { kind: 'problem', problem: clean.message } };
    }
    if (this.writeFails !== null) {
      return {
        answer: { kind: 'failed' },
        status: `The todo could not be created: the todos cannot be stored: ${this.writeFails}`,
      };
    }
    const number = Math.max(this.highest[root] ?? 0, ...this.of(root).map((t) => t.number), 0) + 1;
    const todo: MockTodo = {
      number,
      title: clean,
      urgency: 'normal',
      due: null,
      scheduled: null,
      created: nowText(),
      done: null,
      links: [],
    };
    this.#store(root, todo);
    return { answer: { kind: 'done' }, status: `Created todo ${nameOf(todo)}` };
  }

  change(root: string, number: number, change: TodoChange, notes: string[] = []): MockTodoDone {
    const before = this.shown(root, number);
    if (!before) {
      return { answer: { kind: 'done' } };
    }
    const todo: MockTodo = { ...before, links: [...before.links] };
    switch (change.kind) {
      case 'title': {
        const clean = checkTitle(change.title);
        if (clean instanceof Error) {
          return { answer: { kind: 'problem', problem: clean.message } };
        }
        todo.title = clean;
        break;
      }
      case 'due':
      case 'scheduled': {
        const day = parseDay(change.day);
        if (day instanceof Error) {
          return { answer: { kind: 'problem', problem: day.message } };
        }
        todo[change.kind] = day;
        break;
      }
      case 'urgency':
        todo.urgency = change.urgency;
        break;
      case 'done':
        if (change.done !== (todo.done !== null)) {
          todo.done = change.done ? nowText() : null;
        }
        break;
      case 'link': {
        // Review F15: only a note of the workspace.
        const listed = MockTodos.find(notes, change.note);
        if (listed === undefined) {
          return {
            answer: {
              kind: 'problem',
              problem: `the note ${change.note} is not in this workspace`,
            },
          };
        }
        if (!todo.links.some((l) => l.toLowerCase() === listed.toLowerCase())) {
          todo.links.push(listed);
        }
        break;
      }
      case 'unlink':
        todo.links = todo.links.filter((l) => l.toLowerCase() !== change.link.toLowerCase());
        break;
    }
    if (JSON.stringify(todo) === JSON.stringify(before)) {
      return { answer: { kind: 'done' } };
    }
    const failed = this.#store(root, todo);
    if (failed !== null) {
      this.#unstored(root).set(number, { todo, why: failed });
      return {
        answer: { kind: 'done' },
        status: `Todo #${number} could not be stored: ${failed}`,
      };
    }
    this.#unstored(root).delete(number);
    return { answer: { kind: 'done' } };
  }

  mark(root: string, number: number, done: boolean): MockTodoDone {
    const before = this.shown(root, number);
    if (!before || (before.done !== null) === done) {
      return { answer: { kind: 'done' } };
    }
    const todo = { ...before, done: done ? nowText() : null };
    const failed = this.#store(root, todo);
    if (failed !== null) {
      return {
        answer: { kind: 'failed' },
        status: `Todo #${number} could not be stored: ${failed}`,
      };
    }
    const status = `Marked todo ${nameOf(todo)} ${done ? 'done' : 'open'}`;
    this.marked = { root, todo: before, status };
    return { answer: { kind: 'done' }, status, offer: 'mark' };
  }

  delete(root: string, number: number): MockTodoDone {
    const list = this.of(root);
    const todo = list.find((t) => t.number === number);
    if (!todo) {
      return {
        answer: { kind: 'failed' },
        status: `The todo could not be deleted: there is no todo #${number}`,
      };
    }
    if (this.writeFails !== null) {
      return {
        answer: { kind: 'failed' },
        status: `The todo could not be deleted: the todos cannot be stored: ${this.writeFails}`,
      };
    }
    this.todos[root] = list.filter((t) => t.number !== number);
    this.#unstored(root).delete(number);
    this.highest[root] = Math.max(this.highest[root] ?? 0, number);
    const status = `Deleted todo ${nameOf(todo)}`;
    this.deleted = { root, todo, status };
    return { answer: { kind: 'done' }, status, offer: 'delete' };
  }

  /** M-04.6: "Undo" while its report shows. */
  undo(root: string, status: string): MockTodoDone | null {
    const deleted = this.deleted;
    if (deleted?.root === root && deleted.status === status) {
      this.deleted = null;
      const failed = this.#store(root, deleted.todo);
      return failed === null
        ? { answer: { kind: 'done' }, status: `Todo ${nameOf(deleted.todo)} is back` }
        : {
            answer: { kind: 'failed' },
            status: `Todo ${nameOf(deleted.todo)} could not be brought back: ${failed}`,
          };
    }
    const marked = this.marked;
    if (marked?.root === root && marked.status === status) {
      this.marked = null;
      const failed = this.#store(root, marked.todo);
      return failed === null
        ? { answer: { kind: 'done' }, status: `Todo ${nameOf(marked.todo)} is as before` }
        : {
            answer: { kind: 'failed' },
            status: `Todo #${marked.todo.number} could not be stored: ${failed}`,
          };
    }
    return null;
  }

  /** M-04.6: the offer of the status `status`. */
  offer(root: string | null, status: string): 'undoTodoDelete' | 'undoTodoMark' | null {
    if (this.deleted !== null && (this.deleted.root !== root || this.deleted.status !== status)) {
      this.deleted = null;
    }
    if (this.marked !== null && (this.marked.root !== root || this.marked.status !== status)) {
      this.marked = null;
    }
    return this.deleted ? 'undoTodoDelete' : this.marked ? 'undoTodoMark' : null;
  }

  /** As another program, a sync or an agent: `change` of a todo, created when new. */
  outside(root: string, change: Partial<MockTodo> & { number: number }): void {
    const list = this.of(root);
    const at = list.findIndex((t) => t.number === change.number);
    const base: MockTodo = list[at] ?? {
      number: change.number,
      title: `Todo ${change.number}`,
      urgency: 'normal',
      due: null,
      scheduled: null,
      created: nowText(),
      done: null,
      links: [],
    };
    const todo = { ...base, ...change };
    if (at < 0) {
      list.push(todo);
      list.sort((a, b) => a.number - b.number);
    } else {
      list[at] = todo;
    }
    this.highest[root] = Math.max(this.highest[root] ?? 0, todo.number);
  }

  /** M-19.10: "Take remote" of todo #3 of Team takes the remote's title and urgency. */
  resolve(root: string, number: number, mine: boolean): void {
    if (!mine && number === 3) {
      this.outside(root, { number: 3, title: 'Send the agenda today', urgency: 'high' });
    }
  }

  candidates(root: string, notes: string[], number: number, query: string): TodoCandidatesView {
    const typed = query.trim().toLowerCase();
    const todo = this.shown(root, number);
    if (typed === '' || !todo) {
      return { notes: [], none: null };
    }
    const found = notes
      .filter(
        (note) =>
          !todo.links.some((l) => l.toLowerCase() === note.toLowerCase()) &&
          titleOf(note).toLowerCase().includes(typed),
      )
      .slice(0, 8)
      .map((note) => ({ note, label: note.replace(/-/g, ' ') }));
    return { notes: found, none: found.length === 0 ? 'No note matches' : null };
  }

  /** The note `link` of the workspace, case aside. */
  static find(notes: string[], link: string): string | undefined {
    return notes.find((note) => note.toLowerCase() === link.toLowerCase());
  }

  /** `Session::todos_view`, roughly. */
  view(root: string, notes: string[], active: string | null): TodosView {
    const today = mockDay();
    const list = this.list(root);
    const all = this.#all(root);
    const unstored = this.#unstored(root);
    const overdueDays = (t: MockTodo) =>
      t.due !== null && t.done === null && t.due < today ? daysBetween(t.due, today) : null;
    const views: TodoView[] = all.map((t) => {
      const days = overdueDays(t);
      return {
        number: t.number,
        title: t.title,
        name: nameOf(t),
        urgency: t.urgency,
        urgencyLabel: URGENCY[t.urgency].label,
        done: t.done !== null,
        due: t.due,
        scheduled: t.scheduled,
        overdue:
          days === null
            ? null
            : `${t.due}  ${days === 1 ? '1 day overdue' : `${days} days overdue`}`,
        scheduledAfterDue: t.scheduled !== null && t.due !== null && t.scheduled > t.due,
        links: t.links.map((link) => ({
          link,
          title: titleOf(link),
          exists: MockTodos.find(notes, link) !== undefined,
        })),
        moreLinks: t.links.length > 2 ? `+${t.links.length - 2}` : null,
        linksSummary:
          t.links.slice(0, 2).map(titleOf).join(', ') +
          (t.links.length > 2 ? ` +${t.links.length - 2}` : ''),
        linksActive:
          active !== null && t.links.some((l) => l.toLowerCase() === active.toLowerCase()),
        stamp: t.done !== null ? `done ${minutes(t.done)}` : `created ${minutes(t.created)}`,
        doneAt: t.done === null ? null : minutes(t.done),
        unstored: unstored.get(t.number)?.why ?? null,
      };
    });
    const open = all
      .filter((t) => t.done === null)
      .sort(
        (a, b) =>
          Number(a.due === null) - Number(b.due === null) ||
          (a.due ?? '').localeCompare(b.due ?? '') ||
          a.number - b.number,
      );
    const keeps = (t: MockTodo) =>
      list.filter === 'all' || (list.filter === 'done') === (t.done !== null);
    const byDue = (a: MockTodo, b: MockTodo) =>
      Number(a.due === null) - Number(b.due === null) || (a.due ?? '').localeCompare(b.due ?? '');
    const byUrgency = (a: MockTodo, b: MockTodo) =>
      URGENCY[b.urgency].rank - URGENCY[a.urgency].rank;
    const rows = all
      .filter(keeps)
      .sort((a, b) =>
        list.sort === 'due'
          ? byDue(a, b) || byUrgency(a, b) || a.number - b.number
          : byUrgency(a, b) || byDue(a, b) || a.number - b.number,
      )
      .map((t) => t.number);
    const overdue = all.filter((t) => overdueDays(t) !== null).length;
    const empty =
      rows.length > 0
        ? null
        : list.filter === 'open'
          ? 'No open todos. Type a title above to create one.'
          : list.filter === 'done'
            ? 'No done todos.'
            : 'No todos. Type a title above to create one.';
    return {
      heading: `Todos (${open.length})`,
      openCount: open.length,
      todos: views,
      open: open.map((t) => t.number),
      done: all.filter((t) => t.done !== null).map((t) => t.number),
      unreadable: [...this.unreadable],
      overdue: overdue === 0 ? null : overdue === 1 ? '1 todo overdue' : `${overdue} todos overdue`,
      activeNote: active === null ? null : { note: active, linkLabel: `Link ${titleOf(active)}` },
      table: {
        sort: list.sort,
        filter: list.filter,
        rows,
        empty,
        doneHeading: list.filter === 'open' ? '' : 'Done',
      },
    };
  }
}

const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

/** M-18.3: `calendar_view`, roughly: the month of `day` (today without one), `step` on. */
export function mockCalendar(day: string | null, step: number): CalendarView {
  const parsed = day === null ? null : parseDay(day);
  const base = typeof parsed === 'string' ? parsed : mockDay();
  const first = new Date(+base.slice(0, 4), +base.slice(5, 7) - 1 + step, 1);
  const days = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
  const today = mockDay();
  const month = mockDay(0, first);
  return {
    title: `${MONTHS[first.getMonth()]} ${first.getFullYear()}`,
    month,
    lead: (first.getDay() + 6) % 7,
    days: Array.from({ length: days }, (_, at) => {
      const date = mockDay(at, first);
      return { day: date, label: at + 1, today: date === today };
    }),
  };
}
