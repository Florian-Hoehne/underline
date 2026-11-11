<!--
  M-16.3, M-18.1, M-18.2 (UC-020): the section "Todos" of the tool panel, as
  egui's `todos_ui`: "Todos (N)", open at first, N the open todos.

  At its top:
  - the todos in conflict after a sync (M-19.10, UC-020 A10,
    TodoConflicts.svelte; Q-100: here since F15, as in egui);
  - "Open list" (hover "The tab "Todos": all todos, sorted and filtered"),
    which shows the tab "Todos" (UC-021 step 1);
  - the field "New todo": Enter creates the todo and the keyboard stays
    ("Created todo {name}"); a title that is empty or too long is said
    under it and the text stays (A3);
  - a line per todo file that cannot be read.

  Then the open todos, the next due first and those without a due day
  last, "No open todos" when none is, and "Done (N)", folded at first.
  A row: the check box "Done" (done or open again, steps 9–10, A4), the
  title "#n title" in the colour of its urgency (Low weak, Normal text,
  High orange, Critical red; done ones struck through), "due {d}", and
  "not stored" with the reason at the pointer (A9). A click on the title
  shows the todo's fields below it (TodoDetails.svelte), or hides them.

  Before the todos were read the section shows nothing but its heading.
  For screen readers (new texts, Q-112): the urgency High and Critical,
  "done" and the reason a todo is not stored are said in words.

  Review F15:
  - the urgency High and Critical is not colour alone: "!" or "!!" after
    the title (A-189); a long title wraps beside its check box, "due …"
    and "not stored" stand under the title;
  - after "Delete" the keyboard goes to the title of the next todo of the
    list, else of the one before, else to "New todo";
  - a check box whose change could not be made shows the todo as it is;
  - weak texts reach 4.5:1 (`--todo-weak`, the hint colour moved towards
    the text colour).
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { TodoView } from './bindings';
  import TodoConflicts from './TodoConflicts.svelte';
  import TodoDetails from './TodoDetails.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const todos = $derived(shell.todos);
  const byNumber = $derived(new Map((todos?.todos ?? []).map((todo) => [todo.number, todo])));
  const openTodos = $derived(
    (todos?.open ?? []).flatMap((n) => byNumber.get(n) ?? ([] as TodoView[])),
  );
  const doneTodos = $derived(
    (todos?.done ?? []).flatMap((n) => byNumber.get(n) ?? ([] as TodoView[])),
  );
  /** egui's hover of "Open list". */
  const OPEN_LIST_HINT = 'The tab "Todos": all todos, sorted and filtered';
  const problemId = `todo-new-problem-${Math.random().toString(36).slice(2)}`;

  let section = $state<HTMLElement | null>(null);

  /** Where the keyboard goes once the deleted todo `gone` is gone. */
  let refocus: { gone: number; next: number | null; previous: number | null } | null = null;

  /** M-18.5: "Delete"; the keyboard then goes to a neighbour (UX review F15). */
  async function deleteTodo(number: number) {
    const list = openTodos.some((t) => t.number === number) ? openTodos : doneTodos;
    const at = list.findIndex((t) => t.number === number);
    refocus = {
      gone: number,
      next: list[at + 1]?.number ?? null,
      previous: list[at - 1]?.number ?? null,
    };
    if (!(await shell.deleteTodo(number))) {
      refocus = null;
    }
  }

  $effect(() => {
    const now = shell.todos;
    const wanted = refocus;
    if (wanted === null || now === null || now.todos.some((t) => t.number === wanted.gone)) {
      return;
    }
    refocus = null;
    void tick().then(() => {
      const to = wanted.next ?? wanted.previous;
      const target =
        (to === null ? null : section?.querySelector<HTMLElement>(`[data-todo="${to}"] .title`)) ??
        section?.querySelector<HTMLElement>('input[aria-label="New todo"]');
      target?.focus();
    });
  });

  /** Steps 9–10: the check box; when it could not be changed, it shows the todo as it is. */
  async function checked(todo: TodoView, box: HTMLInputElement) {
    await shell.changeTodo(todo.number, { kind: 'done', done: box.checked });
    box.checked = shell.todoDone(todo.number);
  }

  /** M-18.1, UC-020 step 3: Enter creates the todo; the keyboard stays in the field. */
  function newTodoKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      void shell.createTodo();
    }
  }

  /** What screen readers hear of the urgency besides its colour. */
  function urgencyWords(todo: TodoView): string {
    return todo.urgency === 'high' || todo.urgency === 'critical'
      ? `, urgency ${todo.urgencyLabel.toLowerCase()}`
      : '';
  }

  // UC-021 A4: a todo shown from the tab "Todos" scrolls into view.
  $effect(() => {
    const asked = shell.todoShownAsked;
    const shown = shell.todoShown;
    if (asked === 0 || shown === null) {
      return;
    }
    void tick().then(() => {
      section
        ?.querySelector(`[data-todo="${shown}"]`)
        ?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    });
  });
</script>

