<!--
  M-04.7 (FR-020): the status bar's text: the workspace's root path and the
  status message, joined by "  |  " as in the egui app. The branch, sync,
  agents and todo parts come with their features (M-04.1 to M-04.10).

  UC-007 A4 (M-14.3): after "Deleted {t}" the bar offers "Undo", as long as
  that report shows.
-->
<script lang="ts">
  import type { StatusOffer } from './bindings';

  type Props = {
    /** The root of the open workspace, if one is open. */
    root: string | null;
    /** The status message; empty when there is none. */
    status: string;
    /** What the bar offers with the message. */
    offer?: StatusOffer | null;
    /** "Undo" of a deletion. */
    onUndo?: () => void;
  };

  let { root, status, offer = null, onUndo }: Props = $props();

  const text = $derived([root ?? '', status].filter((part) => part !== '').join('  |  '));
</script>

<footer class="status-bar">
  <p class="text" role="status" title={text}>
    {text}{#if offer === 'undoDelete'}<span class="hidden">. Undo available</span>{/if}
  </p>
  {#if offer === 'undoDelete'}
    <button type="button" class="ui-button undo" onclick={() => onUndo?.()}>Undo</button>
  {/if}
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

  /* For screen readers only (UX review F8). */
  .hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* egui's small button. */
  .undo {
    flex: none;
    min-height: 24px;
    padding: 0 var(--space-2);
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
