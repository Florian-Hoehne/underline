<!--
  M-19.10, UC-015 A5 step 1: "Abort the sync" asks first, with egui's text.
  "Abort the sync" puts the workspace back as it was before the sync joined
  (A5 step 2); "Cancel" or Escape changes nothing.

  New (A-144): the keyboard starts on "Cancel", so Enter never aborts by
  chance; egui's window takes no Enter at all.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const textId = `abort-${Math.random().toString(36).slice(2)}`;
</script>

<DialogWindow
  title="Abort the sync"
  onClose={() => shell.closeAbort()}
  busy={shell.abortBusy}
  describedBy={textId}
>
  <p id={textId} class="text">
    The workspace goes back to how it was before the sync; places you resolved are lost. Your
    commits and the changes you left out stay.
  </p>
  <div class="actions">
    <button
      type="button"
      class="ui-button"
      disabled={shell.abortBusy}
      onclick={() => void shell.abortSync()}>Abort the sync</button
    >
    <button type="button" class="ui-button" data-autofocus="true" onclick={() => shell.closeAbort()}
      >Cancel</button
    >
  </div>
</DialogWindow>

<style>
  .text {
    max-width: 52ch;
  }
</style>
