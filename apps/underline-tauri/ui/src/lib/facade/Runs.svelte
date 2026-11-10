<!--
  M-09.2: the runs of a block in their styles, as egui's `append_runs` sets
  them: bold in the colour "strong", italic, struck through, inline code in
  the code font on the code background, links underlined in the link
  colour. Every run is one element carrying its index (`data-r`), so a
  place in the DOM maps to the markup (`model.ts`). Text only, never HTML
  from the note.

  M-13.2 (UC-008 BR-008): the words of a search are marked inside their
  run, behind the text in the theme's colour for hits and underlined 2 px,
  as egui marks them; the hit the find field selected shows in the
  selection's colour while the keyboard is elsewhere (A6). The marks are
  spans inside the run's element, so the places of the cursor stay.
-->
<script lang="ts">
  import type { BlockView, RunView } from '../bindings';
  import { hitParts, runLength, type HitRange } from './model';

  type Props = {
    runs: readonly RunView[];
    /** A heading or a table header: every run bold in the colour "strong". */
    strong?: boolean;
    /** The runs carry their index, for the places of the cursor. */
    mapped?: boolean;
    /** M-13.2: the block of the runs, for the places of the hits. */
    block?: BlockView | null;
    /** M-13.2: the hits in the block, sorted, in web units of the markup. */
    hits?: readonly HitRange[];
    /** A6: the hit that is the selection. */
    current?: HitRange | null;
  };

  let {
    runs,
    strong = false,
    mapped = true,
    block = null,
    hits = [],
    current = null,
  }: Props = $props();

  /** The parts of each run, when the block has hits. */
  const parts = $derived(
    block && hits.length > 0
      ? runs.map((run) => hitParts(block, run, 0, runLength(run), hits, current))
      : null,
  );
</script>

{#each runs as run, index (index)}<span
    class="run"
    class:bold={run.bold || strong}
    class:italic={run.italic}
    class:strike={run.strike}
    class:code={run.code}
    class:link={run.link !== null}
    data-r={mapped ? index : undefined}
    data-link={run.link ?? undefined}
    >{#if parts}{#each parts[index] as part (part.from)}{#if part.hit}<span
            class="hit"
            class:current={part.current}>{run.text.slice(part.from, part.to)}</span
          >{:else}{run.text.slice(part.from, part.to)}{/if}{/each}{:else}{run.text}{/if}</span
  >{/each}

<style>
  .bold {
    font-weight: 700;
    color: var(--palette-strong);
  }

  .italic {
    font-style: italic;
  }

  .strike {
    text-decoration: line-through;
    text-decoration-color: var(--palette-foreground);
  }

  .code {
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 400;
    background: var(--palette-code-background);
  }

  /* A link wins over bold for the colour, as in egui. */
  .link {
    color: var(--palette-link);
    text-decoration: underline;
    text-decoration-color: var(--palette-link);
  }

  .link.strike {
    text-decoration: underline line-through;
  }

  /* M-13.2: as egui marks a hit, and as the source view does (.cm-ul-found). */
  .hit {
    background: var(--palette-found-background);
    box-shadow: inset 0 -2px var(--palette-found);
  }

  /* The hit chosen by the find field: also by its outline, not only by colour. */
  .hit.current {
    background: var(--palette-selection);
    outline: 1px solid var(--palette-foreground);
    outline-offset: -1px;
  }
</style>
