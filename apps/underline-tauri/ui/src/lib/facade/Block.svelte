<!--
  M-09.1, M-09.2 (UC-004 step 3): one block of the facade, as egui's
  `block_job` and `show_editor` draw it:

  - headings 26/22/19/17/16 px, bold in the colour "strong";
  - text 15 px; a list item, a quote or an admonition starts with its
    prefix in the weak colour (indentation, "•", "n.", "☑"/"☐", the label);
  - a quote has one bar per depth, an admonition a bar in the link colour;
  - a thematic break is a line;
  - code (coloured in its language, M-09.4), markup, and the markup of a
    table or an image in the code font, 14 px, on the code background.

  Package 5: the sizes of the text are times the editor zoom
  (`--editor-zoom`, UC-010 BR-009), figures keep theirs; without soft wrap
  (`.nowrap` on the facade, BR-011, OQ-211) code, markup and tables keep
  their lines and scroll sideways.

  Figures (M-09.5 to M-09.7) show their table or picture; their markup
  shows in a frame below while the cursor is in the block (M-09.8), and
  below a diagram that cannot be rendered (UC-004 A11).

  M-08.8: a heading with a section has its fold arrow in the padding on the
  left, shown while the pointer is on the heading or while it is folded; a
  folded heading ends on "…".

  The block's text carries its runs' indices (`Runs.svelte`); what the
  markup does not hold (prefix, bars, figures) is not editable.

  FR-098, UC-004 BR-019, A21, UC-005 BR-008: a block formula stands
  centred, typeset in the size of the text (it grows with the editor zoom);
  its markup shows in a frame below while the cursor is in it, while it is
  typeset (until the picture comes, the formula is its source), and with an
  error, whose reason stands above the frame. Inline formulas are the runs'
  (`Runs.svelte`).

  M-13.2 (UC-008 BR-008): the words of a search are marked in the text of
  headings, paragraphs, lists, quotes, code and markup, as egui marks them
  where it shows text; not in tables and figures.
-->
<script lang="ts">
  import type { BlockView, Platform, SectionView } from '../bindings';
  import { assetUrl } from './backend';
  import Figure from './Figure.svelte';
  import {
    codeSegments,
    em,
    fontSize,
    formulaBlockClick,
    gapAfter,
    hitParts,
    type HitRange,
  } from './model';
  import Runs from './Runs.svelte';
  import Table from './Table.svelte';

  type Props = {
    block: BlockView;
    index: number;
    platform: Platform;
    /** M-09.8: the cursor is in this figure: its markup shows below it. */
    editing: boolean;
    /** M-08.8: the section whose heading this block is. */
    section: SectionView | null;
    /** M-13.2: the hits in this block, sorted, in web units of the markup. */
    hits?: readonly HitRange[];
    /** A6: the hit that is the selection. */
    current?: HitRange | null;
    /** FR-099: the words the spell check marks in this block, sorted, in web units of the markup. */
    spell?: readonly HitRange[];
    onFold: (line: number) => void;
    onEnlarge: (url: string, width: number, height: number) => void;
    /** M-09.8: a click on the figure puts the cursor into the block. */
    onFigure: (index: number) => void;
    /** FR-098: the cursor, a web offset, when it is in this block. */
    cursor?: number | null;
    /** FR-098: a click on an inline formula puts the cursor there. */
    onFormula?: (offset: number) => void;
  };

  let {
    block,
    index,
    platform,
    editing,
    section,
    hits = [],
    current = null,
    spell = [],
    onFold,
    onEnlarge,
    onFigure,
    cursor = null,
    onFormula,
  }: Props = $props();

  const kind = $derived(block.kind);
  const size = $derived(fontSize(block));
  /** FR-098: the block formula, if the block is one. */
  const formula = $derived(kind.type === 'formula' ? kind.formula : null);
  const markupShown = $derived(
    editing ||
      (block.figure?.sourceShown ?? false) ||
      (formula !== null && formula.picture === null),
  );
  const code = $derived(block.runs[0]?.text ?? '');
  /** The code's coloured segments, split where the hits start and end. */
  const codeParts = $derived.by(() => {
    const segments = codeSegments(code, block.colors);
    const run = block.runs[0];
    if (!run || hits.length === 0) {
      return segments.map((segment) => ({ ...segment, hit: false, current: false }));
    }
    const parts: { text: string; color: string | null; hit: boolean; current: boolean }[] = [];
    let at = 0;
    for (const segment of segments) {
      const end = at + segment.text.length;
      for (const part of hitParts(block, run, at, end, hits, current)) {
        parts.push({
          text: code.slice(part.from, part.to),
          color: segment.color,
          hit: part.hit,
          current: part.current,
        });
      }
      at = end;
    }
    return parts;
  });
  /** The picture's text for screen readers: the alt text, or the caption, or what the diagram is. */
  const pictureAlt = $derived(
    kind.type === 'image'
      ? kind.alt
      : (block.figure?.caption ??
          (kind.type === 'code'
            ? `${kind.language.toLowerCase() === 'mermaid' ? 'Mermaid' : 'PlantUML'} diagram`
            : '')),
  );
  const quoteBars = $derived(
    kind.type === 'quote' ? Array.from({ length: kind.depth }, (_, d) => 4 + d * 14) : [],
  );
