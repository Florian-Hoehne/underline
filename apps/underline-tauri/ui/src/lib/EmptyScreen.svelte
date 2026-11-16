<!--
  M-02.1, UC-002 A2: shown while no workspace is open. A third of the way
  down the central area, centred: the heading and, below it, the two ways to
  get a workspace. Package 2 (UX review): under them, the command palette
  with its key (`palette`, from the registry), so it is found from the
  start.
-->
<script lang="ts">
  type Props = {
    onOpen?: () => void;
    onCreate?: () => void;
    /** "Command palette" and its key, such as "Ctrl+Shift+P". */
    palette?: { label: string; key: string };
    onPalette?: () => void;
  };

  let { onOpen, onCreate, palette, onPalette }: Props = $props();
</script>

<section class="empty" aria-labelledby="empty-heading">
  <div class="third" aria-hidden="true"></div>
  <h1 id="empty-heading">No workspace is open</h1>
  <button type="button" class="ui-button" onclick={() => onOpen?.()}>Open workspace…</button>
  <button type="button" class="ui-button" onclick={() => onCreate?.()}>New workspace…</button>
  {#if palette}
    <button type="button" class="palette-hint" data-palette-toggle onclick={() => onPalette?.()}>
      {palette.label}
      <kbd>{palette.key}</kbd>
    </button>
  {/if}
</section>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    height: 100%;
    padding-inline: var(--space-4);
    text-align: center;
  }

  /* The heading starts a third of the way down the central area. */
  .third {
    flex: 0 0 calc(100% / 3);
    margin-bottom: calc(-1 * var(--space-2));
  }

  /* Quieter than the two ways to a workspace. */
  .palette-hint {
    display: inline-flex;
    gap: var(--space-2);
    align-items: baseline;
    margin-top: var(--space-2);
    padding: 2px var(--space-2);
    font: inherit;
    color: var(--ui-hint);
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .palette-hint:hover {
    color: var(--ui-text);
  }

  .palette-hint:focus-visible {
    outline: 2px solid var(--ui-focus);
  }

  kbd {
    padding: 0 var(--space-1);
    font: inherit;
    font-size: 0.85em;
    border: 1px solid currentColor;
    border-radius: var(--radius);
  }

  h1 {
    margin: 0 0 var(--space-1);
    font-size: 20px;
    font-weight: 700;
    line-height: 1.3;
  }
</style>
