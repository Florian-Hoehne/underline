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

  FR-099, UC-005 BR-023 (ADR-030): the words the spell check marks are
  spans inside their run too, underlined with a wave in the theme's colour
  for errors; the wave is the mark's shape, so it does not hang on colour
  alone. A word marked and found is both.

  FR-098, UC-004 BR-019, A21, UC-005 BR-008: an inline formula shows its
  typeset picture (`ul-asset`, ADR-023), sized in em so it grows with the
  editor zoom, at its baseline, its source as the alternative text; a
  display formula within a paragraph stands centred on its own line. While
  the cursor is in it, while it is typeset, and with an error, its markup
  shows in its place in the code font; an error adds its reason after it.
  A click on the picture puts the cursor into its source; that is the
  mouse's way into it, the keyboard goes into it by moving the cursor, so
  the picture's span needs no role.
-->
<script lang="ts">
  import type { BlockView, Platform, RunView } from '../bindings';
  import { assetUrl } from './backend';
  import { em, formulaClick, inFormula, markParts, runLength, type HitRange } from './model';

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
    /** FR-099: the words the spell check marks in the block, sorted, in web units of the markup. */
    spell?: readonly HitRange[];
    /** FR-098: the cursor, a web offset, when it is in this block. */
    cursor?: number | null;
    /** For the addresses of the formulas' pictures. */
    platform?: Platform;
    /** FR-098: a click on a formula puts the cursor at this web offset. */
    onFormula?: (offset: number) => void;
  };

  let {
    runs,
    strong = false,
    mapped = true,
    block = null,
    hits = [],
    current = null,
    spell = [],
    cursor = null,
    platform = 'linux',
    onFormula,
  }: Props = $props();

  /** FR-098: whether the run is shown as its typeset formula. */
  function typeset(run: RunView): boolean {
    return (
      run.formula !== null &&
      run.formula.picture !== null &&
      !(block !== null && inFormula(block, run, cursor))
    );
  }

  function clickFormula(event: MouseEvent, run: RunView) {
    if (block && onFormula && event.button === 0) {
      event.preventDefault();
      onFormula(formulaClick(block, run));
    }
  }

  /** The parts of each run, when the block has hits or marked words. */
  const parts = $derived(
    block && (hits.length > 0 || spell.length > 0)
      ? runs.map((run) =>
          run.code || run.formula !== null
            ? markParts(block, run, 0, runLength(run), hits, current, [])
            : markParts(block, run, 0, runLength(run), hits, current, spell),
        )
      : null,
  );
</script>

{#each runs as run, index (index)}{#if run.formula && typeset(run)}<!-- svelte-ignore a11y_no_static_element_interactions --><span
      class="run formula"
      class:display={run.formula.display}
      class:bold={run.bold || strong}
      data-r={mapped ? index : undefined}
      data-formula=""
      contenteditable="false"
      title="Edit formula"
      onmousedown={(event) => clickFormula(event, run)}
      ><img
        src={assetUrl(run.formula.picture ?? '', platform)}
        alt="Formula: {run.formula.source}"
        draggable="false"
        style:width={em(run.formula.width)}
        style:aspect-ratio="{run.formula.width} / {Math.max(run.formula.height, 1)}"
        style:vertical-align="-{em(run.formula.depth)}"
      /></span
    >{:else}<span
      class="run"
      class:bold={run.bold || strong}
      class:italic={run.italic}
      class:strike={run.strike}
      class:code={run.code}
      class:link={run.link !== null}
      class:source={run.formula !== null}
      class:failed={run.formula?.error != null}
      data-r={mapped ? index : undefined}
      data-link={run.link ?? undefined}
      >{#if parts}{#each parts[index] as part (part.from)}{#if part.hit || part.spell}<span
              class:hit={part.hit}
              class:current={part.current}
              class:spell={part.spell}
              data-spell={part.spell ? '' : undefined}
              aria-invalid={part.spell ? 'spelling' : undefined}
              >{run.text.slice(part.from, part.to)}</span
            >{:else}{run.text.slice(part.from, part.to)}{/if}{/each}{:else}{run.text}{/if}</span
    >{#if run.formula?.error}<span class="reason" contenteditable="false" role="status"
        >{run.formula.error}</span
      >{/if}{/if}{/each}

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
    font-size: calc(14px * var(--editor-zoom, 1));
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

  /* FR-098: a typeset formula; one set as a display formula stands on its
     own line, centred. */
  /* A formula wider than its line is scaled down to it, keeping its shape. */
  .formula img {
    max-width: 100%;
    height: auto;
    user-select: none;
  }

  /* It is edited by a click: the text cursor, and the code background under
     the pointer. */
  .formula {
    cursor: text;
    border-radius: 3px;
  }

  .formula:hover {
    background: var(--palette-code-background);
  }

  .formula.display {
    display: block;
    margin: 0.4em 0;
    text-align: center;
  }

  /* FR-098, UC-005 BR-008: the markup of a formula while it is edited, typeset
     or has an error. */
  /* In the size of the text around it, so the line keeps its height. */
  .source {
    font-family: var(--font-mono);
    font-size: inherit;
    font-weight: 400;
    background: var(--palette-code-background);
  }

  /* UC-004 A21: the reason after the source, set apart by a rule, not only
     by colour; read out as a status. */
  .reason {
    margin-left: 6px;
    padding-left: 6px;
    font-family: var(--font-ui);
    font-size: 13px;
    font-weight: 600;
    color: var(--palette-foreground);
    border-left: 3px solid var(--palette-foreground);
  }

  /* M-13.2: as egui marks a hit, and as the source view does (.cm-ul-found). */
  .hit {
    background: var(--palette-found-background);
    box-shadow: inset 0 -2px var(--palette-found);
  }

  /* FR-099, UC-005 BR-023: a word the spell check marks (app.css). */
  .spell {
    text-decoration: underline wavy var(--ui-spell);
    text-decoration-thickness: var(--ui-spell-thickness);
    text-decoration-skip-ink: none;
    text-underline-offset: var(--ui-spell-offset);
  }

  /* The hit chosen by the find field: also by its outline, not only by colour. */
  .hit.current {
    background: var(--palette-selection);
    outline: 1px solid var(--palette-foreground);
    outline-offset: -1px;
  }
</style>
