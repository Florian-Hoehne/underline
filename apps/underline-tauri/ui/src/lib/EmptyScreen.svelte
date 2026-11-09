<!--
  M-02.1, UC-002 A2: shown while no workspace is open. A third of the way
  down the central area, centred: the heading and, below it, the two ways to
  get a workspace.
-->
<script lang="ts">
  type Props = {
    onOpen?: () => void;
    onCreate?: () => void;
  };

  let { onOpen, onCreate }: Props = $props();
</script>

<section class="empty" aria-labelledby="empty-heading">
  <div class="third" aria-hidden="true"></div>
  <h1 id="empty-heading">No workspace is open</h1>
  <button type="button" class="ui-button" onclick={() => onOpen?.()}>Open workspace…</button>
  <button type="button" class="ui-button" onclick={() => onCreate?.()}>New workspace…</button>
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

  h1 {
    margin: 0 0 var(--space-1);
    font-size: 20px;
    font-weight: 700;
    line-height: 1.3;
  }
</style>
