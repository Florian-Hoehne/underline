<!--
  M-05.1, M-05.2 (UC-002 step 7, FR-017): the note overview on the left.

  Its width comes from the settings when it first shows and is dragged at
  its right edge with the mouse (Q-06), from 100 to 1000 pixels; let go more than a pixel away
  from the stored width, the width is stored at once (UC-010 A4). The
  notes are listed recently opened first; a click opens a note, which then
  moves to the top. The note of the active tab is marked.

  Feature F8 adds, as in egui:
  - "New note…" opens the dialog "New note" (M-14.1);
  - M-05.4: the context menu of a note, "Rename…" and "Delete…" (the
    submenu "Export" comes with feature F11, paritaet.md A-79); it also
    opens with Shift+F10 or the context menu key (A-80);
  - M-05.5: the Delete key deletes the note last clicked while the
    keyboard is on it (UC-007 BR-007); a click or the keyboard elsewhere
    ends that. No arrow keys, as in egui;
  - M-05.6: "Deleted notes (N)" at the end of the list, folded at first;
    each with its title (resting the pointer shows "{folder} · deleted
    {time}"), when it was deleted, "Restore" and "Remove", and "Empty the
    deleted notes".

  Later features add the git letters (M-05.3, F12).
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import ContextMenu, { asksForMenu, menuPlace, type MenuEntry } from './ContextMenu.svelte';
  import { setFocusHint } from './focus';
  import { overviewWidthWithin, type Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  let panel = $state<HTMLElement | null>(null);
  /** Where a drag of the edge started: pointer and width. */
  let drag: { x: number; width: number } | null = null;

  const width = $derived(
    shell.overviewWidth ?? overviewWidthWithin(shell.settings?.overviewWidth ?? 200),
  );
  const active = $derived(shell.tabs.active);

  onMount(() => shell.pinOverviewWidth());

  /**
   * The note last clicked. Moving its entry to the top takes the keyboard
   * from it in some webviews; it gets it back, as egui keeps the keyboard
   * in the overview after a click.
   */
  let chosen: string | null = null;

  $effect(() => {
    void shell.overview;
    if (chosen === null) {
      return;
    }
    const note = chosen;
    void tick().then(() => {
      if (document.activeElement === document.body) {
        panel?.querySelector<HTMLElement>(`button.entry[data-note="${CSS.escape(note)}"]`)?.focus();
      }
    });
  });

  function open(event: MouseEvent, note: string) {
    chosen = note;
    // The keyboard stays on the note, so Delete deletes it (M-05.5).
    (event.currentTarget as HTMLElement).focus();
    void shell.openNote(note);
  }

  /** M-05.4: the context menu of a note, with the element it opened on. */
  let menu = $state<{ note: string; label: string; x: number; y: number } | null>(null);
  let menuOpener: HTMLElement | null = null;

  function openMenu(event: MouseEvent | KeyboardEvent, note: string, label: string) {
    event.preventDefault();
    menuOpener = event.currentTarget as HTMLElement;
    menu = { note, label, ...menuPlace(event) };
  }

  /** The entries of the menu of `note`; they act on it after the menu closed. */
  function entriesFor(note: string): MenuEntry[] {
    return [
      { label: 'Rename…', action: () => void shell.startRename(note) },
      { label: 'Delete…', shortcut: 'Delete', action: () => void shell.startDelete(note) },
    ];
  }

  const menuEntries = $derived(menu ? entriesFor(menu.note) : []);

  function closeMenu(back: boolean) {
    menu = null;
    if (back) {
      menuOpener?.focus();
    }
  }

  /**
   * M-05.5, UC-007 BR-007: Delete deletes the note whose entry has the
   * keyboard; Shift+F10 or the context menu key opens its menu.
   */
  function onEntryKey(event: KeyboardEvent, note: string, label: string) {
    if (asksForMenu(event)) {
      openMenu(event, note, label);
    } else if (
      event.key === 'Delete' &&
      shell.notes.rename === null &&
      !event.shiftKey &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey
    ) {
      event.preventDefault();
      void shell.startDelete(note);
    }
  }

  /** M-05.6: "Deleted notes (N)" is folded when the overview first shows. */
  let deletedOpen = $state(false);

  /**
   * UX review F8: once the deleted note `gone` left the list, the keyboard
   * goes to the next one's "Restore" (the one before for the last), else to
   * "Deleted notes (N)"; also when a dialog asked first and closes.
   */
  let pendingFocus: { gone: string; target: () => HTMLElement | null | undefined } | null = null;

  function focusAfterLeaving(key: string) {
    const at = shell.deleted.findIndex((note) => note.key === key);
    const next = shell.deleted[at + 1] ?? shell.deleted[at - 1] ?? null;
    const target = () =>
      next
        ? panel?.querySelector<HTMLElement>(`button[data-restore="${CSS.escape(next.key)}"]`)
        : panel?.querySelector<HTMLElement>('button.fold');
    pendingFocus = { gone: key, target };
    setFocusHint(target);
  }

  function restore(key: string) {
    focusAfterLeaving(key);
    void shell.restoreDeleted(key);
  }

  function remove(key: string) {
    focusAfterLeaving(key);
    shell.askPurge(key);
  }

  $effect(() => {
    const deleted = shell.deleted;
    const pending = pendingFocus;
    if (pending === null || deleted.some((note) => note.key === pending.gone)) {
      return;
    }
    pendingFocus = null;
    void tick().then(() => {
      // A dialog that has the keyboard gives it back itself (setFocusHint).
      if (document.activeElement === document.body) {
        setFocusHint(null);
        pending.target()?.focus();
      }
    });
  });

  function startDrag(event: PointerEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { x: event.clientX, width: panel?.getBoundingClientRect().width ?? width };
  }

  function moveDrag(event: PointerEvent) {
    if (drag) {
      shell.overviewWidth = overviewWidthWithin(drag.width + event.clientX - drag.x);
    }
  }

  function endDrag() {
    if (!drag) {
      return;
    }
    drag = null;
    const shown = panel?.getBoundingClientRect().width ?? width;
    void shell.overviewReleased(shown);
  }
</script>

<aside
  class="overview"
  style:width="{width}px"
  aria-labelledby="overview-heading"
  tabindex="-1"
  data-focus-fallback
  bind:this={panel}
>
  <div class="head">
    <h2 id="overview-heading">Notes</h2>
    <button type="button" class="ui-button" onclick={() => void shell.startNewNote()}
      >New note…</button
    >
  </div>
  <div class="scroll">
    <ul class="notes">
      {#each shell.overview as item (item.note)}
        <li>
          <button
            type="button"
            class="entry"
            data-note={item.note}
            aria-current={item.note === active ? 'true' : undefined}
            aria-haspopup="menu"
            aria-keyshortcuts="Delete Shift+F10"
            onclick={(event) => open(event, item.note)}
            oncontextmenu={(event) => openMenu(event, item.note, item.label)}
            onkeydown={(event) => onEntryKey(event, item.note, item.label)}>{item.label}</button
          >
        </li>
      {/each}
    </ul>
    {#if shell.deleted.length > 0}
      <section class="deleted" aria-labelledby="deleted-heading">
        <h3 id="deleted-heading">
          <button
            type="button"
            class="fold"
            aria-expanded={deletedOpen}
            aria-controls="deleted-notes"
            onclick={() => (deletedOpen = !deletedOpen)}
          >
            <span class="arrow" aria-hidden="true">{deletedOpen ? '⏷' : '⏵'}</span>
            Deleted notes ({shell.deleted.length})
          </button>
        </h3>
        {#if deletedOpen}
          <div id="deleted-notes">
            <ul class="deleted-list">
              {#each shell.deleted as note (note.key)}
                <li class="deleted-note">
                  <span class="deleted-title" title={note.hover}>{note.title}</span>
                  <span class="deleted-row">
                    <span class="deleted-time">{note.folder} · {note.deleted}</span>
                    <button
                      type="button"
                      class="ui-button small"
                      data-restore={note.key}
                      aria-label="Restore {note.title} ({note.folder})"
                      onclick={() => restore(note.key)}>Restore</button
                    >
                    <button
                      type="button"
                      class="ui-button small"
                      aria-label="Remove {note.title} ({note.folder})"
                      onclick={() => remove(note.key)}>Remove</button
                    >
                  </span>
                </li>
              {/each}
            </ul>
            <button type="button" class="ui-button" onclick={() => shell.askPurge(null)}
              >Empty the deleted notes</button
            >
          </div>
        {/if}
      </section>
    {/if}
  </div>
  {#if menu}
    <ContextMenu
      label="Note {menu?.label ?? ''}"
      x={menu?.x ?? 0}
      y={menu?.y ?? 0}
      onClose={closeMenu}
      entries={menuEntries}
    />
  {/if}
  <div
    class="edge"
    role="separator"
    aria-orientation="vertical"
    onpointerdown={startDrag}
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
  ></div>
</aside>

<style>
  .overview {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    box-sizing: border-box;
    max-width: calc(100% - 120px);
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--ui-rule);
  }

  .overview:focus {
    outline: none;
  }

  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    line-height: 1.3;
  }

  /* The notes and, after them, the deleted notes scroll together, as in egui. */
  .scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  .notes {
    margin: 0;
    padding: var(--space-1);
    list-style: none;
  }

  .deleted {
    margin: 0 var(--space-1) var(--space-2);
    padding-top: var(--space-1);
    border-top: 1px solid var(--ui-rule);
  }

  h3 {
    margin: 0;
    font: inherit;
  }

  /* egui's collapsing header: an arrow and the text. */
  .fold {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    box-sizing: border-box;
    width: 100%;
    min-height: 24px;
    padding: 2px var(--space-1);
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .fold:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .fold:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .arrow {
    flex: none;
    width: 1em;
    text-align: center;
  }

  #deleted-notes {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-1) 0 0 var(--space-4);
  }

  .deleted-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .deleted-note {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .deleted-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .deleted-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }

  /* egui's weak text. */
  .deleted-time {
    color: var(--ui-hint);
  }

  /* egui's small buttons, still 24 px high for the pointer (WCAG 2.5.8). */
  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  .entry {
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
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  /* The active note: the chosen colour and, not by colour alone, a bar on
     the left (NFR-010, paritaet.md A-15). Pointing is weaker. */
  .entry[aria-current='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    box-shadow: inset 3px 0 0 var(--ui-text);
  }

  .entry:hover:not([aria-current='true']) {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  /* The edge to drag, a little wider than the line it sits on. */
  .edge {
    position: absolute;
    top: 0;
    right: -4px;
    bottom: 0;
    z-index: 1;
    width: 8px;
    cursor: col-resize;
    touch-action: none;
  }

  .edge:hover {
    background: linear-gradient(
      to right,
      transparent 3px,
      var(--ui-border) 3px 5px,
      transparent 5px
    );
  }
</style>
