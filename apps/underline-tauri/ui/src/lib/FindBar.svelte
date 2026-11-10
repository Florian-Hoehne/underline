<!--
  M-13.4 (UC-008 A6, BR-009): the find field of the active note, as egui
  draws it above the note: "Find in note:", a field 200 px wide, "{i} of
  {n}" when the selection is a hit, else "{n} found", "Previous", "Next" and
  "×".

  In the field, Enter selects the next hit and Shift+Enter the one before,
  wrapping around; F3 and Shift+F3 do the same anywhere (App.svelte); the
  keyboard stays where it is. Escape in the field, or "×", closes it and
  gives the keyboard back to the text. A changed text selects the first hit
  from the cursor on. The steps and the count are the editor core's
  (`find`, `findStatus`); this component only shows them.
-->
<script lang="ts">
  import type { FindStatus } from './bindings';

  type Props = {
    text: string;
    /** What the core says of the hits; `null` while no note shows. */
    status: FindStatus | null;
    /** Counts the requests that the field take the keyboard (Ctrl+F). */
    focusAsked: number;
    onText: (text: string) => void;
    /** Next (`1`) or previous (`-1`) hit. */
    onStep: (by: 1 | -1) => void;
    onClose: () => void;
  };

  let { text, status, focusAsked, onText, onStep, onClose }: Props = $props();

  let field = $state<HTMLInputElement | null>(null);

  const count = $derived(
    status === null
      ? ''
      : status.at === null
        ? `${status.count} found`
        : `${status.at} of ${status.count}`,
  );

  $effect(() => {
    if (focusAsked > 0 && field) {
      field.focus();
      field.setSelectionRange(field.value.length, field.value.length);
    }
  });

  /** Enter and Shift+Enter in the field step. */
  function onKey(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      onStep(event.shiftKey ? -1 : 1);
    }
  }

  /** Escape anywhere in the bar closes it (app-tester F9: also on its buttons). */
  function onBarKey(event: KeyboardEvent) {
    if (
      event.key === 'Escape' &&
      !event.shiftKey &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey
    ) {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }

  /** UX review F9: the count, told to screen readers once typing pauses. */
  let announced = $state('');
  $effect(() => {
    const said = count;
    const timer = setTimeout(() => (announced = said), 500);
    return () => clearTimeout(timer);
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="find-bar" role="search" aria-label="Find in note" onkeydown={onBarKey}>
  <label for="find-in-note">Find in note:</label>
  <input
    bind:this={field}
    id="find-in-note"
    class="field"
    type="text"
    spellcheck="false"
    autocomplete="off"
    aria-describedby="find-count"
    value={text}
    oninput={(event) => onText(event.currentTarget.value)}
    onkeydown={onKey}
  />
  <span id="find-count" class="count">{count}</span>
  <span class="visually-hidden" aria-live="polite">{announced}</span>
  <button
    type="button"
    title="Shift+Enter, Shift+F3"
    aria-keyshortcuts="Shift+Enter Shift+F3"
    onclick={() => onStep(-1)}>Previous</button
  >
  <button type="button" title="Enter, F3" aria-keyshortcuts="Enter F3" onclick={() => onStep(1)}
    >Next</button
  >
  <button type="button" class="close" title="Escape" aria-label="Close" onclick={onClose}>×</button>
</div>

<style>
  /* egui's `horizontal_wrapped` row: it wraps in a narrow editor. */
  .find-bar {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
    padding: var(--space-1) var(--space-2);
  }

  .field {
    box-sizing: border-box;
    width: 200px;
    min-height: 24px;
    padding: 0 var(--space-1);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  .field:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -1px;
  }

  .count {
    min-width: 4em;
    color: var(--ui-hint);
  }

  /* egui's small buttons. */
  button {
    min-width: 24px;
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  button:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  button:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .close {
    padding: 0 var(--space-1);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