{#snippet row(todo: TodoView)}
  {@const shown = shell.todoShown === todo.number}
  <li class="todo" data-todo={todo.number}>
    <div class="line">
      <input
        type="checkbox"
        checked={todo.done}
        title="Done"
        aria-label="Done: {todo.name}"
        onchange={(event) => void checked(todo, event.currentTarget as HTMLInputElement)}
      />
      <div class="content">
        <button
          type="button"
          class="title urgency-{todo.urgency}"
          class:done={todo.done}
          class:shown
          aria-expanded={shown}
          aria-controls={shown ? `todo-details-${todo.number}` : undefined}
          onclick={() => shell.toggleTodo(todo.number)}
          >{todo.name}{#if todo.urgency === 'high'}<span class="mark" aria-hidden="true">
              !</span
            >{:else if todo.urgency === 'critical'}<span class="mark" aria-hidden="true">
              !!</span
            >{/if}<span class="visually-hidden"
            >{urgencyWords(todo)}{todo.done ? ', done' : ''}</span
          ></button
        >
        {#if todo.due || todo.unstored}
          <div class="meta">
            {#if todo.due}
              <span class="weak">due {todo.due}</span>
            {/if}
            {#if todo.unstored}
              <span class="unstored" title={todo.unstored}
                ><span aria-hidden="true">⚠ </span>not stored<span class="visually-hidden"
                  >: {todo.unstored}</span
                ></span
              >
            {/if}
          </div>
        {/if}
      </div>
    </div>
    {#if shown}
      <TodoDetails
        {shell}
        {todo}
        activeNote={todos?.activeNote ?? null}
        onDelete={() => void deleteTodo(todo.number)}
      />
    {/if}
  </li>
{/snippet}

<details open class="todos" bind:this={section} aria-busy={todos === null}>
  <summary>{todos?.heading ?? 'Todos'}</summary>
  {#if shell.todoConflicts.length > 0}
    <TodoConflicts {shell} conflicts={shell.todoConflicts} />
  {/if}
  {#if todos !== null}
    <div class="top">
      <button
        type="button"
        class="ui-button small"
        title={OPEN_LIST_HINT}
        onclick={() => void shell.showTodoList()}>Open list</button
      >
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
      />
      {#if shell.todoNewProblem}
        <p class="problem" id={problemId} role="alert">{shell.todoNewProblem}</p>
      {/if}
      {#each todos.unreadable as problem (problem)}
        <p class="problem">{problem}</p>
      {/each}
    </div>
    {#if openTodos.length === 0}
      <p class="weak empty">No open todos</p>
    {:else}
      <ul class="list" aria-label="Open todos">
        {#each openTodos as todo (todo.number)}
          {@render row(todo)}
        {/each}
      </ul>
    {/if}
    {#if doneTodos.length > 0}
      <details class="done-group">
        <summary>Done ({doneTodos.length})</summary>
        <ul class="list" aria-label="Done todos">
          {#each doneTodos as todo (todo.number)}
            {@render row(todo)}
          {/each}
        </ul>
      </details>
    {/if}
  {/if}
</details>

<style>
  .todos {
    /* UX review F15: weak texts at 4.5:1 in every theme (theme.test.ts). */
    --todo-weak: color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text));

    margin-top: var(--space-2);
  }

  summary {
    min-height: 24px;
    overflow: hidden;
    font-weight: 700;
    white-space: nowrap;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .top {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    align-items: flex-start;
    margin: var(--space-2) 0 var(--space-1);
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    min-height: 28px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  /* egui's small buttons, still 24 px high for the pointer (WCAG 2.5.8). */
  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  /* Q-05: problems in the text colour with a bar, not egui's orange. */
  .problem {
    margin: 0;
    padding-left: var(--space-2);
    border-left: 3px solid var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .weak {
    color: var(--todo-weak);
  }

  .empty {
    margin: var(--space-1) 0 0;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* The check box beside the title, which wraps; "due …" under it. */
  .line {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 0 var(--space-1);
    align-items: start;
  }

  .content {
    min-width: 0;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--space-2);
    padding-left: var(--space-1);
  }

  /* A 24 px target for the pointer (WCAG 2.5.8). */
  .line input {
    width: 16px;
    height: 16px;
    margin: 4px;
  }

  .mark {
    margin-left: 0.3em;
    font-weight: 700;
  }

  /* egui's selectable label: the title, tinted under the pointer. */
  .title {
    min-width: 0;
    min-height: 24px;
    padding: 2px var(--space-1);
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    overflow-wrap: anywhere;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .title:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .urgency-low {
    color: var(--todo-weak);
  }

  .urgency-high {
    color: var(--ui-urgency-high);
  }

  .urgency-critical {
    color: var(--ui-urgency-critical);
  }

  /* egui: done ones struck through and weak. */
  .title.done {
    color: var(--todo-weak);
    text-decoration: line-through;
  }

  .title.shown {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .unstored {
    font-weight: 700;
    cursor: help;
  }

  .done-group {
    margin-top: var(--space-1);
  }

  .done-group summary {
    font-weight: 400;
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
