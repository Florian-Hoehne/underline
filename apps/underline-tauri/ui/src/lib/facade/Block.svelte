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

  Figures (M-09.5 to M-09.7) show their table or picture; their markup
  shows in a frame below while the cursor is in the block (M-09.8), and
  below a diagram that cannot be rendered (UC-004 A11).

  M-08.8: a heading with a section has its fold arrow in the padding on the
  left, shown while the pointer is on the heading or while it is folded; a
  folded heading ends on "…".

  The block's text carries its runs' indices (`Runs.svelte`); what the
  markup does not hold (prefix, bars, figures) is not editable.
-->
<script lang="ts">
  import type { BlockView, Platform, SectionView } from '../bindings';
  import Figure from './Figure.svelte';
  import { codeSegments, fontSize, gapAfter } from './model';
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
    onFold: (line: number) => void;
    onEnlarge: (url: string, width: number, height: number) => void;
    /** M-09.8: a click on the figure puts the cursor into the block. */
    onFigure: (index: number) => void;
  };

  let { block, index, platform, editing, section, onFold, onEnlarge, onFigure }: Props = $props();

  const kind = $derived(block.kind);
  const size = $derived(fontSize(block));
  const markupShown = $derived(editing || (block.figure?.sourceShown ?? false));
  const code = $derived(block.runs[0]?.text ?? '');
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
      <Runs runs={block.runs} strong />{#if section?.folded}<span
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
        >{#each codeSegments(code, block.colors) as segment, s (s)}<span style:color={segment.color}
            >{segment.text}</span
          >{/each}</span
      >
    </div>
  {:else if kind.type === 'markup'}
    <div class="code"><Runs runs={block.runs} /></div>
  {:else if kind.type === 'table' || block.figure !== null}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="figure" contenteditable="false" onclick={() => onFigure(index)}>
      {#if kind.type === 'table'}
        <Table table={kind.table} />
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
        >{/if}<Runs runs={block.runs} />
    </div>
  {/if}
</div>

<style>
  .block {
    position: relative;
  }

  .text {
    position: relative;
    font-size: var(--size);
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
    font-size: 14px;
    line-height: 1.25;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: var(--palette-code-background);
    border-radius: 4px;
  }

  .frame {
    margin-top: 12px;
  }

  .figure {
    cursor: default;
  }

  .ellipsis {
    margin-left: 10px;
    font-size: 15px;
    font-weight: 400;
    color: var(--facade-weak);
  }

  /* M-08.8: the arrow in the space left of the heading; it takes the
     pointer over 24 × 24 px (A-44). */
  .fold {
    position: absolute;
    top: calc((var(--size) * 1.25 - 24px) / 2);
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
