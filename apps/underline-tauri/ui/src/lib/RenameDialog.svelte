<!--
  M-14.2, UC-006: "Rename note", as egui asks it. The title, selected when
  the dialog shows, and the folder; while the user types, "File: …" shows
  the file the note will be in. For the Home note it warns that the GitHub
  wiki loses its start page (BR-009). "Rename" or Enter renames, "Cancel"
  or Escape closes; a name that is taken or not allowed keeps the dialog
  and says why.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { RenameDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: RenameDialog };

  let { shell, dialog }: Props = $props();

  const busy = $derived(shell.noteBusy('rename'));

  function edit(change: Partial<RenameDialog>) {
    shell.editNoteDialog('rename', dialog, change);
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!busy) {
      void shell.renameNote();
    }
  }
</script>

<DialogWindow
  title="Rename note"
  onClose={() => shell.closeNoteDialog('rename')}
  {busy}
  step={dialog.note}
>
  <form class="form" onsubmit={submit}>
    <div class="fields">
      <label for="rename-title">Title:</label>
      <input
        id="rename-title"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        data-select="true"
        value={dialog.title}
        oninput={(event) => edit({ title: event.currentTarget.value })}
      />
      <label for="rename-folder">Folder (empty for the top folder, new folders are created):</label>
      <input
        id="rename-folder"
        type="text"
        spellcheck="false"
        autocomplete="off"
        value={dialog.folder}
        oninput={(event) => edit({ folder: event.currentTarget.value })}
      />
      {#if dialog.file !== null}
        <p class="file">File: {dialog.file}</p>
      {/if}
      {#if dialog.home}
        <p class="problem">
          This is the Home note, the start page of a GitHub wiki. Renamed or moved, the wiki has no
          start page.
        </p>
      {/if}
      {#if dialog.problem}
        <p class="problem" role="alert">{dialog.problem}</p>
      {/if}
    </div>
    <div class="actions">
      <button type="submit" class="ui-button" disabled={busy}>Rename</button>
      <button type="button" class="ui-button" onclick={() => shell.closeNoteDialog('rename')}
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

  /* egui's weak text; a long name breaks anywhere. */
  .file {
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }
</style>
