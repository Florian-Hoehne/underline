<!--
  M-19.6, UC-013 steps 1–3, A1, A2, A3, A7, A8, BR-001 to BR-006: the branch
  menu, as egui's window "Branches", at the bottom left above the status bar.

  The list: a field "Filter branches" with the keyboard in it, then the
  local branches and those of the remote, "{name}  (current)" and "{name}
  (on the remote)"; Up and Down move the choice, Enter or a click switches
  (the current branch closes the menu), Escape closes it (A8). While a sync
  waits for its conflicts, "A synchronization left notes in conflict;
  resolve them before switching branches." and nothing can be chosen (A7).
  Below: git's reason, the changes in the way with "Commit first" (A3), "New
  branch…", "Show changes", "Pull" and "Push" (UC-014 A1).

  "New branch…" asks for the name, filled with the filter: "New branch, from
  the current commit with the current changes; it goes to the remote with
  the next synchronization.", "Branch: {clean}" or why git would not take
  it while typing (BR-005), "Create" or Enter, "Cancel" or Escape. A taken
  name shows the list again, filtered by it, saying so (A2).

  For screen readers (new, A-143): the filter is a combobox over the list,
  whose entries are options; the chosen one is `aria-selected`.
-->
<script lang="ts">
  import type { BranchEntryView } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import { filteredBranches, type BranchMenu, type Shell } from './shell.svelte';

  type Props = { shell: Shell; menu: BranchMenu };

  let { shell, menu }: Props = $props();

  const listId = `branches-${Math.random().toString(36).slice(2)}`;
  const nameId = `${listId}-name`;
  const mergingId = `${listId}-merging`;
  const entries = $derived(filteredBranches(menu));
  const merging = $derived(menu.view?.merging ?? false);
  const step = $derived(menu.naming === null ? `list:${menu.problem ?? ''}` : 'naming');

  function optionId(index: number): string {
    return `${listId}-${index}`;
  }

  /** BR-006: Up, Down and Enter in the filter. */
  function filterKey(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) {
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      shell.moveBranchCursor(event.key === 'ArrowDown' ? 1 : -1);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const entry = entries[menu.cursor];
      if (entry) {
        void shell.chooseBranch(entry);
      }
    }
  }

  function nameKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      void shell.createBranch();
    }
  }

  function choose(entry: BranchEntryView) {
    void shell.chooseBranch(entry);
  }
</script>

<DialogWindow
  title="Branches"
  anchor="bottom-left"
  onClose={() => shell.closeBranchMenu()}
  busy={menu.busy}
  {step}
>
  {#if menu.naming !== null}
    <p>
      New branch, from the current commit with the current changes; it goes to the remote with the
      next synchronization.
    </p>
    <input
      type="text"
      aria-label="Name of the new branch"
      aria-describedby={nameId}
      aria-invalid={menu.name !== null && !menu.name.ok}
      spellcheck="false"
      data-autofocus="true"
      value={menu.naming}
      oninput={(event) => void shell.editNewBranch(event.currentTarget.value)}
      onkeydown={nameKey}
    />
    <!-- UX review F13: one live region, always there, describing the field. -->
    <p
      id={nameId}
      class="name"
      class:weak={menu.name?.ok ?? true}
      class:problem={menu.name !== null && !menu.name.ok}
      aria-live="polite"
    >
      {#if menu.name !== null}{#if !menu.name.ok}<span aria-hidden="true">⚠&nbsp;</span>{/if}{menu
          .name.text}{/if}
    </p>
    {#if menu.problem !== null}
      <p class="problem" role="alert">{menu.problem}</p>
    {/if}
    <div class="actions">
      <button
        type="button"
        class="ui-button"
        disabled={!menu.name?.ok || menu.busy}
        onclick={() => void shell.createBranch()}>Create</button
      >
      <button type="button" class="ui-button" onclick={() => shell.cancelNewBranch()}>Cancel</button
      >
    </div>
  {:else}
    <input
      type="text"
      placeholder="Filter branches"
      aria-label="Filter branches"
      role="combobox"
      aria-expanded="true"
      aria-controls={listId}
      aria-activedescendant={entries.length > 0 ? optionId(menu.cursor) : undefined}
      aria-describedby={merging ? mergingId : undefined}
      spellcheck="false"
      data-autofocus="true"
      value={menu.filter}
      oninput={(event) => shell.editBranchFilter(event.currentTarget.value)}
      onkeydown={filterKey}
    />
    {#if merging}
      <p class="problem" id={mergingId} role="alert">
        A synchronization left notes in conflict; resolve them before switching branches.
      </p>
    {/if}
    <ul class="branches" id={listId} role="listbox" aria-label="Branches" aria-busy={menu.busy}>
      {#each entries as entry, index (entry.name)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id={optionId(index)}
          role="option"
          class="entry"
          class:chosen={index === menu.cursor}
          aria-selected={index === menu.cursor}
          aria-disabled={merging}
          onclick={() => choose(entry)}
        >
          {entry.label}
        </li>
      {/each}
    </ul>
    {#if !menu.busy && entries.length === 0 && (menu.view?.branches.length ?? 0) > 0}
      <!-- UX review F13 (new text, Q-97). -->
      <p class="weak" role="status">No branch matches the filter.</p>
    {/if}
    {#if menu.view?.problem}
      <p class="problem" role="alert">{menu.view.problem}</p>
    {/if}
    {#if menu.problem !== null}
      <p class="problem" role="alert">{menu.problem}</p>
    {/if}
    {#if menu.inTheWay !== null}
      <p class="problem" role="alert">{menu.inTheWay}</p>
      <div class="actions">
        <button type="button" class="ui-button" onclick={() => shell.branchShowChanges()}
          >Commit first</button
        >
      </div>
    {/if}
    <hr />
    <div class="actions">
      <button
        type="button"
        class="ui-button"
        disabled={merging || menu.busy}
        onclick={() => shell.startNewBranch()}>New branch…</button
      >
      <button type="button" class="ui-button" onclick={() => shell.branchShowChanges()}
        >Show changes</button
      >
    </div>
    <div class="actions">
      <button
        type="button"
        class="ui-button"
        disabled={merging || menu.busy}
        onclick={() => void shell.sync('pull')}>Pull</button
      >
      <button
        type="button"
        class="ui-button"
        disabled={merging || menu.busy}
        onclick={() => void shell.sync('push')}>Push</button
      >
    </div>
  {/if}
</DialogWindow>

<style>
  .weak {
    color: var(--ui-hint);
  }

  /* App-tester F13: a long name wraps. */
  p {
    max-width: 52ch;
    overflow-wrap: anywhere;
  }

  .name {
    min-height: 1.4em;
  }

  .branches {
    max-height: 40vh;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
  }

  .entry {
    min-height: 24px;
    padding: 2px var(--space-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
    cursor: pointer;
    border-radius: var(--radius);
  }

  .entry.chosen {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .entry:hover:not(.chosen) {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry[aria-disabled='true'] {
    color: var(--ui-hint);
    cursor: default;
  }

  .entry.chosen[aria-disabled='true'] {
    color: var(--ui-chosen-text);
  }

  hr {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--ui-rule);
  }

  .ui-button:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  .ui-button:disabled:hover {
    color: var(--ui-hint);
    background: var(--ui-button);
  }
</style>
