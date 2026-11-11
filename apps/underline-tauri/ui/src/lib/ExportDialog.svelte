<!--
  M-15.1, M-15.2 (UC-009 steps 1–3): the window "Export note", as egui shows
  it:
  - "Export {t} as:" with the kinds of export (the other note format, HTML,
    PDF): Up and Down (Home, End) move the choice, Enter takes it, a click
    takes the kind clicked, Cancel or Escape exports nothing (BR-008, A1).
    The list has the keyboard (a `listbox` with `aria-activedescendant`)
    and scrolls in a low window, so every kind and Cancel stay reachable.
  - "Export {t} as {label} to:" with a path field, when the system's file
    dialog could not show (ADR-016): "Export" or Enter; when the file exists
    "The file exists. Export again to replace it." and the button says
    "Replace", for that path only (D-03, fixed). An empty field cannot be
    exported; why the last try failed stands under the field (A5, a folder,
    a failed export) until the path is changed.
  While the system's dialog is open nothing shows here, as in egui.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { ExportDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: Exclude<ExportDialog, { stage: 'system' }> };

  let { shell, dialog }: Props = $props();

  const listId = `export-kinds-${Math.random().toString(36).slice(2)}`;
  const headingId = `${listId}-heading`;
  const optionId = (index: number) => `${listId}-${index}`;

  function onListKey(event: KeyboardEvent) {
    if (dialog.stage !== 'choose' || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        shell.moveExportChoice(1);
        break;
      case 'ArrowUp':
        shell.moveExportChoice(-1);
        break;
      case 'Home':
        shell.moveExportChoice(-dialog.choices.length);
        break;
      case 'End':
        shell.moveExportChoice(dialog.choices.length);
        break;
      case 'Enter':
        void shell.chooseExportKind(dialog.chosen);
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!empty) {
      void shell.exportToTypedPath();
    }
  }

  const replacing = $derived(
    dialog.stage === 'path' &&
      dialog.replace !== null &&
      dialog.replace.trim() === dialog.path.trim(),
  );
  // The chosen kind stays in view in a list that scrolls.
  $effect(() => {
    if (dialog.stage === 'choose') {
      document.getElementById(optionId(dialog.chosen))?.scrollIntoView?.({ block: 'nearest' });
    }
  });

  const empty = $derived(dialog.stage === 'path' && dialog.path.trim() === '');
  const problemId = `${listId}-problem`;
</script>

<DialogWindow
  title="Export note"
  onClose={() => shell.closeExport()}
  step={`${dialog.stage === 'path' ? `path|${dialog.problem ?? ''}` : 'choose'}|${shell.exportAskedAgain}`}
>
  {#if dialog.stage === 'choose'}
    <p id={headingId}>Export {dialog.title} as:</p>
    <ul
      class="kinds"
      role="listbox"
      tabindex="0"
      aria-labelledby={headingId}
      aria-activedescendant={optionId(dialog.chosen)}
      data-autofocus="true"
      onkeydown={onListKey}
    >
      {#each dialog.choices as choice, index (choice.kind)}
        <!-- The list has the keyboard; an option is for the mouse. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id={optionId(index)}
          role="option"
          aria-selected={index === dialog.chosen}
          class:chosen={index === dialog.chosen}
          onclick={() => void shell.chooseExportKind(index)}
        >
          {choice.label}
        </li>
      {/each}
    </ul>
    <div class="actions">
      <button type="button" class="ui-button" onclick={() => shell.closeExport()}>Cancel</button>
    </div>
  {:else}
    <form class="form" onsubmit={submit}>
      <label for="export-path">{dialog.heading}</label>
      <input
        id="export-path"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        aria-describedby={dialog.problem ? problemId : undefined}
        value={dialog.path}
        oninput={(event) => shell.editExportPath(event.currentTarget.value)}
      />
      {#if dialog.problem}
        <p class="problem" id={problemId} role="alert">{dialog.problem}</p>
      {/if}
      <div class="actions">
        <button type="submit" class="ui-button" disabled={empty}
          >{replacing ? 'Replace' : 'Export'}</button
        >
        <button type="button" class="ui-button" onclick={() => shell.closeExport()}>Cancel</button>
      </div>
    </form>
  {/if}
</DialogWindow>

<style>
  /* A low window: the list scrolls, Cancel stays in view (app-tester F11). */
  .kinds {
    flex: 0 1 auto;
    min-height: 28px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 2px;
    list-style: none;
    border-radius: var(--radius);
  }

  .kinds:focus {
    outline: none;
  }

  .kinds:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  /* egui's selectable labels. */
  [role='option'] {
    flex: none;
    min-height: 24px;
    padding: 2px var(--space-2);
    line-height: 20px;
    border-radius: var(--radius);
    cursor: pointer;
  }

  [role='option']:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  [role='option'].chosen {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: min(420px, 100%);
  }
</style>
