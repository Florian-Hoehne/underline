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
    shown (`find`). Taken before the editors see the key, as egui does,
    by the window's key handler: the keys are those of the commands "Find
    in note", "Replace in note", "Find next" and "Find previous" of the
    registry (`commandList.ts`, package 2), which run `shell.noteActions`.
  - M-13.2: the words of the search overlay are marked in every open note,
    those of the find field instead in the note it finds in
    (`editor/sync.ts`, `setSearches`).

  Package 1 adds (FR-073, FR-075, UC-005 A20 to A22, UC-008 A7):
  - the search switches of the find field, which find again when switched;
  - Ctrl+H (Cmd+Alt+F on macOS), with the input mode Normal as Ctrl+F,
    opens the find field for replacing; its "Replace" and "Replace all" go
    to the view shown (`replace`), and the count says what was replaced.

  Package 4 adds (FR-081 to FR-086, UC-005 A25 to A27, UC-004 A16):
  - close pairs follows the setting in every open note (`setClosePairs`);
  - the palette's "Move line up", "Move line down", "Duplicate line",
    "Indent list item" and "Outdent list item" act in the view shown, as
    their keys do there, and "Go to line" puts its cursor on the line
    asked for (`noteActions.edit`, `goToLine`).

  Package 5 adds (FR-087 to FR-090, UC-010 A3, A11, A12, UC-004 BR-020):
  - the editor zoom of the settings in both views (UC-010 BR-009); the
    mouse wheel over the note with Ctrl held zooms (`editor/zoom.ts`);
  - soft wrap as the settings say, switched in the status line too (BR-011);
  - the focus mode hides the switch and the formatting toolbar and the tool
    panel (BR-010; the top bar and the overview in App.svelte);
  - the text statistics of the note in the tool panel's properties.

  Feature F15 adds, as in egui (M-06.5, UC-021): while the tab "Todos" is
  the active one, its list (TodoTable.svelte) stands in place of the note,
  with the tool panel beside it; the panel also shows without an active
  note tab, for the section "Todos" (M-16.3).

  Feature F13 adds, as in egui:
  - M-19.9: the conflict bar below the find field while the note holds
    places a sync left in conflict (ConflictBar.svelte), and "Abort the
    sync" asks from there (AbortSyncDialog.svelte, in App.svelte).

  Package 7a (FR-096, FR-097, UC-004 A18 to A20, BR-018, UC-005 BR-016,
  OQ-220): the split view. The middle shows one pane, or a left and a right
  pane side by side, each with its tab bar, its note in its editing view,
  the switch and toolbar, the minimap and the status line; they start
  equally wide, and the border between them is dragged with the pointer or
  moved with the arrow keys (not remembered). The tool panel stands right
  of the panes and shows the active pane's note. The pane the user last
  worked in (a click or the keyboard in it) is the active pane: the find
  field, go to line, the toolbar's and the palette's commands, link
  completion and Link and Image act on its view, and its tab bar is marked
  by a line above it. A note shown on both sides has one text and one
  history; each side keeps its own cursor, reading position and folds.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type {
    EditActionName,
    EditingViewName,
    PaneName,
    TabView,
    EditorCommandName,
    FindAnswer,
    FindSpec,
    FindStatus,
    FindStep,
    LineTargetView,
    ToolName,
  } from './bindings';
  import FindBar from './FindBar.svelte';
  import ConflictBar from './ConflictBar.svelte';
  import { activeFacade, type ReplaceDone } from './facade/active';
  import type { SwitchName } from './search.svelte';
  import FacadeView from './facade/FacadeView.svelte';
  import EditorStatus from './EditorStatus.svelte';
  import { EDITOR_FAILED, coreOf, setClosePairs, setSearches } from './editor/sync';
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
  import { WheelZoom } from './editor/zoom';
  import { ariaKeys, keyText } from './commands';
  import type { LineNumberSide } from './editor/numbers';
  import Minimap from './Minimap.svelte';
  import SourceEditor from './SourceEditor.svelte';
  import TabBar from './TabBar.svelte';
  import TodoTable from './TodoTable.svelte';
  import ToolPanel from './ToolPanel.svelte';
  import type { Shell } from './shell.svelte';
  import { panesOf, type PaneTabs } from './panes';
  import { notePanelId, tabId } from './tabs';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  /** ADR-024: a build that measures the source view (editor/bench.ts). */
  const MEASURING = import.meta.env.VITE_UL_BENCH === '1';

  /** FR-096: the panes shown, the left one first. */
  const panes = $derived(panesOf(shell.tabs));
  /** UC-004 BR-018: the pane the user works in. */
  const activePane = $derived<PaneName>(shell.activePane);
  /** M-06.5: the tab "Todos" shows in the left pane. */
  const todosActive = $derived(shell.tabs.todoList.open && shell.tabs.todoList.active);

  /** The tab a pane shows, `undefined` while the tab "Todos" or nothing shows there. */
  function shownTab(pane: PaneTabs) {
    if (pane.pane === 'left' && todosActive) {
      return undefined;
    }
    return pane.tabs.find((tab) => tab.note === pane.active);
  }

  /** The active pane's tab: what the find field and the commands act on. */
  const activeTab = $derived(shownTab(panes.find((pane) => pane.pane === activePane) ?? panes[0]));

  /** What the status line and the minimap show of each pane's view. */
  const infos: Record<PaneName, EditorInfo> = { left: new EditorInfo(), right: new EditorInfo() };
  /** The active pane's. */
  const info = $derived(infos[activePane]);
  const settings = $derived(shell.settings);
  const tools = $derived(shell.start?.tools ?? []);
  /** The width of each pane's editor, for the minimap. */
  let editorWidths = $state<Record<PaneName, number>>({ left: 0, right: 0 });

  /** Q-40: the note on lost edits goes with the next key or click in the note. */
  function lostSeen(note: string) {
    return (element: HTMLElement) => {
      if (!shell.editorLost.includes(note)) {
        return;
      }
      const seen = () => shell.editorLostSeen(note);
      element.addEventListener('keydown', seen, true);
      element.addEventListener('pointerdown', seen, true);
      return () => {
        element.removeEventListener('keydown', seen, true);
        element.removeEventListener('pointerdown', seen, true);
      };
    };
  }

  /** M-12: the minimap beside the editor of `pane`, unless it is too narrow. */
  function withMinimap(pane: PaneName): boolean {
    return minimapShown(settings?.showMinimap ?? false, editorWidths[pane]);
  }

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

  // UC-005 A2: Link and Image ask first; the request opens their dialog,
  // from the view of either pane (it answers into the active one's).
  $effect(() => {
    for (const each of [infos.left, infos.right]) {
      const asked = each.request;
      if (asked !== null) {
        each.request = null;
        inserting.ask(asked);
      }
    }
  });

  // M-10.7: copy on select follows the setting.
  $effect(() => {
    setCopyOnSelect(settings?.copyOnSelect ?? true);
  });

  /** UC-010 BR-006: the key of the command `id`, as the status line's hints name it. */
  function commandKey(id: string): { text: string; aria?: string } {
    const key = shell.commandById(id)?.key;
    const mac = shell.platform === 'macos';
    return { text: keyText(key, mac), aria: ariaKeys(key, mac) };
  }

  /** UC-010 A11: the wheel's turns with Ctrl held, added up into steps. */
  const wheelZoom = new WheelZoom();

  // UC-010 A11, TC-010-18: Ctrl and the mouse wheel over the note of either
  // pane zoom the editor; the webview's own zoom does not happen. The
  // enlarged picture keeps the wheel for itself (it prevents the default
  // first).
  function zoomByWheel(box: HTMLElement) {
    const onWheel = (event: WheelEvent) => {
      if (event.defaultPrevented || !WheelZoom.zooms(event)) {
        return;
      }
      event.preventDefault();
      const step = wheelZoom.step(event);
      if (step !== 0) {
        void shell.zoomEditor(step);
      }
    };
    box.addEventListener('wheel', onWheel, { passive: false });
    return () => box.removeEventListener('wheel', onWheel);
  }

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

  /** The view the user just chose (`pane:note:view`); it takes the keyboard once shown. */
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
    const pane = activePane;
    await activeSourceView()?.settled();
    await info.facade?.settled();
    focusFor = `${pane}:${tab.note}:${view}`;
    await shell.setView(tab.note, view, pane);
  }

  /** M-07.2: the switch of the pane `pane`, which becomes the active one first. */
  async function chooseViewIn(pane: PaneName, view: EditingViewName) {
    if (pane !== activePane) {
      await shell.focusPane(pane);
    }
    await chooseView(view);
  }

  /** M-10.2: a toolbar button of the pane `pane` acts in its view. */
  async function useToolIn(pane: PaneName, tool: ToolName) {
    if (pane !== activePane) {
      await shell.focusPane(pane);
    }
    useTool(tool);
  }

  /**
   * UC-004 BR-018: the pane the user works in: a click or the keyboard in
   * it makes it the active pane.
   */
  function worksIn(pane: PaneName) {
    if (pane !== activePane) {
      void shell.focusPane(pane);
    }
  }

  /** OQ-220: the share of the width the left pane takes; equal at first, not remembered. */
  let leftShare = $state(50);
  let panesBox = $state<HTMLElement | null>(null);
  /** The width of the panes together, and of the whole note area. */
  let panesWidth = $state(0);
  let areaWidth = $state(0);
  /** OQ-278: the narrowest a pane may be, in px and as a share of both. */
  const LEAST_PANE_PX = 280;
  const LEAST_SHARE = 15;
  /** The width of the border between the panes, in px. */
  const BORDER_PX = 12;
  const split = $derived(panes.length > 1);
  /** The least share of each pane, by the width the panes have. */
  const leastShare = $derived(
    panesWidth > 0 ? Math.max(LEAST_SHARE, (LEAST_PANE_PX / panesWidth) * 100) : LEAST_SHARE,
  );
  /**
   * OQ-278: the panes do not fit side by side at their least width: only
   * the active pane shows, with a hint how to reach the other.
   */
  const squeezed = $derived(split && panesWidth > 0 && panesWidth < 2 * LEAST_PANE_PX + BORDER_PX);
  /** The tool panel beside two panes of their least width. */
  const toolPanelRoom = $derived(
    !split || areaWidth === 0 || areaWidth >= 2 * LEAST_PANE_PX + BORDER_PX + toolPanelPx(),
  );
  /** OQ-278: the user showed (`true`) or hid the tool panel while there is no room for it. */
  let toolPanelWanted = $state<boolean | null>(null);
  const toolPanelShown = $derived(!shell.focusMode && (toolPanelWanted ?? toolPanelRoom));

  $effect(() => {
    // Room again, or no split: the tool panel shows as before.
    if (toolPanelRoom) {
      toolPanelWanted = null;
    }
  });

  function toolPanelPx(): number {
    return shell.toolPanelWidth ?? shell.settings?.toolPanelWidth ?? 240;
  }

  function shareWithin(share: number): number {
    const least = Math.min(leastShare, 50);
    return Math.min(100 - least, Math.max(least, share));
  }

  /** The share the left pane shows: what was dragged, within the least share. */
  const shownShare = $derived(shareWithin(leftShare));

  /** TC-004-34 step 4: dragging the border between the panes. */
  function dragBorder(event: PointerEvent) {
    const box = panesBox;
    if (event.button !== 0 || !box) {
      return;
    }
    event.preventDefault();
    const border = event.currentTarget as HTMLElement;
    border.setPointerCapture?.(event.pointerId);
    const move = (moved: PointerEvent) => {
      const bounds = box.getBoundingClientRect();
      if (bounds.width > 0) {
        leftShare = shareWithin(((moved.clientX - bounds.left) / bounds.width) * 100);
      }
    };
    const end = () => {
      border.removeEventListener('pointermove', move);
      border.removeEventListener('pointerup', end);
      border.removeEventListener('pointercancel', end);
    };
    border.addEventListener('pointermove', move);
    border.addEventListener('pointerup', end);
    border.addEventListener('pointercancel', end);
  }

  /**
   * NFR-011: the border moves with the arrow keys too, Home and End to its
   * ends; Enter, as a double click, gives both panes the same width again.
   */
  function borderKey(event: KeyboardEvent) {
    const step = event.shiftKey ? 10 : 2;
    const to = {
      ArrowLeft: shownShare - step,
      ArrowRight: shownShare + step,
      Home: 0,
      End: 100,
      Enter: 50,
    }[event.key];
    if (to !== undefined) {
      event.preventDefault();
      leftShare = shareWithin(to);
    }
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
    // FR-096: within the pane the keyboard is in.
    const scope: HTMLElement = active.closest<HTMLElement>('.pane') ?? area;
    const regions: [HTMLElement | null, HTMLElement | null][] = [
      [
        scope.querySelector<HTMLElement>('[role="tablist"]'),
        scope.querySelector<HTMLElement>('[role="tab"][tabindex="0"]'),
      ],
      [
        scope.querySelector<HTMLElement>('.views'),
        scope.querySelector<HTMLElement>('.views button:not(:disabled)'),
      ],
      [
        scope.querySelector<HTMLElement>('.editor'),
        scope.querySelector<HTMLElement>('.cm-content, .facade-text'),
      ],
      [
        scope.querySelector<HTMLElement>('.editor-status'),
        scope.querySelector<HTMLElement>('.editor-status button'),
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

  /** What the find field finds in the note shown (UC-008 A7, UC-005 BR-017). */
  function findSpec(): FindSpec | null {
    return shell.search.findSpec(activeTab?.view === 'facade');
  }

  // UC-010 BR-001, BR-004, UC-005 BR-019: close pairs in every open note.
  $effect(() => {
    setClosePairs(settings?.closePairs ?? true);
  });

  /** UC-005 A26, A27: an action of the editing comfort in the view shown. */
  function editAction(action: EditActionName) {
    if (activeTab?.view === 'facade') {
      info.facade?.edit(action);
    } else {
      activeSourceView()?.edit(action);
    }
  }

  /** UC-004 A16 step 1: how many lines the active note has, and whether the facade shows it. */
  function lineInfo(): { lines: number; facade: boolean } | null {
    if (!activeTab) {
      return null;
    }
    const facade = activeTab.view === 'facade';
    const lines = facade
      ? (info.facade?.lineCount() ?? null)
      : (activeSourceView()?.lineCount() ?? null);
    return lines === null ? null : { lines, facade };
  }

  /**
   * UC-004 A16 step 3, BR-017: the cursor of the view shown goes to
   * `target`; a line or column beyond the note is said in the status bar.
   */
  function goToLine(target: LineTargetView) {
    const note = activeTab?.note;
    if (!note) {
      return;
    }
    try {
      const tab = coreOf(note);
      const said = tab?.alive ? tab.core.lineNote(target) : null;
      if (said !== null) {
        shell.status = said;
      }
    } catch {
      // Nothing to say without the core; the view still goes.
    }
    if (activeTab?.view === 'facade') {
      info.facade?.goToLine(target);
    } else {
      activeSourceView()?.goToLine(target);
    }
  }

  /**
   * UC-005 A28: the cursor to the next or previous marked word, or the
   * spell menu of the word at the cursor, in the view shown; the status bar
   * says when there is none.
   */
  function spellStep(step: 'next' | 'previous' | 'menu') {
    if (!activeTab) {
      return;
    }
    const view =
      activeTab.view === 'facade'
        ? info.facade
        : activeSourceView()?.note === activeTab.note
          ? activeSourceView()
          : null;
    if (!view) {
      return;
    }
    const done = step === 'menu' ? view.spellMenuAtCursor() : view.stepSpell(step === 'previous');
    if (!done) {
      shell.status =
        step === 'menu' ? 'The word at the cursor is not misspelled' : 'No word is misspelled';
    }
  }

  // M-13.2, UC-008 BR-008: the words marked in the open notes.
  $effect(() => {
    const note = activeTab?.note;
    const spec = findSpec();
    setSearches({
      overlay: shell.search.open ? shell.search.overlaySpec : null,
      find: spec && note ? { note, spec } : null,
    });
  });

  /** UC-005 A21: how many the last "Replace all" replaced, until the field changes. */
  let replaced = $state<{ count: number; text: string } | null>(null);
  /** UC-005 A20, A21: a replacement in the note runs. */
  let replacingInNote = $state(false);

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
        const find = shell.search.find;
        findStatus = tab?.alive
          ? tab.core.findStatus(find?.replacing ? shell.search.findReplacement : null)
          : null;
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
    find(spec: FindSpec, step: FindStep): Promise<FindAnswer | null>;
    replace(spec: FindSpec, replacement: string, all: boolean): Promise<ReplaceDone | null>;
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
      const spec = findSpec();
      const target = findTarget();
      if (!spec || !target) {
        return;
      }
      try {
        const answer = await target.find(spec, step);
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
    replaced = null;
    shell.search.setFindText(text);
    if (text.trim() !== '') {
      findStep('first');
    }
    refreshFindStatus();
  }

  /** UC-008 A7: a switch of the find field finds again (BR-011). */
  function findSwitch(name: SwitchName) {
    replaced = null;
    shell.search.toggleFindSwitch(name);
    if ((shell.search.find?.text ?? '') !== '') {
      findStep('first');
    }
    refreshFindStatus();
  }

  /** UC-005 A20: the replace row shows or hides; the search text is read anew. */
  function findReplacing(replacing: boolean) {
    replaced = null;
    shell.search.setFindReplacing(replacing);
    if ((shell.search.find?.text ?? '') !== '') {
      findStep('first');
    }
    refreshFindStatus();
  }

  /** UC-005 A22: the replacement typed; what is wrong with it shows. */
  function findReplacement(text: string) {
    replaced = null;
    shell.search.findReplacement = text;
    refreshFindStatus();
  }

  /**
   * UC-005 A20, A21: replaces the selected occurrence (and selects the
   * next) or all of them, in the view shown, after the steps before.
   */
  function replaceInNote(all: boolean) {
    // App-tester: a second press while one runs does nothing.
    if (replacingInNote) {
      return;
    }
    replacingInNote = true;
    finding = finding.then(async () => {
      const spec = findSpec();
      const target = findTarget();
      if (!spec || !target) {
        replacingInNote = false;
        return;
      }
      try {
        const done = await target.replace(spec, shell.search.findReplacement, all);
        if (done) {
          findStatus = done.status;
          replaced = all ? { count: done.replaced, text: spec.text } : null;
        }
      } catch (error) {
        shell.editorFailed(error instanceof Error ? error.message : String(error));
      } finally {
        replacingInNote = false;
      }
      refreshFindStatus();
    });
  }

  /** M-13.4: the find field closes; the keyboard goes back to the text. */
  function closeFind() {
    replaced = null;
    shell.search.closeFind();
    if (activeTab?.view === 'facade') {
      info.facade?.focus();
    } else {
      activeSourceView()?.focus();
    }
  }

  /**
   * M-13.4, UC-008 A6, BR-009: opens the find field (for replacing with
   * `replacing`, UC-005 BR-017) holding the selection when it is within one
   * line, else the text it held while open. The commands "Find in note" and
   * "Replace in note" of the registry run it from their keys (Ctrl+F;
   * Ctrl+H, Cmd+Alt+F on macOS; with the input mode Normal) and from the
   * command palette (UC-023).
   */
  function openFind(replacing: boolean) {
    if (!activeTab) {
      return;
    }
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
      replaced = null;
      shell.search.openFind(text, replacing || (shell.search.find?.replacing ?? false));
      if (text.trim() !== '') {
        findStep('first');
      }
    });
  }

  /**
   * FR-080: link completion at the cursor of the view shown; UC-004 A2 to
   * A4, Q-64: the link at its cursor is followed once the inputs before it
   * reached the core, as a Ctrl+click there follows it.
   */
  function linkAction(action: 'complete' | 'follow' | 'followSide') {
    if (!activeTab) {
      return;
    }
    const note = activeTab.note;
    const facade = activeFacade();
    const source = activeSourceView();
    if (action === 'complete') {
      if (facade?.note === note) {
        facade.completeLink();
      } else if (source?.note === note) {
        source.completeLink();
      }
      return;
    }
    void (async () => {
      let head: number | null = null;
      if (facade?.note === note) {
        head = (await facade.mark())?.head ?? null;
      } else if (source?.note === note) {
        await source.settled();
        head = source.view.state.selection.main.head;
      }
      if (head !== null) {
        await shell.followLink(note, head, action === 'followSide');
      }
    })();
  }

  // UC-023 BR-005: the commands of the note shown act here, as its keys
  // and buttons do (F3 and Shift+F3 step while the find field is open).
  onMount(() => {
    shell.noteActions = {
      find: openFind,
      findStep: (back) => {
        if (shell.search.find !== null && activeTab) {
          findStep(back ? 'previous' : 'next');
        }
      },
      tool: useTool,
      view: (view) => void chooseView(view),
      link: linkAction,
      edit: editAction,
      lineInfo,
      goToLine,
      spell: spellStep,
    };
    return () => {
      shell.noteActions = null;
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

  /** M-10.9: the Vim or Nano keys store the note or close its tab (of their pane). */
  function command(note: string, command: EditorCommandName, pane: PaneName) {
    if (command === 'store') {
      void shell.storeTab(note);
    } else {
      void shell.closeTab(note, false, pane);
    }
  }
</script>

<svelte:window onkeydown={regionKey} />

<!-- The find field above the note of the active pane (M-13.4). -->
{#snippet findBar()}
  {#if shell.search.find !== null}
    <FindBar
      text={shell.search.find.text}
      status={findStatus}
      focusAsked={shell.search.findFocusAsked}
      onText={findText}
      onStep={(by) => findStep(by > 0 ? 'next' : 'previous')}
      onClose={closeFind}
      switches={shell.search.findSwitches}
      onSwitch={findSwitch}
      replacing={shell.search.find.replacing}
      replacement={shell.search.findReplacement}
      replaced={replaced !== null && replaced.text === shell.search.find.text
        ? replaced.count
        : null}
      busy={replacingInNote}
      mac={shell.platform === 'macos'}
      onReplacing={findReplacing}
      onReplacement={findReplacement}
      onReplace={replaceInNote}
    />
  {/if}
{/snippet}

<!-- One pane's note: its bars, its view, its minimap and status line. -->
{#snippet noteOf(pane: PaneName, tab: TabView)}
  {@const paneInfo = infos[pane]}
  {@const active = pane === activePane}
  <div
    class="editor-column"
    {@attach lostSeen(tab.note)}
    role="tabpanel"
    id={notePanelId(pane)}
    aria-labelledby={tabId(tab.note, pane)}
    data-view={tab.view}
  >
    {#if active}
      {@render findBar()}
    {/if}
    {#if shell.conflictBar !== null && shell.conflictBar.note === tab.note && active}
      <ConflictBar {shell} bar={shell.conflictBar} onShow={showPlace} />
    {/if}
    {#if tab.saveError !== null}
      <p class="save-error" role="alert">
        Not stored: {tab.saveError}. Underline keeps trying.
      </p>
    {/if}
    {#if shell.editorLost.includes(tab.note)}
      <!-- Q-40: stays until the user acts in the note again. -->
      <p class="save-error" role="alert">{EDITOR_FAILED}</p>
    {/if}
    {#if !shell.focusMode}
      <div class="views" role="group" aria-label="Editing view">
        <button
          type="button"
          class="view"
          aria-pressed={tab.view === 'facade'}
          onclick={() => chooseViewIn(pane, 'facade')}>Facade</button
        >
        <button
          type="button"
          class="view"
          aria-pressed={tab.view === 'source'}
          onclick={() => chooseViewIn(pane, 'source')}>Source</button
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
                onclick={() => useToolIn(pane, tool.tool)}>{tool.label}</button
              >
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    <div
      class="editor"
      {@attach zoomByWheel}
      bind:clientWidth={editorWidths[pane]}
      style:gap="{GAP}px"
    >
      <!-- A measuring build (editor/bench.ts) shows its own source view. -->
      <div class="text">
        <!-- A note of the same name in another workspace is another text. -->
        {#key `${shell.workspace?.root}:${tab.note}:${tab.view}`}
          {#if MEASURING}
            <!-- The measuring build shows its own source view. -->
          {:else if tab.view === 'facade'}
            <FacadeView
              note={tab.note}
              label={tab.title}
              platform={shell.platform}
              {pane}
              mode={(settings?.inputMode ?? 'Normal') as InputModeName}
              focus={focusFor === `${pane}:${tab.note}:facade`}
              info={paneInfo}
              onError={(message) => shell.editorFailed(message)}
              onCommand={(name) => command(tab.note, name, pane)}
              onFollow={(at, side) => void shell.followLink(tab.note, at, side)}
              agentMarks={shell.agentMarksOf(tab.note)}
              spellMarks={shell.spellMarksOf(tab.note)}
              onCorrected={(suggestion) => shell.announceSpell(`Corrected to ${suggestion}`)}
              zoom={shell.editorZoom}
              softWrap={settings?.softWrap ?? true}
            />
          {:else}
            <SourceEditor
              note={tab.note}
              label={tab.title}
              mac={shell.platform === 'macos'}
              {pane}
              mode={(settings?.inputMode ?? 'Normal') as InputModeName}
              lineNumbers={settings?.showLineNumbers ?? false}
              lineNumberSide={(settings?.lineNumberSide ?? 'Left') as LineNumberSide}
              info={paneInfo}
              focus={focusFor === `${pane}:${tab.note}:source`}
              onError={(message) => shell.editorFailed(message)}
              onCommand={(name) => command(tab.note, name, pane)}
              onFollow={(at, side) => void shell.followLink(tab.note, at, side)}
              agentMarks={shell.agentMarksOf(tab.note)}
              spellMarks={shell.spellMarksOf(tab.note)}
              onCorrected={(suggestion) => shell.announceSpell(`Corrected to ${suggestion}`)}
              zoom={shell.editorZoom}
              softWrap={settings?.softWrap ?? true}
            />
          {/if}
        {/key}
      </div>
      {#if withMinimap(pane)}
        <Minimap
          view={paneInfo.minimap}
          palette={settings?.palette ?? null}
          blink={settings?.blinkFoundInMinimap ?? true}
        />
      {/if}
    </div>
    <EditorStatus
      keys={paneInfo.keys}
      place={paneInfo.place}
      inputMode={settings?.inputMode ?? 'Normal'}
      inputModes={shell.start?.inputModes ?? ['Normal', 'Vim', 'Nano']}
      copyOnSelect={settings?.copyOnSelect ?? false}
      softWrap={settings?.softWrap ?? true}
      minimap={settings?.showMinimap ?? false}
      onInputMode={(mode) => shell.quickSetting('input_mode', mode)}
      onCopyOnSelect={(on) => shell.quickSetting('copy_on_select', String(on))}
      onSoftWrap={(on) => shell.quickSetting('soft_wrap', String(on))}
      focusMode={shell.focusMode}
      onFocusMode={(on) => void shell.setFocusMode(on)}
      softWrapKey={commandKey('status.softWrap')}
      focusModeKey={commandKey('view.focusMode')}
      onMinimap={(on) => shell.quickSetting('show_minimap', String(on))}
    />
  </div>
{/snippet}

<!-- FR-096, review: the buttons at the end of the right pane's tab bar. -->
{#snippet paneButtons()}
  {#if !toolPanelRoom && !shell.focusMode}
    <button
      type="button"
      class="bar-button"
      aria-pressed={toolPanelShown}
      title={toolPanelShown ? 'Hide the tool panel' : 'Show the tool panel'}
      aria-label={toolPanelShown ? 'Hide the tool panel' : 'Show the tool panel'}
      onclick={() => (toolPanelWanted = !toolPanelShown)}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <rect x="2.5" y="3.5" width="11" height="9" rx="1.5" />
        <path d="M10 3.5v9" />
      </svg>
    </button>
  {/if}
  <button
    type="button"
    class="bar-button"
    title="End the split view  {keyText(
      shell.commandById('view.endSplit')?.key,
      shell.platform === 'macos',
    )}"
    aria-label="End the split view"
    aria-keyshortcuts={ariaKeys(
      shell.commandById('view.endSplit')?.key,
      shell.platform === 'macos',
    )}
    onclick={() => void shell.endSplit()}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <rect x="2.5" y="3.5" width="11" height="9" rx="1.5" />
      <path d="M8 3.5v9M5.5 6.5l5 3M10.5 6.5l-5 3" />
    </svg>
  </button>
{/snippet}

<section class="note-area" bind:this={area} bind:clientWidth={areaWidth}>
  <div class="panes" class:split class:squeezed bind:this={panesBox} bind:clientWidth={panesWidth}>
    {#each panes as pane (pane.pane)}
      {@const tab = shownTab(pane)}
      {#if pane.pane === 'right'}
        <!-- OQ-220: the border between the panes, dragged or moved with the arrow keys
             (the window splitter of WAI-ARIA, a focusable separator). -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
        <div
          class="border"
          role="separator"
          aria-orientation="vertical"
          aria-label="Border between the panes"
          aria-valuemin={Math.round(Math.min(leastShare, 50))}
          aria-valuemax={Math.round(100 - Math.min(leastShare, 50))}
          aria-valuenow={Math.round(shownShare)}
          aria-valuetext="Left pane {Math.round(shownShare)} percent"
          title="Drag to change the widths; double-click or Enter for equal widths"
          tabindex="0"
          onpointerdown={dragBorder}
          ondblclick={() => (leftShare = 50)}
          onkeydown={borderKey}
        ></div>
      {/if}
      <!-- BR-018: a click or the keyboard in a pane makes it the active one. -->
      <div
        class="pane"
        class:active={split && pane.pane === activePane}
        class:hidden={squeezed && pane.pane !== activePane}
        data-pane={pane.pane}
        role={split ? 'group' : undefined}
        aria-label={split ? (pane.pane === 'left' ? 'Left pane' : 'Right pane') : undefined}
        aria-current={split && pane.pane === activePane ? 'true' : undefined}
        style:flex-basis={split && !squeezed
          ? `${pane.pane === 'left' ? shownShare : 100 - shownShare}%`
          : undefined}
        onfocusin={() => worksIn(pane.pane)}
        onpointerdown={() => worksIn(pane.pane)}
      >
        {#if squeezed}
          <p class="narrow" role="note">
            The window is too narrow for both panes. {keyText(
              shell.commandById('view.focusLeftPane')?.key,
              shell.platform === 'macos',
            )} and {keyText(
              shell.commandById('view.focusRightPane')?.key,
              shell.platform === 'macos',
            )} switch between them.
          </p>
        {/if}
        <TabBar
          {shell}
          pane={pane.pane}
          paneActive={!split || pane.pane === activePane}
          trailing={pane.pane === 'right' ? paneButtons : undefined}
        />
        {#if tab}
          {@render noteOf(pane.pane, tab)}
        {:else if pane.pane === 'left' && todosActive}
          <!-- M-06.5, M-18.6 (UC-021): the tab "Todos" in place of a note. -->
          <TodoTable {shell} />
        {:else}
          <p class="hint">Select or create a note.</p>
        {/if}
      </div>
    {/each}
  </div>
  <!-- M-16.3, M-16.4: egui shows the todos and the git changes without an open note too. -->
  {#if toolPanelShown}
    <ToolPanel {shell} />
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
    min-width: 0;
    min-height: 0;
  }

  .panes {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    min-height: 0;
  }

  .pane {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .pane {
    container-type: inline-size;
  }

  /* UC-004 BR-018: the active pane is marked by a line above its tab bar,
     not by colour alone (NFR-010); the other pane's keeps the space. */
  .split .pane {
    border-top: 3px solid transparent;
  }

  .split .pane.active {
    border-top-color: var(--ui-focus);
  }

  /* OQ-278: too narrow for both panes: the active one only. */
  .squeezed .pane.hidden {
    display: none;
  }

  .narrow {
    margin: 0;
    padding: var(--space-1) var(--space-2);
    color: var(--ui-hint);
    border-bottom: 1px solid var(--ui-rule);
  }

  /* A 12 px hit area around a line of 1 px. */
  .border {
    flex: 0 0 12px;
    cursor: col-resize;
    background: linear-gradient(var(--ui-rule), var(--ui-rule)) center / 1px 100% no-repeat;
    touch-action: none;
  }

  .squeezed .border {
    display: none;
  }

  .border:hover,
  .border:focus-visible {
    background: linear-gradient(var(--ui-focus), var(--ui-focus)) center / 3px 100% no-repeat;
    outline: none;
  }

  .bar-button {
    display: inline-grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    color: var(--ui-text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .bar-button:hover {
    color: var(--ui-chosen-text);
    background: var(--ui-chosen);
  }

  .bar-button[aria-pressed='true'] {
    border-color: var(--ui-border);
  }

  .bar-button:focus-visible {
    outline: 2px solid var(--ui-focus);
    outline-offset: -2px;
  }

  .bar-button svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
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

  /* FR-096 (app-tester): the switch and the toolbar wrap by their own
     width (the minimap may take some of the pane's), so every button stays
     in reach; A-46's row that scrolled sideways is left for this. */
  .views {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    gap: var(--space-1);
    padding: var(--space-1) var(--space-2);
  }

  .views > * {
    flex: 0 0 auto;
  }

  .views > .tools {
    flex: 0 1 auto;
    flex-wrap: wrap;
    min-width: 0;
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
