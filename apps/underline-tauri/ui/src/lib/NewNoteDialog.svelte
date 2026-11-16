<!--
  M-14.1, UC-003: "New note", as egui asks it. Title (with the keyboard),
  the folder ("Folder (empty for the top folder, new folders are
  created):") and the note format, preset with the folder of the active
  note and the default format; while the user types, "File: {name}" shows
  the file the note will get. "Create" or Enter creates it; a title or
  folder that is not allowed keeps the dialog and says why, a note that
  exists offers "Open the existing note".

  Package 6b (FR-094, UC-003 A7, BR-008): "Template:" offers "No template"
  (preselected) and each template by its title; without templates it is
  disabled and says where they are kept; choosing one sets the note format to the template's and
  locks it, which the format's hint says.

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

  /** UC-003 A7: the chosen template, which fixes the note format. */
  const chosen = $derived(dialog.templates.find((template) => template.note === dialog.template));

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
        data-autofocus={dialog.focusTemplate ? undefined : 'true'}
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
      <!-- UX review: always shown; without templates disabled, saying where they are kept. -->
      <label for="new-note-template">Template:</label>
      <select
        id="new-note-template"
        class="template"
        disabled={dialog.templates.length === 0}
        aria-describedby={dialog.templates.length === 0 ? 'new-note-no-templates' : undefined}
        data-autofocus={dialog.focusTemplate && dialog.templates.length > 0 ? 'true' : undefined}
        value={dialog.template ?? ''}
        onchange={(event) => shell.chooseTemplate(dialog, event.currentTarget.value || null)}
      >
        <option value="">No template</option>
        {#each dialog.templates as template (template.note)}
          <option value={template.note}>{template.label}</option>
        {/each}
      </select>
      {#if dialog.templates.length === 0 && dialog.templateFolder !== ''}
        <p class="file" id="new-note-no-templates">
          No templates: notes in the folder {dialog.templateFolder} are offered here.
        </p>
      {/if}
      <fieldset
        class="formats"
        disabled={chosen !== undefined}
        aria-describedby={chosen ? 'new-note-format-fixed' : undefined}
      >
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
      {#if chosen}
        <p class="file" id="new-note-format-fixed">The template sets the note format.</p>
      {/if}
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

  .template {
    box-sizing: border-box;
    width: 100%;
    min-height: 28px;
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  .template:disabled {
    opacity: 0.6;
  }

  .formats:disabled .choice {
    cursor: default;
    opacity: 0.6;
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
