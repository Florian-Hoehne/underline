<!--
  A dialog window as the egui app shows them: not modal, so the window
  behind stays usable (inventory conventions), with its title on top.

  Escape closes it (inventory D-02, fixed in the Tauri app). When it shows,
  and whenever `step` changes, the keyboard goes to the element marked
  `data-autofocus` (its text selected with `data-select`), else to the
  dialog itself, so Tab continues inside it. When it closes with the
  keyboard inside, the keyboard goes back to where it was before, or, when
  that is gone, to the note overview (`focusFallback`).
-->
<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';
  import { focusFallback } from './focus';

  type Props = {
    /** The window's title, such as "Open workspace". */
    title: string;
    /** Cancel or Escape. */
    onClose: () => void;
    /** The dialog waits for the backend. */
    busy?: boolean;
    /** Changes with every stage of the dialog, which takes the focus anew. */
    step?: string;
    children: Snippet;
  };

  let { title, onClose, busy = false, step = '', children }: Props = $props();

  let frame = $state<HTMLElement | null>(null);
  const headingId = `dialog-${Math.random().toString(36).slice(2)}`;

  async function takeFocus() {
    await tick();
    const target = frame?.querySelector<HTMLElement>('[data-autofocus="true"]') ?? frame;
    target?.focus();
    if (target instanceof HTMLInputElement && target.dataset.select === 'true') {
      target.select();
    }
  }

  $effect(() => {
    void step;
    void takeFocus();
  });

  onMount(() => {
    const before = document.activeElement as HTMLElement | null;
    return () => {
      const inside = frame?.contains(document.activeElement) ?? false;
      if (!inside && document.activeElement !== document.body) {
        return;
      }
      if (before?.isConnected && before !== document.body) {
        before.focus();
      } else {
        // After the dialog the window behind is new, such as an opened
        // workspace in place of the empty screen.
        void tick().then(focusFallback);
      }
    };
  });

  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  }
</script>

<div
  class="dialog"
  role="dialog"
  aria-labelledby={headingId}
  aria-busy={busy}
  tabindex="-1"
  bind:this={frame}
  onkeydown={onKey}
>
  <h2 id={headingId}>{title}</h2>
  <div class="body">
    {@render children()}
  </div>
</div>

<style>
  .dialog {
    position: absolute;
    top: var(--space-4);
    left: 50%;
    z-index: 20;
    box-sizing: border-box;
    width: max-content;
    min-width: min(320px, calc(100% - 2 * var(--space-4)));
    max-width: min(640px, calc(100% - 2 * var(--space-4)));
    display: flex;
    flex-direction: column;
    max-height: calc(100% - 2 * var(--space-4));
    overflow: hidden;
    transform: translateX(-50%);
    color: var(--ui-text);
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
  }

  .dialog:focus {
    outline: none;
  }

  .dialog:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  h2 {
    flex: none;
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font-size: 14px;
    font-weight: 700;
    border-bottom: 1px solid var(--ui-rule);
  }

  /* In a small window the dialog scrolls inside, under its title. */
  .body {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
  }

  /* Shared by the dialogs' contents. */
  .body :global(p) {
    margin: 0;
  }

  .body :global(input[type='text']) {
    box-sizing: border-box;
    width: 100%;
    min-height: 28px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-field);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  /* Q-05: egui shows problems in its own orange, which no palette holds;
     here they stay in the text colour, set apart by a rule. */
  .body :global(.problem) {
    padding-left: var(--space-2);
    border-left: 3px solid var(--ui-hint);
    overflow-wrap: anywhere;
  }

  .body :global(.actions) {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .body :global(.ui-button) {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body :global(.ui-button:disabled) {
    cursor: progress;
  }
</style>
