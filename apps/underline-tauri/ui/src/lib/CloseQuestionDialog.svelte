<!--
  M-06.3, M-21.4 (UC-005 A14 step 3): "Changes not stored". Closing a tab or
  Underline stored its changes first, and that failed: "Keep open" keeps
  it, "Close anyway" closes without them, and the keyboard then goes on as
  after Delete in the tab bar. Escape keeps it open (inventory D-02,
  fixed); the keyboard starts on "Keep open", which loses nothing.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { CloseQuestion } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import type { Shell } from './shell.svelte';
  import { focusTabAt } from './tabs';

  type Props = { shell: Shell; question: CloseQuestion };

  let { shell, question }: Props = $props();

  /** After the tab closed, the keyboard goes on as after Delete in the tab bar. */
  async function closeAnyway() {
    const tab = question.tab;
    const at = shell.tabs.tabs.findIndex((t) => t.note === tab);
    await shell.closeAnyway();
    if (tab !== null && at >= 0) {
      await tick();
      focusTabAt(at);
    }
  }
</script>

<DialogWindow
  title="Changes not stored"
  onClose={() => shell.keepOpen()}
  step={`${question.tab ?? ''}|${question.error}`}
>
  <p>Changes could not be stored: {question.error}</p>
  <p>Close {question.what} anyway and lose these changes?</p>
  <div class="actions">
    <button type="button" class="ui-button" data-autofocus="true" onclick={() => shell.keepOpen()}
      >Keep open</button
    >
    <button type="button" class="ui-button" onclick={closeAnyway}>Close anyway</button>
  </div>
</DialogWindow>
