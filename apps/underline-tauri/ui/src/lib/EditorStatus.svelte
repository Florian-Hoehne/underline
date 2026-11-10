<!--
  M-11.4 (FR-026, UC-005): the status line under the editor, as the egui
  app has it (`editor_status_line`).

  - Left, with the Vim or Nano keys: a badge "{keymap} · {mode}" in the
    theme's colour of the mode (only the mode below 450 px), what is typed
    but not done yet, and the commands as key and action. Commands that do
    not fit are left out whole, the least important (last) first.
  - Right, always: the switches "Copy on select" and "Minimap", the choice
    "Keys: {mode}", and the place of the cursor in a box of its own. Below
    720 px the switches have their short names ("Copy", "Map") and the
    choice drops "Keys:".

  Every switch is stored at once (UC-010 A3). In a narrow editor the
  commands go first; then the right part moves to a second row, so the
  choice of the keys and the place stay in reach (A-28). The badge is a
  polite status for screen readers; the commands are not.
-->
<script lang="ts">
  import type { KeysStatusView } from './bindings';
  import { statusTexts } from './editor/status';

  type Props = {
    /** The status of the Vim or Nano keys; `null` with the usual keys. */
    keys: KeysStatusView | null;
    /** "Ln 3, Col 5 · 4 chars". */
    place: string;
    /** The input mode in effect and the ones to choose from. */
    inputMode: string;
    inputModes: readonly string[];
    copyOnSelect: boolean;
    minimap: boolean;
    onInputMode: (mode: string) => void;
    onCopyOnSelect: (on: boolean) => void;
    onMinimap: (on: boolean) => void;
  };

  let {
    keys,
    place,
    inputMode,
    inputModes,
    copyOnSelect,
    minimap,
    onInputMode,
    onCopyOnSelect,
    onMinimap,
  }: Props = $props();

  let width = $state(Infinity);
  let hintsBox = $state<HTMLElement | null>(null);
  /** How many commands fit; the rest is left out. */
  let fitting = $state(Infinity);

  const texts = $derived(statusTexts(width, keys));

  // The commands that fit on the line; laid out in one wrapping row, those
  // that wrap below it are left out (and hidden from screen readers).
  $effect(() => {
    const box = hintsBox;
    void keys;
    if (!box) {
      return;
    }
    const measure = () => {
      const items = [...box.querySelectorAll<HTMLElement>('.hint')];
      const first = items[0]?.offsetTop ?? 0;
      const count = items.findIndex((item) => item.offsetTop > first);
      fitting = count === -1 ? items.length : count;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(box);
    return () => observer.disconnect();
  });
</script>

<div class="editor-status" role="group" aria-label="Editor status" bind:clientWidth={width}>
  {#if keys}
    <span class="mode-status" role="status" aria-live="polite">
      <span
        class="badge"
        style:background="var(--palette-mode-colors-{keys.color})"
        style:color="var(--palette-mode-text-{keys.color})">{texts.badge}</span
      >
    </span>
    <div class="keys" style:--accent="var(--palette-mode-colors-{keys.color})">
      {#if keys.pending !== ''}
        <span class="pending">{keys.pending}</span>
      {/if}
      <div class="hints" bind:this={hintsBox}>
        {#each keys.hints as hint, index (hint.keys + hint.action)}
          <span class="hint" class:first={index === 0} aria-hidden={index >= fitting}>
            <kbd>{hint.keys}</kbd>
            <span class="action">{hint.action}</span>
          </span>
        {/each}
      </div>
    </div>
  {:else}
    <div class="keys"></div>
  {/if}
  <div class="switches">
    <button
      type="button"
      class="switch"
      aria-pressed={copyOnSelect}
      title="Put a selection on the clipboard as soon as it is made"
      onclick={() => onCopyOnSelect(!copyOnSelect)}>{texts.copy}</button
    >
    <button
      type="button"
      class="switch"
      aria-pressed={minimap}
      title="Show or hide the minimap beside the editor"
      onclick={() => onMinimap(!minimap)}>{texts.minimap}</button
    >
    <span class="separator" aria-hidden="true"></span>
    <label class="mode">
      {#if texts.keysLabel}<span>{texts.keysLabel}</span>{/if}
      <select
        value={inputMode}
        aria-label={texts.keysLabel ? undefined : 'Keys'}
        onchange={(event) => onInputMode(event.currentTarget.value)}
      >
        {#each inputModes as mode (mode)}
          <option value={mode}>{mode}</option>
        {/each}
      </select>
    </label>
    <span class="separator" aria-hidden="true"></span>
    <span class="place">{place}</span>
  </div>
</div>

<style>
  .editor-status {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px var(--space-2);
    min-height: 28px;
    padding: 2px var(--space-2);
    white-space: nowrap;
  }

  .mode-status {
    display: contents;
  }

  .badge {
    flex: 0 0 auto;
    padding: 2px 7px;
    font-size: 12px;
    line-height: 1.3;
    border-radius: 999px;
  }

  .keys {
    display: flex;
    flex: 1 1 0;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    overflow: hidden;
  }

  /* What is typed stays readable; only the commands give way. */
  .pending {
    flex: 0 0 auto;
    max-width: 40%;
    overflow: hidden;
    font-family: var(--font-mono);
    color: var(--accent);
    text-overflow: ellipsis;
  }

  /* One row; a command that does not fit wraps below it and is cut off. */
  .hints {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    column-gap: var(--space-4);
    min-width: 0;
    height: 1.4em;
    overflow: hidden;
  }

  .hint.first {
    padding-left: var(--space-2);
    border-left: 1px solid var(--ui-rule);
  }

  kbd {
    font-family: var(--font-mono);
    color: var(--accent);
  }

  .action {
    margin-left: 6px;
    color: var(--ui-hint);
  }

  .switches {
    display: flex;
    flex: 0 1 auto;
    flex-wrap: wrap;
    align-items: center;
    margin-left: auto;
    gap: var(--space-1);
    min-width: 0;
  }

  .switch {
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .switch:hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  /* Pressed shows by a line too, not only by colour (NFR-010). */
  .switch[aria-pressed='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    border-color: var(--ui-chosen-text);
  }

  .switch:focus-visible,
  select:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .separator {
    align-self: stretch;
    width: 1px;
    margin: 2px var(--space-1);
    background: var(--ui-rule);
  }

  .mode {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  select {
    min-height: 24px;
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
  }

  .place {
    min-width: 14ch;
    overflow: hidden;
    padding: 1px var(--space-2);
    font-family: var(--font-mono);
    color: var(--palette-foreground);
    text-overflow: ellipsis;
    background: var(--palette-inset);
    border-radius: var(--radius);
  }
</style>
