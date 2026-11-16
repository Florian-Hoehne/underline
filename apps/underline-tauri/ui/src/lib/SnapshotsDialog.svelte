<!--
  UC-026 Restore Snapshot (FR-101, FR-102): "Snapshots of {title}".

  - Steps 2–3: on the left the snapshots of the note, the newest first, on
    a time line: the day and time each was taken, its cause (BR-001) and
    how many lines it differs from the note as it is now. The newest is
    chosen when the dialog opens.
  - Step 4: on the right the differences of the snapshot chosen with the
    note: lines only in the snapshot marked as taken away, lines only in
    the note as added, as in the diff of the workspace (UC-011); A2:
    "Whole snapshot" shows it as the facade does, read only, to select and
    copy from.
  - Steps 5–7: "Restore" gives the note the snapshot's content; the dialog
    closes and the status bar reports it with "Undo" (A3). When it cannot
    be restored the dialog stays and says why.
  - A1: without snapshots the dialog says when they are taken.
  - A4: when the note or its snapshots change, the list and the
    differences follow.
  - A6, BR-006: "Close" or Escape changes nothing. The list takes the
    arrow keys, Home and End; Tab goes on to the switch, the snapshot, and
    "Restore".
-->
<script lang="ts">
  import type { DiffLineKind, SnapshotItemView } from './bindings';
  import DialogWindow from './DialogWindow.svelte';
  import DiffHunks from './DiffHunks.svelte';
  import Block from './facade/Block.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const snaps = $derived(shell.snapshots);
  const list = $derived(snaps.list);
  const items = $derived(list?.items ?? []);
  const chosen = $derived(snaps.chosenItem);
  const listId = `snapshots-${Math.random().toString(36).slice(2)}`;

  /** What the lines of the differences are, for screen readers. */
  const WORDS: Record<DiffLineKind, string> = {
    same: 'in both',
    removed: 'a restore brings it back',
    added: 'a restore removes it',
  };

  function differs(item: SnapshotItemView): string {
    if (item.problem !== null) {
      return item.problem;
    }
    if (item.differs === null) {
      return '';
    }
    if (item.differs === 0) {
      return 'Same as the note';
    }
    return item.differs === 1 ? '1 line differs' : `${item.differs} lines differ`;
  }

  /** UX review: why "Restore" cannot be used for the snapshot chosen, said in words. */
  function notRestorable(item: SnapshotItemView | null): string | null {
    if (item === null) {
      return 'Choose a snapshot to restore.';
    }
    if (item.problem !== null) {
      return `This snapshot cannot be restored: ${item.problem.charAt(0).toLowerCase()}${item.problem.slice(1)}.`;
    }
    if (item.differs === 0) {
      return 'The note holds this snapshot already.';
    }
    return null;
  }

  function optionId(index: number): string {
    return `${listId}-${index}`;
  }

  function onListKey(event: KeyboardEvent) {
    const moves: Record<string, number> = {
      ArrowDown: 1,
      ArrowUp: -1,
      PageDown: 10,
      PageUp: -10,
      Home: -items.length,
      End: items.length,
    };
    const by = moves[event.key];
    if (by === undefined) {
      return;
    }
    event.preventDefault();
    void snaps.move(by).then(() => {
      document
        .getElementById(optionId(items.findIndex((item) => item.key === snaps.chosen)))
        ?.scrollIntoView?.({ block: 'nearest' });
    });
  }

  const chosenIndex = $derived(items.findIndex((item) => item.key === snaps.chosen));
  const why = $derived(notRestorable(chosen));
  const restorable = $derived(why === null && !snaps.restoring);
</script>

<DialogWindow
  title={`Snapshots of ${snaps.title}`}
  onClose={() => void snaps.close()}
  busy={snaps.loading || snaps.restoring}
  step={`${snaps.opened}-${list === null ? 'reading' : items.length === 0 ? 'none' : 'list'}`}
  width={880}
  resizable
  closeButton
