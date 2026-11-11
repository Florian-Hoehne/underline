<!--
  M-16.4, M-19.1 to M-19.3 (UC-011, UC-012): the git changes in the tool
  panel, as egui's `changes_ui` shows them, only in a git workspace and only
  while git answers (A9: the status bar says why, feature F13).

  M-19.1, the commit box above the list (UC-012 steps 2–4):
  - during a merge, instead, "A synchronization left notes in conflict;
    resolve them before committing." (BR-009);
  - the message follows the proposal until the user types (BR-002);
    Ctrl+Enter (Cmd+Enter) in it commits; the field grows with its text
    (app-tester F12, A-136);
  - "The first line is long; many programs show only its start." when its
    first line has more than 72 characters (BR-003);
  - "Amend the last commit" while the last commit may be amended; choosing
    it takes over that commit's message (A6); unchecking it keeps the
    message, as in egui;
  - "Commit"/"Amend", disabled with nothing chosen ("Nothing is chosen."),
    and "All"/"None" (BR-004); a refusal stays below, marked "⚠" (A1, A2).
  After a commit the keyboard goes to "Changes (N)" (UX review F12).

  M-19.2, "Changes (N)", folded at first; Ctrl+Shift+G (Cmd+Shift+G) opens
  it and puts the keyboard into the message (BR-007). Each row: the check
  box of the commit, the letter in its colour (BR-003), "{title} · {folder}"
  or "{new} ← {old}", which opens the note or shows its diff (scrolled into
  view), and "Discard…" for a note. "Other files" divides the notes from
  the rest; "Nothing changed since the last commit." when empty (A1). The
  actions of conflicts come with F13.

  M-19.3, "Diff", open at first: the file, then each changed part after a
  rule; a line is "{old} {new} {±}" and its text, the words changed marked
  more strongly and underlined, with "(only spaces differ)" or "(only the
  line ending differs)". A click on a line of the active note's diff puts
  the cursor there (UC-011 steps 7–8).

  Keyboard (BR-007, A-128): the list and the diff are one stop of Tab each;
  Up, Down, Home and End move within them (roving tabindex).
