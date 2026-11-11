<!--
  M-19.9, UC-015 steps 3–5, A1, A5, BR-006: the conflict bar over a note in
  conflict, as egui's `conflict_bar`: "Conflict: place i of n open (line L)"
  in the conflict colour, "Previous" (Shift+F7), "Next" (F7), "All: Here",
  "All: Remote" and "Abort the sync"; the place's two versions "Here" and
  "Remote" side by side in monospace, at most 160 px high each (one below
  the other in a narrow window, BR-002; lower in a low window); then "Here"
  (Alt+H), "Remote" (Alt+R), "Both" (Alt+B: here first, then remote) and
  "Or edit the place in the note.". The keys show at the pointer, as in
  egui (which shows them nowhere else).

  The keys work wherever the keyboard is while the bar shows, before the
  editors see them, as egui takes them first; not while a dialog, the
  search or the branch menu is open (UX review F13). Previous and Next show
  the place in the note (the source view puts the cursor there, the facade
  scrolls to it, as the outline does, A-124). A choice is of the text the
  bar shows (its revision, S3).

  UX review F13 (A-150, Q-104): "All: Here" and "All: Remote" stand apart
  from Previous and Next and say for screen readers "Resolve all places
  with Here/Remote"; as in egui they do not ask first. The two versions are
  regions "Here (your version of the place)" and "Remote (the version from
  the remote)" (new texts, Q-97). When the last place goes, the keyboard
  goes into the note.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { ConflictBarView } from './bindings';
  import { activeFacade } from './facade/active';
  import { activeSourceView } from './editor/active';
  import type { Shell } from './shell.svelte';

  type Props = {
    shell: Shell;
    bar: ConflictBarView;
    /** Shows the place `offset` of `note` in its view. */
    onShow: (note: string, offset: number) => void;
  };

  let { shell, bar, onShow }: Props = $props();

  const at = $derived(Math.min(shell.conflictPlace, bar.places.length - 1));
  const place = $derived(bar.places[at]);

  function move(by: number) {
    const index = shell.moveConflictPlace(by);
    const target = index === null ? undefined : bar.places[index];
    if (target) {
      onShow(bar.note, target.offset);
    }
  }

  /** Whether something above the note has the keys now. */
  function keysElsewhere(): boolean {
    return (
      document.querySelector('[role="dialog"]') !== null ||
      shell.search.open ||
      shell.branchMenu !== null
    );
  }

  /** BR-006: F7, Shift+F7, Alt+H, Alt+R, Alt+B. */
  function key(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey || keysElsewhere()) {
      return;
    }
    let act: (() => void) | null = null;
    if (event.key === 'F7' && !event.altKey) {
      act = () => move(event.shiftKey ? -1 : 1);
    } else if (event.altKey && !event.shiftKey) {
      const choice = { h: 'here', r: 'remote', b: 'both' } as const;
      const letter = event.code.startsWith('Key') ? event.code.slice(3).toLowerCase() : '';
      const chosen = choice[letter as keyof typeof choice];
      if (chosen) {
        act = () => void shell.chooseConflict(chosen);
      }
    }
    if (act) {
      event.preventDefault();
      event.stopPropagation();
      act();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', key, true);
    return () => {
      window.removeEventListener('keydown', key, true);
      // UX review F13: the last place went with the keyboard in the bar;
      // the keyboard goes into the note.
      if (document.activeElement === document.body || !document.activeElement?.isConnected) {
        const facade = activeFacade();
        if (facade?.note === bar.note) {
          facade.focus();
        } else if (activeSourceView()?.note === bar.note) {
          activeSourceView()?.focus();
        }
      }
    };
  });

  const hereId = `conflict-here-${Math.random().toString(36).slice(2)}`;
  const remoteId = `conflict-remote-${Math.random().toString(36).slice(2)}`;
</script>