</script>

<div
  class="block"
  data-b={index}
  style:padding-bottom="{gapAfter(block)}px"
  style:--size="{size}px"
>
  {#if section}
    <span
      class="fold"
      class:folded={section.folded}
      contenteditable="false"
      aria-hidden="true"
      onmousedown={(event) => {
        event.preventDefault();
        onFold(section.line);
      }}
    >
      <svg viewBox="-5 -5 10 10" width="10" height="10">
        <polygon points={section.folded ? '-2.4,-4 3.6,0 -2.4,4' : '-4,-2.4 4,-2.4 0,3.6'} />
      </svg>
    </span>
  {/if}
  {#if kind.type === 'heading'}
    <div class="text heading" role="heading" aria-level={kind.level}>
      <Runs
        runs={block.runs}
        strong
        {block}
        {hits}
        {current}
        {spell}
        {cursor}
        {platform}
        {onFormula}
      />{#if section?.folded}<span
          class="ellipsis"
          contenteditable="false"
          role="img"
          aria-label="folded">…</span
        >{/if}
    </div>
  {:else if kind.type === 'thematicBreak'}
    <div class="rule" contenteditable="false" role="separator"></div>
  {:else if kind.type === 'code' && block.figure === null}
    <div class="code">
      <span data-r="0"
        >{#each codeParts as segment, s (s)}<span
            class:hit={segment.hit}
            class:current={segment.current}
            style:color={segment.color}>{segment.text}</span
          >{/each}</span
      >
    </div>
  {:else if kind.type === 'markup'}
    <div class="code"><Runs runs={block.runs} {block} {hits} {current} /></div>
  {:else if formula}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="figure formula"
      contenteditable="false"
      title="Edit formula"
      onclick={() => (onFormula ? onFormula(formulaBlockClick(block)) : onFigure(index))}
    >
      {#if formula.picture}
        <img
          src={assetUrl(formula.picture, platform)}
          alt="Formula: {formula.source}"
          draggable="false"
          style:width={em(formula.width)}
          style:aspect-ratio="{formula.width} / {Math.max(formula.height, 1)}"
        />
      {/if}
      {#if formula.error}
        <p class="failure" role="status">{formula.error}</p>
      {/if}
    </div>
    {#if markupShown}
      <div class="code frame" class:first={formula.picture === null && formula.error === null}>
        <Runs runs={block.runs} />
      </div>
    {/if}
  {:else if kind.type === 'table' || block.figure !== null}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="figure"
      class:table={kind.type === 'table'}
      contenteditable="false"
      onclick={() => onFigure(index)}
    >
      {#if kind.type === 'table'}
        <Table table={kind.table} {platform} {block} {spell} />
      {:else if block.figure}
        <Figure figure={block.figure} {platform} alt={pictureAlt} {onEnlarge} />
      {/if}
    </div>
    {#if markupShown}
      <div class="code frame"><Runs runs={block.runs} /></div>
    {/if}
  {:else}
    <div
      class="text"
      class:quote={kind.type === 'quote'}
      class:admonition={kind.type === 'admonition'}
    >
      {#each quoteBars as left (left)}<span
          class="bar"
          style:left="{left}px"
          contenteditable="false"
          aria-hidden="true"
        ></span>{/each}{#if block.prefix}<span class="prefix" contenteditable="false"
          >{block.prefix}</span
        >{/if}<Runs
        runs={block.runs}
        {block}
        {hits}
        {current}
        {spell}
        {cursor}
        {platform}
        {onFormula}
      />
    </div>
  {/if}
</div>

<style>
  .block {
    position: relative;
  }

  /* UC-010 BR-009: the text grows with the editor zoom (`--editor-zoom`,
     set by the facade); figures keep their size. */
  .text {
    position: relative;
    font-size: calc(var(--size) * var(--editor-zoom, 1));
    line-height: 1.25;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .heading {
    font-weight: 700;
    color: var(--palette-strong);
  }

  .prefix {
    color: var(--facade-weak);
    user-select: none;
  }

  .bar {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 3px;
    margin-left: -1.5px;
    background: var(--facade-weak);
  }

  .admonition::before {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 2.5px;
    width: 3px;
    content: '';
    background: var(--palette-link);
  }

  .rule {
    height: 16px;
    background: linear-gradient(var(--facade-weak), var(--facade-weak)) 0 8px / 100% 1px no-repeat;
  }

  /* egui draws the code background 4 px around the text. */
  .code {
    margin: 0;
    padding: 4px;
    font-family: var(--font-mono);
    font-size: calc(14px * var(--editor-zoom, 1));
    line-height: 1.25;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: var(--palette-code-background);
    border-radius: 4px;
  }

  /* UC-010 BR-011, OQ-211: without soft wrap, code and the frames of markup
     keep their lines and scroll sideways. */
  :global(.nowrap) .code {
    overflow-x: auto;
    white-space: pre;
    overflow-wrap: normal;
  }

  /* A table too (Table.svelte keeps its cells on one line). */
  :global(.nowrap) .figure.table {
    overflow-x: auto;
  }

  .frame {
    margin-top: 12px;
  }

  /* FR-098: until the picture comes, the source stands in its place. */
  .frame.first {
    margin-top: 0;
  }

  /* FR-098: a block formula, centred, in the size of the text. */
  .formula {
    font-size: calc(15px * var(--editor-zoom, 1));
    text-align: center;
  }

  /* Wider than the facade, it is scaled down to it, keeping its shape. */
  .formula img {
    max-width: 100%;
    height: auto;
    vertical-align: top;
    cursor: text;
    user-select: none;
  }

  .formula img:hover {
    background: var(--palette-code-background);
    border-radius: 3px;
  }

  /* UC-004 A21: as a diagram's failure (Figure.svelte). */
  .failure {
    margin: 0;
    padding-left: 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--palette-foreground);
    text-align: left;
    overflow-wrap: anywhere;
    border-left: 3px solid var(--palette-foreground);
  }

  /* M-13.2: a hit in code, as in the runs (Runs.svelte). */
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

  .figure {
    cursor: default;
  }

  .ellipsis {
    margin-left: 10px;
    font-size: calc(15px * var(--editor-zoom, 1));
    font-weight: 400;
    color: var(--facade-weak);
  }

  /* M-08.8: the arrow in the space left of the heading; it takes the
     pointer over 24 × 24 px (A-44). */
  .fold {
    position: absolute;
    top: calc((var(--size) * var(--editor-zoom, 1) * 1.25 - 24px) / 2);
    left: -24px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    cursor: pointer;
    visibility: hidden;
    user-select: none;
  }

  .fold svg {
    fill: var(--facade-weak);
  }

  .block:hover > .fold,
  .fold.folded {
    visibility: visible;
  }
</style>
