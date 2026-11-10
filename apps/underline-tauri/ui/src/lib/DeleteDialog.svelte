<!--
  M-14.3, UC-007 steps 2–3, A8: "Delete note", as egui asks it: "Delete
  the note {t}?", how many links in other notes lead to it (BR-001), for
  the Home note the warning about the GitHub wiki (BR-010), and "It is kept
  among the deleted notes for 30 days." When the note cannot be kept, the
  dialog asks instead to delete it for good ("Delete for good").

  Inventory D-01 (fixed, paritaet.md A-81): Enter and Escape act only in
  the dialog. The keyboard starts on "Delete", so Enter confirms as in egui;
  Escape and "Cancel" change nothing (A1).
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { DeleteDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: DeleteDialog };

  let { shell, dialog }: Props = $props();

  const busy = $derived(shell.noteBusy('delete'));
  const textId = `delete-note-${Math.random().toString(36).slice(2)}`;
</script>

<DialogWindow
  title="Delete note"
  onClose={() => shell.closeNoteDialog('delete')}
  {busy}
  step="{dialog.note}|{dialog.cannotKeep ?? ''}"
  describedBy={textId}
>
  <div class="text" id={textId}>
    <p class="name">Delete the note {dialog.title}?</p>
    <p>{dialog.links}</p>
    {#if dialog.home}
      <p class="problem">
        This is the Home note, the start page of a GitHub wiki. Deleted, the wiki has no start page.
      </p>
    {/if}
    {#if dialog.cannotKeep !== null}
      <p class="problem" role="alert">{dialog.cannotKeep}</p>
    {:else}
      <p class="weak">It is kept among the deleted notes for 30 days.</p>
    {/if}
  </div>
  <div class="actions">
    <button
      type="button"
      class="ui-button"
      data-autofocus="true"
      disabled={busy}
      onclick={() => void shell.deleteNote()}
      >{dialog.cannotKeep !== null ? 'Delete for good' : 'Delete'}</button
    >
    <button type="button" class="ui-button" onclick={() => shell.closeNoteDialog('delete')}
      >Cancel</button
    >
  </div>
</DialogWindow>

<style>
  .text {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .name {
    overflow-wrap: anywhere;
  }

  .weak {
    color: var(--ui-hint);
  }
</style>
