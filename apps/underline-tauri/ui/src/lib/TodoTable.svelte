<!--
  M-18.6 to M-18.8 (UC-021): the tab "Todos" in place of a note, as egui's
  `todo_table`.

  On top: "Todos", the filters "Open", "Done" and "All" (BR-003), "Sort by"
  with "Due date" and "Urgency" (BR-001, BR-002), and the field "New todo"
  (320 px; Enter creates the todo, the keyboard stays, A3 below it). Sort
  and filter are kept for the workspace (BR-005).

  Without a row: "No open todos. Type a title above to create one.", "No
  done todos." or "No todos. Type a title above to create one." (A3).

  The table, striped: the check box "Done" (hover "Done (Space)"), "#N",
  the title (struck through when done; hover "Show and change"), which
  shows the todo in the section "Todos" of the tool panel (A4); "Due",
  overdue ones in red as "{due}  N day(s) overdue" (A2); "Urgency" in its
  colour; "Scheduled"; up to two linked notes, each opening its note
  (A6: a missing one is said in the status bar), then "+N" with all at the
  pointer (BR-009); and when done, the time ("Done", empty under "Open").
  The headings "Due" and "Urgency" sort too (BR-007).

  Keyboard (BR-008, UX review F15, A-181): a plain table whose rows are one
  stop of Tab: only the chosen row's controls (the first row's before one
  is chosen) are in the Tab sequence. Up and Down choose the row above or
  below and move the keyboard to the same control there; Space on any
  control of a row but its check box (which marks by itself) marks the todo
  done or open again; Enter on its title opens the first linked note (a
  title without notes shows the todo, A4). When a marked todo leaves the
  filter, the keyboard goes to the title of the next row, else of the one
  before, else to "New todo". A mark that could not be stored shows the
  check box as it is. egui takes the keys wherever no text field has the
  keyboard.

  Review F15: the head of the table stays in view while it scrolls; the
  sorted column shows its direction (▲ by due date, ▼ by urgency, for
  screen readers `aria-sort`); a change of filter or sort is said
  politely, such as "3 open todos, sorted by due date" (new text, Q-112);
  weak texts reach 4.5:1.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { TodoFilterName, TodoSortName, TodoView } from './bindings';
  import type { Shell } from './shell.svelte';
  import { NOTE_PANEL_ID, TODO_TAB_ID } from './tabs';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const FILTERS: { value: TodoFilterName; label: string }[] = [
    { value: 'open', label: 'Open' },
    { value: 'done', label: 'Done' },
    { value: 'all', label: 'All' },
  ];
  const SORTS: { value: TodoSortName; label: string }[] = [
    { value: 'due', label: 'Due date' },
    { value: 'urgency', label: 'Urgency' },
  ];

  const todos = $derived(shell.todos);
  const table = $derived(todos?.table ?? null);
  const byNumber = $derived(new Map((todos?.todos ?? []).map((todo) => [todo.number, todo])));
  const rows = $derived((table?.rows ?? []).flatMap((n) => byNumber.get(n) ?? ([] as TodoView[])));
  const chosenAt = $derived(rows.findIndex((todo) => todo.number === shell.todoChosen));
  const problemId = `todo-table-problem-${Math.random().toString(36).slice(2)}`;

  let grid = $state<HTMLElement | null>(null);
  let newField = $state<HTMLInputElement | null>(null);

  const rowId = (number: number) => `todo-row-${number}`;
  /** The row whose controls are in the Tab sequence. */
  const reachable = $derived(chosenAt >= 0 ? chosenAt : 0);

  /** Where the keyboard goes once the marked todo `gone` left the rows. */
  let refocus: { gone: number; at: number } | null = null;

  /** What a change of filter or sort says politely. */
  let summary = $state('');
  let told: string | null = null;

  $effect(() => {
    if (table === null) {
      return;
    }
    const shown = `${table.filter}:${table.sort}`;
    if (told !== null && told !== shown) {
      const n = table.rows.length;
      const what = { open: 'open todo', done: 'done todo', all: 'todo' }[table.filter];
      summary = `${n} ${what}${n === 1 ? '' : 's'}, sorted by ${
        table.sort === 'due' ? 'due date' : 'urgency'
      }`;
    }
    told = shown;
  });

  $effect(() => {
    const now = rows;
    const wanted = refocus;
    if (wanted === null || now.some((todo) => todo.number === wanted.gone)) {
      return;
    }
    refocus = null;
    const next = now[Math.min(wanted.at, now.length - 1)];
    if (next === undefined) {
      void tick().then(() => newField?.focus());
      return;
    }
    shell.chooseTodo(next.number);
    void tick().then(() => focusIn(next.number, 'title'));
  });

  function focusIn(number: number, column: string) {
    const row = grid?.querySelector(`#${rowId(number)}`);
    const target =
      row?.querySelector<HTMLElement>(`[data-column="${column}"]`) ??
      row?.querySelector<HTMLElement>('[data-column="title"]');
    target?.focus();
    row?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }

  /** UC-021 step 7: marks `todo`; a check box that could not be changed shows it as it is. */
  async function mark(todo: TodoView, done: boolean, box: HTMLInputElement | null = null) {
    const at = rows.findIndex((t) => t.number === todo.number);
    shell.chooseTodo(todo.number);
    refocus = { gone: todo.number, at };
    const answer = await shell.markTodo(todo.number, done);
    if (answer?.kind !== 'done') {
      refocus = null;
    }
    if (box !== null) {
      box.checked = shell.todoDone(todo.number);
    }
  }

  function newTodoKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      void shell.createTodo();
    }
  }

  function setSort(sort: TodoSortName) {
    if (table !== null && table.sort !== sort) {
      void shell.setTodoList(sort, table.filter);
    }
  }

  function setFilter(filter: TodoFilterName) {
    if (table !== null && table.filter !== filter) {
      void shell.setTodoList(table.sort, filter);
    }
  }

  /** UC-021 BR-008: Up and Down choose a row, Space marks it, Enter opens its note. */
  function rowKey(event: KeyboardEvent, todo: TodoView) {
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    const target = event.target as HTMLElement;
    const at = rows.findIndex((t) => t.number === todo.number);
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const next =
        rows[event.key === 'ArrowDown' ? Math.min(at + 1, rows.length - 1) : Math.max(at - 1, 0)];
      if (next) {
        shell.chooseTodo(next.number);
        const column = target.dataset.column ?? 'title';
        void tick().then(() => focusIn(next.number, column));
      }
    } else if (event.key === ' ' && target.dataset.column !== 'done') {
      event.preventDefault();
      void mark(todo, !todo.done);
    } else if (event.key === 'Enter' && target.dataset.column === 'title') {
      event.preventDefault();
      shell.chooseTodo(todo.number);
      const link = todo.links[0];
      if (link) {
        void shell.openTodoLink(link.link);
      } else {
        shell.showTodo(todo.number);
      }
    }
  }
