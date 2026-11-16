<!--
  UC-022 steps 2–4, A1, A2, A3, A7, BR-004, BR-008: the places of the
  replace of the workspace, in the search overlay under the field "Replace
  with": "{n} places in {m} notes", then each note with its title, its
  folder and how many places it holds, and under it each place with its
  line as it is and as it will be: the place struck through, what takes its
  place marked after it. Each note and each place has a check box, ticked
  at first; a place left out, or a note, stays as it is. "Replace in note"
  replaces in one note only (A3); "Replace" below the list replaces every
  chosen place (step 4).

  The keyboard (BR-008): Down from the fields goes into the list, which is
  one stop for Tab; in it Up and Down move, Space leaves the chosen entry
  out or takes it in again, Ctrl+Enter (Cmd+Enter) replaces in its note,
  and Up at the top goes back to the field. Ctrl+Alt+Enter (Cmd+Alt+Enter)
  replaces anywhere in the overlay.

  The places come page by page, as the found notes of the search do; A7
  says how many more places there are than are shown.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { ReplaceNoteView, ReplacePlaceView } from './bindings';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    /** The list takes the keyboard (Down in a field). */
    focusAsked: number;
    /** Up at the top of the list: back to the field. */
    onLeaveTop: () => void;
  };

  let { shell, focusAsked, onLeaveTop }: Props = $props();

  const search = $derived(shell.search);
  const replace = $derived(shell.search.replace);
  const result = $derived(replace.result);
  const entries = $derived(replace.entries);
  const chosen = $derived(replace.cursor === null ? null : (entries[replace.cursor] ?? null));
  const mac = $derived(shell.platform === 'macos');
  const LIST_ID = 'replace-places';

  let list = $state<HTMLElement | null>(null);
  let scroller = $state<HTMLElement | null>(null);

  function entryId(note: number, place: number | null): string {
    return place === null ? `replace-${note}` : `replace-${note}-${place}`;
  }

  /** Whether the entry is the one the keyboard is at. */
  function isCursor(note: number, place: number | null): boolean {
    return chosen !== null && chosen.note === note && chosen.place === place;
  }

  /** Whether the keyboard is at the note `note` or one of its places. */
  function inNote(note: number): boolean {
    return chosen !== null && chosen.note === note;
  }

  /** A place for screen readers: "line {n}: '{old}' becomes '{new}'". */
  function placeLabel(place: ReplacePlaceView): string {
    return `line ${place.line}: '${place.old}' becomes '${place.new}'`;
  }

  /** The note's name for screen readers. */
  function noteLabel(found: ReplaceNoteView): string {
    const folder = found.folder === null ? '' : ` in ${found.folder}`;
    return `${found.title}${folder}, ${found.count}`;
  }

  const legend = $derived(`Space: leave out · ${mac ? 'Cmd' : 'Ctrl'}+Enter: replace in this note`);

  $effect(() => {
    if (focusAsked > 0 && list) {
      if (replace.cursor === null && entries.length > 0) {
        replace.cursor = 0;
      }
      list.focus();
    }
  });

  // The chosen entry is scrolled into view.
  $effect(() => {
    if (chosen === null) {
      return;
    }
    const id = entryId(chosen.note, chosen.place);
    void tick().then(() => document.getElementById(id)?.scrollIntoView?.({ block: 'nearest' }));
  });

  function onKey(event: KeyboardEvent) {
    const command = mac ? event.metaKey : event.ctrlKey;
    if (event.key === 'Enter' && command && !event.altKey) {
      event.preventDefault();
      const note = replace.chosenNote;
      if (note !== null) {
        void shell.confirmReplace(note.note);
      }
      return;
    }
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        replace.move(1);
        break;
      case 'ArrowUp':
        if (replace.cursor === null || replace.cursor === 0) {
          replace.cursor = null;
          onLeaveTop();
        } else {
          replace.move(-1);
        }
        break;
      case ' ':
        replace.toggleChosen();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  /** The next page when the list is scrolled near its end. */
  function onScroll() {
    if (scroller && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 200) {
      void replace.more();
    }
  }
</script>

<div class="results" bind:this={scroller} onscroll={onScroll}>
  {#if replace.problem !== null}
    <p class="problem">{replace.problem}</p>
  {:else if replace.error !== null}
    <p class="problem">{replace.error}</p>
  {:else if search.text === ''}
    <p class="weak">Type what to replace, then what replaces it.</p>
  {:else if result === null || replace.searching}
    <p class="weak">Searching…</p>
  {:else if result.places === 0}
    <p class="weak">No note holds “{search.text}”. There is nothing to replace.</p>
  {/if}
  {#if replace.problem === null && result !== null && result.places > 0}
    <!-- What is chosen stays in view while the list scrolls under it. -->
    <div class="head" class:stale={replace.searching}>
      <p class="summary">{result.summary}</p>
      {#if replace.chosen !== null}
        <p class="weak">{replace.chosen.summary}</p>
      {/if}
      {#if result.more}
        <p class="weak more">{result.more}</p>
      {/if}
      {#if replace.said !== ''}
        <p class="said">{replace.said}</p>
      {/if}
    </div>
    <ul
      id={LIST_ID}
      class="places"
      class:stale={replace.searching}
      role="listbox"
      aria-label="Places to replace"
      aria-multiselectable="true"
      aria-busy={replace.searching}
      aria-activedescendant={chosen ? entryId(chosen.note, chosen.place) : undefined}
      tabindex="0"
      bind:this={list}
      onkeydown={onKey}
    >
      {#each replace.notes as found, n (found.note)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id={entryId(n, null)}
          class="entry note"
          class:cursor={isCursor(n, null)}
          class:left={found.leftOut}
          role="option"
          aria-selected={!found.leftOut}
          aria-label={noteLabel(found)}
          onclick={() =>
            (replace.cursor = entries.findIndex((e) => e.note === n && e.place === null))}
        >
          <label class="box">
            <input
              type="checkbox"
              tabindex="-1"
              checked={!found.leftOut}
              aria-hidden="true"
              onchange={() => replace.toggle(n, null)}
            />
          </label>
          <span class="name">
            <strong class="title">{found.title}</strong>
            <span class="weak"
              >{found.folder === null ? found.count : `${found.folder} · ${found.count}`}</span
            >
          </span>
          <button
            type="button"
            class="one"
            class:shown={inNote(n)}
            tabindex={inNote(n) ? 0 : -1}
            aria-label="Replace in {found.title}"
            aria-disabled={found.leftOut || replace.replacing || replace.searching}
            title={mac ? 'Cmd+Enter in the list' : 'Ctrl+Enter in the list'}
            onclick={(event) => {
              event.stopPropagation();
              if (!found.leftOut && !replace.replacing && !replace.searching) {
                void shell.confirmReplace(found.note);
              }
            }}>Replace in this note</button
          >
        </li>
        {#each found.places as place, p (place.at)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <li
            id={entryId(n, p)}
            class="entry place"
            class:cursor={isCursor(n, p)}
            class:left={place.leftOut || found.leftOut}
            role="option"
            aria-selected={!place.leftOut && !found.leftOut}
            aria-label={placeLabel(place)}
            onclick={() =>
              (replace.cursor = entries.findIndex((e) => e.note === n && e.place === p))}
          >
            <label class="box">
              <input
                type="checkbox"
                tabindex="-1"
                checked={!place.leftOut}
                disabled={found.leftOut}
                aria-hidden="true"
                onchange={() => replace.toggle(n, p)}
              />
            </label>
            <span class="line" aria-hidden="true">{place.line}</span>
            <span class="change" aria-hidden="true"
              >{place.before}<del>{place.old}</del><ins>{place.new}</ins>{place.after}</span
            >
          </li>
        {/each}
      {/each}
    </ul>
    {#if replace.hasMore}
      <p class="weak paging">
        {replace.paging
          ? 'Loading more…'
          : `Showing ${replace.notes.length} of ${result.listed} notes. Scroll for more.`}
      </p>
    {/if}
    <p class="weak legend">{legend}</p>
  {/if}
  {#if result?.unreadable}
    <p class="weak unreadable">{result.unreadable}</p>
  {/if}
</div>

<style>
  .results {
    max-height: min(60vh, 480px);
    overflow-y: auto;
    border-top: 1px solid var(--ui-rule);
    padding-top: var(--space-2);
  }

  .summary {
    font-weight: 600;
  }

  .places {
    margin: 0;
    padding: 0;
    list-style: none;
    border-radius: var(--radius);
  }

  .places:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .entry {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 24px;
    padding: 2px var(--space-2);
    border-radius: var(--radius);
  }

  .entry:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  /* Where the keyboard is: shown by a line too, not only by colour. */
  .entry.cursor {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 2px 0 var(--ui-chosen-text);
  }

  .note:not(:first-child) {
    margin-top: var(--space-1);
  }

  .name {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    overflow-wrap: anywhere;
  }

  .name .weak {
    font-size: 0.85em;
  }

  .entry.cursor .weak {
    color: inherit;
  }

  .place {
    padding-left: calc(var(--space-2) + 8px);
    font-size: 0.85em;
    cursor: default;
  }

  .line {
    min-width: 2.5em;
    color: var(--ui-hint);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .change {
    flex: 1 1 auto;
    min-width: 0;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  /* What goes is struck through, what comes is marked: not by colour alone. */
  del {
    text-decoration: line-through;
    text-decoration-thickness: 2px;
    background: color-mix(in srgb, var(--ui-hint) 22%, transparent);
  }

  ins {
    text-decoration: none;
    font-weight: 600;
    background: var(--palette-found-background);
  }

  /* Left out: it stays as it is; the change is no longer shown as one. */
  .left .change del {
    text-decoration: none;
    background: transparent;
  }

  .left .change ins {
    display: none;
  }

  .left .title,
  .left .change {
    color: var(--ui-hint);
  }

  /* WCAG 2.2: the whole 24 × 24 px box toggles the check box. */
  .box {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    cursor: pointer;
  }

  input[type='checkbox'] {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--ui-focus);
    cursor: inherit;
  }

  .head {
    position: sticky;
    top: calc(-1 * var(--space-2));
    z-index: 1;
    margin-bottom: var(--space-1);
    padding: var(--space-1) 0;
    background: var(--ui-surface);
  }

  .head p {
    margin: 0;
  }

  .said {
    font-weight: 600;
  }

  .legend {
    margin-top: var(--space-2);
    font-size: 0.85em;
  }

  .one {
    flex: 0 0 auto;
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    font-size: 0.85em;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    cursor: pointer;
    visibility: hidden;
  }

  .note:hover .one,
  .one.shown,
  .one:focus-visible {
    visibility: visible;
  }

  .one:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .one[aria-disabled='true'] {
    color: var(--ui-hint);
    cursor: default;
  }

  .weak {
    margin: 0;
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .more {
    margin-bottom: var(--space-1);
  }

  .problem {
    margin: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .problem::before {
    content: '⚠ ';
  }

  .stale {
    opacity: 0.55;
  }

  .paging {
    margin-top: var(--space-2);
  }

  .unreadable {
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }

  @media (forced-colors: active) {
    ins {
      forced-color-adjust: none;
      color: HighlightText;
      background: Highlight;
    }

    .entry.cursor {
      outline: 2px solid Highlight;
    }
  }
</style>
