<!--
  M-09.5 (UC-004 step 3): a table as egui's `layout_table` draws it: each
  column as wide as its widest cell, narrowed when the table is wider than
  the facade (the webview's table layout, A-36), the text aligned as its
  column says, the header bold in the colour "strong" on the code
  background, a grid in the weak colour, and "Table n. {title}" above it.

  FR-099, UC-005 BR-023: the words of its cells the spell check marks are
  underlined there too (and in the front matter's values), when `block` and
  `spell` are given.
-->
<script lang="ts">
  import type { BlockView, Platform, TableView } from '../bindings';
  import type { HitRange } from './model';
  import Runs from './Runs.svelte';

  type Props = {
    table: TableView;
    platform?: Platform;
    /** FR-099: the block of the table, for the places of the marks. */
    block?: BlockView | null;
    /** FR-099: the marked words of the block, sorted, in web units of the markup. */
    spell?: readonly HitRange[];
  };

  let { table, platform = 'linux', block = null, spell = [] }: Props = $props();

  const columns = $derived(Math.max(1, ...table.rows.map((row) => row.length)));
  const columnIndices = $derived(Array.from({ length: columns }, (_, i) => i));
  const header = $derived(table.header && table.rows.length > 0 ? table.rows[0] : null);
  const body = $derived(header ? table.rows.slice(1) : table.rows);
</script>

{#if table.caption}
  <p class="caption">{table.caption}</p>
{/if}
<table>
  {#if header}
    <thead>
      <tr>
        {#each columnIndices as column (column)}
          <th style:text-align={table.align[column] ?? 'left'}>
            <Runs runs={header[column] ?? []} strong mapped={false} {platform} {block} {spell} />
          </th>
        {/each}
      </tr>
    </thead>
  {/if}
  <tbody>
    {#each body as row, r (r)}
      <tr>
        {#each columnIndices as column (column)}
          <td style:text-align={table.align[column] ?? 'left'}>
            <Runs runs={row[column] ?? []} mapped={false} {platform} {block} {spell} />
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .caption {
    margin: 0 0 6px;
    font-size: calc(14px * var(--editor-zoom, 1));
  }

  table {
    max-width: 100%;
    border-collapse: collapse;
    font-size: calc(15px * var(--editor-zoom, 1));
  }

  th,
  td {
    min-width: 20px;
    /* An empty cell keeps the height of a line (a cell's height is its least). */
    height: 1.25em;
    padding: 5px 10px;
    vertical-align: top;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    border: 1px solid var(--facade-weak);
  }

  /* UC-010 BR-011, OQ-211: without soft wrap a cell keeps its lines; the
     table scrolls sideways in its block (Block.svelte). */
  :global(.nowrap) th,
  :global(.nowrap) td {
    white-space: pre;
    overflow-wrap: normal;
  }

  th {
    font-weight: 700;
    background: var(--palette-code-background);
  }
</style>
