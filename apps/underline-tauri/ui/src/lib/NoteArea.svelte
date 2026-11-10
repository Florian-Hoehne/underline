<!--
  The central area of an open workspace (UC-004): the tab bar on top and,
  below it, the active tab's note as the tab bar's `tabpanel`, with the tool
  panel on its right (M-16). Without an active tab it asks to select or
  create a note (M-07.1).

  Over the note, as in egui:
  - M-10.8: when its last save failed, "Not stored: {err}. Underline keeps
    trying." (Q-10: in the text colour, half bold, with a rule, as egui's
    red is in no palette);
  - M-07.2: the switch "Facade" / "Source". The facade comes with feature
    F5; until then the note shows in the source view and "Facade" is
    disabled. Choosing "Source" puts the keyboard into the text.

  The editing toolbar, the status line under the editor, the line numbers
  and the minimap come with feature F4.
-->
<script lang="ts">
  import { activeSourceView } from './editor/active';
  import SourceEditor from './SourceEditor.svelte';
  import TabBar from './TabBar.svelte';
  import ToolPanel from './ToolPanel.svelte';
  import type { Shell } from './shell.svelte';
  import { NOTE_PANEL_ID, tabId } from './tabs';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  /** ADR-024: a build that measures the source view (editor/bench.ts). */
  const MEASURING = import.meta.env.VITE_UL_BENCH === '1';

  const todosActive = $derived(shell.tabs.todoList.open && shell.tabs.todoList.active);
  const activeTab = $derived(
    todosActive ? undefined : shell.tabs.tabs.find((tab) => tab.note === shell.tabs.active),
  );
</script>

<section class="note-area">
  <TabBar {shell} />
  {#if activeTab}
    <div class="note">
      <div
        class="editor-column"
        role="tabpanel"
        id={NOTE_PANEL_ID}
        aria-labelledby={tabId(activeTab.note)}
        data-view="source"
      >
        {#if activeTab.saveError !== null}
          <p class="save-error" role="alert">
            Not stored: {activeTab.saveError}. Underline keeps trying.
          </p>
        {/if}
        <div class="views" role="group" aria-label="Editing view">
          <button type="button" class="view" aria-pressed="false" disabled>Facade</button>
          <button
            type="button"
            class="view"
            aria-pressed="true"
            onclick={() => activeSourceView()?.focus()}>Source</button
          >
        </div>
        <div class="editor">
          <!-- A measuring build (editor/bench.ts) shows its own source view. -->
          {#key activeTab.note}
            {#if !MEASURING}
              <SourceEditor
                note={activeTab.note}
                label={activeTab.title}
                mac={shell.platform === 'macos'}
                onError={(message) => shell.editorFailed(message)}
              />
            {/if}
          {/key}
        </div>
      </div>
      <ToolPanel {shell} />
    </div>
  {:else if !todosActive}
    <p class="hint">Select or create a note.</p>
  {/if}
</section>

<style>
  .note-area {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .note {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .editor-column {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .save-error {
    margin: var(--space-1) var(--space-2) 0;
    padding-left: var(--space-2);
    font-weight: 600;
    border-left: 3px solid var(--ui-text);
    overflow-wrap: anywhere;
  }

  .views {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
  }

  .view {
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .view[aria-pressed='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .view:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  .view:not(:disabled):hover {
    background: color-mix(in srgb, var(--ui-chosen) 40%, transparent);
  }

  .view[aria-pressed='true']:hover {
    background: var(--ui-chosen);
  }

  .view:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .editor {
    flex: 1 1 auto;
    min-height: 0;
  }

  .hint {
    margin: 0;
    padding: var(--space-2);
  }
</style>
