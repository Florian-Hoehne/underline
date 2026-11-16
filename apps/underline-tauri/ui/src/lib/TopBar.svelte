<!--
  M-03.1 to M-03.3: the top bar. The burger button shows or hides the menu
  bar (stored like a quick switch, UC-010 A3); the magnifier switches the
  search overlay (UC-008 step 1, M-03.2): opening puts the keyboard into its
  field, closing empties it. With the menu bar hidden, the name of the
  workspace stands in its place.

  Package 2 (UX review): after the magnifier, a button opens or closes the
  command palette (UC-023); its hint names the palette's and go to note's
  keys from the registry.

  Package 6b (UC-025 step 1): a calendar button opens the daily note of
  today; its hint names the command and its key from the registry. Without
  a workspace it is disabled.
-->
<script lang="ts">
  import { COMMAND } from './commandList';
  import { ariaKeys, keyText } from './commands';
  import MenuBar from './MenuBar.svelte';
  import type { Shell } from './shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const shown = $derived(shell.settings?.showMenuBar ?? false);
  /** UC-023 BR-003: the name and key of the command, from the registry. */
  const searchHint = $derived.by(() => {
    const command = shell.commandById('search.notes');
    return `${command?.title ?? 'Search notes'}  ${keyText(command?.key, shell.platform === 'macos')}`;
  });

  /** M-03.3: the folder name of the workspace, as the backend names it. */
  const workspaceName = $derived(shell.workspace?.name ?? 'No workspace open');

  const mac = $derived(shell.platform === 'macos');
  const paletteCommand = $derived(shell.commandById(COMMAND.palette));

  /** "Command palette  Ctrl+Shift+P", and go to note under it, from the registry. */
  const paletteHint = $derived.by(() => {
    const goTo = shell.commandById(COMMAND.goToNote);
    return [paletteCommand, goTo]
      .filter((command) => command !== undefined)
      .map((command) => `${command.title.replace(/…$/, '')}  ${keyText(command.key, mac)}`)
      .join('\n');
  });

  /** UC-025 step 1: "Daily note  Ctrl+Shift+J", from the registry, as the palette button's hint. */
  const dailyCommand = $derived(shell.commandById(COMMAND.dailyNote));
  const dailyHint = $derived(
    `${dailyCommand?.title ?? 'Daily note'}  ${keyText(dailyCommand?.key, mac)}`,
  );

  /** The button opens the command palette, or closes it. */
  function togglePalette() {
    if (shell.palette.open) {
      shell.palette.close();
    } else {
      shell.palette.show('commands');
    }
  }

  function toggleSearch() {
    shell.search.toggle();
  }
</script>

<header class="top-bar">
  <button
    type="button"
    class="icon"
    aria-label="Menu"
    title={shown ? 'Hide the menu' : 'Show the menu'}
    aria-expanded={shown}
    onclick={() => shell.toggleMenuBar()}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M3 4.5h10M3 8h10M3 11.5h10" />
    </svg>
  </button>
  <button
    type="button"
    class="icon"
    aria-label="Search"
    title={searchHint}
    aria-expanded={shell.search.open}
    data-search-toggle
    onclick={toggleSearch}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="6.75" cy="6.75" r="3.75" />
      <path d="M9.5 9.5l3.5 3.5" />
    </svg>
  </button>
  <!-- UX review: the command palette, found without knowing its key. -->
  <button
    type="button"
    class="icon"
    aria-label="Command palette"
    aria-keyshortcuts={ariaKeys(paletteCommand?.key, mac)}
    title={paletteHint}
    aria-expanded={shell.palette.open}
    data-palette-toggle
    onclick={togglePalette}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M4 5l3 3-3 3M8.5 11.5h4" />
    </svg>
  </button>
  <button
    type="button"
    class="icon"
    aria-label={dailyCommand?.title ?? 'Daily note'}
    aria-keyshortcuts={ariaKeys(dailyCommand?.key, mac)}
    title={dailyHint}
    disabled={shell.workspace === null}
    data-daily-note
    onclick={() => void shell.openDailyNote()}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <rect x="2.5" y="3.5" width="11" height="10" rx="1.5" />
      <path d="M2.5 6.5h11M5.5 2v3M10.5 2v3" />
      <circle class="dot" cx="8" cy="10" r="1" />
    </svg>
  </button>
  {#if shown}
    <MenuBar {shell} />
  {:else}
    <strong class="workspace-name" title={workspaceName}>{workspaceName}</strong>
  {/if}
</header>

<style>
  .top-bar {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
    padding: var(--space-1) var(--space-2);
    border-bottom: 1px solid var(--ui-rule);
  }

  .icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .icon:hover:not(:disabled) {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .icon:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .icon svg .dot {
    fill: currentColor;
    stroke: none;
  }

  .icon svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
  }

  .workspace-name {
    min-width: 0;
    margin-left: var(--space-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
