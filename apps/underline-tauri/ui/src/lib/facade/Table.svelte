<!--
  M-09.5 (UC-004 step 3): a table as egui's `layout_table` draws it: each
  column as wide as its widest cell, narrowed when the table is wider than
  the facade (the webview's table layout, A-36), the text aligned as its
  column says, the header bold in the colour "strong" on the code
  background, a grid in the weak colour, and "Table n. {title}" above it.
-->
<script lang="ts">
  import type { TableView } from '../bindings';
  import Runs from './Runs.svelte';

  type Props = { table: TableView };

  let { table }: Props = $props();

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
            <Runs runs={header[column] ?? []} strong mapped={false} />
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
            <Runs runs={row[column] ?? []} mapped={false} />
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
</table>

<style>
  .caption {
    margin: 0 0 6px;
    font-size: 14px;
  }

  table {
    max-width: 100%;
    border-collapse: collapse;
    font-size: 15px;
  }

  th,
  td {
    min-width: 20px;
    padding: 5px 10px;
    vertical-align: top;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    border: 1px solid var(--facade-weak);
  }

  th {
    font-weight: 700;
    background: var(--palette-code-background);
  }
</style>
