<!--
  M-15.6, UC-009 A2 step 3: "Approximated blocks", after "Show" of the
  status bar: the lines of the blocks the export to the other note format
  only approximated, as egui lists them ("Line N"), with egui's text. The
  heading names the note ("Approximated blocks of {t}", new text, A-122).
  It stays until "Close" or Escape (UX review F11; egui closed it with the
  next status).
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell; title: string; lines: number[] };

  let { shell, title, lines }: Props = $props();

  const textId = `approximated-${Math.random().toString(36).slice(2)}`;
</script>

<DialogWindow
  title="Approximated blocks of {title}"
  onClose={() => shell.closeApproximated()}
  describedBy={textId}
>
  <p id={textId}>These blocks of the note have no exact counterpart in the export:</p>
  <ul class="lines" aria-labelledby={textId}>
    {#each lines as line, index (index)}
      <li>Line {line}</li>
    {/each}
  </ul>
  <div class="actions">
    <button
      type="button"
      class="ui-button"
      data-autofocus="true"
      onclick={() => shell.closeApproximated()}>Close</button
    >
  </div>
</DialogWindow>

<style>
  .lines {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
</style>
