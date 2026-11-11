<!--
  M-19.10, UC-020 A10: the todos in conflict after a sync, as egui's
  `conflicts_ui` in the section "Todos": "Todo #{n} was changed here and on
  the remote", both versions field by field (Title, Due, Scheduled,
  Urgency, Status, Notes; "–" for none, "(deleted)"), the fields that
  differ in bold, then "Keep mine" and "Take remote".

  Until the section "Todos" comes with feature F15, they stand alone in the
  tool panel, where it will be (A-146). For screen readers (UX review F13,
  new texts, Q-97) a field that differs says "(differs)", and the buttons
  name the todo ("Keep mine for todo #n", "Take remote for todo #n").
-->
<script lang="ts">
  import type { TodoConflictView } from './bindings';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell; conflicts: TodoConflictView[] };

  let { shell, conflicts }: Props = $props();
</script>

{#each conflicts as conflict (conflict.number)}
  <section class="todo-conflict" aria-label={conflict.heading}>
    <p class="heading"><span aria-hidden="true">⚠ </span>{conflict.heading}</p>
    <table>
      <thead>
        <tr>
          <th scope="col"><span class="visually-hidden">Field</span></th>
          <th scope="col">Here</th>
          <th scope="col">Remote</th>
        </tr>
      </thead>
      <tbody>
        {#each conflict.rows as row (row.name)}
          <tr class:differ={row.differ}>
            <th scope="row" class="weak"
              >{row.name}{#if row.differ}<span class="visually-hidden"> (differs)</span>{/if}</th
            >
            <td data-side="Here">{row.here}</td>
            <td data-side="Remote">{row.remote}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="row">
      <button
        type="button"
        class="ui-button"
        aria-label="Keep mine for todo #{conflict.number}"
        onclick={() => void shell.resolveTodoConflict(conflict.number, true)}>Keep mine</button
      >
      <button
        type="button"
        class="ui-button"
        aria-label="Take remote for todo #{conflict.number}"
        onclick={() => void shell.resolveTodoConflict(conflict.number, false)}>Take remote</button
      >
    </div>
  </section>
{/each}

<style>
  .todo-conflict {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: var(--space-2) 0;
    padding: var(--space-2);
    border: 1px solid var(--ui-rule);
    border-radius: var(--radius);
  }

  .heading {
    margin: 0;
    font-weight: 700;
  }

  /* App-tester F13: the table fills the card, a narrow label column, the
     values share the rest and break between words only. */
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  th,
  td {
    padding: 1px var(--space-2) 1px 0;
    text-align: left;
    vertical-align: top;
    overflow-wrap: break-word;
    word-break: normal;
  }

  th.weak {
    width: 6.5em;
    font-weight: 400;
    color: var(--ui-hint);
  }

  thead th:first-child {
    width: 6.5em;
  }

  /* Retest F13: in a narrow panel each field stands on its own, its two
     values below it, named "Here" and "Remote". */
  @container (max-width: 340px) {
    table,
    tbody,
    tr,
    th,
    td {
      display: block;
      width: auto;
    }

    thead {
      display: none;
    }

    tr + tr {
      margin-top: var(--space-1);
    }

    th.weak {
      width: auto;
    }

    td {
      padding-left: var(--space-2);
    }

    td::before {
      content: attr(data-side) ': ';
      color: var(--ui-hint);
      font-weight: 400;
    }
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .differ td {
    font-weight: 700;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }
</style>
