<!--
  M-19.5, UC-011 A6: "Discard changes", as egui asks it: "Discard the
  changes of {title} since the last commit?" and what happens: "The note is
  new; it goes to the deleted notes.", "The note comes back as it was
  committed." or "N changed line(s) will be lost and cannot be restored."
  (BR-005).

  Inventory D-01 (fixed, as A-81): Enter and Escape act only in the dialog.
  The keyboard starts on "Discard", so Enter confirms as in egui; Escape and
  "Cancel" change nothing (A6 step 4).
-->
<script lang="ts">
  import type { DiscardQuestion } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell; question: DiscardQuestion };

  let { shell, question }: Props = $props();

  const textId = `discard-${Math.random().toString(36).slice(2)}`;
</script>

<DialogWindow
  title="Discard changes"
  onClose={() => shell.closeDiscard()}
  busy={shell.discardBusy}
  step={question.path}
  describedBy={textId}
>
  <div class="text" id={textId}>
    <p class="name">{question.question}</p>
    <p>{question.consequence}</p>
  </div>
  <div class="actions">
    <button
      type="button"
      class="ui-button"
      data-autofocus="true"
      disabled={shell.discardBusy}
      onclick={() => void shell.discard()}>Discard</button
    >
    <button type="button" class="ui-button" onclick={() => shell.closeDiscard()}>Cancel</button>
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
</style>