>
  <div class="snapshots">
    {#if list === null}
      {#if snaps.problem !== null}
        <p class="problem" role="alert">{snaps.problem}</p>
      {:else}
        <p class="weak" role="status">Reading the snapshots…</p>
      {/if}
      <div class="actions">
        <button
          type="button"
          class="ui-button"
          data-autofocus="true"
          onclick={() => void snaps.close()}>Close</button
        >
      </div>
    {:else if items.length === 0}
      <div class="none">
        <p>{list.none?.lead}</p>
        <ul>
          {#each list.none?.when ?? [] as line (line)}
            <li>{line}</li>
          {/each}
        </ul>
        <p class="weak">{list.kept}.</p>
      </div>
      {#if list.problem !== null}
        <p class="problem" role="alert">{list.problem}</p>
      {/if}
      <div class="actions">
        <button
          type="button"
          class="ui-button"
          data-autofocus="true"
          onclick={() => void snaps.close()}>Close</button
        >
      </div>
    {:else}
      <div class="panes">
        <div class="side">
          <ol
            class="timeline"
            role="listbox"
            aria-label="Snapshots, the newest first"
            tabindex="0"
            data-autofocus="true"
            aria-activedescendant={chosenIndex < 0 ? undefined : optionId(chosenIndex)}
            onkeydown={onListKey}
          >
            {#each items as item, index (item.key)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <li
                id={optionId(index)}
                role="option"
                aria-selected={item.key === snaps.chosen}
                class:chosen={item.key === snaps.chosen}
                class:damaged={item.problem !== null}
                onclick={() => void snaps.choose(item.key)}
              >
                <span class="when">{item.taken}</span>
                <span class="why" title={item.cause}>{item.cause}</span>
                <span class="differs">{differs(item)}</span>
              </li>
            {/each}
          </ol>
          <p class="kept weak">{list.kept}</p>
        </div>
        <section class="view" aria-label="The snapshot chosen">
          <div class="switch" role="group" aria-label="Show">
            <button
              type="button"
              class="ui-button"
              aria-pressed={snaps.shown === 'differences'}
              onclick={() => void snaps.showAs('differences')}>Differences</button
            >
            <button
              type="button"
              class="ui-button"
              aria-pressed={snaps.shown === 'whole'}
              onclick={() => void snaps.showAs('whole')}>Whole snapshot</button
            >
          </div>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="shown"
            tabindex="0"
            aria-label={snaps.shown === 'whole'
              ? 'Whole snapshot, read only'
              : 'Differences with the note'}
          >
            {#if snaps.shown === 'differences'}
              {#if snaps.diff === null || snaps.diff.key !== snaps.chosen}
                <p class="weak">Comparing with the note…</p>
              {:else if snaps.diff.body.kind === 'same'}
                <p class="said">The snapshot holds what the note holds now.</p>
              {:else if snaps.diff.body.kind === 'problem'}
                <p class="problem" role="alert">{snaps.diff.body.text}</p>
              {:else}
                <p class="legend">
                  <span
                    ><span class="mark removed" aria-hidden="true">−</span> Restore brings back</span
                  >
                  <span><span class="mark added" aria-hidden="true">+</span> Restore removes</span>
                </p>
                <DiffHunks hunks={snaps.diff.body.hunks} words={WORDS} />
              {/if}
            {:else if snaps.whole === null || snaps.whole.key !== snaps.chosen}
              <p class="weak">Reading the snapshot…</p>
            {:else if snaps.whole.problem !== null}
              <p class="problem" role="alert">{snaps.whole.problem}</p>
            {:else}
              <div class="whole">
                {#each snaps.whole.blocks as block, index (index)}
                  <Block
                    {block}
                    {index}
                    platform={shell.platform}
                    editing={false}
                    section={null}
                    onFold={() => {}}
                    onEnlarge={() => {}}
                    onFigure={() => {}}
                  />
                {/each}
              </div>
            {/if}
          </div>
        </section>
      </div>
      {#if snaps.problem !== null}
        <p class="problem" role="alert">{snaps.problem}</p>
      {/if}
      <div class="footer">
        <p class="hint" id="{listId}-hint">
          {why ??
            'Restoring keeps the current text as a snapshot; you can undo it in the status bar.'}
        </p>
        <div class="actions">
          <button
            type="button"
            class="ui-button main"
            disabled={!restorable}
            aria-describedby="{listId}-hint"
            onclick={() => void snaps.restore()}>Restore</button
          >
          <button type="button" class="ui-button" onclick={() => void snaps.close()}>Close</button>
        </div>
      </div>
    {/if}
  </div>
</DialogWindow>

<style>
  .snapshots {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--space-2);
    min-height: 0;
  }

  .snapshots > .actions {
    flex: none;
  }

  .none {
    max-width: 60ch;
  }

  .none ul {
    margin: 0 0 var(--space-2);
    padding-left: 1.4em;
  }

  .weak,
  .said {
    color: var(--ui-hint);
  }

  /* At least about three snapshots in view, growing with the window; a
     long line never widens the dialog (contained). */
  .panes {
    display: grid;
    flex: 1 1 auto;
    grid-template-columns: minmax(12rem, 18rem) minmax(0, 1fr);
    gap: var(--space-3);
    min-height: 150px;
    height: clamp(150px, 60vh, 680px);
    contain: inline-size;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-height: 0;
  }

  .kept {
    flex: none;
    margin: 0;
    font-size: 12px;
  }

  .footer {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .hint {
    flex: 1 1 24ch;
    margin: 0;
    font-size: 12px;
    color: var(--ui-hint);
  }

  .footer .actions {
    flex: none;
  }

  /* The main action: drawn in the chosen colour, as the active choice. */
  .ui-button.main:not(:disabled) {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    font-weight: 600;
  }

  /* The snapshots on a line of time: a rule down the left, a dot for each
     snapshot, filled for the one chosen. */
  .timeline {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    margin: 0;
    padding: var(--space-1) 0;
    overflow-y: auto;
    list-style: none;
    border: 1px solid var(--ui-rule);
    border-radius: var(--radius);
  }

  .timeline::before {
    content: '';
    position: absolute;
    top: var(--space-2);
    bottom: var(--space-2);
    left: 13px;
    border-left: 1px solid var(--ui-rule);
  }

  .timeline li {
    position: relative;
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: var(--space-2);
    padding: var(--space-1) var(--space-2) var(--space-1) 28px;
    cursor: pointer;
  }

  .timeline li::before {
    content: '';
    position: absolute;
    top: 10px;
    left: 9px;
    box-sizing: border-box;
    width: 9px;
    height: 9px;
    background: var(--ui-surface);
    border: 1px solid var(--ui-text);
    border-radius: 50%;
  }

  .timeline li.chosen {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .timeline li.chosen::before {
    background: var(--ui-chosen-text);
    border-color: var(--ui-chosen-text);
  }

  .timeline li:hover:not(.chosen) {
    background: color-mix(in srgb, var(--ui-chosen) 18%, transparent);
  }

  .timeline:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .when {
    grid-column: 1 / -1;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .why,
  .differs {
    font-size: 12px;
  }

  /* App-tester: the rows stay even; a cause too long for the list ends on
     "…", its whole text at the pointer and for screen readers. */
  .why {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .differs {
    justify-self: end;
    text-align: right;
    white-space: nowrap;
  }

  .timeline li:not(.chosen) .why,
  .timeline li:not(.chosen) .differs {
    color: var(--ui-hint);
  }

  .timeline li.damaged .differs {
    grid-column: 1 / -1;
    justify-self: start;
    text-align: left;
    white-space: normal;
  }

  .view {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
    min-height: 0;
  }

  .switch {
    display: flex;
    flex: none;
    gap: var(--space-1);
  }

  .switch .ui-button[aria-pressed='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .shown {
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--space-2);
    overflow: auto;
    border: 1px solid var(--ui-rule);
    border-radius: var(--radius);
  }

  .shown:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-3);
    margin: 0 0 var(--space-2);
    font-size: 12px;
    color: var(--ui-hint);
  }

  .mark {
    display: inline-block;
    width: 1.4em;
    font-family: var(--font-mono);
    text-align: center;
    color: var(--ui-text);
    border-radius: var(--radius);
  }

  .mark.removed {
    background: rgb(248 81 73 / 16%);
  }

  .mark.added {
    background: rgb(46 160 67 / 16%);
  }

  /* A2: as the facade draws the note, on the theme's page, read only. */
  .whole {
    --facade-weak: color-mix(in srgb, var(--palette-foreground) 55%, transparent);

    padding: var(--space-2) var(--space-3);
    font-family: var(--font-ui);
    font-size: calc(15px * var(--editor-zoom, 1));
    color: var(--palette-foreground);
    background: var(--palette-background, var(--ui-field));
    border-radius: var(--radius);
    user-select: text;
  }

  .whole :global(::selection) {
    background: var(--palette-selection);
  }

  /* A narrow window: the list above the snapshot. */
  @media (width <= 640px) {
    .panes {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(150px, 40%) minmax(120px, 1fr);
      height: auto;
      min-height: 280px;
    }
  }
</style>
