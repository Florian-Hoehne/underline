<!--
  M-16 (FR-019, UC-004 step 4): the tool panel on the right of the note.

  M-16.1: its width comes from the settings when it first shows and is
  dragged at its left edge with the mouse only (Q-06), from 100 to 1000
  pixels, leaving the note at least 200 pixels; let go
  more than a pixel away from the stored width with a note tab active, the
  width is stored at once (UC-010 A4). Reading a settings file again sets it
  back (Q-03).

  M-16.2: "Properties", open at first: Place, Format, Size and Last change
  of the active note, or "Not saved yet" when its file cannot be read. The
  backend reads them when the note is stored, changed outside or shown,
  not for every look. Package 5 (FR-090, UC-004 BR-020): after them the
  text statistics of the note, Words, Characters and Reading time, and
  Selected words and Selected characters while text is selected, counted by the note's editor core in
  the webview and asked for every 400 ms, so they follow every change
  within a second (editor/statistics.ts); also before the note was stored.

  Package 6a (FR-091, FR-093, UC-024, UC-004 BR-016, A15): below the text
  statistics, "Tags (N)": the tags of the note by name, in lower case, those
  of its attribute `tags` first, "None" without; choosing one searches the
  workspace for it (UC-008 A8). Below them the note attributes
  (NoteAttributes.svelte). Both are read by the note's editor core in the
  webview, asked for every 400 ms and at once after a change
  (editor/attributes.ts), so they follow every change, also one not stored
  yet. While another note's are read, the last ones stay, dimmed and not
  usable, so nothing flashes.

  M-16.5 (FR-041 to FR-043): "Outline (N)", "Code (N)" and "Figures (N)",
  open at first, each "None" when empty, made by the backend from the
  note's blocks for one revision (event `indexes`); until the first lists
  of the note come, the three are not shown, as in egui before it made
  them. A heading is indented 14 px per level below the note's highest
  level, at most 4 levels deep, and screen readers hear its level; a long
  entry wraps. A click shows the place: the facade scrolls its block to the top
  and leaves the cursor (M-09.16), the source view puts the cursor there
  and takes the keyboard.

  FR-078, FR-079 (UC-004 step 4, BR-015, A14): "Backlinks (N)" and
  "Outgoing links (N)" follow the figures, open at first, made by the
  backend from every note of the workspace (event `links`), the notes open
  in a tab with their text not stored yet; "Looking for links…" until the
  first lists of the note come, "None" when empty. A backlink shows the
  note that holds it (title and folder), then each link with its line
  number and its line, the link marked; choosing it opens that note with
  the cursor on the link. An outgoing link shows its text and where it
  leads; a broken one is marked "broken" in words, not by colour alone, and underlined as an
  error.
  Choosing it puts the cursor on it in the note; with Ctrl (Cmd on macOS)
  held it is followed as a Ctrl+click in the text follows it.

  M-16.4 (feature F12): in a git workspace the commit box, "Changes (N)"
  and "Diff" (GitChanges.svelte) follow the properties, before the
  outline, as in egui. Without an active note tab the panel shows only
  them and the todos (egui shows them without an open note too).

  M-16.3 (feature F15): the section "Todos" (TodoSection.svelte) between
  the properties and the git changes, as in egui, with the todos in
  conflict after a sync at its top (M-19.10, Q-100). It shows with a note
  tab active, without one, and beside the tab "Todos" (A-176); "Link
  {title}" links only the note of an active note tab.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type {
    IndexEntryView,
    NoteHeadView,
    OutgoingLinkView,
    TextStatisticsView,
  } from './bindings';
  import { HEAD_EVERY_MS, headOf, sameHead } from './editor/attributes';
  import NoteAttributes from './NoteAttributes.svelte';
  import { jumpTo } from './editor/jump';
  import { activeSourceView } from './editor/active';
  import { activeFacade } from './facade/active';
  import {
    sameStatistics,
    statisticsOf,
    statisticsTexts,
    STATISTICS_EVERY_MS,
  } from './editor/statistics';
  import GitChanges from './GitChanges.svelte';
  import TodoSection from './TodoSection.svelte';
  import { toolPanelWidthWithin, type Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  let panel = $state<HTMLElement | null>(null);
  /** Where a drag of the edge started: pointer and width. */
  let drag: { x: number; width: number } | null = null;

  const width = $derived(
    shell.toolPanelWidth ?? toolPanelWidthWithin(shell.settings?.toolPanelWidth ?? 240),
  );
  /**
   * A note tab is active; without one only the git changes show. OQ-279:
   * the tab "Todos" is the left pane's; with the right pane active, its
   * note's sections show.
   */
  const noteShown = $derived(shell.tabs.active !== null && !shell.todoListShown);
  const properties = $derived(shell.properties?.properties ?? null);

  /** UC-004 BR-020: the text statistics of the active note; `null` until its core counted them. */
  let statistics = $state.raw<TextStatisticsView | null>(null);
  const statisticsShown = $derived(statistics === null ? null : statisticsTexts(statistics));

  // BR-020: asked for while a note shows; the core answers an unchanged
  // text and selection from what it kept.
  $effect(() => {
    const note = noteShown ? shell.tabs.active : null;
    return untrack(() => {
      statistics = null;
      if (note === null) {
        return undefined;
      }
      const ask = () => {
        const now = statisticsOf(note);
        if (!sameStatistics(now, statistics)) {
          statistics = now;
        }
      };
      ask();
      const timer = setInterval(ask, STATISTICS_EVERY_MS);
      return () => clearInterval(timer);
    });
  });
  /**
   * UC-024 step 2, FR-091: the attributes and tags last read, and the note
   * they are of; `null` until a core read them.
   */
  let read = $state.raw<{ note: string; head: NoteHeadView } | null>(null);
  const headNote = $derived(noteShown ? shell.tabs.active : null);
  const head = $derived(read?.head ?? null);
  /** The note `read` is of, changing only with the note (not with each read). */
  const readNote = $derived(read?.note ?? null);
  /** The attributes shown are another note's, while the active note's are read. */
  const stale = $derived(read !== null && read.note !== headNote);
  /** Asks the active note's core again (`refreshHead`). */
  let askHead: () => void = () => {};

  // UC-024 A7: asked for while a note shows; the core answers an unchanged
  // text from what it kept.
  $effect(() => {
    const note = headNote;
    return untrack(() => {
      if (note === null) {
        read = null;
        askHead = () => {};
        return undefined;
      }
      const ask = () => {
        const now = headOf(note);
        if (now !== null && (read?.note !== note || !sameHead(now, read.head))) {
          read = { note, head: now };
        }
      };
      askHead = ask;
      ask();
      const timer = setInterval(ask, HEAD_EVERY_MS);
      return () => clearInterval(timer);
    });
  });

  /** M-16.5: the lists of the active note, once the backend made them. */
  const lists = $derived(
    shell.indexes !== null && shell.indexes.note === shell.tabs.active ? shell.indexes : null,
  );
  const sections = $derived<{ title: string; entries: IndexEntryView[] }[]>(
    lists
      ? [
          { title: 'Outline', entries: lists.outline },
          { title: 'Code', entries: lists.code },
          { title: 'Figures', entries: lists.figures },
        ]
      : [],
  );

  /** FR-078, FR-079: the links of the active note, once the backend found them. */
  const links = $derived(
    shell.links !== null && shell.links.note === shell.tabs.active ? shell.links : null,
  );
  const mac = $derived(shell.platform === 'macos');

  /** A14 step 2: the cursor to the link; with Ctrl (Cmd) held, the link is followed. */
  function outgoing(event: MouseEvent, link: OutgoingLinkView) {
    if (!links) {
      return;
    }
    if (mac ? event.metaKey : event.ctrlKey) {
      void shell.followOutgoing(links.note, link.offset, links.rev);
      return;
    }
    jumpTo(links.note, links.rev, link.offset);
  }

  /** UC-004 A14, Q-64: Ctrl+Enter (Cmd+Enter) on a link of the list follows it. */
  function outgoingKey(event: KeyboardEvent, link: OutgoingLinkView) {
    const command = mac ? event.metaKey : event.ctrlKey;
    if (event.key !== 'Enter' || !command || event.shiftKey || event.altKey || !links) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    void shell.followOutgoing(links.note, link.offset, links.rev);
  }

  /** What a screen reader hears of an outgoing link. */
  function outgoingLabel(link: OutgoingLinkView): string {
    const kind = { note: 'note', file: 'file', web: 'web address' }[link.kind];
    const broken = link.broken ? ', broken link' : '';
    return `${link.text}, ${kind} ${link.target}${broken}, line ${link.line}`;
  }

  /** Headings deeper than this many levels below the highest are indented no further (ux review F7). */
  const DEEPEST = 4;

  /** M-16.5: 14 px per level below the note's highest heading, as egui, at most `DEEPEST` levels. */
  function indent(depth: number): number {
    return Math.min(depth, DEEPEST) * 14;
  }

  /**
   * M-16.5, UC-004 step 5: the place of an entry: the facade scrolls its
   * block to the top, the source view jumps there and takes the keyboard.
   */
  function show(note: string, offset: number) {
    const facade = activeFacade();
    if (facade?.note === note) {
      facade.reveal(offset);
      return;
    }
    const source = activeSourceView();
    if (source?.note === note) {
      source.jump(offset);
    }
  }

  onMount(() => shell.pinToolPanelWidth());

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
      // The edge is on the left: moving it left widens the panel.
      shell.toolPanelWidth = toolPanelWidthWithin(drag.width - (event.clientX - drag.x));
    }
  }

  function endDrag() {
    if (!drag) {
      return;
    }
    drag = null;
    const shown = panel?.getBoundingClientRect().width ?? width;
    void shell.toolPanelReleased(shown);
  }
