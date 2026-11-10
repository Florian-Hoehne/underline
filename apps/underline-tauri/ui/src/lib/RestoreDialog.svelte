<!--
  M-14.4, UC-007 A6: "Restore note", when the place of a note to restore
  is taken: another title or folder, as when renaming (UC-006 BR-001).
  "Restore" or Enter restores it there, "Cancel" or Escape leaves it among
  the deleted notes; a place that is taken or not allowed keeps the dialog
  and says why.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { RestoreDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: RestoreDialog };

  let { shell, dialog }: Props = $props();

  const busy = $derived(shell.noteBusy('restore'));

  function edit(change: Partial<RestoreDialog>) {
    shell.editNoteDialog('restore', dialog, change);
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!busy) {
      void shell.restoreAs();
    }
  }
</script>

<DialogWindow title="Restore note" onClose={() => shell.closeNoteDialog('restore')} {busy}>
  <form class="form" onsubmit={submit}>
    <div class="fields">
      <label for="restore-title">Title:</label>
      <input
        id="restore-title"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        data-select="true"
        value={dialog.title}
        oninput={(event) => edit({ title: event.currentTarget.value })}
      />
      <label for="restore-folder">Folder (empty for the top folder, new folders are created):</label
      >
      <input
        id="restore-folder"
        type="text"
        spellcheck="false"
        autocomplete="off"
        value={dialog.folder}
        oninput={(event) => edit({ folder: event.currentTarget.value })}
      />
      {#if dialog.problem}
        <p class="problem" role="alert">{dialog.problem}</p>
      {/if}
    </div>
    <div class="actions">
      <button type="submit" class="ui-button" disabled={busy}>Restore</button>
      <button type="button" class="ui-button" onclick={() => shell.closeNoteDialog('restore')}
        >Cancel</button
      >
    </div>
  </form>
</DialogWindow>

<style>
  /* In a low window the fields scroll and "Create"/"Rename"/"Restore"
     and "Cancel" stay in view, as in "Insert link" (app-tester F8). */
  .form {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--space-2);
    min-height: 0;
  }

  .form > .actions {
    flex: none;
  }

  .fields {
    display: flex;
    flex: 0 1 auto;
    flex-direction: column;
    gap: var(--space-2);
    min-height: 48px;
    margin: -3px;
    padding: 3px;
    overflow: auto;
  }
</style>
