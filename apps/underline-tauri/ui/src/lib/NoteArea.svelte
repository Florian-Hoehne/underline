<!--
  The central area of an open workspace (UC-004): the tab bar on top and,
  below it, the active tab's note as the tab bar's `tabpanel`, with the tool
  panel on its right (M-16). Without an active tab it asks to select or
  create a note (M-07.1).

  Over the note, as in egui:
  - M-10.8: when its last save failed, "Not stored: {err}. Underline keeps
    trying." (Q-10: in the text colour, half bold, with a rule, as egui's
    red is in no palette);
  - M-07.2: the switch "Facade" / "Source". The choice is the tab's and is
    remembered with the tabs; the cursor and the selection are the note's,
    so they stay. Choosing a view puts the keyboard into it.

  Feature F4 adds, as in egui:
  - M-10.2: the formatting toolbar after the switch, one button per
    function with its hint; a button puts the keyboard back into the text.
    Link and Image ask first; their dialogs come with feature F6.
  - M-12: the minimap beside the source view or the facade (F5a), unless
    switched off or the editor is narrower than 240 px.
  - M-11.4: the status line under the editor.
-->
<script lang="ts">
  import type { EditingViewName, EditorCommandName, ToolName } from './bindings';
  import FacadeView from './facade/FacadeView.svelte';
  import EditorStatus from './EditorStatus.svelte';
  import { activeSourceView } from './editor/active';
  import { nextRegion } from './focus';
  import { EditorInfo } from './editor/info.svelte';
  import type { InputModeName } from './editor/keys';
  import { GAP, minimapShown } from './editor/minimap';
  import type { LineNumberSide } from './editor/numbers';
  import Minimap from './Minimap.svelte';
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

  /** Q-14: the functions that ask first, without their dialogs until F6. */
  const ASKS_FIRST: ReadonlySet<ToolName> = new Set(['link', 'image']);

  /** What the status line and the minimap show of the source view. */
  const info = new EditorInfo();
  const settings = $derived(shell.settings);
  const tools = $derived(shell.start?.tools ?? []);
  let editorWidth = $state(0);
  const withMinimap = $derived(minimapShown(settings?.showMinimap ?? false, editorWidth));

  let area = $state<HTMLElement | null>(null);

  /** The view the user just chose (`note:view`); it takes the keyboard once shown. */
  let focusFor = $state<string | null>(null);

  /** M-07.2: the switch "Facade"/"Source"; the view shown takes the keyboard. */
  async function chooseView(view: EditingViewName) {
    const tab = activeTab;
    if (!tab) {
      return;
    }
    if (tab.view === view) {
      if (view === 'facade') {
        info.facade?.focus();
      } else {
        activeSourceView()?.focus();
      }
      return;
    }
    // The inputs and the selection of the view shown reach the backend first.
    await activeSourceView()?.settled();
    await info.facade?.settled();
    focusFor = `${tab.note}:${view}`;
    await shell.setView(tab.note, view);
  }

  /** M-10.2: a toolbar button acts in the view shown. */
  function useTool(tool: ToolName) {
    if (activeTab?.view === 'facade') {
      info.facade?.tool(tool);
    } else {
      activeSourceView()?.tool(tool);
    }
  }

  /**
   * NFR-011, A-33: F6 and Shift+F6 go from region to region: the tab bar,
   * the switch and toolbar, the text, the status line.
   */
  function regionKey(event: KeyboardEvent) {
    if (event.key !== 'F6' || event.ctrlKey || event.altKey || event.metaKey || !area) {
      return;
    }
    const active = document.activeElement;
    if (!active || !area.contains(active)) {
      return;
    }
    const regions: [HTMLElement | null, HTMLElement | null][] = [
      [
        area.querySelector<HTMLElement>('[role="tablist"]'),
        area.querySelector<HTMLElement>('[role="tab"][tabindex="0"]'),
      ],
      [
        area.querySelector<HTMLElement>('.views'),
        area.querySelector<HTMLElement>('.views button:not(:disabled)'),
      ],
      [
        area.querySelector<HTMLElement>('.editor'),
        area.querySelector<HTMLElement>('.cm-content, .facade-text'),
      ],
      [
        area.querySelector<HTMLElement>('.editor-status'),
        area.querySelector<HTMLElement>('.editor-status button'),
      ],
    ];
    const present = regions.filter(([box, target]) => box && target);
    const current = present.findIndex(([box]) => box!.contains(active));
    const next = nextRegion(present.length, current, event.shiftKey);
    if (next >= 0) {
      event.preventDefault();
      present[next][1]!.focus();
    }
  }

  /** M-10.9: the Vim or Nano keys store the note or close its tab. */
  function command(note: string, command: EditorCommandName) {
    if (command === 'store') {
      void shell.storeTab(note);
    } else {
      void shell.closeTab(note);
    }
  }
</script>