</script>

<!-- UC-004 BR-020: the text statistics, as rows of the properties. -->
{#snippet textStatistics()}
  {#if statisticsShown}
    <dt>Words</dt>
    <dd>{statisticsShown.words}</dd>
    <dt>Characters</dt>
    <dd>{statisticsShown.characters}</dd>
    <dt>Reading time</dt>
    <dd>{statisticsShown.reading}</dd>
    {#if statisticsShown.selection !== null}
      <dt>Selected words</dt>
      <dd>{statisticsShown.selection.words}</dd>
      <dt>Selected characters</dt>
      <dd>{statisticsShown.selection.characters}</dd>
    {/if}
  {/if}
{/snippet}

<aside class="tool-panel" style:width="{width}px" aria-label="Tools" bind:this={panel}>
  <!-- Q-06: the edge is dragged with the mouse only, so it is no control
       for screen readers. -->
  <div
    class="edge"
    role="presentation"
    onpointerdown={startDrag}
    onpointermove={moveDrag}
    onpointerup={endDrag}
    onpointercancel={endDrag}
  ></div>
  <div class="sections">
    {#if noteShown}
      <details open>
        <summary>Properties</summary>
        {#if properties}
          <dl class="properties">
            <dt>Place</dt>
            <dd>{properties.place}</dd>
            <dt>Format</dt>
            <dd>{properties.format}</dd>
            <dt>Size</dt>
            <dd>{properties.size}</dd>
            <dt>Last change</dt>
            <dd>{properties.lastChange}</dd>
            {@render textStatistics()}
          </dl>
        {:else}
          <p class="weak">Not saved yet</p>
          {#if statisticsShown}
            <dl class="properties">{@render textStatistics()}</dl>
          {/if}
        {/if}
        <!-- FR-091, UC-004 A15: the tags of the note; choosing one searches for it. -->
        <section class="tags" class:stale aria-labelledby="tags-title" inert={stale}>
          <h3 id="tags-title">Tags{head ? ` (${head.tags.length})` : ''}</h3>
          {#if head === null}
            <p class="weak" aria-live="polite">Reading the tags…</p>
          {:else if head.tags.length === 0}
            <p class="weak">None</p>
          {:else}
            <ul class="tag-list" aria-label="Tags">
              {#each head.tags as tag (tag)}
                <li>
                  <button
                    type="button"
                    class="tag"
                    aria-label="#{tag}, search notes tagged {tag}"
                    title="Search notes tagged {tag}. The command palette has “Search by tag…” too."
                    onclick={() => shell.searchTag(tag)}
                    ><span class="hash" aria-hidden="true">#</span>{tag}</button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </section>
        {#if head && readNote}
          <div class:stale inert={stale}>
            <NoteAttributes {shell} note={readNote} {head} onChanged={() => askHead()} />
          </div>
        {/if}
      </details>
    {/if}
    <TodoSection {shell} />
    <GitChanges {shell} onShow={show} />
    {#if lists && noteShown}
      {#each sections as section (section.title)}
        <details open>
          <summary>{section.title} ({section.entries.length})</summary>
          {#if section.entries.length === 0}
            <p class="weak">None</p>
          {:else}
            <ul class="entries" aria-label={section.title}>
              {#each section.entries as entry (`${entry.offset}:${entry.label}`)}
                <li>
                  <button
                    type="button"
                    class="entry"
                    style:padding-left="calc(var(--space-1) + {indent(entry.depth)}px)"
                    aria-label={section.title === 'Outline'
                      ? `${entry.label}, level ${entry.depth + 1}`
                      : undefined}
                    onclick={() => show(lists.note, entry.offset)}>{entry.label}</button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </details>
      {/each}
    {/if}
    {#if noteShown}
      <details open class="links">
        <summary>Backlinks{links ? ` (${links.backlinkCount})` : ''}</summary>
        {#if !links}
          <p class="weak" aria-live="polite">Looking for links…</p>
        {:else if links.backlinks.length === 0}
          <p class="weak">None</p>
        {:else}
          <ul class="entries" aria-label="Backlinks">
            {#each links.backlinks as source (source.note)}
              <li class="source">
                <p class="source-name">
                  <span class="title">{source.title}</span>
                  {#if source.folder}
                    <span class="weak folder">{source.folder}</span>
                  {/if}
                </p>
                <ul class="entries" aria-label="Links in {source.title}">
                  {#each source.links as link (`${link.place.from}`)}
                    <li>
                      <button
                        type="button"
                        class="entry backlink"
                        aria-label="{link.line} {link.before}{link.link}{link.after}, in {source.title}"
                        onclick={() => void shell.openBacklink(source.note, link.place)}
                      >
                        <span class="line" aria-hidden="true">{link.line}</span>
                        <span class="context" aria-hidden="true"
                          >{link.before}<mark>{link.link}</mark>{link.after}</span
                        >
                      </button>
                    </li>
                  {/each}
                </ul>
              </li>
            {/each}
          </ul>
        {/if}
      </details>
      <details open class="links">
        <summary>Outgoing links{links ? ` (${links.outgoing.length})` : ''}</summary>
        {#if !links}
          <p class="weak" aria-live="polite">Looking for links…</p>
        {:else if links.outgoing.length === 0}
          <p class="weak">None</p>
        {:else}
          <p class="weak hint" id="outgoing-hint">
            {mac ? 'Cmd' : 'Ctrl'}+click or {mac ? 'Cmd' : 'Ctrl'}+Enter follows a link.
          </p>
          <ul class="entries" aria-label="Outgoing links">
            {#each links.outgoing as link, at (`${at}:${link.offset}`)}
              <li>
                <button
                  type="button"
                  class="entry outgoing"
                  class:broken={link.broken}
                  title="{mac ? 'Cmd' : 'Ctrl'}+click to follow"
                  aria-label={outgoingLabel(link)}
                  aria-describedby="outgoing-hint"
                  aria-keyshortcuts={mac ? 'Meta+Enter' : 'Control+Enter'}
                  onclick={(event) => outgoing(event, link)}
                  onkeydown={(event) => outgoingKey(event, link)}
                >
                  <span class="text" aria-hidden="true">{link.text}</span>
                  <span class="target weak" aria-hidden="true">{link.target}</span>
                  {#if link.broken}
                    <span class="mark" aria-hidden="true">broken</span>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </details>
    {/if}
  </div>
</aside>

<style>
  .tool-panel {
    position: relative;
    flex: none;
    box-sizing: border-box;
    /* A-13: the panel leaves the note at least 200 px of the room beside
       the overview. */
    max-width: calc(100% - 200px);
    min-height: 0;
    border-left: 1px solid var(--ui-rule);
  }

  .sections {
    height: 100%;
    overflow: auto;
    padding: var(--space-2) var(--space-3);
    box-sizing: border-box;
  }

  summary {
    min-height: 24px;
    overflow: hidden;
    font-weight: 700;
    white-space: nowrap;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .properties {
    display: grid;
    grid-template-columns: fit-content(45%) minmax(0, 1fr);
    gap: var(--space-1) var(--space-3);
    margin: var(--space-2) 0 0;
  }

  dt,
  .weak {
    color: var(--ui-hint);
  }

  dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: break-word;
  }

  .weak {
    margin: var(--space-2) 0 0;
  }

  details + details {
    margin-top: var(--space-2);
  }

  .entries {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
  }

  /* egui's selectable labels: plain text, tinted under the pointer. A long
     label wraps, so it can be read without a pointer (ux review F7). */
  .entry {
    display: block;
    box-sizing: border-box;
    width: 100%;
    min-height: 24px;
    padding: 2px var(--space-1);
    font: inherit;
    color: var(--ui-text);
    text-align: left;
    overflow-wrap: anywhere;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  .entry:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  /* Long places without breaks wrap; nothing overflows the panel. */
  .links {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .links .entry {
    max-width: 100%;
  }

  .hint {
    font-size: 0.85em;
  }

  /* FR-078: a note that links here, then its links, set in by a rule. */
  .source + .source {
    margin-top: var(--space-2);
  }

  .source-name {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-2);
    margin: 0;
    padding: 2px var(--space-1);
  }

  .title {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .folder {
    margin: 0;
    font-size: 0.85em;
  }

  .source .entries {
    margin: 0 0 0 var(--space-1);
    padding-left: var(--space-1);
    border-left: 1px solid var(--ui-rule);
  }

  .backlink {
    display: flex;
    gap: var(--space-2);
    align-items: baseline;
  }

  /* The line number, in step with the editor's numbers. */
  .line {
    flex: none;
    min-width: 2ch;
    font-size: 0.85em;
    color: var(--ui-hint);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .context {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  /* The link in its line: marked by weight and an underline, not only a tint. */
  .context mark {
    font-weight: 600;
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    background: color-mix(in srgb, var(--ui-chosen) 25%, transparent);
    border-radius: 2px;
  }

  .outgoing {
    display: flex;
    flex-wrap: wrap;
    gap: 0 var(--space-2);
    align-items: baseline;
  }

  .outgoing .text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .outgoing .target {
    margin: 0;
    font-size: 0.85em;
    overflow-wrap: anywhere;
  }

  /* BR-015: a broken link, said in words and underlined as an error. */
  .outgoing.broken .text {
    text-decoration: underline wavy;
    text-decoration-thickness: 1px;
    text-underline-offset: 3px;
  }

  .mark {
    padding: 0 var(--space-1);
    font-size: 0.8em;
    line-height: 1.4;
    border: 1px solid currentColor;
    border-radius: var(--radius);
  }

  .tags {
    margin-top: var(--space-3);
    padding-top: var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }

  .tags h3 {
    margin: 0;
    font-size: 1em;
    font-weight: 700;
  }

  /* Another note's tags and attributes, until the active note's are read. */
  .stale {
    opacity: 0.5;
  }

  /* FR-091: the tags as the markup writes them, `#` set back. */
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
  }

  .tag {
    min-height: 24px;
    max-width: 100%;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    overflow-wrap: anywhere;
    cursor: pointer;
    background: none;
    border: 1px solid var(--ui-border);
    border-radius: 12px;
  }

  .tag:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .tag .hash {
    margin-right: 1px;
    font-family: var(--font-mono);
    color: var(--ui-hint);
  }

  .tag:hover .hash {
    color: inherit;
  }

  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    width: 6px;
    cursor: col-resize;
    touch-action: none;
  }
</style>
