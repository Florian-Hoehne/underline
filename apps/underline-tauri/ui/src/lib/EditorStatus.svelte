<!--
  M-11.4 (FR-026, UC-005): the status line under the editor, as the egui
  app has it (`editor_status_line`).

  - Left, with the Vim or Nano keys: a badge "{keymap} · {mode}" in the
    theme's colour of the mode (only the mode below 450 px), what is typed
    but not done yet, and the commands as key and action. Commands that do
    not fit are left out whole, the least important (last) first.
  - Right, always: the switches "Copy on select", "Soft wrap" (package 5,
    UC-010 A3, BR-011), "Focus mode" (A12, BR-010, UX review: it stays in
    the focus mode, so the mode can be seen and left) and "Minimap", the choice
    "Keys: {mode}", and the place of the cursor in a box of its own. Below
    720 px the switches have their short names ("Copy", "Wrap", "Focus", "Map") and the
    choice drops "Keys:".

  Package 7a (FR-096, review): in a pane narrower than 420 px (the split
  view) the four switches fold into one button "Switches", whose menu holds
  them, so the line stays one row.

  Every switch is stored at once (UC-010 A3). In a narrow editor the
  commands go first; then the right part moves to a second row, so the
  choice of the keys and the place stay in reach (A-28). The badge is a
  polite status for screen readers; the commands are not.
-->
<script lang="ts">
  import type { KeysStatusView } from './bindings';
  import ContextMenu, { menuPlace, type MenuEntry } from './ContextMenu.svelte';
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
    /** UC-010 BR-011: soft wrap is on. */
    softWrap: boolean;
    /** UC-010 BR-010: the focus mode is on. */
    focusMode?: boolean;
    /** The keys of soft wrap and the focus mode, as the platform names them and for `aria-keyshortcuts`. */
    softWrapKey?: { text: string; aria?: string };
    focusModeKey?: { text: string; aria?: string };
    minimap: boolean;
    onInputMode: (mode: string) => void;
    onCopyOnSelect: (on: boolean) => void;
    onSoftWrap: (on: boolean) => void;
    onFocusMode?: (on: boolean) => void;
    onMinimap: (on: boolean) => void;
  };

  let {
    keys,
    place,
    inputMode,
    inputModes,
    copyOnSelect,
    softWrap,
    focusMode = false,
    softWrapKey = { text: 'Alt+Z', aria: 'Alt+Z' },
    focusModeKey = { text: 'Ctrl+Alt+F', aria: 'Control+Alt+F' },
    minimap,
    onInputMode,
    onCopyOnSelect,
    onSoftWrap,
    onFocusMode = () => {},
    onMinimap,
  }: Props = $props();

  let width = $state(Infinity);
  let hintsBox = $state<HTMLElement | null>(null);
  /** How many commands fit; the rest is left out. */
  let fitting = $state(Infinity);

  const texts = $derived(statusTexts(width, keys));

  /** FR-096: below this width the switches fold into a menu. */
  const FOLD_BELOW = 420;
  const folded = $derived(width > 0 && width < FOLD_BELOW);
  let switchesMenu = $state<{ x: number; y: number } | null>(null);
  let switchesButton = $state<HTMLElement | null>(null);

  const switchEntries = $derived<MenuEntry[]>([
    { label: 'Copy on select', checked: copyOnSelect, action: () => onCopyOnSelect(!copyOnSelect) },
    {
      label: 'Soft wrap',
      shortcut: softWrapKey.text,
      checked: softWrap,
      action: () => onSoftWrap(!softWrap),
    },
    {
      label: 'Focus mode',
      shortcut: focusModeKey.text,
      checked: focusMode,
      action: () => onFocusMode(!focusMode),
    },
    { label: 'Minimap', checked: minimap, action: () => onMinimap(!minimap) },
  ]);

  function closeSwitches(back: boolean) {
    switchesMenu = null;
    if (back) {
      switchesButton?.focus();
    }
  }

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

<div
  class="editor-status"
  class:folded
  role="group"
  aria-label="Editor status"
  bind:clientWidth={width}
>
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
    {#if folded}
      <button
        type="button"
        class="switch"
        aria-haspopup="menu"
        aria-expanded={switchesMenu !== null}
        bind:this={switchesButton}
        onclick={(event) => (switchesMenu = menuPlace(event))}>Switches</button
      >
    {:else}
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
        aria-pressed={softWrap}
        title="Wrap long lines at the edge of the editor ({softWrapKey.text})"
        aria-keyshortcuts={softWrapKey.aria}
        onclick={() => onSoftWrap(!softWrap)}>{texts.wrap}</button
      >
      <button
        type="button"
        class="switch"
        aria-pressed={focusMode}
        title="Hide the note overview, the tool panel and the toolbars ({focusModeKey.text})"
        aria-keyshortcuts={focusModeKey.aria}
        onclick={() => onFocusMode(!focusMode)}>{texts.focus}</button
      >
      <button
        type="button"
        class="switch"
        aria-pressed={minimap}
        title="Show or hide the minimap beside the editor"
        onclick={() => onMinimap(!minimap)}>{texts.minimap}</button
      >
    {/if}
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
{#if switchesMenu}
  <ContextMenu
    label="Switches"
    entries={switchEntries}
    x={switchesMenu.x}
    y={switchesMenu.y}
    above
    onClose={closeSwitches}
  />
{/if}

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

  /* FR-096: one row in a narrow pane; the place gives way last. */
  .editor-status.folded,
  .folded .switches {
    flex-wrap: nowrap;
  }

  .folded .place {
    min-width: 8ch;
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