<svelte:window onkeydown={regionKey} />

<section class="note-area" bind:this={area}>
  <TabBar {shell} />
  {#if activeTab}
    <div class="note">
      <div
        class="editor-column"
        role="tabpanel"
        id={NOTE_PANEL_ID}
        aria-labelledby={tabId(activeTab.note)}
        data-view={activeTab.view}
      >
        {#if activeTab.saveError !== null}
          <p class="save-error" role="alert">
            Not stored: {activeTab.saveError}. Underline keeps trying.
          </p>
        {/if}
        <div class="views" role="group" aria-label="Editing view">
          <button
            type="button"
            class="view"
            aria-pressed={activeTab.view === 'facade'}
            onclick={() => chooseView('facade')}>Facade</button
          >
          <button
            type="button"
            class="view"
            aria-pressed={activeTab.view === 'source'}
            onclick={() => chooseView('source')}>Source</button
          >
          {#if tools.length > 0}
            <span class="separator" aria-hidden="true"></span>
            <div class="tools" role="group" aria-label="Formatting">
              {#each tools as tool (tool.tool)}
                <!-- Q-14: Link and Image ask first; their dialogs come with F6. -->
                <button
                  type="button"
                  class="tool"
                  title={tool.hint}
                  aria-label={tool.hint}
                  disabled={ASKS_FIRST.has(tool.tool)}
                  onclick={() => useTool(tool.tool)}>{tool.label}</button
                >
              {/each}
            </div>
          {/if}
        </div>
        <div class="editor" bind:clientWidth={editorWidth} style:gap="{GAP}px">
          <!-- A measuring build (editor/bench.ts) shows its own source view. -->
          <div class="text">
            {#key `${activeTab.note}:${activeTab.view}`}
              {#if MEASURING}
                <!-- The measuring build shows its own source view. -->
              {:else if activeTab.view === 'facade'}
                <FacadeView
                  note={activeTab.note}
                  label={activeTab.title}
                  platform={shell.platform}
                  mode={(settings?.inputMode ?? 'Normal') as InputModeName}
                  focus={focusFor === `${activeTab.note}:facade`}
                  {info}
                  onError={(message) => shell.editorFailed(message)}
                  onCommand={(name) => command(activeTab.note, name)}
                />
              {:else}
                <SourceEditor
                  note={activeTab.note}
                  label={activeTab.title}
                  mac={shell.platform === 'macos'}
                  mode={(settings?.inputMode ?? 'Normal') as InputModeName}
                  lineNumbers={settings?.showLineNumbers ?? false}
                  lineNumberSide={(settings?.lineNumberSide ?? 'Left') as LineNumberSide}
                  {info}
                  focus={focusFor === `${activeTab.note}:source`}
                  onError={(message) => shell.editorFailed(message)}
                  onCommand={(name) => command(activeTab.note, name)}
                />
              {/if}
            {/key}
          </div>
          {#if withMinimap}
            <Minimap
              view={info.minimap}
              palette={settings?.palette ?? null}
              blink={settings?.blinkFoundInMinimap ?? true}
            />
          {/if}
        </div>
        <EditorStatus
          keys={info.keys}
          place={info.place}
          inputMode={settings?.inputMode ?? 'Normal'}
          inputModes={shell.start?.inputModes ?? ['Normal', 'Vim', 'Nano']}
          copyOnSelect={settings?.copyOnSelect ?? false}
          minimap={settings?.showMinimap ?? false}
          onInputMode={(mode) => shell.quickSetting('input_mode', mode)}
          onCopyOnSelect={(on) => shell.quickSetting('copy_on_select', String(on))}
          onMinimap={(on) => shell.quickSetting('show_minimap', String(on))}
        />
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
    flex: 0 0 auto;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
    /* One row however narrow the editor: it scrolls instead of wrapping (A-46). */
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .views > * {
    flex: 0 0 auto;
  }

  .view {
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  /* Pressed shows by a line too, not only by colour (NFR-010). */
  .view[aria-pressed='true'] {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
    border-color: var(--ui-chosen-text);
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

  .separator {
    align-self: stretch;
    width: 1px;
    margin: 2px var(--space-1);
    background: var(--ui-rule);
  }

  .tools {
    display: flex;
    gap: var(--space-1);
  }

  /* egui's buttons of the toolbar: framed, compact. */
  .tool {
    min-width: 28px;
    min-height: 24px;
    padding: 0 var(--space-2);
    font: inherit;
    color: var(--ui-text);
    background: var(--ui-button);
    border: 1px solid var(--ui-border);
    border-radius: var(--radius);
    cursor: pointer;
  }

  .tool:disabled {
    color: var(--ui-hint);
    cursor: default;
  }

  .tool:not(:disabled):hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .tool:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: 1px;
  }

  .editor {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .text {
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  .hint {
    margin: 0;
    padding: var(--space-2);
  }
</style>
