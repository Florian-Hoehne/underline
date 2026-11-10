<!--
  M-10.3, UC-005 A2: "Insert link", as egui asks it. A web address, typed
  in the field that has the keyboard, goes in with "Insert" or Enter; or a
  note of the workspace, found by a filter over the overview's names (case
  does not count, at most 50, the list at most 160 px high), goes in when
  chosen: as a link relative to the note, with its title.

  Escape closes it (inventory D-02, fixed in the Tauri app); "Insert" is
  disabled without an address (A-16). Without a note that matches, the list
  says "No notes found" (egui shows nothing). In a low window the list
  shrinks first, so "Insert" and "Cancel" stay in view.
-->
<script lang="ts">
  import type { OverviewItem } from './bindings';
  import DialogWindow from './DialogWindow.svelte';

  type Props = {
    /** The notes of the overview. */
    notes: readonly OverviewItem[];
    /** The backend works on the answer. */
    busy?: boolean;
    /** "Insert": a link to the web address `address`. */
    onAddress: (address: string) => void;
    /** A note was chosen. */
    onNote: (note: string) => void;
    onClose: () => void;
  };

  let { notes, busy = false, onAddress, onNote, onClose }: Props = $props();

  /** egui shows at most this many notes. */
  const SHOWN = 50;

  let address = $state('');
  let filter = $state('');

  const empty = $derived(address.trim() === '');
  const found = $derived.by(() => {
    const needle = filter.toLowerCase();
    return notes.filter((item) => item.label.toLowerCase().includes(needle)).slice(0, SHOWN);
  });

  function insert(event: SubmitEvent) {
    event.preventDefault();
    if (!empty && !busy) {
      onAddress(address.trim());
    }
  }

  /** As in egui, Enter inserts from the address field only. */
  function filterKey(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
    }
  }
</script>

<DialogWindow title="Insert link" {onClose} {busy}>
  <form class="form" onsubmit={insert}>
    <label for="link-address">Web address:</label>
    <input
      id="link-address"
      type="text"
      spellcheck="false"
      autocomplete="off"
      data-autofocus="true"
      bind:value={address}
    />
    <label for="link-filter">Or a note of the workspace:</label>
    <input
      id="link-filter"
      type="text"
      spellcheck="false"
      autocomplete="off"
      bind:value={filter}
      onkeydown={filterKey}
    />
    {#if found.length === 0}
      <p class="none" role="status">No notes found</p>
    {/if}
    <ul class="notes" aria-label="Notes of the workspace">
      {#each found as item (item.note)}
        <li>
          <button
            type="button"
            class="note"
            title={item.note}
            disabled={busy}
            onclick={() => onNote(item.note)}>{item.label}</button
          >
        </li>
      {/each}
    </ul>
    <div class="actions">
      <button type="submit" class="ui-button" disabled={busy || empty}>Insert</button>
      <button type="button" class="ui-button" onclick={onClose}>Cancel</button>
    </div>
  </form>
</DialogWindow>

<style>
  .form {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--space-2);
    min-height: 0;
  }

  .form > * {
    flex: none;
  }

  /* egui: at most 160 px high, scrolling inside; in a low window it
     shrinks to two rows, so the buttons stay in view. */
  .form > .notes {
    flex: 0 1 auto;
    min-height: 48px;
    max-height: 160px;
    overflow: auto;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* egui's selectable labels: plain text, tinted under the pointer. */
  .note {
    display: block;
    box-sizing: border-box;
    width: 100%;
    min-height: 24px;
    padding: 2px var(--space-2);
    overflow: hidden;
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .note:hover:not(:disabled) {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .note:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }
</style>
