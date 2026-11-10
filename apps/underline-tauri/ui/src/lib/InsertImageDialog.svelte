<!--
  M-10.4, UC-005 A2, A19: "Insert image", as egui asks it. The path of an
  image file, typed (no system dialog, inventory D-12), goes in with
  "Insert" or Enter: copied next to the note from outside the workspace,
  referred to where it is inside; a file that is no image becomes a link
  with its name. A file over 10 MB is asked about first: "Copy", "Link to
  its place" (it then stays outside the workspace) or "Cancel".

  Escape closes it (inventory D-02, fixed in the Tauri app); "Insert" is
  disabled without a path (A-16). While the backend puts the file in, the
  dialog says so and cannot be closed (A-63).
-->
<script lang="ts">
  import DialogWindow from './DialogWindow.svelte';

  type Props = {
    /** A19: the large file asked about, with egui's question. */
    large?: { path: string; question: string } | null;
    /** The backend works on the answer. */
    busy?: boolean;
    /** "Insert": the file at `path`. */
    onFile: (path: string) => void;
    /** A19 "Copy": the large file is copied all the same. */
    onCopy: () => void;
    /** A19 "Link to its place". */
    onLinkPlace: () => void;
    onClose: () => void;
  };

  let { large = null, busy = false, onFile, onCopy, onLinkPlace, onClose }: Props = $props();

  let path = $state('');
  const empty = $derived(path.trim() === '');
  const questionId = `image-question-${Math.random().toString(36).slice(2)}`;
  /** The file being put in, while the backend works. */
  const working = $derived(large ? large.path : path.trim());

  function insert(event: SubmitEvent) {
    event.preventDefault();
    if (!empty && !busy) {
      onFile(path.trim());
    }
  }
</script>

<DialogWindow
  title="Insert image"
  {onClose}
  {busy}
  step={large ? 'large' : 'file'}
  describedBy={large ? questionId : undefined}
>
  {#if large}
    <p class="question" id={questionId}>{large.question}</p>
    <div class="actions">
      <button type="button" class="ui-button" disabled={busy} onclick={onCopy}>Copy</button>
      <button type="button" class="ui-button" disabled={busy} onclick={onLinkPlace}
        >Link to its place</button
      >
      <button type="button" class="ui-button" disabled={busy} onclick={onClose}>Cancel</button>
    </div>
  {:else}
    <form class="form" onsubmit={insert}>
      <label for="image-path">Image file:</label>
      <input
        id="image-path"
        type="text"
        spellcheck="false"
        autocomplete="off"
        data-autofocus="true"
        bind:value={path}
      />
      <div class="actions">
        <button type="submit" class="ui-button" disabled={busy || empty}>Insert</button>
        <button type="button" class="ui-button" disabled={busy} onclick={onClose}>Cancel</button>
      </div>
    </form>
  {/if}
  {#if busy}
    <p class="working" role="status">Inserting {working}…</p>
  {/if}
</DialogWindow>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .question,
  .working {
    max-width: 60ch;
    overflow-wrap: anywhere;
  }
</style>
