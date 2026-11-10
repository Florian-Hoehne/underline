<!--
  M-03.2, M-13.1, M-13.3 (UC-008 steps 1–3, A1, A2, A4, A5): the search
  overlay, as egui's `search_overlay` and `found_list` draw it: a frame
  over the window, centred 44 px from its top, as wide as the window less
  32 px (200 to 640 px), with the field "Search notes" and, while it holds
  text, the found notes under it in a list that scrolls (at most 60 % of
  the window's height, 480 px):

  - each note with its title (bold; the hint names its folder and places),
    "{folder} · {places}" under it, its first lines found with the words
    marked (two rows at most), and "and {n} more";
  - "Searching…" before the first result, "No note holds “{text}”." when
    nothing is found, "{n} notes could not be searched." at the end.

  BR-007, the keyboard: the field keeps it; Down and Up choose an entry
  (`aria-activedescendant`), Enter opens the chosen one (or the first) and
  closes the overlay. Escape anywhere closes it (App.svelte). A click on an
  entry opens it and keeps the overlay open; a click outside closes it,
  except on the magnifier, which closes it itself. Closing empties the
  field, which ends the search and its marks (A2).

  The found notes come page by page (A-89): the next page is asked for when
  the list is scrolled near its end or the keyboard nears the last entry;
  "Showing {k} of {n} notes. Scroll for more." / "Loading more…" say so
  (new texts, egui shows all at once).

  After the UX review (A-96, A-97): while the list is of an older text it is
  dimmed and `aria-busy` under "Searching…", and Enter waits for the result
  of the text held now; a failed search says why in the overlay; a hidden
  live region tells screen readers how many notes were found; a line's name
  is "{title}: {line}"; Tab out of the overlay closes it; closing gives the
  keyboard back where it was (`SearchState.close`).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import type { FoundLineView, FoundNoteView } from './bindings';
  import { choiceOf, type FoundEntry } from './search.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const search = $derived(shell.search);
  const LIST_ID = 'search-found';

  let frame = $state<HTMLElement | null>(null);
  let field = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLElement | null>(null);
  /** Enter was pressed while the list was of an older text (A-96). */
  let enterWaits = $state(false);

  const entries = $derived(search.entries);
  const chosen = $derived<FoundEntry | null>(
    search.cursor === null ? null : (entries[search.cursor] ?? null),
  );
  const result = $derived(search.result);
  /** Waiting for the first result, or for the one of the text held now. */
  const searching = $derived(search.searched !== '' && (result === null || search.stale));

  /** What a screen reader is told of the search (UX review F9). */
  const announcement = $derived.by(() => {
    if (search.searched === '') {
      return '';
    }
    if (search.error !== null) {
      return '';
    }
    if (searching || result === null) {
      return 'Searching…';
    }
    if (result.total === 0) {
      return `No note holds “${search.searched}”.`;
    }
    const found = result.total === 1 ? '1 note found' : `${result.total} notes found`;
    return search.notes.length < result.total
      ? `${found}, ${search.notes.length} of ${result.total} shown`
      : found;
  });

  function entryId(note: number, line: number | null): string {
    return line === null ? `found-${note}` : `found-${note}-${line}`;
  }

  function isChosen(note: number, line: number | null): boolean {
    return chosen !== null && chosen.note === note && chosen.line === line;
  }

  /** "{folder} · {places}", or "{places}" in the top folder. */
  function under(found: FoundNoteView): string {
    return found.folder === null ? found.places : `${found.folder} · ${found.places}`;
  }

  /** The hint of a title: "{folder or 'top folder'} · {places}". */
  function hint(found: FoundNoteView): string {
    return `${found.folder ?? 'top folder'} · ${found.places}`;
  }

  /** A line's name for screen readers: "{title}: {line}". */
  function lineLabel(found: FoundNoteView, line: FoundLineView): string {
    return `${found.title}: ${line.parts.map((part) => part.text).join('')}`;
  }

  // Ctrl+Shift+F or the magnifier: the field takes the keyboard.
  $effect(() => {
    if (search.focusAsked > 0 && field) {
      field.focus();
    }
  });

  // The chosen entry is scrolled into view.
  $effect(() => {
    if (chosen === null) {
      return;
    }
    const id = entryId(chosen.note, chosen.line);
    void tick().then(() => document.getElementById(id)?.scrollIntoView?.({ block: 'nearest' }));
  });

  // A-96: Enter pressed on an older list opens once the fresh result came.
  $effect(() => {
    if (!enterWaits || searching || result === null) {
      return;
    }
    enterWaits = false;
    const choice = search.chosen();
    if (choice !== null) {
      void shell.openFound(choice, true);
    }
  });

  function onKey(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        search.move(1);
        break;
      case 'ArrowUp':
        search.move(-1);
        break;
      case 'Enter': {
        if (searching) {
          // The list shown is of another text: what Enter opens is decided
          // by the result of this one (A-96).
          enterWaits = search.searched !== '';
          break;
        }
        const choice = search.chosen();
        if (choice === null) {
          return;
        }
        void shell.openFound(choice, true);
        break;
      }
      default:
        return;
    }
    event.preventDefault();
  }

  function onInput(text: string) {
    enterWaits = false;
    search.setText(text);
  }

  /** The next page when the list is scrolled near its end. */
  function onScroll() {
    if (list && list.scrollTop + list.clientHeight >= list.scrollHeight - 200) {
      void search.more();
    }
  }

  /** The keyboard left the overlay (Tab): it closes, unless to the magnifier. */
  function onFocusOut(event: FocusEvent) {
    const to = event.relatedTarget as Element | null;
    if (to && !frame?.contains(to) && !to.closest?.('[data-search-toggle]')) {
      search.close(false);
    }
  }

  // A press outside closes the overlay; the magnifier closes it itself.
  onMount(() => {
    const outside = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (frame?.contains(target) || target?.closest?.('[data-search-toggle]')) {
        return;
      }
      search.close();
    };
    window.addEventListener('pointerdown', outside, true);
    return () => window.removeEventListener('pointerdown', outside, true);
  });
