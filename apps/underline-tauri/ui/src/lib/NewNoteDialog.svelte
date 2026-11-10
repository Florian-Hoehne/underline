<!--
  M-14.1, UC-003: "New note", as egui asks it. Title (with the keyboard),
  the folder ("Folder (empty for the top folder, new folders are
  created):") and the note format, preset with the folder of the active
  note and the default format; while the user types, "File: {name}" shows
  the file the note will get. "Create" or Enter creates it; a title or
  folder that is not allowed keeps the dialog and says why, a note that
  exists offers "Open the existing note".

  Not modal, like egui's window. Escape closes it (inventory D-02, fixed in
  the Tauri app, paritaet.md A-78); Enter creates from either field
  (A-78).
-->
<script lang="ts">
  import type { FormatName } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import type { NewNoteDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: NewNoteDialog };

  let { shell, dialog }: Props = $props();

  const FORMATS: FormatName[] = ['Markdown', 'AsciiDoc'];

  const busy = $derived(shell.noteBusy('newNote'));

  function edit(change: Partial<NewNoteDialog>) {
    shell.editNoteDialog('newNote', dialog, change);
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!busy) {
      void shell.createNote();
    }
  }
</script>

<DialogWindow title="New note" onClose={() => shell.closeNoteDialog('newNote')} {busy}>
  <form class="form" onsubmit={submit}>
    <div class="fields">
      <label for="new-note-title">Title:</label>
      <input
        id="new-note-title"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        value={dialog.title}
        oninput={(event) => edit({ title: event.currentTarget.value })}
      />
      <label for="new-note-folder"
        >Folder (empty for the top folder, new folders are created):</label
      >
      <input
        id="new-note-folder"
        type="text"
        spellcheck="false"
        autocomplete="off"
        value={dialog.folder}
        oninput={(event) => edit({ folder: event.currentTarget.value })}
      />
      <fieldset class="formats">
        <legend>Note format:</legend>
        {#each FORMATS as option (option)}
          <label class="choice">
            <input
              type="radio"
              name="new-note-format"
              value={option}
              checked={dialog.format === option}
              onchange={() => edit({ format: option })}
            />
            {option}
          </label>
        {/each}
      </fieldset>
      {#if dialog.file !== null}
        <p class="file">File: {dialog.file}</p>
      {/if}
      {#if dialog.problem}
        <p class="problem" role="alert">{dialog.problem}</p>
      {/if}
      {#if dialog.existing}
        {@const existing = dialog.existing}
        <div class="actions">
          <button
            type="button"
            class="ui-button"
            disabled={busy}
            onclick={() => void shell.openExistingNote(existing)}>Open the existing note</button
          >
        </div>
      {/if}
    </div>
    <div class="actions">
      <button type="submit" class="ui-button" disabled={busy}>Create</button>
      <button type="button" class="ui-button" onclick={() => shell.closeNoteDialog('newNote')}
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

  /* egui: "Note format:" and the choices in one row. */
  .formats {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    padding: 0;
    border: 0;
  }

  .formats legend {
    float: left;
    padding: 0;
  }

  .choice {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 24px;
    cursor: pointer;
  }

  .choice input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--ui-chosen);
  }

  /* egui's weak text; a long name breaks anywhere. */
  .file {
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }
</style>
