<!--
  M-04.7 (FR-020): the status bar's text: the workspace's root path and the
  status message, joined by "  |  " as in the egui app. The branch, sync,
  agents and todo parts come with their features (M-04.1 to M-04.10).

  UC-007 A4 (M-14.3): after "Deleted {t}" the bar offers "Undo", as long as
  that report shows.

  UC-009 step 5, A2 (M-04.8): after "Exported {t} to {p}" the bar offers
  "Open" (the system's program) and, when blocks were only approximated,
  "Show" ("Approximated blocks"), right of the text as in egui, as long as
  that report shows. Reports of exports that ended together stand on lines
  of their own, and their buttons name the note ("Open {t}", as egui's
  "Open {title}" after agent changes, A-120). A failed export is marked
  as an error by a sign and the word "Error:" for screen readers, not by
  colour alone; while an export runs the bar is busy (review F11).
-->
<script lang="ts">
  import type { ExportOfferView, StatusOffer } from './bindings';

  type Props = {
    /** The root of the open workspace, if one is open. */
    root: string | null;
    /** The status message; empty when there is none. */
    status: string;
    /** What the bar offers with the message. */
    offer?: StatusOffer | null;
    /** "Open" and "Show" for each export the message reports. */
    exports?: ExportOfferView[];
    /** The message reports a failed export. */
    error?: boolean;
    /** An export is running. */
    busy?: boolean;
    /** "Undo" of a deletion. */
    onUndo?: () => void;
    /** "Open" of the export `index`. */
    onOpenExport?: (index: number) => void;
    /** "Show" of the approximated blocks of the export `index`. */
    onShowApproximated?: (index: number) => void;
  };

  let {
    root,
    status,
    offer = null,
    exports = [],
    error = false,
    busy = false,
    onUndo,
    onOpenExport,
    onShowApproximated,
  }: Props = $props();

  /** For screen readers before a failure (new text, A-118). */
  const ERROR_PREFIX = 'Error: ';

  const several = $derived(exports.length > 1);
  const opens = $derived(exports.some((e) => e.open));
  const shows = $derived(exports.some((e) => e.show));
  /** For screen readers, after the text (new, as "Undo available", A-83). */
  const offered = $derived(
    offer === 'undoDelete'
      ? '. Undo available'
      : opens && shows
        ? '. Open and Show available'
        : opens
          ? '. Open available'
          : shows
            ? '. Show available'
            : '',
  );

  const text = $derived([root ?? '', status].filter((part) => part !== '').join('  |  '));
</script>

<footer class="status-bar" aria-busy={busy}>
  {#if error}
    <span class="sign" aria-hidden="true">⚠</span>
  {/if}
  <p class="text" role={error ? 'alert' : 'status'} title={text}>
    {#if error}<span class="hidden">{ERROR_PREFIX}</span>{/if}{text}{#if offered}<span
        class="hidden">{offered}</span
      >{/if}
  </p>
  {#if offer === 'undoDelete'}
    <button type="button" class="ui-button undo" onclick={() => onUndo?.()}>Undo</button>
  {/if}
  {#each exports as report, index (index)}
    {#if report.open}
      {@const label = several ? `Open exported file ${report.title}` : 'Open exported file'}
      <button
        type="button"
        class="ui-button undo"
        aria-label={label}
        title={label}
        onclick={() => onOpenExport?.(index)}>{several ? `Open ${report.title}` : 'Open'}</button
      >
    {/if}
    {#if report.show}
      {@const label = several
        ? `Show approximated blocks of ${report.title}`
        : 'Show approximated blocks'}
      <button
        type="button"
        class="ui-button undo"
        aria-label={label}
        title={label}
        onclick={() => onShowApproximated?.(index)}
        >{several ? `Show ${report.title}` : 'Show'}</button
      >
    {/if}
  {/each}
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

  /* UC-009 A4: the sign of a failed export, in the text colour. */
  .sign {
    flex: none;
    font-weight: 700;
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
