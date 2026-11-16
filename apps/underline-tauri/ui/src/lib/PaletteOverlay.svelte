<!--
  FR-076, FR-077: go to note (UC-004 A12, A13) and the command palette
  (UC-023), one overlay over the window where the search's stands (44 px
  from the top, centred, 200 to 640 px wide), with one field and the list
  under it (at most 60 % of the window's height, 480 px).

  - Go to note: each note with its title and its folder; with nothing
    matching, "No note matches “{text}”." and the entry to create a note
    with the text typed as its title (A13). The notes come page by page as
    the list is scrolled or the keyboard nears its end.
  - The command palette, while the field starts with `>` (UC-023 BR-007):
    each command with its name, its menu and its key as the platform names
    it (BR-003), one not available now dimmed and marked "not available"
    after the others (A2, OQ-197); choosing one says why in the overlay and
    runs nothing. "No command matches “{text}”." when none does (A1).
  - A command that asks for a choice (A3) lists its choices in the same
    field, the one in effect marked "current" and chosen at first.
  - FR-086, UC-004 A16, A17: "Go to line" asks in the same field, saying
    how many lines the note has and that a column may follow a colon;
    what is no line number is said above the hint, and the text stays.

  After the UX review (package 2): the field's name stands above it, go to
  note hints "Go to note (type > for commands)"; "New note “{text}”…" stands
  last whenever something is typed; the chosen command that is not
  available says why on a line under it (every such row is described by
  its reason); a key that runs its command in one place only names it (OQ-243);
  the pointer resting on an entry chooses it; Backspace in the empty field
  of choices goes back to the commands; in a narrow window a row wraps.

  The keyboard stays in the field (combobox with `aria-activedescendant`):
  Down and Up move round the list, Enter chooses (Ctrl+Enter, Cmd+Enter on
  macOS, opens the note chosen to the side, UC-004 A18), Escape closes and gives
  the keyboard back where it was (A5), as does a press outside; Tab out of
  the overlay closes it. A click on an entry chooses it. A hidden polite
  region says how many match once typing pauses.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { KEY_PLACE_TEXT, keyText, type Command } from './commands';
  import type { PaletteEntry } from './palette.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const palette = $derived(shell.palette);
  const LIST_ID = 'palette-list';
  const mac = $derived(shell.platform === 'macos');

  let frame = $state<HTMLElement | null>(null);
  let field = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLElement | null>(null);

  const entries = $derived(palette.list.entries);
  /** UC-023 BR-004: how many entries at the top are the recently run commands. */
  const recentCount = $derived(palette.list.recent ?? 0);
  const RECENT_ID = 'palette-recently-used';
  const mode = $derived(palette.mode);
  const typed = $derived(palette.typed.trim());

  /** The field's name and hint, by what it is now. */
  const label = $derived.by(() => {
    if (mode === 'line') {
      return 'Go to line';
    }
    if (mode === 'choices') {
      return (palette.asking?.title ?? '').replace(/…$/, '');
    }
    return mode === 'commands' ? 'Command palette' : 'Go to note';
  });

  /** The hint in the empty field: go to note says how to reach the commands. */
  const placeholder = $derived.by(() => {
    if (mode === 'line') {
      return palette.lines === null ? 'Line number' : `Line number, 1 to ${palette.lines}`;
    }
    return mode === 'notes' ? 'Go to note (type > for commands)' : label;
  });

  /** How many notes the list shows (without the entry to create one). */
  const notesShown = $derived(entries.filter((entry) => entry.kind === 'note').length);

  /** A command's key with the place it runs there, if not everywhere (OQ-243). */
  function keyOf(command: Command): string {
    const key = keyText(command.key, mac);
    return command.keyPlace === undefined || key === ''
      ? key
      : `${key} ${KEY_PLACE_TEXT[command.keyPlace]}`;
  }

  /** What stands above the list instead of entries, or before them (A1, A13). */
  const message = $derived.by(() => {
    if (!palette.ready) {
      return '';
    }
    if (mode === 'line') {
      // A16 step 1: how many lines the note has, and how a column is given.
      const lines = palette.lines === 1 ? 'line' : 'lines';
      if (palette.linesOfFacade) {
        // UX review: the facade counts the lines of the markup.
        return palette.lines === null ? '' : `The note has ${palette.lines} ${lines} of markup.`;
      }
      const count = palette.lines === null ? '' : `The note has ${palette.lines} ${lines}. `;
      return `${count}Add a column after a colon, such as 120:8.`;
    }
    if (mode === 'notes') {
      if (shell.workspace === null) {
        return 'No workspace is open.';
      }
      if (typed === '' && entries.length === 0) {
        return 'The workspace holds no notes yet.';
      }
      return palette.list.total === 0 && typed !== '' ? `No note matches “${typed}”.` : '';
    }
    if (entries.length > 0) {
      return '';
    }
    return mode === 'commands' ? `No command matches “${typed}”.` : `No choice matches “${typed}”.`;
  });

  /** What a screen reader is told of the list, once typing pauses. */
  const counted = $derived.by(() => {
    if (message !== '') {
      return message;
    }
    if (mode === 'line') {
      return message;
    }
    const total = mode === 'notes' ? palette.list.total : entries.length;
    const noun = { notes: 'note', commands: 'command', choices: 'choice' }[mode];
    const count = total === 1 ? `1 ${noun}` : `${total} ${noun}s`;
    // A13: the last entry creates a note with the text typed.
    return mode === 'notes' && typed !== '' ? `${count}, or New note “${typed}”…` : count;
  });
  let spoken = $state('');
  $effect(() => {
    const said = counted;
    const timer = setTimeout(() => (spoken = said), 400);
    return () => clearTimeout(timer);
  });

  function entryId(index: number): string {
    return `palette-entry-${index}`;
  }

  /** The entry's name for screen readers. */
  function entryLabel(entry: PaletteEntry): string {
    switch (entry.kind) {
      case 'note':
        return entry.folder === null ? entry.title : `${entry.title}, ${entry.folder}`;
      case 'command': {
        const key = keyOf(entry.command);
        return [
          entry.command.title,
          entry.command.menu,
          key === '' ? null : key,
          entry.unavailable === null ? null : 'not available',
        ]
          .filter((part) => part !== null)
          .join(', ');
      }
      case 'choice':
        return entry.choice.current ? `${entry.choice.label}, current` : entry.choice.label;
      case 'create':
        return `New note “${entry.title}”…`;
    }
  }

  // Opening, and a command that asks (A3): the field takes the keyboard.
  $effect(() => {
    if (palette.focusAsked > 0 && field) {
      field.focus();
    }
  });

  // The chosen entry is scrolled into view.
  $effect(() => {
    const at = palette.cursor;
    void entries;
    void tick().then(() =>
      document.getElementById(entryId(at))?.scrollIntoView?.({ block: 'nearest' }),
    );
  });

  // A press outside closes the overlay, running nothing (A5).
  $effect(() => {
    const outside = (event: PointerEvent) => {
      const target = event.target as Element | null;
      // The top bar's button closes it itself.
      if (!frame?.contains(target) && !target?.closest?.('[data-palette-toggle]')) {
        palette.close(true, true);
      }
    };
    window.addEventListener('pointerdown', outside, true);
    return () => window.removeEventListener('pointerdown', outside, true);
  });

  function onKey(event: KeyboardEvent) {
    const command = mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
    if (event.key === 'Enter' && command && !event.altKey && !event.shiftKey) {
      // UC-004 A18 (FR-096): the note chosen opens to the side.
      event.preventDefault();
      palette.choose(undefined, true);
      return;
    }
    if (event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    switch (event.key) {
      case 'ArrowDown':
        palette.move(1);
        break;
      case 'ArrowUp':
        palette.move(-1);
        break;
      case 'Enter':
        palette.choose();
        break;
      case 'Escape':
        event.stopPropagation();
        palette.close();
        break;
      case 'Backspace':
        // A3: an empty field of choices goes back to the commands.
        if (!palette.back()) {
          return;
        }
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  /** The keyboard left the overlay (Tab): it closes. */
  function onFocusOut(event: FocusEvent) {
    const to = event.relatedTarget as Element | null;
    if (to && !frame?.contains(to) && !to.closest?.('[data-palette-toggle]')) {
      palette.close(false);
    }
  }

  /** The next page of notes when the list is scrolled near its end. */
  function onScroll() {
    if (list && list.scrollTop + list.clientHeight >= list.scrollHeight - 200) {
      palette.more();
    }
  }
</script>

<div class="palette" role="dialog" aria-label={label} bind:this={frame} onfocusout={onFocusOut}>
  <!-- UX review: what the field is now, visible, not only in its hint. -->
  <div class="head">
    <span class="mode" aria-hidden="true">{label}</span>
    {#if mode === 'notes'}
      <!-- UC-004 A18 (review): how a note opens to the side. -->
      <span id="palette-side-hint" class="side-hint"
        >{mac ? 'Cmd' : 'Ctrl'}+Enter opens to the side</span
      >
    {/if}
  </div>
  <input
    bind:this={field}
    class="field"
    type="text"
    role="combobox"
    {placeholder}
    aria-label={label}
    aria-describedby={mode === 'notes' ? 'palette-side-hint' : undefined}
    aria-autocomplete="list"
    aria-controls={entries.length > 0 ? LIST_ID : undefined}
    aria-expanded={entries.length > 0}
    aria-activedescendant={palette.chosen ? entryId(palette.cursor) : undefined}
    spellcheck="false"
    autocomplete="off"
    value={palette.text}
    oninput={(event) => palette.setText(event.currentTarget.value)}
    onkeydown={onKey}
  />
  {#if palette.notice !== null}
    <p class="notice" role="alert">{palette.notice}</p>
  {/if}
  <p class="visually-hidden" aria-live="polite">{spoken}</p>
  {#if message !== '' || entries.length > 0}
    <!-- A press on an entry leaves the keyboard in the field. -->
    <div
      class="results"
      role="presentation"
      tabindex="-1"
      bind:this={list}
      onscroll={onScroll}
      onmousedown={(event) => event.preventDefault()}
    >
      {#if message !== ''}
        <p class="weak message">{message}</p>
      {/if}
      {#if entries.length > 0}
        <span id={RECENT_ID} class="visually-hidden">Recently used</span>
        <ul id={LIST_ID} class="entries" role="listbox" aria-label={label}>
          {#each entries as entry, at (at)}
            {#if recentCount > 0 && (at === 0 || at === recentCount) && mode === 'commands'}
              <!-- UC-023 BR-004 (app-tester): the recently run commands, then all; each
                   recent entry says so to screen readers itself. -->
              <li class="group" role="presentation" aria-hidden="true">
                {at === 0 ? 'Recently used' : 'All commands'}
              </li>
            {/if}
            <!-- The keyboard stays in the field; the entries are for the pointer. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              id={entryId(at)}
              class="entry {entry.kind}"
              class:unavailable={entry.kind === 'command' && entry.unavailable !== null}
              role="option"
              aria-selected={at === palette.cursor}
              aria-disabled={entry.kind === 'command' && entry.unavailable !== null
                ? true
                : undefined}
              aria-label={entryLabel(entry)}
              aria-describedby={entry.kind === 'command' && entry.unavailable !== null
                ? `${entryId(at)}-why`
                : at < recentCount
                  ? RECENT_ID
                  : undefined}
              onclick={() => palette.choose(at)}
              onpointermove={() => palette.point(at)}
            >
              {#if entry.kind === 'note'}
                <span class="name">{entry.title}</span>
                {#if entry.folder !== null}
                  <span class="weak where">{entry.folder}</span>
                {/if}
              {:else if entry.kind === 'command'}
                <span class="name">{entry.command.title}</span>
                <span class="menu">{entry.command.menu}</span>
                {#if entry.unavailable !== null}
                  <span class="weak off">not available</span>
                {/if}
                {#if entry.command.key}
                  <kbd class="key">{keyText(entry.command.key, mac)}</kbd>
                  {#if entry.command.keyPlace}
                    <span class="weak place">{KEY_PLACE_TEXT[entry.command.keyPlace]}</span>
                  {/if}
                {/if}
                {#if entry.unavailable !== null}
                  <!-- A2: why, shown on the chosen row, told for every row. -->
                  <span
                    id="{entryId(at)}-why"
                    class={at === palette.cursor ? 'reason' : 'visually-hidden'}
                    >{entry.unavailable}</span
                  >
                {/if}
              {:else if entry.kind === 'choice'}
                <span class="radio" class:current={entry.choice.current} aria-hidden="true"></span>
                <span class="name">{entry.choice.label}</span>
                {#if entry.choice.current}
                  <span class="weak where">current</span>
                {/if}
              {:else}
                <span class="name">New note “{entry.title}”…</span>
              {/if}
            </li>
          {/each}
        </ul>
        {#if mode === 'notes' && palette.list.total > notesShown}
          <p class="weak paging">
            Showing {notesShown} of {palette.list.total} notes. Scroll for more.
          </p>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  /* Where the search overlay stands, so the two read as one family. */
  .palette {
    position: fixed;
    top: 44px;
    left: 50%;
    z-index: 26;
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

  .entries {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* UC-023 BR-004: "Recently used" and "All commands" above their entries. */
  .group {
    padding: var(--space-2) var(--space-2) var(--space-1);
    font-size: 0.85em;
    font-weight: 600;
    color: color-mix(in srgb, var(--ui-hint) 35%, var(--ui-text));
  }

  .group:not(:first-child) {
    margin-top: var(--space-1);
    border-top: 1px solid var(--ui-rule);
  }

  /* One row: the name, then where it belongs, the key at the right edge. */
  .entry {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
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

  .entry[aria-selected='true'] .weak {
    color: inherit;
  }

  .name {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .note .name {
    font-weight: 600;
  }

  .where {
    font-size: 0.85em;
  }

  /* The menu a command belongs to: in the text colour, set off by a rule. */
  .menu {
    padding-left: var(--space-2);
    font-size: 0.85em;
    border-left: 1px solid var(--ui-rule);
    overflow-wrap: anywhere;
  }

  .place {
    font-size: 0.85em;
  }

  /* A2: the reason under the chosen row, on a line of its own. */
  .reason {
    flex-basis: 100%;
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 0 var(--space-2);
  }

  .mode {
    font-size: 0.85em;
    font-weight: 600;
  }

  .side-hint {
    font-size: 0.85em;
    color: var(--ui-hint);
  }

  /* A2: not available now, said in words, not only by the dimming. */
  .unavailable .name {
    opacity: 0.7;
  }

  .off {
    font-size: 0.85em;
    font-style: italic;
  }

  .key {
    flex: none;
    margin-left: auto;
    padding: 0 var(--space-1);
    font: inherit;
    font-size: 0.85em;
    white-space: nowrap;
    border: 1px solid currentColor;
    border-radius: var(--radius);
    opacity: 0.8;
  }

  .radio {
    flex: none;
    align-self: center;
    box-sizing: border-box;
    width: 14px;
    height: 14px;
    border: 1.5px solid currentColor;
    border-radius: 50%;
  }

  .radio.current {
    background: radial-gradient(circle, currentColor 0 3px, transparent 3.5px);
  }

  .weak {
    margin: 0;
    color: var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .message {
    padding: 0 var(--space-2) var(--space-1);
  }

  .notice {
    margin: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .notice::before {
    content: '⚠ ';
  }

  .paging {
    margin-top: var(--space-2);
    padding: 0 var(--space-2);
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