</script>

<div
  class="search-overlay"
  role="search"
  aria-label="Search notes"
  bind:this={frame}
  onfocusout={onFocusOut}
>
  <input
    bind:this={field}
    class="field"
    type="text"
    role="combobox"
    placeholder="Search notes"
    aria-label="Search notes"
    aria-autocomplete="list"
    aria-controls={LIST_ID}
    aria-expanded={search.searched !== '' && search.notes.length > 0}
    aria-activedescendant={chosen ? entryId(chosen.note, chosen.line) : undefined}
    spellcheck="false"
    autocomplete="off"
    value={search.text}
    oninput={(event) => onInput(event.currentTarget.value)}
    onkeydown={onKey}
  />
  <p class="visually-hidden" aria-live="polite">{announcement}</p>
  {#if search.searched !== ''}
    <div class="results" bind:this={list} onscroll={onScroll}>
      {#if search.error !== null}
        <p class="error" role="alert">{search.error}</p>
      {:else if result === null || searching}
        <p class="weak">Searching…</p>
      {:else if result.total === 0}
        <p class="weak">No note holds “{search.searched}”.</p>
      {/if}
      {#if result !== null && result.total > 0}
        <ul
          id={LIST_ID}
          class="found"
          class:stale={searching}
          role="listbox"
          aria-label="Found notes"
          aria-busy={searching}
        >
          {#each search.notes as found, n (found.note)}
            <!-- The keyboard stays in the field (BR-007); the entries are for the pointer. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              id={entryId(n, null)}
              class="entry note"
              role="option"
              aria-selected={isChosen(n, null)}
              title={hint(found)}
              onclick={() => void shell.openFound(choiceOf(found, null))}
            >
              <strong class="title">{found.title}</strong>
              <span class="weak under">{under(found)}</span>
            </li>
            {#each found.lines as line, l (l)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <li
                id={entryId(n, l)}
                class="entry line"
                role="option"
                aria-selected={isChosen(n, l)}
                aria-label={lineLabel(found, line)}
                onclick={() => void shell.openFound(choiceOf(found, l))}
              >
                {#each line.parts as part, p (p)}{#if part.hit}<mark>{part.text}</mark
                    >{:else}{part.text}{/if}{/each}
              </li>
            {/each}
            {#if found.more > 0}
              <li class="weak more" role="presentation">and {found.more} more</li>
            {/if}
          {/each}
        </ul>
        {#if search.hasMore}
          <p class="weak paging">
            {search.paging
              ? 'Loading more…'
              : `Showing ${search.notes.length} of ${result.total} notes. Scroll for more.`}
          </p>
        {/if}
      {/if}
      {#if result?.unreadable}
        <p class="weak unreadable">{result.unreadable}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* egui's popup frame, anchored at the top centre, 44 px down. */
  .search-overlay {
    position: fixed;
    top: 44px;
    left: 50%;
    z-index: 25;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: clamp(200px, calc(100vw - 32px), 640px);
    padding: var(--space-2);
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    box-shadow: 0 6px 16px rgb(0 0 0 / 0.25);
    transform: translateX(-50%);
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    min-height: 28px;
    padding: 0 var(--space-2);
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

  .results {
    max-height: min(60vh, 480px);
    overflow-y: auto;
    border-top: 1px solid var(--ui-rule);
    padding-top: var(--space-2);
  }

  .found {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .entry {
    min-height: 24px;
    padding: 2px var(--space-2);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .entry:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  /* Chosen with the keyboard: shown by a line too, not only by colour. */
  .entry[aria-selected='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 2px 0 var(--ui-chosen-text);
  }

  .note {
    display: flex;
    flex-direction: column;
  }

  .note:not(:first-child) {
    margin-top: var(--space-1);
  }

  .title {
    overflow-wrap: anywhere;
  }

  .entry[aria-selected='true'] .weak {
    color: inherit;
  }

  /* A line found: smaller, indented 8 px, two rows at most. */
  .line {
    display: -webkit-box;
    padding-left: calc(var(--space-2) + 8px);
    overflow: hidden;
    font-size: 0.85em;
    overflow-wrap: anywhere;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  mark {
    color: inherit;
    background: var(--palette-found-background);
  }

  .weak {
    margin: 0;
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .error {
    margin: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  /* A-96: the list of an older text while the new one is searched. */
  .stale {
    opacity: 0.55;
  }

  .paging {
    margin-top: var(--space-2);
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

  @media (forced-colors: active) {
    mark {
      forced-color-adjust: none;
      color: HighlightText;
      background: Highlight;
    }

    .entry[aria-selected='true'] {
      outline: 2px solid Highlight;
    }
  }

  .under {
    font-size: 0.85em;
  }

  .more {
    padding-left: calc(var(--space-2) + 16px);
    font-size: 0.85em;
  }

  .unreadable {
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }
</style>
