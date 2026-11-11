<!--
  A dialog window as the egui app shows them: not modal, so the window
  behind stays usable (inventory conventions), with its title on top.

  Escape closes it (inventory D-02, fixed in the Tauri app), and Tab and
  Shift+Tab stay inside it (ux review F3). When it shows,
  and whenever `step` changes, the keyboard goes to the element marked
  `data-autofocus` (its text selected with `data-select`), else to the
  dialog itself, so Tab continues inside it. When it closes with the
  keyboard inside, the keyboard goes back to where it was before, or, when
  that is gone, to the note overview (`focusFallback`).

  A window that stays open while the user works (the settings, M-20.1)
  gives `width`, may be `resizable`, and shows a close button "×" beside its
  title, as egui's windows do. One anchored at the bottom left (the branch
  menu, M-19.6) stands above the status bar there, as egui anchors it.
-->
<script lang="ts" module>
  /** The event on `window` when a dialog shows. */
  export const DIALOG_OPENED = 'ul-dialog-opened';
</script>

<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';
  import { focusFallback, setFocusHint } from './focus';

  type Props = {
    /** The window's title, such as "Open workspace". */
    title: string;
    /** Cancel or Escape. */
    onClose: () => void;
    /** The dialog waits for the backend. */
    busy?: boolean;
    /** Changes with every stage of the dialog, which takes the focus anew. */
    step?: string;
    /** The id of the text that describes the dialog, for screen readers. */
    describedBy?: string;
    /** The width it opens with, in pixels; else as wide as its content. */
    width?: number;
    /** The user can change its size with the corner (M-20.1). */
    resizable?: boolean;
    /** A close button "×" beside the title. */
    closeButton?: boolean;
    /** Where it stands: centred at the top, or at the bottom left. */
    anchor?: 'top' | 'bottom-left';
    children: Snippet;
  };

  let {
    title,
    onClose,
    busy = false,
    step = '',
    describedBy,
    width,
    resizable = false,
    closeButton = false,
    anchor = 'top',
    children,
  }: Props = $props();

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

  // Only a new step takes the focus anew: `step` is a getter over the
  // caller's state, which every edit of a field replaces, so it is read
  // through a derived (equal values stop there) and compared with the last
  // one (app-tester F8: the rename's title was selected again on every key).
  const current = $derived(step);
  let lastStep: string | null = null;

  $effect(() => {
    const now = current;
    if (now === lastStep) {
      return;
    }
    lastStep = now;
    void takeFocus();
  });

  /**
   * A dialog with a width is centred where it opens, and again when the
   * window changes size, measured at the left edge so a narrower window
   * does not squeeze it (app-tester F10: 1200 → 500 px left it 114 px wide).
   */
  function place() {
    const parent = frame?.offsetParent as HTMLElement | null;
    if (width === undefined || !frame || !parent) {
      return;
    }
    frame.style.setProperty('--dialog-left', '16px');
    const left = Math.max(16, (parent.clientWidth - frame.offsetWidth) / 2);
    frame.style.setProperty('--dialog-left', `${Math.round(left)}px`);
  }

  onMount(() => {
    place();
    window.addEventListener('resize', place);
    const before = document.activeElement as HTMLElement | null;
    // Hints shown at the pointer go (app-tester F8: a link's hint stayed
    // above "New note").
    window.dispatchEvent(new Event(DIALOG_OPENED));
    return () => {
      window.removeEventListener('resize', place);
      const inside = frame?.contains(document.activeElement) ?? false;
      if (!inside && document.activeElement !== document.body) {
        return;
      }
      if (before?.isConnected && before !== document.body) {
        // A place asked for after the dialog is not needed any more.
        setFocusHint(null);
        before.focus();
      } else {
        // After the dialog the window behind is new, such as an opened
        // workspace in place of the empty screen.
        void tick().then(focusFallback);
      }
    };
  });

  const FOCUSABLE =
    'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

  /**
   * Escape closes the dialog, also with the keyboard on its frame; Tab and
   * Shift+Tab go round inside it (ux review F3).
   */
  function onKey(event: KeyboardEvent) {
    // A control inside took the key for itself (an open list, F10).
    if (event.defaultPrevented) {
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab' || !frame) {
      return;
    }
    const inside = Array.from(frame.querySelectorAll<HTMLElement>(FOCUSABLE));
    const first = inside[0];
    const last = inside[inside.length - 1];
    const active = document.activeElement;
    if (!first || !last) {
      event.preventDefault();
      frame.focus();
    } else if (event.shiftKey && (active === first || active === frame)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<div
  class="dialog"
  class:sized={width !== undefined}
  class:resizable
  class:bottom-left={anchor === 'bottom-left'}
  style:--dialog-width={width === undefined ? undefined : `${width}px`}
  role="dialog"
  aria-labelledby={headingId}
  aria-describedby={describedBy}
  aria-busy={busy}
  tabindex="-1"
  bind:this={frame}
  onkeydown={onKey}
>
  <div class="title">
    <h2 id={headingId}>{title}</h2>
    {#if closeButton}
      <button type="button" class="close" aria-label="Close" title="Close" onclick={onClose}
        >×</button
      >
    {/if}
  </div>
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

  /* M-20.1: at least its width and as wide as its content, inside the
     window; centred once when it opens (`place`), not by a transform, so
     the resize corner follows the pointer. */
  .dialog.sized {
    left: var(--dialog-left, var(--space-4));
    width: max-content;
    min-width: min(var(--dialog-width), calc(100% - var(--dialog-left, 0px) - var(--space-4)));
    max-width: calc(100% - var(--dialog-left, var(--space-4)) - var(--space-4));
    transform: none;
  }

  /* M-19.6: egui's branch menu, anchored at the bottom left. */
  .dialog.bottom-left {
    top: auto;
    bottom: var(--space-2);
    left: var(--space-2);
    transform: none;
  }

  .dialog.resizable {
    min-height: 160px;
    resize: both;
  }

  .dialog:focus {
    outline: none;
  }

  .dialog:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .title {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-1) 0 0;
    border-bottom: 1px solid var(--ui-rule);
  }

  h2 {
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
    padding: var(--space-2) var(--space-3);
    font-size: 14px;
    font-weight: 700;
  }

  .close {
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    font: inherit;
    font-size: 18px;
    line-height: 1;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .close:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
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