-->
<script lang="ts">
  import { tick, untrack } from 'svelte';
  import type { ChangeKindName, ChangeView, DiffLineKind, DiffView } from './bindings';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    /** Shows the place `offset` of `note` in its view (as the outline does). */
    onShow: (note: string, offset: number) => void;
  };

  let { shell, onShow }: Props = $props();

  const changes = $derived(shell.changes);
  const box = $derived(shell.commit);
  const diff = $derived<DiffView | null>(shell.diff);
  const command = $derived(shell.platform === 'macos' ? 'Cmd' : 'Ctrl');
  const commandKey = $derived(shell.platform === 'macos' ? 'Meta' : 'Control');

  let listOpen = $state(false);
  let message = $state<HTMLTextAreaElement | null>(null);
  let summary = $state<HTMLElement | null>(null);
  let diffSection = $state<HTMLElement | null>(null);
  /** The row of the list and the line of the diff that are the Tab stop. */
  let rowStop = $state(0);
  let lineStop = $state(0);
  /** A file was chosen in the list: its diff is scrolled into view when it comes. */
  let revealDiff = false;

  /** BR-003: a first line longer than 72 characters. */
  const longFirstLine = $derived([...(box.message.split('\n')[0] ?? '')].length > 72);
  const nothing = $derived(
    changes !== null && changes.files.every((change) => !change.chosen) && !box.amend,
  );
  const boxShown = $derived(
    changes !== null && (changes.files.length > 0 || changes.amendable !== null),
  );
  const rowCount = $derived(changes?.files.length ?? 0);
  const lineCount = $derived(
    diff !== null && diff.body.kind === 'hunks' && diff.note !== null
      ? diff.body.hunks.reduce((sum, hunk) => sum + hunk.lines.length, 0)
      : 0,
  );

  /** What the letter says, for screen readers (new text, A-128). */
  const KIND_WORDS: Record<ChangeKindName, string> = {
    new: 'new',
    changed: 'changed',
    deleted: 'deleted',
    renamed: 'renamed',
    conflict: 'in conflict',
  };

  /** What a diff line is, for screen readers (new text, A-128). */
  const LINE_WORDS: Record<DiffLineKind, string> = {
    same: 'unchanged',
    removed: 'removed',
    added: 'added',
  };

  // The Tab stops stay within the list and the diff when they shrink.
  $effect(() => {
    if (rowStop >= rowCount) {
      rowStop = Math.max(0, rowCount - 1);
    }
  });
  $effect(() => {
    if (lineStop >= lineCount) {
      lineStop = Math.max(0, lineCount - 1);
    }
  });

  // BR-007: Ctrl+Shift+G opens the list and puts the keyboard into the message.
  let asked = untrack(() => shell.changesAsked);
  $effect(() => {
    const now = shell.changesAsked;
    if (now === asked) {
      return;
    }
    asked = now;
    listOpen = true;
    void tick().then(() => (message ?? summary)?.focus());
  });

  // UX review F12: after a commit the keyboard goes to "Changes (N)".
  let committed = untrack(() => shell.committedCount);
  $effect(() => {
    const now = shell.committedCount;
    if (now === committed) {
      return;
    }
    committed = now;
    void tick().then(() => summary?.focus());
  });

  // UX review F12: the diff of a file chosen in the list is scrolled into view.
  $effect(() => {
    void diff;
    if (revealDiff && diff !== null) {
      revealDiff = false;
      void tick().then(() => diffSection?.scrollIntoView?.({ block: 'nearest' }));
    }
  });

  // App-tester F12: the message field grows with its text.
  $effect(() => {
    void box.message;
    const field = message;
    if (field) {
      field.style.height = 'auto';
      field.style.height = `${field.scrollHeight + 2}px`;
    }
  });

  /** Step 4: Ctrl+Enter (Cmd+Enter) in the message commits. */
  function messageKey(event: KeyboardEvent) {
    const mac = shell.platform === 'macos';
    const commandDown = mac ? event.metaKey : event.ctrlKey;
    const other = mac ? event.ctrlKey : event.metaKey;
    if (event.key === 'Enter' && commandDown && !other && !event.altKey && !event.shiftKey) {
      event.preventDefault();
      if (!nothing) {
        void shell.commitChanges();
      }
    }
  }

  /** Whether the row of `change` is the one chosen or whose diff shows. */
  function selected(change: ChangeView): boolean {
    return diff?.path === change.path;
  }

  /** "Other files" goes before the first file that is not a note. */
  function startsOthers(index: number): boolean {
    const files = changes?.files ?? [];
    return !files[index].note && (index === 0 || files[index - 1].note);
  }

  function choose(change: ChangeView, index: number) {
    rowStop = index;
    if (!change.note || change.kind === 'deleted') {
      revealDiff = true;
    }
    void shell.chooseChange(change.path);
  }

  /**
   * BR-007: Up, Down, Home and End move the Tab stop to the row or line
   * before, after, first or last, and the keyboard with it.
   */
  function moveKey(
    event: KeyboardEvent,
    selector: string,
    count: number,
    at: number,
    set: (next: number) => void,
  ) {
    if (event.ctrlKey || event.altKey || event.metaKey || event.shiftKey || count === 0) {
      return;
    }
    const next =
      event.key === 'ArrowDown'
        ? Math.min(count - 1, at + 1)
        : event.key === 'ArrowUp'
          ? Math.max(0, at - 1)
          : event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? count - 1
              : null;
    if (next === null) {
      return;
    }
    event.preventDefault();
    set(next);
    const all = (event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(selector);
    all[next]?.focus();
  }

  /** Steps 7–8: a line of the active note's diff shows its place there. */
  function showLine(offset: number | null, index: number) {
    lineStop = index;
    if (diff?.note && offset !== null) {
      onShow(diff.note, offset);
    }
  }

  /** The number of `line` of `hunk` over all hunks, for the Tab stop. */
  function lineIndex(hunk: number, line: number): number {
    if (diff === null || diff.body.kind !== 'hunks') {
      return 0;
    }
    return diff.body.hunks.slice(0, hunk).reduce((sum, h) => sum + h.lines.length, 0) + line;
  }
</script>

{#if changes !== null && changes.problem === null}
  <div class="git">
    {#if changes.merging}
      <p class="warning">
        A synchronization left notes in conflict; resolve them before committing.
      </p>
    {:else if boxShown}
      <div class="commit">
        <textarea
          rows="3"
          placeholder="Commit message"
          aria-label="Commit message"
          aria-keyshortcuts="{commandKey}+Enter"
          spellcheck="false"
          value={box.message}
          bind:this={message}
          oninput={(event) => shell.editCommitMessage(event.currentTarget.value)}
          onkeydown={messageKey}></textarea>
        {#if longFirstLine}
          <p class="weak">The first line is long; many programs show only its start.</p>
        {/if}
        {#if changes.amendable !== null}
          <label class="amend">
            <input
              type="checkbox"
              checked={box.amend}
              onchange={(event) => shell.setAmend(event.currentTarget.checked)}
            />
            Amend the last commit
          </label>
        {/if}
        {#if nothing}
          <p class="weak">Nothing is chosen.</p>
        {/if}
        {#if box.problem !== null}
          <p class="problem" role="alert">
            <span aria-hidden="true">⚠ </span><span class="visually-hidden"
              >Error:
            </span>{box.problem}
          </p>
        {/if}
        <div class="row">
          <button
            type="button"
            class="ui-button"
            disabled={nothing || box.busy}
            aria-busy={box.busy}
            title="{command}+Enter"
            aria-keyshortcuts="{commandKey}+Enter"
            onclick={() => void shell.commitChanges()}>{box.amend ? 'Amend' : 'Commit'}</button
          >
          <button
            type="button"
            class="ui-button small"
            aria-label="Choose all changes"
            onclick={() => void shell.chooseForCommit(null, true)}>All</button
          >
          <button
            type="button"
            class="ui-button small"
            aria-label="Choose no change"
            onclick={() => void shell.chooseForCommit(null, false)}>None</button
          >
        </div>
      </div>
    {/if}
    <details bind:open={listOpen}>
      <summary bind:this={summary} aria-keyshortcuts="{commandKey}+Shift+G"
        >Changes ({changes.files.length})</summary
      >
      {#if changes.files.length === 0}
        <p class="weak">Nothing changed since the last commit.</p>
      {:else}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <ul
          class="changes"
          aria-label="Changes"
          onkeydown={(event) =>
            moveKey(event, 'button.entry', rowCount, rowStop, (next) => (rowStop = next))}
        >
          {#each changes.files as change, index (change.path)}
            {@const stop = index === rowStop ? 0 : -1}
            {#if startsOthers(index)}
              <li class="weak others">Other files</li>
            {/if}
            <li class="change">
              {#if !changes.merging}
                <input
                  type="checkbox"
                  tabindex={stop}
                  checked={change.chosen}
                  aria-label="Commit {change.name}"
                  onfocus={() => (rowStop = index)}
                  onchange={(event) =>
                    void shell.chooseForCommit(change.path, event.currentTarget.checked)}
                />
              {/if}
              <span class="git-letter" data-letter={change.letter} aria-hidden="true"
                >{change.letter}</span
              >
              <button
                type="button"
                class="entry"
                tabindex={stop}
                title={change.name}
                aria-current={selected(change) ? 'true' : undefined}
                onfocus={() => (rowStop = index)}
                onclick={() => choose(change, index)}
                >{change.name}<span class="visually-hidden">, {KIND_WORDS[change.kind]}</span
                ></button
              >
              {#if change.note && change.kind !== 'conflict'}
                <button
                  type="button"
                  class="ui-button small"
                  tabindex={stop}
                  aria-label="Discard the changes of {change.name}…"
                  onfocus={() => (rowStop = index)}
                  onclick={() => void shell.startDiscard(change.path)}>Discard…</button
                >
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </details>
    <details open bind:this={diffSection}>
      <summary>Diff</summary>
      {#if diff === null}
        <p class="weak">Open a note to see its changes.</p>
      {:else}
        <p class="weak path">{diff.path}</p>
        {#if diff.body.kind === 'notText'}
          <p class="said">{diff.body.text}</p>
        {:else if diff.body.kind === 'unchanged'}
          <p class="said">The note did not change since the last commit.</p>
        {:else if diff.body.kind === 'problem'}
          <p class="said problem" role="alert">
            <span aria-hidden="true">⚠ </span><span class="visually-hidden">Error: </span>{diff.body
              .text}
          </p>
        {:else}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="hunks"
            onkeydown={(event) =>
              moveKey(event, 'button.line', lineCount, lineStop, (next) => (lineStop = next))}
          >
            {#each diff.body.hunks as hunk, at (at)}
              <ul class="hunk" aria-label="Changed part {at + 1}">
                {#each hunk.lines as line, number (number)}
                  {@const index = lineIndex(at, number)}
                  <li>
                    {#snippet content()}
                      <span class="visually-hidden">{`${LINE_WORDS[line.kind]}: `}</span><span
                        class="numbers"
                        aria-hidden="true">{line.numbers}</span
                      ><span class="text"
                        >{#each line.parts as part, p (p)}<span class:strong={part.strong}
                            >{part.text}</span
                          >{/each}</span
                      >{#if line.only}<span class="only">{line.only}</span>{/if}
                    {/snippet}
                    {#if diff.note !== null && line.offset !== null}
                      <button
                        type="button"
                        class="line"
                        data-kind={line.kind}
                        tabindex={index === lineStop ? 0 : -1}
                        title="Show in note"
                        onfocus={() => (lineStop = index)}
                        onclick={() => showLine(line.offset, index)}>{@render content()}</button
                      >
                    {:else}
                      <div class="line" data-kind={line.kind}>{@render content()}</div>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/each}
          </div>
        {/if}
      {/if}
    </details>
  </div>
{/if}

<style>
  .git {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: var(--space-2) 0 var(--space-3);
  }

  .weak {
    margin: 0;
    color: var(--ui-hint);
  }

  .warning,
  .problem {
    margin: 0;
    padding-left: var(--space-2);
    border-left: 3px solid var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .commit {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  textarea {
    box-sizing: border-box;
    width: 100%;
    min-height: calc(3lh + 2 * var(--space-1));
    max-height: 16em;
    padding: var(--space-1) var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    resize: vertical;
  }

  .amend {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    min-height: 24px;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    align-items: center;
  }

  .ui-button:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  .ui-button:disabled:hover {
    color: var(--ui-hint);
    background: var(--ui-button);
  }

  /* egui's small buttons, still 24 px high for the pointer (WCAG 2.5.8). */
  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  summary {
    min-height: 24px;
    overflow: hidden;
    font-weight: 700;
    white-space: nowrap;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .changes,
  .hunk {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
  }

  .change {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    min-width: 0;
  }

  /* A 24 px target for the pointer (WCAG 2.5.8). */
  .change input {
    flex: none;
    width: 16px;
    height: 16px;
    margin: 4px;
  }

  .others {
    padding-top: var(--space-1);
  }

  .git-letter {
    flex: none;
    width: 1ch;
  }

  .entry {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 24px;
    padding: 2px var(--space-1);
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

  .entry[aria-current='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .entry:hover:not([aria-current='true']),
  button.line:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .entry:focus-visible,
  button.line:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .path {
    overflow-wrap: anywhere;
  }

  .said {
    margin: var(--space-1) 0 0;
  }

  .hunk {
    padding-top: var(--space-1);
    border-top: 1px solid var(--ui-rule);
  }

  .hunk + .hunk {
    margin-top: var(--space-1);
  }

  /* egui's diff: monospace, removed lines on a light red, added on a light
     green, the words changed in the stronger tint and underlined (UX review
     F12: not by colour alone). A wrapped line hangs after its numbers. */
  .line {
    display: block;
    box-sizing: border-box;
    width: 100%;
    padding: 0 var(--space-1) 0 calc(12ch + var(--space-1));
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--ui-text);
    text-align: left;
    text-indent: -12ch;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: none;
    border: 0;
    border-radius: var(--radius);
  }

  button.line {
    cursor: pointer;
  }

  .numbers {
    color: var(--ui-hint);
  }

  .line[data-kind='removed'] .text {
    background: rgb(248 81 73 / 16%);
  }

  .line[data-kind='added'] .text {
    background: rgb(46 160 67 / 16%);
  }

  .strong {
    text-decoration: underline;
    text-decoration-thickness: 2px;
  }

  .line[data-kind='removed'] .strong {
    background: rgb(248 81 73 / 47%);
  }

  .line[data-kind='added'] .strong {
    background: rgb(46 160 67 / 47%);
  }

  .only {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--ui-hint);
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
