<!--
  The window: the top bar with the menu (M-03), the central area and the
  status bar (M-04.7).

  The central area (UC-002):
  - while the workspace to start with opens, nothing yet (plan section 14),
    so the empty screen does not flash;
  - without a workspace, the offer to open or create one (M-02);
  - with one, the note overview (M-05) beside the tabs and the note (M-06).
  The Open and Create dialogs float above it, not modal (M-21.1, M-21.2),
  and so do "Open web address" (M-07.3), "Changes not stored" (M-06.3,
  M-21.4) and the note dialogs "New note", "Rename note", "Delete note",
  "Restore note" and "Remove deleted notes" (M-14).

  F2 renames the note of the active tab (M-14.2, UC-006 step 1), wherever
  the keyboard is, as in egui.

  M-13.1 (UC-008 BR-007): Ctrl+Shift+F (Cmd+Shift+F) opens the search
  overlay and puts the keyboard into its field, wherever the keyboard is,
  before the editors see the key (the Vim keys too). While the overlay is
  open, Escape closes it wherever the keyboard is, as in egui; only a
  dialog that has the keyboard keeps its own Escape.

  M-15.1 (UC-009 BR-008): Ctrl+Shift+E (Cmd+Shift+E on macOS) asks how to
  export the note of the active tab, wherever the keyboard is, before the
  editors see the key; "Export note" and "Approximated blocks" float above
  the central area like the other dialogs (M-15.6).

  M-19.2 (UC-011 BR-007): Ctrl+Shift+G (Cmd+Shift+G) opens the git changes
  in the tool panel and puts the keyboard into the commit message, wherever
  the keyboard is, before the editors see the key; "Discard changes" and
  "Your name for commits" float above the central area (M-19.4, M-19.5).

  M-19.6 (UC-013 BR-006): Ctrl+Shift+B (Cmd+Shift+B) opens the branch menu,
  wherever the keyboard is, before the editors see the key, in a git
  workspace (as egui); the menu stands at the bottom left. "Abort the sync"
  (M-19.10) floats above the central area like the other dialogs.

  M-18.3 (feature F15): the calendar "Choose a day" floats above the
  central area like the other dialogs.

  M-20.1 (UC-010 BR-006): Ctrl+, (Cmd+, on macOS) opens the settings window
  or closes it, wherever the keyboard is, before the editors see the key.
  The window floats above the central area, not modal, like egui's.

  Package 2 (FR-076, FR-077, UC-004 A12, A13, UC-023): every one of these
  keys is the key of a command of the registry (`lib/commandList.ts`),
  which one handler runs; Ctrl+P (Cmd+P) opens go to note, Ctrl+Shift+P
  (Cmd+Shift+P) and F1 the command palette, one overlay over the window
  like the search's (PaletteOverlay.svelte).

  Package 5 (FR-088, UC-010 A12, BR-010, OQ-209): the focus mode hides the
  top bar with the menu bar and the note overview (the note area hides its
  toolbar and the tool panel), so the tabs and the notes take the whole
  width; the status bar stays. Every hidden function keeps its key and its
  command in the palette.
