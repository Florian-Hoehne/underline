<!--
  M-21.1, UC-002 steps 2–4: the Open dialog. The recently opened
  workspaces, a typed path (no system dialog, inventory D-12), and what the
  backend offers when the directory cannot be opened as chosen: create a
  workspace there (A4), open the outer workspace (A6) or the directory
  anyway (A5).
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { OpenDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: OpenDialog };

  let { shell, dialog }: Props = $props();

  const busy = $derived(shell.dialogBusy);
  const empty = $derived(dialog.path.trim() === '');
  // As in egui, the field takes the keyboard in an empty dialog; after a
  // problem it takes it too, with the path selected to type it anew.
  const focusField = $derived(
    dialog.problem !== null ||
      (dialog.path === '' && dialog.problem === null && dialog.recent.length === 0),
  );

  function openTyped(event: SubmitEvent) {
    event.preventDefault();
    if (!empty && !busy) {
      void shell.openWorkspace(dialog.path.trim());
    }
  }
</script>

<DialogWindow
  title="Open workspace"
  onClose={() => shell.closeDialog()}
  {busy}
  step={`${dialog.recent.length}|${dialog.problem ?? ''}`}
>
  {#if dialog.recent.length > 0}
    <p id="open-recent">Recently opened:</p>
    <ul class="recent" aria-labelledby="open-recent">
      {#each dialog.recent as dir (dir)}
        <li>
          <button
            type="button"
            class="ui-button"
            title={dir}
            disabled={busy}
            onclick={() => shell.openWorkspace(dir)}>{dir}</button
          >
        </li>
      {/each}
    </ul>
    <hr />
  {/if}
  <form class="form" onsubmit={openTyped}>
    <label for="open-path">Another directory:</label>
    <input
      id="open-path"
      type="text"
      spellcheck="false"
      autocomplete="off"
      data-autofocus={focusField}
      data-select={dialog.problem !== null}
      value={dialog.path}
      oninput={(event) => shell.editDialog(dialog, { path: event.currentTarget.value })}
    />
    {#if dialog.problem}
      <p class="problem" role="alert">{dialog.problem}</p>
    {/if}
    {#if dialog.offer?.kind === 'create'}
      {@const directory = dialog.offer.directory}
      <div class="actions">
        <button
          type="button"
          class="ui-button"
          disabled={busy}
          onclick={() => shell.createThere(directory)}>Create a new workspace there</button
        >
      </div>
    {:else if dialog.offer?.kind === 'outer'}
      {@const { outer, inner } = dialog.offer}
      <div class="actions offers">
        <button
          type="button"
          class="ui-button"
          title={outer}
          disabled={busy}
          onclick={() => shell.openWorkspace(outer, true)}>Open {outer}</button
        >
        <button
          type="button"
          class="ui-button"
          title={inner}
          disabled={busy}
          onclick={() => shell.openWorkspace(inner, true)}>Open {inner} anyway</button
        >
      </div>
    {/if}
    <div class="actions">
      <button type="submit" class="ui-button" disabled={busy || empty}>Open</button>
      <button type="button" class="ui-button" onclick={() => shell.closeDialog()}>Cancel</button>
    </div>
  </form>
</DialogWindow>

<style>
  .recent {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .recent li {
    max-width: 100%;
  }

  hr {
    width: 100%;
    margin: var(--space-1) 0;
    border: 0;
    border-top: 1px solid var(--ui-rule);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .offers {
    flex-direction: column;
    align-items: flex-start;
  }
</style>
