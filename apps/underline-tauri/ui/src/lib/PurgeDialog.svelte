<!--
  M-14.4, UC-007 A7: "Remove deleted notes", before one deleted note
  ("Remove") or all of them ("Empty the deleted notes") are removed for
  good: "Remove {n} deleted note(s) for good? They cannot be restored
  afterwards."

  Inventory D-01 (fixed, paritaet.md A-81): Enter and Escape act only in
  the dialog; the keyboard starts on "Remove", so Enter confirms as in egui.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { PurgeDialog, Shell } from './shell.svelte';

  type Props = { shell: Shell; dialog: PurgeDialog };

  let { shell, dialog }: Props = $props();

  const busy = $derived(shell.noteBusy('purge'));
  const textId = `purge-${Math.random().toString(36).slice(2)}`;
  /** The number of notes when the question showed (UX review F8). */
  const notes = $derived(dialog.count === 1 ? '1 deleted note' : `${dialog.count} deleted notes`);
  const shownAt = performance.now();

  /**
   * UX review F8: an Enter held down from before, or repeated, does not
   * remove notes for good right as the question shows.
   */
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && (event.repeat || performance.now() - shownAt < 300)) {
      event.preventDefault();
    }
  }
</script>

<DialogWindow
  title="Remove deleted notes"
  onClose={() => shell.closeNoteDialog('purge')}
  {busy}
  step={dialog.key ?? 'all'}
  describedBy={textId}
>
  <p id={textId}>Remove {notes} for good? They cannot be restored afterwards.</p>
  <div class="actions">
    <button
      type="button"
      class="ui-button"
      data-autofocus="true"
      disabled={busy}
      onkeydown={onKey}
      onclick={() => void shell.purge()}>Remove</button
    >
    <button type="button" class="ui-button" onclick={() => shell.closeNoteDialog('purge')}
      >Cancel</button
    >
  </div>
</DialogWindow>