-->
<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { COMMAND } from './lib/commandList';
  import { commandForKey, keyText } from './lib/commands';
  import { focusFallback } from './lib/focus';
  import CloseQuestionDialog from './lib/CloseQuestionDialog.svelte';
  import CreateDialog from './lib/CreateDialog.svelte';
  import DeleteDialog from './lib/DeleteDialog.svelte';
  import DiscardDialog from './lib/DiscardDialog.svelte';
  import AbortSyncDialog from './lib/AbortSyncDialog.svelte';
  import BranchMenu from './lib/BranchMenu.svelte';
  import IdentityDialog from './lib/IdentityDialog.svelte';
  import NewNoteDialog from './lib/NewNoteDialog.svelte';
  import PurgeDialog from './lib/PurgeDialog.svelte';
  import RenameDialog from './lib/RenameDialog.svelte';
  import RestoreDialog from './lib/RestoreDialog.svelte';
  import SnapshotsDialog from './lib/SnapshotsDialog.svelte';
  import PaletteOverlay from './lib/PaletteOverlay.svelte';
  import LinkCompletion from './lib/LinkCompletion.svelte';
  import SearchOverlay from './lib/SearchOverlay.svelte';
  import SettingsWindow from './lib/SettingsWindow.svelte';
  import EmptyScreen from './lib/EmptyScreen.svelte';
  import ExportDialog from './lib/ExportDialog.svelte';
  import ApproximatedDialog from './lib/ApproximatedDialog.svelte';
  import NoteArea from './lib/NoteArea.svelte';
  import OpenDialog from './lib/OpenDialog.svelte';
  import OpenWebDialog from './lib/OpenWebDialog.svelte';
  import Overview from './lib/Overview.svelte';
  import StatusBar from './lib/StatusBar.svelte';
  import TodoCalendar from './lib/TodoCalendar.svelte';
  import TopBar from './lib/TopBar.svelte';
  import type { Shell } from './lib/shell.svelte';

  type Props = { shell: Shell };

  let { shell }: Props = $props();

  const openWorkspace = () => void shell.startOpen();
  const newWorkspace = () => shell.startCreate();

  onDestroy(() => shell.disconnect());

  /** UX review: the command palette and its key on the empty screen, from the registry. */
  const paletteEntry = $derived.by(() => {
    const command = shell.commandById(COMMAND.palette);
    return command
      ? { label: command.title, key: keyText(command.key, shell.platform === 'macos') }
      : undefined;
  });

  /**
   * UC-023 BR-001, BR-005: the window's keys are those of the commands of
   * the registry (`commandList.ts`), taken before the editors see them (the
   * Vim and Nano keys too), wherever the keyboard is, as egui takes F2
   * first: F2 renames the note of the active tab (M-14.2); Ctrl+Shift+F
   * and Ctrl+Shift+H open the search and the replace of the workspace
   * (M-13.1, UC-022 BR-008; Ctrl+Shift+H only outside the text of a note,
   * where it is the toolbar's heading, OQ-243); Ctrl+, the settings (M-20.1); Ctrl+Shift+E
   * exports (M-15.1); Ctrl+Shift+G the changes (M-19.2); Ctrl+Shift+B the
   * branch menu (M-19.6); Ctrl+F, Ctrl+H (Cmd+Alt+F), F3 and Shift+F3 the
   * find field of the note (M-13.4); Ctrl+P go to note (UC-004 A12);
   * Ctrl+Shift+P and F1 the command palette (UC-023 BR-007), these three not
   * over a dialog that waits for an answer. Cmd stands for
   * Ctrl on macOS. A key whose command does not take it now keeps its way.
   */
  function commandKey(event: KeyboardEvent) {
    if (event.isComposing) {
      return;
    }
    const command = commandForKey(event, shell.commands, shell.platform === 'macos');
    if (command === null) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const why = command.unavailable();
    if (why === null) {
      command.run();
    } else {
      // Review of package 4: a key taken that cannot run says why.
      shell.status = why;
    }
  }

  /**
   * M-13.1, BR-007: while the search overlay is open, Escape closes the
   * replace first (UC-022 A8: the search keeps its text), then the overlay,
   * wherever the keyboard is; only a dialog that has the keyboard keeps its
   * own Escape.
   */
  function searchEscape(event: KeyboardEvent) {
    const plain = !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
    if (event.key !== 'Escape' || !plain || !shell.search.open) {
      return;
    }
    const inDialog = (document.activeElement as Element | null)?.closest?.('[role="dialog"]');
    if (inDialog) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (shell.search.replacing) {
      shell.search.stopReplacing();
    } else {
      shell.search.close();
    }
  }

  // Before the editors see it, as egui takes F2 first.
  onMount(() => {
    window.addEventListener('keydown', commandKey, true);
    window.addEventListener('keydown', searchEscape, true);
    return () => {
      window.removeEventListener('keydown', commandKey, true);
      window.removeEventListener('keydown', searchEscape, true);
    };
  });

  // When a workspace opens, the screen behind the keyboard may be gone
  // (the empty screen, a closed dialog); the keyboard goes to the overview.
  $effect(() => {
    if (shell.workspace?.root === undefined) {
      return;
    }
    void tick().then(() => {
      if (document.activeElement === document.body) {
        focusFallback();
      }
    });
  });
</script>

