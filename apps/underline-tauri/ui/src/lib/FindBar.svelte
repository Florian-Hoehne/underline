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

  FR-075, UC-008 A7, A9: the search switches after the field (Alt+C,
  Alt+W, Alt+R anywhere in the bar); what is wrong with an expression
  stands in place of the count.

  FR-073, UC-005 A20 to A22, BR-017: the button "Replace" before the label
  shows the row "Replace with:" (Ctrl+H, Cmd+Alt+F on macOS, opens the bar
  with it): Enter in its field, or "Replace", replaces the selected
  occurrence and selects the next; Ctrl+Alt+Enter (Cmd+Alt+Enter), or
  "Replace all", replaces every one as one change and says how many. With
  no occurrence the bar says so, and nothing can be replaced.
-->
<script lang="ts">
  import type { FindStatus, SearchSwitches as Switches } from './bindings';
  import SearchSwitches from './SearchSwitches.svelte';
  import { switchKey, type SwitchName } from './search.svelte';

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
    /** FR-075: the search switches of the field. */
    switches?: Switches;
    onSwitch?: (name: SwitchName) => void;
    /** UC-005 A20: the replace row shows. */
    replacing?: boolean;
    /** The replacement as typed. */
    replacement?: string;
    /** A21: how many the last "Replace all" replaced; `null` when nothing to say. */
    replaced?: number | null;
    /** A replacement runs; the buttons wait (app-tester: no double replace). */
    busy?: boolean;
    /** Cmd+Alt+Enter on macOS, Ctrl+Alt+Enter elsewhere. */
    mac?: boolean;
    onReplacing?: (replacing: boolean) => void;
    onReplacement?: (text: string) => void;
    /** A20 (`false`) or A21 (`true`). */
    onReplace?: (all: boolean) => void;
  };

  let {
    text,
    status,
    focusAsked,
    onText,
    onStep,
    onClose,
    switches = { matchCase: false, wholeWord: false, regex: false },
    onSwitch,
    replacing = false,
    replacement = '',
    replaced = null,
    busy = false,
    mac = false,
    onReplacing,
    onReplacement,
    onReplace,
  }: Props = $props();

  let field = $state<HTMLInputElement | null>(null);

  /** A22: no occurrence, or what is wrong: nothing can be replaced. */
  const problem = $derived(status?.problem ?? null);
  const none = $derived(
    replacing && text !== '' && status !== null && status.count === 0 && problem === null,
  );
  const cannotReplace = $derived(problem !== null || status === null || status.count === 0);

  const count = $derived(
    status === null || problem !== null
      ? ''
      : replaced !== null
        ? `${replaced} replaced`
        : none
          ? 'No occurrence'
          : status.at === null
            ? `${status.count} found`
            : `${status.at} of ${status.count}`,
  );

  const allKeys = $derived(mac ? 'Cmd+Alt+Enter' : 'Ctrl+Alt+Enter');

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

  /** A20: Enter in the replace field replaces the selected occurrence. */
  function onReplaceKey(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (!cannotReplace && !busy) {
        onReplace?.(false);
      }
    }
  }

  /**
   * Anywhere in the bar: Escape closes it (app-tester F9: also on its
   * buttons), Alt+C/W/R switch, Ctrl+Alt+Enter replaces all.
   */
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
      return;
    }
    const name = switchKey(event);
    if (name !== null && onSwitch) {
      event.preventDefault();
      onSwitch(name);
      return;
    }
    const command = mac ? event.metaKey : event.ctrlKey;
    if (event.key === 'Enter' && command && event.altKey && replacing) {
      event.preventDefault();
      if (!cannotReplace && !busy) {
        onReplace?.(true);
      }
    }
  }

  /** UX review F9: the count, told to screen readers once typing pauses. */
  let announced = $state('');
  $effect(() => {
    const said = problem ?? count;
    const timer = setTimeout(() => (announced = said), 500);
    return () => clearTimeout(timer);
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- In the order of Tab: the find field, then the replace field and its
     buttons, then the switches and the steps, which the grid shows on the
     first row. -->
<div class="find-bar" role="search" aria-label="Find in note" onkeydown={onBarKey}>
  <div class="grid" class:replacing>
    {#if onReplacing}
      <button
        type="button"
        class="toggle"
        aria-expanded={replacing}
        aria-controls="replace-in-note-field"
        aria-label="Replace in this note"
        title={mac ? 'Replace in this note (Cmd+Alt+F)' : 'Replace in this note (Ctrl+H)'}
        onclick={() => onReplacing?.(!replacing)}><span aria-hidden="true">›</span></button
      >
    {/if}
    <label class="find-label" for="find-in-note">Find in note:</label>
    <input
      bind:this={field}
      id="find-in-note"
      class="field find"
      type="text"
      spellcheck="false"
      autocomplete="off"
      aria-describedby="find-count"
      aria-invalid={problem !== null}
      value={text}
      oninput={(event) => onText(event.currentTarget.value)}
      onkeydown={onKey}
    />
    {#if replacing}
      <label class="with-label" for="replace-in-note-field">Replace with:</label>
      <input
        id="replace-in-note-field"
        class="field with"
        type="text"
        spellcheck="false"
        autocomplete="off"
        value={replacement}
        oninput={(event) => onReplacement?.(event.currentTarget.value)}
        onkeydown={onReplaceKey}
      />
      <button
        type="button"
        class="one"
        title="Enter"
        aria-keyshortcuts="Enter"
        disabled={cannotReplace || busy}
        onclick={() => onReplace?.(false)}>Replace</button
      >
      <button
        type="button"
        class="all"
        title={allKeys}
        aria-keyshortcuts={mac ? 'Meta+Alt+Enter' : 'Control+Alt+Enter'}
        disabled={cannotReplace || busy}
        onclick={() => onReplace?.(true)}>Replace all</button
      >
    {/if}
    {#if onSwitch}
      <div class="switches">
        <SearchSwitches {switches} onToggle={onSwitch} />
      </div>
    {/if}
    {#if problem !== null}
      <span id="find-count" class="count problem">{problem}</span>
    {:else}
      <span id="find-count" class="count">{count}</span>
    {/if}
    <span class="visually-hidden" aria-live="polite">{announced}</span>
    <button
      type="button"
      class="previous"
      title="Shift+Enter, Shift+F3"
      aria-keyshortcuts="Shift+Enter Shift+F3"
      aria-label="Previous"
      disabled={problem !== null}
      onclick={() => onStep(-1)}
      ><span class="word">Previous</span><span class="arrow" aria-hidden="true">↑</span></button
    >
    <button
      type="button"
      class="next"
      title="Enter, F3"
      aria-keyshortcuts="Enter F3"
      aria-label="Next"
      disabled={problem !== null}
      onclick={() => onStep(1)}
      ><span class="word">Next</span><span class="arrow" aria-hidden="true">↓</span></button
    >
    <button type="button" class="close" title="Escape" aria-label="Close" onclick={onClose}
      >×</button
    >
  </div>
</div>

<style>
  /* Two rows on a grid; the order of Tab is the order in the markup. */
  .find-bar {
    container-type: inline-size;
    flex: 0 0 auto;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(9, auto);
    grid-template-areas: 'toggle flabel find switches count previous next close .';
    justify-content: start;
    align-items: center;
    gap: var(--space-1) var(--space-2);
    padding: var(--space-1) var(--space-2);
  }

  .grid.replacing {
    grid-template-areas:
      'toggle flabel find switches count previous next close .'
      '. wlabel with one all all all . .';
  }

  .toggle {
    grid-area: toggle;
  }

  .find-label {
    grid-area: flabel;
  }

  .find {
    grid-area: find;
  }

  .with-label {
    grid-area: wlabel;
  }

  .with {
    grid-area: with;
  }

  .one {
    grid-area: one;
  }

  .all {
    grid-area: all;
    justify-self: start;
  }

  .switches {
    grid-area: switches;
  }

  .count {
    grid-area: count;
  }

  .previous {
    grid-area: previous;
  }

  .next {
    grid-area: next;
  }

  .close {
    grid-area: close;
  }

  /* A narrow editor: the parts wrap as egui's `horizontal_wrapped` row. */
  @container (max-width: 640px) {
    .grid,
    .grid.replacing {
      display: flex;
      flex-wrap: wrap;
    }

    /* FR-096, review: in a pane of the split view the field gives way
       before the buttons wrap. */
    .field {
      flex: 1 1 120px;
      width: auto;
      min-width: 120px;
      max-width: 200px;
    }
  }

  /* FR-096 (app-tester): below 520 px, as in a pane of the split view,
     finding stays one row: the label is only for screen readers, the field
     narrows, and Previous and Next are arrows (their names stay). */
  .arrow {
    display: none;
  }

  @container (max-width: 520px) {
    .grid:not(.replacing) {
      flex-wrap: nowrap;
      align-items: center;
    }

    .find-label {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }

    .field.find {
      flex: 1 1 60px;
      min-width: 60px;
    }

    .count {
      min-width: 0;
      white-space: nowrap;
    }

    .word {
      display: none;
    }

    .arrow {
      display: inline;
    }
  }

  /* Narrower still, the steps and Close go to a second row rather than out
     of the pane. */
  @container (max-width: 380px) {
    .grid:not(.replacing) {
      flex-wrap: wrap;
    }
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

  .field[aria-invalid='true'] {
    border-style: dashed;
  }

  .count {
    min-width: 4em;
    color: var(--ui-hint);
  }

  /* UC-008 A9, UC-005 A22: marked by a sign, not by colour alone. */
  .problem {
    max-width: 32em;
    color: var(--ui-text);
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .problem::before {
    content: '⚠ ';
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

  button:hover:not(:disabled) {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  button:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  button:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .close {
    padding: 0 var(--space-1);
  }

  /* The chevron turns down while the replace row shows. */
  .toggle {
    padding: 0;
    background: transparent;
    border-color: transparent;
  }

  .toggle span {
    display: inline-block;
    transition: transform 120ms ease-out;
  }

  .toggle[aria-expanded='true'] span {
    transform: rotate(90deg);
  }

  @media (prefers-reduced-motion: reduce) {
    .toggle span {
      transition: none;
    }
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
