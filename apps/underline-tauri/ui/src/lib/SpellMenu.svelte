<!--
  UC-005 A28 (FR-099, ADR-030, OQ-302): the context menu of a word the
  spell check marks, in either editing view.

  - It waits up to 100 ms for the corrections, then opens: the word as a
    header, a line saying how many corrections there are, the corrections
    (the likeliest first), a line, and "Add "{word}" to dictionary" last.
  - Corrections there: the first has the keyboard. None: "Add …" has it.
  - Still looking after 100 ms: it opens with "Looking for corrections…",
    the keyboard on the menu itself; when the corrections come, the first
    takes the keyboard unless the user pressed a key or moved the pointer.
  - The line under the header stays, so the entries do not move under the
    pointer as its text changes; screen readers hear it through a polite
    live region outside the menu ("Looking for corrections", "3
    corrections", "No corrections found", "Corrections unavailable").
  - Escape closes the menu and gives the keyboard back to the text.
-->
<script lang="ts" module>
  /** UC-005 A28: the text changed after the word was marked. */
  export const CHANGED_SINCE =
    'The text changed since the word was marked; the correction was not made';
</script>

<script lang="ts">
  import { onMount } from 'svelte';
  import ContextMenu, { type MenuEntry } from './ContextMenu.svelte';
  import { addToDictionary, spellSuggestions } from './spell';

  type Props = {
    /** The word as shown. */
    word: string;
    /** Where the menu opens, in CSS pixels of the window: below the word's line. */
    x: number;
    y: number;
    /** A correction was chosen. */
    onChoose: (suggestion: string) => void;
    /** Adding the word failed: the reason, for the status bar. */
    onError: (message: string) => void;
    /** Closed; `back` gives the keyboard back to the text. */
    onClose: (back: boolean) => void;
    /** How long the menu waits for the corrections before it opens. */
    wait?: number;
  };

  let { word, x, y, onChoose, onError, onClose, wait = 100 }: Props = $props();

  /** The corrections; `null` while they are looked for. */
  let suggestions = $state<string[] | null>(null);
  /** The corrections could not be looked for. */
  let failed = $state(false);
  /** The menu shows (after the corrections or the wait). */
  let shown = $state(false);
  /** It opened while the corrections were looked for. */
  let openedEarly = $state(false);
  /** The user pressed a key or moved the pointer in the menu. */
  let touched = false;
  /** Asks the menu to give the keyboard to the first correction. */
  let focusFirst = $state(0);

  onMount(() => {
    const asked = word;
    let gone = false;
    const timer = setTimeout(() => {
      if (!gone && !shown) {
        openedEarly = true;
        shown = true;
      }
    }, wait);
    spellSuggestions(asked)
      .then((found) => {
        if (gone) return;
        suggestions = found;
        if (openedEarly && found.length > 0 && !touched) {
          focusFirst++;
        }
      })
      .catch(() => {
        if (gone) return;
        failed = true;
        suggestions = [];
      })
      .finally(() => {
        if (!gone) shown = true;
      });
    return () => {
      gone = true;
      clearTimeout(timer);
    };
  });

  async function add() {
    const refused = await addToDictionary(word);
    if (refused !== null) {
      onError(refused);
    }
  }

  const entries = $derived<MenuEntry[]>([
    ...(suggestions ?? []).map((suggestion) => ({
      label: suggestion,
      action: () => onChoose(suggestion),
    })),
    {
      label: `Add "${word}" to dictionary`,
      separatorBefore: (suggestions?.length ?? 0) > 0,
      action: () => void add(),
    },
  ]);

  const count = $derived(suggestions?.length ?? 0);
  /** The line under the header; always there, so nothing moves. */
  const status = $derived(
    suggestions === null
      ? 'Looking for corrections…'
      : failed
        ? 'Corrections unavailable'
        : count === 0
          ? 'No corrections found'
          : count === 1
            ? '1 correction'
            : `${count} corrections`,
  );
  /** What screen readers hear of it. */
  const said = $derived(status.replace('…', ''));
</script>

{#if shown}
  <ContextMenu
    label="Spelling of {word}, misspelled"
    header={word}
    {entries}
    {status}
    {x}
    {y}
    focusMenu={openedEarly && suggestions === null}
    {focusFirst}
    onTouched={() => (touched = true)}
    {onClose}
  />
{/if}
<div class="visually-hidden" aria-live="polite">{shown ? said : ''}</div>

<style>
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
