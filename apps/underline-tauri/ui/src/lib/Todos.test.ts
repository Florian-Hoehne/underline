// Feature F15: the todos (UC-020, UC-021; inventory M-04.5, M-04.6, M-06.5,
// M-16.3, M-18) against the mock backend's todos (`mock-todos.ts`):
// `/home/user/Notes` starts with #1 "Book the ferry" (overdue), #2 "Renew
// passport" (critical, four notes), #3 "Read chapter 4" (a missing note) and
// #4 "Write the log" (done); `/home/user/Work` with none.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHooks, TEAM, type MockStart } from './mock-backend';
import { mockCalendar, mockDay } from './mock-todos';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
const WORK: MockStart = { givenDir: '/home/user/Work' };

async function app(start: MockStart = NOTES) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  await waitFor(() => expect(shell.todos).not.toBeNull());
  return shell;
}

const bar = () => screen.getByRole('contentinfo');
const tools = () => screen.getByRole('complementary', { name: 'Tools' });
const tabs = () => screen.getByRole('tablist', { name: 'Open notes' });
const todosTab = () => within(tabs()).getByRole('tab', { name: /^Todos/ });
const table = () => screen.getByRole('table', { name: 'Todos' });
/** A day as screen readers hear it in the calendar. */
const spoken = (day: string) =>
  new Date(+day.slice(0, 4), +day.slice(5, 7) - 1, +day.slice(8, 10)).toLocaleDateString('en-GB', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
const rowTitle = (title: string) =>
  within(table()).getByRole('button', { name: new RegExp(`^${title}`) });
const statusText = () => bar().querySelector('.text')?.textContent ?? '';
const title = (name: RegExp) => within(tools()).getByRole('button', { name });

describe('M-16.3, M-18.1, M-18.2 the section "Todos" (UC-020)', () => {
  it('lists the open todos by due day, the done ones folded, with their colours', async () => {
    await app();

    expect(within(tools()).getByText('Todos (3)')).toBeInTheDocument();
    const open = within(tools()).getByRole('list', { name: 'Open todos' });
    const rows = within(open)
      .getAllByRole('button', { expanded: false })
      .map((b) => b.textContent?.replace(/\s+/g, ' '));
    // Review F15: "!" and "!!" besides the colour, hidden from screen readers.
    expect(rows).toEqual([
      '#1 Book the ferry!, urgency high',
      '#2 Renew passport!!, urgency critical',
      '#3 Read chapter 4',
    ]);
    expect(title(/^#1 Book the ferry, urgency high$/)).toBeInTheDocument();
    expect(title(/^#1 Book the ferry/)).toHaveClass('urgency-high');
    expect(title(/^#3 Read chapter 4/)).toHaveClass('urgency-low');
    expect(within(open).getByText(`due ${mockDay(-3)}`)).toBeInTheDocument();
    expect(within(tools()).getByText('Done (1)')).toBeInTheDocument();
    expect(within(tools()).getByRole('button', { name: 'Open list' })).toHaveAttribute(
      'title',
      'The tab "Todos": all todos, sorted and filtered',
    );
  });

  it('creates a todo with Enter and keeps the keyboard; a missing title is said (A3)', async () => {
    const shell = await app();
    const field = within(tools()).getByRole('textbox', { name: 'New todo' });
    field.focus();

    await fireEvent.keyDown(field, { key: 'Enter' });
    expect(await within(tools()).findByRole('alert')).toHaveTextContent('a todo needs a title');

    await fireEvent.input(field, { target: { value: 'Buy milk' } });
    await fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(statusText()).toContain('Created todo #5 Buy milk'));
    await waitFor(() => expect(title(/^#5 Buy milk/)).toBeInTheDocument());
    await waitFor(() => expect(field).toHaveValue(''));
    expect(field).toHaveFocus();
    expect(within(tools()).queryByRole('alert')).toBeNull();
    expect(shell.todos?.heading).toBe('Todos (4)');
  });

  it('says "No open todos" in a workspace without any', async () => {
    await app(WORK);

    expect(within(tools()).getByText('Todos (0)')).toBeInTheDocument();
    expect(within(tools()).getByText('No open todos')).toBeInTheDocument();
  });

  it('marks a todo done and open again with its check box, without an undo', async () => {
    await app();

    await fireEvent.click(
      within(tools()).getByRole('checkbox', { name: 'Done: #3 Read chapter 4' }),
    );

    await waitFor(() => expect(within(tools()).getByText('Todos (2)')).toBeInTheDocument());
    expect(within(tools()).getByText('Done (2)')).toBeInTheDocument();
    expect(within(bar()).queryByRole('button', { name: 'Undo' })).toBeNull();
  });
});

describe('M-18.3 to M-18.5 the fields of a todo (UC-020 steps 5–10)', () => {
  it('stores the fields when left and says a wrong day', async () => {
    const shell = await app();
    await fireEvent.click(title(/^#1 Book the ferry/));
    const fields = within(tools()).getByRole('group', { name: 'Todo #1 Book the ferry' });

    const due = within(fields).getByRole('textbox', { name: 'Due' });
    expect(due).toHaveValue(mockDay(-3));
    expect(due).toHaveAttribute('placeholder', 'YYYY-MM-DD');
    await fireEvent.input(due, { target: { value: '2030-13-40' } });
    await fireEvent.change(due);
    expect(await within(fields).findByRole('alert')).toHaveTextContent(
      '`2030-13-40` is not a day; write it as YYYY-MM-DD',
    );
    expect(due).toHaveValue('2030-13-40');

    await fireEvent.input(due, { target: { value: '2030-01-02' } });
    await fireEvent.change(due);
    await waitFor(() => expect(within(fields).queryByRole('alert')).toBeNull());
    expect(mockHooks().todos()[0]?.due).toBe('2030-01-02');

    const urgency = within(fields).getByRole('combobox', { name: 'Urgency' });
    await fireEvent.change(urgency, { target: { value: 'critical' } });
    await waitFor(() => expect(mockHooks().todos()[0]?.urgency).toBe('critical'));

    const titleField = within(fields).getByRole('textbox', { name: 'Title' });
    await fireEvent.input(titleField, { target: { value: 'Book the night ferry' } });
    await fireEvent.change(titleField);
    await waitFor(() => expect(mockHooks().todos()[0]?.title).toBe('Book the night ferry'));
    expect(shell.todoShown).toBe(1);
    expect(within(fields).getByText(/^created /)).toBeInTheDocument();
  });

  it('shows the hint when the scheduled day is after the due day', async () => {
    await app();
    await fireEvent.click(title(/^#2 Renew passport/));

    expect(
      within(tools()).getByText('The scheduled day is after the due day.'),
    ).toBeInTheDocument();
  });

  it('"Pick" chooses a day in the calendar, stored at once; "No day" takes it away', async () => {
    await app();
    await fireEvent.click(title(/^#3 Read chapter 4/));
    const fields = within(tools()).getByRole('group', { name: 'Todo #3 Read chapter 4' });

    const pick = within(fields).getByRole('button', { name: 'Pick the due day' });
    expect(pick).toHaveAttribute('title', 'Choose a day in a calendar');
    await fireEvent.click(pick);
    const calendar = await screen.findByRole('dialog', { name: 'Choose a day' });
    const month = mockCalendar(null, 0);
    expect(within(calendar).getByText(month.title)).toBeInTheDocument();
    expect(within(calendar).getByText('Mo')).toBeInTheDocument();
    const today = within(calendar).getByRole('button', { name: `${spoken(mockDay())}, today` });
    expect(today).toHaveClass('today');
    await waitFor(() => expect(today).toHaveFocus());

    await fireEvent.click(within(calendar).getByRole('button', { name: 'Next month' }));
    await waitFor(() =>
      expect(within(calendar).getByText(mockCalendar(null, 1).title)).toBeInTheDocument(),
    );
    const day = mockCalendar(null, 1).days[14]?.day ?? '';
    await fireEvent.click(within(calendar).getByRole('button', { name: spoken(day) }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Choose a day' })).toBeNull());
    await waitFor(() => expect(mockHooks().todos()[2]?.due).toBe(day));
    expect(within(fields).getByRole('textbox', { name: 'Due' })).toHaveValue(day);

    await fireEvent.click(within(fields).getByRole('button', { name: 'Pick the due day' }));
    const again = await screen.findByRole('dialog', { name: 'Choose a day' });
    await fireEvent.click(within(again).getByRole('button', { name: 'No day' }));
    await waitFor(() => expect(mockHooks().todos()[2]?.due).toBeNull());
    expect(within(fields).getByRole('textbox', { name: 'Due' })).toHaveValue('');
  });

  it('links the active note and another one, opens, says missing and removes links', async () => {
    const shell = await app();
    await fireEvent.click(title(/^#3 Read chapter 4/));
    const fields = within(tools()).getByRole('group', { name: 'Todo #3 Read chapter 4' });

    const missing = within(fields).getByRole('button', { name: 'Old Plan (missing)' });
    expect(missing).toHaveAttribute('title', 'Old-Plan');
    await fireEvent.click(missing);
    await waitFor(() =>
      expect(statusText()).toContain('The note Old-Plan no longer exists; remove the link with ×'),
    );
    await fireEvent.click(
      within(fields).getByRole('button', { name: 'Remove the link to Old Plan' }),
    );
    await waitFor(() => expect(mockHooks().todos()[2]?.links).toEqual(['Reading-List']));

    await fireEvent.click(within(fields).getByRole('button', { name: 'Link Home' }));
    await waitFor(() => expect(mockHooks().todos()[2]?.links).toContain('Home'));
    await waitFor(() =>
      expect(within(fields).queryByRole('button', { name: 'Link Home' })).toBeNull(),
    );

    const query = within(fields).getByRole('textbox', { name: 'Link a note' });
    await fireEvent.input(query, { target: { value: 'zebra' } });
    expect(await within(fields).findByText('No note matches')).toBeInTheDocument();
    await fireEvent.input(query, { target: { value: 'trip' } });
    const offered = await within(fields).findByRole('list', { name: 'Notes to link' });
    await fireEvent.click(within(offered).getByRole('button', { name: 'Trip' }));
    await waitFor(() => expect(mockHooks().todos()[2]?.links).toContain('Trip'));
    expect(query).toHaveValue('');

    await fireEvent.click(within(fields).getByRole('button', { name: 'Trip' }));
    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
  });

  it('"Delete" without asking; "Undo" before the report brings it back (A5, M-04.6)', async () => {
    const shell = await app();
    await fireEvent.click(title(/^#2 Renew passport/));

    await fireEvent.click(
      within(tools()).getByRole('button', { name: 'Delete todo #2 Renew passport' }),
    );

    await waitFor(() => expect(statusText()).toContain('Deleted todo #2 Renew passport'));
    expect(shell.todoShown).toBeNull();
    // UX review F15: the keyboard goes to the next todo.
    await waitFor(() => expect(title(/^#3 Read chapter 4/)).toHaveFocus());
    const undo = within(bar()).getByRole('button', { name: 'Undo' });
    // egui: before the text.
    expect(
      undo.compareDocumentPosition(bar().querySelector('.text') as Node) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(bar().querySelector('.text')).toHaveTextContent('. Undo available');

    await fireEvent.click(undo);
    await waitFor(() => expect(statusText()).toContain('Todo #2 Renew passport is back'));
    expect(
      mockHooks()
        .todos()
        .map((t) => t.number),
    ).toEqual([1, 2, 3, 4]);
    expect(within(bar()).queryByRole('button', { name: 'Undo' })).toBeNull();
  });
});

describe('review F15: the fields of a todo', () => {
  it('a title not taken stays in the field (A3); only the wrong field is invalid', async () => {
    await app();
    await fireEvent.click(title(/^#1 Book the ferry/));
    const fields = within(tools()).getByRole('group', { name: 'Todo #1 Book the ferry' });
    const titleField = within(fields).getByRole('textbox', { name: 'Title' });
    const due = within(fields).getByRole('textbox', { name: 'Due' });

    await fireEvent.input(titleField, { target: { value: '  ' } });
    await fireEvent.change(titleField);

    expect(await within(fields).findByRole('alert')).toHaveTextContent('a todo needs a title');
    expect(titleField).toHaveValue('  ');
    expect(titleField).toHaveAttribute('aria-invalid', 'true');
    expect(due).toHaveAttribute('aria-invalid', 'false');
    expect(due).toHaveAccessibleDescription('YYYY-MM-DD');
  });

  it('says why a todo is not stored below its fields', async () => {
    await app();
    mockHooks().todoWriteFails('Permission denied (os error 13)');
    await fireEvent.click(title(/^#1 Book the ferry/));
    const fields = within(tools()).getByRole('group', { name: 'Todo #1 Book the ferry' });

    await fireEvent.change(within(fields).getByRole('combobox', { name: 'Urgency' }), {
      target: { value: 'low' },
    });

    expect(
      await within(fields).findByText(
        'Not stored: the todos cannot be stored: Permission denied (os error 13)',
      ),
    ).toBeInTheDocument();
  });

  it('the calendar goes on into the next month with the arrows and PageDown', async () => {
    await app();
    await fireEvent.click(title(/^#3 Read chapter 4/));
    await fireEvent.click(within(tools()).getByRole('button', { name: 'Pick the due day' }));
    const calendar = await screen.findByRole('dialog', { name: 'Choose a day' });
    const month = mockCalendar(null, 0);
    const last = month.days.at(-1)?.day ?? '';
    const lastButton = within(calendar).getByRole('button', { name: new RegExp(spoken(last)) });
    lastButton.focus();

    await fireEvent.keyDown(lastButton, { key: 'ArrowRight' });
    const next = mockCalendar(null, 1);
    await waitFor(() => expect(within(calendar).getByText(next.title)).toBeInTheDocument());
    await waitFor(() =>
      expect(
        within(calendar).getByRole('button', { name: new RegExp(spoken(next.days[0]?.day ?? '')) }),
      ).toHaveFocus(),
    );
    await fireEvent.keyDown(document.activeElement as Element, { key: 'PageDown' });
    await waitFor(() =>
      expect(within(calendar).getByText(mockCalendar(null, 2).title)).toBeInTheDocument(),
    );
  });
});

describe('M-06.5, M-18.6 to M-18.8 the tab "Todos" (UC-021)', () => {
  it('"Open list" shows the tab with the open todos by due day; a note leaves it open', async () => {
    const shell = await app();

    await fireEvent.click(within(tools()).getByRole('button', { name: 'Open list' }));

    await waitFor(() => expect(todosTab()).toHaveAttribute('aria-selected', 'true'));
    expect(todosTab()).toHaveTextContent('Todos (3)');
    expect(within(tabs()).getByRole('tab', { name: 'Home' })).toHaveAttribute(
      'aria-selected',
      'false',
    );
    const rows = within(table()).getAllByRole('row').slice(1);
    expect(rows.map((r) => r.querySelectorAll('td')[1]?.textContent)).toEqual(['#1', '#2', '#3']);
    const overdue = within(rows[0] as HTMLElement).getByText(/days overdue/);
    // Two spaces, as egui.
    expect(overdue.textContent).toBe(`${mockDay(-3)}  3 days overdue`);
    expect(overdue).toHaveClass('overdue');
    expect(within(rows[1] as HTMLElement).getByText('+2')).toHaveAttribute(
      'title',
      'Trip, 2026 11 09 +2',
    );
    // The tool panel shows the todos beside it, but no note to link.
    expect(within(tools()).getByText('Todos (3)')).toBeInTheDocument();
    await waitFor(() => expect(shell.todos?.activeNote).toBeNull());

    // UC-021 step 5–6: a linked note opens; the tab "Todos" stays open.
    await fireEvent.click(within(rows[0] as HTMLElement).getByRole('button', { name: 'Trip' }));
    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
    expect(todosTab()).toHaveAttribute('aria-selected', 'false');
    expect(screen.queryByRole('table', { name: 'Todos' })).toBeNull();

    await fireEvent.click(todosTab());
    await waitFor(() => expect(todosTab()).toHaveAttribute('aria-selected', 'true'));
  });

  it('sorts by urgency, filters done and all, and says when none is shown (A1, A3)', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => table());

    await fireEvent.click(screen.getByRole('button', { name: 'Urgency', pressed: false }));
    await waitFor(() => expect(shell.todos?.table.rows).toEqual([2, 1, 3]));
    expect(within(table()).getByRole('columnheader', { name: 'Urgency' })).toHaveAttribute(
      'aria-sort',
      'descending',
    );

    await fireEvent.click(screen.getByRole('button', { name: 'Done', pressed: false }));
    await waitFor(() => expect(shell.todos?.table.rows).toEqual([4]));
    expect(within(table()).getByRole('columnheader', { name: 'Done' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'All', pressed: false }));
    await waitFor(() => expect(shell.todos?.table.rows).toEqual([2, 1, 4, 3]));

    // The headings sort too (BR-007).
    await fireEvent.click(within(table()).getByRole('button', { name: 'Due' }));
    await waitFor(() => expect(shell.todos?.table.sort).toBe('due'));
  });

  it('says "No open todos. Type a title above to create one." without any', async () => {
    const shell = await app(WORK);
    await shell.showTodoList();

    expect(
      await screen.findByText('No open todos. Type a title above to create one.'),
    ).toBeInTheDocument();
    const field = within(screen.getByRole('tabpanel')).getByRole('textbox', { name: 'New todo' });
    await fireEvent.input(field, { target: { value: 'First' } });
    await fireEvent.keyDown(field, { key: 'Enter' });
    await waitFor(() => expect(within(table()).getByText('First')).toBeInTheDocument());
  });

  it('Up, Down, Space and Enter work through the rows, with "Undo" (BR-008, M-04.6)', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => table());
    // One stop of Tab: only the first row's controls before one is chosen.
    expect(rowTitle('Book the ferry')).toHaveAttribute('tabindex', '0');
    expect(rowTitle('Renew passport')).toHaveAttribute('tabindex', '-1');
    rowTitle('Book the ferry').focus();

    await fireEvent.keyDown(document.activeElement as Element, { key: 'ArrowDown' });
    await waitFor(() => expect(rowTitle('Renew passport')).toHaveFocus());
    expect(shell.todoChosen).toBe(2);
    await fireEvent.keyDown(document.activeElement as Element, { key: 'ArrowDown' });
    await fireEvent.keyDown(document.activeElement as Element, { key: 'ArrowDown' });
    await waitFor(() => expect(shell.todoChosen).toBe(3));
    await fireEvent.keyDown(document.activeElement as Element, { key: 'ArrowUp' });
    await waitFor(() => expect(rowTitle('Renew passport')).toHaveFocus());
    expect(rowTitle('Renew passport')).toHaveAttribute('tabindex', '0');

    // Space marks it; it leaves "Open", and the keyboard goes to the next row.
    await fireEvent.keyDown(document.activeElement as Element, { key: ' ' });
    await waitFor(() => expect(statusText()).toContain('Marked todo #2 Renew passport done'));
    await waitFor(() => expect(shell.todos?.table.rows).toEqual([1, 3]));
    await waitFor(() => expect(rowTitle('Read chapter 4')).toHaveFocus());
    await fireEvent.click(within(bar()).getByRole('button', { name: 'Undo' }));
    await waitFor(() => expect(statusText()).toContain('Todo #2 Renew passport is as before'));
    await waitFor(() => expect(shell.todos?.table.rows).toEqual([1, 2, 3]));

    rowTitle('Book the ferry').focus();
    await fireEvent.keyDown(rowTitle('Book the ferry'), { key: 'Enter' });
    await waitFor(() => expect(shell.tabs.active).toBe('Trip'));
    expect(shell.tabs.todoList).toEqual({ open: true, active: false });
  });

  it('a filter or sort chosen is said politely; a failed mark shows the check box as it is', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => table());

    await fireEvent.click(screen.getByRole('button', { name: 'Urgency', pressed: false }));
    await waitFor(() =>
      expect(screen.getByText('3 open todos, sorted by urgency')).toBeInTheDocument(),
    );

    mockHooks().todoWriteFails('Permission denied (os error 13)');
    const box = within(table()).getByRole('checkbox', { name: 'Done: #3 Read chapter 4' });
    await fireEvent.click(box);
    await waitFor(() => expect(statusText()).toContain('Todo #3 could not be stored'));
    await waitFor(() => expect(box).not.toBeChecked());
    expect(shell.todos?.table.rows).toEqual([2, 1, 3]);
  });

  it('the title of a row shows the todo in the section (A4)', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => table());

    await fireEvent.click(within(table()).getByRole('button', { name: 'Renew passport' }));

    expect(shell.todoChosen).toBe(2);
    expect(
      within(tools()).getByRole('group', { name: 'Todo #2 Renew passport' }),
    ).toBeInTheDocument();
  });

  it('"×", a middle click or Delete closes the tab', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => todosTab());

    await fireEvent.click(within(tabs()).getByRole('button', { name: 'Close Todos' }));
    await waitFor(() => expect(within(tabs()).queryByRole('tab', { name: /^Todos/ })).toBeNull());
    expect(shell.tabs.todoList).toEqual({ open: false, active: false });

    await shell.showTodoList();
    await waitFor(() => todosTab());
    await fireEvent(todosTab(), new MouseEvent('auxclick', { bubbles: true, button: 1 }));
    await waitFor(() => expect(within(tabs()).queryByRole('tab', { name: /^Todos/ })).toBeNull());

    await shell.showTodoList();
    await waitFor(() => todosTab());
    todosTab().focus();
    await fireEvent.keyDown(todosTab(), { key: 'Delete' });
    await waitFor(() => expect(within(tabs()).queryByRole('tab', { name: /^Todos/ })).toBeNull());
  });

  it('Q-88: Ctrl+Shift+E does nothing while the tab "Todos" is active', async () => {
    const shell = await app();
    await shell.showTodoList();
    await waitFor(() => table());

    await fireEvent.keyDown(document.body, { key: 'E', ctrlKey: true, shiftKey: true });

    expect(screen.queryByRole('dialog', { name: 'Export note' })).toBeNull();
    expect(shell.exporting).toBeNull();
  });
});

describe('M-04.5 the overdue todos of the status bar (UC-021 A2)', () => {
  it('"1 todo overdue" opens the tab "Todos" with the filter "Open"', async () => {
    const shell = await app();
    await shell.showTodoList();
    await shell.setTodoList('due', 'done');
    await shell.closeTodoList();

    const overdue = await within(bar()).findByRole('button', { name: '1 todo overdue' });
    expect(overdue).toHaveAttribute('title', 'Open the todo list');
    await fireEvent.click(overdue);

    await waitFor(() => expect(todosTab()).toHaveAttribute('aria-selected', 'true'));
    await waitFor(() => expect(shell.todos?.table.filter).toBe('open'));
  });

  it('is not shown while no todo is overdue', async () => {
    await app(WORK);

    expect(within(bar()).queryByRole('button', { name: /overdue/ })).toBeNull();
  });
});

describe('M-18.9 changes elsewhere and what cannot be stored (UC-020 A7 to A9)', () => {
  it('a todo changed outside or by an agent shows within a second', async () => {
    const shell = await app();

    mockHooks().todoOutside({ number: 9, title: 'From the sync', due: mockDay(1) });
    await waitFor(() => expect(title(/^#9 From the sync/)).toBeInTheDocument());

    await mockHooks().agentTodo('Claude', 'changed todo #1', { number: 1, urgency: 'low' });
    await waitFor(() => expect(statusText()).toContain('Claude changed todo #1'));
    await waitFor(() => expect(title(/^#1 Book the ferry$/)).toHaveClass('urgency-low'));
    expect(shell.todos?.heading).toBe('Todos (4)');
  });

  it('a change that cannot be stored is kept, marked "not stored", and stored later (A9)', async () => {
    await app();
    mockHooks().todoWriteFails('Permission denied (os error 13)');

    await fireEvent.click(
      within(tools()).getByRole('checkbox', { name: 'Done: #1 Book the ferry' }),
    );

    await waitFor(() =>
      expect(statusText()).toContain(
        'Todo #1 could not be stored: the todos cannot be stored: Permission denied (os error 13)',
      ),
    );
    const mark = await within(tools()).findByText('not stored');
    expect(mark.closest('[title]')).toHaveAttribute(
      'title',
      'the todos cannot be stored: Permission denied (os error 13)',
    );
    expect(within(tools()).getByText('Done (2)')).toBeInTheDocument();

    mockHooks().todoWriteFails(null);
    await waitFor(() => expect(within(tools()).queryByText('not stored')).toBeNull(), {
      timeout: 2000,
    });
    expect(mockHooks().todos()[0]?.done).not.toBeNull();
  });

  it('a todo file that cannot be read is a line of the section', async () => {
    await app();

    mockHooks().todoUnreadable(['todo #7 cannot be read: mapping values are not allowed']);

    expect(
      await within(tools()).findByText('todo #7 cannot be read: mapping values are not allowed'),
    ).toBeInTheDocument();
  });
});

describe('Q-100: the todos in conflict at the top of the section "Todos"', () => {
  it('stand in the section, before "Open list"', async () => {
    await app({ givenDir: TEAM });
    mockHooks().syncOutcome('conflict');
    await fireEvent.click(await within(bar()).findByRole('button', { name: 'Sync now' }));

    const conflict = await within(tools()).findByRole(
      'region',
      { name: 'Todo #3 was changed here and on the remote' },
      { timeout: 3000 },
    );
    const openList = within(tools()).getByRole('button', { name: 'Open list' });
    expect(
      conflict.compareDocumentPosition(openList) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(conflict.closest('details')?.querySelector('summary')).toHaveTextContent('Todos (2)');

    await fireEvent.click(
      within(conflict).getByRole('button', { name: 'Take remote for todo #3' }),
    );
    await waitFor(() => expect(title(/^#3 Send the agenda today/)).toBeInTheDocument());
  });
});

describe('review F15: todos not stored before another workspace opens', () => {
  it('"Changes not stored" asks; "Close anyway" opens the other without them', async () => {
    const shell = await app();
    mockHooks().todoWriteFails('Permission denied (os error 13)');
    await fireEvent.click(
      within(tools()).getByRole('checkbox', { name: 'Done: #3 Read chapter 4' }),
    );
    await within(tools()).findByText('not stored');

    await shell.openWorkspace('/home/user/Work');

    const question = await screen.findByRole('dialog', { name: 'Changes not stored' });
    expect(
      within(question).getByText('Close the workspace Notes anyway and lose these changes?'),
    ).toBeInTheDocument();
    expect(shell.workspace?.root).toBe('/home/user/Notes');
    await fireEvent.click(within(question).getByRole('button', { name: 'Close anyway' }));

    await waitFor(() => expect(shell.workspace?.root).toBe('/home/user/Work'));
    await waitFor(() => expect(statusText()).toContain('Opened /home/user/Work'));
    expect(screen.queryByRole('dialog', { name: 'Changes not stored' })).toBeNull();
  });
});
