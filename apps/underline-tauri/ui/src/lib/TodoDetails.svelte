<!--
  M-18.3 to M-18.5 (UC-020 steps 5–10, A1, A2, A5, A6): the fields of the
  todo shown in the section "Todos", as egui's `todo_row` below its line.

  - "Title", "Due" and "Scheduled" (YYYY-MM-DD) are stored when the field
    is left (or with Enter) and has changed; what is not taken is said
    below the fields and the text stays (A3, BR-003). What is typed stays
    while the todo changes elsewhere (A7); the fields are filled anew only
    when the todo is shown again.
  - "Pick" (hover "Choose a day in a calendar") opens "Choose a day"
    (TodoCalendar.svelte); the day chosen is stored at once and shows in
    the field.
  - "Urgency": Low, Normal, High, Critical, stored when chosen.
  - "The scheduled day is after the due day." as a hint.
  - "Notes": each linked note opens with a click (its place at the
    pointer); one that no longer exists shows "{title} (missing)" and its
    click says so in the status bar (A6); "×" (hover "Remove the link")
    removes the link (A2). "Link {title}" links the note of the active tab
    (step 7); "Link a note…" offers up to 8 notes whose title holds what
    is typed, else "No note matches" (A1).
  - "Delete" without asking (A5; the status bar offers "Undo"), and
    "created …" or "done …".
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import type { TodoActiveNoteView, TodoCandidateView, TodoView, UrgencyName } from './bindings';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    todo: TodoView;
    /** The note of the active tab, which "Link {title}" links. */
    activeNote: TodoActiveNoteView | null;
    /** "Delete" (the section moves the keyboard on after it). */
    onDelete: () => void;
  };

  let { shell, todo, activeNote, onDelete }: Props = $props();

  const URGENCIES: { value: UrgencyName; label: string }[] = [
    { value: 'low', label: 'Low' },
    { value: 'normal', label: 'Normal' },
    { value: 'high', label: 'High' },
    { value: 'critical', label: 'Critical' },
  ];

  // What is typed, filled once from the todo (egui's `Edit::of`).
  let title = $state(untrack(() => todo.title));
  let due = $state(untrack(() => todo.due ?? ''));
  let scheduled = $state(untrack(() => todo.scheduled ?? ''));
  let linkQuery = $state('');
  let candidates = $state.raw<TodoCandidateView[]>([]);
  let noneMatches = $state<string | null>(null);
  /** Only the answer to the latest query shows; answers can cross. */
  let asked = 0;

  const id = $derived(`todo-details-${todo.number}`);
  const problemId = $derived(`${id}-problem`);
  const formatId = $derived(`${id}-format`);
  /** UX review F15: only the field a problem is about is marked. */
  const invalid = (field: 'title' | 'due' | 'scheduled') =>
    shell.todoProblem !== null && shell.todoProblemField === field;
  const describe = (field: 'title' | 'due' | 'scheduled') =>
    [field === 'title' ? null : formatId, invalid(field) ? problemId : null]
      .filter((one) => one !== null)
      .join(' ') || undefined;

  // M-18.3: a day chosen in the calendar shows in its field.
  $effect(() => {
    const picked = shell.todoDayPicked;
    if (picked === null || picked.todo !== untrack(() => todo.number)) {
      return;
    }
    if (picked.field === 'due') {
      due = picked.day;
    } else {
      scheduled = picked.day;
    }
    // Taken; a later showing of the todo fills its fields from the todo.
    shell.todoDayPicked = null;
  });

  /** UC-020 A3: a title not taken stays in the field, the problem below it. */
  function storeTitle() {
    if (title !== todo.title) {
      void shell.changeTodo(todo.number, { kind: 'title', title });
    }
  }

  function storeDay(field: 'due' | 'scheduled') {
    const day = field === 'due' ? due : scheduled;
    void shell.changeTodo(
      todo.number,
      field === 'due' ? { kind: 'due', day } : { kind: 'scheduled', day },
    );
  }

  /** egui's single-line fields: Enter leaves them, which stores. */
  function leaveOnEnter(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault();
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  async function findNotes(query: string) {
    linkQuery = query;
    const mine = ++asked;
    const found = await shell.todoCandidates(todo.number, query);
    if (mine !== asked || found === null) {
      return;
    }
    candidates = found.notes;
    noneMatches = found.none;
  }

  function link(note: string) {
    void shell.changeTodo(todo.number, { kind: 'link', note });
    linkQuery = '';
    candidates = [];
    noneMatches = null;
    asked += 1;
  }
</script>

<div class="details" {id} role="group" aria-label="Todo {todo.name}">
  <div class="fields">
    <label for="{id}-title" class="weak">Title</label>
    <input
      id="{id}-title"
      type="text"
      class="field"
      bind:value={title}
      aria-invalid={invalid('title')}
      aria-describedby={describe('title')}
      onchange={storeTitle}
      onkeydown={leaveOnEnter}
    />
    {#each [{ field: 'due', label: 'Due' }, { field: 'scheduled', label: 'Scheduled' }] as const as entry (entry.field)}
      <label for="{id}-{entry.field}" class="weak">{entry.label}</label>
      <div class="day">
        {#if entry.field === 'due'}
          <input
            id="{id}-due"
            type="text"
            class="field date"
            placeholder="YYYY-MM-DD"
            bind:value={due}
            aria-invalid={invalid('due')}
            aria-describedby={describe('due')}
            onchange={() => storeDay('due')}
            onkeydown={leaveOnEnter}
          />
        {:else}
          <input
            id="{id}-scheduled"
            type="text"
            class="field date"
            placeholder="YYYY-MM-DD"
            bind:value={scheduled}
            aria-invalid={invalid('scheduled')}
            aria-describedby={describe('scheduled')}
            onchange={() => storeDay('scheduled')}
            onkeydown={leaveOnEnter}
          />
        {/if}
        <button
          type="button"
          class="ui-button small"
          title="Choose a day in a calendar"
          aria-label="Pick the {entry.label.toLowerCase()} day"
          aria-haspopup="dialog"
          onclick={() =>
            void shell.openTodoCalendar(
              entry.field,
              todo.number,
              entry.field === 'due' ? todo.due : todo.scheduled,
            )}>Pick</button
        >
      </div>
    {/each}
    <label for="{id}-urgency" class="weak">Urgency</label>
    <select
      id="{id}-urgency"
      class="select"
      value={todo.urgency}
      onchange={(event) =>
        void shell.changeTodo(todo.number, {
          kind: 'urgency',
          urgency: (event.currentTarget as HTMLSelectElement).value as UrgencyName,
        })}
    >
      {#each URGENCIES as urgency (urgency.value)}
        <option value={urgency.value}>{urgency.label}</option>
      {/each}
    </select>
  </div>
  <span id={formatId} class="visually-hidden">YYYY-MM-DD</span>
  {#if todo.unstored}
    <!-- UX review F15 (new text, Q-112): the reason, not only at the pointer. -->
    <p class="problem">Not stored: {todo.unstored}</p>
  {/if}
  {#if todo.scheduledAfterDue}
    <p class="weak">The scheduled day is after the due day.</p>
  {/if}
  {#if shell.todoProblem}
    <p class="problem" id={problemId} role="alert">{shell.todoProblem}</p>
  {/if}
  <p class="weak notes-heading" id="{id}-notes">Notes</p>
  {#if todo.links.length > 0}
    <ul class="links" aria-labelledby="{id}-notes">
      {#each todo.links as entry (entry.link)}
        <li class="link-row">
          <button
            type="button"
            class="link"
            class:missing={!entry.exists}
            title={entry.link}
            onclick={() => void shell.openTodoLink(entry.link)}
            >{entry.exists ? entry.title : `${entry.title} (missing)`}</button
          >
          <button
            type="button"
            class="ui-button small remove"
            title="Remove the link"
            aria-label="Remove the link to {entry.title}"
            onclick={() => void shell.changeTodo(todo.number, { kind: 'unlink', link: entry.link })}
            >×</button
          >
        </li>
      {/each}
    </ul>
  {/if}
  {#if activeNote !== null && !todo.linksActive}
    {@const note = activeNote.note}
    <button
      type="button"
      class="ui-button small"
      onclick={() => void shell.changeTodo(todo.number, { kind: 'link', note })}
      >{activeNote.linkLabel}</button
    >
  {/if}
  <input
    type="text"
    class="field"
    placeholder="Link a note…"
    aria-label="Link a note"
    value={linkQuery}
    oninput={(event) => void findNotes((event.currentTarget as HTMLInputElement).value)}
  />
  {#if linkQuery.trim() !== ''}
    {#if candidates.length > 0}
      <ul class="candidates" aria-label="Notes to link">
        {#each candidates as candidate (candidate.note)}
          <li>
            <button type="button" class="candidate" onclick={() => link(candidate.note)}
              >{candidate.label}</button
            >
          </li>
        {/each}
      </ul>
    {:else if noneMatches}
      <p class="weak">{noneMatches}</p>
    {/if}
  {/if}
  <div class="end">
    <button
      type="button"
      class="ui-button small"
      aria-label="Delete todo {todo.name}"
      onclick={() => onDelete()}>Delete</button
    >
    <span class="weak">{todo.stamp}</span>
  </div>
</div>

<style>
  .details {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    align-items: flex-start;
    margin: var(--space-1) 0 var(--space-2) calc(24px + var(--space-1));
  }

  /* egui's grid: the names beside their fields. */
  .fields {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--space-1) var(--space-2);
    align-items: center;
    width: 100%;
  }

  .weak {
    margin: 0;
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
  }

  .field,
  .select {
    box-sizing: border-box;
    min-height: 28px;
    font: inherit;
    color: var(--ui-text);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  .field {
    width: 100%;
    min-width: 0;
    padding: 0 var(--space-2);
    background: var(--ui-field);
  }

  .select {
    justify-self: start;
    max-width: 100%;
    background: var(--ui-button);
  }

  .day {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    min-width: 0;
  }

  /* egui: 90 px, room for YYYY-MM-DD. */
  .date {
    flex: none;
    width: calc(10ch + 2 * var(--space-2) + 2px);
  }

  /* In a narrow panel each name stands above its field, so a day is never
     cut. */
  @container (max-width: 240px) {
    .fields {
      grid-template-columns: minmax(0, 1fr);
      gap: 2px;
    }

    .fields label:not(:first-child) {
      margin-top: var(--space-1);
    }
  }

  /* egui's small buttons, still 24 px high for the pointer (WCAG 2.5.8). */
  .small {
    flex: none;
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  .remove {
    min-width: 24px;
    padding: 0;
  }

  /* Q-05: problems in the text colour with a bar, not egui's orange. */
  .problem {
    margin: 0;
    padding-left: var(--space-2);
    border-left: 3px solid var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .notes-heading {
    margin-top: var(--space-1);
  }

  .links,
  .candidates {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .link-row {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    min-width: 0;
  }

  /* egui's links: the link colour, underlined. */
  .link {
    min-width: 0;
    min-height: 24px;
    padding: 0;
    font: inherit;
    color: var(--palette-link, var(--ui-text));
    text-align: left;
    text-decoration: underline;
    overflow-wrap: anywhere;
    cursor: pointer;
    background: none;
    border: 0;
  }

  .link.missing {
    color: var(--todo-weak, color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text)));
  }

  .candidate {
    box-sizing: border-box;
    width: 100%;
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

  .candidate:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .end {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
    margin-top: var(--space-1);
  }
</style>