<section class="conflict" aria-label="Conflict">
  <div class="row">
    <p class="where" aria-live="polite">
      Conflict: place {at + 1} of {bar.places.length} open (line {place.line})
    </p>
    <button
      type="button"
      class="ui-button small"
      title="Shift+F7"
      aria-keyshortcuts="Shift+F7"
      onclick={() => move(-1)}>Previous</button
    >
    <button
      type="button"
      class="ui-button small"
      title="F7"
      aria-keyshortcuts="F7"
      onclick={() => move(1)}>Next</button
    >
    <span class="apart" role="group" aria-label="Whole note">
      <button
        type="button"
        class="ui-button small"
        aria-label="Resolve all places with Here"
        onclick={() => void shell.chooseConflict('here', true)}>All: Here</button
      >
      <button
        type="button"
        class="ui-button small"
        aria-label="Resolve all places with Remote"
        onclick={() => void shell.chooseConflict('remote', true)}>All: Remote</button
      >
      <button type="button" class="ui-button small" onclick={() => shell.askAbort()}
        >Abort the sync</button
      >
    </span>
  </div>
  <div class="sides">
    <div class="side">
      <h3 id={hereId}>Here</h3>
      <!-- A scrolling box takes the keyboard, so it scrolls without a mouse (WCAG 2.1.1). -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <pre
        tabindex="0"
        role="region"
        aria-label="Here (your version of the place)">{place.here}</pre>
    </div>
    <div class="side">
      <h3 id={remoteId}>Remote</h3>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <pre
        tabindex="0"
        role="region"
        aria-label="Remote (the version from the remote)">{place.remote}</pre>
    </div>
  </div>
  <div class="row">
    <button
      type="button"
      class="ui-button"
      title="Alt+H"
      aria-keyshortcuts="Alt+H"
      onclick={() => void shell.chooseConflict('here')}>Here</button
    >
    <button
      type="button"
      class="ui-button"
      title="Alt+R"
      aria-keyshortcuts="Alt+R"
      onclick={() => void shell.chooseConflict('remote')}>Remote</button
    >
    <button
      type="button"
      class="ui-button"
      title="Alt+B: here first, then remote"
      aria-keyshortcuts="Alt+B"
      onclick={() => void shell.chooseConflict('both')}>Both</button
    >
    <span class="weak">Or edit the place in the note.</span>
  </div>
</section>

<style>
  /* egui's group frame with a stroke in the conflict colour. */
  .conflict {
    --conflict: rgb(255 110 40);

    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: var(--space-1) var(--space-2);
    padding: var(--space-2);
    border: 1px solid var(--conflict);
    border-radius: var(--radius);
    /* Retest F13: the note keeps most of the column; the bar scrolls. */
    flex: none;
    max-height: 40vh;
    overflow: auto;
  }

  /* UX review F13: a darker orange on light themes reaches 4.5:1. */
  :global(:root[data-scheme='light']) .conflict {
    --conflict: rgb(178 60 0);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    align-items: center;
  }

  /* UX review F13: the choices for the whole note stand apart. */
  .apart {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin-left: var(--space-3);
    padding-left: var(--space-3);
    border-left: 1px solid var(--ui-rule);
  }

  .where {
    margin: 0 var(--space-1) 0 0;
    font-weight: 700;
    color: var(--conflict);
  }

  .small {
    min-height: 24px;
    padding: 0 var(--space-2);
  }

  /* Side by side; one below the other in a narrow window (BR-002). */
  .sides {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16em, 1fr));
    gap: var(--space-2);
  }

  .side {
    min-width: 0;
  }

  h3 {
    margin: 0 0 var(--space-1);
    font-size: inherit;
    font-weight: 700;
  }

  pre {
    max-height: 160px;
    margin: 0;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 13px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  /* UX review F13: a low window keeps room for the note. */
  @media (max-height: 600px) {
    .conflict {
      max-height: 30vh;
    }

    pre {
      max-height: 64px;
    }
  }

  .weak {
    color: var(--ui-hint);
  }
</style>