</script>

<div class="todo-list" role="tabpanel" id={NOTE_PANEL_ID} aria-labelledby={TODO_TAB_ID}>
  <div class="head">
    <h2 class="heading">Todos</h2>
    <div class="choices" role="group" aria-label="Show">
      {#each FILTERS as filter (filter.value)}
        <button
          type="button"
          class="choice"
          aria-pressed={table?.filter === filter.value}
          onclick={() => setFilter(filter.value)}>{filter.label}</button
        >
      {/each}
    </div>
    <span class="weak" id="todo-sort-by">Sort by</span>
    <div class="choices" role="group" aria-labelledby="todo-sort-by">
      {#each SORTS as sort (sort.value)}
        <button
          type="button"
          class="choice"
          aria-pressed={table?.sort === sort.value}
          onclick={() => setSort(sort.value)}>{sort.label}</button
        >
      {/each}
    </div>
  </div>
  <div class="new">
    <input
      type="text"
      class="field"
      placeholder="New todo"
      aria-label="New todo"
      aria-describedby={shell.todoNewProblem ? problemId : undefined}
      aria-invalid={shell.todoNewProblem !== null}
      value={shell.todoNewTitle}
      oninput={(event) => shell.editNewTodo((event.currentTarget as HTMLInputElement).value)}
      onkeydown={newTodoKey}
      bind:this={newField}
    />
    {#if shell.todoNewProblem}
      <p class="problem" id={problemId} role="alert">{shell.todoNewProblem}</p>
    {/if}
  </div>
  {#if table === null}
    <!-- Before the todos were read. -->
  {:else if table.empty !== null}
    <p class="weak empty">{table.empty}</p>
  {:else}
    <div class="scroll">
      <table aria-label="Todos" bind:this={grid}>
        <thead>
          <tr>
            <th scope="col"><span class="visually-hidden">Mark done</span></th>
            <th scope="col">#</th>
            <th scope="col">Title</th>
            <th scope="col" aria-sort={table.sort === 'due' ? 'ascending' : undefined}>
              <button
                type="button"
                class="sort"
                class:sorted={table.sort === 'due'}
                onclick={() => setSort('due')}
                >Due{#if table.sort === 'due'}<span aria-hidden="true"> ▲</span>{/if}</button
              >
            </th>
            <th scope="col" aria-sort={table.sort === 'urgency' ? 'descending' : undefined}>
              <button
                type="button"
                class="sort"
                class:sorted={table.sort === 'urgency'}
                onclick={() => setSort('urgency')}
                >Urgency{#if table.sort === 'urgency'}<span aria-hidden="true">
                    ▼</span
                  >{/if}</button
              >
            </th>
            <th scope="col">Scheduled</th>
            <th scope="col">Notes</th>
            <th scope="col">{table.doneHeading}</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as todo, index (todo.number)}
            {@const chosen = shell.todoChosen === todo.number}
            {@const tab = index === reachable ? 0 : -1}
            <tr
              id={rowId(todo.number)}
              class:chosen
              aria-current={chosen ? 'true' : undefined}
              onkeydown={(event) => rowKey(event, todo)}
            >
              <td>
                <input
                  type="checkbox"
                  data-column="done"
                  tabindex={tab}
                  checked={todo.done}
                  title="Done (Space)"
                  aria-label="Done: {todo.name}"
                  aria-keyshortcuts="ArrowUp ArrowDown"
                  onchange={(event) =>
                    void mark(
                      todo,
                      (event.currentTarget as HTMLInputElement).checked,
                      event.currentTarget as HTMLInputElement,
                    )}
                />
              </td>
              <td class="weak">#{todo.number}</td>
              <td>
                <button
                  type="button"
                  class="title"
                  class:done={todo.done}
                  class:chosen
                  data-column="title"
                  tabindex={tab}
                  title="Show and change"
                  aria-keyshortcuts="ArrowUp ArrowDown Space Enter"
                  onclick={() => shell.showTodo(todo.number)}
                  >{todo.title}{#if todo.done}<span class="visually-hidden">, done</span
                    >{/if}</button
                >
              </td>
              <td>
                {#if todo.overdue}
                  <span class="overdue">{todo.overdue}</span>
                {:else if todo.due}
                  {todo.due}
                {:else}
                  <span class="weak">–</span>
                {/if}
              </td>
              <td class="urgency-{todo.urgency}">{todo.urgencyLabel}</td>
              <td>
                {#if todo.scheduled}{todo.scheduled}{:else}<span class="weak">–</span>{/if}
              </td>
              <td>
                <span class="notes">
                  {#each todo.links.slice(0, 2) as link, at (link.link)}
                    <button
                      type="button"
                      data-column="link-{at}"
                      tabindex={tab}
                      class="link"
                      class:missing={!link.exists}
                      title={link.link}
                      onclick={() => void shell.openTodoLink(link.link)}>{link.title}</button
                    >
                  {/each}
                  {#if todo.moreLinks}
                    <span class="weak" title={todo.linksSummary}
                      >{todo.moreLinks}<span class="visually-hidden">: {todo.linksSummary}</span
                      ></span
                    >
                  {/if}
                </span>
              </td>
              <td class="weak">{todo.doneAt ?? ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
  <p class="visually-hidden" aria-live="polite">{summary}</p>
</div>

<style>
  .todo-list {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    min-height: 0;
    padding: var(--space-2) var(--space-3);
    box-sizing: border-box;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2) var(--space-3);
    align-items: center;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  .heading {
    margin: 0;
    font-size: 1.25rem;
  }

  .choices {
    display: flex;
    gap: 2px;
  }

  /* egui's selectable values: plain, the chosen one filled. */
  .choice,
  .sort {
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .choice:hover,
  .sort:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .choice[aria-pressed='true'],
  .sort.sorted {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 0 -2px 0 var(--ui-text);
  }

  .sort {
    padding: 0 var(--space-1);
    font-weight: 700;
  }

  .new {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  /* egui: 320 px. */
  .field {
    box-sizing: border-box;
    width: min(320px, 100%);
    min-height: 28px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  /* Q-05: problems in the text colour with a bar, not egui's orange. */
  .problem {
    margin: 0;
    padding-left: var(--space-2);
    border-left: 3px solid var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .weak {
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
  }

  .empty {
    margin: 0;
  }

  .scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  table {
    border-collapse: collapse;
  }

  /* Review F15: the head stays in view. */
  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--ui-surface);
  }

  th,
  td {
    padding: 3px 7px;
    text-align: left;
    vertical-align: middle;
    white-space: nowrap;
  }

  th {
    font-weight: 700;
  }

  /* egui's striped grid. */
  tbody tr:nth-child(odd) {
    background: color-mix(in srgb, var(--ui-rule) 30%, transparent);
  }

  /* The chosen row: a bar at its start, not by colour alone. */
  tr.chosen td:first-child {
    box-shadow: inset 3px 0 0 var(--ui-focus);
  }

  /* A 24 px target for the pointer (WCAG 2.5.8). */
  td input {
    width: 16px;
    height: 16px;
    margin: 4px;
  }

  .title {
    max-width: 40ch;
    min-height: 24px;
    padding: 0 var(--space-1);
    overflow: hidden;
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    text-overflow: ellipsis;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .title:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .title.done {
    text-decoration: line-through;
  }

  .title.chosen {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .overdue {
    color: var(--ui-overdue);
    white-space: pre;
  }

  .urgency-low {
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
  }

  .urgency-high {
    color: var(--ui-urgency-high);
  }

  .urgency-critical {
    color: var(--ui-urgency-critical);
  }

  .notes {
    display: inline-flex;
    gap: var(--space-2);
    align-items: center;
  }

  /* egui's links: the link colour, underlined. */
  .link {
    min-height: 24px;
    padding: 0;
    font: inherit;
    color: var(--palette-link, var(--ui-text));
    text-decoration: underline;
    cursor: pointer;
    background: none;
    border: 0;
  }

  .link.missing {
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