<div class="window" class:focus={shell.focusMode}>
  {#if !shell.focusMode}
    <TopBar {shell} />
  {/if}
  <main class="central" aria-busy={shell.starting}>
    {#if shell.starting}
      <!-- Q-04: the egui app opens before its first frame and has no text for this. -->
    {:else if shell.workspace === null}
      <EmptyScreen
        onOpen={openWorkspace}
        onCreate={newWorkspace}
        palette={paletteEntry}
        onPalette={() => shell.palette.show('commands')}
      />
    {:else}
      <div class="workbench">
        {#if !shell.focusMode}
          <Overview {shell} />
        {/if}
        <NoteArea {shell} />
      </div>
    {/if}
    {#if shell.dialog?.kind === 'open'}
      <OpenDialog {shell} dialog={shell.dialog} />
    {:else if shell.dialog?.kind === 'create'}
      <CreateDialog {shell} dialog={shell.dialog} />
    {/if}
    {#if shell.webQuestion !== null}
      <OpenWebDialog {shell} address={shell.webQuestion} />
    {/if}
    {#if shell.notes.newNote}
      <NewNoteDialog {shell} dialog={shell.notes.newNote} />
    {/if}
    {#if shell.notes.rename}
      <RenameDialog {shell} dialog={shell.notes.rename} />
    {/if}
    {#if shell.notes.delete}
      <DeleteDialog {shell} dialog={shell.notes.delete} />
    {/if}
    {#if shell.notes.restore}
      <RestoreDialog {shell} dialog={shell.notes.restore} />
    {/if}
    {#if shell.notes.purge}
      <PurgeDialog {shell} dialog={shell.notes.purge} />
    {/if}
    {#if shell.snapshots.note !== null}
      <SnapshotsDialog {shell} />
    {/if}
    {#if shell.discarding !== null}
      <DiscardDialog {shell} question={shell.discarding} />
    {/if}
    {#if shell.identity !== null}
      <IdentityDialog {shell} dialog={shell.identity} />
    {/if}
    {#if shell.branchMenu !== null}
      <BranchMenu {shell} menu={shell.branchMenu} />
    {/if}
    {#if shell.abortAsked}
      <AbortSyncDialog {shell} />
    {/if}
    {#if shell.closeQuestion}
      <CloseQuestionDialog {shell} question={shell.closeQuestion} />
    {/if}
    {#if shell.exporting !== null && shell.exporting.stage !== 'system'}
      <ExportDialog {shell} dialog={shell.exporting} />
    {/if}
    {#if shell.approximated !== null}
      <ApproximatedDialog
        {shell}
        title={shell.approximated.title}
        lines={shell.approximated.lines}
      />
    {/if}
    {#if shell.todoCalendar !== null}
      <TodoCalendar {shell} calendar={shell.todoCalendar} />
    {/if}
    {#if shell.settingsOpen}
      <SettingsWindow {shell} />
    {/if}
    {#if shell.search.open}
      <SearchOverlay {shell} />
    {/if}
    {#if shell.palette.open}
      <PaletteOverlay {shell} />
    {/if}
    <!-- FR-080: the list of link completion, at the link typed. -->
    <LinkCompletion />
  </main>
  <!-- UX review F14: the agents, told politely to screen readers. -->
  <p class="announce" aria-live="polite">{shell.agentAnnouncement}</p>
  <!-- UC-004 BR-018: the active pane of the split view, told politely. -->
  <p class="announce" aria-live="polite">{shell.paneAnnouncement}</p>
  <!-- FR-099, UC-005 A28: corrections and the count of misspelled words. -->
  <p class="announce" aria-live="polite">{shell.spellAnnouncement}</p>
  <StatusBar
    root={shell.workspaceRoot}
    status={shell.status}
    offer={shell.statusOffer}
    exports={shell.statusExports}
    error={shell.statusError}
    busy={shell.statusBusy}
    git={shell.gitBar}
    command={shell.command}
    onBranches={() => void shell.openBranchMenu()}
    onAbort={() => shell.askAbort()}
    onSyncNow={() => void shell.sync('full')}
    onUndo={() => void shell.undoDelete()}
    onUndoReplace={() => void shell.undoReplace()}
    onUndoRestore={() => void shell.undoRestore()}
    onOpenExport={(index) => void shell.openExport(index)}
    onShowApproximated={(index) => void shell.showApproximated(index)}
    agents={shell.agents.menu}
    agentOpens={shell.statusAgents}
    onAgentLevel={(level) => void shell.setAgentAccess(level)}
    onAgentSettings={() => void shell.openSettings('workspace')}
    onOpenAgentNote={(note) => void shell.openAgentNote(note)}
    overdue={shell.workspace === null ? null : (shell.todos?.overdue ?? null)}
    spellProblem={shell.workspace === null ? null : shell.spellState.problem}
    spellDetail={shell.spellState.detail}
    onSpellSettings={() => void shell.openSettings('me')}
    onOverdue={() => void shell.showTodoList('open')}
    onUndoTodo={() => void shell.undoTodo()}
    onWorkspaceSettings={() => void shell.openSettings('workspace')}
  />
</div>

<style>
  .announce {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .window {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    height: 100%;
  }

  /* UC-010 BR-010: without the top bar the notes take its room. */
  .window.focus {
    grid-template-rows: minmax(0, 1fr) auto;
  }

  .central {
    position: relative;
    min-height: 0;
    overflow: auto;
  }

  .workbench {
    display: flex;
    height: 100%;
    min-height: 0;
  }
</style>
