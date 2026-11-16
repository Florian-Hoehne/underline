<!--
  UC-026 step 4 (as UC-011 BR-004, M-19.3): changed parts of a note, read
  only, as the diff of the workspace shows them: each line after its
  numbers, lines taken away on a light red, lines added on a light green,
  the words changed within a line in the stronger tint and underlined (not
  by colour alone), "(only spaces differ)" after a line. Screen readers
  hear what each line is.

  The diff of the git changes (`GitChanges.svelte`) draws its lines the
  same way; there a line of the open note is a button that shows its place.
-->
<script lang="ts">
  import type { DiffLineKind, HunkView } from './bindings';

  type Props = {
    hunks: HunkView[];
    /** What the lines are, for screen readers; "removed", "added" by default. */
    words?: Record<DiffLineKind, string>;
  };

  let { hunks, words = { same: 'unchanged', removed: 'removed', added: 'added' } }: Props =
    $props();
</script>

<div class="hunks">
  {#each hunks as hunk, at (at)}
    <ul class="hunk" aria-label="Changed part {at + 1}">
      {#each hunk.lines as line, number (number)}
        <li>
          <div class="line" data-kind={line.kind}>
            <span class="visually-hidden">{`${words[line.kind]}: `}</span><span
              class="numbers"
              aria-hidden="true">{line.numbers}</span
            ><span class="text"
              >{#each line.parts as part, p (p)}<span class:strong={part.strong}>{part.text}</span
                >{/each}</span
            >{#if line.only}<span class="only">{line.only}</span>{/if}
          </div>
        </li>
      {/each}
    </ul>
  {/each}
</div>

<style>
  .hunk {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .hunk + .hunk {
    margin-top: var(--space-1);
    padding-top: var(--space-1);
    border-top: 1px dashed var(--ui-rule);
  }

  /* A wrapped line hangs after its numbers. */
  .line {
    box-sizing: border-box;
    padding: 0 var(--space-1) 0 calc(12ch + var(--space-1));
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--ui-text);
    text-indent: -12ch;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
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
