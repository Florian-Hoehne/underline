<!--
  M-07.3, UC-004 A4, BR-007: "Open web address", as egui asks it the first
  time a link to a web address is followed in a session: "Open" opens it in
  the default browser, and later ones open without asking until Underline
  closes; "Cancel" opens nothing and asks again next time.

  Not modal, like egui's window. Escape cancels (inventory D-02, fixed in
  the Tauri app); the keyboard starts on "Cancel", so Enter opens nothing
  by mistake.
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell; address: string };

  let { shell, address }: Props = $props();

  const textId = `web-address-${Math.random().toString(36).slice(2)}`;
</script>

<DialogWindow
  title="Open web address"
  onClose={() => void shell.answerWeb(false)}
  step={address}
  describedBy={textId}
>
  <div class="text" id={textId}>
    <p class="address">Open {address} in your browser?</p>
    <p>Underline asks this once until it is closed.</p>
  </div>
  <div class="actions">
    <button type="button" class="ui-button" onclick={() => void shell.answerWeb(true)}>Open</button>
    <button
      type="button"
      class="ui-button"
      data-autofocus="true"
      onclick={() => void shell.answerWeb(false)}>Cancel</button
    >
  </div>
</DialogWindow>

<style>
  .text {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* A long address breaks anywhere instead of widening the dialog. */
  .address {
    overflow-wrap: anywhere;
  }
</style>
