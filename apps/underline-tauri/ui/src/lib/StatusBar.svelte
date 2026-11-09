<!--
  M-04.7 (FR-020): the status bar's text: the workspace's root path and the
  status message, joined by "  |  " as in the egui app. The branch, sync,
  agents and todo parts come with their features (M-04.1 to M-04.10).
-->
<script lang="ts">
  type Props = {
    /** The root of the open workspace, if one is open. */
    root: string | null;
    /** The status message; empty when there is none. */
    status: string;
  };

  let { root, status }: Props = $props();

  const text = $derived([root ?? '', status].filter((part) => part !== '').join('  |  '));
</script>

<footer class="status-bar">
  <p class="text" role="status" title={text}>{text}</p>
</footer>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
    padding: var(--space-1) var(--space-2);
    border-top: 1px solid var(--ui-rule);
  }

  /* A very long message wraps, up to four lines, then scrolls. */
  .text {
    min-width: 0;
    max-height: calc(4 * 1.4em);
    overflow: auto;
    margin: 0;
    /* "  |  " keeps its two spaces; a long message wraps instead of being cut. */
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
