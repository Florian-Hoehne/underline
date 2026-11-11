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
    Link and Image ask first (feature F6).
  - M-12: the minimap beside the source view or the facade (F5a), unless
    switched off or the editor is narrower than 240 px.
  - M-11.4: the status line under the editor.

  Feature F6 adds, as in egui:
  - M-10.3, M-10.4: the dialogs "Insert link" and "Insert image" of Link
    and Image (editor/inserting.svelte.ts), above the central area, not
    modal; their answer goes in at the cursor of the note shown.
  - M-10.5: files dropped on the window go into the note shown, at the
    place under the pointer (editor/drop.ts).
  - M-10.7: copy on select follows the setting (editor/clipboard.ts).

  Feature F7 adds, as in egui:
  - M-07.3: a Ctrl+click (Cmd+click) on a link in either view follows it
    (`shell.followLink`).

  Feature F9 adds, as in egui:
  - M-13.4 (UC-008 A6): Ctrl+F (Cmd+F) opens the find field above the note,
    only with the input mode Normal (BR-009: Vim and Nano keep Ctrl+F),
    holding the selection when it is within one line, else the text it
    held while open. Its steps, and F3/Shift+F3 anywhere, go to the view
    shown (`find`). Taken before the editors see the key, as egui does.
  - M-13.2: the words of the search overlay are marked in every open note,
    those of the find field instead in the note it finds in
    (`editor/sync.ts`, `setSearches`).

  Feature F13 adds, as in egui:
  - M-19.9: the conflict bar below the find field while the note holds
    places a sync left in conflict (ConflictBar.svelte), and "Abort the
    sync" asks from there (AbortSyncDialog.svelte, in App.svelte).
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type {
    EditingViewName,
    EditorCommandName,
    FindAnswer,
    FindStatus,
    FindStep,
    ToolName,
  } from './bindings';
  import FindBar from './FindBar.svelte';
  import ConflictBar from './ConflictBar.svelte';
  import { activeFacade } from './facade/active';
  import FacadeView from './facade/FacadeView.svelte';
  import EditorStatus from './EditorStatus.svelte';
  import { EDITOR_FAILED, coreOf, setSearches } from './editor/sync';
  import { activeSourceView } from './editor/active';
  import { setCopyOnSelect } from './editor/clipboard';
  import { listenDrops } from './editor/drop';
  import { Inserting, type InsertTarget } from './editor/inserting.svelte';
  import InsertImageDialog from './InsertImageDialog.svelte';
  import InsertLinkDialog from './InsertLinkDialog.svelte';
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

  /** What the status line and the minimap show of the source view. */
  const info = new EditorInfo();
  const settings = $derived(shell.settings);
  const tools = $derived(shell.start?.tools ?? []);
  let editorWidth = $state(0);
  let column = $state<HTMLElement | null>(null);

  // Q-40: the note on lost edits goes with the next key or click in the note.
  $effect(() => {
    const element = column;
    const note = activeTab?.note;
    if (!element || !note || !shell.editorLost.includes(note)) {
      return;
    }
    const seen = () => shell.editorLostSeen(note);
    element.addEventListener('keydown', seen, true);
    element.addEventListener('pointerdown', seen, true);
    return () => {
      element.removeEventListener('keydown', seen, true);
      element.removeEventListener('pointerdown', seen, true);
    };
  });
  const withMinimap = $derived(minimapShown(settings?.showMinimap ?? false, editorWidth));

  /** The note shown and its view, for the dialogs' answers and drops. */
  function insertTarget(): InsertTarget | null {
    const tab = activeTab;
    if (!tab) {
      return null;
    }
    if (tab.view === 'facade') {
      const facade = info.facade;
      return facade && facade.note === tab.note ? { note: tab.note, editor: facade } : null;
    }
    const source = activeSourceView();
    return source && source.note === tab.note ? { note: tab.note, editor: source } : null;
  }

  /** M-10.3 to M-10.5: the Link and Image dialogs and dropped files. */
  const inserting = new Inserting(insertTarget, (message) => shell.editorFailed(message));

  // UC-005 A2: Link and Image ask first; the request opens their dialog.
  $effect(() => {
    const asked = info.request;
    if (asked !== null) {
      info.request = null;
      inserting.ask(asked);
    }
  });

  // M-10.7: copy on select follows the setting.
  $effect(() => {
    setCopyOnSelect(settings?.copyOnSelect ?? true);
  });

  // M-10.5: files dropped on the window.
  onMount(() => {
    let stop: (() => void) | null = null;
    let gone = false;
    listenDrops((event) => void inserting.drop(event)).then(
      (unlisten) => {
        if (gone) {
          unlisten();
        } else {
          stop = unlisten;
        }
      },
      (error: unknown) => console.error('Dropping files is not available', error),
    );
    return () => {
      gone = true;
      stop?.();
    };
  });

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

  // M-13.2, UC-008 BR-008: the words marked in the open notes.
  $effect(() => {
    const find = shell.search.find;
    const note = activeTab?.note;
    setSearches({
      overlay: shell.search.text,
      find: find && note ? { note, text: find.text } : null,
    });
  });

  /** M-13.4: what the find field says of the active note's hits. */
  let findStatus = $state.raw<FindStatus | null>(null);
  let statusFrame: number | null = null;

  /** M-13.4: the count of the find field, worked out after the frame. */
  function refreshFindStatus() {
    if (statusFrame !== null) {
      return;
    }
    const frame = globalThis.requestAnimationFrame ?? ((f: () => void) => setTimeout(f, 16));
    statusFrame = frame(() => {
      statusFrame = null;
      const note = activeTab?.note;
      const tab = note ? coreOf(note) : undefined;
      try {
        findStatus = tab?.alive ? tab.core.findStatus() : null;
      } catch {
        findStatus = null;
      }
    }) as number;
  }

  // The count follows the text, the hits and the selection of the note.
  $effect(() => {
    if (shell.search.find === null) {
      return;
    }
    void info.place;
    void info.found;
    void activeTab?.note;
    untrack(refreshFindStatus);
  });

  /** The view shown, for the find field's steps. */
  function findTarget(): {
    find(text: string, step: FindStep): Promise<FindAnswer | null>;
    settled(): Promise<unknown>;
  } | null {
    const tab = activeTab;
    if (!tab) {
      return null;
    }
    if (tab.view === 'facade') {
      const facade = info.facade;
      return facade && facade.note === tab.note ? facade : null;
    }
    const source = activeSourceView();
    return source && source.note === tab.note ? source : null;
  }

  /** The steps of the find field run one after the other. */
  let finding: Promise<unknown> = Promise.resolve();

  /** M-13.4: a step of the find field in the view shown. */
  function findStep(step: FindStep) {
    finding = finding.then(async () => {
      const find = shell.search.find;
      const target = findTarget();
      if (!find || !target) {
        return;
      }
      try {
        const answer = await target.find(find.text, step);
        if (answer) {
          findStatus = answer.status;
        }
      } catch (error) {
        shell.editorFailed(error instanceof Error ? error.message : String(error));
      }
      refreshFindStatus();
    });
  }

  /** M-13.4: a changed text selects the first hit from the cursor on. */
  function findText(text: string) {
    shell.search.setFindText(text);
    if (text.trim() !== '') {
      findStep('first');
    }
    refreshFindStatus();
  }

  /** M-13.4: the find field closes; the keyboard goes back to the text. */
  function closeFind() {
    shell.search.closeFind();
    if (activeTab?.view === 'facade') {
      info.facade?.focus();
    } else {
      activeSourceView()?.focus();
    }
  }

  /**
   * M-13.4, UC-008 A6, BR-009: Ctrl+F (Cmd+F) with the input mode Normal
   * opens the find field; F3 and Shift+F3 step while it is open.
   */
  function findKey(event: KeyboardEvent) {
    const command = shell.platform === 'macos' ? event.metaKey : event.ctrlKey;
    const other = shell.platform === 'macos' ? event.ctrlKey : event.metaKey;
    if (
      event.key === 'F3' &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey &&
      shell.search.find !== null &&
      activeTab
    ) {
      event.preventDefault();
      event.stopPropagation();
      findStep(event.shiftKey ? 'previous' : 'next');
      return;
    }
    if (
      event.key.toLowerCase() !== 'f' ||
      !command ||
      other ||
      event.shiftKey ||
      event.altKey ||
      (settings?.inputMode ?? 'Normal') !== 'Normal' ||
      !activeTab
    ) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const note = activeTab.note;
    const target = findTarget();
    void (target?.settled() ?? Promise.resolve()).then(() => {
      let selected: string;
      try {
        const tab = coreOf(note);
        selected = tab?.alive ? tab.core.selectedText() : '';
      } catch {
        selected = '';
      }
      const text =
        selected !== '' && !selected.includes('\n') ? selected : (shell.search.find?.text ?? '');
      shell.search.openFind(text);
      if (text.trim() !== '') {
        findStep('first');
      }
    });
  }

  // Before the editors see the keys, as egui takes them first.
  onMount(() => {
    window.addEventListener('keydown', findKey, true);
    return () => {
      window.removeEventListener('keydown', findKey, true);
      if (statusFrame !== null) cancelAnimationFrame?.(statusFrame);
    };
  });

  /** M-19.9: a place of the conflict bar shows in the note, as the outline's do (M-16.5). */
  function showPlace(note: string, offset: number) {
    const facade = activeFacade();
    if (facade?.note === note) {
      facade.reveal(offset);
      return;
    }
    const source = activeSourceView();
    if (source?.note === note) {
      source.jump(offset);
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
        bind:this={column}
        role="tabpanel"
        id={NOTE_PANEL_ID}
        aria-labelledby={tabId(activeTab.note)}
        data-view={activeTab.view}
      >
        {#if shell.search.find !== null}
          <FindBar
            text={shell.search.find.text}
            status={findStatus}
            focusAsked={shell.search.findFocusAsked}
            onText={findText}
            onStep={(by) => findStep(by > 0 ? 'next' : 'previous')}
            onClose={closeFind}
          />
        {/if}
        {#if shell.conflictBar !== null && shell.conflictBar.note === activeTab.note}
          <ConflictBar {shell} bar={shell.conflictBar} onShow={showPlace} />
        {/if}
        {#if activeTab.saveError !== null}
          <p class="save-error" role="alert">
            Not stored: {activeTab.saveError}. Underline keeps trying.
          </p>
        {/if}
        {#if shell.editorLost.includes(activeTab.note)}
          <!-- Q-40: stays until the user acts in the note again. -->
          <p class="save-error" role="alert">{EDITOR_FAILED}</p>
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
                <button
                  type="button"
                  class="tool"
                  title={tool.hint}
                  aria-label={tool.hint}
                  onclick={() => useTool(tool.tool)}>{tool.label}</button
                >
              {/each}
            </div>
          {/if}
        </div>
        <div class="editor" bind:clientWidth={editorWidth} style:gap="{GAP}px">
          <!-- A measuring build (editor/bench.ts) shows its own source view. -->
          <div class="text">
            <!-- A note of the same name in another workspace is another text. -->
            {#key `${shell.workspace?.root}:${activeTab.note}:${activeTab.view}`}
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
                  onFollow={(at) => void shell.followLink(activeTab.note, at)}
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
                  onFollow={(at) => void shell.followLink(activeTab.note, at)}
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
    <div class="note">
      <p class="hint">Select or create a note.</p>
      <!-- M-16.4: egui shows the git changes without an open note too. -->
      {#if shell.changes !== null}
        <ToolPanel {shell} />
      {/if}
    </div>
  {/if}
</section>
{#key inserting.dialog}
  {#if inserting.dialog?.kind === 'link'}
    <InsertLinkDialog
      notes={shell.overview}
      busy={inserting.busy}
      onAddress={(address) => void inserting.address(address)}
      onNote={(note) => void inserting.note(note)}
      onClose={() => inserting.close()}
    />
  {:else if inserting.dialog?.kind === 'image'}
    <InsertImageDialog
      large={inserting.dialog.large}
      busy={inserting.busy}
      onFile={(path) => void inserting.file(path)}
      onCopy={() => void inserting.copyLarge()}
      onLinkPlace={() => void inserting.linkPlace()}
      onClose={() => inserting.close()}
    />
  {/if}
{/key}

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
    flex: 1 1 auto;
    min-width: 0;
    margin: 0;
    padding: var(--space-2);
  }
</style>
