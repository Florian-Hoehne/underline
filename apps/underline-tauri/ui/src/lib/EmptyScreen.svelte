<!--
  M-02.1, UC-002 A2: shown while no workspace is open. A third of the way
  down, centred: the heading and the two ways to get a workspace.
-->
<script lang="ts">
  type Props = {
    onOpen?: () => void;
    onCreate?: () => void;
  };

  let { onOpen, onCreate }: Props = $props();
</script>

<section class="empty" aria-labelledby="empty-heading">
  <h1 id="empty-heading">No workspace is open</h1>
  <div class="actions">
    <button type="button" onclick={() => onOpen?.()}>Open workspace…</button>
    <button type="button" onclick={() => onCreate?.()}>New workspace…</button>
  </div>
</section>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding-top: 33vh;
    padding-inline: 1rem;
    text-align: center;
  }

  h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.5rem;
  }

  button {
    min-height: 2rem;
    padding: 0.25rem 0.875rem;
    font: inherit;
    color: var(--button-text);
    background: var(--button);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
  }

  button:hover {
    background: var(--button-hover);
  }

  button:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
</style>
