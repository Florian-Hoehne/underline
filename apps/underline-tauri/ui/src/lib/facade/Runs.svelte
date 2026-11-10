<!--
  M-09.2: the runs of a block in their styles, as egui's `append_runs` sets
  them: bold in the colour "strong", italic, struck through, inline code in
  the code font on the code background, links underlined in the link
  colour. Every run is one element carrying its index (`data-r`), so a
  place in the DOM maps to the markup (`model.ts`). Text only, never HTML
  from the note.
-->
<script lang="ts">
  import type { RunView } from '../bindings';

  type Props = {
    runs: readonly RunView[];
    /** A heading or a table header: every run bold in the colour "strong". */
    strong?: boolean;
    /** The runs carry their index, for the places of the cursor. */
    mapped?: boolean;
  };

  let { runs, strong = false, mapped = true }: Props = $props();
</script>

{#each runs as run, index (index)}<span
    class="run"
    class:bold={run.bold || strong}
    class:italic={run.italic}
    class:strike={run.strike}
    class:code={run.code}
    class:link={run.link !== null}
    data-r={mapped ? index : undefined}
    data-link={run.link ?? undefined}>{run.text}</span
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
</style>
