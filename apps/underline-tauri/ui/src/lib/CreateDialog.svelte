<!--
  M-21.2, UC-001: the Create dialog in three stages.
  1. The directory, new or empty; refused with the reason, and with
     "Open it as a workspace instead" for one that holds files (A1, A2).
  2. The default format of new notes and "Create as git repository" (off).
  3. With the git option, the optional address of the git remote (A3).
  Enter in a stage does what its first button does.
-->
<script lang="ts">
  import type { FormatName } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import type { CreateDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: CreateDialog };

  let { shell, dialog }: Props = $props();

  /** UC-001 BR-001: Markdown preselected. */
  const FORMATS: FormatName[] = ['Markdown', 'AsciiDoc'];

  const busy = $derived(shell.dialogBusy);
  const step = $derived(
    dialog.stage === 'directory' ? `directory|${dialog.problem ?? ''}` : dialog.stage,
  );

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) {
      return;
    }
    switch (dialog.stage) {
      case 'directory':
        if (dialog.path.trim() !== '') {
          void shell.checkNewDirectory(dialog.path);
        }
        break;
      case 'options':
        if (dialog.git) {
          shell.askRemote(dialog.directory, dialog.format);
        } else {
          void shell.createWorkspace(dialog.directory, dialog.format, false);
        }
        break;
      case 'remote':
        void shell.createWorkspace(dialog.directory, dialog.format, true, dialog.address);
        break;
    }
  }
</script>

<DialogWindow title="New workspace" onClose={() => shell.closeDialog()} {busy} {step}>
  <form class="form" onsubmit={submit}>
    {#if dialog.stage === 'directory'}
      {@const current = dialog}
      <label for="create-path">Directory for the new workspace (new or empty):</label>
      <input
        id="create-path"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus={current.problem !== null || current.path === ''}
        data-select={current.problem !== null}
        value={current.path}
        oninput={(event) => shell.editDialog(current, { path: event.currentTarget.value })}
      />
      {#if current.problem}
        <p class="problem" role="alert">{current.problem}</p>
      {/if}
      {#if current.offer}
        {@const directory = current.offer}
        <div class="actions">
          <button
            type="button"
            class="ui-button"
            disabled={busy}
            onclick={() => shell.openInstead(directory)}>Open it as a workspace instead</button
          >
        </div>
      {/if}
      <div class="actions">
        <button type="submit" class="ui-button" disabled={busy || current.path.trim() === ''}
          >Next</button
        >
        <button type="button" class="ui-button" onclick={() => shell.closeDialog()}>Cancel</button>
      </div>
    {:else if dialog.stage === 'options'}
      {@const current = dialog}
      <p class="directory">Directory: {current.directory}</p>
      <fieldset>
        <legend>Default format of new notes:</legend>
        {#each FORMATS as option (option)}
          <label class="choice">
            <input
              type="radio"
              name="create-format"
              value={option}
              checked={current.format === option}
              onchange={() => shell.editDialog(current, { format: option })}
            />
            {option}
          </label>
        {/each}
      </fieldset>
      <label class="choice">
        <input
          type="checkbox"
          checked={current.git}
          onchange={(event) => shell.editDialog(current, { git: event.currentTarget.checked })}
        />
        Create as git repository
      </label>
      <div class="actions">
        <button type="submit" class="ui-button" disabled={busy}
          >{current.git ? 'Next' : 'Create'}</button
        >
        <button type="button" class="ui-button" onclick={() => shell.closeDialog()}>Cancel</button>
      </div>
    {:else}
      {@const current = dialog}
      <label for="create-remote">Address of the git remote (optional):</label>
      <input
        id="create-remote"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        value={current.address}
        oninput={(event) => shell.editDialog(current, { address: event.currentTarget.value })}
      />
      <p>The remote is not contacted now; the first git sync shows whether it is reachable.</p>
      <div class="actions">
        <button type="submit" class="ui-button" disabled={busy}>Create</button>
        <button type="button" class="ui-button" onclick={() => shell.closeDialog()}>Cancel</button>
      </div>
    {/if}
  </form>
</DialogWindow>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .directory {
    overflow-wrap: anywhere;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: var(--space-1);
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
</style>
